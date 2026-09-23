use crate::ctx::RunCtx;
use crate::interface::*;
use crate::parser::common::SUI_COIN_DECIMALS;
use crate::parser::tx::TxPrincipals;
use crate::swap::params::TxParams;
use crate::ui::common::*;
use crate::utils::*;

extern crate alloc;
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::crypto_helpers::common::{Address, HexSlice};
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

/// The accounts a review names, rendered for display and kept alive by the
/// caller so the `Field`s below can borrow them.
///
/// Every role is read off the signed transaction, never off the signing path.
/// A Sui multisig address is derived from the multisig policy rather than from
/// any constituent key, so this device can hold a key that authorizes the
/// sender or the gas owner while its own address equals neither. Sponsorship
/// therefore follows from the principals alone, and the derived address is
/// shown as the key it is rather than standing in for an account.
struct Principals {
    /// `TransactionData.sender`: the account the transaction is from, and whose
    /// effects it accrues to.
    sender: alloc::string::String,
    /// `GasData.owner`, shown whenever it is not the sender, i.e. exactly when
    /// the transaction is sponsored.
    gas_owner: Option<alloc::string::String>,
    /// The derived address, shown when the transaction names neither principal
    /// with it: the device is then signing *for* an account rather than *as*
    /// one, and the address must not stand in for either role.
    signing_key: Option<alloc::string::String>,
}

fn principal_values(principals: &TxPrincipals, address: &SuiPubKeyAddress) -> Principals {
    Principals {
        sender: format!("0x{}", HexSlice(&principals.sender)),
        gas_owner: principals
            .is_sponsored()
            .then(|| format!("0x{}", HexSlice(&principals.gas_owner))),
        signing_key: (!principals.names(address.get_binary_address()))
            .then(|| format!("{address}")),
    }
}

impl Principals {
    /// The fields naming the accounts, in the order they are reviewed: who the
    /// transaction is from, who pays for it, and which key is signing.
    ///
    /// `from` names the sender field, which some reviews label by the role the
    /// sender plays in that particular transaction.
    fn fields(&self, from: &'static str) -> Vec<Field<'_>> {
        let mut fields = alloc::vec![Field {
            name: from,
            value: self.sender.as_str(),
        }];
        fields.extend(self.gas_owner.as_ref().map(|v| Field {
            name: "Gas paid by",
            value: v.as_str(),
        }));
        fields.extend(self.signing_key.as_ref().map(|v| Field {
            name: "Signing key",
            value: v.as_str(),
        }));
        fields
    }

    /// What to call the transaction in the review titles.
    fn kind(&self) -> &'static str {
        if self.gas_owner.is_some() {
            "sponsored transaction"
        } else {
            "transaction"
        }
    }
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
        principals: &TxPrincipals,
        replay: Option<ReplayDomain>,
        ctx: &RunCtx,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        // Sponsored: some other account pays the gas for this transfer, and its
        // gas coin funds the PTB. Naming only one account would read as the
        // user's own transaction.
        let who = principal_values(principals, address);
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

        let kind = who.kind();
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

        let mut fields: Vec<Field> = who.fields("From");
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
        principals: &TxPrincipals,
        replay: Option<ReplayDomain>,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        // request_add_stake credits the resulting StakedSui to the sender, so on a
        // sponsored stake the position belongs to that account and not to whoever
        // pays for it. The sender field is labelled by that role there, because
        // for a stake the beneficiary is the fact that matters.
        let who = principal_values(principals, address);
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

        let kind = who.kind();
        let mut fields: Vec<Field> = who.fields(if principals.is_sponsored() {
            "Stake owner"
        } else {
            "From"
        });
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
        principals: &TxPrincipals,
        replay: Option<ReplayDomain>,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        // See confirm_sign_tx.
        let who = principal_values(principals, address);
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

        let kind = who.kind();
        let mut fields: Vec<Field> = who.fields("From");
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
