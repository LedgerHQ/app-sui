import tomli
from pathlib import Path
from application_client.client import Client

# In this test we check the behavior of the device when asked to provide the app version
def test_version(backend):
    # Resolved from this file, so the test does not depend on the working directory.
    cargo_path = Path(__file__).resolve().parents[2] / "rust-app" / "Cargo.toml"

    with open(cargo_path, "rb") as f:
        data = tomli.load(f)

    version = (tuple(map(int, data['package']['version'].split('.'))), "sui")
    # Use the app interface instead of raw interface
    client = Client(backend, use_block_protocol=True)
    # Send the GET_VERSION instruction
    response = client.get_app_and_version()
    assert response == (version)
