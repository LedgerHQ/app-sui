import sys
from pathlib import Path

import pytest

# Add parent tests directory so 'application_client' imports; add this dir for standalone helpers.
_tests_root = Path(__file__).parent.parent.resolve()
_standalone = Path(__file__).parent.resolve()
sys.path.insert(0, str(_tests_root))
sys.path.insert(0, str(_standalone))

# Tee Speculos stderr before Ragger loads (emulator starts with inherited fds otherwise).
import speculos_stderr_tee

speculos_stderr_tee.install()

from ragger.conftest import configuration
from ragger.navigator import NavInsID

###########################
### CONFIGURATION START ###
###########################

# You can configure optional parameters by overriding the value of ragger.configuration.OPTIONAL_CONFIGURATION
# Please refer to ragger/conftest/configuration.py for their descriptions and accepted values

#########################
### CONFIGURATION END ###
#########################

# Pull all features from the base ragger conftest using the overridden configuration
pytest_plugins = ("ragger.conftest.base_conftest", )


def pytest_configure(config: pytest.Config) -> None:
    # pysui: DeprecationWarning noise (e.g. sui_builder / GraphQL transition since 0.65.0)
    config.addinivalue_line(
        "filterwarnings",
        r"ignore::DeprecationWarning:pysui\..*",
    )
    # ragger: Firmware vs ledgered.devices migration (ragger.firmware.structs)
    config.addinivalue_line(
        "filterwarnings",
        r"ignore:.*ragger\.firmware\.Firmware.*deprecated.*:UserWarning",
    )


def pytest_unconfigure(config: pytest.Config) -> None:
    speculos_stderr_tee.uninstall()

# Notes :
# 1. Remove this fixture once the pending review screen is removed from the app
# 2. This fixture clears the pending review screen before each test
# 3. The scope should be the same as the one configured by BACKEND_SCOPE in 
# ragger/conftest/configuration.py
# @pytest.fixture(scope="class", autouse=True)
# def clear_pending_review(firmware, navigator):
#     # Press a button to clear the pending review
#     if firmware.device.startswith("nano"):
#         print("Clearing pending review")
#         instructions = [
#             NavInsID.BOTH_CLICK,
#         ]
#         navigator.navigate(instructions,screen_change_before_first_instruction=False)
