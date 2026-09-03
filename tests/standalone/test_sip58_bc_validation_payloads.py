# SIP-58 device validation payloads (BCS `IntentMessage<TransactionData>` for signing).
#
# Golden snapshots for this file only (inside container, cwd /app), e.g.:
#   source /opt/venv/bin/activate && pytest ./tests/standalone/test_sip58_bc_validation_payloads.py --tb=short -v --device nanosp --golden_run -s
#
# Source: coin-sui / @mysten/sui@2.9.0 (2026-03-18). Sender for all txs:
#   0x33444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e0164
#
# Pass `signTransaction(intent_message_bcs, objects)` where the payload is BCS
# `IntentMessage { intent: sui_transaction(), value: TransactionData }` (3-byte intent + tx).
# Use empty `objects` for SIP-58 rows with no coin inputs.
#
# If `check_signature_validity` fails, the emulator seed does not match the key used
# to build the fixtures—regenerate payloads for your test seed or align Speculos.

from application_client.client import Client
from ragger.navigator import NavInsID
from utils import check_signature_validity, run_apdu_and_nav_tasks_concurrently

_PATH = "m/44'/784'/0'/0'/0'"

# BCS `Intent` prefix for signing: must match `Intent::sui_transaction()` (scope=TransactionData,
# version=V0, app_id=Sui). The device parses `IntentMessage = (Intent, TransactionData)`; do not pass
# raw `TransactionData` only — the first bytes would be mis-read as intent and parsing fails.
_INTENT_SUI_TX = bytes([0, 0, 0])


def _intent_message(tx_data: bytes) -> bytes:
    return _INTENT_SUI_TX + tx_data


# --- 1. Standard SUI transfer (baseline: gas from coin objects) ---

_TX_STANDARD_TRANSFER_UNSIGNED = bytes.fromhex(
    "000002000800e1f5050000000000206e143fe0a8ca010a86580dafac44298e5b1b7d73efc345356a59a15f0d7824f0020200010100000101030000000001010033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e0164029ed951d7cc89f1bd2c6bb7067b854572cf510382b9c1480b487a85fc47550d8a2dcb8c22000000002052e101e20014b7956b4a436372a9efc34b93527bc6b2c36a89cf4360c23b3dcf22325369d637252301e7818ccc3aeffd17fac500f82aec94fac7093c3e359609d67f26210000000020c2ae921a27878edd38d7a6a92b8f398863ea0d6f6f1268409146d4cfd089f69133444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01642b02000000000000b0162f00000000000201340400000000000001350400000000000000002035834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499bb746a7f0"
)

_TX_STANDARD_TRANSFER = _intent_message(_TX_STANDARD_TRANSFER_UNSIGNED)

_OBJECTS_STANDARD_TWO = [
    bytes.fromhex(
        "0001012dcb8c2200000000289ed951d7cc89f1bd2c6bb7067b854572cf510382b9c1480b487a85fc47550d8a00c2eb0b000000000033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e016420210ca182d0f0b99f42fb6ff2be74627c6f631df95f8e0f027e529107aa95289560130f0000000000"
    ),
    bytes.fromhex(
        "000101d67f2621000000002822325369d637252301e7818ccc3aeffd17fac500f82aec94fac7093c3e359609eca91201000000000033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e016420005fd53cf8409229002b4487fbf69536a3c26b37f139e0c20303a837728c71e960130f0000000000"
    ),
]

# --- 2. SIP-58 SUI transfer (empty gas payment) ---

_TX_SIP58_TRANSFER_UNSIGNED = bytes.fromhex(
    "000002000800e1f5050000000000206e143fe0a8ca010a86580dafac44298e5b1b7d73efc345356a59a15f0d7824f0020200010100000101030000000001010033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01640033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01642b0200000000000080969800000000000201340400000000000001350400000000000000002035834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499ba08e848f"
)

_TX_SIP58_TRANSFER = _intent_message(_TX_SIP58_TRANSFER_UNSIGNED)

# --- 3. SIP-58 delegate stake (empty gas payment) ---

_TX_SIP58_DELEGATE_STAKE_UNSIGNED = bytes.fromhex(
    "000003000800ca9a3b00000000010100000000000000000000000000000000000000000000000000000000000000050100000000000000010020cb7530490045f19514eed2f7efa4bca56854e54470fa23e8c91c46eb8a78d72f020200010100000000000000000000000000000000000000000000000000000000000000000000030a7375695f73797374656d11726571756573745f6164645f7374616b65000301010002000001020033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01640033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01642b0200000000000000e1f505000000000201340400000000000001350400000000000000002035834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499b3de0e361"
)

_TX_SIP58_DELEGATE_STAKE = _intent_message(_TX_SIP58_DELEGATE_STAKE_UNSIGNED)

# --- 4. Side-by-side: same 0.05 SUI transfer, standard vs SIP-58 ---

_TX_STANDARD_005_UNSIGNED = bytes.fromhex(
    "000002000880f0fa020000000000206e143fe0a8ca010a86580dafac44298e5b1b7d73efc345356a59a15f0d7824f0020200010100000101030000000001010033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e0164029ed951d7cc89f1bd2c6bb7067b854572cf510382b9c1480b487a85fc47550d8a2dcb8c22000000002052e101e20014b7956b4a436372a9efc34b93527bc6b2c36a89cf4360c23b3dcf22325369d637252301e7818ccc3aeffd17fac500f82aec94fac7093c3e359609d67f26210000000020c2ae921a27878edd38d7a6a92b8f398863ea0d6f6f1268409146d4cfd089f69133444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01642b0200000000000080969800000000000201340400000000000001350400000000000000002035834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499b18336e62"
)

_TX_STANDARD_005 = _intent_message(_TX_STANDARD_005_UNSIGNED)

_TX_SIP58_005_UNSIGNED = bytes.fromhex(
    "000002000880f0fa020000000000206e143fe0a8ca010a86580dafac44298e5b1b7d73efc345356a59a15f0d7824f0020200010100000101030000000001010033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01640033444cf803c690db96527cec67e3c9ab512596f4ba2d4eace43f0b4f716e01642b0200000000000080969800000000000201340400000000000001350400000000000000002035834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499b5697bca8"
)

_TX_SIP58_005 = _intent_message(_TX_SIP58_005_UNSIGNED)


def _nav_transfer_nano(navigator, scenario_navigator):
    navigator.navigate_until_text_and_compare(
        NavInsID.RIGHT_CLICK,
        [NavInsID.BOTH_CLICK],
        "Sign transaction",
        scenario_navigator.screenshot_path,
        scenario_navigator.test_name,
        screen_change_after_last_instruction=False,
    )


def _nav_stake_nano(navigator, scenario_navigator):
    navigator.navigate_until_text_and_compare(
        NavInsID.RIGHT_CLICK,
        [NavInsID.BOTH_CLICK],
        "Sign transaction",
        scenario_navigator.screenshot_path,
        scenario_navigator.test_name,
        screen_change_after_last_instruction=False,
    )


def _run_sign(
    backend,
    scenario_navigator,
    firmware,
    navigator,
    transaction: bytes,
    object_list: list,
):
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, _ = client.get_public_key(path=_PATH)
    assert len(public_key) == 32

    def apdu_task():
        return client.sign_tx(path=_PATH, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            _nav_transfer_nano(navigator, scenario_navigator)
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)


def _run_sign_stake(
    backend,
    scenario_navigator,
    firmware,
    navigator,
    transaction: bytes,
    object_list: list,
):
    client = Client(backend, use_block_protocol=True)
    _, public_key, _, _ = client.get_public_key(path=_PATH)
    assert len(public_key) == 32

    def apdu_task():
        return client.sign_tx(path=_PATH, transaction=transaction, object_list=object_list)

    def nav_task():
        if firmware.device.startswith("nano"):
            _nav_stake_nano(navigator, scenario_navigator)
        else:
            scenario_navigator.review_approve()

    def check_result(result):
        assert len(result) == 64
        assert check_signature_validity(public_key, result, transaction)

    run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result)




def test_sip58_bc_standard_sui_transfer_two_gas_coins(
    backend, scenario_navigator, firmware, navigator
):
    """Baseline: 0.1 SUI transfer with two gas coin object refs (non–SIP-58 gas)."""
    _run_sign(
        backend,
        scenario_navigator,
        firmware,
        navigator,
        _TX_STANDARD_TRANSFER,
        _OBJECTS_STANDARD_TWO,
    )


def test_sip58_bc_sip58_transfer_empty_gas_payment(
    backend, scenario_navigator, firmware, navigator
):
    """SIP-58: 0.1 SUI transfer, gas_data.payment = [], empty object list."""
    _run_sign(
        backend,
        scenario_navigator,
        firmware,
        navigator,
        _TX_SIP58_TRANSFER,
        [],
    )


def test_sip58_bc_sip58_delegate_stake_empty_gas_payment(
    backend, scenario_navigator, firmware, navigator
):
    """SIP-58: delegate stake, gas_data.payment = [], empty object list."""
    _run_sign_stake(
        backend,
        scenario_navigator,
        firmware,
        navigator,
        _TX_SIP58_DELEGATE_STAKE,
        [],
    )


def test_sip58_bc_standard_transfer_005_sui_two_gas_coins(
    backend, scenario_navigator, firmware, navigator
):
    """0.05 SUI transfer with traditional gas coins (comparison set)."""
    _run_sign(
        backend,
        scenario_navigator,
        firmware,
        navigator,
        _TX_STANDARD_005,
        _OBJECTS_STANDARD_TWO,
    )


def test_sip58_bc_sip58_transfer_005_sui_empty_gas_payment(
    backend, scenario_navigator, firmware, navigator
):
    """Same logical 0.05 SUI transfer as standard_005, SIP-58 empty gas payment."""
    _run_sign(
        backend,
        scenario_navigator,
        firmware,
        navigator,
        _TX_SIP58_005,
        [],
    )
