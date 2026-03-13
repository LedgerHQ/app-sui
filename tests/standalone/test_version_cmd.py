import tomli
from pathlib import Path
from application_client.client import Client

# In this test we check the behavior of the device when asked to provide the app version
def test_version(backend):
    cargo_path = Path("./rust-app/Cargo.toml")

    if not cargo_path.exists():
        cargo_path = Path("./Cargo.toml")

    if not cargo_path.exists():
        raise FileNotFoundError("Cargo.toml not found")

    with open(cargo_path, "rb") as f:
        data = tomli.load(f)

    version = (tuple(map(int, data['package']['version'].split('.'))), "sui")
    # Use the app interface instead of raw interface
    client = Client(backend, use_block_protocol=True)
    # Send the GET_VERSION instruction
    response = client.get_app_and_version()
    assert response == (version)
