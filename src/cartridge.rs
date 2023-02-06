// Cartridge       
// 
//  ------------
// | NES Header |
//  ------------   BUS   -----
// | PRG ROM    | ----- | CPU |
//  ------------         -----
// | CHR ROM    | ----- | PPU |
//  ------------         -----

// NES Header
//
// NES Signature  PRG ROM size  CHR ROM size   Control Byte1  Control Byte2    PRG RAM size        Reserved 
// 4E 45 53 1A /      01      /      01      /      00      /      00      /      00 00      / 00 00 00 00 00 00 /
#[derive(Debug, PartialEq)]
pub enum Mirroring {
    VERTICAL,
    HORIZONTAL,
    FOUR_SCREEN,
}

pub struct Rom {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8, 
    pub screen_mirroring: Mirroring, // PPU
}
