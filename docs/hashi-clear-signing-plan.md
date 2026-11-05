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
  - ⚠️ **Testnet was redeployed.** As of 2026-10-08, ts-sdks `NETWORK_CONFIG.testnet` points to a new Hashi object, `0x342e9080…d1bb`, with type package `0x5e0b99c3…8485`. The app now pins that deployment (see the implementation notes); the values above are the retired one.
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

> The screens below are the initial design. They were later aligned with the Figma user flows (deposit in 0acae37, withdrawal in 39ed4ca, then cancel): each review now leads with what moves where, the accounts come after it, and Nano gets a short title plus an Operation screen. `nbgl.rs` and the snapshots are the reference.

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
- Bump the version in `rust-app/Cargo.toml` (1.5.14 → 1.6.0). **Done** in e64caac, with the changelog entry.
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
- **Deposit crediting another account — to discuss with the Ledger integration team and MystenLabs.** A `deposit` whose `derivation_path` is not the signer is currently clear-signed with a "Warning: Not your account" field (`test_sign_hashi_deposit_other_recipient`). Keep this as is until the discussion concludes.
  - **Current basis:** CUSTOM-1 requires it to be flagged, not rejected (requirements §, acceptance criterion "A recipient other than the signer is flagged"). `deposit` is permissionless and a wrong recipient cannot redirect funds, only fail validation (KB `move-calls-deposit-withdraw.md`, deposit section).
  - **Why question it:** no document describes a real need for it. The Hashi design page ([deposit](https://mystenlabs.github.io/hashi/design/deposit/)) and the integration plan only describe users registering their own deposit. Whether the dApp has the user sign `deposit` at all, or relays it, is still UNCERTAIN in CUSTOM-1 and the KB (`blockchain-integration-ai@9f36fcb`).
  - **To ask MystenLabs:** does the dApp have the user sign `deposit`, and for which recipient?
  - **Possible outcome:** if no flow needs another recipient, reject it instead (a fail-safe one-line parser change; the snapshot test moves to `test_sign_hashi_rejected.py`). This needs CUSTOM-1's acceptance criterion changed first, and Ledger Security's view.
- Whether Security accepts cancel without an amount.
- The withdrawal and deposit minimum (30 000 sat by default, governable) is not enforced on device. The chain rejects anything below it and no funds are lost, so no screen warning is planned.
- Re-pin the SDK version at implementation time.

## Implementation notes (deviations from the plan)
- **Pinned deployment:** testnet as targeted by `@mysten/hashi` 0.6.26 (`NETWORK_CONFIG.testnet`, re-read on chain 2026-10-19, epoch 1257): Hashi object `0x342e9080…d1bb`, package `0x5e0b99c3…8485` for both calls and types (`versioning` enables version 1 only, with no upgrade yet). Move signatures are unchanged from the retired deployment (`0x22c0ce66…28f8`, call package `0x8f7efd74…2c7e`, type package `0xfcea10ca…4360`), which is no longer clear-signed; `retired_deployment` in `test_sign_hashi_rejected.py` covers it.
- **Real PTB shapes** (confirmed from the SDK source and the generated fixtures):
  - In the coin path, the change `coin::send_funds` always comes *before* `request_withdrawal`.
  - `coin::destroy_zero` is never emitted for a Hashi withdrawal (its balance intent always returns change via `send_funds`), so it is not supported. That is the fail-safe direction.
- **No-widening check:** coin types are not tracked incrementally. Instead, `finish_hashi` scans the remaining inputs and results once. Any Hashi route taken by a transaction that does not end as a complete Hashi flow rejects (`hashi.is_some()`).
- **Deposit:** at most 4 UTXOs per transaction (`MAX_HASHI_DEPOSIT_UTXOS`), all crediting one recipient.
- **Review labels** that wrapped mid-word on Flex were shortened: "Bitcoin fee: From the amount", "Warning: Not your account".
- **nanox state budget:** the future state is 5560 bytes against a 5552 baseline.
  - To get there, the Hashi output is boxed and the parse result is scoped in `sign_apdu`.
  - Hashi calls get their own guarded match arm, and the cancel/deployment data travels in the Hashi output rather than in `PtbFacts`.
  - Details are in the comments at those sites.
- **Swap deps:** the Exchange/Ethereum test ELFs must be rebuilt with the current image: the Sui app is now API level 27 and the old ELFs were 26.
- **Done after the parser change:**
  - the version bump to 1.6.0 and the changelog entry (e64caac); the main-menu goldens changed on Flex, Stax and Apex P only, since the Nano screen does not show the version;
  - the hBTC swap tests (8c4177d), part of NAPPS-1503.
- **Not done here:** the hBTC token transfer tests (NAPPS-1503).

## Withdrawal test scenarios (as implemented)
The five withdrawal cases in `tests/standalone/test_sign_hashi.py` all end in the same Hashi call, `withdraw::request_withdrawal(hashi, clock, Balance<BTC>, btc_address)`. They vary along three axes: **where the user's hBTC sits** (which sets the PTB shape), **the Bitcoin address type**, and **the transaction expiration**.

The SDK's `requestWithdrawal` only asks for "a `Balance<BTC>` of N sat" (`tx.balance(...)`). The commands that produce that balance come from the Sui SDK's balance intent, and depend on where the hBTC is held:
- **Address balance.** Deposits mint hBTC with `balance::send_funds`, which credits the Sui address balance, not a coin object. This is the normal case.
- **Coin objects.** A cancelled withdrawal refunds a `Coin<BTC>` **object** (`cancel_withdrawal` → `coin::from_balance` → `TransferObjects`), so after a cancel part or all of the hBTC is held as coins.

| Test | Funded from | Commands (decoded from the fixtures) | Review shows |
|---|---|---|---|
| `withdraw_ab_p2wpkh` | address balance | `balance::redeem_funds(FundsWithdrawal 1 234 567)` → `request_withdrawal` | Amount, To `tb1q…` (bech32, segwit v0, 20-byte program) |
| `withdraw_ab_p2tr` | address balance | same as above | To `tb1p…` (bech32m, taproot v1, 32-byte program) |
| `withdraw_coins_change` | two coins (700 000 + 900 000) | `MergeCoins` → `SplitCoins(1 234 567)` → `coin::into_balance` → `coin::send_funds(rest, sender)` → `request_withdrawal` | Amount, **Change 365 433** to the sender |
| `withdraw_coins_topup` | one 500 000 coin + 734 567 from the address balance | `coin::redeem_funds(FundsWithdrawal 734 567)` → `MergeCoins` → `SplitCoins` → `into_balance` → `send_funds` → `request_withdrawal` | Amount, no Change line |
| `withdraw_valid_during_testnet` | address balance | same as `withdraw_ab_p2wpkh` | plus **Network** and **Nonce** |

What each one covers:
- **`withdraw_ab_p2wpkh` / `withdraw_ab_p2tr`:** the common short path, once per Bitcoin address type the bridge can pay out to. The app rebuilds the `tb1…` address from the raw witness program in the transaction (`ui/bech32.rs`); these two prove both encodings, which differ in checksum constant and in program length. The addresses are the BIP-173 / BIP-350 test vectors.
- **`withdraw_coins_change`:** the coin path. The unused part of the merged coin goes back through `coin::send_funds(coin, sender)`. The parser requires that recipient to be the sender and shows the amount as "Change"; change sent to anyone else is rejected (`change_to_other` in `test_sign_hashi_rejected.py`).
- **`withdraw_coins_topup`:** the coins alone are not enough, so the SDK draws the missing 734 567 from the address balance with `coin::redeem_funds`, merges it in, and splits exactly. The SDK still emits `send_funds` for the now-empty coin, so the parser sees a zero change and the review omits the line (Change is shown only when it is above 0). This is the most complex withdrawal shape, combining both funding sources.
- **`withdraw_valid_during_testnet`:** the same transaction as `withdraw_ab_p2wpkh`, with a `ValidDuring` expiration naming the Sui chain. The review adds the existing Network / Nonce replay-protection fields. The parser also requires that chain to match the deployment's; `withdraw_valid_during_mainnet` is the rejected counterpart.

All five also show Bridge (`Hashi Testnet`), the Bitcoin network, "Bitcoin fee: From the amount" and Max Gas.
