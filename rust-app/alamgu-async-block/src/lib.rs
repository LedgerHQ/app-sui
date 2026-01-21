//! Stock code for writing APDUs that use the block protocol using futures.
//!
//! To use, for each APDU write an async function taking HostIO, which calls HostIO::get_params
//! as it's first action (before any .await), and write an function from HostIO and some instruction
//! type, returning a future (impl trait in bindings is useful here). Then in main, call
//! poll_apdu_handlers for Command events.
//!
//! Individual APDU handlers can then call parsers with the ByteStreams from the get_params call,
//! and send responses back to the host with result_accumulating and result_final on HostIO.
//!
//! # Example
//!
//!
//! ```
//! use core::future::Future;
//! use core::task::*;
//! use ledger_async_block::*;
//! use ledger_parser_combinators::async_parser::*;
//! use core::mem::MaybeUninit;
//!
//! pub fn example_apdu(self, io: HostIO) -> Self::State<'a> {
//!     async move {
//!         let mut input = io.get_params::<1>();
//!         // Parse an array of four bytes from the input; ignore any remainder.
//!         let data = (DefaultInterp as AsyncParser<Array<u8, 4>>).parse(&mut input[0]).await;
//!         // Send the four bytes back to the host.
//!         io.result_final(&data).await;
//!     }
//! }
//!
//! static mut COMM_CELL : MaybeUninit<RefCell<io::Comm>> = MaybeUninit::uninit();
//! static mut HOST_IO_STATE : MaybeUninit<RefCell<HostIOState>> = MaybeUninit::uninit();
//! static mut STATES_BACKING : MaybeUninit<Option<APDUsFuture>> = MaybeUninit::uninit();
//!
//! unsafe fn initialize() {
//!    STATES_BACKING.write(None);
//!    COMM_CELL.write(RefCell::new(io::Comm::new()));
//!    let comm = COMM_CELL.assume_init_ref();
//!    HOST_IO_STATE.write(RefCell::new(HostIOState {
//!        comm: comm,
//!        requested_block: None,
//!        sent_command: None,
//!    }));
//! }
//!
//! extern "C" fn sample_main() {
//!   ...
//!   unsafe { initialize(); }
//!   let host_io = HostIO(unsafe { HOST_IO_STATE.assume_init_ref() });
//!   let mut states = unsafe { Pin::new_unchecked( STATES_BACKING.assume_init_mut() ) };
//!   ...
//!   match poll_apdu_handlers(&mut states, ins, host_io, (), handle_apdu) {
//!     Ok(()) => { comm.borrow_mut().reply_ok() }
//!     Err(sw) => { comm.borrow_mut().reply(sw) }
//!   }
//! }
//!
//! type APDUsFuture = impl Future<Output = ()>;
//! handle_apdu<'a: 'b, 'b>(io: HostIO, ins: Ins) -> APDUsFuture {
//!         match ins {
//!             Ins::ExampleAPDU => example_apdu(io).await,
//!         }
//!     }
//!
//!
//! ```
//!
#![no_std]
#![feature(type_alias_impl_trait)]
#![cfg_attr(not(version("1.63")), feature(cell_filter_map))]
#![feature(cfg_version)]
#![cfg_attr(
    all(target_family = "bolos", not(version("1.64"))),
    feature(future_poll_fn)
)]
#![cfg_attr(
    all(target_family = "bolos", not(version("1.65"))),
    feature(generic_associated_types)
)]
#![cfg_attr(version("1.71"), feature(impl_trait_in_assoc_type))]

use arrayvec::ArrayVec;
use core::future::Future;
use core::pin::Pin;
use ledger_device_sdk::io;
use ledger_device_sdk::io::SyscallError;
use ledger_device_sdk::log::*;
use ledger_device_sdk::sys::*;
use ledger_parser_combinators::async_parser::{
    reject, Readable, UnwrappableReadable, REJECTED_CODE,
};

use core::cell::{Ref, RefCell, RefMut};
use core::convert::TryFrom;
use core::convert::TryInto;
use core::task::*;
use ledger_device_sdk::io::Reply; //, BorrowMutError};

#[cfg(feature = "prompts")]
pub mod prompts;

#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum HostToLedgerCmd {
    START = 0,
    GetChunkResponseSuccess = 1,
    GetChunkResponseFailure = 2,
    PutChunkResponse = 3,
    ResultAccumulatingResponse = 4,
}

impl TryFrom<u8> for HostToLedgerCmd {
    type Error = Reply;
    fn try_from(a: u8) -> Result<HostToLedgerCmd, Reply> {
        match a {
            0 => Ok(HostToLedgerCmd::START),
            1 => Ok(HostToLedgerCmd::GetChunkResponseSuccess),
            2 => Ok(HostToLedgerCmd::GetChunkResponseFailure),
            3 => Ok(HostToLedgerCmd::PutChunkResponse),
            4 => Ok(HostToLedgerCmd::ResultAccumulatingResponse),
            _ => Err(SyscallError::InvalidParameter.into()),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum LedgerToHostCmd {
    ResultAccumulating = 0, // Not used yet in this app.
    ResultFinal = 1,
    GetChunk = 2,
    PutChunk = 3,
}

const HASH_LEN: usize = 32;
pub type SHA256Sum = [u8; HASH_LEN];

#[derive(Debug)]
pub struct ChunkNotFound;

pub struct HostIOState {
    pub comm: &'static RefCell<io::Comm>,
    pub requested_block: Option<SHA256Sum>,
    pub sent_command: Option<LedgerToHostCmd>,
}

impl HostIOState {
    pub const fn new(comm: &'static RefCell<io::Comm>) -> HostIOState {
        HostIOState {
            comm: comm,
            requested_block: None,
            sent_command: None,
        }
    }
}

impl core::fmt::Debug for HostIOState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "HostIOState {{ comm: {}, requested_block: {:?}, sent_command: {:?} }}",
            if self.comm.try_borrow().is_ok() {
                "not borrowed"
            } else {
                "borrowed"
            },
            self.requested_block,
            self.sent_command
        )
    }
}

#[derive(PartialEq, Debug)]
pub enum AsyncTrampolineResult {
    NothingPending,
    Pending,
    Resolved,
}

pub trait AsyncTrampoline {
    fn handle_command(&mut self) -> AsyncTrampolineResult;
}
impl AsyncTrampoline for () {
    fn handle_command(&mut self) -> AsyncTrampolineResult {
        AsyncTrampolineResult::NothingPending
    }
}

pub struct WriterFuture<'a> {
    io: HostIO,
    sent: bool,
    cmd: LedgerToHostCmd,
    data: &'a [u8],
    wait: bool,
}

impl<'a> Future for WriterFuture<'a> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let sel = Pin::into_inner(self); // We can do this because we know Self is Unpin, no
                                         // self-references in it.
        match sel.io.0.try_borrow_mut() {
            Ok(ref mut s) => {
                if sel.sent {
                    Poll::Ready(())
                } else if s.sent_command.is_some() {
                    Poll::Pending
                } else {
                    s.requested_block = None;
                    s.sent_command = Some(sel.cmd);
                    let mut io = s.comm.borrow_mut();
                    io.append(&[sel.cmd as u8]);
                    io.append(sel.data);
                    sel.sent = true;
                    if sel.wait {
                        Poll::Pending
                    } else {
                        Poll::Ready(())
                    }
                }
            }
            Err(_) => Poll::Pending,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HostIO(pub &'static RefCell<HostIOState>);

impl HostIO {
    /// Grab a mutable reference to the [io::Comm] object, to do some direct stuff with.
    pub fn get_comm<'a>(self) -> Result<RefMut<'a, io::Comm>, Reply> {
        self.0
            .try_borrow_mut()
            .or(Err(SyscallError::InvalidState))?
            .comm
            .try_borrow_mut()
            .or(Err(SyscallError::InvalidState.into()))
    }

    /// Get a chunk, identified by SHA256 hash of it's contents, from the host.
    pub fn get_chunk(
        self,
        sha: SHA256Sum,
    ) -> impl Future<Output = Result<Ref<'static, [u8]>, ChunkNotFound>> {
        core::future::poll_fn(move |_| match self.0.try_borrow_mut() {
            Ok(ref mut s) => {
                if s.sent_command.is_some() {
                    Poll::Pending
                } else if s.requested_block == Some(sha) {
                    let resp_code = s.comm.borrow().get_data().ok().unwrap()[0];
                    match resp_code.try_into() {
                        Ok(HostToLedgerCmd::GetChunkResponseSuccess) => {
                            Poll::Ready(Ok(Ref::map(s.comm.borrow(), |comm| {
                                &comm.get_data().ok().unwrap()[1..]
                            })))
                        }
                        Ok(HostToLedgerCmd::GetChunkResponseFailure) => {
                            Poll::Ready(Err(ChunkNotFound))
                        }
                        Ok(HostToLedgerCmd::START) => {
                            error!("Start response code received when expecting GetChunkResponse");
                            Poll::Ready(Err(ChunkNotFound))
                        }
                        _ => {
                            error!("Reached unreachable, resp code: {:#}", resp_code);
                            panic!("Unreachable: should be filtered out by protocol rules before this point.")
                        }
                    }
                } else {
                    s.requested_block = Some(sha);
                    s.sent_command = Some(LedgerToHostCmd::GetChunk);
                    let mut io = s.comm.borrow_mut();
                    io.append(&[LedgerToHostCmd::GetChunk as u8]);
                    io.append(&sha);
                    Poll::Pending
                }
            }
            Err(_) => Poll::Pending,
        })
    }

    fn send_write_command<'a: 'c, 'b: 'c, 'c>(
        self,
        cmd: LedgerToHostCmd,
        data: &'b [u8],
        wait: bool,
    ) -> impl 'c + Future<Output = ()> {
        WriterFuture {
            io: self,
            sent: false,
            cmd: cmd,
            data: data,
            wait: wait,
        }
    }

    /// Write a chunk to the host, returning the SHA256 of it's contents, used as an address for a
    /// future get_chunk.
    pub fn put_chunk<'a: 'c, 'b: 'c, 'c>(
        self,
        chunk: &'b [u8],
    ) -> impl 'c + Future<Output = SHA256Sum> {
        async move {
            self.send_write_command(LedgerToHostCmd::PutChunk, chunk, true)
                .await;
            sha256_hash(chunk)
        }
    }

    /// Write a piece of output to the host, but don't declare that we are done; the host will
    /// return to the ledger to get more.
    pub fn result_accumulating<'a: 'c, 'b: 'c, 'c>(
        self,
        chunk: &'b [u8],
    ) -> impl 'c + Future<Output = ()> {
        self.send_write_command(LedgerToHostCmd::ResultAccumulating, chunk, true)
    }
    /// Write the final piece of output to the host; after this, we're done and the host does not
    /// contact us again on this subject.
    pub fn result_final<'a: 'c, 'b: 'c, 'c>(
        self,
        chunk: &'b [u8],
    ) -> impl 'c + Future<Output = ()> {
        self.send_write_command(LedgerToHostCmd::ResultFinal, chunk, false)
    }

    /// Get the parameters for the current APDU. Must be called first, while the
    /// HostToLedgerCmd::START message is still in the buffer.
    pub fn get_params<const N: usize>(self) -> Option<ArrayVec<ByteStream, N>> {
        if (*self.get_comm().ok()?.get_data().ok()?.get(0)?)
            .try_into()
            .ok()
            == Some(HostToLedgerCmd::START)
        {
            let mut params = ArrayVec::<ByteStream, N>::new();
            for param in self.get_comm().ok()?.get_data().ok()?[1..].chunks_exact(HASH_LEN) {
                info!("Creating ByteStream for chunk: {:02x?}", param);
                params
                    .try_push(ByteStream {
                        host_io: self,
                        current_chunk: param
                            .try_into()
                            .or(Err(SyscallError::InvalidParameter))
                            .ok()?,
                        current_offset: 0,
                    })
                    .ok()?;
            }
            Some(params)
        } else {
            None
        }
    }
}

/// A block must be at least [HASH_LEN] bytes long.
#[allow(dead_code)]
pub struct Block([u8]);

impl Block {
    pub fn as_raw_slice(&self) -> &[u8] {
        let block = self as *const Block as *const [u8];
        unsafe { &*block }
    }

    pub fn from_raw_slice_opt(block: &[u8]) -> Option<&Self> {
        if block.len() >= HASH_LEN {
            let block2 = block as *const [u8] as *const Block;
            Some(unsafe { &*block2 })
        } else {
            None
        }
    }

    /// Panics if block is illegally short
    pub fn next_block(&self) -> SHA256Sum {
        self.as_raw_slice()[..HASH_LEN].try_into().unwrap()
    }

    /// Panics if block is illegally short
    pub fn data(&self) -> &[u8] {
        &self.as_raw_slice()[HASH_LEN..]
    }
}

/// Concrete implementation of the interface defined by
/// [ledger_parser_combinators::async_parser::Readable] for the block protocol.
#[derive(Clone)]
pub struct ByteStream {
    host_io: HostIO,
    current_chunk: SHA256Sum,
    current_offset: usize,
}

impl ByteStream {
    /// Get the current block.
    fn get_current_block<'a>(&'a mut self) -> impl 'a + Future<Output = Ref<'static, Block>> {
        async move {
            if self.current_chunk == [0; 32] {
                let _: () = reject(SyscallError::InvalidParameter as u16).await;
            }
            let chunk_res = self.host_io.get_chunk(self.current_chunk).await;
            match chunk_res {
                Ok(a) => match Ref::filter_map(a, Block::from_raw_slice_opt) {
                    Ok(r) => r,
                    Err(_) => reject(SyscallError::InvalidParameter as u16).await,
                },
                Err(_) => reject(SyscallError::InvalidParameter as u16).await,
            }
        }
    }

    /// Get the rest of the current block that we have not already processed.
    ///
    /// [block] must be the current block.
    fn slice_from_block<'a, 'b>(&'a mut self, block: &'b Block) -> &'b [u8] {
        &block.data()[self.current_offset..]
    }

    /// Consume [consume] bytes from the current block.
    ///
    /// [block] must be the current block. [consume] must be less than or equal
    /// to `self.slice_from_block(block).len()`.
    fn consume(&mut self, block: &Block, consume: usize) {
        self.current_offset += consume;
        debug_assert!(self.current_offset <= block.data().len());
        if self.current_offset == block.data().len() {
            self.current_chunk = block.next_block();
            self.current_offset = 0;
        }
    }
}

impl Readable for ByteStream {
    type OutFut<'a, const N: usize> = impl 'a + core::future::Future<Output = [u8; N]>;
    fn read<'a: 'b, 'b, const N: usize>(&'a mut self) -> Self::OutFut<'b, N> {
        async move {
            let mut buffer = ArrayVec::<u8, N>::new();
            while !buffer.is_full() {
                let block = self.get_current_block().await;
                let avail = self.slice_from_block(&block);
                let consuming = core::cmp::min(avail.len(), buffer.remaining_capacity());
                buffer.try_extend_from_slice(&avail[0..consuming]).ok();
                self.consume(&*block, consuming);
            }
            buffer.into_inner().unwrap()
        }
    }
}

impl UnwrappableReadable for ByteStream {
    type Wrapped = Self;
    fn unwrap_clone(&self) -> Self::Wrapped {
        self.clone()
    }
}

pub static RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |a| RawWaker::new(a, &RAW_WAKER_VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

pub fn poll_with_trivial_context<Fut: Future + ?Sized>(
    f: Pin<&mut Fut>,
) -> core::task::Poll<Fut::Output> {
    let waker = unsafe { Waker::from_raw(RawWaker::new(&(), pic_rs(&RAW_WAKER_VTABLE))) };
    let mut ctxd = Context::from_waker(&waker);
    let r = f.poll(&mut ctxd);
    core::mem::forget(ctxd);
    core::mem::forget(waker);
    r
}

// Hashing required for validating blocks from the host.

fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut rv = [0; 32];
    unsafe {
        let mut hasher = cx_sha256_s::default();
        cx_sha256_init_no_throw(&mut hasher);
        let hasher_ref = &mut hasher as *mut cx_sha256_s as *mut cx_hash_t;
        cx_hash_update(hasher_ref, data.as_ptr(), data.len());
        cx_hash_final(hasher_ref, rv.as_mut_ptr());
    }
    rv
}

// Stack control helper.
#[inline(never)]
pub fn call_me_maybe<F: FnOnce() -> Option<()>>(f: F) -> Option<()> {
    f()
}

/// Main entry point: run an AsyncAPDU given an input.
#[inline(never)]
pub fn poll_apdu_handlers<'a: 'b, 'b, F: Future<Output = ()>, Ins, A: Fn(HostIO, Ins) -> F>(
    mut s: core::pin::Pin<&'a mut Option<F>>,
    ins: Ins,
    io: HostIO,
    apdus: A,
) -> Result<(), Reply> {
    let command = if io.get_comm()?.get_data()?.len() > 0 {
        io.get_comm()?.get_data()?[0].try_into()
    } else {
        Ok(HostToLedgerCmd::START)
    }; // Map empty APDUs to STARTs, so we can handle those the same as ones with inputs.
    match command {
        Ok(HostToLedgerCmd::START) => {
            call_me_maybe(|| {
                s.set(Some(apdus(io, ins))); // Initialize the APDU represented.
                Some(())
            })
            .ok_or(SyscallError::InvalidState)?;
        }
        Ok(HostToLedgerCmd::GetChunkResponseSuccess)
            if io.0.borrow().sent_command == Some(LedgerToHostCmd::GetChunk) =>
        {
            if io.0.borrow().comm.borrow().get_data()?.len() < HASH_LEN + 1 {
                return Err(SyscallError::InvalidParameter.into());
            }

            // Check the hash, so the host can't lie.
            call_me_maybe(|| {
                let hashed = sha256_hash(&io.0.borrow().comm.borrow().get_data().ok()?[1..]);

                if Some(hashed) != io.0.borrow().requested_block {
                    None
                } else {
                    Some(())
                }
            })
            .ok_or(Reply::from(SyscallError::InvalidParameter))?;
        }
        // Only need to check that these are responses to things we did; there's no data to
        // validate.
        Ok(HostToLedgerCmd::GetChunkResponseFailure)
            if io.0.borrow().sent_command == Some(LedgerToHostCmd::GetChunk) => {}
        Ok(HostToLedgerCmd::PutChunkResponse)
            if io.0.borrow().sent_command == Some(LedgerToHostCmd::PutChunk) => {}
        Ok(HostToLedgerCmd::ResultAccumulatingResponse)
            if io.0.borrow().sent_command == Some(LedgerToHostCmd::ResultAccumulating) => {}
        // Reject otherwise.
        _ => Err(Reply::from(SyscallError::InvalidParameter))?,
    }

    // We use this to wait if we've already got a command to send, so clear it now that we're
    // in a validated state.
    io.0.borrow_mut().sent_command = None;

    loop {
        // And run the future for this APDU.
        match poll_with_trivial_context(
            s.as_mut()
                .as_pin_mut()
                .ok_or(Reply::from(SyscallError::InvalidState))?,
        ) {
            Poll::Pending => {
                // Then, check that if we're waiting that we've actually given the host something to do.
                // In case of ResultFinal allow the Future to run to completion, and reset the state.
                if io.0.borrow().sent_command.is_some()
                    && io.0.borrow().sent_command != Some(LedgerToHostCmd::ResultFinal)
                {
                    return Ok(());
                } else {
                    error!("APDU handler future neither completed nor sent a command; something is probably missing an .await");
                    unsafe {
                        if REJECTED_CODE == 0 {
                            Err(Reply::from(SyscallError::InvalidState))?
                        } else {
                            Err(Reply(REJECTED_CODE))?
                        }
                    }
                }
            }
            Poll::Ready(()) => {
                if io.0.borrow().sent_command.is_none() {
                    error!("APDU handler future completed but did not send a command; the last line is probably missing an .await");
                    Err(Reply::from(SyscallError::InvalidState))?
                }
                call_me_maybe(|| {
                    s.set(None);
                    Some(())
                });
                return Ok(());
            }
        }
    }
}
