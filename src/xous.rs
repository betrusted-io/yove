use riscv_cpu::cpu::EventHandler;
mod definitions;

use definitions::{Syscall, SyscallNumber, SyscallResultNumber};

#[derive(Debug)]
pub enum LoadError {
    IncorrectFormat,
    BitSizeError,
    SatpWriteError,
    MstatusWriteError,
    CpuTrap(riscv_cpu::cpu::Trap),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LoadError::IncorrectFormat => write!(f, "Incorrect format"),
            LoadError::BitSizeError => write!(f, "Incorrect bit size"),
            LoadError::SatpWriteError => write!(f, "Couldn't write to SATP register"),
            LoadError::MstatusWriteError => write!(f, "Couldn't write to MSTATUS register"),
            LoadError::CpuTrap(trap) => write!(f, "CPU trap: {:?}", trap),
        }
    }
}

const MMUFLAG_VALID: u32 = 0x01;
const MMUFLAG_READABLE: u32 = 0x02;
const MMUFLAG_WRITABLE: u32 = 0x04;
const MMUFLAG_EXECUTABLE: u32 = 0x8;
const MMUFLAG_USERMODE: u32 = 0x10;
// const MMUFLAG_GLOBAL: u32 = 0x20;
const MMUFLAG_ACCESSED: u32 = 0x40;
const MMUFLAG_DIRTY: u32 = 0x80;

impl std::error::Error for LoadError {}

pub struct XousHandler {
    memory_base: u32,
    allocator_offset: u32,
    satp: u32,
    l1_pt: u32,
    heap_start: u32,
    heap_size: u32,
}

impl XousHandler {
    pub fn new(cpu: &riscv_cpu::Cpu) -> Self {
        let memory_base = cpu.memory_base() as u32;
        // let memory_size = cpu.memory_size();

        Self {
            memory_base,
            l1_pt: memory_base + 4096,
            allocator_offset: 8192,
            satp: ((4096 + memory_base) >> 12) | 0x8000_0000,
            heap_start: 0xa000_0000,
            heap_size: 0,
        }
    }

    fn allocate_page(&mut self) -> u32 {
        let page = self.allocator_offset;
        self.allocator_offset += 4096;
        page + self.memory_base
    }

    fn write_bytes(&mut self, cpu: &mut riscv_cpu::Cpu, data: &[u8], start: u32) {
        for (i, byte) in data.iter().enumerate() {
            let i = i as u32;
            self.ensure_page(cpu, start + i);
            let phys = self.virt_to_phys(cpu, start + i).unwrap();

            cpu.phys_write_u8(phys as u64, *byte);
        }
    }

    #[allow(dead_code)]
    pub fn print_mmu(&self, cpu: &riscv_cpu::Cpu) {
        println!("Memory Map:");
        for vpn1 in (0..4096).step_by(4) {
            let l1_entry = cpu.phys_read_u32(self.l1_pt as u64 + vpn1);
            if l1_entry & MMUFLAG_VALID == 0 {
                continue;
            }
            let superpage_addr = vpn1 as u32 * (1 << 22);
            println!(
                "    {:4} Superpage for {:08x} @ {:08x} (flags: {:?})",
                vpn1,
                superpage_addr,
                (l1_entry >> 10) << 12,
                // MMUFlags::from_bits(l1_entry & 0xff).unwrap()
                l1_entry & 0xff,
            );

            for vpn0 in (0..4096).step_by(4) {
                let l0_entry = cpu.phys_read_u32((((l1_entry >> 10) << 12) as u64) + vpn0 as u64);
                if l0_entry & 0x7 == 0 {
                    continue;
                }
                let page_addr = vpn0 as u32 * (1 << 12);
                println!(
                    "        {:4} {:08x} -> {:08x} (flags: {:?})",
                    vpn0,
                    superpage_addr + page_addr,
                    (l0_entry >> 10) << 12,
                    // MMUFlags::from_bits(l0_entry & 0xff).unwrap()
                    l0_entry & 0xff,
                );
            }
        }
    }

    pub fn virt_to_phys(&self, cpu: &riscv_cpu::Cpu, virt: u32) -> Option<u32> {
        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;
        let offset = virt & ((1 << 12) - 1);

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.
        let l1_pt_entry = cpu.phys_read_u32(self.l1_pt as u64 + vpn1 as u64);

        // If the level 1 pagetable doesn't exist, then this address is invalid
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            return None;
        }
        if l1_pt_entry & (MMUFLAG_EXECUTABLE | MMUFLAG_READABLE | MMUFLAG_WRITABLE) != 0 {
            return None;
        }

        let l0_pt_entry = cpu.phys_read_u32((((l1_pt_entry >> 10) << 12) + vpn0 as u32) as u64);

        // Ensure the entry hasn't already been mapped.
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            return None;
        }
        Some(((l0_pt_entry >> 10) << 12) | offset)
    }

    fn ensure_page(&mut self, cpu: &mut riscv_cpu::Cpu, address: u32) {
        let vpn1 = ((address >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((address >> 12) & ((1 << 10) - 1)) as usize * 4;

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.

        // If the level 1 pagetable doesn't exist, then this address is invalid
        let mut l1_pt_entry = cpu.phys_read_u32(self.l1_pt as u64 + vpn1 as u64);
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            // Allocate a new page for the level 1 pagetable
            let l0_pt_phys = self.allocate_page();
            // println!("Allocating level 0 pagetable at {:08x}", l0_pt_phys);
            l1_pt_entry =
                ((l0_pt_phys >> 12) << 10) | MMUFLAG_VALID | MMUFLAG_DIRTY | MMUFLAG_ACCESSED;
            // Map the level 1 pagetable into the root pagetable
            cpu.phys_write_u32(self.l1_pt as u64 + vpn1 as u64, l1_pt_entry);
        }

        let l0_pt_phys = ((l1_pt_entry >> 10) << 12) + vpn0 as u32;
        let mut l0_pt_entry = cpu.phys_read_u32(l0_pt_phys as u64);

        // Ensure the entry hasn't already been mapped.
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            let page_phys = self.allocate_page();
            l0_pt_entry = ((page_phys >> 12) << 10)
                | MMUFLAG_VALID
                | MMUFLAG_WRITABLE
                | MMUFLAG_READABLE
                | MMUFLAG_EXECUTABLE
                | MMUFLAG_USERMODE
                | MMUFLAG_DIRTY
                | MMUFLAG_ACCESSED;
            // Map the level 0 pagetable into the level 1 pagetable
            cpu.phys_write_u32(l0_pt_phys as u64, l0_pt_entry);
        }
    }

    pub fn load_program_to_cpu(
        &mut self,
        cpu: &mut riscv_cpu::Cpu,
        program: &[u8],
    ) -> Result<(), LoadError> {
        let goblin::Object::Elf(elf) =
            goblin::Object::parse(program).map_err(|_| LoadError::IncorrectFormat)?
        else {
            return Err(LoadError::IncorrectFormat);
        };
        if elf.is_64 {
            return Err(LoadError::BitSizeError);
        }

        for sh in elf.section_headers {
            if sh.sh_flags as u32 & goblin::elf::section_header::SHF_ALLOC == 0 {
                continue;
            }
            if sh.sh_type & goblin::elf::section_header::SHT_NOBITS != 0 {
                for addr in sh.sh_addr..(sh.sh_addr + sh.sh_size) {
                    self.ensure_page(cpu, addr.try_into().unwrap());
                    // self.write_virt_u8(cpu, addr.try_into().unwrap(), 0);
                }
            } else {
                self.write_bytes(
                    cpu,
                    &program[sh.sh_offset as usize..(sh.sh_offset + sh.sh_size) as usize],
                    sh.sh_addr.try_into().unwrap(),
                );
            }
        }

        self.print_mmu(cpu);

        // TODO: Get memory permissions correct

        let satp = self.satp.into();

        // Ensure stack is allocated
        for page in (0xc000_0000..0xc002_0000).step_by(4096) {
            self.ensure_page(cpu, page);
        }

        // for (offset, byte) in shadow_memory.into_iter().enumerate() {
        //     if byte == 0 {
        //         continue;
        //     }
        //     // println!("Writing {:02x} to {:08x}", byte, offset as u64 + memory_base);
        //     cpu.phys_write_u8(offset as u64 + memory_base, byte);
        // }

        cpu.write_csr(riscv_cpu::cpu::CSR_SATP_ADDRESS, satp)
            .map_err(|_| LoadError::SatpWriteError)?;
        cpu.update_pc(elf.entry);

        // Return to User Mode (0 << 11) with interrupts disabled (1 << 5)
        cpu.write_csr(riscv_cpu::cpu::CSR_MSTATUS_ADDRESS, 1 << 5)
            .map_err(|_| LoadError::MstatusWriteError)?;

        cpu.write_csr(riscv_cpu::cpu::CSR_SEPC_ADDRESS, elf.entry)
            .unwrap();

        // SRET to return to user mode
        cpu.execute_opcode(0x10200073).map_err(LoadError::CpuTrap)?;

        // Update the stack pointer
        cpu.write_register(2, 0xc002_0000 - 4);

        Ok(())
    }
}

impl XousHandler {
    fn syscall(&mut self, cpu: &mut riscv_cpu::Cpu, syscall: Syscall) -> [i64; 8] {
        match syscall {
            Syscall::IncreaseHeap(bytes, flags) => {
                let heap_address = self.heap_start + self.heap_size;
                if bytes < 0 {
                    self.heap_size -= bytes.abs() as u32;
                    panic!("Reducing size not supported!");
                } else if bytes > 0 {
                    for new_address in (heap_address..(heap_address + bytes as u32)).step_by(4096) {
                        self.ensure_page(cpu, new_address);
                    }
                    self.heap_size += bytes as u32;
                }
                [
                    SyscallResultNumber::MemoryRange as i64,
                    heap_address as i64,
                    bytes,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]
            }
            Syscall::Unknown(args) => {
                println!("Unknown syscall {:?}: {:?}", SyscallNumber::from(args[0]), args);
                [SyscallResultNumber::Unimplemented as _, 0, 0, 0, 0, 0, 0, 0]
            }
        }
    }
}

impl EventHandler for XousHandler {
    fn handle_event(&mut self, cpu: &mut riscv_cpu::Cpu, args: [i64; 8]) -> [i64; 8] {
        let syscall: Syscall = args.into();
        println!("Syscall {:?} with args: {:?}", syscall, &args[1..]);
        self.syscall(cpu, syscall)
    }
}
