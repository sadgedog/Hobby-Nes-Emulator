use crate::cartridge::Mirroring;
use registers::addr::AddrRegister;
use registers::control::ControlRegister;
use registers::mask::MaskRegister;
use registers::status::StatusRegister;
use registers::scroll::ScrollRegister;
use registers::oam::OamRegisters;

pub mod registers;

pub struct NesPPU {
    // カートリッジに保存されている画像に関するデータ
    pub chr_rom: Vec<u8>,
    // PPUミラーリング
    pub mirroring: Mirroring,
    // 背景情報を保持する内部メモリ
    pub vram: [u8; 2048],
    // スプライト情報を保持する内部メモリ
    // スプライト：背景画像の上にコマ送りでキャラクターを描画する技術らしい
    pub oam_addr: u8,
    pub oam_data: [u8; 256],
    pub oam: OamRegisters,
    // 画面で使用するパレットテーブルのデータを保持するための内部メモリ
    pub palette_table: [u8; 32],
    // 内部バッファ(addr)
    internal_data_buf: u8,
    // ./registers/addr.rs
    pub addr: AddrRegister,
    // ./registers/control.rs
    pub ctrl: ControlRegister,
    // ./registers/mask.rs
    pub mask: MaskRegister,
    // ./registers/sutatus.rs
    pub status: StatusRegister,
    // ./registers/scroll.rs
    pub scroll: ScrollRegister,
    // NMI interrupt
    scanline: u16,
    cycles: usize,
    pub nmi_interrupt: Option<u8>,
}

pub trait PPU {
    fn write_to_ppu_addr(&mut self, value: u8);
    fn write_to_ctrl(&mut self, value: u8);
    fn read_data(&mut self) -> u8;
    fn write_to_mask(&mut self, value: u8);
    fn read_status(&mut self) -> u8;
    fn write_to_scroll(&mut self, value: u8);
    fn write_to_data(&mut self, value: u8);
    fn write_to_oam_addr(&mut self, value: u8);
    fn write_to_oam_data(&mut self, value: u8);
    fn read_oam_data(&mut self) -> u8;
    fn write_oam_dma(&mut self, value: &[u8; 256]);
}

impl NesPPU {
    pub fn new_empty_rom() -> Self {
	NesPPU::new(vec![0; 2048], Mirroring::HORIZONTAL)
    }
    
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
	NesPPU {
	    chr_rom: chr_rom,
	    mirroring: mirroring,
	    vram: [0; 2048],
	    oam: OamRegisters::new(),
	    oam_addr: 0,
	    oam_data: [0; 64 * 4],
	    palette_table: [0; 32],
	    internal_data_buf: 0,
	    addr: AddrRegister::new(),
	    ctrl: ControlRegister::new(),
	    mask: MaskRegister::new(),
	    status: StatusRegister::new(),
	    scroll: ScrollRegister::new(),
	    scanline: 0,
	    cycles: 0,
	    nmi_interrupt: None,
	}
    }

    // addr register
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

    // NMI Interrupt
    pub fn tick(&mut self, cycles: u8) -> bool {
	self.cycles += cycles as usize;
	if self.cycles >= 341 {
	    self.cycles = self.cycles - 341;
	    self.scanline += 1;

	    if self.scanline == 241 {
		self.status.set_vblank_started(true);
		self.status.set_sprite_zero_hit(false);
		if self.ctrl.generate_nmi() {
		    // self.status.set_vblank_started(true);
		    self.nmi_interrupt = Some(1);
		}
	    }

	    if self.scanline >= 262 {
		self.scanline = 0;
		self.nmi_interrupt = None;
		self.status.set_sprite_zero_hit(false);
		self.status.reset_vblank_started();
		return true;
	    }
	}
	return false;
    }

    pub fn poll_nmi_interrupt(&mut self) -> Option<u8> {
	self.nmi_interrupt.take()
    }
}

impl PPU for NesPPU {
    // addr register
    fn write_to_ppu_addr(&mut self, value: u8) {
	self.addr.update(value);
    }

    // control
    fn write_to_ctrl(&mut self, value: u8) {
	let before_nmi_status = self.ctrl.generate_nmi();
	self.ctrl.update(value);
	if !before_nmi_status &&
	    self.ctrl.generate_nmi() &&
	    self.status.check_vblank_started() {
	    self.nmi_interrupt = Some(1);
	}
    }
    
    // addr register
    fn read_data(&mut self) -> u8 {
	let addr = self.addr.get();
	self.increment_vram_addr();

	match addr {
	    // read from CHR ROM
	    0..=0x1FFF => {
		let result = self.internal_data_buf;
		self.internal_data_buf = self.chr_rom[addr as usize];
		result
	    }
	    // PPU addr register
	    // read from RAM
	    0x2000..=0x2FFF => {
		let result = self.internal_data_buf;
		self.internal_data_buf = self.vram[self.mirror_vram_addr(addr) as usize];
		result
	    }
	    
	    0x3000..=0x3EFF => panic!("addr space 0x3000..0x3EFF is not expected to be used, requested = {}", addr),
	    0x3F10 | 0x3F14 | 0x3F18 | 0x3F1C => {
		let add_mirror = addr - 0x10;
		self.palette_table[(add_mirror - 0x3F00) as usize]
	    }
	    
	    0x3F00..=0x3FFF => {
		self.palette_table[(addr - 0x3F00) as usize]
	    }
	    _ => panic!("unexpected access to mirrored space {}", addr),
	}
    }

    // mask
    fn write_to_mask(&mut self, value: u8) {
	self.mask.update(value);
    }

    // status
    // ステータスを読み込むと、VBlankとScroll、PPU_Addrのラッチがクリアされる
    fn read_status(&mut self) -> u8 {
	let data = self.status.get_status();
	// reset
	self.status.reset_vblank_started();
	self.addr.reset_latch();
	self.scroll.reset_latch();
	data
    }

    // scroll
    fn write_to_scroll(&mut self, value: u8) {
	self.scroll.write(value);
    }

    // oam addr
    fn write_to_oam_addr(&mut self, value: u8) {
	self.oam.write_addr(value);
	self.oam_addr = self.oam.oam_addr;
    }

    // oam data
    fn write_to_oam_data(&mut self, value: u8) {
	self.oam.write_data(value);
	self.oam_data = self.oam.oam_data;
    }

    fn read_oam_data(&mut self) -> u8 {
	self.oam.get_data()
    }

    fn write_oam_dma(&mut self, data: &[u8; 256]) {
	self.oam.write_dma(data);
	self.oam_addr = self.oam.oam_addr;
	self.oam_data = self.oam.oam_data;
    }

    fn write_to_data(&mut self, value: u8) {
	let addr = self.addr.get();
	match addr {
	    0..=0x1FFF => println!("attempt to write to CHR ROM space {}", addr),
	    0x2000..=0x2FFF => {
		self.vram[self.mirror_vram_addr(addr) as usize] = value;
	    }
	    0x3000..=0x3EFF => unimplemented!("addr {:x} shouldnt be used in reality", addr),

	    0x3F10 | 0x3F14 | 0x3F18 | 0x3F1C => {
		let addr_mirror = addr - 0x10;
		self.palette_table[(addr_mirror - 0x3F00) as usize] = value;
	    }
	    0x3F00..=0x3FFF => {
		self.palette_table[(addr - 0x3F00) as usize] = value;
	    }
	    _ => panic!("unexpected access to mirrored space {:x}", addr),
	}
	self.increment_vram_addr();
    }
}


#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_ppu_vram_writes() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);
        ppu.write_to_data(0x66);

        assert_eq!(ppu.vram[0x0305], 0x66);
    }
    
    #[test]
    fn test_ppu_vram_reads() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_ctrl(0);
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load_into_buffer
        assert_eq!(ppu.addr.get(), 0x2306);
        assert_eq!(ppu.read_data(), 0x66);
    }

    #[test]
    fn test_ppu_vram_reads_cross_page() { 
	let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_ctrl(0);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x0200] = 0x77;

        ppu.write_to_ppu_addr(0x21);
        ppu.write_to_ppu_addr(0xff);

        ppu.read_data(); //load_into_buffer
        assert_eq!(ppu.read_data(), 0x66);
        assert_eq!(ppu.read_data(), 0x77);
    }

    #[test]
    fn test_ppu_vram_reads_step_32() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_ctrl(0b100);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x01ff + 32] = 0x77;
        ppu.vram[0x01ff + 64] = 0x88;

        ppu.write_to_ppu_addr(0x21);
        ppu.write_to_ppu_addr(0xff);

        ppu.read_data(); //load_into_buffer
        assert_eq!(ppu.read_data(), 0x66);
        assert_eq!(ppu.read_data(), 0x77);
        assert_eq!(ppu.read_data(), 0x88);
    }

    #[test]
    fn test_vram_horizontal_mirror() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_ppu_addr(0x24);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x66); //write to a

        ppu.write_to_ppu_addr(0x28);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x77); //write to B

        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x66); //read from A

        ppu.write_to_ppu_addr(0x2C);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x77); //read from b
    }

    #[test]
    fn test_vram_vertical_mirror() {
        let mut ppu = NesPPU::new(vec![0; 2048], Mirroring::VERTICAL);

        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x66); //write to A

        ppu.write_to_ppu_addr(0x2C);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x77); //write to b

        ppu.write_to_ppu_addr(0x28);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x66); //read from a

        ppu.write_to_ppu_addr(0x24);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x77); //read from B
    }

    #[test]
    fn test_read_status_resets_latch() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_ppu_addr(0x21);
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load_into_buffer
        assert_ne!(ppu.read_data(), 0x66);

        ppu.read_status();

        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load_into_buffer
        assert_eq!(ppu.read_data(), 0x66);
    }

    #[test]
    fn test_ppu_vram_mirroring() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_ctrl(0);
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_ppu_addr(0x63); //0x6305 -> 0x2305
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into_buffer
        assert_eq!(ppu.read_data(), 0x66);
    }

    #[test]
    fn test_read_status_resets_vblank() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.status.set_vblank_started(true);

        let status = ppu.read_status();

        assert_eq!(status >> 7, 1);
        assert_eq!(ppu.status.get_status() >> 7, 0);
    }

    #[test]
    fn test_oam_read_write() {
        let mut ppu = NesPPU::new_empty_rom();
        ppu.write_to_oam_addr(0x10);
        ppu.write_to_oam_data(0x66);
        ppu.write_to_oam_data(0x77);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_oam_data(), 0x66);

        ppu.write_to_oam_addr(0x11);
        assert_eq!(ppu.read_oam_data(), 0x77);
    }

    #[test]
    fn test_oam_dma() {
        let mut ppu = NesPPU::new_empty_rom();

        let mut data = [0x66; 256];
        data[0] = 0x77;
        data[255] = 0x88;

        ppu.write_to_oam_addr(0x10);
        ppu.write_oam_dma(&data);

        ppu.write_to_oam_addr(0xf); //wrap around
        assert_eq!(ppu.read_oam_data(), 0x88);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_oam_data(), 0x77);
  
        ppu.write_to_oam_addr(0x11);
        assert_eq!(ppu.read_oam_data(), 0x66);
    }
}
