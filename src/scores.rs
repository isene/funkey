//! A top ten with initials, the way an arcade cabinet keeps one. A game
//! says `scores::record(GAME, score)` when it is over, and the engine
//! does the rest. If the score makes the list, the initials are asked
//! for a second later, over the game's own picture, and then the list is
//! shown. Nothing runs meanwhile but the wait for a key.
//!
//! In a terminal the list is a file, ~/.funkey/<game>/scores, a line per
//! score: "ABC 12345". In a web page the page keeps it (see `page`), so
//! everyone who plays there can share one list. The engine tells the page
//! "list" when the game starts and "score ABC 12345" for a new score; the
//! page answers "scores" and a line per score, and "name ABC" with the
//! initials last typed there.

use crate::input::clock;
use crate::{page, store, Frame, Input, Key, Rgb};
use std::cell::RefCell;

/// One line of a list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub score: u32,
}

const KEEP: usize = 10;
/// The game runs on this long after its score, so its own "game over" is
/// seen before the initials are asked for.
const DELAY: f64 = 1.0;
/// Keys still going down from the game are not initials: a new screen
/// takes its first key only after this long with none.
const QUIET: f64 = 0.4;

const GOLD: Rgb = 0xf0c040;
const TEXT: Rgb = 0xe0e4f0;
const DIM: Rgb = 0x8088a0;

thread_local! {
    /// A score that made the list and waits for its initials: the game,
    /// the score and when it came.
    static PENDING: RefCell<Option<(String, u32, f64)>> = const { RefCell::new(None) };
    /// The game whose list the page was asked for.
    static ASKED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Three capital letters.
fn valid_name(n: &str) -> bool {
    n.len() == 3 && n.bytes().all(|b| b.is_ascii_uppercase())
}

fn text(list: &[Entry]) -> String {
    list.iter().map(|e| format!("{} {}\n", e.name, e.score)).collect()
}

/// A list from its text, best first. Lines that are no score are left
/// out, and so is anything on a line after the score.
fn parse(t: &str) -> Vec<Entry> {
    let mut list: Vec<Entry> = t.lines().filter_map(|l| {
        let mut w = l.split_whitespace();
        let name = w.next()?.to_string();
        let score = w.next()?.parse().ok()?;
        valid_name(&name).then_some(Entry { name, score })
    }).collect();
    list.sort_by(|a, b| b.score.cmp(&a.score));
    list.truncate(KEEP);
    list
}

/// Put a score where it belongs, below any equal one. None when it
/// misses the list.
fn place(list: &mut Vec<Entry>, e: Entry) -> Option<usize> {
    let at = list.iter().position(|o| e.score > o.score).unwrap_or(list.len());
    if at >= KEEP { return None; }
    list.insert(at, e);
    list.truncate(KEEP);
    Some(at)
}

/// In a web page: ask the page for this game's list, once.
fn ask_page(game: &str) {
    if !page::web() { return; }
    ASKED.with(|a| {
        if a.borrow().is_none() {
            *a.borrow_mut() = Some(game.to_string());
            page::send("list");
        }
    });
}

/// The top ten of a game, best first.
pub fn list(game: &str) -> Vec<Entry> {
    parse(&store::load(game, "scores").unwrap_or_default())
}

/// The best score the game has seen.
pub fn best(game: &str) -> u32 {
    ask_page(game);
    store::high_score(game).max(list(game).first().map_or(0, |e| e.score))
}

/// A game is over with this score. True when it beats the best so far.
/// A score that makes the top ten has the engine ask for initials.
pub fn record(game: &str, score: u32) -> bool {
    ask_page(game);
    let l = list(game);
    if score > 0 && (l.len() < KEEP || score > l[KEEP - 1].score) {
        PENDING.with(|p| *p.borrow_mut() = Some((game.to_string(), score, clock())));
    }
    store::record_score(game, score)
}

/// The score whose initials are to be asked for now, if there is one.
pub(crate) fn due() -> Option<(String, u32)> { due_at(clock()) }

fn due_at(now: f64) -> Option<(String, u32)> {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        if !matches!(*p, Some((_, _, t)) if now - t >= DELAY) { return None; }
        p.take().map(|(game, score, _)| (game, score))
    })
}

/// Take what the page has sent: the shared list, and the initials last
/// typed there. True when a list came. A game that never asked for a
/// list keeps its messages.
#[cfg(target_arch = "wasm32")]
pub(crate) fn hear() -> bool {
    if ASKED.with(|a| a.borrow().is_none()) { return false; }
    let mut new = false;
    while let Some(m) = page::recv() {
        if let Some(t) = m.strip_prefix("scores\n") {
            let game = ASKED.with(|a| a.borrow().clone()).unwrap_or_default();
            store::save(&game, "scores", &text(&parse(t)));
            new = true;
        } else if let Some(n) = m.strip_prefix("name ") {
            let n = n.trim().to_ascii_uppercase();
            if valid_name(&n) { store::save("", "name", &n); }
        }
    }
    new
}

/// What a key did to the screen that asks for initials.
#[derive(Debug, PartialEq)]
pub(crate) enum Step { Same, Changed, Done }

/// The screen that asks for initials, and then shows the list.
pub(crate) struct Ask {
    game: String,
    score: u32,
    name: [u8; 3],
    at: usize,
    /// True once the initials are on the list and the list is shown.
    listed: bool,
    /// When this screen came up, or the last key that came too early.
    since: f64,
    /// True once the keys have been quiet for a moment: from then on
    /// every key counts.
    armed: bool,
    /// The game's picture, dark, behind the box.
    back: Vec<Rgb>,
}

impl Ask {
    /// Open over `back`, the game's picture already made dark. The
    /// initials start as the ones typed last time, in any game.
    pub(crate) fn open(game: &str, score: u32, back: &Frame) -> Ask {
        let mut name = *b"AAA";
        if let Some(n) = store::load("", "name").filter(|n| valid_name(n)) { name.copy_from_slice(n.as_bytes()); }
        Ask { game: game.to_string(), score, name, at: 0, listed: false, since: clock(), armed: false, back: back.px.clone() }
    }

    /// This tick's keys. Type the initials, or turn a letter with Up and
    /// Down and move with Left and Right. Enter or Space puts them on the
    /// list, Escape skips it; a key on the list closes it. Space counts
    /// since a phone's buttons under the game may have no Enter.
    pub(crate) fn keys(&mut self, input: &Input) -> Step { self.keys_at(input, clock()) }

    fn keys_at(&mut self, input: &Input, now: f64) -> Step {
        // A fire key held through the last life must not sign the score:
        // while keys keep coming, the wait starts over.
        if !self.armed {
            if now - self.since < QUIET {
                if input.any_pressed() { self.since = now; }
                return Step::Same;
            }
            self.armed = true;
        }
        if self.listed { return if input.any_pressed() { Step::Done } else { Step::Same }; }
        let before = (self.name, self.at);
        for c in 'a'..='z' {
            if input.pressed(Key::Char(c)) || input.pressed(Key::Char(c.to_ascii_uppercase())) {
                self.name[self.at] = c.to_ascii_uppercase() as u8;
                self.at = (self.at + 1).min(2);
            }
        }
        let l = &mut self.name[self.at];
        if input.pressed(Key::Up) { *l = if *l >= b'Z' { b'A' } else { *l + 1 }; }
        if input.pressed(Key::Down) { *l = if *l <= b'A' { b'Z' } else { *l - 1 }; }
        if input.pressed(Key::Left) || input.pressed(Key::Backspace) { self.at = self.at.saturating_sub(1); }
        if input.pressed(Key::Right) { self.at = (self.at + 1).min(2); }
        if input.pressed(Key::Escape) { return Step::Done; }
        if input.pressed(Key::Enter) || input.pressed(Key::Space) {
            self.submit(now);
            return Step::Changed;
        }
        if before != (self.name, self.at) { Step::Changed } else { Step::Same }
    }

    fn initials(&self) -> String { String::from_utf8_lossy(&self.name).into_owned() }

    /// The initials go on the list: at once here, and to the page, which
    /// answers with the list everyone shares.
    fn submit(&mut self, now: f64) {
        let name = self.initials();
        let mut l = list(&self.game);
        place(&mut l, Entry { name: name.clone(), score: self.score });
        store::save(&self.game, "scores", &text(&l));
        store::save("", "name", &name);
        page::send(&format!("score {} {}", name, self.score));
        (self.listed, self.since, self.armed) = (true, now, false);
    }

    /// The box over the game's picture: the initials, or the list with
    /// the new score in gold. It grows with the frame.
    pub(crate) fn draw(&self, f: &mut Frame) {
        if self.back.len() == f.px.len() { f.px.copy_from_slice(&self.back); }
        let u = (f.w / 224).min(f.h / 190).max(1);
        let (x, y, cx) = ((f.w - 200 * u) / 2, (f.h - 170 * u) / 2, f.w / 2);
        f.rect(x, y, 200 * u, 170 * u, 0x0a0b16);
        f.rect(x, y, 200 * u, u, GOLD);
        f.rect(x, y + 169 * u, 200 * u, u, GOLD);
        let list = list(&self.game);
        if self.listed {
            let me = self.initials();
            let mine = list.iter().position(|e| e.name == me && e.score == self.score);
            f.text_centered(cx, y + 10 * u, "HIGH SCORES", GOLD, true, 2 * u);
            for (i, e) in list.iter().enumerate() {
                let yy = y + (34 + 12 * i as i32) * u;
                let c = if mine == Some(i) { GOLD } else { TEXT };
                f.text_scaled(x + 34 * u, yy, &format!("{:>2}", i + 1), DIM, true, u);
                f.text_scaled(x + 62 * u, yy, &e.name, c, true, u);
                let sc = e.score.to_string();
                f.text_scaled(x + 166 * u - Frame::text_width(&sc, true, u), yy, &sc, c, true, u);
            }
            f.text_centered(cx, y + 158 * u, "ANY KEY", DIM, false, u);
            return;
        }
        let rank = list.iter().position(|e| self.score > e.score).unwrap_or(list.len()) + 1;
        f.text_centered(cx, y + 10 * u, if rank == 1 { "NEW HIGH SCORE" } else { "TOP TEN" }, GOLD, true, 2 * u);
        f.text_centered(cx, y + 34 * u, &format!("{}   RANK {}", self.score, rank), TEXT, true, u);
        f.text_centered(cx, y + 52 * u, "YOUR INITIALS", DIM, false, u);
        for i in 0..3 {
            let bx = cx - 49 * u + i as i32 * 34 * u;
            let on = i == self.at;
            f.rect(bx, y + 64 * u, 30 * u, 34 * u, if on { GOLD } else { 0x3a4260 });
            f.rect(bx + 2 * u, y + 66 * u, 26 * u, 30 * u, 0x07080f);
            f.text_scaled(bx + 8 * u, y + 71 * u, &(self.name[i] as char).to_string(), if on { GOLD } else { TEXT }, true, 3 * u);
        }
        f.text_centered(cx, y + 108 * u, "TYPE THEM, OR UP AND DOWN TURN A LETTER", DIM, false, u);
        f.text_centered(cx, y + 118 * u, "LEFT AND RIGHT MOVE", DIM, false, u);
        f.text_centered(cx, y + 128 * u, "ENTER OR SPACE PUTS THEM ON THE LIST", DIM, false, u);
        f.text_centered(cx, y + 138 * u, "ESC SKIPS", DIM, false, u);
        if page::web() { f.text_centered(cx, y + 154 * u, "EVERYONE WHO PLAYS HERE SEES THIS LIST", 0x606880, false, u); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(game: &str) {
        let _ = std::fs::remove_dir_all(crate::test_dir().join(game));
        PENDING.with(|p| *p.borrow_mut() = None);
    }

    fn press(key: Key) -> Input {
        let mut i = Input::new();
        i.inject(key);
        i
    }

    #[test]
    fn a_list_is_read_best_first_and_junk_is_left_out() {
        let l = parse("ABC 100\nxyz 900\nDEF 300 40 5\n\nGHI many\nTOOLONG 5\n");
        assert_eq!(l, vec![Entry { name: "DEF".into(), score: 300 }, Entry { name: "ABC".into(), score: 100 }]);
        assert_eq!(text(&l), "DEF 300\nABC 100\n");
        let long: String = (1..=14).map(|n| format!("AAA {n}\n")).collect();
        assert_eq!(parse(&long).len(), 10, "ten are kept");
        assert_eq!(parse(&long)[9].score, 5);
    }

    #[test]
    fn a_score_goes_below_an_equal_one_and_the_eleventh_falls_off() {
        let mut l = parse("AAA 50\nBBB 30\n");
        assert_eq!(place(&mut l, Entry { name: "CCC".into(), score: 50 }), Some(1));
        assert_eq!(text(&l), "AAA 50\nCCC 50\nBBB 30\n");
        let mut full = parse(&(1..=10).map(|n| format!("AAA {}\n", n * 10)).collect::<String>());
        assert_eq!(place(&mut full, Entry { name: "LOW".into(), score: 10 }), None, "equal to the last misses");
        assert_eq!(place(&mut full, Entry { name: "NEW".into(), score: 55 }), Some(5));
        assert_eq!(full.len(), 10);
        assert_eq!(full[9].score, 20, "the lowest fell off");
    }

    #[test]
    fn a_score_for_the_list_is_asked_about_a_second_later() {
        let game = "scores-due";
        fresh(game);
        assert!(!record(game, 0), "nothing is no score");
        assert_eq!(due_at(clock() + 5.0), None);
        assert!(record(game, 120));
        assert_eq!(due_at(clock() + 0.5), None, "the game's own game over is seen first");
        assert_eq!(due_at(clock() + 1.5), Some((game.to_string(), 120)));
        assert_eq!(due_at(clock() + 9.0), None, "and asked for once");
        // A full list takes only a score above its last.
        store::save(game, "scores", &(1..=10).map(|n| format!("AAA {}\n", n * 100)).collect::<String>());
        record(game, 100);
        assert_eq!(due_at(clock() + 5.0), None);
        record(game, 101);
        assert_eq!(due_at(clock() + 5.0), Some((game.to_string(), 101)));
        assert_eq!(best(game), 1000, "the best is the top of the list");
    }

    #[test]
    fn initials_are_typed_or_turned_and_go_on_the_list() {
        let game = "scores-ask";
        fresh(game);
        store::save(game, "scores", "TOP 900\nLOW 100\n");
        let back = Frame::new(224, 256);
        let mut a = Ask::open(game, 500, &back);
        a.name = *b"AAA";
        let t = a.since;
        // Fire held through the last life: its repeats never sign the score.
        for n in 1..=40 {
            assert_eq!(a.keys_at(&press(Key::Space), t + n as f64 * 0.1), Step::Same, "a key from the game is dropped");
        }
        assert_eq!((a.name, a.listed), (*b"AAA", false));
        let t = t + 4.0;
        assert_eq!(a.keys_at(&press(Key::Char('g')), t + 1.0), Step::Changed);
        assert_eq!((a.name, a.at), (*b"GAA", 1));
        assert_eq!(a.keys_at(&press(Key::Down), t + 1.0), Step::Changed);
        assert_eq!(a.name, *b"GZA", "down from A is Z");
        assert_eq!(a.keys_at(&press(Key::Up), t + 1.0), Step::Changed);
        a.keys_at(&press(Key::Right), t + 1.0);
        a.keys_at(&press(Key::Char('I')), t + 1.0);
        assert_eq!((a.name, a.at), (*b"GAI", 2), "a capital is a letter too, and the last box stays");
        assert_eq!(a.keys_at(&press(Key::Left), t + 1.0), Step::Changed);
        a.keys_at(&press(Key::Char('e')), t + 1.0);
        assert_eq!(a.name, *b"GEI");
        assert_eq!(a.keys_at(&Input::new(), t + 1.0), Step::Same);
        assert_eq!(a.keys_at(&press(Key::Enter), t + 2.0), Step::Changed);
        assert_eq!(text(&list(game)), "TOP 900\nGEI 500\nLOW 100\n");
        assert_eq!(store::load("", "name").as_deref(), Some("GEI"), "the initials are kept for the next game");
        assert_eq!(a.keys_at(&press(Key::Enter), t + 2.1), Step::Same, "the same Enter does not close the list");
        assert_eq!(a.keys_at(&press(Key::Char('q')), t + 3.0), Step::Done);
        // Space signs too, for a phone with no Enter under the game.
        let mut c = Ask::open(game, 600, &back);
        assert_eq!(c.keys_at(&press(Key::Space), c.since + 1.0), Step::Changed);
        assert_eq!(list(game).len(), 4);
        // The next time the initials are there already, and Escape skips.
        let mut b = Ask::open(game, 700, &back);
        assert_eq!(b.name, *b"GEI");
        assert_eq!(b.keys_at(&press(Key::Escape), b.since + 1.0), Step::Done);
        assert_eq!(text(&list(game)), "TOP 900\nGEI 600\nGEI 500\nLOW 100\n", "a skipped score is not listed");
    }

    #[test]
    fn the_box_fits_the_smallest_game_and_grows_with_a_big_one() {
        let game = "scores-draw";
        fresh(game);
        for (w, h, u) in [(224, 256, 1), (640, 400, 2)] {
            let mut f = Frame::new(w, h);
            f.clear(0x123456);
            let mut a = Ask::open(game, 4200, &f);
            a.draw(&mut f);
            let (x, y) = ((w - 200 * u) / 2, (h - 170 * u) / 2);
            assert_eq!(f.get(x, y), GOLD, "the box starts where it should");
            assert_eq!(f.get(x + 200 * u - 1, y + 170 * u - 1), GOLD);
            assert_eq!(f.get(x - 1, y), 0x123456, "and leaves the game's picture around it");
            assert_eq!(f.get(x, y + 170 * u), 0x123456);
            let inked = |f: &Frame, c: Rgb| f.px.iter().filter(|&&p| p == c).count();
            assert!(inked(&f, TEXT) > 50 * (u * u) as usize, "the score and two letters are written");
            // The two screens as pictures, for a look after a change.
            let shot = |f: &Frame, what: &str| { let _ = std::fs::write(crate::test_dir().join(format!("ask-{w}-{what}.ppm")), f.to_ppm()); };
            shot(&f, "name");
            // On the list the new score is gold, the others are not.
            let others: String = [("BEN", 3100), ("CAT", 2500), ("DAN", 1800), ("EVE", 900), ("FAY", 700), ("GUS", 450), ("HAL", 120), ("IDA", 60)]
                .iter().map(|(n, s)| format!("{n} {s}\n")).collect();
            store::save(game, "scores", &format!("TOP 9000\nAAA 4200\n{others}"));
            (a.name, a.listed) = (*b"AAA", true);
            a.draw(&mut f);
            let row = |f: &Frame, i: i32, c: Rgb| (0..8 * u).any(|dy| (x + 62 * u..x + 166 * u).any(|px| f.get(px, y + (34 + 12 * i) * u + dy) == c));
            shot(&f, "list");
            assert!(row(&f, 0, TEXT) && !row(&f, 0, GOLD));
            assert!(row(&f, 1, GOLD) && !row(&f, 1, TEXT));
            assert_eq!(f.get(x - 1, y), 0x123456);
        }
    }
}
