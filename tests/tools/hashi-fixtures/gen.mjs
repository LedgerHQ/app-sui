// Generates the Hashi PTB fixtures used by tests/standalone/test_sign_hashi*.py from the
// pinned @mysten/hashi builders, so the parser is tested against the transactions the
// real SDK emits rather than hand-made shapes.
//
//   cd tests/tools/hashi-fixtures && npm install && node gen.mjs
//
// Balance and coin queries are stubbed (no funded account is needed); shared-object
// versions and the call-package retarget come from the live Sui testnet, through the SDK's
// client-side resolver so that the synthetic coin objects are never looked up on chain.
// Output: ../../standalone/hashi_fixtures.json (base64 tx bytes, then the object blobs to
// pass as `object_list`, gas coin first).
import { SuiGraphQLClient } from '@mysten/sui/graphql';
import { Transaction } from '@mysten/sui/transactions';
import { hashi } from '@mysten/hashi';
import { blake2b } from '@noble/hashes/blake2.js';
import { toBase58, fromBase64, toBase64, toHex } from '@mysten/sui/utils';
import { writeFileSync } from 'node:fs';
import { readFileSync } from 'node:fs';
// Not a public export; client-side resolution avoids server simulation of synthetic coins.
import { coreClientResolveTransactionPlugin } from './node_modules/@mysten/sui/dist/client/core-resolver.mjs';

const pkgVersion = (n) => JSON.parse(readFileSync(new URL(`./node_modules/${n}/package.json`, import.meta.url))).version;
const HASHI_VERSION = pkgVersion('@mysten/hashi');
const SUI_VERSION = pkgVersion('@mysten/sui');

const SENDER = '0xd5a6dc2129672c36d318ecc28c96a25fdf640933d6ed8840b1f78171f50f9cc9'; // OWNED_ADDRESS
const OTHER = '0x5a64eec558ee719741578942714a0b35058ced15d79f4af64da014715ada4497';  // FOREIGN_ADDRESS
const PKG = '0xfcea10cadbb553c4874201584abf68771592678952efd957b2e82c010c7f4360';
const HASHI_OBJ = '0x22c0ce66ce09df2dc88a31bd320d4177b766518b9b88010368cfbdcd724528f8';
const BTC_TYPE = `${PKG}::btc::BTC`;

// --- object blobs -------------------------------------------------------------------
const GAS_COIN = fromBase64(
  'AAEB03ZCEQAAAAAoQA29z+2o4eZOu/VxDM9aZ7+m5/lFySvIYR4MtEtyGd4QDpQ5AAAAAABvsh/urQJ9' +
  'pIcyla/9bE82GP4Xb6L78+e17x2UY7MeISB0/j3Uc6ljNbb1tbWgvj5PAz7MCgIO6e91iU9asLM9x2AT' +
  'DwAAAAAA');
const GAS_BAL_OFF = 2 + 1 + 8 + 1 + 32;
const GAS_ID = '0x400dbdcfeda8e1e64ebbf5710ccf5a67bfa6e7f945c92bc8611e0cb44b7219de';
const GAS_VERSION = 289568467;
const USDC_OBJ = fromBase64(
  'AAMH26NGcuMMsGWx+T46tVMYdo/W/vZsFZQsn3y4RuL5AOcEdXNkYwRVU0RDAAF2L6QdAAAAACjTuql6RqINZfG+YMuqFghW5qrlB44WVFBx4v0+MUEFubeGAQAAAAAAAA8vjdSeJp2gZvN2JNM6i07RsmLWpf3ytzW+OfjXxaz1ICV9oiz28QN2+VFgs3VVcob35zoaZgQf5WcAe9gWdNyWoC0UAAAAAAA=');

const digest = (obj) => toBase58(blake2b(new Uint8Array([...new TextEncoder().encode('Object::'), ...obj]), { dkLen: 32 }));
const u64le = (v) => { const b = new Uint8Array(8); new DataView(b.buffer).setBigUint64(0, BigInt(v), true); return b; };
const hexBytes = (h) => Uint8Array.from(h.replace(/^0x/, '').match(/../g).map((x) => parseInt(x, 16)));

function gasCoin(balance) {
  const o = Uint8Array.from(GAS_COIN);
  o.set(u64le(balance), GAS_BAL_OFF);
  return o;
}

// Rebuild the USDC coin blob as Coin<PKG::btc::BTC> with a fresh id and the given balance.
// USDC layout: 00 03 07 | addr[32] | vec "usdc" | vec "USDC" | vec [] | has_public_transfer |
//              version[8] | contents(vec: uid[32] | bal[8]) | owner, previous tx, storage rebate
function btcCoin(idByte, balance) {
  const src = USDC_OBJ;
  let p = 3 + 32;
  const modLen = src[p]; p += 1 + modLen;
  const nameLen = src[p]; p += 1 + nameLen;
  p += 1; // empty type params
  const hasPublicTransfer = src[p]; p += 1;
  const version = src.slice(p, p + 8); p += 8;
  const contentsLen = src[p]; p += 1;
  p += contentsLen;
  const rest = src.slice(p);
  const id = new Uint8Array(32).fill(0xb7); id[31] = idByte;
  const enc = (s) => [s.length, ...new TextEncoder().encode(s)];
  const obj = new Uint8Array([0x00, 0x03, 0x07, ...hexBytes(PKG), ...enc('btc'), ...enc('BTC'), 0x00,
    hasPublicTransfer, ...version, 40, ...id, ...u64le(balance), ...rest]);
  return { obj, objectId: '0x' + toHex(id), version: String(new DataView(version.buffer, version.byteOffset).getBigUint64(0, true)), digest: digest(obj) };
}

// --- client with stubbed balances -----------------------------------------------------
function makeClient({ addressBalance = 0n, coins = [] }) {
  const base = new SuiGraphQLClient({ network: 'testnet', url: 'https://graphql.testnet.sui.io/graphql' });
  const core = base.core;
  core.resolveTransactionPlugin = () => coreClientResolveTransactionPlugin;
  const coinBalance = coins.reduce((s, c) => s + c.balance, 0n);
  core.getBalance = async () => ({ balance: {
    coinType: BTC_TYPE, balance: String(addressBalance + coinBalance),
    coinBalance: String(coinBalance), addressBalance: String(addressBalance) } });
  core.listCoins = async () => ({ hasNextPage: false, cursor: null, objects: coins.map((c) => ({
    objectId: c.objectId, version: c.version, digest: c.digest, balance: String(c.balance), type: BTC_TYPE, owner: { $kind: 'AddressOwner', AddressOwner: SENDER } })) });
  return base.$extend(hashi({ hashiObjectId: HASHI_OBJ, packageId: PKG, bitcoinNetwork: 'signet' }));
}

const GAS_BALANCE = 1_000_000_000n;
const gas = gasCoin(GAS_BALANCE);
async function build(client, tx, expiration = { None: true }) {
  tx.setSender(SENDER);
  tx.setGasPrice(1000);
  tx.setGasBudget(5_000_000);
  tx.setGasPayment([{ objectId: GAS_ID, version: GAS_VERSION, digest: digest(gas) }]);
  tx.setExpiration(expiration);
  const bytes = await tx.build({ client });
  return bytes;
}

const P2WPKH = hexBytes('751e76e8199196d454941c45d1b3a323f1433bd6');                            // BIP-173 vector
const P2TR = hexBytes('a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c');        // BIP-350 vector
const fixtures = { _meta: { '@mysten/hashi': HASHI_VERSION, '@mysten/sui': SUI_VERSION, sender: SENDER, hashiObject: HASHI_OBJ, typePackage: PKG, gasCoinBalance: String(GAS_BALANCE) } };
// The device signs the intent message: intent (version 0, scope TransactionData, app Sui)
// followed by the TransactionData bytes `build()` returns.
const INTENT = new Uint8Array([0, 0, 0]);
const out = (name, bytes, objects = []) => {
  fixtures[name] = { tx: toBase64(new Uint8Array([...INTENT, ...bytes])), objects: [toBase64(gas), ...objects.map((o) => toBase64(o.obj))] };
  console.log(name, bytes.length, 'bytes');
};

// 1/2. Withdraw from address balance (Path 1)
for (const [name, addr] of [['withdraw_ab_p2wpkh', P2WPKH], ['withdraw_ab_p2tr', P2TR]]) {
  const c = makeClient({ addressBalance: 10_000_000n });
  out(name, await build(c, c.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: addr })));
}
// 3. Withdraw from two coin objects with change (Path 2)
{
  const coins = [btcCoin(1, 700_000n), btcCoin(2, 900_000n)].map((c, i) => ({ ...c, balance: [700_000n, 900_000n][i] }));
  const c = makeClient({ coins });
  out('withdraw_coins_change', await build(c, c.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: P2WPKH })), coins);
}
// 4. Withdraw from a coin plus address-balance top-up (Path 2 + coin::redeem_funds)
{
  const coins = [{ ...btcCoin(3, 500_000n), balance: 500_000n }];
  const c = makeClient({ coins, addressBalance: 2_000_000n });
  // addressBalance >= total would pick Path 1; make AB alone insufficient so the coin is used.
  const c2 = makeClient({ coins, addressBalance: 1_000_000n });
  out('withdraw_coins_topup', await build(c2, c2.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: P2TR })), coins);
}
// 5. Cancel
{
  const c = makeClient({});
  out('cancel', await build(c, c.hashi.tx.cancelWithdrawal({
    requestId: '0x' + '11'.repeat(32), recipient: SENDER })));
}
// 6/7. Deposit (1 and 4 UTXOs); txid given in display order
const TXID = '0xc0ffee' + '00'.repeat(27) + '0123';
for (const [name, n] of [['deposit_1', 1], ['deposit_4', 4]]) {
  const c = makeClient({});
  const utxos = Array.from({ length: n }, (_, i) => ({ vout: i, amountSats: BigInt(50_000 * (i + 1)) }));
  out(name, await build(c, c.hashi.tx.deposit({ txid: TXID, utxos, recipient: SENDER })));
}
// 8. Deposit crediting another account (flag, not reject)
{
  const c = makeClient({});
  out('deposit_other_recipient', await build(c, c.hashi.tx.deposit({ txid: TXID, utxos: [{ vout: 1, amountSats: 75_000n }], recipient: OTHER })));
}

// ---- extra transactions for negative and non-widening tests --------------------------
const SUI_MAINNET_CHAIN = '35834a8ac17ca48fb14ac8f99c17c98747e95dd07294ae41a46b382246a4499b';
const SUI_TESTNET_CHAIN = '4c78adacf2a2f5ad80f27ed7d54aa69d3a78f1ca67fdef9ecf5754f5b8bb77b0';
const validDuring = (chainHex) => ({ ValidDuring: { minEpoch: null, maxEpoch: null, minTimestamp: null, maxTimestamp: null, chain: toBase58(hexBytes(chainHex)), nonce: 7 } });
for (const [name, chain] of [['withdraw_valid_during_testnet', SUI_TESTNET_CHAIN], ['withdraw_valid_during_mainnet', SUI_MAINNET_CHAIN]]) {
  const c = makeClient({ addressBalance: 10_000_000n });
  out(name, await build(c, c.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: P2WPKH }), validDuring(chain)));
}
{
  const c = makeClient({ addressBalance: 10_000_000n });
  out('withdraw_bad_len', await build(c, c.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: new Uint8Array(25).fill(0x42) })));
}
{
  const c = makeClient({});
  out('cancel_other_recipient', await build(c, c.hashi.tx.cancelWithdrawal({ requestId: '0x' + '11'.repeat(32), recipient: OTHER })));
}
{
  const c = makeClient({});
  const utxos = Array.from({ length: 5 }, (_, i) => ({ vout: i, amountSats: BigInt(50_000 * (i + 1)) }));
  out('deposit_5', await build(c, c.hashi.tx.deposit({ txid: TXID, utxos, recipient: SENDER })));
}
{
  const c = makeClient({ addressBalance: 10_000_000n });
  const tx = c.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: P2WPKH });
  const [s] = tx.splitCoins(tx.gas, [1000]);
  tx.transferObjects([s], OTHER);
  out('withdraw_then_sui_transfer', await build(c, tx));
}
{
  const c = makeClient({ addressBalance: 10_000_000n });
  const tx = new Transaction();
  const [s] = tx.splitCoins(tx.gas, [1000]);
  tx.transferObjects([s], OTHER);
  tx.add(c.hashi.call.requestWithdrawal({ btc: tx.balance({ type: BTC_TYPE, balance: 1_234_567n }), bitcoinAddress: Array.from(P2WPKH) }));
  out('sui_transfer_then_withdraw', await build(c, tx));
}
{
  const c = makeClient({ addressBalance: 10_000_000n });
  const tx = c.hashi.tx.requestWithdrawal({ amount: 1_234_567n, bitcoinAddress: P2WPKH });
  tx.add(c.hashi.call.requestWithdrawal({ btc: tx.balance({ type: BTC_TYPE, balance: 1000n }), bitcoinAddress: Array.from(P2TR) }));
  out('double_withdraw', await build(c, tx));
}
// A USDC coin (blob from tests/application_client/sui_utils.py, 100023 units).
const USDC = { objectId: '0xd3baa97a46a20d65f1be60cbaa160856e6aae5078e16545071e2fd3e314105b9', version: '497299318', digest: digest(USDC_OBJ), obj: USDC_OBJ };
const USDC_TYPE = '0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC';
{
  const c = makeClient({ addressBalance: 10_000_000n });
  const tx = new Transaction();
  const [s] = tx.splitCoins(tx.objectRef(USDC), [1000]);
  const bal = tx.moveCall({ target: '0x2::coin::into_balance', typeArguments: [USDC_TYPE], arguments: [s] });
  tx.add(c.hashi.call.requestWithdrawal({ btc: bal, bitcoinAddress: Array.from(P2WPKH) }));
  out('withdraw_usdc_balance', await build(c, tx), [USDC]);
}
// Non-Hashi shapes reaching the new 0x2 routes: they must stay unknown.
{
  const c = makeClient({});
  const tx = new Transaction();
  const coin = tx.objectRef(USDC);
  const [s] = tx.splitCoins(coin, [1000]);
  tx.moveCall({ target: '0x2::coin::send_funds', typeArguments: [USDC_TYPE], arguments: [coin, tx.pure.address(SENDER)] });
  tx.transferObjects([s], OTHER);
  out('nonhashi_usdc_send_funds_input', await build(c, tx), [USDC]);
}
{
  const coins = [{ ...btcCoin(4, 900_000n), balance: 900_000n }];
  const c = makeClient({ coins });
  const tx = new Transaction();
  const coin = tx.objectRef(coins[0]);
  const [s] = tx.splitCoins(coin, [1000]);
  tx.moveCall({ target: '0x2::coin::send_funds', typeArguments: [BTC_TYPE], arguments: [coin, tx.pure.address(SENDER)] });
  tx.transferObjects([s], OTHER);
  out('nonhashi_btc_send_funds_input', await build(c, tx), coins);
}
{
  const c = makeClient({});
  const tx = new Transaction();
  const [s] = tx.splitCoins(tx.objectRef(USDC), [1000]);
  const bal = tx.moveCall({ target: '0x2::coin::into_balance', typeArguments: [USDC_TYPE], arguments: [s] });
  tx.moveCall({ target: '0x2::balance::send_funds', typeArguments: [USDC_TYPE], arguments: [bal, tx.pure.address(OTHER)] });
  out('nonhashi_into_balance_send', await build(c, tx), [USDC]);
}
{
  const c = makeClient({});
  const tx = new Transaction();
  const bal = tx.moveCall({ target: '0x2::balance::redeem_funds', typeArguments: [BTC_TYPE], arguments: [tx.withdrawal({ amount: 5000, type: BTC_TYPE })] });
  tx.moveCall({ target: '0x2::balance::send_funds', typeArguments: [BTC_TYPE], arguments: [bal, tx.pure.address(OTHER)] });
  out('nonhashi_btc_redeem_input_send', await build(c, tx));
}

writeFileSync(new URL('../../standalone/hashi_fixtures.json', import.meta.url), JSON.stringify(fixtures, null, 2) + '\n');
