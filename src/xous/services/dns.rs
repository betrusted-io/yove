use std::net::{SocketAddr, ToSocketAddrs};

use super::{LendResult, Service};
use crate::xous::Memory;
const DNS_NAME_LENGTH_LIMIT: usize = 256;

enum DnsLendMutOpcode {
    RawLookup = 6,
}

pub struct DnsResolver {}

fn name_from_msg(msg: &[u8], valid: u32) -> Result<String, ()> {
    let valid_bytes = usize::min(msg.len(), valid as usize);
    if valid_bytes == 0 || valid_bytes >= DNS_NAME_LENGTH_LIMIT {
        return Err(());
    }

    // Safe because we've already validated that it's a valid range
    let str_slice = &msg[0..valid_bytes];
    let name_string = core::str::from_utf8(str_slice).or(Err(()))?;
    Ok(name_string.to_owned())
}

impl DnsResolver {
    pub fn new() -> Self {
        DnsResolver {}
    }

    fn lookup(&self, buf: &mut [u8], valid: u32) -> LendResult {
        let Ok(query_string) = name_from_msg(buf, valid) else {
            buf[0..4].copy_from_slice(&1u32.to_le_bytes());
            buf[4..8].copy_from_slice(&1u32.to_le_bytes());
            return LendResult::MemoryReturned([0, 0]);
        };
        let Ok(addrs) = (query_string.as_str(), 0u16)
            .to_socket_addrs()
            .map(|iter| iter.collect::<Vec<_>>())
            .map_err(|_| {
                buf[0..4].copy_from_slice(&1u32.to_le_bytes());
                buf[4..8].copy_from_slice(&1u32.to_le_bytes());
            })
        else {
            return LendResult::MemoryReturned([0, 0]);
        };

        let mut cursor = buf.iter_mut();

        // No error
        *cursor.next().unwrap() = 0;

        // Number of entries
        *cursor.next().unwrap() = addrs.len() as u8;

        for entry in addrs {
            match entry {
                SocketAddr::V4(a) => {
                    *cursor.next().unwrap() = 4;
                    for byte in a.ip().octets().iter() {
                        *cursor.next().unwrap() = *byte;
                    }
                }
                SocketAddr::V6(a) => {
                    *cursor.next().unwrap() = 6;
                    for byte in a.ip().octets().iter() {
                        *cursor.next().unwrap() = *byte;
                    }
                }
            }
        }

        LendResult::MemoryReturned([0, 0])
    }
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for DnsResolver {
    fn lend_mut(
        &self,
        _memory: &Memory,
        _sender: u32,
        opcode: u32,
        buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode == DnsLendMutOpcode::RawLookup as u32 {
            return self.lookup(buf, extra[1]);
        }
        panic!("Unhandled dns opcode {}", opcode);
    }
}
