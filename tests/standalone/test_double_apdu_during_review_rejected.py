# A second APDU arriving while a review is on screen must be rejected outright,
# and must not disturb the command being reviewed.
#
# The app registers its Comm with NBGL (app_main.rs, init_comm) and runs reviews
# synchronously from inside a live APDU future, so the in-flight command owns the
# APDU buffer, the transport type and the RX/TX lengths for the whole duration of the
# review. On ledger_device_sdk 1.36.2, next_event_ahead decoded a second APDU straight
# into that shared state. Since 1.37.0 the raw frame is rejected with CmdNotAccepted
# (0x6901) before any decoding, gated on an apdu_in_progress latch that is set before
# the command reaches app code and cleared only after its reply.
#
# Two host peers are needed to express this, so the reviewed command is driven over
# Speculos' raw APDU socket while the intruder arrives on the HTTP API. Timing is not
# left to a sleep: an APDU that lands between two block-protocol exchanges is not a
# double APDU at all, it is simply the next command, and the device is entitled to
# accept it. The block protocol therefore runs in its own thread and reports the
# exchange the device leaves unanswered — that pause is the review, and it is the
# only window in which the intruder is sent.
#
# Two limits of the emulator, both of which need hardware to close:
#   * Speculos broadcasts every response the SE emits to every connected APDU client
#     (seproxyhal.py, SephTag.RAPDU), so the reviewed command's transport also sees
#     the rejection meant for the intruder. It is dropped below. A device with real
#     per-transport routing would not emit it there at all, which is the part this
#     test cannot reach.
#   * There is no second physical transport, so the USB-command/BLE-intruder case is
#     out of scope here.

import base64
import concurrent.futures
import socket
import threading
from hashlib import sha256

import pytest
import requests

from application_client.client import (
    CLA,
    HostToLedger,
    InsType,
    LedgerToHost,
    P1,
    P2,
    pack_derivation_path,
    pop_size_prefixed_buf_from_buf,
)
from ragger.navigator import NavInsID
from utils import ROOT_SCREENSHOT_PATH, check_signature_validity

SW_OK = 0x9000
# StatusWords::CmdNotAccepted, added in ledger_device_sdk 1.37.0.
SW_CMD_NOT_ACCEPTED = 0x6901

# A well-formed application APDU (GET_VERSION), harmless on its own. What matters is
# that it is a normal CLA — the SDK deliberately lets BOLOS APDUs (CLA 0xB0) through.
INTRUDER_APDU = bytes([CLA, InsType.GET_VERSION, P1, P2, 0x00])

# Bound every wait, so a device that stops answering fails the test instead of
# stalling the suite.
INTRUDER_TIMEOUT_S = 20
RESPONSE_TIMEOUT_S = 30
REVIEW_WAIT_S = 30
# How long an exchange may go unanswered before the device is taken to be waiting on
# the user. Ordinary chunk exchanges answer in milliseconds and are picked up as soon
# as the bytes land, so this only delays the review exchange itself.
REVIEW_DETECT_S = 5.0

# Client.send_with_blocks uses this when splitting a parameter into blocks.
CHUNK_SIZE = 180


class RawApduPeer:
    """A host peer on Speculos' raw APDU socket.

    Frames are `length || payload`, big-endian over 4 bytes, with the length of a
    response counting its data only — the two status bytes follow it.
    """

    def __init__(self, port: int) -> None:
        self._socket = socket.create_connection(("127.0.0.1", port), timeout=10)

    def close(self) -> None:
        self._socket.close()

    def send(self, apdu: bytes) -> None:
        self._socket.sendall(len(apdu).to_bytes(4, "big") + apdu)

    def _read_exactly(self, size: int, timeout: float):
        self._socket.settimeout(timeout)
        buffer = b""
        while len(buffer) < size:
            try:
                chunk = self._socket.recv(size - len(buffer))
            except socket.timeout:
                return None
            if not chunk:
                raise RuntimeError("Speculos closed the raw APDU socket")
            buffer += chunk
        return buffer

    def receive(self, timeout: float = RESPONSE_TIMEOUT_S):
        """Return the next `(data, sw)` frame, or None if none arrives in time."""
        header = self._read_exactly(4, timeout)
        if header is None:
            return None
        # The frame is committed now, so give the remainder its own full timeout.
        body = self._read_exactly(int.from_bytes(header, "big") + 2, RESPONSE_TIMEOUT_S)
        if body is None:
            raise RuntimeError("Speculos sent a truncated APDU frame")
        return body[:-2], int.from_bytes(body[-2:], "big")


@pytest.fixture
def raw_peer(backend):
    """The peer that issues the command the user reviews."""
    if not hasattr(backend, "_apdu_port"):
        pytest.skip("Racing two APDU peers requires the Speculos backend")
    peer = RawApduPeer(backend._apdu_port)
    yield peer
    peer.close()


def _link_blocks(payload_items):
    """Hash-link each parameter into blocks, as Client.send_with_blocks does.

    Returns the START payload and the chunk table the device will pull from.
    """
    table = {}
    parameters = []
    for item in payload_items:
        chunks = [item[i:i + CHUNK_SIZE] for i in range(0, len(item), CHUNK_SIZE)]
        last_hash = b"\x00" * 32
        for chunk in reversed(chunks):
            linked = last_hash + chunk
            last_hash = sha256(linked).digest()
            table[last_hash.hex()] = linked
        parameters.append(last_hash)
    start = bytes([HostToLedger.START]) + b"".join(parameters)
    return start, table


def _exchange_blocks(peer, ins, payload_items, reviewing=None):
    """Run the block protocol on `peer` and return the app's final result.

    When the device leaves an exchange unanswered it is waiting on the user; `reviewing`
    is set so the caller can act during that window, and the reply is then waited for
    without a deadline. Passing None asserts the device never pauses — used for the
    commands that carry no confirmation.

    Everything the test asks of the device goes through this one peer: Speculos
    broadcasts responses to every connected APDU client, so traffic sent on another
    channel would queue up here and desynchronise the exchange.
    """
    payload, table = _link_blocks(payload_items)
    result = b""
    paused = False

    while True:
        peer.send(bytes([CLA, ins, P1, P2, len(payload)]) + payload)
        frame = peer.receive(REVIEW_DETECT_S if not paused else RESPONSE_TIMEOUT_S)

        if frame is None:
            assert reviewing is not None, (
                "device paused for the user during a command that has no review"
            )
            assert not paused, (
                f"device stopped answering for {RESPONSE_TIMEOUT_S}s after the review "
                f"was approved"
            )
            paused = True
            reviewing.set()
            frame = peer.receive()
            assert frame is not None, "device never answered the reviewed command"
            if frame[1] == SW_CMD_NOT_ACCEPTED:
                # Speculos' broadcast of the intruder's rejection (see header). A real
                # device keeps this on the intruder's own transport.
                frame = peer.receive()
                assert frame is not None, (
                    "device never answered the reviewed command after the intruder "
                    "was rejected"
                )

        response, status = frame
        assert status == SW_OK, (
            f"reviewed command answered 0x{status:04x} on its own transport"
        )
        command, body = response[0], response[1:]

        if command == LedgerToHost.RESULT_FINAL:
            assert reviewing is None or paused, (
                "device answered without ever pausing for a review"
            )
            return result + body
        if command == LedgerToHost.RESULT_ACCUMULATING:
            result += body
            payload = bytes([HostToLedger.RESULT_ACCUMULATING_RESPONSE])
        elif command == LedgerToHost.GET_CHUNK:
            chunk = table.get(body.hex())
            if chunk is None:
                payload = bytes([HostToLedger.GET_CHUNK_RESPONSE_FAILURE])
            else:
                payload = bytes([HostToLedger.GET_CHUNK_RESPONSE_SUCCESS]) + chunk
        elif command == LedgerToHost.PUT_CHUNK:
            table[sha256(body).hexdigest()] = body
            payload = bytes([HostToLedger.PUT_CHUNK_RESPONSE])
        else:
            raise RuntimeError(f"Unknown instruction returned from ledger: {command}")


def _send_intruder_apdu(backend, apdu: bytes = INTRUDER_APDU):
    """POST one APDU to Speculos' HTTP API, returning (sw, data).

    This is the second peer: it does not share a connection with the command being
    reviewed, which is the whole point of the exercise.
    """
    with requests.Session() as session:
        response = session.post(
            f"{backend.url}/apdu",
            json={"data": apdu.hex()},
            timeout=INTRUDER_TIMEOUT_S,
        )
        response.raise_for_status()
        raw = bytes.fromhex(response.json()["data"])
    assert len(raw) >= 2, f"Speculos returned a truncated response: {raw.hex()}"
    return int.from_bytes(raw[-2:], "big"), raw[:-2]


def _review_with_intruder(peer, backend, ins, payload_items, approve):
    """Drive `ins` to its review, race an APDU in, approve, and return the result.

    The block protocol has to keep answering while the navigation runs: after the user
    confirms, the app fetches more chunks before it can draw its final screen, and the
    navigator waits for that screen. So the exchange loop gets its own thread, exactly
    as the other UI-driving tests in this suite do.
    """
    reviewing = threading.Event()
    intruder: dict = {}

    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
        exchanges = executor.submit(
            _exchange_blocks, peer, ins, payload_items, reviewing
        )
        try:
            assert reviewing.wait(REVIEW_WAIT_S), (
                "device never paused to display a review"
            )
            try:
                intruder["sw"], intruder["data"] = _send_intruder_apdu(backend)
            except Exception as exc:  # noqa: BLE001 - surfaced by _assert_rejected
                intruder["error"] = exc
            # Approve either way, so a bad intruder result is reported by the
            # assertions rather than by the exchange thread hanging.
            approve()
        except BaseException:
            exchanges.cancel()
            raise
        result = exchanges.result(timeout=RESPONSE_TIMEOUT_S)

    return result, intruder


def _assert_rejected(intruder: dict) -> None:
    assert "error" not in intruder, (
        f"Intruder APDU got no answer while the review was displayed: "
        f"{intruder['error']!r}. Before SDK 1.37.0 it was decoded into the in-flight "
        f"command's buffer instead of being rejected."
    )
    assert intruder["sw"] == SW_CMD_NOT_ACCEPTED, (
        f"Intruder APDU answered 0x{intruder['sw']:04x}, expected "
        f"0x{SW_CMD_NOT_ACCEPTED:04x} (CmdNotAccepted)"
    )
    assert intruder["data"] == b"", (
        f"A rejected APDU must carry no payload, got {intruder['data'].hex()}"
    )


# A second APDU sent while the address confirmation screen is up is rejected, and the
# VERIFY_ADDRESS command it interrupted still returns its own result afterwards.
def test_double_apdu_during_address_review_rejected(
    backend, raw_peer, scenario_navigator, firmware, navigator
):
    path = "m/44'/784'/0'"

    response, intruder = _review_with_intruder(
        raw_peer,
        backend,
        InsType.VERIFY_ADDRESS,
        [pack_derivation_path(path)],
        scenario_navigator.address_review_approve,
    )

    _assert_rejected(intruder)

    # Unchanged from test_get_public_key_confirm_accepted: the intruder must not have
    # perturbed what the approved command returns.
    response, _, public_key = pop_size_prefixed_buf_from_buf(response)
    _, _, address = pop_size_prefixed_buf_from_buf(response)
    assert public_key.hex() == (
        "6fc6f39448ad7af0953b78b16d0f840e6fe718ba4a89384239ff20ed088da2fa"
    )
    assert address.hex() == (
        "56b19e720f3bfa8caaef806afdd5dfaffd0d6ec9476323a14d1638ad734b2ba5"
    )

    # And the rejection left no debris in Comm or in the block-protocol state: an
    # ordinary command on the same transport still works afterwards.
    follow_up = _exchange_blocks(
        raw_peer, InsType.GET_PUBLIC_KEY, [pack_derivation_path(path)]
    )
    _, _, follow_up_key = pop_size_prefixed_buf_from_buf(follow_up)
    assert follow_up_key == public_key


# The same race during a transaction review: the intruding peer must not be able to
# collect the signature for a transaction the user approved on someone else's
# request.
def test_double_apdu_during_tx_review_rejected(
    backend, raw_peer, scenario_navigator, firmware, navigator
):
    path = "m/44'/784'/0'/0'/0'"

    _, _, public_key = pop_size_prefixed_buf_from_buf(
        _exchange_blocks(raw_peer, InsType.GET_PUBLIC_KEY, [pack_derivation_path(path)])
    )
    assert len(public_key) == 32

    # Same whole-gas-coin transfer as test_sign_tx_sui_whole_gas_coin.
    transaction = base64.b64decode(
        "AAAAAAABACAdPyZDMFdgIm5RjJtalhZTg4CN2XeXH3PeqXFUOwvkiAEBAQABAABvsh/urQJ9pIcy"
        "la/9bE82GP4Xb6L78+e17x2UY7MeIQFADb3P7ajh5k679XEMz1pnv6bn+UXJK8hhHgy0S3IZ3tN2"
        "QhEAAAAAIGbFq2VJip03FgAaA0gV/0q8p2X39vI3XMkdKt23nCCKb7If7q0CfaSHMpWv/WxPNhj+"
        "F2+i+/Pnte8dlGOzHiHoAwAAAAAAAOCXLQAAAAAAAA=="
    )
    coin_object = base64.b64decode(
        "AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/u"
        "rQJ9pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9a"
        "sLM9x2ATDwAAAAAA"
    )

    # Same framing as Client.sign_tx: length-prefixed tx, path, then the object list.
    tx_param = len(transaction).to_bytes(4, "little") + transaction
    object_list = (
        (1).to_bytes(4, "little")
        + len(coin_object).to_bytes(4, "little")
        + coin_object
    )

    def approve():
        if firmware.device.startswith("nano"):
            navigator.navigate_and_compare(
                instructions=[
                    NavInsID.RIGHT_CLICK,  # Transfer SUI
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,  # From ...
                    NavInsID.RIGHT_CLICK, NavInsID.RIGHT_CLICK,  # To ...
                    NavInsID.RIGHT_CLICK,  # Amount
                    NavInsID.RIGHT_CLICK,  # Max Gas
                    NavInsID.BOTH_CLICK,
                ],
                timeout=10,
                test_case_name=scenario_navigator.test_name,
                path=scenario_navigator.screenshot_path,
                screen_change_before_first_instruction=True,
                screen_change_after_last_instruction=False,
            )
        else:
            scenario_navigator.review_approve()

    signature, intruder = _review_with_intruder(
        raw_peer,
        backend,
        InsType.SIGN_TX,
        [tx_param, pack_derivation_path(path), object_list],
        approve,
    )

    _assert_rejected(intruder)

    # The signature still belongs to the transaction the user actually reviewed, and
    # it came back on the transport that asked for it.
    assert len(signature) == 64
    assert check_signature_validity(public_key, signature, transaction)
