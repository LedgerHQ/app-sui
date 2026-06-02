# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Ledger hardware-wallet app for the Sui blockchain, built on the [Alamgu](https://github.com/alamgu/) framework. Embedded Rust (`#![no_std]`) compiled to ARM for Nano S+, Nano X, Flex, Stax, and Apex P. The Rust crate lives in `rust-app/`; Python integration tests live in `tests/`.

`.github/copilot-instructions.md` is an accurate, detailed companion to this file (APDU instruction table, BCS transaction layout, device cfg flags, unstable Rust features). Read it for specifics; this file covers the build/test workflow and the cross-file architecture.

## Build & test

Two environments work. **Nix is the canonical/reproducible path** (used for releases and CI); the prebuilt Docker image is the no-Nix fallback used in practice here.

Devices are `nanosplus`, `nanox`, `flex`, `stax`, `apex_p` (never `nanos` — original Nano S is unsupported).

### Nix
```bash
nix-shell -A flex.rustShell          # device-specific dev shell
cd rust-app && cargo build --release --target=$TARGET_JSON
./run-ragger-tests.sh                # builds + runs ragger tests for all devices
```

### Docker (image: `ghcr.io/ledgerhq/ledger-app-builder/ledger-app-dev-tools:latest`)
Mount the repo root at `/app`. `cargo-ledger` is on PATH; Python tooling is in the `/opt/venv` virtualenv.
```bash
# Build the app (also produces the speculos ELF used by tests at rust-app/target/<device>/release/sui)
docker run --rm -v "$(pwd):/app" -w /app/rust-app <image> bash -c 'cargo ledger build flex'

# Run tests (deps are not persisted across --rm containers; install them per run)
docker run --rm -v "$(pwd):/app" -w /app <image> bash -c '
  source /opt/venv/bin/activate
  cd rust-app && cargo ledger build flex >/dev/null && cd ..
  pip install -q -r tests/standalone/requirements.txt   # or tests/swap/requirements.txt
  cd tests/standalone && python -m pytest <FILE>::<TEST> --device flex'
```

### Test layout (note: there is no `ragger-tests/` dir despite older docs)
- `tests/standalone/` — APDU-level tests against the app alone (sign/transfer/stake/token/swap-params). Run a single one: `pytest tests/standalone/test_x.py::test_fn --device flex`, or filter with `-k`.
- `tests/swap/` — Exchange-integration tests. The app runs as a *sideloaded library* under the Exchange app; the Exchange/Ethereum ELFs are prebuilt under `tests/swap/.test_dependencies/{main,libraries}`, while ragger builds the current `sui` ELF from the repo.
- New tests that drive the device UI need golden snapshots: add `--golden_run` on first run to generate `tests/<suite>/snapshots/<device>/...`, and commit those dirs (per device).

### Crafting object / transaction fixtures (for parser regression tests)
A transaction references coin/stake objects by digest; the matching object bytes are supplied in a separate APDU stream and resolved by `implementation.rs::get_object_data`.
- Object digest = `blake2b-256(b"Object::" + object_bytes)`. An `ImmOrOwnedObject` digest inside the tx BCS is the raw 32-byte hash — locate it and overwrite in place to re-point an input at a crafted object.
- `client.sign_tx(..., object_list=[raw_obj_bytes, ...])` — the Python client frames the list (count + per-item length prefix), so pass raw object bytes.
- A coin object's `StructTag` is `0x00 0x03 0x07 | addr[32] | module(BCS vec) | struct(vec) | type_params(vec) | ...`; mutate names/length there to build collision/overlong cases. See `test_sign_token_overlong_name_rejected.py` and `test_sign_sui_transfer_staked_rejected.py`.

### Writing a swap test
Subclass `ExchangeTestRunner` (`tests/swap/test_sui.py`): set `currency_configuration` to a `CurrencyConfiguration` built from `create_currency_config(ticker, "Sui", (subticker, decimals))` (the subticker is what the device parses as the coin-config ticker; use one absent from `KNOWN_COINS` to exercise the unknown-token path), and implement `perform_final_tx` to build+sign the coin-app tx. Call `client.provide_dynamic_token(ticker, decimals, addr, module, struct)` to supply a signed dynamic descriptor. For a negative case, drive `perform_valid_swap_from_custom(...)` then assert `perform_coin_specific_final_tx(...)` raises `SUI_SWAP_TX_PARAM_MISMATCH`.

### Formatting
`cargo fmt` fails here with "Failed to find targets" (the `[[bin]]` only builds for device targets). Format changed files directly: `rustfmt --edition 2021 <files>` (CI gates on `--check`).

### Lockfile
After changing `Cargo.lock`, run `./update-crate-hashes.sh` to refresh `crate-hashes.json` (Nix supply-chain integrity for git deps).

## Architecture

### Sign flow spans several files — trace it end to end
`handle_apdu.rs` (async dispatch) → `implementation.rs::sign_apdu` reads the tx over the **block protocol** (chunked, SHA-256-verified transfer with a `usize` length prefix — `docs/block-protocol.md`; skip the prefix before parsing) → `parser/tx.rs` parses BCS into a `KnownTx` (Transfer/Stake/Unstake) → either `prompt_tx_params` (normal) or `swap::check_tx_params` (swap) → blind-sign fallback when the tx is unrecognized. Token/coin object data referenced by a tx is fetched lazily via the `HasObjectData` trait (`parser/common.rs`), backed by a third APDU input stream and parsed in `parser/object.rs`.

### `CoinType` is the canonical asset identity
`CoinType = (package_id[32], module[32], type_name[32])` (`parser/common.rs`). It drives parsing, amount aggregation, swap matching, and UI display. Names are zero-padded to `COIN_STRING_LENGTH`; the parser **rejects** names longer than that rather than truncating (truncation would let distinct tokens collide on a shared prefix). `KNOWN_COINS` (a static table in `ui/common.rs`) maps coin types to tickers; unknown tickers are resolved only via a signed dynamic token descriptor (`ProvideTrustedDynamicDescriptor` APDU → `ctx.rs`).

### Swap is the security-critical trust boundary
In swap mode the device signs **without the normal confirmation UI**, trusting `swap/mod.rs::check_tx_params` to match the host-supplied transaction against partner-signed swap parameters (`swap/params.rs`). Treat any parser-derived classification used here as security-relevant: a parsing ambiguity becomes a "sign the wrong thing silently" bug. The established fail-safe is to **reject (return `None` / `reject_on`) on any ambiguity**, which routes the tx to the not-recognized/blind-sign path and makes swaps reject outright.

### Device targeting
Heavy `cfg` use selects UI and entry points: NBGL (`ui/nbgl.rs`) for `stax`/`flex`/`apex_p`, BAGL for `nanosplus`/`nanox`; entry split as `main_stax.rs` vs `main_nanos.rs`, re-exported through `ui.rs`. `target_family = "bolos"` = any device.

### Embedded constraints
`#![no_std]`; use `ArrayVec`, not `Vec`; avoid heap in hot paths. Async is cooperative via `alamgu-async-block` (no runtime); handlers return opaque `impl Future`. Panics exit the app. Enable emulator logging with `--features speculos,ledger-log/log_info` (or `extra_debug` for trace) — release builds emit no logs.

## Conventions

- Commit subjects on fix branches follow `Fix <problem statement>` (e.g. `Fix Swap transaction coin type can fall back to SUI for unknown token tickers`).
- For a parser/security change, add a regression test and verify it actually discriminates: confirm it passes with the fix and **fails with the fix reverted** (`git stash` the source change, rebuild, rerun — the test file is untracked so it survives the stash). A test that also passes on the buggy code is worthless.
- Stray local artifacts in the repo root (`*.apdu`, `out.json`, `apdu_TestsSui.log`, `.DS_Store`) are not tracked — keep them out of commits.
