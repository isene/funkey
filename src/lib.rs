//! funkey: a game engine for the terminal.
//!
//! A game draws into a [`Frame`] of pixels and reads keys from
//! [`Input`]; funkey runs the loop at a fixed rate and shows the frame on
//! the terminal as half-block cells, two pixels per cell, in full colour.
//! Later backends show real pixels where the terminal can.
//!
//! ```no_run
//! use funkey::*;
//!
//! struct Ball { x: f32, y: f32, vx: f32, vy: f32 }
//!
//! impl Game for Ball {
//!     fn update(&mut self, input: &Input, dt: f32) -> Flow {
//!         if input.pressed(Key::Char('q')) { return Flow::Quit; }
//!         self.x += self.vx * dt;
//!         self.y += self.vy * dt;
//!         if self.x < 0.0 || self.x > 156.0 { self.vx = -self.vx; }
//!         if self.y < 0.0 || self.y > 96.0 { self.vy = -self.vy; }
//!         Flow::Continue
//!     }
//!     fn draw(&mut self, f: &mut Frame) {
//!         f.clear(0x102030);
//!         f.rect(self.x as i32, self.y as i32, 4, 4, 0xffcc00);
//!     }
//! }
//!
//! fn main() {
//!     run(&mut Ball { x: 10.0, y: 10.0, vx: 60.0, vy: 40.0 }, Config { width: 160, height: 100, fps: 60 });
//! }
//! ```

pub mod doom;
pub mod font;
pub mod frame;
pub mod input;
pub mod screen;
pub mod sprite;
pub mod tilemap;
pub mod wad;

pub use frame::{parts, rgb, Frame, Rgb, BLACK, WHITE};
pub use input::{Input, Key};
pub use screen::Screen;
pub use sprite::Sprite;
pub use tilemap::{Body, Tilemap};

use std::time::{Duration, Instant};

/// Where `FUNKEY_DEBUG` notes go: ~/.funkey/debug.log.
pub fn debug_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = std::path::PathBuf::from(home).join(".funkey");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("debug.log")
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Flow { Continue, Quit }

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// The game's own resolution; the screen scales it to fit.
    pub width: i32,
    pub height: i32,
    /// Updates and frames a second.
    pub fps: u32,
}

impl Default for Config {
    fn default() -> Self { Config { width: 160, height: 100, fps: 60 } }
}

pub trait Game {
    /// Advance the game by `dt` seconds with the keys as they are.
    fn update(&mut self, input: &Input, dt: f32) -> Flow;
    /// Draw the game into the frame. Mutable, so a renderer may keep
    /// its scratch buffers in the game.
    fn draw(&mut self, frame: &mut Frame);
}

/// Run the game until it asks to quit. Updates happen at the configured
/// rate; a slow frame is caught up with extra updates, at most a few.
pub fn run(game: &mut dyn Game, cfg: Config) {
    let mut screen = Screen::open();
    let mut input = Input::new();
    let mut frame = Frame::new(cfg.width, cfg.height);
    let step = Duration::from_secs_f64(1.0 / cfg.fps.max(1) as f64);
    let dt = step.as_secs_f32();
    let shot = std::env::var_os("FUNKEY_SHOT").map(std::path::PathBuf::from);
    let mut next = Instant::now();
    let mut ticks: u64 = 0;
    loop {
        input.poll();
        if input.resized { screen.resize(); }
        let mut updates = 0;
        while Instant::now() >= next && updates < 4 {
            if game.update(&input, dt) == Flow::Quit { return; }
            next += step;
            updates += 1;
            ticks += 1;
        }
        if updates > 0 {
            game.draw(&mut frame);
            // Real pixels over a whole window cost the display server a
            // scale pass per frame; 30 a second is plenty there.
            if !(screen.big() && ticks % 2 == 1) { screen.present(&frame); }
            if let Some(p) = &shot { if ticks % 30 == 0 { let _ = std::fs::write(p, frame.to_ppm()); } }
        }
        let now = Instant::now();
        if next > now { std::thread::sleep((next - now).min(step)); }
    }
}
