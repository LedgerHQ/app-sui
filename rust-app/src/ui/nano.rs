use crate::interface::*;
use crate::utils::*;

extern crate alloc;
use alloc::format;

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
            &[&"Provide Public Key"],
            None,
            &"Approve",
            Some(&CHECKMARK),
            &"Reject",
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
        gas_budget: u64,
    ) -> Option<()> {
        let fields = [
            Field {
                name: "From",
                value: &format!("{address}"),
            },
            Field {
                name: "To",
                value: &format!("0x{}", HexSlice(&recipient)),
            },
            Field {
                name: "Amount",
                value: {
                    let (quotient, remainder_str) = get_amount_in_decimals(total_amount);
                    &format!("SUI {}.{}", quotient, remainder_str.as_str())
                },
            },
            Field {
                name: "Max Gas",
                value: {
                    let (quotient, remainder_str) = get_amount_in_decimals(gas_budget);
                    &format!("SUI {}.{}", quotient, remainder_str.as_str())
                },
            },
        ];
        let success = MultiFieldReview::new(
            &fields,
            &[&"Review", &"transaction"],
            Some(&EYE),
            &"Accept and send",
            Some(&CHECKMARK),
            &"Reject",
            Some(&CROSS),
        )
        .show();
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
            &[&"WARNING transaction", &"not recognized"],
            Some(&WARNING),
            &"Accept and send",
            Some(&CHECKMARK),
            &"Reject",
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
