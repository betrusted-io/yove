use super::{
    decode_privilege_mode, Cpu, PrivilegeMode, Trap, TrapType, CSR_MEPC_ADDRESS,
    CSR_MHARTID_ADDRESS, CSR_MSTATUS_ADDRESS, CSR_SEPC_ADDRESS, CSR_SSTATUS_ADDRESS,
};

pub struct Instruction {
    pub mask: u32,
    pub data: u32, // @TODO: rename
    pub name: &'static str,
    pub operation: fn(cpu: &mut Cpu, word: u32, address: u32) -> Result<(), Trap>,
    pub disassemble: fn(cpu: &Cpu, word: u32, address: u32, evaluate: bool) -> String,
}

pub const INSTRUCTION_NUM: usize = 82;

// @TODO: Reorder in often used order as
pub const fn get_instructions() -> [Instruction; INSTRUCTION_NUM] {
    [
        Instruction {
            mask: 0xfe00707f,
            data: 0x00000033,
            name: "ADD",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1].wrapping_add(cpu.x[f.rs2]));
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00000013,
            name: "ADDI",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                // println!(
                //     "ADDI: {:08x} + {:08x} = {:08x}",
                //     cpu.x[f.rs1],
                //     f.imm,
                //     cpu.x[f.rs1].wrapping_add(f.imm)
                // );
                cpu.x[f.rd] = cpu.x[f.rs1].wrapping_add(f.imm);
                Ok(())
            },
            disassemble: dump_format_i,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x0000001b,
            name: "ADDIW",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = cpu.x[f.rs1].wrapping_add(f.imm) as i32;
                Ok(())
            },
            disassemble: dump_format_i,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0000003b,
            name: "ADDW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.x[f.rs1].wrapping_add(cpu.x[f.rs2]) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        // Instruction {
        //     mask: 0xf800707f,
        //     data: 0x0000302f,
        //     name: "AMOADD.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let tmp = match cpu.mmu.load_doubleword(cpu.x[f.rs1] as u64) {
        //             Ok(data) => data as i64,
        //             Err(e) => return Err(e),
        //         };
        //         match cpu
        //             .mmu
        //             .store_doubleword(cpu.x[f.rs1] as u64, cpu.x[f.rs2].wrapping_add(tmp) as u64)
        //         {
        //             Ok(()) => {}
        //             Err(e) => return Err(e),
        //         };
        //         cpu.x[f.rd] = tmp;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0xf800707f,
            data: 0x0000202f,
            name: "AMOADD.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let tmp = match cpu.mmu.load_word(cpu.x[f.rs1] as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                match cpu
                    .mmu
                    .store_word(cpu.x[f.rs1] as u32, cpu.x[f.rs2].wrapping_add(tmp) as u32)
                {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = tmp;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        // Instruction {
        //     mask: 0xf800707f,
        //     data: 0x6000302f,
        //     name: "AMOAND.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let tmp = match cpu.mmu.load_doubleword(cpu.x[f.rs1] as u64) {
        //             Ok(data) => data as i64,
        //             Err(e) => return Err(e),
        //         };
        //         match cpu
        //             .mmu
        //             .store_doubleword(cpu.x[f.rs1] as u64, (cpu.x[f.rs2] & tmp) as u64)
        //         {
        //             Ok(()) => {}
        //             Err(e) => return Err(e),
        //         };
        //         cpu.x[f.rd] = tmp;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0xf800707f,
            data: 0x6000202f,
            name: "AMOAND.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let tmp = match cpu.mmu.load_word(cpu.x[f.rs1] as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                match cpu
                    .mmu
                    .store_word(cpu.x[f.rs1] as u32, (cpu.x[f.rs2] & tmp) as u32)
                {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = tmp;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        // Instruction {
        //     mask: 0xf800707f,
        //     data: 0xe000302f,
        //     name: "AMOMAXU.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let tmp = match cpu.mmu.load_doubleword(cpu.x[f.rs1] as u64) {
        //             Ok(data) => data,
        //             Err(e) => return Err(e),
        //         };
        //         let max = match cpu.x[f.rs2] as u64 >= tmp {
        //             true => cpu.x[f.rs2] as u64,
        //             false => tmp,
        //         };
        //         match cpu.mmu.store_doubleword(cpu.x[f.rs1] as u64, max) {
        //             Ok(()) => {}
        //             Err(e) => return Err(e),
        //         };
        //         cpu.x[f.rd] = tmp as i64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0xf800707f,
            data: 0xe000202f,
            name: "AMOMAXU.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let tmp = match cpu.mmu.load_word(cpu.x[f.rs1] as u32) {
                    Ok(data) => data,
                    Err(e) => return Err(e),
                };
                let max = match cpu.x[f.rs2] as u32 >= tmp {
                    true => cpu.x[f.rs2] as u32,
                    false => tmp,
                };
                match cpu.mmu.store_word(cpu.x[f.rs1] as u32, max) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = tmp as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        // Instruction {
        //     mask: 0xf800707f,
        //     data: 0x4000302f,
        //     name: "AMOOR.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let tmp = match cpu.mmu.load_doubleword(cpu.x[f.rs1] as u64) {
        //             Ok(data) => data as i64,
        //             Err(e) => return Err(e),
        //         };
        //         match cpu
        //             .mmu
        //             .store_doubleword(cpu.x[f.rs1] as u64, (cpu.x[f.rs2] | tmp) as u64)
        //         {
        //             Ok(()) => {}
        //             Err(e) => return Err(e),
        //         };
        //         cpu.x[f.rd] = tmp;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0xf800707f,
            data: 0x4000202f,
            name: "AMOOR.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let tmp = match cpu.mmu.load_word(cpu.x[f.rs1] as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                match cpu
                    .mmu
                    .store_word(cpu.x[f.rs1] as u32, (cpu.x[f.rs2] | tmp) as u32)
                {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = tmp;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        // Instruction {
        //     mask: 0xf800707f,
        //     data: 0x0800302f,
        //     name: "AMOSWAP.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let tmp = match cpu.mmu.load_doubleword(cpu.x[f.rs1] as u64) {
        //             Ok(data) => data as i64,
        //             Err(e) => return Err(e),
        //         };
        //         match cpu
        //             .mmu
        //             .store_doubleword(cpu.x[f.rs1] as u64, cpu.x[f.rs2] as u64)
        //         {
        //             Ok(()) => {}
        //             Err(e) => return Err(e),
        //         };
        //         cpu.x[f.rd] = tmp;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0xf800707f,
            data: 0x0800202f,
            name: "AMOSWAP.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let tmp = match cpu.mmu.load_word(cpu.x[f.rs1] as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                match cpu.mmu.store_word(cpu.x[f.rs1] as u32, cpu.x[f.rs2] as u32) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = tmp;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00007033,
            name: "AND",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] & cpu.x[f.rs2]);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00007013,
            name: "ANDI",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] & f.imm);
                Ok(())
            },
            disassemble: dump_format_i,
        },
        Instruction {
            mask: 0x0000007f,
            data: 0x00000017,
            name: "AUIPC",
            operation: |cpu, word, address| {
                let f = parse_format_u(word);
                cpu.x[f.rd] = cpu.sign_extend(address.wrapping_add(f.imm) as i32);
                Ok(())
            },
            disassemble: dump_format_u,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00000063,
            name: "BEQ",
            operation: |cpu, word, address| {
                let f = parse_format_b(word);
                if cpu.sign_extend(cpu.x[f.rs1]) == cpu.sign_extend(cpu.x[f.rs2]) {
                    cpu.pc = address.wrapping_add(f.imm);
                }
                Ok(())
            },
            disassemble: dump_format_b,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00005063,
            name: "BGE",
            operation: |cpu, word, address| {
                let f = parse_format_b(word);
                if cpu.sign_extend(cpu.x[f.rs1]) >= cpu.sign_extend(cpu.x[f.rs2]) {
                    cpu.pc = address.wrapping_add(f.imm);
                }
                Ok(())
            },
            disassemble: dump_format_b,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00007063,
            name: "BGEU",
            operation: |cpu, word, address| {
                let f = parse_format_b(word);
                if cpu.unsigned_data(cpu.x[f.rs1]) >= cpu.unsigned_data(cpu.x[f.rs2]) {
                    cpu.pc = address.wrapping_add(f.imm);
                }
                Ok(())
            },
            disassemble: dump_format_b,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00004063,
            name: "BLT",
            operation: |cpu, word, address| {
                let f = parse_format_b(word);
                if cpu.x[f.rs1] < cpu.x[f.rs2] {
                    cpu.pc = address.wrapping_add(f.imm);
                }
                Ok(())
            },
            disassemble: dump_format_b,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00006063,
            name: "BLTU",
            operation: |cpu, word, address| {
                let f = parse_format_b(word);
                if (cpu.x[f.rs1] as u32) < (cpu.x[f.rs2] as u32) {
                    cpu.pc = address.wrapping_add(f.imm);
                }
                Ok(())
            },
            disassemble: dump_format_b,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00001063,
            name: "BNE",
            operation: |cpu, word, address| {
                let f = parse_format_b(word);
                if cpu.sign_extend(cpu.x[f.rs1]) != cpu.sign_extend(cpu.x[f.rs2]) {
                    cpu.pc = address.wrapping_add(f.imm);
                }
                Ok(())
            },
            disassemble: dump_format_b,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00003073,
            name: "CSRRC",
            operation: |cpu, word, _address| {
                let f = parse_format_csr(word);
                let data = match cpu.read_csr(f.csr) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                let tmp = cpu.x[f.rs];
                cpu.x[f.rd] = cpu.sign_extend(data);
                match cpu.write_csr(f.csr, (cpu.x[f.rd] & !tmp) as u32) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_csr,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00007073,
            name: "CSRRCI",
            operation: |cpu, word, _address| {
                let f = parse_format_csr(word);
                let data = match cpu.read_csr(f.csr) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = cpu.sign_extend(data);
                match cpu.write_csr(f.csr, (cpu.x[f.rd] & !(f.rs as i32)) as u32) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_csr,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00002073,
            name: "CSRRS",
            operation: |cpu, word, _address| {
                let f = parse_format_csr(word);
                let data = match cpu.read_csr(f.csr) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                let tmp = cpu.x[f.rs];
                cpu.x[f.rd] = cpu.sign_extend(data);
                match cpu.write_csr(f.csr, cpu.unsigned_data(cpu.x[f.rd] | tmp)) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_csr,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00006073,
            name: "CSRRSI",
            operation: |cpu, word, _address| {
                let f = parse_format_csr(word);
                let data = match cpu.read_csr(f.csr) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = cpu.sign_extend(data);
                match cpu.write_csr(f.csr, cpu.unsigned_data(cpu.x[f.rd] | (f.rs as i32))) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_csr,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00001073,
            name: "CSRRW",
            operation: |cpu, word, _address| {
                let f = parse_format_csr(word);
                let data = match cpu.read_csr(f.csr) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                let tmp = cpu.x[f.rs];
                cpu.x[f.rd] = cpu.sign_extend(data);
                match cpu.write_csr(f.csr, cpu.unsigned_data(tmp)) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_csr,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00005073,
            name: "CSRRWI",
            operation: |cpu, word, _address| {
                let f = parse_format_csr(word);
                let data = match cpu.read_csr(f.csr) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                cpu.x[f.rd] = cpu.sign_extend(data);
                match cpu.write_csr(f.csr, f.rs as u32) {
                    Ok(()) => {}
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_csr,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02004033,
            name: "DIV",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.x[f.rs1];
                let divisor = cpu.x[f.rs2];
                if divisor == 0 {
                    cpu.x[f.rd] = -1;
                } else if dividend == cpu.most_negative() && divisor == -1 {
                    cpu.x[f.rd] = dividend;
                } else {
                    cpu.x[f.rd] = cpu.sign_extend(dividend.wrapping_div(divisor))
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02005033,
            name: "DIVU",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.unsigned_data(cpu.x[f.rs1]);
                let divisor = cpu.unsigned_data(cpu.x[f.rs2]);
                if divisor == 0 {
                    cpu.x[f.rd] = -1;
                } else {
                    cpu.x[f.rd] = cpu.sign_extend(dividend.wrapping_div(divisor) as i32)
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0200503b,
            name: "DIVUW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.unsigned_data(cpu.x[f.rs1]) as u32;
                let divisor = cpu.unsigned_data(cpu.x[f.rs2]) as u32;
                if divisor == 0 {
                    cpu.x[f.rd] = -1;
                } else {
                    cpu.x[f.rd] = dividend.wrapping_div(divisor) as i32
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0200403b,
            name: "DIVW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.x[f.rs1] as i32;
                let divisor = cpu.x[f.rs2] as i32;
                if divisor == 0 {
                    cpu.x[f.rd] = -1;
                } else if dividend == std::i32::MIN && divisor == -1 {
                    cpu.x[f.rd] = dividend as i32;
                } else {
                    cpu.x[f.rd] = dividend.wrapping_div(divisor) as i32
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xffffffff,
            data: 0x00100073,
            name: "EBREAK",
            operation: |_cpu, _word, _address| {
                // @TODO: Implement
                Ok(())
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0xffffffff,
            data: 0x00000073,
            name: "ECALL",
            operation: |cpu, _word, address| {
                let mut args = [0i32; 8];
                for (src, dest) in cpu.x[10..].iter().zip(args.iter_mut()) {
                    *dest = *src;
                }
                use crate::mmu::SyscallResult;
                match cpu.memory.lock().unwrap().syscall(args) {
                    SyscallResult::Ok(result) => {
                        for (src, dest) in result.iter().zip(cpu.x[10..].iter_mut()) {
                            *dest = *src;
                        }
                        Ok(())
                    }
                    SyscallResult::Defer(receiver) => Err(Trap {
                        trap_type: TrapType::PauseEmulation(receiver),
                        value: address,
                    }),
                    SyscallResult::Terminate(_) => panic!("Unhandled termination"),
                    SyscallResult::Continue => {
                        println!("Got \"ECALL\" from address {:08x} -- issuing trap", address);
                        let exception_type = match cpu.privilege_mode {
                            PrivilegeMode::User => TrapType::EnvironmentCallFromUMode,
                            PrivilegeMode::Supervisor => TrapType::EnvironmentCallFromSMode,
                            PrivilegeMode::Machine => TrapType::EnvironmentCallFromMMode,
                            PrivilegeMode::Reserved => panic!("Unknown Privilege mode"),
                        };
                        Err(Trap {
                            trap_type: exception_type,
                            value: address,
                        })
                    }
                }
            },
            disassemble: dump_empty,
        },
        // Instruction {
        //     mask: 0xfe00007f,
        //     data: 0x02000053,
        //     name: "FADD.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = cpu.f[f.rs1] + cpu.f[f.rs2];
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0007f,
        //     data: 0xd2200053,
        //     name: "FCVT.D.L",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = cpu.x[f.rs1] as f64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0007f,
        //     data: 0x42000053,
        //     name: "FCVT.D.S",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         // Is this implementation correct?
        //         cpu.f[f.rd] = f32::from_bits(cpu.f[f.rs1].to_bits() as u32) as f64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0007f,
        //     data: 0xd2000053,
        //     name: "FCVT.D.W",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = cpu.x[f.rs1] as i32 as f64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0007f,
        //     data: 0xd2100053,
        //     name: "FCVT.D.WU",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = cpu.x[f.rs1] as u32 as f64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0007f,
        //     data: 0x40100053,
        //     name: "FCVT.S.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         // Is this implementation correct?
        //         cpu.f[f.rd] = cpu.f[f.rs1] as f32 as f64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0007f,
        //     data: 0xc2000053,
        //     name: "FCVT.W.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         // Is this implementation correct?
        //         cpu.x[f.rd] = cpu.f[f.rs1] as u32 as i32 as i64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfe00007f,
        //     data: 0x1a000053,
        //     name: "FDIV.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let dividend = cpu.f[f.rs1];
        //         let divisor = cpu.f[f.rs2];
        //         // Is this implementation correct?
        //         if divisor == 0.0 {
        //             cpu.f[f.rd] = std::f64::INFINITY;
        //             cpu.set_fcsr_dz();
        //         } else if divisor == -0.0 {
        //             cpu.f[f.rd] = std::f64::NEG_INFINITY;
        //             cpu.set_fcsr_dz();
        //         } else {
        //             cpu.f[f.rd] = dividend / divisor;
        //         }
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0x0000707f,
            data: 0x0000000f,
            name: "FENCE",
            operation: |_cpu, _word, _address| {
                // Do nothing?
                Ok(())
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x0000100f,
            name: "FENCE.I",
            operation: |_cpu, _word, _address| {
                // Do nothing?
                Ok(())
            },
            disassemble: dump_empty,
        },
        // Instruction {
        //     mask: 0xfe00707f,
        //     data: 0xa2002053,
        //     name: "FEQ.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.x[f.rd] = match cpu.f[f.rs1] == cpu.f[f.rs2] {
        //             true => 1,
        //             false => 0,
        //         };
        //         Ok(())
        //     },
        //     disassemble: dump_empty,
        // },
        // Instruction {
        //     mask: 0x0000707f,
        //     data: 0x00003007,
        //     name: "FLD",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_i(word);
        //         cpu.f[f.rd] = match cpu
        //             .mmu
        //             .load_doubleword(cpu.x[f.rs1].wrapping_add(f.imm) as u64)
        //         {
        //             Ok(data) => f64::from_bits(data),
        //             Err(e) => return Err(e),
        //         };
        //         Ok(())
        //     },
        //     disassemble: dump_format_i,
        // },
        // Instruction {
        //     mask: 0xfe00707f,
        //     data: 0xa2000053,
        //     name: "FLE.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.x[f.rd] = match cpu.f[f.rs1] <= cpu.f[f.rs2] {
        //             true => 1,
        //             false => 0,
        //         };
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfe00707f,
        //     data: 0xa2001053,
        //     name: "FLT.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.x[f.rd] = match cpu.f[f.rs1] < cpu.f[f.rs2] {
        //             true => 1,
        //             false => 0,
        //         };
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0x0000707f,
        //     data: 0x00002007,
        //     name: "FLW",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_i(word);
        //         cpu.f[f.rd] = match cpu.mmu.load_word(cpu.x[f.rs1].wrapping_add(f.imm) as u64) {
        //             Ok(data) => f64::from_bits(data as i32 as i64 as u64),
        //             Err(e) => return Err(e),
        //         };
        //         Ok(())
        //     },
        //     disassemble: dump_format_i_mem,
        // },
        // Instruction {
        //     mask: 0x0600007f,
        //     data: 0x02000043,
        //     name: "FMADD.D",
        //     operation: |cpu, word, _address| {
        //         // @TODO: Update fcsr if needed?
        //         let f = parse_format_r2(word);
        //         cpu.f[f.rd] = cpu.f[f.rs1] * cpu.f[f.rs2] + cpu.f[f.rs3];
        //         Ok(())
        //     },
        //     disassemble: dump_format_r2,
        // },
        // Instruction {
        //     mask: 0xfe00007f,
        //     data: 0x12000053,
        //     name: "FMUL.D",
        //     operation: |cpu, word, _address| {
        //         // @TODO: Update fcsr if needed?
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = cpu.f[f.rs1] * cpu.f[f.rs2];
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0707f,
        //     data: 0xf2000053,
        //     name: "FMV.D.X",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = f64::from_bits(cpu.x[f.rs1] as u64);
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0707f,
        //     data: 0xe2000053,
        //     name: "FMV.X.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.x[f.rd] = cpu.f[f.rs1].to_bits() as i64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0707f,
        //     data: 0xe0000053,
        //     name: "FMV.X.W",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.x[f.rd] = cpu.f[f.rs1].to_bits() as i32 as i64;
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfff0707f,
        //     data: 0xf0000053,
        //     name: "FMV.W.X",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         cpu.f[f.rd] = f64::from_bits(cpu.x[f.rs1] as u32 as u64);
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0x0600007f,
        //     data: 0x0200004b,
        //     name: "FNMSUB.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r2(word);
        //         cpu.f[f.rd] = -(cpu.f[f.rs1] * cpu.f[f.rs2]) + cpu.f[f.rs3];
        //         Ok(())
        //     },
        //     disassemble: dump_format_r2,
        // },
        // Instruction {
        //     mask: 0x0000707f,
        //     data: 0x00003027,
        //     name: "FSD",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_s(word);
        //         cpu.mmu.store_doubleword(
        //             cpu.x[f.rs1].wrapping_add(f.imm) as u64,
        //             cpu.f[f.rs2].to_bits(),
        //         )
        //     },
        //     disassemble: dump_format_s,
        // },
        // Instruction {
        //     mask: 0xfe00707f,
        //     data: 0x22000053,
        //     name: "FSGNJ.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let rs1_bits = cpu.f[f.rs1].to_bits();
        //         let rs2_bits = cpu.f[f.rs2].to_bits();
        //         let sign_bit = rs2_bits & 0x8000000000000000;
        //         cpu.f[f.rd] = f64::from_bits(sign_bit | (rs1_bits & 0x7fffffffffffffff));
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfe00707f,
        //     data: 0x22002053,
        //     name: "FSGNJX.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         let rs1_bits = cpu.f[f.rs1].to_bits();
        //         let rs2_bits = cpu.f[f.rs2].to_bits();
        //         let sign_bit = (rs1_bits ^ rs2_bits) & 0x8000000000000000;
        //         cpu.f[f.rd] = f64::from_bits(sign_bit | (rs1_bits & 0x7fffffffffffffff));
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0xfe00007f,
        //     data: 0x0a000053,
        //     name: "FSUB.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         // @TODO: Update fcsr if needed?
        //         cpu.f[f.rd] = cpu.f[f.rs1] - cpu.f[f.rs2];
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        // Instruction {
        //     mask: 0x0000707f,
        //     data: 0x00002027,
        //     name: "FSW",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_s(word);
        //         cpu.mmu.store_word(
        //             cpu.x[f.rs1].wrapping_add(f.imm) as u64,
        //             cpu.f[f.rs2].to_bits() as u32,
        //         )
        //     },
        //     disassemble: dump_format_s,
        // },
        Instruction {
            mask: 0x0000007f,
            data: 0x0000006f,
            name: "JAL",
            operation: |cpu, word, address| {
                let f = parse_format_j(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.pc as i32);
                cpu.pc = address.wrapping_add(f.imm);
                Ok(())
            },
            disassemble: dump_format_j,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00000067,
            name: "JALR",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                let tmp = cpu.sign_extend(cpu.pc as i32);
                cpu.pc = (cpu.x[f.rs1] as u32).wrapping_add(f.imm as u32);
                cpu.x[f.rd] = tmp;
                Ok(())
            },
            disassemble: |cpu, word, _address, evaluate| {
                let f = parse_format_i(word);
                let mut s = String::new();
                s += get_register_name(f.rd);
                if evaluate {
                    s += &format!(":{:x}", cpu.x[f.rd]);
                }
                s += &format!(",{:x}({}", f.imm, get_register_name(f.rs1));
                if evaluate {
                    s += &format!(":{:x}", cpu.x[f.rs1]);
                }
                s += ")";
                s
            },
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00000003,
            name: "LB",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu.mmu.load(cpu.x[f.rs1].wrapping_add(f.imm) as u32) {
                    Ok(data) => data as i8 as i32,
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_i_mem,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00004003,
            name: "LBU",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu.mmu.load(cpu.x[f.rs1].wrapping_add(f.imm) as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_i_mem,
        },
        // Instruction {
        //     mask: 0x0000707f,
        //     data: 0x00003003,
        //     name: "LD",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_i(word);
        //         cpu.x[f.rd] = match cpu
        //             .mmu
        //             .load_doubleword(cpu.x[f.rs1].wrapping_add(f.imm) as u64)
        //         {
        //             Ok(data) => data as i32,
        //             Err(e) => return Err(e),
        //         };
        //         Ok(())
        //     },
        //     disassemble: dump_format_i_mem,
        // },
        Instruction {
            mask: 0x0000707f,
            data: 0x00001003,
            name: "LH",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu
                    .mmu
                    .load_halfword(cpu.x[f.rs1].wrapping_add(f.imm) as u32)
                {
                    Ok(data) => data as i16 as i32,
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_i_mem,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00005003,
            name: "LHU",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu
                    .mmu
                    .load_halfword(cpu.x[f.rs1].wrapping_add(f.imm) as u32)
                {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_i_mem,
        },
        // Instruction {
        //     mask: 0xf9f0707f,
        //     data: 0x1000302f,
        //     name: "LR.D",
        //     operation: |cpu, word, _address| {
        //         let f = parse_format_r(word);
        //         // @TODO: Implement properly
        //         let address = cpu.x[f.rs1] as u64;
        //         cpu.x[f.rd] = cpu.mmu.load_doubleword(address)? as i64;
        //         if cpu.mmu.reserve(address) {
        //             cpu.reservation = Some(address);
        //         }
        //         Ok(())
        //     },
        //     disassemble: dump_format_r,
        // },
        Instruction {
            mask: 0xf9f0707f,
            data: 0x1000202f,
            name: "LR.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                // @TODO: Implement properly
                let address = cpu.x[f.rs1] as u32;
                let core = cpu.read_csr_raw(CSR_MHARTID_ADDRESS);
                cpu.x[f.rd] = cpu.mmu.load_word(address)? as i32;
                cpu.mmu.reserve(core, address);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000007f,
            data: 0x00000037,
            name: "LUI",
            operation: |cpu, word, _address| {
                let f = parse_format_u(word);
                cpu.x[f.rd] = f.imm as i32;
                Ok(())
            },
            disassemble: dump_format_u,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00002003,
            name: "LW",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu.mmu.load_word(cpu.x[f.rs1].wrapping_add(f.imm) as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_i_mem,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00006003,
            name: "LWU",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu.mmu.load_word(cpu.x[f.rs1].wrapping_add(f.imm) as u32) {
                    Ok(data) => data as i32,
                    Err(e) => return Err(e),
                };
                Ok(())
            },
            disassemble: dump_format_i_mem,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02000033,
            name: "MUL",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.x[f.rs1].wrapping_mul(cpu.x[f.rs2]);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02001033,
            name: "MULH",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] =
                    (((cpu.x[f.rs1] as i64).wrapping_mul(cpu.x[f.rs2] as i64)) >> 32) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02003033,
            name: "MULHU",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let r1 = cpu.x[f.rs1] as u32 as u64;
                let r2 = cpu.x[f.rs2] as u32 as u64;
                cpu.x[f.rd] = (r1.wrapping_mul(r2) >> 32) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02002033,
            name: "MULHSU",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(
                    ((cpu.x[f.rs1] as i64).wrapping_mul(cpu.x[f.rs2] as u32 as i64) >> 32) as i32,
                );
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0200003b,
            name: "MULW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] =
                    cpu.sign_extend((cpu.x[f.rs1] as i32).wrapping_mul(cpu.x[f.rs2] as i32) as i32);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xffffffff,
            data: 0x30200073,
            name: "MRET",
            operation: |cpu, _word, _address| {
                cpu.pc = match cpu.read_csr(CSR_MEPC_ADDRESS) {
                    Ok(data) => data,
                    Err(e) => return Err(e),
                };
                let status = cpu.read_csr_raw(CSR_MSTATUS_ADDRESS);
                let mpie = (status >> 7) & 1;
                let mpp = (status >> 11) & 0x3;
                let mprv = match decode_privilege_mode(mpp) {
                    PrivilegeMode::Machine => (status >> 17) & 1,
                    _ => 0,
                };
                // Override MIE[3] with MPIE[7], set MPIE[7] to 1, set MPP[12:11] to 0
                // and override MPRV[17]
                let new_status = (status & !0x21888) | (mprv << 17) | (mpie << 3) | (1 << 7);
                cpu.write_csr_raw(CSR_MSTATUS_ADDRESS, new_status);
                cpu.privilege_mode = match mpp {
                    0 => PrivilegeMode::User,
                    1 => PrivilegeMode::Supervisor,
                    3 => PrivilegeMode::Machine,
                    _ => panic!(), // Shouldn't happen
                };
                cpu.mmu.update_privilege_mode(cpu.privilege_mode);
                Ok(())
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00006033,
            name: "OR",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] | cpu.x[f.rs2]);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00006013,
            name: "ORI",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] | f.imm);
                Ok(())
            },
            disassemble: dump_format_i,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02006033,
            name: "REM",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.x[f.rs1];
                let divisor = cpu.x[f.rs2];
                if divisor == 0 {
                    cpu.x[f.rd] = dividend;
                } else if dividend == cpu.most_negative() && divisor == -1 {
                    cpu.x[f.rd] = 0;
                } else {
                    cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1].wrapping_rem(cpu.x[f.rs2]));
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x02007033,
            name: "REMU",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.unsigned_data(cpu.x[f.rs1]);
                let divisor = cpu.unsigned_data(cpu.x[f.rs2]);
                cpu.x[f.rd] = match divisor {
                    0 => cpu.sign_extend(dividend as i32),
                    _ => cpu.sign_extend(dividend.wrapping_rem(divisor) as i32),
                };
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0200703b,
            name: "REMUW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.x[f.rs1] as u32;
                let divisor = cpu.x[f.rs2] as u32;
                cpu.x[f.rd] = match divisor {
                    0 => dividend as i32,
                    _ => dividend.wrapping_rem(divisor) as i32,
                };
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0200603b,
            name: "REMW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let dividend = cpu.x[f.rs1] as i32;
                let divisor = cpu.x[f.rs2] as i32;
                if divisor == 0 {
                    cpu.x[f.rd] = dividend as i32;
                } else if dividend == std::i32::MIN && divisor == -1 {
                    cpu.x[f.rd] = 0;
                } else {
                    cpu.x[f.rd] = dividend.wrapping_rem(divisor) as i32;
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00000023,
            name: "SB",
            operation: |cpu, word, _address| {
                let f = parse_format_s(word);
                cpu.mmu
                    .store(cpu.x[f.rs1].wrapping_add(f.imm) as u32, cpu.x[f.rs2] as u8)
            },
            disassemble: dump_format_s,
        },
        Instruction {
            mask: 0xf800707f,
            data: 0x1800202f,
            name: "SC.W",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let address = cpu.x[f.rs1] as u32;
                let core = cpu.read_csr_raw(CSR_MHARTID_ADDRESS);
                if cpu.mmu.clear_reservation(core, address) {
                    cpu.mmu.store_word(address, cpu.x[f.rs2] as u32)?;
                    cpu.x[f.rd] = 0;
                } else {
                    cpu.x[f.rd] = 1;
                }
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe007fff,
            data: 0x12000073,
            name: "SFENCE.VMA",
            operation: |_cpu, _word, _address| {
                // Do nothing?
                Ok(())
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00001023,
            name: "SH",
            operation: |cpu, word, _address| {
                let f = parse_format_s(word);
                cpu.mmu
                    .store_halfword(cpu.x[f.rs1].wrapping_add(f.imm) as u32, cpu.x[f.rs2] as u16)
            },
            disassemble: dump_format_s,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00001033,
            name: "SLL",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1].wrapping_shl(cpu.x[f.rs2] as u32));
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfc00707f,
            data: 0x00001013,
            name: "SLLI",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let shamt = f.rs2;
                cpu.x[f.rd] = cpu.x[f.rs1] << shamt;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0000101b,
            name: "SLLIW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let shamt = f.rs2 as u32;
                cpu.x[f.rd] = (cpu.x[f.rs1] << shamt) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0000103b,
            name: "SLLW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = (cpu.x[f.rs1] as u32).wrapping_shl(cpu.x[f.rs2] as u32) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00002033,
            name: "SLT",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = match cpu.x[f.rs1] < cpu.x[f.rs2] {
                    true => 1,
                    false => 0,
                };
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00002013,
            name: "SLTI",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu.x[f.rs1] < f.imm {
                    true => 1,
                    false => 0,
                };
                Ok(())
            },
            disassemble: dump_format_i,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00003013,
            name: "SLTIU",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = match cpu.unsigned_data(cpu.x[f.rs1]) < cpu.unsigned_data(f.imm) {
                    true => 1,
                    false => 0,
                };
                Ok(())
            },
            disassemble: dump_format_i,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00003033,
            name: "SLTU",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] =
                    match cpu.unsigned_data(cpu.x[f.rs1]) < cpu.unsigned_data(cpu.x[f.rs2]) {
                        true => 1,
                        false => 0,
                    };
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x40005033,
            name: "SRA",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1].wrapping_shr(cpu.x[f.rs2] as u32));
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfc00707f,
            data: 0x40005013,
            name: "SRAI",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let mask = 0x1f;
                let shamt = (word >> 20) & mask;
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] >> shamt);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfc00707f,
            data: 0x4000501b,
            name: "SRAIW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let shamt = (word >> 20) & 0x1f;
                cpu.x[f.rd] = (cpu.x[f.rs1] as i32) >> shamt;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x4000503b,
            name: "SRAW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = (cpu.x[f.rs1] as i32).wrapping_shr(cpu.x[f.rs2] as u32) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xffffffff,
            data: 0x10200073,
            name: "SRET",
            operation: |cpu, _word, _address| {
                // @TODO: Throw error if higher privilege return instruction is executed
                cpu.pc = match cpu.read_csr(CSR_SEPC_ADDRESS) {
                    Ok(data) => data,
                    Err(e) => return Err(e),
                };
                let status = cpu.read_csr_raw(CSR_SSTATUS_ADDRESS);
                let spie = (status >> 5) & 1;
                let spp = (status >> 8) & 1;
                let mprv = match decode_privilege_mode(spp) {
                    PrivilegeMode::Machine => (status >> 17) & 1,
                    _ => 0,
                };
                // Override SIE[1] with SPIE[5], set SPIE[5] to 1, set SPP[8] to 0,
                // and override MPRV[17]
                let new_status = (status & !0x20122) | (mprv << 17) | (spie << 1) | (1 << 5);
                cpu.write_csr_raw(CSR_SSTATUS_ADDRESS, new_status);
                cpu.privilege_mode = match spp {
                    0 => PrivilegeMode::User,
                    1 => PrivilegeMode::Supervisor,
                    _ => panic!(), // Shouldn't happen
                };
                // println!("Updating privilege mode to {:?}", cpu.privilege_mode);
                cpu.mmu.update_privilege_mode(cpu.privilege_mode);
                Ok(())
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00005033,
            name: "SRL",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(
                    cpu.unsigned_data(cpu.x[f.rs1])
                        .wrapping_shr(cpu.x[f.rs2] as u32) as i32,
                );
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfc00707f,
            data: 0x00005013,
            name: "SRLI",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let mask = 0x1f;
                let shamt = (word >> 20) & mask;
                cpu.x[f.rd] = cpu.sign_extend((cpu.unsigned_data(cpu.x[f.rs1]) >> shamt) as i32);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfc00707f,
            data: 0x0000501b,
            name: "SRLIW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                let mask = 0x1f;
                let shamt = (word >> 20) & mask;
                cpu.x[f.rd] = ((cpu.x[f.rs1] as u32) >> shamt) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x0000503b,
            name: "SRLW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = (cpu.x[f.rs1] as u32).wrapping_shr(cpu.x[f.rs2] as u32) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x40000033,
            name: "SUB",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1].wrapping_sub(cpu.x[f.rs2]));
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x4000003b,
            name: "SUBW",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.x[f.rs1].wrapping_sub(cpu.x[f.rs2]) as i32;
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00002023,
            name: "SW",
            operation: |cpu, word, _address| {
                let f = parse_format_s(word);
                cpu.mmu
                    .store_word(cpu.x[f.rs1].wrapping_add(f.imm) as u32, cpu.x[f.rs2] as u32)
            },
            disassemble: dump_format_s,
        },
        Instruction {
            mask: 0xffffffff,
            data: 0x00200073,
            name: "URET",
            operation: |_cpu, _word, _address| {
                // @TODO: Implement
                panic!("URET instruction is not implemented yet.");
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0xffffffff,
            data: 0x10500073,
            name: "WFI",
            operation: |cpu, _word, _address| {
                cpu.wfi = true;
                Ok(())
            },
            disassemble: dump_empty,
        },
        Instruction {
            mask: 0xfe00707f,
            data: 0x00004033,
            name: "XOR",
            operation: |cpu, word, _address| {
                let f = parse_format_r(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] ^ cpu.x[f.rs2]);
                Ok(())
            },
            disassemble: dump_format_r,
        },
        Instruction {
            mask: 0x0000707f,
            data: 0x00004013,
            name: "XORI",
            operation: |cpu, word, _address| {
                let f = parse_format_i(word);
                cpu.x[f.rd] = cpu.sign_extend(cpu.x[f.rs1] ^ f.imm);
                Ok(())
            },
            disassemble: dump_format_i,
        },
    ]
}

struct FormatB {
    rs1: usize,
    rs2: usize,
    imm: u32,
}

fn parse_format_b(word: u32) -> FormatB {
    FormatB {
        rs1: ((word >> 15) & 0x1f) as usize, // [19:15]
        rs2: ((word >> 20) & 0x1f) as usize, // [24:20]
        imm: (
            match word & 0x80000000 { // imm[31:12] = [31]
				0x80000000 => 0xfffff000,
				_ => 0
			} |
			((word << 4) & 0x00000800) | // imm[11] = [7]
			((word >> 20) & 0x000007e0) | // imm[10:5] = [30:25]
			((word >> 7) & 0x0000001e)
            // imm[4:1] = [11:8]
        ) as i32 as u32,
    }
}

fn dump_format_b(cpu: &Cpu, word: u32, address: u32, evaluate: bool) -> String {
    let f = parse_format_b(word);
    let mut s = String::new();
    s += get_register_name(f.rs1);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs1]);
    }
    s += &format!(",{}", get_register_name(f.rs2));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs2]);
    }
    s += &format!(",{:x}", address.wrapping_add(f.imm));
    s
}

struct FormatCSR {
    csr: u16,
    rs: usize,
    rd: usize,
}

fn parse_format_csr(word: u32) -> FormatCSR {
    FormatCSR {
        csr: ((word >> 20) & 0xfff) as u16, // [31:20]
        rs: ((word >> 15) & 0x1f) as usize, // [19:15], also uimm
        rd: ((word >> 7) & 0x1f) as usize,  // [11:7]
    }
}

fn dump_format_csr(cpu: &Cpu, word: u32, _address: u32, evaluate: bool) -> String {
    let f = parse_format_csr(word);
    let mut s = String::new();
    s += get_register_name(f.rd);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rd]);
    }
    // @TODO: Use CSR name
    s += &format!(",{:x}", f.csr);
    if evaluate {
        s += &format!(":{:x}", cpu.read_csr_raw(f.csr));
    }
    s += &format!(",{}", get_register_name(f.rs));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs]);
    }
    s
}

struct FormatI {
    rd: usize,
    rs1: usize,
    imm: i32,
}

fn parse_format_i(word: u32) -> FormatI {
    FormatI {
        rd: ((word >> 7) & 0x1f) as usize,   // [11:7]
        rs1: ((word >> 15) & 0x1f) as usize, // [19:15]
        imm: (
            if word & 0x80000000 != 0 {
                // imm[31:11] = [31]
                0xfffff800
            } else {
                0
            } | ((word >> 20) & 0x000007ff)
            // imm[10:0] = [30:20]
        ) as i32,
    }
}

fn dump_format_i(cpu: &Cpu, word: u32, _address: u32, evaluate: bool) -> String {
    let f = parse_format_i(word);
    let mut s = String::new();
    s += get_register_name(f.rd);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rd]);
    }
    s += &format!(",{}", get_register_name(f.rs1));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs1]);
    }
    s += &format!(",{:x}", f.imm);
    s
}

fn dump_format_i_mem(cpu: &Cpu, word: u32, _address: u32, evaluate: bool) -> String {
    let f = parse_format_i(word);
    let mut s = String::new();
    s += get_register_name(f.rd);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rd]);
    }
    s += &format!(",{:x}({}", f.imm, get_register_name(f.rs1));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs1]);
    }
    s += ")";
    s
}

struct FormatJ {
    rd: usize,
    imm: u32,
}

fn parse_format_j(word: u32) -> FormatJ {
    FormatJ {
        rd: ((word >> 7) & 0x1f) as usize, // [11:7]
        imm: (
            match word & 0x80000000 { // imm[31:20] = [31]
				0x80000000 => 0xfff00000,
				_ => 0
			} |
			(word & 0x000ff000) | // imm[19:12] = [19:12]
			((word & 0x00100000) >> 9) | // imm[11] = [20]
			((word & 0x7fe00000) >> 20)
            // imm[10:1] = [30:21]
        ),
    }
}

fn dump_format_j(cpu: &Cpu, word: u32, address: u32, evaluate: bool) -> String {
    let f = parse_format_j(word);
    let mut s = String::new();
    s += get_register_name(f.rd);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rd]);
    }
    s += &format!(",{:x}", address.wrapping_add(f.imm));
    s
}

struct FormatR {
    rd: usize,
    rs1: usize,
    rs2: usize,
}

fn parse_format_r(word: u32) -> FormatR {
    // println!(
    //     "parse_format_r({:x}) -> rd:{} rs1:{} rs2:{}",
    //     word,
    //     ((word >> 7) & 0x1f) as usize,
    //     ((word >> 15) & 0x1f) as usize,
    //     ((word >> 20) & 0x1f) as usize
    // );
    FormatR {
        rd: ((word >> 7) & 0x1f) as usize,   // [11:7]
        rs1: ((word >> 15) & 0x1f) as usize, // [19:15]
        rs2: ((word >> 20) & 0x1f) as usize, // [24:20]
    }
}

fn dump_format_r(cpu: &Cpu, word: u32, _address: u32, evaluate: bool) -> String {
    let f = parse_format_r(word);
    let mut s = String::new();
    s += get_register_name(f.rd);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rd]);
    }
    s += &format!(",{}", get_register_name(f.rs1));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs1]);
    }
    s += &format!(",{}", get_register_name(f.rs2));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs2]);
    }
    s
}

// // has rs3
// struct FormatR2 {
//     rd: usize,
//     rs1: usize,
//     rs2: usize,
//     rs3: usize,
// }

// fn parse_format_r2(word: u32) -> FormatR2 {
//     FormatR2 {
//         rd: ((word >> 7) & 0x1f) as usize,   // [11:7]
//         rs1: ((word >> 15) & 0x1f) as usize, // [19:15]
//         rs2: ((word >> 20) & 0x1f) as usize, // [24:20]
//         rs3: ((word >> 27) & 0x1f) as usize, // [31:27]
//     }
// }

// fn dump_format_r2(cpu: &Cpu, word: u32, _address: u64, evaluate: bool) -> String {
//     let f = parse_format_r2(word);
//     let mut s = String::new();
//     s += get_register_name(f.rd);
//     if evaluate {
//         s += &format!(":{:x}", cpu.x[f.rd]);
//     }
//     s += &format!(",{}", get_register_name(f.rs1));
//     if evaluate {
//         s += &format!(":{:x}", cpu.x[f.rs1]);
//     }
//     s += &format!(",{}", get_register_name(f.rs2));
//     if evaluate {
//         s += &format!(":{:x}", cpu.x[f.rs2]);
//     }
//     s += &format!(",{}", get_register_name(f.rs3));
//     if evaluate {
//         s += &format!(":{:x}", cpu.x[f.rs3]);
//     }
//     s
// }

struct FormatS {
    rs1: usize,
    rs2: usize,
    imm: i32,
}

fn parse_format_s(word: u32) -> FormatS {
    // println!(
    //     "parse_format_s(0x{:08x}): rs1:{} rs2:{} imm:{}",
    //     word,
    //     ((word >> 15) & 0x1f) as usize,
    //     ((word >> 20) & 0x1f) as usize,
    //     (match word & 0x80000000 {
    //             0x80000000 => 0xfffff000,
    //             _ => 0,
    //         } | // imm[31:12] = [31]
    //         ((word >> 20) & 0xfe0) | // imm[11:5] = [31:25]
    //         ((word >> 7) & 0x1f)) as i32
    // );
    FormatS {
        rs1: ((word >> 15) & 0x1f) as usize, // [19:15]
        rs2: ((word >> 20) & 0x1f) as usize, // [24:20]
        imm: (
            match word & 0x80000000 {
				0x80000000 => 0xfffff000,
				_ => 0
			} | // imm[31:12] = [31]
			((word >> 20) & 0xfe0) | // imm[11:5] = [31:25]
			((word >> 7) & 0x1f)
            // imm[4:0] = [11:7]
        ) as i32,
    }
}

fn dump_format_s(cpu: &Cpu, word: u32, _address: u32, evaluate: bool) -> String {
    let f = parse_format_s(word);
    let mut s = String::new();
    s += get_register_name(f.rs2);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs2]);
    }
    s += &format!(",{:x}({}", f.imm, get_register_name(f.rs1));
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rs1]);
    }
    s += ")";
    s
}

struct FormatU {
    rd: usize,
    imm: u32,
}

fn parse_format_u(word: u32) -> FormatU {
    FormatU {
        rd: ((word >> 7) & 0x1f) as usize, // [11:7]
        imm: word & 0xfffff000,
    }
}

fn dump_format_u(cpu: &Cpu, word: u32, _address: u32, evaluate: bool) -> String {
    let f = parse_format_u(word);
    let mut s = String::new();
    s += get_register_name(f.rd);
    if evaluate {
        s += &format!(":{:x}", cpu.x[f.rd]);
    }
    s += &format!(",{:x}", f.imm);
    s
}

fn dump_empty(_cpu: &Cpu, _word: u32, _address: u32, _evaluate: bool) -> String {
    String::new()
}

fn get_register_name(num: usize) -> &'static str {
    match num {
        0 => "zero",
        1 => "ra",
        2 => "sp",
        3 => "gp",
        4 => "tp",
        5 => "t0",
        6 => "t1",
        7 => "t2",
        8 => "s0",
        9 => "s1",
        10 => "a0",
        11 => "a1",
        12 => "a2",
        13 => "a3",
        14 => "a4",
        15 => "a5",
        16 => "a6",
        17 => "a7",
        18 => "s2",
        19 => "s3",
        20 => "s4",
        21 => "s5",
        22 => "s6",
        23 => "s7",
        24 => "s8",
        25 => "s9",
        26 => "s10",
        27 => "s11",
        28 => "t3",
        29 => "t4",
        30 => "t5",
        31 => "t6",
        _ => panic!("Unknown register num {}", num),
    }
}
