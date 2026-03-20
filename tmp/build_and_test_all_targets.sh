#!/bin/bash
# Build and run all standalone tests for each Ledger target.
# Uses --golden_run to generate/update test frames.

set -e

TARGETS="apex_p nanox nanos+ flex stax"
RUST_APP="./rust-app"
TESTS_DIR="./tests/standalone"

# Install test dependencies once
echo "=== Installing test dependencies ==="
docker exec --user 1000:1000 app-sui-container bash -c \
  'source /opt/venv/bin/activate && [ -f ./tests/standalone/requirements.txt ] && pip install -r ./tests/standalone/requirements.txt'

for TARGET in $TARGETS; do
  echo ""
  echo "=== Target: $TARGET ==="

  echo "Building..."
  docker exec --user 1000:1000 app-sui-container bash -c \
    "cd $RUST_APP && cargo ledger build $TARGET -- -Zunstable-options --out-dir build/$TARGET/bin && \
     mv build/$TARGET/bin/sui build/$TARGET/bin/app.elf && \
     mv build/$TARGET/bin/sui.apdu build/$TARGET/bin/app.apdu"

  echo "Running all standalone tests (--golden_run)..."
  docker exec --user 1000:1000 app-sui-container bash -c \
    "source /opt/venv/bin/activate && pytest $TESTS_DIR --tb=short -v --device $TARGET --golden_run -s"

  echo "Done: $TARGET"
done

echo ""
echo "=== All targets completed ==="
