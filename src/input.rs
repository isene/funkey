//! Keys, with a held state a terminal does not give on its own. There is
//! no key-up: a key counts as held from its press until its repeats stop
//! coming. The first repeat takes the terminal's repeat delay to arrive,
//! so a fresh press is held a little longer than a repeating one.

use crust::input::KeyState;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// After a first press, how long the key stays held without a repeat.
const FIRST_HOLD: Duration = Duration::from_millis(650);
/// Between repeats, how long the key stays held without the next one.
const REPEAT_HOLD: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Up, Down, Left, Right, Space, Enter, Escape, Tab, Backspace,
    Char(char),
}

struct Held {
    last: Instant,
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
}

impl Input {
    pub fn new() -> Input { Input::default() }

    /// Drain everything the terminal has queued. Call once per tick.
    pub fn poll(&mut self) {
        self.pressed.clear();
        self.resized = false;
        let now = Instant::now();
        while crust::input::Input::peek_pending() {
            let Some((name, state)) = crust::input::Input::event_ms(0) else { break };
            if name == "RESIZE" { self.resized = true; continue; }
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
                KeyState::Pressed => match self.held.get_mut(&key) {
                    // Without release reports a press within the window is a repeat.
                    Some(h) if self.exact || now.duration_since(h.last) < hold_for(h.repeats) => { h.last = now; h.repeats += 1; }
                    _ => { self.held.insert(key, Held { last: now, repeats: 0 }); self.pressed.push(key); }
                },
            }
        }
        if !self.exact {
            self.held.retain(|_, h| now.duration_since(h.last) < hold_for(h.repeats));
        }
    }

    /// True when the terminal reports key releases, so `held` is exact.
    pub fn exact(&self) -> bool { self.exact }

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

fn hold_for(repeats: u32) -> Duration {
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
        i.held.insert(Key::Up, Held { last: Instant::now() - Duration::from_secs(5), repeats: 0 });
        i.exact = true;
        assert!(i.held(Key::Up), "no timeout once releases are reported");
        assert!(i.motion(Key::Up));
        assert!(!i.tapped(Key::Up));
        i.held.remove(&Key::Up);
        assert!(!i.held(Key::Up));
    }

    #[test]
    fn a_press_is_held_until_its_repeats_stop() {
        let mut i = Input::new();
        let t0 = Instant::now();
        i.held.insert(Key::Right, Held { last: t0, repeats: 0 });
        assert!(i.held(Key::Right));
        i.held.insert(Key::Right, Held { last: t0 - Duration::from_millis(700), repeats: 0 });
        i.held.retain(|_, h| Instant::now().duration_since(h.last) < hold_for(h.repeats));
        assert!(!i.held(Key::Right), "no repeat within the first delay: released");
        i.held.insert(Key::Right, Held { last: t0 - Duration::from_millis(200), repeats: 3 });
        i.held.retain(|_, h| Instant::now().duration_since(h.last) < hold_for(h.repeats));
        assert!(!i.held(Key::Right), "repeats stopped: released");
    }
}
