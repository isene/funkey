//! chomp: a maze of dots, a muncher and four chasers. Pac-Man in spirit,
//! with a maze, art and sound of its own. The chasers hunt by the
//! arcade's rules: they scatter to their corners and chase by turns,
//! each steers for a spot of its own, and a power dot turns the hunt
//! around for a few seconds.
//!
//!     cargo run --release --example chomp
//!
//! The arrows steer, P pauses, Q quits. A turn asked for early is taken
//! at the next opening.
//!
//! `CHOMP_START=<level>` starts at another level; `CHOMP_BENCH=<frames>`
//! plays that many frames with no terminal and prints the time one takes.

use funkey::*;
use Dir::*;

const COLS: i32 = 28;
const ROWS: i32 = 31;
/// Pixels to a tile.
const T: i32 = 8;
/// The maze starts under the score.
const TOP: i32 = 24;
const W: i32 = COLS * T;
const H: i32 = TOP + ROWS * T + 16;
const GAME: &str = "chomp";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";

const YELLOW: Rgb = 0xffe020;
const DOT: Rgb = 0xf0d8a0;
const CYAN: Rgb = 0x40e0e0;
const RED: Rgb = 0xff4040;
const COLORS: [Rgb; 4] = [0xf04040, 0xd070ff, 0x40d8c0, 0xf09030];
const NAMES: [(&str, &str); 4] = [
    ("HUNTER", "COMES STRAIGHT FOR YOU"),
    ("AMBUSH", "AIMS FOR WHERE YOU ARE GOING"),
    ("FLANK", "CLOSES IN FROM THE OTHER SIDE"),
    ("SHY", "TURNS AWAY WHEN IT GETS CLOSE"),
];
/// The walls take a new colour each level.
const WALLS: [Rgb; 6] = [0x4060ff, 0x30b060, 0xb050e0, 0x30b0c0, 0xd05040, 0x8088ff];

/// `#` wall, `.` dot, `o` power dot, `H` the chasers' house, `=` its
/// door. The open row in the middle is the tunnel: it leaves the maze on
/// one side and comes back in on the other.
const MAZE: [&str; ROWS as usize] = [
    "############################",
    "#..........................#",
    "#.###.######.##.######.###.#",
    "#o###.######.##.######.###o#",
    "#.###.######.##.######.###.#",
    "#.........##....##.........#",
    "#.#######.########.#######.#",
    "#.#######.########.#######.#",
    "#.#######.########.#######.#",
    "#.....###..........###.....#",
    "#####.######.##.######.#####",
    "#####.######.##.######.#####",
    "#####........  ........#####",
    "#####.###.###==###.###.#####",
    "#####.###.#HHHHHH#.###.#####",
    "         .#HHHHHH#.         ",
    "#####.###.#HHHHHH#.###.#####",
    "#####.###.########.###.#####",
    "#####........  ........#####",
    "#####.######.##.######.#####",
    "#####.######.##.######.#####",
    "#.....###....  ....###.....#",
    "#.#######.########.#######.#",
    "#.#######.########.#######.#",
    "#.#######.########.#######.#",
    "#.........##....##.........#",
    "#.###.######.##.######.###.#",
    "#o###.######.##.######.###o#",
    "#.###.######.##.######.###.#",
    "#..........................#",
    "############################",
];

fn cell(x: i32, y: i32) -> u8 {
    if !(0..ROWS).contains(&y) { return b'#'; }
    MAZE[y as usize].as_bytes()[x.rem_euclid(COLS) as usize]
}

/// A tile to walk on. The house and its door are closed: a chaser goes
/// through the door by a path of its own.
fn open(x: i32, y: i32) -> bool { matches!(cell(x, y), b'.' | b'o' | b' ') }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Dir { Up, Left, Down, Right }

/// The order a chaser tries its ways out in, when two are as good.
const DIRS: [Dir; 4] = [Up, Left, Down, Right];

impl Dir {
    fn d(self) -> (i32, i32) { match self { Up => (0, -1), Left => (-1, 0), Down => (0, 1), Right => (1, 0) } }
    fn back(self) -> Dir { match self { Up => Down, Left => Right, Down => Up, Right => Left } }
}

/// Where something is in the corridors: on its way from the middle of
/// one tile to the middle of the next, `t` of the way there.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mover { x: i32, y: i32, dir: Dir, t: f32 }

impl Mover {
    /// Its middle, in pixels from the maze's corner.
    fn at(&self) -> (f32, f32) {
        let (dx, dy) = self.dir.d();
        let far = self.t * T as f32;
        ((self.x * T + 4) as f32 + dx as f32 * far, (self.y * T + 4) as f32 + dy as f32 * far)
    }

    /// The tile its middle is in.
    fn tile(&self) -> (i32, i32) { if self.t >= 0.5 { self.ahead(self.dir) } else { (self.x, self.y) } }

    /// The tile next to the one it left, one way.
    fn ahead(&self, d: Dir) -> (i32, i32) { ((self.x + d.d().0).rem_euclid(COLS), self.y + d.d().1) }

    fn free(&self, d: Dir) -> bool { let (x, y) = self.ahead(d); open(x, y) }

    /// It has reached the next tile.
    fn arrive(&mut self) {
        (self.x, self.y) = self.ahead(self.dir);
        self.t -= 1.0;
    }

    fn turn_back(&mut self) {
        (self.x, self.y) = self.ahead(self.dir);
        (self.dir, self.t) = (self.dir.back(), 1.0 - self.t);
    }
}

/// The muncher's first spot, and a chaser's spot on the porch: both
/// between two tiles, on the maze's middle line.
const START: Mover = Mover { x: 14, y: 21, dir: Left, t: 0.5 };
const PORCH: Mover = Mover { x: 14, y: 12, dir: Left, t: 0.5 };
/// The tile an eaten chaser's eyes head for, above the door.
const DOOR: (i32, i32) = (13, 12);
/// The door's middle, the porch above it and the house's middle, in pixels.
const MID: f32 = 112.0;
const PORCH_Y: f32 = 100.0;
const BED_Y: f32 = 124.0;
const BEDS: [f32; 4] = [112.0, 112.0, 96.0, 128.0];
/// Where each chaser goes when they scatter: outside the maze, so it
/// circles the corner.
const CORNERS: [(i32, i32); 4] = [(25, -3), (2, -3), (27, 32), (0, 32)];
/// Where the gem lies, under the house.
const GEM: (f32, f32) = (112.0, 148.0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum State { Home, Leaving, Out, Eyes, Entering }

#[derive(Clone, Copy, Debug)]
struct Foe {
    /// Its place in the corridors, while it is out or on its way home.
    m: Mover,
    /// Its place in pixels, in the house and through the door.
    x: f32, y: f32,
    state: State,
    scared: bool,
}

impl Foe {
    fn at(&self) -> (f32, f32) { if matches!(self.state, State::Out | State::Eyes) { self.m.at() } else { (self.x, self.y) } }

    fn tile(&self) -> (i32, i32) {
        let (x, y) = self.at();
        ((x / T as f32).floor() as i32, (y / T as f32).floor() as i32)
    }

    fn look(&self) -> Dir {
        match self.state { State::Out | State::Eyes => self.m.dir, State::Leaving => Up, _ => Down }
    }
}

/// Tiles a second at full speed.
const FULL: f32 = 9.5;

/// The paces of a level, as parts of full speed: the muncher, the
/// muncher while they are scared, a chaser, a scared chaser, a chaser in
/// the tunnel.
fn paces(level: u32) -> [f32; 5] {
    match level {
        1 => [0.80, 0.90, 0.75, 0.50, 0.40],
        2..=4 => [0.90, 0.95, 0.85, 0.55, 0.45],
        5..=20 => [1.00, 1.00, 0.95, 0.60, 0.50],
        _ => [0.90, 0.90, 0.95, 0.60, 0.50],
    }
}

/// How long a power dot scares them, in seconds, level by level.
const SCARE: [f32; 18] = [6.0, 5.0, 4.0, 3.0, 2.0, 5.0, 2.0, 2.0, 1.0, 5.0, 2.0, 1.0, 1.0, 3.0, 1.0, 1.0, 0.0, 1.0];

fn scare_time(level: u32) -> f32 { SCARE.get(level as usize - 1).copied().unwrap_or(0.0) }

/// Scatter and chase by turns, in seconds, scatter first. After the last
/// one they chase for good.
fn turns(level: u32) -> &'static [f32] {
    match level {
        1 => &[7.0, 20.0, 7.0, 20.0, 5.0, 20.0, 5.0],
        2..=4 => &[7.0, 20.0, 7.0, 20.0, 5.0],
        _ => &[5.0, 20.0, 5.0, 20.0, 5.0],
    }
}

/// Dots a chaser waits for before it leaves the house.
fn waits(level: u32) -> [u32; 4] { match level { 1 => [0, 0, 30, 60], 2 => [0, 0, 0, 50], _ => [0; 4] } }

/// Dots left when the hunter stops scattering and picks up speed.
fn rush(level: u32) -> u32 {
    match level { 1 => 20, 2 => 30, 3..=5 => 40, 6..=8 => 50, 9..=11 => 60, 12..=14 => 80, 15..=18 => 100, _ => 120 }
}

/// What the gem of a level pays, and its colour.
fn gem(level: u32) -> (u32, Rgb) {
    match level {
        1 => (100, 0xff5050), 2 => (300, 0xff9030), 3 | 4 => (500, 0xffe040), 5 | 6 => (700, 0x60e060),
        7 | 8 => (1000, 0x40e0e0), 9 | 10 => (2000, 0x5080ff), 11 | 12 => (3000, 0xc070ff), _ => (5000, 0xffffff),
    }
}

/// Move a value a step toward a goal. True once it is there.
fn toward(v: &mut f32, goal: f32, step: f32) -> bool {
    if (goal - *v).abs() <= step { *v = goal; return true; }
    *v += step * (goal - *v).signum();
    false
}

/// The walls as one white picture: a line three pixels inside every
/// wall, so a corridor looks fourteen pixels wide.
fn wall_art() -> Sprite {
    let (w, h) = (W, ROWS * T);
    let wall = |x: i32, y: i32| cell(x.div_euclid(T), y.div_euclid(T)) == b'#';
    let inside = |x: i32, y: i32| (-3..=3).all(|dy| (-3..=3).all(|dx| wall(x + dx, y + dy)));
    let mut px = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            if inside(x, y) && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| !inside(x + dx, y + dy)) {
                px[(y * w + x) as usize] = 0xffff_ffff;
            }
        }
    }
    Sprite { w, h, px }
}

/// How many tiles it is from each tile to the door, by the corridors.
fn way_home() -> Vec<u16> {
    let key = |(x, y): (i32, i32)| (y * COLS + x) as usize;
    let mut far = vec![u16::MAX; (COLS * ROWS) as usize];
    let mut queue = std::collections::VecDeque::from([DOOR]);
    far[key(DOOR)] = 0;
    while let Some((x, y)) = queue.pop_front() {
        let here = Mover { x, y, dir: Up, t: 0.0 };
        for d in DIRS {
            let next = here.ahead(d);
            if here.free(d) && far[key(next)] == u16::MAX {
                far[key(next)] = far[key((x, y))] + 1;
                queue.push_back(next);
            }
        }
    }
    far
}

/// A picture in one colour.
fn painted(s: &Sprite, c: Rgb) -> Sprite {
    Sprite { w: s.w, h: s.h, px: s.px.iter().map(|&p| if p == 0 { 0 } else { 0xff00_0000 | c }).collect() }
}

/// A chaser, with its feet out and in.
fn bug_art(c: Rgb) -> [Sprite; 2] {
    let top = ["..X.......X..", "...X.....X...", "....XXXXX....", "..XXXXXXXXX..", ".XXXXXXXXXXX.", ".XXXXXXXXXXX.",
        "XXXXXXXXXXXXX", "XXXXXXXXXXXXX", "XXXXXXXXXXXXX", ".XXXXXXXXXXX.", "..XXXXXXXXX.."];
    let feet = [["..XX..X..XX..", ".XX...X...XX."], ["...XX.X.XX...", "...X..X..X..."]];
    feet.map(|f| {
        let rows: Vec<&str> = top.iter().chain(f.iter()).copied().collect();
        Sprite::from_rows(&rows, &[('X', c)])
    })
}

fn gem_art(c: Rgb) -> Sprite {
    Sprite::from_rows(&["..XXXXX..", ".XooXXXX.", "XXoXXXXXX", "XXXXXXXXX", ".XXXXXXX.", "..XXXXX..", "...XXX...", "....X...."],
        &[('X', c), ('o', WHITE)])
}

/// The muncher: a disc with a wedge cut out, `gape` radians to each side
/// of the way it faces, and an eye above the mouth.
fn disc(f: &mut Frame, cx: i32, cy: i32, dir: Dir, gape: f32, c: Rgb) {
    let (fx, fy) = dir.d();
    for dy in -6..=6i32 {
        for dx in -6..=6i32 {
            if dx * dx + dy * dy > 40 { continue; }
            let (along, across) = ((dx * fx + dy * fy) as f32, (dx * fy - dy * fx).abs() as f32);
            if across.atan2(along) >= gape { f.put(cx + dx, cy + dy, c); }
        }
    }
    if gape > 1.2 { return; }
    if fy == 0 { f.rect(cx - (fx < 0) as i32, cy - 4, 2, 2, BLACK); } else { f.rect(cx + 2, cy - (fy < 0) as i32, 2, 2, BLACK); }
}

/// A chaser's eyes, looking the way it goes. `x`, `y` is its corner.
fn eyes(f: &mut Frame, x: i32, y: i32, look: Dir) {
    let (ox, oy) = match look { Up => (1, 0), Left => (0, 1), Down => (1, 2), Right => (2, 1) };
    for ex in [2, 7] {
        f.rect(x + ex + 1, y + 3, 2, 4, WHITE);
        f.rect(x + ex, y + 4, 4, 2, WHITE);
        f.rect(x + ex + ox, y + 3 + oy, 2, 2, 0x2030c0);
    }
}

/// A scared chaser's face.
fn fear(f: &mut Frame, x: i32, y: i32, c: Rgb) {
    f.rect(x + 3, y + 4, 2, 2, c);
    f.rect(x + 8, y + 4, 2, 2, c);
    for k in 0..5 { f.put(x + 2 + 2 * k, y + 9, c); }
    for k in 0..4 { f.put(x + 3 + 2 * k, y + 8, c); }
}

struct Art { flash: Sprite, walls: Sprite, bug: [[Sprite; 2]; 4], scared: [[Sprite; 2]; 2], gem: Sprite }

struct Sounds { dot: [Sample; 2], big: Sample, ate: Sample, die: Sample, gem: Sample, life: Sample, start: Sample, clear: Sample, scared: Sample }

impl Sounds {
    fn new() -> Sounds {
        let (sq, tri) = (Wave::Square, Wave::Triangle);
        let mut die = Sample::sweep(tri, 760.0, 520.0, 0.11, 0.4);
        for k in 1..10 {
            let hz = 760.0 - 60.0 * k as f32;
            die = die.then(&Sample::sweep(tri, hz, hz - 240.0, 0.11, 0.4));
        }
        Sounds {
            dot: [Sample::sweep(tri, 360.0, 620.0, 0.055, 0.35), Sample::sweep(tri, 620.0, 360.0, 0.055, 0.35)],
            big: Sample::sweep(sq, 180.0, 90.0, 0.25, 0.3),
            ate: Sample::sweep(sq, 250.0, 1500.0, 0.4, 0.3),
            die: die.then(&Sample::noise(0.25, 0.3)),
            gem: Sample::tone(tri, 880.0, 0.07, 0.4).then(&Sample::tone(tri, 1320.0, 0.14, 0.4)),
            life: Tune::parse("480 c5/8 e5/8 g5/8 c6/4", tri, 0.4).render(),
            start: Tune::parse("300 c5/8 e5/8 g5/8 e5/8 c6 g5 d5/8 f5/8 a5/8 f5/8 d6 a5 e5/8 g5/8 b5/8 g5/8 c6/8 e6/8 c7", sq, 0.22).render(),
            clear: Tune::parse("360 g5/8 c6/8 e6/8 g6 e6/8 g6/2", sq, 0.22).render(),
            scared: Sample::sweep(sq, 140.0, 280.0, 0.14, 0.16),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Ready, Play, Ate, Dying, Clear, Over }

/// Points shown where they were won.
struct Pop { x: i32, y: i32, points: u32, t: f32, color: Rgb }

struct Chomp {
    /// A tile's dot: 0 none, 1 a dot, 2 a power dot.
    dots: Vec<u8>,
    left: u32, eaten: u32,
    me: Mover,
    /// The way last asked for.
    want: Option<Dir>,
    moving: bool,
    /// Seconds the muncher stands still: eating takes a moment.
    rest: f32,
    chew: f32,
    foes: [Foe; 4],
    /// Which turn of scatter and chase it is, and how long it has run.
    turn: usize, turn_t: f32,
    /// Seconds they stay scared, and how many were eaten on this power dot.
    fright: f32, chain: u32, caught: usize,
    /// Dots each chaser waits for, dots eaten since the life began, and
    /// seconds since the last dot.
    wait: [u32; 4], since: u32, idle: f32,
    /// Seconds the gem still lies there.
    bonus: f32,
    pops: Vec<Pop>,
    score: u32, high: u32, lives: u32, level: u32, start: u32,
    extra: bool,
    mode: Mode, timer: f32, time: f32,
    /// Tiles to the door from each tile: the way eaten eyes go home.
    home: Vec<u16>,
    rng: Rng, audio: Audio, art: Art, snd: Sounds,
}

impl Chomp {
    fn new() -> Chomp {
        let flash = wall_art();
        let mut game = Chomp {
            dots: Vec::new(), left: 0, eaten: 0, me: START, want: None, moving: true, rest: 0.0, chew: 0.0,
            foes: [Foe { m: PORCH, x: MID, y: BED_Y, state: State::Home, scared: false }; 4],
            turn: 0, turn_t: 0.0, fright: 0.0, chain: 0, caught: 0, wait: [0; 4], since: 0, idle: 0.0, bonus: 0.0,
            pops: Vec::new(), score: 0, high: funkey::scores::best(GAME), lives: 3, level: 1, start: 1, extra: false,
            mode: Mode::Title, timer: 0.0, time: 0.0, rng: Rng::from_time(), audio: Audio::off(),
            art: Art { walls: painted(&flash, WALLS[0]), flash, bug: COLORS.map(bug_art),
                scared: [bug_art(0x3040d0), bug_art(0xe8e8f0)], gem: gem_art(gem(1).1) },
            snd: Sounds::new(), home: way_home(),
        };
        game.fill();
        game.place();
        game
    }

    /// Every dot back in the maze.
    fn fill(&mut self) {
        self.dots = MAZE.iter().flat_map(|r| r.bytes().map(|b| match b { b'.' => 1, b'o' => 2, _ => 0 })).collect();
        self.left = self.dots.iter().filter(|&&d| d > 0).count() as u32;
        self.eaten = 0;
    }

    /// Everyone back where a life starts.
    fn place(&mut self) {
        (self.me, self.want, self.moving, self.rest, self.chew) = (START, None, true, 0.0, 0.0);
        for (i, f) in self.foes.iter_mut().enumerate() {
            *f = Foe { m: PORCH, x: BEDS[i], y: BED_Y, state: if i == 0 { State::Out } else { State::Home }, scared: false };
        }
        (self.turn, self.turn_t, self.fright, self.chain, self.since, self.idle, self.bonus) = (0, 0.0, 0.0, 0, 0, 0.0, 0.0);
        self.audio.stop(1);
    }

    fn start_level(&mut self, level: u32) {
        self.level = level;
        self.fill();
        self.place();
        self.wait = waits(level);
        self.art.walls = painted(&self.art.flash, WALLS[(level as usize - 1) % WALLS.len()]);
        self.art.gem = gem_art(gem(level).1);
        (self.mode, self.timer) = (Mode::Ready, 1.8);
    }

    /// A new game.
    fn begin(&mut self) {
        (self.score, self.lives, self.extra) = (0, 3, false);
        self.pops.clear();
        self.start_level(self.start);
        self.audio.play(&self.snd.start, 1.0);
        self.timer = 2.6;
    }

    fn gain(&mut self, points: u32) {
        self.score += points;
        if !self.extra && self.score >= 10_000 {
            (self.extra, self.lives) = (true, self.lives + 1);
            self.audio.play(&self.snd.life, 1.0);
        }
    }

    fn chasing(&self) -> bool { self.turn % 2 == 1 || self.turn >= turns(self.level).len() }

    /// Move the muncher. It turns the way last asked for at the first
    /// tile that is open that way, and turns back at once. A turn asked
    /// for just past an opening is still taken.
    fn steer(&mut self, dt: f32) {
        if self.rest > 0.0 { self.rest -= dt; return; }
        if let Some(w) = self.want {
            if self.moving && w == self.me.dir.back() { self.me.turn_back(); }
            else if (!self.moving || self.me.t < 0.35) && w != self.me.dir && self.me.free(w) { (self.me.dir, self.moving) = (w, true); }
        }
        if !self.moving { return; }
        let p = paces(self.level);
        self.chew += dt;
        self.me.t += FULL * dt * if self.fright > 0.0 { p[1] } else { p[0] };
        while self.me.t >= 1.0 {
            self.me.arrive();
            if let Some(w) = self.want { if self.me.free(w) { self.me.dir = w; } }
            if !self.me.free(self.me.dir) { (self.me.t, self.moving) = (0.0, false); }
        }
    }

    /// Eat what lies where the muncher is.
    fn eat(&mut self) {
        let (x, y) = self.me.tile();
        let kind = std::mem::take(&mut self.dots[(y * COLS + x) as usize]);
        if kind > 0 {
            (self.left, self.eaten, self.since, self.idle) = (self.left - 1, self.eaten + 1, self.since + 1, 0.0);
            if kind == 1 {
                self.gain(10);
                self.rest = 1.0 / 60.0;
                self.audio.play(&self.snd.dot[(self.eaten % 2) as usize], 1.0);
            } else {
                self.gain(50);
                self.rest = 3.0 / 60.0;
                self.scare();
            }
            if self.eaten == 80 || self.eaten == 180 { self.bonus = 9.5; }
            if self.left == 0 {
                (self.mode, self.timer) = (Mode::Clear, 2.2);
                self.audio.stop(1);
                self.audio.play(&self.snd.clear, 1.0);
                return;
            }
        }
        let (px, py) = self.me.at();
        if self.bonus > 0.0 && (px - GEM.0).abs() < 5.0 && (py - GEM.1).abs() < 5.0 {
            let points = gem(self.level).0;
            self.bonus = 0.0;
            self.gain(points);
            self.pops.push(Pop { x: GEM.0 as i32, y: GEM.1 as i32, points, t: 1.5, color: 0xffb0d0 });
            self.audio.play(&self.snd.gem, 1.0);
        }
    }

    /// A power dot: every chaser turns back, and for a while they are
    /// the ones to be eaten.
    fn scare(&mut self) {
        (self.chain, self.fright) = (0, scare_time(self.level));
        for f in &mut self.foes {
            if f.state == State::Out { f.m.turn_back(); }
            if self.fright > 0.0 && !matches!(f.state, State::Eyes | State::Entering) { f.scared = true; }
        }
        self.audio.play(&self.snd.big, 1.0);
        if self.fright > 0.0 { self.audio.play_loop(1, &self.snd.scared, 1.0); }
    }

    /// Scatter and chase by turns. The clock stands still while they
    /// are scared.
    fn schedule(&mut self, dt: f32) {
        if self.fright > 0.0 {
            self.fright -= dt;
            if self.fright <= 0.0 {
                for f in &mut self.foes { f.scared = false; }
                self.audio.stop(1);
            }
            return;
        }
        let turns = turns(self.level);
        if self.turn >= turns.len() { return; }
        self.turn_t += dt;
        if self.turn_t >= turns[self.turn] {
            (self.turn, self.turn_t) = (self.turn + 1, 0.0);
            for f in &mut self.foes { if f.state == State::Out { f.m.turn_back(); } }
        }
    }

    /// Let the next chaser out of the house: when enough dots are gone,
    /// or when none has been eaten for a while.
    fn release(&mut self, dt: f32) {
        self.idle += dt;
        if self.foes.iter().any(|f| f.state == State::Leaving) { return; }
        let Some(i) = (1..4).find(|&i| self.foes[i].state == State::Home) else { return };
        if self.since >= self.wait[i] || self.idle > if self.level < 5 { 4.0 } else { 3.0 } {
            self.foes[i].state = State::Leaving;
            self.idle = 0.0;
        }
    }

    /// The tile a chaser steers for.
    fn target(&self, i: usize) -> (i32, i32) {
        let f = &self.foes[i];
        let rushing = i == 0 && self.left <= rush(self.level);
        if !self.chasing() && !rushing { return CORNERS[i]; }
        let ((mx, my), (dx, dy)) = (self.me.tile(), self.me.dir.d());
        match i {
            0 => (mx, my),
            1 => (mx + 4 * dx, my + 4 * dy),
            // As far past a spot two tiles ahead of the muncher as the
            // hunter is short of it.
            2 => { let (hx, hy) = self.foes[0].tile(); (2 * (mx + 2 * dx) - hx, 2 * (my + 2 * dy) - hy) }
            _ => { let (x, y) = f.m.tile(); if (x - mx).pow(2) + (y - my).pow(2) > 64 { (mx, my) } else { CORNERS[3] } }
        }
    }

    /// The way a chaser takes at a tile: never back, and of the rest the
    /// one that ends nearest its target. A scared one picks at random,
    /// and eaten eyes take the shortest way home.
    fn choose(&mut self, i: usize) -> Dir {
        let f = self.foes[i];
        if f.state == State::Eyes {
            let far = |d: &Dir| { let (x, y) = f.m.ahead(*d); self.home[(y * COLS + x) as usize] };
            return DIRS.into_iter().filter(|d| f.m.free(*d)).min_by_key(far).unwrap_or(f.m.dir);
        }
        let (mut ways, mut n) = ([Up; 4], 0);
        for d in DIRS { if d != f.m.dir.back() && f.m.free(d) { ways[n] = d; n += 1; } }
        let ways = &ways[..n];
        if f.scared && f.state == State::Out { return self.rng.pick(ways).copied().unwrap_or(f.m.dir.back()); }
        let (gx, gy) = self.target(i);
        let far = |d: &&Dir| { let (x, y) = (f.m.x + d.d().0 - gx, f.m.y + d.d().1 - gy); x * x + y * y };
        ways.iter().min_by_key(far).copied().unwrap_or(f.m.dir.back())
    }

    /// Move a chaser: through the corridors, or in and out of the house.
    fn walk(&mut self, i: usize, dt: f32) {
        let f = self.foes[i];
        match f.state {
            State::Home => self.foes[i].y = BED_Y + 3.0 * (self.time * 5.0 + i as f32 * 2.0).sin(),
            State::Leaving | State::Entering => {
                let leaving = f.state == State::Leaving;
                let step = FULL * T as f32 * dt * if leaving { 0.5 } else { 1.4 };
                let f = &mut self.foes[i];
                if !toward(&mut f.x, MID, step) || !toward(&mut f.y, if leaving { PORCH_Y } else { BED_Y }, step) { return; }
                if leaving { (f.state, f.m) = (State::Out, PORCH); } else { (f.state, f.scared) = (State::Leaving, false); }
            }
            State::Out | State::Eyes => {
                let (p, (tx, ty)) = (paces(self.level), f.m.tile());
                let pace = if f.state == State::Eyes { 1.6 }
                    else if ty == 15 && !(5..=22).contains(&tx) { p[4] }
                    else if f.scared { p[3] }
                    else if i == 0 && self.left <= rush(self.level) / 2 { p[2] + 0.10 }
                    else if i == 0 && self.left <= rush(self.level) { p[2] + 0.05 }
                    else { p[2] };
                let mut m = f.m;
                m.t += FULL * pace * dt;
                while m.t >= 1.0 {
                    m.arrive();
                    if f.state == State::Eyes && (m.x, m.y) == DOOR {
                        let f = &mut self.foes[i];
                        (f.state, f.x, f.y) = (State::Entering, (m.x * T + 4) as f32, PORCH_Y);
                        return;
                    }
                    self.foes[i].m = m;
                    m.dir = self.choose(i);
                }
                self.foes[i].m = m;
            }
        }
    }

    /// The muncher and a chaser on the same spot: one of them is eaten.
    fn meet(&mut self) {
        let (mx, my) = self.me.at();
        for i in 0..4 {
            let f = self.foes[i];
            if f.state != State::Out { continue; }
            let (x, y) = f.at();
            let dx = (x - mx + 112.0).rem_euclid(W as f32) - 112.0;
            if dx * dx + (y - my) * (y - my) > 30.0 { continue; }
            if f.scared {
                let points = 200 << self.chain;
                self.chain += 1;
                self.gain(points);
                (self.foes[i].state, self.foes[i].scared, self.caught) = (State::Eyes, false, i);
                self.pops.push(Pop { x: x as i32, y: y as i32, points, t: 0.6, color: CYAN });
                (self.mode, self.timer) = (Mode::Ate, 0.6);
                self.audio.play(&self.snd.ate, 1.0);
            } else {
                (self.mode, self.timer) = (Mode::Dying, 1.7);
                self.audio.stop(1);
                self.audio.play(&self.snd.die, 1.0);
            }
            return;
        }
    }

    fn play(&mut self, dt: f32) {
        self.steer(dt);
        self.eat();
        if self.mode != Mode::Play { return; }
        self.schedule(dt);
        self.release(dt);
        for i in 0..4 { self.walk(i, dt); }
        self.meet();
        if self.bonus > 0.0 { self.bonus -= dt; }
    }

    fn step(&mut self, dt: f32) {
        self.time += dt;
        self.timer -= dt;
        self.pops.retain_mut(|p| { p.t -= dt; p.t > 0.0 });
        match self.mode {
            Mode::Title => {}
            Mode::Play => self.play(dt),
            Mode::Ready | Mode::Ate => if self.timer <= 0.0 { self.mode = Mode::Play; },
            Mode::Dying => if self.timer <= 0.0 {
                self.lives -= 1;
                if self.lives == 0 {
                    if !cfg!(test) { funkey::scores::record(GAME, self.score); }
                    self.high = self.high.max(self.score);
                    (self.mode, self.timer) = (Mode::Over, 5.0);
                } else {
                    self.place();
                    self.wait = [0, 7, 17, 32];
                    (self.mode, self.timer) = (Mode::Ready, 1.5);
                }
            },
            Mode::Clear => if self.timer <= 0.0 { self.start_level(self.level + 1); },
            Mode::Over => if self.timer <= 0.0 { self.mode = Mode::Title; },
        }
    }

    fn title(&self, f: &mut Frame) {
        f.fancy_text(W / 2, 40, "CHOMP", 4, self.time, (0xffe040, 0xff8020));
        for (i, (name, what)) in NAMES.iter().enumerate() {
            let y = 92 + i as i32 * 30;
            f.blit(&self.art.bug[i][(self.time * 4.0) as usize % 2], 30, y);
            eyes(f, 30, y, Right);
            f.text_big(52, y, name, COLORS[i]);
            f.text(52, y + 10, what, 0xa0a0b8);
        }
        f.rect(71, 219, 2, 2, DOT);
        f.text_big(80, 216, "10", WHITE);
        f.circle(124, 219, 3, DOT);
        f.text_big(134, 216, "50", WHITE);
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 238, "PRESS SPACE", CYAN, true, 1); }
        f.text_centered(W / 2, 260, "ARROWS STEER  P PAUSES  Q QUITS", 0x8080a0, false, 1);
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 6, VERSION, 0x404060);
    }
}

impl Game for Chomp {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        if input.pressed(Key::Char('p')) && self.mode == Mode::Play { return Flow::Pause; }
        for (key, dir) in [(Key::Up, Up), (Key::Left, Left), (Key::Down, Down), (Key::Right, Right)] {
            if input.pressed(key) { self.want = Some(dir); }
        }
        if input.pressed(Key::Space) || input.pressed(Key::Enter) {
            match self.mode { Mode::Title => self.begin(), Mode::Over => self.mode = Mode::Title, _ => {} }
        }
        self.step(dt);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        f.clear(BLACK);
        f.text_big(8, 3, "SCORE", WHITE);
        f.text_big(8, 13, &format!("{:05}", self.score), WHITE);
        f.text_big(W - 8 - Frame::text_width("HIGH", true, 1), 3, "HIGH", WHITE);
        let high = format!("{:05}", self.high.max(self.score));
        f.text_big(W - 8 - Frame::text_width(&high, true, 1), 13, &high, WHITE);
        if self.mode == Mode::Title { self.title(f); return; }

        let flash = self.mode == Mode::Clear && self.timer < 1.6 && (self.timer * 5.0) as i32 % 2 == 0;
        f.blit(if flash { &self.art.flash } else { &self.art.walls }, 0, TOP);
        f.rect(104, TOP + 13 * T + 3, 16, 2, 0xffb0d0);
        for (k, &d) in self.dots.iter().enumerate() {
            let (x, y) = ((k as i32 % COLS) * T, TOP + (k as i32 / COLS) * T);
            if d == 1 { f.rect(x + 3, y + 3, 2, 2, DOT); }
            if d == 2 && (self.time * 4.0) as i32 % 2 == 0 { f.circle(x + 4, y + 4, 3, DOT); }
        }
        if self.bonus > 0.0 { f.blit(&self.art.gem, GEM.0 as i32 - 4, TOP + GEM.1 as i32 - 4); }

        // The chasers: gone while the maze flashes and while the muncher
        // folds up.
        let shown = match self.mode { Mode::Clear | Mode::Over => false, Mode::Dying => self.timer > 1.3, _ => true };
        for (i, foe) in self.foes.iter().enumerate() {
            if !shown || (self.mode == Mode::Ate && i == self.caught) { continue; }
            let (x, y) = foe.at();
            let (x, y) = (x.round() as i32 - 6, TOP + y.round() as i32 - 6);
            let feet = ((self.time * 8.0) as usize + i) % 2;
            for x in [x - W, x, x + W] {
                if matches!(foe.state, State::Eyes | State::Entering) { eyes(f, x, y, foe.look()); }
                else if foe.scared {
                    let pale = self.fright < 2.0 && (self.fright * 5.0) as i32 % 2 == 0;
                    f.blit(&self.art.scared[pale as usize][feet], x, y);
                    fear(f, x, y, if pale { RED } else { 0xffc8b0 });
                } else {
                    f.blit(&self.art.bug[i][feet], x, y);
                    eyes(f, x, y, foe.look());
                }
            }
        }

        let (x, y) = self.me.at();
        let (x, y) = (x.round() as i32, TOP + y.round() as i32);
        match self.mode {
            Mode::Ate | Mode::Over => {}
            Mode::Dying => disc(f, x, y, Up, 0.5 + (1.3 - self.timer).max(0.0) / 1.3 * 2.9, YELLOW),
            _ => for x in [x - W, x, x + W] { disc(f, x, y, self.me.dir, [0.1, 0.5, 0.9, 0.5][(self.chew * 14.0) as usize % 4], YELLOW); },
        }
        for p in &self.pops { f.text_centered(p.x, TOP + p.y - 2, &p.points.to_string(), p.color, false, 1); }

        let word = match self.mode { Mode::Ready => Some(("READY!", YELLOW)), Mode::Over => Some(("GAME OVER", RED)), _ => None };
        if let Some((word, color)) = word {
            let w = Frame::text_width(word, true, 1);
            f.rect(W / 2 - w / 2 - 3, TOP + 18 * T - 1, w + 5, 10, BLACK);
            f.text_centered(W / 2, TOP + 18 * T + 1, word, color, true, 1);
        }

        for k in 0..self.lives.saturating_sub(1) as i32 { disc(f, 14 + k * 16, H - 8, Left, 0.5, YELLOW); }
        f.text_centered(W / 2, H - 11, &format!("LEVEL {}", self.level), 0x8080a0, false, 1);
        f.blit(&self.art.gem, W - 18, H - 12);
    }
}

/// The game as it is played: with sound, from the title or straight
/// into the level `CHOMP_START` names.
fn chomp() -> Chomp {
    let mut game = Chomp::new();
    game.audio = Audio::open();
    if let Some(level) = std::env::var("CHOMP_START").ok().and_then(|s| s.trim().parse::<u32>().ok()) {
        game.start = level.max(1);
        game.begin();
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = Config { width: W, height: H, fps: 60 };
    // CHOMP_BENCH=<frames> plays that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("CHOMP_BENCH") {
        let mut game = Chomp::new();
        game.begin();
        bench(&mut game, &Input::new(), cfg, n.parse().unwrap_or(600));
        return;
    }
    run(&mut chomp(), cfg);
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(chomp(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
impl Chomp {
    /// The way a careful player might ask for: toward the nearest dot, or
    /// a scared chaser, by tiles that no biting chaser is near.
    fn bot(&self) -> Option<Dir> {
        let key = |(x, y): (i32, i32)| (y * COLS + x) as usize;
        let biters: Vec<(i32, i32)> = self.foes.iter()
            .filter(|f| f.state == State::Out && !(f.scared && self.fright > 0.7)).map(|f| f.m.tile()).collect();
        let prey: Vec<(i32, i32)> = self.foes.iter()
            .filter(|f| f.state == State::Out && f.scared && self.fright > 1.5).map(|f| f.m.tile()).collect();
        let near = |(x, y): (i32, i32)| biters.iter().map(|&(bx, by)| (bx - x).abs() + (by - y).abs()).min().unwrap_or(99);
        // Where it can be next, and what to ask for to get there. Close
        // to a tile's middle every way is open; past it, the choice is
        // to go back or to pick a way at the next tile.
        let m = self.me;
        let mut seen = vec![false; (COLS * ROWS) as usize];
        let mut seeds: Vec<((i32, i32), Dir)> = Vec::new();
        let mut from = m;
        if self.moving && m.t >= 0.35 {
            let next = m.ahead(m.dir);
            if near(next) <= 2 { return Some(m.dir.back()); }
            if self.dots[key(next)] > 0 || prey.contains(&next) { return Some(m.dir); }
            seeds.push(((m.x, m.y), m.dir.back()));
            seen[key(next)] = true;
            from = Mover { x: next.0, y: next.1, dir: m.dir, t: 0.0 };
        }
        seen[key((m.x, m.y))] = true;
        for d in DIRS { if from.free(d) && !seen[key(from.ahead(d))] { seeds.push((from.ahead(d), d)); } }
        for (tile, _) in &seeds { seen[key(*tile)] = true; }
        let mut queue: std::collections::VecDeque<((i32, i32), Dir)> = seeds.iter().copied().collect();
        while let Some((tile, ask)) = queue.pop_front() {
            if near(tile) <= 2 { continue; }
            if self.dots[key(tile)] > 0 || prey.contains(&tile) { return Some(ask); }
            let here = Mover { x: tile.0, y: tile.1, dir: Up, t: 0.0 };
            for d in DIRS {
                let n = here.ahead(d);
                if here.free(d) && !seen[key(n)] { seen[key(n)] = true; queue.push_back((n, ask)); }
            }
        }
        seeds.iter().max_by_key(|(tile, _)| near(*tile)).map(|(_, ask)| *ask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// A game in play at a level.
    fn on(level: u32) -> Chomp {
        let mut game = Chomp::new();
        game.start = level;
        game.begin();
        game.mode = Mode::Play;
        game
    }

    fn run(game: &mut Chomp, secs: f32) { for _ in 0..(secs * 60.0) as usize { game.step(DT); } }

    fn held(key: Key) -> Input { let mut i = Input::new(); i.inject(key); i }

    fn every_tile() -> Vec<(i32, i32)> {
        (0..ROWS).flat_map(|y| (0..COLS).map(move |x| (x, y))).filter(|&(x, y)| open(x, y)).collect()
    }

    #[test]
    fn the_maze_has_no_dead_end_and_no_island() {
        for (y, row) in MAZE.iter().enumerate() {
            assert_eq!(row.len() as i32, COLS, "row {y}");
            assert_eq!(row.chars().rev().collect::<String>(), *row, "row {y} is not the same on both sides");
        }
        let tiles = every_tile();
        for &(x, y) in &tiles {
            let here = Mover { x, y, dir: Up, t: 0.0 };
            assert!(DIRS.iter().filter(|d| here.free(**d)).count() >= 2, "a dead end at {x},{y}");
        }
        let mut seen = vec![(START.x, START.y)];
        let mut k = 0;
        while k < seen.len() {
            let here = Mover { x: seen[k].0, y: seen[k].1, dir: Up, t: 0.0 };
            for d in DIRS { if here.free(d) && !seen.contains(&here.ahead(d)) { seen.push(here.ahead(d)); } }
            k += 1;
        }
        assert_eq!(seen.len(), tiles.len());
        assert_eq!(Chomp::new().left, 260);
    }

    #[test]
    fn a_turn_asked_for_early_is_taken_at_the_next_opening() {
        let mut game = on(1);
        game.update(&held(Key::Up), DT);
        assert_eq!(game.want, Some(Up));
        run(&mut game, 1.0);
        // Left along the row to the first way up, then up to the wall.
        assert_eq!((game.me.x, game.me.y, game.me.dir, game.moving), (12, 18, Up, false));
    }

    #[test]
    fn a_wall_stops_the_muncher_until_it_is_turned() {
        let mut game = on(1);
        run(&mut game, 1.5);
        assert_eq!((game.me.x, game.me.y, game.moving), (9, 21, false));
        assert_eq!(game.score, 40);
        game.want = Some(Right);
        game.step(DT);
        assert_eq!((game.me.dir, game.moving), (Right, true));
    }

    #[test]
    fn a_turn_just_past_an_opening_is_still_taken() {
        let mut game = on(1);
        game.me = Mover { x: 12, y: 21, dir: Left, t: 0.3 };
        game.want = Some(Up);
        game.step(DT);
        assert_eq!((game.me.x, game.me.dir), (12, Up));
        // Too far past it: the turn waits for the next opening.
        game.me = Mover { x: 12, y: 21, dir: Left, t: 0.6 };
        game.step(DT);
        assert_eq!(game.me.dir, Left);
    }

    #[test]
    fn the_tunnel_comes_out_on_the_other_side() {
        let mut game = on(1);
        game.me = Mover { x: 1, y: 15, dir: Left, t: 0.0 };
        run(&mut game, 0.5);
        assert!(game.me.x > 20 && game.me.y == 15, "at {:?}", game.me);
    }

    #[test]
    fn each_chaser_steers_for_a_spot_of_its_own() {
        let mut game = on(1);
        game.turn = 1;
        game.me = Mover { x: 9, y: 9, dir: Right, t: 0.0 };
        for f in &mut game.foes { f.state = State::Out; }
        game.foes[0].m = Mover { x: 5, y: 5, dir: Left, t: 0.0 };
        game.foes[3].m = Mover { x: 22, y: 25, dir: Left, t: 0.0 };
        assert_eq!(game.target(0), (9, 9));
        assert_eq!(game.target(1), (13, 9));
        assert_eq!(game.target(2), (17, 13));
        assert_eq!(game.target(3), (9, 9));
        game.foes[3].m = Mover { x: 12, y: 9, dir: Left, t: 0.0 };
        assert_eq!(game.target(3), CORNERS[3]);
        // Scattering, each has its corner, until few dots are left: then
        // the hunter stays on the muncher.
        game.turn = 0;
        for i in 0..4 { assert_eq!(game.target(i), CORNERS[i]); }
        game.left = rush(1);
        assert_eq!(game.target(0), (9, 9));
    }

    #[test]
    fn a_power_dot_turns_the_hunt_around() {
        let mut game = on(1);
        game.me = Mover { x: 1, y: 4, dir: Up, t: 0.0 };
        game.foes[0].m = Mover { x: 6, y: 1, dir: Left, t: 0.0 };
        run(&mut game, 0.2);
        assert!(game.fright > 5.0);
        assert!(game.foes.iter().all(|f| f.scared));
        assert_eq!(game.foes[0].m.dir, Right);
        // The first one eaten pays 200, the next 400.
        game.dots.iter_mut().for_each(|d| *d = 0);
        let before = game.score;
        game.me = game.foes[0].m;
        game.step(DT);
        assert_eq!((game.mode, game.score - before, game.foes[0].state), (Mode::Ate, 200, State::Eyes));
        run(&mut game, 0.7);
        (game.foes[1].state, game.foes[1].m) = (State::Out, Mover { x: 20, y: 29, dir: Left, t: 0.0 });
        game.me = game.foes[1].m;
        let before = game.score;
        game.step(DT);
        assert_eq!(game.score - before, 400);
        // When the time is up nobody is scared.
        game.mode = Mode::Play;
        game.fright = 0.01;
        game.me = Mover { x: 1, y: 29, dir: Right, t: 0.0 };
        game.step(DT);
        assert!(game.foes.iter().all(|f| !f.scared));
    }

    #[test]
    fn eaten_eyes_find_the_door_from_every_tile_and_come_out_again() {
        let mut game = on(1);
        for (x, y) in every_tile() {
            for dir in DIRS {
                let m = Mover { x, y, dir, t: 0.0 };
                if !m.free(dir) { continue; }
                (game.foes[0].state, game.foes[0].m, game.foes[0].scared) = (State::Eyes, m, false);
                let mut steps = 0;
                while game.foes[0].state == State::Eyes && steps < 60 * 20 { game.walk(0, DT); steps += 1; }
                assert_eq!(game.foes[0].state, State::Entering, "eyes from {x},{y} going {dir:?} never got home");
            }
        }
        // Down into the house, and out of it again by the door.
        let mut steps = 0;
        while game.foes[0].state != State::Out && steps < 60 * 3 { game.walk(0, DT); steps += 1; }
        assert_eq!((game.foes[0].state, game.foes[0].at()), (State::Out, (MID, PORCH_Y)));
    }

    #[test]
    fn a_bite_costs_a_life_and_the_last_one_ends_the_game() {
        let mut game = on(1);
        game.foes[0].m = game.me;
        game.step(DT);
        assert_eq!(game.mode, Mode::Dying);
        run(&mut game, 2.0);
        assert_eq!((game.lives, game.mode, game.wait), (2, Mode::Ready, [0, 7, 17, 32]));
        assert_eq!(game.me, START);
        (game.lives, game.mode) = (1, Mode::Play);
        game.foes[0].m = game.me;
        run(&mut game, 2.5);
        assert_eq!(game.mode, Mode::Over);
    }

    #[test]
    fn the_last_dot_ends_the_level() {
        let mut game = on(1);
        game.dots.iter_mut().for_each(|d| *d = 0);
        game.dots[(21 * COLS + 12) as usize] = 1;
        game.left = 1;
        run(&mut game, 0.5);
        assert_eq!(game.mode, Mode::Clear);
        run(&mut game, 2.5);
        assert_eq!((game.level, game.left, game.mode), (2, 260, Mode::Ready));
    }

    #[test]
    fn the_gem_shows_after_eighty_dots_and_pays() {
        let mut game = on(1);
        game.dots[(21 * COLS + 13) as usize] = 1;
        (game.left, game.eaten) = (game.left + 1, 79);
        run(&mut game, 0.1);
        assert!(game.bonus > 9.0);
        game.me = Mover { x: 14, y: 18, dir: Left, t: 0.5 };
        game.step(DT);
        assert_eq!((game.bonus, game.score), (0.0, 110));
    }

    #[test]
    fn the_chasers_leave_the_house_one_by_one() {
        let mut game = on(1);
        game.want = Some(Right);
        run(&mut game, 1.5);
        // The second leaves at once; the third waits for thirty dots, or
        // for four seconds with no dot eaten.
        assert_eq!(game.foes[1].state, State::Out);
        assert_eq!(game.foes[2].state, State::Home);
        game.since = 30;
        run(&mut game, 1.5);
        assert_eq!((game.foes[2].state, game.foes[3].state), (State::Out, State::Home));
        game.me = Mover { x: 26, y: 21, dir: Left, t: 0.0 };
        (game.moving, game.want, game.idle) = (false, None, 4.1);
        game.dots.iter_mut().for_each(|d| *d = 0);
        game.step(DT);
        assert_eq!(game.foes[3].state, State::Leaving);
    }

    #[test]
    fn nobody_leaves_the_corridors_in_ten_minutes_of_play() {
        let mut game = on(1);
        let mut rng = Rng::new(7);
        for n in 0..60 * 600 {
            if n % 20 == 0 { game.want = rng.pick(&DIRS).copied(); }
            if game.mode == Mode::Over { game.begin(); }
            game.step(DT);
            let (x, y) = game.me.tile();
            assert!(open(x, y) && open(game.me.x, game.me.y), "the muncher is in a wall: {:?}", game.me);
            for f in &game.foes {
                if !matches!(f.state, State::Out | State::Eyes) { continue; }
                let (x, y) = f.m.tile();
                assert!(open(x, y) && open(f.m.x, f.m.y), "a chaser is in a wall: {:?}", f.m);
            }
        }
    }

    #[test]
    fn every_screen_draws() {
        let mut f = Frame::new(W, H);
        let mut game = Chomp::new();
        game.draw(&mut f);
        game.begin();
        for mode in [Mode::Ready, Mode::Play, Mode::Ate, Mode::Dying, Mode::Clear, Mode::Over] {
            (game.mode, game.timer, game.fright, game.bonus) = (mode, 0.7, 1.0, 1.0);
            game.foes[1].scared = true;
            game.foes[2].state = State::Eyes;
            game.draw(&mut f);
        }
        // The maze's lines are on the picture, and the muncher is yellow.
        game.mode = Mode::Play;
        game.draw(&mut f);
        assert_eq!(f.get(4, TOP + 12), WALLS[0]);
        assert_eq!(f.get(112, TOP + 172 + 5), YELLOW);
    }

    /// `cargo test --release --example chomp the_bot_plays -- --ignored --nocapture`
    /// plays some games and says how they went.
    /// `CHOMP_FILM=<folder>,<level>,<every>,<frames>` writes pictures of
    /// one game in place of that.
    #[test]
    #[ignore]
    fn the_bot_plays() {
        if let Ok(film) = std::env::var("CHOMP_FILM") {
            let p: Vec<&str> = film.split(',').collect();
            let (every, frames): (usize, usize) = (p[2].parse().unwrap(), p[3].parse().unwrap());
            let (mut game, mut f) = (on(p[1].parse().unwrap()), Frame::new(W, H));
            game.rng = Rng::new(7);
            for n in 1..=every * frames {
                if let Some(ask) = game.bot() { game.want = Some(ask); }
                game.step(DT);
                game.draw(&mut f);
                if n % every == 0 { std::fs::write(format!("{}/f-{:04}.ppm", p[0], n / every), f.to_ppm()).unwrap(); }
            }
            return;
        }
        let mut games = Vec::new();
        for seed in (1..24).step_by(2) {
            let mut game = on(1);
            game.rng = Rng::new(seed);
            let mut eaten = 0;
            while game.mode != Mode::Over && game.time < 1200.0 {
                if let Some(ask) = game.bot() { game.want = Some(ask); }
                let chain = game.chain;
                game.step(DT);
                if game.chain > chain { eaten += 1; }
            }
            games.push((game.level, game.score, (game.time / 60.0) as u32, eaten));
        }
        games.sort();
        for (level, score, minutes, eaten) in &games { println!("level {level:2}  score {score:6}  {minutes:2} min  {eaten:3} chasers eaten"); }
    }
}
