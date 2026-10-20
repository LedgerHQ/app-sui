# Plan — app-hashi: Hashi BTC deposit derived app ([B2CA-2948](https://ledgerhq.atlassian.net/browse/B2CA-2948))

> The companion app-sui plan ([B2CA-2945](https://ledgerhq.atlassian.net/browse/B2CA-2945)) is saved at [`docs/hashi-clear-signing-plan.md`](hashi-clear-signing-plan.md).

## Context
A Hashi deposit is a plain Bitcoin transaction that pays a **per-user P2TR address**. That address is derived from two on-chain keys (the MPC master key `P` and the Guardian key `G_btc`) and the 32-byte Sui address that will receive hBTC.
- The user has **no recovery path**: the recovery leaf is MPC-only after 60 days. A payment to a wrong address is lost.
- The standard Bitcoin app shows only a `tb1p…`/`bc1p…` address. The user cannot tell that it is a Hashi deposit, nor which Sui account gets credited.

[B2CA-2948](https://ledgerhq.atlassian.net/browse/B2CA-2948) asks for a **derived Bitcoin app** that recomputes that address on device from pinned keys and a host-supplied Sui recipient, and shows the recipient before signing. It is forked from `app-btcext-boilerplate`, which runs on `app-bitcoin` branch `baseapp`.

**Specs:** [`blockchain-integration-ai@b7e0584 integrations/hashi/`](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi)
- [T-APP-01](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/T-APP-01-hashi-init-derived-app.md): skeleton
- [T-APP-02](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/T-APP-02-hashi-deposit-verification.md): deposit verification
- [T-APP-04](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/T-APP-04-hashi-app-database.md): ledger-app-database
- Hashi knowledge base: [`deposit-address-derivation.md`](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/docs/deposit-address-derivation.md)

**Decisions (user):**
- Planning only for now; implementation starts once the Embedded Apps team creates `app-hashi` / `app-hashi-dev`.
- HMAC-SHA3-256 is built **in-app as an isolated module**, tested against vectors and flagged for audit.
- Any PSBT that is not exactly a Hashi deposit is **rejected**, as Babylon Vault does.

**Already decided upstream:**
- MPC and Guardian keys are pinned per network in the binary. Move enforces that the MPC key never changes after DKG, and the Guardian key is write-once and not governable ([`deposit-address-derivation.md` §4](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/docs/deposit-address-derivation.md#4-key-stability)).
- The Sui recipient arrives in a PSBT proprietary output field (`0xFC`), not a custom APDU, so it crosses WalletConnect and Ledger Wallet unchanged.

## Key findings that shape the design
- **Closest precedent: Babylon Vault** (`../app-babylon-vault-dev`).
  - It reads a `0xFC` proprietary key (`src/bip322.c:10-11`, read at `sign_psbt_validate.c:2827`).
  - It rebuilds a NUMS-internal-key P2TR on device and `memcmp`s it against the output (`vault_script.c:141-214`, `_validate_prepegin` `sign_psbt_validate.c:511-735`).
  - It rejects every unrecognised shape (`:3580-3616`).
  - It has cmocka unit tests that mock `cx` with real libsecp256k1 (`unit-tests/`) and fuzzing.
- **app-core (`../app-core-dev`) shows the Makefile and variant pattern.** Do not copy its lenient fallback: it shows only "Fees" for unknown transactions (`main.c:281`).
- **Hooks (base app):**
  - `validate_and_display_transaction(dc, st, internal_inputs, internal_outputs)` (`sign_psbt.c:45`). Returning `false` means an SW has already been sent.
  - `sign_custom_inputs` and `custom_apdu_handler` stay at their defaults.
  - Internal inputs are signed by the base app only when a wallet policy is present.
- **PSBT reads:**
  - `call_get_merkleized_map(dc, st->outputs_root, st->n_outputs, i, &map)` and `call_get_merkleized_map_value(...)`.
  - `psbt_get_output_script` / `psbt_get_output_amount`.
  - **The host can suppress a key, and absence is never proved** (`map_value_status.h`). A missing recipient field must therefore reject.
- **Crypto available in the base app** (`crypto.h`): `crypto_tr_tapleaf_hash_init`, `crypto_tr_combine_taptree_hashes`, `crypto_tr_tweak_pubkey(pk, merkle_root, 32, &parity, out)`, `crypto_tr_lift_x`, `secp256k1_point_unsafe`.
- **Crypto available in the SDK:** `cx_sha3_init_no_throw`, `cx_math_modm_no_throw`, `cx_ecfp_add_point_no_throw`, `cx_ecfp_scalar_mult_no_throw`.
  - `cx_hmac`/`cx_hkdf` reject SHA3 (fail silently), hence the in-app HMAC.
  - Address display: `get_script_address` / `segwit_addr_encode` with `COIN_NATIVE_SEGWIT_PREFIX`.
- **The Hashi SDK builds no PSBT** (deposit flow: "send BTC from any wallet"). The `0xFC` recipient field is a new Ledger-defined convention that the host (Ledger Wallet / WC live app, CUSTOM-2/3) must add. The key layout must be agreed with the dApp and Embedded Apps teams ([integration-plan UNCERTAIN #1](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/integration-plan.md?plain=1#L282)).
- **Testnet drift.** As of 2026-10-08, ts-sdks `NETWORK_CONFIG.testnet` points to a **new** Hashi object `0x342e9080…d1bb` (type package `0x5e0b99c3…8485`), not `0x22c0ce66…`.
  - Live keys on the new object: `mpc_public_key` = `cb7bacef…d7f800` (arkworks), Guardian = `7a708f47…7cd3`.
  - Re-read both from chain at implementation time and record object id, version and epoch for traceability.
  - The same drift affected the app-sui pinned testnet ids; app-sui now pins the new deployment (`@mysten/hashi` 0.6.26).

## Design

### Repo skeleton (T-APP-01)
- **Fork.** Fork `app-btcext-boilerplate@74e22bc` and keep its `bitcoin_app_base` submodule as is: public `app-bitcoin`, branch `baseapp`, at `78243239`. That is the same pin as `app-core-dev` and `app-babylon-vault-dev`. The boilerplate commit "Switching to app-bitcoin-dev@baseapp@v2.5.1" only moved that pointer; `.gitmodules` still names the public repo.
- **`Makefile` variants** (app-core / Babylon pattern):
  - `VARIANT_VALUES = hashi hashi_testnet`.
  - `APPNAME` is "Hashi" / "Hashi Testnet"; final names are a product decision.
  - `BITCOIN_NETWORK = mainnet|testnet`.
  - `DEFINES += HASHI_MAINNET | HASHI_TESTNET`, with a `#error` guard in `src/hashi_constants.h`. Testnet means signet with the `tb` HRP.
  - No extra derivation path. Optionally override the ticker after `include bitcoin_app_base/Makefile`, as Babylon does at `:83-85`.
- **`ledger_app.toml`:** devices `nanox, nanos+, flex, stax, apex_p`. No Nano S.
- **Cleanup:** remove the boilerplate's Foo magic input and XOR APDU (`custom_apdu_handler` returns `false`).
- **Docs and assets:** icons per target, `README.md`, `APP_SPECIFICATION.md` (protocol, trust anchors, non-deposit = reject, HMAC decision).
- **CI** (from Babylon):
  - build + ragger for both variants;
  - clang-format, codespell, guidelines enforcer;
  - the hardened `check_baseapp_submodule.yml`;
  - `unit_tests.yml` (cmocka);
  - optional ClusterFuzzLite.

### Source modules (`src/`)
| File | Content |
|---|---|
| `hashi_constants.h` | Per-network pinned `HASHI_MPC_PUBKEY_SEC1[33]` (converted from arkworks once, offline, with a comment giving the source object, version and epoch) and `HASHI_GUARDIAN_XONLY[32]`. Shared `HASHI_NUMS_XONLY`, `HASHI_CSV_SEQ = 0x0040278D`, and the proprietary key bytes `{0xFC, 0x05,'h','a','s','h','i', 0x00}` (layout TBD) |
| `hmac_sha3.c/.h` | `hkdf_sha3_256(ikm, ikm_len, okm, 64)`: HMAC-SHA3-256 per RFC 2104 with a 136-byte block, empty salt (= 32 zero bytes, matching RFC 5869), empty info. Uses `cx_sha3_*_no_throw` and `explicit_bzero` on intermediates. Nothing else in the app touches it |
| `hashi_derive.c/.h` | `hashi_child_xonly(P_sec1, sui_addr, out)`: `tweak = HKDF(x_be(P) ‖ addr) mod n` (via `cx_math_modm_no_throw`), then `C = P + tweak·G`, keeping P's parity. `hashi_deposit_spk(sui_addr, spk[34])`: build both leaves byte-exact (`20 G ac 20 h ba 52 9c` and `03 8d 27 40 b2 69 20 h ac`), TapLeaf `0xc0`, sorted TapBranch, `crypto_tr_tweak_pubkey(NUMS, root)`, then `51 20 <Q>`. Uses `_no_throw` everywhere; any error returns false |
| `hashi_validate.c` | Overrides `validate_and_display_transaction` (rules below) |
| `display.c/.h` | NBGL review, following Babylon `display.c` (`nbgl_useCaseReview(TYPE_TRANSACTION, …)` + `io_ui_process`) |

### Validation rules (`hashi_validate.c`, fail closed, `SW_INCORRECT_DATA`)
1. A wallet policy is present, and **every** input is internal (`bitvector_get(internal_inputs, i)`, as in Babylon `:532-537`). `!st->external_amount_unverified`, and no open sighash (`sighash_inputs_open` / `sighash_outputs_open`).
2. `st->n_external_outputs == 1`, and every other output is internal change. The base app already caps change at 10 and rejects a negative fee.
3. For the external output:
   - the script is exactly 34 bytes, `0x51 0x20 …` (P2TR);
   - the `0xFC` recipient value is present and **exactly 32 bytes** (absent or any other length rejects);
   - `hashi_deposit_spk(recipient)` must `memcmp`-equal the script.
4. Optional (decided with product, [integration-plan UNCERTAIN #10](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/integration-plan.md?plain=1#L293)): reject an amount below a pinned floor of 546 sat (dust). The governable 30 000 sat minimum cannot be read on device, so the screen shows a note instead.
5. Anything else rejects. There is no blind or standard fallback.

### Review screens (portal `transactions.mdx`)
Title: "Review transaction to deposit BTC to Hashi".

| Field | Content |
|---|---|
| Amount | `format_sats_amount(COIN_COINID_SHORT, amount)` |
| hBTC recipient (Sui) | `0x` + 64 hex chars, in full |
| You receive | `hBTC <amount>` (1:1, 8 decimals), with a short note that it is credited after confirmations and committee approval |
| Fees | inputs total − outputs total |
| Change | `outputs.change_total_amount`, if > 0 |
| Network | testnet only |

Finish: "Sign transaction to deposit BTC to Hashi".

`#ifdef SCREEN_SIZE_WALLET` selects short or long titles on Nano.

### ledger-app-database (T-APP-04)
- Two entries, `hashi` and `hashi_testnet`, generated with `scripts/app_load_params_gen_db.py --app_path`.
- Values: curve secp256k1, inherited paths, appFlags derived from the build.

## Tests
**Unit (cmocka, `unit-tests/`, Babylon layout with libsecp256k1-backed `mock_cx.c`):**
- **Derivation vectors:** all **16** Hashi vectors from `crates/hashi-types/src/bitcoin/fixtures/deposit-address-vectors.json` (4 master keys covering both parities × 4 paths). Assert `child_pubkey` and the signet/mainnet address.
- **HMAC/HKDF checks:** RFC 5869-style known-answer tests for the SHA3 variant, generated with Python `hmac` + `hashlib.sha3_256`.
- **Parity bug:** a negative test where the master is flipped to even-y must give a different child, the bug class [`deposit-address-derivation.md` §2](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/docs/deposit-address-derivation.md#2-child-key-derivation-mpc-key-h) warns about.

**Functional (ragger, PSBTs built with `ledger_bitcoin.psbt`, the recipient in `PartiallySignedOutput.unknown`):**
- **Positive:** a valid deposit without change, with change, and with several internal inputs. Golden snapshots for 5 devices × both variants; mainnet runs only the dashboard test until mainnet keys exist.
- **Negative:**
  - recipient field missing;
  - recipient field 31 or 33 bytes;
  - a recipient that doesn't match the address (one byte flipped);
  - an address built with swapped `multi_a` key order;
  - an address built from the wrong-network keys;
  - an address built without the parity fix;
  - two external outputs;
  - an external input;
  - no wallet policy;
  - a non-P2TR external output.
  - Each must give `SW_INCORRECT_DATA` and no signature.
- **Real PSBT:** one captured from the actual dApp or host flow on testnet once CUSTOM-2/3 exist. Fixtures alone are not acceptance evidence ([T-APP-02](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/T-APP-02-hashi-deposit-verification.md) acceptance criteria).

## Verification
1. `make COIN=hashi_testnet` and `make COIN=hashi` for every device, using the Docker `ledger-app-dev-tools` image.
2. `unit-tests`: `cmake`, then `ctest` with all 16 vectors green, plus coverage via `gen_coverage.sh`.
3. `pytest tests/ --device <dev>` for both variants. Review the snapshots: the recipient is in full and the amounts are correct.
4. Discrimination: disable each rule in turn (parity handling, length check, absent-field check, single-external-output check) and confirm that its negative test fails.
5. CI green, and the guidelines enforcer passes against the updated `ledger-app-database`.

## Follow-ups / open items
- **Repos.** `app-hashi` / `app-hashi-dev` need to be created by Embedded Apps.
- **Proprietary key layout** ([integration-plan UNCERTAIN #1](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/integration-plan.md?plain=1#L282)), and the PSBTv2 requirement: a PSBTv0 loses the field in `PsbtV2.fromV0` ([integration-plan UNCERTAIN #3](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/integration-plan.md?plain=1#L284)).
- **HMAC-SHA3 sign-off** from Security (in-app implementation). Record it in `APP_SPECIFICATION.md`.
- **Mainnet MPC and Guardian keys** (post-mainnet).
- **Whether a Move upgrade could relax key immutability** ([integration-plan UNCERTAIN #5](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/integration-plan.md?plain=1#L286)).
- **Icons and final app names** (product / partner).
