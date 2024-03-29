use std::{
    collections::HashMap,
    sync::{atomic::Ordering, mpsc::channel, Arc, Mutex},
    thread,
};

use crate::xous::{definitions::SyscallResultNumber, Memory};

use super::{LendResult, Service};

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
    connection_index: Arc<Mutex<HashMap<String, u32>>>,
    name_map: Arc<Mutex<HashMap<String, u128>>>,
}

impl Name {
    pub fn new() -> Self {
        Name {
            connection_index: Arc::new(Mutex::new(HashMap::default())),
            name_map: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    fn djb2_hash(path: &str) -> u128 {
        let mut hash = 5381u128;
        for c in path.bytes() {
            // This algorithm appears to be wrong, but it's what the original
            // algorithm in mkfrogfs.py does.
            hash = (hash << 5).wrapping_add(hash) ^ (c as u128);
        }
        hash
    }

    fn register_name(&self, buf: &mut [u8], rkyv_header: u32) -> LendResult {
        let rkyv_header = rkyv_header as usize;

        let conn_limit_is_some =
            u32::from_le_bytes(buf[rkyv_header..rkyv_header + 4].try_into().unwrap()) != 0;
        let conn_limit = if conn_limit_is_some {
            Some(u32::from_le_bytes(
                buf[rkyv_header + 4..rkyv_header + 8].try_into().unwrap(),
            ))
        } else {
            None
        };

        let name_header = rkyv_header + 8;
        let name_offset = i32::from_le_bytes(buf[name_header..name_header + 4].try_into().unwrap());
        let name_length =
            u32::from_le_bytes(buf[name_header + 4..name_header + 8].try_into().unwrap());
        let name_slice = &buf[name_header + name_offset as usize
            ..name_header + name_offset as usize + name_length as usize];

        let server_name = std::str::from_utf8(name_slice)
            .unwrap_or("<invalid>")
            .to_owned();
        let hash = Self::djb2_hash(&server_name);
        println!(
            "Program is registering service \"{}\" with {}",
            server_name,
            if let Some(max) = conn_limit {
                format!(
                    "a maximum of {} {}",
                    max,
                    if max == 1 {
                        "connection"
                    } else {
                        "connections"
                    }
                )
            } else {
                "an unlimited number of connections".to_owned()
            }
        );

        // Construct the rkyv object by hand.
        let rkyv_offset = 0;
        buf[rkyv_offset..rkyv_offset + 4].copy_from_slice(&2u32.to_le_bytes());
        buf[rkyv_offset + 4..rkyv_offset + 20].copy_from_slice(&hash.to_le_bytes());

        assert!(self
            .name_map
            .lock()
            .unwrap()
            .insert(server_name, hash)
            .is_none());
        LendResult::MemoryReturned([rkyv_offset as u32, 0])
    }
}

impl Default for Name {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for Name {
    fn lend_mut(
        &self,
        memory: &Memory,
        sender: u32,
        opcode: u32,
        buf: &mut [u8],
        extra: [u32; 2],
    ) -> LendResult {
        if opcode == NameLendOpcode::Register as u32 {
            self.register_name(buf, extra[0])
        } else if opcode == NameLendOpcode::TryConnect as u32
            || opcode == NameLendOpcode::BlockingConnect as u32
        {
            let buf_len = buf.len().min(extra[1] as usize);
            let name = std::str::from_utf8(&buf[0..buf_len]).unwrap_or("<invalid>");
            // println!("Connecting to {}", name);

            if let Some(connection_id) = self.connection_index.lock().unwrap().get(name) {
                println!(
                    "Existing server found at connection index {}",
                    connection_id
                );
                buf[0..4].copy_from_slice(&0u32.to_le_bytes());
                buf[4..8].copy_from_slice(&connection_id.to_le_bytes());
                return LendResult::MemoryReturned([0, 0]);
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
            let (tx, rx) = channel();
            let connection_id = memory.connection_index.fetch_add(1, Ordering::Relaxed);
            let connections: Arc<Mutex<HashMap<u32, Box<dyn Service + Send + Sync>>>> =
                memory.connections.clone();
            let name_connection_mapping = self.connection_index.clone();
            let buffer_length = buf.len();
            let name = name.to_owned();
            thread::spawn(move || {
                let mut connections = connections.lock().unwrap();
                connections.insert(connection_id, service);

                // Insert it into the connection map so subsequent lookups get the same service
                name_connection_mapping
                    .lock()
                    .unwrap()
                    .insert(name, connection_id);

                // println!("Inserted new connection {}", connection_id);

                let mut buf = vec![0u8; buffer_length];
                buf[0..4].copy_from_slice(&0u32.to_le_bytes());
                buf[4..8].copy_from_slice(&connection_id.to_le_bytes());
                tx.send((
                    [
                        SyscallResultNumber::MemoryReturned as i32,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ],
                    Some(buf),
                ))
                .unwrap();
            });
            LendResult::WaitForResponse(rx)
        } else {
            panic!(
                "Unhandled name lend_mut {}: {} {:x?}",
                sender, opcode, extra
            );
        }
        //
    }
}
