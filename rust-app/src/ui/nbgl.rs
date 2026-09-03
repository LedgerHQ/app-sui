use crate::ctx::RunCtx;
use crate::interface::*;
use crate::parser::common::{SuiAddressRaw, SUI_COIN_DECIMALS};
use crate::swap::params::TxParams;
use crate::ui::common::*;
use crate::utils::*;

extern crate alloc;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::crypto_helpers::common::HexSlice;
use crate::crypto_helpers::hasher::HexHash;
use core::cell::RefCell;
use either::*;
use ledger_device_sdk::nbgl::*;

use super::*;

#[derive(Copy, Clone)]
pub struct UserInterface {
    pub main_menu: &'static RefCell<NbglHomeAndSettings>,
    pub do_refresh: &'static RefCell<bool>,
}

/// The replay domain rendered for display, kept alive by the caller so the
/// `Field`s below can borrow it.
fn replay_values(
    replay: &Option<ReplayDomain>,
) -> Option<(alloc::string::String, alloc::string::String)> {
    replay.map(|r| {
        let chain = match chain_name(&r.chain) {
            Some(name) => name.to_string(),
            None => format!("0x{}", HexSlice(&r.chain)),
        };
        (chain, format!("{}", r.nonce))
    })
}

fn replay_fields(vals: &Option<(alloc::string::String, alloc::string::String)>) -> Vec<Field<'_>> {
    match vals {
        Some((chain, nonce)) => alloc::vec![
            Field {
                name: "Network",
                value: chain.as_str(),
            },
            Field {
                name: "Nonce",
                value: nonce.as_str(),
            },
        ],
        None => Vec::new(),
    }
}

impl UserInterface {
    pub fn show_main_menu(&self) {
        let refresh = self.do_refresh.replace(false);
        if refresh {
            self.main_menu.borrow_mut().show_and_return();
        }
    }

    pub fn confirm_address(&self, address: &SuiPubKeyAddress) -> Option<()> {
        self.do_refresh.replace(true);
        let success = NbglAddressReview::new()
            .glyph(&APP_ICON)
            .review_title("Provide Public Key")
            .show(&format!("{address}"));
        NbglReviewStatus::new()
            .status_type(StatusType::Address)
            .show(success);
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn confirm_sign_tx(
        &self,
        address: &SuiPubKeyAddress,
        params: &TxParams,
        sponsored_sender: Option<SuiAddressRaw>,
        replay: Option<ReplayDomain>,
        ctx: &RunCtx,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
        // Sponsored: this device pays the gas for a transaction someone else sent,
        // so its gas coin funds their PTB. "From" alone would read as the user's
        // own transaction.
        let sponsor_val = sponsored_sender.map(|s| format!("0x{}", HexSlice(&s)));
        let sponsor = sponsor_val.as_ref().map(|v| Field {
            name: "Sent by",
            value: v.as_str(),
        });
        let to = Field {
            name: "To",
            value: &format!("0x{}", HexSlice(&params.destination_address)),
        };
        let gas_val = format_gas_amount(
            params.fee,
            GasSource::new(params.gas_from_address_balance, params.includes_gas_coin),
        );
        let gas = Field {
            name: "Max Gas",
            value: &gas_val,
        };
        let ((amt_str, amt_val), coin_fields) = get_coin_and_amount_fields(
            params.amount,
            params.coin_type,
            ctx,
            params.includes_gas_coin,
        );
        let amt = Field {
            name: amt_str.as_str(),
            value: amt_val.as_str(),
        };

        let replay_vals = replay_values(&replay);

        let kind = if sponsor.is_some() {
            "sponsored transaction"
        } else {
            "transaction"
        };
        let (ticker, coin_field) = match &coin_fields {
            Left(ticker) => (ticker.as_str(), None),
            Right((coin_str, id_str)) => (
                "coins",
                Some(Field {
                    name: coin_str.as_str(),
                    value: id_str.as_str(),
                }),
            ),
        };

        let mut fields: Vec<Field> = Vec::new();
        fields.push(from);
        fields.extend(sponsor);
        fields.push(to);
        fields.extend(coin_field);
        fields.push(amt);
        fields.push(gas);
        fields.extend(replay_fields(&replay_vals));

        let first_msg = &format!("Review {kind} to transfer {ticker}");
        let last_msg = &format!("Sign {kind} to transfer {ticker}");
        let success = NbglReview::new()
            .glyph(&APP_ICON)
            .titles(first_msg, "", last_msg)
            .show(&fields);
        NbglReviewStatus::new()
            .status_type(StatusType::Transaction)
            .show(success);
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn confirm_stake_tx(
        &self,
        address: &SuiPubKeyAddress,
        params: &StakeParams,
        sponsored_sender: Option<SuiAddressRaw>,
        replay: Option<ReplayDomain>,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
        // request_add_stake credits the resulting StakedSui to the sender, so on a
        // sponsored stake the position belongs to that account and not to the
        // signer paying for it. Labelled by that role rather than "Sent by" as
        // elsewhere, because for a stake the beneficiary is the fact that matters
        // and the two are the same address.
        let sponsor_val = sponsored_sender.map(|s| format!("0x{}", HexSlice(&s)));
        let sponsor = sponsor_val.as_ref().map(|v| Field {
            name: "Stake owner",
            value: v.as_str(),
        });
        let to = Field {
            name: "Validator",
            value: if params.recipient == LEDGER_STAKE_ADDRESS {
                "Ledger by P2P"
            } else {
                &format!("0x{}", HexSlice(&params.recipient))
            },
        };
        let gas_val = format_gas_amount(
            params.gas_budget,
            GasSource::new(params.gas_from_address_balance, params.includes_gas_coin),
        );
        let gas = Field {
            name: "Max Gas",
            value: &gas_val,
        };

        let (quotient, remainder_str) =
            get_amount_in_decimals(params.total_amount, SUI_COIN_DECIMALS);
        // Staking the gas coin by value stakes at most this much: gas comes out of
        // it (B2CA-2793 follow-up finding 2).
        let amt = Field {
            name: if params.includes_gas_coin {
                "Stake amount (max)"
            } else {
                "Stake amount"
            },
            value: &format!("SUI {}.{}", quotient, remainder_str.as_str()),
        };

        let replay_vals = replay_values(&replay);

        let kind = if sponsor.is_some() {
            "sponsored transaction"
        } else {
            "transaction"
        };
        let mut fields: Vec<Field> = Vec::new();
        fields.push(from);
        fields.extend(sponsor);
        fields.push(amt);
        fields.push(to);
        fields.push(gas);
        fields.extend(replay_fields(&replay_vals));

        let first_msg = format!("Review {kind} to stake SUI");
        let last_msg = format!("Sign {kind} to stake SUI");
        let success = NbglReview::new()
            .glyph(&APP_ICON)
            .titles(&first_msg, "", &last_msg)
            .show(&fields);
        NbglReviewStatus::new()
            .status_type(StatusType::Transaction)
            .show(success);
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn confirm_unstake_tx(
        &self,
        address: &SuiPubKeyAddress,
        total_amount: u64,
        gas_budget: u64,
        gas_from_address_balance: bool,
        sponsored_sender: Option<SuiAddressRaw>,
        replay: Option<ReplayDomain>,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
        // See confirm_sign_tx.
        let sponsor_val = sponsored_sender.map(|s| format!("0x{}", HexSlice(&s)));
        let sponsor = sponsor_val.as_ref().map(|v| Field {
            name: "Sent by",
            value: v.as_str(),
        });
        // Unstaking never consumes the gas coin as the unstaked object, so gas is
        // always charged separately here.
        let gas_val =
            format_gas_amount(gas_budget, GasSource::new(gas_from_address_balance, false));
        let gas = Field {
            name: "Max Gas",
            value: &gas_val,
        };

        let (quotient, remainder_str) = get_amount_in_decimals(total_amount, SUI_COIN_DECIMALS);
        let amt = Field {
            name: "Unstake amount",
            value: &format!("SUI {}.{}", quotient, remainder_str.as_str()),
        };

        let replay_vals = replay_values(&replay);

        let kind = if sponsor.is_some() {
            "sponsored transaction"
        } else {
            "transaction"
        };
        let mut fields: Vec<Field> = Vec::new();
        fields.push(from);
        fields.extend(sponsor);
        fields.push(amt);
        fields.push(gas);
        fields.extend(replay_fields(&replay_vals));

        let first_msg = format!("Review {kind} to unstake SUI");
        let last_msg = format!("Sign {kind} to unstake SUI");
        let success = NbglReview::new()
            .glyph(&APP_ICON)
            .titles(&first_msg, "", &last_msg)
            .show(&fields);
        NbglReviewStatus::new()
            .status_type(StatusType::Transaction)
            .show(success);
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn confirm_blind_sign_tx(&self, hash: &HexHash<32>) -> Option<()> {
        self.do_refresh.replace(true);
        let tx_fields = [Field {
            name: "Transaction hash",
            value: &format!("0x{hash}"),
        }];

        let success = NbglReview::new()
            .glyph(&APP_ICON)
            .blind()
            .titles("Review transaction", "", "Sign transaction")
            .show(&tx_fields);
        NbglReviewStatus::new()
            .status_type(StatusType::Transaction)
            .show(success);
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn warn_tx_not_recognized(&self) {
        let choice = NbglChoice::new().show(
            "This transaction cannot be clear-signed",
            "Enable blind-signing in the settings to sign this transaction",
            "Go to settings",
            "Reject transaction",
        );
        if choice {
            let mut mm = self.main_menu.borrow_mut();
            mm.set_start_page(PageIndex::Settings(0));
            mm.show_and_return();
            mm.set_start_page(PageIndex::Home);
        } else {
            self.do_refresh.replace(true);
        }
    }
}

/// Where the gas for this transaction is charged from. This is user-visible
/// information, not a detail: for `PaymentObjects` the gas is charged on top of the
/// reviewed amount (a separate coin pays it), while for `TransferredCoin` it comes
/// *out of* that amount, so the same two on-screen numbers must not be read the same
/// way (B2CA-2793 follow-up finding 2).
#[derive(Copy, Clone)]
pub enum GasSource {
    /// Gas paid by the transaction's own gas payment objects.
    PaymentObjects,
    /// SIP-58: empty `gas_data.payment`, gas paid from the sender's address balance.
    AddressBalance,
    /// The reviewed amount *is* the gas coin, so gas is deducted from it.
    TransferredCoin,
}

impl GasSource {
    pub fn new(gas_from_address_balance: bool, includes_gas_coin: bool) -> Self {
        // A GasCoin transfer/stake whose gas also comes from the address balance
        // cannot be resolved to a real balance and is rejected as an unrecognized
        // tx upstream (B2CA-2793 finding 4), so these cannot both hold here.
        if gas_from_address_balance {
            GasSource::AddressBalance
        } else if includes_gas_coin {
            GasSource::TransferredCoin
        } else {
            GasSource::PaymentObjects
        }
    }
}

pub fn format_gas_amount(gas_budget: u64, gas_source: GasSource) -> alloc::string::String {
    let (quotient, remainder_str) = get_amount_in_decimals(gas_budget, SUI_COIN_DECIMALS);
    match gas_source {
        GasSource::AddressBalance => {
            format!("SUI {}.{} (from balance)", quotient, remainder_str.as_str())
        }
        GasSource::TransferredCoin => {
            format!("SUI {}.{} (from amount)", quotient, remainder_str.as_str())
        }
        GasSource::PaymentObjects => format!("SUI {}.{}", quotient, remainder_str.as_str()),
    }
}
