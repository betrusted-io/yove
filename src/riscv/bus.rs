use crate::riscv::exception::Exception;

pub enum XLen {
    X32,
    X64,
}

pub trait Bus<XLen> {
    fn read(&mut self, addr: XLen, size: u8) -> Result<XLen, Exception>;
    fn write(&mut self, addr: XLen, value: XLen, size: u8) -> Result<(), Exception>;
}
