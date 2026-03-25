#!/usr/bin/env bash
# Build app for all targets and run golden snapshot tests (USDC empty gas payment + app mainmenu).
# Installs deps once, then builds & tests each target in its own container in parallel.

set -eu

TARGETS="apex_p nanox nanosplus flex stax"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

log() { echo "[$(date '+%H:%M:%S')] $*"; }
log_target() { echo "[$(date '+%H:%M:%S')] [$1] $2"; }

log "Project root: $PROJECT_ROOT"

if docker inspect app-sui-container &>/dev/null; then
    IMAGE=$(docker inspect app-sui-container --format '{{.Config.Image}}')
    log "Using image from app-sui-container: $IMAGE"
else
    IMAGE="ghcr.io/ledgerhq/ledger-app-builder/ledger-app-dev-tools:latest"
    log "Using default image: $IMAGE"
fi

device_for_pytest() { case "$1" in nanosplus) echo nanosp ;; *) echo "$1" ;; esac; }
container_name() { echo "app-sui-golden-$1"; }

ensure_container() {
    local target=$1 name
    name=$(container_name "$target")
    if ! docker inspect "$name" &>/dev/null; then
        log_target "$target" "Creating container $name..."
        docker create --name "$name" \
            --user 1000:1000 \
            -v "$PROJECT_ROOT:/app" \
            -w /app \
            -t \
            -e PYTHONUNBUFFERED=1 \
            -e CARGO_TARGET_DIR=/app/rust-app/target/"$target" \
            "$IMAGE" \
            sleep infinity >/dev/null
    fi
    if [ "$(docker inspect -f '{{.State.Running}}' "$name")" != "true" ]; then
        docker start "$name" >/dev/null
    fi
}

# --- Ensure all containers exist and are running ---
for TARGET in $TARGETS; do
    ensure_container "$TARGET"
done

# --- Install deps in first container, copy venv to the rest ---
FIRST_TARGET=$(echo "$TARGETS" | awk '{print $1}')
FIRST_CNAME=$(container_name "$FIRST_TARGET")

if ! docker exec "$FIRST_CNAME" test -f /tmp/.deps-installed; then
    log "Installing deps in $FIRST_CNAME..."
    docker exec "$FIRST_CNAME" bash -c '
        source /opt/venv/bin/activate && pip install -q -r ./tests/standalone/requirements.txt && touch /tmp/.deps-installed
    '
    log "Copying venv to other containers..."
    for TARGET in $TARGETS; do
        [ "$TARGET" = "$FIRST_TARGET" ] && continue
        CNAME=$(container_name "$TARGET")
        docker cp "$FIRST_CNAME:/opt/venv" - | docker cp - "$CNAME:/opt/"
        docker exec "$CNAME" touch /tmp/.deps-installed
        log_target "$TARGET" "venv copied"
    done
    log "All containers ready"
else
    log "Deps already installed, skipping"
fi

# --- Build & test each target in parallel ---
for TARGET in $TARGETS; do
    (
        DEVICE=$(device_for_pytest "$TARGET")
        CNAME=$(container_name "$TARGET")
        log_target "$TARGET" "Using container $CNAME"

        docker exec "$CNAME" bash -c '
                set -e
                TAG='"$TARGET"'
                log() { echo "[$(date +%H:%M:%S)] [$TAG] $*"; }

                DEVICE='"$DEVICE"'
                log "Building..."
                cd ./rust-app/
                cargo ledger build '"$TARGET"' -- -Zunstable-options --out-dir build/$DEVICE/bin
                mv build/$DEVICE/bin/sui build/$DEVICE/bin/app.elf
                mv build/$DEVICE/bin/sui.apdu build/$DEVICE/bin/app.apdu

                log "Running all tests..."
                cd /app
                /opt/venv/bin/pytest ./tests/standalone/ --tb=short -v --device $DEVICE --golden_run -s
                # To run individual tests instead:
                # /opt/venv/bin/pytest ./tests/standalone/test_sign_token_transfer_1.py::test_sign_tx_usdc_empty_gas_payment_sip58 --tb=short -v --device $DEVICE --golden_run -s
                # /opt/venv/bin/pytest ./tests/standalone/test_app_mainmenu.py::test_app_mainmenu --tb=short -v --device $DEVICE --golden_run -s

                log "All tests passed"
            ' 2>&1 && log_target "$TARGET" "Done" || { log_target "$TARGET" "FAILED"; exit 1; }
    ) &
done

log "Waiting for all targets..."
wait

log "=== All targets completed ==="
