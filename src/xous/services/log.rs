use super::{LendResult, ScalarResult, Service};
use crate::xous::Memory;
use std::io::Write;

enum LogLendOpcode {
    /// A `&[u8]` destined for stdout
    StandardOutput = 1,

    /// A `&[u8]` destined for stderr
    StandardError = 2,
}

#[allow(dead_code)]
enum LogSendOpcode {
    /// A panic occurred, and a panic log is forthcoming
    PanicStarted = 1000,

    /// Log messages of varying size
    PanicMessage0 = 1100,
    PanicMessage1 = 1101,
    PanicMessage2 = 1102,
    PanicMessage3 = 1103,
    PanicMessage4 = 1104,
    PanicMessage5 = 1105,
    PanicMessage6 = 1106,
    PanicMessage7 = 1107,
    PanicMessage8 = 1108,
    PanicMessage9 = 1109,
    PanicMessage10 = 1110,
    PanicMessage11 = 1111,
    PanicMessage12 = 1112,
    PanicMessage13 = 1113,
    PanicMessage14 = 1114,
    PanicMessage15 = 1115,
    PanicMessage16 = 1116,
    PanicMessage17 = 1117,
    PanicMessage18 = 1118,
    PanicMessage19 = 1119,
    PanicMessage20 = 1120,
    PanicMessage21 = 1121,
    PanicMessage22 = 1122,
    PanicMessage23 = 1123,
    PanicMessage24 = 1124,
    PanicMessage25 = 1125,
    PanicMessage26 = 1126,
    PanicMessage27 = 1127,
    PanicMessage28 = 1128,
    PanicMessage29 = 1129,
    PanicMessage30 = 1130,
    PanicMessage31 = 1131,
    PanicMessage32 = 1132,

    /// End of a panic
    PanicFinished = 1200,
}

pub struct Log {}

impl Log {
    pub fn new() -> Self {
        Log {}
    }
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for Log {
    fn scalar(&mut self, _memory: &Memory, sender: u32, opcode: u32, args: [u32; 4]) {
        let message_bytes = if opcode >= LogSendOpcode::PanicMessage0 as u32
            && opcode <= LogSendOpcode::PanicMessage32 as u32
        {
            Some(opcode - LogSendOpcode::PanicMessage0 as u32)
        } else {
            None
        };

        if LogSendOpcode::PanicStarted as u32 == opcode {
            println!("Panic started");
        } else if LogSendOpcode::PanicFinished as u32 == opcode {
            println!();
            println!("Panic finished");
        } else if let Some(message_bytes) = message_bytes {
            let mut output_bfr = [0u8; core::mem::size_of::<u32>() * 4 /*args.len()*/];
            // let mut output_iter = output_bfr.iter_mut();

            // Combine the four arguments to form a single
            // contiguous buffer. Note: The buffer size will change
            // depending on the platfor's `usize` length.
            for (src, dest) in args.iter().zip(output_bfr.chunks_mut(4)) {
                dest.copy_from_slice(src.to_le_bytes().as_ref());
                // for src in word.to_le_bytes().iter() {
                //     *(output_iter.next().unwrap()) = *src;
                // }
            }
            print!(
                "{}",
                std::str::from_utf8(&output_bfr[0..message_bytes as usize]).unwrap_or("<invalid>")
            );
        } else {
            println!("Log scalar {}: {} {:x?}", sender, opcode, args);
        }
    }

    fn blocking_scalar(
        &mut self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        args: [u32; 4],
    ) -> ScalarResult {
        println!(
            "Unhandled log blocking_scalar {}: {} {:x?}",
            sender, opcode, args
        );
        ScalarResult::Scalar1(0)
    }

    fn lend(
        &mut self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        buf: &[u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode == LogLendOpcode::StandardOutput as u32 {
            let print_buffer = &buf[0..extra[1] as usize];
            // println!("Log stdout:");
            std::io::stdout().write_all(print_buffer).unwrap();
            std::io::stdout().flush().unwrap();
        } else if opcode == LogLendOpcode::StandardError as u32 {
            let print_buffer = &buf[0..extra[1] as usize];
            // println!("Log stderr:");
            std::io::stderr().write_all(print_buffer).unwrap();
            std::io::stderr().flush().unwrap();
        } else {
            panic!("Unhandled log lend {}: {} {:x?}", sender, opcode, buf);
        }
        LendResult::MemoryReturned([0, 0])
    }

    fn lend_mut(
        &mut self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        _buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        println!("Unhandled log lend_mut {}: {} {:x?}", sender, opcode, extra);
        LendResult::MemoryReturned([0, 0])
    }

    fn send(
        &mut self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        _buf: &[u8],
        extra: [u32; 2],
    ) {
        println!("Unhandled log send {}: {} {:x?}", sender, opcode, extra);
    }
}
