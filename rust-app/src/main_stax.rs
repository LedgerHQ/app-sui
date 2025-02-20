use crate::ctx::RunCtx;
use crate::handle_apdu::*;
use crate::interface::*;
use crate::settings::*;
use crate::ui::{UserInterface, APP_ICON};

use alamgu_async_block::*;

use core::cell::RefCell;
use core::pin::Pin;
use pin_cell::*;

use ledger_device_sdk::io;
use ledger_device_sdk::nbgl::{init_comm, NbglHomeAndSettings};
use ledger_log::{info, trace};

#[allow(dead_code)]
pub fn app_main(ctx: &RunCtx) {
    let comm: SingleThreaded<RefCell<io::Comm>> = SingleThreaded(RefCell::new(io::Comm::new()));

    let hostio_state: SingleThreaded<RefCell<HostIOState>> =
        SingleThreaded(RefCell::new(HostIOState::new(unsafe {
            core::mem::transmute::<
                &core::cell::RefCell<ledger_device_sdk::io::Comm>,
                &core::cell::RefCell<ledger_device_sdk::io::Comm>,
            >(&comm.0)
        })));
    let hostio: SingleThreaded<HostIO> = SingleThreaded(HostIO(unsafe {
        core::mem::transmute::<
            &core::cell::RefCell<alamgu_async_block::HostIOState>,
            &core::cell::RefCell<alamgu_async_block::HostIOState>,
        >(&hostio_state.0)
    }));
    let states_backing: SingleThreaded<PinCell<Option<APDUsFuture>>> =
        SingleThreaded(PinCell::new(None));
    let states: SingleThreaded<Pin<&PinCell<Option<APDUsFuture>>>> =
        SingleThreaded(Pin::static_ref(unsafe {
            core::mem::transmute::<
                &pin_cell::PinCell<core::option::Option<APDUsFuture>>,
                &pin_cell::PinCell<core::option::Option<APDUsFuture>>,
            >(&states_backing.0)
        }));
    // SAFETY:
    // Prolong the lifetime of `ctx``, because it should outlive the `APDUsFuture` future.
    // It is safe because `ctx` comes from the caller and is guaranteed to outlive the future.
    let ctx: &'static _ = unsafe { &*(ctx as *const RunCtx) };

    let mut settings = Settings;

    // Initialize reference to Comm instance for NBGL
    // API calls.
    init_comm(&mut comm.borrow_mut());

    info!("Sui {}", env!("CARGO_PKG_VERSION"));
    info!(
        "State sizes\ncomm: {}\nstates: {}",
        core::mem::size_of::<io::Comm>(),
        core::mem::size_of::<Option<APDUsFuture>>()
    );

    let settings_strings = [[
        "Blind Signing",
        "Sign transactions for which details cannot be verified",
    ]];

    let main_menu = SingleThreaded(RefCell::new(
        NbglHomeAndSettings::new()
            .glyph(&APP_ICON)
            .settings(settings.get_mut(), &settings_strings)
            .infos("Sui", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_AUTHORS")),
    ));
    let do_refresh_val = true;
    let do_refresh = SingleThreaded(RefCell::new(do_refresh_val));
    let ui = UserInterface {
        main_menu: unsafe {
            core::mem::transmute::<
                &core::cell::RefCell<ledger_device_sdk::nbgl::NbglHomeAndSettings>,
                &core::cell::RefCell<ledger_device_sdk::nbgl::NbglHomeAndSettings>,
            >(&main_menu.0)
        },
        do_refresh: unsafe {
            core::mem::transmute::<&core::cell::RefCell<bool>, &core::cell::RefCell<bool>>(
                &do_refresh.0,
            )
        },
    };

    let menu = |states: core::cell::Ref<'_, Option<APDUsFuture>>| {
        if ctx.is_swap() {
            return;
        }
        if states.is_none() {
            ui.show_main_menu()
        }
    };

    loop {
        if ctx.is_swap_finished() {
            return;
        }

        // This must be here, before handle_apdu
        // somehow doesn't work if its after handle_apdu
        menu(states.borrow());
        let ins: Ins = comm.borrow_mut().next_command();

        let poll_rv = poll_apdu_handlers(
            PinMut::as_mut(&mut states.0.borrow_mut()),
            ins,
            *hostio,
            |io, ins| handle_apdu_async(io, ins, ctx, settings, ui),
        );
        match poll_rv {
            Ok(()) => {
                trace!("APDU accepted; sending response");
                if ctx.is_swap() {
                    comm.borrow_mut().swap_reply_ok();
                } else {
                    comm.borrow_mut().reply_ok();
                }
                trace!("Replied");
            }
            Err(sw) => {
                PinMut::as_mut(&mut states.0.borrow_mut()).set(None);
                if ctx.is_swap() {
                    comm.borrow_mut().swap_reply(sw);
                } else {
                    comm.borrow_mut().reply(sw);
                }
            }
        };
    }
}
