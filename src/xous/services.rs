pub mod log;
pub mod ticktimer;

pub enum ScalarResult {
    Scalar1(u32),
    Scalar2([u32; 2]),
    Scalar5([u32; 5]),
}

pub trait Service {
    fn scalar(&mut self, sender: u32, opcode: u32, _args: [u32; 4]) {
        panic!("Unknown scalar to service {}: {}", sender, opcode);
    }
    fn blocking_scalar(&mut self, sender: u32, opcode: u32, _args: [u32; 4]) -> ScalarResult {
        panic!("Unknown scalar to service {}: {}", sender, opcode);
    }
    fn lend(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) -> [u32; 2] {
        panic!("Unknown lend to service {}: {} ({:?})", sender, opcode, extra);
    }
    fn lend_mut(&mut self, sender: u32, opcode: u32, _buf: &mut [u8], extra: [u32; 2]) -> [u32; 2] {
        panic!("Unknown lend_mut to service {}: {} ({:?})", sender, opcode, extra);
    }
    fn send(&mut self, sender: u32, opcode: u32, _buf: &[u8], extra: [u32; 2]) {
        panic!("Unknown send to service {}: {} ({:?})", sender, opcode, extra);
    }
}

pub fn get_service(name: &[u32; 4]) -> Option<Box<dyn Service + Sync + Send>> {
    match name {
        [0x6b636974, 0x656d6974, 0x65732d72, 0x72657672] => {
            Some(Box::new(ticktimer::Ticktimer::new()))
        }
        [0x73756f78, 0x676f6c2d, 0x7265732d, 0x20726576] => Some(Box::new(log::Log::new())),
        _ => None,
    }
}
