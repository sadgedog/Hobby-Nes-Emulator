use crate::cartridge::Mirroring;

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
    // ミラーリング
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
