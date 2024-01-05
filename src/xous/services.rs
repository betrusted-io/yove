use std::sync::mpsc::Receiver;
pub mod log;
pub mod name;
pub mod panic_to_screen;
pub mod ticktimer;
use super::Memory;

pub type ResponseData = ([i64; 8], Option<(Vec<u8>, u64)>);

#[allow(dead_code)]
pub enum ScalarResult {
    Scalar1(u32),
    Scalar2([u32; 2]),
    Scalar5([u32; 5]),
    WaitForResponse(Receiver<ResponseData>),
}

#[allow(dead_code)]
pub enum LendResult {
    MemoryReturned([u32; 2]),
    WaitForResponse(Receiver<ResponseData>),
}

pub trait Service {
    fn scalar(&mut self, _memory: &mut Memory, sender: u32, opcode: u32, args: [u32; 4]) {
        panic!(
            "Unknown scalar to service {}: {} ({:?})",
            sender, opcode, args
        );
    }

    fn blocking_scalar(
        &mut self,
        _memory: &mut Memory,
        sender: u32,
        opcode: u32,
        args: [u32; 4],
    ) -> ScalarResult {
        panic!(
            "Unknown scalar to service {}: {} ({:?})",
            sender, opcode, args
        );
    }

    fn lend(
        &mut self,
        _memory: &mut Memory,
        sender: u32,
        opcode: u32,
        buf: &[u8],
        extra: [u32; 2],
    ) -> LendResult {
        panic!(
            "Unknown lend {} bytes to service {}: {} ({:?})",
            buf.len(),
            sender,
            opcode,
            extra
        );
    }

    fn lend_mut(
        &mut self,
        _memory: &mut Memory,
        sender: u32,
        opcode: u32,
        buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        panic!(
            "Unknown lend_mut {} bytes to service {}: {} ({:?})",
            buf.len(),
            sender,
            opcode,
            extra
        );
    }

    fn send(
        &mut self,
        _memory: &mut Memory,
        sender: u32,
        opcode: u32,
        buf: &[u8],
        extra: [u32; 2],
    ) {
        panic!(
            "Unknown send {} bytes to service {}: {} ({:?})",
            buf.len(),
            sender,
            opcode,
            extra
        );
    }
}

pub fn get_service(name: &[u32; 4]) -> Option<Box<dyn Service + Sync + Send>> {
    let mut output_bfr = [0u8; core::mem::size_of::<u32>() * 4 /*args.len()*/];
    // Combine the four arguments to form a single
    // contiguous buffer. Note: The buffer size will change
    // depending on the platfor's `usize` length.
    for (src, dest) in name.iter().zip(output_bfr.chunks_mut(4)) {
        dest.copy_from_slice(src.to_le_bytes().as_ref());
    }
    println!(
        "Connecting to service: {}",
        std::str::from_utf8(&output_bfr).unwrap_or("<invalid name>")
    );

    match name {
        [0x6b636974, 0x656d6974, 0x65732d72, 0x72657672] => {
            Some(Box::new(ticktimer::Ticktimer::new()))
        }
        [0x73756f78, 0x676f6c2d, 0x7265732d, 0x20726576] => Some(Box::new(log::Log::new())),
        [0x73756f78, 0x6d616e2d, 0x65732d65, 0x72657672] => Some(Box::new(name::Name::new())),
        _ => panic!("Unhandled service request: {:x?}", name),
    }
}
