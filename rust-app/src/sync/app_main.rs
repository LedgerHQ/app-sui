use crate::interface::*;
use crate::settings::*;
use crate::sync::ctx::RunCtx;
use crate::sync::handle_apdu::handle_apdu;
use crate::sync::ui::APP_ICON;

use ledger_device_sdk::nbgl::{init_comm, NbglHomeAndSettings};
use ledger_device_sdk::{info, trace};

pub fn app_main(ctx: &mut RunCtx) {
    let mut settings = Settings;

    // Initialize reference to Comm instance for NBGL
    // API calls.
    init_comm(&mut ctx.comm);

    info!("Sui {}", env!("CARGO_PKG_VERSION"));

    let settings_strings = [[
        "Blind Signing",
        "Sign transactions for which details cannot be verified",
    ]];

    let mut menu = NbglHomeAndSettings::new()
        .glyph(&APP_ICON)
        .settings(settings.get_mut(), &settings_strings)
        .infos("Sui", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS"));

    menu.show_and_return();

    loop {
        let ins: Ins = ctx.comm.next_command();

        match handle_apdu(ctx, ins) {
            Ok(()) => {
                trace!("APDU handled successfully");
                ctx.comm.reply_ok();
            }
            Err(_e) => {
                trace!("Error during APDU handling");
                ctx.comm.reply(ledger_device_sdk::io::StatusWords::Unknown);
            }
        }
    }
}
