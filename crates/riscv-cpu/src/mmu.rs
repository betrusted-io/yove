use std::{
    collections::HashMap,
    sync::mpsc::Receiver,
    sync::{Arc, Mutex},
};

use crate::cpu::{decode_privilege_mode, PrivilegeMode, ResponseData, Trap, TrapType, Xlen};

pub enum SyscallResult {
    Ok([i64; 8]),
    Defer(Receiver<ResponseData>),
    Terminate(usize /* Result */),

    /// Pass the exception to the CPU
    Continue,
}

impl From<[i64; 8]> for SyscallResult {
    fn from(args: [i64; 8]) -> Self {
        SyscallResult::Ok(args)
    }
}

impl From<std::sync::mpsc::Receiver<ResponseData>> for SyscallResult {
    fn from(receiver: std::sync::mpsc::Receiver<ResponseData>) -> Self {
        SyscallResult::Defer(receiver)
    }
}

pub trait Memory {
    fn read_u8(&self, p_address: u64) -> u8;
    fn read_u16(&self, p_address: u64) -> u16;
    fn read_u32(&self, p_address: u64) -> u32;
    fn read_u64(&self, p_address: u64) -> u64;
    fn write_u8(&mut self, p_address: u64, value: u8);
    fn write_u16(&mut self, p_address: u64, value: u16);
    fn write_u32(&mut self, p_address: u64, value: u32);
    fn write_u64(&mut self, p_address: u64, value: u64);
    fn validate_address(&self, address: u64) -> bool;
    fn syscall(&mut self, args: [i64; 8]) -> SyscallResult;
    fn translate(&self, v_address: u64) -> Option<u64>;
    fn reserve(&mut self, p_address: u64) -> bool;
    fn clear_reservation(&mut self, p_address: u64);
}

/// Emulates Memory Management Unit. It holds the Main memory and peripheral
/// devices, maps address to them, and accesses them depending on address.
/// It also manages virtual-physical address translation and memoty protection.
/// It may also be said Bus.
/// @TODO: Memory protection is not implemented yet. We should support.
pub struct Mmu {
    // clock: u64,
    xlen: Xlen,
    ppn: u64,
    addressing_mode: AddressingMode,
    privilege_mode: PrivilegeMode,
    memory: Arc<Mutex<dyn Memory + Send + Sync>>,

    /// Address translation can be affected `mstatus` (MPRV, MPP in machine mode)
    /// then `Mmu` has copy of it.
    mstatus: u64,

    /// A cache of instructions. We assume that instruction memory does not change.
    instruction_cache: Arc<Mutex<HashMap<u64, u32>>>,
}

#[derive(Debug)]
pub enum AddressingMode {
    None,
    SV32,
    SV39,
    SV48, // @TODO: Implement
}

enum MemoryAccessType {
    Execute,
    Read,
    Write,
    DontCare,
}

fn _get_addressing_mode_name(mode: &AddressingMode) -> &'static str {
    match mode {
        AddressingMode::None => "None",
        AddressingMode::SV32 => "SV32",
        AddressingMode::SV39 => "SV39",
        AddressingMode::SV48 => "SV48",
    }
}

impl Mmu {
    /// Creates a new `Mmu`.
    ///
    /// # Arguments
    /// * `xlen`
    pub fn new(xlen: Xlen, memory: Arc<Mutex<dyn Memory + Send + Sync>>) -> Self {
        Mmu {
            // clock: 0,
            xlen,
            ppn: 0,
            addressing_mode: AddressingMode::None,
            privilege_mode: PrivilegeMode::Machine,
            memory,
            mstatus: 0,
            instruction_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Updates XLEN, 32-bit or 64-bit
    ///
    /// # Arguments
    /// * `xlen`
    pub fn update_xlen(&mut self, xlen: Xlen) {
        self.xlen = xlen;
    }

    /// Runs one cycle of MMU and peripheral devices.
    pub fn tick(&mut self, _mip: &mut u64) {}

    /// Updates addressing mode
    ///
    /// # Arguments
    /// * `new_addressing_mode`
    pub fn update_addressing_mode(&mut self, new_addressing_mode: AddressingMode) {
        self.addressing_mode = new_addressing_mode;
    }

    /// Updates privilege mode
    ///
    /// # Arguments
    /// * `mode`
    pub fn update_privilege_mode(&mut self, mode: PrivilegeMode) {
        self.privilege_mode = mode;
    }

    /// Updates mstatus copy. `CPU` needs to call this method whenever
    /// `mstatus` is updated.
    ///
    /// # Arguments
    /// * `mstatus`
    pub fn update_mstatus(&mut self, mstatus: u64) {
        self.mstatus = mstatus;
    }

    /// Updates PPN used for address translation
    ///
    /// # Arguments
    /// * `ppn`
    pub fn update_ppn(&mut self, ppn: u64) {
        self.ppn = ppn;
    }

    fn trim_to_xlen(&self, address: u64) -> u64 {
        match self.xlen {
            Xlen::Bit32 => address & 0xffffffff,
            Xlen::Bit64 => address,
        }
    }

    /// Fetches an instruction byte. This method takes virtual address
    /// and translates into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    fn fetch(&self, v_address: u64) -> Result<u8, Trap> {
        self.translate_address(v_address, &MemoryAccessType::Execute)
            .map(|p_address| self.load_raw(p_address))
            .map_err(|()| Trap {
                trap_type: TrapType::InstructionPageFault,
                value: v_address,
            })
    }

    /// Fetches instruction four bytes. This method takes virtual address
    /// and translates into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    pub fn fetch_word(&self, v_address: u64) -> Result<u32, Trap> {
        // if let Some(data) = self.instruction_cache.lock().unwrap().get(&v_address) {
        //     return Ok(*data);
        // }
        let width = 4;
        if (v_address & 0xfff) <= (0x1000 - width) {
            // Fast path. All bytes fetched are in the same page so
            // translating an address only once.
            let effective_address = self.trim_to_xlen(v_address);
            self.translate_address(effective_address, &MemoryAccessType::Execute)
                .map(|p_address| {
                    let data = self.load_word_raw(p_address);
                    self.instruction_cache
                        .lock()
                        .unwrap()
                        .insert(v_address, data);
                    data
                })
                .map_err(|()| Trap {
                    trap_type: TrapType::InstructionPageFault,
                    value: effective_address,
                })
        } else {
            let mut data = 0;
            for i in 0..width {
                match self.fetch(v_address.wrapping_add(i)) {
                    Ok(byte) => data |= (byte as u32) << (i * 8),
                    Err(e) => return Err(e),
                };
            }
            self.instruction_cache
                .lock()
                .unwrap()
                .insert(v_address, data);
            Ok(data)
        }
    }

    /// Loads an byte. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    pub fn load(&self, v_address: u64) -> Result<u8, Trap> {
        let effective_address = self.trim_to_xlen(v_address);
        match self.translate_address(effective_address, &MemoryAccessType::Read) {
            Ok(p_address) => Ok(self.load_raw(p_address)),
            Err(()) => Err(Trap {
                trap_type: TrapType::LoadPageFault,
                value: v_address,
            }),
        }
    }

    /// Loads multiple bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    /// * `width` Must be 1, 2, 4, or 8
    fn load_bytes(&self, v_address: u64, width: u64) -> Result<u64, Trap> {
        debug_assert!(
            width == 1 || width == 2 || width == 4 || width == 8,
            "Width must be 1, 2, 4, or 8. {:X}",
            width
        );
        match (v_address & 0xfff) <= (0x1000 - width) {
            true => match self.translate_address(v_address, &MemoryAccessType::Read) {
                Ok(p_address) => {
                    // Fast path. All bytes fetched are in the same page so
                    // translating an address only once.
                    match width {
                        1 => Ok(self.load_raw(p_address) as u64),
                        2 => Ok(self.load_halfword_raw(p_address) as u64),
                        4 => Ok(self.load_word_raw(p_address) as u64),
                        8 => Ok(self.load_doubleword_raw(p_address)),
                        _ => panic!("Width must be 1, 2, 4, or 8. {:X}", width),
                    }
                }
                Err(()) => Err(Trap {
                    trap_type: TrapType::LoadPageFault,
                    value: v_address,
                }),
            },
            false => {
                let mut data = 0;
                for i in 0..width {
                    match self.load(v_address.wrapping_add(i)) {
                        Ok(byte) => data |= (byte as u64) << (i * 8),
                        Err(e) => return Err(e),
                    };
                }
                Ok(data)
            }
        }
    }

    /// Loads two bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    pub fn load_halfword(&self, v_address: u64) -> Result<u16, Trap> {
        self.load_bytes(v_address, 2).map(|data| data as u16)
    }

    /// Loads four bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    pub fn load_word(&self, v_address: u64) -> Result<u32, Trap> {
        self.load_bytes(v_address, 4).map(|data| data as u32)
    }

    /// Loads eight bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    pub fn load_doubleword(&self, v_address: u64) -> Result<u64, Trap> {
        self.load_bytes(v_address, 8)
    }

    /// Store an byte. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    /// * `value`
    pub fn store(&self, v_address: u64, value: u8) -> Result<(), Trap> {
        self.translate_address(v_address, &MemoryAccessType::Write)
            .map(|p_address| self.store_raw(p_address, value))
            .map_err(|()| Trap {
                trap_type: TrapType::StorePageFault,
                value: v_address,
            })
    }

    /// Stores multiple bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    /// * `value` data written
    /// * `width` Must be 1, 2, 4, or 8
    fn store_bytes(&self, v_address: u64, value: u64, width: u64) -> Result<(), Trap> {
        debug_assert!(
            width == 1 || width == 2 || width == 4 || width == 8,
            "Width must be 1, 2, 4, or 8. {:X}",
            width
        );
        match (v_address & 0xfff) <= (0x1000 - width) {
            true => match self.translate_address(v_address, &MemoryAccessType::Write) {
                Ok(p_address) => {
                    // Fast path. All bytes fetched are in the same page so
                    // translating an address only once.
                    match width {
                        1 => self.store_raw(p_address, value as u8),
                        2 => self.store_halfword_raw(p_address, value as u16),
                        4 => self.store_word_raw(p_address, value as u32),
                        8 => self.store_doubleword_raw(p_address, value),
                        _ => panic!("Width must be 1, 2, 4, or 8. {:X}", width),
                    }
                    Ok(())
                }
                Err(()) => Err(Trap {
                    trap_type: TrapType::StorePageFault,
                    value: v_address,
                }),
            },
            false => {
                for i in 0..width {
                    match self.store(v_address.wrapping_add(i), ((value >> (i * 8)) & 0xff) as u8) {
                        Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
        }
    }

    /// Stores two bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    /// * `value` data written
    pub fn store_halfword(&self, v_address: u64, value: u16) -> Result<(), Trap> {
        self.store_bytes(v_address, value as u64, 2)
    }

    /// Stores four bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    /// * `value` data written
    pub fn store_word(&self, v_address: u64, value: u32) -> Result<(), Trap> {
        self.store_bytes(v_address, value as u64, 4)
    }

    /// Stores eight bytes. This method takes virtual address and translates
    /// into physical address inside.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    /// * `value` data written
    pub fn store_doubleword(&self, v_address: u64, value: u64) -> Result<(), Trap> {
        self.store_bytes(v_address, value, 8)
    }

    /// Loads a byte from main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    pub(crate) fn load_raw(&self, p_address: u64) -> u8 {
        self.memory
            .lock() // .read()
            .unwrap()
            .read_u8(self.trim_to_xlen(p_address))
    }

    /// Loads two bytes from main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    fn load_halfword_raw(&self, p_address: u64) -> u16 {
        self.memory
            .lock() // .read()
            .unwrap()
            .read_u16(self.trim_to_xlen(p_address))
    }

    /// Loads four bytes from main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    pub fn load_word_raw(&self, p_address: u64) -> u32 {
        self.memory
            .lock() // .read()
            .unwrap()
            .read_u32(self.trim_to_xlen(p_address))
    }

    /// Loads eight bytes from main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    fn load_doubleword_raw(&self, p_address: u64) -> u64 {
        self.memory
            .lock() // .read()
            .unwrap()
            .read_u64(self.trim_to_xlen(p_address))
    }

    /// Stores a byte to main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    /// * `value` data written
    pub(crate) fn store_raw(&self, p_address: u64, value: u8) {
        self.memory
            .lock() // .write()
            .unwrap()
            .write_u8(self.trim_to_xlen(p_address), value)
    }

    /// Stores two bytes to main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    /// * `value` data written
    pub(crate) fn store_halfword_raw(&self, p_address: u64, value: u16) {
        self.memory
            .lock() // .write()
            .unwrap()
            .write_u16(self.trim_to_xlen(p_address), value)
    }

    /// Stores four bytes to main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    /// * `value` data written
    pub(crate) fn store_word_raw(&self, p_address: u64, value: u32) {
        self.memory
            .lock() // .write()
            .unwrap()
            .write_u32(self.trim_to_xlen(p_address), value)
    }

    /// Stores eight bytes to main memory or peripheral devices depending on
    /// physical address.
    ///
    /// # Arguments
    /// * `p_address` Physical address
    /// * `value` data written
    fn store_doubleword_raw(&self, p_address: u64, value: u64) {
        self.memory
            .lock() // .write()
            .unwrap()
            .write_u64(self.trim_to_xlen(p_address), value)
    }

    /// Checks if passed virtual address is valid (pointing a certain device) or not.
    /// This method can return page fault trap.
    ///
    /// # Arguments
    /// * `v_address` Virtual address
    pub fn validate_address(&self, v_address: u64) -> Option<bool> {
        self.translate_address(v_address, &MemoryAccessType::DontCare)
            .ok()
            .map(|p_address| {
                self.memory
                    .lock() // .read()
                    .unwrap()
                    .validate_address(self.trim_to_xlen(p_address))
            })
    }

    pub fn reserve(&mut self, p_address: u64) -> bool {
        self.memory
            .lock() // .write()
            .unwrap()
            .reserve(self.trim_to_xlen(p_address))
    }

    pub fn clear_reservation(&mut self, p_address: u64) {
        self.memory
            .lock() // .write()
            .unwrap()
            .clear_reservation(self.trim_to_xlen(p_address))
    }

    fn translate_address(&self, v_address: u64, access_type: &MemoryAccessType) -> Result<u64, ()> {
        if let AddressingMode::None = self.addressing_mode {
            Ok(v_address)
        } else {
            // self.memory.lock() // .read().unwrap().translate(v_address).ok_or(())
            let phys = self.translate_address_with_privilege_mode(
                v_address,
                access_type,
                self.privilege_mode,
            )?;
            Ok(phys)
        }
    }

    fn translate_address_with_privilege_mode(
        &self,
        v_address: u64,
        access_type: &MemoryAccessType,
        privilege_mode: PrivilegeMode,
    ) -> Result<u64, ()> {
        let address = self.trim_to_xlen(v_address);

        match self.addressing_mode {
            AddressingMode::None => Ok(address),
            AddressingMode::SV32 => match privilege_mode {
                // @TODO: Optimize
                PrivilegeMode::Machine => {
                    if let MemoryAccessType::Execute = access_type {
                        Ok(address)
                    } else if (self.mstatus >> 17) & 1 == 0 {
                        Ok(address)
                    } else {
                        match decode_privilege_mode((self.mstatus >> 9) & 3) {
                            PrivilegeMode::Machine => Ok(address),
                            temp_privilege_mode => self.translate_address_with_privilege_mode(
                                v_address,
                                access_type,
                                temp_privilege_mode,
                            ),
                        }
                    }
                }
                PrivilegeMode::User | PrivilegeMode::Supervisor => {
                    let vpns = [(address >> 12) & 0x3ff, (address >> 22) & 0x3ff];
                    self.traverse_page(address, 2 - 1, self.ppn, &vpns, access_type)
                }
                _ => Ok(address),
            },
            AddressingMode::SV39 => match self.privilege_mode {
                // @TODO: Optimize
                // @TODO: Remove duplicated code with SV32
                PrivilegeMode::Machine => {
                    if let MemoryAccessType::Execute = access_type {
                        Ok(address)
                    } else if (self.mstatus >> 17) & 1 == 0 {
                        Ok(address)
                    } else {
                        match decode_privilege_mode((self.mstatus >> 9) & 3) {
                            PrivilegeMode::Machine => Ok(address),
                            temp_privilege_mode => self.translate_address_with_privilege_mode(
                                v_address,
                                access_type,
                                temp_privilege_mode,
                            ),
                        }
                    }
                }
                PrivilegeMode::User | PrivilegeMode::Supervisor => {
                    let vpns = [
                        (address >> 12) & 0x1ff,
                        (address >> 21) & 0x1ff,
                        (address >> 30) & 0x1ff,
                    ];
                    self.traverse_page(address, 3 - 1, self.ppn, &vpns, access_type)
                }
                _ => Ok(address),
            },
            AddressingMode::SV48 => {
                panic!("AddressingMode SV48 is not supported yet.");
            }
        }
    }

    fn traverse_page(
        &self,
        v_address: u64,
        level: u8,
        parent_ppn: u64,
        vpns: &[u64],
        access_type: &MemoryAccessType,
    ) -> Result<u64, ()> {
        let pagesize = 4096;
        let ptesize = match self.addressing_mode {
            AddressingMode::SV32 => 4,
            _ => 8,
        };
        let pte_address = parent_ppn * pagesize + vpns[level as usize] * ptesize;
        let pte = match self.addressing_mode {
            AddressingMode::SV32 => self.load_word_raw(pte_address) as u64,
            _ => self.load_doubleword_raw(pte_address),
        };
        let ppn = match self.addressing_mode {
            AddressingMode::SV32 => (pte >> 10) & 0x3fffff,
            _ => (pte >> 10) & 0xfffffffffff,
        };
        let ppns = match self.addressing_mode {
            AddressingMode::SV32 => [(pte >> 10) & 0x3ff, (pte >> 20) & 0xfff, 0 /*dummy*/],
            AddressingMode::SV39 => [
                (pte >> 10) & 0x1ff,
                (pte >> 19) & 0x1ff,
                (pte >> 28) & 0x3ffffff,
            ],
            _ => panic!(), // Shouldn't happen
        };
        let _rsw = (pte >> 8) & 0x3;
        let d = (pte >> 7) & 1;
        let a = (pte >> 6) & 1;
        let _g = (pte >> 5) & 1;
        let _u = (pte >> 4) & 1;
        let x = (pte >> 3) & 1;
        let w = (pte >> 2) & 1;
        let r = (pte >> 1) & 1;
        let v = pte & 1;

        // println!("VA:{:X} Level:{:X} PTE_AD:{:X} PTE:{:X} PPPN:{:X} PPN:{:X} PPN1:{:X} PPN0:{:X}", v_address, level, pte_address, pte, parent_ppn, ppn, ppns[1], ppns[0]);

        if v == 0 || (r == 0 && w == 1) {
            return Err(());
        }

        if r == 0 && x == 0 {
            return match level {
                0 => Err(()),
                _ => self.traverse_page(v_address, level - 1, ppn, vpns, access_type),
            };
        }

        // Leaf page found

        if a == 0
            || (match access_type {
                MemoryAccessType::Write => d == 0,
                _ => false,
            })
        {
            let new_pte = pte
                | (1 << 6)
                | (match access_type {
                    MemoryAccessType::Write => 1 << 7,
                    _ => 0,
                });
            match self.addressing_mode {
                AddressingMode::SV32 => self.store_word_raw(pte_address, new_pte as u32),
                _ => self.store_doubleword_raw(pte_address, new_pte),
            };
        }

        match access_type {
            MemoryAccessType::Execute => {
                if x == 0 {
                    return Err(());
                }
            }
            MemoryAccessType::Read => {
                if r == 0 {
                    return Err(());
                }
            }
            MemoryAccessType::Write => {
                if w == 0 {
                    return Err(());
                }
            }
            _ => {}
        };

        let offset = v_address & 0xfff; // [11:0]
                                        // @TODO: Optimize
        let p_address = match self.addressing_mode {
            AddressingMode::SV32 => match level {
                1 => {
                    if ppns[0] != 0 {
                        return Err(());
                    }
                    (ppns[1] << 22) | (vpns[0] << 12) | offset
                }
                0 => (ppn << 12) | offset,
                _ => panic!(), // Shouldn't happen
            },
            _ => match level {
                2 => {
                    if ppns[1] != 0 || ppns[0] != 0 {
                        return Err(());
                    }
                    (ppns[2] << 30) | (vpns[1] << 21) | (vpns[0] << 12) | offset
                }
                1 => {
                    if ppns[0] != 0 {
                        return Err(());
                    }
                    (ppns[2] << 30) | (ppns[1] << 21) | (vpns[0] << 12) | offset
                }
                0 => (ppn << 12) | offset,
                _ => panic!(), // Shouldn't happen
            },
        };

        // println!("PA:{:X}", p_address);
        Ok(p_address)
    }
}
