use crate::parser::common::{COIN_STRING_LENGTH, SUI_ADDRESS_LENGTH, SUI_COIN_DECIMALS};
//use crate::swap::params::TxParams;
use crate::sync::ui::nbgl::UserInterface;
use arrayvec::ArrayString;
use ledger_device_sdk::nbgl::NbglHomeAndSettings;

pub mod block_protocol;
use block_protocol::BlockProtocolHandler;

pub const TICKER_LENGTH: usize = 8;

#[derive(Default, Clone, Copy)]
#[repr(u8)]
pub enum State {
    #[default]
    App = 0x00,
    LibSwapIdle,
    LibSwapSignSuccess,
    LibSwapSignFailure,
}

#[derive(Default)]
pub struct RunCtx {
    pub state: State,
    pub block_protocol_handler: BlockProtocolHandler,
    pub comm: ledger_device_sdk::io::Comm,
    pub ui: UserInterface,
    pub token_coin_id: [u8; SUI_ADDRESS_LENGTH],
    pub token_coin_module: ArrayString<COIN_STRING_LENGTH>,
    pub token_coin_function: ArrayString<COIN_STRING_LENGTH>,
    pub token_divisor: u8,
    pub token_ticker: ArrayString<TICKER_LENGTH>,
}

impl RunCtx {
    // pub fn app() -> Self {
    //     RunCtx {
    //         state: State::App,
    //         block_protocol_handler: BlockProtocolHandler::default(),
    //         comm: ledger_device_sdk::io::Comm::new(),
    //         ui: UserInterface {
    //             main_menu: NbglHomeAndSettings::new(),
    //             do_refresh: true,
    //         },
    //         //tx_params: TxParams::default(),
    //         token_coin_id: [0; SUI_ADDRESS_LENGTH],
    //         token_coin_module: ArrayString::zero_filled(),
    //         token_coin_function: ArrayString::zero_filled(),
    //         token_divisor: SUI_COIN_DECIMALS,
    //         token_ticker: ArrayString::zero_filled(),
    //     }
    // }

    // pub fn lib_swap(tx_params: TxParams) -> Self {
    //     RunCtx {
    //         state: State::LibSwapIdle,
    //         block_protocol_handler: BlockProtocolHandler::default(),
    //         comm: ledger_device_sdk::io::Comm::new(),
    //         tx_params,
    //         token_coin_id: [0; SUI_ADDRESS_LENGTH],
    //         token_coin_module: ArrayString::zero_filled(),
    //         token_coin_function: ArrayString::zero_filled(),
    //         token_divisor: SUI_COIN_DECIMALS,
    //         token_ticker: ArrayString::zero_filled(),
    //     }
    // }

    // pub fn is_swap(&self) -> bool {
    //     !matches!(self.state, State::App)
    // }

    // pub fn is_swap_finished(&self) -> bool {
    //     matches!(
    //         self.state,
    //         State::LibSwapSignSuccess | State::LibSwapSignFailure,
    //     )
    // }

    // pub fn is_swap_sign_succeeded(&self) -> bool {
    //     matches!(self.state, State::LibSwapSignSuccess)
    // }

    // pub fn set_swap_sign_success(&mut self) {
    //     if self.is_swap() {
    //         self.state = State::LibSwapSignSuccess;
    //     }
    // }

    // pub fn set_swap_sign_failure(&mut self) {
    //     if self.is_swap() {
    //         self.state = State::LibSwapSignFailure;
    //     }
    // }

    // Panics if not in swap mode
    // pub fn get_swap_tx_params(&self) -> &TxParams {
    //     assert!(self.is_swap(), "attempt to get swap tx params in app mode");
    //     &self.tx_params
    // }

    pub fn set_token(
        &mut self,
        coin_id: [u8; SUI_ADDRESS_LENGTH],
        coin_module: ArrayString<COIN_STRING_LENGTH>,
        coin_function: ArrayString<COIN_STRING_LENGTH>,
        divisor: u8,
        ticker: ArrayString<TICKER_LENGTH>,
    ) {
        self.token_coin_id = coin_id;
        self.token_coin_module = coin_module;
        self.token_coin_function = coin_function;
        self.token_divisor = divisor;
        self.token_ticker = ticker;
    }

    pub fn get_token_coin_id(&mut self) -> [u8; SUI_ADDRESS_LENGTH] {
        self.token_coin_id
    }

    pub fn get_token_coin_module(&mut self) -> ArrayString<COIN_STRING_LENGTH> {
        self.token_coin_module
    }

    pub fn get_token_coin_function(&mut self) -> ArrayString<COIN_STRING_LENGTH> {
        self.token_coin_function
    }

    pub fn get_token_divisor(&mut self) -> u8 {
        self.token_divisor
    }

    pub fn get_token_ticker(&mut self) -> ArrayString<TICKER_LENGTH> {
        self.token_ticker
    }
}

// fn handle_state(&mut self, comm: &mut Comm) -> Result<(), Reply> {
//     match &self.state {
//         State::Idle => {
//             // Waiting for START
//             Ok(())
//         }

//         State::AppInfo => {
//             let mut rv = ArrayVec::<u8, 220>::new();
//             let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
//             let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
//             let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
//             const APP_NAME: &str = "sui";
//             let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
//             self.protocol.result_final(comm, &rv)?;
//             Ok(())
//         }

//         State::NeedInputParam { param_index } => {
//             let hash = self.protocol.get_input_hashes()[*param_index];
//             self.protocol.get_chunk(comm, hash)?;
//             // Return OK, wait for GET_CHUNK_RESPONSE
//             Ok(())
//         }

//         State::ComputingPublicKey { buffer } => {
//             crate::implementation::get_address_apdu_sync(
//                 &mut self.protocol,
//                 comm,
//                 buffer,
//                 false,
//             );
//             Ok(())
//         }

//         State::ParsingTransaction {
//             buffer,
//             bytes_needed,
//         } => {
//             if buffer.len() >= *bytes_needed {
//                 // Have enough data, parse it
//                 let _tx = KnownTx::TransferTx {
//                     recipient: [0u8; 32],
//                     coin_type: (
//                         crate::parser::common::SUI_COIN_ID,
//                         ArrayVec::new(),
//                         ArrayVec::new(),
//                     ),
//                     total_amount: 0,
//                     gas_budget: 0,
//                 }; //parse_transaction(&buffer)?;
//                 self.state = State::ReadyForUI {
//                     tx: KnownTx::TransferTx {
//                         recipient: [0u8; 32],
//                         coin_type: (
//                             crate::parser::common::SUI_COIN_ID,
//                             ArrayVec::new(),
//                             ArrayVec::new(),
//                         ),
//                         total_amount: 0,
//                         gas_budget: 0,
//                     },
//                     path: ArrayVec::new(),
//                 };
//                 self.handle_state(comm)
//             } else {
//                 // Need more data, request next chunk
//                 // ... (similar to above)
//                 Ok(())
//             }
//         }

//         State::ReadyToSign {
//             tx_hash: _,
//             path: _,
//         } => {
//             // Sign and return
//             let signature = [0u8; 32]; //sign_ed25519(path, tx_hash)?;
//             self.protocol.result_final(comm, &signature)?;
//             Ok(())
//         }

//         _ => Ok(()),
//     }
// }

// fn start(&mut self, ins: Ins) -> Result<(), Reply> {
//     // Initialize state based on instruction
//     match ins {
//         Ins::GetVersion => {
//             self.state = State::AppInfo;
//         }
//         Ins::GetPubkey => {
//             // Need to fetch BIP32 path (parameter 0)
//             self.state = State::NeedInputParam { param_index: 0 };
//         }
//         Ins::Sign => {
//             // Need to fetch transaction data (parameter 0)
//             self.state = State::NeedInputParam { param_index: 0 };
//         }
//         _ => return Err(Reply(0x6D00)), // Instruction not supported
//     }
//     Ok(())
// }

// fn process_chunk(&mut self, ins: Ins, data: &[u8]) -> Result<(), Reply> {
//     match (ins, &self.state) {
//         (Ins::GetPubkey, State::NeedInputParam { .. }) => {
//             // Received BIP32 path
//             ledger_device_sdk::log::info!("Received BIP32 path chunk");

//             self.state = State::ComputingPublicKey {
//                 buffer: data[32..].to_vec(),
//             };
//         }
//         // (Ins::Sign, CommandState::NeedInputParam { param_index }) if *param_index == 0 => {
//         //     // Received transaction data
//         //     let bytes_needed = 256; // Example fixed size, should be parsed from header
//         //     self.state = CommandState::ParsingTransaction { buffer: data.to_vec(), bytes_needed };
//         // }
//         _ => return Err(Reply(0x6A80)), // Unexpected state
//     }

//     // Process received chunk based on current state
//     // match (ins, &mut self.state) {
//     //     (Ins::Sign, CommandState::ParsingTransaction { buffer, .. }) => {
//     //         buffer.extend_from_slice(data);
//     //     }
//     //     (Ins::GetPubkey, CommandState::ParsingPath { buffer }) => {
//     //         buffer.extend_from_slice(data);
//     //     }
//     //     _ => {}
//     // }
//     Ok(())
// }
