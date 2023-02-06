use crate::cpu::Mem;

//
// -------  0x2000
//
// CPU RAM
//
// -------  0x0000

//
// ----- 0x0800
//  RAM
// ----- 0x0000

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;

pub struct Bus {
    cpu_vram: [u8; 2048]
}

impl Bus {
    pub fn new() -> Self {
	Bus {
	    cpu_vram: [0; 2048]
	}
    }
}

impl Mem for Bus {
    fn mem_read(&self, addr: u16) -> u8 {
	match addr {
	    RAM ..= RAM_MIRRORS_END => {
		// CPUは0x0000~0x2000の13bitをRAM用に確保してる
		// RAMは11pinでCPUは16pinなので, 11bitに調整しないといけない
		let mirror_down_addr = addr & 0b00000_0111_1111_1111;
		self.cpu_vram[mirror_down_addr as usize]
	    }
	    PPU_REGISTERS ..= PPU_REGISTERS_MIRRORS_END => {
		let _mirror_down_addr = addr & 0b0010_0000_0000_0111;
		todo!("PPU implement")
	    }
	    _ => {
		println!("Ignoring mem access at {:x}", addr);
		0
	    }
	}
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
	match addr {
	    RAM ..= RAM_MIRRORS_END => {
		let mirror_down_addr = addr & 0b11111111111;
		self.cpu_vram[mirror_down_addr as usize] = data;
	    }
	    PPU_REGISTERS ..= PPU_REGISTERS_MIRRORS_END => {
		let _mirror_down_addr = addr & 0b0010_0000_0000_0111;
		todo!("PPU implement");
	    }
	    _ => {
		println!("Ignoring mem write-access at {:x}", addr);
	    }
	}
    }
}
