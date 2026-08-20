use crate::ctx::RunCtx;
use crate::interface::*;
use crate::parser::common::SUI_COIN_DECIMALS;
use crate::swap::params::TxParams;
use crate::ui::common::*;
use crate::utils::*;

extern crate alloc;
use alloc::format;
use alloc::string::ToString;

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
        ctx: &RunCtx,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
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

        let do_review = |fields, ticker| {
            let first_msg = &format!("Review transaction to transfer {ticker}");
            let last_msg = &format!("Sign transaction to transfer {ticker}");
            NbglReview::new()
                .glyph(&APP_ICON)
                .titles(first_msg, "", last_msg)
                .show(fields)
        };
        let success = match coin_fields {
            Left(ticker) => do_review(&[from, to, amt, gas], ticker.as_str()),
            Right((coin_str, id_str)) => {
                let coin = Field {
                    name: coin_str.as_str(),
                    value: id_str.as_str(),
                };
                do_review(&[from, to, coin, amt, gas], "coins")
            }
        };
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
        recipient: [u8; 32],
        total_amount: u64,
        gas_budget: u64,
        gas_from_address_balance: bool,
        includes_gas_coin: bool,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
        let to = Field {
            name: "Validator",
            value: if recipient == LEDGER_STAKE_ADDRESS {
                "Ledger by P2P"
            } else {
                &format!("0x{}", HexSlice(&recipient))
            },
        };
        let gas_val = format_gas_amount(
            gas_budget,
            GasSource::new(gas_from_address_balance, includes_gas_coin),
        );
        let gas = Field {
            name: "Max Gas",
            value: &gas_val,
        };

        let (quotient, remainder_str) = get_amount_in_decimals(total_amount, SUI_COIN_DECIMALS);
        // Staking the gas coin by value stakes at most this much: gas comes out of
        // it (B2CA-2793 follow-up finding 2).
        let amt = Field {
            name: if includes_gas_coin {
                "Stake amount (max)"
            } else {
                "Stake amount"
            },
            value: &format!("SUI {}.{}", quotient, remainder_str.as_str()),
        };

        let do_review = |fields| {
            let first_msg = "Review transaction to stake SUI".to_string();
            let last_msg = "Sign transaction to stake SUI".to_string();
            NbglReview::new()
                .glyph(&APP_ICON)
                .titles(&first_msg, "", &last_msg)
                .show(fields)
        };
        let success = do_review(&[from, amt, to, gas]);
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
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
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

        let do_review = |fields| {
            let first_msg = "Review transaction to unstake SUI".to_string();
            let last_msg = "Sign transaction to unstake SUI".to_string();
            NbglReview::new()
                .glyph(&APP_ICON)
                .titles(&first_msg, "", &last_msg)
                .show(fields)
        };
        let success = do_review(&[from, amt, gas]);
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
