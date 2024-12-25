use crate::implementation::*;
use crate::interface::*;
use crate::menu::*;
use crate::settings::*;
use crate::swap;

use alamgu_async_block::*;

use ledger_device_sdk::io;
use ledger_device_sdk::libcall;
use ledger_device_sdk::libcall::swap::SwapResult;
use ledger_device_sdk::libcall::swap::{
    get_check_address_params, get_printable_amount_params, sign_tx_params, swap_return,
};
use ledger_device_sdk::libcall::LibCallCommand;
use ledger_device_sdk::uxapp::{UxEvent, BOLOS_UX_OK};
use ledger_log::{info, trace};
use ledger_prompts_ui::{handle_menu_button_event, show_menu};

use core::cell::RefCell;
use core::pin::Pin;
use pin_cell::*;

#[allow(dead_code)]
pub fn app_main() {
    let comm: SingleThreaded<RefCell<io::Comm>> = SingleThreaded(RefCell::new(io::Comm::new()));

    let hostio_state: SingleThreaded<RefCell<HostIOState>> =
        SingleThreaded(RefCell::new(HostIOState::new(unsafe {
            core::mem::transmute(&comm.0)
        })));
    let hostio: SingleThreaded<HostIO> =
        SingleThreaded(HostIO(unsafe { core::mem::transmute(&hostio_state.0) }));
    let states_backing: SingleThreaded<PinCell<Option<APDUsFuture>>> =
        SingleThreaded(PinCell::new(None));
    let states: SingleThreaded<Pin<&PinCell<Option<APDUsFuture>>>> =
        SingleThreaded(Pin::static_ref(unsafe {
            core::mem::transmute(&states_backing.0)
        }));

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
                busy: &BusyMenu| match states.is_none() {
        true => show_menu(idle),
        _ => show_menu(busy),
    };

    // Draw some 'welcome' screen
    menu(states.borrow(), &idle_menu, &busy_menu);
    loop {
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
                    |io, ins| handle_apdu_async(io, ins, idle_menu.settings),
                );
                match poll_rv {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        comm.borrow_mut().reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => {
                        PinMut::as_mut(&mut states.0.borrow_mut()).set(None);
                        comm.borrow_mut().reply(sw);
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
                    UxEvent::block();
                    // Redisplay application menu here
                    menu(states.borrow(), &idle_menu, &busy_menu);
                }
                //trace!("Ignoring ticker event");
            }
        }
    }
}

// We are single-threaded in fact, albeit with nontrivial code flow. We don't need to worry about
// full atomicity of the below globals.
struct SingleThreaded<T>(T);
unsafe impl<T> Send for SingleThreaded<T> {}
unsafe impl<T> Sync for SingleThreaded<T> {}
impl<T> core::ops::Deref for SingleThreaded<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> core::ops::DerefMut for SingleThreaded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

pub fn lib_main(arg0: u32) {
    let cmd = libcall::get_command(arg0);

    match cmd {
        LibCallCommand::SwapCheckAddress => {
            let mut params = get_check_address_params(arg0);
            let is_matched = swap::check_address(&mut params).unwrap();

            swap_return(SwapResult::CheckAddressResult(
                &mut params,
                is_matched as i32,
            ));
        }
        LibCallCommand::SwapGetPrintableAmount => {
            let mut params = get_printable_amount_params(arg0);
            let amount_str = swap::get_printable_amount(&mut params).unwrap();

            swap_return(SwapResult::PrintableAmountResult(
                &mut params,
                amount_str.as_str(),
            ));
        }
        LibCallCommand::SwapSignTransaction => {
            let mut params = sign_tx_params(arg0);
            let res = swap::sign_transaction(&mut params).unwrap();

            swap_return(SwapResult::CreateTxResult(&mut params, res));
        }
    }
}
