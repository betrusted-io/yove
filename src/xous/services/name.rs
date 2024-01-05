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

pub struct Name {}

impl Name {
    pub fn new() -> Self {
        Name {}
    }
}

impl Default for Name {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for Name {
    fn scalar(&mut self, _memory: &mut Memory, sender: u32, opcode: u32, args: [u32; 4]) {
        panic!("Unhandled name scalar {}: {} {:x?}", sender, opcode, args);
    }

    fn blocking_scalar(
        &mut self,
        _memory: &mut Memory,
        sender: u32,
        opcode: u32,
        args: [u32; 4],
    ) -> ScalarResult {
        panic!(
            "Unhandled name blocking_scalar {}: {} {:x?}",
            sender, opcode, args
        );
        // ScalarResult::Scalar1(0)
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
            "Unhandled name lend {}: {} {:x?} {:x?}",
            sender, opcode, buf, extra
        );
        // LendResult::MemoryReturned([0, 0])
    }

    fn lend_mut(
        &mut self,
        memory: &mut Memory,
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
            println!("Registering name {}", name);

            let service = Box::new(if name == "panic-to-screen!" {
                println!("Panic-to-screen registered");
                super::panic_to_screen::PanicToScreen::new()
            } else {
                panic!("Unrecognized service name {}", name);
            });

            let connection_id = memory.connections.len() as u32 + 1;
            memory.connections.insert(connection_id, service);

            buf[0..4].copy_from_slice(&0u32.to_le_bytes());
            buf[4..8].copy_from_slice(&connection_id.to_le_bytes());
            LendResult::MemoryReturned([0, 0])
        } else {
            panic!(
                "Unhandled name lend_mut {}: {} {:x?}",
                sender, opcode, extra
            );
        }
        //
    }

    fn send(
        &mut self,
        _memory: &mut Memory,
        sender: u32,
        opcode: u32,
        _buf: &[u8],
        extra: [u32; 2],
    ) {
        panic!("Unhandled name send {}: {} {:x?}", sender, opcode, extra);
    }
}
