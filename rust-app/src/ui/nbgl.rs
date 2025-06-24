extern crate alloc;
use alloc::format;

use core::cell::RefCell;
use include_gif::include_gif;
use ledger_crypto_helpers::eddsa::Ed25519RawPubKeyAddress;
use ledger_crypto_helpers::hasher::Base64Hash;
use ledger_device_sdk::nbgl::*;

#[cfg(any(target_os = "stax", target_os = "flex"))]
pub const APP_ICON: NbglGlyph = NbglGlyph::from_include(include_gif!("crab_64x64.gif", NBGL));

#[cfg(not(any(target_os = "stax", target_os = "flex")))]
pub const APP_ICON: NbglGlyph = NbglGlyph::from_include(include_gif!("crab-small.gif", NBGL));

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

    pub fn confirm_address(&self, pkh: &Ed25519RawPubKeyAddress) -> Option<()> {
        self.do_refresh.replace(true);
        let success = NbglAddressReview::new()
            .glyph(&APP_ICON)
            .verify_str("Provide Public Key")
            .show(&format!("{pkh}"));
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
        pkh: &Ed25519RawPubKeyAddress,
        hash: &Base64Hash<32>,
    ) -> Option<()> {
        self.do_refresh.replace(true);
        let tx_fields = [
            Field {
                name: "Tx hash",
                value: &format!("{hash}"),
            },
            Field {
                name: "Address",
                value: &format!("{pkh}"),
            },
        ];

        let success = NbglReview::new()
            .glyph(&APP_ICON)
            .blind()
            .titles("Sign Transaction", "", "")
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
