use std::collections::HashMap;
use crate::opcodes;
use crate::bus::Bus;

// stack
const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xFD;

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: u8,
    pub program_counter: u16,
    pub stack_pointer: u8,
    // memory: [u8; 0xFFFF]
    pub bus: Bus
}

const CLEAR_STATUS: u8      = 0b0000_0000;
// Processor Status
const CARRY_FLAG: u8        = 0b0000_0001;
const ZERO_FLAG: u8         = 0b0000_0010;
const INTERRUPT_DISABLE: u8 = 0b0000_0100;
const DECIMAL_MODE_FLAG:u8  = 0b0000_1000;
const BREAK_COMMAND:u8      = 0b0001_0000;
const BREAK2_COMMAND:u8     = 0b0010_0000;
const OVERFLOW_FLAG:u8      = 0b0100_0000;
const NEGATIVE_FLAG:u8      = 0b1000_0000;

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
    Indirect_jmp,
    NoneAddressing,
}

pub trait Mem {
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
	self.bus.mem_read(addr)
    }

    fn mem_read_u16(&self, pos: u16) -> u16 {
	self.bus.mem_read_u16(pos)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
	self.bus.mem_write(addr, data)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
	self.bus.mem_write_u16(pos, data)
    }
}

impl CPU {
    pub fn new(bus: Bus) -> Self {
	CPU {
	    register_a: 0,
	    register_x: 0,
	    register_y: 0,
	    status: 0b100100,
	    program_counter: 0,
	    stack_pointer: STACK_RESET,
	    // memory: [0; 0xFFFF]
	    bus: bus,
	}
    }

    pub fn get_absolute_address(&self, mode: &AddressingMode, addr: u16) -> u16 {
	match mode {
	    // LDA #$C0 -> A9 C0 (??????)
	    AddressingMode::Immediate => self.program_counter,
	    // LDA $C0 -> A5 C0 (0xC0???????????????A_register????????? 0 ~ 255)
	    AddressingMode::ZeroPage => self.mem_read(addr) as u16,
	    // LDA $C000 -> AD 00 C0 (0xC000???????????????A_register????????? (0x0000~0xFFFF????????????))
	    AddressingMode::Absolute => self.mem_read_u16(addr),
	    // LDA $C0,X -> A5 C0+X (C0???register_x????????????????????????????????????A_register?????????) 
	    AddressingMode::ZeroPage_X => {
		let pos = self.mem_read(addr);
		let addr = pos.wrapping_add(self.register_x) as u16;
		addr
	    }
	    // LDA $C0,Y -> A5 C0+Y (C0???register_y????????????????????????????????????A_register?????????) 
	    AddressingMode::ZeroPage_Y => {
		let pos = self.mem_read(addr);
		let addr = pos.wrapping_add(self.register_y) as u16;
		addr
	    }
	    // LDA $C000,X -> BD 00 C0 (C000???register_x????????????????????????????????????A_register?????????)
	    AddressingMode::Absolute_X => {
		let base = self.mem_read_u16(addr);
		let addr = base.wrapping_add(self.register_x as u16);
		addr
	    }
	    // LDA $C000,Y -> B9 00 C0 (C000???register_y????????????????????????????????????A_register?????????)
	    AddressingMode::Absolute_Y => {
		let base = self.mem_read_u16(addr);
		let addr = base.wrapping_add(self.register_y as u16);
		addr
	    }
	    // LDA ($C0,X) -> A1 C0+X+1
	    AddressingMode::Indirect_X => {
		let base = self.mem_read(addr);
		let ptr: u8 = (base as u8).wrapping_add(self.register_x);
		let lo = self.mem_read(ptr as u16);
		let hi = self.mem_read(ptr.wrapping_add(1) as u16);
		(hi as u16) << 8 | (lo as u16)
	    }
	    // LDA ($C0,Y) -> B1 C0+1+Y
	    AddressingMode::Indirect_Y => {
		// let base = self.mem_read(self.program_counter);
		let base = self.mem_read(addr);
		let lo = self.mem_read(base as u16);
		let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
		let deref_base = (hi as u16) << 8 | (lo as u16);
		let deref = deref_base.wrapping_add(self.register_y as u16);
		deref
	    }
	    // Indirect for jump instruction
	    AddressingMode::Indirect_jmp => {
		let addr = self.mem_read_u16(self.program_counter);
		let indirect_ref;
		if addr & 0x00FF == 0x00FF {
		    let lo = self.mem_read(addr);
		    let hi = self.mem_read(addr & 0xFF00);
		    indirect_ref = (hi as u16) << 8 | (lo as u16);
		} else {
		    indirect_ref = self.mem_read_u16(addr);
		};
		indirect_ref
	    }
	    // Undefined Addressing
	    AddressingMode::NoneAddressing => {
		panic!("mode {:?} is not supported", mode);
	    }
	}	
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
	match mode {
	    AddressingMode::Immediate => self.program_counter,
	    _ => self.get_absolute_address(mode, self.program_counter),
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
	
	// 0xFF??????????????????????????????????????????
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

    fn update_negative_flags(&mut self, result: u8) {
	if result >> 7 == 1 {
	    self.status |= NEGATIVE_FLAG;
	} else {
	    self.status &= !NEGATIVE_FLAG;
	}
    }

    fn stack_pop(&mut self) -> u8 {
	self.stack_pointer = self.stack_pointer.wrapping_add(1);
	self.mem_read((STACK as u16) + self.stack_pointer as u16)
    }

    fn stack_push(&mut self, data: u8) {
	self.mem_write((STACK as u16) + self.stack_pointer as u16, data);
	self.stack_pointer = self.stack_pointer.wrapping_sub(1)
    }

    fn stack_pop_u16(&mut self) -> u16 {
	let lo = self.stack_pop() as u16;
	let hi = self.stack_pop() as u16;
	hi << 8 | lo
    }

    fn stack_push_u16(&mut self, data: u16) {
	let hi = (data >> 8) as u8;
	let lo = (data & 0xFF) as u8;
	self.stack_push(hi);
	self.stack_push(lo);
    }
    
    fn adc(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.add_to_register_a(value);
    }

    fn and(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(value & self.register_a);
    }

    fn asl_accumulator(&mut self) {
	let mut data = self.register_a;
	// 7bit??????????????????????????????
	if data >> 7 == 1 {
	    self.status |= CARRY_FLAG; // set carry flag
	} else {
	    self.status &= !CARRY_FLAG; // remove carry flag
	}
	data = data << 1;
	self.register_a = data;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) -> u8 {
	let addr = self.get_operand_address(&mode);
	let mut data = self.mem_read(addr);
	// 7bit??????????????????????????????
	if data >> 7 == 1 {
	    self.status |= CARRY_FLAG; // set carry flag
	} else {
	    self.status &= !CARRY_FLAG; // remove carry flag
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
	// CARRY FLAG????????????????????????????????????
	// PC += PC??????????????????+1
	if self.status & CARRY_FLAG != CARRY_FLAG {
	    self.branch();
	}
    }

    fn bcs(&mut self) {
	// CARRY FLAG?????????????????????????????????
	// PC += PC??????????????????+1
	if self.status & CARRY_FLAG == CARRY_FLAG {
	    self.branch();
	}
    }

    fn beq(&mut self) {
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
	// let v7_bit = value & NEGATIVE_FLAG; // 0b1000_0000
	// let v6_bit = value & OVERFLOW_FLAG; // 0b0100_0000
	if value & 0b1000_0000 > 0 {
	    self.status |= NEGATIVE_FLAG;
	} else {
	    self.status &= !NEGATIVE_FLAG;
	}
	if value & 0b0100_0000 > 0 {
	    self.status |= OVERFLOW_FLAG;
	} else {
	    self.status &= !OVERFLOW_FLAG;
	}
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

    fn brk(&mut self) {
    }

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
	if cmp_data >= value {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG;
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

    fn dec(&mut self, mode: &AddressingMode) -> u8 {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = value.wrapping_sub(1);
	self.mem_write(addr, res);
	self.update_zero_and_negative_flags(res);
	res
    }

    fn dex(&mut self) {
	self.register_x = self.register_x.wrapping_sub(1);
	self.update_zero_and_negative_flags(self.register_x);
    }

    fn dey(&mut self) {
	self.register_y = self.register_y.wrapping_sub(1);
	self.update_zero_and_negative_flags(self.register_y);
    }

    fn eor(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_a ^= value;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn inc(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let mut value = self.mem_read(addr);
	value = value.wrapping_add(1);
	self.mem_write(addr, value);
	self.update_zero_and_negative_flags(value);
    }

    fn inx(&mut self) {
	self.register_x = self.register_x.wrapping_add(1);
	self.update_zero_and_negative_flags(self.register_x);
    }

    fn iny(&mut self) {
	self.register_y = self.register_y.wrapping_add(1);
	self.update_zero_and_negative_flags(self.register_y);
    }

    fn jmp(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	self.program_counter = addr;
    }

    fn jsr(&mut self, mode: &AddressingMode) {
	self.stack_push_u16(self.program_counter + 2 - 1);
	let addr = self.get_operand_address(&mode);
	self.program_counter = addr;
    }

    fn lda(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_a = value;
	// self.update_zero_and_negative_flags(self.register_a);
	self.set_register_a(value);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_x = value;
	self.update_zero_and_negative_flags(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_y = value;
	self.update_zero_and_negative_flags(self.register_y);
    }

    fn lsr_accumulator(&mut self) {
	let mut data = self.register_a;
	if data & 1 == 1 {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG;
	}
	data = data >> 1; // data * 2
	self.set_register_a(data)
    }

    fn lsr(&mut self, mode: &AddressingMode) -> u8 {
	let addr = self.get_operand_address(mode);
	let mut value = self.mem_read(addr);
	if value & 1 == 1 {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG;
	}
	value = value >> 1;
	self.mem_write(addr, value);
	self.update_zero_and_negative_flags(value);
	value
    }

    fn nop(&mut self) {
	return
    }

    fn ora(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(mode);
	let value = self.mem_read(addr);
	self.set_register_a(value | self.register_a);
    }

    fn pha(&mut self) {
	self.stack_push(self.register_a);
    }
    
    fn php(&mut self) {
	let mut copy = self.status.clone();
	copy |= BREAK_COMMAND;
	copy |= BREAK2_COMMAND;
	self.stack_push(copy);
    }
    
    fn pla(&mut self) {
	let value = self.stack_pop();
	self.set_register_a(value);
    }
    
    fn plp(&mut self) {
	self.status = self.stack_pop();
	self.status &= !BREAK_COMMAND;
	self.status |= BREAK2_COMMAND;
    }

    fn rol_accumulator(&mut self) {
	let mut value = self.register_a;
	let tmp = self.status & CARRY_FLAG;
	if value >> 7 == 1 {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG
	}
	// left shift
	value = value << 1;
	if tmp != 0 {
	    value |= 1;
	}
	self.set_register_a(value);
    }

    fn rol(&mut self, mode: &AddressingMode) -> u8 {
	let addr = self.get_operand_address(mode);
	let mut value = self.mem_read(addr);
	let tmp = self.status & CARRY_FLAG;

	if value >> 7 == 1 {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG;
	}
	value = value << 1;
	if tmp != 0 {
	    value |= 1;
	}
	self.mem_write(addr, value);
	self.update_negative_flags(value);
	value
    }

    fn ror_accumulator(&mut self) {
	let mut value = self.register_a;
	let tmp = self.status & CARRY_FLAG;
	if value & 1 == 1 {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG
	}
	// right shift
	value = value >> 1;
	if tmp == CARRY_FLAG {
	    value |= 0x80; // 0b1000_0000
	}
	self.set_register_a(value);
    }
    
    fn ror(&mut self, mode: &AddressingMode) -> u8 {
	let addr = self.get_operand_address(mode);
	let mut value = self.mem_read(addr);
	let tmp = self.status & CARRY_FLAG;

	if value & 1 == 1 {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG;
	}
	value = value >> 1;
	if tmp == 1 {
	    value |= NEGATIVE_FLAG; // 0b1000_0000
	} else {
	    value &= !NEGATIVE_FLAG;
	}
	self.mem_write(addr, value);
	self.update_negative_flags(value);
	value
    }

    fn rti(&mut self) {
	self.status = self.stack_pop();
	self.status &= !BREAK_COMMAND;
	self.status |= BREAK2_COMMAND;
	self.program_counter = self.stack_pop_u16();
    }

    fn rts(&mut self) {
	self.program_counter = self.stack_pop_u16() + 1;
    }

    fn sbc(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.add_to_register_a(((value as i8).wrapping_neg().wrapping_sub(1)) as u8);
    }

    fn sec(&mut self) {
	self.status |= CARRY_FLAG;
    }

    fn sed(&mut self) {
	self.status |= DECIMAL_MODE_FLAG;
    }

    fn sei(&mut self) {
	self.status |= INTERRUPT_DISABLE;
    }

    fn sta(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(mode);
	self.mem_write(addr, self.register_a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	self.mem_write(addr, self.register_x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	self.mem_write(addr, self.register_y);
    }

    fn tax(&mut self) {
	self.register_x = self.register_a;
	self.update_zero_and_negative_flags(self.register_x);
    }    

    fn tay(&mut self) {
	self.register_y = self.register_a;
	self.update_zero_and_negative_flags(self.register_y);
    }

    fn tsx(&mut self) {
	self.register_x = self.stack_pointer;
	self.update_zero_and_negative_flags(self.register_x);
    }

    fn txa(&mut self) {
	self.register_a = self.register_x;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn txs(&mut self) {
	self.stack_pointer = self.register_x;
    }

    fn tya(&mut self) {
	self.register_a = self.register_y;
	self.update_zero_and_negative_flags(self.register_a);
    }

    // unofficial
    // not confirmed
    fn anc(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	// self.register_a &= value;
	// self.update_zero_and_negative_flags(self.register_a);
	self.set_register_a(self.register_a & value);
	if self.register_a & NEGATIVE_FLAG == NEGATIVE_FLAG {
	    self.status |= CARRY_FLAG;
	} else {
	    self.status &= !CARRY_FLAG;
	}
    }
    
    // not confirmed
    fn sax(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let res = self.register_x & self.register_a;
	self.mem_write(addr, res);
    }
    
    // not confirmed
    fn arr(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	
	// self.register_a &= value;
	// self.update_zero_and_negative_flags(self.register_a);
	self.set_register_a(self.register_a & value);
	
	self.ror_accumulator();
	let res_bit_5 = (self.register_a >> 5) & 1;
	let res_bit_6 = (self.register_a >> 6) & 1;

	if (res_bit_5 == 1) & (res_bit_6 == 1) {
	    self.status |= CARRY_FLAG;
	    self.status &= !OVERFLOW_FLAG;
	} else if (res_bit_5 == 0) & (res_bit_6 == 0) {
	    self.status &= !CARRY_FLAG;
	    self.status &= !OVERFLOW_FLAG;
	} else if (res_bit_5 == 1) & (res_bit_6 == 0) {
	    self.status &= !CARRY_FLAG;
	    self.status |= OVERFLOW_FLAG;
	} else if (res_bit_5 == 0) & (res_bit_6 == 1) {
	    self.status |= CARRY_FLAG;
	    self.status &= !OVERFLOW_FLAG;
	}
	self.update_zero_and_negative_flags(self.register_a);
    }
    
    // not confirmed
    fn alr(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(self.register_a & value);
	self.lsr_accumulator();
    }

    // not confirmed
    fn lxa(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(self.register_a & value);
	self.register_x = self.register_a;
	self.update_zero_and_negative_flags(self.register_a);
    }
    
    // not confirmed
    fn ahx(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = self.register_x & self.register_a & (addr >> 8) as u8;
	self.mem_write(addr, res);
    }
    
    // not confirmed
    fn axs(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let tmp = self.register_x & self.register_a;
	let res = tmp.wrapping_sub(tmp);
	if value <= tmp {
	    self.status |= CARRY_FLAG;
	}
	self.update_zero_and_negative_flags(res);
	self.register_x = res;
    }

    fn dcp(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = value.wrapping_sub(1);
	self.mem_write(addr, res);
	if res <= self.register_a {
	    self.status |= CARRY_FLAG;
	}
	self.update_zero_and_negative_flags(self.register_a.wrapping_sub(res));
    }

    fn nop_dop(&mut self) {
	return
    }

    fn isb(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = value.wrapping_add(1);
	self.update_zero_and_negative_flags(res);
	self.add_to_register_a(res.wrapping_neg().wrapping_sub(1));
	self.mem_write(addr, res);
    }
    
    // not confirmed
    fn kil(&mut self) {
	return
    }
    
    // not confirmed
    fn las(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = value & self.stack_pointer;
	self.register_a = res;
	self.register_x = res;
	self.stack_pointer = res;
	self.update_zero_and_negative_flags(res);
    }
    
    fn lax(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(value);
	self.register_x = value;
    }

    fn rla(&mut self, mode: &AddressingMode) {
	let value = self.rol(mode);
	self.set_register_a(value & self.register_a);
    }

    fn rra(&mut self, mode: &AddressingMode) {
	let value = self.ror(mode);
	self.add_to_register_a(value);
    }

    fn sbc_ex(&mut self, mode: &AddressingMode) {
	self.sbc(mode);
    }

    fn slo(&mut self, mode: &AddressingMode) {
	let value = self.asl(mode);
	self.register_a |= value;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn sre(&mut self, mode: &AddressingMode) {
	let value = self.lsr(mode);
	self.register_a ^= value;
	self.update_zero_and_negative_flags(self.register_a);
    }

    // not confirmed
    fn shx(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let tmp = (addr >> 8 as u8).wrapping_add(1) as u8;
	let res = self.register_x & tmp;
	self.mem_write(addr, res);
    }

    // not confirmed
    fn shy(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let tmp = (addr >> 8 as u8).wrapping_add(1) as u8;
	let res = self.register_y & tmp;
	self.mem_write(addr, res);
    }
    
    fn nop_top(&mut self) {
	return
    }

    // not confirmed
    fn xaa(&mut self, mode: &AddressingMode) {
	// Exact operation unknown. <- ???
	self.register_a = self.register_x;
	self.update_zero_and_negative_flags(self.register_a);
	let addr = self.get_operand_address(mode);
	let data = self.mem_read(addr);
	self.set_register_a(data & self.register_a);
    }
    
    // not confirmed
    fn tas(&mut self, mode: &AddressingMode) {
	let addr = self.get_operand_address(&mode);
	let res = self.register_x & self.register_a;
	self.stack_pointer = res;
	let res1 = self.stack_pointer & (addr >> 8 as u8).wrapping_add(1) as u8;
	self.mem_write(addr, res1);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
	self.load(program);
	self.reset();
	self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
	// self.memory[0x8000 .. (0x8000 + program.len())].copy_from_slice(&program[..]);
	// self.memory[0x0600 .. (0x0600 + program.len())].copy_from_slice(&program[..]);
	for i in 0..(program.len() as u16) {
	    self.mem_write(0x0600 + i, program[i as usize]);
	}
	// self.mem_write_u16(0xFFFC, 0x8000);
	// self.mem_write_u16(0xFFFC, 0x0600);
    }

    pub fn reset(&mut self) {
	self.register_a = 0;
	self.register_x = 0;
	self.register_y = 0;
	self.stack_pointer = STACK_RESET;
	// self.status = 0;
	self.status = 0b100100;
	// 0xFFFC, 0xFFFD??????load????????????0x00,0x80?????????0x8000????????????????????????
	self.program_counter = self.mem_read_u16(0xFFFC);
    }

    fn crash(&self) {
	let pc = self.program_counter.wrapping_sub(1); 
	panic!("unexpected opecode was executed {:?} ", self.mem_read(pc));
    }
    
    pub fn run(&mut self) {
	self.run_with_callback(|_| {});
    }
    
    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
	F: FnMut(&mut CPU),
    {
	let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPCODES_MAP;
	loop {
	    callback(self);
	    // 0x8000??????(??????)???????????????
	    let code = self.mem_read(self.program_counter);
	    self.program_counter += 1;
	    let program_counter_state = self.program_counter;
	    // println!("{}", self.program_counter);
	    let opcode = opcodes.get(&code).expect(&format!("OpCode {:x} is not recognized", code));
	    // println!("{:x}", code);

	    match code {
		// ADC (Add with Carry)
		0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => {
		    self.adc(&opcode.mode);
		}
		// AND (Logical AND)
		0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => {
		    self.and(&opcode.mode);
		}
		// ASL (Arithmetic Shift Left Accumulator) Accumulator???opecode????????????????????????
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
		0x00 => {
		    self.brk();
		    return
		}
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
		// DEC (Decrement Memory)
		0xC6 | 0xD6 | 0xCE | 0xDE => {
		    self.dec(&opcode.mode);
		}
		// DEX (Decrement X Register)
		0xCA => self.dex(),
		// DEY (Decrement Y Register)
		0x88 => self.dey(),
		// EOR (Exclusive OR)
		0x49 | 0x45 | 0x55 | 0x4D | 0x5D | 0x59 | 0x41 | 0x51 => {
		    self.eor(&opcode.mode);
		}
		// INC (Increment Memory)
		0xE6 | 0xF6 | 0xEE | 0xFE => {
		    self.inc(&opcode.mode);
		}
		// INX (Incremetn X Register)
		0xE8 => self.inx(),
		// INY (Increment Y Register)
		0xC8 => self.iny(),
		// JMP (Jump)
		0x4C | 0x6C => {
		    self.jmp(&opcode.mode);
		}
		// JSR (Jump to Subroutine)
		0x20 => self.jsr(&opcode.mode),
		// LDA (Load Accumulator)
		0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
		    self.lda(&opcode.mode);
		}
		// LDX (Load X Register)
		0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => {
		    self.ldx(&opcode.mode);
		}
		// LDY (Load Y Register)
		0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => {
		    self.ldy(&opcode.mode);
		}
		// LSR (Logic Shift Right Accumulator)
		0x4A => self.lsr_accumulator(),
		// LSR (Logic Shift Right)
		0x46 | 0x56 | 0x4E | 0x5E => {
		    self.lsr(&opcode.mode);
		}
		// NOP (No Operation)
		0xEA => self.nop(),
		// ORA (Logical Inclusive OR)
		0x09 | 0x05 | 0x15 | 0x0D | 0x1D | 0x19 | 0x01 | 0x11 => {
		    self.ora(&opcode.mode);
		}
		// PHA (Push Accumulator)
		0x48 => self.pha(),
		// PHP (Push Processor Status)
		0x08 => self.php(),
		// PLA (Pull Accumulator)
		0x68 => self.pla(),
		// PLP (Pull Processor Status)
		0x28 => self.plp(),
		// ROL (Rotate Left Accumulator)
		0x2A => self.rol_accumulator(),
		// ROL (Rotate Left)
		0x26 | 0x36 | 0x2E | 0x3E => {
		    self.rol(&opcode.mode);
		}
		// ROR (Rotate Right Accumulator)
		0x6A => self.ror_accumulator(),
		// ROR (Rotate Right)
		0x66 | 0x76 | 0x6E | 0x7E => {
		    self.ror(&opcode.mode);
		}
		// RTI (Return from Interrupt)
		0x40 => self.rti(),
		// RTS (Return from Subeourine)
		0x60 => self.rts(),
		// SBC (Sbstract with Carry)
		0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xE1 | 0xF1 => {
		    self.sbc(&opcode.mode);
		}
		// SEC (Set Carry Flag)
		0x38 => self.sec(),
		// SED (Set Decimal Flag)
		0xF8 => self.sed(),
		// SEI (SetInterrupt Disable)
		0x78 => self.sei(),
		// STA (Store Accumulator)
		0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => {
		    self.sta(&opcode.mode);
		}
		// STX (Store X Register)
		0x86 | 0x96 | 0x8E => {
		    self.stx(&opcode.mode);
		}
		// STY (Store Y Register)
		0x84 | 0x94 | 0x8C => {
		    self.sty(&opcode.mode);
		}
		// TAX (Transfer Accumulator to X)
		0xAA => self.tax(),
		// TAY (Transfer Accumulator to Y)
		0xA8 => self.tay(),
		// TSX (Transfer Stack Pointer to X)
		0xBA => self.tsx(),
		// TXA (Transfer X to Accumulator)
		0x8A => self.txa(),
		// TXS (Transfer X to Stack Pointer)
		0x9A => self.txs(),
		// TYA (Transfer Y to Accumulator)
		0x98 => self.tya(),

		
		// illegal opcodes
		// *ANC(AAC)
		0x0B | 0x2B => {
		    self.anc(&opcode.mode);
		}
		// *SAX(AAX)
		0x87 | 0x97 | 0x8F | 0x83 => {
		    self.sax(&opcode.mode);
		}
		// *ARR
		0x6B => self.arr(&opcode.mode),
		// *ALR
		0x4B => self.alr(&opcode.mode),
		// *LXA(ATX)
		0xAB => self.lxa(&opcode.mode),
		// *AHX(AXA)
		0x9F | 0x93 => self.ahx(&opcode.mode),
		// *AXS
		0xCB => self.axs(&opcode.mode),
		// *DCP
		0xC7 | 0xD7 | 0xCF | 0xDF | 0xDB | 0xD3 | 0xC3 => self.dcp(&opcode.mode),
		// *NOP(DOP) (No Operation)
		0x04 | 0x14 | 0x34 | 0x44 | 0x54 | 0x64 | 0x74 |
		0x80 | 0x82 | 0x89 | 0xC2 | 0xD4 | 0xE2 | 0xF4 => {
		    // let addr = self.get_operand_address(&opcode.mode);
		    // let value = self.mem_read(addr);
		    // do nothing
		    self.nop_dop();
		}
		// *ISB(ISC)
		0xE7 | 0xF7 | 0xEF | 0xFF | 0xFB | 0xE3 | 0xF3 => {
		    self.isb(&opcode.mode);
		}
		// *NOP(KIL)
		0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => {
		    self.kil();
		}
		// *LAS(LAR)
		0xBB => self.las(&opcode.mode),
		// *LAX
		0xA7 | 0xB7 | 0xAF | 0xBF | 0xA3 | 0xB3 => {
		    self.lax(&opcode.mode);
		}
		// *NOP(NOP)
		0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => {
		    self.nop();
		}
		// *RLA
		0x27 | 0x37 | 0x2F | 0x3F | 0x3B | 0x23 | 0x33 => {
		    self.rla(&opcode.mode);
		}
		// *RRA
		0x67 | 0x77 | 0x6F | 0x7F | 0x7B | 0x63 | 0x73 => {
		    self.rra(&opcode.mode);
		}
		// *SBC
		0xEB => self.sbc_ex(&opcode.mode),
		// *SLO
		0x07 | 0x17 | 0x0F | 0x1F | 0x1B | 0x03 | 0x13 => {
		    self.slo(&opcode.mode);
		}
		// *SRE
		0x47 | 0x57 | 0x4F | 0x5F | 0x5B | 0x43 | 0x53 => {
		    self.sre(&opcode.mode);
		}
		// *SHX(SXA)
		0x9E => self.shx(&opcode.mode),
		// *SHY(SYA)
		0x9C => self.shy(&opcode.mode),
		// *NOP(TOP)
		0x0C | 0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
		    self.nop_top();
		}
		// *XAA
		0x8B => self.xaa(&opcode.mode),
		// *TAS(XAS)
		0x9B => self.tas(&opcode.mode),
		// other opecode (crash)
		_ => self.crash(),
	    }

	    if program_counter_state == self.program_counter {
		self.program_counter += (opcode.len - 1) as u16;
	    }
	    // callback(self);
	}
    }
}


// #[cfg(test)]
// mod test {
//     use super::*;
//     // ADC
//     #[test]
//     fn test_0x69_adc_immediate() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x69, 0x10, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x01;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x11);
//     }
    
//     #[test]
//     fn test_0x69_adc_calc_with_carry() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x69, 0x10, 0x00]); 
// 	cpu.reset();
// 	cpu.status = 0x01; // carry flag
// 	cpu.register_a = 0x01;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x12);
//     }

//     #[test]
//     fn test_0x69_adc_set_carry() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x69, 0xFF, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x02;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x01);
// 	assert_eq!(cpu.status, 0x01);
//     }

//     #[test]
//     fn test_0x69_adc_overflow() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x69, 0x7F, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x01;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x80);
// 	assert_eq!(cpu.status, 0xC0); // overflow flag and negative flag
//     }

//     #[test]
//     fn test_0x69_adc_overflow_with_carry() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x69, 0x7F, 0x00]); 
// 	cpu.reset();
// 	cpu.status = 0x01;
// 	cpu.register_a = 0x01;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x81);
// 	assert_eq!(cpu.status, 0xC0); // overflow flag and negative flag
//     }

//     // AND
//     #[test]
//     fn test_0x29_and_immediate() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x29, 0xAE, 0x00]); // AE => 10101110
// 	cpu.reset();
// 	cpu.register_a = 0xF3;            // F3 => 11110011
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0xA2); // A2 => 10100010
//     }
    
//     // ASL
//     #[test]
//     fn test_0x0a_asl_accumulator() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x0A, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0x03;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x03 * 2);
//     }
    
//     #[test]
//     fn test_0x06_asl_zero_page() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x06, 0x01, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x0001, 0x03);
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x0001), 0x03 * 2);
//     }
    
//     // BCC
//     #[test]
//     fn test_0x90_bcc_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x90, 0x00]);
// 	cpu.reset();
// 	cpu.run();
// 	// PC???????????????0x8000
// 	// 0x90, 0x00???1++
// 	// 0x90?????????1++
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }   
//     // BCS
//     #[test]
//     fn test_0xb0_bcs_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xB0, 0x00]);
// 	cpu.reset();
// 	cpu.status = 0x01; // carry flag
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // BEQ
//     #[test]
//     fn test_0xf0_beq_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xB0, 0x00]);
// 	cpu.reset();
// 	cpu.status = 0x02; // zero flag
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // BIT
//     #[test]
//     fn test_0x24_bit_zeropage() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x24, 0x01, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write_u16(0x01, 0b1100_0000);
// 	cpu.register_a = 0b0000_0001; // 0b11000000 & 0b00000001 = 0 -> zeroflag up
// 	cpu.run();
// 	assert_eq!(cpu.status, 0b1100_0010);
//     }
    
//     #[test]
//     fn test_0x2c_bit_absolute() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x2C, 0x04, 0x80, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write_u16(0x8004, 0b1100_0000);
// 	cpu.register_a = 0b000_00001; // 0b11000000 & 0b00000001 = 0 -> zeroflag up
// 	cpu.run();
// 	assert_eq!(cpu.status, 0b1100_0010);
//     }
    
//     // BMI
//     #[test]
//     fn test_0x30_bmi_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x30, 0x00]);
// 	cpu.reset();
// 	cpu.status = 0x80; // zero flag
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // BNE
//     #[test]
//     fn test_0xd0_bne_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xD0, 0x00]);
// 	cpu.reset();
// 	cpu.status = !ZERO_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // BPL
//     #[test]
//     fn test_0x10_bpl_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x10, 0x00]);
// 	cpu.reset();
// 	cpu.status = !NEGATIVE_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // BRK
//     #[test]
//     fn test_0x00_brk_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x00]);
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.status, 0x00);
//     }
    
//     // BVC
//     #[test]
//     fn test_0x50_bvc_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x50, 0x00]);
// 	cpu.reset();
// 	cpu.status = !OVERFLOW_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // BVS
//     #[test]
//     fn test_0x70_bvs_relative() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x70, 0x00]);
// 	cpu.reset();
// 	cpu.status = OVERFLOW_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x8000 + 0x01 * 3);
//     }
    
//     // CLC
//     #[test]
//     fn test_0x18_clc_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x18, 0x00]);
// 	cpu.reset();
// 	cpu.status = CARRY_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.status, CLEAR_STATUS);
//     }
    
//     // CLD
//     #[test]
//     fn test_0xd8_cld_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xd8, 0x00]);
// 	cpu.reset();
// 	cpu.status = DECIMAL_MODE_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.status, CLEAR_STATUS);
//     }
    
//     // CLI
//     #[test]
//     fn test_0x58_cli_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x58, 0x00]);
// 	cpu.reset();
// 	cpu.status = INTERRUPT_DISABLE;
// 	cpu.run();
// 	assert_eq!(cpu.status, CLEAR_STATUS);
//     }
    
//     // CLV
//     #[test]
//     fn test_0xb8_clv_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xB8, 0x00]);
// 	cpu.reset();
// 	cpu.status = OVERFLOW_FLAG;
// 	cpu.run();
// 	assert_eq!(cpu.status, CLEAR_STATUS);
//     }
    
//     // CMP
//     #[test]
//     fn test_0xc9_cmp_immediate_a_equal_m() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xC9, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0x11;
// 	cpu.run();
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0xc9_cmp_immediate_a_bigger_m() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xC9, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0x12;
// 	cpu.run();
// 	assert_eq!(cpu.status, CARRY_FLAG);
//     }

//     #[test]
//     fn test_0xc9_cmp_immediate_negflg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xC9, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0xFF;
// 	cpu.run();
// 	assert_eq!(cpu.status, NEGATIVE_FLAG | CARRY_FLAG);
//     }
    
//     // CPX
//     #[test]
//     fn test_0xe0_cpx_immediate_x_equal_m() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xe0, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x11;
// 	cpu.run();
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0xe0_cpx_immediate_x_bigger_m() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xe0, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x12;
// 	cpu.run();
// 	assert_eq!(cpu.status, CARRY_FLAG);
//     }

//     #[test]
//     fn test_0xe0_cpx_immediate_negflg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xe0, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0xFF;
// 	cpu.run();
// 	assert_eq!(cpu.status, CARRY_FLAG | NEGATIVE_FLAG);
//     }

//     // CPY
//     #[test]
//     fn test_0xc0_cpy_immediate_y_equal_m() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xc0, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x11;
// 	cpu.run();
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0xc0_cpy_immediate_y_bigger_m() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xc0, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x12;
// 	cpu.run();
// 	assert_eq!(cpu.status, CARRY_FLAG);
//     }

//     #[test]
//     fn test_0xc0_cpy_immediate_negflg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xc0, 0x11, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0xFF;
// 	cpu.run();
// 	assert_eq!(cpu.status, CARRY_FLAG | NEGATIVE_FLAG);
//     }
    
//     // DEC
//     #[test]
//     fn test_0xc6_dec_zero_page_zero() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xc6, 0x02, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x0002, 0x01);
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x02), 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }
    
//     #[test]
//     fn test_0xc6_dec_zero_page_neg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xc6, 0x02, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x0002, 0xFF);
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x02), 0xFE);
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }
    
//     // DEX
//     #[test]
//     fn test_0xca_dex_implied_zero() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xca, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x01;
// 	cpu.run();
// 	assert_eq!(cpu.register_x, 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0xca_dex_implied_neg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xca, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0xFF;
// 	cpu.run();
// 	assert_eq!(cpu.register_x, 0xFE);
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }
    
//     // DEY
//     #[test]
//     fn test_0x88_dey_implied_zero() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x88, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x01;
// 	cpu.run();
// 	assert_eq!(cpu.register_y, 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0x88_dex_implied_neg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x88, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0xFF;
// 	cpu.run();
// 	assert_eq!(cpu.register_y, 0xFE);
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }

//     // EOR
//     #[test]
//     fn test_0x49_eor_immediate_zero() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x49, 0x11, 0x00]); // 0x11 => 0b10001
// 	cpu.reset();
// 	cpu.register_a = 0x11;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0x49_eor_immediate_neg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x49, 0x11, 0x00]); // 0x11 => 0b0001_0001
// 	cpu.reset();
// 	cpu.register_a = 0xFF;            // 0xFF => 0b1111_1111
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0xEE); // 0xaa => 0b1110_1110
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }
        
//     // INC
//     #[test]
//     fn test_0xe6_inc_zeropage_zero() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xe6, 0x02, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x02, 0xFF);
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x02), 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_0xe6_inc_zeropage_neg() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xe6, 0x02, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x02, 0x80);
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x02), 0x81);
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }
    
//     // INX
//     #[test]
//     fn test_inx_overflow() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA9, 0xFF, 0xAA,0xE8, 0xE8, 0x00]);
//         assert_eq!(cpu.register_x, 1);
//     }
    
//     // INY
//     #[test]
//     fn test_iny_0xc8_zero_flag() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xC8, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0xFF;
// 	cpu.run();
//         assert_eq!(cpu.register_y, 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG);
//     }

//     #[test]
//     fn test_iny_0xc8_neg_flag() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xC8, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x80;
// 	cpu.run();
//         assert_eq!(cpu.register_y, 0x81);
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }
    
//     // JMP
//     #[test]
//     fn test_0x4c_jmp_absolute() {
// 	let mut cpu = CPU::new();
//         cpu.load(vec![0x4C, 0x05, 0x10, 0x00]);	// 0x1005
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x1005 + 0x01); // 0x00???+1
//     }

//     #[test]
//     fn test_0x6c_jmp_indirect() {
// 	// no bug
// 	let mut cpu = CPU::new();
//         cpu.load(vec![0x6C, 0x05, 0x10, 0x00]);	// 0x1005
// 	cpu.reset();
// 	cpu.mem_write_u16(0x1005, 0x50);
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x50 + 0x01); // 0x00???+1
// 	// with bug
// 	let mut cpu = CPU::new();
//         cpu.load(vec![0x6C, 0xFF, 0x30, 0x00]);	// 0x30FF
// 	cpu.reset();
// 	cpu.mem_write_u16(0x3000, 0x40);
// 	cpu.mem_write_u16(0x30FF, 0x80);
// 	cpu.mem_write_u16(0x3100, 0x50); // ?????????0x5080??????????????????????????????0x4080??????????????????
// 	cpu.run(); // ??????8bit?????????????????????????????????JMP???????????????
// 	assert_eq!(cpu.program_counter, 0x4080 + 0x01); // 0x00???+1
//     }
    
//     // JSR
//     #[test]
//     fn test_0x20_jsr_absolute() {
// 	let mut cpu = CPU::new();
//         cpu.load(vec![0x20, 0x05, 0x10, 0x00]);	// 0x1005
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x1005 + 0x01); // 0x00???+1
// 	assert_eq!(cpu.stack_pointer, 0xFB);
//     }
    
//     // LDA
//     #[test]
//     fn test_0xa9_lda_immediate_load_data() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA9, 0x05, 0x00]);	
//         assert_eq!(cpu.register_a, 5);
//         assert!(cpu.status & 0b0000_0010 == 0);
//         assert!(cpu.status & 0b1000_0000 == 0);
//     }

//     #[test]
//     fn test_0xa9_lda_zero_page() {
// 	let mut cpu = CPU::new();
// 	cpu.mem_write(0x10, 0x55);
// 	cpu.load_and_run(vec![0xA5, 0x10, 0x00]);
// 	assert_eq!(cpu.register_a, 0x55);
//     }

//     #[test]
//     fn test_0xb5_lda_zero_page_x() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xB5, 0x0F, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x0F, 0x12);	
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x12);
//     }

//     #[test]
//     fn test_0xad_lda_absolute() {
// 	let mut cpu = CPU::new();
// 	cpu.load_and_run(vec![0xAD, 0x00, 0x80, 0x00]);
// 	assert_eq!(cpu.register_a, 0xAD);
//     }

//     #[test]
//     fn test_0xbd_lda_absolute_x() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xBD, 0x12, 0x34, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x02;
// 	cpu.mem_write_u16(0x3414, 0x34);	
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x34);
//     }

//     #[test]
//     fn test_0xb9_lda_absolute_y() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xB9, 0x12, 0x34, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x01;
// 	cpu.mem_write_u16(0x3413, 0x34);	
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x34);
//     }

//     // 0x10+0x02=0x12??????(0x34)???0x10+0x02+0x01=0x13??????(0x55)??????????????????????????????
//     // 0x5534?????????A???????????????????????????
//     #[test]
//     fn test_0xa1_lda_indirect_x() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xA1, 0x10, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x02;
// 	cpu.mem_write(0x12, 0x34);
// 	cpu.mem_write(0x13, 0x55);
// 	cpu.mem_write_u16(0x5534, 0x1F);
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x1F);
//     }

//     // 0x10??????(0x34)???0x11??????(0x55)??????????????????????????????
//     // 0x5534+0x02=0x5536?????????A???????????????????????????
//     #[test]
//     fn test_0xb1_lda_uindirect_y() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xB1, 0x10, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x02;
// 	cpu.mem_write(0x10, 0x34);
// 	cpu.mem_write(0x11, 0x55);
// 	cpu.mem_write_u16(0x5536, 0x1F);
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x1F);
//     }

//     #[test]
//     fn test_0xa9_lda_zero_flag() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA9, 0x00, 0x00]);	
//         assert!(cpu.status & 0b0000_0010 == 0b10);
//     }

//     // LDX
//     #[test]
//     fn test_0xa2_ldx_immediate() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA2, 0x05, 0x00]);	
//         assert_eq!(cpu.register_x, 5);
//         assert!(cpu.status & 0b0000_0010 == 0);
//         assert!(cpu.status & 0b1000_0000 == 0);
//     }
    
//     // LDY
//     #[test]
//     fn test_0xa0_ldy_immediate() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA0, 0x05, 0x00]);	
//         assert_eq!(cpu.register_y, 5);
//         assert!(cpu.status & 0b0000_0010 == 0);
//         assert!(cpu.status & 0b1000_0000 == 0);
//     }

//     // LSR
//     #[test]
//     fn test_0x4a_lsr_accumulator() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x4a, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x04;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x02);
//     }

//     #[test]
//     fn test_0x46_lsr_zero_page_carry_and_zero_flag() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x46, 0x10, 0x00]); 
// 	cpu.reset();
// 	cpu.mem_write(0x10, 0x01);
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x10), 0x00);
// 	assert_eq!(cpu.status, ZERO_FLAG | CARRY_FLAG);
//     }

//     // NEGATIVE_FLAG?????????????????????
//     // fn test_0x46_lsr_zero_page_negative_flag() {}
    
//     // NOP
//     #[test]
//     fn test_0xea_nop_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xEA, 0x00]); 
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.status, 0x00);
//     }
    
//     // ORA
//     #[test]
//     fn test_0x09_ora_immediate() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x09, 0x11, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x02;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x13);
//     }
    
//     // PHA
//     #[test]
//     fn test_0x48_pha_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x48, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x02;
// 	cpu.run();
// 	assert_eq!(cpu.stack_pointer, 0xFD - 0x01);
//     }
    
//     // PHP
//     #[test]
//     fn test_0x08_php_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x08, 0x00]); 
// 	cpu.reset();
// 	cpu.status = 0x02;
// 	cpu.run();
// 	assert_eq!(cpu.stack_pointer, 0b0011_0000 | 0xFD - 0x01);
// 	let tmp = STACK.wrapping_add(cpu.stack_pointer.into()).wrapping_add(1);
// 	let tmp_2 = 0x02 | BREAK_COMMAND | BREAK2_COMMAND;
// 	assert_eq!(cpu.mem_read(tmp), tmp_2);
//     }
    
//     // PLA
//     #[test]
//     fn test_0x68_pla_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x68, 0x00]); 
// 	cpu.reset();
// 	let tmp = STACK.wrapping_add(cpu.stack_pointer.into()).wrapping_add(1);
// 	cpu.mem_write(tmp, 0x81);
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x81);
// 	assert_eq!(cpu.status, NEGATIVE_FLAG);
//     }
    
//     // PLP
//     #[test]
//     fn test_0x28_plp_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x28, 0x00]); 
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.status, BREAK2_COMMAND);
//     }
    
//     // ROL
//     #[test]
//     fn test_0x2a_rol_accumulator() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x2A, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0xA1;            // 0b1010_0001
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x42); // 0b0100_0010
//     }

//     #[test]
//     fn test_0x26_rol_zero_page() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x26, 0x10, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x10, 0x50);            // 0b0101_0000
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x10), 0xA0); // 0b1010_0000
//     }
    
//     // ROR
//     #[test]
//     fn test_0x6a_ror_accumulator() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x6A, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0xA1;            // 0b1010_0001
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x50); // 0b0101_0000
//     }

//     #[test]
//     fn test_0x66_ror_zero_page() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x66, 0x10, 0x00]);
// 	cpu.reset();
// 	cpu.mem_write(0x10, 0xA1);            // 0b1010_0001
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x10), 0x42); // 0b0100_0010
//     }
    
    
//     // RTI
//     #[test]
//     fn test_0x40_rti_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x40, 0x00]);
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x01);
//     }
    
//     // RTS
//     #[test]
//     fn test_0x60_rts_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x60, 0x00]);
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.program_counter, 0x02);
//     }

//     // SBC A-M-(1-C)
//     #[test]
//     fn test_0xe9_sbc_immediate() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xE9, 0x01, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x10;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x0E);
//     }

//     #[test]
//     fn test_0xe9_sbc_calc_with_carry() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xE9, 0x10, 0x00]); 
// 	cpu.reset();
// 	cpu.status = 0x01; // carry flag
// 	cpu.register_a = 0x50;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x40);
//     }

//     #[test]
//     fn test_0xe9_sbc_set_carry() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xE9, 0x03, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x02;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0xFE);
// 	assert_eq!(cpu.status, 0x80);
//     }

//     #[test]
//     fn test_0xe9_sbc_overflow() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xE9, 0x81, 0x00]); 
// 	cpu.reset();
// 	cpu.register_a = 0x7F;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0xFD);
// 	assert_eq!(cpu.status, 0xC0); // overflow flag and negative flag
//     }

//     #[test]
//     fn test_0xe9_sbc_overflow_with_carry() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xE9, 0x7F, 0x00]); 
// 	cpu.reset();
// 	cpu.status = 0x01;
// 	cpu.register_a = 0x7E;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0xFF);
// 	assert_eq!(cpu.status, 0x80); // overflow flag and negative flag
//     }

//     // SEC
//     #[test]
//     fn test_0x38_sec_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x38, 0x00]); 
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.status, CARRY_FLAG);
//     }

//     // SED
//     #[test]
//     fn test_0xf8_sed_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xf8, 0x00]); 
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.status, DECIMAL_MODE_FLAG);
//     }
    
//     // SEI
//     #[test]
//     fn test_0x78_sei_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x78, 0x00]); 
// 	cpu.reset();
// 	cpu.run();
// 	assert_eq!(cpu.status, INTERRUPT_DISABLE);
//     }

//     // STA
//     #[test]
//     fn test_0x85_sta_immediate() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x85, 0xA8, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0x45;
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0xA8), 0x45);
//     }
    
//     // STX
//     #[test]
//     fn test_0x86_stx_zero_page() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x86, 0x10, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x50;
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x10), 0x50);
//     }
    
//     // STY
//     #[test]
//     fn test_0x84_sty_zero_page() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x84, 0x10, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x50;
// 	cpu.run();
// 	assert_eq!(cpu.mem_read(0x10), 0x50);
//     }

//     // TAX
//     #[test]
//     fn test_0xaa_tax_implied() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA9, 0x0A, 0xAA, 0x00]);	
//         assert_eq!(cpu.register_x, 10)
//     }

//     // TAY
//     #[test]
//     fn test_0xa8_tay_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xA8, 0x00]);
// 	cpu.reset();
// 	cpu.register_a = 0x10;
// 	cpu.run();
// 	assert_eq!(cpu.register_y, 0x10);
//     }
    
//     // TSX
//     #[test]
//     fn test_0xba_tsx_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0xBA, 0x00]);
// 	cpu.reset();
// 	cpu.stack_pointer = 0x35;
// 	cpu.run();
// 	assert_eq!(cpu.register_x, 0x35);
//     }
    
//     // TXA
//     #[test]
//     fn test_0x8a_tsa_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x8A, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x35;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x35);
//     }
    
//     // TXS
//     #[test]
//     fn test_0x9a_txs_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x9A, 0x00]);
// 	cpu.reset();
// 	cpu.register_x = 0x35;
// 	cpu.run();
// 	assert_eq!(cpu.stack_pointer, 0x35);
//     }
    
//     // TYA
//     #[test]
//     fn test_0x98_tya_implied() {
// 	let mut cpu = CPU::new();
// 	cpu.load(vec![0x98, 0x00]);
// 	cpu.reset();
// 	cpu.register_y = 0x35;
// 	cpu.run();
// 	assert_eq!(cpu.register_a, 0x35);
//     }

//     // other
//     #[test]
//     fn test_5_ops_working_together() {
//         let mut cpu = CPU::new();
//         cpu.load_and_run(vec![0xA9, 0xC0, 0xAA, 0xE8, 0x00]);
//         assert_eq!(cpu.register_x, 0xC1)
//     }

//     // game code
//     #[test]
//     fn test_game_code() {
// 	let mut cpu = CPU::new();
// 	let game_code = vec![
// 	    0x20, 0x06, 0x06, 0x20, 0x38, 0x06, 0x20, 0x0d, 0x06, 0x20, 0x2a, 0x06, 0x60, 0xa9, 0x02, 0x85,
// 	    0x02, 0xa9, 0x04, 0x85, 0x03, 0xa9, 0x11, 0x85, 0x10, 0xa9, 0x10, 0x85, 0x12, 0xa9, 0x0f, 0x85,
// 	    0x14, 0xa9, 0x04, 0x85, 0x11, 0x85, 0x13, 0x85, 0x15, 0x60, 0xa5, 0xfe, 0x85, 0x00, 0xa5, 0xfe,
// 	    0x29, 0x03, 0x18, 0x69, 0x02, 0x85, 0x01, 0x60, 0x20, 0x4d, 0x06, 0x20, 0x8d, 0x06, 0x20, 0xc3,
// 	    0x06, 0x20, 0x19, 0x07, 0x20, 0x20, 0x07, 0x20, 0x2d, 0x07, 0x4c, 0x38, 0x06, 0xa5, 0xff, 0xc9,
// 	    0x77, 0xf0, 0x0d, 0xc9, 0x64, 0xf0, 0x14, 0xc9, 0x73, 0xf0, 0x1b, 0xc9, 0x61, 0xf0, 0x22, 0x60,
// 	    0xa9, 0x04, 0x24, 0x02, 0xd0, 0x26, 0xa9, 0x01, 0x85, 0x02, 0x60, 0xa9, 0x08, 0x24, 0x02, 0xd0,
// 	    0x1b, 0xa9, 0x02, 0x85, 0x02, 0x60, 0xa9, 0x01, 0x24, 0x02, 0xd0, 0x10, 0xa9, 0x04, 0x85, 0x02,
// 	    0x60, 0xa9, 0x02, 0x24, 0x02, 0xd0, 0x05, 0xa9, 0x08, 0x85, 0x02, 0x60, 0x60, 0x20, 0x94, 0x06,
// 	    0x20, 0xa8, 0x06, 0x60, 0xa5, 0x00, 0xc5, 0x10, 0xd0, 0x0d, 0xa5, 0x01, 0xc5, 0x11, 0xd0, 0x07,
// 	    0xe6, 0x03, 0xe6, 0x03, 0x20, 0x2a, 0x06, 0x60, 0xa2, 0x02, 0xb5, 0x10, 0xc5, 0x10, 0xd0, 0x06,
// 	    0xb5, 0x11, 0xc5, 0x11, 0xf0, 0x09, 0xe8, 0xe8, 0xe4, 0x03, 0xf0, 0x06, 0x4c, 0xaa, 0x06, 0x4c,
// 	    0x35, 0x07, 0x60, 0xa6, 0x03, 0xca, 0x8a, 0xb5, 0x10, 0x95, 0x12, 0xca, 0x10, 0xf9, 0xa5, 0x02,
// 	    0x4a, 0xb0, 0x09, 0x4a, 0xb0, 0x19, 0x4a, 0xb0, 0x1f, 0x4a, 0xb0, 0x2f, 0xa5, 0x10, 0x38, 0xe9,
// 	    0x20, 0x85, 0x10, 0x90, 0x01, 0x60, 0xc6, 0x11, 0xa9, 0x01, 0xc5, 0x11, 0xf0, 0x28, 0x60, 0xe6,
// 	    0x10, 0xa9, 0x1f, 0x24, 0x10, 0xf0, 0x1f, 0x60, 0xa5, 0x10, 0x18, 0x69, 0x20, 0x85, 0x10, 0xb0,
// 	    0x01, 0x60, 0xe6, 0x11, 0xa9, 0x06, 0xc5, 0x11, 0xf0, 0x0c, 0x60, 0xc6, 0x10, 0xa5, 0x10, 0x29,
// 	    0x1f, 0xc9, 0x1f, 0xf0, 0x01, 0x60, 0x4c, 0x35, 0x07, 0xa0, 0x00, 0xa5, 0xfe, 0x91, 0x00, 0x60,
// 	    0xa6, 0x03, 0xa9, 0x00, 0x81, 0x10, 0xa2, 0x00, 0xa9, 0x01, 0x81, 0x10, 0x60, 0xa2, 0x00, 0xea,
// 	    0xea, 0xca, 0xd0, 0xfb, 0x60
// 	];
// 	cpu.load(game_code);
// 	cpu.reset();
// 	cpu.run();
//     }
// }

