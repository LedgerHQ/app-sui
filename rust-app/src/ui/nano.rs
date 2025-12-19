use crate::ctx::RunCtx;
use crate::interface::*;
use crate::parser::common::{CoinType, SUI_COIN_DECIMALS};
use crate::ui::common::*;
use crate::utils::*;

extern crate alloc;
use alloc::format;

use either::*;
use ledger_crypto_helpers::common::HexSlice;
use ledger_crypto_helpers::hasher::HexHash;

use ledger_device_sdk::buttons::ButtonEvent;
use ledger_device_sdk::ui::bitmaps::{CHECKMARK, CROSS, EYE, WARNING};
use ledger_device_sdk::ui::gadgets::*;

#[derive(Copy, Clone)]
pub struct UserInterface {}

impl UserInterface {
    pub fn confirm_address(&self, address: &SuiPubKeyAddress) -> Option<()> {
        let fields = [Field {
            name: "Address",
            value: &format!("{address}"),
        }];
        let success = MultiFieldReview::new(
            &fields,
            &["Provide Public Key"],
            None,
            "Approve",
            Some(&CHECKMARK),
            "Reject",
            Some(&CROSS),
        )
        .show();
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
        ctx: &RunCtx,
    ) -> Option<()> {
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
        let ((amt_str, amt_val), coin_fields) =
            get_coin_and_amount_fields(total_amount, coin_type, ctx);
        let amt = Field {
            name: amt_str.as_str(),
            value: amt_val.as_str(),
        };

        let do_review = |fields| {
            MultiFieldReview::new(
                fields,
                &["Review", "transaction"],
                Some(&EYE),
                "Accept and send",
                Some(&CHECKMARK),
                "Reject",
                Some(&CROSS),
            )
            .show()
        };
        let success = match coin_fields {
            Left(_ticker) => do_review(&[from, to, amt, gas]),
            Right((coin_str, id_str)) => {
                let coin = Field {
                    name: coin_str.as_str(),
                    value: id_str.as_str(),
                };
                do_review(&[from, to, coin, amt, gas])
            }
        };
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
            MultiFieldReview::new(
                fields,
                &["Review", "transaction"],
                Some(&EYE),
                "Accept and send",
                Some(&CHECKMARK),
                "Reject",
                Some(&CROSS),
            )
            .show()
        };
        let success = do_review(&[from, amt, to, gas]);
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
            MultiFieldReview::new(
                fields,
                &["Review", "transaction"],
                Some(&EYE),
                "Accept and send",
                Some(&CHECKMARK),
                "Reject",
                Some(&CROSS),
            )
            .show()
        };
        let success = do_review(&[from, amt, gas]);
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn confirm_blind_sign_tx(&self, hash: &HexHash<32>) -> Option<()> {
        let fields = [Field {
            name: "Transaction hash",
            value: &format!("0x{hash}"),
        }];
        let success = MultiFieldReview::new(
            &fields,
            &["WARNING transaction", "not recognized"],
            Some(&WARNING),
            "Accept and send",
            Some(&CHECKMARK),
            "Reject",
            Some(&CROSS),
        )
        .show();
        if success {
            Some(())
        } else {
            None
        }
    }

    pub fn warn_tx_not_recognized(&self) {
        let field = Field {
            name: "WARNING",
            value: "transaction not recognized, enable blind signing to sign unknown transactions",
        };
        field.event_loop(ButtonEvent::RightButtonRelease, true);
    }
}
