bitflags! {
    // 7  bit  0
    // ---- ----
    // VSO. ....
    // |||| ||||
    // |||+-++++- PPU open bus. Returns stale PPU bus contents.
    // ||+------- Sprite overflow. The intent was for this flag to be set
    // ||         whenever more than eight sprites appear on a scanline, but a
    // ||         hardware bug causes the actual behavior to be more complicated
    // ||         and generate false positives as well as false negatives; see
    // ||         PPU sprite evaluation. This flag is set during sprite
    // ||         evaluation and cleared at dot 1 (the second dot) of the
    // ||         pre-render line.
    // |+-------- Sprite 0 Hit.  Set when a nonzero pixel of sprite 0 overlaps
    // |          a nonzero background pixel; cleared at dot 1 of the pre-render
    // |          line.  Used for raster timing.
    // +--------- Vertical blank has started (0: not in vblank; 1: in vblank).
    //            Set at dot 1 of line 241 (the line *after* the post-render
    //            line); cleared after reading $2002 and at dot 1 of the
    //            pre-render line.
    pub struct StatusRegister: u8 {
	const OPEN_BUS_1       = 0b0000_0001;
	const OPEN_BUS_2       = 0b0000_0010;
	const OPEN_BUS_3       = 0b0000_0100;
	const OPEN_BUS_4       = 0b0000_1000;
	const OPEN_BUS_5       = 0b0001_0000;
	const SPRITE_OVERFLOW  = 0b0010_0000;
	const SPRITE_ZERO_HIT  = 0b0100_0000;
	const VBLANK_STARTED   = 0b1000_0000;
    }
}

impl StatusRegister {
    pub fn new() -> Self {
	StatusRegister::from_bits_truncate(0b0000_0000)
    }

    pub fn open_bus() {}

    pub fn set_sprite_overflow(&mut self, flag: bool) {
	self.set(StatusRegister::SPRITE_OVERFLOW, flag);
    }

    pub fn set_sprite_zero_hit(&mut self, flag: bool) {
	self.set(StatusRegister::SPRITE_ZERO_HIT, flag);
    }

    pub fn set_vblank_started(&mut self, flag: bool) {
	self.set(StatusRegister::VBLANK_STARTED, flag);
    }

    pub fn check_vblank_started(&self) -> bool {
	self.contains(StatusRegister::VBLANK_STARTED)
    }

    pub fn reset_vblank_started(&mut self) {
	self.remove(StatusRegister::VBLANK_STARTED);
    }
    
    pub fn get_status(&self) -> u8 {
	self.bits
    }
}
