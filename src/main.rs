mod xous;

use std::io::Read;
use xous::XousHandler;

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

    for _tick in 0..1000 {
        cpu.tick();
    }
}
