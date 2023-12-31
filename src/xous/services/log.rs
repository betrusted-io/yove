use super::{LendResult, ScalarResult, Service};
use std::io::Write;

enum LogLendOpcode {
    /// A `&[u8]` destined for stdout
    StandardOutput = 1,

    /// A `&[u8]` destined for stderr
    StandardError = 2,
}

pub struct Log {}

impl Log {
    pub fn new() -> Self {
        // println!("Constructing a log server");
        Log {}
    }
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for Log {
    fn scalar(&mut self, sender: u32, opcode: u32, args: [u32; 4]) {
        println!("Log scalar {}: {} {:x?}", sender, opcode, args);
    }

    fn blocking_scalar(&mut self, sender: u32, opcode: u32, args: [u32; 4]) -> ScalarResult {
        println!("Log blocking_scalar {}: {} {:x?}", sender, opcode, args);
        ScalarResult::Scalar1(0)
    }

    fn lend(&mut self, sender: u32, opcode: u32, buf: &[u8], extra: [u32; 2]) -> LendResult {
        if opcode == LogLendOpcode::StandardOutput as u32 {
            let print_buffer = &buf[0..extra[1] as usize];
            // println!("Log stdout:");
            std::io::stdout().write_all(print_buffer).unwrap();
        } else if opcode == LogLendOpcode::StandardError as u32 {
            let print_buffer = &buf[0..extra[1] as usize];
            // println!("Log stderr:");
            std::io::stderr().write_all(print_buffer).unwrap();
        } else {
            panic!("Log lend {}: {} {:x?}", sender, opcode, buf);
        }
        LendResult::MemoryReturned([0, 0])
    }

    fn lend_mut(
        &mut self,
        sender: u32,
        opcode: u32,
        _buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        println!("Log lend_mut {}: {} {:x?}", sender, opcode, extra);
        LendResult::MemoryReturned([0, 0])
    }

    fn send(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) {
        println!("Log send {}: {} {:x?}", sender, opcode, extra);
    }
}
