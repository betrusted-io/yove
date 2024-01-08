use super::Memory as CpuMemory;
use std::collections::HashMap;

const MEMORY_BASE: usize = 0x8000_0000;

/// Emulates main memory.
pub struct Memory {
    /// Memory contents
    data: Vec<u32>,

    /// Offset where RAM lives
    base: usize,

    /// Set to `true` if the program finishes
    vm_result: Option<u32>,

    /// Address of the `tohost` offset
    tohost: u32,

    /// Which addresses are reserved
    reservations: HashMap<u32, u32>,
}

impl Memory {
    /// Creates a new `Memory`
    pub fn new(memory_size: usize, base: usize, tohost: u32) -> Self {
        Memory {
            data: vec![0u32; memory_size / 2],
            base,
            vm_result: None,
            tohost,
            reservations: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn memory_base(&self) -> u32 {
        self.base as u32
    }

    #[allow(dead_code)]
    pub fn vm_result(&self) -> Option<u32> {
        self.vm_result
    }

    pub fn set_tohost(&mut self, tohost: u32) {
        self.tohost = tohost;
    }

    /// Reads multiple bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `width` up to eight
    pub fn read_bytes(&self, address: u32, width: u32) -> u32 {
        let mut data = 0;
        for i in 0..width {
            data |= (self.read_u8(address.wrapping_add(i)) as u32) << (i * 8);
        }
        data
    }

    /// Write multiple bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    /// * `width` up to eight
    pub fn write_bytes(&mut self, address: u32, value: u32, width: u32) {
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
    fn write_u8(&mut self, address: u32, value: u8) {
        let address = address as usize - MEMORY_BASE;
        let index = (address >> 2) as usize;
        let pos = (address % 4) * 8;
        if address == self.tohost as usize {
            panic!("tohost write_u8: {:04x}", value);
        }
        // println!("Writing {:02x} to {:08x}", value, address);
        self.data[index] = (self.data[index] & !(0xff << pos)) | ((value as u32) << pos);
    }

    /// Writes two bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    fn write_u16(&mut self, address: u32, value: u16) {
        if (address % 2) == 0 {
            if address == self.tohost {
                panic!("tohost write_u16: {:04x}", value);
            }
            let address = address - MEMORY_BASE as u32;
            let index = (address >> 2) as usize;
            let pos = (address % 4) * 8;
            self.data[index] = (self.data[index] & !(0xffff << pos)) | ((value as u32) << pos);
        } else {
            self.write_bytes(address, value as u32, 2);
        }
    }

    /// Writes four bytes to memory.
    ///
    /// # Arguments
    /// * `address`
    /// * `value`
    fn write_u32(&mut self, address: u32, value: u32) {
        if (address % 4) == 0 {
            if address == self.tohost {
                println!("tohost write_u32: {:08x}", value);
                self.vm_result = Some(value);
            }
            let address = address - MEMORY_BASE as u32;
            let index = (address >> 2) as usize;
            self.data[index] = value;
        } else {
            self.write_bytes(address, value as u32, 4);
        }
    }

    /// Reads a byte from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u8(&self, address: u32) -> u8 {
        let address = address - MEMORY_BASE as u32;
        let index = (address >> 2) as usize;
        let pos = (address % 4) * 8;
        (self.data[index] >> pos) as u8
    }

    /// Reads two bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u16(&self, address: u32) -> u16 {
        if (address % 2) == 0 {
            let address = address - MEMORY_BASE as u32;
            let index = (address / 4) as usize;
            let pos = (address % 4) * 8;
            (self.data[index] >> pos) as u16
        } else {
            self.read_bytes(address, 2) as u16
        }
    }

    /// Reads four bytes from memory.
    ///
    /// # Arguments
    /// * `address`
    fn read_u32(&self, address: u32) -> u32 {
        let val = if (address % 4) == 0 {
            let address = address - MEMORY_BASE as u32;
            let index = (address / 4) as usize;
            self.data[index]
        } else {
            self.read_bytes(address, 4) as u32
        };
        // println!("Val at {:08x} is {:08x}", address, val);
        val
    }

    /// Check if the address is valid memory address
    ///
    /// # Arguments
    /// * `address`
    fn validate_address(&self, address: u32) -> bool {
        let address = address - MEMORY_BASE as u32;
        (address as usize) < self.data.len()
    }

    fn syscall(&mut self, _args: [i32; 8]) -> crate::mmu::SyscallResult {
        crate::mmu::SyscallResult::Continue
    }

    fn translate(&self, _v_address: u32) -> Option<u32> {
        todo!()
    }

    fn reserve(&mut self, core: u32, p_address: u32) {
        self.reservations.insert(core, p_address);
    }

    fn clear_reservation(&mut self, core: u32, p_address: u32) -> bool {
        self.reservations.remove(&core) == Some(p_address)
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new(16384, 0x0000_0000, 0x8000_1000)
    }
}
