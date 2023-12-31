pub struct Ticktimer {
    start: std::time::SystemTime,
}

enum ScalarOpcode {
    ElapsedMs = 0,
    WaitForCondition = 8
}

impl Ticktimer {
    pub fn new() -> Self {
        // println!("Constructing a ticktimer");
        Ticktimer {
            start: std::time::SystemTime::now(),
        }
    }
}

impl Default for Ticktimer {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Service for Ticktimer {
    fn scalar(&mut self, sender: u32, opcode: u32, args: [u32; 4]) {
        println!("Ticktimer scalar {}: {} {:x?}", sender, opcode, args);
    }
    fn blocking_scalar(&mut self, sender: u32, opcode: u32, args: [u32; 4]) -> super::ScalarResult {
        if opcode == ScalarOpcode::ElapsedMs as u32 {
            let elapsed_ms = std::time::SystemTime::now()
                .duration_since(self.start)
                .unwrap()
                .as_millis() as u64;
            super::ScalarResult::Scalar2([elapsed_ms as u32, (elapsed_ms >> 32) as u32])
        } else if opcode == ScalarOpcode::WaitForCondition as u32 {
            let start = std::time::SystemTime::now();
            let mut elapsed_ms = start
                .duration_since(self.start)
                .unwrap()
                .as_millis() as u64;
            let mut condition = args[0];
            while condition != 0 {
                std::thread::sleep(std::time::Duration::from_millis(1));
                elapsed_ms = start
                    .duration_since(self.start)
                    .unwrap()
                    .as_millis() as u64;
                condition = args[0];
            }
            super::ScalarResult::Scalar2([elapsed_ms as u32, (elapsed_ms >> 32) as u32])
        } else {
            panic!(
                "Ticktimer blocking_scalar {}: {} {:x?}",
                sender, opcode, args
            );
        }
    }
    fn lend(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) -> [u32; 2] {
        println!("Ticktimer lend {}: {} {:x?}", sender, opcode, extra);
        [0, 0]
    }
    fn lend_mut(&mut self, sender: u32, opcode: u32, _buf: &mut [u8], extra: [u32; 2]) -> [u32; 2] {
        println!("Ticktimer lend_mut {}: {} {:x?}", sender, opcode, extra);
        [0, 0]
    }
    fn send(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) {
        println!("Ticktimer send {}: {} {:x?}", sender, opcode, extra);
    }
}
