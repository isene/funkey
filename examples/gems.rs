//! gems: a tribute to Crystal Castles (Atari, 1983). A bear walks the
//! walls of a castle seen from its corner and takes every gem before the
//! gem eaters do. Trees walk, a crystal ball rolls, bees swarm, and in
//! the third wave of each castle the witch comes. The hat keeps the bear
//! safe for a while. There is a warp, for those who know where to jump.
//!
//!     cargo run --release --example gems
//!
//! The arrows walk, two at once for the diagonals; Space jumps; Q quits.
//! `GEMS_START=<castle>,<wave>` starts elsewhere; `GEMS_BENCH=<frames>`
//! times the game with no terminal.
//! Everything here is new: the castles, the pictures and the music.

use funkey::*;
use std::collections::VecDeque;

const W: i32 = 512;
const H: i32 = 384;
const TOP: i32 = 32;
/// The castle is N by N blocks.
const N: i32 = 14;
/// Where the back corner of the ground sits on screen.
const OX: i32 = W / 2;
const OY: i32 = 116;
/// Half a block's width and half its depth, on screen.
const HALF_W: i32 = 16;
const HALF_H: i32 = 8;
/// Pixels a block rises for each step of height.
const HZ: i32 = 8;
/// How far the foundations reach below the ground, in steps.
const BASE: i32 = -2;
const GAME: &str = "gems";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.3";
const JUMP: f32 = 0.75;
/// How fast the whole game moves: the bear, the foes and the bees alike.
const PACE: f32 = 1.15;
const BEAR_SPEED: f32 = 3.2 * PACE;
const HAT_TIME: f32 = 8.0;

/// A castle: heights as digits, a space where there is nothing, and the
/// things in it by letter: B the bear, E gem eaters, T trees, O the
/// crystal ball, W the witch, V where the bees live, H the hat, P the
/// honey pot, * a warp, and - a block without a gem.
struct Castle { name: &'static str, rows: [&'static str; 14], objs: &'static [(char, i32, i32)] }

const CASTLES: [Castle; 5] = [
    Castle { name: "THE COURTYARD", rows: [
        "00000000000000",
        "0     00     0",
        "0 2222112222 0",
        "0 2        2 0",
        "0 2 444444 2 0",
        "0 2 4    4 2 0",
        "00123 88 32100",
        "00123 88 32100",
        "0 2 4    4 2 0",
        "0 2 444444 2 0",
        "0 2        2 0",
        "0 2222222222 0",
        "0            0",
        "00000000000000",
    ], objs: &[('B', 13, 13), ('*', 12, 13), ('E', 0, 0), ('E', 13, 0), ('T', 0, 13), ('O', 6, 0),
               ('W', 0, 6), ('V', 0, 0), ('H', 6, 4), ('P', 6, 9)] },
    Castle { name: "TWIN TOWERS", rows: [
        "00000000000000",
        "00000000000000",
        "00444400444400",
        "00444400444400",
        "00444444444400",
        "00444400444400",
        "00344400444300",
        "00200000000200",
        "00100000000100",
        "00000000000000",
        "000  0000  000",
        "000  0000  000",
        "00000000000000",
        "00000000000000",
    ], objs: &[('B', 6, 12), ('E', 0, 0), ('E', 13, 0), ('E', 6, 2), ('T', 0, 13), ('T', 13, 13), ('O', 7, 9),
               ('W', 0, 0), ('V', 6, 0), ('H', 3, 3), ('P', 10, 3)] },
    Castle { name: "THE MAZE", rows: [
        "00000000000000",
        "0 0   00   0 0",
        "0 0 0 00 0 0 0",
        "0 000 00 000 0",
        "0     00     0",
        "0000 0110 0000",
        "   0 2222 0   ",
        "   0 2222 0   ",
        "0000 0110 0000",
        "0     00     0",
        "0 000 00 000 0",
        "0 0 0 00 0 0 0",
        "0 0   00   0 0",
        "00000000000000",
    ], objs: &[('B', 6, 13), ('E', 0, 0), ('E', 13, 0), ('E', 6, 0), ('T', 0, 13), ('T', 13, 13), ('O', 6, 9),
               ('W', 0, 0), ('V', 6, 0), ('H', 6, 6), ('P', 0, 5)] },
    Castle { name: "HIGH BRIDGES", rows: [
        "66666666666666",
        "66666666666666",
        "6   6    6   6",
        "5   5    5   5",
        "4   4    4   4",
        "3   3    3   3",
        "2   2    2   2",
        "1   1    1   1",
        "00000000000000",
        "0  0  00  0  0",
        "0  0  00  0  0",
        "0000  00  0000",
        "0  00000000  0",
        "00000000000000",
    ], objs: &[('B', 6, 13), ('E', 0, 0), ('E', 13, 0), ('E', 6, 1), ('T', 0, 13), ('T', 13, 8), ('O', 6, 8),
               ('W', 13, 0), ('V', 6, 4), ('H', 6, 0), ('P', 13, 13)] },
    Castle { name: "THE KEEP", rows: [
        "00000000000000",
        "02222222222220",
        "02444444444420",
        "02466656666420",
        "02468888886420",
        "02468888886420",
        "02368888876420",
        "02468888886420",
        "02468888886420",
        "02468888886420",
        "02466666666420",
        "02444444444420",
        "02222212222220",
        "00000000000000",
    ], objs: &[('B', 6, 13), ('E', 0, 0), ('E', 13, 0), ('E', 0, 13), ('T', 13, 13), ('T', 0, 6), ('O', 6, 11),
               ('W', 13, 6), ('V', 6, 6), ('H', 7, 7), ('P', 6, 1)] },
];

/// The colours of a castle: the tops, the two walls we see, and the rims.
struct Look { top: Rgb, left: Rgb, right: Rgb, rim: Rgb }

const LOOKS: [Look; 5] = [
    Look { top: 0x6a5acd, left: 0x2c2470, right: 0x463a9e, rim: 0x9d90ff },
    Look { top: 0x3aa0a0, left: 0x145050, right: 0x207878, rim: 0x7fe0e0 },
    Look { top: 0xb05a8a, left: 0x4a1838, right: 0x7a2a5e, rim: 0xf0a0d0 },
    Look { top: 0xc8a040, left: 0x5a4010, right: 0x8a6420, rim: 0xffe090 },
    Look { top: 0x6090d0, left: 0x203a60, right: 0x305890, rim: 0xa0d0ff },
];

const GEM_COLORS: [Rgb; 4] = [0xffffff, 0xff90d0, 0x90ffff, 0xffff90];
const TEXT: Rgb = 0xf0f0f0;
const GOLD: Rgb = 0xffd040;

/// Heights by block, None where there is no block.
#[derive(Clone)]
struct Map { h: Vec<Option<i32>> }

impl Map {
    fn from_rows(rows: &[&str; 14]) -> Map {
        let mut h = vec![None; (N * N) as usize];
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate().take(N as usize) {
                h[y * N as usize + x] = c.to_digit(10).map(|d| d as i32);
            }
        }
        Map { h }
    }

    fn at(&self, x: i32, y: i32) -> Option<i32> {
        if x < 0 || y < 0 || x >= N || y >= N { None } else { self.h[(y * N + x) as usize] }
    }

    /// One step up or down is a stair; more is a wall.
    fn walk(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        matches!((self.at(a.0, a.1), self.at(b.0, b.1)), (Some(ha), Some(hb)) if (ha - hb).abs() <= 1)
    }

    fn neighbours(&self, c: (i32, i32)) -> impl Iterator<Item = (i32, i32)> + '_ {
        [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter()
            .map(move |(dx, dy)| (c.0 + dx, c.1 + dy))
            .filter(move |&n| self.walk(c, n))
    }

    /// Steps from the nearest source to every block; i32::MAX where none reaches.
    fn field(&self, sources: &[(i32, i32)]) -> Vec<i32> {
        let mut d = vec![i32::MAX; (N * N) as usize];
        let mut q = VecDeque::new();
        for &s in sources {
            if self.at(s.0, s.1).is_some() && d[(s.1 * N + s.0) as usize] != 0 {
                d[(s.1 * N + s.0) as usize] = 0;
                q.push_back(s);
            }
        }
        while let Some(c) = q.pop_front() {
            let dc = d[(c.1 * N + c.0) as usize];
            for n in self.neighbours(c) {
                let i = (n.1 * N + n.0) as usize;
                if d[i] == i32::MAX { d[i] = dc + 1; q.push_back(n); }
            }
        }
        d
    }
}

fn cell_of(x: f32, y: f32) -> (i32, i32) { (x.floor() as i32, y.floor() as i32) }

/// A point on the castle, in blocks and steps of height, on screen.
fn project(x: f32, y: f32, h: f32) -> (i32, i32) {
    (OX + ((x - y) * HALF_W as f32).round() as i32, OY + ((x + y) * HALF_H as f32).round() as i32 - (h * HZ as f32).round() as i32)
}

struct Gem { x: f32, y: f32, h: i32, alive: bool }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Eater, Tree, Ball, Witch }

/// Someone who walks the castle block by block.
struct Foe {
    kind: Kind, from: (i32, i32), to: (i32, i32), t: f32, speed: f32, wait: f32, eating: f32, start: (i32, i32), start_wait: f32,
    /// Jumped over: harmless until it has passed, even if the bear lands on it.
    jumped: bool,
}

impl Foe {
    fn new(kind: Kind, at: (i32, i32), speed: f32, wait: f32) -> Foe {
        Foe { kind, from: at, to: at, t: 0.0, speed, wait, eating: 0.0, start: at, start_wait: wait, jumped: false }
    }
    fn pos(&self) -> (f32, f32) {
        let (a, b) = (self.from, self.to);
        (a.0 as f32 + 0.5 + (b.0 - a.0) as f32 * self.t, a.1 as f32 + 0.5 + (b.1 - a.1) as f32 * self.t)
    }
    fn height(&self, map: &Map) -> f32 {
        let ha = map.at(self.from.0, self.from.1).unwrap_or(0) as f32;
        let hb = map.at(self.to.0, self.to.1).unwrap_or(0) as f32;
        ha + (hb - ha) * self.t
    }
    fn reset(&mut self) {
        self.from = self.start;
        self.to = self.start;
        self.t = 0.0;
        self.wait = self.start_wait;
        self.eating = 0.0;
        self.jumped = false;
    }
}

/// The bees: one swarm a wave, straight at the bear over walls and all.
struct Swarm { x: f32, y: f32, h: f32, life: f32 }

#[derive(Clone, Copy, PartialEq)]
enum Mode { Title, Play, Dying(f32), Clear(f32), Over(f32), Warp(f32) }

struct Gems {
    map: Map,
    castle: usize,
    wave: usize,
    round: u32,
    gems: Vec<Gem>,
    by_cell: Vec<Vec<usize>>,
    left: usize,
    foes: Vec<Foe>,
    bees: Option<Swarm>,
    bees_done: bool,
    hive: (f32, f32),
    start: (i32, i32),
    warp: Option<(i32, i32)>,
    hat: Option<(i32, i32)>,
    pot: Option<(i32, i32)>,
    hat_time: f32,
    bear: (f32, f32),
    bear_h: f32,
    jump: f32,
    face_left: bool,
    walking: f32,
    bear_field: Vec<i32>,
    bear_cell: (i32, i32),
    gem_field: Vec<i32>,
    gems_changed: bool,
    took_any: bool,
    last_bonus: u32,
    note: Option<(String, f32)>,
    score: u32,
    high: u32,
    lives: u32,
    next_life: u32,
    mode: Mode,
    time: f32,
    wave_time: f32,
    gem_step: usize,
    gem_clock: f32,
    rng: Rng,
    particles: Particles,
    audio: Audio,
    s_gem: Vec<Sample>, s_jump: Sample, s_die: Sample, s_hat: Sample, s_kill: Sample, s_clear: Sample,
    s_bees: Sample, s_warp: Sample, s_pot: Sample, s_title: Sample,
    bear_img: Vec<Sprite>, tree: Vec<Sprite>, eater: Vec<Sprite>, ball: Sprite, witch: Sprite, hat_img: Sprite, pot_img: Sprite,
}

impl Gems {
    fn new() -> Gems {
        // The bear, drawn at full size: an outline, fur in three shades, a muzzle.
        let bp = [('o', 0x3a1c08), ('b', 0x9a5a28), ('B', 0x6e3c18), ('l', 0xc88a4a), ('m', 0xe8c090),
                  ('k', 0x101010), ('w', 0xffffff), ('n', 0x201008), ('p', 0xe08080)];
        let head = [".oo.......oo.", "obpo.....opbo", "obbooooooobbo", ".obbbbbbbbbo.", "obbbbbbbbbbbo",
                    "obwkbbbbbwkbo", "obkkbmmmbkkbo", "obbbmmnmmbbbo", ".obbmmmmmbbo.", "..obbmmmbbo..",
                    ".obbbbbbbbbo.", "obbblllllbbbo", "obbblllllbbbo", ".obblllllbbo.", "..obbbbbbbo.."];
        let bear = |legs: [&'static str; 2]| -> Sprite {
            let rows: Vec<&str> = head.iter().copied().chain(legs).collect();
            Sprite::from_rows(&rows, &bp)
        };
        let bear_img = vec![
            bear(["..obbo.obbo..", ".oBBBo.oBBBo."]),
            bear([".obbo...obbo.", ".oBBo...oBBo."]),
            bear(["...oboobo....", "...oBBoBBo..."]),
        ];
        // The others are drawn small, then doubled, lit and outlined.
        let tp = [('g', 0x38a040), ('d', 0x1c6020), ('k', 0x101010), ('y', 0xffe040), ('t', 0x7a4a20)];
        let tree = vec![
            fancy(&["..ggg..", ".gdggg.", "ggggdgg", "gykgykg", ".ggggg.", "..ggg..", "...t...", "..ttt..", ".t.t.t.", "t.....t"], &tp),
            fancy(&["..ggg..", ".gggdg.", "ggdgggg", "gykgykg", ".ggggg.", "..ggg..", "...t...", "..ttt..", ".t.t.t.", ".t...t."], &tp),
        ];
        let ep = [('m', 0xc050e0), ('w', 0xffffff), ('k', 0x101010), ('r', 0xff4060)];
        let eater = vec![
            fancy(&["..mmm..", ".mwmwm.", ".mkmkm.", "mmmmmmm", "m.m.m.m"], &ep),
            fancy(&["..mmm..", ".mwmwm.", ".mkmkm.", "mmrrrmm", ".m.m.m."], &ep),
        ];
        let ball = crystal_ball();
        let witch = fancy(&["...k...", "..kkk..", ".kkkkk.", "kkkkkkk", "..gyg..", "..ggg..", ".ppppp.", "ppppppp", ".ppppp.", ".p...p.", "ooooooo"],
            &[('k', 0x202020), ('g', 0x60c040), ('y', 0xffff40), ('p', 0x7030a0), ('o', 0x9a6a30)]);
        let hat_img = fancy(&["..r..", ".rsr.", ".rrr.", "rrrrr"], &[('r', 0xa040e0), ('s', 0xffff80)]);
        let pot_img = fancy(&[".yyy.", "ooooo", "oyyyo", "ooooo", ".ooo."], &[('o', 0xd07820), ('y', 0xffd040)]);
        let s_gem = ["c6", "d6", "e6", "g6", "a6", "c7", "d7", "e7"].iter()
            .map(|n| Sample::tone(Wave::Square, audio::note_hz(n), 0.035, 0.22)).collect();
        let s_title = Tune::parse(
            "170 d5/8 f5/8 a5/8 d6/8 c6/8 a5/8 bb5/4 a5/8 g5/8 f5/4 e5/8 f5/8 g5/8 a5/8 e5/2 -/4 \
             d5/8 f5/8 a5/8 d6/8 e6/8 d6/8 c6/8 a5/8 bb5/8 g5/8 e5/8 c#5/8 d5/2 -/4",
            Wave::Triangle, 0.4).render();
        let mut g = Gems {
            map: Map::from_rows(&CASTLES[0].rows), castle: 0, wave: 0, round: 0,
            gems: Vec::new(), by_cell: Vec::new(), left: 0, foes: Vec::new(), bees: None, bees_done: false, hive: (0.0, 0.0),
            start: (0, 0), warp: None, hat: None, pot: None, hat_time: 0.0,
            bear: (0.5, 0.5), bear_h: 0.0, jump: 0.0, face_left: false, walking: 0.0,
            bear_field: Vec::new(), bear_cell: (-1, -1), gem_field: Vec::new(), gems_changed: true,
            took_any: false, last_bonus: 0, note: None,
            score: 0, high: funkey::store::high_score(GAME), lives: 4, next_life: 25000, mode: Mode::Title,
            time: 0.0, wave_time: 0.0, gem_step: 0, gem_clock: 0.0,
            rng: Rng::from_time(), particles: Particles::new(), audio: Audio::off(),
            s_gem,
            s_jump: Sample::sweep(Wave::Triangle, 330.0, 880.0, 0.14, 0.4),
            s_die: Sample::sweep(Wave::Saw, 600.0, 80.0, 0.8, 0.45).then(&Sample::noise(0.2, 0.3)),
            s_hat: Tune::parse("320 c5/16 e5/16 g5/16 c6/16 e6/16 g6/8", Wave::Square, 0.35).render(),
            s_kill: Sample::noise(0.06, 0.35).then(&Sample::sweep(Wave::Square, 900.0, 300.0, 0.12, 0.3)),
            s_clear: Tune::parse("220 g5/8 c6/8 e6/8 g6/4 e6/8 g6/2", Wave::Triangle, 0.5).render(),
            s_bees: Sample::loop_tone(Wave::Saw, 115.0, 0.5, 0.1),
            s_warp: Sample::sweep(Wave::Sine, 200.0, 2400.0, 1.2, 0.45),
            s_pot: Tune::parse("260 e6/16 g6/16 e7/8", Wave::Square, 0.35).render(),
            s_title,
            bear_img, tree, eater, ball, witch, hat_img, pot_img,
        };
        g.particles.gravity = 180.0;
        g.load(0, 0);
        let mut audio = Audio::open();
        audio.play_loop(1, &g.s_title, 1.0);
        g.audio = audio;
        g
    }

    /// Set up a wave: fresh gems on every block the bear can reach, and
    /// the foes this wave brings.
    fn load(&mut self, castle: usize, wave: usize) {
        self.castle = castle % CASTLES.len();
        self.wave = wave;
        let c = &CASTLES[self.castle];
        self.map = Map::from_rows(&c.rows);
        let objs = |ch: char| -> Vec<(i32, i32)> { c.objs.iter().filter(|o| o.0 == ch).map(|o| (o.1, o.2)).collect() };
        self.start = objs('B').first().copied().unwrap_or((0, 0));
        self.warp = if self.castle == 0 && wave == 0 && self.round == 0 { objs('*').first().copied() } else { None };
        let bare: Vec<(i32, i32)> = objs('B').into_iter().chain(objs('*')).chain(objs('-')).collect();
        let reach = self.map.field(&[self.start]);
        let ok = |x: i32, y: i32| x >= 0 && y >= 0 && x < N && y < N && reach[(y * N + x) as usize] != i32::MAX && !bare.contains(&(x, y));
        // Every block gets a gem, and a narrow path one between every two
        // blocks as well; on open ground that would take all day.
        let path = |x: i32, y: i32| self.map.neighbours((x, y)).count() <= 2;
        self.gems.clear();
        for y in 0..N {
            for x in 0..N {
                if !ok(x, y) { continue; }
                let h = self.map.at(x, y).unwrap_or(0);
                self.gems.push(Gem { x: x as f32 + 0.5, y: y as f32 + 0.5, h, alive: true });
                if ok(x + 1, y) && self.map.at(x + 1, y) == Some(h) && (path(x, y) || path(x + 1, y)) { self.gems.push(Gem { x: x as f32 + 1.0, y: y as f32 + 0.5, h, alive: true }); }
                if ok(x, y + 1) && self.map.at(x, y + 1) == Some(h) && (path(x, y) || path(x, y + 1)) { self.gems.push(Gem { x: x as f32 + 0.5, y: y as f32 + 1.0, h, alive: true }); }
            }
        }
        self.by_cell = vec![Vec::new(); (N * N) as usize];
        for (i, g) in self.gems.iter().enumerate() {
            let (x, y) = cell_of(g.x, g.y);
            self.by_cell[(y * N + x) as usize].push(i);
        }
        self.left = self.gems.len();
        self.gems_changed = true;
        // Faster with every castle, every wave and every time round.
        let pace = PACE * (1.0 + 0.06 * self.castle as f32 + 0.05 * wave as f32 + 0.15 * self.round as f32);
        self.foes.clear();
        for (i, at) in objs('E').into_iter().enumerate() { self.foes.push(Foe::new(Kind::Eater, at, 1.3 * pace, 1.0 + i as f32)); }
        for at in objs('T') { self.foes.push(Foe::new(Kind::Tree, at, 1.25 * pace, 3.0)); }
        if wave >= 1 { for at in objs('O') { self.foes.push(Foe::new(Kind::Ball, at, 1.6 * pace, 2.0)); } }
        if wave >= 2 { for at in objs('W') { self.foes.push(Foe::new(Kind::Witch, at, 2.35 * pace, 4.0)); } }
        self.hive = objs('V').first().map(|&(x, y)| (x as f32 + 0.5, y as f32 + 0.5)).unwrap_or((0.5, 0.5));
        self.hat = objs('H').first().copied();
        self.pot = if wave == 1 { objs('P').first().copied() } else { None };
        self.took_any = false;
        self.wave_time = 0.0;
        self.respawn();
    }

    fn respawn(&mut self) {
        self.bear = (self.start.0 as f32 + 0.5, self.start.1 as f32 + 0.5);
        self.bear_h = self.map.at(self.start.0, self.start.1).unwrap_or(0) as f32;
        self.jump = 0.0;
        self.hat_time = 0.0;
        self.bear_cell = (-1, -1);
        for f in &mut self.foes { f.reset(); }
        self.bees = None;
        self.bees_done = false;
        self.audio.stop(2);
        self.wave_time = self.wave_time.min(8.0);
    }

    fn start_game(&mut self) {
        self.score = 0;
        self.lives = 4;
        self.next_life = 25000;
        self.round = 0;
        // GEMS_START=<castle>[,<wave>], counted from 1, for screenshots and tests.
        let from = std::env::var("GEMS_START").unwrap_or_default();
        let mut at = from.split(',').map(|n| n.trim().parse::<usize>().unwrap_or(1).max(1) - 1);
        let (castle, wave) = (at.next().unwrap_or(0), at.next().unwrap_or(0).min(2));
        self.load(castle, wave);
        self.mode = Mode::Play;
        self.audio.stop(1);
    }

    fn add(&mut self, points: u32) {
        self.score += points;
        if self.score >= self.next_life {
            self.next_life += 25000;
            self.lives += 1;
            self.say("EXTRA BEAR");
        }
    }

    fn say(&mut self, s: &str) { self.note = Some((s.to_string(), 1.8)); }

    fn cell_h(&self, c: (i32, i32)) -> i32 { self.map.at(c.0, c.1).unwrap_or(0) }

    fn die(&mut self) {
        if self.mode != Mode::Play { return; }
        self.mode = Mode::Dying(1.6);
        let (sx, sy) = project(self.bear.0, self.bear.1, self.bear_h);
        self.particles.burst(sx as f32, sy as f32 - 8.0, 40, 120.0, 1.0, 0x9a5a28);
        self.particles.burst(sx as f32, sy as f32 - 8.0, 16, 80.0, 1.0, 0xffffff);
        self.audio.stop(2);
        self.audio.play(&self.s_die, 1.0);
    }

    fn kill_foe(&mut self, i: usize) {
        let f = self.foes.remove(i);
        let (x, y) = f.pos();
        let (sx, sy) = project(x, y, f.height(&self.map));
        let (points, color) = match f.kind { Kind::Eater => (500, 0xc050e0), Kind::Tree => (1000, 0x38a040), Kind::Ball => (1500, 0x80e0ff), Kind::Witch => (3000, 0x7030a0) };
        self.add(points);
        self.particles.burst(sx as f32, sy as f32 - 8.0, 30, 100.0, 0.7, color);
        self.say(&format!("{points}"));
        self.audio.play(&self.s_kill, 1.0);
    }

    fn take_gem(&mut self, i: usize, by_bear: bool) {
        self.gems[i].alive = false;
        self.left -= 1;
        self.gems_changed = true;
        if by_bear {
            let g = &self.gems[i];
            let (sx, sy) = project(g.x, g.y, g.h as f32);
            let c = GEM_COLORS[i % GEM_COLORS.len()];
            self.particles.burst(sx as f32, sy as f32 - 2.0, 4, 40.0, 0.3, c);
            self.took_any = true;
            self.add(10);
            self.gem_step = if self.gem_clock > 0.0 { (self.gem_step + 1) % self.s_gem.len() } else { 0 };
            self.gem_clock = 0.3;
            let s = self.s_gem[self.gem_step].clone();
            self.audio.play_on(3, &s, 1.0);
        }
        if self.left == 0 {
            self.last_bonus = if by_bear { 1000 * (self.castle as u32 + 1) } else { 0 };
            if self.last_bonus > 0 { self.add(self.last_bonus); }
            self.mode = Mode::Clear(2.8);
            self.audio.stop(2);
            self.audio.play(&self.s_clear, 1.0);
        }
    }

    fn play(&mut self, input: &Input, dt: f32) {
        self.wave_time += dt;
        self.gem_clock -= dt;
        self.hat_time = (self.hat_time - dt).max(0.0);

        // The bear: the arrows point on screen, and the castle turns them.
        let (ax, ay) = (input.axis_x() as f32, input.axis_y() as f32);
        let (mut dx, mut dy) = (ax + ay, ay - ax);
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.0 { dx /= len; dy /= len; }
        let step = BEAR_SPEED * dt;
        // Where the block ahead cannot be entered, the bear walks to the
        // middle of his own instead: pushing into a wall slides him onto
        // the line of gems, as a trackball game keeps you on the path.
        let toward = |v: f32, to: f32| v + (to - v).clamp(-step, step);
        let here = cell_of(self.bear.0, self.bear.1);
        if dx != 0.0 {
            if self.map.walk(here, (here.0 + dx.signum() as i32, here.1)) { self.bear.0 += dx * step; }
            else { self.bear.0 = toward(self.bear.0, here.0 as f32 + 0.5); }
        }
        let here = cell_of(self.bear.0, self.bear.1);
        if dy != 0.0 {
            if self.map.walk(here, (here.0, here.1 + dy.signum() as i32)) { self.bear.1 += dy * step; }
            else { self.bear.1 = toward(self.bear.1, here.1 as f32 + 0.5); }
        }
        if ax != 0.0 { self.face_left = ax < 0.0; }
        self.walking = if len > 0.0 { self.walking + dt } else { 0.0 };
        let cell = cell_of(self.bear.0, self.bear.1);
        let h = self.cell_h(cell);
        self.bear_h += (h as f32 - self.bear_h) * (dt * 16.0).min(1.0);
        self.jump = (self.jump - dt).max(0.0);
        if input.pressed(Key::Space) && self.jump == 0.0 {
            self.jump = JUMP;
            self.audio.play(&self.s_jump, 0.8);
            if self.warp == Some(cell) && !self.took_any {
                self.mode = Mode::Warp(2.2);
                self.audio.play(&self.s_warp, 1.0);
                return;
            }
        }
        if cell != self.bear_cell {
            self.bear_cell = cell;
            self.bear_field = self.map.field(&[cell]);
        }

        // Gems under the bear's feet.
        let near: Vec<usize> = self.by_cell[(cell.1 * N + cell.0) as usize].iter().copied()
            .filter(|&i| { let g = &self.gems[i]; g.alive && g.h == h && (g.x - self.bear.0).abs() < 0.34 && (g.y - self.bear.1).abs() < 0.34 })
            .collect();
        for i in near { self.take_gem(i, true); if self.mode != Mode::Play { return; } }
        // The gems at the edge of the next block count too.
        for (ox, oy) in [(1, 0), (0, 1)] {
            let c = (cell.0 + ox, cell.1 + oy);
            if c.0 >= N || c.1 >= N { continue; }
            let near: Vec<usize> = self.by_cell[(c.1 * N + c.0) as usize].iter().copied()
                .filter(|&i| { let g = &self.gems[i]; g.alive && g.h == h && (g.x - self.bear.0).abs() < 0.34 && (g.y - self.bear.1).abs() < 0.34 })
                .collect();
            for i in near { self.take_gem(i, true); if self.mode != Mode::Play { return; } }
        }
        if self.hat == Some(cell) {
            self.hat = None;
            self.hat_time = HAT_TIME;
            self.say("THE HAT");
            self.audio.play(&self.s_hat, 1.0);
        }
        if self.pot == Some(cell) {
            self.pot = None;
            self.add(1000);
            self.say("HONEY 1000");
            let (sx, sy) = project(self.bear.0, self.bear.1, self.bear_h);
            self.particles.burst(sx as f32, sy as f32 - 12.0, 24, 80.0, 0.6, GOLD);
            self.audio.play(&self.s_pot, 1.0);
        }

        self.move_foes(dt);
        if self.mode != Mode::Play { return; }
        self.move_bees(dt);
        if self.mode != Mode::Play { return; }

        // Touching a foe. In the air nothing on the ground can touch the
        // bear, and what he jumped stays harmless until it has passed.
        let air = self.jump > 0.0;
        let mut i = 0;
        while i < self.foes.len() {
            let (fx, fy) = self.foes[i].pos();
            let close = self.foes[i].wait <= 0.0
                && (fx - self.bear.0).hypot(fy - self.bear.1) < 0.5
                && (self.foes[i].height(&self.map) - self.bear_h).abs() < 1.2;
            let f = &mut self.foes[i];
            if !close { f.jumped = false; i += 1; continue; }
            if self.hat_time > 0.0 || (f.kind == Kind::Eater && f.eating > 0.0) { self.kill_foe(i); continue; }
            if air { f.jumped = true; }
            if f.jumped { i += 1; continue; }
            self.die();
            return;
        }
    }

    fn move_foes(&mut self, dt: f32) {
        if self.gems_changed {
            let centres: Vec<(i32, i32)> = self.gems.iter().filter(|g| g.alive && g.x.fract() == 0.5 && g.y.fract() == 0.5).map(|g| cell_of(g.x, g.y)).collect();
            self.gem_field = self.map.field(&centres);
            self.gems_changed = false;
        }
        for k in 0..self.foes.len() {
            if self.foes[k].wait > 0.0 { self.foes[k].wait -= dt; continue; }
            if self.foes[k].eating > 0.0 {
                self.foes[k].eating -= dt;
                if self.foes[k].eating <= 0.0 {
                    let c = self.foes[k].from;
                    let at: Vec<usize> = self.by_cell[(c.1 * N + c.0) as usize].iter().copied()
                        .filter(|&i| self.gems[i].alive && self.gems[i].x.fract() == 0.5 && self.gems[i].y.fract() == 0.5).collect();
                    for i in at { self.take_gem(i, false); if self.mode != Mode::Play { return; } }
                }
                continue;
            }
            // Gems the eaters and the ball roll over on the way.
            if matches!(self.foes[k].kind, Kind::Eater | Kind::Ball) {
                let (x, y) = self.foes[k].pos();
                let c = cell_of(x, y);
                if c.0 >= 0 && c.1 >= 0 && c.0 < N && c.1 < N {
                    let at: Vec<usize> = self.by_cell[(c.1 * N + c.0) as usize].iter().copied()
                        .filter(|&i| { let g = &self.gems[i]; g.alive && (g.x.fract() != 0.5 || g.y.fract() != 0.5) && (g.x - x).abs() < 0.2 && (g.y - y).abs() < 0.2 })
                        .collect();
                    for i in at { self.take_gem(i, false); if self.mode != Mode::Play { return; } }
                }
            }
            let f = &mut self.foes[k];
            f.t += f.speed * dt;
            if f.t < 1.0 { continue; }
            f.from = f.to;
            f.t = 0.0;
            let here = f.from;
            if f.kind == Kind::Eater {
                let gem_here = self.by_cell[(here.1 * N + here.0) as usize].iter()
                    .any(|&i| self.gems[i].alive && self.gems[i].x.fract() == 0.5 && self.gems[i].y.fract() == 0.5);
                if gem_here { f.eating = 0.7 / PACE; continue; }
            }
            let field = match f.kind { Kind::Eater => &self.gem_field, _ => &self.bear_field };
            let wander = match f.kind { Kind::Ball => 0.3, Kind::Tree => 0.12, _ => 0.0 };
            let options: Vec<(i32, i32)> = self.map.neighbours(here).collect();
            let best = options.iter().copied().min_by_key(|n| field.get((n.1 * N + n.0) as usize).copied().unwrap_or(i32::MAX));
            let here_d = field.get((here.1 * N + here.0) as usize).copied().unwrap_or(i32::MAX);
            f.to = match best {
                Some(b) if !self.rng.chance(wander) && field[(b.1 * N + b.0) as usize] < here_d => b,
                _ => self.rng.pick(&options).copied().unwrap_or(here),
            };
        }
    }

    fn move_bees(&mut self, dt: f32) {
        if self.wave >= 1 && self.bees.is_none() && !self.bees_done && self.wave_time > 22.0 {
            self.bees = Some(Swarm { x: self.hive.0, y: self.hive.1, h: 12.0, life: 14.0 });
            self.audio.play_loop(2, &self.s_bees, 0.8);
        }
        let Some(b) = self.bees.as_mut() else { return };
        b.life -= dt;
        let pace = PACE * (1.1 + 0.1 * self.castle as f32);
        let (dx, dy) = (self.bear.0 - b.x, self.bear.1 - b.y);
        let d = dx.hypot(dy).max(0.001);
        if b.life > 1.5 {
            b.x += dx / d * pace * dt;
            b.y += dy / d * pace * dt;
            b.h += (self.bear_h + 2.0 - b.h) * (dt * 2.0).min(1.0);
        } else {
            b.h += 12.0 * dt;
        }
        if b.life <= 0.0 {
            self.bees = None;
            self.bees_done = true;
            self.audio.stop(2);
            return;
        }
        if d < 0.6 && b.life > 1.5 {
            if self.hat_time > 0.0 {
                let (sx, sy) = project(b.x, b.y, b.h);
                self.bees = None;
                self.bees_done = true;
                self.audio.stop(2);
                self.particles.burst(sx as f32, sy as f32, 30, 100.0, 0.7, GOLD);
                self.add(2000);
                self.say("2000");
                self.audio.play(&self.s_kill, 1.0);
            } else {
                self.die();
            }
        }
    }

    // ---------- drawing ----------

    /// Night falling behind the castle, with stars that twinkle.
    fn draw_sky(&self, f: &mut Frame) {
        for y in 0..H { f.hline(0, y, W, mix(0x04020c, 0x1c1240, y as f32 / H as f32)); }
        let mut r = Rng::new(7);
        for k in 0..90 {
            let (x, y) = (r.below(W as u32) as i32, r.below(H as u32) as i32);
            let glow = ((self.time * (0.7 + r.float() * 2.0) + k as f32).sin() * 0.5 + 0.5) * 0.8 + 0.2;
            let c = tint(if k % 7 == 0 { 0xc0d0ff } else { 0xffffff }, glow * 0.7);
            f.put(x, y, c);
            if k % 11 == 0 && glow > 0.85 {
                let d = tint(c, 0.5);
                f.put(x - 1, y, d); f.put(x + 1, y, d); f.put(x, y - 1, d); f.put(x, y + 1, d);
            }
        }
    }

    /// A block: a tiled top, and below it the walls we see, laid in brick
    /// courses and darker further down.
    fn draw_block(&self, f: &mut Frame, x: i32, y: i32, look: &Look, walls: &Walls) {
        let Some(h) = self.map.at(x, y) else { return };
        let cx = OX + (x - y) * HALF_W;
        let cy = OY + (x + y + 1) * HALF_H - h * HZ;
        let below = |nx: i32, ny: i32| -> i32 {
            let nh = self.map.at(nx, ny).unwrap_or(BASE);
            if nh < h { (h - nh) * HZ } else { 0 }
        };
        let (dl, dr) = (below(x, y + 1), below(x + 1, y));
        let (lit, seam) = (tint(look.top, 1.08), tint(look.top, 0.86));
        for i in 0..HALF_W * 2 {
            let d = i.min(HALF_W * 2 - 1 - i);
            let (top, bot) = (cy - (d / 2 + 1), cy + d / 2);
            let px = cx - HALF_W + i;
            f.vline(px, top, bot - top + 1, look.top);
            if d >= HALF_W / 2 {
                let e = d - HALF_W / 2;
                f.vline(px, cy - (e / 2 + 1), e / 2 * 2 + 2, lit);
            }
            f.put(px, bot, seam);
            let (depth, face, col) = if i < HALF_W { (dl, &walls.left, i) } else { (dr, &walls.right, i - HALF_W) };
            for r in 0..depth.min(WALL_MAX) {
                let joint = (col + if (r / 4) % 2 == 1 { 4 } else { 0 }) % 8 == 0;
                let (plain, mortar) = face[r as usize];
                f.put(px, bot + 1 + r, if r % 4 == 3 || joint { mortar } else { plain });
            }
        }
        // A rim where the block stands above the one behind it.
        let higher = |nx: i32, ny: i32| self.map.at(nx, ny).map(|nh| nh < h).unwrap_or(true);
        if higher(x - 1, y) { for i in 0..HALF_W { f.put(cx - HALF_W + i, cy - (i / 2 + 1), look.rim); } }
        if higher(x, y - 1) { for i in HALF_W..HALF_W * 2 { f.put(cx - HALF_W + i, cy - ((HALF_W * 2 - 1 - i) / 2 + 1), look.rim); } }
    }

    /// Where on the back-to-front sweep a thing standing at (x, y) is
    /// drawn: after its own block, or after the next row of blocks when
    /// those are no taller, so its feet stay on top of them.
    fn order(&self, x: f32, y: f32, h: f32) -> i32 {
        let (cx, cy) = cell_of(x, y);
        let low = |nx: i32, ny: i32| self.map.at(nx, ny).map(|nh| nh as f32 <= h + 0.01).unwrap_or(true);
        cx + cy + (low(cx + 1, cy) && low(cx, cy + 1)) as i32
    }

    fn draw_world(&mut self, f: &mut Frame) {
        let look = &LOOKS[self.castle % LOOKS.len()];
        let walls = Walls::new(look);
        // The things to draw between the blocks: (order, what, index).
        let mut things: Vec<(i32, u8, usize)> = Vec::new();
        let playing = !matches!(self.mode, Mode::Title);
        if playing && !matches!(self.mode, Mode::Dying(_)) {
            things.push((self.order(self.bear.0, self.bear.1, self.bear_h), 0, 0));
        }
        if playing {
            for (i, fo) in self.foes.iter().enumerate() {
                if fo.wait > 0.0 { continue; }
                let (x, y) = fo.pos();
                things.push((self.order(x, y, fo.height(&self.map)), 1, i));
            }
        }
        if let Some(c) = self.hat { things.push((self.order(c.0 as f32 + 0.5, c.1 as f32 + 0.5, self.cell_h(c) as f32), 2, 0)); }
        if let Some(c) = self.pot { things.push((self.order(c.0 as f32 + 0.5, c.1 as f32 + 0.5, self.cell_h(c) as f32), 3, 0)); }
        things.sort_by_key(|t| t.0);
        let mut next = 0;
        for s in 0..(2 * N - 1) {
            for x in (s - N + 1).max(0)..=s.min(N - 1) {
                let y = s - x;
                self.draw_block(f, x, y, look, &walls);
                for &i in &self.by_cell[(y * N + x) as usize] {
                    let g = &self.gems[i];
                    if !g.alive { continue; }
                    let (sx, sy) = project(g.x, g.y, g.h as f32);
                    let c = GEM_COLORS[(i + (self.time * 3.0) as usize / 4) % GEM_COLORS.len()];
                    f.put(sx, sy - 1, mix(c, WHITE, 0.6));
                    f.hline(sx - 1, sy, 3, c);
                    f.put(sx, sy + 1, tint(c, 0.6));
                    if (i * 7 + (self.time * 8.0) as usize) % 29 == 0 {
                        f.hline(sx - 2, sy, 5, WHITE);
                        f.vline(sx, sy - 2, 5, WHITE);
                    }
                }
            }
            while next < things.len() && things[next].0 <= s {
                let (_, what, i) = things[next];
                self.draw_thing(f, what, i);
                next += 1;
            }
        }
        while next < things.len() { let (_, what, i) = things[next]; self.draw_thing(f, what, i); next += 1; }
        if let Some(b) = &self.bees {
            let (sx, sy) = project(b.x, b.y, b.h);
            for k in 0..10 {
                let a = self.time * (5.0 + k as f32) + k as f32 * 1.7;
                let (px, py) = (sx + (a.cos() * 12.0) as i32, sy + ((a * 1.3).sin() * 6.0) as i32);
                let flap = ((self.time * 30.0) as i32 + k) % 2 == 0;
                f.put(px, py, 0xffd020);
                f.put(px + 1, py, 0x202020);
                f.put(px + 2, py, 0xffd020);
                f.put(px + 1, py - 1, if flap { 0xffffff } else { 0x9090a0 });
            }
        }
    }

    fn draw_thing(&self, f: &mut Frame, what: u8, i: usize) {
        let feet = |f: &mut Frame, s: &Sprite, sx: i32, sy: i32, lift: i32, flip: bool| f.blit_flip(s, sx - s.w / 2, sy - s.h + 2 - lift, flip);
        match what {
            0 => {
                let (sx, sy) = project(self.bear.0, self.bear.1, self.bear_h);
                let z = if self.jump > 0.0 { ((1.0 - self.jump / JUMP) * std::f32::consts::PI).sin() * 22.0 } else { 0.0 } as i32;
                shadow(f, sx, sy, if z > 8 { 4 } else { 6 });
                let img = if self.jump > 0.0 { &self.bear_img[2] } else if self.walking > 0.0 && (self.walking * 8.0) as i32 % 2 == 1 { &self.bear_img[1] } else { &self.bear_img[0] };
                if self.hat_time > 0.0 {
                    // The hat's magic, circling.
                    for k in 0..12 {
                        let a = self.time * 3.0 + k as f32 * 0.52;
                        let (px, py) = (sx + (a.cos() * 11.0) as i32, sy - 8 - z + (a.sin() * 5.0) as i32);
                        f.rect(px, py, 2, 1, if k % 2 == 0 { 0xc070ff } else { GOLD });
                    }
                }
                feet(f, img, sx, sy, z, self.face_left);
                let blink = self.hat_time > 0.0 && self.hat_time < 2.0 && (self.time * 10.0) as i32 % 2 == 0;
                if self.hat_time > 0.0 && !blink { f.blit(&self.hat_img, sx - self.hat_img.w / 2, sy - img.h - self.hat_img.h + 6 - z); }
            }
            1 => {
                let fo = &self.foes[i];
                let (x, y) = fo.pos();
                let (sx, sy) = project(x, y, fo.height(&self.map));
                let step = (self.time * 6.0) as usize % 2;
                let left = fo.to.0 < fo.from.0 || fo.to.1 > fo.from.1;
                match fo.kind {
                    Kind::Eater => {
                        shadow(f, sx, sy, 6);
                        feet(f, &self.eater[if fo.eating > 0.0 { (self.time * 10.0) as usize % 2 } else { 0 }], sx, sy, 0, left);
                    }
                    Kind::Tree => { shadow(f, sx, sy, 7); feet(f, &self.tree[step], sx, sy, 0, left); }
                    Kind::Ball => {
                        shadow(f, sx, sy, 6);
                        feet(f, &self.ball, sx, sy, 0, false);
                        let a = self.time * 7.0;
                        f.put(sx + (a.cos() * 3.0) as i32, sy - 7 + (a.sin() * 3.0) as i32, WHITE);
                    }
                    Kind::Witch => {
                        shadow(f, sx, sy, 5);
                        feet(f, &self.witch, sx, sy, 6 + ((self.time * 4.0).sin() * 3.0) as i32, left);
                    }
                }
            }
            2 => if let Some(c) = self.hat {
                let (sx, sy) = project(c.0 as f32 + 0.5, c.1 as f32 + 0.5, self.cell_h(c) as f32);
                shadow(f, sx, sy, 4);
                feet(f, &self.hat_img, sx, sy, 3 + ((self.time * 3.0).sin() * 2.0) as i32, false);
            },
            3 => if let Some(c) = self.pot {
                let (sx, sy) = project(c.0 as f32 + 0.5, c.1 as f32 + 0.5, self.cell_h(c) as f32);
                shadow(f, sx, sy, 5);
                feet(f, &self.pot_img, sx, sy, 0, false);
            },
            _ => {}
        }
    }

    fn draw_hud(&self, f: &mut Frame) {
        let rim = LOOKS[self.castle].rim;
        f.rect(0, 0, W, TOP, 0x000000);
        f.hline(0, TOP - 1, W, tint(rim, 0.4));
        f.text_scaled(4, 2, &format!("SCORE {:06}", self.score), TEXT, true, 2);
        let hi = format!("HI {:06}", self.high.max(self.score));
        f.text_scaled(W - 4 - Frame::text_width(&hi, true, 2), 2, &hi, 0x9090b0, true, 2);
        let where_ = format!("CASTLE {} WAVE {}", self.castle + 1 + self.round as usize * CASTLES.len(), self.wave + 1);
        f.text_scaled(4, 19, &where_, rim, false, 2);
        let gems = format!("GEMS {}", self.left);
        f.text_scaled(W / 2 - Frame::text_width(&gems, false, 2) / 2, 19, &gems, WHITE, false, 2);
        let bears = format!("BEARS {}", self.lives.saturating_sub(1));
        f.text_scaled(W - 4 - Frame::text_width(&bears, false, 2), 19, &bears, 0xd8a060, false, 2);
        if self.hat_time > 0.0 {
            let w = (self.hat_time / HAT_TIME * 80.0) as i32;
            f.rect(W / 2 - 40, TOP - 3, w, 2, 0xa040e0);
        }
    }
}

/// The tallest wall there is: from the highest block to the foundations.
const WALL_MAX: i32 = (9 - BASE) * HZ;

/// A wall's colours down its height, plain brick and mortar, worked out
/// once a frame instead of once a pixel.
struct Walls { left: Vec<(Rgb, Rgb)>, right: Vec<(Rgb, Rgb)> }

impl Walls {
    fn new(look: &Look) -> Walls {
        let down = |face: Rgb| (0..WALL_MAX).map(|r| {
            let k = 1.0 - (r as f32 / 90.0).min(0.35);
            (tint(face, k), tint(face, 0.74 * k))
        }).collect();
        Walls { left: down(look.left), right: down(look.right) }
    }
}

/// A colour made brighter or darker by a factor.
fn tint(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    let s = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    rgb(s(r), s(g), s(b))
}

/// The colour `t` of the way from `a` to `b`.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let ((ar, ag, ab), (br, bg, bb)) = (parts(a), parts(b));
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    rgb(l(ar, br), l(ag, bg), l(ab, bb))
}

/// A soft dark oval on the floor under something standing there.
fn shadow(f: &mut Frame, sx: i32, sy: i32, rx: i32) {
    let ry = (rx / 2).max(1);
    for dy in -ry..=ry {
        let half = ((1.0 - (dy * dy) as f32 / (ry * ry) as f32).max(0.0).sqrt() * rx as f32) as i32;
        for dx in -half..=half { let c = f.get(sx + dx, sy + dy); f.put(sx + dx, sy + dy, tint(c, 0.55)); }
    }
}

/// Twice the size, with the diagonal steps smoothed (the Scale2x rule).
fn scale2x(s: &Sprite) -> Sprite {
    let (w, h) = (s.w, s.h);
    let at = |x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { 0 } else { s.px[(y * w + x) as usize] };
    let mut px = vec![0u32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (p, a, b, c, d) = (at(x, y), at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1));
            let e0 = if c == a && c != d && a != b { a } else { p };
            let e1 = if a == b && a != c && b != d { b } else { p };
            let e2 = if d == c && d != b && c != a { c } else { p };
            let e3 = if b == d && b != a && d != c { d } else { p };
            let (i, w2) = ((y * 2 * w * 2 + x * 2) as usize, (w * 2) as usize);
            px[i] = e0;
            px[i + 1] = e1;
            px[i + w2] = e2;
            px[i + w2 + 1] = e3;
        }
    }
    Sprite { w: w * 2, h: h * 2, px }
}

/// Light from above and the left: edges facing it lighter, the others darker.
fn shade(s: &Sprite) -> Sprite {
    let on = |x: i32, y: i32| x >= 0 && y >= 0 && x < s.w && y < s.h && s.px[(y * s.w + x) as usize] >> 24 != 0;
    let mut out = s.clone();
    for y in 0..s.h {
        for x in 0..s.w {
            let i = (y * s.w + x) as usize;
            if s.px[i] >> 24 == 0 { continue; }
            let k = if !on(x, y - 1) || !on(x - 1, y) { 1.25 } else if !on(x, y + 1) || !on(x + 1, y) { 0.72 } else { 1.0 };
            out.px[i] = 0xff00_0000 | tint(s.px[i] & 0xff_ffff, k);
        }
    }
    out
}

/// A one-pixel outline in `c` round the sprite.
fn outline(s: &Sprite, c: Rgb) -> Sprite {
    let (w, h) = (s.w + 2, s.h + 2);
    let on = |x: i32, y: i32| x >= 1 && y >= 1 && x <= s.w && y <= s.h && s.px[((y - 1) * s.w + x - 1) as usize] >> 24 != 0;
    let mut px = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            px[(y * w + x) as usize] = if on(x, y) { s.px[((y - 1) * s.w + x - 1) as usize] }
                else if on(x - 1, y) || on(x + 1, y) || on(x, y - 1) || on(x, y + 1) { 0xff00_0000 | c } else { 0 };
        }
    }
    Sprite { w, h, px }
}

/// A small drawing made ready for the big screen: doubled, lit, outlined.
fn fancy(rows: &[&str], palette: &[(char, Rgb)]) -> Sprite {
    outline(&shade(&scale2x(&Sprite::from_rows(rows, palette))), 0x100818)
}

/// The crystal ball: a lit sphere with a bright spot.
fn crystal_ball() -> Sprite {
    let (n, r) = (13, 6.5f32);
    let mut px = vec![0u32; (n * n) as usize];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 6.0, y as f32 - 6.0);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r { continue; }
            let nz = (1.0 - d2 / (r * r)).sqrt();
            let lit = (-0.5 * dx / r - 0.6 * dy / r + 0.62 * nz).max(0.0);
            let mut c = mix(0x184860, 0x9ae8ff, lit);
            if (dx + 2.5).powi(2) + (dy + 2.5).powi(2) < 2.2 { c = WHITE; }
            if (dx - 2.0).powi(2) + (dy - 3.0).powi(2) < 1.2 { c = mix(c, WHITE, 0.5); }
            px[(y * n + x) as usize] = 0xff00_0000 | c;
        }
    }
    outline(&Sprite { w: n, h: n, px }, 0x0c1830)
}

/// Big letters in a crystal gradient, with a light sweeping across them
/// and a shadow under them.
fn fancy_text(f: &mut Frame, cx: i32, y: i32, s: &str, scale: i32, time: f32) {
    let (w, h) = (Frame::text_width(s, true, scale), 7 * scale);
    let mut m = Frame::new(w + 1, h + 1);
    m.text_scaled(0, 0, s, WHITE, true, scale);
    let x0 = cx - w / 2;
    let lit = |xx: i32, yy: i32| m.get(xx, yy) != BLACK;
    for yy in 0..h { for xx in 0..w { if lit(xx, yy) { f.put(x0 + xx + scale / 2 + 1, y + yy + scale / 2 + 1, 0x000000); } } }
    let band = ((time * 0.5).fract() * (w + h) as f32 * 1.6) as i32 - h;
    for yy in 0..h {
        for xx in 0..w {
            if !lit(xx, yy) { continue; }
            let mut c = mix(0xe8f8ff, 0x6070ff, yy as f32 / h as f32);
            let d = (xx + yy - band).abs();
            if d < scale * 2 { c = mix(c, WHITE, 1.0 - d as f32 / (scale * 2) as f32); }
            f.put(x0 + xx, y + yy, c);
        }
    }
}

impl Game for Gems {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        self.particles.update(dt);
        if let Some((_, t)) = self.note.as_mut() { *t -= dt; if *t <= 0.0 { self.note = None; } }
        match self.mode {
            Mode::Title => if input.pressed(Key::Space) || input.pressed(Key::Enter) { self.start_game(); },
            Mode::Play => self.play(input, dt),
            Mode::Dying(t) => {
                let t = t - dt;
                if t > 0.0 { self.mode = Mode::Dying(t); }
                else {
                    self.lives -= 1;
                    if self.lives == 0 {
                        if funkey::store::record_score(GAME, self.score) { self.high = self.score; }
                        self.mode = Mode::Over(4.0);
                    } else { self.respawn(); self.mode = Mode::Play; }
                }
            }
            Mode::Clear(t) => {
                let t = t - dt;
                if t > 0.0 { self.mode = Mode::Clear(t); }
                else {
                    let (mut castle, mut wave) = (self.castle, self.wave + 1);
                    if wave == 3 { wave = 0; castle += 1; if castle == CASTLES.len() { castle = 0; self.round += 1; } }
                    self.load(castle, wave);
                    self.mode = Mode::Play;
                }
            }
            Mode::Warp(t) => {
                let t = t - dt;
                if t > 0.0 { self.mode = Mode::Warp(t); } else { self.add(5000); self.load(2, 0); self.mode = Mode::Play; }
            }
            Mode::Over(t) => {
                let t = t - dt;
                if t <= 0.0 || input.pressed(Key::Space) {
                    self.mode = Mode::Title;
                    self.load(0, 0);
                    self.audio.play_loop(1, &self.s_title, 1.0);
                } else { self.mode = Mode::Over(t); }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        self.draw_sky(f);
        self.draw_world(f);
        self.particles.draw(f, 0, 0);
        if self.mode == Mode::Title {
            f.dim(0.5);
            fancy_text(f, W / 2, 48 + ((self.time * 2.0).sin() * 5.0) as i32, "GEMS", 12, self.time);
            f.text_centered(W / 2, 150, "A TRIBUTE TO CRYSTAL CASTLES", TEXT, true, 2);
            f.text_centered(W / 2, 170, "ATARI 1983", 0x9090b0, false, 2);
            f.blit_scaled(&self.bear_img[0], W / 2 - 26, 190, 4, false);
            f.text_centered(W / 2, 276, &format!("HIGH SCORE {:06}", self.high), TEXT, true, 2);
            if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 302, "PRESS SPACE", GOLD, true, 2); }
            f.text_centered(W / 2, 336, "ARROWS WALK  TWO FOR DIAGONALS  SPACE JUMPS", 0x8080a0, false, 2);
            f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x505070);
            return;
        }
        self.draw_hud(f);
        if let Some((s, _)) = &self.note { f.text_centered(W / 2, TOP + 8, s, GOLD, true, 2); }
        match self.mode {
            Mode::Clear(_) => {
                f.dim(0.6);
                fancy_text(f, W / 2, 140, "WAVE CLEAR", 5, self.time);
                if self.last_bonus > 0 { f.text_centered(W / 2, 196, &format!("LAST GEM BONUS {}", self.last_bonus), TEXT, true, 2); }
                if self.wave == 2 { f.text_centered(W / 2, 222, &format!("ON TO {}", CASTLES[(self.castle + 1) % CASTLES.len()].name), GOLD, true, 2); }
            }
            Mode::Warp(_) => {
                f.dim(0.5);
                fancy_text(f, W / 2, 130, "WARP", 9, self.time * 4.0);
                f.text_centered(W / 2, 212, "TO CASTLE 3  BONUS 5000", TEXT, true, 2);
            }
            Mode::Over(_) => {
                f.dim(0.5);
                f.text_centered(W / 2, 150, "GAME OVER", 0xe03030, true, 4);
                if self.score >= self.high && self.score > 0 { f.text_centered(W / 2, 196, "NEW HIGH SCORE", GOLD, true, 2); }
            }
            Mode::Play if self.wave_time < 2.5 => {
                f.text_centered(W / 2, H - 22, CASTLES[self.castle].name, LOOKS[self.castle].rim, true, 2);
            }
            _ => {}
        }
    }
}

fn main() {
    let mut game = Gems::new();
    // GEMS_BENCH=<frames> plays that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("GEMS_BENCH") {
        let n: u32 = n.parse().unwrap_or(1000);
        game.start_game();
        let mut f = Frame::new(W, H);
        let input = Input::new();
        let t0 = std::time::Instant::now();
        for _ in 0..n { game.update(&input, 1.0 / 60.0); game.draw(&mut f); }
        eprintln!("{:.3} ms a frame at {}x{} over {} frames", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n);
        return;
    }
    run(&mut game, Config { width: W, height: H, fps: 60 });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_castle_is_whole() {
        for c in &CASTLES {
            for r in &c.rows { assert_eq!(r.chars().count(), N as usize, "{}: a row is not {N} wide", c.name); }
            let map = Map::from_rows(&c.rows);
            let start = c.objs.iter().find(|o| o.0 == 'B').map(|o| (o.1, o.2)).expect("a start");
            let reach = map.field(&[start]);
            for &(k, x, y) in c.objs {
                if matches!(k, 'E' | 'T' | 'O' | 'W' | 'H' | 'P' | '*') {
                    assert!(reach[(y * N + x) as usize] != i32::MAX, "{}: {k} at {x},{y} cannot be reached", c.name);
                }
            }
            let blocks = map.h.iter().enumerate().filter(|(i, h)| h.is_some() && reach[*i] != i32::MAX).count();
            assert!(blocks >= 60, "{}: only {blocks} blocks to walk", c.name);
        }
    }

    /// Run the bear left along the front row at a tree coming the other
    /// way, jump when it is close, and land alive on the far side.
    #[test]
    fn a_tree_can_be_jumped() {
        std::env::set_var("FUNKEY_SOUND", "0");
        let mut g = Gems::new();
        g.start_game();
        g.foes = vec![Foe::new(Kind::Tree, (7, 13), 1.25 * PACE, 0.0)];
        let mut input = Input::new();
        let mut jumped = false;
        for _ in 0..180 {
            input.release_all();
            input.inject(Key::Left);
            let (tx, _) = g.foes[0].pos();
            if !jumped && (tx - g.bear.0).abs() < 0.9 { input.inject(Key::Space); jumped = true; }
            g.update(&input, 1.0 / 60.0);
            assert!(g.mode == Mode::Play, "the bear died at {:?}, the tree at {:?}", g.bear, g.foes[0].pos());
        }
        assert!(jumped);
        assert!(g.foes[0].pos().0 > g.bear.0, "the tree should be behind the bear");
    }

    #[test]
    fn stairs_climb_and_walls_do_not() {
        let map = Map::from_rows(&CASTLES[0].rows);
        assert!(map.walk((1, 6), (2, 6)));
        assert!(!map.walk((1, 5), (2, 5)));
    }
}
