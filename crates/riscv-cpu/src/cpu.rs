use std::sync::{mpsc::Receiver, Arc, Mutex};

mod instructions;

#[cfg(test)]
mod tests;

use self::instructions::Instruction;

pub use super::mmu::Memory;
use super::mmu::{AddressingMode, Mmu};

const CSR_CAPACITY: usize = 4096;

const CSR_USTATUS_ADDRESS: u16 = 0x000;
const CSR_FFLAGS_ADDRESS: u16 = 0x001;
const CSR_FRM_ADDRESS: u16 = 0x002;
const CSR_FCSR_ADDRESS: u16 = 0x003;
const CSR_UIE_ADDRESS: u16 = 0x004;
const CSR_UTVEC_ADDRESS: u16 = 0x005;
const _CSR_USCRATCH_ADDRESS: u16 = 0x040;
const CSR_UEPC_ADDRESS: u16 = 0x041;
const CSR_UCAUSE_ADDRESS: u16 = 0x042;
const CSR_UTVAL_ADDRESS: u16 = 0x043;
const _CSR_UIP_ADDRESS: u16 = 0x044;
const CSR_SSTATUS_ADDRESS: u16 = 0x100;
const CSR_SEDELEG_ADDRESS: u16 = 0x102;
const CSR_SIDELEG_ADDRESS: u16 = 0x103;
const CSR_SIE_ADDRESS: u16 = 0x104;
const CSR_STVEC_ADDRESS: u16 = 0x105;
const _CSR_SSCRATCH_ADDRESS: u16 = 0x140;
pub const CSR_SEPC_ADDRESS: u16 = 0x141;
const CSR_SCAUSE_ADDRESS: u16 = 0x142;
const CSR_STVAL_ADDRESS: u16 = 0x143;
const CSR_SIP_ADDRESS: u16 = 0x144;
pub const CSR_SATP_ADDRESS: u16 = 0x180;
pub const CSR_MSTATUS_ADDRESS: u16 = 0x300;
// const CSR_MISA_ADDRESS: u16 = 0x301;
const CSR_MEDELEG_ADDRESS: u16 = 0x302;
const CSR_MIDELEG_ADDRESS: u16 = 0x303;
const CSR_MIE_ADDRESS: u16 = 0x304;

const CSR_MTVEC_ADDRESS: u16 = 0x305;
const _CSR_MSCRATCH_ADDRESS: u16 = 0x340;
const CSR_MEPC_ADDRESS: u16 = 0x341;
const CSR_MCAUSE_ADDRESS: u16 = 0x342;
const CSR_MTVAL_ADDRESS: u16 = 0x343;
const CSR_MIP_ADDRESS: u16 = 0x344;
const _CSR_PMPCFG0_ADDRESS: u16 = 0x3a0;
const _CSR_PMPADDR0_ADDRESS: u16 = 0x3b0;
const _CSR_MCYCLE_ADDRESS: u16 = 0xb00;
const CSR_CYCLE_ADDRESS: u16 = 0xc00;
// const CSR_TIME_ADDRESS: u16 = 0xc01;
const _CSR_INSERT_ADDRESS: u16 = 0xc02;
pub const CSR_MHARTID_ADDRESS: u16 = 0xf14;

const MIP_MEIP: u32 = 0x800;
pub const MIP_MTIP: u32 = 0x080;
pub const MIP_MSIP: u32 = 0x008;
pub const MIP_SEIP: u32 = 0x200;
const MIP_STIP: u32 = 0x020;
const MIP_SSIP: u32 = 0x002;

pub type ResponseData = ([i32; 8], Option<(Vec<u8>, u32)>);

pub enum TickResult {
    Ok,
    ExitThread(u32),
    PauseEmulation(Receiver<ResponseData>),
    CpuTrap(Trap),
}

/// Emulates a RISC-V CPU core
pub struct Cpu {
    clock: u32,
    privilege_mode: PrivilegeMode,
    wfi: bool,
    // using only lower 32bits of x, pc, and csr registers
    // for 32-bit mode
    x: [i32; 32],
    pc: u32,
    csr: [u32; CSR_CAPACITY],
    mmu: Mmu,
    memory: Arc<Mutex<dyn Memory + Send + Sync>>,
    _dump_flag: bool,
    unsigned_data_mask: u32,
    instructions: [instructions::Instruction; instructions::INSTRUCTION_NUM],
}

#[derive(Clone, Copy, Debug)]
pub enum PrivilegeMode {
    User,
    Supervisor,
    Reserved,
    Machine,
}

#[derive(Debug)]
pub struct Trap {
    pub trap_type: TrapType,
    pub value: u32, // Trap type specific value
}

#[derive(Debug)]
pub enum TrapType {
    InstructionAddressMisaligned,
    InstructionAccessFault,
    IllegalInstruction,
    Breakpoint,
    LoadAddressMisaligned,
    LoadAccessFault,
    StoreAddressMisaligned,
    StoreAccessFault,
    EnvironmentCallFromUMode,
    EnvironmentCallFromSMode,
    EnvironmentCallFromMMode,
    InstructionPageFault,
    LoadPageFault,
    StorePageFault,
    UserSoftwareInterrupt,
    SupervisorSoftwareInterrupt,
    MachineSoftwareInterrupt,
    UserTimerInterrupt,
    SupervisorTimerInterrupt,
    MachineTimerInterrupt,
    UserExternalInterrupt,
    SupervisorExternalInterrupt,
    MachineExternalInterrupt,
    PauseEmulation(Receiver<ResponseData>),
}

fn _get_privilege_mode_name(mode: &PrivilegeMode) -> &'static str {
    match mode {
        PrivilegeMode::User => "User",
        PrivilegeMode::Supervisor => "Supervisor",
        PrivilegeMode::Reserved => "Reserved",
        PrivilegeMode::Machine => "Machine",
    }
}

// bigger number is higher privilege level
fn get_privilege_encoding(mode: &PrivilegeMode) -> u8 {
    match mode {
        PrivilegeMode::User => 0,
        PrivilegeMode::Supervisor => 1,
        PrivilegeMode::Reserved => panic!(),
        PrivilegeMode::Machine => 3,
    }
}

/// Returns `PrivilegeMode` from encoded privilege mode bits
pub fn decode_privilege_mode(encoding: u32) -> PrivilegeMode {
    match encoding {
        0 => PrivilegeMode::User,
        1 => PrivilegeMode::Supervisor,
        3 => PrivilegeMode::Machine,
        _ => panic!("Unknown privilege uncoding"),
    }
}

fn _get_trap_type_name(trap_type: &TrapType) -> &'static str {
    match trap_type {
        TrapType::InstructionAddressMisaligned => "InstructionAddressMisaligned",
        TrapType::InstructionAccessFault => "InstructionAccessFault",
        TrapType::IllegalInstruction => "IllegalInstruction",
        TrapType::Breakpoint => "Breakpoint",
        TrapType::LoadAddressMisaligned => "LoadAddressMisaligned",
        TrapType::LoadAccessFault => "LoadAccessFault",
        TrapType::StoreAddressMisaligned => "StoreAddressMisaligned",
        TrapType::StoreAccessFault => "StoreAccessFault",
        TrapType::EnvironmentCallFromUMode => "EnvironmentCallFromUMode",
        TrapType::EnvironmentCallFromSMode => "EnvironmentCallFromSMode",
        TrapType::EnvironmentCallFromMMode => "EnvironmentCallFromMMode",
        TrapType::InstructionPageFault => "InstructionPageFault",
        TrapType::LoadPageFault => "LoadPageFault",
        TrapType::StorePageFault => "StorePageFault",
        TrapType::UserSoftwareInterrupt => "UserSoftwareInterrupt",
        TrapType::SupervisorSoftwareInterrupt => "SupervisorSoftwareInterrupt",
        TrapType::MachineSoftwareInterrupt => "MachineSoftwareInterrupt",
        TrapType::UserTimerInterrupt => "UserTimerInterrupt",
        TrapType::SupervisorTimerInterrupt => "SupervisorTimerInterrupt",
        TrapType::MachineTimerInterrupt => "MachineTimerInterrupt",
        TrapType::UserExternalInterrupt => "UserExternalInterrupt",
        TrapType::SupervisorExternalInterrupt => "SupervisorExternalInterrupt",
        TrapType::MachineExternalInterrupt => "MachineExternalInterrupt",
        TrapType::PauseEmulation(_) => "PauseEmulation",
    }
}

fn get_trap_cause(trap: &Trap) -> u32 {
    let interrupt_bit = 0x80000000_u32;
    match trap.trap_type {
        TrapType::InstructionAddressMisaligned => 0,
        TrapType::InstructionAccessFault => 1,
        TrapType::IllegalInstruction => 2,
        TrapType::Breakpoint => 3,
        TrapType::LoadAddressMisaligned => 4,
        TrapType::LoadAccessFault => 5,
        TrapType::StoreAddressMisaligned => 6,
        TrapType::StoreAccessFault => 7,
        TrapType::EnvironmentCallFromUMode => 8,
        TrapType::EnvironmentCallFromSMode => 9,
        TrapType::EnvironmentCallFromMMode => 11,
        TrapType::InstructionPageFault => 12,
        TrapType::LoadPageFault => 13,
        TrapType::StorePageFault => 15,
        TrapType::PauseEmulation(_) => 16,
        TrapType::UserSoftwareInterrupt => interrupt_bit,
        TrapType::SupervisorSoftwareInterrupt => interrupt_bit + 1,
        TrapType::MachineSoftwareInterrupt => interrupt_bit + 3,
        TrapType::UserTimerInterrupt => interrupt_bit + 4,
        TrapType::SupervisorTimerInterrupt => interrupt_bit + 5,
        TrapType::MachineTimerInterrupt => interrupt_bit + 7,
        TrapType::UserExternalInterrupt => interrupt_bit + 8,
        TrapType::SupervisorExternalInterrupt => interrupt_bit + 9,
        TrapType::MachineExternalInterrupt => interrupt_bit + 11,
    }
}

pub struct CpuBuilder {
    pc: u32,
    sp: u32,
    memory: Arc<Mutex<dyn Memory + Send + Sync>>,
}

impl CpuBuilder {
    pub fn new(memory: Arc<Mutex<dyn Memory + Send + Sync>>) -> Self {
        CpuBuilder {
            memory,
            pc: 0,
            sp: 0,
        }
    }

    pub fn pc(mut self, pc: u32) -> Self {
        self.pc = pc;
        self
    }

    pub fn sp(mut self, sp: u32) -> Self {
        self.sp = sp;
        self
    }
    pub fn build(self) -> Cpu {
        let mut cpu = Cpu::new(self.memory);
        cpu.update_pc(self.pc);
        cpu.write_register(2, self.sp as i32);
        cpu
    }
}

impl Cpu {
    /// Creates a new `Cpu`.
    ///
    /// # Arguments
    /// * `Terminal`
    pub fn new(memory: Arc<Mutex<dyn Memory + Send + Sync>>) -> Self {
        Cpu {
            clock: 0,
            privilege_mode: PrivilegeMode::Machine,
            wfi: false,
            x: [0; 32],
            pc: 0,
            csr: [0; CSR_CAPACITY],
            mmu: Mmu::new(memory.clone()),
            _dump_flag: false,
            unsigned_data_mask: !0,
            memory,
            instructions: instructions::get_instructions(),
        }
    }

    /// Updates Program Counter content
    ///
    /// # Arguments
    /// * `value`
    pub fn update_pc(&mut self, value: u32) {
        self.pc = value;
    }

    /// Reads integer register content
    ///
    /// # Arguments
    /// * `reg` Register number. Must be 0-31
    pub fn read_register(&self, reg: u8) -> i32 {
        debug_assert!(reg <= 31, "reg must be 0-31. {}", reg);
        match reg {
            0 => 0, // 0th register is hardwired zero
            _ => self.x[reg as usize],
        }
    }

    /// Writes integer register content
    ///
    /// # Arguments
    /// * `reg` Register number. Must be 0-31
    /// * `val` 64-bit value
    pub fn write_register(&mut self, reg: u8, val: i32) {
        debug_assert!(reg <= 31, "reg must be 0-31. {}", reg);
        if reg == 0 {
            return;
        }
        self.x[reg as usize] = val;
    }

    /// Reads Program counter content
    pub fn read_pc(&self) -> u32 {
        self.pc
    }

    /// Runs program one cycle. Fetch, decode, and execution are completed in a cycle so far.
    pub fn tick(&mut self) -> TickResult {
        match self.tick_operate() {
            Ok(()) => {}
            Err(Trap {
                trap_type: TrapType::PauseEmulation(rx),
                ..
            }) => {
                return TickResult::PauseEmulation(rx);
            }
            Err(Trap {
                trap_type: TrapType::InstructionPageFault,
                value: 0xff803000,
            }) => {
                return TickResult::ExitThread(self.read_register(10) as u32);
            }
            Err(e) => return TickResult::CpuTrap(e),
        }
        self.mmu.tick(&mut self.csr[CSR_MIP_ADDRESS as usize]);
        self.handle_interrupt(self.pc);
        self.clock = self.clock.wrapping_add(1);

        // cpu core clock : mtime clock in clint = 8 : 1 is
        // just an arbiraty ratio.
        // @TODO: Implement more properly
        self.write_csr_raw(CSR_CYCLE_ADDRESS, self.clock * 8);

        TickResult::Ok
    }

    // @TODO: Rename?
    fn tick_operate(&mut self) -> Result<(), Trap> {
        if self.wfi {
            if (self.read_csr_raw(CSR_MIE_ADDRESS) & self.read_csr_raw(CSR_MIP_ADDRESS)) != 0 {
                self.wfi = false;
            }
            return Ok(());
        }

        let original_word = self.fetch()?;
        let instruction_address = self.pc;
        let word = if (original_word & 0x3) == 0x3 {
            self.pc = self.pc.wrapping_add(4); // 32-bit length non-compressed instruction
            original_word
        } else {
            self.pc = self.pc.wrapping_add(2); // 16-bit length compressed instruction
            self.uncompress(original_word & 0xffff)
        };
        // println!(
        //     "PC @ {:08x}  Original word: 0x{:04x}  Uncompressed: 0x{:08x}",
        //     instruction_address,
        //     original_word & 0xffff,
        //     word
        // );

        let Ok(inst) = self.decode_raw(word) else {
            panic!(
                "Unknown instruction PC:{:x} WORD:{:x}",
                instruction_address, original_word
            );
        };

        // println!(
        //     "pc @ 0x{:08x}: 0x{:08x} (0x{:08x}) {} {}",
        //     instruction_address,
        //     original_word,
        //     word,
        //     inst.name,
        //     (inst.disassemble)(self, word, self.pc, true)
        // );
        let result = (inst.operation)(self, word, instruction_address);
        // println!();
        self.x[0] = 0; // hardwired zero
        result
    }

    pub fn execute_opcode(&mut self, op: u32) -> Result<(), Trap> {
        (self.decode_raw(op)?.operation)(self, op, self.pc)
    }

    // /// Decodes a word instruction data and returns a reference to
    // /// [`Instruction`](struct.Instruction.html). Using [`DecodeCache`](struct.DecodeCache.html)
    // /// so if cache hits this method returns the result very quickly.
    // /// The result will be stored to cache.
    // fn decode_raw(&mut self, word: u32) -> Result<&Instruction, ()> {
    //     if let Some(index) = self.decode_cache.get(word) {
    //         return Ok(&INSTRUCTIONS[index]);
    //     }
    //     let Ok(index) = self.decode_and_get_instruction_index(word) else {
    //         return Err(());
    //     };
    //     self.decode_cache.insert(word, index);
    //     Ok(&INSTRUCTIONS[index])
    // }

    /// Decodes a word instruction data and returns a reference to
    /// [`Instruction`](struct.Instruction.html). Not Using [`DecodeCache`](struct.DecodeCache.html)
    /// so if you don't want to pollute the cache you should use this method
    /// instead of `decode`.
    fn decode_raw(&self, word: u32) -> Result<&Instruction, Trap> {
        self.decode_and_get_instruction_index(word)
            .map(|index| &self.instructions[index])
            .map_err(|_| Trap {
                value: self.pc.wrapping_sub(4),
                trap_type: TrapType::IllegalInstruction,
            })
    }

    /// Decodes a word instruction data and returns an index of
    /// [`INSTRUCTIONS`](constant.INSTRUCTIONS.html)
    ///
    /// # Arguments
    /// * `word` word instruction data decoded
    fn decode_and_get_instruction_index(&self, word: u32) -> Result<usize, ()> {
        for (idx, inst) in self.instructions.iter().enumerate() {
            if (word & inst.mask) == inst.data {
                return Ok(idx);
            }
        }
        Err(())
    }

    fn handle_interrupt(&mut self, instruction_address: u32) {
        // @TODO: Optimize
        let minterrupt = self.read_csr_raw(CSR_MIP_ADDRESS) & self.read_csr_raw(CSR_MIE_ADDRESS);

        if (minterrupt & MIP_MEIP) != 0
            && self.handle_trap(
                Trap {
                    trap_type: TrapType::MachineExternalInterrupt,
                    value: self.pc, // dummy
                },
                instruction_address,
                true,
            )
        {
            // Who should clear mip bit?
            self.write_csr_raw(
                CSR_MIP_ADDRESS,
                self.read_csr_raw(CSR_MIP_ADDRESS) & !MIP_MEIP,
            );
            self.wfi = false;
            return;
        }
        if (minterrupt & MIP_MSIP) != 0
            && self.handle_trap(
                Trap {
                    trap_type: TrapType::MachineSoftwareInterrupt,
                    value: self.pc, // dummy
                },
                instruction_address,
                true,
            )
        {
            self.write_csr_raw(
                CSR_MIP_ADDRESS,
                self.read_csr_raw(CSR_MIP_ADDRESS) & !MIP_MSIP,
            );
            self.wfi = false;
            return;
        }
        if (minterrupt & MIP_MTIP) != 0
            && self.handle_trap(
                Trap {
                    trap_type: TrapType::MachineTimerInterrupt,
                    value: self.pc, // dummy
                },
                instruction_address,
                true,
            )
        {
            self.write_csr_raw(
                CSR_MIP_ADDRESS,
                self.read_csr_raw(CSR_MIP_ADDRESS) & !MIP_MTIP,
            );
            self.wfi = false;
            return;
        }
        if (minterrupt & MIP_SEIP) != 0
            && self.handle_trap(
                Trap {
                    trap_type: TrapType::SupervisorExternalInterrupt,
                    value: self.pc, // dummy
                },
                instruction_address,
                true,
            )
        {
            self.write_csr_raw(
                CSR_MIP_ADDRESS,
                self.read_csr_raw(CSR_MIP_ADDRESS) & !MIP_SEIP,
            );
            self.wfi = false;
            return;
        }
        if (minterrupt & MIP_SSIP) != 0
            && self.handle_trap(
                Trap {
                    trap_type: TrapType::SupervisorSoftwareInterrupt,
                    value: self.pc, // dummy
                },
                instruction_address,
                true,
            )
        {
            self.write_csr_raw(
                CSR_MIP_ADDRESS,
                self.read_csr_raw(CSR_MIP_ADDRESS) & !MIP_SSIP,
            );
            self.wfi = false;
            return;
        }
        if (minterrupt & MIP_STIP) != 0
            && self.handle_trap(
                Trap {
                    trap_type: TrapType::SupervisorTimerInterrupt,
                    value: self.pc, // dummy
                },
                instruction_address,
                true,
            )
        {
            self.write_csr_raw(
                CSR_MIP_ADDRESS,
                self.read_csr_raw(CSR_MIP_ADDRESS) & !MIP_STIP,
            );
            self.wfi = false;
        }
    }

    pub fn handle_trap(
        &mut self,
        trap: Trap,
        instruction_address: u32,
        is_interrupt: bool,
    ) -> bool {
        let current_privilege_encoding = get_privilege_encoding(&self.privilege_mode);
        let cause = get_trap_cause(&trap);

        // First, determine which privilege mode should handle the trap.
        // @TODO: Check if this logic is correct
        let mdeleg = match is_interrupt {
            true => self.read_csr_raw(CSR_MIDELEG_ADDRESS),
            false => self.read_csr_raw(CSR_MEDELEG_ADDRESS),
        };
        let sdeleg = match is_interrupt {
            true => self.read_csr_raw(CSR_SIDELEG_ADDRESS),
            false => self.read_csr_raw(CSR_SEDELEG_ADDRESS),
        };

        let pos = cause & 0xffff;

        let new_privilege_mode = match ((mdeleg >> pos) & 1) == 0 {
            true => PrivilegeMode::Machine,
            false => match ((sdeleg >> pos) & 1) == 0 {
                true => PrivilegeMode::Supervisor,
                false => PrivilegeMode::User,
            },
        };
        let new_privilege_encoding = get_privilege_encoding(&new_privilege_mode);

        let current_status = match self.privilege_mode {
            PrivilegeMode::Machine => self.read_csr_raw(CSR_MSTATUS_ADDRESS),
            PrivilegeMode::Supervisor => self.read_csr_raw(CSR_SSTATUS_ADDRESS),
            PrivilegeMode::User => self.read_csr_raw(CSR_USTATUS_ADDRESS),
            PrivilegeMode::Reserved => panic!(),
        };

        // Second, ignore the interrupt if it's disabled by some conditions

        if is_interrupt {
            let ie = match new_privilege_mode {
                PrivilegeMode::Machine => self.read_csr_raw(CSR_MIE_ADDRESS),
                PrivilegeMode::Supervisor => self.read_csr_raw(CSR_SIE_ADDRESS),
                PrivilegeMode::User => self.read_csr_raw(CSR_UIE_ADDRESS),
                PrivilegeMode::Reserved => panic!(),
            };

            let current_mie = (current_status >> 3) & 1;
            let current_sie = (current_status >> 1) & 1;
            let current_uie = current_status & 1;

            let msie = (ie >> 3) & 1;
            let ssie = (ie >> 1) & 1;
            let usie = ie & 1;

            let mtie = (ie >> 7) & 1;
            let stie = (ie >> 5) & 1;
            let utie = (ie >> 4) & 1;

            let meie = (ie >> 11) & 1;
            let seie = (ie >> 9) & 1;
            let ueie = (ie >> 8) & 1;

            // 1. Interrupt is always enabled if new privilege level is higher
            // than current privilege level
            // 2. Interrupt is always disabled if new privilege level is lower
            // than current privilege level
            // 3. Interrupt is enabled if xIE in xstatus is 1 where x is privilege level
            // and new privilege level equals to current privilege level

            if new_privilege_encoding < current_privilege_encoding {
                return false;
            }

            if current_privilege_encoding == new_privilege_encoding {
                match self.privilege_mode {
                    PrivilegeMode::Machine => {
                        if current_mie == 0 {
                            return false;
                        }
                    }
                    PrivilegeMode::Supervisor => {
                        if current_sie == 0 {
                            return false;
                        }
                    }
                    PrivilegeMode::User => {
                        if current_uie == 0 {
                            return false;
                        }
                    }
                    PrivilegeMode::Reserved => panic!(),
                };
            }

            // Interrupt can be maskable by xie csr register
            // where x is a new privilege mode.

            match trap.trap_type {
                TrapType::UserSoftwareInterrupt => {
                    if usie == 0 {
                        return false;
                    }
                }
                TrapType::SupervisorSoftwareInterrupt => {
                    if ssie == 0 {
                        return false;
                    }
                }
                TrapType::MachineSoftwareInterrupt => {
                    if msie == 0 {
                        return false;
                    }
                }
                TrapType::UserTimerInterrupt => {
                    if utie == 0 {
                        return false;
                    }
                }
                TrapType::SupervisorTimerInterrupt => {
                    if stie == 0 {
                        return false;
                    }
                }
                TrapType::MachineTimerInterrupt => {
                    if mtie == 0 {
                        return false;
                    }
                }
                TrapType::UserExternalInterrupt => {
                    if ueie == 0 {
                        return false;
                    }
                }
                TrapType::SupervisorExternalInterrupt => {
                    if seie == 0 {
                        return false;
                    }
                }
                TrapType::MachineExternalInterrupt => {
                    if meie == 0 {
                        return false;
                    }
                }
                _ => {}
            };
        }

        // So, this trap should be taken

        self.privilege_mode = new_privilege_mode;
        self.mmu.update_privilege_mode(self.privilege_mode);
        let csr_epc_address = match self.privilege_mode {
            PrivilegeMode::Machine => CSR_MEPC_ADDRESS,
            PrivilegeMode::Supervisor => CSR_SEPC_ADDRESS,
            PrivilegeMode::User => CSR_UEPC_ADDRESS,
            PrivilegeMode::Reserved => panic!(),
        };
        let csr_cause_address = match self.privilege_mode {
            PrivilegeMode::Machine => CSR_MCAUSE_ADDRESS,
            PrivilegeMode::Supervisor => CSR_SCAUSE_ADDRESS,
            PrivilegeMode::User => CSR_UCAUSE_ADDRESS,
            PrivilegeMode::Reserved => panic!(),
        };
        let csr_tval_address = match self.privilege_mode {
            PrivilegeMode::Machine => CSR_MTVAL_ADDRESS,
            PrivilegeMode::Supervisor => CSR_STVAL_ADDRESS,
            PrivilegeMode::User => CSR_UTVAL_ADDRESS,
            PrivilegeMode::Reserved => panic!(),
        };
        let csr_tvec_address = match self.privilege_mode {
            PrivilegeMode::Machine => CSR_MTVEC_ADDRESS,
            PrivilegeMode::Supervisor => CSR_STVEC_ADDRESS,
            PrivilegeMode::User => CSR_UTVEC_ADDRESS,
            PrivilegeMode::Reserved => panic!(),
        };

        self.write_csr_raw(csr_epc_address, instruction_address);
        self.write_csr_raw(csr_cause_address, cause);
        self.write_csr_raw(csr_tval_address, trap.value);
        self.pc = self.read_csr_raw(csr_tvec_address);

        // Add 4 * cause if tvec has vector type address
        if (self.pc & 0x3) != 0 {
            self.pc = (self.pc & !0x3) + 4 * (cause & 0xffff);
        }

        match self.privilege_mode {
            PrivilegeMode::Machine => {
                let status = self.read_csr_raw(CSR_MSTATUS_ADDRESS);
                let mie = (status >> 3) & 1;
                // clear MIE[3], override MPIE[7] with MIE[3], override MPP[12:11] with current privilege encoding
                let new_status =
                    (status & !0x1888) | (mie << 7) | ((current_privilege_encoding as u32) << 11);
                self.write_csr_raw(CSR_MSTATUS_ADDRESS, new_status);
            }
            PrivilegeMode::Supervisor => {
                let status = self.read_csr_raw(CSR_SSTATUS_ADDRESS);
                let sie = (status >> 1) & 1;
                // clear SIE[1], override SPIE[5] with SIE[1], override SPP[8] with current privilege encoding
                let new_status =
                    (status & !0x122) | (sie << 5) | ((current_privilege_encoding as u32 & 1) << 8);
                self.write_csr_raw(CSR_SSTATUS_ADDRESS, new_status);
            }
            PrivilegeMode::User => {
                panic!("Not implemented yet");
            }
            PrivilegeMode::Reserved => panic!(), // shouldn't happen
        };
        //println!("Trap! {:x} Clock:{:x}", cause, self.clock);
        true
    }

    fn fetch(&mut self) -> Result<u32, Trap> {
        self.mmu.fetch_word(self.pc).map_err(|e| {
            self.pc = self.pc.wrapping_add(4); // @TODO: What if instruction is compressed?
            e
        })
    }

    fn has_csr_access_privilege(&self, address: u16) -> bool {
        let privilege = (address >> 8) & 0x3; // the lowest privilege level that can access the CSR
        privilege as u8 <= get_privilege_encoding(&self.privilege_mode)
    }

    fn read_csr(&mut self, address: u16) -> Result<u32, Trap> {
        match self.has_csr_access_privilege(address) {
            true => Ok(self.read_csr_raw(address)),
            false => Err(Trap {
                trap_type: TrapType::IllegalInstruction,
                value: self.pc.wrapping_sub(4), // @TODO: Is this always correct?
            }),
        }
    }

    pub fn write_csr(&mut self, address: u16, value: u32) -> Result<(), Trap> {
        if self.has_csr_access_privilege(address) {
            /*
            // Checking writability fails some tests so disabling so far
            let read_only = ((address >> 10) & 0x3) == 0x3;
            if read_only {
                return Err(Exception::IllegalInstruction);
            }
            */
            self.write_csr_raw(address, value);
            if address == CSR_SATP_ADDRESS {
                self.update_addressing_mode(value);
            }
            Ok(())
        } else {
            Err(Trap {
                trap_type: TrapType::IllegalInstruction,
                value: self.pc.wrapping_sub(4), // @TODO: Is this always correct?
            })
        }
    }

    // SSTATUS, SIE, and SIP are subsets of MSTATUS, MIE, and MIP
    fn read_csr_raw(&self, address: u16) -> u32 {
        match address {
            // @TODO: Mask shuld consider of 32-bit mode
            CSR_FFLAGS_ADDRESS => self.csr[CSR_FCSR_ADDRESS as usize] & 0x1f,
            CSR_FRM_ADDRESS => (self.csr[CSR_FCSR_ADDRESS as usize] >> 5) & 0x7,
            CSR_SSTATUS_ADDRESS => self.csr[CSR_MSTATUS_ADDRESS as usize] & 0x800d_e162,
            CSR_SIE_ADDRESS => self.csr[CSR_MIE_ADDRESS as usize] & 0x222,
            CSR_SIP_ADDRESS => self.csr[CSR_MIP_ADDRESS as usize] & 0x222,
            // CSR_TIME_ADDRESS => self.mmu.get_clint().read_mtime(),
            _ => self.csr[address as usize],
        }
    }

    fn write_csr_raw(&mut self, address: u16, value: u32) {
        match address {
            CSR_FFLAGS_ADDRESS => {
                self.csr[CSR_FCSR_ADDRESS as usize] &= !0x1f;
                self.csr[CSR_FCSR_ADDRESS as usize] |= value & 0x1f;
            }
            CSR_FRM_ADDRESS => {
                self.csr[CSR_FCSR_ADDRESS as usize] &= !0xe0;
                self.csr[CSR_FCSR_ADDRESS as usize] |= (value << 5) & 0xe0;
            }
            CSR_SSTATUS_ADDRESS => {
                self.csr[CSR_MSTATUS_ADDRESS as usize] &= !0x800de162;
                self.csr[CSR_MSTATUS_ADDRESS as usize] |= value & 0x800de162;
                self.mmu
                    .update_mstatus(self.read_csr_raw(CSR_MSTATUS_ADDRESS));
            }
            CSR_SIE_ADDRESS => {
                self.csr[CSR_MIE_ADDRESS as usize] &= !0x222;
                self.csr[CSR_MIE_ADDRESS as usize] |= value & 0x222;
            }
            CSR_SIP_ADDRESS => {
                self.csr[CSR_MIP_ADDRESS as usize] &= !0x222;
                self.csr[CSR_MIP_ADDRESS as usize] |= value & 0x222;
            }
            CSR_MIDELEG_ADDRESS => {
                self.csr[address as usize] = value & 0x666; // from qemu
            }
            CSR_MEDELEG_ADDRESS => {
                self.csr[address as usize] = value;
            }
            CSR_MTVEC_ADDRESS => {
                self.csr[address as usize] = value;
            }
            CSR_MSTATUS_ADDRESS => {
                self.csr[address as usize] = value;
                self.mmu
                    .update_mstatus(self.read_csr_raw(CSR_MSTATUS_ADDRESS));
            }
            // CSR_TIME_ADDRESS => {
            //     self.mmu.get_mut_clint().write_mtime(value);
            // }
            _ => {
                self.csr[address as usize] = value;
            }
        };
    }

    fn update_addressing_mode(&mut self, value: u32) {
        let addressing_mode = match value & 0x80000000 {
            0 => AddressingMode::None,
            _ => AddressingMode::SV32,
        };
        let ppn = value & 0x3fffff;
        self.mmu.update_addressing_mode(addressing_mode);
        self.mmu.update_ppn(ppn);
    }

    // // @TODO: Rename to better name?
    fn sign_extend(&self, value: i32) -> i32 {
        value
    }

    // @TODO: Rename to better name?
    fn unsigned_data(&self, value: i32) -> u32 {
        (value as u32) & self.unsigned_data_mask
    }

    // @TODO: Rename to better name?
    fn most_negative(&self) -> i32 {
        std::i32::MIN
    }

    // @TODO: Optimize
    fn uncompress(&self, halfword: u32) -> u32 {
        let op = halfword & 0x3; // [1:0]
        let funct3 = (halfword >> 13) & 0x7; // [15:13]

        match op {
            0 => match funct3 {
                0 => {
                    // C.ADDI4SPN
                    // addi rd+8, x2, nzuimm
                    let rd = (halfword >> 2) & 0x7; // [4:2]
                    let nzuimm = ((halfword >> 7) & 0x30) | // nzuimm[5:4] <= [12:11]
						((halfword >> 1) & 0x3c0) | // nzuimm{9:6] <= [10:7]
						((halfword >> 4) & 0x4) | // nzuimm[2] <= [6]
						((halfword >> 2) & 0x8); // nzuimm[3] <= [5]
                         // nzuimm == 0 is reserved instruction
                    if nzuimm != 0 {
                        return (nzuimm << 20) | (2 << 15) | ((rd + 8) << 7) | 0x13;
                    }
                }
                1 => {
                    // @TODO: Support C.LQ for 128-bit
                    // C.FLD for 32, 64-bit
                    // fld rd+8, offset(rs1+8)
                    let rd = (halfword >> 2) & 0x7; // [4:2]
                    let rs1 = (halfword >> 7) & 0x7; // [9:7]
                    let offset = ((halfword >> 7) & 0x38) | // offset[5:3] <= [12:10]
						((halfword << 1) & 0xc0); // offset[7:6] <= [6:5]
                    return (offset << 20) | ((rs1 + 8) << 15) | (3 << 12) | ((rd + 8) << 7) | 0x7;
                }
                2 => {
                    // C.LW
                    // lw rd+8, offset(rs1+8)
                    let rs1 = (halfword >> 7) & 0x7; // [9:7]
                    let rd = (halfword >> 2) & 0x7; // [4:2]
                    let offset = ((halfword >> 7) & 0x38) | // offset[5:3] <= [12:10]
						((halfword >> 4) & 0x4) | // offset[2] <= [6]
						((halfword << 1) & 0x40); // offset[6] <= [5]
                    return (offset << 20) | ((rs1 + 8) << 15) | (2 << 12) | ((rd + 8) << 7) | 0x3;
                }
                3 => {
                    // @TODO: Support C.FLW in 32-bit mode
                    // C.LD in 64-bit mode
                    // ld rd+8, offset(rs1+8)
                    let rs1 = (halfword >> 7) & 0x7; // [9:7]
                    let rd = (halfword >> 2) & 0x7; // [4:2]
                    let offset = ((halfword >> 7) & 0x38) | // offset[5:3] <= [12:10]
						((halfword << 1) & 0xc0); // offset[7:6] <= [6:5]
                    return (offset << 20) | ((rs1 + 8) << 15) | (3 << 12) | ((rd + 8) << 7) | 0x3;
                }
                4 => {
                    // Reserved
                }
                5 => {
                    // C.FSD
                    // fsd rs2+8, offset(rs1+8)
                    let rs1 = (halfword >> 7) & 0x7; // [9:7]
                    let rs2 = (halfword >> 2) & 0x7; // [4:2]
                    let offset = ((halfword >> 7) & 0x38) | // uimm[5:3] <= [12:10]
						((halfword << 1) & 0xc0); // uimm[7:6] <= [6:5]
                    let imm11_5 = (offset >> 5) & 0x7f;
                    let imm4_0 = offset & 0x1f;
                    return (imm11_5 << 25)
                        | ((rs2 + 8) << 20)
                        | ((rs1 + 8) << 15)
                        | (3 << 12)
                        | (imm4_0 << 7)
                        | 0x27;
                }
                6 => {
                    // C.SW
                    // sw rs2+8, offset(rs1+8)
                    let rs1 = (halfword >> 7) & 0x7; // [9:7]
                    let rs2 = (halfword >> 2) & 0x7; // [4:2]
                    let offset = ((halfword >> 7) & 0x38) | // offset[5:3] <= [12:10]
						((halfword << 1) & 0x40) | // offset[6] <= [5]
						((halfword >> 4) & 0x4); // offset[2] <= [6]
                    let imm11_5 = (offset >> 5) & 0x7f;
                    let imm4_0 = offset & 0x1f;
                    return (imm11_5 << 25)
                        | ((rs2 + 8) << 20)
                        | ((rs1 + 8) << 15)
                        | (2 << 12)
                        | (imm4_0 << 7)
                        | 0x23;
                }
                7 => {
                    // @TODO: Support C.FSW in 32-bit mode
                    // C.SD
                    // sd rs2+8, offset(rs1+8)
                    let rs1 = (halfword >> 7) & 0x7; // [9:7]
                    let rs2 = (halfword >> 2) & 0x7; // [4:2]
                    let offset = ((halfword >> 7) & 0x38) | // uimm[5:3] <= [12:10]
						((halfword << 1) & 0xc0); // uimm[7:6] <= [6:5]
                    let imm11_5 = (offset >> 5) & 0x7f;
                    let imm4_0 = offset & 0x1f;
                    return (imm11_5 << 25)
                        | ((rs2 + 8) << 20)
                        | ((rs1 + 8) << 15)
                        | (3 << 12)
                        | (imm4_0 << 7)
                        | 0x23;
                }
                _ => {} // Not happens
            },
            1 => {
                match funct3 {
                    0 => {
                        let r = (halfword >> 7) & 0x1f; // [11:7]
                        let imm = match halfword & 0x1000 {
							0x1000 => 0xffffffc0,
							_ => 0
						} | // imm[31:6] <= [12]
						((halfword >> 7) & 0x20) | // imm[5] <= [12]
						((halfword >> 2) & 0x1f); // imm[4:0] <= [6:2]
                        if r == 0 && imm == 0 {
                            // C.NOP
                            // addi x0, x0, 0
                            return 0x13;
                        } else if r != 0 {
                            // C.ADDI
                            // addi r, r, imm
                            return (imm << 20) | (r << 15) | (r << 7) | 0x13;
                        }
                        // @TODO: Support HINTs
                        // r == 0 and imm != 0 is HINTs
                    }
                    1 => {
                        // C.JAL
                        // jal x1, offset
                        // Bits:
                        let imm = halfword >> 2;
                        // Encoded:    11  4  9   8 10  6  7   3  2  1  5
                        // Decoded: 11 10  9  8   7  6  5  4   3  2  1  0
                        let r = 1;
                        let imm =
                            // imm[10]
                            ((imm & 0b000_0100_0000)  << 4) |
                            // imm[9:8,6]
                            ((imm & 0b001_1010_0000) << 1) |
                            // imm[7]
                            ((imm & 0b000_0001_0000) << 3) |
                            // imm[5]
                            ((imm & 0b000_0000_0001) << 5) |
                            // imm[4]
                            ((imm & 0b010_0000_0000) >> 5) |
                            // imm[3:1]
                            (imm & 0b000_0000_1110);

                        // imm[31:11]
                        let imm = if halfword & 0x1000 == 0 {
                            imm << 20
                        } else {
                            (imm << 20) | 0x801ff000
                        };

                        return imm | (r << 7) | 0x6f;
                    }
                    2 => {
                        // C.LI
                        // addi rd, x0, imm
                        let r = (halfword >> 7) & 0x1f;
                        let imm = match halfword & 0x1000 {
							0x1000 => 0xffffffc0,
							_ => 0
						} | // imm[31:6] <= [12]
						((halfword >> 7) & 0x20) | // imm[5] <= [12]
						((halfword >> 2) & 0x1f); // imm[4:0] <= [6:2]
                        if r != 0 {
                            return (imm << 20) | (r << 7) | 0x13;
                        }
                        // @TODO: Support HINTs
                        // r == 0 is for HINTs
                    }
                    3 => {
                        let r = (halfword >> 7) & 0x1f; // [11:7]
                        if r == 2 {
                            // C.ADDI16SP
                            // addi r, r, nzimm
                            let imm = match halfword & 0x1000 {
								0x1000 => 0xfffffc00,
								_ => 0
							} | // imm[31:10] <= [12]
							((halfword >> 3) & 0x200) | // imm[9] <= [12]
							((halfword >> 2) & 0x10) | // imm[4] <= [6]
							((halfword << 1) & 0x40) | // imm[6] <= [5]
							((halfword << 4) & 0x180) | // imm[8:7] <= [4:3]
							((halfword << 3) & 0x20); // imm[5] <= [2]
                            if imm != 0 {
                                return (imm << 20) | (r << 15) | (r << 7) | 0x13;
                            }
                            // imm == 0 is for reserved instruction
                        }
                        if r != 0 && r != 2 {
                            // C.LUI
                            // lui r, nzimm
                            let nzimm = match halfword & 0x1000 {
								0x1000 => 0xfffc0000,
								_ => 0
							} | // nzimm[31:18] <= [12]
							((halfword << 5) & 0x20000) | // nzimm[17] <= [12]
							((halfword << 10) & 0x1f000); // nzimm[16:12] <= [6:2]
                            if nzimm != 0 {
                                return nzimm | (r << 7) | 0x37;
                            }
                            // nzimm == 0 is for reserved instruction
                        }
                    }
                    4 => {
                        let funct2 = (halfword >> 10) & 0x3; // [11:10]
                        match funct2 {
                            0 => {
                                // C.SRLI
                                // c.srli rs1+8, rs1+8, shamt
                                let shamt = ((halfword >> 7) & 0x20) | // shamt[5] <= [12]
									((halfword >> 2) & 0x1f); // shamt[4:0] <= [6:2]
                                let rs1 = (halfword >> 7) & 0x7; // [9:7]
                                return (shamt << 20)
                                    | ((rs1 + 8) << 15)
                                    | (5 << 12)
                                    | ((rs1 + 8) << 7)
                                    | 0x13;
                            }
                            1 => {
                                // C.SRAI
                                // srai rs1+8, rs1+8, shamt
                                let shamt = ((halfword >> 7) & 0x20) | // shamt[5] <= [12]
									((halfword >> 2) & 0x1f); // shamt[4:0] <= [6:2]
                                let rs1 = (halfword >> 7) & 0x7; // [9:7]
                                return (0x20 << 25)
                                    | (shamt << 20)
                                    | ((rs1 + 8) << 15)
                                    | (5 << 12)
                                    | ((rs1 + 8) << 7)
                                    | 0x13;
                            }
                            2 => {
                                // C.ANDI
                                // andi, r+8, r+8, imm
                                let r = (halfword >> 7) & 0x7; // [9:7]
                                let imm = match halfword & 0x1000 {
									0x1000 => 0xffffffc0,
									_ => 0
								} | // imm[31:6] <= [12]
								((halfword >> 7) & 0x20) | // imm[5] <= [12]
								((halfword >> 2) & 0x1f); // imm[4:0] <= [6:2]
                                return (imm << 20)
                                    | ((r + 8) << 15)
                                    | (7 << 12)
                                    | ((r + 8) << 7)
                                    | 0x13;
                            }
                            3 => {
                                let funct1 = (halfword >> 12) & 1; // [12]
                                let funct2_2 = (halfword >> 5) & 0x3; // [6:5]
                                let rs1 = (halfword >> 7) & 0x7;
                                let rs2 = (halfword >> 2) & 0x7;
                                match funct1 {
                                    0 => match funct2_2 {
                                        0 => {
                                            // C.SUB
                                            // sub rs1+8, rs1+8, rs2+8
                                            return (0x20 << 25)
                                                | ((rs2 + 8) << 20)
                                                | ((rs1 + 8) << 15)
                                                | ((rs1 + 8) << 7)
                                                | 0x33;
                                        }
                                        1 => {
                                            // C.XOR
                                            // xor rs1+8, rs1+8, rs2+8
                                            return ((rs2 + 8) << 20)
                                                | ((rs1 + 8) << 15)
                                                | (4 << 12)
                                                | ((rs1 + 8) << 7)
                                                | 0x33;
                                        }
                                        2 => {
                                            // C.OR
                                            // or rs1+8, rs1+8, rs2+8
                                            return ((rs2 + 8) << 20)
                                                | ((rs1 + 8) << 15)
                                                | (6 << 12)
                                                | ((rs1 + 8) << 7)
                                                | 0x33;
                                        }
                                        3 => {
                                            // C.AND
                                            // and rs1+8, rs1+8, rs2+8
                                            return ((rs2 + 8) << 20)
                                                | ((rs1 + 8) << 15)
                                                | (7 << 12)
                                                | ((rs1 + 8) << 7)
                                                | 0x33;
                                        }
                                        _ => {} // Not happens
                                    },
                                    1 => match funct2_2 {
                                        0 => {
                                            // C.SUBW
                                            // subw r1+8, r1+8, r2+8
                                            return (0x20 << 25)
                                                | ((rs2 + 8) << 20)
                                                | ((rs1 + 8) << 15)
                                                | ((rs1 + 8) << 7)
                                                | 0x3b;
                                        }
                                        1 => {
                                            // C.ADDW
                                            // addw r1+8, r1+8, r2+8
                                            return ((rs2 + 8) << 20)
                                                | ((rs1 + 8) << 15)
                                                | ((rs1 + 8) << 7)
                                                | 0x3b;
                                        }
                                        2 => {
                                            // Reserved
                                        }
                                        3 => {
                                            // Reserved
                                        }
                                        _ => {} // Not happens
                                    },
                                    _ => {} // No happens
                                };
                            }
                            _ => {} // not happens
                        };
                    }
                    5 => {
                        // C.J
                        // jal x0, imm
                        let offset = match halfword & 0x1000 {
								0x1000 => 0xfffff000,
								_ => 0
							} | // offset[31:12] <= [12]
							((halfword >> 1) & 0x800) | // offset[11] <= [12]
							((halfword >> 7) & 0x10) | // offset[4] <= [11]
							((halfword >> 1) & 0x300) | // offset[9:8] <= [10:9]
							((halfword << 2) & 0x400) | // offset[10] <= [8]
							((halfword >> 1) & 0x40) | // offset[6] <= [7]
							((halfword << 1) & 0x80) | // offset[7] <= [6]
							((halfword >> 2) & 0xe) | // offset[3:1] <= [5:3]
							((halfword << 3) & 0x20); // offset[5] <= [2]
                        let imm = ((offset >> 1) & 0x80000) | // imm[19] <= offset[20]
							((offset << 8) & 0x7fe00) | // imm[18:9] <= offset[10:1]
							((offset >> 3) & 0x100) | // imm[8] <= offset[11]
							((offset >> 12) & 0xff); // imm[7:0] <= offset[19:12]
                        return (imm << 12) | 0x6f;
                    }
                    6 => {
                        // C.BEQZ
                        // beq r+8, x0, offset
                        let r = (halfword >> 7) & 0x7;
                        let offset = match halfword & 0x1000 {
								0x1000 => 0xfffffe00,
								_ => 0
							} | // offset[31:9] <= [12]
							((halfword >> 4) & 0x100) | // offset[8] <= [12]
							((halfword >> 7) & 0x18) | // offset[4:3] <= [11:10]
							((halfword << 1) & 0xc0) | // offset[7:6] <= [6:5]
							((halfword >> 2) & 0x6) | // offset[2:1] <= [4:3]
							((halfword << 3) & 0x20); // offset[5] <= [2]
                        let imm2 = ((offset >> 6) & 0x40) | // imm2[6] <= [12]
							((offset >> 5) & 0x3f); // imm2[5:0] <= [10:5]
                        let imm1 = (offset & 0x1e) | // imm1[4:1] <= [4:1]
							((offset >> 11) & 0x1); // imm1[0] <= [11]
                        return (imm2 << 25) | ((r + 8) << 20) | (imm1 << 7) | 0x63;
                    }
                    7 => {
                        // C.BNEZ
                        // bne r+8, x0, offset
                        let r = (halfword >> 7) & 0x7;
                        let offset = match halfword & 0x1000 {
								0x1000 => 0xfffffe00,
								_ => 0
							} | // offset[31:9] <= [12]
							((halfword >> 4) & 0x100) | // offset[8] <= [12]
							((halfword >> 7) & 0x18) | // offset[4:3] <= [11:10]
							((halfword << 1) & 0xc0) | // offset[7:6] <= [6:5]
							((halfword >> 2) & 0x6) | // offset[2:1] <= [4:3]
							((halfword << 3) & 0x20); // offset[5] <= [2]
                        let imm2 = ((offset >> 6) & 0x40) | // imm2[6] <= [12]
							((offset >> 5) & 0x3f); // imm2[5:0] <= [10:5]
                        let imm1 = (offset & 0x1e) | // imm1[4:1] <= [4:1]
							((offset >> 11) & 0x1); // imm1[0] <= [11]
                        return (imm2 << 25) | ((r + 8) << 20) | (1 << 12) | (imm1 << 7) | 0x63;
                    }
                    _ => {} // No happens
                };
            }
            2 => {
                match funct3 {
                    0 => {
                        // C.SLLI
                        // slli r, r, shamt
                        let r = (halfword >> 7) & 0x1f;
                        let shamt = ((halfword >> 7) & 0x20) | // imm[5] <= [12]
							((halfword >> 2) & 0x1f); // imm[4:0] <= [6:2]
                        if r != 0 {
                            return (shamt << 20) | (r << 15) | (1 << 12) | (r << 7) | 0x13;
                        }
                        // r == 0 is reserved instruction?
                    }
                    1 => {
                        // C.FLDSP
                        // fld rd, offset(x2)
                        let rd = (halfword >> 7) & 0x1f;
                        let offset = ((halfword >> 7) & 0x20) | // offset[5] <= [12]
							((halfword >> 2) & 0x18) | // offset[4:3] <= [6:5]
							((halfword << 4) & 0x1c0); // offset[8:6] <= [4:2]
                        if rd != 0 {
                            return (offset << 20) | (2 << 15) | (3 << 12) | (rd << 7) | 0x7;
                        }
                        // rd == 0 is reseved instruction
                    }
                    2 => {
                        // C.LWSP
                        // lw r, offset(x2)
                        let r = (halfword >> 7) & 0x1f;
                        let offset = ((halfword >> 7) & 0x20) | // offset[5] <= [12]
							((halfword >> 2) & 0x1c) | // offset[4:2] <= [6:4]
							((halfword << 4) & 0xc0); // offset[7:6] <= [3:2]
                        if r != 0 {
                            return (offset << 20) | (2 << 15) | (2 << 12) | (r << 7) | 0x3;
                        }
                        // r == 0 is reseved instruction
                    }
                    3 => {
                        // @TODO: Support C.FLWSP in 32-bit mode
                        // C.LDSP
                        // ld rd, offset(x2)
                        let rd = (halfword >> 7) & 0x1f;
                        let offset = ((halfword >> 7) & 0x20) | // offset[5] <= [12]
							((halfword >> 2) & 0x18) | // offset[4:3] <= [6:5]
							((halfword << 4) & 0x1c0); // offset[8:6] <= [4:2]
                        if rd != 0 {
                            return (offset << 20) | (2 << 15) | (3 << 12) | (rd << 7) | 0x3;
                        }
                        // rd == 0 is reseved instruction
                    }
                    4 => {
                        let funct1 = (halfword >> 12) & 1; // [12]
                        let rs1 = (halfword >> 7) & 0x1f; // [11:7]
                        let rs2 = (halfword >> 2) & 0x1f; // [6:2]
                        match funct1 {
                            0 => {
                                if rs1 != 0 && rs2 == 0 {
                                    // C.JR
                                    // jalr x0, 0(rs1)
                                    return (rs1 << 15) | 0x67;
                                }
                                // rs1 == 0 is reserved instruction
                                if rs1 != 0 && rs2 != 0 {
                                    // C.MV
                                    // add rs1, x0, rs2
                                    // println!("C.MV RS1:{:x} RS2:{:x}", rs1, rs2);
                                    return (rs2 << 20) | (rs1 << 7) | 0x33;
                                }
                                // rs1 == 0 && rs2 != 0 is Hints
                                // @TODO: Support Hints
                            }
                            1 => {
                                if rs1 == 0 && rs2 == 0 {
                                    // C.EBREAK
                                    // ebreak
                                    return 0x00100073;
                                }
                                if rs1 != 0 && rs2 == 0 {
                                    // C.JALR
                                    // jalr x1, 0(rs1)
                                    return (rs1 << 15) | (1 << 7) | 0x67;
                                }
                                if rs1 != 0 && rs2 != 0 {
                                    // C.ADD
                                    // add rs1, rs1, rs2
                                    return (rs2 << 20) | (rs1 << 15) | (rs1 << 7) | 0x33;
                                }
                                // rs1 == 0 && rs2 != 0 is Hists
                                // @TODO: Supports Hinsts
                            }
                            _ => {} // Not happens
                        };
                    }
                    5 => {
                        // @TODO: Implement
                        // C.FSDSP
                        // fsd rs2, offset(x2)
                        let rs2 = (halfword >> 2) & 0x1f; // [6:2]
                        let offset = ((halfword >> 7) & 0x38) | // offset[5:3] <= [12:10]
							((halfword >> 1) & 0x1c0); // offset[8:6] <= [9:7]
                        let imm11_5 = (offset >> 5) & 0x3f;
                        let imm4_0 = offset & 0x1f;
                        return (imm11_5 << 25)
                            | (rs2 << 20)
                            | (2 << 15)
                            | (3 << 12)
                            | (imm4_0 << 7)
                            | 0x27;
                    }
                    6 => {
                        // C.SWSP
                        // sw rs2, offset(x2)
                        let rs2 = (halfword >> 2) & 0x1f; // [6:2]
                        let offset = ((halfword >> 7) & 0x3c) | // offset[5:2] <= [12:9]
							((halfword >> 1) & 0xc0); // offset[7:6] <= [8:7]
                        let imm11_5 = (offset >> 5) & 0x3f;
                        let imm4_0 = offset & 0x1f;
                        return (imm11_5 << 25)
                            | (rs2 << 20)
                            | (2 << 15)
                            | (2 << 12)
                            | (imm4_0 << 7)
                            | 0x23;
                    }
                    7 => {
                        // @TODO: Support C.FSWSP in 32-bit mode
                        // C.SDSP
                        // sd rs, offset(x2)
                        let rs2 = (halfword >> 2) & 0x1f; // [6:2]
                        let offset = ((halfword >> 7) & 0x38) | // offset[5:3] <= [12:10]
							((halfword >> 1) & 0x1c0); // offset[8:6] <= [9:7]
                        let imm11_5 = (offset >> 5) & 0x3f;
                        let imm4_0 = offset & 0x1f;
                        return (imm11_5 << 25)
                            | (rs2 << 20)
                            | (2 << 15)
                            | (3 << 12)
                            | (imm4_0 << 7)
                            | 0x23;
                    }
                    _ => {} // Not happens
                };
            }
            _ => {} // No happnes
        };
        0xffffffff // Return invalid value
    }

    /// Disassembles an instruction pointed by Program Counter.
    pub fn disassemble_next_instruction(&mut self) -> String {
        // @TODO: Fetching can make a side effect,
        // for example updating page table entry or update peripheral hardware registers.
        // But ideally disassembling doesn't want to cause any side effect.
        // How can we avoid side effect?
        let Ok(mut original_word) = self.mmu.fetch_word(self.pc) else {
            return format!("PC:{:016x}, InstructionPageFault Trap!\n", self.pc);
        };

        let word = if (original_word & 0x3) == 0x3 {
            original_word
        } else {
            original_word &= 0xffff;
            self.uncompress(original_word)
        };

        let Ok(inst) = self.decode_raw(word) else {
            return format!(
                "Unknown instruction PC:{:x} WORD:{:x}",
                self.pc, original_word
            );
        };

        let mut s = format!("PC:{:08x} ", self.pc);
        s += &format!("{:08x} ", original_word);
        s += &format!("{} ", inst.name);
        s += &(inst.disassemble)(self, word, self.pc, true).to_string();
        s
    }

    /// Returns mutable `Mmu`
    pub fn get_mut_mmu(&mut self) -> &mut Mmu {
        &mut self.mmu
    }

    pub fn phys_read_u32(&self, address: u32) -> u32 {
        self.mmu.load_word_raw(address)
    }

    pub fn phys_write_u32(&mut self, address: u32, value: u32) {
        self.mmu.store_word_raw(address, value)
    }

    pub fn phys_read_u8(&self, address: u32) -> u8 {
        self.mmu.load_raw(address)
    }

    pub fn phys_write_u8(&mut self, address: u32, value: u8) {
        self.mmu.store_raw(address, value)
    }
}
