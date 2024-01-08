use super::*;
const MEMORY_BASE: u64 = 0x8000_0000;

fn create_cpu(memory_capacity: usize) -> (Cpu, Arc<Mutex<memory::Memory>>) {
    let memory = Arc::new(Mutex::new(memory::Memory::new(
        memory_capacity,
        MEMORY_BASE as usize,
        0x8000_1000,
    )));
    (Cpu::new(memory.clone()), memory)
}

#[test]
fn initialize() {
    let _cpu = create_cpu(0).0;
}

#[test]
fn update_pc() {
    let mut cpu = create_cpu(0).0;
    assert_eq!(0, cpu.read_pc());
    cpu.update_pc(1);
    assert_eq!(1, cpu.read_pc());
    cpu.update_pc(0xffffffffffffffff);
    assert_eq!(0xffffffffffffffff, cpu.read_pc());
}

#[test]
fn update_xlen() {
    let mut cpu = create_cpu(0).0;
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
    let mut cpu = create_cpu(0).0;
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
    let mut cpu = create_cpu(4).0;
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
    let mut cpu = create_cpu(4).0;
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
    let mut cpu = create_cpu(4).0;
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
    let cpu = create_cpu(0).0;
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
    let cpu = create_cpu(0).0;
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
    let mut cpu = create_cpu(4).0;
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
    let mut cpu = create_cpu(4).0;
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
    let mut cpu = create_cpu(4).0;
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
    let mut cpu = create_cpu(8).0;
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
    let mut cpu = create_cpu(4).0;
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

fn load_elf(cpu: &mut Cpu, program: &[u8]) {
    let goblin::Object::Elf(elf) =
        goblin::Object::parse(program).expect("Failed to parse ELF file")
    else {
        panic!("Test binary is not ELF file")
    };
    if elf.is_64 {
        panic!("Test binary is 64-bits");
    }

    let mmu = cpu.get_mut_mmu();
    for sh in elf.section_headers {
        if sh.sh_flags as u32 & goblin::elf::section_header::SHF_ALLOC == 0 {
            continue;
        }

        if sh.sh_type & goblin::elf::section_header::SHT_NOBITS != 0 {
            for addr in sh.sh_addr..(sh.sh_addr + sh.sh_size) {
                mmu.store_doubleword(addr, 0).unwrap();
            }
        } else {
            for (offset, byte) in program
                [sh.sh_offset as usize..(sh.sh_offset + sh.sh_size) as usize]
                .iter()
                .enumerate()
            {
                mmu.store(sh.sh_addr + offset as u64, *byte).unwrap();
            }
        }
    }

    cpu.update_pc(elf.entry as u64);
}

fn test_program(program: &[u8]) {
    let (mut cpu, memory) = create_cpu(65536);
    cpu.update_xlen(Xlen::Bit32);
    load_elf(&mut cpu, program);

    while memory.lock().unwrap().vm_result().is_none() {
        let pc = cpu.read_pc();
        let result = cpu.tick();
        if let TickResult::CpuTrap(trap) = result {
            println!("CPU trap: {:?}", trap);
            if let Trap {
                trap_type: TrapType::InstructionPageFault,
                ..
            } = trap
            {
                panic!("Instruction page fault: {:?}", trap);
            }
            cpu.handle_trap(trap, pc, false);
        }
    }

    let vm_result = memory.lock().unwrap().vm_result().unwrap();
    println!("VM result: {}", vm_result);

    assert_eq!(vm_result, 1);
}

#[test]
fn rv32ua_p_amoadd_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amoadd_w"));
}

#[test]
fn rv32ua_p_amoand_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amoand_w"));
}

#[test]
fn rv32ua_p_amomax_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amomax_w"));
}

#[test]
fn rv32ua_p_amomaxu_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amomaxu_w"));
}

#[test]
fn rv32ua_p_amomin_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amomin_w"));
}

#[test]
fn rv32ua_p_amominu_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amominu_w"));
}

#[test]
fn rv32ua_p_amoor_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amoor_w"));
}

#[test]
fn rv32ua_p_amoswap_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amoswap_w"));
}

#[test]
fn rv32ua_p_amoxor_w() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-amoxor_w"));
}

#[test]
fn rv32ua_p_lrsc() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ua-p-lrsc"));
}

#[test]
fn rv32uc_p_rvc() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32uc-p-rvc"));
}

#[test]
fn rv32ui_p_add() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-add"));
}

#[test]
fn rv32ui_p_addi() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-addi"));
}

#[test]
fn rv32ui_p_and() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-and"));
}

#[test]
fn rv32ui_p_andi() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-andi"));
}

#[test]
fn rv32ui_p_auipc() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-auipc"));
}

#[test]
fn rv32ui_p_beq() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-beq"));
}

#[test]
fn rv32ui_p_bge() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-bge"));
}

#[test]
fn rv32ui_p_bgeu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-bgeu"));
}

#[test]
fn rv32ui_p_blt() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-blt"));
}

#[test]
fn rv32ui_p_bltu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-bltu"));
}

#[test]
fn rv32ui_p_bne() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-bne"));
}

#[test]
fn rv32ui_p_fence_i() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-fence_i"));
}

#[test]
fn rv32ui_p_jal() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-jal"));
}

#[test]
fn rv32ui_p_jalr() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-jalr"));
}

#[test]
fn rv32ui_p_lb() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-lb"));
}

#[test]
fn rv32ui_p_lbu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-lbu"));
}

#[test]
fn rv32ui_p_lh() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-lh"));
}

#[test]
fn rv32ui_p_lhu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-lhu"));
}

#[test]
fn rv32ui_p_lui() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-lui"));
}

#[test]
fn rv32ui_p_lw() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-lw"));
}

#[test]
fn rv32ui_p_ma_data() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-ma_data"));
}

#[test]
fn rv32ui_p_or() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-or"));
}

#[test]
fn rv32ui_p_ori() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-ori"));
}

#[test]
fn rv32ui_p_sb() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sb"));
}

#[test]
fn rv32ui_p_sh() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sh"));
}

#[test]
fn rv32ui_p_simple() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-simple"));
}

#[test]
fn rv32ui_p_sll() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sll"));
}

#[test]
fn rv32ui_p_slli() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-slli"));
}

#[test]
fn rv32ui_p_slt() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-slt"));
}

#[test]
fn rv32ui_p_slti() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-slti"));
}

#[test]
fn rv32ui_p_sltiu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sltiu"));
}

#[test]
fn rv32ui_p_sltu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sltu"));
}

#[test]
fn rv32ui_p_sra() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sra"));
}

#[test]
fn rv32ui_p_srai() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-srai"));
}

#[test]
fn rv32ui_p_srl() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-srl"));
}

#[test]
fn rv32ui_p_srli() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-srli"));
}

#[test]
fn rv32ui_p_sub() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sub"));
}

#[test]
fn rv32ui_p_sw() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-sw"));
}

#[test]
fn rv32ui_p_xor() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-xor"));
}

#[test]
fn rv32ui_p_xori() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32ui-p-xori"));
}

#[test]
fn rv32um_p_div() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-div"));
}

#[test]
fn rv32um_p_divu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-divu"));
}

#[test]
fn rv32um_p_mul() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-mul"));
}

#[test]
fn rv32um_p_mulh() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-mulh"));
}

#[test]
fn rv32um_p_mulhsu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-mulhsu"));
}

#[test]
fn rv32um_p_mulhu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-mulhu"));
}

#[test]
fn rv32um_p_rem() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-rem"));
}

#[test]
fn rv32um_p_remu() {
    test_program(include_bytes!("../../riscv-tests/isa/rv32um-p-remu"));
}
