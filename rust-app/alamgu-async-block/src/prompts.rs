use crate::*;
use arrayvec::ArrayString;
use core::fmt::{Arguments, Error, Write};
use core::str::from_utf8;

use ledger_device_sdk::buttons::*;
use ledger_device_sdk::ui::bagls::*;
use ledger_device_sdk::ui::layout::*;
use ledger_device_sdk::ui::gadgets::{clear_screen, get_event};

#[derive(Clone, Debug)]
pub struct PromptQueue {
    io: HostIO,
    prev: SHA256Sum,
}

const HASH_LENGTH: usize = 32;

const PROMPT_TITLE_LENGTH: usize = 17;

#[cfg(target_os = "nanos")]
const PROMPT_CHUNK_LENGTH: usize = 16;

#[cfg(not(target_os = "nanos"))]
const PROMPT_CHUNK_LENGTH: usize = 48;

// The two bytes are to store the actual "length" of title and chunk, respectively
type PromptBuffer = ArrayVec<u8, { HASH_LENGTH + 2 + PROMPT_TITLE_LENGTH + PROMPT_CHUNK_LENGTH }>;

#[derive(Debug, PartialEq)]
pub struct PromptingError;

impl From<core::str::Utf8Error> for PromptingError {
    fn from(_: core::str::Utf8Error) -> PromptingError {
        PromptingError
    }
}
impl From<arrayvec::CapacityError> for PromptingError {
    fn from(_: arrayvec::CapacityError) -> PromptingError {
        PromptingError
    }
}
impl From<arrayvec::CapacityError<&str>> for PromptingError {
    fn from(_: arrayvec::CapacityError<&str>) -> PromptingError {
        PromptingError
    }
}
impl From<ChunkNotFound> for PromptingError {
    fn from(_: ChunkNotFound) -> PromptingError {
        PromptingError
    }
}

impl PromptQueue {
    pub fn new(io: HostIO) -> PromptQueue {
        PromptQueue {
            io,
            prev: [0; HASH_LENGTH],
        }
    }

    async fn pop(
        &mut self,
    ) -> Result<
        Option<(
            ArrayString<PROMPT_TITLE_LENGTH>,
            ArrayString<PROMPT_CHUNK_LENGTH>,
        )>,
        PromptingError,
    > {
        if self.prev == [0; HASH_LENGTH] {
            return Ok(None);
        }
        let chunk = PromptBuffer::try_from(self.io.get_chunk(self.prev).await?.as_ref())?;
        self.prev[0..HASH_LENGTH].copy_from_slice(&chunk[0..HASH_LENGTH]);
        let title_len = chunk[32] as usize;
        let title_end = 34 + title_len;
        let body_len = chunk[33] as usize;
        let body_end = title_end + body_len;
        let title = ArrayString::try_from(from_utf8(&chunk[34..title_end])?)?;
        let body = ArrayString::try_from(from_utf8(&chunk[title_end..body_end])?)?;
        Ok(Some((title, body)))
    }

    async fn reverse(&self) -> Result<Self, PromptingError> {
        let mut cursor = self.clone();
        let mut accum = Self::new(self.io);
        while let Some((title, body)) = cursor.pop().await? {
            accum.add_prompt_chunk(&title, &body).await?;
        }
        Ok(accum)
    }

    pub async fn append(&mut self, other: &PromptQueue) -> Result<(), PromptingError> {
        let mut from = other.reverse().await?;
        while let Some((title, body)) = from.pop().await? {
            self.add_prompt_chunk(&title, &body).await?;
        }
        Ok(())
    }

    pub async fn show(&mut self) -> Result<bool, PromptingError> {
        if self.prev == [0; HASH_LENGTH] {
            return Err(PromptingError);
        } // No showing empty PromptQueues.

        let mut forward = self.reverse().await?;
        let mut backward = Self::new(self.io);
        let mut title_and_body = forward.pop().await?.expect("already checked for this?");

        #[derive(Clone, Copy, Debug)]
        enum PromptingState {
            Prompts,
            Confirm,
            Cancel,
        }
        let mut state = PromptingState::Prompts;
        let mut buttons = Default::default();

        loop {
            clear_screen();
            // Display
            let (current_title, current_body) = title_and_body;
            match state {
                PromptingState::Prompts => {
                    current_title
                        .as_str()
                        .place(Location::Top, Layout::Centered, false);
                    #[cfg(target_os = "nanos")]
                    {
                        current_body
                            .as_str()
                            .place(Location::Custom(15), Layout::Centered, false);
                    }
                    #[cfg(not(target_os = "nanos"))]
                    {
                        let mut iter = current_body.as_bytes().chunks(16);
                        if let Some(body) = iter.next().map(|s| from_utf8(s).ok()).flatten() {
                            body.place(Location::Custom(16), Layout::Centered, false);
                        };
                        if let Some(body) = iter.next().map(|s| from_utf8(s).ok()).flatten() {
                            body.place(Location::Custom(31), Layout::Centered, false);
                        };
                        if let Some(body) = iter.next().map(|s| from_utf8(s).ok()).flatten() {
                            body.place(Location::Custom(46), Layout::Centered, false);
                        };
                    }
                    if backward.prev != [0; HASH_LENGTH] {
                        LEFT_ARROW.instant_display();
                    }
                    RIGHT_ARROW.instant_display();
                }
                PromptingState::Confirm => {
                    CHECKMARK_ICON.set_x(18).display();
                    "Confirm".place(Location::Middle, Layout::Centered, false);
                    LEFT_ARROW.instant_display();
                    RIGHT_ARROW.instant_display();
                }
                PromptingState::Cancel => {
                    CROSS_ICON.set_x(18).display();
                    "Reject".place(Location::Middle, Layout::Centered, false);
                    LEFT_ARROW.instant_display();
                }
            }
            // Handle buttons
            /* let buttons_evt = loop { match self.io.get_comm().unwrap().next_event::<u8>() {
                Event::Button(button) => { break button.clone(); }
                _ => { }
            } }; */

            // let buttons_evt = self.io.next_button().await;
            loop {
                if let Some(buttons_evt) = get_event(&mut buttons) {
                    // {
                    match (state.clone(), buttons_evt) {
                        (PromptingState::Prompts, ButtonEvent::LeftButtonRelease) => {
                            if backward.prev != [0; HASH_LENGTH] {
                                forward
                                    .add_prompt_chunk(&current_title, &current_body)
                                    .await?;
                                title_and_body = backward.pop().await?.unwrap();
                            }
                        }
                        (PromptingState::Prompts, ButtonEvent::RightButtonRelease) => {
                            if forward.prev != [0; HASH_LENGTH] {
                                backward
                                    .add_prompt_chunk(&current_title, &current_body)
                                    .await?;
                                title_and_body = forward.pop().await?.unwrap();
                            } else {
                                state = PromptingState::Confirm;
                            }
                        }
                        (PromptingState::Confirm, ButtonEvent::LeftButtonRelease) => {
                            state = PromptingState::Prompts;
                        }
                        (PromptingState::Confirm, ButtonEvent::RightButtonRelease) => {
                            state = PromptingState::Cancel;
                        }
                        (PromptingState::Confirm, ButtonEvent::BothButtonsRelease) => {
                            return Ok(true);
                        }
                        (PromptingState::Cancel, ButtonEvent::BothButtonsRelease) => {
                            return Ok(false);
                        }
                        (PromptingState::Cancel, ButtonEvent::LeftButtonRelease) => {
                            state = PromptingState::Confirm;
                        }
                        _ => {
                            continue;
                        }
                    }
                    break;
                } else {
                }
            }
        }
    }

    async fn add_prompt_chunk(&mut self, title: &str, segment: &str) -> Result<(), PromptingError> {
        if title.len() > PROMPT_TITLE_LENGTH {
            return Err(PromptingError);
        }
        if segment.len() > PROMPT_CHUNK_LENGTH {
            return Err(PromptingError);
        }
        let mut chunk = PromptBuffer::new();
        chunk.try_extend_from_slice(&self.prev)?;
        chunk.push(title.len() as u8);
        chunk.push(segment.len() as u8);
        chunk.try_extend_from_slice(title.as_bytes())?;
        chunk.try_extend_from_slice(segment.as_bytes())?;
        self.prev = self.io.put_chunk(&chunk).await;
        Ok(())
    }

    pub async fn add_prompt(
        &mut self,
        title: &str,
        args: Arguments<'_>,
    ) -> Result<(), PromptingError> {
        let mut writer = ChunkedWrite::new();
        while !writer.terminated() {
            core::fmt::write(&mut writer, args).map_err(|_| PromptingError)?;
            self.add_prompt_chunk(title, writer.get_buffer()).await?;
            writer.advance();
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ChunkedWrite {
    skip: usize,
    skip_this_pass: usize,
    len: usize,
    terminated: bool,
    buffer: ArrayString<PROMPT_CHUNK_LENGTH>,
}

impl ChunkedWrite {
    fn new() -> ChunkedWrite {
        ChunkedWrite {
            skip: 0,
            skip_this_pass: 0,
            len: 0,
            terminated: false,
            buffer: ArrayString::new(),
        }
    }
    fn advance(&mut self) {
        self.terminated = self.skip + self.buffer.len() >= self.len;
        self.buffer.clear();
        self.skip += PROMPT_CHUNK_LENGTH;
        self.skip_this_pass = self.skip;
        self.len = 0;
    }
    fn terminated(&self) -> bool {
        self.terminated
    }
    fn get_buffer(&self) -> &str {
        &self.buffer
    }
}

impl Write for ChunkedWrite {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        self.len += s.len();

        if self.skip_this_pass > s.len() {
            self.skip_this_pass -= s.len();
            return Ok(());
        }
        if self.buffer.is_full() {
            return Ok(());
        }
        let pushed_slice = &s[self.skip_this_pass
            ..core::cmp::min(
                s.len(),
                PROMPT_CHUNK_LENGTH + self.skip_this_pass - self.buffer.len(),
            )];
        self.skip_this_pass =
            core::cmp::max(0, self.skip_this_pass as isize - s.len() as isize) as usize;

        self.buffer.try_push_str(pushed_slice).map_err(|_| Error)
    }
}
