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

    fn append_panic_text(&self, buf: &[u8], valid: u32) -> LendResult {
        let _panic_str: &str = std::str::from_utf8(&buf[0..valid as usize]).unwrap_or("<invalid>");
        // println!("Panic to screen: {}", panic_str);
        LendResult::MemoryReturned([0, 0])
    }
}

impl Default for PanicToScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for PanicToScreen {
    fn lend(
        &self,
        _memory: &Memory,
        _sender: u32,
        opcode: u32,
        buf: &[u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode == PanicToScreenLendMutOpcode::AppendPanicText as _ {
            return self.append_panic_text(buf, extra[1]);
        }
        panic!(
            "panic-to-screen lent {} bytes to service for opcode {} ({:?})",
            buf.len(),
            opcode,
            extra
        );
    }

    fn lend_mut(
        &self,
        _memory: &Memory,
        _sender: u32,
        opcode: u32,
        buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode == PanicToScreenLendMutOpcode::AppendPanicText as _ {
            return self.append_panic_text(buf, extra[1]);
        }
        panic!(
            "panic-to-screen mutably lent {} bytes to service for opcode {} ({:?})",
            buf.len(),
            opcode,
            extra
        );
    }
}
