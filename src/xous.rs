use riscv_cpu::cpu::Memory as OtherMemory;
mod definitions;

use definitions::{Syscall, SyscallNumber, SyscallResultNumber};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{
        mpsc::{Receiver, Sender},
        Arc, RwLock,
    },
};

const MEMORY_BASE: u32 = 0x8000_0000;

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

struct Memory {
    base: u32,
    data: HashMap<usize, [u8; 4096]>,
    allocated_pages: BTreeSet<usize>,
    free_pages: BTreeSet<usize>,
    heap_start: u32,
    heap_size: u32,
    allocation_start: u32,
    allocation_previous: u32,
    l1_pt: u32,
    satp: u32,
}

enum WorkerCommand {
    Start,
    MemoryRange(u32 /* address */, u32 /* size */),
}

enum WorkerResponse {
    Started,
    Exited(u32),
    AllocateMemory(
        u32, /* phys */
        u32, /* virt */
        u32, /* size */
        u32, /* flags */
    ),
}

struct Worker {
    cpu: riscv_cpu::Cpu,
    tx: Sender<WorkerResponse>,
    rx: Receiver<WorkerCommand>,
}

impl Worker {
    fn new(
        cpu: riscv_cpu::Cpu,
        rx: Receiver<WorkerCommand>,
        worker_response_tx: Sender<WorkerResponse>,
    ) -> Self {
        Self {
            cpu,
            tx: worker_response_tx,
            rx,
        }
    }
    fn run(&mut self) {
        self.rx.recv().unwrap();
        for _tick in 0..1000 {
            self.cpu.tick();
        }
        self.tx.send(WorkerResponse::Exited(1)).unwrap();
    }
}

struct WorkerHandle {
    tx: Sender<WorkerCommand>,
}

impl Memory {
    pub fn new(base: u32, size: usize) -> Self {
        let mut data = HashMap::new();
        let mut free_pages = BTreeSet::new();
        let mut allocated_pages = BTreeSet::new();
        for page in (base..(base + size as u32)).step_by(4096) {
            data.insert(page as usize, [0; 4096]);
            free_pages.insert(page as usize);
        }
        // Remove the l0 page table
        free_pages.remove(&(MEMORY_BASE as usize + 4096));
        allocated_pages.insert(MEMORY_BASE as usize + 4096);
        Self {
            base,
            data,
            allocated_pages,
            free_pages,
            l1_pt: MEMORY_BASE + 4096,
            satp: ((4096 + MEMORY_BASE) >> 12) | 0x8000_0000,
            heap_start: 0x6000_0000,
            heap_size: 0,
            allocation_previous: 0x4000_0000,
            allocation_start: 0x4000_0000,
        }
    }

    fn allocate_page(&mut self) -> u32 {
        let page = self.free_pages.pop_first().expect("out of memory");
        self.allocated_pages.insert(page);
        page as u32
    }

    fn allocate_virt_region(&mut self, size: usize) -> Option<u32> {
        let mut start = self.allocation_previous;
        // Find a free region that will fit this page.
        'outer: loop {
            for page in (start..(start + size as u32)).step_by(4096) {
                if self.virt_to_phys(page).is_some() {
                    start = page + 4096;
                    continue 'outer;
                }
            }
            break;
        }
        // Allocate the region
        for page in (start..(start + size as u32)).step_by(4096) {
            self.ensure_page(page);
        }
        self.allocation_previous = start + size as u32 + 4096;
        Some(start)
    }

    fn ensure_page(&mut self, address: u32) {
        let vpn1 = ((address >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((address >> 12) & ((1 << 10) - 1)) as usize * 4;

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.

        // If the level 1 pagetable doesn't exist, then this address is invalid
        let mut l1_pt_entry = self.read_u32(self.l1_pt as u64 + vpn1 as u64);
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            // Allocate a new page for the level 1 pagetable
            let l0_pt_phys = self.allocate_page();
            // println!("Allocating level 0 pagetable at {:08x}", l0_pt_phys);
            l1_pt_entry =
                ((l0_pt_phys >> 12) << 10) | MMUFLAG_VALID | MMUFLAG_DIRTY | MMUFLAG_ACCESSED;
            // Map the level 1 pagetable into the root pagetable
            self.write_u32(self.l1_pt as u64 + vpn1 as u64, l1_pt_entry);
        }

        let l0_pt_phys = ((l1_pt_entry >> 10) << 12) + vpn0 as u32;
        let mut l0_pt_entry = self.read_u32(l0_pt_phys as u64);

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
            self.write_u32(l0_pt_phys as u64, l0_pt_entry);
        }
    }

    fn write_bytes(&mut self, data: &[u8], start: u32) {
        for (i, byte) in data.iter().enumerate() {
            let i = i as u32;
            self.ensure_page(start + i);
            let phys = self.virt_to_phys(start + i).unwrap();

            self.write_u8(phys as u64, *byte);
        }
    }

    #[allow(dead_code)]
    pub fn print_mmu(&self) {
        println!("Memory Map:");
        for vpn1 in (0..4096).step_by(4) {
            let l1_entry = self.read_u32(self.l1_pt as u64 + vpn1);
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
                let l0_entry = self.read_u32((((l1_entry >> 10) << 12) as u64) + vpn0 as u64);
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

    pub fn virt_to_phys(&self, virt: u32) -> Option<u32> {
        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;
        let offset = virt & ((1 << 12) - 1);

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.
        let l1_pt_entry = self.read_u32(self.l1_pt as u64 + vpn1 as u64);

        // If the level 1 pagetable doesn't exist, then this address is invalid
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            return None;
        }
        if l1_pt_entry & (MMUFLAG_EXECUTABLE | MMUFLAG_READABLE | MMUFLAG_WRITABLE) != 0 {
            return None;
        }

        let l0_pt_entry = self.read_u32((((l1_pt_entry >> 10) << 12) + vpn0 as u32) as u64);

        // Ensure the entry hasn't already been mapped.
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            return None;
        }
        Some(((l0_pt_entry >> 10) << 12) | offset)
    }
}

impl riscv_cpu::cpu::Memory for Memory {
    fn read_u8(&self, address: u64) -> u8 {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        self.data.get(&page).map(|page| page[offset]).unwrap_or(0)
    }

    fn read_u16(&self, address: u64) -> u16 {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        self.data
            .get(&page)
            .map(|page| u16::from_le_bytes([page[offset], page[offset + 1]]))
            .unwrap_or(0)
    }

    fn read_u32(&self, address: u64) -> u32 {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        self.data
            .get(&page)
            .map(|page| {
                u32::from_le_bytes([
                    page[offset],
                    page[offset + 1],
                    page[offset + 2],
                    page[offset + 3],
                ])
            })
            .unwrap_or(0)
    }

    fn read_u64(&self, address: u64) -> u64 {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        self.data
            .get(&page)
            .map(|page| {
                u64::from_le_bytes([
                    page[offset],
                    page[offset + 1],
                    page[offset + 2],
                    page[offset + 3],
                    page[offset + 4],
                    page[offset + 5],
                    page[offset + 6],
                    page[offset + 7],
                ])
            })
            .unwrap_or(0)
    }

    fn write_u8(&mut self, address: u64, value: u8) {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        if let Some(page) = self.data.get_mut(&page) {
            page[offset] = value;
        }
    }

    fn write_u16(&mut self, address: u64, value: u16) {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        if let Some(page) = self.data.get_mut(&page) {
            let bytes = value.to_le_bytes();
            page[offset] = bytes[0];
            page[offset + 1] = bytes[1];
        }
    }

    fn write_u32(&mut self, address: u64, value: u32) {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        if let Some(page) = self.data.get_mut(&page) {
            let bytes = value.to_le_bytes();
            page[offset] = bytes[0];
            page[offset + 1] = bytes[1];
            page[offset + 2] = bytes[2];
            page[offset + 3] = bytes[3];
        }
    }

    fn write_u64(&mut self, address: u64, value: u64) {
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        if let Some(page) = self.data.get_mut(&page) {
            let bytes = value.to_le_bytes();
            page[offset] = bytes[0];
            page[offset + 1] = bytes[1];
            page[offset + 2] = bytes[2];
            page[offset + 3] = bytes[3];
            page[offset + 4] = bytes[4];
            page[offset + 5] = bytes[5];
            page[offset + 6] = bytes[6];
            page[offset + 7] = bytes[7];
        }
    }

    fn validate_address(&self, address: u64) -> bool {
        if address < self.base as u64 {
            return false;
        }
        let address = address as usize - self.base as usize;
        address < self.data.len()
    }

    fn syscall(&mut self, args: [i64; 8]) -> [i64; 8] {
        let syscall: Syscall = args.into();
        println!("Syscall {:?} with args: {:?}", syscall, &args[1..]);

        print!("Syscall: ");
        match syscall {
            Syscall::IncreaseHeap(bytes, _flags) => {
                println!("IncreaseHeap({} bytes, flags: {:02x})", bytes, _flags);
                let heap_start = self.heap_start;
                let heap_address = self.heap_start + self.heap_size;
                match bytes {
                    bytes if bytes < 0 => {
                        self.heap_size -= bytes.unsigned_abs() as u32;
                        panic!("Reducing size not supported!");
                    }
                    bytes if bytes > 0 => {
                        for new_address in
                            (heap_address..(heap_address + bytes as u32)).step_by(4096)
                        {
                            self.ensure_page(new_address);
                        }
                        self.heap_size += bytes as u32;
                    }
                    _ => {}
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

            Syscall::MapMemory(phys, virt, size, _flags) => {
                if virt != 0 {
                    unimplemented!("Non-zero virt address");
                }
                if phys != 0 {
                    unimplemented!("Non-zero phys address");
                }
                let region = self
                    .allocate_virt_region(size as usize)
                    .expect("out of memory");
                [
                    SyscallResultNumber::MemoryRange as i64,
                    region as i64,
                    size,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]
            }
            Syscall::Unknown(args) => {
                println!(
                    "Unhandled {:?}: {:?}",
                    SyscallNumber::from(args[0]),
                    &args[1..]
                );
                [SyscallResultNumber::Unimplemented as _, 0, 0, 0, 0, 0, 0, 0]
            }
        }
    }
}

pub struct Machine {
    memory: Arc<RwLock<Memory>>,
    workers: Vec<WorkerHandle>,
    worker_response: Receiver<WorkerResponse>,
    worker_response_tx: Sender<WorkerResponse>,
}

impl Machine {
    pub fn new(program: &[u8]) -> Result<Self, LoadError> {
        let memory = Arc::new(RwLock::new(Memory::new(MEMORY_BASE, 16 * 1024 * 1024)));

        let (worker_response_tx, worker_response) = std::sync::mpsc::channel();
        let mut machine = Self {
            memory,
            workers: vec![],
            worker_response_tx,
            worker_response,
        };

        machine.load_program(program)?;

        Ok(machine)
    }

    pub fn load_program(&mut self, program: &[u8]) -> Result<(), LoadError> {
        let mut cpu = riscv_cpu::CpuBuilder::new(self.memory.clone())
            .xlen(riscv_cpu::Xlen::Bit32)
            .build();

        let goblin::Object::Elf(elf) =
            goblin::Object::parse(program).map_err(|_| LoadError::IncorrectFormat)?
        else {
            return Err(LoadError::IncorrectFormat);
        };
        if elf.is_64 {
            return Err(LoadError::BitSizeError);
        }

        let mut memory_writer = self.memory.write().unwrap();
        for sh in elf.section_headers {
            if sh.sh_flags as u32 & goblin::elf::section_header::SHF_ALLOC == 0 {
                continue;
            }
            if sh.sh_type & goblin::elf::section_header::SHT_NOBITS != 0 {
                for addr in sh.sh_addr..(sh.sh_addr + sh.sh_size) {
                    memory_writer.ensure_page(addr.try_into().unwrap());
                }
            } else {
                memory_writer.write_bytes(
                    &program[sh.sh_offset as usize..(sh.sh_offset + sh.sh_size) as usize],
                    sh.sh_addr.try_into().unwrap(),
                );
            }
        }

        memory_writer.print_mmu();

        // TODO: Get memory permissions correct

        let satp = memory_writer.satp.into();

        // Ensure stack is allocated
        for page in (0xc000_0000..0xc002_0000).step_by(4096) {
            memory_writer.ensure_page(page);
        }

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

        let (tx, rx) = std::sync::mpsc::channel();
        let worker_tx = self.worker_response_tx.clone();
        let mem = self.memory.clone();
        std::thread::spawn(move || Worker::new(cpu, rx, worker_tx).run());

        self.workers.push(WorkerHandle { tx });

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.workers[0].tx.send(WorkerCommand::Start)?;
        self.worker_response.recv().unwrap();

        Ok(())
    }
}

// impl SyscallHandler for Worker {
//     fn syscall(&mut self, cpu: &mut riscv_cpu::Cpu, args: [i64; 8]) -> [i64; 8] {
//         let syscall: Syscall = args.into();
//         println!("Syscall {:?} with args: {:?}", syscall, &args[1..]);
//         // self.syscall(cpu, syscall)
//         [
//             SyscallResultNumber::Unimplemented as i64,
//             0,
//             0,
//             0,
//             0,
//             0,
//             0,
//             0,
//         ]
//     }
// }
