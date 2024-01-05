use super::{LendResult, Service};
use crate::xous::Memory;

enum PanicToScreenLendMutOpcode {
    AppendPanicText = 0,
}

pub struct PanicToScreen {}

impl PanicToScreen {
    pub fn new() -> Self {
        PanicToScreen {}
    }
}

impl Default for PanicToScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for PanicToScreen {
    fn lend_mut(
        &mut self,
        _memory: &mut Memory,
        _sender: u32,
        opcode: u32,
        buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode != PanicToScreenLendMutOpcode::AppendPanicText as _ {
            panic!("Unhandled panic-to-screen opcode {}", opcode);
        }

        let panic_str = std::str::from_utf8(&buf[0..extra[1] as usize]).unwrap_or("<invalid>");
        println!("Panic to screen: {}", panic_str);
        LendResult::MemoryReturned([0, 0])
    }
}
