use riscv_cpu::{cpu::Memory as OtherMemory, mmu::SyscallResult};
mod definitions;
mod services;

use definitions::{Syscall, SyscallNumber, SyscallResultNumber};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{
        atomic::{AtomicI64, Ordering},
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

enum MemoryCommand {
    Exit,
    ExitThread(u32 /* tid */, u32 /* result */),
    CreateThread(
        u32,         /* entry point */
        u32,         /* stack pointer */
        u32,         /* stack length */
        u32,         /* argument 1 */
        u32,         /* argument 2 */
        u32,         /* argument 3 */
        u32,         /* argument 4 */
        Sender<i64>, /* Thread ID */
    ),
    JoinThread(u32, Sender<([i64; 8], Option<(Vec<u8>, u64)>)>),
}

struct Memory {
    base: u32,
    data: HashMap<usize, [u8; 4096]>,
    allocated_pages: BTreeSet<usize>,
    free_pages: BTreeSet<usize>,
    heap_start: u32,
    heap_size: u32,
    // allocation_start: u32,
    allocation_previous: u32,
    l1_pt: u32,
    satp: u32,
    connections: HashMap<u32, Box<dyn services::Service + Send + Sync>>,
    memory_cmd: Sender<MemoryCommand>,
    turbo: bool,
}

struct Worker {
    cpu: riscv_cpu::Cpu,
    cmd: Sender<MemoryCommand>,
    tid: i64,
    memory: Arc<RwLock<Memory>>,
}

impl Worker {
    fn new(
        cpu: riscv_cpu::Cpu,
        cmd: Sender<MemoryCommand>,
        tid: i64,
        memory: Arc<RwLock<Memory>>,
    ) -> Self {
        Self {
            cpu,
            cmd,
            tid,
            memory,
        }
    }
    fn run(&mut self) {
        use riscv_cpu::cpu::TickResult;
        // println!("Running CPU thread {}", self.tid);
        loop {
            match self.cpu.tick() {
                TickResult::PauseEmulation(e) => {
                    let (result, data) = e.recv().unwrap();
                    if let Some(data) = data {
                        let start = data.1;
                        let data = data.0;
                        let mmu = self.cpu.get_mut_mmu();
                        for (offset, byte) in data.into_iter().enumerate() {
                            mmu.store(offset as u64 + start, byte).unwrap();
                        }
                    }
                    for (index, value) in result.iter().enumerate() {
                        self.cpu.write_register(10 + index as u8, *value);
                    }
                }
                TickResult::ExitThread(val) => {
                    self.cmd
                        .send(MemoryCommand::ExitThread(self.tid as u32, val as u32))
                        .unwrap();
                    // println!("Thread {} exited", self.tid);
                    return;
                }
                TickResult::CpuTrap(trap) => {
                    self.memory.read().unwrap().print_mmu();
                    // called `Result::unwrap()` on an `Err` value: "Valid bit is 0, or read is 0 and write is 1 at 40002fec: 000802e6"
                    println!(
                        "CPU trap at PC {:08x}, exiting thread {}: {:x?}",
                        self.cpu.read_pc(),
                        self.tid,
                        trap
                    );
                    self.cmd
                        .send(MemoryCommand::ExitThread(self.tid as u32, 1))
                        .unwrap();
                    return;
                }
                TickResult::Ok => {}
            }
        }
    }
}

struct WorkerHandle {
    joiner: std::thread::JoinHandle<()>,
}

impl Memory {
    pub fn new(base: u32, size: usize) -> (Self, Receiver<MemoryCommand>) {
        let mut backing = HashMap::new();
        let mut free_pages = BTreeSet::new();
        let mut allocated_pages = BTreeSet::new();
        for phys in (base..(base + size as u32)).step_by(4096) {
            backing.insert(phys as usize, [0; 4096]);
            free_pages.insert(phys as usize);
        }
        // Remove the l0 page table
        assert!(free_pages.remove(&(MEMORY_BASE as usize + 4096)));
        assert!(allocated_pages.insert(MEMORY_BASE as usize + 4096));
        let (memory_cmd, memory_cmd_rx) = std::sync::mpsc::channel();
        (
            Self {
                base,
                data: backing,
                allocated_pages,
                free_pages,
                l1_pt: MEMORY_BASE + 4096,
                satp: ((4096 + MEMORY_BASE) >> 12) | 0x8000_0000,
                heap_start: 0x6000_0000,
                heap_size: 0,
                allocation_previous: 0x4000_0000,
                connections: HashMap::new(),
                memory_cmd,
                turbo: false,
            },
            memory_cmd_rx,
        )
    }

    pub fn turbo(&mut self) {
        self.turbo = true;
    }

    pub fn normal(&mut self) {
        self.turbo = false;
        self.memory_ck();
    }

    fn memory_ck(&self) {
        // if self.turbo {
        //     return;
        // }
        // let mut seen_pages = HashMap::new();
        // seen_pages.insert(self.l1_pt, 0);
        // for vpn1 in 0..1024 {
        //     let l1_entry = self.read_u32(self.l1_pt as u64 + vpn1 * 4);
        //     if l1_entry & MMUFLAG_VALID == 0 {
        //         continue;
        //     }

        //     let superpage_addr = vpn1 as u32 * (1 << 22);

        //     for vpn0 in 0..1024 {
        //         let l0_entry = self.read_u32((((l1_entry >> 10) << 12) as u64) + vpn0 as u64 * 4);
        //         if l0_entry & 0x1 == 0 {
        //             continue;
        //         }
        //         let phys = (l0_entry >> 10) << 12;
        //         let current = superpage_addr + vpn0 as u32 * (1 << 12);
        //         if let Some(existing) = seen_pages.get(&phys) {
        //             self.print_mmu();
        //             panic!(
        //                 "Error! Page {:08x} is mapped twice! Once at {:08x} and once at {:08x}",
        //                 phys, existing, current,
        //             );
        //         }
        //         seen_pages.insert(phys, current);
        //     }
        // }
    }

    fn allocate_page(&mut self) -> u32 {
        self.memory_ck();
        let phys = self.free_pages.pop_first().expect("out of memory");
        assert!(self.allocated_pages.insert(phys));

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.
        if phys == 0 {
            panic!("Attempt to allocate zero page");
        }
        self.memory_ck();
        phys as u32
    }

    fn free_page(&mut self, virt: u32) -> Result<(), ()> {
        self.memory_ck();
        let phys = self.virt_to_phys(virt).ok_or(())?;

        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.

        // If the level 1 pagetable doesn't exist, then this address is invalid
        let l1_pt_entry = self.read_u32(self.l1_pt as u64 + vpn1 as u64);
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            return Ok(());
        }

        // println!("Deallocating page {:08x} @ {:08x}", virt, phys);
        if !self.allocated_pages.remove(&(phys as usize)) {
            // self.print_mmu();
            panic!(
                "Page {:08x} @ {:08x} wasn't in the list of allocated pages!",
                phys, virt
            );
        }
        assert!(self.free_pages.insert(phys as usize));

        let l0_pt_phys = ((l1_pt_entry >> 10) << 12) + vpn0 as u32;
        self.write_u32(l0_pt_phys as u64, 0);
        self.memory_ck();

        Ok(())
    }

    fn allocate_virt_region(&mut self, size: usize) -> Option<u32> {
        self.memory_ck();
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
            // println!(
            //     "Allocated {:08x} @ {:08x}",
            //     page,
            //     self.virt_to_phys(page).unwrap()
            // );
        }
        self.allocation_previous = start + size as u32 + 4096;
        self.memory_ck();
        Some(start)
    }

    fn ensure_page(&mut self, address: u32) {
        self.memory_ck();
        let vpn1 = ((address >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((address >> 12) & ((1 << 10) - 1)) as usize * 4;

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
        assert!(self
            .allocated_pages
            .contains(&(((l0_pt_entry >> 10) << 12) as usize)));
        assert!(!self
            .free_pages
            .contains(&(((l0_pt_entry >> 10) << 12) as usize)));
        self.memory_ck();
    }

    fn remove_memory_flags(&mut self, virt: u32, new_flags: u32) {
        self.memory_ck();
        // Ensure they're only adjusting legal flags
        assert!(new_flags & !(MMUFLAG_READABLE | MMUFLAG_WRITABLE | MMUFLAG_EXECUTABLE) == 0);

        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.
        let l1_pt_entry = self.read_u32(self.l1_pt as u64 + vpn1 as u64);

        // If the level 1 pagetable doesn't exist, then this address is invalid
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            return;
        }

        let l0_pt_entry = self.read_u32((((l1_pt_entry >> 10) << 12) + vpn0 as u32) as u64);

        // Ensure the entry hasn't already been mapped.
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            return;
        }

        let old_flags = l0_pt_entry & 0xff;

        // Ensure we're not adding flags
        assert!(old_flags | new_flags == old_flags);

        let l0_pt_entry =
            (l0_pt_entry & !(MMUFLAG_READABLE | MMUFLAG_WRITABLE | MMUFLAG_EXECUTABLE)) | new_flags;

        self.write_u32(
            (((l1_pt_entry >> 10) << 12) + vpn0 as u32) as u64,
            l0_pt_entry,
        );
        self.memory_ck();
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
        use crate::xous::definitions::memoryflags::MemoryFlags;
        println!();
        println!("Memory Map:");
        for vpn1 in 0..1024 {
            let l1_entry = self.read_u32(self.l1_pt as u64 + vpn1 * 4);
            if l1_entry & MMUFLAG_VALID == 0 {
                continue;
            }
            let superpage_addr = vpn1 as u32 * (1 << 22);
            println!(
                "    {:4} Superpage for {:08x} @ {:08x} (flags: {})",
                vpn1,
                superpage_addr,
                (l1_entry >> 10) << 12,
                MemoryFlags::from_bits(l1_entry as usize & 0xff).unwrap(),
            );

            for vpn0 in 0..1024 {
                let l0_entry = self.read_u32((((l1_entry >> 10) << 12) as u64) + vpn0 as u64 * 4);
                if l0_entry & 0x1 == 0 {
                    continue;
                }
                let page_addr = vpn0 as u32 * (1 << 12);
                println!(
                    "        {:4} {:08x} -> {:08x} (flags: {})",
                    vpn0,
                    superpage_addr + page_addr,
                    (l0_entry >> 10) << 12,
                    MemoryFlags::from_bits(l0_entry as usize & 0xff).unwrap()
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

    fn syscall(&mut self, args: [i64; 8]) -> SyscallResult {
        let syscall: Syscall = args.into();

        // println!("Syscall {:?}", SyscallNumber::from(args[0]));
        match syscall {
            Syscall::IncreaseHeap(bytes, _flags) => {
                // println!("IncreaseHeap({} bytes, flags: {:02x})", bytes, _flags);
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
                .into()
            }

            Syscall::MapMemory(phys, virt, size, _flags) => {
                // print!(
                //     "MapMemory(phys: {:08x}, virt: {:08x}, bytes: {}, flags: {:02x})",
                //     phys, virt, size, _flags
                // );
                if virt != 0 {
                    unimplemented!("Non-zero virt address");
                }
                if phys != 0 {
                    unimplemented!("Non-zero phys address");
                }
                let region = self
                    .allocate_virt_region(size as usize)
                    .expect("out of memory");
                // println!(" -> {:08x}", region);
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
                .into()
            }
            Syscall::Connect(id) => {
                // println!(
                //     "Connect([0x{:08x}, 0x{:08x}, 0x{:08x}, 0x{:08x}])",
                //     id[0], id[1], id[2], id[3]
                // );
                if let Some(service) = services::get_service(&id) {
                    let connection_id = self.connections.len() as u32 + 1;
                    self.connections.insert(connection_id, service);
                    [
                        SyscallResultNumber::ConnectionId as i64,
                        connection_id as i64,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ]
                    .into()
                } else {
                    [
                        SyscallResultNumber::ConnectionId as i64,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ]
                    .into()
                }
            }
            Syscall::SendMessage(connection_id, kind, opcode, args) => {
                // println!(
                //     "SendMessage({}, {}, {}: {:x?})",
                //     connection_id, kind, opcode, args
                // );
                let memory_region = if kind == 1 || kind == 2 || kind == 3 {
                    let mut memory_region = vec![0; args[1] as usize];
                    for (offset, value) in memory_region.iter_mut().enumerate() {
                        *value = self.read_u8(
                            self.virt_to_phys(args[0] + offset as u32)
                                .expect("invalid memory address")
                                as u64,
                        );
                    }
                    Some(memory_region)
                } else {
                    None
                };
                let Some(service) = self.connections.get_mut(&connection_id) else {
                    println!("Unhandled connection ID {}", connection_id);
                    return [
                        SyscallResultNumber::Error as i64,
                        9, /* ServerNotFound */
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ]
                    .into();
                };
                match kind {
                    1..=3 => {
                        let mut memory_region = memory_region.unwrap();
                        let extra = [args[2], args[3]];
                        match kind {
                            1 => match service.lend_mut(0, opcode, &mut memory_region, extra) {
                                services::LendResult::WaitForResponse(msg) => msg.into(),
                                services::LendResult::MemoryReturned(result) => {
                                    for (offset, value) in memory_region.into_iter().enumerate() {
                                        self.write_u8(args[0] as u64 + offset as u64, value);
                                    }
                                    [
                                        SyscallResultNumber::MemoryReturned as i64,
                                        result[0] as i64,
                                        result[1] as i64,
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                    ]
                                    .into()
                                }
                            },
                            2 => match service.lend(0, opcode, &memory_region, extra) {
                                services::LendResult::WaitForResponse(msg) => msg.into(),
                                services::LendResult::MemoryReturned(result) => [
                                    SyscallResultNumber::MemoryReturned as i64,
                                    result[0] as i64,
                                    result[1] as i64,
                                    0,
                                    0,
                                    0,
                                    0,
                                    0,
                                ]
                                .into(),
                            },
                            3 => {
                                service.send(0, opcode, &memory_region, extra);
                                [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into()
                            }
                            _ => unreachable!(),
                        }
                    }
                    4 => {
                        service.scalar(0, opcode, args);
                        [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into()
                    }
                    5 => match service.blocking_scalar(0, opcode, args) {
                        services::ScalarResult::Scalar1(result) => [
                            SyscallResultNumber::Scalar1 as i64,
                            result as i64,
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                        ]
                        .into(),
                        services::ScalarResult::Scalar2(result) => [
                            SyscallResultNumber::Scalar2 as i64,
                            result[0] as i64,
                            result[1] as i64,
                            0,
                            0,
                            0,
                            0,
                            0,
                        ]
                        .into(),
                        services::ScalarResult::Scalar5(result) => [
                            SyscallResultNumber::Scalar5 as i64,
                            result[0] as i64,
                            result[1] as i64,
                            result[2] as i64,
                            result[3] as i64,
                            result[4] as i64,
                            0,
                            0,
                        ]
                        .into(),
                        services::ScalarResult::WaitForResponse(msg) => msg.into(),
                    },
                    _ => {
                        println!("Unknown message kind {}", kind);
                        [
                            SyscallResultNumber::Error as i64,
                            9, /* ServerNotFound */
                            0,
                            0,
                            0,
                            0,
                            0,
                            0,
                        ]
                        .into()
                    }
                }
            }
            Syscall::UpdateMemoryFlags(address, range, value) => {
                for addr in address..(address + range) {
                    self.remove_memory_flags(addr as u32, value as u32);
                }
                [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into()
            }
            Syscall::Yield => [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into(),
            Syscall::CreateThread(
                entry_point,
                stack_pointer,
                stack_length,
                argument_1,
                argument_2,
                argument_3,
                argument_4,
            ) => {
                let (tx, rx) = std::sync::mpsc::channel();
                self.memory_cmd
                    .send(MemoryCommand::CreateThread(
                        entry_point as _,
                        stack_pointer as _,
                        stack_length as _,
                        argument_1 as _,
                        argument_2 as _,
                        argument_3 as _,
                        argument_4 as _,
                        tx,
                    ))
                    .unwrap();
                let thread_id = rx.recv().unwrap();
                [
                    SyscallResultNumber::ThreadId as i64,
                    thread_id,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]
                .into()
            }
            Syscall::UnmapMemory(address, size) => {
                // println!("UnmapMemory({:08x}, {})", address, size);
                for offset in (address..address + size).step_by(4096) {
                    self.free_page(offset as u32).unwrap();
                }
                [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into()
            }
            Syscall::JoinThread(thread_id) => {
                // println!("JoinThread({})", thread_id);
                let (tx, rx) = std::sync::mpsc::channel();
                self.memory_cmd
                    .send(MemoryCommand::JoinThread(thread_id as _, tx))
                    .unwrap();
                rx.into()
            }
            Syscall::Unknown(args) => {
                println!(
                    "Unhandled {:?}: {:?}",
                    SyscallNumber::from(args[0]),
                    &args[1..]
                );
                unimplemented!("Unhandled syscall");
                // [SyscallResultNumber::Unimplemented as _, 0, 0, 0, 0, 0, 0, 0]
            }
        }
    }
}

pub struct Machine {
    memory: Arc<RwLock<Memory>>,
    workers: Vec<WorkerHandle>,
    satp: u64,
    memory_cmd_sender: Sender<MemoryCommand>,
    memory_cmd: Receiver<MemoryCommand>,
    thread_id_counter: AtomicI64,
}

impl Machine {
    pub fn new(program: &[u8]) -> Result<Self, LoadError> {
        let (memory, memory_cmd) = Memory::new(MEMORY_BASE, 16 * 1024 * 1024);
        let memory_cmd_sender = memory.memory_cmd.clone();
        let memory = Arc::new(RwLock::new(memory));

        let mut machine = Self {
            memory,
            workers: vec![],
            satp: 0,
            memory_cmd,
            memory_cmd_sender,
            thread_id_counter: AtomicI64::new(1),
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
        memory_writer.turbo();
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

        // memory_writer.print_mmu();
        memory_writer.normal();

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
        cpu.write_register(2, 0xc002_0000 - 16);

        let cmd = self.memory_cmd_sender.clone();
        let memory = self.memory.clone();
        let joiner = std::thread::spawn(move || Worker::new(cpu, cmd, 0, memory).run());

        self.workers.push(WorkerHandle { joiner });
        self.satp = satp;

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (join_tx, rx) = std::sync::mpsc::channel();
        let main_worker: WorkerHandle = self.workers.pop().unwrap();
        join_tx.send(main_worker.joiner).unwrap();
        let memory_cmd_sender = self.memory_cmd_sender.clone();
        std::thread::spawn(move || {
            while let Ok(msg) = rx.try_recv() {
                if let Err(_e) = msg.join() {}
            }
            memory_cmd_sender.send(MemoryCommand::Exit).unwrap();
        });
        let mut joining_threads = HashMap::new();
        let mut exited_threads = HashSet::new();
        while let Ok(msg) = self.memory_cmd.recv() {
            match msg {
                MemoryCommand::JoinThread(tid, sender) => {
                    if exited_threads.contains(&tid) {
                        sender
                            .send((
                                [SyscallResultNumber::Scalar1 as i64, 0, 0, 0, 0, 0, 0, 0],
                                None,
                            ))
                            .unwrap();
                    } else {
                        joining_threads
                            .entry(tid)
                            .or_insert_with(Vec::new)
                            .push(sender);
                    }
                }
                MemoryCommand::ExitThread(tid, result) => {
                    exited_threads.insert(tid);
                    if let Some(joiners) = joining_threads.remove(&tid) {
                        for joiner in joiners {
                            joiner
                                .send((
                                    [
                                        SyscallResultNumber::Scalar1 as i64,
                                        result.into(),
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                    ],
                                    None,
                                ))
                                .unwrap();
                        }
                    }
                }
                MemoryCommand::CreateThread(
                    entry_point,
                    stack_pointer,
                    stack_length,
                    argument_1,
                    argument_2,
                    argument_3,
                    argument_4,
                    tx,
                ) => {
                    let mut cpu = riscv_cpu::CpuBuilder::new(self.memory.clone())
                        .xlen(riscv_cpu::Xlen::Bit32)
                        .build();

                    cpu.write_csr(riscv_cpu::cpu::CSR_SATP_ADDRESS, self.satp)
                        .map_err(|_| LoadError::SatpWriteError)?;
                    cpu.update_pc(entry_point as u64);

                    // Return to User Mode (0 << 11) with interrupts disabled (1 << 5)
                    cpu.write_csr(riscv_cpu::cpu::CSR_MSTATUS_ADDRESS, 1 << 5)
                        .map_err(|_| LoadError::MstatusWriteError)?;

                    cpu.write_csr(riscv_cpu::cpu::CSR_SEPC_ADDRESS, entry_point as u64)
                        .unwrap();

                    // SRET to return to user mode
                    cpu.execute_opcode(0x10200073).map_err(LoadError::CpuTrap)?;

                    // Update the stack pointer
                    cpu.write_register(2, (stack_pointer + stack_length) as i64 - 16);
                    cpu.write_register(10, argument_1 as i64);
                    cpu.write_register(11, argument_2 as i64);
                    cpu.write_register(12, argument_3 as i64);
                    cpu.write_register(13, argument_4 as i64);

                    let cmd = self.memory_cmd_sender.clone();
                    let tid = self.thread_id_counter.fetch_add(1, Ordering::SeqCst);
                    let memory = self.memory.clone();
                    join_tx
                        .send(std::thread::spawn(move || {
                            Worker::new(cpu, cmd, tid, memory).run()
                        }))
                        .unwrap();
                    tx.send(tid).unwrap();
                }
                MemoryCommand::Exit => {
                    break;
                }
            }
        }
        println!("Done! memory_cmd returned error");

        Ok(())
    }
}
