#!/usr/bin/env bash
set -eu

OUT_DIR=/app/docker-outputs

for device in nanosplus nanox flex stax
do
    cd rust-app
    cargo ledger build $device
    cd ..
    pytest ragger-tests --tb=short -v --device ${device/nanosplus/nanosp};
    chown -R $HOST_UID:$HOST_GID rust-app/target/ ragger-tests/
    cp rust-app/target/$device/release/$APP_NAME $OUT_DIR/$device
done
