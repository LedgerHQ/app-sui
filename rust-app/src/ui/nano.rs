use crate::interface::*;
use crate::utils::*;
use core::fmt::Write;
use ledger_crypto_helpers::common::HexSlice;
use ledger_crypto_helpers::hasher::HexHash;
use ledger_prompts_ui::*;

#[derive(Copy, Clone)]
pub struct UserInterface {}

impl UserInterface {
    pub fn confirm_address(&self, address: &SuiPubKeyAddress) -> Option<()> {
        scroller("Provide Public Key", |_w| Ok(()))?;
        scroller_paginated("Address", |w| Ok(write!(w, "{address}")?))?;
        final_accept_prompt(&[])
    }

    pub fn confirm_sign_tx(
        &self,
        address: &SuiPubKeyAddress,
        recipient: [u8; 32],
        total_amount: u64,
        gas_budget: u64,
    ) -> Option<()> {
        scroller("Transfer", |w| Ok(write!(w, "SUI")?))?;

        scroller_paginated("From", |w| Ok(write!(w, "{address}")?))?;
        scroller_paginated("To", |w| Ok(write!(w, "0x{}", HexSlice(&recipient))?))?;

        let (quotient, remainder_str) = get_amount_in_decimals(total_amount);
        scroller_paginated("Amount", |w| {
            Ok(write!(w, "SUI {quotient}.{}", remainder_str.as_str())?)
        })?;

        let (quotient, remainder_str) = get_amount_in_decimals(gas_budget);
        scroller("Max Gas", |w| {
            Ok(write!(w, "SUI {}.{}", quotient, remainder_str.as_str())?)
        })?;
        final_accept_prompt(&["Sign Transaction?"])
    }

    pub fn confirm_blind_sign_tx(&self, hash: &HexHash<32>) -> Option<()> {
        scroller("WARNING", |w| Ok(write!(w, "Transaction not recognized")?))?;
        scroller("Transaction Hash", |w| Ok(write!(w, "0x{hash}")?))?;
        final_accept_prompt(&["Blind Sign Transaction?"])
    }

    pub fn warn_tx_not_recognized(&self) {
        scroller("WARNING", |w| {
            Ok(write!(
                w,
                "Transaction not recognized, enable blind signing to sign unknown transactions"
            )?)
        });
    }
}

#[cfg(not(target_os = "nanos"))]
#[inline(never)]
pub fn scroller<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    title: &str,
    prompt_function: F,
) -> Option<()> {
    ledger_prompts_ui::write_scroller_three_rows(false, title, prompt_function)
}

#[cfg(target_os = "nanos")]
#[inline(never)]
pub fn scroller<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    title: &str,
    prompt_function: F,
) -> Option<()> {
    ledger_prompts_ui::write_scroller(false, title, prompt_function)
}

#[cfg(not(target_os = "nanos"))]
#[inline(never)]
pub fn scroller_paginated<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    title: &str,
    prompt_function: F,
) -> Option<()> {
    ledger_prompts_ui::write_scroller_three_rows(true, title, prompt_function)
}

#[cfg(target_os = "nanos")]
#[inline(never)]
pub fn scroller_paginated<F: for<'b> Fn(&mut PromptWrite<'b, 16>) -> Result<(), ScrollerError>>(
    title: &str,
    prompt_function: F,
) -> Option<()> {
    ledger_prompts_ui::write_scroller(true, title, prompt_function)
}
