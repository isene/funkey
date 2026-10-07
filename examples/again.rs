//! again: a puzzle you solve with your own past.
//!
//! You stand in a small room and a life is a handful of steps. When they
//! are used up, time starts over: you are back where you began, and beside
//! you walks the self you just were, doing every step again. So one of you
//! can stand on a plate while the next walks through the door it opens.
//! A room is solved when someone stands on every white ring at once.
//!
//!     cargo run --release --example again
//!
//! The arrows step (or WASD, or HJKL) and Space stands still for a step.
//! Enter lets the rest of the life run out. U takes back a step, Backspace
//! takes back the life, R starts the room over. N and P go to the next
//! room and the one before, Q leaves.
//!
//! The rules, all of them:
//! - Every self steps at the same moment. A past self presses the keys it
//!   pressed then; a step that is stopped is lost, the rest still come.
//! - Selves walk through each other.
//! - A door is open while someone stands on a plate of its colour, and
//!   while someone stands in the doorway.
//! - A pale gate lets only a past self through. A bright gate lets only
//!   the present self through.
//!
//! Every room is proven by a search in the tests: it can be solved with
//! the selves and steps it gives, and not with one self fewer.
//!
//! `AGAIN_START=<room>` starts in a room; `AGAIN_BENCH=<frames>` times the
//! game with no terminal. All of it is new: the rooms, the look, the
//! sounds and the round that plays, which gains a voice with every self.

use funkey::*;
use std::f32::consts::PI;

const W: i32 = 480;
const H: i32 = 270;
const GAME: &str = "again";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";
/// The most selves a room can give.
const MOST: usize = 4;
/// A tile's side in pixels, and the band of the screen a room is drawn in.
const TILE: i32 = 20;
const BOARD_Y: i32 = 18;
const BOARD_H: i32 = 180;
/// Seconds: one step sliding, one step when time runs by itself, and the
/// whole way back to the start of a life.
const SLIDE: f32 = 0.11;
const RUN: f32 = 0.09;
const BACK: f32 = 0.8;

const BG: Rgb = 0x0b0d14;
const FLOOR: [Rgb; 2] = [0x171b28, 0x1b2030];
const WALL: Rgb = 0x3b4460;
const WALL_LIT: Rgb = 0x505c7e;
const WALL_DARK: Rgb = 0x2b3248;
/// The present self is bright and a past self pale, and so are the gates
/// that let each through.
const BRIGHT: Rgb = 0xfff1cc;
const GOLD: Rgb = 0xffc857;
const PALE: Rgb = 0x86aabf;
const PALE_DARK: Rgb = 0x4a6577;
const TEXT: Rgb = 0xd8dcec;
const DIM: Rgb = 0x6a7190;
const FAINT: Rgb = 0x3a4160;
const RED: Rgb = 0xff5a5a;
/// The colours of the plates and their doors: red, blue, green, amber.
const HUES: [Rgb; 4] = [0xe5484d, 0x3e8ef7, 0x2fbf71, 0xf2a33a];

/// The ways a self can step: up, right, down, left.
const WAYS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// What a self does in one step: stand still, or go one of the four ways.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Act { Wait, Go(u8) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tile { Wall, Floor, Ring, Plate(u8), Door(u8), Pale, Bright }

/// A room as it is written down. In the map `#` is wall, `.` floor, `@`
/// where every self starts and `X` a ring. `a` to `d` are plates and `A`
/// to `D` their doors. `:` is a pale gate and `!` a bright one.
struct Room {
    name: &'static str,
    /// How many selves the room gives, and how many steps a life is.
    selves: usize,
    steps: usize,
    /// A line for the first life and one for the lives after it.
    says: [&'static str; 2],
    map: &'static [&'static str],
}

/// The rooms `THREE`, `ROUND` and `THREE MINDS` come back later with fewer
/// selves, so their maps stand apart.
const THREE: &[&str] = &[
    "#############",
    "#.a.#...#...#",
    "#...#...#...#",
    "#.@.A...B.X.#",
    "#...#...#...#",
    "#...#.b.#...#",
    "#############"];
const MINDS: &[&str] = &[
    "###############",
    "#...#.....#...#",
    "#.a.:..@..!.b.#",
    "#...#.....#...#",
    "#######A#######",
    "#######B#######",
    "#######X#######",
    "###############"];

const ROOMS: &[Room] = &[
    Room { name: "TWICE", selves: 2, steps: 10,
        says: ["THE RED DOOR IS OPEN WHILE SOMEONE STANDS ON THE RED PLATE. STAND ON IT, THEN PRESS ENTER.",
            "THAT IS YOU, A LIFE AGO. NOW WALK TO THE WHITE RING."],
        map: &[
            "###########",
            "#....#....#",
            "#.@..A..X.#",
            "#....#....#",
            "#.a..#....#",
            "###########"] },
    Room { name: "THREE", selves: 3, steps: 11, says: ["TWO DOORS, AND A PLATE FOR EACH.", ""], map: THREE },
    Room { name: "BOTH", selves: 2, steps: 9,
        says: ["EVERY RING NEEDS SOMEONE ON IT, ALL AT THE SAME MOMENT.", ""],
        map: &[
            "###############",
            "#...#.........#",
            "#.X.A....@..a.#",
            "#...#.......X.#",
            "###############"] },
    Room { name: "HAND OVER", selves: 2, steps: 12,
        says: ["TWO DOORS AGAIN, AND ONLY TWO OF YOU.", ""],
        map: &[
            "###############",
            "#.a...#...#...#",
            "#.....#...#...#",
            "#.@...A...B.X.#",
            "#.....#...#...#",
            "#.b...#...#...#",
            "###############"] },
    Room { name: "DOORSTOP", selves: 2, steps: 13,
        says: ["A DOOR STAYS OPEN WHILE SOMEONE STANDS IN IT.", ""],
        map: &[
            "###########",
            "#...#.....#",
            "#.@.A...X.#",
            "#...#.....#",
            "#.a.#...X.#",
            "###########"] },
    Room { name: "TWO WILL DO", selves: 2, steps: 14, says: ["YOU HAVE BEEN HERE BEFORE, AS THREE.", ""], map: THREE },
    Room { name: "AT ONCE", selves: 2, steps: 10,
        says: ["A PAST SELF PRESSES THE KEYS IT PRESSED THEN. A DOOR THAT STOPPED IT THEN MAY BE OPEN NOW.", ""],
        map: &[
            "#################",
            "#...#.......#...#",
            "#.X.Ab.@....aB.X#",
            "#...#.......#...#",
            "#################"] },
    Room { name: "ROUND", selves: 4, steps: 16,
        says: ["FOUR RINGS AND FOUR OF YOU. EACH DOOR IS OPENED FROM THE ROOM BEFORE IT.", ""],
        map: &[
            "###############",
            "######cX.######",
            "#######B#######",
            "#####a....#####",
            "#Xb.A..@..C.dX#",
            "#####.....#####",
            "#######D#######",
            "######.X.######",
            "###############"] },
    Room { name: "BUSY", selves: 3, steps: 18,
        says: ["THE SAME DOORS, ONE RING, AND THREE OF YOU.", ""],
        map: &[
            "###############",
            "######c..######",
            "#######B#######",
            "#####a....#####",
            "#.b.A..@..C.d.#",
            "#####.....#####",
            "#######D#######",
            "######.X.######",
            "###############"] },
    Room { name: "NOT YOU", selves: 2, steps: 8,
        says: ["THE PALE GATE LETS ONLY A PAST SELF THROUGH. WALK AS IF IT WERE OPEN, THEN PRESS ENTER.",
            "NOW PRESS ENTER AND WATCH."],
        map: &[
            "#########",
            "#....#..#",
            "#.@..:.X#",
            "#....#..#",
            "#########"] },
    Room { name: "THEN AND NOW", selves: 2, steps: 6,
        says: ["THE BRIGHT GATE LETS ONLY YOU THROUGH, AS YOU ARE NOW.", ""],
        map: &[
            "###########",
            "#..#...#..#",
            "#X.:.@.!.X#",
            "#..#...#..#",
            "###########"] },
    Room { name: "HAVE FAITH", selves: 2, steps: 9,
        says: ["ONLY THE LAST OF YOU CAN REACH THE PLATE.", ""],
        map: &[
            "#############",
            "#...#....#..#",
            "#.a.!.@..A.X#",
            "#...#....#..#",
            "#############"] },
    Room { name: "IN TIME", selves: 2, steps: 9,
        says: ["A STEP THAT IS STOPPED IS LOST. COUNT THEM.", ""],
        map: &[
            "#############",
            "#...#....#..#",
            "#.a.!.@..A..#",
            "#...#....#.X#",
            "#############"] },
    Room { name: "GLASS ROOM", selves: 2, steps: 10,
        says: ["YOU CANNOT GO IN THERE. YOU CAN HAVE BEEN THERE.", ""],
        map: &[
            "##############",
            "#.....#...#..#",
            "#..a..:.@.A.X#",
            "#.....#...#..#",
            "##############"] },
    Room { name: "GLASS HANDS", selves: 2, steps: 14,
        says: ["ONE PAST SELF, TWO PLATES.", ""],
        map: &[
            "#############",
            "#.a...#.....#",
            "#.....:..@..#",
            "#.b...#.....#",
            "########A####",
            "########B####",
            "########X####",
            "#############"] },
    Room { name: "HELD OPEN", selves: 2, steps: 16,
        says: ["BOTH OF YOU MUST GET THROUGH THE DOOR.", ""],
        map: &[
            "#############",
            "#...#...#...#",
            "#.a.!.@.A..X#",
            "#...#...#...#",
            "#...#...#..X#",
            "#############"] },
    Room { name: "THREE MINDS", selves: 3, steps: 10,
        says: ["ONE FOR THE PALE GATE, ONE FOR THE BRIGHT GATE, ONE FOR THE DOORS.", ""], map: MINDS },
    Room { name: "ALL OF YOU", selves: 3, steps: 13,
        says: ["THREE RINGS, AND ALL YOU KNOW.", ""],
        map: &[
            "####################",
            "#...#..c#.....###..#",
            "#.X.Ab..:..@..!aB.X#",
            "#...#...#.....###..#",
            "###########C########",
            "##########.X.#######",
            "####################"] },
    Room { name: "KEEP TIME", selves: 2, steps: 16,
        says: ["THE LAST OF YOU HAS TWO PLATES TO STAND ON. THE FIRST MUST KNOW WHEN.", ""],
        map: &[
            "#################",
            "#.a.#...#...#...#",
            "#...#...#...#...#",
            "#...!.@.A...B...#",
            "#...#...#...#..X#",
            "#.b.#...#...#...#",
            "#################"] },
    Room { name: "TWO MINDS", selves: 2, steps: 22, says: ["YOU HAVE BEEN HERE BEFORE, AS THREE.", ""], map: MINDS },
];

/// A room read from its map, with the places that matter looked up.
struct Level {
    w: i32,
    h: i32,
    tiles: Vec<Tile>,
    start: u8,
    rings: Vec<u8>,
    /// Each plate: where it is and its colour.
    plates: Vec<(u8, u8)>,
}

impl Level {
    fn parse(map: &[&str]) -> Level {
        let (w, h) = (map[0].len() as i32, map.len() as i32);
        let mut lv = Level { w, h, tiles: Vec::new(), start: 0, rings: Vec::new(), plates: Vec::new() };
        for row in map {
            assert_eq!(row.len() as i32, w, "a row of another width: {row}");
            for ch in row.bytes() {
                let at = lv.tiles.len() as u8;
                lv.tiles.push(match ch {
                    b'#' => Tile::Wall,
                    b'@' => { lv.start = at; Tile::Floor }
                    b'X' => { lv.rings.push(at); Tile::Ring }
                    b'a'..=b'd' => { lv.plates.push((at, ch - b'a')); Tile::Plate(ch - b'a') }
                    b'A'..=b'D' => Tile::Door(ch - b'A'),
                    b':' => Tile::Pale,
                    b'!' => Tile::Bright,
                    _ => Tile::Floor,
                });
            }
        }
        lv
    }

    fn xy(&self, at: u8) -> (i32, i32) { (at as i32 % self.w, at as i32 / self.w) }

    fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { Tile::Wall } else { self.tiles[(y * self.w + x) as usize] }
    }

    /// The colours with someone on a plate, one bit a colour.
    fn power(&self, who: &[u8]) -> u8 {
        self.plates.iter().filter(|p| who.contains(&p.0)).fold(0, |bits, p| bits | 1 << p.1)
    }

    /// True when a door or a gate at `at` lets a self in. `now` says the
    /// self is the present one.
    fn open(&self, who: &[u8], power: u8, at: u8, now: bool) -> bool {
        match self.tiles[at as usize] {
            Tile::Wall => false,
            Tile::Door(c) => power >> c & 1 == 1 || who.contains(&at),
            Tile::Pale => !now,
            Tile::Bright => now,
            _ => true,
        }
    }

    /// Where a step ends, or None when it is stopped.
    fn walk(&self, who: &[u8], power: u8, from: u8, way: u8, now: bool) -> Option<u8> {
        let ((x, y), (dx, dy)) = (self.xy(from), WAYS[way as usize]);
        if self.tile(x + dx, y + dy) == Tile::Wall { return None; }
        let to = ((y + dy) * self.w + x + dx) as u8;
        self.open(who, power, to, now).then_some(to)
    }

    /// One step of time. Everyone steps at once, into the room as it was
    /// before the step; the last of `who` is the present self. Gives where
    /// they all are after it, and a bit for each self that was stopped.
    fn step(&self, who: &[u8], acts: &[Act]) -> (Vec<u8>, u8) {
        let power = self.power(who);
        let (mut next, mut stopped) = (who.to_vec(), 0);
        for (i, act) in acts.iter().enumerate() {
            let Act::Go(way) = *act else { continue };
            match self.walk(who, power, who[i], way, i + 1 == who.len()) {
                Some(to) => next[i] = to,
                None => stopped |= 1 << i,
            }
        }
        (next, stopped)
    }

    fn won(&self, who: &[u8]) -> bool { self.rings.iter().all(|r| who.contains(r)) }
}

/// What every self does at a step. A life with no more steps stands still.
fn acts_at(lives: &[Vec<Act>], t: usize) -> Vec<Act> {
    lives.iter().map(|l| l.get(t).copied().unwrap_or(Act::Wait)).collect()
}

// ------------------------------------------------------------------- sounds

struct Sounds {
    step: Sample,
    stop: Sample,
    plate: Sample,
    door: Sample,
    shut: Sample,
    rewind: Sample,
    born: Sample,
    solved: Sample,
    spent: Sample,
    undo: Sample,
    /// The round with one voice, with two, with three and with four.
    round: Vec<Sample>,
}

/// The round that plays, a bar an entry. Every self sings it two bars
/// behind the one before, so a room sounds fuller with every life.
const BARS: [&str; 8] = [
    "a4 c5 e5/2", "d5 c5 a4/2", "g4 a4 c5 d5", "e5/2 - e5/8 g5/8",
    "a5 g5 e5/2", "d5 e5 c5/2", "a4 g4 a4 c5", "a4/2 -/2"];

/// One voice of the round: it comes in `late` bars after the first, and
/// sings `up` octaves higher.
fn voice(late: usize, up: i32, wave: Wave, vol: f32) -> Sample {
    let mut text = String::from("104");
    for n in 0..BARS.len() {
        for note in BARS[(n + BARS.len() - late) % BARS.len()].split(' ') {
            text.push(' ');
            text.extend(note.chars().enumerate().map(|(i, c)| if i == 1 && c.is_ascii_digit() { char::from((c as i32 + up) as u8) } else { c }));
        }
    }
    Tune::parse(&text, wave, vol).render()
}

/// Several samples played at once, as one.
fn mixed(parts: &[Sample]) -> Sample {
    let len = parts.iter().map(|p| p.data.len()).max().unwrap_or(0);
    Sample::from_i16((0..len).map(|i| parts.iter().map(|p| *p.data.get(i).unwrap_or(&0) as i32).sum::<i32>().clamp(-32768, 32767) as i16).collect())
}

impl Sounds {
    fn new() -> Sounds {
        let voices = [voice(0, 0, Wave::Triangle, 0.26), voice(2, -1, Wave::Sine, 0.3),
            voice(4, 0, Wave::Square, 0.045), voice(6, 1, Wave::Triangle, 0.09)];
        Sounds {
            step: Sample::sweep(Wave::Triangle, 520.0, 360.0, 0.035, 0.3),
            stop: Sample::sweep(Wave::Square, 150.0, 90.0, 0.07, 0.16),
            plate: Sample::tone(Wave::Sine, 660.0, 0.05, 0.3).then(&Sample::tone(Wave::Sine, 990.0, 0.08, 0.3)),
            door: Sample::sweep(Wave::Saw, 170.0, 420.0, 0.14, 0.1),
            shut: Sample::sweep(Wave::Saw, 380.0, 150.0, 0.12, 0.1),
            rewind: Sample::sweep(Wave::Triangle, 1300.0, 140.0, BACK, 0.22),
            born: Tune::parse("300 e5/8 a5/8 e6/4", Wave::Triangle, 0.25).render(),
            solved: Tune::parse("220 a4/8 c5/8 e5/8 a5/8 c6/8 e6/8 a6/2", Wave::Triangle, 0.28).render(),
            spent: Tune::parse("150 e4/8 c4/8 a3/2", Wave::Triangle, 0.28).render(),
            undo: Sample::sweep(Wave::Sine, 420.0, 560.0, 0.04, 0.25),
            round: (1..=voices.len()).map(|n| mixed(&voices[..n])).collect(),
        }
    }
}

// --------------------------------------------------------------------- game

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Title,
    /// A key is waited for.
    Play,
    /// Time runs by itself to the end of the life: the seconds since the
    /// last step.
    Run(f32),
    /// The life is over and time goes back to its start: seconds, from a
    /// short wait below zero up to `BACK`.
    Rewind(f32),
    /// Seconds since the room was solved.
    Solved(f32),
    /// The last life is over and the room is not solved.
    Spent,
    /// Seconds since the last of all the rooms was solved.
    End(f32),
}

/// What a key asks for.
#[derive(Clone, Copy)]
enum Want { Step(Act), Undo }

/// Seconds a stopped self leans into what stopped it.
const BUMP: f32 = 0.16;

struct Again {
    audio: Audio,
    snd: Sounds,
    mode: Mode,
    time: f32,
    room: usize,
    lv: Level,
    /// The lives lived in this room so far: the keys each self pressed.
    lives: Vec<Vec<Act>>,
    /// The keys of the present self.
    live: Vec<Act>,
    /// Where everyone stood at the start of this life and after each step
    /// of it, and who was stopped in each step, a bit a self.
    path: Vec<Vec<u8>>,
    stops: Vec<u8>,
    /// Where each self is drawn, in pixels from the room's corner, the
    /// way it faces, and the seconds left of its lean into a shut door.
    seen: Vec<(f32, f32)>,
    face: Vec<u8>,
    bump: Vec<f32>,
    /// How far open each tile's door is drawn, 0 to 1.
    ajar: Vec<f32>,
    /// Where the mark on the timeline is drawn, in steps.
    head: f32,
    /// Seconds until a key that is kept down steps again.
    hold: f32,
    /// True at the start of a life until the keys are let go: a key kept
    /// down from the life before takes no step in the new one.
    rest: bool,
    /// The rooms solved, a bit a room.
    done: u32,
    /// How many voices of the round are playing.
    voices: usize,
    particles: Particles,
}

impl Again {
    fn new() -> Again {
        let done = if cfg!(test) { 0 } else { store::load(GAME, "solved").and_then(|s| s.trim().parse().ok()).unwrap_or(0) };
        let mut game = Again {
            audio: Audio::off(), snd: Sounds::new(), mode: Mode::Title, time: 0.0, room: 0, lv: Level::parse(ROOMS[0].map),
            lives: Vec::new(), live: Vec::new(), path: Vec::new(), stops: Vec::new(), seen: Vec::new(), face: Vec::new(),
            bump: Vec::new(), ajar: Vec::new(), head: 0.0, hold: 0.0, rest: false, done, voices: 0, particles: Particles::new(),
        };
        game.go(game.first_open());
        game.mode = Mode::Title;
        game
    }

    /// The first room not solved yet.
    fn first_open(&self) -> usize { (0..ROOMS.len()).find(|n| self.done >> n & 1 == 0).unwrap_or(0) }

    /// How many steps of this life have gone.
    fn tick(&self) -> usize { self.path.len() - 1 }

    /// The corner of the room on the screen.
    fn origin(&self) -> (i32, i32) { ((W - self.lv.w * TILE) / 2, BOARD_Y + (BOARD_H - self.lv.h * TILE) / 2) }

    /// The middle of a tile, in pixels from the room's corner.
    fn spot(&self, at: u8) -> (f32, f32) {
        let (x, y) = self.lv.xy(at);
        ((x * TILE + TILE / 2) as f32, (y * TILE + TILE / 2) as f32)
    }

    /// While time goes back: the step it has come to, with its fraction.
    fn rewound(&self) -> Option<f32> {
        match self.mode {
            Mode::Rewind(s) if s >= 0.0 => Some((1.0 - ease(s / BACK)) * self.tick() as f32),
            _ => None,
        }
    }

    /// Where everyone stands at the moment shown.
    fn shown(&self) -> &[u8] {
        let t = self.rewound().map(|r| r.round() as usize).unwrap_or(self.tick());
        &self.path[t]
    }

    /// Walk into a room, as the first self.
    fn go(&mut self, room: usize) {
        self.room = room;
        self.lv = Level::parse(ROOMS[room].map);
        self.lives.clear();
        self.live.clear();
        self.ajar = vec![0.0; self.lv.tiles.len()];
        self.seen.clear();
        self.particles.list.clear();
        self.rest = true;
        self.replay();
    }

    fn to_title(&mut self) {
        self.mode = Mode::Title;
        self.music();
    }

    /// After a solved room: the next one, or the end when all are solved.
    fn next(&mut self) {
        if self.room + 1 < ROOMS.len() { self.go(self.room + 1); }
        else if self.done.count_ones() as usize == ROOMS.len() { self.mode = Mode::End(0.0); }
        else {
            self.room = self.first_open();
            self.to_title();
        }
    }

    /// The round with a voice for every self in the room; all four on the
    /// title.
    fn music(&mut self) {
        let n = if self.mode == Mode::Title { MOST } else { (self.lives.len() + 1).min(MOST) };
        if n != self.voices {
            self.voices = n;
            self.audio.play_loop(1, &self.snd.round[n - 1], 0.45);
        }
    }

    /// Live this life again from its start, with the keys pressed so far.
    fn replay(&mut self) {
        let n = self.lives.len() + 1;
        self.path = vec![vec![self.lv.start; n]];
        self.stops.clear();
        self.face = vec![2; n];
        for t in 0..self.live.len() {
            let mut acts = acts_at(&self.lives, t);
            acts.push(self.live[t]);
            let (next, stopped) = self.lv.step(&self.path[t], &acts);
            for (face, act) in self.face.iter_mut().zip(&acts) {
                if let Act::Go(way) = *act { *face = way; }
            }
            self.path.push(next);
            self.stops.push(stopped);
        }
        let home = self.spot(self.lv.start);
        self.seen.resize(n, home);
        self.bump = vec![0.0; n];
        self.mode = Mode::Play;
        self.music();
    }

    /// One step of time. `by_hand` says the present self's act came from
    /// a key, and so is kept for when this self is a past one.
    fn advance(&mut self, mine: Act, by_hand: bool) {
        let t = self.tick();
        let mut acts = acts_at(&self.lives, t);
        acts.push(mine);
        let who = self.path[t].clone();
        let (next, stopped) = self.lv.step(&who, &acts);
        if by_hand { self.live.push(mine); }
        for (i, act) in acts.iter().enumerate() {
            let Act::Go(way) = *act else { continue };
            self.face[i] = way;
            if stopped >> i & 1 == 1 { self.bump[i] = BUMP; }
        }
        // What the step sounded like.
        let (was, is) = (self.lv.power(&who), self.lv.power(&next));
        let doors = |who: &[u8], power: u8| (0..self.lv.tiles.len())
            .filter(|&at| matches!(self.lv.tiles[at], Tile::Door(_)) && self.lv.open(who, power, at as u8, false)).count();
        let (shut, open) = (doors(&who, was), doors(&next, is));
        let me = who.len() - 1;
        if is & !was != 0 { self.audio.play(&self.snd.plate, 0.5); }
        if open > shut { self.audio.play(&self.snd.door, 0.6); }
        if open < shut { self.audio.play(&self.snd.shut, 0.6); }
        match mine {
            Act::Go(_) if stopped >> me & 1 == 1 => self.audio.play(&self.snd.stop, 0.6),
            Act::Go(_) => self.audio.play(&self.snd.step, 0.5),
            Act::Wait if next != who => self.audio.play(&self.snd.step, 0.2),
            Act::Wait => {}
        }
        self.path.push(next);
        self.stops.push(stopped);
        // And what it led to.
        let room = &ROOMS[self.room];
        if self.lv.won(&self.path[t + 1]) {
            self.mode = Mode::Solved(0.0);
            self.done |= 1 << self.room;
            if !cfg!(test) { store::save(GAME, "solved", &self.done.to_string()); }
            self.audio.play(&self.snd.solved, 0.6);
        } else if t + 1 >= room.steps {
            if self.lives.len() + 1 < room.selves { self.mode = Mode::Rewind(-0.25); }
            else {
                self.mode = Mode::Spent;
                self.audio.play(&self.snd.spent, 0.5);
            }
        }
    }

    /// Take back a step. At the start of a life that is the last step of
    /// the life before, as it was when its keys ended.
    fn undo(&mut self) {
        if self.tick() > self.live.len() {
            // Time ran on by itself: back to the last key.
        } else if self.live.pop().is_none() {
            let Some(life) = self.lives.pop() else { return };
            self.live = life;
            if self.live.len() >= ROOMS[self.room].steps { self.live.pop(); }
        }
        self.replay();
        self.audio.play(&self.snd.undo, 0.4);
    }

    /// Take back the life: this one, or at its start the one before.
    fn life_back(&mut self) {
        if self.tick() == 0 { self.lives.pop(); }
        self.live.clear();
        self.replay();
        self.audio.play(&self.snd.undo, 0.4);
    }

    /// The key that counts this tick: a fresh press at once, a key kept
    /// down every so often.
    fn wanted(&mut self, input: &Input, dt: f32) -> Option<Want> {
        const KEYS: [(Want, [Key; 3]); 6] = [
            (Want::Step(Act::Go(0)), [Key::Up, Key::Char('w'), Key::Char('k')]),
            (Want::Step(Act::Go(1)), [Key::Right, Key::Char('d'), Key::Char('l')]),
            (Want::Step(Act::Go(2)), [Key::Down, Key::Char('s'), Key::Char('j')]),
            (Want::Step(Act::Go(3)), [Key::Left, Key::Char('a'), Key::Char('h')]),
            (Want::Step(Act::Wait), [Key::Space, Key::Char('.'), Key::Char('.')]),
            (Want::Undo, [Key::Char('u'), Key::Char('z'), Key::Char('z')]),
        ];
        self.hold -= dt;
        let (mut want, mut fresh) = (None, false);
        for (w, keys) in KEYS {
            if keys.iter().any(|&k| input.pressed(k)) { (want, fresh) = (Some(w), true); }
            else if want.is_none() && keys.iter().any(|&k| input.repeating(k)) { want = Some(w); }
        }
        if self.rest {
            if want.is_some() && !fresh { return None; }
            self.rest = false;
        }
        if want.is_some() && fresh { self.hold = 0.28; }
        else if want.is_some() && self.hold <= 0.0 { self.hold = 0.12; }
        else { return None; }
        want
    }

    /// A tick inside a room.
    fn play(&mut self, input: &Input, dt: f32) {
        let want = self.wanted(input, dt);
        match self.mode {
            Mode::Play => match want {
                _ if input.pressed(Key::Enter) => self.mode = Mode::Run(RUN),
                Some(Want::Step(act)) => self.advance(act, true),
                Some(Want::Undo) => self.undo(),
                None => {}
            },
            Mode::Run(s) => {
                if let Some(Want::Undo) = want { self.undo(); }
                else if s + dt >= RUN {
                    self.mode = Mode::Run(0.0);
                    self.advance(Act::Wait, false);
                } else { self.mode = Mode::Run(s + dt); }
            }
            Mode::Rewind(s) => {
                // Any key cuts the way back short.
                let t = if s >= 0.0 && input.any_pressed() { BACK } else { s + dt };
                if s < 0.0 && t >= 0.0 {
                    self.audio.stop(1);
                    self.voices = 0;
                    self.audio.play(&self.snd.rewind, 0.45);
                }
                if t < BACK { self.mode = Mode::Rewind(t); return; }
                let life = std::mem::take(&mut self.live);
                self.lives.push(life);
                self.rest = true;
                self.replay();
                let ((ox, oy), (x, y)) = (self.origin(), self.spot(self.lv.start));
                sparks(&mut self.particles, ox as f32 + x, oy as f32 + y, 18, 60.0, 0.5, PALE);
                self.audio.play(&self.snd.born, 0.5);
            }
            Mode::Solved(s) => {
                if s < 0.15 && s + dt >= 0.15 {
                    let (ox, oy) = self.origin();
                    for n in 0..self.lv.rings.len() {
                        let (x, y) = self.spot(self.lv.rings[n]);
                        sparks(&mut self.particles, ox as f32 + x, oy as f32 + y, 26, 80.0, 0.9, if n & 1 == 0 { GOLD } else { BRIGHT });
                    }
                }
                self.mode = Mode::Solved(s + dt);
                if s > 0.3 && (input.pressed(Key::Enter) || input.pressed(Key::Space)) { self.next(); }
                else if let Some(Want::Undo) = want { self.undo(); }
            }
            Mode::Spent => if let Some(Want::Undo) = want { self.undo(); },
            Mode::Title | Mode::End(_) => {}
        }
    }

    /// Move what is drawn towards what is true: the selves, the doors and
    /// the mark on the timeline.
    fn animate(&mut self, dt: f32) {
        let who = self.shown().to_vec();
        let power = self.lv.power(&who);
        for at in 0..self.ajar.len() {
            if !matches!(self.lv.tiles[at], Tile::Door(_)) { continue; }
            let to = if self.lv.open(&who, power, at as u8, false) { 1.0 } else { 0.0 };
            self.ajar[at] += (to - self.ajar[at]).clamp(-dt * 9.0, dt * 9.0);
        }
        let back = self.rewound();
        for i in 0..self.seen.len() {
            self.bump[i] = (self.bump[i] - dt).max(0.0);
            let Some(r) = back else {
                let to = self.spot(self.path[self.tick()][i]);
                let (dx, dy) = (to.0 - self.seen[i].0, to.1 - self.seen[i].1);
                let far = (dx * dx + dy * dy).sqrt();
                // A step takes SLIDE seconds; a long way back goes faster.
                let reach = (TILE as f32 / SLIDE).max(far * 14.0) * dt;
                self.seen[i] = if far <= reach { to } else { (self.seen[i].0 + dx / far * reach, self.seen[i].1 + dy / far * reach) };
                continue;
            };
            let (a, k) = (r.floor() as usize, r.fract());
            let b = (a + 1).min(self.tick());
            let (p, q) = (self.spot(self.path[a][i]), self.spot(self.path[b][i]));
            self.seen[i] = (p.0 + (q.0 - p.0) * k, p.1 + (q.1 - p.1) * k);
        }
        let to = back.unwrap_or(self.tick() as f32);
        self.head += (to - self.head) * (dt * 26.0).min(1.0);
    }
}

impl Game for Again {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        self.time += dt;
        self.particles.update(dt);
        let pressed = |c: char| input.pressed(Key::Char(c));
        let quit = pressed('q') || input.pressed(Key::Escape);
        match self.mode {
            Mode::Title => {
                if quit { return Flow::Quit; }
                let n = ROOMS.len();
                let turn = (input.pressed(Key::Right) || pressed('l') || pressed('d') || pressed('n')) as usize
                    + (input.pressed(Key::Left) || pressed('h') || pressed('a') || pressed('p')) as usize * (n - 1);
                if turn > 0 {
                    self.room = (self.room + turn) % n;
                    self.audio.play(&self.snd.step, 0.5);
                }
                if input.pressed(Key::Enter) || input.pressed(Key::Space) { self.go(self.room); }
                return Flow::Continue;
            }
            Mode::End(s) => {
                // Sparks while the last screen is up, four bursts a second.
                let n = (s * 4.0) as u32;
                if ((s + dt) * 4.0) as u32 != n {
                    let (x, y) = (40 + hash(n, 1) % (W as u32 - 80), 30 + hash(n, 2) % 110);
                    sparks(&mut self.particles, x as f32, y as f32, 30, 90.0, 1.2, [GOLD, BRIGHT, PALE][n as usize % 3]);
                }
                self.mode = Mode::End(s + dt);
                if s > 1.0 && input.any_pressed() {
                    self.room = 0;
                    self.particles.list.clear();
                    self.to_title();
                }
                return Flow::Continue;
            }
            _ => {}
        }
        if quit { self.to_title(); return Flow::Continue; }
        if pressed('r') {
            self.lives.clear();
            self.live.clear();
            self.replay();
            self.audio.play(&self.snd.undo, 0.4);
        } else if input.pressed(Key::Backspace) { self.life_back(); }
        else if pressed('n') { self.go((self.room + 1) % ROOMS.len()); }
        else if pressed('p') { self.go((self.room + ROOMS.len() - 1) % ROOMS.len()); }
        else { self.play(input, dt); }
        self.animate(dt);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        f.clear(BG);
        match self.mode {
            Mode::Title => self.draw_title(f),
            Mode::End(s) => self.draw_end(f, s),
            _ => {
                self.draw_room(f);
                self.draw_selves(f);
                if let Mode::Rewind(s) = self.mode {
                    if s >= 0.0 {
                        let k = (PI * s / BACK).sin();
                        tear(f, k, (self.time * 30.0) as u32);
                        ghost(f, W / 2, BOARD_Y + BOARD_H / 2 - 17, "AGAIN", PALE, 0.5 * k, 5);
                    }
                }
                self.particles.draw(f, 0, 0);
                self.draw_words(f);
                self.draw_timeline(f);
            }
        }
    }
}

// ------------------------------------------------------------------ drawing

/// A colour between two: all `a` at 0, all `b` at 1.
fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    let ch = |s: u32| (((a >> s & 255) as f32 * (1.0 - k) + (b >> s & 255) as f32 * k) as u32) << s;
    ch(16) | ch(8) | ch(0)
}

/// Slow at both ends, 0 to 1.
fn ease(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// A number that looks random, the same for the same two numbers.
fn hash(a: u32, b: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9e37_79b1) ^ b.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^ x >> 12
}

/// The sentences of a line of words, one a row.
fn rows(text: &str) -> Vec<String> {
    let parts: Vec<&str> = text.split(". ").collect();
    parts.iter().enumerate().map(|(n, p)| if n + 1 < parts.len() { format!("{p}.") } else { p.to_string() }).collect()
}

/// Words laid over the picture so it shows through: `k` is how much of
/// the colour comes through.
fn ghost(f: &mut Frame, cx: i32, y: i32, s: &str, c: Rgb, k: f32, scale: i32) {
    let (w, h) = (Frame::text_width(s, true, scale), 7 * scale);
    let mut t = Frame::new(w, h);
    t.text_scaled(0, 0, s, 1, true, scale);
    for j in 0..h {
        for i in 0..w {
            if t.get(i, j) == 0 { continue; }
            let (x, y) = (cx - w / 2 + i, y + j);
            let under = f.get(x, y);
            f.put(x, y, mix(under, c, k));
        }
    }
}

/// Time going back: bands of the room slip sideways, more at `k` 1.
fn tear(f: &mut Frame, k: f32, seed: u32) {
    for y in BOARD_Y..BOARD_Y + BOARD_H {
        let r = hash(y as u32 / 3, seed);
        let slip = ((r >> 8) % 11) as i32 - 5;
        let slip = (slip as f32 * k) as i32;
        if r % 5 > 1 || slip == 0 { continue; }
        let row = &mut f.px[(y * f.w) as usize..((y + 1) * f.w) as usize];
        if slip > 0 { row.rotate_right(slip as usize); } else { row.rotate_left(-slip as usize); }
    }
}

/// A burst of sparks two pixels wide.
fn sparks(p: &mut Particles, x: f32, y: f32, n: usize, speed: f32, life: f32, c: Rgb) {
    let from = p.list.len();
    p.burst(x, y, n, speed, life, c);
    for s in &mut p.list[from..] { s.size = 2; }
}

/// A ring of pixels.
fn ring(f: &mut Frame, cx: i32, cy: i32, r: f32, c: Rgb) {
    let n = r.ceil() as i32 + 1;
    for y in -n..=n {
        for x in -n..=n {
            let d = ((x * x + y * y) as f32).sqrt();
            if d <= r + 0.4 && d >= r - 1.1 { f.put(cx + x, cy + y, c); }
        }
    }
}

/// The sign a colour carries besides its hue, so a plate and its door
/// match for eyes that mix up red and green: a dot, a bar, a cross, a box.
fn sign(f: &mut Frame, cx: i32, cy: i32, colour: u8, c: Rgb) {
    match colour {
        0 => f.rect(cx - 1, cy - 1, 2, 2, c),
        1 => f.rect(cx - 3, cy - 1, 6, 2, c),
        2 => {
            f.rect(cx - 3, cy - 1, 6, 2, c);
            f.rect(cx - 1, cy - 3, 2, 6, c);
        }
        _ => {
            f.rect(cx - 3, cy - 3, 6, 1, c);
            f.rect(cx - 3, cy + 2, 6, 1, c);
            f.rect(cx - 3, cy - 3, 1, 6, c);
            f.rect(cx + 2, cy - 3, 1, 6, c);
        }
    }
}

/// A step's mark on the timeline, five pixels square: an arrow the way
/// it went, a dot for standing still.
fn mark(f: &mut Frame, x: i32, y: i32, act: Act, c: Rgb) {
    const UP: [u8; 5] = [0b00100, 0b01110, 0b10101, 0b00100, 0b00100];
    let Act::Go(way) = act else { f.rect(x + 2, y + 2, 1, 1, c); return };
    for j in 0..5 {
        for i in 0..5 {
            if UP[j as usize] >> (4 - i) & 1 == 0 { continue; }
            let (px, py) = match way { 0 => (i, j), 1 => (4 - j, i), 2 => (4 - i, 4 - j), _ => (j, 4 - i) };
            f.put(x + px, y + py, c);
        }
    }
}

/// One self with its middle at (cx, cy). The present self is bright. A
/// past one is pale, shows a little of what is behind it, and carries its
/// number as dots. `foot` lifts the left foot at 1 and the right at 2.
fn figure(f: &mut Frame, cx: i32, cy: i32, face: u8, past: Option<usize>, foot: i32) {
    let (body, shade, k) = if past.is_some() { (PALE, PALE_DARK, 0.8) } else { (BRIGHT, GOLD, 1.0) };
    let mut dot = |x: i32, y: i32, c: Rgb| {
        let under = f.get(cx + x, cy + y);
        f.put(cx + x, cy + y, mix(under, c, k));
    };
    for y in -7..=4 {
        for x in -5..=5i32 {
            // A box with its corners cut off.
            if 5 - x.abs() + (y + 7).min(4 - y) < 2 { continue; }
            dot(x, y, if y >= 2 { shade } else { body });
        }
    }
    for (side, x) in [(1, -4), (2, 2)] {
        for i in 0..3 {
            dot(x + i, 5, shade);
            if foot != side { dot(x + i, 6, shade); }
        }
    }
    let (ex, ey) = [(0, -5), (2, -4), (0, -3), (-2, -4)][face as usize & 3];
    for x in [ex - 2, ex + 1] {
        for i in 0..4 { dot(x + i % 2, ey + i / 2, 0x14161f); }
    }
    if let Some(n) = past {
        for i in 0..=n as i32 { dot(2 * i - n as i32, 0, 0x14161f); }
    }
}

impl Level {
    /// True for a wall with something other than wall beside it: the
    /// only walls that are drawn.
    fn edge(&self, x: i32, y: i32) -> bool {
        (-1..=1).any(|dy| (-1..=1).any(|dx| self.tile(x + dx, y + dy) != Tile::Wall))
    }
}

impl Again {
    fn draw_room(&self, f: &mut Frame) {
        let (ox, oy) = self.origin();
        let who = self.shown();
        // The gates shimmer and a ring with someone on it beats, both in
        // slow steps: a still room sends nothing to the terminal.
        let beat = (self.time * 8.0) as i32;
        for ty in 0..self.lv.h {
            for tx in 0..self.lv.w {
                let (x, y, at) = (ox + tx * TILE, oy + ty * TILE, (ty * self.lv.w + tx) as u8);
                let tile = self.lv.tiles[at as usize];
                if tile == Tile::Wall {
                    if !self.lv.edge(tx, ty) { continue; }
                    f.rect(x, y, TILE, TILE, WALL);
                    f.rect(x, y, TILE, 2, WALL_LIT);
                    if self.lv.tile(tx, ty + 1) != Tile::Wall { f.rect(x, y + TILE - 5, TILE, 5, WALL_DARK); }
                    continue;
                }
                let floor = FLOOR[((tx + ty) & 1) as usize];
                f.rect(x, y, TILE, TILE, floor);
                let (cx, cy) = (x + TILE / 2, y + TILE / 2);
                // A door or a gate fills the gap in a wall: `flat` when the
                // wall runs left to right.
                let flat = self.lv.tile(tx - 1, ty) == Tile::Wall && self.lv.tile(tx + 1, ty) == Tile::Wall;
                match tile {
                    Tile::Ring => {
                        let on = who.contains(&at);
                        ring(f, cx, cy, 7.0, if on { GOLD } else { 0xdfe6ff });
                        if on { ring(f, cx, cy, 9.0, mix(floor, GOLD, if beat & 3 == 0 { 0.75 } else { 0.4 })); }
                    }
                    Tile::Plate(c) => {
                        let (hue, down) = (HUES[c as usize], who.contains(&at));
                        f.rect(x + 3, y + 3, 14, 14, mix(hue, BG, 0.55));
                        f.rect(x + 4, y + 4, 12, 12, if down { hue } else { mix(hue, BG, 0.72) });
                        if !down { f.rect(x + 4, y + 4, 12, 1, mix(hue, 0xffffff, 0.25)); }
                        sign(f, cx, cy, c, if down { 0xffffff } else { mix(hue, 0xffffff, 0.35) });
                    }
                    Tile::Door(c) => {
                        // Two leaves that pull back into the wall.
                        let (hue, len) = (HUES[c as usize], 2 + (8.0 * (1.0 - self.ajar[at as usize])).round() as i32);
                        let dark = mix(hue, BG, 0.45);
                        f.rect(x, y, TILE, TILE, mix(floor, hue, 0.16));
                        if flat {
                            f.rect(x, y + 5, len, 10, hue);
                            f.rect(x + TILE - len, y + 5, len, 10, hue);
                            f.rect(x, y + 13, len, 2, dark);
                            f.rect(x + TILE - len, y + 13, len, 2, dark);
                        } else {
                            f.rect(x + 5, y, 10, len, hue);
                            f.rect(x + 5, y + TILE - len, 10, len, hue);
                            f.rect(x + 13, y, 2, len, dark);
                            f.rect(x + 13, y + TILE - len, 2, len, dark);
                        }
                        if len == 10 { sign(f, cx, cy, c, 0xffffff); }
                    }
                    Tile::Pale | Tile::Bright => {
                        // A curtain of light in the colour of the self it lets through.
                        let (lit, low) = if tile == Tile::Pale { (PALE, mix(floor, PALE, 0.35)) } else { (BRIGHT, mix(floor, GOLD, 0.45)) };
                        for n in 0..3 {
                            for j in 0..TILE {
                                let c = if (j + beat * (n - 1) + n * 2).rem_euclid(6) < 3 { lit } else { low };
                                let (px, py) = if flat { (x + j, y + 5 + n * 4) } else { (x + 5 + n * 4, y + j) };
                                f.rect(px, py, 1 + !flat as i32, 1 + flat as i32, c);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_selves(&self, f: &mut Frame) {
        let ((ox, oy), n) = (self.origin(), self.seen.len());
        for i in 0..n {
            let (x, y) = self.seen[i];
            // Selves on one tile fan out, the present one in front.
            let over = (i + 1..n).filter(|&j| (self.seen[j].0 - x).abs() + (self.seen[j].1 - y).abs() < 4.0).count() as i32;
            let lean = (PI * self.bump[i] / BUMP).sin() * 3.0;
            let (lx, ly) = (WAYS[self.face[i] as usize].0 as f32 * lean, WAYS[self.face[i] as usize].1 as f32 * lean);
            let to = self.spot(self.path[self.tick()][i]);
            let walks = self.rewound().is_some() || (to.0 - x).abs() + (to.1 - y).abs() > 0.5;
            let foot = if walks { 1 + ((self.time * 14.0) as i32 + i as i32 & 1) } else { 0 };
            figure(f, ox + (x + lx).round() as i32 - over * 3, oy + (y + ly).round() as i32 - over * 2,
                self.face[i], (i + 1 < n).then_some(i), foot);
        }
    }

    /// The bar on top, the line of words under the room and the keys.
    fn draw_words(&self, f: &mut Frame) {
        let room = &ROOMS[self.room];
        if self.done >> self.room & 1 == 1 { f.rect(8, 5, 5, 5, GOLD); }
        f.text_big(18, 4, &format!("{:02}  {}", self.room + 1, room.name), TEXT);
        let right = format!("LIFE {} OF {}    STEP {} OF {}", self.lives.len() + 1, room.selves, self.head.round() as usize, room.steps);
        f.text_big(W - 8 - Frame::text_width(&right, true, 1), 4, &right, DIM);
        let (text, c) = match self.mode {
            Mode::Solved(_) => ("SOLVED. ENTER FOR THE NEXT ROOM.", GOLD),
            Mode::Spent => ("NO SELF IS LEFT. U TAKES BACK A STEP, BACKSPACE THE LIFE, R THE WHOLE ROOM.", RED),
            _ => (room.says[(!self.lives.is_empty() && !room.says[1].is_empty()) as usize], TEXT),
        };
        for (n, row) in rows(text).iter().enumerate() { f.text_centered(W / 2, 200 + n as i32 * 10, row, c, true, 1); }
        f.text_centered(W / 2, 262, "ARROWS STEP  SPACE WAITS  ENTER RUNS TIME ON  U UNDOES A STEP  BACKSPACE A LIFE  R THE ROOM  N P ROOMS  Q LEAVES", FAINT, false, 1);
    }

    /// The lives as rows of steps: the past ones, the present one, and
    /// an empty row for each life still to come.
    fn draw_timeline(&self, f: &mut Frame) {
        let room = &ROOMS[self.room];
        let (cw, rh, y0) = (16, 9, 222);
        let x0 = (W - room.steps as i32 * cw) / 2;
        let shown = self.head.round() as usize;
        for row in 0..room.selves {
            let (y, past, later) = (y0 + row as i32 * rh, row < self.lives.len(), row > self.lives.len());
            let c = if later { FAINT } else if past { PALE } else { BRIGHT };
            f.rect(x0 - 11, y + 1, 6, 6, c);
            if later { f.rect(x0 - 10, y + 2, 4, 4, BG); }
            for t in 0..room.steps {
                let x = x0 + t as i32 * cw;
                f.rect(x + 1, y, cw - 2, rh - 1, if later { 0x0f121b } else { 0x141826 });
                let act = if past { self.lives[row].get(t) } else { self.live.get(t).filter(|_| !later) };
                let Some(&act) = act else { continue };
                let stopped = t < shown && t < self.stops.len() && self.stops[t] >> row & 1 == 1;
                let c = if stopped { RED } else if !past { BRIGHT } else if t < shown { PALE } else { PALE_DARK };
                mark(f, x + cw / 2 - 2, y + 2, act, c);
            }
        }
        let x = x0 + (self.head * cw as f32).round() as i32;
        f.rect(x, y0 - 2, 1, room.selves as i32 * rh + 3, GOLD);
    }

    fn draw_title(&self, f: &mut Frame) {
        // The word and its past selves: they fan out behind it, stay a
        // while, and are pulled back in.
        let t = self.time % 7.0;
        let out = if t < 5.6 { ease(t / 1.4) } else { 1.0 - ease((t - 5.6) / 0.5) };
        for k in (1..=3).rev() {
            let far = k as f32 * out;
            ghost(f, W / 2 - (far * 11.0) as i32, 36 - (far * 5.0) as i32, "AGAIN", PALE, (0.62 - 0.15 * k as f32) * out, 6);
        }
        f.text_centered(W / 2, 36, "AGAIN", BRIGHT, true, 6);
        f.text_centered(W / 2, 92, "A PUZZLE YOU SOLVE WITH YOUR OWN PAST", TEXT, true, 1);
        // One self walks by with its past selves behind it. Each hops
        // where the one before it hopped.
        f.rect(40, 137, W - 80, 1, FAINT);
        let lead = (self.time * 46.0) % (W as f32 + 140.0);
        for i in (0..4).rev() {
            let x = lead - i as f32 * 30.0;
            if x < 46.0 || x > W as f32 - 46.0 { continue; }
            let at = x % 150.0;
            let hop = if at < 26.0 { (PI * at / 26.0).sin() * 8.0 } else { 0.0 };
            figure(f, x as i32, 129 - hop as i32, 1, (i > 0).then(|| i - 1), 1 + ((x / 7.0) as i32 & 1));
        }
        let room = &ROOMS[self.room];
        f.text_centered(W / 2, 156, &format!("<   ROOM {:02}   {}   >", self.room + 1, room.name), TEXT, true, 1);
        let x0 = W / 2 - (ROOMS.len() as i32 * 11 - 3) / 2;
        for n in 0..ROOMS.len() {
            let x = x0 + n as i32 * 11;
            if n == self.room { f.rect(x - 2, 170, 12, 12, BRIGHT); }
            f.rect(x, 172, 8, 8, if self.done >> n & 1 == 1 { GOLD } else { FAINT });
            if self.done >> n & 1 == 0 { f.rect(x + 1, 173, 6, 6, BG); }
        }
        let k = 0.5 + 0.5 * (self.time * 3.0).sin();
        f.text_centered(W / 2, 196, "ENTER STARTS THE ROOM", mix(DIM, GOLD, k), true, 1);
        f.text_centered(W / 2, 222, "LEFT AND RIGHT PICK A ROOM.  Q LEAVES.", DIM, false, 1);
        f.text_centered(W / 2, 232, "IN A ROOM THE ARROWS STEP, SPACE WAITS A STEP AND ENTER LETS THE REST OF THE LIFE RUN OUT.", DIM, false, 1);
        f.text_centered(W / 2, 242, "U TAKES BACK A STEP, BACKSPACE THE LIFE, R THE WHOLE ROOM.", DIM, false, 1);
        let v = format!("V{VERSION}");
        f.text(W - 6 - Frame::text_width(&v, false, 1), H - 9, &v, FAINT);
    }

    fn draw_end(&self, f: &mut Frame, s: f32) {
        self.particles.draw(f, 0, 0);
        f.text_centered(W / 2, 70, "EVERY ROOM IS SOLVED", GOLD, true, 2);
        f.text_centered(W / 2, 104, "ALL THE HELP YOU NEEDED WAS YOU.", TEXT, true, 1);
        for i in 0..4 { figure(f, W / 2 - 45 + i * 30, 150, 2, (i > 0).then(|| i as usize - 1), 0); }
        if s > 1.0 { f.text_centered(W / 2, 200, "PRESS A KEY", DIM, false, 1); }
    }
}

fn again() -> Again {
    let mut game = Again::new();
    (game.audio, game.voices) = (Audio::open(), 0);
    match std::env::var("AGAIN_START").ok().and_then(|s| s.trim().parse::<usize>().ok()) {
        Some(n) => game.go(n.clamp(1, ROOMS.len()) - 1),
        None => game.music(),
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // AGAIN_BENCH=<frames> draws a room that many times with no terminal
    // and prints the time one frame takes.
    if let Ok(n) = std::env::var("AGAIN_BENCH") {
        let n: u32 = n.parse().unwrap_or(600);
        let mut game = Again::new();
        game.go(ROOMS.len() - 3);
        let (mut f, input) = (Frame::new(W, H), Input::new());
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            game.update(&input, 1.0 / 60.0);
            game.draw(&mut f);
        }
        eprintln!("{:.3} ms a frame at {}x{} over {} frames", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n);
        return;
    }
    let mut game = again();
    run(&mut game, Config { width: W, height: H, fps: 60 });
}

#[cfg(target_arch = "wasm32")]
funkey::web!(again(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const DT: f32 = 1.0 / 60.0;

    /// A set of places for the solver: the past selves sorted, since they
    /// are alike, then the present self; unused slots are 255.
    type Spot = [u8; MOST];

    /// The quickest way for `k` selves to solve a room within `limit`
    /// steps: what each presses, the present self last. It tries every
    /// set of places the selves can be in together, a step at a time, so
    /// the first answer found is the shortest there is.
    fn solve(lv: &Level, k: usize, limit: usize) -> Option<Vec<Vec<Act>>> {
        let mut start: Spot = [255; MOST];
        start[..k].fill(lv.start);
        if lv.won(&start[..k]) { return Some(vec![Vec::new(); k]); }
        // For each set of places: the set before it, what each self there
        // did, and which of them each self here was.
        let mut from: HashMap<Spot, (Spot, [Act; MOST], [u8; MOST])> = HashMap::new();
        from.insert(start, (start, [Act::Wait; MOST], [255; MOST]));
        let mut layer = vec![start];
        for _ in 0..limit {
            let mut next = Vec::new();
            for s in &layer {
                let power = lv.power(&s[..k]);
                // What each self can do that is not stopped, and where it ends.
                let opts: Vec<Vec<(Act, u8)>> = (0..k).map(|i| {
                    let mut o = vec![(Act::Wait, s[i])];
                    o.extend((0..4).filter_map(|way| lv.walk(&s[..k], power, s[i], way, i + 1 == k).map(|to| (Act::Go(way), to))));
                    o
                }).collect();
                let mut pick = [0usize; MOST];
                loop {
                    let mut order = [255u8; MOST];
                    for (i, o) in order.iter_mut().enumerate().take(k) { *o = i as u8; }
                    order[..k - 1].sort_by_key(|&i| opts[i as usize][pick[i as usize]].1);
                    let (mut child, mut acts) = ([255u8; MOST], [Act::Wait; MOST]);
                    for i in 0..k {
                        child[i] = opts[order[i] as usize][pick[order[i] as usize]].1;
                        acts[i] = opts[i][pick[i]].0;
                    }
                    if let std::collections::hash_map::Entry::Vacant(e) = from.entry(child) {
                        e.insert((*s, acts, order));
                        if lv.won(&child[..k]) { return Some(plan(&from, child, k)); }
                        next.push(child);
                    }
                    // The next choice, counted like a number with a digit a self.
                    if !(0..k).any(|i| { pick[i] += 1; if pick[i] < opts[i].len() { true } else { pick[i] = 0; false } }) { break; }
                }
            }
            layer = next;
        }
        None
    }

    /// Walk an answer back from its end to the start, and give each self
    /// its own run of acts.
    fn plan(from: &HashMap<Spot, (Spot, [Act; MOST], [u8; MOST])>, end: Spot, k: usize) -> Vec<Vec<Act>> {
        let mut lives = vec![Vec::new(); k];
        let mut seat: Vec<usize> = (0..k).collect();
        let mut at = end;
        loop {
            let (before, acts, order) = from[&at];
            if order[0] == 255 { break; }
            for (life, s) in lives.iter_mut().zip(seat.iter_mut()) {
                *s = order[*s] as usize;
                life.push(acts[*s]);
            }
            at = before;
        }
        for l in &mut lives {
            l.reverse();
            while l.last() == Some(&Act::Wait) { l.pop(); }
        }
        lives
    }

    fn key_of(act: Act) -> Key {
        match act { Act::Wait => Key::Space, Act::Go(0) => Key::Up, Act::Go(1) => Key::Right, Act::Go(2) => Key::Down, Act::Go(_) => Key::Left }
    }

    /// A life as text: `.` for a wait, then U R D L.
    fn keys(life: &[Act]) -> String {
        life.iter().map(|a| match a { Act::Wait => '.', Act::Go(w) => ['U', 'R', 'D', 'L'][*w as usize] }).collect()
    }

    /// A key pressed and let go, through the game's own key reading.
    fn tap(game: &mut Again, key: Key) {
        let mut input = Input::new();
        input.inject(key);
        game.update(&input, DT);
        input.release_all();
        game.update(&input, DT);
    }

    /// Let time pass with no key until `done` says so.
    fn wait(game: &mut Again, done: impl Fn(&Again) -> bool) {
        let input = Input::new();
        for _ in 0..3000 {
            if done(game) { return; }
            game.update(&input, DT);
        }
        panic!("waited too long in {}, mode {:?}", ROOMS[game.room].name, game.mode);
    }

    /// Play an answer with the keys: each life's steps, then Enter.
    fn play(game: &mut Again, answer: &[Vec<Act>]) {
        for (i, life) in answer.iter().enumerate() {
            for act in life { tap(game, key_of(*act)); }
            if matches!(game.mode, Mode::Solved(_)) { return; }
            if game.mode == Mode::Play { tap(game, Key::Enter); }
            if i + 1 < answer.len() { wait(game, |g| g.mode == Mode::Play && g.lives.len() == i + 1); }
            else { wait(game, |g| matches!(g.mode, Mode::Solved(_) | Mode::Spent)); }
        }
    }

    fn game_in(name: &str) -> Again {
        let mut game = Again::new();
        game.go(ROOMS.iter().position(|r| r.name == name).unwrap());
        game
    }

    #[test]
    fn every_room_is_solved_with_the_keys() {
        let mut game = Again::new();
        for (n, room) in ROOMS.iter().enumerate() {
            let answer = solve(&Level::parse(room.map), room.selves, room.steps).unwrap_or_else(|| panic!("{} has no answer", room.name));
            game.go(n);
            play(&mut game, &answer);
            assert!(matches!(game.mode, Mode::Solved(_)), "{} was not solved, mode {:?}", room.name, game.mode);
        }
        assert_eq!(game.done.count_ones() as usize, ROOMS.len());
        wait(&mut game, |g| matches!(g.mode, Mode::Solved(s) if s > 0.4));
        tap(&mut game, Key::Enter);
        assert!(matches!(game.mode, Mode::End(_)), "the last room leads to the end, not {:?}", game.mode);
    }

    #[test]
    fn no_room_can_be_solved_with_a_self_fewer() {
        for room in ROOMS {
            assert!((1..=MOST).contains(&room.selves) && room.steps <= 22, "{}", room.name);
            if room.selves > 1 {
                assert!(solve(&Level::parse(room.map), room.selves - 1, room.steps).is_none(), "{} needs fewer selves", room.name);
            }
        }
    }

    #[test]
    fn a_room_fits_the_screen_and_its_words_fit_two_rows() {
        for room in ROOMS {
            let lv = Level::parse(room.map);
            assert!(lv.w * TILE <= W - 40 && lv.h * TILE <= BOARD_H && lv.w * lv.h < 255, "{}", room.name);
            for says in room.says {
                let rows = rows(says);
                assert!(rows.len() <= 2 && rows.iter().all(|r| r.len() <= 78), "{}: {:?}", room.name, rows);
            }
        }
    }

    #[test]
    fn a_door_is_open_while_its_plate_or_its_doorway_is_stood_on() {
        let lv = Level::parse(&["#######", "#@a.A.#", "#######"]);
        let (start, plate, before, door, after) = (8, 9, 10, 11, 12);
        assert_eq!(lv.start, start);
        // Nobody on the plate: the door stops a step.
        assert_eq!(lv.step(&[before], &[Act::Go(1)]), (vec![before], 1));
        // One on the plate, one through the door.
        assert_eq!(lv.step(&[plate, before], &[Act::Wait, Act::Go(1)]), (vec![plate, door], 0));
        // The one on the plate leaves as the other steps in: both steps
        // are taken in the room as it was.
        assert_eq!(lv.step(&[plate, before], &[Act::Go(3), Act::Go(1)]), (vec![start, door], 0));
        // The doorway keeps the door open for the next one, plate or no plate.
        assert_eq!(lv.step(&[door, before], &[Act::Wait, Act::Go(1)]), (vec![door, door], 0));
        assert_eq!(lv.step(&[after, before], &[Act::Wait, Act::Go(1)]), (vec![after, before], 2));
    }

    #[test]
    fn each_gate_lets_one_kind_of_self_through() {
        let lv = Level::parse(&["#####", "#:@!#", "#####"]);
        // Two selves on the start: the first is a past self, the last the present one.
        assert_eq!(lv.step(&[7, 7], &[Act::Go(3), Act::Go(3)]), (vec![6, 7], 2), "the pale gate stops the present self");
        assert_eq!(lv.step(&[7, 7], &[Act::Go(1), Act::Go(1)]), (vec![7, 8], 1), "the bright gate stops a past self");
    }

    #[test]
    fn a_stopped_step_is_lost_and_the_rest_still_come() {
        let mut game = game_in("NOT YOU");
        for _ in 0..5 { tap(&mut game, Key::Right); }
        // Two steps to the gate, three lost against it.
        assert_eq!((game.lv.xy(game.path[5][0]), &game.stops[..]), ((4, 2), &[0, 0, 1, 1, 1][..]));
        tap(&mut game, Key::Enter);
        wait(&mut game, |g| g.mode == Mode::Play && g.lives.len() == 1);
        tap(&mut game, Key::Enter);
        wait(&mut game, |g| g.mode != Mode::Play && !matches!(g.mode, Mode::Run(_)));
        // The same five keys take the past self through its gate to the ring.
        assert!(matches!(game.mode, Mode::Solved(_)), "{:?}", game.mode);
        assert_eq!(game.lv.xy(game.path[5][0]), (7, 2));
    }

    #[test]
    fn taking_back_goes_across_lives() {
        let mut game = game_in("TWICE");
        tap(&mut game, Key::Down);
        tap(&mut game, Key::Down);
        tap(&mut game, Key::Enter);
        wait(&mut game, |g| g.mode == Mode::Play && g.lives.len() == 1);
        tap(&mut game, Key::Right);
        assert_eq!((game.lives.len(), game.live.len()), (1, 1));
        tap(&mut game, Key::Char('u'));
        assert_eq!((game.lives.len(), game.live.len()), (1, 0));
        // One more goes back into the first life, to where its keys ended.
        tap(&mut game, Key::Char('u'));
        assert_eq!((game.lives.len(), game.live.len(), game.path.len(), game.mode), (0, 2, 3, Mode::Play));
        tap(&mut game, Key::Char('u'));
        assert_eq!(game.live.len(), 1);
        // Backspace takes the life back, R the room.
        tap(&mut game, Key::Enter);
        wait(&mut game, |g| g.mode == Mode::Play && g.lives.len() == 1);
        tap(&mut game, Key::Right);
        tap(&mut game, Key::Backspace);
        assert_eq!((game.lives.len(), game.live.len()), (1, 0));
        tap(&mut game, Key::Backspace);
        assert_eq!((game.lives.len(), game.live.len()), (0, 0));
    }

    #[test]
    fn the_last_life_ends_with_no_self_left_and_can_be_taken_back() {
        let mut game = game_in("TWICE");
        tap(&mut game, Key::Enter);
        wait(&mut game, |g| g.mode == Mode::Play && g.lives.len() == 1);
        tap(&mut game, Key::Up);
        tap(&mut game, Key::Enter);
        wait(&mut game, |g| g.mode == Mode::Spent);
        tap(&mut game, Key::Char('u'));
        assert_eq!((game.mode, game.lives.len(), game.live.len(), game.path.len()), (Mode::Play, 1, 1, 2));
    }

    #[test]
    fn a_key_kept_down_through_the_rewind_takes_no_step_in_the_new_life() {
        let mut game = game_in("THEN AND NOW");
        let mut input = Input::new();
        for _ in 0..600 {
            input.clear_pressed();
            input.inject(Key::Left);
            game.update(&input, DT);
        }
        assert_eq!((game.mode, game.lives.len(), game.live.len()), (Mode::Play, 1, 0));
        input.release_all();
        game.update(&input, DT);
        tap(&mut game, Key::Left);
        assert_eq!(game.live.len(), 1);
    }

    #[test]
    fn every_screen_draws() {
        let (mut game, mut f) = (Again::new(), Frame::new(W, H));
        game.draw(&mut f);
        for n in 0..ROOMS.len() {
            game.go(n);
            game.draw(&mut f);
        }
        game.mode = Mode::Rewind(BACK / 2.0);
        game.draw(&mut f);
        game.mode = Mode::End(2.0);
        game.draw(&mut f);
    }

    /// Not a check: shows how the rooms are solved, or films one.
    /// `AGAIN_TRY=<file>` reports on rooms being drawn up, parted by an
    /// empty line: `= NAME <most selves>`, then the map.
    /// `AGAIN_FILM=<folder>,<room>,<every>` plays a room's answer and
    /// saves every so-many-th frame; room 0 is the title.
    /// With neither, each room's answer is printed.
    #[test]
    #[ignore]
    fn the_bot_plays() {
        if let Ok(film) = std::env::var("AGAIN_FILM") {
            let p: Vec<&str> = film.split(',').collect();
            let (room, every): (usize, usize) = (p[1].parse().unwrap(), p[2].parse().unwrap());
            let (mut game, mut f, mut n) = (Again::new(), Frame::new(W, H), 0);
            let mut shoot = |game: &mut Again, key: Option<Key>, frames: usize| {
                let mut input = Input::new();
                for i in 0..frames {
                    if let (Some(k), 0) = (key, i) { input.inject(k); } else { input.release_all(); }
                    game.update(&input, DT);
                    n += 1;
                    if n % every == 0 {
                        game.draw(&mut f);
                        std::fs::write(format!("{}/f-{:04}.ppm", p[0], n / every), f.to_ppm()).unwrap();
                    }
                }
            };
            if room == 0 { shoot(&mut game, None, 400); return; }
            game.go(room - 1);
            let r = &ROOMS[room - 1];
            let answer = solve(&Level::parse(r.map), r.selves, r.steps).unwrap();
            shoot(&mut game, None, 30);
            for life in &answer {
                for act in life { shoot(&mut game, Some(key_of(*act)), 14); }
                if game.mode == Mode::Play { shoot(&mut game, Some(Key::Enter), 2); }
                while !matches!(game.mode, Mode::Play | Mode::Solved(_) | Mode::Spent) { shoot(&mut game, None, 1); }
                shoot(&mut game, None, 20);
            }
            shoot(&mut game, None, 60);
            return;
        }
        let report = |name: &str, lv: &Level, most: usize, limit: usize| {
            println!("= {name}");
            for k in 1..=most {
                let t0 = std::time::Instant::now();
                match solve(lv, k, limit) {
                    Some(lives) => {
                        println!("   {k} selves: {} steps  ({} ms)", lives.iter().map(|l| l.len()).max().unwrap_or(0), t0.elapsed().as_millis());
                        for l in &lives { println!("        {}", keys(l)); }
                    }
                    None => println!("   {k} selves: no way  ({} ms)", t0.elapsed().as_millis()),
                }
            }
        };
        if let Ok(file) = std::env::var("AGAIN_TRY") {
            for block in std::fs::read_to_string(file).unwrap().split("\n\n") {
                let mut lines = block.lines().filter(|l| !l.trim().is_empty());
                let Some(head) = lines.next() else { continue };
                let (name, most) = head.trim_start_matches("= ").rsplit_once(' ').unwrap();
                let map: Vec<&str> = lines.collect();
                report(name, &Level::parse(&map), most.parse().unwrap(), 30);
            }
            return;
        }
        for room in ROOMS { report(&format!("{} ({} selves, {} steps)", room.name, room.selves, room.steps), &Level::parse(room.map), room.selves, room.steps); }
    }
}
