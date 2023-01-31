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
	    + (if self.status & 0x01 == 0x01 {
		1
	    } else {
		0
	    }) as u16;
	
	// 0xFFより大きければキャリーは立つ
	let carry = result > 0xFF;
	if carry {
	    self.status |= 0x01; // 0b 0000,0001
	} else {
	    self.status &= 0xFE; // 0b 1111,1110
	}

	let tmp = result as u8;

	// overflow check
	if (value ^ tmp) & (tmp ^ self.register_a) & 0x80 != 0 {
	    self.status |= 0x40; // 0b 0100,0000 overflow flag
	} else {
	    self.status &= 0xBF; // 0b 1011,1111
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
	    self.status |= 0x01; // set carry flag
	} else {
	    self.status |= !0x01; // remove carry flag
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
	    self.status |= 0x01; // set carry flag
	} else {
	    self.status |= !0x01; // remove carry flag
	}
	data = data << 1;
	self.mem_write(addr, data);
	self.update_zero_and_negative_flags(data);
	data
    }

    fn bcc(&mut self) {
	// CARRY FLAGがセットされていない場合
	// PC += PCアドレスの値+1
	if self.status & 0x01 != 0x01 {
	    let branch: i8 = self.mem_read(self.program_counter) as i8;
	    let branch_addr = self
		.program_counter.
		wrapping_add(1).
		wrapping_add(branch as u16);
	    self.program_counter = branch_addr;
	}
    }

    fn bcs(&mut self) {
	// CARRY FLAGがセットされている場合
	// PC += PCアドレスの値+1
	if self.status & 0x01 == 0x01 {
	    let branch: i8 = self.mem_read(self.program_counter) as i8;
	    let branch_addr = self
		.program_counter.
		wrapping_add(1).
		wrapping_add(branch as u16);
	    self.program_counter = branch_addr;
	}
    }

    fn beq(&mut self) {
	// ZERO FLAGがセットされている場合
	// PC += PCアドレスの値+1
	if self.status & 0x02 == 0x02 {
	    let branch: i8 = self.mem_read(self.program_counter) as i8;
	    let branch_addr = self
		.program_counter.
		wrapping_add(1).
		wrapping_add(branch as u16);
	    self.program_counter = branch_addr;
	}
    }

    fn bit(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let tmp = self.register_a & value;
	if tmp == 0 {
	    self.status |= 0x40; // zero flag
	} else {
	    self.status &= !0x40;
	}
	let v7_bit = value & 0b01000000;
	let v6_bit = value & 0b10000000;
	self.status |= v7_bit;
	self.status |= v6_bit;	
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
	    self.status = self.status | 0b0000_0010;
	} else {
	    self.status = self.status & 0b1111_1101;
	}

	if result & 0b1000_0000 != 0 {
	    self.status = self.status | 0b1000_0000;
	} else {
	    self.status = self.status & 0b0111_1111;
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
		// BIT
		0x24 | 0x2C => {
		    self.bit(&opcode.mode);
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
		// BRK (Force Interrupt)
		0x00 => return,
		
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
	cpu.mem_write(0x01, 0b11000000);
	cpu.register_a = 0b00000001; // 0b11000000 & 0b00000001 = 0 -> zeroflag up
	cpu.run();
	assert_eq!(cpu.status, 0b11000000);
    }
    
    // BMI
    // BNE
    // BPL
    // BRK
    // BVC
    // CLC
    // CLD
    // CLI
    // CLV
    // CMP
    // CPX
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
