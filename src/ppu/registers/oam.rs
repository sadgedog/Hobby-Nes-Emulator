pub struct OamRegisters {
    oam_addr: u8,
    oam_data: [u8; 256],
}

impl OamRegisters {
    pub fn new() -> Self {
	OamRegisters {
	    oam_addr: 0,
	    oam_data: [0; 256],
	}
    }

    pub fn write_addr(&mut self, data: u8) {
	self.oam_addr = data;
    }

    pub fn write_data(&mut self, data: u8) {
	self.oam_data[self.oam_addr as usize] = data;
	self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    pub fn write_dma(&mut self, data: &[u8; 256]) {
	for x in data.iter() {
	    self.oam_data[self.oam_addr as usize] = *x;
	    self.oam_addr = self.oam_addr.wrapping_add(1);
	}
    }

    pub fn get_data(&mut self) -> u8 {
	self.oam_data[self.oam_addr as usize]
    }
}
