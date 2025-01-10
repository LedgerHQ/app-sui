#!/usr/bin/env bash
set -eu

export APP_NAME=`grep name rust-app/Cargo.toml | cut -d '"' -f2 | head -n1`

OUT_DIR="./docker-outputs"
for device in nanosplus nanox flex stax
do
    mkdir -p $OUT_DIR/$device
done

# Build apps using nightly
docker run \
  --env APP_NAME \
  --env HOST_UID=$(id -u) \
  --env HOST_GID=$(id -g) \
  --rm -ti -v "$(realpath .):/app" \
  ghcr.io/ledgerhq/ledger-app-builder/ledger-app-dev-tools:latest \
  docker/run-build-in-docker.sh

# Create app.hex
for device in nanosplus nanox flex stax
do
    cp rust-app/Cargo.toml $OUT_DIR/$device/
    cp rust-app/*.gif $OUT_DIR/$device/
    nix-shell -A alamgu.perDevice.$device.rustShell --run "cd $OUT_DIR/$device; cargo ledger --use-prebuilt $APP_NAME build $device"
done

echo "Use the following commands to install app"
echo 'nix-shell -A alamgu.rustShell --run "cd docker-outputs/nanox; ledgerctl install -f app_nanox.json"'
echo 'nix-shell -A alamgu.rustShell --run "cd docker-outputs/nanosplus; ledgerctl install -f app_nanosplus.json"'
echo 'nix-shell -A alamgu.rustShell --run "cd docker-outputs/flex; ledgerctl install -f app_flex.json"'
echo 'nix-shell -A alamgu.rustShell --run "cd docker-outputs/stax; ledgerctl install -f app_stax.json"'
