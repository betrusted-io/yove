#![allow(dead_code)]

#[derive(Debug, Copy, Clone)]
pub enum SyscallResultNumber {
    Ok = 0,
    Error = 1,
    MemoryRange = 3,
    ConnectionId = 7,
    Message = 9,
    ThreadId = 10,
    Unimplemented = 12,
    Scalar1 = 14,
    Scalar2 = 15,
    MemoryReturned = 18,
    Scalar5 = 20,
}

#[derive(Debug)]
pub enum Syscall {
    Unknown([i64; 8]),
    IncreaseHeap(
        i64, /* number of bytes to add */
        i64, /* memory flags */
    ),
    MapMemory(
        i64, /* address */
        i64, /* size */
        i64, /* flags */
        i64, /* name */
    ),
}

#[derive(Debug)]
pub enum SyscallNumber {
    MapMemory = 2,
    Yield = 3,
    IncreaseHeap = 10,
    UpdateMemoryFlags = 12,
    ReceiveMessage = 15,
    SendMessage = 16,
    Connect = 17,
    CreateThread = 18,
    UnmapMemory = 19,
    ReturnMemory = 20,
    TerminateProcess = 22,
    TrySendMessage = 24,
    TryConnect = 25,
    GetThreadId = 32,
    JoinThread = 36,
    AdjustProcessLimit = 38,
    ReturnScalar = 40,
    Unknown = 0,
}

impl From<[i64; 8]> for Syscall {
    fn from(value: [i64; 8]) -> Self {
        match value[0].into() {
            SyscallNumber::IncreaseHeap => Syscall::IncreaseHeap(value[1], value[2]),
            SyscallNumber::MapMemory => Syscall::MapMemory(value[1], value[2], value[3], value[4]),
            _ => Syscall::Unknown(value),
        }
    }
}

impl From<i64> for SyscallNumber {
    fn from(value: i64) -> Self {
        match value {
            2 => SyscallNumber::MapMemory,
            3 => SyscallNumber::Yield,
            10 => SyscallNumber::IncreaseHeap,
            12 => SyscallNumber::UpdateMemoryFlags,
            15 => SyscallNumber::ReceiveMessage,
            16 => SyscallNumber::SendMessage,
            17 => SyscallNumber::Connect,
            18 => SyscallNumber::CreateThread,
            19 => SyscallNumber::UnmapMemory,
            20 => SyscallNumber::ReturnMemory,
            22 => SyscallNumber::TerminateProcess,
            24 => SyscallNumber::TrySendMessage,
            25 => SyscallNumber::TryConnect,
            32 => SyscallNumber::GetThreadId,
            36 => SyscallNumber::JoinThread,
            38 => SyscallNumber::AdjustProcessLimit,
            40 => SyscallNumber::ReturnScalar,
            _ => SyscallNumber::Unknown,
        }
    }
}
