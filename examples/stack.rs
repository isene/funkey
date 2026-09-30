//! stack: a tribute to Tetris (Alexey Pajitnov, 1984). Seven pieces of
//! four blocks fall into a well ten wide, and a full row clears. The
//! pieces turn the way modern Tetris turns them, kicks off the walls
//! included, and a bag of seven deals them, so the long bar is never far
//! off. Hold a piece for later, see the next five, drop soft or hard.
//! Clear four rows at once, spin a T into a slot, chain clears into
//! combos; every ten rows the level goes up and the pieces fall faster.
//!
//!     cargo run --release --example stack
//!
//! Left and right move, Down drops softly, Space drops hard, Up or X turns
//! right, Z turns left, C holds, P pauses, Q or Esc quits.
//! A top-ten game asks for three initials and puts them on the list: in a
//! terminal the list lives on disk, in a web page the page keeps one list
//! for everyone who plays there (server/scores.rb).
//!
//! Left alone on the title, the game plays itself. `STACK_LEVEL=<n>` sets
//! the starting level, `STACK_DEMO=1` starts with the game playing itself.
//! The music is Korobeiniki, a Russian folk song from the 1860s; the rest
//! is new.

use funkey::*;
use std::collections::VecDeque;

const W: i32 = 480;
const H: i32 = 270;
const COLS: i32 = 10;
/// Rows in the well; the top two are hidden, where pieces enter.
const ROWS: i32 = 22;
const HIDDEN: i32 = 2;
const CELL: i32 = 12;
/// The well's top left corner on screen.
const FX: i32 = (W - COLS * CELL) / 2;
const FY: i32 = 15;
const GAME: &str = "stack";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.1";
const TEXT: Rgb = 0xf0f0f0;
const DIM: Rgb = 0x8890a8;
const GOLD: Rgb = 0xffd040;
/// Seconds a side key is held before the piece slides, and between steps.
const DAS: f32 = 0.16;
const ARR: f32 = 0.035;
/// Seconds a landed piece waits before it locks, and how many moves may
/// restart that wait.
const LOCK: f32 = 0.5;
const RESETS: u32 = 15;
/// Seconds a clear takes on screen.
const CLEAR: f32 = 0.32;

// ------------------------------------------------------------------ pieces

/// The pieces in order: I, O, T, S, Z, J, L.
const T: usize = 2;
const COLORS: [Rgb; 9] = [0, 0x2ee0ee, 0xf2cc2a, 0xb050e8, 0x50d060, 0xf04850, 0x3c6af0, 0xf08c28, 0x5a6070];
/// The last colour: a row of the game-over curtain.
const GREY: u8 = 8;

/// Each piece in its first position, inside a box 4 wide (I, O) or 3.
const SHAPES: [[(i32, i32); 4]; 7] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(1, 0), (2, 0), (1, 1), (2, 1)],
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
];

/// The blocks of a piece turned right `rot` times, inside its box.
fn cells(kind: usize, rot: usize) -> [(i32, i32); 4] {
    let mut c = SHAPES[kind];
    if kind == 1 { return c; }
    let n = if kind == 0 { 4 } else { 3 };
    for _ in 0..rot % 4 {
        for p in c.iter_mut() { *p = (n - 1 - p.1, p.0); }
    }
    c
}

/// The standard wall kicks, tried in order when a turn is blocked. Up is
/// positive here, as the tables are written; the well counts down.
const KICKS: [[(i32, i32); 5]; 8] = [
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)], // 0 to R
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],     // R to 0
    [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)],     // R to 2
    [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)], // 2 to R
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],    // 2 to L
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],  // L to 2
    [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)],  // L to 0
    [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)],    // 0 to L
];
const KICKS_I: [[(i32, i32); 5]; 8] = [
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
    [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)],
    [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)],
    [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)],
    [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)],
];

fn kicks(kind: usize, from: usize, to: usize) -> [(i32, i32); 5] {
    let i = match (from, to) { (0, 1) => 0, (1, 0) => 1, (1, 2) => 2, (2, 1) => 3, (2, 3) => 4, (3, 2) => 5, (3, 0) => 6, _ => 7 };
    if kind == 0 { KICKS_I[i] } else { KICKS[i] }
}

/// Seconds a piece takes to fall one row at a level, as modern Tetris has it.
fn fall_time(level: u32) -> f32 {
    let l = level.clamp(1, 20) as f32;
    (0.8 - (l - 1.0) * 0.007).powf(l - 1.0)
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Piece { kind: usize, rot: usize, x: i32, y: i32 }

// ------------------------------------------------------------------ music

/// Korobeiniki: two turns of the tune, then the slow part.
const TUNE_A: &str = "e5/4 b4/8 c5/8 d5/4 c5/8 b4/8 a4/4 a4/8 c5/8 e5/4 d5/8 c5/8 b4/2.667 c5/8 d5/4 e5/4 c5/4 a4/4 a4/4 -/4 \
    d5/2.667 f5/8 a5/4 g5/8 f5/8 e5/2.667 c5/8 e5/4 d5/8 c5/8 b4/4 b4/8 c5/8 d5/4 e5/4 c5/4 a4/4 a4/4 -/4";
const TUNE_B: &str = "e5/2 c5/2 d5/2 b4/2 c5/2 a4/2 g#4/2 b4/2 e5/2 c5/2 d5/2 b4/2 c5/4 e5/4 a5/2 g#5/1";
const CHORDS_A: [&str; 8] = ["e", "a", "e", "a", "d", "c", "e", "a"];
const CHORDS_B: [&str; 8] = ["a", "e", "a", "e", "a", "e", "a", "e"];

/// The whole song at a tempo: the tune on a square wave over a bass that
/// walks the root in octaves, mixed into one loop so the two never drift.
fn song(bpm: u32) -> Sample {
    let bar = |c: &str| format!("{c}2/8 {c}3/8 {c}2/8 {c}3/8 {c}2/8 {c}3/8 {c}2/8 {c}3/8 ");
    let bass_a: String = CHORDS_A.iter().map(|c| bar(c)).collect();
    let bass_b: String = CHORDS_B.iter().map(|c| bar(c)).collect();
    let tune = format!("{bpm} {TUNE_A} {TUNE_A} {TUNE_B}");
    let bass = format!("{bpm} {bass_a} {bass_a} {bass_b}");
    let a = Tune::parse(&tune, Wave::Square, 0.13).render();
    let b = Tune::parse(&bass, Wave::Triangle, 0.3).render();
    let at = |s: &Sample, i: usize| *s.data.get(i).unwrap_or(&0) as i32;
    let n = a.data.len().max(b.data.len());
    Sample::from_i16((0..n).map(|i| (at(&a, i) + at(&b, i)).clamp(-32768, 32767) as i16).collect())
}

struct Sounds {
    step: Sample,
    turn: Sample,
    land: Sample,
    drop: Sample,
    hold: Sample,
    clear: [Sample; 3],
    four: Sample,
    spin: Sample,
    level: Sample,
    over: Sample,
    calm: Sample,
    rush: Sample,
    title: Sample,
}

fn sounds() -> Sounds {
    let t = |s: &str, w: Wave, v: f32| Tune::parse(s, w, v).render();
    Sounds {
        step: Sample::tone(Wave::Square, 1400.0, 0.012, 0.12),
        turn: Sample::sweep(Wave::Triangle, 900.0, 1400.0, 0.035, 0.3),
        land: Sample::sweep(Wave::Triangle, 220.0, 120.0, 0.05, 0.45),
        drop: Sample::sweep(Wave::Square, 520.0, 80.0, 0.09, 0.3),
        hold: t("600 e6/32 b5/32", Wave::Triangle, 0.35),
        clear: [
            t("420 c6/16 g6/16", Wave::Triangle, 0.4),
            t("420 c6/16 e6/16 g6/16", Wave::Triangle, 0.4),
            t("420 c6/16 e6/16 g6/16 c7/16", Wave::Triangle, 0.4),
        ],
        four: t("320 c6/16 e6/16 g6/16 c7/16 e7/16 g7/16 c8/8", Wave::Square, 0.26),
        spin: t("360 g5/16 d6/16 g6/16 b6/16 d7/8", Wave::Square, 0.26),
        level: t("260 e6/16 g6/16 c7/16 e7/16 g7/8", Wave::Triangle, 0.45),
        over: t("150 e5/8 d#5/8 d5/8 c#5/8 c5/8 b4/8 a#4/8 a4/2", Wave::Square, 0.25),
        calm: song(150),
        rush: song(196),
        title: song(120),
    }
}

// ------------------------------------------------------------------ the game

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Play, Paused, Over(f32), Name }

/// A line of the top ten.
#[derive(Clone, Debug, PartialEq)]
struct Entry { name: String, score: u32, lines: u32, level: u32 }

/// A word that rises from the well and fades: TETRIS, COMBO 3, LEVEL 4.
struct Popup { text: String, color: Rgb, age: f32, big: bool }

struct Stack {
    mode: Mode,
    grid: [[u8; COLS as usize]; ROWS as usize],
    cur: Piece,
    hold: Option<usize>,
    held: bool,
    bag: Vec<usize>,
    queue: VecDeque<usize>,
    rng: Rng,
    score: u32,
    high: u32,
    lines: u32,
    level: u32,
    start_level: u32,
    /// Rows of fall owed to gravity, and the landed piece's lock clock.
    fall: f32,
    lock_t: f32,
    resets: u32,
    lowest: i32,
    /// The side key sliding the piece, how long it has been held, and the
    /// clock between slide steps.
    das_dir: i32,
    das_t: f32,
    arr_t: f32,
    /// Whether the piece's last move was a turn, and by which kick: the
    /// T-spin test needs both.
    last_turn: bool,
    last_kick: usize,
    combo: i32,
    b2b: bool,
    clearing: Vec<i32>,
    clear_t: f32,
    clear_len: f32,
    popups: Vec<Popup>,
    particles: Particles,
    /// Streaks a hard drop leaves: column, top row, bottom row, colour, age.
    trails: Vec<(i32, i32, i32, Rgb, f32)>,
    shake: f32,
    time: f32,
    fours: u32,
    spins: u32,
    best_combo: i32,
    rain: Vec<(usize, usize, f32, f32, f32)>,
    danger: bool,
    backdrop: Vec<Rgb>,
    backdrop_level: u32,
    new_high: bool,
    /// The top ten, best first. In a web page the page brings the list
    /// everyone shares; in a terminal it lives on disk.
    board: Vec<Entry>,
    /// The initials being typed after a top-ten game, the letter the
    /// cursor is on, and where the new score landed on the list.
    name: [u8; 3],
    name_at: usize,
    placed: Option<usize>,
    /// This game's initials have been asked for (or skipped), and how long
    /// the asking has shown: keys still falling from the game are ignored.
    asked: bool,
    name_t: f32,
    /// The game playing itself: its plan for the piece (turn, column), the
    /// clock between its keys, and how long the title has sat untouched.
    demo: bool,
    plan: Option<(usize, i32)>,
    demo_t: f32,
    idle: f32,
    audio: Audio,
    s: Sounds,
}

impl Stack {
    fn new() -> Stack {
        let mut rng = Rng::from_time();
        let rain = (0..16).map(|i| (i % 7, rng.below(4) as usize, rng.range(0.0, W as f32), rng.range(-60.0, H as f32), rng.range(12.0, 34.0))).collect();
        let start_level = std::env::var("STACK_LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(1u32).clamp(1, 15);
        let board = parse_board(&funkey::store::load(GAME, "scores").unwrap_or_default());
        let mut name = *b"AAA";
        if let Some(n) = funkey::store::load(GAME, "name").filter(|n| valid_name(n)) { name.copy_from_slice(n.as_bytes()); }
        let mut g = Stack {
            mode: Mode::Title, grid: [[0; COLS as usize]; ROWS as usize], cur: Piece { kind: 0, rot: 0, x: 3, y: 0 },
            hold: None, held: false, bag: Vec::new(), queue: VecDeque::new(), rng, score: 0,
            high: funkey::store::high_score(GAME).max(board.first().map_or(0, |e| e.score)), lines: 0, level: start_level, start_level,
            fall: 0.0, lock_t: 0.0, resets: 0, lowest: 0, das_dir: 0, das_t: 0.0, arr_t: 0.0,
            last_turn: false, last_kick: 0, combo: -1, b2b: false, clearing: Vec::new(), clear_t: 0.0, clear_len: CLEAR,
            popups: Vec::new(), particles: Particles::new(), trails: Vec::new(), shake: 0.0, time: 0.0,
            fours: 0, spins: 0, best_combo: 0, rain, danger: false, backdrop: Vec::new(), backdrop_level: 0,
            new_high: false, board, name, name_at: 0, placed: None, asked: false, name_t: 0.0, demo: false, plan: None, demo_t: 0.0, idle: 0.0, audio: Audio::off(), s: sounds(),
        };
        g.particles.gravity = 260.0;
        g.backdrop = backdrop(g.level);
        g.backdrop_level = g.level;
        g
    }

    fn start(&mut self) {
        self.grid = [[0; COLS as usize]; ROWS as usize];
        self.hold = None;
        self.held = false;
        self.bag.clear();
        self.queue.clear();
        self.score = 0;
        self.lines = 0;
        self.level = self.start_level;
        self.combo = -1;
        self.b2b = false;
        self.clearing.clear();
        self.popups.clear();
        self.trails.clear();
        self.fours = 0;
        self.spins = 0;
        self.best_combo = 0;
        self.new_high = false;
        self.placed = None;
        self.asked = false;
        self.danger = false;
        self.demo = false;
        self.mode = Mode::Play;
        self.spawn();
        self.audio.play_loop(1, &self.s.calm, 0.55);
    }

    // -------------------------------------------------------------- the well

    fn fits(&self, kind: usize, rot: usize, x: i32, y: i32) -> bool {
        cells(kind, rot).iter().all(|&(cx, cy)| {
            let (bx, by) = (x + cx, y + cy);
            (0..COLS).contains(&bx) && (0..ROWS).contains(&by) && self.grid[by as usize][bx as usize] == 0
        })
    }

    fn fits_at(&self, dx: i32, dy: i32) -> bool {
        let p = self.cur;
        self.fits(p.kind, p.rot, p.x + dx, p.y + dy)
    }

    /// How far the piece can fall from where it is.
    fn drop_distance(&self) -> i32 {
        let mut d = 0;
        while self.fits_at(0, d + 1) { d += 1; }
        d
    }

    /// The next piece: the bag of seven, shuffled, refilled when empty.
    fn next_piece(&mut self) -> usize {
        while self.queue.len() < 6 {
            if self.bag.is_empty() {
                self.bag = (0..7).collect();
                for i in (1..7).rev() {
                    let j = self.rng.below(i as u32 + 1) as usize;
                    self.bag.swap(i, j);
                }
            }
            let k = self.bag.pop().unwrap();
            self.queue.push_back(k);
        }
        self.queue.pop_front().unwrap()
    }

    fn spawn(&mut self) {
        let k = self.next_piece();
        self.enter(k);
    }

    /// A piece enters at the top, and drops one row at once if it can. It
    /// cannot enter where blocks already are: the game is over.
    fn enter(&mut self, kind: usize) {
        self.cur = Piece { kind, rot: 0, x: 3, y: 0 };
        self.fall = 0.0;
        self.lock_t = 0.0;
        self.resets = 0;
        self.lowest = 0;
        self.last_turn = false;
        self.plan = None;
        if !self.fits(kind, 0, 3, 0) { self.game_over(); return; }
        if self.fits_at(0, 1) { self.cur.y = 1; self.lowest = 1; }
    }

    /// After a move that worked: a landed piece gets its lock wait back, a
    /// few times over.
    fn moved(&mut self) {
        if self.resets < RESETS { self.lock_t = 0.0; self.resets += 1; }
    }

    fn shift(&mut self, dx: i32) -> bool {
        if !self.fits_at(dx, 0) { return false; }
        self.cur.x += dx;
        self.last_turn = false;
        self.moved();
        self.audio.play(&self.s.step, 0.6);
        true
    }

    fn down(&mut self) -> bool {
        if !self.fits_at(0, 1) { return false; }
        self.cur.y += 1;
        self.last_turn = false;
        if self.cur.y > self.lowest { self.lowest = self.cur.y; self.resets = 0; self.lock_t = 0.0; }
        true
    }

    /// Turn right (1) or left (-1), trying the kicks in order.
    fn turn(&mut self, dir: i32) -> bool {
        let p = self.cur;
        if p.kind == 1 { return false; }
        let to = (p.rot as i32 + dir).rem_euclid(4) as usize;
        for (i, (kx, ky)) in kicks(p.kind, p.rot, to).into_iter().enumerate() {
            if self.fits(p.kind, to, p.x + kx, p.y - ky) {
                self.cur = Piece { rot: to, x: p.x + kx, y: p.y - ky, ..p };
                self.last_turn = true;
                self.last_kick = i;
                if self.cur.y > self.lowest { self.lowest = self.cur.y; self.resets = 0; }
                self.moved();
                self.audio.play(&self.s.turn, 0.5);
                return true;
            }
        }
        false
    }

    fn hard_drop(&mut self) {
        let d = self.drop_distance();
        let p = self.cur;
        if d > 0 {
            let color = COLORS[p.kind + 1];
            let mut cols: Vec<(i32, i32)> = Vec::new();
            for (cx, cy) in cells(p.kind, p.rot) {
                match cols.iter_mut().find(|c| c.0 == p.x + cx) {
                    Some(c) => c.1 = c.1.min(p.y + cy),
                    None => cols.push((p.x + cx, p.y + cy)),
                }
            }
            for (x, top) in cols { self.trails.push((x, top, top + d, color, 0.0)); }
            self.cur.y += d;
            self.last_turn = false;
            self.score += 2 * d as u32;
            self.shake = self.shake.max(0.12);
        }
        self.audio.play(&self.s.drop, 0.8);
        self.lock();
    }

    fn hold_piece(&mut self) {
        if self.held { return; }
        let k = self.cur.kind;
        match self.hold.replace(k) {
            Some(h) => self.enter(h),
            None => self.spawn(),
        }
        self.held = true;
        self.audio.play(&self.s.hold, 0.7);
    }

    /// A T turned into place with three of the four corners around its
    /// middle filled is a T-spin; with only one of the two corners it
    /// points at, and no long kick, it is a mini.
    fn tspin(&self) -> Option<bool> {
        let p = self.cur;
        if p.kind != T || !self.last_turn { return None; }
        let filled = |x: i32, y: i32| !(0..COLS).contains(&x) || y >= ROWS || (y >= 0 && self.grid[y as usize][x as usize] != 0);
        let c = [(p.x, p.y), (p.x + 2, p.y), (p.x + 2, p.y + 2), (p.x, p.y + 2)];
        if c.iter().filter(|&&(x, y)| filled(x, y)).count() < 3 { return None; }
        let front = match p.rot { 0 => [0, 1], 1 => [1, 2], 2 => [2, 3], _ => [3, 0] };
        Some(front.iter().all(|&i| filled(c[i].0, c[i].1)) || self.last_kick == 4)
    }

    fn lock(&mut self) {
        let p = self.cur;
        let spin = self.tspin();
        let mut above = true;
        for (cx, cy) in cells(p.kind, p.rot) {
            self.grid[(p.y + cy) as usize][(p.x + cx) as usize] = p.kind as u8 + 1;
            if p.y + cy >= HIDDEN { above = false; }
        }
        self.held = false;
        self.audio.play(&self.s.land, 0.7);
        let rows: Vec<i32> = (0..ROWS).filter(|&y| self.grid[y as usize].iter().all(|&c| c != 0)).collect();
        self.score_clear(rows.len(), spin);
        if above { self.game_over(); return; }
        if rows.is_empty() { self.spawn(); return; }
        for &y in &rows {
            if y < HIDDEN { continue; }
            for x in 0..COLS {
                let c = COLORS[self.grid[y as usize][x as usize] as usize];
                let (sx, sy) = (FX + x * CELL + CELL / 2, FY + (y - HIDDEN) * CELL + CELL / 2);
                self.particles.burst(sx as f32, sy as f32, if rows.len() == 4 { 7 } else { 4 }, 110.0, 0.7, c);
            }
        }
        self.clear_len = if rows.len() == 4 { CLEAR * 1.5 } else { CLEAR };
        self.clear_t = self.clear_len;
        self.clearing = rows;
    }

    /// The points for a lock that cleared `n` rows, the words that go with
    /// them, and the level.
    fn score_clear(&mut self, n: usize, spin: Option<bool>) {
        let lvl = self.level;
        let (base, name) = match spin {
            Some(true) => ([400, 800, 1200, 1600][n.min(3)], ["T-SPIN", "T-SPIN SINGLE", "T-SPIN DOUBLE", "T-SPIN TRIPLE"][n.min(3)]),
            Some(false) => ([100, 200, 400, 400][n.min(3)], ["MINI T-SPIN", "MINI T-SPIN SINGLE", "MINI T-SPIN DOUBLE", "MINI T-SPIN DOUBLE"][n.min(3)]),
            None => ([0, 100, 300, 500, 800][n.min(4)], ["", "", "DOUBLE", "TRIPLE", "TETRIS"][n.min(4)]),
        };
        let mut pts = base * lvl;
        if spin.is_some() {
            self.spins += 1;
            self.audio.play(&self.s.spin, 0.8);
        }
        if n > 0 {
            let hard = n == 4 || spin.is_some();
            if hard && self.b2b {
                pts = pts * 3 / 2;
                self.say("BACK-TO-BACK", 0x80e0ff, false);
            }
            self.b2b = hard;
            self.combo += 1;
            if self.combo > 0 {
                pts += 50 * self.combo as u32 * lvl;
                self.best_combo = self.best_combo.max(self.combo);
                self.say(&format!("COMBO {}", self.combo), 0xff9040, false);
            }
            if n == 4 {
                self.fours += 1;
                self.shake = 0.35;
                self.audio.play(&self.s.four, 0.9);
            } else {
                self.audio.play(&self.s.clear[n - 1], 0.8);
            }
        } else {
            self.combo = -1;
        }
        if !name.is_empty() {
            let c = if n == 4 { GOLD } else if spin.is_some() { 0xd080ff } else { TEXT };
            self.say(name, c, n == 4 || (spin == Some(true) && n > 0));
        }
        self.score += pts;
        self.lines += n as u32;
        let level = self.start_level + self.lines / 10;
        if level > self.level {
            self.level = level;
            self.say(&format!("LEVEL {}", level), 0x80ff90, false);
            self.audio.play(&self.s.level, 0.8);
        }
    }

    /// Take the cleared rows out and let the rest fall; an empty well
    /// after that is an all clear.
    fn finish_clear(&mut self) {
        let n = self.clearing.len();
        let mut kept: Vec<[u8; COLS as usize]> = (0..ROWS).filter(|y| !self.clearing.contains(y)).map(|y| self.grid[y as usize]).collect();
        while kept.len() < ROWS as usize { kept.insert(0, [0; COLS as usize]); }
        for (y, row) in kept.into_iter().enumerate() { self.grid[y] = row; }
        self.clearing.clear();
        if self.grid.iter().all(|r| r.iter().all(|&c| c == 0)) {
            self.score += [0, 800, 1200, 1800, 2000][n.min(4)] * self.level;
            self.say("ALL CLEAR", 0x60ffe0, true);
        }
        self.spawn();
    }

    fn say(&mut self, text: &str, color: Rgb, big: bool) {
        self.popups.push(Popup { text: text.to_string(), color, age: 0.0, big });
    }

    fn game_over(&mut self) {
        if matches!(self.mode, Mode::Over(_)) { return; }
        self.mode = Mode::Over(0.0);
        // Tests play to the end too; they leave the high score alone.
        self.new_high = !cfg!(test) && !self.demo && funkey::store::record_score(GAME, self.score);
        if self.new_high { self.high = self.score; }
        self.audio.stop(1);
        self.audio.play(&self.s.over, 0.9);
    }

    /// The highest row with a block in it.
    fn top(&self) -> i32 {
        (0..ROWS).find(|&y| self.grid[y as usize].iter().any(|&c| c != 0)).unwrap_or(ROWS)
    }

    fn to_title(&mut self) {
        self.mode = Mode::Title;
        self.demo = false;
        self.idle = 0.0;
        self.audio.play_loop(1, &self.s.title, 0.5);
    }

    /// A top-ten score, from a game played by hand.
    fn top_ten(&self) -> bool {
        !self.demo && self.score > 0 && (self.board.len() < 10 || self.score > self.board[9].score)
    }

    /// A message from the web page: the shared list, or the last initials
    /// typed in this browser.
    fn heard(&mut self, m: &str) {
        if let Some(t) = m.strip_prefix("scores\n") {
            self.board = parse_board(t);
            if let Some(top) = self.board.first() { self.high = self.high.max(top.score); }
            if self.placed.is_some() {
                let me = String::from_utf8_lossy(&self.name).into_owned();
                self.placed = self.board.iter().position(|e| e.name == me && e.score == self.score);
            }
        } else if let Some(n) = m.strip_prefix("name ") {
            let n = n.trim().to_ascii_uppercase();
            if valid_name(&n) { self.name.copy_from_slice(n.as_bytes()); }
        }
    }

    /// Initials, arcade style: type them, or turn a letter with Up and
    /// Down and move with Left and Right. Enter puts them on the list.
    fn name_entry(&mut self, input: &Input) {
        for c in 'a'..='z' {
            if input.pressed(Key::Char(c)) {
                self.name[self.name_at] = c.to_ascii_uppercase() as u8;
                self.name_at = (self.name_at + 1).min(2);
            }
        }
        let l = &mut self.name[self.name_at];
        if input.pressed(Key::Up) { *l = if *l >= b'Z' { b'A' } else { *l + 1 }; }
        if input.pressed(Key::Down) { *l = if *l <= b'A' { b'Z' } else { *l - 1 }; }
        if input.pressed(Key::Left) || input.pressed(Key::Backspace) { self.name_at = self.name_at.saturating_sub(1); }
        if input.pressed(Key::Right) { self.name_at = (self.name_at + 1).min(2); }
        if input.pressed(Key::Enter) { self.submit(); }
        if input.pressed(Key::Escape) { self.mode = Mode::Over(1.0); }
    }

    /// The initials go on the list: at once here, and to the page, which
    /// sends back the shared list; in a terminal, to disk.
    fn submit(&mut self) {
        let name = String::from_utf8_lossy(&self.name).into_owned();
        let e = Entry { name: name.clone(), score: self.score, lines: self.lines, level: self.level };
        self.placed = place(&mut self.board, e.clone());
        self.high = self.high.max(self.score);
        if funkey::page::web() {
            funkey::page::send(&format!("score {} {} {} {}", e.name, e.score, e.lines, e.level));
        } else if !cfg!(test) {
            funkey::store::save(GAME, "scores", &board_text(&self.board));
            funkey::store::save(GAME, "name", &name);
        }
        self.audio.play(&self.s.level, 0.8);
        self.mode = Mode::Over(1.0);
    }

    /// Where the game, playing itself, puts the piece: the turn and column
    /// that leave the well lowest and flattest, with full rows and few
    /// holes. The weights are Yiyuan Lee's, from his Tetris AI.
    fn best_move(&self) -> (usize, i32) {
        let p = self.cur;
        let mut best = (f32::MIN, p.rot, p.x);
        for r in 0..if p.kind == 1 { 1 } else { 4 } {
            for x in -2..COLS {
                if !self.fits(p.kind, r, x, p.y) { continue; }
                let mut y = p.y;
                while self.fits(p.kind, r, x, y + 1) { y += 1; }
                let mut g = self.grid;
                for (cx, cy) in cells(p.kind, r) { g[(y + cy) as usize][(x + cx) as usize] = 1; }
                let score = judge(&g);
                if score > best.0 { best = (score, r, x); }
            }
        }
        (best.1, best.2)
    }

    /// One key from the game playing itself: turn, then slide, then drop.
    fn demo_step(&mut self) {
        let (r, x) = match self.plan {
            Some(m) => m,
            None => { let m = self.best_move(); self.plan = Some(m); m }
        };
        if self.cur.rot != r {
            if !self.turn(1) { self.hard_drop(); }
        } else if self.cur.x != x {
            if !self.shift((x - self.cur.x).signum()) { self.hard_drop(); }
        } else {
            self.hard_drop();
        }
    }

    // -------------------------------------------------------------- a tick

    /// Left and right: one step on the press, then after a moment a slide.
    fn sideways(&mut self, input: &Input, dt: f32) {
        for (key, dir) in [(Key::Left, -1), (Key::Right, 1)] {
            if input.pressed(key) {
                self.das_dir = dir;
                self.das_t = 0.0;
                self.arr_t = ARR;
                self.shift(dir);
            }
        }
        let still = match self.das_dir { -1 => input.motion(Key::Left), 1 => input.motion(Key::Right), _ => false };
        if !still {
            self.das_dir = if input.motion(Key::Left) { -1 } else if input.motion(Key::Right) { 1 } else { 0 };
            self.das_t = 0.0;
            self.arr_t = ARR;
            return;
        }
        self.das_t += dt;
        if self.das_t < DAS { return; }
        self.arr_t += dt;
        while self.arr_t >= ARR {
            self.arr_t -= ARR;
            if !self.shift(self.das_dir) { self.arr_t = 0.0; break; }
        }
    }

    fn gravity(&mut self, input: &Input, dt: f32) {
        let soft = input.motion(Key::Down);
        let rate = 1.0 / fall_time(self.level);
        let rate = if soft { (rate * 20.0).max(20.0) } else { rate };
        self.fall += dt * rate;
        let mut n = 0;
        while self.fall >= 1.0 && n < ROWS {
            self.fall -= 1.0;
            n += 1;
            if self.down() {
                if soft { self.score += 1; }
            } else {
                self.fall = 0.0;
                break;
            }
        }
        if self.fits_at(0, 1) { return; }
        self.lock_t += dt;
        if self.lock_t >= LOCK { self.lock(); }
    }

    fn play(&mut self, input: &Input, dt: f32) {
        let pressed = |c: char| input.pressed(Key::Char(c));
        if !self.clearing.is_empty() {
            self.clear_t -= dt;
            if self.clear_t <= 0.0 { self.finish_clear(); }
            return;
        }
        if self.demo {
            self.demo_t -= dt;
            if self.demo_t <= 0.0 {
                self.demo_t = 0.06;
                self.demo_step();
            }
            if self.mode == Mode::Play && self.clearing.is_empty() { self.gravity(&Input::new(), dt); }
            return;
        }
        if pressed('c') { self.hold_piece(); }
        if input.pressed(Key::Up) || pressed('x') { self.turn(1); }
        if pressed('z') { self.turn(-1); }
        self.sideways(input, dt);
        if input.pressed(Key::Space) { self.hard_drop(); return; }
        if input.pressed(Key::Down) && self.down() { self.score += 1; self.fall = 0.0; }
        self.gravity(input, dt);
    }
}

impl Game for Stack {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        self.time += dt;
        self.particles.update(dt);
        for p in &mut self.popups { p.age += dt; }
        self.popups.retain(|p| p.age < 1.3);
        for t in &mut self.trails { t.4 += dt; }
        self.trails.retain(|t| t.4 < 0.25);
        self.shake = (self.shake - dt).max(0.0);
        let pressed = |c: char| input.pressed(Key::Char(c));
        let quit = input.pressed(Key::Escape) || pressed('q');
        while let Some(m) = funkey::page::recv() { self.heard(&m); }
        match self.mode {
            Mode::Title => {
                if quit { return Flow::Quit; }
                self.idle = if input.any_pressed() { 0.0 } else { self.idle + dt };
                if self.idle > 20.0 {
                    self.start();
                    self.demo = true;
                    return Flow::Continue;
                }
                if input.pressed(Key::Left) { self.start_level = (self.start_level + 13) % 15 + 1; }
                if input.pressed(Key::Right) { self.start_level = self.start_level % 15 + 1; }
                // The sky shows the level's colour before the game starts.
                self.level = self.start_level;
                if input.pressed(Key::Enter) || input.pressed(Key::Space) { self.start(); }
                for r in &mut self.rain {
                    r.3 += r.4 * dt;
                    if r.3 > H as f32 + 10.0 { r.3 = -50.0; r.1 = (r.1 + 1) % 4; }
                }
            }
            Mode::Play if self.demo => {
                if input.any_pressed() { self.to_title(); return Flow::Continue; }
                self.play(input, dt);
            }
            Mode::Play => {
                if quit { self.game_over(); return Flow::Continue; }
                if pressed('p') { self.mode = Mode::Paused; self.audio.volume(1, 0.15); return Flow::Continue; }
                self.play(input, dt);
                let danger = self.top() < HIDDEN + 6;
                if danger != self.danger && matches!(self.mode, Mode::Play) {
                    self.danger = danger;
                    self.audio.play_loop(1, if danger { &self.s.rush } else { &self.s.calm }, 0.55);
                }
            }
            Mode::Paused => {
                if quit { self.game_over(); return Flow::Continue; }
                if pressed('p') || input.pressed(Key::Enter) || input.pressed(Key::Space) {
                    self.mode = Mode::Play;
                    self.audio.volume(1, 0.55);
                }
            }
            Mode::Over(t) => {
                self.mode = Mode::Over(t + dt);
                // Once the curtain is down, a top-ten score asks for initials.
                if t > 0.9 && !self.asked && self.top_ten() {
                    self.asked = true;
                    self.name_at = 0;
                    self.name_t = 0.0;
                    self.mode = Mode::Name;
                } else if (t > 2.0 && input.any_pressed()) || (self.demo && t > 3.0) {
                    self.to_title();
                }
            }
            Mode::Name => {
                self.name_t += dt;
                if self.name_t > 0.4 { self.name_entry(input); }
            }
        }
        if self.level != self.backdrop_level {
            self.backdrop = backdrop(self.level);
            self.backdrop_level = self.level;
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        f.px.copy_from_slice(&self.backdrop);
        self.draw_stars(f);
        if self.mode == Mode::Title { self.draw_title(f); return; }
        self.draw_well(f);
        self.draw_left(f);
        self.draw_right(f);
        self.particles.draw(f, 0, 0);
        self.draw_popups(f);
        if self.demo {
            let k = 0.5 + 0.5 * (self.time * 3.0).sin();
            f.text_centered(W / 2, 4, "DEMO  -  ANY KEY TO PLAY", mix(DIM, GOLD, k), false, 1);
        }
        match self.mode {
            Mode::Paused => {
                let (x, y, w, h) = (FX - 2, FY - 2, COLS * CELL + 4, (ROWS - HIDDEN) * CELL + 4);
                f.rect(x, y, w, h, 0x07080f);
                f.text_centered(W / 2, 100, "PAUSED", GOLD, true, 2);
                f.text_centered(W / 2, 124, "P GOES ON", TEXT, false, 1);
                f.text_centered(W / 2, 134, "Q ENDS THE GAME", DIM, false, 1);
            }
            Mode::Over(t) if t > 0.9 => self.draw_scores(f, t),
            Mode::Name => self.draw_name(f),
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ drawing

impl Stack {
    fn draw_stars(&self, f: &mut Frame) {
        for i in 0..70 {
            let x = (hash(i, 1, 7) * W as f32) as i32;
            let y = (hash(i, 2, 7) * H as f32) as i32;
            let tw = 0.5 + 0.5 * (self.time * (0.6 + hash(i, 3, 7) * 2.0) + i as f32).sin();
            let c = mix(f.get(x, y), 0xdce4ff, 0.25 + 0.6 * tw * hash(i, 4, 7));
            f.put(x, y, c);
        }
    }

    fn draw_well(&self, f: &mut Frame) {
        let (sx, sy) = if self.shake > 0.0 {
            let k = self.shake * 10.0;
            (((self.time * 91.0).sin() * k) as i32, ((self.time * 77.0).cos() * k) as i32)
        } else {
            (0, 0)
        };
        let (ox, oy) = (FX + sx, FY + sy);
        let (w, h) = (COLS * CELL, (ROWS - HIDDEN) * CELL);
        // The frame: dark, a lit edge, a glow outside.
        f.rect(ox - 5, oy - 5, w + 10, h + 10, 0x05060c);
        let edge = if self.danger { mix(0xff4050, 0x602030, 0.5 + 0.5 * (self.time * 6.0).sin()) } else { 0x5a6aa8 };
        for (d, k) in [(3, 1.0), (4, 0.45), (5, 0.2)] {
            let c = mix(0x05060c, edge, k);
            f.rect(ox - d, oy - d, w + 2 * d, 1, c);
            f.rect(ox - d, oy + h + d - 1, w + 2 * d, 1, c);
            f.rect(ox - d, oy - d, 1, h + 2 * d, c);
            f.rect(ox + w + d - 1, oy - d, 1, h + 2 * d, c);
        }
        for r in 0..ROWS - HIDDEN {
            for c in 0..COLS {
                f.rect(ox + c * CELL, oy + r * CELL, CELL, CELL, if (r + c) % 2 == 0 { 0x0b0d1a } else { 0x0d1020 });
            }
        }
        // Streaks from a hard drop.
        for &(x, top, bot, c, age) in &self.trails {
            let k = 1.0 - age / 0.25;
            for y in top.max(HIDDEN)..bot {
                let fade = k * (y - top + 1) as f32 / (bot - top + 1) as f32 * 0.6;
                let px = ox + x * CELL;
                let py = oy + (y - HIDDEN) * CELL;
                for yy in 0..CELL {
                    for xx in 2..CELL - 2 { let old = f.get(px + xx, py + yy); f.put(px + xx, py + yy, mix(old, c, fade)); }
                }
            }
        }
        // The blocks at rest, and the rows on their way out.
        let over = match self.mode {
            Mode::Over(t) => ((t * 34.0) as i32).min(ROWS - HIDDEN),
            Mode::Name => ROWS - HIDDEN,
            _ => 0,
        };
        let clear_p = if self.clear_len > 0.0 { 1.0 - self.clear_t / self.clear_len } else { 0.0 };
        for y in HIDDEN..ROWS {
            let px_y = oy + (y - HIDDEN) * CELL;
            let curtain = ROWS - y <= over;
            let leaving = self.clearing.contains(&y);
            for x in 0..COLS {
                let v = self.grid[y as usize][x as usize];
                if curtain { block(f, ox + x * CELL, px_y, CELL, COLORS[GREY as usize]); continue; }
                if v == 0 { continue; }
                let mut c = COLORS[v as usize];
                if leaving {
                    if clear_p < 0.35 {
                        c = mix(c, 0xffffff, 0.4 + 0.6 * (clear_p / 0.35));
                    } else {
                        let gone = (clear_p - 0.35) / 0.65 * 5.6;
                        if (x as f32 - 4.5).abs() < gone { continue; }
                        c = 0xffffff;
                    }
                }
                block(f, ox + x * CELL, px_y, CELL, c);
            }
        }
        if self.mode != Mode::Play || !self.clearing.is_empty() { return; }
        // The ghost where the piece would land, then the piece.
        let p = self.cur;
        let c = COLORS[p.kind + 1];
        let g = self.drop_distance();
        for (cx, cy) in cells(p.kind, p.rot) {
            let (x, y) = (p.x + cx, p.y + cy + g);
            if y < HIDDEN { continue; }
            let (px, py) = (ox + x * CELL, oy + (y - HIDDEN) * CELL);
            f.rect(px, py, CELL, CELL, mix(c, 0x0b0d1a, 0.55));
            f.rect(px + 1, py + 1, CELL - 2, CELL - 2, mix(c, 0x0b0d1a, 0.86));
        }
        let glow = if self.fits_at(0, 1) { 0.0 } else { 0.35 * (self.lock_t / LOCK).min(1.0) };
        for (cx, cy) in cells(p.kind, p.rot) {
            let (x, y) = (p.x + cx, p.y + cy);
            if y < HIDDEN { continue; }
            block(f, ox + x * CELL, oy + (y - HIDDEN) * CELL, CELL, mix(c, 0xffffff, glow));
        }
    }

    fn draw_left(&self, f: &mut Frame) {
        let x = 40;
        f.text_big(x, 16, "HOLD", GOLD);
        panel(f, x - 4, 26, 108, 44);
        if let Some(k) = self.hold {
            let c = if self.held { 0x505568 } else { COLORS[k + 1] };
            preview(f, k, x - 4 + 54, 48, 11, c);
        }
        let stat = |f: &mut Frame, y: i32, label: &str, value: String, big: bool| {
            f.text(x, y, label, DIM);
            f.text_scaled(x, y + 8, &value, TEXT, true, if big { 2 } else { 1 });
        };
        stat(f, 82, "SCORE", self.score.to_string(), true);
        stat(f, 108, "HIGH", self.high.max(self.score).to_string(), false);
        stat(f, 128, "LEVEL", self.level.to_string(), false);
        f.text(x + 50, 128, "LINES", DIM);
        f.text_big(x + 50, 136, &self.lines.to_string(), TEXT);
        // Rows to the next level.
        let done = (self.lines % 10) as i32;
        f.rect(x, 150, 100, 4, 0x1a1e30);
        f.rect(x, 150, done * 10, 4, 0x60d880);
        f.text(x, 158, &format!("{} ROWS TO LEVEL {}", 10 - done, self.level + 1), DIM);
        let rows = [
            ("TETRIS", self.fours.to_string()),
            ("T-SPIN", self.spins.to_string()),
            ("BEST COMBO", self.best_combo.max(0).to_string()),
        ];
        for (i, (l, v)) in rows.iter().enumerate() {
            let y = 178 + i as i32 * 11;
            f.text(x, y, l, DIM);
            f.text(x + 100 - Frame::text_width(v, false, 1), y, v, TEXT);
        }
        if self.b2b { f.text(x, 214, "BACK-TO-BACK READY", 0x80e0ff); }
        if self.combo > 0 { f.text(x, 224, &format!("COMBO {}", self.combo), 0xff9040); }
    }

    fn draw_right(&self, f: &mut Frame) {
        let x = FX + COLS * CELL + 24;
        f.text_big(x + 4, 16, "NEXT", GOLD);
        panel(f, x, 26, 104, 178);
        for (i, &k) in self.queue.iter().take(5).enumerate() {
            if i == 0 { preview(f, k, x + 52, 50, 12, COLORS[k + 1]); }
            else { preview(f, k, x + 52, 64 + i as i32 * 28, 8, COLORS[k + 1]); }
        }
        let keys = ["LEFT RIGHT  MOVE", "DOWN  SOFT DROP", "SPACE  HARD DROP", "UP X  TURN   Z  BACK", "C  HOLD   P  PAUSE"];
        for (i, k) in keys.iter().enumerate() { f.text(x, 212 + i as i32 * 9, k, DIM); }
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x404860);
    }

    fn draw_popups(&self, f: &mut Frame) {
        for (i, p) in self.popups.iter().enumerate() {
            let y = 96 - (p.age * 26.0) as i32 + i as i32 * 18;
            let k = if p.age > 0.9 { 1.0 - (p.age - 0.9) / 0.4 } else { 1.0 };
            let c = mix(0x0b0d1a, p.color, k.max(0.0));
            if p.big && p.age < 0.9 {
                fancy_text(f, W / 2, y, &p.text, 2, self.time);
            } else {
                let w = Frame::text_width(&p.text, true, 1);
                f.text_scaled(W / 2 - w / 2 + 1, y + 1, &p.text, 0x000000, true, 1);
                f.text_scaled(W / 2 - w / 2, y, &p.text, c, true, 1);
            }
        }
    }

    /// The top ten, a row each: rank, initials, score, level. The new
    /// score blinks.
    fn draw_board(&self, f: &mut Frame, x: i32, y: i32) {
        if self.board.is_empty() {
            f.text_centered(W / 2, y + 40, "NO SCORES YET", DIM, true, 1);
            return;
        }
        for (i, e) in self.board.iter().enumerate() {
            let yy = y + i as i32 * 12;
            let mine = self.placed == Some(i);
            let c = if mine { mix(GOLD, 0xffffff, 0.5 + 0.5 * (self.time * 8.0).sin()) } else { TEXT };
            f.text_big(x, yy, &format!("{:>2}", i + 1), DIM);
            f.text_big(x + 24, yy, &e.name, c);
            let sc = e.score.to_string();
            f.text_big(x + 138 - Frame::text_width(&sc, true, 1), yy, &sc, c);
            f.text(x + 148, yy + 2, &format!("LV {}", e.level), DIM);
        }
    }

    fn draw_scores(&self, f: &mut Frame, t: f32) {
        let (x, y, w, h) = (W / 2 - 112, 30, 224, 212);
        f.rect(x, y, w, h, 0x0a0b16);
        f.rect(x, y, w, 1, 0xe04848);
        f.rect(x, y + h - 1, w, 1, 0xe04848);
        f.text_centered(W / 2, y + 8, "GAME OVER", 0xff5a5a, true, 2);
        f.text_centered(W / 2, y + 28, &format!("SCORE {}   ROWS {}   LEVEL {}", self.score, self.lines, self.level), TEXT, false, 1);
        f.text_centered(W / 2, y + 42, "HIGH SCORES", GOLD, true, 1);
        self.draw_board(f, x + 22, y + 56);
        if t > 2.0 { f.text_centered(W / 2, y + h - 14, "ANY KEY", DIM, false, 1); }
    }

    fn draw_name(&self, f: &mut Frame) {
        let (x, y, w, h) = (W / 2 - 112, 30, 224, 212);
        f.rect(x, y, w, h, 0x0a0b16);
        f.rect(x, y, w, 1, GOLD);
        f.rect(x, y + h - 1, w, 1, GOLD);
        let rank = self.board.iter().position(|e| self.score > e.score).unwrap_or(self.board.len()) + 1;
        fancy_text(f, W / 2, y + 12, if rank == 1 { "NEW HIGH SCORE" } else { "TOP TEN" }, 2, self.time);
        f.text_centered(W / 2, y + 40, &format!("{}   RANK {}", self.score, rank), TEXT, true, 1);
        f.text_centered(W / 2, y + 60, "YOUR INITIALS", DIM, false, 1);
        for i in 0..3 {
            let bx = W / 2 - 49 + i as i32 * 34;
            let on = i == self.name_at;
            let edge = if on { mix(GOLD, 0xffffff, 0.5 + 0.5 * (self.time * 6.0).sin()) } else { 0x3a4260 };
            f.rect(bx, y + 72, 30, 34, edge);
            f.rect(bx + 2, y + 74, 26, 30, 0x07080f);
            let ch = (self.name[i] as char).to_string();
            f.text_scaled(bx + 8, y + 79, &ch, if on { GOLD } else { TEXT }, true, 3);
        }
        f.text_centered(W / 2, y + 122, "TYPE THEM, OR UP AND DOWN TURN A LETTER", DIM, false, 1);
        f.text_centered(W / 2, y + 132, "LEFT AND RIGHT MOVE    ENTER PUTS THEM ON THE LIST", DIM, false, 1);
        f.text_centered(W / 2, y + 142, "ESC SKIPS", DIM, false, 1);
        if funkey::page::web() { f.text_centered(W / 2, y + 166, "EVERYONE WHO PLAYS HERE SEES THIS LIST", 0x606880, false, 1); }
    }

    fn draw_title(&self, f: &mut Frame) {
        for &(k, rot, x, y, _) in &self.rain {
            for (cx, cy) in cells(k, rot) {
                let (px, py) = (x as i32 + cx * 10, y as i32 + cy * 10);
                let c = mix(COLORS[k + 1], f.get(px + 4, py + 4), 0.55);
                block(f, px, py, 10, c);
            }
        }
        f.rect(90, 30, W - 180, 206, 0x07080f);
        f.rect(90, 30, W - 180, 1, 0x5a6aa8);
        f.rect(90, 235, W - 180, 1, 0x5a6aa8);
        fancy_text(f, W / 2, 44, "STACK", 7, self.time);
        f.text_centered(W / 2, 100, "A TRIBUTE TO TETRIS", TEXT, true, 1);
        f.text_centered(W / 2, 124, &format!("START LEVEL  <  {}  >", self.start_level), GOLD, true, 1);
        f.text_centered(W / 2, 142, "ENTER STARTS   LEFT RIGHT LEVEL   Q QUITS", TEXT, false, 1);
        f.text_centered(W / 2, 156, &format!("HIGH SCORE {}", self.high), GOLD, true, 1);
        let keys = [
            "LEFT RIGHT MOVE    DOWN SOFT DROP    SPACE HARD DROP",
            "UP OR X TURNS RIGHT    Z TURNS LEFT    C HOLDS    P PAUSES",
            "FOUR ROWS AT ONCE, A T SPUN INTO ITS SLOT, CLEARS IN A ROW:",
            "THEY ALL SCORE MORE. EVERY TEN ROWS THE LEVEL GOES UP.",
        ];
        if !self.board.is_empty() && (self.time / 6.0) as i32 % 2 == 1 {
            f.text_centered(W / 2, 172, "HIGH SCORES", GOLD, false, 1);
            for (i, e) in self.board.iter().enumerate() {
                let (x, y) = (W / 2 - 104 + (i as i32 / 5) * 112, 184 + (i as i32 % 5) * 8);
                f.text(x, y, &format!("{:>2}  {}  {:>7}  LV {}", i + 1, e.name, e.score, e.level), DIM);
            }
        } else {
            for (i, k) in keys.iter().enumerate() { f.text_centered(W / 2, 178 + i as i32 * 10, k, DIM, false, 1); }
        }
        f.text_centered(W / 2, 222, "MUSIC: KOROBEINIKI, A RUSSIAN FOLK SONG. ALL ELSE NEW.", 0x606880, false, 1);
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x404860);
    }
}

/// Three capital letters.
fn valid_name(n: &str) -> bool {
    n.len() == 3 && n.bytes().all(|b| b.is_ascii_uppercase())
}

/// A list as text, a line per score, best first: "ABC 12345 40 5". The
/// same text goes to disk, to the page and to the score server.
fn board_text(b: &[Entry]) -> String {
    b.iter().map(|e| format!("{} {} {} {}\n", e.name, e.score, e.lines, e.level)).collect()
}

fn parse_board(t: &str) -> Vec<Entry> {
    let mut b: Vec<Entry> = t.lines().filter_map(|l| {
        let mut w = l.split_whitespace();
        let name = w.next()?.to_string();
        let e = Entry { score: w.next()?.parse().ok()?, lines: w.next()?.parse().ok()?, level: w.next()?.parse().ok()?, name };
        valid_name(&e.name).then_some(e)
    }).collect();
    b.sort_by(|a, c| c.score.cmp(&a.score));
    b.truncate(10);
    b
}

/// Put a score where it belongs on a list, below any equal one. None when
/// it misses the top ten.
fn place(b: &mut Vec<Entry>, e: Entry) -> Option<usize> {
    let i = b.iter().position(|x| e.score > x.score).unwrap_or(b.len());
    if i >= 10 { return None; }
    b.insert(i, e);
    b.truncate(10);
    Some(i)
}

/// How good a well is for the game playing itself: full rows count for
/// it; height, holes and a ragged top count against.
fn judge(g: &[[u8; COLS as usize]; ROWS as usize]) -> f32 {
    let full = g.iter().filter(|r| r.iter().all(|&c| c != 0)).count();
    let rest: Vec<&[u8; COLS as usize]> = g.iter().filter(|r| !r.iter().all(|&c| c != 0)).collect();
    let n = rest.len();
    let mut heights = [0i32; COLS as usize];
    let mut holes = 0;
    for x in 0..COLS as usize {
        let mut seen = false;
        for (y, row) in rest.iter().enumerate() {
            if row[x] != 0 {
                if !seen { heights[x] = (n - y) as i32; seen = true; }
            } else if seen {
                holes += 1;
            }
        }
    }
    let height: i32 = heights.iter().sum();
    let bumps: i32 = heights.windows(2).map(|w| (w[0] - w[1]).abs()).sum();
    -0.510066 * height as f32 + 0.760666 * full as f32 - 0.35663 * holes as f32 - 0.184483 * bumps as f32
}

/// A block with a lit top and left edge, a shaded bottom and right, and a
/// gleam in the corner.
fn block(f: &mut Frame, x: i32, y: i32, s: i32, c: Rgb) {
    f.rect(x, y, s, s, mix(c, 0x000000, 0.5));
    f.rect(x, y, s - 1, s - 1, mix(c, 0xffffff, 0.45));
    f.rect(x + 1, y + 1, s - 2, s - 2, c);
    f.rect(x + 1, y + s - 3, s - 2, 1, mix(c, 0x000000, 0.18));
    if s >= 8 {
        let g = mix(c, 0xffffff, 0.75);
        f.rect(x + 2, y + 2, s / 3, 1, g);
        f.rect(x + 2, y + 2, 1, s / 3, g);
    }
}

/// A piece centred on a point, in its first position.
fn preview(f: &mut Frame, kind: usize, cx: i32, cy: i32, s: i32, c: Rgb) {
    let cs = cells(kind, 0);
    let (x0, x1) = (cs.iter().map(|p| p.0).min().unwrap(), cs.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (cs.iter().map(|p| p.1).min().unwrap(), cs.iter().map(|p| p.1).max().unwrap());
    let (w, h) = ((x1 - x0 + 1) * s, (y1 - y0 + 1) * s);
    for (x, y) in cs { block(f, cx - w / 2 + (x - x0) * s, cy - h / 2 + (y - y0) * s, s, c); }
}

fn panel(f: &mut Frame, x: i32, y: i32, w: i32, h: i32) {
    f.rect(x, y, w, h, 0x07080f);
    f.rect(x, y, w, 1, 0x2a3252);
    f.rect(x, y + h - 1, w, 1, 0x2a3252);
    f.rect(x, y, 1, h, 0x2a3252);
    f.rect(x + w - 1, y, 1, h, 0x2a3252);
}

/// The sky behind the well, a new colour every level.
fn backdrop(level: u32) -> Vec<Rgb> {
    const SKIES: [(Rgb, Rgb); 8] = [
        (0x0a0f2e, 0x2a0e3c), (0x061c2c, 0x0a3a38), (0x1c0a2c, 0x3c0c22), (0x0c0c14, 0x1c2a48),
        (0x10240a, 0x2c3a0c), (0x2a1606, 0x3a0c0c), (0x061430, 0x04303c), (0x240a26, 0x0a0a3c),
    ];
    let (top, bot) = SKIES[(level as usize - 1) % SKIES.len()];
    let mut px = Vec::with_capacity((W * H) as usize);
    for y in 0..H {
        for x in 0..W {
            let t = y as f32 / H as f32;
            let mut c = mix(top, bot, t);
            // Two slow bands of light across the sky, and a dark rim.
            let band = ((x as f32 * 0.012 + y as f32 * 0.02).sin() * 0.5 + 0.5).powf(6.0) * 0.12;
            c = mix(c, 0x9fb0ff, band);
            let (dx, dy) = (x as f32 / W as f32 - 0.5, y as f32 / H as f32 - 0.5);
            c = mix(c, 0x000000, ((dx * dx + dy * dy) * 1.6).min(0.6));
            if hash(x, y, level) > 0.5 { c = mix(c, 0x000000, 0.04); }
            px.push(c);
        }
    }
    px
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = parts(a);
    let (br, bg, bb) = parts(b);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    rgb(m(ar, br), m(ag, bg), m(ab, bb))
}

/// Big letters with a shadow, a warm gradient and a gleam that sweeps across.
fn fancy_text(f: &mut Frame, cx: i32, y: i32, s: &str, scale: i32, time: f32) {
    let (w, h) = (Frame::text_width(s, true, scale), 7 * scale);
    let mut m = Frame::new(w + 1, h + 1);
    m.text_scaled(0, 0, s, WHITE, true, scale);
    let x0 = cx - w / 2;
    let lit = |xx: i32, yy: i32| m.get(xx, yy) != BLACK;
    for yy in 0..h {
        for xx in 0..w {
            if lit(xx, yy) { f.put(x0 + xx + scale / 2 + 1, y + yy + scale / 2 + 1, 0x000000); }
        }
    }
    let band = ((time * 0.5).fract() * (w + h) as f32 * 1.6) as i32 - h;
    for yy in 0..h {
        for xx in 0..w {
            if !lit(xx, yy) { continue; }
            let mut c = mix(0xfff0a0, 0xe03a50, yy as f32 / h as f32);
            let d = (xx + yy - band).abs();
            if d < scale * 2 { c = mix(c, WHITE, 1.0 - d as f32 / (scale * 2) as f32); }
            f.put(x0 + xx, y + yy, c);
        }
    }
}

/// The game with its sound on and the title tune playing, or playing
/// itself when `STACK_DEMO` is set.
fn stack() -> Stack {
    let mut game = Stack::new();
    // In a web page: ask for the shared list and the last initials.
    funkey::page::send("list");
    game.audio = Audio::open();
    game.audio.play_loop(1, &game.s.title, 0.5);
    if std::env::var_os("STACK_DEMO").is_some() {
        game.start();
        game.demo = true;
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    run(&mut stack(), Config { width: W, height: H, fps: 60 });
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(stack(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Stack {
        let mut g = Stack::new();
        g.rng = Rng::new(5);
        g
    }

    #[test]
    fn the_bag_deals_every_piece_once_in_each_seven() {
        let mut g = quiet();
        for _ in 0..20 {
            let mut seen: Vec<usize> = (0..7).map(|_| g.next_piece()).collect();
            seen.sort();
            assert_eq!(seen, vec![0, 1, 2, 3, 4, 5, 6]);
        }
    }

    #[test]
    fn four_turns_bring_every_piece_back() {
        for k in 0..7 {
            for r in 0..4 {
                let c = cells(k, r);
                let mut s = c.to_vec();
                s.sort();
                s.dedup();
                assert_eq!(s.len(), 4, "piece {} turned {} has four blocks", k, r);
            }
            assert_eq!(cells(k, 4), cells(k, 0));
        }
    }

    #[test]
    fn a_turn_against_the_wall_kicks_off_it() {
        let mut g = quiet();
        g.start();
        // An upright I flat against the right wall turns flat by moving in.
        g.cur = Piece { kind: 0, rot: 1, x: 7, y: 10 };
        assert!(g.fits(0, 1, 7, 10));
        assert!(!g.fits(0, 2, 7, 10), "no room to turn in place");
        assert!(g.turn(1));
        assert!(g.cur.x < 7);
        // A T at the left wall pointing right turns up by stepping right.
        g.cur = Piece { kind: T, rot: 1, x: -1, y: 10 };
        assert!(g.fits(T, 1, -1, 10));
        assert!(!g.fits(T, 0, -1, 10), "no room to turn in place");
        assert!(g.turn(-1));
        assert_eq!(g.cur.rot, 0);
        assert!(g.cur.x >= 0);
    }

    #[test]
    fn four_rows_at_once_score_eight_hundred_a_level() {
        let mut g = quiet();
        g.start();
        for y in ROWS - 4..ROWS { for x in 0..COLS - 1 { g.grid[y as usize][x as usize] = 8; } }
        g.cur = Piece { kind: 0, rot: 1, x: 7, y: ROWS - 4 };
        g.score = 0;
        g.lock();
        assert_eq!(g.score, 800);
        assert_eq!(g.clearing.len(), 4);
        g.finish_clear();
        assert_eq!(g.lines, 4);
        assert_eq!(g.score, 800 + 2000, "and the empty well is an all clear");
    }

    #[test]
    fn a_t_spun_into_its_slot_scores_a_t_spin_double() {
        let mut g = quiet();
        g.start();
        let (b, m) = (ROWS - 1, ROWS - 2);
        for x in 0..COLS {
            if x != 4 { g.grid[b as usize][x as usize] = 8; }
            if !(3..=5).contains(&x) { g.grid[m as usize][x as usize] = 8; }
        }
        // The overhang that makes it a spin.
        g.grid[(m - 1) as usize][3] = 8;
        g.cur = Piece { kind: T, rot: 2, x: 3, y: m - 1 };
        g.last_turn = true;
        g.last_kick = 0;
        assert_eq!(g.tspin(), Some(true));
        g.score = 0;
        g.lock();
        assert_eq!(g.clearing.len(), 2);
        assert_eq!(g.score, 1200);
    }

    #[test]
    fn pieces_fall_faster_every_level() {
        for l in 1..20 { assert!(fall_time(l + 1) < fall_time(l)); }
        assert!((fall_time(1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_game_playing_itself_clears_rows_and_lasts() {
        let mut g = quiet();
        g.start();
        for _ in 0..400 {
            if g.mode != Mode::Play { break; }
            let (r, x) = g.best_move();
            while g.cur.rot != r { if !g.turn(1) { break; } }
            while g.cur.x != x { if !g.shift((x - g.cur.x).signum()) { break; } }
            g.hard_drop();
            if !g.clearing.is_empty() { g.finish_clear(); }
        }
        assert_eq!(g.mode, Mode::Play, "400 pieces and still going");
        assert!(g.lines >= 120, "only {} rows", g.lines);
    }

    fn ten(g: &mut Stack) {
        g.board = (1..=10).rev().map(|i| Entry { name: "AAA".into(), score: i * 100, lines: 1, level: 1 }).collect();
    }

    fn tick(g: &mut Stack, k: Option<Key>, dt: f32) {
        let mut i = Input::new();
        if let Some(k) = k { i.inject(k); }
        g.update(&i, dt);
    }

    #[test]
    fn a_list_keeps_ten_best_first_and_reads_back_as_written() {
        let mut b = Vec::new();
        for (i, s) in [300, 900, 300, 50].iter().enumerate() {
            place(&mut b, Entry { name: format!("A{}C", (b'A' + i as u8) as char), score: *s, lines: 1, level: 1 });
        }
        let scores: Vec<u32> = b.iter().map(|e| e.score).collect();
        assert_eq!(scores, vec![900, 300, 300, 50]);
        assert_eq!(b[1].name, "AAC", "the older of two equal scores stays above");
        assert_eq!(parse_board(&board_text(&b)), b);
        assert_eq!(parse_board("abc 1 1 1\nXYZ nine 1 1\nOK 5 1 1\n"), Vec::new(), "bad lines are skipped");
        let mut g = quiet();
        ten(&mut g);
        assert_eq!(place(&mut g.board, Entry { name: "LOW".into(), score: 100, lines: 1, level: 1 }), None);
    }

    #[test]
    fn a_top_ten_game_asks_for_initials_and_lands_on_the_list() {
        let mut g = quiet();
        ten(&mut g);
        g.start();
        g.score = 550;
        g.game_over();
        tick(&mut g, None, 1.0);
        tick(&mut g, None, 0.1);
        assert_eq!(g.mode, Mode::Name);
        tick(&mut g, Some(Key::Space), 0.01);
        assert_eq!(g.mode, Mode::Name, "a drop still falling from the game is ignored");
        tick(&mut g, None, 0.5);
        for c in ['g', 'e', 'i'] { tick(&mut g, Some(Key::Char(c)), 0.01); }
        tick(&mut g, Some(Key::Enter), 0.01);
        assert!(matches!(g.mode, Mode::Over(_)));
        assert_eq!(g.placed, Some(5));
        assert_eq!(g.board[5].name, "GEI");
        assert_eq!(g.board.len(), 10);
        assert_eq!(g.board[9].score, 200, "the lowest fell off");
        // The page's copy of the list replaces ours, and the new line is found in it.
        g.heard("scores\nZZZ 9000 40 5\nGEI 550 0 1\n");
        assert_eq!(g.placed, Some(1));
        assert_eq!(g.high.max(9000), g.high);
    }

    #[test]
    fn a_score_below_the_list_asks_for_nothing() {
        let mut g = quiet();
        ten(&mut g);
        g.start();
        g.score = 50;
        g.game_over();
        tick(&mut g, None, 1.0);
        tick(&mut g, None, 0.1);
        assert!(matches!(g.mode, Mode::Over(_)));
    }

    #[test]
    fn a_careless_player_fills_the_well_without_a_crash() {
        let mut g = quiet();
        g.start();
        let mut rng = Rng::new(9);
        let mut pieces = 0;
        while g.mode == Mode::Play && pieces < 5000 {
            for _ in 0..rng.below(4) { g.turn(1); }
            let dx = rng.below(9) as i32 - 4;
            for _ in 0..dx.abs() { g.shift(dx.signum()); }
            if rng.chance(0.1) { g.hold_piece(); }
            g.hard_drop();
            if !g.clearing.is_empty() { g.finish_clear(); }
            pieces += 1;
        }
        assert!(matches!(g.mode, Mode::Over(_)), "the well fills in the end");
        let mut f = Frame::new(W, H);
        g.draw(&mut f);
    }
}
