import pytest
import concurrent.futures
import time

from pathlib import Path
from hashlib import blake2b
from hashlib import sha256

from ecdsa.curves import Ed25519
from ecdsa.keys import VerifyingKey


ROOT_SCREENSHOT_PATH = Path(__file__).parent.resolve()


# Check if a signature of a given message is valid
def check_signature_validity(public_key: bytes, signature: bytes, message: bytes) -> bool:
    pk: VerifyingKey = VerifyingKey.from_string(
        public_key,
        curve=Ed25519,
    )
    hash_object = blake2b(digest_size=32)
    hash_object.update(message)
    return pk.verify(
        signature=signature,
        data=hash_object.digest()
    )

# Run APDU and navigation tasks concurrently
def run_apdu_and_nav_tasks_concurrently(apdu_task, nav_task, check_result):
    executor = concurrent.futures.ThreadPoolExecutor(max_workers=2)
    future_apdu = executor.submit(apdu_task)

    # Submit nav_task after a delay
    future_nav = executor.submit(lambda: time.sleep(2) or nav_task())

    try:
        # Wait for both futures to complete
        done, not_done = concurrent.futures.wait([future_apdu, future_nav], timeout=30, return_when=concurrent.futures.FIRST_EXCEPTION)
        print("DEBUG: run_apdu_and_nav_tasks_concurrently, after wait")

        # Check if apdu_task completed successfully
        if future_apdu.done():
            result = future_apdu.result()
            check_result(result)
        else:
            for future in done:
                try:
                    future.result()
                except Exception as e:
                    raise

    except concurrent.futures.TimeoutError:
        print("DEBUG: run_apdu_and_nav_tasks_concurrently, TimeoutError")
        # Cancel both tasks
        future_apdu.cancel()
        future_nav.cancel()
        pytest.fail("Timeout")

    except Exception as e:
        print("DEBUG: run_apdu_and_nav_tasks_concurrently, Exception")
        raise
