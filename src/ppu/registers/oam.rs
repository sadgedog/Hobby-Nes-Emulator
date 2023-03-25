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

    pub fn update_addr(&mut self, data: u8) {
	self.oam_addr = data;
    }

    pub fn update_data(&mut self, data: u8) {
	self.oam_data[self.oam_addr as usize] = data;
	self.oam_addr = self.oam_addr.wrapping_add(1);
    }
}
