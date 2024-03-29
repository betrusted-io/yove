use riscv_cpu::{cpu::Memory as OtherMemory, mmu::SystemBus};
mod definitions;
mod services;
mod syscalls;

use definitions::{Syscall, SyscallNumber, SyscallResultNumber};
pub use riscv_cpu::mmu::SyscallResult;
use std::{
    collections::{BTreeSet, HashMap},
    num::NonZeroU32,
    sync::{
        atomic::{AtomicI32, AtomicU32, Ordering},
        mpsc::{Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::JoinHandle,
};

use self::definitions::SyscallErrorNumber;

const MEMORY_BASE: u32 = 0x8000_0000;
const ALLOCATION_START: u32 = 0x4000_0000;
const ALLOCATION_END: u32 = ALLOCATION_START + 5 * 1024 * 1024;
const HEAP_START: u32 = 0xa000_0000;
const HEAP_END: u32 = HEAP_START + 5 * 1024 * 1024;
const STACK_START: u32 = 0xc000_0000;
const STACK_END: u32 = 0xc002_0000;

/// Magic number indicating we have an environment block
const ENV_MAGIC: [u8; 4] = *b"EnvB";

/// Command line arguments list
const ARGS_MAGIC: [u8; 4] = *b"ArgL";

/// Magic number indicating the loader has passed application parameters
const PARAMS_MAGIC: [u8; 4] = *b"AppP";

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
// pub type ResponseData = ([i32; 8], Option<(Vec<u8>, u32)>);

enum MemoryCommand {
    // Exit,
    // ExitThread(u32 /* tid */, u32 /* result */),
    CreateThread(
        u32,                                         /* entry point */
        u32,                                         /* stack pointer */
        u32,                                         /* stack length */
        u32,                                         /* argument 1 */
        u32,                                         /* argument 2 */
        u32,                                         /* argument 3 */
        u32,                                         /* argument 4 */
        Sender<(i32, std::thread::JoinHandle<u32>)>, /* Thread ID + Result*/
    ),
    // JoinThread(u32, Sender<ResponseData>),
}

struct Worker {
    cpu: riscv_cpu::Cpu,
    // cmd: Sender<MemoryCommand>,
    tid: i32,
    memory: Box<Memory>,
}

impl Worker {
    fn new(
        cpu: riscv_cpu::Cpu,
        // cmd: Sender<MemoryCommand>,
        tid: i32,
        memory: Box<Memory>,
    ) -> Self {
        Self {
            cpu,
            // cmd,
            tid,
            memory,
        }
    }

    fn run(&mut self) -> u32 {
        use riscv_cpu::cpu::TickResult;
        loop {
            match self.cpu.tick() {
                // If we get a PauseEmulation result, it will have an accompanying Receiver.
                // Block on this receiver until we get a result, then load that result into
                // the CPU.
                TickResult::PauseEmulation(e) => {
                    let (result, data) = e.recv().unwrap();
                    if let Some(data) = data {
                        let syscall_type = self.cpu.read_register(10);
                        let message_kind = self.cpu.read_register(12);
                        let memory_offset = self.cpu.read_register(14) as u32;
                        // let memory_size = self.cpu.read_register(15);

                        assert!(syscall_type == SyscallNumber::SendMessage as i32);
                        assert!(message_kind == 1 || message_kind == 2);
                        let mmu = self.cpu.get_mut_mmu();
                        for (offset, byte) in data.into_iter().enumerate() {
                            mmu.store(offset as u32 + memory_offset, byte).unwrap();
                        }
                    }
                    for (index, value) in result.iter().enumerate() {
                        self.cpu.write_register(10 + index as u8, *value);
                    }
                }
                TickResult::ExitThread(val) => {
                    //     self.cmd
                    //         .send(MemoryCommand::ExitThread(self.tid as u32, val))
                    //         .unwrap();
                    // eprintln!("Thread {} exited", self.tid);
                    return val;
                }
                TickResult::JoinThread(handle) => {
                    let result = handle.join().unwrap();
                    self.cpu
                        .write_register(10, SyscallResultNumber::Scalar1 as i32);
                    self.cpu.write_register(11, result as i32);
                    for reg in 12..18 {
                        self.cpu.write_register(reg, 0);
                    }
                    // self.cmd
                    //     .send(MemoryCommand::ExitThread(self.tid as u32, result))
                    //     .unwrap();
                }
                TickResult::CpuTrap(trap) => {
                    self.memory.print_mmu();
                    // called `Result::unwrap()` on an `Err` value: "Valid bit is 0, or read is 0 and write is 1 at 40002fec: 000802e6"
                    println!(
                        "CPU trap at PC {:08x}, exiting thread {}: {:x?}",
                        self.cpu.read_pc(),
                        self.tid,
                        trap
                    );
                    // self.cmd
                    //     .send(MemoryCommand::ExitThread(self.tid as u32, 1))
                    //     .unwrap();
                    return !0;
                }
                TickResult::Ok => {}
            }
        }
    }
}

#[derive(Clone)]
struct Memory {
    base: u32,
    data: Arc<Vec<RwLock<Vec<u32>>>>,
    allocated_pages: Arc<Mutex<BTreeSet<usize>>>,
    free_pages: Arc<Mutex<BTreeSet<usize>>>,
    heap_start: Arc<AtomicU32>,
    heap_size: Arc<AtomicU32>,
    allocation_previous: Arc<AtomicU32>,
    l1_pt: u32,
    satp: u32,
    connections: Arc<Mutex<HashMap<u32, Box<dyn services::Service + Send + Sync>>>>,
    connection_index: Arc<AtomicU32>,
    named_connections_index: Arc<Mutex<HashMap<[u32; 4], u32>>>,
    memory_cmd: Sender<MemoryCommand>,
    translation_cache: Arc<RwLock<Vec<Option<NonZeroU32>>>>,
    allocated_bytes: Arc<AtomicU32>,
    reservations: Arc<Mutex<HashMap<u32, u32>>>,
    thread_handles: Arc<Mutex<HashMap<i32, JoinHandle<u32>>>>,
}

impl Memory {
    pub fn new(base: u32, size: usize) -> (Self, Receiver<MemoryCommand>) {
        let mut backing = vec![];
        let mut free_pages = BTreeSet::new();
        let mut allocated_pages = BTreeSet::new();

        // Populate the backing table as well as the list of free pages
        for phys in (0..(size as u32)).step_by(4096) {
            backing.push(RwLock::new(vec![0; 1024]));
            free_pages.insert((phys + base) as usize);
        }
        // Allocate the l0 page table
        assert!(free_pages.remove(&(MEMORY_BASE as usize + 4096)));
        assert!(allocated_pages.insert(MEMORY_BASE as usize + 4096));

        let (memory_cmd, memory_cmd_rx) = std::sync::mpsc::channel();
        (
            Self {
                base,
                data: Arc::new(backing),
                allocated_pages: Arc::new(Mutex::new(allocated_pages)),
                free_pages: Arc::new(Mutex::new(free_pages)),
                l1_pt: MEMORY_BASE + 4096,
                satp: ((4096 + MEMORY_BASE) >> 12) | 0x8000_0000,
                heap_start: Arc::new(AtomicU32::new(HEAP_START)),
                heap_size: Arc::new(AtomicU32::new(0)),
                allocation_previous: Arc::new(AtomicU32::new(ALLOCATION_START)),
                connections: Arc::new(Mutex::new(HashMap::new())),
                connection_index: Arc::new(AtomicU32::new(1)),
                memory_cmd,
                translation_cache: Arc::new(RwLock::new(vec![None; 0x000f_ffff])),
                allocated_bytes: Arc::new(AtomicU32::new(4096)),
                reservations: Arc::new(Mutex::new(HashMap::new())),
                thread_handles: Arc::new(Mutex::new(HashMap::new())),
                named_connections_index: Arc::new(Mutex::new(HashMap::new())),
            },
            memory_cmd_rx,
        )
    }

    // fn memory_ck(&self) {
    //     if self.turbo {
    //         return;
    //     }
    //     let mut seen_pages = HashMap::new();
    //     seen_pages.insert(self.l1_pt, 0);
    //     for vpn1 in 0..1024 {
    //         let l1_entry = self.read_u32(self.l1_pt as u64 + vpn1 * 4);
    //         if l1_entry & MMUFLAG_VALID == 0 {
    //             continue;
    //         }

    //         let superpage_addr = vpn1 as u32 * (1 << 22);

    //         for vpn0 in 0..1024 {
    //             let l0_entry = self.read_u32((((l1_entry >> 10) << 12) as u64) + vpn0 as u64 * 4);
    //             if l0_entry & 0x1 == 0 {
    //                 continue;
    //             }
    //             let phys = (l0_entry >> 10) << 12;
    //             let current = superpage_addr + vpn0 as u32 * (1 << 12);
    //             if let Some(existing) = seen_pages.get(&phys) {
    //                 self.print_mmu();
    //                 panic!(
    //                     "Error! Page {:08x} is mapped twice! Once at {:08x} and once at {:08x}",
    //                     phys, existing, current,
    //                 );
    //             }
    //             seen_pages.insert(phys, current);
    //         }
    //     }
    // }

    /// Allocate a physical page from RAM.
    fn allocate_phys_page(&self) -> Option<u32> {
        let Some(phys) = self.free_pages.lock().unwrap().pop_first() else {
            // panic!(
            //     "out of memory when attempting to allocate a page. There are {} bytes allocated.",
            //     self.allocated_bytes
            // );
            return None;
        };
        assert!(self.allocated_pages.lock().unwrap().insert(phys));
        self.allocated_bytes.fetch_add(4096, Ordering::Relaxed);

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.
        if phys == 0 {
            panic!("Attempt to allocate zero page");
        }
        Some(phys as u32)
    }

    fn free_virt_page(&self, virt: u32) -> Result<(), ()> {
        let phys = self
            .virt_to_phys(virt)
            .ok_or(())
            .expect("tried to free a page that was allocated");

        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;
        self.allocated_bytes.fetch_sub(4096, Ordering::Relaxed);

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.

        // If the level 1 pagetable doesn't exist, then this address is invalid
        let l1_pt_entry = self.read_u32(self.l1_pt + vpn1 as u32);
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            panic!("Tried to free a page where the level 1 pagetable didn't exist");
        }

        assert!(self
            .allocated_pages
            .lock()
            .unwrap()
            .remove(&(phys as usize)));
        assert!(self.free_pages.lock().unwrap().insert(phys as usize));
        self.translation_cache.write().unwrap()[phys as usize >> 12] = None;

        let l0_pt_phys = ((l1_pt_entry >> 10) << 12) + vpn0 as u32;
        assert!(self.read_u32(l0_pt_phys) & MMUFLAG_VALID != 0);
        self.write_u32(l0_pt_phys, 0);

        Ok(())
    }

    fn allocate_virt_region(&self, size: usize) -> Option<u32> {
        let size = size as u32;
        // Look for a sequence of `size` pages that are free.
        let mut address = None;
        let allocation_previous = self.allocation_previous.load(Ordering::Relaxed);
        for potential_start in (allocation_previous..ALLOCATION_END - size)
            .step_by(4096)
            .chain((ALLOCATION_START..allocation_previous - size).step_by(4096))
        {
            let mut all_free = true;
            for check_page in (potential_start..potential_start + size).step_by(4096) {
                if self.virt_to_phys(check_page).is_some() {
                    all_free = false;
                    break;
                }
            }
            if all_free {
                self.allocation_previous
                    .store(potential_start + size, Ordering::Relaxed);
                address = Some(potential_start);
                break;
            }
        }
        if let Some(address) = address {
            let mut error_mark = None;
            for page in (address..(address + size)).step_by(4096) {
                if self.ensure_page(page).is_none() {
                    error_mark = Some(page);
                    break;
                }
            }
            if let Some(error_mark) = error_mark {
                for page in (address..error_mark).step_by(4096) {
                    self.free_virt_page(page).unwrap();
                }
                return None;
            }
        }
        address
    }

    fn ensure_page(&self, virt: u32) -> Option<bool> {
        assert!(virt != 0);
        let mut allocated = false;
        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;

        // If the level 1 pagetable doesn't exist, then this address is invalid
        let mut l1_pt_entry = self.read_u32(self.l1_pt + vpn1 as u32);
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            // Allocate a new page for the level 1 pagetable
            let Some(l0_pt_phys) = self.allocate_phys_page() else {
                return None;
            };
            // println!("Allocating level 0 pagetable at {:08x}", l0_pt_phys);
            l1_pt_entry =
                ((l0_pt_phys >> 12) << 10) | MMUFLAG_VALID | MMUFLAG_DIRTY | MMUFLAG_ACCESSED;
            // Map the level 1 pagetable into the root pagetable
            self.write_u32(self.l1_pt + vpn1 as u32, l1_pt_entry);
            allocated = true;
        }

        let l0_pt_phys = ((l1_pt_entry >> 10) << 12) + vpn0 as u32;
        let mut l0_pt_entry = self.read_u32(l0_pt_phys);

        // Ensure the entry hasn't already been mapped.
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            let Some(phys) = self.allocate_phys_page() else {
                return None;
            };
            l0_pt_entry = ((phys >> 12) << 10)
                | MMUFLAG_VALID
                | MMUFLAG_WRITABLE
                | MMUFLAG_READABLE
                | MMUFLAG_EXECUTABLE
                | MMUFLAG_USERMODE
                | MMUFLAG_DIRTY
                | MMUFLAG_ACCESSED;
            // Map the level 0 pagetable into the level 1 pagetable
            self.write_u32(l0_pt_phys, l0_pt_entry);
            self.translation_cache.write().unwrap()[(virt >> 12) as usize] = NonZeroU32::new(phys);

            allocated = true;
        }
        assert!(self
            .allocated_pages
            .lock()
            .unwrap()
            .contains(&(((l0_pt_entry >> 10) << 12) as usize)));
        assert!(!self
            .free_pages
            .lock()
            .unwrap()
            .contains(&(((l0_pt_entry >> 10) << 12) as usize)));
        Some(allocated)
    }

    fn remove_memory_flags(&self, virt: u32, new_flags: u32) {
        // Ensure they're only adjusting legal flags
        assert!(new_flags & !(MMUFLAG_READABLE | MMUFLAG_WRITABLE | MMUFLAG_EXECUTABLE) == 0);

        let vpn1 = ((virt >> 22) & ((1 << 10) - 1)) as usize * 4;
        let vpn0 = ((virt >> 12) & ((1 << 10) - 1)) as usize * 4;

        // The root (l1) pagetable is defined to be mapped into our virtual
        // address space at this address.
        let l1_pt_entry = self.read_u32(self.l1_pt + vpn1 as u32);

        // If the level 1 pagetable doesn't exist, then this address is invalid
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            return;
        }

        let l0_pt_entry = self.read_u32(((l1_pt_entry >> 10) << 12) + vpn0 as u32);

        // Ensure the entry hasn't already been mapped.
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            return;
        }

        let old_flags = l0_pt_entry & 0xff;

        // Ensure we're not adding flags
        assert!(old_flags | new_flags == old_flags);

        let l0_pt_entry =
            (l0_pt_entry & !(MMUFLAG_READABLE | MMUFLAG_WRITABLE | MMUFLAG_EXECUTABLE)) | new_flags;

        self.write_u32(((l1_pt_entry >> 10) << 12) + vpn0 as u32, l0_pt_entry);
    }

    fn write_bytes(&mut self, data: &[u8], start: u32) {
        for (i, byte) in data.iter().enumerate() {
            let i = i as u32;
            self.ensure_page(start + i);
            let phys = self.virt_to_phys(start + i).unwrap();

            self.write_u8(phys, *byte);
        }
    }

    #[allow(dead_code)]
    pub fn print_mmu(&self) {
        use crate::xous::definitions::memoryflags::MemoryFlags;
        println!();
        println!("Memory Map:");
        for vpn1 in 0..1024 {
            let l1_entry = self.read_u32(self.l1_pt + vpn1 * 4);
            if l1_entry & MMUFLAG_VALID == 0 {
                continue;
            }
            let superpage_addr = vpn1 * (1 << 22);
            println!(
                "    {:4} Superpage for {:08x} @ {:08x} (flags: {})",
                vpn1,
                superpage_addr,
                (l1_entry >> 10) << 12,
                MemoryFlags::from_bits(l1_entry as usize & 0xff).unwrap(),
            );

            for vpn0 in 0..1024 {
                let l0_entry = self.read_u32(((l1_entry >> 10) << 12) + vpn0 as u32 * 4);
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
        let l1_pt_entry = self.read_u32(self.l1_pt + vpn1 as u32);

        // If the level 1 pagetable doesn't exist, then this address is invalid
        if l1_pt_entry & MMUFLAG_VALID == 0 {
            return None;
        }
        if l1_pt_entry & (MMUFLAG_EXECUTABLE | MMUFLAG_READABLE | MMUFLAG_WRITABLE) != 0 {
            return None;
        }

        let l0_pt_entry = self.read_u32(((l1_pt_entry >> 10) << 12) + vpn0 as u32);

        // Check if the mapping is valid
        if l0_pt_entry & MMUFLAG_VALID == 0 {
            None
        } else {
            Some(((l0_pt_entry >> 10) << 12) | offset)
        }
    }
}

impl riscv_cpu::cpu::Memory for Memory {
    fn read_u8(&self, address: u32) -> u8 {
        let address = address - self.base;
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        let index = offset >> 2;
        let pos = (offset % 4) * 8;

        self.data
            .get(page >> 12)
            .map(|page| page.read().unwrap()[index] >> pos)
            .unwrap_or(0) as u8
    }

    fn read_u16(&self, address: u32) -> u16 {
        if address & 1 == 0 {
            let address = address - self.base;
            let page = address as usize & !0xfff;
            let offset = address as usize & 0xfff;
            let index = offset / 4;
            let pos = (offset % 4) * 8;
            self.data
                .get(page >> 12)
                .map(|page| page.read().unwrap()[index] >> pos)
                .unwrap_or(0) as u16
        } else {
            let data = [self.read_u8(address), self.read_u8(address + 1)];
            u16::from_le_bytes(data)
        }
    }

    fn read_u32(&self, address: u32) -> u32 {
        if address & 3 == 0 {
            let address = address - self.base;
            let page = address as usize & !0xfff;
            let offset = address as usize & 0xfff;
            let index = offset / 4;
            self.data
                .get(page >> 12)
                .map(|page| page.read().unwrap()[index])
                .unwrap_or(0)
        } else {
            let data = [
                self.read_u8(address),
                self.read_u8(address + 1),
                self.read_u8(address + 2),
                self.read_u8(address + 3),
            ];
            u32::from_le_bytes(data)
        }
    }

    fn write_u8(&self, address: u32, value: u8) {
        let address = address - self.base;
        let page = address as usize & !0xfff;
        let offset = address as usize & 0xfff;
        let index = offset / 4;
        let pos = (offset % 4) * 8;
        if let Some(page) = self.data.get(page >> 12) {
            let mut data = page.write().unwrap();
            data[index] = (data[index] & !(0xff << pos)) | ((value as u32) << pos);
        }
    }

    fn write_u16(&self, address: u32, value: u16) {
        if address & 1 == 0 {
            let address = address - self.base;
            let page = address as usize & !0xfff;
            let offset = address as usize & 0xfff;
            let index = offset >> 2;
            let pos = (offset % 4) * 8;
            if let Some(page) = self.data.get(page >> 12) {
                let mut data = page.write().unwrap();
                data[index] = (data[index] & !(0xffff << pos)) | ((value as u32) << pos);
            }
        } else {
            for (offset, byte) in value.to_le_bytes().iter().enumerate() {
                self.write_u8(address + offset as u32, *byte);
            }
        }
    }

    fn write_u32(&self, address: u32, value: u32) {
        if address & 3 == 0 {
            let address = address - self.base;
            let page = address as usize & !0xfff;
            let offset = address as usize & 0xfff;
            let index = offset >> 2;
            if let Some(page) = self.data.get(page >> 12) {
                let mut page = page.write().unwrap();
                page[index] = value;
            }
        } else {
            for (offset, byte) in value.to_le_bytes().iter().enumerate() {
                self.write_u8(address + offset as u32, *byte);
            }
        }
    }

    fn validate_address(&self, address: u32) -> bool {
        if address < self.base {
            return false;
        }
        let address = address as usize - self.base as usize;
        address < self.data.len()
    }

    fn syscall(&self, args: [i32; 8]) -> SyscallResult {
        let syscall: Syscall = args.into();

        // println!("Syscall {:?}", SyscallNumber::from(args[0]));
        match syscall {
            Syscall::IncreaseHeap(bytes, flags) => syscalls::increase_heap(self, bytes, flags),

            Syscall::MapMemory(phys, virt, size, flags) => {
                syscalls::map_memory(self, phys, virt, size, flags)
            }
            Syscall::Connect(id) => syscalls::connect(self, id),
            Syscall::TryConnect(id) => syscalls::try_connect(self, id),
            Syscall::SendMessage(connection_id, kind, opcode, args) => {
                syscalls::send_message(self, connection_id, kind, opcode, args)
            }
            Syscall::TrySendMessage(connection_id, kind, opcode, args) => {
                syscalls::try_send_message(self, connection_id, kind, opcode, args)
            }
            Syscall::UpdateMemoryFlags(address, range, value) => {
                for addr in address..(address + range) {
                    self.remove_memory_flags(addr as u32, value as u32);
                }
                [SyscallResultNumber::Ok as i32, 0, 0, 0, 0, 0, 0, 0].into()
            }
            Syscall::Yield => [SyscallResultNumber::Ok as i32, 0, 0, 0, 0, 0, 0, 0].into(),
            Syscall::CreateThread(
                entry_point,
                stack_pointer,
                stack_length,
                argument_1,
                argument_2,
                argument_3,
                argument_4,
            ) => syscalls::create_thread(
                self,
                entry_point,
                stack_pointer,
                stack_length,
                [argument_1, argument_2, argument_3, argument_4],
            ),
            Syscall::UnmapMemory(address, size) => {
                // println!("UnmapMemory({:08x}, {})", address, size);
                for offset in (address..address + size).step_by(4096) {
                    self.free_virt_page(offset as u32).unwrap();
                }
                [SyscallResultNumber::Ok as i32, 0, 0, 0, 0, 0, 0, 0].into()
            }
            Syscall::JoinThread(thread_id) => {
                // println!("JoinThread({})", thread_id);
                // let (tx, rx) = std::sync::mpsc::channel();
                // self.memory_cmd
                //     .send(MemoryCommand::JoinThread(thread_id as _, tx))
                //     .unwrap();
                // rx.into()
                if let Some(val) = self.thread_handles.lock().unwrap().remove(&thread_id) {
                    SyscallResult::JoinThread(val)
                } else {
                    [
                        SyscallResultNumber::Error as i32,
                        SyscallErrorNumber::ThreadNotAvailable as i32,
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
            Syscall::TerminateProcess(exit_code) => {
                // println!("TerminateProcess({})", result);
                syscalls::terminate_process(self, exit_code)
            }
            Syscall::GetProcessId => {
                [SyscallResultNumber::ProcessId as i32, 2, 0, 0, 0, 0, 0, 0].into()
            }
            Syscall::Unknown(args) => {
                eprintln!(
                    "Unhandled syscall #{} {:?}: {:?}",
                    args[0],
                    SyscallNumber::from(args[0]),
                    &args[1..]
                );
                unimplemented!("Unhandled syscall");
                // [SyscallResultNumber::Unimplemented as _, 0, 0, 0, 0, 0, 0, 0]
            }
        }
    }

    fn translate(&self, v_address: u32) -> Option<u32> {
        self.translation_cache.read().unwrap()[v_address as usize >> 12]
            .map(|x| x.get() | v_address & 0xfff)
    }

    fn reserve(&self, core: u32, p_address: u32) {
        self.reservations.lock().unwrap().insert(p_address, core);
    }

    fn clear_reservation(&self, core: u32, p_address: u32) -> bool {
        self.reservations.lock().unwrap().remove(&{ p_address }) == Some(core)
    }

    fn clone(&self) -> Box<dyn OtherMemory + Send + Sync> {
        Box::new(Clone::clone(self))
    }
}

impl SystemBus for Memory {}

pub struct Machine {
    memory: Box<Memory>,
    // workers: Vec<WorkerHandle>,
    satp: u32,
    // memory_cmd_sender: Sender<MemoryCommand>,
    memory_cmd: Receiver<MemoryCommand>,
    thread_id_counter: AtomicI32,
}

impl Machine {
    pub fn new(program: &[u8]) -> Result<Self, LoadError> {
        let (memory, memory_cmd) = Memory::new(MEMORY_BASE, 16 * 1024 * 1024);
        // let memory_cmd_sender = memory.memory_cmd.clone();
        let memory = Box::new(memory);

        let mut machine = Self {
            memory,
            // workers: vec![],
            satp: 0,
            memory_cmd,
            // memory_cmd_sender,
            thread_id_counter: AtomicI32::new(1),
        };

        machine.load_program(program)?;

        Ok(machine)
    }

    pub fn create_params() -> std::io::Result<Vec<u8>> {
        use std::io::Write;

        // Copy the host's environment variables into the target's environment
        let mut env_map = HashMap::new();
        for (key, value) in std::env::vars() {
            env_map.insert(key, value);
        }

        let mut env_data = vec![];
        // Number of environment variables
        env_data.write_all(&(env_map.len() as u16).to_le_bytes())?;
        for (key, value) in env_map.iter() {
            env_data.extend_from_slice(&(key.len() as u16).to_le_bytes());
            env_data.extend_from_slice(key.as_bytes());
            env_data.extend_from_slice(&(value.len() as u16).to_le_bytes());
            env_data.extend_from_slice(value.as_bytes());
        }
        let mut env_tag = vec![];

        // Magic number
        env_tag.write_all(&ENV_MAGIC)?;
        // Size of the EnvB block
        env_tag.write_all(&(env_data.len() as u32).to_le_bytes())?;

        // Environment variables
        env_tag.write_all(&env_data)?;

        let mut arg_tag = vec![];
        let mut arg_data = vec![];
        // Copy arguments, making sure to skip the program name and target name
        let our_args = std::env::args().skip(1).collect::<Vec<_>>();
        if our_args.contains(&"--".to_owned()) {
            let mut found = false;
            let mut first = false;
            for arg in our_args.iter() {
                // Always push the first argument, since it's the program name
                if first {
                    arg_data.push(arg);
                    first = false;
                } else if found {
                    arg_data.push(arg);
                } else if arg == "--" {
                    found = true;
                }
            }
        } else {
            for arg in our_args.iter() {
                arg_data.push(arg);
            }
        }
        arg_tag.write_all(&ARGS_MAGIC)?;
        let mut args_size = 0;
        for entry in arg_data.iter() {
            args_size += entry.len() + 2;
        }
        arg_tag.write_all(&(args_size as u32 + 2).to_le_bytes())?;
        arg_tag.write_all(&(arg_data.len() as u16).to_le_bytes())?;
        for entry in arg_data {
            arg_tag.write_all(&(entry.len() as u16).to_le_bytes())?;
            arg_tag.write_all(entry.as_bytes())?;
        }

        // Magic number
        let mut params_tag = vec![];
        params_tag.write_all(&PARAMS_MAGIC)?;
        // Size of the AppP block
        params_tag.write_all(&8u32.to_le_bytes())?;
        // Size of the entire array
        params_tag.write_all(&((env_tag.len() + arg_tag.len()) as u32 + 16).to_le_bytes())?;
        // Number of blocks
        params_tag.write_all(&3u32.to_le_bytes())?;

        let mut sample_data = vec![];
        sample_data.write_all(&params_tag)?;
        sample_data.write_all(&env_tag)?;
        sample_data.write_all(&arg_tag)?;

        Ok(sample_data)
    }

    pub fn load_program(&mut self, program: &[u8]) -> Result<(), LoadError> {
        let mut cpu = riscv_cpu::CpuBuilder::new(self.memory.clone()).build();

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
                // println!(
                //     "Ignoring section {}...",
                //     elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("???")
                // );
                continue;
            }

            // Place the eh_frame offset into $a0 so the program can unwind correctly
            if elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("???") == ".eh_frame" {
                cpu.write_register(10, sh.sh_addr.try_into().unwrap());
            }

            if sh.sh_type & goblin::elf::section_header::SHT_NOBITS != 0 {
                for addr in sh.sh_addr..(sh.sh_addr + sh.sh_size) {
                    self.memory
                        .ensure_page(addr.try_into().unwrap())
                        .expect("out of memory");
                }
            } else {
                self.memory.write_bytes(
                    &program[sh.sh_offset as usize..(sh.sh_offset + sh.sh_size) as usize],
                    sh.sh_addr.try_into().unwrap(),
                );
            }
        }

        let satp = self.memory.satp;

        // Create the argument block and shove it at the top of stack.
        let param_block = Self::create_params().expect("failed to create argument block");
        let param_block_start = STACK_END - param_block.len() as u32;
        self.memory.write_bytes(&param_block, param_block_start);
        // Place the argument block into $a1
        cpu.write_register(11, param_block_start as i32);

        // Ensure stack is allocated
        for page in (STACK_START..STACK_END).step_by(4096) {
            self.memory.ensure_page(page).expect("out of memory");
        }

        cpu.write_csr(riscv_cpu::cpu::CSR_SATP_ADDRESS, satp)
            .map_err(|_| LoadError::SatpWriteError)?;
        cpu.update_pc(elf.entry as u32);

        // Return to User Mode (0 << 11) with interrupts disabled (1 << 5)
        cpu.write_csr(riscv_cpu::cpu::CSR_MSTATUS_ADDRESS, 1 << 5)
            .map_err(|_| LoadError::MstatusWriteError)?;

        cpu.write_csr(riscv_cpu::cpu::CSR_SEPC_ADDRESS, elf.entry as u32)
            .unwrap();

        // SRET to return to user mode
        cpu.execute_opcode(0x10200073).map_err(LoadError::CpuTrap)?;

        // Update the stack pointer
        cpu.write_register(2, (STACK_END as i32 - 16 - param_block.len() as i32) & !0xf);

        let memory = self.memory.clone();
        std::thread::spawn(move || {
            std::process::exit(Worker::new(cpu, 0, memory).run() as i32);
        });

        self.satp = satp;

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while let Ok(msg) = self.memory_cmd.recv() {
            match msg {
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
                    let mut cpu = riscv_cpu::CpuBuilder::new(self.memory.clone()).build();
                    let tid = self.thread_id_counter.fetch_add(1, Ordering::SeqCst);
                    cpu.write_csr(riscv_cpu::cpu::CSR_MHARTID_ADDRESS, tid as u32)
                        .unwrap();

                    cpu.write_csr(riscv_cpu::cpu::CSR_SATP_ADDRESS, self.satp)
                        .map_err(|_| LoadError::SatpWriteError)?;
                    cpu.update_pc(entry_point);

                    // Return to User Mode (0 << 11) with interrupts disabled (1 << 5)
                    cpu.write_csr(riscv_cpu::cpu::CSR_MSTATUS_ADDRESS, 1 << 5)
                        .map_err(|_| LoadError::MstatusWriteError)?;

                    cpu.write_csr(riscv_cpu::cpu::CSR_SEPC_ADDRESS, entry_point)
                        .unwrap();

                    // SRET to return to user mode
                    cpu.execute_opcode(0x10200073).map_err(LoadError::CpuTrap)?;

                    // Update the stack pointer
                    cpu.write_register(2, (stack_pointer + stack_length) as i32 - 16);
                    cpu.write_register(10, argument_1 as i32);
                    cpu.write_register(11, argument_2 as i32);
                    cpu.write_register(12, argument_3 as i32);
                    cpu.write_register(13, argument_4 as i32);

                    // let cmd = self.memory_cmd_sender.clone();
                    let memory = self.memory.clone();
                    let join_handle =
                        std::thread::spawn(move || Worker::new(cpu, tid, memory).run());
                    tx.send((tid, join_handle)).unwrap();
                }
            }
        }
        println!("Done! memory_cmd returned error");

        Ok(())
    }
}
