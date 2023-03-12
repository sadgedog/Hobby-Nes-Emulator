use crate::cartridge::Mirroring;
use registers::addr::AddrRegister;
use registers::control::ControlRegister;

pub mod registers;

pub struct NesPPU {
    // カートリッジに保存されている画像に関するデータ
    pub chr_rom: Vec<u8>,
    // 画面で使用するパレットテーブルのデータを保持するための内部メモリ
    pub palette_table: [u8: 32],
    // 背景情報を保持する内部メモリ
    pub vram: [u8: 2048],
    // スプライト情報を保持する内部メモリ
    // スプライト：背景画像の上にコマ送りでキャラクターを描画する技術らしい
    pub oam_data: [u8: 256],
    pub mirroring: Mirroring,
    // ./registers/addr.rs
    pub addr: AddrRegister,
    // ./registers/control.rs
    pub ctrl: ControlRegister,
    // 内部バッファ
    internal_data_buf: u8,
}

impl NesPPU {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
	NesPPU {
	    chr_rom: chr_rom,
	    mirroring: mirroring,
	    vram: [0; 2048],
	    oam_data: [0; 64 * 4],
	    palette_table: [0; 32],
	}
    }

    fn write_to_ppu_addr(&mut self, value: u8) {
	self.addr.update(value);
    }


    fn increment_vram_addr(&mut self) {
	self.addr.increment(self.ctrl.vram_addr_increment());
    }
    
    // Horizontal:
    //   [ A ] [ a ]
    //   [ B ] [ b ]
    
    // Vertical:
    //   [ A ] [ B ]
    //   [ a ] [ b ]
    pub fn mirror_vram_addr(&self, addr: u16) -> u16 {
	let mirrored_vram = addr & 0b1011_1111_1111_11;
	let vram_index = mirrored_vram - 0x2000;
	let name_table = vram_index / 0x400;
	match (&self.mirroring, name_table) {
	    (Mirroring::VERTICAL, 2) | (Mirroring::VERTICAL, 3) => vram_index - 0x800,
	    (Mirroring::HORIZONTAL, 2) => vram_index - 0x400,
	    (Mirroring::HORIZONTAL, 1) => vram_index - 0x400,
	    (Mirroring::HORIZONTAL, 3) => vram_index - 0x800,
	    _ => vram_index,
	}
    }

    fn read_data(&mut self) -> u8 {
	let addr = self.addr.get();
	self.increment_vram_addr();

	match addr {
	    // read from CHR ROM
	    0..=0x1FFF => {
		let result = self.internal_data_buf;
		self.internal_data_bud = self.chr_rom[addr as usize];
		result
	    }
	    // read from RAM
	    0x2000..=0x2FFF => {
		let result = self.internal_data_buf;
		self.internal_data_buf = self.vram[self.mirror_vram_addr(addr) as usize];
		result
	    }
	    0x3000..=0x3EFF => panic!("addr space 0x3000..0x3EFF is not expected to be used, requested = {}", addr),
	    0x3F00..=0x3FFF => {
		self.palette_table[(addr - 0x3F00) as usize]
	    }
	    _ => panic!("unexpected access to mirrored space {}", addr),
	}
    }
}
