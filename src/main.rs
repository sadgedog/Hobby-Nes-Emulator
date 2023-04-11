pub mod cpu;
pub mod opcodes;
pub mod bus;
pub mod cartridge;
pub mod trace;
pub mod ppu;
pub mod render;
pub mod joypad;

use cpu::Mem;
use cpu::CPU;
use bus::Bus;
use cartridge::Rom;
use render::frame::Frame;
use ppu::NesPPU;

use sdl2::event::Event;
use sdl2::EventPump;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;

use std::collections::HashMap;



#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate bitflags;


fn color(byte: u8) -> Color {
    match byte {
	0 => sdl2::pixels::Color::BLACK,
	1 => sdl2::pixels::Color::WHITE,
	2 | 9 => sdl2::pixels::Color::GREY,
	3 | 10 => sdl2::pixels::Color::RED,
	4 | 11 => sdl2::pixels::Color::GREEN,
	5 | 12 => sdl2::pixels::Color::BLUE,
	6 | 13 => sdl2::pixels::Color::MAGENTA,
	7 | 14 => sdl2::pixels::Color::YELLOW,
	_ => sdl2::pixels::Color::CYAN,
    }
}

fn read_screen_state(cpu: &mut CPU, frame: &mut [u8; 32 * 3 * 32]) -> bool {
    let mut frame_idx = 0;
    let mut update = false;
    for i in 0x0200..0x0600 {
	let color_idx = cpu.mem_read(i as u16);
	let (b1, b2, b3) = color(color_idx).rgb();
	if frame[frame_idx] != b1 || frame[frame_idx + 1] != b2 || frame[frame_idx + 2] != b3 {
	    frame[frame_idx] = b1;
	    frame[frame_idx + 1] = b2;
	    frame[frame_idx + 2] = b3;
	    update = true;
	}
	frame_idx += 3;
    }
    update
}

// User Input
// W ↑
// A ←
// S ↓
// D →
fn handle_user_input(cpu: &mut CPU, event_pump: &mut EventPump) {
    for event in event_pump.poll_iter() {
	match event {
	    Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
		std::process::exit(0)
	    },
	    Event::KeyDown { keycode: Some(Keycode::W), .. } => {
		cpu.mem_write(0xFF, 0x77);
	    },
	    Event::KeyDown { keycode: Some(Keycode::S), .. } => {
		cpu.mem_write(0xFF, 0x73);
	    },
	    Event::KeyDown { keycode: Some(Keycode::A), .. } => {
		cpu.mem_write(0xFF, 0x61);
	    },
	    Event::KeyDown { keycode: Some(Keycode::D), .. } => {
		cpu.mem_write(0xFF, 0x64);
	    }
	    _ => {}
	}
    }
}

fn main() {
    // initialize SDL2
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
	.window("NES EMULATOR", (256.0 * 3.0) as u32, (240.0 * 3.0) as u32)
	// .window("TILE TEST", (256.0 * 4.0) as u32, (240.0 * 2.0) as u32) // 1pixを10倍
	.position_centered()
	.build()
	.unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(3.0, 3.0).unwrap();
    // canvas.set_scale(2.0, 2.0).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
	.create_texture_target(PixelFormatEnum::RGB24, 256, 240).unwrap();
    // .create_texture_target(PixelFormatEnum::RGB24, 256 * 2, 240).unwrap();
    
    // cartridge
    // let bytes: Vec<u8> = std::fs::read("Alter_Ego.nes").unwrap();
    let bytes: Vec<u8> = std::fs::read("mojon-twins--multicart.nes").unwrap();
    // let bytes: Vec<u8> = std::fs::read("cyo.nes").unwrap();
    let rom = Rom::new(&bytes).unwrap();    
    // bus
    let mut frame = Frame::new();

    let mut key_map = HashMap::new();
    key_map.insert(Keycode::Down, joypad::JoyPadButton::DOWN);
    key_map.insert(Keycode::Up, joypad::JoyPadButton::UP);
    key_map.insert(Keycode::Right, joypad::JoyPadButton::RIGHT);
    key_map.insert(Keycode::Left, joypad::JoyPadButton::LEFT);
    key_map.insert(Keycode::Space, joypad::JoyPadButton::SELECT);
    key_map.insert(Keycode::Return, joypad::JoyPadButton::START);
    key_map.insert(Keycode::A, joypad::JoyPadButton::BUTTON_A);
    key_map.insert(Keycode::S, joypad::JoyPadButton::BUTTON_B);
    
   
    let bus = Bus::new(rom, move |ppu: &NesPPU, joypad: &mut joypad::JoyPad| {
	render::render(ppu, &mut frame);
	// texture.update(None, &frame.data, 256 * 3).unwrap();
	texture.update(None, &frame.data, 256 * 2 * 3).unwrap();
	canvas.copy(&texture, None, None).unwrap();
	canvas.present();
	for event in event_pump.poll_iter() {
	    match event {
	        Event::Quit { .. }
	        | Event::KeyDown {
	            keycode: Some(Keycode::Escape),
	            ..
	        } => std::process::exit(0),

		Event::KeyDown { keycode, .. } => {
		    if let Some(key) = key_map.get(&keycode.unwrap_or(Keycode::Ampersand)) {
			joypad.set_button_pressed_status(*key, true);
		    }
		}
		Event::KeyUp { keycode, .. } => {
		    if let Some(key) = key_map.get(&keycode.unwrap_or(Keycode::Ampersand)) {
			joypad.set_button_pressed_status(*key, false);
		    }
		}
		
	        _ => { /* do nothing */ }
	    }
	}
    });
    // cpu
    let mut cpu = CPU::new(bus);

    cpu.reset();
    // cpu.run();
    
    cpu.run_with_callback(move |cpu| {
	// println!("{}", trace(cpu));
    })
}
