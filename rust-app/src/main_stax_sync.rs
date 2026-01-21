use crate::ctx::RunCtx;
use crate::ctx_sync::Context;
use crate::interface::*;
use crate::settings::*;
use crate::ui::APP_ICON;

use ledger_device_sdk::io::Comm;
use ledger_device_sdk::nbgl::{init_comm, NbglHomeAndSettings};
use ledger_device_sdk::{info, trace};

pub fn app_main(_ctx: &RunCtx) {
    let mut comm = Comm::new().set_expected_cla(0x00);
    let mut cmd_ctx = Context::new();

    let mut settings = Settings;

    // Initialize reference to Comm instance for NBGL
    // API calls.
    init_comm(&mut comm);

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
        let ins: Ins = comm.next_command();

        match cmd_ctx.handle_apdu(&mut comm, ins) {
            Ok(()) => {}
            Err(e) => {
                let _ = e;
                trace!("Error during APDU handling: {:?}", e);
                comm.reply(ledger_device_sdk::io::StatusWords::Unknown);
            }
        }
    }
}
