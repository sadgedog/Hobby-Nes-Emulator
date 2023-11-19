pub struct ScrollRegister {
    pub h_scroll: u8,
    pub v_scroll: u8,
    pub latch: bool,
}

impl ScrollRegister {
    pub fn new() -> Self {
        ScrollRegister {
            h_scroll: 0,
            v_scroll: 0,
            latch: false,
        }
    }

    pub fn write(&mut self, data: u8) {
        if !self.latch {
            self.h_scroll = data;
        } else {
            self.v_scroll = data;
        }
        self.latch = !self.latch;
    }

    pub fn reset_latch(&mut self) {
        self.latch = false;
    }
}
