use std::{collections::HashMap, sync::atomic::Ordering};

use crate::xous::Memory;

use super::{LendResult, ScalarResult, Service};

#[allow(dead_code)]
enum NameLendOpcode {
    /// Create a new server with the given name and return its SID.
    Register = 0,

    /// Create a connection to the target server.
    Lookup = 1,

    /// Create an authenticated connection to the target server.
    AuthenticatedLookup = 2,

    /// unregister a server, given its cryptographically unique SID.
    Unregister = 3,

    /// disconnect, given a server name and a cryptographically unique, one-time use token
    Disconnect = 4,

    /// indicates if all inherently trusted slots have been occupied. Should not run untrusted code until this is the case.
    TrustedInitDone = 5,

    /// Connect to a Server, blocking if the Server does not exist. When the Server is started,
    /// return with either the CID or an AuthenticationRequest
    ///
    /// # Message Types
    ///
    ///     * MutableLend
    ///
    /// # Arguments
    ///
    /// The memory being pointed to should be a &str, and the length of the string should
    /// be specified in the `valid` field.
    ///
    /// # Return Values
    ///
    /// Memory is overwritten to contain a return value.  This return value can be defined
    /// as the following enum:
    ///
    /// ```rust
    /// #[repr(C)]
    /// #[non_exhaustive]
    /// enum ConnectResult {
    ///     Success(xous::CID /* connection ID */, [u32; 4] /* Disconnection token */),
    ///     Error(u32 /* error code */),
    ///     Unhandled, /* Catchall for future Results */
    /// }
    /// ```
    BlockingConnect = 6,

    /// Connect to a Server, returning the connection ID or an authentication request if
    /// it exists, and returning ServerNotFound if it does not exist.
    ///
    /// # Message Types
    ///
    ///     * MutableLend
    ///
    /// # Arguments
    ///
    /// The memory being pointed to should be a &str, and the length of the string should
    /// be specified in the `valid` field.
    ///
    /// # Return Values
    ///
    /// Memory is overwritten to contain a return value.  This return value can be defined
    /// as the following enum:
    ///
    /// ```rust
    /// #[repr(C)]
    /// #[non_exhaustive]
    /// enum ConnectResult {
    ///     Success(xous::CID /* connection ID */, [u32; 4] /* Disconnection token */),
    ///     Error(u32 /* error code */),
    ///     Unhandled, /* Catchall for future Results */
    /// }
    /// ```
    TryConnect = 7,
}

pub struct Name {
    connection_index: HashMap<String, u32>,
}

impl Name {
    pub fn new() -> Self {
        Name {
            connection_index: HashMap::default(),
        }
    }

    fn return_connection(&self, buf: &mut [u8], connection_id: u32) -> LendResult {
        buf[0..4].copy_from_slice(&0u32.to_le_bytes());
        buf[4..8].copy_from_slice(&connection_id.to_le_bytes());
        LendResult::MemoryReturned([0, 0])
    }
}

impl Default for Name {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for Name {
    fn scalar(&mut self, _memory: &Memory, sender: u32, opcode: u32, args: [u32; 4]) {
        panic!("Unhandled name scalar {}: {} {:x?}", sender, opcode, args);
    }

    fn blocking_scalar(
        &mut self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        args: [u32; 4],
    ) -> ScalarResult {
        panic!(
            "Unhandled name blocking_scalar {}: {} {:x?}",
            sender, opcode, args
        );
    }

    fn lend(
        &mut self,
        _memory: &Memory,
        sender: u32,
        opcode: u32,
        buf: &[u8],
        extra: [u32; 2],
    ) -> LendResult {
        panic!(
            "Unhandled name lend {}: {} {:x?} {:x?}",
            sender, opcode, buf, extra
        );
    }

    fn lend_mut(
        &mut self,
        memory: &Memory,
        sender: u32,
        opcode: u32,
        buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode == NameLendOpcode::Register as u32 {
            panic!("Register opcode unimplemented");
        } else if opcode == NameLendOpcode::TryConnect as u32
            || opcode == NameLendOpcode::BlockingConnect as u32
        {
            let buf_len = buf.len().min(extra[1] as usize);
            let name = std::str::from_utf8(&buf[0..buf_len]).unwrap_or("<invalid>");

            if let Some(connection_id) = self.connection_index.get(name) {
                println!(
                    "Existing server found at connection index {}",
                    connection_id
                );
                return self.return_connection(buf, *connection_id);
            }

            let service: Box<dyn Service + Send + Sync> = if name == "panic-to-screen!" {
                Box::new(super::panic_to_screen::PanicToScreen::new())
            } else if name == "_DNS Resolver Middleware_" {
                Box::new(super::dns::DnsResolver::new())
            } else {
                eprintln!("Unrecognized service name {}", name);
                std::process::exit(1);
            };

            // Insert the connection into the system bus' connection table
            let connection_id = memory.connection_index.fetch_add(1, Ordering::Relaxed);
            let mut connections = memory.connections.lock().unwrap();
            connections.insert(connection_id, service);

            // Insert it into the connection map so subsequent lookups get the same service
            self.connection_index.insert(name.to_owned(), connection_id);

            self.return_connection(buf, connection_id)
        } else {
            panic!(
                "Unhandled name lend_mut {}: {} {:x?}",
                sender, opcode, extra
            );
        }
        //
    }

    fn send(&mut self, _memory: &Memory, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) {
        panic!("Unhandled name send {}: {} {:x?}", sender, opcode, extra);
    }
}
