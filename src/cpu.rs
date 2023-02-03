use std::collections::HashMap;
use crate::opcodes;

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: u8,
    pub program_counter: u16,
    memory: [u8; 0xFFFF]
}

// 必要ないけどなんとなく
const CLEAR_STATUS: u8      = 0b0000_0000;
// Processor Status
const CARRY_FLAG: u8        = 0b0000_0001;
const ZERO_FLAG: u8         = 0b0000_0010;
const INTERRUPT_DISABLE: u8 = 0b0000_0100;
const DECIMAL_MODE_FLAG:u8  = 0b0000_1000;
const BREAK_COMMAND:u8     = 0b0001_0000;
const BREAK2_COMMAND:u8    = 0b0010_0000;
const OVERFLOW_FLAG:u8     = 0b0100_0000;
const NEGATIVE_FLAG:u8     = 0b1000_0000;

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}

trait Mem {
    fn mem_read(&self, add: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16 {
	let lo = self.mem_read(pos) as u16;
	let hi = self.mem_read(pos + 1) as u16;
	(hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
	let hi = (data >> 8) as u8;
	let lo = (data & 0xFF) as u8;
	self.mem_write(pos, lo);
	self.mem_write(pos + 1, hi);
    }
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
	self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
	self.memory[addr as usize] = data;
    }
}

impl CPU {
    pub fn new() -> Self {
	CPU {
	    register_a: 0,
	    register_x: 0,
	    register_y: 0,
	    status: 0,
	    program_counter: 0,
	    memory: [0; 0xFFFF]
	}
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
	match mode {
	    // LDA #$C0 -> A9 C0 (即値)
	    AddressingMode::Immediate => self.program_counter,
	    // LDA $C0 -> A5 C0 (0xC0にある値をA_registerに代入 0 ~ 255)
	    AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,
	    // LDA $C000 -> AD 00 C0 (0xC000にある値をA_registerに代入 (0x0000~0xFFFFまで全て))
	    AddressingMode::Absolute => self.mem_read_u16(self.program_counter),
	    // LDA $C0,X -> A5 C0+X (C0にregister_xの値を加算した場所の値をA_registerに代入) 
	    AddressingMode::ZeroPage_X => {
		let pos = self.mem_read(self.program_counter);
		let addr = pos.wrapping_add(self.register_x) as u16;
		addr
	    }
	    // LDA $C0,Y -> A5 C0+Y (C0にregister_yの値を加算した場所の値をA_registerに代入) 
	    AddressingMode::ZeroPage_Y => {
		let pos = self.mem_read(self.program_counter);
		let addr = pos.wrapping_add(self.register_y) as u16;
		addr
	    }
	    // LDA $C000,X -> BD 00 C0 (C000にregister_xの値を加算した場所の値をA_registerに代入)
	    AddressingMode::Absolute_X => {
		let base = self.mem_read_u16(self.program_counter);
		let addr = base.wrapping_add(self.register_x as u16);
		addr
	    }
	    // LDA $C000,Y -> B9 00 C0 (C000にregister_yの値を加算した場所の値をA_registerに代入)
	    AddressingMode::Absolute_Y => {
		let base = self.mem_read_u16(self.program_counter);
		let addr = base.wrapping_add(self.register_y as u16);
		addr
	    }
	    // LDA ($C0,X) -> A1 C0+X+1
	    AddressingMode::Indirect_X => {
		let base = self.mem_read(self.program_counter);
		let ptr: u8 = (base as u8).wrapping_add(self.register_x);
		let lo = self.mem_read(ptr as u16);
		let hi = self.mem_read(ptr.wrapping_add(1) as u16);
		(hi as u16) << 8 | (lo as u16)
	    }
	    // LDA ($C0,Y) -> B1 C0+1+Y
	    AddressingMode::Indirect_Y => {
		let base = self.mem_read(self.program_counter);
		let lo = self.mem_read(base as u16);
		let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
		let deref_base = (hi as u16) << 8 | (lo as u16);
		let deref = deref_base.wrapping_add(self.register_y as u16);
		deref
	    }
	    // Undefined Addressing
	    AddressingMode::NoneAddressing => {
		panic!("mode {:?} is not supported", mode);
	    }
	}
    }
    
    fn add_to_register_a(&mut self, value: u8) {
	let result = self.register_a as u16
	    + value as u16
	    + (if self.status & CARRY_FLAG == CARRY_FLAG {
		1
	    } else {
		0
	    }) as u16;
	
	// 0xFFより大きければキャリーは立つ
	let carry = result > 0xFF;
	if carry {
	    self.status |= CARRY_FLAG; // 0b 0000,0001
	} else {
	    self.status &= !CARRY_FLAG; // 0b 1111,1110
	}

	let tmp = result as u8;

	// overflow check
	if (value ^ tmp) & (tmp ^ self.register_a) & NEGATIVE_FLAG != 0 {
	    self.status |= OVERFLOW_FLAG; // 0b 0100,0000 overflow flag
	} else {
	    self.status &= !OVERFLOW_FLAG; // 0b 1011,1111
	}
	
	self.set_register_a(tmp);
    }

    fn set_register_a(&mut self, value: u8) {
	self.register_a = value;
	self.update_zero_and_negative_flags(self.register_a);
    }
    
    fn adc(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.add_to_register_a(value);
    }

    fn sbc(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.add_to_register_a(((value as i8).wrapping_neg().wrapping_sub(1)) as u8);
    }

    fn and(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(value & self.register_a);
    }

    fn asl_accumulator(&mut self) {
	let mut data = self.register_a;
	// 7bitが設定されている場合
	if data >> 7 == 1 {
	    self.status |= CARRY_FLAG; // set carry flag
	} else {
	    self.status |= !CARRY_FLAG; // remove carry flag
	}
	data = data << 1;
	self.register_a = data;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) -> u8 {
	let addr = self.get_operand_address(&mode);
	let mut data = self.mem_read(addr);
	// 7bitが設定されている場合
	if data >> 7 == 1 {
	    self.status |= CARRY_FLAG; // set carry flag
	} else {
	    self.status |= !CARRY_FLAG; // remove carry flag
	}
	data = data << 1;
	self.mem_write(addr, data);
	self.update_zero_and_negative_flags(data);
	data
    }

    // Branch opecode (bcc, bcs, ...)
    fn branch(&mut self) {
	let branch: i8 = self.mem_read(self.program_counter) as i8;
	let branch_addr = self
	    .program_counter.
	    wrapping_add(1).
	    wrapping_add(branch as u16);
	self.program_counter = branch_addr;
    }

    fn bcc(&mut self) {
	// CARRY FLAGがセットされていない場合
	// PC += PCアドレスの値+1
	if self.status & CARRY_FLAG != CARRY_FLAG {
	    self.branch();
	}
    }

    fn bcs(&mut self) {
	// CARRY FLAGがセットされている場合
	// PC += PCアドレスの値+1
	if self.status & CARRY_FLAG == CARRY_FLAG {
	    self.branch();
	}
    }

    fn beq(&mut self) {
	// ZERO FLAGがセットされている場合
	// PC += PCアドレスの値+1
	if self.status & ZERO_FLAG == ZERO_FLAG {
	    self.branch();
	}
    }

    fn bit(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let tmp = self.register_a & value;
	if tmp == 0 {
	    self.status |= ZERO_FLAG; // zero flag
	} else {
	    self.status &= !ZERO_FLAG;
	}
	let v7_bit = value & NEGATIVE_FLAG;
	let v6_bit = value & OVERFLOW_FLAG;
	self.status |= v7_bit;
	self.status |= v6_bit;
    }

    fn bmi(&mut self) {
	// negative flag
	if self.status & NEGATIVE_FLAG == NEGATIVE_FLAG {
	    self.branch();
	}
    }

    fn bne(&mut self) {
	if self.status & ZERO_FLAG == CLEAR_STATUS {
	    self.branch();
	}
    }

    fn bpl(&mut self) {
	if self.status & NEGATIVE_FLAG == CLEAR_STATUS {
	    self.branch();
	}
    }

    // fn brk(&mut self) {
    // }

    fn bvc(&mut self) {
	if self.status & OVERFLOW_FLAG == CLEAR_STATUS {
	    self.branch();
	}
    }

    fn bvs(&mut self) {
	if self.status & OVERFLOW_FLAG == OVERFLOW_FLAG {
	    self.branch();
	}
    }

    fn clc(&mut self) {
	if self.status & CARRY_FLAG == CARRY_FLAG {
	    self.status -= 1;
	}
	// if self.status % 2 != 0 {
	//     self.status -= 1;
	// }
    }

    fn cld(&mut self) {
	if self.status & DECIMAL_MODE_FLAG == DECIMAL_MODE_FLAG {
	    self.status -= DECIMAL_MODE_FLAG;
	}
    }

    fn cli(&mut self) {
	if self.status & INTERRUPT_DISABLE == INTERRUPT_DISABLE {
	    self.status -= INTERRUPT_DISABLE;
	}
    }

    fn clv(&mut self) {
	if self.status & OVERFLOW_FLAG == OVERFLOW_FLAG {
	    self.status -= OVERFLOW_FLAG;
	}
    }

    fn compare(&mut self, cmp_data: u8, value: u8) {
	if cmp_data == value {
	    self.status |= ZERO_FLAG;
	} else if cmp_data >= value {
	    self.status |= CARRY_FLAG;
	}
	let res = cmp_data.wrapping_sub(value);
	self.update_zero_and_negative_flags(res);
    }

    fn cmp(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.compare(self.register_a, value);
    }

    fn cpx(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.compare(self.register_x, value);
    }

    fn cpy(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.compare(self.register_y, value);
    }

    fn lda(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	
	self.register_a = value;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn sta(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(mode);
	self.mem_write(addr, self.register_a);
    }

    fn tax(&mut self) {
	self.register_x = self.register_a;
	self.update_zero_and_negative_flags(self.register_x);
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
	if result == 0 {
	    self.status = self.status | ZERO_FLAG;
	} else {
	    self.status = self.status & !ZERO_FLAG;
	}

	if result & 0b1000_0000 != 0 {
	    self.status = self.status | NEGATIVE_FLAG;
	} else {
	    self.status = self.status & !NEGATIVE_FLAG;
	}
    }

    fn inx(&mut self) {
	self.register_x = self.register_x.wrapping_add(1);
	self.update_zero_and_negative_flags(self.register_x);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
	self.load(program);
	self.reset();
	self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
	self.memory[0x8000 .. (0x8000 + program.len())].copy_from_slice(&program[..]);
	self.mem_write_u16(0xFFFC, 0x8000);
    }

    pub fn reset(&mut self) {
	self.register_a = 0;
	self.register_x = 0;
	self.status = 0;
	// 0xFFFC, 0xFFFDにはloadの時点で0x00,0x80つまり0x8000が入っているはず
	self.program_counter = self.mem_read_u16(0xFFFC);
    }
    
    pub fn run(&mut self) {
	let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPCODES_MAP;
	
	loop {
	    // 0x8000の値(命令)を読み込む
	    let code = self.mem_read(self.program_counter);
	    self.program_counter += 1;
	    let program_counter_state = self.program_counter;

	    let opcode = opcodes.get(&code).expect(&format!("OpCode {:x} is not recognized", code));

	    match code {
		// ADC (Add with Carry)
		0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => {
		    self.adc(&opcode.mode);
		}
		// AND (Logical AND)
		0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => {
		    self.and(&opcode.mode);
		}
		// ASL (Arithmetic Shift Left Accumulator) Accumulatorはopecodeのみなので分ける
		0x0A => self.asl_accumulator(),
		// ASL (Arithmetic Shift Left other)
		0x06 | 0x16 | 0x0E | 0x1E => {
		    self.asl(&opcode.mode);
		}
		// BCC (Branch if Carry Clear)
		0x90 => self.bcc(),
		// BCS (Branch if Carry Set)
		0xB0 => self.bcs(),
		// BEQ (Branch if Equal)
		0xF0 => self.beq(),
		// BIT (Bit Test)
		0x24 | 0x2C => {
		    self.bit(&opcode.mode);
		}
		// BMI (Branch if Minus)
		0x30 => self.bmi(),
		// BNE (Branch if Not Equal)
		0xD0 => self.bne(),
		// BPL (Branch if Positive)
		0x10 => self.bpl(),
		// BRK (Force Interrupt)
		0x00 => return,
		// BVC (Branch if Overflow Clear)
		0x50 => self.bvc(),
		// BVS (Branch if Overflow Set)
		0x70 => self.bvs(),
		// CLC (Clear Carry Flag)
		0x18 => self.clc(),
		// CLD (Clear Decimal Mode)
		0xd8 => self.cld(),
		// CLI (Clear Interruput Disable)
		0x58 => self.cli(),
		// CLV (Clear Oveflow Flag)
		0xb8 => self.clv(),
		// CMP (Compare)
		0xC9 | 0xC5 | 0xD5 | 0xCD | 0xDD | 0xD9 | 0xC1 | 0xD1 => {
		    self.cmp(&opcode.mode);
		}
		// CPX (Compare X Register)
		0xE0 | 0xE4 | 0xEC => {
		    self.cpx(&opcode.mode);
		}
		// CPY (Compare Y Register)
		0xC0 | 0xC4 | 0xCC => {
		    self.cpy(&opcode.mode);
		}
		// LDA (Load Accumulator)
		0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
		    self.lda(&opcode.mode);
		}
		// SBC (Sbstract with Carry)
		0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xE1 | 0xF1 => {
		    self.sbc(&opcode.mode);
		}
		// STA (Store Accumulator)
		0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => {
		    self.sta(&opcode.mode);
		}
		// TAX (Transfer Accumulator to X)
		0xAA => self.tax(),
		// INX
		0xE8 => self.inx(),
		
		_ => todo!(),
	    }

	    if program_counter_state == self.program_counter {
		self.program_counter += (opcode.len - 1) as u16;
	    }
	}
    }
}


#[cfg(test)]
mod test {
    use super::*;
    // ADC
    #[test]
    fn test_0x69_adc_immediate() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x69, 0x10, 0x00]); 
	cpu.reset();
	cpu.register_a = 0x01;
	cpu.run();
	assert_eq!(cpu.register_a, 0x11);
    }
    
    #[test]
    fn test_0x69_adc_calc_with_carry() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x69, 0x10, 0x00]); 
	cpu.reset();
	cpu.status = 0x01; // carry flag
	cpu.register_a = 0x01;
	cpu.run();
	assert_eq!(cpu.register_a, 0x12);
    }

    #[test]
    fn test_0x69_adc_set_carry() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x69, 0xFF, 0x00]); 
	cpu.reset();
	cpu.register_a = 0x02;
	cpu.run();
	assert_eq!(cpu.register_a, 0x01);
	assert_eq!(cpu.status, 0x01);
    }

    #[test]
    fn test_0x69_adc_overflow() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x69, 0x7F, 0x00]); 
	cpu.reset();
	cpu.register_a = 0x01;
	cpu.run();
	assert_eq!(cpu.register_a, 0x80);
	assert_eq!(cpu.status, 0xC0); // overflow flag and negative flag
    }

    #[test]
    fn test_0x69_adc_overflow_with_carry() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x69, 0x7F, 0x00]); 
	cpu.reset();
	cpu.status = 0x01;
	cpu.register_a = 0x01;
	cpu.run();
	assert_eq!(cpu.register_a, 0x81);
	assert_eq!(cpu.status, 0xC0); // overflow flag and negative flag
    }

    // AND
    #[test]
    fn test_0x29_and_immediate() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x29, 0xAE, 0x00]); // AE => 10101110
	cpu.reset();
	cpu.register_a = 0xF3;            // F3 => 11110011
	cpu.run();
	assert_eq!(cpu.register_a, 0xA2); // A2 => 10100010
    }
    
    // ASL
    #[test]
    fn test_0x0a_asl_accumulator() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x0A, 0x00]);
	cpu.reset();
	cpu.register_a = 0x03;
	cpu.run();
	assert_eq!(cpu.register_a, 0x03 * 2);
    }
    
    #[test]
    fn test_0x06_asl_zero_page() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x06, 0x01, 0x00]);
	cpu.reset();
	cpu.mem_write(0x0001, 0x03);
	cpu.run();
	assert_eq!(cpu.mem_read(0x0001), 0x03 * 2);
    }
    
    // BCC
    #[test]
    fn test_0x90_bcc_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x90, 0x00]);
	cpu.reset();
	cpu.run();
	// PCの初期値は0x8000
	// 0x90, 0x00で1++
	// 0x90の中で1++
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }   
    // BCS
    #[test]
    fn test_0xb0_bcs_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xB0, 0x00]);
	cpu.reset();
	cpu.status = 0x01; // carry flag
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // BEQ
    #[test]
    fn test_0xf0_beq_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xB0, 0x00]);
	cpu.reset();
	cpu.status = 0x02; // zero flag
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // BIT
    #[test]
    fn test_0x24_bit_zeropage() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x24, 0x01, 0x00]);
	cpu.reset();
	cpu.mem_write_u16(0x01, 0b1100_0000);
	cpu.register_a = 0b0000_0001; // 0b11000000 & 0b00000001 = 0 -> zeroflag up
	cpu.run();
	assert_eq!(cpu.status, 0b1100_0010);
    }
    
    #[test]
    fn test_0x2c_bit_absolute() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x2C, 0x04, 0x80, 0x00]);
	cpu.reset();
	cpu.mem_write_u16(0x8004, 0b1100_0000);
	cpu.register_a = 0b000_00001; // 0b11000000 & 0b00000001 = 0 -> zeroflag up
	cpu.run();
	assert_eq!(cpu.status, 0b1100_0010);
    }
    
    // BMI
    #[test]
    fn test_0x30_bmi_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x30, 0x00]);
	cpu.reset();
	cpu.status = 0x80; // zero flag
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // BNE
    #[test]
    fn test_0xd0_bne_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xD0, 0x00]);
	cpu.reset();
	cpu.status = !ZERO_FLAG;
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // BPL
    #[test]
    fn test_0x10_bpl_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x10, 0x00]);
	cpu.reset();
	cpu.status = !NEGATIVE_FLAG;
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // BRK
    #[test]
    fn test_0x00_brk_implied() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x00]);
	cpu.reset();
	cpu.run();
	assert_eq!(cpu.status, 0x00);
    }
    
    // BVC
    #[test]
    fn test_0x50_bvc_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x50, 0x00]);
	cpu.reset();
	cpu.status = !OVERFLOW_FLAG;
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // BVS
    #[test]
    fn test_0x70_bvs_relative() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x70, 0x00]);
	cpu.reset();
	cpu.status = OVERFLOW_FLAG;
	cpu.run();
	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
    }
    
    // CLC
    #[test]
    fn test_0x18_clc_implied() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x18, 0x00]);
	cpu.reset();
	cpu.status = CARRY_FLAG;
	cpu.run();
	assert_eq!(cpu.status, CLEAR_STATUS);
    }
    
    // CLD
    #[test]
    fn test_0xd8_cld_implied() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xd8, 0x00]);
	cpu.reset();
	cpu.status = DECIMAL_MODE_FLAG;
	cpu.run();
	assert_eq!(cpu.status, CLEAR_STATUS);
    }
    
    // CLI
    #[test]
    fn test_0x58_cli_implied() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x58, 0x00]);
	cpu.reset();
	cpu.status = INTERRUPT_DISABLE;
	cpu.run();
	assert_eq!(cpu.status, CLEAR_STATUS);
    }
    
    // CLV
    #[test]
    fn test_0xb8_clv_implied() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xB8, 0x00]);
	cpu.reset();
	cpu.status = OVERFLOW_FLAG;
	cpu.run();
	assert_eq!(cpu.status, CLEAR_STATUS);
    }
    
    // CMP
    #[test]
    fn test_0xc9_cmp_immediate_A_equal_M() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xC9, 0x11, 0x00]);
	cpu.reset();
	cpu.register_a = 0x11;
	cpu.run();
	assert_eq!(cpu.status, ZERO_FLAG);
    }

    #[test]
    fn test_0xc9_cmp_immediate_A_bigger_M() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xC9, 0x11, 0x00]);
	cpu.reset();
	cpu.register_a = 0x12;
	cpu.run();
	assert_eq!(cpu.status, CARRY_FLAG);
    }

    #[test]
    fn test_0xc9_cmp_immediate_negflg() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xC9, 0x11, 0x00]);
	cpu.reset();
	cpu.register_a = 0xFF;
	cpu.run();
	assert_eq!(cpu.status, NEGATIVE_FLAG | CARRY_FLAG);
    }
    
    // CPX
    #[test]
    fn test_0xe0_cpx_immediate_X_equal_M() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xe0, 0x11, 0x00]);
	cpu.reset();
	cpu.register_x = 0x11;
	cpu.run();
	assert_eq!(cpu.status, ZERO_FLAG);
    }

    #[test]
    fn test_0xe0_cpx_immediate_X_bigger_M() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xe0, 0x11, 0x00]);
	cpu.reset();
	cpu.register_x = 0x12;
	cpu.run();
	assert_eq!(cpu.status, CARRY_FLAG);
    }

    #[test]
    fn test_0xe0_cpx_immediate_negflg() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xe0, 0x11, 0x00]);
	cpu.reset();
	cpu.register_x = 0xFF;
	cpu.run();
	assert_eq!(cpu.status, CARRY_FLAG | NEGATIVE_FLAG);
    }

    // CPY
    #[test]
    fn test_0xc0_cpy_immediate_Y_equal_M() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xc0, 0x11, 0x00]);
	cpu.reset();
	cpu.register_y = 0x11;
	cpu.run();
	assert_eq!(cpu.status, ZERO_FLAG);
    }

    #[test]
    fn test_0xc0_cpy_immediate_Y_bigger_M() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xc0, 0x11, 0x00]);
	cpu.reset();
	cpu.register_y = 0x12;
	cpu.run();
	assert_eq!(cpu.status, CARRY_FLAG);
    }

    #[test]
    fn test_0xc0_cpy_immediate_negflg() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xc0, 0x11, 0x00]);
	cpu.reset();
	cpu.register_y = 0xFF;
	cpu.run();
	assert_eq!(cpu.status, CARRY_FLAG | NEGATIVE_FLAG);
    }
    
    // DEC
    // DEX
    // DEY
    // EOR
    // INC
    
    // INX Increment X Register
    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0xFF, 0xAA,0xE8, 0xE8, 0x00]);
        assert_eq!(cpu.register_x, 1)
    }
    
    // INY
    // JMP
    // JSR
    
    // LDA
    #[test]
    fn test_0xa9_lda_immediate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0x05, 0x00]);	
        assert_eq!(cpu.register_a, 5);
        assert!(cpu.status & 0b0000_0010 == 0);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xa9_lda_zero_page() {
	let mut cpu = CPU::new();
	cpu.mem_write(0x10, 0x55);
	cpu.load_and_run(vec![0xA5, 0x10, 0x00]);
	assert_eq!(cpu.register_a, 0x55);
    }

    #[test]
    fn test_0xb5_lda_zero_page_x() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xB5, 0x0F, 0x00]);
	cpu.reset();
	cpu.mem_write(0x0F, 0x12);	
	cpu.run();
	assert_eq!(cpu.register_a, 0x12);
    }

    #[test]
    fn test_0xad_lda_absolute() {
	let mut cpu = CPU::new();
	cpu.load_and_run(vec![0xAD, 0x00, 0x80, 0x00]);
	assert_eq!(cpu.register_a, 0xAD);
    }

    #[test]
    fn test_0xbd_lda_absolute_x() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xBD, 0x12, 0x34, 0x00]);
	cpu.reset();
	cpu.register_x = 0x02;
	cpu.mem_write_u16(0x3414, 0x34);	
	cpu.run();
	assert_eq!(cpu.register_a, 0x34);
    }

    #[test]
    fn test_0xb9_lda_absolute_y() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xB9, 0x12, 0x34, 0x00]);
	cpu.reset();
	cpu.register_y = 0x01;
	cpu.mem_write_u16(0x3413, 0x34);	
	cpu.run();
	assert_eq!(cpu.register_a, 0x34);
    }

    // 0x10+0x02=0x12の値(0x34)と0x10+0x02+0x01=0x13の値(0x55)をアドレスとみなして
    // 0x5534の値をAレジスタに書き込む
    #[test]
    fn test_0xa1_lda_indirect_x() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xA1, 0x10, 0x00]);
	cpu.reset();
	cpu.register_x = 0x02;
	cpu.mem_write(0x12, 0x34);
	cpu.mem_write(0x13, 0x55);
	cpu.mem_write_u16(0x5534, 0x1F);
	cpu.run();
	assert_eq!(cpu.register_a, 0x1F);
    }

    // 0x10の値(0x34)と0x11の値(0x55)をアドレスとみなして
    // 0x5534+0x02=0x5536の値をAレジスタに書き込む
    #[test]
    fn test_0xb1_lda_uindirect_y() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xB1, 0x10, 0x00]);
	cpu.reset();
	cpu.register_y = 0x02;
	cpu.mem_write(0x10, 0x34);
	cpu.mem_write(0x11, 0x55);
	cpu.mem_write_u16(0x5536, 0x1F);
	cpu.run();
	assert_eq!(cpu.register_a, 0x1F);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0x00, 0x00]);	
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }


    // LDX
    // LDY
    // LSR
    // NOP
    // ORA
    // PHA
    // PHP
    // PLA
    // PLP
    // ROL
    // ROR
    // RTI
    // RTS

    // SBC A-M-(1-C)
    #[test]
    fn test_0xe9_sbc_immediate() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xE9, 0x01, 0x00]); 
	cpu.reset();
	cpu.register_a = 0x10;
	cpu.run();
	assert_eq!(cpu.register_a, 0x0E);
    }

    #[test]
    fn test_0xe9_sbc_calc_with_carry() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xE9, 0x10, 0x00]); 
	cpu.reset();
	cpu.status = 0x01; // carry flag
	cpu.register_a = 0x50;
	cpu.run();
	assert_eq!(cpu.register_a, 0x40);
    }

    #[test]
    fn test_0xe9_sbc_set_carry() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xE9, 0x03, 0x00]); 
	cpu.reset();
	cpu.register_a = 0x02;
	cpu.run();
	assert_eq!(cpu.register_a, 0xFE);
	assert_eq!(cpu.status, 0x80);
    }

    #[test]
    fn test_0xe9_sbc_overflow() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xE9, 0x81, 0x00]); 
	cpu.reset();
	cpu.register_a = 0x7F;
	cpu.run();
	assert_eq!(cpu.register_a, 0xFD);
	assert_eq!(cpu.status, 0xC0); // overflow flag and negative flag
    }

    #[test]
    fn test_0xe9_sbc_overflow_with_carry() {
	let mut cpu = CPU::new();
	cpu.load(vec![0xE9, 0x7F, 0x00]); 
	cpu.reset();
	cpu.status = 0x01;
	cpu.register_a = 0x7E;
	cpu.run();
	assert_eq!(cpu.register_a, 0xFF);
	assert_eq!(cpu.status, 0x80); // overflow flag and negative flag
    }

    // SEC
    // SED
    // SEI

    // STA
    #[test]
    fn test_0x85_sta_immediate() {
	let mut cpu = CPU::new();
	cpu.load(vec![0x85, 0xA8, 0x00]);
	cpu.reset();
	cpu.register_a = 0x45;
	cpu.run();
	assert_eq!(cpu.mem_read(0xA8), 0x45);
    }

    
    // STX
    // STX
    // STY

    // TAX Transfer Accumulator to X
    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0x0A, 0xAA, 0x00]);	
        assert_eq!(cpu.register_x, 10)
    }

    // TAY
    // TSX
    // TXA
    // TXS
    // TYA

    // other
    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0xC0, 0xAA, 0xE8, 0x00]);
        assert_eq!(cpu.register_x, 0xC1)
    }    
}
