use crate::cpu::Mem;
use crate::cartridge::Rom;
use crate::ppu::NesPPU;
// use crate::ppu::PPU;

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
    cpu_vram: [u8; 2048],
    // rom: Rom,
    prg_rom: Vec<u8>,
    ppu: NesPPU,
}

impl Bus {
    pub fn new(rom: Rom) -> Self {
	let ppu = NesPPU::new(rom.chr_rom, rom.screen_mirroring);
	
	Bus {
	    cpu_vram: [0; 2048],
	    // rom: rom,
	    prg_rom: rom.prg_rom,
	    ppu: ppu,
	}
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
	addr -= 0x8000;
	if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
	    addr = addr % 0x4000;
	}
	self.prg_rom[addr as usize]
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
	    0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
		panic!("Attempt to read from write_only PPU address {:x}", addr);
	    }

	    // TODO : ppuをtraitにしてpubにするのかな？
	    // 0x2007 => self.ppu.read_data(),

	    0x2008..= PPU_REGISTERS_MIRRORS_END => {
		let mirror_down_addr = addr & 0b00100000_00000111;
		self.mem_read(mirror_down_addr)
	    }	    
	    // PPU_REGISTERS ..= PPU_REGISTERS_MIRRORS_END => {
	    // 	let _mirror_down_addr = addr & 0b0010_0000_0000_0111;
	    // 	// todo!("PPU implement")
	    // 	0
	    // }
	    0x8000..=0xFFFF => self.read_prg_rom(addr),
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
		// todo!("PPU implement");
	    }
	    _ => {
		println!("Ignoring mem write-access at {:x}", addr);
	    }
	}
    }
}
