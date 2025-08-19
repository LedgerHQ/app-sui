use crate::interface::*;
use crate::parser::common::{CoinType, SUI_COIN_DECIMALS};
use crate::ui::common::*;
use crate::utils::*;

extern crate alloc;
use alloc::format;
use alloc::string::ToString;

use core::cell::RefCell;
use either::*;
use include_gif::include_gif;
use ledger_crypto_helpers::common::HexSlice;
use ledger_crypto_helpers::hasher::HexHash;
use ledger_device_sdk::nbgl::*;

pub const APP_ICON: NbglGlyph = NbglGlyph::from_include(include_gif!("sui_64x64.gif", NBGL));

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
            .verify_str("Provide Public Key")
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
        recipient: [u8; 32],
        total_amount: u64,
        coin_type: CoinType,
        gas_budget: u64,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
        let to = Field {
            name: "To",
            value: &format!("0x{}", HexSlice(&recipient)),
        };
        let gas = Field {
            name: "Max Gas",
            value: {
                let (quotient, remainder_str) =
                    get_amount_in_decimals(gas_budget, SUI_COIN_DECIMALS);
                &format!("SUI {}.{}", quotient, remainder_str.as_str())
            },
        };
        let ((amt_str, amt_val), coin_fields) = get_coin_and_amount_fields(total_amount, coin_type);
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
            }
        };
        let gas = Field {
            name: "Max Gas",
            value: {
                let (quotient, remainder_str) =
                    get_amount_in_decimals(gas_budget, SUI_COIN_DECIMALS);
                &format!("SUI {}.{}", quotient, remainder_str.as_str())
            },
        };

        let (quotient, remainder_str) = get_amount_in_decimals(total_amount, SUI_COIN_DECIMALS);
        let amt = Field {
            name: "Stake amount",
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
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let from = Field {
            name: "From",
            value: &format!("{address}"),
        };
        let gas = Field {
            name: "Max Gas",
            value: {
                let (quotient, remainder_str) =
                    get_amount_in_decimals(gas_budget, SUI_COIN_DECIMALS);
                &format!("SUI {}.{}", quotient, remainder_str.as_str())
            },
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
