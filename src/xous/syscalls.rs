use std::sync::mpsc::channel;

use super::super::xous::services::get_service;
use super::definitions::{SyscallErrorNumber, SyscallResultNumber};
use super::services;
use super::Memory;
use super::SyscallResult;
use riscv_cpu::cpu::Memory as OtherMemory;

pub fn map_memory(
    memory: &mut Memory,
    phys: i64,
    virt: i64,
    size: i64,
    _flags: i64,
) -> SyscallResult {
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
    if let Some(region) = memory.allocate_virt_region(size as usize) {
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
    } else {
        // self.print_mmu();
        println!(
            "Couldn't find a free spot to allocate {} bytes of virtual memory, or out of memory",
            size as usize
        );
        [
            SyscallResultNumber::Error as i64,
            SyscallErrorNumber::OutOfMemory as i64,
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

pub fn connect(memory: &mut Memory, id: [u32; 4]) -> SyscallResult {
    // println!(
    //     "Connect([0x{:08x}, 0x{:08x}, 0x{:08x}, 0x{:08x}])",
    //     id[0], id[1], id[2], id[3]
    // );
    if let Some(service) = get_service(&id) {
        let connection_id = memory.connections.len() as u32 + 1;
        memory.connections.insert(connection_id, service);
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

pub fn try_connect(memory: &mut Memory, id: [u32; 4]) -> SyscallResult {
    connect(memory, id)
}

pub fn send_message(
    memory: &mut Memory,
    connection_id: u32,
    kind: u32,
    opcode: u32,
    args: [u32; 4],
) -> SyscallResult {
    // println!(
    //     "SendMessage({}, {}, {}: {:x?})",
    //     connection_id, kind, opcode, args
    // );
    let memory_region = if kind == 1 || kind == 2 || kind == 3 {
        let mut memory_region = vec![0; args[1] as usize];
        for (offset, value) in memory_region.iter_mut().enumerate() {
            *value = memory.read_u8(
                memory
                    .virt_to_phys(args[0] + offset as u32)
                    .expect("invalid memory address") as u64,
            );
        }
        Some(memory_region)
    } else {
        None
    };
    // Pull the service out of the connections table so that we can send
    // a mutable copy of the memory object to the service.
    let Some(mut service) = memory.connections.remove(&connection_id) else {
        println!("Unhandled connection ID {}", connection_id);
        return [
            SyscallResultNumber::Error as i64,
            SyscallErrorNumber::ServerNotFound as i64,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
        .into();
    };
    let response = match kind {
        1..=3 => {
            let mut memory_region = memory_region.unwrap();
            let extra = [args[2], args[3]];
            match kind {
                1 => match service.lend_mut(memory, 0, opcode, &mut memory_region, extra) {
                    services::LendResult::WaitForResponse(msg) => msg.into(),
                    services::LendResult::MemoryReturned(result) => {
                        for (offset, value) in memory_region.into_iter().enumerate() {
                            memory.write_u8(args[0] as u64 + offset as u64, value);
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
                2 => match service.lend(memory, 0, opcode, &memory_region, extra) {
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
                    service.send(memory, 0, opcode, &memory_region, extra);
                    [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into()
                }
                _ => unreachable!(),
            }
        }
        4 => {
            service.scalar(memory, 0, opcode, args);
            [SyscallResultNumber::Ok as i64, 0, 0, 0, 0, 0, 0, 0].into()
        }
        5 => match service.blocking_scalar(memory, 0, opcode, args) {
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
            panic!("Unknown message kind {}", kind);
            // [
            //     SyscallResultNumber::Error as i64,
            //     9, /* ServerNotFound */
            //     0,
            //     0,
            //     0,
            //     0,
            //     0,
            //     0,
            // ]
            // .into()
        }
    };
    memory.connections.insert(connection_id, service);
    response
}

pub fn try_send_message(
    memory: &mut Memory,
    connection_id: u32,
    kind: u32,
    opcode: u32,
    args: [u32; 4],
) -> SyscallResult {
    send_message(memory, connection_id, kind, opcode, args)
}

pub fn increase_heap(memory: &mut Memory, delta: i64, _flags: i64) -> SyscallResult {
    assert!(delta & 0xfff == 0, "delta must be page-aligned");
    let increase_bytes = delta as u32;
    let heap_address = memory.heap_start + memory.heap_size;
    if delta == 0 {
        return [
            SyscallResultNumber::MemoryRange as i64,
            memory.heap_start as i64,
            if memory.heap_size == 0 {
                4096
            } else {
                memory.heap_size
            } as i64,
            0,
            0,
            0,
            0,
            0,
        ]
        .into();
    }
    if heap_address.saturating_add(increase_bytes) > super::HEAP_END {
        [
            SyscallResultNumber::Error as i64,
            SyscallErrorNumber::OutOfMemory as i64,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
        .into()
    } else {
        for new_address in (heap_address..(heap_address + increase_bytes)).step_by(4096) {
            memory.ensure_page(new_address);
        }
        let new_heap_region = memory.heap_start + memory.heap_size;
        memory.heap_size += increase_bytes;
        [
            SyscallResultNumber::MemoryRange as i64,
            new_heap_region as i64,
            delta,
            0,
            0,
            0,
            0,
            0,
        ]
        .into()
    }
}

pub fn create_thread(
    memory: &mut Memory,
    entry_point: i64,
    stack_pointer: i64,
    stack_length: i64,
    arguments: [i64; 4],
) -> SyscallResult {
    let (tx, rx) = channel();
    memory
        .memory_cmd
        .send(super::MemoryCommand::CreateThread(
            entry_point as _,
            stack_pointer as _,
            stack_length as _,
            arguments[0] as _,
            arguments[1] as _,
            arguments[2] as _,
            arguments[3] as _,
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
