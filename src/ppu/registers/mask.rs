bitflags! {
    // 7  bit  0
    // ---- ----
    // BGRs bMmG
    // |||| ||||
    // |||| |||+- Greyscale (0: normal color, 1: produce a greyscale display)
    // |||| ||+-- 1: Show background in leftmost 8 pixels of screen, 0: Hide
    // |||| |+--- 1: Show sprites in leftmost 8 pixels of screen, 0: Hide
    // |||| +---- 1: Show background
    // |||+------ 1: Show sprites
    // ||+------- Emphasize red (green on PAL/Dendy)
    // |+-------- Emphasize green (red on PAL/Dendy)
    // +--------- Emphasize blue
    pub struct MaskRegister: u8 {
	const GRAY_SCALE           = 0b0000_0001;
	const SHOW_BACKGROUND_LEFT = 0b0000_0010;
	const SHOW_SPRITE_LEFT     = 0b0000_0100;
	const SHOW_BACKGROUND      = 0b0000_1000;
	const SHOW_SPRITES         = 0b0001_0000;
	const EMPHASIZE_RED        = 0b0010_0000;
	const EMPHASIZE_GREEN      = 0b0100_0000;
	const EMPHASIZE_BLUE       = 0b1000_0000;
    }
}

pub enum Color {
    Red,
    Green,
    Blue,
}

impl MaskRegister {
    pub fn new() -> Self {
	MaskRegister::from_bits_truncate(0b0000_0000)
    }

    pub fn check_gray_scale(&self) -> u8 {
	if self.contains(MaskRegister::GRAY_SCALE) {
	    1
	} else {
	    0
	}
    }

    pub fn check_show_background_left(&self) -> u8 {
	if self.contains(MaskRegister::SHOW_BACKGROUND_LEFT) {
	    1
	} else {
	    0
	}
    }

    pub fn check_show_sprite_left(&self) -> u8 {
	if self.contains(MaskRegister::SHOW_SPRITE_LEFT) {
	    1
	} else {
	    0
	}
    }

    pub fn check_show_background(&self) -> u8 {
	if self.contains(MaskRegister::SHOW_BACKGROUND) {
	    1
	} else {
	    0
	}
    }

    pub fn emphasize_red(&self) -> Vec<Color> {
	let mut res = Vec::<Color>::new();
	if self.contains(MaskRegister::EMPHASIZE_RED) {
	    res.push(Color::Red);
	}
	if self.contains(MaskRegister::EMPHASIZE_BLUE) {
	    res.push(Color::Blue);
	}
	if self.contains(MaskRegister::EMPHASIZE_GREEN) {
	    res.push(Color::Green);
	}
	res
    }

    pub fn update(&mut self, data: u8) {
	self.bits = data;
    }

}
