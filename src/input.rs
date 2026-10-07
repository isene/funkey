//! Keys, with a held state a terminal does not give on its own. There is
//! no key-up: a key counts as held from its press until its repeats stop
//! coming. The first repeat takes the terminal's repeat delay to arrive,
//! so a fresh press is held a little longer than a repeating one. A web
//! page reports releases, so there none of the timing is needed.

// Without the terminal the timers go unused.
#![cfg_attr(not(feature = "term"), allow(dead_code))]

#[cfg(feature = "term")]
use crust::input::KeyState;
use std::collections::HashMap;

/// After a first press, how long the key stays held without a repeat, in
/// seconds.
const FIRST_HOLD: f64 = 0.65;
/// Between repeats, how long the key stays held without the next one.
const REPEAT_HOLD: f64 = 0.12;

/// Seconds on a clock that only runs forward. std has no clock in a web
/// page, so there the page sets it every frame (see `web`).
pub(crate) fn clock() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::now()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Up, Down, Left, Right, Space, Enter, Escape, Tab, Backspace,
    Char(char),
}

struct Held {
    last: f64,
    repeats: u32,
}

#[derive(Default)]
pub struct Input {
    held: HashMap<Key, Held>,
    pressed: Vec<Key>,
    pub resized: bool,
    /// True once the terminal has reported a release or a repeat: from
    /// then on a key is held exactly, from its press to its release.
    exact: bool,
    /// With `FUNKEY_KEYLOG=<file>`: every key event as it arrives, for a
    /// terminal where keys stick.
    pub(crate) keylog: Option<std::fs::File>,
}

impl Input {
    pub fn new() -> Input { Input::default() }

    /// Drain everything the terminal has queued. Call once per tick.
    #[cfg(feature = "term")]
    pub fn poll(&mut self) {
        self.pressed.clear();
        self.resized = false;
        let now = clock();
        while crust::input::Input::peek_pending() {
            let Some((name, state)) = crust::input::Input::event_ms(0) else { break };
            if name == "RESIZE" { self.resized = true; continue; }
            if let Some(f) = &mut self.keylog {
                use std::io::Write;
                let what = match state { KeyState::Pressed => "press", KeyState::Repeated => "repeat", KeyState::Released => "release" };
                let _ = writeln!(f, "{:.3} {} {}", now, if name == " " { "SPACE" } else { &name }, what);
            }
            let Some(key) = key_from_name(&name) else { continue };
            match state {
                KeyState::Released => { self.exact = true; self.held.remove(&key); }
                KeyState::Repeated => {
                    self.exact = true;
                    match self.held.get_mut(&key) {
                        Some(h) => { h.last = now; h.repeats += 1; }
                        None => { self.held.insert(key, Held { last: now, repeats: 1 }); }
                    }
                }
                KeyState::Pressed => self.press(key, now),
            }
        }
        self.expire(now);
    }

    fn press(&mut self, key: Key, now: f64) {
        match self.held.get_mut(&key) {
            // Without release reports a press within the window is a repeat.
            Some(h) if (self.exact && releases(key)) || now - h.last < hold_for(h.repeats) => { h.last = now; h.repeats += 1; }
            _ => { self.held.insert(key, Held { last: now, repeats: 0 }); self.pressed.push(key); }
        }
    }

    /// Let go of the keys whose repeats have stopped, where no release
    /// report will come.
    fn expire(&mut self, now: f64) {
        let exact = self.exact;
        self.held.retain(|&k, h| (exact && releases(k)) || now - h.last < hold_for(h.repeats));
    }

    /// True when the terminal reports key releases, so `held` is exact.
    pub fn exact(&self) -> bool { self.exact }

    /// True on a tick when any key went down.
    pub fn any_pressed(&self) -> bool { !self.pressed.is_empty() }

    /// Feed a key from outside the terminal: held this tick, and pressed
    /// unless it was already down. For scripted runs and tests.
    pub fn inject(&mut self, key: Key) {
        let now = clock();
        if !self.held.contains_key(&key) { self.pressed.push(key); }
        self.held.insert(key, Held { last: now, repeats: 1 });
        self.exact = true;
    }

    /// A key went down or up in a web page, which reports both, Enter's
    /// too. Repeats are the page's to drop.
    pub fn key(&mut self, key: Key, down: bool) {
        self.exact = true;
        if !down { self.held.remove(&key); return; }
        if !self.held.contains_key(&key) { self.pressed.push(key); }
        self.held.insert(key, Held { last: clock(), repeats: 0 });
    }

    /// Forget this tick's presses, keeping what is held: the start of a
    /// scripted tick.
    pub fn clear_pressed(&mut self) { self.pressed.clear(); }

    /// Let go of every key fed with `inject`.
    pub fn release_all(&mut self) { self.held.clear(); self.pressed.clear(); }

    /// Tell the input up front that the terminal reports releases, so
    /// the first press is not held on a timer while it waits to learn.
    pub fn set_exact(&mut self, exact: bool) { self.exact = self.exact || exact; }

    /// True while a key keeps repeating: held past the terminal's repeat
    /// delay. With release reports this is the same as `held`.
    pub fn repeating(&self, key: Key) -> bool {
        self.exact && self.held(key) || self.held.get(&key).map(|h| h.repeats > 0).unwrap_or(false)
    }

    /// A short press without release reports: the game should take one
    /// step. With release reports a tap is a short `held`, and this is
    /// never true.
    pub fn tapped(&self, key: Key) -> bool { !self.exact && self.pressed(key) }

    /// Continuous motion for a key: exact holding where the terminal
    /// reports releases, repeating elsewhere.
    pub fn motion(&self, key: Key) -> bool { if self.exact { self.held(key) } else { self.repeating(key) } }

    /// True while the key is down, as far as a terminal can tell.
    pub fn held(&self, key: Key) -> bool { self.held.contains_key(&key) }

    /// True on the tick the key went down.
    pub fn pressed(&self, key: Key) -> bool { self.pressed.contains(&key) }

    /// Left and right as -1, 0 or 1; the arrows or A and D.
    pub fn axis_x(&self) -> i32 {
        let l = self.held(Key::Left) || self.held(Key::Char('a')) || self.held(Key::Char('h'));
        let r = self.held(Key::Right) || self.held(Key::Char('d')) || self.held(Key::Char('l'));
        r as i32 - l as i32
    }

    /// Up and down as -1, 0 or 1; the arrows or W and S.
    pub fn axis_y(&self) -> i32 {
        let u = self.held(Key::Up) || self.held(Key::Char('w')) || self.held(Key::Char('k'));
        let d = self.held(Key::Down) || self.held(Key::Char('s')) || self.held(Key::Char('j'));
        d as i32 - u as i32
    }
}

/// A terminal that reports releases still sends none for Enter, Tab and
/// Backspace (the kitty protocol keeps them plain, so a shell stays
/// usable after a crash). Those three are held on a timer even then.
fn releases(key: Key) -> bool { !matches!(key, Key::Enter | Key::Tab | Key::Backspace) }

fn hold_for(repeats: u32) -> f64 {
    if repeats == 0 { FIRST_HOLD } else { REPEAT_HOLD }
}

/// crust names keys the way rcurses did: "UP", "ENTER", "ESC", or the
/// character itself.
pub fn key_from_name(name: &str) -> Option<Key> {
    Some(match name {
        "UP" => Key::Up, "DOWN" => Key::Down, "LEFT" => Key::Left, "RIGHT" => Key::Right,
        "ENTER" => Key::Enter, "ESC" => Key::Escape, "TAB" => Key::Tab, "BACK" => Key::Backspace,
        " " | "SPACE" => Key::Space,
        _ => {
            let mut it = name.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Key::Char(c.to_ascii_lowercase()),
                _ => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_keys() {
        assert_eq!(key_from_name("UP"), Some(Key::Up));
        assert_eq!(key_from_name("x"), Some(Key::Char('x')));
        assert_eq!(key_from_name("X"), Some(Key::Char('x')));
        assert_eq!(key_from_name(" "), Some(Key::Space));
        assert_eq!(key_from_name("C-UP"), None);
    }

    #[test]
    fn with_release_reports_a_key_is_held_until_released() {
        let mut i = Input::new();
        i.held.insert(Key::Up, Held { last: clock() - 5.0, repeats: 0 });
        i.exact = true;
        assert!(i.held(Key::Up), "no timeout once releases are reported");
        assert!(i.motion(Key::Up));
        assert!(!i.tapped(Key::Up));
        i.held.remove(&Key::Up);
        assert!(!i.held(Key::Up));
    }

    #[test]
    fn enter_is_pressed_again_though_its_release_never_comes() {
        let mut i = Input::new();
        i.exact = true;
        let t0 = clock();
        i.press(Key::Enter, t0);
        assert!(i.pressed(Key::Enter));
        i.expire(t0 + 0.7);
        assert!(!i.held(Key::Enter), "let go on the timer");
        i.pressed.clear();
        i.press(Key::Enter, t0 + 0.9);
        assert!(i.pressed(Key::Enter), "a second Enter is a new press");
        // A key with release reports stays held, with no timer.
        i.press(Key::Up, t0);
        i.expire(t0 + 5.0);
        assert!(i.held(Key::Up));
    }

    #[test]
    fn a_press_is_held_until_its_repeats_stop() {
        let mut i = Input::new();
        let t0 = clock();
        i.held.insert(Key::Right, Held { last: t0, repeats: 0 });
        assert!(i.held(Key::Right));
        i.held.insert(Key::Right, Held { last: t0 - 0.7, repeats: 0 });
        i.held.retain(|_, h| clock() - h.last < hold_for(h.repeats));
        assert!(!i.held(Key::Right), "no repeat within the first delay: released");
        i.held.insert(Key::Right, Held { last: t0 - 0.2, repeats: 3 });
        i.held.retain(|_, h| clock() - h.last < hold_for(h.repeats));
        assert!(!i.held(Key::Right), "repeats stopped: released");
    }
}
