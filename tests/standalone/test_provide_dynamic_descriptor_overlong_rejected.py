# Security regression for audit finding LOW-6: PROVIDE_TRUSTED_DYNAMIC_DESCRIPTOR
# TLV length not bounds-checked.
#
# The descriptor is copied into a 1024-byte buffer, and the copy ignored the
# capacity error, so a declared length above 1024 silently dropped the tail and the
# parser saw a different byte string than the host sent. Not exploitable on its own,
# since the signature covers the full TLV, but the app must refuse what it cannot
# hold instead of parsing a truncation.
#
# A valid descriptor padded to 1100 bytes must be refused with the TLV error range
# (0x7000 + TlvError::UnexpectedEof) before any parsing. Without the check the
# truncated bytes reach the parser and the status is some other value.

import pytest

from application_client.client import Client
from ragger.error import ExceptionRAPDU

TLV_UNEXPECTED_EOF = 0x7000  # TLV_ERROR_OFFSET + TlvError::UnexpectedEof


def test_provide_dynamic_descriptor_overlong_rejected(backend, firmware):
    client = Client(backend, use_block_protocol=True)

    with pytest.raises(ExceptionRAPDU) as e:
        client.provide_dynamic_token(
            "DEEP", 6,
            "0xdeeb7a4662eec9f2f3def03fb937a663dddaa2e215b8078a284d026b7946c270",
            "deep", "DEEP",
            trailing_padding=1100)

    assert e.value.status == TLV_UNEXPECTED_EOF
