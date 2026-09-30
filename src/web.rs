//! A game in a web page. Build the game for wasm32 without the terminal
//! (`--target wasm32-unknown-unknown --no-default-features`) and have it
//! say `funkey::web!(MyGame::new(), Config { .. });` in place of `run`.
//! The page's script, web/funkey.js, does the rest: it calls `fk_start`
//! once, `fk_key` for every key that goes down or up, and `fk_frame` on
//! every animation frame; it paints the pixels at `fk_pixels` and plays
//! the sound from `fk_sound`. Messages between the game and the page (see
//! `page`) go out through `fk_out` and come in through `fk_in`.

use crate::{audio, Config, Flow, Frame, Game, Input, Key};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

struct Web {
    make: fn() -> Box<dyn Game>,
    game: Box<dyn Game>,
    cfg: Config,
    frame: Frame,
    input: Input,
    rgba: Vec<u8>,
    /// The page's clock at the last frame, and the time not yet updated.
    last: f64,
    left: f64,
    pcm: Vec<i16>,
    sound: Vec<f32>,
}

thread_local! {
    static WEB: RefCell<Option<Web>> = const { RefCell::new(None) };
    static NOW: Cell<f64> = const { Cell::new(0.0) };
    static SEED: Cell<u64> = const { Cell::new(1) };
    static OUTBOX: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static INBOX: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static IN: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn outbox_push(m: String) { OUTBOX.with(|o| o.borrow_mut().push_back(m)); }

pub(crate) fn inbox_pop() -> Option<String> { INBOX.with(|i| i.borrow_mut().pop_front()) }

/// The next message for the page: its length in bytes, 0 when there is
/// none. The bytes sit at `out_ptr` until the next call.
pub fn out_next() -> u32 {
    let m = OUTBOX.with(|o| o.borrow_mut().pop_front());
    OUT.with(|b| {
        let mut b = b.borrow_mut();
        b.clear();
        if let Some(m) = m { b.extend_from_slice(m.as_bytes()); }
        b.len() as u32
    })
}

pub fn out_ptr() -> *const u8 { OUT.with(|b| b.borrow().as_ptr()) }

/// Room for a message from the page, `len` bytes long.
pub fn in_buf(len: u32) -> *mut u8 {
    IN.with(|b| {
        let mut b = b.borrow_mut();
        b.clear();
        b.resize(len as usize, 0);
        b.as_mut_ptr()
    })
}

/// The page has written its message into `in_buf`: hand it to the game.
pub fn in_done() {
    let m = IN.with(|b| String::from_utf8_lossy(&b.borrow()).into_owned());
    INBOX.with(|i| i.borrow_mut().push_back(m));
}

/// The page's clock, in seconds.
pub(crate) fn now() -> f64 { NOW.with(|n| n.get()) }

/// A seed from the page, a new one on every call.
pub(crate) fn seed() -> u64 {
    SEED.with(|s| {
        let v = s.get();
        s.set(v.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407));
        v
    })
}

pub fn start(seed: u32, make: fn() -> Box<dyn Game>, cfg: Config) {
    SEED.with(|s| s.set(seed as u64 | 1));
    let game = make();
    let rgba = vec![255; (cfg.width * cfg.height * 4) as usize];
    let frame = Frame::new(cfg.width, cfg.height);
    let web = Web { make, game, cfg, frame, input: Input::new(), rgba, last: -1.0, left: 0.0, pcm: Vec::new(), sound: Vec::new() };
    WEB.with(|w| *w.borrow_mut() = Some(web));
}

/// A key from the page: 1 to 8 for the arrows, Enter, Escape, Tab and
/// Backspace, otherwise the character itself.
pub fn key(code: u32, down: bool) {
    let key = match code {
        1 => Key::Up, 2 => Key::Down, 3 => Key::Left, 4 => Key::Right,
        5 => Key::Enter, 6 => Key::Escape, 7 => Key::Tab, 8 => Key::Backspace,
        32 => Key::Space,
        c => match char::from_u32(c) { Some(ch) => Key::Char(ch), None => return },
    };
    WEB.with(|w| if let Some(w) = w.borrow_mut().as_mut() { w.input.key(key, down) });
}

/// One animation frame at `ms` on the page's clock: the updates due, at
/// most four, then a new picture. True when there is one to paint. A game
/// that quits starts over, since a page has nowhere to quit to.
pub fn frame(ms: f64) -> bool {
    let now = ms / 1000.0;
    NOW.with(|n| n.set(now));
    WEB.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return false };
        let step = 1.0 / w.cfg.fps.max(1) as f64;
        if w.last < 0.0 { w.last = now; }
        w.left += (now - w.last).clamp(0.0, 0.25);
        w.last = now;
        let mut ticks = 0;
        while w.left >= step && ticks < 4 {
            if w.game.update(&w.input, step as f32) == Flow::Quit { w.game = (w.make)(); }
            w.input.clear_pressed();
            w.left -= step;
            ticks += 1;
        }
        if ticks == 0 { return false; }
        w.game.draw(&mut w.frame);
        for (p, o) in w.frame.px.iter().zip(w.rgba.chunks_exact_mut(4)) {
            o[0] = (p >> 16) as u8;
            o[1] = (p >> 8) as u8;
            o[2] = *p as u8;
        }
        true
    })
}

/// The picture: width x height x RGBA.
pub fn pixels() -> *const u8 {
    WEB.with(|w| w.borrow().as_ref().map_or(std::ptr::null(), |w| w.rgba.as_ptr()))
}

/// The next `n` samples of sound, mono floats at `audio::RATE`.
pub fn sound(n: u32) -> *const f32 {
    WEB.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return std::ptr::null() };
        w.pcm.resize(n as usize, 0);
        audio::web_mix(&mut w.pcm);
        w.sound.clear();
        w.sound.extend(w.pcm.iter().map(|&s| s as f32 / 32768.0));
        w.sound.as_ptr()
    })
}

/// The exports a page needs, for the game `$make` builds.
#[macro_export]
macro_rules! web {
    ($make:expr, $cfg:expr) => {
        #[no_mangle]
        pub extern "C" fn fk_start(seed: u32) { $crate::web::start(seed, || -> Box<dyn $crate::Game> { Box::new($make) }, $cfg) }
        #[no_mangle]
        pub extern "C" fn fk_key(code: u32, down: u32) { $crate::web::key(code, down != 0) }
        #[no_mangle]
        pub extern "C" fn fk_frame(ms: f64) -> u32 { $crate::web::frame(ms) as u32 }
        #[no_mangle]
        pub extern "C" fn fk_pixels() -> *const u8 { $crate::web::pixels() }
        #[no_mangle]
        pub extern "C" fn fk_width() -> u32 { ($cfg).width as u32 }
        #[no_mangle]
        pub extern "C" fn fk_height() -> u32 { ($cfg).height as u32 }
        #[no_mangle]
        pub extern "C" fn fk_rate() -> u32 { $crate::audio::RATE }
        #[no_mangle]
        pub extern "C" fn fk_sound(n: u32) -> *const f32 { $crate::web::sound(n) }
        #[no_mangle]
        pub extern "C" fn fk_out() -> u32 { $crate::web::out_next() }
        #[no_mangle]
        pub extern "C" fn fk_out_ptr() -> *const u8 { $crate::web::out_ptr() }
        #[no_mangle]
        pub extern "C" fn fk_in(len: u32) -> *mut u8 { $crate::web::in_buf(len) }
        #[no_mangle]
        pub extern "C" fn fk_in_done() { $crate::web::in_done() }
    };
}
