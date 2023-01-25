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
	// 0xFFFC, 0xFFFDにはloadの時点で0x00,0x800つまり0x8000が入っているはず
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
		// LDA (Load Accumulator)
		0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
		    self.lda(&opcode.mode);
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
    // LDA
    #[test]
    fn test_0xa9_lda_immidiate_load_data() {
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

    // load_and_runでメモリが初期化されていい感じにテストできてない
    #[test]
    fn test_0xb5_lda_zero_page_x() {
	let mut cpu = CPU::new();
	cpu.load_and_run(vec![0xB5, 0x0F, 0x00]);
	assert_eq!(cpu.register_a, 0x00);
    }

    #[test]
    fn test_0xad_lda_absolute() {
	let mut cpu = CPU::new();
	cpu.load_and_run(vec![0xAD, 0x00, 0x80, 0x00]);
	assert_eq!(cpu.register_a, 0xAD);
    }

    #[test]
    // register_xが初期化されているので略
    fn test_0xbd_lda_absolute_x() {
	let mut cpu = CPU::new();
	cpu.load_and_run(vec![0xBD, 0x01, 0x80, 0x00]);
	assert_eq!(cpu.register_a, 0x01);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0x00, 0x00]);	
        assert!(cpu.status & 0b0000_0010 == 0b10);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0x0A, 0xAA, 0x00]);	
        assert_eq!(cpu.register_x, 10)
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0xC0, 0xAA, 0xE8, 0x00]);
        assert_eq!(cpu.register_x, 0xC1)
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xA9, 0xFF, 0xAA,0xE8, 0xE8, 0x00]);
        assert_eq!(cpu.register_x, 1)
    }
}
