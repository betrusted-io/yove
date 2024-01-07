use super::*;
const MEMORY_BASE: u64 = 0x8000_0000;

fn create_cpu(memory_capacity: usize) -> Cpu {
    let memory = memory::Memory::new(memory_capacity, MEMORY_BASE as usize);
    Cpu::new(Arc::new(Mutex::new(memory)))
}

#[test]
fn initialize() {
    let _cpu = create_cpu(0);
}

#[test]
fn update_pc() {
    let mut cpu = create_cpu(0);
    assert_eq!(0, cpu.read_pc());
    cpu.update_pc(1);
    assert_eq!(1, cpu.read_pc());
    cpu.update_pc(0xffffffffffffffff);
    assert_eq!(0xffffffffffffffff, cpu.read_pc());
}

#[test]
fn update_xlen() {
    let mut cpu = create_cpu(0);
    assert!(matches!(cpu.xlen, Xlen::Bit64));
    cpu.update_xlen(Xlen::Bit32);
    assert!(matches!(cpu.xlen, Xlen::Bit32));
    cpu.update_xlen(Xlen::Bit64);
    assert!(matches!(cpu.xlen, Xlen::Bit64));
    // Note: cpu.update_xlen() updates cpu.mmu.xlen, too.
    // The test for mmu.xlen should be in Mmu?
}

#[test]
fn read_register() {
    let mut cpu = create_cpu(0);
    // Initial register values are 0 other than 0xb th register.
    // Initial value of 0xb th register is temporal for Linux boot and
    // I'm not sure if the value is correct. Then skipping so far.
    for i in 0..31 {
        if i != 0xb {
            assert_eq!(0, cpu.read_register(i));
        }
    }

    for i in 0..31 {
        cpu.x[i] = i as i64 + 1;
    }

    for i in 0..31 {
        match i {
            // 0th register is hardwired zero
            0 => assert_eq!(0, cpu.read_register(i)),
            _ => assert_eq!(i as i64 + 1, cpu.read_register(i)),
        }
    }

    for i in 0..31 {
        cpu.x[i] = (0xffffffffffffffff - i) as i64;
    }

    for i in 0..31 {
        match i {
            // 0th register is hardwired zero
            0 => assert_eq!(0, cpu.read_register(i)),
            _ => assert_eq!(-(i as i64 + 1), cpu.read_register(i)),
        }
    }

    // @TODO: Should I test the case where the argument equals to or is
    // greater than 32?
}

#[test]
fn tick() {
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    cpu.update_pc(memory_base);

    // Write non-compressed "addi x1, x1, 1" instruction
    match cpu.get_mut_mmu().store_word(memory_base, 0x00108093) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    // Write compressed "addi x8, x0, 8" instruction
    match cpu.get_mut_mmu().store_word(memory_base + 4, 0x20) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };

    cpu.tick();

    assert_eq!(memory_base + 4, cpu.read_pc());
    assert_eq!(1, cpu.read_register(1));

    cpu.tick();

    assert_eq!(memory_base + 6, cpu.read_pc());
    assert_eq!(8, cpu.read_register(8));
}

#[test]
fn tick_operate() {
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    cpu.update_pc(memory_base);
    // write non-compressed "addi a0, a0, 12" instruction
    match cpu.get_mut_mmu().store_word(memory_base, 0xc50513) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    assert_eq!(memory_base, cpu.read_pc());
    assert_eq!(0, cpu.read_register(10));
    match cpu.tick_operate() {
        Ok(()) => {}
        Err(_e) => panic!("tick_operate() unexpectedly did panic"),
    };
    // .tick_operate() increments the program counter by 4 for
    // non-compressed instruction.
    assert_eq!(memory_base + 4, cpu.read_pc());
    // "addi a0, a0, a12" instruction writes 12 to a0 register.
    assert_eq!(12, cpu.read_register(10));
    // @TODO: Test compressed instruction operation
}

#[test]
fn fetch() {
    // .fetch() reads four bytes from the memory
    // at the address the program counter points to.
    // .fetch() doesn't increment the program counter.
    // .tick_operate() does.
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    cpu.update_pc(memory_base);
    match cpu.get_mut_mmu().store_word(memory_base, 0xaaaaaaaa) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    match cpu.fetch() {
        Ok(data) => assert_eq!(0xaaaaaaaa, data),
        Err(_e) => panic!("Failed to fetch"),
    };
    match cpu.get_mut_mmu().store_word(memory_base, 0x55555555) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    match cpu.fetch() {
        Ok(data) => assert_eq!(0x55555555, data),
        Err(_e) => panic!("Failed to fetch"),
    };
    // @TODO: Write test cases where Trap happens
}

#[test]
fn decode_raw() {
    let cpu = create_cpu(0);
    // 0x13 is addi instruction
    match cpu.decode_raw(0x13) {
        Ok(inst) => assert_eq!(inst.name, "ADDI"),
        Err(_e) => panic!("Failed to decode"),
    };
    // .decode_raw() returns error for invalid word data.
    match cpu.decode_raw(0x0) {
        Ok(_inst) => panic!("Unexpectedly succeeded in decoding"),
        Err(_trap) => assert!(true),
    };
    // @TODO: Should I test all instructions?
}

#[test]
fn uncompress() {
    let cpu = create_cpu(0);
    // .uncompress() doesn't directly return an instruction but
    // it returns uncompressed word. Then you need to call .decode_raw().
    match cpu.decode_raw(cpu.uncompress(0x20)) {
        Ok(inst) => assert_eq!(inst.name, "ADDI"),
        Err(_e) => panic!("Failed to decode"),
    };
    // @TODO: Should I test all compressed instructions?
}

#[test]
fn wfi() {
    let wfi_instruction = 0x10500073;
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    // Just in case
    match cpu.decode_raw(wfi_instruction) {
        Ok(inst) => assert_eq!(inst.name, "WFI"),
        Err(_e) => panic!("Failed to decode"),
    };
    cpu.update_pc(memory_base);
    // write WFI instruction
    match cpu.get_mut_mmu().store_word(memory_base, wfi_instruction) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    cpu.tick();
    assert_eq!(memory_base + 4, cpu.read_pc());
    for _i in 0..10 {
        // Until interrupt happens, .tick() does nothing
        // @TODO: Check accurately that the state is unchanged
        cpu.tick();
        assert_eq!(memory_base + 4, cpu.read_pc());
    }
    // Machine timer interrupt
    cpu.write_csr_raw(CSR_MIE_ADDRESS, MIP_MTIP);
    cpu.write_csr_raw(CSR_MIP_ADDRESS, MIP_MTIP);
    cpu.write_csr_raw(CSR_MSTATUS_ADDRESS, 0x8);
    cpu.write_csr_raw(CSR_MTVEC_ADDRESS, 0x0);
    cpu.tick();
    // Interrupt happened and moved to handler
    assert_eq!(0, cpu.read_pc());
}

#[test]
fn interrupt() {
    let handler_vector = 0x10000000;
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    // Write non-compressed "addi x0, x0, 1" instruction
    match cpu.get_mut_mmu().store_word(memory_base, 0x00100013) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    cpu.update_pc(memory_base);

    // Machine timer interrupt but mie in mstatus is not enabled yet
    cpu.write_csr_raw(CSR_MIE_ADDRESS, MIP_MTIP);
    cpu.write_csr_raw(CSR_MIP_ADDRESS, MIP_MTIP);
    cpu.write_csr_raw(CSR_MTVEC_ADDRESS, handler_vector);

    cpu.tick();

    // Interrupt isn't caught because mie is disabled
    assert_eq!(memory_base + 4, cpu.read_pc());

    cpu.update_pc(memory_base);
    // Enable mie in mstatus
    cpu.write_csr_raw(CSR_MSTATUS_ADDRESS, 0x8);

    cpu.tick();

    // Interrupt happened and moved to handler
    assert_eq!(handler_vector, cpu.read_pc());

    // CSR Cause register holds the reason what caused the interrupt
    assert_eq!(0x8000000000000007, cpu.read_csr_raw(CSR_MCAUSE_ADDRESS));

    // @TODO: Test post CSR status register
    // @TODO: Test xIE bit in CSR status register
    // @TODO: Test privilege levels
    // @TODO: Test delegation
    // @TODO: Test vector type handlers
}

#[test]
fn syscall() {
    let handler_vector = 0x10000000;
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    // Write ECALL instruction
    match cpu.get_mut_mmu().store_word(memory_base, 0x00000073) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    cpu.write_csr_raw(CSR_MTVEC_ADDRESS, handler_vector);
    cpu.update_pc(memory_base);

    cpu.tick();

    // // Interrupt happened and moved to handler
    // assert_eq!(handler_vector, cpu.read_pc());

    // // CSR Cause register holds the reason what caused the trap
    // assert_eq!(0xb, cpu.read_csr_raw(CSR_MCAUSE_ADDRESS));

    // @TODO: Test post CSR status register
    // @TODO: Test privilege levels
    // @TODO: Test delegation
    // @TODO: Test vector type handlers
}

#[test]
fn hardocded_zero() {
    let mut cpu = create_cpu(8);
    let memory_base = MEMORY_BASE;
    cpu.update_pc(memory_base);

    // Write non-compressed "addi x0, x0, 1" instruction
    match cpu.get_mut_mmu().store_word(memory_base, 0x00100013) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };
    // Write non-compressed "addi x1, x1, 1" instruction
    match cpu.get_mut_mmu().store_word(memory_base + 4, 0x00108093) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };

    // Test x0
    assert_eq!(0, cpu.read_register(0));
    cpu.tick(); // Execute  "addi x0, x0, 1"
                // x0 is still zero because it's hardcoded zero
    assert_eq!(0, cpu.read_register(0));

    // Test x1
    assert_eq!(0, cpu.read_register(1));
    cpu.tick(); // Execute  "addi x1, x1, 1"
                // x1 is not hardcoded zero
    assert_eq!(1, cpu.read_register(1));
}

#[test]
fn disassemble_next_instruction() {
    let mut cpu = create_cpu(4);
    let memory_base = MEMORY_BASE;
    cpu.update_pc(memory_base);

    // Write non-compressed "addi x0, x0, 1" instruction
    match cpu.get_mut_mmu().store_word(memory_base, 0x00100013) {
        Ok(()) => {}
        Err(_e) => panic!("Failed to store"),
    };

    assert_eq!(
        "PC:0000000080000000 00100013 ADDI zero:0,zero:0,1",
        cpu.disassemble_next_instruction()
    );

    // No effect to PC
    assert_eq!(memory_base, cpu.read_pc());
}
