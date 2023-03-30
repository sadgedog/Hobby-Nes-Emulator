use std::collections::HashMap;
use crate::opcodes;
use crate::bus::Bus;

// stack
const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xFD;

bitflags! {
    pub struct CpuFlags: u8 {
	const CLEAR_STATUS      = 0b0000_0000;
	// Processor Status
	const CARRY_FLAG        = 0b0000_0001;
	const ZERO_FLAG         = 0b0000_0010;
	const INTERRUPT_DISABLE = 0b0000_0100;
	const DECIMAL_MODE_FLAG = 0b0000_1000;
	const BREAK_COMMAND     = 0b0001_0000;
	const BREAK2_COMMAND    = 0b0010_0000;
	const OVERFLOW_FLAG     = 0b0100_0000;
	const NEGATIVE_FLAG     = 0b1000_0000;
    }
}

pub struct CPU<'a> {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: CpuFlags,
    pub program_counter: u16,
    pub stack_pointer: u8,
    // memory: [u8; 0xFFFF]
    pub bus: Bus<'a>,
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
    Indirect_jmp,
    NoneAddressing,
}

pub trait Mem {
    fn mem_read(&mut self, add: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
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

impl Mem for CPU<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
	self.bus.mem_read(addr)
    }

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
	self.bus.mem_read_u16(pos)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
	self.bus.mem_write(addr, data)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
	self.bus.mem_write_u16(pos, data)
    }
}

// メモリページサイズは256byte,
// [0x0000 .. 0x00FF], [0x0100 .. 0x01FF]
fn page_cross(base: u16, addr: u16) -> bool {
    base & 0xFF00 != addr & 0xFF00
}

impl<'a> CPU<'a> {
    pub fn new<'b>(bus: Bus<'b>) -> CPU<'b> {
	CPU {
	    register_a: 0,
	    register_x: 0,
	    register_y: 0,
	    status: CpuFlags::from_bits_truncate(0b100100),
	    program_counter: 0,
	    stack_pointer: STACK_RESET,
	    // memory: [0; 0xFFFF]
	    bus: bus,
	}
    }

    pub fn get_absolute_address(&mut self, mode: &AddressingMode, addr: u16) -> (u16, bool) {
	match mode {
	    // ページ跨りがあるのはAbsolute_X, Absolute_Y, Indirect_Y
	    // LDA $C0 -> A5 C0 (0xC0にある値をA_registerに代入 0 ~ 255)
	    AddressingMode::ZeroPage => (self.mem_read(addr) as u16, false),
	    // LDA $C000 -> AD 00 C0 (0xC000にある値をA_registerに代入 (0x0000~0xFFFFまで全て))
	    AddressingMode::Absolute => (self.mem_read_u16(addr), false),
	    // LDA $C0,X -> A5 C0+X (C0にregister_xの値を加算した場所の値をA_registerに代入) 
	    AddressingMode::ZeroPage_X => {
		let pos = self.mem_read(addr);
		let addr = pos.wrapping_add(self.register_x) as u16;
		(addr, false)
	    }
	    // LDA $C0,Y -> A5 C0+Y (C0にregister_yの値を加算した場所の値をA_registerに代入) 
	    AddressingMode::ZeroPage_Y => {
		let pos = self.mem_read(addr);
		let addr = pos.wrapping_add(self.register_y) as u16;
		(addr, false)
	    }
	    // LDA $C000,X -> BD 00 C0 (C000にregister_xの値を加算した場所の値をA_registerに代入)
	    AddressingMode::Absolute_X => {
		let base = self.mem_read_u16(addr);
		let addr = base.wrapping_add(self.register_x as u16);
		(addr, page_cross(base, addr))
	    }
	    // LDA $C000,Y -> B9 00 C0 (C000にregister_yの値を加算した場所の値をA_registerに代入)
	    AddressingMode::Absolute_Y => {
		let base = self.mem_read_u16(addr);
		let addr = base.wrapping_add(self.register_y as u16);
		(addr, page_cross(base, addr))
	    }
	    // LDA ($C0,X) -> A1 C0+X+1
	    AddressingMode::Indirect_X => {
		let base = self.mem_read(addr);
		let ptr: u8 = (base as u8).wrapping_add(self.register_x);
		let lo = self.mem_read(ptr as u16);
		let hi = self.mem_read(ptr.wrapping_add(1) as u16);
		((hi as u16) << 8 | (lo as u16), false)
	    }
	    // LDA ($C0,Y) -> B1 C0+1+Y
	    AddressingMode::Indirect_Y => {
		// let base = self.mem_read(self.program_counter);
		let base = self.mem_read(addr);
		let lo = self.mem_read(base as u16);
		let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
		let deref_base = (hi as u16) << 8 | (lo as u16);
		let deref = deref_base.wrapping_add(self.register_y as u16);
		(deref, page_cross(deref_base, deref))
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
		(indirect_ref, false)
	    }
	    // Undefined Addressing
	    AddressingMode::NoneAddressing => {
		panic!("mode {:?} is NoneAddressing", mode);
	    }

	    _ => {
		panic!("mode {:?} is not supported", mode);
	    }
	}	
    }

    fn get_operand_address(&mut self, mode: &AddressingMode) -> (u16, bool) {
	match mode {
	    AddressingMode::Immediate => (self.program_counter, false),
	    _ => self.get_absolute_address(mode, self.program_counter),
	}
    }
    
    fn add_to_register_a(&mut self, value: u8) {
	let result = self.register_a as u16
	    + value as u16
	    + (if self.status.contains(CpuFlags::CARRY_FLAG) {
		1
	    } else {
		0
	    }) as u16;
	
	// 0xFFより大きければキャリーは立つ
	let carry = result > 0xFF;
	if carry {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}

	let tmp = result as u8;

	// overflow check
	if (value ^ tmp) & (tmp ^ self.register_a) & 0x80 != 0 {
	    self.status.insert(CpuFlags::OVERFLOW_FLAG);
	} else {
	    self.status.remove(CpuFlags::OVERFLOW_FLAG);
	}
	
	self.set_register_a(tmp);
    }

    fn set_register_a(&mut self, value: u8) {
	self.register_a = value;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
	if result == 0 {
	    self.status.insert(CpuFlags::ZERO_FLAG);
	} else {
	    self.status.remove(CpuFlags::ZERO_FLAG);
	}
	
	if result & 0b1000_0000 != 0 {
	    self.status.insert(CpuFlags::NEGATIVE_FLAG);
	} else {
	    self.status.remove(CpuFlags::NEGATIVE_FLAG);
	}
    }

    fn update_negative_flags(&mut self, result: u8) {
	if result >> 7 == 1 {
	    self.status.insert(CpuFlags::NEGATIVE_FLAG);
	} else {
	    self.status.remove(CpuFlags::NEGATIVE_FLAG);
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
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.add_to_register_a(value);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn and(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(value & self.register_a);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn asl_accumulator(&mut self) {
	let mut data = self.register_a;
	// 7bitが設定されている場合
	if data >> 7 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG); // set carry flag
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG); // remove carry flag
	}
	data = data << 1;
	self.register_a = data;
	self.update_zero_and_negative_flags(self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) -> u8 {
	let (addr, _) = self.get_operand_address(&mode);
	let mut data = self.mem_read(addr);
	// 7bitが設定されている場合
	if data >> 7 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG); // set carry flag
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG); // remove carry flag
	}
	data = data << 1;
	self.mem_write(addr, data);
	self.update_zero_and_negative_flags(data);
	data
    }

    // Branch opecode (bcc, bcs, ...)
    fn branch(&mut self) {
	// 分岐するならクロック時間消費
	self.bus.tick(1);
	let branch: i8 = self.mem_read(self.program_counter) as i8;
	let branch_addr = self
	    .program_counter.
	    wrapping_add(1).
	    wrapping_add(branch as u16);

	if self.program_counter.wrapping_add(1) & 0xFF00 != branch_addr & 0xFF00 {
	    self.bus.tick(1);
	}
	self.program_counter = branch_addr;
    }

    fn bcc(&mut self) {
	// CARRY FLAGがセットされていない場合
	// PC += PCアドレスの値+1
	if !self.status.contains(CpuFlags::CARRY_FLAG) {
	    self.branch();
	}
    }

    fn bcs(&mut self) {
	// CARRY FLAGがセットされている場合
	// PC += PCアドレスの値+1
	if self.status.contains(CpuFlags::CARRY_FLAG) {
	    self.branch();
	}
    }

    fn beq(&mut self) {
	if self.status.contains(CpuFlags::ZERO_FLAG) {
	    self.branch();
	}
    }

    fn bit(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let tmp = self.register_a & value;
	if tmp == 0 {
	    self.status.insert(CpuFlags::ZERO_FLAG);
	} else {
	    self.status.remove(CpuFlags::ZERO_FLAG);
	}
	if value & 0b1000_0000 > 0 {
	    self.status.insert(CpuFlags::NEGATIVE_FLAG);
	} else {
	    self.status.remove(CpuFlags::NEGATIVE_FLAG);
	}
	if value & 0b0100_0000 > 0 {
	    self.status.insert(CpuFlags::OVERFLOW_FLAG);
	} else {
	    self.status.remove(CpuFlags::OVERFLOW_FLAG);
	}
    }

    fn bmi(&mut self) {
	// negative flag
	if self.status.contains(CpuFlags::NEGATIVE_FLAG) {
	    self.branch();
	}
    }

    fn bne(&mut self) {
	if !self.status.contains(CpuFlags::ZERO_FLAG) {
	    self.branch();
	}
    }

    fn bpl(&mut self) {
	if !self.status.contains(CpuFlags::NEGATIVE_FLAG) {
	    self.branch();
	}
    }

    fn brk(&mut self) {
    }

    fn bvc(&mut self) {
	if !self.status.contains(CpuFlags::OVERFLOW_FLAG) {
	    self.branch();
	}
    }

    fn bvs(&mut self) {
	// if self.status & OVERFLOW_FLAG == OVERFLOW_FLAG {
	if self.status.contains(CpuFlags::OVERFLOW_FLAG) {
	    self.branch();
	}
    }

    fn clc(&mut self) {
	self.status.remove(CpuFlags::CARRY_FLAG);
    }

    fn cld(&mut self) {
	self.status.remove(CpuFlags::DECIMAL_MODE_FLAG);
    }

    fn cli(&mut self) {
	self.status.remove(CpuFlags::INTERRUPT_DISABLE);
    }

    fn clv(&mut self) {
	self.status.remove(CpuFlags::OVERFLOW_FLAG);
    }

    fn compare(&mut self, cmp_data: u8, value: u8) {
	if cmp_data >= value {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
	let res = cmp_data.wrapping_sub(value);
	self.update_zero_and_negative_flags(res);
    }

    fn cmp(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.compare(self.register_a, value);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn cpx(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.compare(self.register_x, value);
    }

    fn cpy(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.compare(self.register_y, value);
    }

    fn dec(&mut self, mode: &AddressingMode) -> u8 {
	let (addr, _) = self.get_operand_address(&mode);
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
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_a ^= value;
	self.update_zero_and_negative_flags(self.register_a);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn inc(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
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
	let (addr, _) = self.get_operand_address(&mode);
	self.program_counter = addr;
    }

    fn jsr(&mut self, mode: &AddressingMode) {
	self.stack_push_u16(self.program_counter + 2 - 1);
	let (addr, _) = self.get_operand_address(&mode);
	self.program_counter = addr;
    }

    fn lda(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_a = value;
	self.set_register_a(value);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn ldx(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_x = value;
	self.update_zero_and_negative_flags(self.register_x);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn ldy(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.register_y = value;
	self.update_zero_and_negative_flags(self.register_y);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn lsr_accumulator(&mut self) {
	let mut data = self.register_a;
	if data & 1 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
	data = data >> 1; // data * 2
	self.set_register_a(data)
    }

    fn lsr(&mut self, mode: &AddressingMode) -> u8 {
	let (addr, _) = self.get_operand_address(mode);
	let mut value = self.mem_read(addr);
	if value & 1 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
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
	let (addr, page_cross) = self.get_operand_address(mode);
	let value = self.mem_read(addr);
	self.set_register_a(value | self.register_a);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn pha(&mut self) {
	self.stack_push(self.register_a);
    }
    
    fn php(&mut self) {
	let mut copy = self.status.clone();
	copy.insert(CpuFlags::BREAK_COMMAND);
	copy.insert(CpuFlags::BREAK2_COMMAND);
	self.stack_push(copy.bits());
    }
    
    fn pla(&mut self) {
	let value = self.stack_pop();
	self.set_register_a(value);
    }
    
    fn plp(&mut self) {
	self.status.bits = self.stack_pop();
	self.status.remove(CpuFlags::BREAK_COMMAND);
	self.status.insert(CpuFlags::BREAK2_COMMAND);
    }

    fn rol_accumulator(&mut self) {
	let mut value = self.register_a;
	let tmp = self.status.contains(CpuFlags::CARRY_FLAG);
	if value >> 7 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
	// left shift
	value = value << 1;
	if tmp {
	    value |= 1;
	}
	self.set_register_a(value);
    }

    fn rol(&mut self, mode: &AddressingMode) -> u8 {
	let (addr, _) = self.get_operand_address(mode);
	let mut value = self.mem_read(addr);
	let tmp = self.status.contains(CpuFlags::CARRY_FLAG);
	
	if value >> 7 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
	value = value << 1;
	if tmp {
	    value |= 1;
	}
	self.mem_write(addr, value);
	self.update_negative_flags(value);
	value
    }

    fn ror_accumulator(&mut self) {
	let mut value = self.register_a;
	let tmp = self.status.contains(CpuFlags::CARRY_FLAG);
	if value & 1 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
	// right shift
	value = value >> 1;
	if tmp {
	    value |= 0b1000_0000;
	}
	self.set_register_a(value);
    }
    
    fn ror(&mut self, mode: &AddressingMode) -> u8 {
	let (addr, _) = self.get_operand_address(mode);
	let mut value = self.mem_read(addr);
	let tmp = self.status.contains(CpuFlags::CARRY_FLAG);

	if value & 1 == 1 {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
	value = value >> 1;
	if tmp {
	    value |= 0b1000_0000;
	} else {
	    value &= !0b1000_0000;
	}
	self.mem_write(addr, value);
	self.update_negative_flags(value);
	value
    }

    fn rti(&mut self) {
	self.status.bits = self.stack_pop();
	self.status.remove(CpuFlags::BREAK_COMMAND);
	self.status.insert(CpuFlags::BREAK2_COMMAND);
	self.program_counter = self.stack_pop_u16();
    }

    fn rts(&mut self) {
	self.program_counter = self.stack_pop_u16() + 1;
    }

    fn sbc(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.add_to_register_a(((value as i8).wrapping_neg().wrapping_sub(1)) as u8);
	if page_cross {
	    self.bus.tick(1);
	}
    }

    fn sec(&mut self) {
	self.status.insert(CpuFlags::CARRY_FLAG);
    }

    fn sed(&mut self) {
	self.status.insert(CpuFlags::DECIMAL_MODE_FLAG);
    }

    fn sei(&mut self) {
	self.status.insert(CpuFlags::INTERRUPT_DISABLE);
    }

    fn sta(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(mode);
	self.mem_write(addr, self.register_a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	self.mem_write(addr, self.register_x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
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
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	// self.register_a &= value;
	// self.update_zero_and_negative_flags(self.register_a);
	self.set_register_a(self.register_a & value);
	if self.status.contains(CpuFlags::NEGATIVE_FLAG) {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	} else {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	}
    }
    
    // not confirmed
    fn sax(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let res = self.register_x & self.register_a;
	self.mem_write(addr, res);
    }
    
    // not confirmed
    fn arr(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	
	self.set_register_a(self.register_a & value);
	
	self.ror_accumulator();
	let res_bit_5 = (self.register_a >> 5) & 1;
	let res_bit_6 = (self.register_a >> 6) & 1;

	if (res_bit_5 == 1) & (res_bit_6 == 1) {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	    self.status.remove(CpuFlags::OVERFLOW_FLAG);
	} else if (res_bit_5 == 0) & (res_bit_6 == 0) {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	    self.status.remove(CpuFlags::OVERFLOW_FLAG);
	} else if (res_bit_5 == 1) & (res_bit_6 == 0) {
	    self.status.remove(CpuFlags::CARRY_FLAG);
	    self.status.insert(CpuFlags::OVERFLOW_FLAG);
	} else if (res_bit_5 == 0) & (res_bit_6 == 1) {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	    self.status.remove(CpuFlags::OVERFLOW_FLAG);
	}
	self.update_zero_and_negative_flags(self.register_a);
    }
    
    // not confirmed
    fn alr(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(self.register_a & value);
	self.lsr_accumulator();
    }

    // not confirmed
    fn lxa(&mut self, mode: &AddressingMode) {
	let (addr, page_cross) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	self.set_register_a(self.register_a & value);
	self.register_x = self.register_a;
	self.update_zero_and_negative_flags(self.register_a);
	if page_cross {
	    self.bus.tick(1);
	}
    }
    
    // not confirmed
    fn ahx(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = self.register_x & self.register_a & (addr >> 8) as u8;
	self.mem_write(addr, res);
    }
    
    // not confirmed
    fn axs(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let tmp = self.register_x & self.register_a;
	let res = tmp.wrapping_sub(tmp);
	if value <= tmp {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	}
	self.update_zero_and_negative_flags(res);
	self.register_x = res;
    }

    fn dcp(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = value.wrapping_sub(1);
	self.mem_write(addr, res);
	if res <= self.register_a {
	    self.status.insert(CpuFlags::CARRY_FLAG);
	}
	self.update_zero_and_negative_flags(self.register_a.wrapping_sub(res));
    }

    fn nop_dop(&mut self) {
	return
    }

    fn isb(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
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
	let (addr, _) = self.get_operand_address(&mode);
	let value = self.mem_read(addr);
	let res = value & self.stack_pointer;
	self.register_a = res;
	self.register_x = res;
	self.stack_pointer = res;
	self.update_zero_and_negative_flags(res);
    }
    
    fn lax(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
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
	let (addr, _) = self.get_operand_address(&mode);
	let tmp = (addr >> 8 as u8).wrapping_add(1) as u8;
	let res = self.register_x & tmp;
	self.mem_write(addr, res);
    }

    // not confirmed
    fn shy(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
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
	let (addr, _) = self.get_operand_address(mode);
	let data = self.mem_read(addr);
	self.set_register_a(data & self.register_a);
    }
    
    // not confirmed
    fn tas(&mut self, mode: &AddressingMode) {
	let (addr, _) = self.get_operand_address(&mode);
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
	for i in 0..(program.len() as u16) {
	    self.mem_write(0x0600 + i, program[i as usize]);
	}
    }

    pub fn reset(&mut self) {
	self.register_a = 0;
	self.register_x = 0;
	self.register_y = 0;
	self.stack_pointer = STACK_RESET;
	// self.status = 0;
	self.status = CpuFlags::from_bits_truncate(0b100100);
	// 0xFFFC, 0xFFFDにはloadの時点で0x00,0x80つまり0x8000が入っているはず
	self.program_counter = self.mem_read_u16(0xFFFC);
    }

    fn crash(&mut self) {
	let pc = self.program_counter.wrapping_sub(1); 
	panic!("unexpected opecode was executed {:?} ", self.mem_read(pc));
    }

    fn interrupt_nmi(&mut self) {
	self.stack_push_u16(self.program_counter);
	let mut flag = self.status.clone();
	flag.set(CpuFlags::BREAK_COMMAND, false);
	flag.set(CpuFlags::BREAK2_COMMAND, true);
	
	self.stack_push(flag.bits);
	self.status.insert(CpuFlags::INTERRUPT_DISABLE);

	self.bus.tick(2);
	self.program_counter = self.mem_read_u16(0xFFFA);
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
	    if let Some(_nmi) = self.bus.poll_nmi_status() {
		self.interrupt_nmi();
	    }
	    
	    callback(self);
	    // 0x8000の値(命令)を読み込む
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
		    // TODO: CHECK
		    let (addr, page_cross) = self.get_operand_address(&opcode.mode);
		    let value = self.mem_read(addr);
		    if page_cross {
			self.bus.tick(1);
		    }
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

	    self.bus.tick(opcode.cycles);

	    if program_counter_state == self.program_counter {
		self.program_counter += (opcode.len - 1) as u16;
	    }
	    // callback(self);
	}
    }
}
