use crate::ctx::RunCtx;
use crate::handle_apdu::*;
use crate::interface::*;
use crate::menu::*;
use crate::settings::*;
use crate::ui::UserInterface;

use alamgu_async_block::*;

use ledger_device_sdk::io;
use ledger_device_sdk::uxapp::{UxEvent, BOLOS_UX_OK};
use ledger_log::{info, trace};
use ledger_prompts_ui::{handle_menu_button_event, show_menu};

use core::cell::RefCell;
use core::pin::Pin;
use pin_cell::*;

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

    let mut idle_menu = IdleMenuWithSettings {
        idle_menu: IdleMenu::AppMain,
        settings: Settings,
    };
    let mut busy_menu = BusyMenu::Working;

    info!("Sui {}", env!("CARGO_PKG_VERSION"));
    info!(
        "State sizes\ncomm: {}\nstates: {}",
        core::mem::size_of::<io::Comm>(),
        core::mem::size_of::<Option<APDUsFuture>>()
    );

    let menu = |states: core::cell::Ref<'_, Option<APDUsFuture>>,
                idle: &IdleMenuWithSettings,
                busy: &BusyMenu| {
        if ctx.is_swap() {
            return;
        }

        match states.is_none() {
            true => show_menu(idle),
            _ => show_menu(busy),
        }
    };

    // Draw some 'welcome' screen
    menu(states.borrow(), &idle_menu, &busy_menu);
    loop {
        if ctx.is_swap_finished() {
            return;
        }

        // Wait for either a specific button push to exit the app
        // or an APDU command
        let evt = comm.borrow_mut().next_event::<Ins>();
        match evt {
            io::Event::Command(ins) => {
                trace!("Command received");
                let poll_rv = poll_apdu_handlers(
                    PinMut::as_mut(&mut states.0.borrow_mut()),
                    ins,
                    *hostio,
                    |io, ins| handle_apdu_async(io, ins, ctx, idle_menu.settings, UserInterface {}),
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
                // Reset BusyMenu if we are done handling APDU
                if states.borrow().is_none() {
                    busy_menu = BusyMenu::Working;
                }
                menu(states.borrow(), &idle_menu, &busy_menu);
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match states.borrow().is_none() {
                    true => {
                        if let Some(DoExitApp) = handle_menu_button_event(&mut idle_menu, btn) {
                            info!("Exiting app at user direction via root menu");
                            ledger_device_sdk::exit_app(0)
                        }
                    }
                    _ => {
                        if let Some(DoCancel) = handle_menu_button_event(&mut busy_menu, btn) {
                            info!("Resetting at user direction via busy menu");
                            PinMut::as_mut(&mut states.borrow_mut()).set(None);
                        }
                    }
                };
                menu(states.borrow(), &idle_menu, &busy_menu);
                trace!("Button done");
            }
            io::Event::Ticker => {
                if UxEvent::Event.request() != BOLOS_UX_OK {
                    UxEvent::block_and_get_event::<Dummy>(&mut comm.borrow_mut());
                    // Redisplay application menu here
                    menu(states.borrow(), &idle_menu, &busy_menu);
                }
                //trace!("Ignoring ticker event");
            }
        }
    }
}

// Trick to manage pin code
use core::convert::TryFrom;
struct Dummy {}
impl TryFrom<io::ApduHeader> for Dummy {
    type Error = io::StatusWords;
    fn try_from(_header: io::ApduHeader) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}
