#![allow(dead_code)]

pub mod memoryflags;

#[derive(Debug, Copy, Clone)]
pub enum Message {
    MutableBorrow = 0,
    Borrow = 1,
    Move = 2,
    Scalar = 3,
    BlockingScalar = 4,
}

#[derive(Debug, Copy, Clone)]
pub enum SyscallResultNumber {
    Ok = 0,
    Error = 1,
    MemoryRange = 3,
    ConnectionId = 7,
    Message = 9,
    ThreadId = 10,
    ProcessId = 11,
    Unimplemented = 12,
    Scalar1 = 14,
    Scalar2 = 15,
    MemoryReturned = 18,
    Scalar5 = 20,
}

#[derive(Debug, Copy, Clone)]
pub enum SyscallErrorNumber {
    NoError = 0,
    BadAlignment = 1,
    BadAddress = 2,
    OutOfMemory = 3,
    MemoryInUse = 4,
    InterruptNotFound = 5,
    InterruptInUse = 6,
    InvalidString = 7,
    ServerExists = 8,
    ServerNotFound = 9,
    ProcessNotFound = 10,
    ProcessNotChild = 11,
    ProcessTerminated = 12,
    Timeout = 13,
    InternalError = 14,
    ServerQueueFull = 15,
    ThreadNotAvailable = 16,
    UnhandledSyscall = 17,
    InvalidSyscall = 18,
    ShareViolation = 19,
    InvalidThread = 20,
    InvalidPID = 21,
    UnknownError = 22,
    AccessDenied = 23,
    UseBeforeInit = 24,
    DoubleFree = 25,
    DebugInProgress = 26,
    InvalidLimit = 27,
}

#[derive(Debug)]
pub enum Syscall {
    Unknown([i32; 8]),
    Yield,
    IncreaseHeap(
        i32, /* number of bytes to add */
        i32, /* memory flags */
    ),
    MapMemory(
        i32, /* address */
        i32, /* size */
        i32, /* flags */
        i32, /* name */
    ),
    Connect([u32; 4] /* Server ID */),
    TryConnect([u32; 4] /* Server ID */),
    SendMessage(
        u32,      /* Connection ID */
        u32,      /* message kind */
        u32,      /* opcode */
        [u32; 4], /* descriptor */
    ),
    TrySendMessage(
        u32,      /* Connection ID */
        u32,      /* message kind */
        u32,      /* opcode */
        [u32; 4], /* descriptor */
    ),
    UpdateMemoryFlags(
        i32, /* address */
        i32, /* range */
        i32, /* flags */
    ),
    CreateThread(
        i32, /* entry point */
        i32, /* stack pointer */
        i32, /* stack length */
        i32, /* argument 1 */
        i32, /* argument 2 */
        i32, /* argument 3 */
        i32, /* argument 4 */
    ),
    JoinThread(i32 /* thread ID */),
    UnmapMemory(i32, /* address */ i32 /* size */),
    TerminateProcess(i32 /* Exit code */),
    GetProcessId,
}

#[derive(Debug)]
pub enum SyscallNumber {
    MapMemory = 2,
    Yield = 3,
    ReturnToParent = 4,
    ClaimInterrupt = 5,
    FreeInterrupt = 6,
    SwitchTo = 7,
    ReadyThreads = 8,
    WaitEvent = 9,
    IncreaseHeap = 10,
    DecreaseHeap = 11,
    UpdateMemoryFlags = 12,
    SetMemRegion = 13,
    CreateServerWithAddress = 14,
    ReceiveMessage = 15,
    SendMessage = 16,
    Connect = 17,
    CreateThread = 18,
    UnmapMemory = 19,
    ReturnMemory = 20,
    CreateProcess = 21,
    TerminateProcess = 22,
    Shutdown = 23,
    TrySendMessage = 24,
    TryConnect = 25,
    ReturnScalar1 = 26,
    ReturnScalar2 = 27,
    TryReceiveMessage = 28,
    CreateServer = 29,
    ConnectForProcess = 30,
    CreateServerId = 31,
    GetThreadId = 32,
    GetProcessId = 33,
    DestroyServer = 34,
    Disconnect = 35,
    JoinThread = 36,
    SetExceptionHandler = 37,
    AdjustProcessLimit = 38,
    VirtToPhys = 39,
    ReturnScalar = 40,
    ReplyAndReceiveNext = 41,
    VirtToPhysPid = 42,
    Unknown = 0,
}

impl From<[i32; 8]> for Syscall {
    fn from(value: [i32; 8]) -> Self {
        match value[0].into() {
            SyscallNumber::IncreaseHeap => Syscall::IncreaseHeap(value[1], value[2]),
            SyscallNumber::MapMemory => Syscall::MapMemory(value[1], value[2], value[3], value[4]),
            SyscallNumber::UnmapMemory => Syscall::UnmapMemory(value[1], value[2]),
            SyscallNumber::Connect => Syscall::Connect([
                value[1] as u32,
                value[2] as u32,
                value[3] as u32,
                value[4] as u32,
            ]),
            SyscallNumber::TryConnect => Syscall::TryConnect([
                value[1] as u32,
                value[2] as u32,
                value[3] as u32,
                value[4] as u32,
            ]),
            SyscallNumber::SendMessage => Syscall::SendMessage(
                value[1] as u32,
                value[2] as u32,
                value[3] as u32,
                [
                    value[4] as u32,
                    value[5] as u32,
                    value[6] as u32,
                    value[7] as u32,
                ],
            ),
            SyscallNumber::TrySendMessage => Syscall::TrySendMessage(
                value[1] as u32,
                value[2] as u32,
                value[3] as u32,
                [
                    value[4] as u32,
                    value[5] as u32,
                    value[6] as u32,
                    value[7] as u32,
                ],
            ),
            SyscallNumber::UpdateMemoryFlags => {
                Syscall::UpdateMemoryFlags(value[1], value[2], value[3])
            }
            SyscallNumber::CreateThread => Syscall::CreateThread(
                value[1], value[2], value[3], value[4], value[5], value[6], value[7],
            ),
            SyscallNumber::Yield => Syscall::Yield,
            SyscallNumber::JoinThread => Syscall::JoinThread(value[1]),
            SyscallNumber::TerminateProcess => Syscall::TerminateProcess(value[1]),
            SyscallNumber::GetProcessId => Syscall::GetProcessId,
            _ => Syscall::Unknown(value),
        }
    }
}

impl From<i32> for SyscallNumber {
    fn from(value: i32) -> Self {
        match value {
            2 => SyscallNumber::MapMemory,
            3 => SyscallNumber::Yield,
            4 => SyscallNumber::ReturnToParent,
            5 => SyscallNumber::ClaimInterrupt,
            6 => SyscallNumber::FreeInterrupt,
            7 => SyscallNumber::SwitchTo,
            8 => SyscallNumber::ReadyThreads,
            9 => SyscallNumber::WaitEvent,
            10 => SyscallNumber::IncreaseHeap,
            11 => SyscallNumber::DecreaseHeap,
            12 => SyscallNumber::UpdateMemoryFlags,
            13 => SyscallNumber::SetMemRegion,
            14 => SyscallNumber::CreateServerWithAddress,
            15 => SyscallNumber::ReceiveMessage,
            16 => SyscallNumber::SendMessage,
            17 => SyscallNumber::Connect,
            18 => SyscallNumber::CreateThread,
            19 => SyscallNumber::UnmapMemory,
            20 => SyscallNumber::ReturnMemory,
            21 => SyscallNumber::CreateProcess,
            22 => SyscallNumber::TerminateProcess,
            23 => SyscallNumber::Shutdown,
            24 => SyscallNumber::TrySendMessage,
            25 => SyscallNumber::TryConnect,
            26 => SyscallNumber::ReturnScalar1,
            27 => SyscallNumber::ReturnScalar2,
            28 => SyscallNumber::TryReceiveMessage,
            29 => SyscallNumber::CreateServer,
            30 => SyscallNumber::ConnectForProcess,
            31 => SyscallNumber::CreateServerId,
            32 => SyscallNumber::GetThreadId,
            33 => SyscallNumber::GetProcessId,
            34 => SyscallNumber::DestroyServer,
            35 => SyscallNumber::Disconnect,
            36 => SyscallNumber::JoinThread,
            37 => SyscallNumber::SetExceptionHandler,
            38 => SyscallNumber::AdjustProcessLimit,
            39 => SyscallNumber::VirtToPhys,
            40 => SyscallNumber::ReturnScalar,
            41 => SyscallNumber::ReplyAndReceiveNext,
            42 => SyscallNumber::VirtToPhysPid,
            _ => SyscallNumber::Unknown,
        }
    }
}
