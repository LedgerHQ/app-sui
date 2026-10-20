//! Hashi, MystenLabs' Sui-native Bitcoin bridge: the deployments this app trusts and the
//! values its Move calls carry.
//!
//! A Hashi transaction is recognised only when its calls target a package pinned below.
//! Calls go to the latest package upgrade while type tags keep the original package id,
//! so a deployment pins both. Pinning in the binary means every Hashi package upgrade
//! needs an app release: a call to an unpinned package falls back to the unknown
//! (blind-sign) path, which is the fail-safe direction.
//!
//! The Bitcoin network is not in the transaction, so it comes from the deployment too.

use crate::parser::common::*;
use hex_literal::hex;

pub struct HashiDeployment {
    /// Shown on the review, so the user can tell testnet from mainnet.
    pub label: &'static str,
    /// SIP-58 `ValidDuring.chain` of the Sui network this deployment lives on.
    pub sui_chain: SuiAddressRaw,
    /// The `Hashi` shared object every call takes as its first argument.
    pub hashi_object: SuiAddressRaw,
    /// Package ids Move calls may target: only upgrades whose code has been reviewed.
    pub call_packages: &'static [CoinID],
    /// Original package id, used by type tags (`<type_package>::btc::BTC`).
    pub type_package: CoinID,
    /// Bech32 human-readable part of the Bitcoin network the bridge pays out on.
    pub btc_hrp: &'static str,
    pub btc_network: &'static str,
}

pub const SUI_CLOCK_ID: CoinID =
    hex!("0000000000000000000000000000000000000000000000000000000000000006");

pub const HASHI_BTC_DECIMALS: u8 = 8;
pub const HASHI_BTC_TICKER: &str = "hBTC";
const HASHI_BTC_MODULE: &str = "btc";
const HASHI_BTC_STRUCT: &str = "BTC";

/// Upper bound on the UTXOs one deposit transaction may register. Each one costs
/// three commands and four inputs in the parser's heap-tracked maps; more go to the
/// unknown path.
pub const MAX_HASHI_DEPOSIT_UTXOS: usize = 4;

/// Testnet, as targeted by `@mysten/hashi` 0.6.26 (`NETWORK_CONFIG.testnet`), with the
/// call package read from the object's `versioning.upgrade_cap` (enabled version 1, so
/// calls and types share the original package id).
static HASHI_DEPLOYMENTS: [HashiDeployment; 1] = [HashiDeployment {
    label: "Hashi Testnet",
    sui_chain: SUI_TESTNET_CHAIN,
    hashi_object: hex!("342e908043db6090558ed7766df95bfc3609881b2eac7edb5343dd5bb3ddd1bb"),
    call_packages: &[hex!(
        "5e0b99c31f691742acea5de4ca1bf53c398d5639fed6503a51ffe8035dc68485"
    )],
    type_package: hex!("5e0b99c31f691742acea5de4ca1bf53c398d5639fed6503a51ffe8035dc68485"),
    btc_hrp: "tb",
    btc_network: "Bitcoin Signet",
}];

pub fn deployment(ix: u8) -> Option<&'static HashiDeployment> {
    HASHI_DEPLOYMENTS.get(ix as usize)
}

pub fn deployment_for_call_package(package: &CoinID) -> Option<u8> {
    HASHI_DEPLOYMENTS
        .iter()
        .position(|d| d.call_packages.contains(package))
        .and_then(|ix| u8::try_from(ix).ok())
}

pub fn btc_coin_type(dep: &HashiDeployment) -> CoinType {
    coin_type_from_short_str(dep.type_package, HASHI_BTC_MODULE, HASHI_BTC_STRUCT)
}

/// Whether `coin_type` is the hBTC of some pinned deployment.
pub fn is_btc_coin_type(coin_type: &CoinType) -> bool {
    HASHI_DEPLOYMENTS
        .iter()
        .any(|d| btc_coin_type(d) == *coin_type)
}

/// Longest Pure argument kept undecoded: `vector<u8>` of 32 bytes or `Option<address>`.
pub const RAW_PURE_MAX: usize = 33;

/// Pure lengths kept as raw bytes, decoded only where a Hashi call expects a given
/// Move type: 4 (`u32`), 21 (`vector<u8>` of 20) and 33 (`vector<u8>` of 32, or
/// `Option<address>`). Before Hashi support these were skipped and never stored, and
/// no other call consumes a raw value, so keeping them accepts nothing new elsewhere.
pub fn is_raw_pure_len(len: u32) -> bool {
    matches!(len, 4 | 21 | 33)
}

#[derive(Clone, Copy)]
pub struct RawPure {
    len: u8,
    bytes: [u8; RAW_PURE_MAX],
}

impl RawPure {
    /// `None` if `src` is longer than `RAW_PURE_MAX`.
    pub fn new(src: &[u8]) -> Option<Self> {
        let mut bytes = [0u8; RAW_PURE_MAX];
        bytes.get_mut(..src.len())?.copy_from_slice(src);
        Some(RawPure {
            len: src.len() as u8,
            bytes,
        })
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// BCS `u32`.
    pub fn as_u32(&self) -> Option<u32> {
        let b: [u8; 4] = self.as_slice().try_into().ok()?;
        Some(u32::from_le_bytes(b))
    }

    /// BCS `Option<address>`, only the `Some` form (`None` is a 1-byte Pure).
    pub fn as_some_address(&self) -> Option<SuiAddressRaw> {
        match self.as_slice().split_first() {
            Some((1, addr)) => addr.try_into().ok(),
            _ => None,
        }
    }

    /// `request_withdrawal`'s `bitcoin_address: vector<u8>`: a raw witness program.
    /// The committee pays 20 bytes as witness v0 (P2WPKH) and 32 bytes as v1 (P2TR);
    /// a 32-byte P2WSH program cannot be expressed. Anything else is refused.
    pub fn as_witness_program(&self) -> Option<BtcAddress> {
        let (len, program) = self.as_slice().split_first()?;
        let version = match (*len as usize, program.len()) {
            (20, 20) => 0,
            (32, 32) => 1,
            _ => return None,
        };
        let mut out = [0u8; 32];
        out[..program.len()].copy_from_slice(program);
        Some(BtcAddress {
            version,
            len: *len,
            program: out,
        })
    }
}

#[derive(Clone, Copy)]
pub struct BtcAddress {
    pub version: u8,
    len: u8,
    program: [u8; 32],
}

impl BtcAddress {
    pub fn program(&self) -> &[u8] {
        &self.program[..self.len as usize]
    }
}

#[derive(Clone, Copy)]
pub struct HashiUtxo {
    /// Internal byte order, as in the transaction; reversed for display.
    pub txid: [u8; 32],
    pub vout: u32,
    pub amount: u64,
}

#[derive(Clone, Copy)]
pub enum HashiOp {
    Withdraw {
        /// hBTC locked in the request; the Bitcoin miner fee is deducted from it later.
        amount: u64,
        /// hBTC split off the same coins and returned to the sender's balance.
        change: u64,
        btc_address: BtcAddress,
    },
    /// Refunds the full pending amount, which lives in the request and not in the
    /// transaction, as a coin to `refund_recipient` (checked against the sender).
    Cancel {
        request_id: SuiAddressRaw,
        refund_recipient: SuiAddressRaw,
    },
    Deposit {
        /// The Sui account the bridge mints hBTC to (`Utxo.derivation_path`).
        recipient: SuiAddressRaw,
        utxos: HashiUtxos,
        total: u64,
    },
}

/// Up to `MAX_HASHI_DEPOSIT_UTXOS` UTXOs, in command order.
#[derive(Clone, Copy)]
pub struct HashiUtxos {
    len: u8,
    items: [HashiUtxo; MAX_HASHI_DEPOSIT_UTXOS],
}

impl HashiUtxos {
    pub const fn new() -> Self {
        HashiUtxos {
            len: 0,
            items: [HashiUtxo {
                txid: [0; 32],
                vout: 0,
                amount: 0,
            }; MAX_HASHI_DEPOSIT_UTXOS],
        }
    }

    /// `None` when full.
    pub fn push(&mut self, utxo: HashiUtxo) -> Option<()> {
        *self.items.get_mut(self.len as usize)? = utxo;
        self.len += 1;
        Some(())
    }

    pub fn as_slice(&self) -> &[HashiUtxo] {
        &self.items[..self.len as usize]
    }
}

impl Default for HashiUtxos {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
pub struct HashiTx {
    pub deployment: u8,
    pub op: HashiOp,
}
