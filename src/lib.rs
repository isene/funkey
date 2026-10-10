//! funkey: a game engine for the terminal.
//!
//! A game draws into a [`Frame`] of pixels and reads keys from
//! [`Input`]; funkey runs the loop at a fixed rate and shows the frame on
//! the terminal as half-block cells, two pixels per cell, in full colour.
//! Later backends show real pixels where the terminal can.
//!
//! Built for wasm32 without the `term` feature, the same game runs in a
//! web page instead: see `src/web.rs` and `web/`.
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

pub mod audio;
pub mod doom;
pub mod font;
pub mod frame;
pub mod input;
pub mod noise;
pub mod page;
pub mod particles;
pub mod raster;
pub mod rng;
#[cfg(feature = "term")]
pub mod screen;
pub mod sprite;
pub mod store;
pub mod tilemap;
pub mod wad;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub use audio::{Audio, Sample, Tune, Wave};
pub use frame::{mix, parts, rgb, tint, Frame, Rgb, BLACK, WHITE};
pub use input::{Input, Key};
pub use particles::Particles;
pub use raster::{Cam3, Mat, Mesh, Model, Raster, Scene, Texture, Vert, CUTOUT, M4, V3};
pub use rng::Rng;
#[cfg(feature = "term")]
pub use screen::Screen;
pub use sprite::Sprite;
pub use tilemap::{Body, Tilemap};

#[cfg(feature = "term")]
use std::time::{Duration, Instant};

/// Where the engine's own tests write their files: inside the build
/// folder, so nothing is left in the home folder or in /tmp.
#[cfg(test)]
pub(crate) fn test_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-tmp")
}

/// Where `FUNKEY_DEBUG` notes go: ~/.funkey/debug.log.
pub fn debug_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = std::path::PathBuf::from(home).join(".funkey");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("debug.log")
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Flow {
    Continue,
    Quit,
    /// Stand still until a key is pressed: the picture goes dark, the
    /// sound stops and the game costs nothing. The key that ends the
    /// pause is not passed on. The engine pauses the same way by itself
    /// when the window stops being the one in front.
    Pause,
}

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
    /// The picture while the game is paused: the game as it stands,
    /// darkened, under the word PAUSED. A game whose picture should not
    /// be studied at rest covers it here and then calls [`pause_veil`].
    fn draw_paused(&mut self, frame: &mut Frame) {
        self.draw(frame);
        pause_veil(frame);
    }
}

/// Darken a frame and write PAUSED across it.
pub fn pause_veil(frame: &mut Frame) {
    frame.dim(0.45);
    let (cx, s) = (frame.w / 2, (frame.w / 160).max(1));
    let y = frame.h / 2 - 7 * s;
    frame.text_centered(cx, y, "PAUSED", WHITE, true, s);
    frame.text_centered(cx, y + 9 * s, "ANY KEY GOES ON", 0xb0b0b0, false, (s + 1) / 2);
}

/// Run the game until it asks to quit. Updates happen at the configured
/// rate; a slow frame is caught up with extra updates, at most a few.
///
/// With `FUNKEY_SCRIPT` set, no terminal is touched: the keys in the
/// script are fed for the given ticks (`"right*60,space*5,-*30"`) and
/// the last frame goes to `FUNKEY_SHOT` as a PPM, or every frame when
/// `FUNKEY_SHOT_EVERY` is set.
#[cfg(feature = "term")]
pub fn run(game: &mut dyn Game, cfg: Config) {
    if let Ok(script) = std::env::var("FUNKEY_SCRIPT") {
        let every = std::env::var("FUNKEY_SHOT_EVERY").ok().and_then(|e| e.parse().ok()).unwrap_or(0);
        return run_script(game, cfg, &script, std::env::var("FUNKEY_SHOT").ok(), every);
    }
    let mut screen = Screen::open();
    let mut input = Input::new();
    input.set_exact(crust::Crust::supports_key_release());
    input.keylog = std::env::var_os("FUNKEY_KEYLOG").and_then(|p| std::fs::File::create(p).ok());
    let mut frame = Frame::new(cfg.width, cfg.height);
    let step = Duration::from_secs_f64(1.0 / cfg.fps.max(1) as f64);
    let dt = step.as_secs_f32();
    let shot = std::env::var_os("FUNKEY_SHOT").map(std::path::PathBuf::from);
    // Real pixels over a whole window cost the display server a scale
    // pass per frame; 30 a second is plenty there, so faster games show
    // every other frame.
    let halve = cfg.fps > 30;
    let mut next = Instant::now();
    let mut ticks: u64 = 0;
    loop {
        input.poll();
        if input.resized { screen.resize(); }
        let mut pause = input.blurred;
        let mut updates = 0;
        while !pause && Instant::now() >= next && updates < 4 {
            match game.update(&input, dt) {
                Flow::Quit => return,
                Flow::Pause => pause = true,
                Flow::Continue => {}
            }
            next += step;
            updates += 1;
            ticks += 1;
        }
        if pause {
            wait_for_a_key(game, &mut screen, &mut input, &mut frame);
            next = Instant::now();
            continue;
        }
        if updates > 0 {
            game.draw(&mut frame);
            if !(halve && screen.big() && ticks % 2 == 1) { screen.present(&frame); }
            if let Some(p) = &shot { if ticks % 30 == 0 { let _ = std::fs::write(p, frame.to_ppm()); } }
        }
        let now = Instant::now();
        if next > now { std::thread::sleep((next - now).min(step)); }
    }
}

/// The pause: the dark picture goes up once, the sound stops, and the
/// loop sleeps until a key goes down. No timer runs meanwhile.
#[cfg(feature = "term")]
fn wait_for_a_key(game: &mut dyn Game, screen: &mut Screen, input: &mut Input, frame: &mut Frame) {
    game.draw_paused(frame);
    screen.present(frame);
    audio::hush(true);
    loop {
        input.wait();
        if input.resized {
            screen.resize();
            screen.present(frame);
        }
        if input.any_pressed() { break; }
    }
    audio::hush(false);
    // The key that ended the pause is not the game's, and a key let go
    // meanwhile may never have said so.
    input.release_all();
}

/// Time a game with no terminal: `frames` updates and draws with the
/// keys in `input` held, then the time one frame took, on stderr. The
/// last frame comes back, for a picture of where the run ended.
#[cfg(feature = "term")]
pub fn bench(game: &mut dyn Game, input: &Input, cfg: Config, frames: u32) -> Frame {
    let mut frame = Frame::new(cfg.width, cfg.height);
    let dt = 1.0 / cfg.fps.max(1) as f32;
    let t0 = Instant::now();
    for _ in 0..frames {
        game.update(input, dt);
        game.draw(&mut frame);
    }
    let ms = t0.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64;
    eprintln!("{:.3} ms a frame at {}x{} over {} frames", ms, cfg.width, cfg.height, frames);
    frame
}

#[cfg(feature = "term")]
fn script_key(name: &str) -> Option<Key> {
    Some(match name {
        "up" => Key::Up, "down" => Key::Down, "left" => Key::Left, "right" => Key::Right, "space" => Key::Space,
        "enter" => Key::Enter, "tab" => Key::Tab, "esc" => Key::Escape, "back" => Key::Backspace,
        "-" | "none" => return None,
        s if s.chars().count() == 1 => Key::Char(s.chars().next()?),
        _ => return None,
    })
}

/// The scripted run: each item of the script is a key and the ticks it
/// is held for. The last frame goes to `shot`, and with `every` above 0
/// every n-th frame goes to `<shot>-<number>.ppm` on the way. A pause is
/// no stop here: nobody is there to press a key.
#[cfg(feature = "term")]
fn run_script(game: &mut dyn Game, cfg: Config, script: &str, shot: Option<String>, every: u32) {
    let mut input = Input::new();
    let mut frame = Frame::new(cfg.width, cfg.height);
    let dt = 1.0 / cfg.fps.max(1) as f32;
    let mut n = 0u32;
    'outer: for item in script.split(',') {
        let (name, count) = item.trim().split_once('*').unwrap_or((item.trim(), "1"));
        let key = script_key(name);
        input.release_all();
        for _ in 0..count.trim().parse::<u32>().unwrap_or(1) {
            input.clear_pressed();
            if let Some(k) = key { input.inject(k); }
            if game.update(&input, dt) == Flow::Quit { break 'outer; }
            n += 1;
            if every > 0 && n % every == 0 {
                game.draw(&mut frame);
                if let Some(p) = &shot { let _ = std::fs::write(format!("{}-{:06}.ppm", p, n / every), frame.to_ppm()); }
            }
        }
    }
    game.draw(&mut frame);
    if let Some(p) = shot { let _ = std::fs::write(p, frame.to_ppm()); }
}

#[cfg(all(test, feature = "term"))]
mod tests {
    use super::*;

    /// A game that writes down what it was given, tick by tick.
    #[derive(Default)]
    struct Probe { ticks: Vec<String>, draws: u32 }

    impl Game for Probe {
        fn update(&mut self, input: &Input, dt: f32) -> Flow {
            assert!((dt - 1.0 / 50.0).abs() < 1e-6, "a tick is one over the frame rate");
            if input.pressed(Key::Char('q')) { return Flow::Quit; }
            let mut t = String::new();
            for (k, name) in [(Key::Right, 'R'), (Key::Space, 'S'), (Key::Char('p'), 'P')] {
                if input.pressed(k) { t.push(name); } else if input.held(k) { t.push(name.to_ascii_lowercase()); }
            }
            self.ticks.push(t);
            if input.pressed(Key::Char('p')) { Flow::Pause } else { Flow::Continue }
        }
        fn draw(&mut self, f: &mut Frame) {
            self.draws += 1;
            f.clear(0x808080);
            f.put(0, 0, rgb(self.ticks.len() as u8, 0, 0));
        }
    }

    const CFG: Config = Config { width: 320, height: 200, fps: 50 };

    fn shot_file(name: &str) -> String {
        let dir = test_dir().join("shots");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir.join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn a_script_holds_each_key_for_its_ticks() {
        let mut g = Probe::default();
        run_script(&mut g, CFG, "right*3,-*2,space,p,right*2,q,space*9", None, 0);
        // A capital is the tick the key went down, a small letter a tick
        // it stayed down. The pause is passed by, and Q ends the run.
        assert_eq!(g.ticks, ["R", "r", "r", "", "", "S", "P", "R", "r"]);
        assert_eq!(g.draws, 1, "with no shots asked for, only the last frame is drawn");
    }

    #[test]
    fn a_script_writes_the_frames_it_is_asked_for() {
        let shot = shot_file("script");
        let mut g = Probe::default();
        run_script(&mut g, CFG, "-*6", Some(shot.clone()), 2);
        assert_eq!(g.draws, 4, "frames 2, 4 and 6, then the last one");
        let ppm = |p: String| std::fs::read(p).expect("the frame was written");
        let header = b"P6\n320 200\n255\n";
        for (file, ticks) in [(format!("{shot}-000001.ppm"), 2u8), (format!("{shot}-000003.ppm"), 6), (shot.clone(), 6)] {
            let b = ppm(file);
            assert_eq!(&b[..header.len()], header);
            assert_eq!(b.len(), header.len() + 320 * 200 * 3);
            assert_eq!(b[header.len()], ticks, "the first pixel says how many ticks had run");
        }
    }

    #[test]
    fn a_timing_run_makes_every_frame_and_hands_back_the_last() {
        let mut g = Probe::default();
        let mut input = Input::new();
        input.inject(Key::Space);
        let f = bench(&mut g, &input, CFG, 5);
        assert_eq!(g.ticks, ["S", "S", "S", "S", "S"], "the keys given stay as they are");
        assert_eq!(g.draws, 5);
        assert_eq!((f.w, f.h, f.get(0, 0)), (320, 200, 0x050000));
    }

    #[test]
    fn the_pause_picture_is_the_game_gone_dark_under_a_word() {
        let mut g = Probe::default();
        let mut f = Frame::new(320, 200);
        g.draw_paused(&mut f);
        assert_eq!(f.get(5, 5), 0x393939, "grey at 45 percent");
        let count = |c: Rgb| f.px.iter().filter(|&&p| p == c).count();
        assert!(count(WHITE) > 100, "PAUSED in white");
        assert!(count(0xb0b0b0) > 50, "and the line under it");
        // The words sit in the middle, clear of the edges.
        assert!((0..320).all(|x| f.get(x, 60) == 0x393939 && f.get(x, 140) == 0x393939));
        let mut small = Frame::new(40, 30);
        pause_veil(&mut small);
        assert_eq!(small.px.len(), 1200, "a frame too small for the words is still fine");
    }

    #[test]
    fn script_keys_have_names() {
        assert_eq!(script_key("right"), Some(Key::Right));
        assert_eq!(script_key("esc"), Some(Key::Escape));
        assert_eq!(script_key("x"), Some(Key::Char('x')));
        assert_eq!(script_key("-"), None);
        assert_eq!(script_key("nonsense"), None);
    }
}
