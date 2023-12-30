use std::io::Read;

use xous::XousHandler;

mod xous;

// #[derive(Debug)]
// enum LoadError {
//     IncorrectFormat,
//     BitSizeError,
//     SatpWriteError,
//     MstatusWriteError,
//     CpuTrap(riscv_cpu::cpu::Trap),
// }

// impl std::fmt::Display for LoadError {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         match self {
//             LoadError::IncorrectFormat => write!(f, "Incorrect format"),
//             LoadError::BitSizeError => write!(f, "Incorrect bit size"),
//             LoadError::SatpWriteError => write!(f, "Couldn't write to SATP register"),
//             LoadError::MstatusWriteError => write!(f, "Couldn't write to MSTATUS register"),
//             LoadError::CpuTrap(trap) => write!(f, "CPU trap: {:?}", trap),
//         }
//     }
// }

// impl std::error::Error for LoadError {}

// struct MemoryManager32<'a> {
//     memory: &'a mut [u8],
//     base: u32,
//     allocator_offset: u32,
//     satp: u32,
//     l1_pt: u32,
// }

// const MMUFLAG_VALID: u32 = 0x01;
// pub const MMUFLAG_READABLE: u32 = 0x02;
// pub const MMUFLAG_WRITABLE: u32 = 0x04;
// pub const MMUFLAG_EXECUTABLE: u32 = 0x8;
// pub const MMUFLAG_U: u32 = 0x10;
// pub const MMUFLAG_ACCESSED: u32 = 0x40;
// pub const MMUFLAG_DIRTY: u32 = 0x80;

// impl<'a> MemoryManager32<'a> {
//     fn new(memory: &'a mut [u8], base: u32) -> Self {
//         // Allocate a single process. Place the root page table at
//         // the second block of memory.
//         Self {
//             memory,
//             base,
//             allocator_offset: 8192,
//             l1_pt: base + 4096,
//             satp: ((4096 + base) >> 12) | 0x8000_0000,
//         }
//     }

//     fn allocate_page(&mut self) -> u32 {
//         let page = self.allocator_offset;
//         self.allocator_offset += 4096;
//         page + self.base
//     }

//     pub fn virt_to_phys(&self, virt: u32) -> Option<u32> {
//         let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
//         let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;
//         let offset = virt & ((1 << 12) - 1);

//         // The root (l1) pagetable is defined to be mapped into our virtual
//         // address space at this address.
//         let l1_pt = &self.memory[(self.l1_pt - self.base).try_into().unwrap()..];

//         // If the level 1 pagetable doesn't exist, then this address is invalid
//         let l1_pt_entry = u32::from_le_bytes(l1_pt[vpn1..vpn1 + 4].try_into().unwrap());
//         if l1_pt_entry & MMUFLAG_VALID == 0 {
//             return None;
//         }
//         if l1_pt_entry & (MMUFLAG_EXECUTABLE | MMUFLAG_READABLE | MMUFLAG_WRITABLE) != 0 {
//             return None;
//         }

//         let l0_pt = &self.memory[(((l1_pt_entry >> 10) << 12) - self.base)
//             .try_into()
//             .unwrap()..];
//         let l0_pt_entry = u32::from_le_bytes(l0_pt[vpn0..vpn0 + 4].try_into().unwrap());

//         // Ensure the entry hasn't already been mapped.
//         if l0_pt_entry & MMUFLAG_VALID == 0 {
//             return None;
//         }
//         Some(((l0_pt_entry >> 10) << 12) | offset)
//     }

//     fn read_phys_u32(&self, address: u32) -> u32 {
//         assert!(address >= self.base && address <= self.base + self.memory.len() as u32);
//         u32::from_le_bytes(
//             self.memory[(address - self.base).try_into().unwrap()
//                 ..(address - self.base + 4).try_into().unwrap()]
//                 .try_into()
//                 .unwrap(),
//         )
//     }

//     fn write_phys_u32(&mut self, address: u32, value: u32) {
//         assert!(address >= self.base && address <= self.base + self.memory.len() as u32);
//         for (src, dest) in value
//             .to_le_bytes()
//             .iter()
//             .zip(self.memory[(address - self.base).try_into().unwrap()..].iter_mut())
//         {
//             *dest = *src;
//         }
//     }

//     fn read_virt_u32(&self, address: u32) -> u32 {
//         self.read_phys_u32(self.virt_to_phys(address).unwrap())
//     }

//     fn write_virt_u8(&mut self, address: u32, value: u8) {
//         let phys: usize = (self.virt_to_phys(address).unwrap() - self.base)
//             .try_into()
//             .unwrap();
//         self.memory[phys] = value;
//     }

//     fn is_allocated(&self, address: u32) -> bool {
//         self.virt_to_phys(address).is_some()
//     }

//     /// Allocate a brand-new memory mapping. When this memory mapping is created,
//     /// it will be ready to use in a new process, however it will have no actual
//     /// program code. It will, however, have the following pages mapped:
//     ///
//     ///     1. The kernel will be mapped to superpage 1023, meaning the kernel can
//     ///        switch to this process and do things.
//     ///     2. A page will be allocated for superpage 1022, to contain pages for
//     ///        process-specific code.
//     ///     3. A page will be allocated for superpage 1021, to contain pages for
//     ///        managing pages.
//     ///     4. The root pagetable will be allocated and mapped at 0xff800000,
//     ///        ensuring new superpages can be allocated.
//     ///     5. A context page will be allocated at 0xff801000, ensuring the
//     ///        process can actually be run.
//     ///     6. Individual pagetable mappings are mapped at 0xff400000
//     /// At the end of this operation, the following mapping will take place. Note that
//     /// names are repeated in the chart below to indicate they are the same page
//     /// represented multiple times. Items in brackets are offsets (in `usize`-words)
//     /// from the start of the page. For example, offset 1023 on the root pagetable
//     /// (address 4092) contains an entry that points to the kernel superpage.
//     ///                         +----------------+
//     ///                         | Root Pagetable |
//     ///                         |      root      |
//     ///                         +----------------+
//     ///                                  |
//     ///                  +---------------+-------------------+------------------+
//     ///                  |                                   |                  |
//     ///               [1021]                              [1022]             [1023]
//     ///                  v                                   v                  v
//     ///          +--------------+                    +--------------+       +--------+
//     ///          | Level 0/1021 |                    | Level 0/1022 |       | Kernel |
//     ///          |   pages_l0   |                    |  process_l0  |       |        |
//     ///          +--------------+                    +--------------+       +--------+
//     ///                  |                                   |
//     ///          +-------+---------+                     +---+-----------+
//     ///          |                 |                     |               |
//     ///       [1021]            [1022]                  [0]             [1]
//     ///          v                 v                     v               v
//     ///  +--------------+  +--------------+     +----------------+  +---------+
//     ///  | Level 0/1021 |  | Level 0/1022 |     | Root Pagetable |  | Context |
//     ///  +--------------+  +--------------+     +----------------+  +---------+
//     fn ensure_page(&mut self, virt: u32) {
//         let vpn1 = (virt >> 22) & ((1 << 10) - 1);
//         let vpn0 = (virt >> 12) & ((1 << 10) - 1);

//         // println!("Ensuring page {:08x} exists", virt);

//         // Ensure there's a level 0 pagetable
//         let mut l1_pt_entry = self.read_phys_u32(self.l1_pt + vpn1 * 4);
//         if l1_pt_entry & MMUFLAG_VALID == 0 {
//             // Allocate a new page for the level 1 pagetable
//             let l0_pt_phys = self.allocate_page();
//             // println!("Allocating level 0 pagetable at {:08x}", l0_pt_phys);
//             l1_pt_entry =
//                 ((l0_pt_phys >> 12) << 10) | MMUFLAG_VALID | MMUFLAG_DIRTY | MMUFLAG_ACCESSED;
//             // Map the level 1 pagetable into the root pagetable
//             self.write_phys_u32(self.l1_pt + vpn1 * 4, l1_pt_entry);
//         }

//         let l0_pt_phys = l1_pt_entry >> 10 << 12;
//         // println!(
//         //     "Level 0 pagetable at {:08x} (l1_pt_entry: {:08x})",
//         //     l0_pt_phys, l1_pt_entry
//         // );

//         // Ensure the page is mapped
//         let mut l0_pt_entry = self.read_phys_u32(l0_pt_phys + vpn0 * 4);
//         if l0_pt_entry & MMUFLAG_VALID == 0 {
//             // Allocate a new page for the level 0 pagetable
//             let page_phys = self.allocate_page();
//             // println!("Allocating physical page at {:08x}", page_phys);
//             l0_pt_entry = ((page_phys >> 12) << 10)
//                 | MMUFLAG_VALID
//                 | MMUFLAG_WRITABLE
//                 | MMUFLAG_READABLE
//                 | MMUFLAG_EXECUTABLE
//                 | MMUFLAG_DIRTY
//                 | MMUFLAG_ACCESSED;
//             // Map the level 0 pagetable into the level 1 pagetable
//             self.write_phys_u32(l0_pt_phys + vpn0 * 4, l0_pt_entry);
//         }
//     }

//     fn write_bytes(&mut self, data: &[u8], start: u32) {
//         for (i, byte) in data.iter().enumerate() {
//             let i = i as u32;
//             // println!("Map: {}", self);
//             self.ensure_page(start + i);
//             // println!("Writing byte to {:08x}...", start + i);
//             // println!("Map: {}", self);
//             if start + i == 0x258062 {
//                 println!("Writing {:02x} to {:08x}", byte, start + i);
//             }

//             self.write_virt_u8(start + i, *byte);
//         }
//     }
// }

// impl core::fmt::Display for MemoryManager32<'_> {
//     fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
//         writeln!(f, "Memory Maps:")?;
//         let l1_pt = &self.memory[(self.l1_pt - self.base).try_into().unwrap()..];
//         for (i, l1_entry) in l1_pt[0..4096].chunks(4).enumerate() {
//             let l1_entry = u32::from_le_bytes(l1_entry.try_into().unwrap());
//             if l1_entry == 0 {
//                 continue;
//             }
//             let superpage_addr = i as u32 * (1 << 22);
//             writeln!(
//                 f,
//                 "    {:4} Superpage for {:08x} @ {:08x} (flags: {:?})",
//                 i,
//                 superpage_addr,
//                 (l1_entry >> 10) << 12,
//                 // MMUFlags::from_bits(l1_entry & 0xff).unwrap()
//                 l1_entry & 0xff,
//             )?;

//             let l0_pt = &self.memory[(((l1_entry >> 10) << 12) - self.base).try_into().unwrap()..];
//             for (j, l0_entry) in l0_pt[0..4096].chunks(4).enumerate() {
//                 let l0_entry = u32::from_le_bytes(l0_entry.try_into().unwrap());
//                 if l0_entry & 0x7 == 0 {
//                     continue;
//                 }
//                 let page_addr = j as u32 * (1 << 12);
//                 writeln!(
//                     f,
//                     "        {:4} {:08x} -> {:08x} (flags: {:?})",
//                     j,
//                     superpage_addr + page_addr,
//                     (l0_entry >> 10) << 12,
//                     // MMUFlags::from_bits(l0_entry & 0xff).unwrap()
//                     l0_entry & 0xff,
//                 )?;
//             }
//         }
//         Ok(())
//     }
// }

// fn load_program_to_cpu(cpu: &mut riscv_cpu::Cpu, program: &[u8]) -> Result<(), LoadError> {
//     let memory_base = cpu.memory_base();
//     let memory_size = cpu.memory_size();

//     let goblin::Object::Elf(elf) =
//         goblin::Object::parse(program).map_err(|_| LoadError::IncorrectFormat)?
//     else {
//         return Err(LoadError::IncorrectFormat);
//     };
//     if elf.is_64 {
//         return Err(LoadError::BitSizeError);
//     }

//     let mut shadow_memory = vec![0; memory_size as usize];
//     let mut mm = MemoryManager32::new(&mut shadow_memory, memory_base as u32);

//     for sh in elf.section_headers {
//         if sh.sh_flags as u32 & goblin::elf::section_header::SHF_ALLOC == 0 {
//             // println!(
//             //     "Skipping section {}",
//             //     elf.shdr_strtab
//             //         .get_at(sh.sh_name)
//             //         .unwrap_or("???unknown???")
//             // );
//             continue;
//         }
//         // println!(
//         //     "Section {}: Loading {} bytes at {:x}",
//         //     elf.shdr_strtab
//         //         .get_at(sh.sh_name)
//         //         .unwrap_or("???unknown???"),
//         //     sh.sh_size,
//         //     sh.sh_offset
//         // );
//         if sh.sh_type & goblin::elf::section_header::SHT_NOBITS != 0 {
//             for addr in sh.sh_addr..(sh.sh_addr + sh.sh_size) {
//                 mm.ensure_page(addr.try_into().unwrap());
//                 mm.write_virt_u8(addr.try_into().unwrap(), 0);
//             }
//         } else {
//             mm.write_bytes(
//                 &program[sh.sh_offset as usize..(sh.sh_offset + sh.sh_size) as usize],
//                 sh.sh_addr.try_into().unwrap(),
//             );
//         }
//     }

//     // TODO: Get memory permissions correct

//     let satp = mm.satp.into();

//     // Ensure stack is allocated
//     for page in (0xc000_0000..0xc002_0000).step_by(4096) {
//         mm.ensure_page(page);
//     }

//     for (offset, byte) in shadow_memory.into_iter().enumerate() {
//         if byte == 0 {
//             continue;
//         }
//         // println!("Writing {:02x} to {:08x}", byte, offset as u64 + memory_base);
//         cpu.phys_write_u8(offset as u64 + memory_base, byte);
//     }

//     cpu.write_csr(riscv_cpu::cpu::CSR_SATP_ADDRESS, satp)
//         .map_err(|_| LoadError::SatpWriteError)?;
//     cpu.update_pc(elf.entry);

//     // Return to User Mode (0 << 11) with interrupts disabled (1 << 5)
//     cpu.write_csr(riscv_cpu::cpu::CSR_MSTATUS_ADDRESS, 1 << 5)
//         .map_err(|_| LoadError::MstatusWriteError)?;

//     cpu.write_csr(riscv_cpu::cpu::CSR_SEPC_ADDRESS, elf.entry)
//         .unwrap();

//     // SRET to return to user mode
//     cpu.execute_opcode(0x10200073).map_err(LoadError::CpuTrap)?;

//     // Update the stack pointer
//     cpu.write_register(2, 0xc002_0000 - 4);

//     Ok(())
// }

fn main() {
    let mut std_tests = Vec::new();
    std::fs::File::open("std-tests")
        .expect("couldn't open std-tests")
        .read_to_end(&mut std_tests)
        .expect("couldn't read std-tests");

    let mut cpu = riscv_cpu::CpuBuilder::new()
        .memory_size(16 * 1024 * 1024)
        .xlen(riscv_cpu::Xlen::Bit32)
        .build();

    let mut xous = XousHandler::new(&cpu);
    xous.load_program_to_cpu(&mut cpu, &std_tests)
        .expect("couldn't load std-tests");

    cpu.set_handler(Some(Box::new(xous)));

    for tick in 0..1000 {
        cpu.tick();
    }
}
