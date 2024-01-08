use super::Memory as CpuMemory;

const MEMORY_BASE: usize = 0x8000_0000;

/// Emulates main memory.
pub struct Memory {
    /// Memory contents
    data: Vec<u64>,

    /// Offset where RAM lives
    base: usize,

    /// Set to `true` if the program finishes
    vm_result: Option<u32>,

    /// Address of the `tohost` offset
    tohost: u64,
}

impl Memory {
    /// Creates a new `Memory`
    pub fn new(memory_size: usize, base: usize, tohost: u64) -> Self {
        Memory {
            data: vec![0u64; memory_size / 4],
            base,
            vm_result: None,
            tohost,
        }
    }

    #[allow(dead_code)]
    pub fn memory_base(&self) -> u64 {
        self.base as u64
    }

    #[allow(dead_code)]
    pub fn vm_result(&self) -> Option<u32> {
        self.vm_result
    }

    /// Reads multiple bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `width` up to eight
    pub fn read_bytes(&self, address: u64, width: u64) -> u64 {
        let mut data = 0;
        for i in 0..width {
            data |= (self.read_u8(address.wrapping_add(i)) as u64) << (i * 8);
        }
        data
    }

    /// Write multiple bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    /// * `width` up to eight
    pub fn write_bytes(&mut self, address: u64, value: u64, width: u64) {
        for i in 0..width {
            self.write_u8(address.wrapping_add(i), (value >> (i * 8)) as u8);
        }
    }
}

impl super::Memory for Memory {
    /// Writes a byte to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    fn write_u8(&mut self, address: u64, value: u8) {
        let address = address - MEMORY_BASE as u64;
        let index = (address >> 3) as usize;
        let pos = (address % 8) * 8;
        if address == self.tohost {
            panic!("tohost write_u8: {:04x}", value);
        }
        self.data[index] = (self.data[index] & !(0xff << pos)) | ((value as u64) << pos);
    }

    /// Writes two bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    fn write_u16(&mut self, address: u64, value: u16) {
        if (address % 2) == 0 {
            if address == self.tohost {
                panic!("tohost write_u16: {:04x}", value);
            }
            let address = address - MEMORY_BASE as u64;
            let index = (address >> 3) as usize;
            let pos = (address % 8) * 8;
            self.data[index] = (self.data[index] & !(0xffff << pos)) | ((value as u64) << pos);
        } else {
            self.write_bytes(address, value as u64, 2);
        }
    }

    /// Writes four bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    fn write_u32(&mut self, address: u64, value: u32) {
        if (address % 4) == 0 {
            if address == self.tohost {
                println!("tohost write_u32: {:08x}", value);
                self.vm_result = Some(value);
            }
            let address = address - MEMORY_BASE as u64;
            let index = (address >> 3) as usize;
            let pos = (address % 8) * 8;
            self.data[index] = (self.data[index] & !(0xffffffff << pos)) | ((value as u64) << pos);
        } else {
            self.write_bytes(address, value as u64, 4);
        }
    }

    /// Writes eight bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    fn write_u64(&mut self, address: u64, value: u64) {
        if (address % 8) == 0 {
            if address == self.tohost {
                panic!("tohost write_u64: {:016x}", value);
            }
            let address = address - MEMORY_BASE as u64;
            let index = (address >> 3) as usize;
            self.data[index] = value;
        } else if (address % 4) == 0 {
            self.write_u32(address, (value & 0xffffffff) as u32);
            self.write_u32(address.wrapping_add(4), (value >> 32) as u32);
        } else {
            self.write_bytes(address, value, 8);
        }
    }

    /// Reads a byte from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u8(&self, address: u64) -> u8 {
        let address = address - MEMORY_BASE as u64;
        let index = (address >> 3) as usize;
        let pos = (address % 8) * 8;
        (self.data[index] >> pos) as u8
    }

    /// Reads two bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u16(&self, address: u64) -> u16 {
        if (address % 2) == 0 {
            let address = address - MEMORY_BASE as u64;
            let index = (address >> 3) as usize;
            let pos = (address % 8) * 8;
            (self.data[index] >> pos) as u16
        } else {
            self.read_bytes(address, 2) as u16
        }
    }

    /// Reads four bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u32(&self, address: u64) -> u32 {
        if (address % 4) == 0 {
            let address = address - MEMORY_BASE as u64;
            let index = (address >> 3) as usize;
            let pos = (address % 8) * 8;
            (self.data[index] >> pos) as u32
        } else {
            self.read_bytes(address, 4) as u32
        }
    }

    /// Reads eight bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u64(&self, address: u64) -> u64 {
        if (address % 8) == 0 {
            let address = address - MEMORY_BASE as u64;
            let index = (address >> 3) as usize;
            self.data[index]
        } else if (address % 4) == 0 {
            (self.read_u32(address) as u64) | ((self.read_u32(address.wrapping_add(4)) as u64) << 4)
        } else {
            self.read_bytes(address, 8)
        }
    }

    /// Check if the address is valid memory address
    ///
    /// # Arguments
    /// * `address`
    fn validate_address(&self, address: u64) -> bool {
        let address = address - MEMORY_BASE as u64;
        (address as usize) < self.data.len()
    }

    fn syscall(&mut self, _args: [i64; 8]) -> crate::mmu::SyscallResult {
        crate::mmu::SyscallResult::Continue
    }

    fn translate(&self, _v_address: u64) -> Option<u64> {
        todo!()
    }

    fn reserve(&mut self, _p_address: u64) -> bool {
        todo!()
    }

    fn clear_reservation(&mut self, _p_address: u64) {
        todo!()
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new(16384, 0x0000_0000, 0x8000_1000)
    }
}
