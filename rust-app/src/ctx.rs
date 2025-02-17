use core::cell::Cell;

use crate::swap::params::TxParams;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum State {
    App = 0x00,
    LibSwapIdle,
    LibSwapSignSuccess,
    LibSwapSignFailure,
}

pub struct RunCtx {
    state: Cell<State>,
    tx_params: TxParams,
}

impl RunCtx {
    pub fn app() -> Self {
        RunCtx {
            state: Cell::new(State::App),
            tx_params: TxParams::default(),
        }
    }

    pub fn lib_swap(tx_params: TxParams) -> Self {
        RunCtx {
            state: Cell::new(State::LibSwapIdle),
            tx_params,
        }
    }

    pub fn is_swap(&self) -> bool {
        !matches!(self.state.get(), State::App)
    }

    pub fn is_swap_finished(&self) -> bool {
        matches!(
            self.state.get(),
            State::LibSwapSignSuccess | State::LibSwapSignFailure,
        )
    }

    pub fn is_swap_sign_succeeded(&self) -> bool {
        matches!(self.state.get(), State::LibSwapSignSuccess)
    }

    pub fn set_swap_sign_success(&self) {
        if self.is_swap() {
            self.state.set(State::LibSwapSignSuccess);
        }
    }

    pub fn set_swap_sign_failure(&self) {
        if self.is_swap() {
            self.state.set(State::LibSwapSignFailure);
        }
    }

    // Panics if not in swap mode
    pub fn get_swap_tx_params(&self) -> &TxParams {
        assert!(self.is_swap(), "attempt to get swap tx params in app mode");
        &self.tx_params
    }
}
