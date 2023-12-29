fn main() {
    let mut cpu = riscv_cpu::CpuBuilder::new()
        .memory_size(64 * 1024 * 1024)
        .xlen(riscv_cpu::Xlen::Bit32)
        .build();
    println!("Hello, world!");
}
