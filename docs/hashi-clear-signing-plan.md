# Plan — Clear-sign Hashi Move calls in app-sui ([B2CA-2945](https://ledgerhq.atlassian.net/browse/B2CA-2945))

## Context
Hashi (MystenLabs, Sui-native BTC bridge) has users sign three Sui calls: `withdraw::request_withdrawal` (locks hBTC, gives a Bitcoin destination), `withdraw::cancel_withdrawal` (refunds), and `deposit::deposit` (registers a BTC UTXO). Today `parser/tx.rs:1382` rejects every MoveCall outside 0x2/0x3, so all three are blind-signed or rejected (always rejected in swap). A withdrawal sends value to a Bitcoin address and must not be blind-signed. Spec: [`CUSTOM-1-hashi-app-sui-clear-signing.md`](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/CUSTOM-1-hashi-app-sui-clear-signing.md) (draft, `blockchain-integration-ai@b7e0584`).

**Decisions (user):** trusted deployments live in a **pinned const table** in the binary, so each Hashi package upgrade costs an app release; this is recorded in the PR. The **testnet deployment is always enabled**. **Scope: withdraw + cancel + deposit.**

**Verified facts:**
- **Testnet deployment:**
  - Hashi object `0x22c0ce66…28f8`
  - Original package `0xfcea10ca…4360` (hBTC type `<orig>::btc::BTC`, 8 decimals)
  - Called package v2 `0x8f7efd74…2c7e`; the SDK retargets calls to the latest upgrade, while types keep the original id
  - HRP `tb` (mainnet `bc`); Clock `0x6`
  - ⚠️ **Testnet was redeployed.** As of 2026-10-08, ts-sdks `NETWORK_CONFIG.testnet` points to a new Hashi object, `0x342e9080…d1bb`, with type package `0x5e0b99c3…8485`. Re-read the Hashi object, the type package and the current call package from the SDK and the chain at implementation time; don't use the values above.
- **No mainnet deployment yet.** Adding one later is a one-line table edit.
- **SDK PTB shapes** (`@mysten/hashi` 0.6.25):
  - **Withdraw P1:** `0x2::balance::redeem_funds<BTC>(FundsWithdrawal input)` → `request_withdrawal(hashi, clock, Result, pure vector<u8>)`
  - **Withdraw P2:** `MergeCoins(coin0,[coins…, coin::redeem_funds?])` → `SplitCoins` → `coin::into_balance` → `coin::send_funds(rest, sender)` / `destroy_zero` → `request_withdrawal`
  - **Cancel:** `cancel_withdrawal(hashi, pure addr, clock)` → `coin::from_balance` → `TransferObjects([coin], recipient)`
  - **Deposit:** per UTXO `utxo_id(pure addr txid, pure u32)` → `utxo(Result, pure u64, pure Some(addr))` → `deposit(hashi, Result, clock)`
- **Bitcoin address encoding:** the address arg is the raw witness program, 20 bytes → bech32 v0 or 32 bytes → bech32m v1. Any other length is invalid.

**Additional inputs from the Hashi knowledge base** (`blockchain-integration-ai@b7e0584`: [`move-calls-deposit-withdraw.md`](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/docs/move-calls-deposit-withdraw.md) and [`integration-plan.md`](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/integration-plan.md)):
- **Withdraw paths.**
  - Path 1 (address balance, exactly 2 Move calls) is the common case, because deposits mint with `balance::send_funds`.
  - Path 2 is still required: a cancel refunds a `Coin<BTC>` **object**, so the next withdrawal takes the coin path.
- **Deposit.**
  - A tampered `derivation_path` or amount only makes committee validation fail; it cannot redirect funds. That is why a recipient other than the signer is *flagged*, not rejected.
  - The deposit call is permissionless, so the user may never sign it. Deposit is in scope anyway (user decision).
- **Display.** Follow developer-portal `design-guidelines/transactions.mdx`: start with a "Review …" prompt and state the intent.
  - Use portal field names ("To", "Amount") where they fit.
  - Keep the app's existing "Max Gas" / "From" / Network fields for consistency.
- **Scope boundaries.**
  - **[NAPPS-1503](https://ledgerhq.atlassian.net/browse/NAPPS-1503) ([T-APP-03](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/T-APP-03-hashi-app-sui-tests-release.md))** owns the hBTC *token* regression tests (address-balance and coin transfer, no-descriptor fallback), the hBTC Exchange swap test, and the release (changelog, security review).
  - This task ([B2CA-2945](https://ledgerhq.atlassian.net/browse/B2CA-2945)) adds only the regression tests for each new parser rule, and the swap-mode rejection test, as CUSTOM-1 requires.
  - **[T-APP-02](https://github.com/LedgerHQ/blockchain-integration-ai/blob/b7e0584a5f2360890bb369a2efdd7146bf30ea43/integrations/hashi/specifications/draft/WS-02-Signer/T-APP-02-hashi-deposit-verification.md)** is the separate BTC derived app (`app-hashi`) for the Bitcoin-side deposit. It is not part of app-sui.
- **Fixtures.**
  - They must come from the pinned `@mysten/hashi` builders, not hand-made shapes. A classifier tested only on idealised fixtures can reject the real dApp flow.
  - Record the SDK version in each test file header.
  - Take the testnet ids from the SDK's `NETWORK_CONFIG` at implementation time, and cross-check them against the table in §1.
- **Host path.** Ledger Wallet's `signRawOperation` (T-WAPI-03) may also send a CAL hBTC descriptor. The Hashi review ignores it: ticker and decimals come from the pinned deployment.

## Design principles
- **One sync `hashi_route()` gate.** It sends a command down a Hashi path only if that command was rejected before this change. Every other command goes through today's code unchanged, so non-Hashi acceptance does not widen.
- **New types are invisible to existing handlers.** New `CommandResult` / `InputValue` variants fall into the existing handlers' `_ => reject` arms. Hashi handlers consume them with `command_results.remove()`, so each is used once.
- **Fail-safe.** Every check failure calls `reject_on`, which leads to the unknown/blind-sign path.
- **Swap.** Hashi transactions are mapped to `None` in swap mode, so they reject with no new await. This respects the nanox await budget.

## Changes

### 1. `rust-app/src/parser/hashi.rs` (new; `pub mod hashi;` in `parser.rs`)
- **Deployment table:**
  - `HashiDeployment { label, sui_chain, hashi_object, call_packages: &[CoinID], type_package, btc_hrp, btc_network }`
  - `HASHI_DEPLOYMENTS` holds testnet only (v2 call package only)
- **Lookups:** `deployment_for_call_package`, `deployment_for_btc_type`, and `deployment(ix)`. `deployment(ix)` uses `.get()` so it can never panic.
- **Constants:** `SUI_CLOCK_ID`, `MAX_HASHI_DEPOSIT_UTXOS = 4`.
- **Types:**
  - `BtcAddress { version, len, program:[u8;32] }`
  - `HashiUtxo { txid, vout, amount }`
  - `HashiOp::{Withdraw{amount, change, btc_address}, Cancel{request_id}, Deposit{recipient, utxos: ArrayVec<_,4>, total}}`
  - `HashiTx { deployment, op }`
- **Chain digests:** move the SIP-58 chain digests from `ui/common.rs:21-24` to `parser/common.rs`.
- **No `KNOWN_COINS` entry for hBTC.** Its ticker and decimals stay private to the Hashi review, so the swap ticker lookup cannot reach it.

### 2. `rust-app/src/ui/bech32.rs` (new)
- **API:** `segwit_address(hrp, version, program) -> Option<ArrayString<64>>`, covering bech32 (v0) and bech32m (v1).
- **Implementation:** no alloc; about 100 lines, hand-written since there is no crate dependency.
- **Tests:** BIP-173 / BIP-350 test vectors in a `#[cfg(test)]` block, following the existing pattern in `crypto_helpers/ed25519.rs:58`.

### 3. `rust-app/src/parser/tx.rs`

**Inputs (158-260, 795-826).**
- Add `CallArg` / `InputValue::RawPure{len, bytes:[u8;33]}` for Pure lengths **4 / 21 / 33**, which today are skipped as `Other`.
- Interpretation is type-directed, only at the Hashi call sites:
  - `u32` → len 4
  - `vector<u8>` → 21 with prefix 20, or 33 with prefix 32
  - `Option<address>` → 33 with tag 1
- Add const-asserts that `size_of` `InputValue` / `CommandResult` is unchanged, so the `check_heap_use` (750-767) estimate for existing transactions does not move.

**New `CommandResult` variants (473-482):** `HashiBtcBalance{dep,amount}`, `HashiCancelBalance`, `HashiCancelCoin`, `HashiUtxoId`, `HashiUtxo`, `HashiDeposited`.

**State (700-705):** add `HashiWithdrawTx | HashiCancelTx | HashiDepositTx`, plus a parser-local `HashiAccum`.
- `HashiAccum` fields: bound deployment, `coin_dep`, `foreign` flag, withdraw/cancel/deposit data, change amount.
- Bookkeeping in the existing Split/Merge/MakeMoveVec/`Right(..)` arms only **notes** coin types; it never rejects.
- Split/Merge/MakeMoveVec are rejected once the state is a Hashi state.
- `PtbFacts` (713-726) gains `refund_recipient` and `hashi_dep`.

**Loop routing (~883):** `hashi_route(&call, &tx_type)` returns `Some` only for:
- calls to a pinned Hashi package with an exact module/function match;
- `0x2::coin::{into_balance, from_balance, destroy_zero}`;
- `0x2::balance::redeem_funds(Input FW)`;
- `0x2::coin::send_funds(Input coin, …)`, or `coin::send_funds` with any argument while in a Hashi state.

Everything else follows the existing 884-945 path verbatim.

**Hashi handlers (`NoinlineFut`).** Every handler requires exact arity. Hashi-object arguments must be the pinned `SharedObject` id and clock arguments must be `0x6`. Deployment binding must stay consistent across calls.

| Call | State transition | Checks / effect |
|---|---|---|
| `request_withdrawal` | `Unknown → HashiWithdrawTx` (only one per tx) | Takes `HashiBtcBalance` with amount > 0; address must be RawPure 21/20 or 33/32 |
| `cancel_withdrawal` | `Unknown → HashiCancelTx` | `request_id` must be a 32-byte pure; produces `HashiCancelBalance` |
| `utxo_id` | `Unknown \| HashiDepositTx → HashiDepositTx` | Records txid and vout |
| `utxo` | in `HashiDepositTx` | amount > 0; `derivation_path` must be `Some` (`None` is rejected) |
| `deposit` | in `HashiDepositTx` | Same recipient across all UTXOs, at most 4 UTXOs, checked total |
| `coin::into_balance` / `balance::redeem_funds(Input FW)` | — | Coin type must be the deployment's BTC → `HashiBtcBalance` |
| `coin::send_funds` (change) | — | Must be BTC; the recipient goes through the existing `self_deposit` → sender check (extract `record_self_deposit` from 1361-1377). Records the change amount and writes back zero. Works in either order relative to `request_withdrawal` |
| `coin::destroy_zero` | — | Tracked amount must be 0 |
| `coin::from_balance` | in cancel | Takes `HashiCancelBalance` |
| `TransferObjects` | in cancel | Exactly 1 object, which must be `HashiCancelCoin`; records the refund recipient |

**Finish (replaces the gate at 1012-1023 for Hashi states).** `finish_hashi` rejects unless all of these hold:
- no `total_coin_amount` or recipient;
- `!touches_gas_coin`;
- `!foreign`;
- no unconsumed Hashi result;
- the per-flow completeness checks pass.

On success it returns `ProgrammableTransaction::Hashi(Box<HashiTx>)`.

**`TransactionDataParser` (2632-2800):**
- Reject if `refund_recipient != sender`.
- Reject if a `ValidDuring` chain differs from the deployment's `sui_chain`.
- The existing sponsor checks are unchanged.

**`KnownTx`:** add `Hashi { tx: Box<HashiTx>, gas_budget, gas_from_address_balance }` (2803-2832). Map it in `tx_parser` (2859-2955).

**Refactors (no behaviour change):** extract `record_self_deposit`, and make `handle_merge_coins` return its `CoinType`. Land these first and keep the suite green with no snapshot diffs.

### 4. `rust-app/src/ui/nbgl.rs`
Add `confirm_hashi_tx(address, &HashiTx, HashiGas, &TxPrincipals, replay)`. It reuses `principal_values`/`fields("From")`, `format_gas_amount` (388-424), `replay_fields` and `get_amount_in_decimals`.

**Withdraw** — "Review {kind} to withdraw hBTC to Bitcoin":
- From
- Amount `hBTC x`
- Change `hBTC y` (only if > 0; returned to sender)
- To BTC address `tb1…`/`bc1…`
- BTC network
- Bridge
- Note: "Bitcoin network fee is deducted from the amount"
- Max Gas
- Network/Nonce

**Cancel:**
- From
- Request ID `0x…`
- Refund to: sender
- Amount: full pending amount
- Bridge
- Max Gas
- replay

**Deposit:**
- From
- hBTC recipient
- If the recipient ≠ signing address: "Recipient is not this account" warning field (flag, not reject, per spec)
- Per UTXO: `UTXO i` = txid in display order (byte-reversed) `:vout`, and `Amount i` `BTC x`
- Total (if more than 1 UTXO)
- Bridge
- Max Gas
- replay

**String lifetimes:** all owned Strings are built before the `fields` vector, following the pattern at 172-181.

### 5. `rust-app/src/implementation.rs` (`sign_apdu` 116-365)
- **Before the match (~174):** `Some(KnownTx::Hashi{..}) if ctx.is_swap() => None`. This reuses the existing swap rejection with no new await.
- **New arm:** a copy of the Unstake arm (274-307): BIP path check, `with_public_keys`, and `principals.and_then(|p| ui.confirm_hashi_tx(..))`. The arm contains no `is_swap` await.

### 6. Housekeeping
- Run `rustfmt --edition 2021` on the changed files.
- Bump the version in `rust-app/Cargo.toml` (1.5.14 → 1.6.0). Check whether develop's version is already tagged.
- Note in `.github/copilot-instructions.md`: the Hashi section, and that the UI is NBGL on all devices.
- The PR description records the package-trust decision and its upgrade cost.

## Fixtures & tests (`tests/standalone/`)
**Fixtures.** Generate real transaction bytes with `@mysten/hashi` 0.6.25 against testnet, using a small Node script kept in the scratchpad (not committed). Supply coin objects via `object_list`. This also confirms the actual ordering of `send_funds` relative to `request_withdrawal`. Negative cases are byte-patched from these fixtures, as described in CLAUDE.md.

**Positive tests** (golden snapshots on 5 devices):
- withdraw P1 (P2WPKH);
- withdraw P2 coins + merge + redeem top-up with change;
- withdraw redeem→split variant;
- withdraw P2TR (bech32m);
- withdraw exact amount (`destroy_zero`);
- cancel;
- deposit with 1 UTXO and with 4 UTXOs;
- deposit with a recipient that is not the signer (warning shown);
- withdraw with `ValidDuring` testnet.

**Negative tests** (must land on the not-recognized screen):
- untrusted or original package id;
- wrong Hashi object;
- wrong clock;
- address length 25;
- 21-byte address with a bad prefix;
- 33-byte address with an `Option` tag;
- cancel recipient ≠ sender;
- cancel with an extra object;
- deposit `None` path;
- 5 UTXOs;
- mixed recipients;
- amount 0;
- withdraw + SUI transfer, in both orders;
- deposit + `SplitCoins(GasCoin)`;
- `into_balance<SUI>` → `request_withdrawal`;
- `withdrawal_split` → `balance::redeem_funds` → `request_withdrawal`;
- change sent to a non-sender, in both orders;
- double `request_withdrawal`;
- a balance consumed twice;
- `ValidDuring` with the mainnet digest;
- sponsor-funded withdrawal.

**Non-widening regressions** (must stay unknown):
- `coin::send_funds(Input SUI)` + `TransferObjects`;
- `into_balance<SUI>` + `balance::send_funds`;
- `balance::redeem_funds(Input FW SUI)` + `send_funds`.

**Swap:** add a case to `tests/swap/test_sui.py` asserting that a Hashi withdraw is rejected in swap mode.

**Out of scope here ([NAPPS-1503](https://ledgerhq.atlassian.net/browse/NAPPS-1503)):** hBTC token transfer tests, the hBTC swap success test, the changelog and the release.

## Verification
1. **Existing suite after refactors.** Use Docker with `cargo ledger build <dev>` + `pytest tests/standalone --device <dev>` for all 5 devices. The suite must pass unchanged.
2. **New tests per device.** Generate snapshots with `--golden_run`. Review the PNGs by eye: the bech32 address matches the SDK input, txid order, amounts.
3. **Discrimination (CLAUDE.md).** For each gate (Hashi object, clock, package, refund recipient, change recipient, `None` path, UTXO cap, `foreign`), revert that one check, rebuild, and confirm the matching negative test fails.
4. **Size and stack.** Compare the State-sizes log (`app_main.rs:54-58`, built with `--features speculos,ledger-log/log_info`) before and after on nanox.
5. **Nanox swap.** Run `tests/swap` on nanox to catch the await/future-size regression. This requires building the Exchange/Ethereum dependency ELFs first; they are currently absent, which makes swap tests error instantly.

## Open items (from the spec; not blocking implementation)
- Mainnet package ids and their BTC network: add a table row when published.
- Whether users actually sign `deposit` (MystenLabs).
- Whether Security accepts cancel without an amount and the deposit flag-not-reject behaviour.
- The withdrawal and deposit minimum (30 000 sat by default, governable) is not enforced on device. The chain rejects anything below it and no funds are lost, so no screen warning is planned.
- Re-pin the SDK version at implementation time.
