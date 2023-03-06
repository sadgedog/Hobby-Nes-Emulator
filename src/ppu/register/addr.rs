// Nesは分散システムなので、CPUは直接PPUのメモリ(CHR ROM)を読み込めない
// なのでCPUはPPUの0x2006レジスタにCHR ROMの欲しい値のアドレスを書き込む
// PPUはPPU 0x2006レジスタに書き込まれたことを確認すると,0x2006に書き込まれたCHR ROMアドレスを0x2007に書き込む
// CPUはPPU 0x2007レジスタでPPUに要求したデータを取得する

pub struct AddrRegister {
    value: (u8, u8),
    hi_ptr: bool,
}

impl AddrRegister {
    pub fn new() -> Self {
	AddrRegister {
	    value: (0, 0), // big endian
	    hi_ptr: true,
	}
    }

    fn set(&mut self, data: u16) {
	self.value.0 = (data >> 8) as u8;
	self.value.1 = (data & 0xFF) as u8;
    }

    pub fn update(&mut self, data: u8) {
	if self.hi_ptr {
	    self.value.0 = data;
	} else {
	    self.value.1 = data;
	}

	if self.get() > 0x3FFF {
	    self.set(self.get() & 0b1111_1111_1111_11);
	}
	self.hi_ptr = !self.hi_ptr;
    }

    pub fn increment(&mut self, inc: u8) {
	let lo = self.value.1;
	self.value.1 = self.value.1.wrapping_add(inc);
	if lo > self.value.1 {
	    self.value.0 = self.value.0.wrapping_add(1);
	}
	if self.get() > 0x3FFF {
	    self.set(self.get() & 0b1111_1111_1111_11);
	}
    }

    pub fn reset_latch(&mut self) {
	self.hi_ptr = true;
    }

    pub fn get(&self) -> u16 {
	((self.value.0 as u16) << 8) | (self.value.1 as u16)
    }
   
}
