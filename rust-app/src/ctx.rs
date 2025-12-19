use crate::parser::common::{COIN_STRING_LENGTH, SUI_ADDRESS_LENGTH, SUI_COIN_DECIMALS};
use crate::swap::params::TxParams;

use arrayvec::ArrayString;
use core::cell::Cell;

pub const TICKER_LENGTH: usize = 8;

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
    token_coin_id: Cell<[u8; SUI_ADDRESS_LENGTH]>,
    token_coin_module: Cell<ArrayString<COIN_STRING_LENGTH>>,
    token_coin_function: Cell<ArrayString<COIN_STRING_LENGTH>>,
    token_divisor: Cell<u8>,
    token_ticker: Cell<ArrayString<TICKER_LENGTH>>,
}

impl RunCtx {
    pub fn app() -> Self {
        RunCtx {
            state: Cell::new(State::App),
            tx_params: TxParams::default(),
            token_coin_id: Cell::new([0; SUI_ADDRESS_LENGTH]),
            token_coin_module: Cell::new(ArrayString::zero_filled()),
            token_coin_function: Cell::new(ArrayString::zero_filled()),
            token_divisor: Cell::new(SUI_COIN_DECIMALS),
            token_ticker: Cell::new(ArrayString::zero_filled()),
        }
    }

    pub fn lib_swap(tx_params: TxParams) -> Self {
        RunCtx {
            state: Cell::new(State::LibSwapIdle),
            tx_params,
            token_coin_id: Cell::new([0; SUI_ADDRESS_LENGTH]),
            token_coin_module: Cell::new(ArrayString::zero_filled()),
            token_coin_function: Cell::new(ArrayString::zero_filled()),
            token_divisor: Cell::new(SUI_COIN_DECIMALS),
            token_ticker: Cell::new(ArrayString::zero_filled()),
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

    pub fn set_token(
        &self,
        coin_id: [u8; SUI_ADDRESS_LENGTH],
        coin_module: ArrayString<COIN_STRING_LENGTH>,
        coin_function: ArrayString<COIN_STRING_LENGTH>,
        divisor: u8,
        ticker: ArrayString<TICKER_LENGTH>,
    ) {
        self.token_coin_id.set(coin_id);
        self.token_coin_module.set(coin_module);
        self.token_coin_function.set(coin_function);
        self.token_divisor.set(divisor);
        self.token_ticker.set(ticker);
    }

    pub fn get_token_coin_id(&self) -> [u8; SUI_ADDRESS_LENGTH] {
        self.token_coin_id.get()
    }

    pub fn get_token_coin_module(&self) -> ArrayString<COIN_STRING_LENGTH> {
        self.token_coin_module.get()
    }

    pub fn get_token_coin_function(&self) -> ArrayString<COIN_STRING_LENGTH> {
        self.token_coin_function.get()
    }

    pub fn get_token_divisor(&self) -> u8 {
        self.token_divisor.get()
    }

    pub fn get_token_ticker(&self) -> ArrayString<TICKER_LENGTH> {
        self.token_ticker.get()
    }
}
