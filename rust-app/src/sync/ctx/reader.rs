use super::{BlockProtocolHandler, Hash, HASH_LEN};
use ledger_device_sdk::io::{Comm, Reply};

extern crate alloc;
use alloc::vec::Vec;

/// Reads chunked data via block protocol
pub struct ChunkedReader<'a> {
    protocol: &'a mut BlockProtocolHandler,
    comm: &'a mut Comm,
    current_block: Vec<u8>,
    current_offset: usize,
    next_hash: Hash,
    finished: bool,
}

impl<'a> ChunkedReader<'a> {
    /// Create reader for a parameter hash
    pub fn new(
        protocol: &'a mut BlockProtocolHandler,
        comm: &'a mut Comm,
        initial_hash: Hash,
    ) -> Self {
        ChunkedReader {
            protocol,
            comm,
            current_block: Vec::new(),
            current_offset: 0,
            next_hash: initial_hash,
            finished: false,
        }
    }
    
    /// Read exactly N bytes
    pub fn read<const N: usize>(&mut self) -> Result<[u8; N], Reply> {
        let mut result = [0u8; N];
        
        for i in 0..N {
            result[i] = self.read_byte()?;
        }
        
        Ok(result)
    }
    
    /// Read a single byte
    fn read_byte(&mut self) -> Result<u8, Reply> {
        if self.finished {
            return Err(Reply(0x6A80)); // No more data
        }
        
        // Need to fetch next block?
        if self.current_offset >= self.current_block.len() {
            self.fetch_next_block()?;
        }
        
        let byte = self.current_block[self.current_offset];
        self.current_offset += 1;
        
        Ok(byte)
    }
    
    fn fetch_next_block(&mut self) -> Result<(), Reply> {
        // Check if this is the end marker
        if self.next_hash == [0u8; HASH_LEN] {
            self.finished = true;
            return Err(Reply(0x6A80)); // End of data
        }
        
        // Request chunk from host
        self.protocol.get_chunk(self.comm, self.next_hash)?;
        
        // Wait for response (next command will be GET_CHUNK_RESPONSE)
        // This returns control to main loop
        // Main loop will call process_command() again with the response
        
        // This is where the synchronous approach gets tricky:
        // We need to return from this function and wait for next APDU
        Err(Reply(0x9000)) // OK, but need more data
    }
}