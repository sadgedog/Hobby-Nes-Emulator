use crate::cartridge::Mirroring;

pub struct NesPPU {
    pub chr_rom: Vec<u8>,
    pub palette_table: [u8: 32],
    pub vram: [u8: 2048],
    pub oam_data: [u8: 256],
    pub mirroring: Mirroring,
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
}
