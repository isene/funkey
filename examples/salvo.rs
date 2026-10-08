//! salvo: a tribute to Gradius (Konami, 1985). A ship flies right through
//! seven stages, each ending in a boss: a volcano, stone heads, crystals,
//! living cells, the sun, a high-speed maze and the fortress. Capsules
//! light the power-up bar one step at a time, and a key takes what is
//! lit: speed, missiles, a double shot, the laser, up to four options
//! that follow the ship's path, and a shield. After the seventh stage it
//! all begins again, harder.
//!
//!     cargo run --release --example salvo
//!
//! Arrows fly, Space fires (hold it), Z, X or Enter takes the lit
//! power-up, P pauses, Q quits. `SALVO_START=<stage>` starts at another
//! stage; `SALVO_BENCH=<frames>` times the game with no terminal.
//! Everything here is new: the stages, the ships and the music.

use funkey::*;
use std::collections::VecDeque;
use std::f32::consts::PI;

const W: i32 = 480;
const H: i32 = 270;
/// The bottom of the play area; the power-up bar and the score sit below.
const BOT: i32 = H - 26;
const GAME: &str = "salvo";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.2";
const TEXT: Rgb = 0xf0f0f0;
const GOLD: Rgb = 0xffd040;
/// Moves of the ship between it and its first option, and between options.
const SPACING: usize = 12;
const MAX_OPTIONS: usize = 4;
/// Hits the shield takes before it is gone.
const SHIELD: u32 = 4;
const BAR: [&str; 6] = ["SPEED", "MISSILE", "DOUBLE", "LASER", "OPTION", "?"];
/// Links in the serpent's body.
const SEGMENTS: usize = 14;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ground { Hills, Heads, Open, Cave, Fire, Maze, Fort }

#[derive(Clone, Copy, PartialEq, Debug)]
enum BossKind { Core, Head, Eye, Serpent, Brain }

struct Stage {
    name: &'static str,
    ground: Ground,
    /// Deep space at the top, the haze low down, the rock and its lit rim.
    sky: Rgb,
    haze: Rgb,
    rock: Rgb,
    rim: Rgb,
    /// Pixels flown before the boss comes, and how fast.
    length: f32,
    scroll: f32,
    boss: BossKind,
    /// Plates in front of a core.
    plates: usize,
    tune: usize,
}

static STAGES: [Stage; 7] = [
    Stage { name: "VOLCANO", ground: Ground::Hills, sky: 0x04060e, haze: 0x3a1618, rock: 0x7a4a30, rim: 0xe8a060, length: 5200.0, scroll: 60.0, boss: BossKind::Core, plates: 3, tune: 0 },
    Stage { name: "STONE HEADS", ground: Ground::Heads, sky: 0x04080e, haze: 0x243040, rock: 0x6a6a74, rim: 0xc8c8d4, length: 5200.0, scroll: 60.0, boss: BossKind::Head, plates: 0, tune: 1 },
    Stage { name: "CRYSTALS", ground: Ground::Open, sky: 0x020612, haze: 0x0c2a48, rock: 0x3a6a8a, rim: 0xa0f0ff, length: 5000.0, scroll: 60.0, boss: BossKind::Core, plates: 4, tune: 0 },
    Stage { name: "LIVING CELLS", ground: Ground::Cave, sky: 0x100306, haze: 0x3a0c16, rock: 0xa84058, rim: 0xff98b0, length: 5000.0, scroll: 54.0, boss: BossKind::Eye, plates: 0, tune: 1 },
    Stage { name: "THE SUN", ground: Ground::Fire, sky: 0x140400, haze: 0x5a1c00, rock: 0xd05010, rim: 0xffe070, length: 5200.0, scroll: 60.0, boss: BossKind::Serpent, plates: 0, tune: 0 },
    Stage { name: "HIGH SPEED", ground: Ground::Maze, sky: 0x02030c, haze: 0x0c1a3a, rock: 0x3e4e7e, rim: 0x98c8ff, length: 7400.0, scroll: 150.0, boss: BossKind::Core, plates: 5, tune: 1 },
    Stage { name: "THE FORTRESS", ground: Ground::Fort, sky: 0x040406, haze: 0x1c1c26, rock: 0x585868, rim: 0xb8b8d0, length: 5600.0, scroll: 54.0, boss: BossKind::Brain, plates: 0, tune: 0 },
];

const TUNE_A: &str = "150 d5/8 d5/8 a5/8 d5/8 c6/8 d5/8 a5/8 f5/8 g5/8 f5/8 e5/8 f5/8 d5/4 -/4 \
    d5/8 d5/8 a5/8 d5/8 c6/8 d5/8 bb5/8 a5/8 g5/8 a5/8 bb5/8 c6/8 a5/2 \
    f5/8 g5/8 a5/8 c6/8 bb5/8 a5/8 g5/8 f5/8 e5/8 f5/8 g5/8 a5/8 f5/4 d5/4 \
    bb4/8 d5/8 f5/8 bb5/8 a5/8 f5/8 c5/8 e5/8 d5/4 a4/4 d5/2";
const BASS_A: &str = "150 d3/8 d3/8 d4/8 d3/8 d3/8 d4/8 d3/8 d4/8 c3/8 c3/8 c4/8 c3/8 bb2/8 bb2/8 bb3/8 bb2/8 \
    d3/8 d3/8 d4/8 d3/8 d3/8 d4/8 d3/8 d4/8 g2/8 g2/8 g3/8 g2/8 a2/8 a2/8 a3/8 a2/8 \
    f2/8 f2/8 f3/8 f2/8 c3/8 c3/8 c4/8 c3/8 c3/8 c3/8 c4/8 c3/8 d3/8 d3/8 d4/8 d3/8 \
    bb2/8 bb2/8 bb3/8 bb2/8 a2/8 a2/8 a3/8 a2/8 d3/8 d3/8 d4/8 d3/8 a2/8 a2/8 a3/8 a2/8";
const TUNE_B: &str = "150 a4/8 c5/8 e5/8 a5/8 g5/8 e5/8 c5/8 e5/8 f5/8 e5/8 d5/8 c5/8 b4/4 e5/4 \
    a4/8 c5/8 e5/8 a5/8 b5/8 a5/8 g5/8 e5/8 f5/8 g5/8 a5/8 b5/8 c6/2 \
    c6/8 b5/8 a5/8 g5/8 a5/8 g5/8 f5/8 e5/8 d5/8 e5/8 f5/8 g5/8 e5/4 c5/4 \
    d5/8 f5/8 a5/8 d6/8 c6/8 a5/8 g#5/8 b5/8 a5/4 e5/4 a4/2";
const BASS_B: &str = "150 a2/8 a2/8 a3/8 a2/8 a2/8 a3/8 a2/8 a3/8 d3/8 d3/8 d4/8 d3/8 e2/8 e2/8 e3/8 e2/8 \
    a2/8 a2/8 a3/8 a2/8 g2/8 g2/8 g3/8 g2/8 f2/8 f2/8 f3/8 f2/8 e2/8 e2/8 e3/8 e2/8 \
    a2/8 a2/8 a3/8 a2/8 c3/8 c3/8 c4/8 c3/8 d3/8 d3/8 d4/8 d3/8 c3/8 c3/8 c4/8 c3/8 \
    d3/8 d3/8 d4/8 d3/8 e2/8 e2/8 e3/8 e2/8 a2/8 a2/8 a3/8 a2/8 e2/8 e2/8 e3/8 e2/8";
const TUNE_BOSS: &str = "160 e5/8 e5/8 f5/8 e5/8 bb5/4 a5/4 e5/8 e5/8 f5/8 e5/8 c6/4 b5/4 \
    e5/8 g5/8 bb5/8 c#6/8 e6/4 d6/8 c#6/8 bb5/8 a5/8 g5/8 f5/8 e5/2";
const BASS_BOSS: &str = "160 e2/8 e3/8 e2/8 e3/8 e2/8 e3/8 e2/8 e3/8 f2/8 f3/8 f2/8 f3/8 f2/8 f3/8 f2/8 f3/8 \
    g2/8 g3/8 g2/8 g3/8 a2/8 a3/8 a2/8 a3/8 bb2/8 bb3/8 a2/8 a3/8 e2/8 e3/8 e2/8 e3/8";
const TUNE_TITLE: &str = "120 d5/4 a4/8 d5/8 f5/4 a5/4 g5/4 f5/8 e5/8 d5/2 bb4/4 d5/8 f5/8 a5/4 g5/4 \
    f5/8 e5/8 d5/8 e5/8 a4/2 d5/4 a4/8 d5/8 f5/4 a5/4 c6/4 bb5/8 a5/8 g5/2 \
    a5/8 g5/8 f5/8 e5/8 f5/4 e5/4 d5/1";
const BASS_TITLE: &str = "120 d3/4 a3/4 d3/4 a3/4 g2/4 d3/4 g2/4 d3/4 bb2/4 f3/4 bb2/4 f3/4 a2/4 e3/4 a2/4 c#3/4 \
    d3/4 a3/4 d3/4 a3/4 c3/4 g3/4 bb2/4 f3/4 a2/4 e3/4 a2/4 c#3/4 d3/4 a2/4 d3/2";

/// A tune over a bass line, mixed into one loop so the two never drift.
fn duet(tune: &str, bass: &str) -> Sample {
    let a = Tune::parse(tune, Wave::Square, 0.14).render();
    let b = Tune::parse(bass, Wave::Triangle, 0.32).render();
    let at = |s: &Sample, i: usize| *s.data.get(i).unwrap_or(&0) as i32;
    let n = a.data.len().max(b.data.len());
    Sample::from_i16((0..n).map(|i| (at(&a, i) + at(&b, i)).clamp(-32768, 32767) as i16).collect())
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (smooth(x - xi as f32), smooth(y - yi as f32));
    let a = hash(xi, yi, seed) + (hash(xi + 1, yi, seed) - hash(xi, yi, seed)) * fx;
    let b = hash(xi, yi + 1, seed) + (hash(xi + 1, yi + 1, seed) - hash(xi, yi + 1, seed)) * fx;
    a + (b - a) * fy
}

/// Noise along a line in three layers, each finer and fainter.
fn ridge(x: f32, seed: u32) -> f32 {
    noise(x, 0.0, seed) * 0.6 + noise(x * 2.1, 0.0, seed + 1) * 0.28 + noise(x * 4.3, 0.0, seed + 2) * 0.12
}

/// A stage's ground and ceiling, a pixel column at a time: the row where
/// the ground begins (BOT for none) and the row where the ceiling ends
/// (0 for none).
struct Land { floor: Vec<i16>, ceil: Vec<i16>, volcanoes: Vec<f32> }

impl Land {
    fn new(n: usize) -> Land {
        let st = &STAGES[n];
        let len = st.length as usize + W as usize + 64;
        let seed = 101 + n as u32 * 17;
        let b = BOT as f32;
        let mut fl = vec![b; len];
        let mut ce = vec![0.0f32; len];
        let mut volcanoes = Vec::new();
        match st.ground {
            Ground::Hills => {
                volcanoes = vec![st.length * 0.3, st.length * 0.52, st.length * 0.74];
                for x in 0..len {
                    let xf = x as f32;
                    let mut f = b - 12.0 - 74.0 * ridge(xf / 230.0, seed).powf(1.6);
                    let c = ridge(xf / 310.0, seed + 7);
                    let mut top = if c > 0.55 { (c - 0.55) / 0.45 * 150.0 } else { 0.0 };
                    for &v in &volcanoes {
                        let d = (xf - v).abs();
                        if d < 130.0 { f = f.min(b - 116.0 + d * 0.82 + if d < 8.0 { 6.0 } else { 0.0 }); }
                        if d < 230.0 { top = 0.0; }
                    }
                    fl[x] = f;
                    ce[x] = top.min(f - 104.0).max(0.0);
                }
            }
            Ground::Heads => for x in 0..len {
                let xf = x as f32;
                fl[x] = b - 22.0 - 12.0 * ridge(xf / 170.0, seed);
                ce[x] = 22.0 + 12.0 * ridge(xf / 190.0, seed + 5);
            },
            Ground::Open => {}
            Ground::Cave => for x in 0..len {
                let xf = x as f32;
                let mid = b / 2.0 + 48.0 * (xf / 380.0).sin() + 28.0 * (ridge(xf / 260.0, seed) - 0.5);
                let half = 60.0 + 24.0 * ridge(xf / 200.0, seed + 3);
                fl[x] = (mid + half).min(b - 6.0);
                ce[x] = (mid - half).max(6.0);
            },
            Ground::Fire => for x in 0..len {
                let xf = x as f32;
                fl[x] = b - 20.0 - 10.0 * (xf / 70.0).sin() - 10.0 * ridge(xf / 40.0, seed);
                ce[x] = 20.0 + 10.0 * (xf / 90.0 + 1.0).sin() + 10.0 * ridge(xf / 50.0, seed + 4);
            },
            Ground::Maze => {
                // Blocks of walls, the opening wandering up and down; each
                // opening overlaps the one before by more than a ship.
                let mut rng = Rng::new(seed as u64);
                let seg = 56;
                let mut mid = b / 2.0;
                for s0 in (0..len).step_by(seg) {
                    mid = (mid + rng.range(-26.0, 26.0)).clamp(64.0, b - 64.0);
                    let gap = rng.range(96.0, 124.0);
                    let q = |v: f32| (v / 8.0).round() * 8.0;
                    for x in s0..(s0 + seg).min(len) {
                        fl[x] = q(mid + gap / 2.0);
                        ce[x] = q(mid - gap / 2.0);
                    }
                }
            }
            Ground::Fort => {
                let mut rng = Rng::new(seed as u64);
                let seg = 40;
                for s0 in (0..len).step_by(seg) {
                    let (mut f, mut c) = (b - 26.0, 26.0);
                    if rng.chance(0.35) { f -= [16.0, 24.0, 40.0][rng.below(3) as usize]; }
                    if rng.chance(0.35) { c += [16.0, 24.0, 40.0][rng.below(3) as usize]; }
                    if f - c < 116.0 { c = f - 116.0; }
                    for x in s0..(s0 + seg).min(len) {
                        fl[x] = f;
                        ce[x] = c;
                    }
                }
            }
        }
        // Calm where the flight starts, and room for the boss at the end.
        let (cf, cc) = match st.ground {
            Ground::Open => (b, 0.0),
            Ground::Cave | Ground::Fire => (b - 14.0, 14.0),
            Ground::Maze | Ground::Fort => (b - 18.0, 18.0),
            _ => (b - 12.0, 0.0),
        };
        let end = st.length - 560.0;
        for x in 0..len {
            let xf = x as f32;
            let t = smooth((xf - 300.0) / 260.0).min(1.0 - smooth((xf - end) / 300.0));
            fl[x] = cf + (fl[x] - cf) * t;
            ce[x] = cc + (ce[x] - cc) * t;
        }
        Land {
            floor: fl.iter().map(|&v| v.round() as i16).collect(),
            ceil: ce.iter().map(|&v| v.round() as i16).collect(),
            volcanoes,
        }
    }

    /// The ground's top and the ceiling's bottom at a world column.
    fn at(&self, wx: f32) -> (f32, f32) {
        let i = (wx.max(0.0) as usize).min(self.floor.len() - 1);
        (self.floor[i] as f32, self.ceil[i] as f32)
    }

    /// True inside rock.
    fn solid(&self, wx: f32, y: f32) -> bool {
        let (fl, ce) = self.at(wx);
        y >= fl || y < ce
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Fan, Dart, Swoop, Rusher, Walker, Turret, Hatch, Hopper, Volcano, Rock, Head, Ring, Crystal, Amoeba, Flare, Bird, Shutter }

#[derive(Clone, Copy, Debug)]
struct Foe {
    kind: Kind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    hp: i32,
    t: f32,
    /// What the kind needs to remember: a wave's middle, a crystal's size,
    /// whether a shutter is hard.
    a: f32,
    /// Hangs from the ceiling.
    up: bool,
    /// The formation it flies in (0 for none), and whether it brings a capsule.
    group: u32,
    carrier: bool,
    /// Seconds to its next shot.
    fire: f32,
    turned: bool,
    flash: f32,
}

impl Foe {
    fn new(kind: Kind, x: f32, y: f32) -> Foe {
        let hp = match kind {
            Kind::Hatch | Kind::Head | Kind::Crystal => 6,
            Kind::Turret | Kind::Rock | Kind::Amoeba | Kind::Bird => 2,
            Kind::Shutter => 12,
            _ => 1,
        };
        Foe { kind, x, y, vx: 0.0, vy: 0.0, hp, t: 0.0, a: y, up: false, group: 0, carrier: false, fire: 1.0, turned: false, flash: 0.0 }
    }

    fn radius(&self) -> f32 {
        match self.kind {
            Kind::Hatch => 11.0,
            Kind::Head => 13.0,
            Kind::Hopper | Kind::Ring => 4.5,
            Kind::Rock => 6.0,
            Kind::Crystal => 2.0 + self.a * 4.0,
            _ => 7.0,
        }
    }

    /// Shots fly through it: a volcano's mouth, a flare of the sun.
    fn ghost(&self) -> bool { matches!(self.kind, Kind::Volcano | Kind::Flare) }

    /// Shots stop on it and do no harm: a head with its mouth shut, a
    /// hard shutter.
    fn armoured(&self) -> bool {
        (self.kind == Kind::Head && !self.mouth_open()) || (self.kind == Kind::Shutter && self.a > 0.5)
    }

    fn mouth_open(&self) -> bool { self.kind == Kind::Head && self.t % 3.0 > 1.9 }

    /// Small enough for a blue capsule to sweep away.
    fn small(&self) -> bool {
        matches!(self.kind, Kind::Fan | Kind::Dart | Kind::Swoop | Kind::Rusher | Kind::Hopper | Kind::Rock | Kind::Ring | Kind::Amoeba | Kind::Bird | Kind::Walker)
            || (self.kind == Kind::Crystal && self.a < 1.5)
    }

    fn points(&self) -> u32 {
        match self.kind {
            Kind::Fan | Kind::Swoop | Kind::Hopper | Kind::Ring => 100,
            Kind::Dart | Kind::Rusher | Kind::Walker | Kind::Rock | Kind::Amoeba | Kind::Bird => 200,
            Kind::Turret => 300,
            Kind::Crystal => 100 * self.a as u32,
            Kind::Shutter => 500,
            Kind::Hatch => 1000,
            Kind::Head => 1500,
            Kind::Volcano | Kind::Flare => 0,
        }
    }

    fn color(&self) -> Rgb {
        match self.kind {
            Kind::Fan => 0x4aa0e8,
            Kind::Dart => 0xe84040,
            Kind::Swoop => 0x5ac85a,
            Kind::Rusher => 0xff9a30,
            Kind::Walker => 0xc89040,
            Kind::Crystal => 0x90e0ff,
            Kind::Amoeba => 0xe070a8,
            Kind::Bird | Kind::Flare => 0xff8020,
            Kind::Rock => 0x8a6040,
            Kind::Hopper => 0xe0e050,
            _ => 0xa0a0b0,
        }
    }
}

/// The two pillars of a shutter, as left, top, right, bottom.
fn shutter_rects(f: &Foe, land: &Land, cam: f32) -> [(f32, f32, f32, f32); 2] {
    let (fl, ce) = land.at(cam + f.x);
    let mid = (fl + ce) / 2.0;
    let gap = 70.0 + 44.0 * (f.t * 1.7 + f.vy).sin();
    [(f.x - 7.0, ce, f.x + 7.0, mid - gap / 2.0), (f.x - 7.0, mid + gap / 2.0, f.x + 7.0, fl)]
}

/// The fireballs of a flare, rising from the surface in an arch and
/// sinking back.
fn flare_balls(f: &Foe) -> [(f32, f32); 9] {
    let c = f.t % 3.2;
    let h = if c < 2.4 { 84.0 * (PI * c / 2.4).sin() } else { 0.0 };
    let mut out = [(0.0, 0.0); 9];
    for (i, o) in out.iter_mut().enumerate() {
        let a = PI * i as f32 / 8.0;
        let dy = h * a.sin();
        *o = (f.x + 36.0 - 36.0 * a.cos(), if f.up { f.y + dy } else { f.y - dy });
    }
    out
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Fans { y: f32, n: u32 },
    Swoops { top: bool, n: u32 },
    Rushers { n: u32 },
    Carrier { y: f32 },
    Birds { y: f32, n: u32 },
    Crystal { y: f32 },
    Amoebas { y: f32 },
    Ground { kind: Kind, up: bool },
    Volcano,
    Flare { up: bool },
    Wall,
    Shutter { hard: bool },
}

impl Event {
    /// Part of the landscape: placed even when the ship comes back to a
    /// checkpoint with it already on screen.
    fn fixed(&self) -> bool {
        matches!(self, Event::Ground { .. } | Event::Volcano | Event::Flare { .. } | Event::Wall | Event::Shutter { .. })
    }
}

/// What comes along a stage, and where. The same every time, so a stage
/// can be learned.
fn events(n: usize, land: &Land) -> Vec<(f32, Event)> {
    #[derive(Clone, Copy)]
    enum Pick { Fans, Swoops, Rushers, Carrier, Birds, Crystal, Amoebas, Turret, Walker, Hatch, Flare, Shutter }
    use Pick::*;
    let st = &STAGES[n];
    let mut rng = Rng::new(7 + n as u64 * 31);
    let b = BOT as f32;
    let mut ev: Vec<(f32, Event)> = land.volcanoes.iter().map(|&v| (v, Event::Volcano)).collect();
    // A capsule early on, every stage.
    ev.push((700.0, Event::Carrier { y: b / 2.0 }));
    ev.push((950.0, Event::Fans { y: b / 2.0 - 30.0, n: 5 }));
    let menu: &[Pick] = match st.ground {
        Ground::Hills => &[Fans, Fans, Fans, Fans, Swoops, Swoops, Carrier, Carrier, Turret, Turret, Turret, Walker, Walker, Hatch, Hatch, Rushers],
        Ground::Heads => &[Fans, Fans, Fans, Fans, Swoops, Swoops, Carrier, Carrier, Walker, Walker, Turret, Turret, Rushers],
        Ground::Open => &[Fans, Fans, Crystal, Crystal, Crystal, Crystal, Swoops, Swoops, Carrier, Carrier, Rushers],
        Ground::Cave => &[Fans, Fans, Amoebas, Amoebas, Amoebas, Carrier, Carrier, Hatch, Hatch, Turret, Turret, Swoops],
        Ground::Fire => &[Birds, Birds, Birds, Flare, Flare, Flare, Fans, Carrier, Carrier, Swoops, Swoops],
        Ground::Maze => &[Fans, Fans, Fans, Carrier, Carrier, Shutter, Shutter, Shutter],
        Ground::Fort => &[Turret, Turret, Turret, Hatch, Hatch, Shutter, Shutter, Fans, Fans, Carrier, Carrier, Walker, Walker, Rushers],
    };
    let step = if st.ground == Ground::Maze { 2.3 } else { 1.0 };
    let mut x = 1180.0;
    while x < st.length - 320.0 {
        let y = rng.range(40.0, b - 50.0);
        let up = rng.chance(0.4);
        let e = match menu[rng.below(menu.len() as u32) as usize] {
            Fans => Event::Fans { y, n: 5 + rng.below(2) },
            Swoops => Event::Swoops { top: up, n: 4 },
            Rushers => Event::Rushers { n: 3 },
            Carrier => Event::Carrier { y },
            Birds => Event::Birds { y, n: 4 },
            Crystal => Event::Crystal { y },
            Amoebas => Event::Amoebas { y },
            Turret => Event::Ground { kind: Kind::Turret, up },
            Walker => Event::Ground { kind: Kind::Walker, up },
            Hatch => Event::Ground { kind: Kind::Hatch, up },
            Flare => Event::Flare { up },
            Shutter => Event::Shutter { hard: st.ground == Ground::Maze },
        };
        ev.push((x, e));
        x += rng.range(180.0, 270.0) * step;
    }
    match st.ground {
        Ground::Heads => {
            let (mut x, mut up) = (520.0, false);
            while x < st.length - 300.0 {
                ev.push((x, Event::Ground { kind: Kind::Head, up }));
                up = !up;
                x += 330.0;
            }
        }
        Ground::Cave => {
            let mut x = 900.0;
            while x < st.length - 400.0 {
                ev.push((x, Event::Wall));
                x += 620.0;
            }
        }
        _ => {}
    }
    ev.sort_by(|a, b| a.0.total_cmp(&b.0));
    ev
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Weapon { Shot, Double, Laser }

#[derive(Clone, Copy, PartialEq, Debug)]
enum ShotKind { Shot, Double, Laser, Missile }

#[derive(Clone, Copy, Debug)]
struct Shot { kind: ShotKind, x: f32, y: f32, vx: f32, vy: f32, src: u8, alive: bool, ground: bool }

/// An enemy shot; a beam is a boss's laser, 28 pixels long from `x`.
#[derive(Clone, Copy, Debug)]
struct Bullet { x: f32, y: f32, vx: f32, vy: f32, beam: bool }

struct Cap { x: f32, y: f32, blue: bool }

/// One block of a wall of living cells; shot away, it grows back.
struct Cell { wx: f32, y: f32, alive: bool, back: f32 }

struct Boss {
    kind: BossKind,
    x: f32,
    y: f32,
    /// Where it stops coming in.
    tx: f32,
    t: f32,
    hp: i32,
    /// Hits left in each plate before a core, the front one first.
    plates: Vec<i32>,
    full: i32,
    fire: f32,
    spawn: f32,
    flash: f32,
    was_open: bool,
    /// The serpent's head, a tick at a time, newest first.
    path: VecDeque<(f32, f32)>,
    /// The fortress walls before the brain: offset and hits left.
    blocks: Vec<(f32, f32, i32)>,
    dying: f32,
}

fn boss_open(b: &Boss) -> bool {
    match b.kind {
        BossKind::Head => b.t % 2.8 > 1.5,
        BossKind::Eye => b.t % 3.4 > 1.8,
        _ => true,
    }
}

fn serpent_parts(b: &Boss) -> impl Iterator<Item = (f32, f32)> + '_ {
    (1..=SEGMENTS).filter_map(move |i| b.path.get(i * 6).copied())
}

/// The core ship's hull, relative to its core: two wings, the spine and
/// the channel that leads in to the core.
fn core_solid(dx: f32, dy: f32) -> bool {
    (dx >= -52.0 && dx <= 62.0 && dy.abs() >= 18.0 && dy.abs() <= 34.0)
        || (dx >= -2.0 && dx <= 62.0 && dy.abs() <= 18.0)
        || (dx >= -50.0 && dx <= 0.0 && dy.abs() <= 13.0)
}

/// True where the boss is solid, for the ship to crash into.
fn boss_solid(b: &Boss, x: f32, y: f32) -> bool {
    let (dx, dy) = (x - b.x, y - b.y);
    match b.kind {
        BossKind::Core => core_solid(dx, dy),
        BossKind::Head => dx.abs() <= 24.0 && dy.abs() <= 28.0,
        BossKind::Eye => dx * dx + dy * dy <= 32.0 * 32.0 || (dx + 26.0).powi(2) + dy * dy <= 121.0,
        BossKind::Serpent => dx * dx + dy * dy <= 121.0 || serpent_parts(b).any(|(px, py)| (x - px).powi(2) + (y - py).powi(2) <= 64.0),
        BossKind::Brain => dx * dx + dy * dy <= 26.0 * 26.0
            || b.blocks.iter().any(|&(ox, oy, hp)| hp > 0 && dx >= ox && dx <= ox + 12.0 && dy >= oy && dy <= oy + 12.0),
    }
}

/// An aimed shot from a foe, when it is on screen.
fn aim(bullets: &mut Vec<Bullet>, from: (f32, f32), to: (f32, f32), speed: f32, turn: f32) {
    if from.0 < 8.0 || from.0 > W as f32 - 8.0 { return; }
    let a = (to.1 - from.1).atan2(to.0 - from.0) + turn;
    bullets.push(Bullet { x: from.0, y: from.1, vx: a.cos() * speed, vy: a.sin() * speed, beam: false });
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Play, Paused, Dying(f32), Clear(f32), Over(f32) }

struct Art {
    ship: Sprite,
    option: [Sprite; 2],
    fan: [Sprite; 2],
    dart: Sprite,
    swoop: Sprite,
    rusher: Sprite,
    /// Floor two steps, then ceiling two steps.
    walker: [Sprite; 4],
    turret: [Sprite; 2],
    /// Floor shut and open, then ceiling shut and open.
    hatch: [Sprite; 4],
    hopper: Sprite,
    rock: Sprite,
    head: [Sprite; 4],
    crystal: [Sprite; 3],
    amoeba: [Sprite; 2],
    bird: [Sprite; 2],
    cap: [Sprite; 2],
    big_head: [Sprite; 2],
}

impl Art {
    fn new() -> Art {
        let ship = fancy(&["...dww..........", "..dbbww.........", "rrdbbbbbbbbw....", ".ddbbbkkbbbbbww.",
                           "rrdbbbbbbbbw....", "..dbbww.........", "...dww.........."],
            &[('d', 0x3a4a9a), ('w', 0xe8f0ff), ('b', 0x7a92e0), ('k', 0x70e8ff), ('r', 0xff5a30)]);
        let orb = |y: Rgb| fancy(&[".oooo.", "oyyyyo", "oywyyo", "oyyyyo", "oyyyyo", ".oooo."], &[('o', 0xc84010), ('y', y), ('w', 0xffffd0)]);
        let fp = [('c', 0x4aa0e8), ('d', 0x1a4a90), ('w', 0xffffff), ('k', 0x102040), ('r', 0xff4a4a)];
        let fan = [
            fancy(&["..cccc..", ".cwccdc.", "ccdkkdcc", "cckrrkcc", "cckrrkcc", "ccdkkdcc", ".cdccwc.", "..cccc.."], &fp),
            fancy(&["..cccc..", ".cdccwc.", "ccdkkdcc", "cckrrkcc", "cckrrkcc", "ccdkkdcc", ".cwccdc.", "..cccc.."], &fp),
        ];
        let dart = fancy(&["......rr..", "....rrrd..", "rrwrrrrddd", ".rrrkkrrrd", "rrwrrrrddd", "....rrrd..", "......rr.."],
            &[('r', 0xe84040), ('d', 0x902020), ('w', 0xffffff), ('k', 0xffe040)]);
        let swoop = fancy(&["..gggg...", ".gwggggd.", "gggkkgggd", "ggkkkkggd", "gggkkgggd", ".gggggdd.", "..gggg..."],
            &[('g', 0x5ac85a), ('d', 0x2a6a30), ('w', 0xffffff), ('k', 0xffe040)]);
        let rusher = fancy(&["..oo......", ".oooooo...", "oowoookkoo", "ooooookkoo", ".oooooo...", "..oo......"],
            &[('o', 0xff9a30), ('w', 0xffffff), ('k', 0x402010)]);
        let wp = [('m', 0xc89040), ('d', 0x7a5020), ('w', 0xffffff), ('k', 0x301808)];
        let w0 = fancy(&[".mmmmmm.", "mwmmmmdm", "mkkmmmdm", "mmmmmmmm", ".dmmmmd.", "..m..m..", ".m....m.", "mm....mm"], &wp);
        let w1 = fancy(&[".mmmmmm.", "mwmmmmdm", "mkkmmmdm", "mmmmmmmm", ".dmmmmd.", "..m..m..", "..m..m..", ".mm..mm."], &wp);
        let turret = fancy(&["..tttt..", ".twtttd.", "tttttttd", "tttttttd", "dddddddd", "dkdkdkdd"],
            &[('t', 0x98a2b0), ('d', 0x505a68), ('w', 0xffffff), ('k', 0x303840)]);
        let hp = [('h', 0x9a8a60), ('d', 0x5a4a30), ('w', 0xffffff), ('k', 0x201810), ('r', 0xff6020), ('y', 0xffd040)];
        let h0 = fancy(&["hhhhhhhhhhhh", "hwhhhhhhhhdh", "hdkkkkkkkkdh", "hdkkkkkkkkdh", "hdhhhhhhhhdh", "dddddddddddd"], &hp);
        let h1 = fancy(&["hhhhhhhhhhhh", "hwhhhhhhhhdh", "hdrrrrrrrrdh", "hdryyyyyyrdh", "hdhhhhhhhhdh", "dddddddddddd"], &hp);
        let hopper = fancy(&[".yyyy.", "yykkyy", "yyyyyy", ".y..y.", "y....y"], &[('y', 0xe0e050), ('k', 0x303010)]);
        let rock = fancy(&["..rrr..", ".rwrrr.", "rrrrrdr", "rrrrrrd", "rrdrrrd", ".rrrdd.", "..ddd.."],
            &[('r', 0x8a6040), ('d', 0x503018), ('w', 0xc09060)]);
        let head_rows = |open: bool| -> Vec<&'static str> {
            let mut r = vec!["...ssssss...", "..swssssss..", ".sssssssssd.", ".skksssssdd.", ".ssssssssdd.", "sssssssssddd", "ssdssssssddd"];
            if open { r.extend(["skkkkkssssdd", "krrrrksssdd.", "krrrrksssdd.", "skkkkkssssd."]); }
            else { r.extend(["ssssssssssdd", ".skkkkssssd.", ".ssssssssdd.", ".ssssssssdd."]); }
            r.extend(["..ssssssddd.", "..sssssdddd.", "..dddddddd.."]);
            r
        };
        let sp = [('s', 0x8a8a94), ('d', 0x4e4e58), ('w', 0xd8d8e0), ('k', 0x16161e), ('r', 0xff7040)];
        let head0 = fancy(&head_rows(false), &sp);
        let head1 = fancy(&head_rows(true), &sp);
        let big = |open: bool| outline(&shade(&scale2x(&scale2x(&Sprite::from_rows(&head_rows(open), &sp)))), 0x100818);
        let ap = [('p', 0xe070a8), ('w', 0xffe0f0), ('k', 0x902050)];
        let amoeba = [
            fancy(&["..pppp..", ".pwppppp", "pppkkppp", "ppkkkkpp", "ppkkkkpp", "pppkkppp", ".pppppp.", "..pppp.."], &ap),
            fancy(&[".pppp...", "pwppppp.", "ppkkkppp", "pkkkkkpp", "ppkkkkpp", "pppkkppp", ".ppppppp", "...pppp."], &ap),
        ];
        let bp = [('f', 0xff6010), ('y', 0xffd040), ('w', 0xffffff)];
        let bird = [
            fancy(&["....ff....", "..ffyyf...", "ffyywyyfff", ".ffyyyyff.", "...ffff...", "....f....."], &bp),
            fancy(&["..........", "..ffyyf...", "ffyywyyfff", ".ffyyyyff.", "..ffffff..", ".ff...ff.."], &bp),
        ];
        let cap = |c: Rgb, w: Rgb, d: Rgb| fancy(&[".cccccc.", "cwwccccc", "cwcccccc", "cccccccc", "ccccccdc", ".cccccc."], &[('c', c), ('w', w), ('d', d)]);
        Art {
            ship,
            option: [orb(0xff9030), orb(0xffc868)],
            fan,
            dart,
            swoop,
            rusher,
            walker: [w0.clone(), w1.clone(), vflip(&w0), vflip(&w1)],
            turret: [turret.clone(), vflip(&turret)],
            hatch: [h0.clone(), h1.clone(), vflip(&h0), vflip(&h1)],
            hopper,
            rock,
            head: [head0.clone(), head1.clone(), vflip(&head0), vflip(&head1)],
            crystal: [crystal(5), crystal(9), crystal(13)],
            amoeba,
            bird,
            cap: [cap(0xff7a20, 0xffe0b0, 0xa04010), cap(0x3a70ff, 0xc0e0ff, 0x1a3090)],
            big_head: [big(false), big(true)],
        }
    }
}

struct Salvo {
    stage: usize,
    round: u32,
    land: Land,
    events: Vec<(f32, Event)>,
    next: usize,
    sky: Vec<Rgb>,
    cam: f32,
    scroll: f32,
    ship: (f32, f32),
    /// Where the ship has been, newest first; the options fly along it.
    trail: VecDeque<(f32, f32)>,
    speed: u32,
    missile: bool,
    weapon: Weapon,
    options: usize,
    shield: u32,
    /// The lit slot of the power-up bar, -1 for none.
    bar: i32,
    inv: f32,
    fire_cd: f32,
    shots: Vec<Shot>,
    bullets: Vec<Bullet>,
    foes: Vec<Foe>,
    caps: Vec<Cap>,
    cells: Vec<Cell>,
    /// Formations still flying: id, how many are left, none escaped yet.
    groups: Vec<(u32, u32, bool)>,
    next_group: u32,
    dropped: u32,
    boss: Option<Boss>,
    boss_on: bool,
    warning: f32,
    stars: Vec<(f32, f32, u8)>,
    booms: Vec<(f32, f32, f32, f32)>,
    white: f32,
    score: u32,
    high: u32,
    lives: u32,
    next_life: u32,
    mode: Mode,
    time: f32,
    note: Option<(String, f32)>,
    rng: Rng,
    particles: Particles,
    audio: Audio,
    art: Art,
    m_stage: [Sample; 2],
    m_boss: Sample,
    m_title: Sample,
    s_shot: Sample,
    s_laser: Sample,
    s_boom: Sample,
    s_big: Sample,
    s_hit: Sample,
    s_hurt: Sample,
    s_cap: Sample,
    s_power: Sample,
    s_blue: Sample,
    s_die: Sample,
    s_warn: Sample,
    s_clear: Sample,
    s_life: Sample,
    s_beam: Sample,
}

impl Salvo {
    fn new() -> Salvo {
        let mut rng = Rng::new(5);
        let stars = (0..90).map(|_| (rng.range(0.0, W as f32), rng.range(0.0, BOT as f32), rng.below(3) as u8)).collect();
        let mut g = Salvo {
            stage: 0, round: 0, land: Land::new(0), events: Vec::new(), next: 0, sky: Vec::new(), cam: 0.0, scroll: 40.0,
            ship: (60.0, BOT as f32 / 2.0), trail: VecDeque::new(),
            speed: 0, missile: false, weapon: Weapon::Shot, options: 0, shield: 0, bar: -1, inv: 0.0, fire_cd: 0.0,
            shots: Vec::new(), bullets: Vec::new(), foes: Vec::new(), caps: Vec::new(), cells: Vec::new(),
            groups: Vec::new(), next_group: 0, dropped: 0, boss: None, boss_on: false, warning: 0.0,
            stars, booms: Vec::new(), white: 0.0,
            score: 0, high: funkey::store::high_score(GAME), lives: 3, next_life: 20000, mode: Mode::Title, time: 0.0, note: None,
            rng: Rng::from_time(), particles: Particles::new(), audio: Audio::off(), art: Art::new(),
            m_stage: [duet(TUNE_A, BASS_A), duet(TUNE_B, BASS_B)],
            m_boss: duet(TUNE_BOSS, BASS_BOSS),
            m_title: duet(TUNE_TITLE, BASS_TITLE),
            s_shot: Sample::sweep(Wave::Square, 1500.0, 800.0, 0.035, 0.08),
            s_laser: Sample::sweep(Wave::Saw, 2200.0, 500.0, 0.09, 0.09),
            s_boom: Sample::noise(0.18, 0.32),
            s_big: Sample::noise(0.9, 0.5).then(&Sample::sweep(Wave::Saw, 160.0, 40.0, 0.5, 0.3)),
            s_hit: Sample::tone(Wave::Square, 180.0, 0.03, 0.14),
            s_hurt: Sample::sweep(Wave::Square, 900.0, 400.0, 0.05, 0.22),
            s_cap: Tune::parse("420 c6/16 g6/16 c7/16", Wave::Square, 0.28).render(),
            s_power: Tune::parse("300 g5/16 c6/16 e6/16 g6/16 c7/8", Wave::Triangle, 0.45).render(),
            s_blue: Sample::sweep(Wave::Sine, 200.0, 1800.0, 0.6, 0.45),
            s_die: Sample::sweep(Wave::Saw, 700.0, 50.0, 0.9, 0.45).then(&Sample::noise(0.4, 0.35)),
            s_warn: Tune::parse("200 a5/8 e5/8 a5/8 e5/8 a5/8 e5/8 a5/8 e5/8", Wave::Square, 0.3).render(),
            s_clear: Tune::parse("200 d5/8 f5/8 a5/8 d6/4 c6/8 d6/2", Wave::Triangle, 0.5).render(),
            s_life: Tune::parse("300 e6/16 g6/16 e7/16 c7/16 d7/16 g7/8", Wave::Square, 0.35).render(),
            s_beam: Sample::sweep(Wave::Saw, 900.0, 300.0, 0.25, 0.2),
        };
        g.particles.gravity = 40.0;
        g.load(0);
        g.note = None;
        g
    }

    /// A stage from its start: the ground, what comes along it, the sky.
    fn load(&mut self, n: usize) {
        self.stage = n;
        self.land = Land::new(n);
        self.events = events(n, &self.land);
        self.next = 0;
        self.cam = 0.0;
        self.reset_world();
        let st = &STAGES[n];
        self.sky = (0..BOT).map(|y| mix(st.sky, st.haze, (y as f32 / BOT as f32).powf(1.8))).collect();
        self.say(&format!("STAGE {}  {}", n + 1, st.name));
    }

    fn reset_world(&mut self) {
        self.foes.clear();
        self.shots.clear();
        self.bullets.clear();
        self.caps.clear();
        self.cells.clear();
        self.groups.clear();
        self.boss = None;
        self.boss_on = false;
        self.warning = 0.0;
    }

    /// The ship in at the left, the music on.
    fn enter(&mut self) {
        let (fl, ce) = self.land.at(self.cam + 60.0);
        self.ship = (60.0, (fl + ce) / 2.0);
        self.trail = VecDeque::from(vec![self.ship; SPACING * MAX_OPTIONS + 1]);
        self.inv = 2.0;
        self.fire_cd = 0.0;
        self.audio.play_loop(1, &self.m_stage[STAGES[self.stage].tune], 0.8);
    }

    fn start_game(&mut self) {
        self.score = 0;
        self.lives = 3;
        self.next_life = 20000;
        self.round = 0;
        // SALVO_START=<stage>[,<pixels>], the stage counted from 1, for
        // screenshots and tests.
        let from = std::env::var("SALVO_START").unwrap_or_default();
        let mut at = from.split(',').map(|v| v.trim().parse::<f32>().unwrap_or(0.0));
        let n = (at.next().unwrap_or(1.0) as usize).clamp(1, STAGES.len()) - 1;
        self.load(n);
        self.cam = at.next().unwrap_or(0.0).clamp(0.0, STAGES[n].length);
        self.place();
        self.mode = Mode::Play;
    }

    /// Back at the last checkpoint: the start, the middle, or just before
    /// the boss. All power-ups are gone, but a ship that had any, or a lit
    /// bar, comes back with the first step lit.
    fn respawn(&mut self) {
        let l = STAGES[self.stage].length;
        self.cam = [0.0, l * 0.5, l - 900.0].into_iter().filter(|&c| c <= self.cam).fold(0.0, f32::max);
        let had = self.bar >= 0 || self.speed > 0 || self.missile || self.weapon != Weapon::Shot || self.options > 0 || self.shield > 0;
        self.place();
        if had { self.bar = 0; }
    }

    /// Fly on from where the camera is, with nothing: the landscape already
    /// on screen put back in place.
    fn place(&mut self) {
        self.reset_world();
        self.next = self.events.partition_point(|e| e.0 < self.cam - 40.0);
        while self.next < self.events.len() && self.events[self.next].0 <= self.cam + W as f32 + 24.0 {
            let (wx, e) = self.events[self.next];
            self.next += 1;
            if e.fixed() { self.spawn(wx, e); }
        }
        self.speed = 0;
        self.missile = false;
        self.weapon = Weapon::Shot;
        self.options = 0;
        self.shield = 0;
        self.bar = -1;
        self.enter();
    }

    fn say(&mut self, s: &str) { self.note = Some((s.to_string(), 2.2)); }

    fn add(&mut self, points: u32) {
        self.score += points;
        if self.score >= self.next_life {
            self.next_life += 70000;
            self.lives += 1;
            self.say("EXTRA SHIP");
            self.audio.play(&self.s_life, 1.0);
        }
    }

    fn boom(&mut self, x: f32, y: f32, r: f32) { self.booms.push((x, y, 0.0, r)); }

    fn group(&mut self, n: u32) -> u32 {
        self.next_group += 1;
        self.groups.push((self.next_group, n, true));
        self.next_group
    }

    fn escaped(&mut self, g: u32) {
        if let Some(k) = self.groups.iter().position(|e| e.0 == g) {
            self.groups[k].1 -= 1;
            self.groups[k].2 = false;
            if self.groups[k].1 == 0 { self.groups.swap_remove(k); }
        }
    }

    fn drop_cap(&mut self, x: f32, y: f32) {
        self.dropped += 1;
        self.caps.push(Cap { x, y, blue: self.dropped % 7 == 0 });
    }

    fn spawn(&mut self, wx: f32, e: Event) {
        let x = wx - self.cam;
        let b = BOT as f32;
        match e {
            Event::Fans { y, n } => {
                let g = self.group(n);
                for i in 0..n {
                    let mut f = Foe::new(Kind::Fan, x + i as f32 * 20.0, y);
                    f.vx = -150.0;
                    f.group = g;
                    self.foes.push(f);
                }
            }
            Event::Swoops { top, n } => {
                let g = self.group(n);
                for i in 0..n {
                    let k = i as f32;
                    let mut f = Foe::new(Kind::Swoop, x - 60.0 + k * 26.0, if top { -8.0 - k * 14.0 } else { b + 8.0 + k * 14.0 });
                    f.vx = -100.0;
                    f.vy = if top { 95.0 } else { -95.0 };
                    f.group = g;
                    self.foes.push(f);
                }
            }
            Event::Rushers { n } => {
                let g = self.group(n);
                for i in 0..n {
                    let mut f = Foe::new(Kind::Rusher, -80.0 - i as f32 * 28.0, self.ship.1);
                    f.vx = 170.0;
                    f.group = g;
                    self.foes.push(f);
                }
            }
            Event::Carrier { y } => {
                let mut f = Foe::new(Kind::Dart, x, y);
                f.vx = -120.0;
                f.carrier = true;
                self.foes.push(f);
            }
            Event::Birds { y, n } => {
                let g = self.group(n);
                for i in 0..n {
                    let mut f = Foe::new(Kind::Bird, x + i as f32 * 24.0, y);
                    f.vx = -125.0;
                    f.t = i as f32 * 0.3;
                    f.group = g;
                    self.foes.push(f);
                }
            }
            Event::Crystal { y } => {
                let mut f = Foe::new(Kind::Crystal, x + 16.0, y);
                f.a = 3.0;
                f.vx = -22.0;
                f.vy = self.rng.range(-20.0, 20.0);
                self.foes.push(f);
            }
            Event::Amoebas { y } => {
                for i in 0..3 {
                    let mut f = Foe::new(Kind::Amoeba, x + i as f32 * 18.0, y + (i as f32 - 1.0) * 16.0);
                    f.t = i as f32 * 0.7;
                    self.foes.push(f);
                }
            }
            Event::Ground { kind, up } => {
                let (fl, ce) = self.land.at(wx);
                let up = up && ce > 4.0;
                if !up && fl >= b - 1.0 { return; }
                let half = match kind { Kind::Head => 15.0, Kind::Walker => 9.0, _ => 7.0 };
                let mut f = Foe::new(kind, x, if up { ce + half } else { fl - half });
                f.up = up;
                f.fire = 0.8 + self.rng.float();
                self.foes.push(f);
            }
            Event::Volcano => {
                let (fl, _) = self.land.at(wx);
                self.foes.push(Foe::new(Kind::Volcano, x, fl));
            }
            Event::Flare { up } => {
                let (fl, ce) = self.land.at(wx);
                let mut f = Foe::new(Kind::Flare, x, if up { ce } else { fl });
                f.up = up;
                f.t = self.rng.range(0.0, 3.0);
                self.foes.push(f);
            }
            Event::Wall => {
                for col in 0..3 {
                    let cx = wx + col as f32 * 11.0;
                    let (fl, ce) = self.land.at(cx);
                    let mut y = ce;
                    while y + 6.0 <= fl {
                        self.cells.push(Cell { wx: cx, y, alive: true, back: 0.0 });
                        y += 11.0;
                    }
                }
            }
            Event::Shutter { hard } => {
                let mut f = Foe::new(Kind::Shutter, x, 0.0);
                f.a = if hard { 1.0 } else { 0.0 };
                f.vy = self.rng.range(0.0, 6.0);
                self.foes.push(f);
            }
        }
    }

    fn slot_open(&self, i: usize) -> bool {
        match i {
            0 => self.speed < 5,
            1 => !self.missile,
            2 => self.weapon != Weapon::Double,
            3 => self.weapon != Weapon::Laser,
            4 => self.options < MAX_OPTIONS,
            _ => self.shield == 0,
        }
    }

    /// Take what the bar has lit.
    fn take_power(&mut self) {
        if self.bar < 0 || !self.slot_open(self.bar as usize) { return; }
        let name = match self.bar {
            0 => { self.speed += 1; "SPEED UP" }
            1 => { self.missile = true; "MISSILE" }
            2 => { self.weapon = Weapon::Double; "DOUBLE" }
            3 => { self.weapon = Weapon::Laser; "LASER" }
            4 => { self.options += 1; "OPTION" }
            _ => { self.shield = SHIELD; "SHIELD" }
        };
        self.say(name);
        self.bar = -1;
        self.audio.play(&self.s_power, 1.0);
    }

    /// A capsule taken: the bar one step on, or a blue one's sweep.
    fn collect(&mut self, blue: bool) {
        self.add(100);
        if blue {
            let small: Vec<usize> = (0..self.foes.len()).filter(|&i| self.foes[i].small()).collect();
            for &i in small.iter().rev() { self.kill_foe(i); }
            self.bullets.clear();
            self.white = 0.3;
            self.audio.play(&self.s_blue, 1.0);
        } else {
            self.bar = (self.bar + 1) % 6;
            self.audio.play(&self.s_cap, 1.0);
        }
    }

    fn option_at(&self, k: usize) -> (f32, f32) {
        let i = ((k + 1) * SPACING).min(self.trail.len().saturating_sub(1));
        self.trail.get(i).copied().unwrap_or(self.ship)
    }

    fn fire(&mut self) {
        let mut sound = 0;
        let sources: Vec<(f32, f32)> = std::iter::once(self.ship).chain((0..self.options).map(|k| self.option_at(k))).collect();
        let count = |shots: &[Shot], src: u8, k: ShotKind| shots.iter().filter(|s| s.src == src && s.kind == k).count();
        for (i, (x, y)) in sources.into_iter().enumerate() {
            let src = i as u8;
            let shot = |kind, x, y, vx, vy| Shot { kind, x, y, vx, vy, src, alive: true, ground: false };
            if self.weapon == Weapon::Laser {
                if count(&self.shots, src, ShotKind::Laser) < 3 {
                    self.shots.push(shot(ShotKind::Laser, x + 16.0, y, 640.0, 0.0));
                    sound = 2;
                }
            } else {
                if count(&self.shots, src, ShotKind::Shot) < 3 {
                    self.shots.push(shot(ShotKind::Shot, x + 16.0, y, 460.0, 0.0));
                    sound = sound.max(1);
                }
                if self.weapon == Weapon::Double && count(&self.shots, src, ShotKind::Double) == 0 {
                    self.shots.push(shot(ShotKind::Double, x + 10.0, y - 3.0, 330.0, -330.0));
                }
            }
            if self.missile && count(&self.shots, src, ShotKind::Missile) == 0 {
                self.shots.push(shot(ShotKind::Missile, x, y + 5.0, 110.0, 150.0));
            }
        }
        match sound {
            2 => self.audio.play_on(3, &self.s_laser, 1.0),
            1 => self.audio.play_on(3, &self.s_shot, 1.0),
            _ => {}
        }
        self.fire_cd = if self.weapon == Weapon::Laser { 0.11 } else { 0.08 };
    }

    fn fly(&mut self, input: &Input, dt: f32) {
        let v = 80.0 + 26.0 * self.speed as f32;
        let (ax, ay) = (input.axis_x() as f32, input.axis_y() as f32);
        let k = if ax != 0.0 && ay != 0.0 { 0.7071 } else { 1.0 };
        let old = self.ship;
        self.ship.0 = (old.0 + ax * v * k * dt).clamp(14.0, W as f32 - 26.0);
        self.ship.1 = (old.1 + ay * v * k * dt).clamp(8.0, BOT as f32 - 7.0);
        if self.ship != old {
            self.trail.push_front(self.ship);
            self.trail.truncate(SPACING * MAX_OPTIONS + 1);
        }
        if input.pressed(Key::Char('z')) || input.pressed(Key::Char('x')) || input.pressed(Key::Enter) { self.take_power(); }
        self.fire_cd -= dt;
        if input.held(Key::Space) && self.fire_cd <= 0.0 { self.fire(); }
    }

    fn play(&mut self, input: &Input, dt: f32) {
        let st = &STAGES[self.stage];
        self.inv = (self.inv - dt).max(0.0);
        self.scroll = if self.boss_on { 0.0 } else { st.scroll * (1.0 + 0.1 * self.round as f32) };
        self.cam += self.scroll * dt;
        if !self.boss_on && self.cam >= st.length {
            self.cam = st.length;
            self.boss_on = true;
            self.warning = 2.4;
            self.audio.stop(1);
            self.audio.play(&self.s_warn, 1.0);
        }
        if self.warning > 0.0 {
            self.warning -= dt;
            if self.warning <= 0.0 { self.start_boss(); }
        }
        while self.next < self.events.len() && self.events[self.next].0 <= self.cam + W as f32 + 24.0 {
            let (wx, e) = self.events[self.next];
            self.next += 1;
            self.spawn(wx, e);
        }
        self.fly(input, dt);
        self.move_shots(dt);
        self.move_foes(dt);
        self.move_bullets(dt);
        self.move_boss(dt);
        let s = self.scroll;
        for c in &mut self.caps {
            c.x -= (s + 14.0) * dt;
        }
        self.caps.retain(|c| c.x > -20.0);
        let (sx, sy) = self.ship;
        let cam = self.cam;
        for c in &mut self.cells {
            if !c.alive {
                c.back -= dt;
                let (dx, dy) = (c.wx - cam + 5.0 - sx, c.y + 5.0 - sy);
                if c.back <= 0.0 && dx * dx + dy * dy > 30.0 * 30.0 { c.alive = true; }
            }
        }
        self.cells.retain(|c| c.wx - cam > -30.0);
        if self.mode == Mode::Play { self.collide(); }
    }

    fn move_shots(&mut self, dt: f32) {
        let (land, cam) = (&self.land, self.cam);
        let mut sparks = Vec::new();
        for s in &mut self.shots {
            if s.kind == ShotKind::Missile {
                if s.ground {
                    s.x += 170.0 * dt;
                    let (fl, _) = land.at(cam + s.x);
                    if fl < s.y - 8.0 { s.alive = false; } else { s.y = fl - 3.0; }
                } else {
                    s.x += s.vx * dt;
                    s.y += s.vy * dt;
                    let (fl, _) = land.at(cam + s.x);
                    if s.y >= fl - 3.0 {
                        if fl < BOT as f32 { s.ground = true; s.y = fl - 3.0; } else { s.alive = false; }
                    }
                }
                let (_, ce) = land.at(cam + s.x);
                if s.y < ce { s.alive = false; }
            } else {
                s.x += s.vx * dt;
                s.y += s.vy * dt;
                if land.solid(cam + s.x, s.y) {
                    s.alive = false;
                    sparks.push((s.x, s.y));
                }
            }
            if s.x > W as f32 + 40.0 || s.y < -8.0 || s.y > BOT as f32 + 4.0 { s.alive = false; }
        }
        self.shots.retain(|s| s.alive);
        for (x, y) in sparks { self.particles.burst(x, y, 3, 50.0, 0.2, 0xffe0a0); }
    }

    fn move_foes(&mut self, dt: f32) {
        let (sx, sy) = self.ship;
        let (s, cam, w) = (self.scroll, self.cam, W as f32);
        let speed = 100.0 + 25.0 * self.round as f32;
        let land = &self.land;
        let bullets = &mut self.bullets;
        let rng = &mut self.rng;
        let mut born: Vec<Foe> = Vec::new();
        for f in &mut self.foes {
            let before = f.t;
            f.t += dt;
            f.flash -= dt;
            let on_screen = f.x > 12.0 && f.x < w - 12.0;
            match f.kind {
                Kind::Fan => {
                    if !f.turned {
                        f.x += f.vx * dt;
                        f.y = f.a + 22.0 * (f.t * 5.0).sin();
                        if f.x < 210.0 { f.turned = true; }
                    } else {
                        f.vx = (f.vx + 420.0 * dt).min(170.0);
                        f.x += f.vx * dt;
                        f.y += (sy - f.y).clamp(-1.0, 1.0) * 80.0 * dt;
                    }
                }
                Kind::Dart => {
                    f.x += f.vx * dt;
                    f.y = f.a + 14.0 * (f.t * 3.0).sin();
                }
                Kind::Swoop => {
                    f.x += f.vx * dt;
                    f.y += f.vy * dt;
                    if (f.vy > 0.0 && f.y > sy - 6.0) || (f.vy < 0.0 && f.y < sy + 6.0) { f.turned = true; }
                    if f.turned {
                        f.vy *= 1.0 - (4.0 * dt).min(1.0);
                        f.vx = (f.vx - 220.0 * dt).max(-220.0);
                    }
                }
                Kind::Rusher => f.x += f.vx * dt,
                Kind::Walker => {
                    f.x -= (s + 26.0) * dt;
                    let (fl, ce) = land.at(cam + f.x);
                    f.y = if f.up { ce + 9.0 } else { fl - 9.0 };
                    f.fire -= dt;
                    if f.fire <= 0.0 {
                        f.fire = 2.4;
                        if on_screen && (sy < f.y) != f.up { aim(bullets, (f.x, f.y), (sx, sy), speed, 0.0); }
                    }
                }
                Kind::Turret => {
                    f.x -= s * dt;
                    f.fire -= dt;
                    if f.fire <= 0.0 {
                        f.fire = 2.0;
                        if on_screen && (sy < f.y) != f.up { aim(bullets, (f.x, f.y), (sx, sy), speed, 0.0); }
                    }
                }
                Kind::Hatch => {
                    f.x -= s * dt;
                    f.fire -= dt;
                    if f.fire <= 0.0 && on_screen {
                        f.fire = 3.6;
                        for k in 0..3 {
                            let mut h = Foe::new(Kind::Hopper, f.x, f.y + if f.up { 8.0 } else { -8.0 });
                            h.vx = -40.0 - 35.0 * k as f32;
                            h.vy = if f.up { 110.0 } else { -110.0 };
                            born.push(h);
                        }
                    }
                }
                Kind::Hopper => {
                    f.x += (f.vx - s * 0.5) * dt;
                    f.y += f.vy * dt;
                    if f.t > 0.5 { f.vy = (f.vy + (sy - f.y).signum() * 170.0 * dt).clamp(-90.0, 90.0); }
                }
                Kind::Volcano => {
                    f.x -= s * dt;
                    f.fire -= dt;
                    if f.fire <= 0.0 && f.x > -30.0 && f.x < w + 30.0 {
                        f.fire = 0.42;
                        let mut r = Foe::new(Kind::Rock, f.x, f.y - 3.0);
                        r.vx = rng.range(-110.0, 50.0);
                        r.vy = rng.range(-215.0, -140.0);
                        born.push(r);
                    }
                }
                Kind::Rock => {
                    f.vy += 150.0 * dt;
                    f.x += (f.vx - s) * dt;
                    f.y += f.vy * dt;
                }
                Kind::Head => {
                    f.x -= s * dt;
                    let was = before % 3.0 > 1.9;
                    if f.mouth_open() && !was && on_screen {
                        let a = (sy - f.y).atan2(sx - f.x);
                        let mut r = Foe::new(Kind::Ring, f.x - 10.0, f.y);
                        r.vx = a.cos() * 72.0;
                        r.vy = a.sin() * 72.0;
                        born.push(r);
                    }
                }
                Kind::Ring => {
                    let want = (sy - f.y).atan2(sx - f.x);
                    let have = f.vy.atan2(f.vx);
                    let d = (want - have + PI).rem_euclid(2.0 * PI) - PI;
                    let a = have + d.clamp(-0.9 * dt, 0.9 * dt);
                    let sp = (f.vx * f.vx + f.vy * f.vy).sqrt();
                    f.vx = a.cos() * sp;
                    f.vy = a.sin() * sp;
                    f.x += f.vx * dt;
                    f.y += f.vy * dt;
                }
                Kind::Crystal => {
                    f.x += (f.vx - s) * dt;
                    f.y += f.vy * dt;
                    let r = f.radius();
                    if (f.y < r && f.vy < 0.0) || (f.y > BOT as f32 - r && f.vy > 0.0) { f.vy = -f.vy; }
                }
                Kind::Amoeba => {
                    f.x -= (s + 20.0) * dt;
                    let (fl, ce) = land.at(cam + f.x);
                    f.y = (f.a + 24.0 * (f.t * 2.0).sin()).clamp(ce + 8.0, (fl - 8.0).max(ce + 8.0));
                }
                Kind::Flare | Kind::Shutter => f.x -= s * dt,
                Kind::Bird => {
                    f.x += f.vx * dt;
                    f.y = f.a + 34.0 * (f.t * 3.2).sin();
                    if !f.turned && f.x < w * 0.72 {
                        f.turned = true;
                        aim(bullets, (f.x, f.y), (sx, sy), speed, 0.0);
                    }
                }
            }
        }
        let bot = BOT as f32;
        let mut gone = Vec::new();
        for (i, f) in self.foes.iter().enumerate() {
            let out = match f.kind {
                Kind::Rusher => f.x > w + 30.0,
                Kind::Fan => f.x < -40.0 || (f.turned && f.x > w + 30.0),
                Kind::Rock => f.y > bot + 20.0 || f.x < -40.0 || (f.vy > 0.0 && self.land.solid(cam + f.x, f.y)),
                Kind::Ring | Kind::Hopper if f.t > 7.0 => true,
                _ => f.x < -60.0 || f.x > w + 400.0 || f.y < -80.0 || f.y > bot + 40.0,
            };
            if out { gone.push(i); }
        }
        for &i in gone.iter().rev() {
            let f = self.foes.swap_remove(i);
            if f.group != 0 { self.escaped(f.group); }
        }
        self.foes.extend(born);
    }

    fn move_bullets(&mut self, dt: f32) {
        let (land, cam) = (&self.land, self.cam);
        for b in &mut self.bullets {
            b.x += b.vx * dt;
            b.y += b.vy * dt;
        }
        self.bullets.retain(|b| {
            b.x > -40.0 && b.x < W as f32 + 40.0 && b.y > -10.0 && b.y < BOT as f32 + 10.0 && (b.beam || !land.solid(cam + b.x, b.y))
        });
    }

    fn start_boss(&mut self) {
        let st = &STAGES[self.stage];
        let r = self.round as i32;
        let w = W as f32;
        let mut b = Boss {
            kind: st.boss, x: w + 90.0, y: BOT as f32 / 2.0, tx: w - 100.0, t: 0.0, hp: 0, plates: Vec::new(), full: 0,
            fire: 2.0, spawn: 3.0, flash: 0.0, was_open: false, path: VecDeque::new(), blocks: Vec::new(), dying: 0.0,
        };
        match st.boss {
            BossKind::Core => {
                b.full = 5 + 2 * r;
                b.plates = vec![b.full; st.plates];
                b.hp = 6 + 2 * r;
                b.tx = w - 90.0;
            }
            BossKind::Head => { b.hp = 28 + 8 * r; b.tx = w - 70.0; }
            BossKind::Eye => { b.hp = 30 + 8 * r; b.tx = w - 80.0; }
            BossKind::Serpent => b.hp = 36 + 8 * r,
            BossKind::Brain => {
                b.hp = 10 + 2 * r;
                b.tx = w - 60.0;
                b.full = 5 + 2 * r;
                for c in 0..3 {
                    for k in 0..7 { b.blocks.push((-104.0 + c as f32 * 20.0, -48.0 + k as f32 * 14.0, b.full)); }
                }
            }
        }
        self.boss = Some(b);
        self.audio.play_loop(1, &self.m_boss, 0.9);
    }

    fn move_boss(&mut self, dt: f32) {
        let Some(mut b) = self.boss.take() else { return };
        b.t += dt;
        b.flash -= dt;
        let (sx, sy) = self.ship;
        let bot = BOT as f32;
        let r = self.round as f32;
        let speed = 100.0 + 25.0 * r;
        if b.dying > 0.0 {
            b.dying -= dt;
            if self.rng.chance(0.3) {
                let (x, y, size) = (b.x + self.rng.range(-50.0, 50.0), b.y + self.rng.range(-36.0, 36.0), self.rng.range(8.0, 20.0));
                self.boom(x, y, size);
                self.audio.play(&self.s_boom, 0.7);
            }
            if b.dying <= 0.0 {
                self.boom(b.x, b.y, 40.0);
                self.particles.burst(b.x, b.y, 80, 160.0, 1.2, 0xffd080);
                self.add(10000 * (self.stage as u32 + 1));
                self.foes.clear();
                self.bullets.clear();
                self.mode = Mode::Clear(4.0);
                self.audio.play(&self.s_clear, 1.0);
                return;
            }
            self.boss = Some(b);
            return;
        }
        if b.hp <= 0 {
            b.dying = 2.2;
            self.bullets.clear();
            self.audio.stop(1);
            self.audio.play(&self.s_big, 1.0);
            self.boss = Some(b);
            return;
        }
        let step = |from: f32, to: f32, v: f32| from + (to - from).clamp(-v * dt, v * dt);
        match b.kind {
            BossKind::Core => {
                b.x = step(b.x, b.tx, 70.0);
                b.y = step(b.y, sy, 40.0).clamp(46.0, bot - 46.0);
                b.fire -= dt;
                if b.fire <= 0.0 && b.x < W as f32 - 10.0 {
                    b.fire = (1.3 - 0.15 * r).max(0.7);
                    // From both wings and down the channel: lined up with
                    // the core is no place to wait.
                    let spread = if STAGES[self.stage].plates >= 4 { 42.0 } else { 0.0 };
                    for (x, dy, vy) in [(70.0, -26.0, -spread), (78.0, 0.0, 0.0), (70.0, 26.0, spread)] {
                        self.bullets.push(Bullet { x: b.x - x, y: b.y + dy, vx: -340.0 - 30.0 * r, vy, beam: true });
                    }
                    self.audio.play(&self.s_beam, 0.8);
                }
            }
            BossKind::Head => {
                b.x = step(b.x, b.tx, 60.0);
                b.y = bot / 2.0 + (bot / 2.0 - 48.0) * (b.t * 0.55).sin();
                let open = boss_open(&b);
                if open && !b.was_open && b.x < W as f32 - 20.0 {
                    for k in 0..5 {
                        let a = PI + (k as f32 - 2.0) * 0.25;
                        let mut ring = Foe::new(Kind::Ring, b.x - 22.0, b.y + 8.0);
                        ring.vx = a.cos() * (72.0 + 10.0 * r);
                        ring.vy = a.sin() * (72.0 + 10.0 * r);
                        self.foes.push(ring);
                    }
                }
                b.was_open = open;
            }
            BossKind::Eye => {
                b.x = step(b.x, b.tx + 12.0 * (b.t * 0.8).sin(), 60.0);
                b.y = step(b.y, bot / 2.0 + 52.0 * (b.t * 0.9).sin(), 50.0);
                if boss_open(&b) {
                    b.fire -= dt;
                    if b.fire <= 0.0 {
                        b.fire = 0.38 - 0.05 * r;
                        aim(&mut self.bullets, (b.x - 26.0, b.y), (sx, sy), speed + 10.0, 0.0);
                    }
                }
                b.spawn -= dt;
                if b.spawn <= 0.0 && b.x < W as f32 - 20.0 {
                    b.spawn = 4.2;
                    for s in [-1.0, 1.0] {
                        let mut a = Foe::new(Kind::Amoeba, b.x - 40.0, b.y + s * 22.0);
                        a.t = if s > 0.0 { 1.5 } else { 0.0 };
                        self.foes.push(a);
                    }
                }
            }
            BossKind::Serpent => {
                let (px, py) = (300.0 + 110.0 * (b.t * 0.8).sin(), bot / 2.0 + (bot / 2.0 - 34.0) * (b.t * 1.6).sin());
                b.x = px + (3.0 - b.t).max(0.0) * 90.0;
                b.y = py;
                b.path.push_front((b.x, b.y));
                b.path.truncate(SEGMENTS * 6 + 1);
                b.fire -= dt;
                if b.fire <= 0.0 && b.t > 3.0 {
                    b.fire = (1.6 - 0.2 * r).max(0.9);
                    for turn in [-0.25, 0.0, 0.25] { aim(&mut self.bullets, (b.x, b.y), (sx, sy), speed + 20.0, turn); }
                }
            }
            BossKind::Brain => {
                b.x = step(b.x, b.tx, 50.0);
                b.y = bot / 2.0 + 5.0 * (b.t * 1.3).sin();
                // A block shot away counts up from below zero, then grows back.
                for k in &mut b.blocks {
                    if k.2 < 0 {
                        k.2 += 1;
                        if k.2 == 0 { k.2 = b.full; }
                    }
                }
                b.spawn -= dt;
                if b.spawn <= 0.0 && b.x < W as f32 - 20.0 {
                    b.spawn = (2.8 - 0.3 * r).max(1.6);
                    for (y, vy) in [(4.0, 100.0), (bot - 4.0, -100.0)] {
                        let mut h = Foe::new(Kind::Hopper, b.x - 150.0, y);
                        h.vx = -60.0;
                        h.vy = vy;
                        self.foes.push(h);
                    }
                }
            }
        }
        self.boss = Some(b);
    }

    /// A shot at a point: true when the boss stopped it.
    fn boss_hit(&mut self, x: f32, y: f32, dmg: i32) -> bool {
        let Some(b) = self.boss.as_mut() else { return false };
        if b.dying > 0.0 || b.x > W as f32 + 20.0 { return false; }
        let (dx, dy) = (x - b.x, y - b.y);
        let open = boss_open(b);
        let mut hurt = false;
        let mut broke = None;
        let stop = match b.kind {
            BossKind::Core => {
                if (-50.0..=0.0).contains(&dx) && dy.abs() < 9.0 {
                    match b.plates.iter().position(|&p| p > 0) {
                        Some(i) if dx >= -46.0 + i as f32 * 8.0 => {
                            b.plates[i] -= dmg;
                            if b.plates[i] <= 0 { broke = Some((b.x - 44.0 + i as f32 * 8.0, b.y)); }
                            true
                        }
                        Some(_) => false,
                        None if dx >= -6.0 => { b.hp -= dmg; hurt = true; true }
                        None => false,
                    }
                } else {
                    core_solid(dx, dy)
                }
            }
            BossKind::Head => {
                if boss_solid(b, x, y) {
                    if open { b.hp -= dmg; hurt = true; }
                    true
                } else { false }
            }
            BossKind::Eye => {
                if open && (dx + 26.0).powi(2) + dy * dy <= 121.0 { b.hp -= dmg; hurt = true; true } else { boss_solid(b, x, y) }
            }
            BossKind::Serpent => {
                if dx * dx + dy * dy <= 121.0 { b.hp -= dmg; hurt = true; true } else { boss_solid(b, x, y) }
            }
            BossKind::Brain => {
                if let Some(k) = b.blocks.iter().position(|&(ox, oy, hp)| hp > 0 && dx >= ox && dx <= ox + 12.0 && dy >= oy && dy <= oy + 12.0) {
                    b.blocks[k].2 -= dmg;
                    if b.blocks[k].2 <= 0 {
                        // Five seconds of ticks before it grows back.
                        b.blocks[k].2 = -300;
                        broke = Some((b.x + b.blocks[k].0 + 6.0, b.y + b.blocks[k].1 + 6.0));
                    }
                    true
                } else if dx * dx + dy * dy <= 26.0 * 26.0 { b.hp -= dmg; hurt = true; true } else { false }
            }
        };
        if hurt || broke.is_some() { b.flash = 0.07; }
        if let Some((bx, by)) = broke {
            self.boom(bx, by, 10.0);
            self.add(200);
            self.audio.play(&self.s_boom, 0.7);
        } else if hurt {
            self.audio.play(&self.s_hurt, 0.8);
        } else if stop {
            self.audio.play(&self.s_hit, 0.5);
        }
        stop
    }

    fn kill_foe(&mut self, i: usize) {
        let f = self.foes.swap_remove(i);
        self.add(f.points());
        let big = matches!(f.kind, Kind::Hatch | Kind::Head | Kind::Shutter);
        self.boom(f.x, f.y, if big { 16.0 } else { 9.0 });
        self.particles.burst(f.x, f.y, if big { 30 } else { 12 }, 90.0, 0.5, f.color());
        self.audio.play(if big { &self.s_big } else { &self.s_boom }, 0.6);
        if f.carrier { self.drop_cap(f.x, f.y); }
        if f.group != 0 {
            if let Some(k) = self.groups.iter().position(|e| e.0 == f.group) {
                self.groups[k].1 -= 1;
                let (left, whole) = (self.groups[k].1, self.groups[k].2);
                if left == 0 {
                    self.groups.swap_remove(k);
                    if whole { self.drop_cap(f.x, f.y); }
                }
            }
        }
        if f.kind == Kind::Crystal && f.a > 1.5 {
            for k in 0..3 {
                let ang = k as f32 * 2.09 + self.rng.range(0.0, 1.0);
                let mut c = Foe::new(Kind::Crystal, f.x, f.y);
                c.a = f.a - 1.0;
                c.hp = c.a as i32 * 2;
                c.vx = ang.cos() * 55.0 - 10.0;
                c.vy = ang.sin() * 55.0;
                self.foes.push(c);
            }
        }
        if self.round > 0 && f.small() {
            let speed = 100.0 + 25.0 * self.round as f32;
            aim(&mut self.bullets, (f.x, f.y), self.ship, speed, 0.0);
        }
    }

    /// The shield takes the blow, or the ship is lost.
    fn hurt(&mut self) {
        if self.shield > 0 {
            self.shield -= 1;
            self.inv = 0.5;
            self.audio.play(&self.s_hurt, 1.0);
        } else {
            self.die();
        }
    }

    fn die(&mut self) {
        if self.mode != Mode::Play { return; }
        self.mode = Mode::Dying(2.4);
        let (x, y) = self.ship;
        self.boom(x, y, 22.0);
        self.particles.burst(x, y, 50, 140.0, 1.0, 0x7a92e0);
        self.particles.burst(x, y, 20, 90.0, 0.8, WHITE);
        self.audio.stop(1);
        self.audio.play(&self.s_die, 1.0);
    }

    fn collide(&mut self) {
        let cam = self.cam;
        // Shots against the boss, the foes and the cells.
        let mut dead: Vec<usize> = Vec::new();
        let mut cell_points = 0;
        for si in 0..self.shots.len() {
            let s = self.shots[si];
            if !s.alive { continue; }
            let dmg = if s.kind == ShotKind::Missile { 2 } else { 1 };
            let probes: &[f32] = if s.kind == ShotKind::Laser { &[0.0, -18.0, -34.0] } else { &[0.0] };
            if self.boss.is_some() && probes.iter().any(|&o| self.boss_hit(s.x + o, s.y, dmg)) {
                self.shots[si].alive = false;
                continue;
            }
            let mut stop = false;
            for fi in 0..self.foes.len() {
                let f = &mut self.foes[fi];
                if f.hp <= 0 || f.ghost() { continue; }
                let hit = if f.kind == Kind::Shutter {
                    shutter_rects(f, &self.land, cam).iter().any(|&(x0, y0, x1, y1)| probes.iter().any(|&o| s.x + o >= x0 && s.x + o <= x1 && s.y >= y0 && s.y <= y1))
                } else {
                    let r = f.radius() + 2.0;
                    probes.iter().any(|&o| (s.x + o - f.x).powi(2) + (s.y - f.y).powi(2) <= r * r)
                };
                if !hit { continue; }
                if f.armoured() {
                    self.particles.burst(s.x, s.y, 3, 40.0, 0.2, 0xc0c0d0);
                    self.audio.play(&self.s_hit, 0.4);
                    stop = true;
                    break;
                }
                f.hp -= dmg;
                f.flash = 0.07;
                if f.hp <= 0 {
                    dead.push(fi);
                    if s.kind == ShotKind::Laser { continue; }
                }
                stop = true;
                break;
            }
            if !stop {
                for c in self.cells.iter_mut().filter(|c| c.alive) {
                    let x0 = c.wx - cam;
                    if probes.iter().any(|&o| s.x + o >= x0 && s.x + o <= x0 + 10.0) && s.y >= c.y && s.y <= c.y + 10.0 {
                        c.alive = false;
                        c.back = 7.0;
                        cell_points += 10;
                        stop = true;
                        break;
                    }
                }
            }
            if stop { self.shots[si].alive = false; }
        }
        dead.sort_unstable();
        dead.dedup();
        for &i in dead.iter().rev() { self.kill_foe(i); }
        self.shots.retain(|s| s.alive);
        if cell_points > 0 { self.add(cell_points); }

        // The ship. Rock, cells, shutters and a boss's hull: no shield helps.
        let (x, y) = self.ship;
        let pts = [(x + 12.0, y), (x - 9.0, y), (x, y - 3.0), (x, y + 3.0)];
        let crash = pts.iter().any(|&(px, py)| self.land.solid(cam + px, py))
            || self.cells.iter().any(|c| c.alive && pts.iter().any(|&(px, py)| px >= c.wx - cam && px <= c.wx - cam + 10.0 && py >= c.y && py <= c.y + 10.0))
            || self.foes.iter().any(|f| f.kind == Kind::Shutter
                && shutter_rects(f, &self.land, cam).iter().any(|&(x0, y0, x1, y1)| pts.iter().any(|&(px, py)| px >= x0 && px <= x1 && py >= y0 && py <= y1)))
            || self.boss.as_ref().is_some_and(|b| b.dying <= 0.0 && pts.iter().any(|&(px, py)| boss_solid(b, px, py)));
        if crash {
            self.die();
            return;
        }
        // Capsules.
        let mut took = Vec::new();
        for (i, c) in self.caps.iter().enumerate() {
            if (c.x - x).powi(2) + (c.y - y).powi(2) < 14.0 * 14.0 { took.push(i); }
        }
        for &i in took.iter().rev() {
            let c = self.caps.swap_remove(i);
            self.collect(c.blue);
        }
        if self.inv > 0.0 { return; }
        // Foes and bullets: the shield takes those.
        let touch = self.foes.iter().position(|f| match f.kind {
            Kind::Volcano | Kind::Shutter => false,
            Kind::Flare => flare_balls(f).iter().any(|&(bx, by)| (bx - x).powi(2) + (by - y).powi(2) < 64.0),
            _ => (f.x - x).powi(2) + (f.y - y).powi(2) < (f.radius() + 3.0).powi(2),
        });
        if let Some(i) = touch {
            if self.shield > 0 && self.foes[i].small() { self.kill_foe(i); }
            self.hurt();
            return;
        }
        let shot = self.bullets.iter().position(|b| {
            if b.beam { x >= b.x - 2.0 && x <= b.x + 30.0 && (b.y - y).abs() < 4.0 } else { (b.x - x).powi(2) + (b.y - y).powi(2) < 16.0 }
        });
        if let Some(i) = shot {
            self.bullets.swap_remove(i);
            self.hurt();
        }
    }

    /// A position on screen, moved with the ground so things standing on
    /// it do not slide against it by a pixel.
    fn sx(&self, x: f32) -> i32 { (x + self.cam - self.cam.round()).round() as i32 }

    fn rock(&self, st: &Stage, wx: i32, y: i32, depth: i32) -> Rgb {
        match st.ground {
            Ground::Fire => {
                let n = noise(wx as f32 * 0.08, y as f32 * 0.1 + self.time * 2.2, 9);
                let k = (depth as f32 / 22.0).min(1.0);
                mix(mix(0xfff0a0, 0xff7a10, (k * 1.6).min(1.0)), 0x8a1000, (k * 0.8 + n * 0.45 - 0.2).clamp(0.0, 1.0))
            }
            Ground::Maze | Ground::Fort => {
                if depth < 2 { return st.rim; }
                let (px, py) = (wx.rem_euclid(24), y.rem_euclid(24));
                if px == 0 || py == 0 { return tint(st.rock, 0.55); }
                if px == 1 || py == 1 { return tint(st.rock, 1.3); }
                if (px == 4 || px == 20) && (py == 4 || py == 20) { return tint(st.rock, 1.5); }
                tint(st.rock, 1.0 - (depth as f32 / 160.0).min(0.35))
            }
            Ground::Cave => {
                if depth < 2 { return st.rim; }
                let n = noise(wx as f32 * 0.13, y as f32 * 0.13, 5);
                let c = mix(st.rock, 0x4a1020, ((n - 0.35) * 2.2).clamp(0.0, 1.0));
                if depth < 5 { mix(st.rim, c, (depth - 2) as f32 / 3.0) } else { tint(c, 1.0 - (depth as f32 / 120.0).min(0.4)) }
            }
            _ => {
                if depth < 2 { return st.rim; }
                let n = hash(wx >> 1, y >> 1, 3);
                let base = if depth < 6 { mix(st.rim, st.rock, (depth - 2) as f32 / 4.0) } else { st.rock };
                tint(base, (0.8 + 0.32 * n) * (1.0 - (depth as f32 / 150.0).min(0.45)))
            }
        }
    }

    fn draw_back(&self, f: &mut Frame) {
        for y in 0..BOT { f.hline(0, y, W, self.sky[y as usize]); }
        for (i, &(x, y, layer)) in self.stars.iter().enumerate() {
            let tw = hash(i as i32, (self.time * 3.0) as i32, 1) > 0.15;
            let c = [0x404868, 0x8088b0, 0xe0e8ff][layer as usize];
            if tw || layer < 2 { f.put(x as i32, y as i32, c); }
        }
    }

    fn draw_land(&self, f: &mut Frame) {
        let st = &STAGES[self.stage];
        let c0 = self.cam.round() as i32;
        let w = W as usize;
        let last = self.land.floor.len() - 1;
        for sx in 0..W {
            let wx = c0 + sx;
            let i = (wx.max(0) as usize).min(last);
            let (fl, ce) = (self.land.floor[i] as i32, self.land.ceil[i] as i32);
            for y in fl.max(0)..BOT { f.px[y as usize * w + sx as usize] = self.rock(st, wx, y, y - fl); }
            for y in 0..ce.min(BOT) { f.px[y as usize * w + sx as usize] = self.rock(st, wx, y, ce - 1 - y); }
        }
        let c0 = self.cam.round();
        for c in &self.cells {
            let (x, y) = ((c.wx - c0) as i32, c.y as i32);
            if c.alive {
                f.circle(x + 5, y + 5, 5, 0xc84a70);
                f.circle(x + 4, y + 4, 3, 0xf08aa8);
                f.circle(x + 6, y + 6, 1, 0x6a1030);
            } else if c.back < 2.0 {
                f.circle(x + 5, y + 5, ((2.0 - c.back) * 2.5) as i32, 0x8a3050);
            }
        }
    }

    fn draw_foe(&self, fr: &mut Frame, f: &Foe) {
        let a = &self.art;
        let (x, y) = (self.sx(f.x), f.y.round() as i32);
        let odd = ((f.t * 8.0) as i32 % 2) as usize;
        let up = f.up as usize;
        let s: &Sprite = match f.kind {
            Kind::Fan => &a.fan[odd],
            Kind::Dart => &a.dart,
            Kind::Swoop => &a.swoop,
            Kind::Rusher => {
                // Coming from behind: a blinking mark at the left edge first.
                if f.x < 0.0 {
                    if odd == 0 {
                        fr.rect(2, y - 3, 3, 7, 0xff9a30);
                        fr.rect(6, y - 1, 2, 3, 0xff9a30);
                    }
                    return;
                }
                &a.rusher
            }
            Kind::Walker => &a.walker[up * 2 + odd],
            Kind::Turret => {
                let ang = (self.ship.1 - f.y).atan2(self.ship.0 - f.x);
                let (ex, ey) = (x + (ang.cos() * 10.0) as i32, y + (ang.sin() * 10.0) as i32);
                fr.line(x, y, ex, ey, 0x303840);
                fr.line(x, y + 1, ex, ey + 1, 0x606a78);
                &a.turret[up]
            }
            Kind::Hatch => &a.hatch[up * 2 + (f.fire < 0.6 || f.fire > 3.1) as usize],
            Kind::Hopper => &a.hopper,
            Kind::Rock => &a.rock,
            Kind::Head => &a.head[up * 2 + f.mouth_open() as usize],
            Kind::Crystal => &a.crystal[(f.a as usize).clamp(1, 3) - 1],
            Kind::Amoeba => &a.amoeba[odd],
            Kind::Bird => &a.bird[odd],
            Kind::Ring => {
                let c = if odd == 0 { 0xffa040 } else { 0xffe080 };
                ring(fr, x, y, 4, c);
                ring(fr, x, y, 3, 0xff6020);
                return;
            }
            Kind::Volcano => {
                let glow = 0.5 + 0.5 * (self.time * 7.0).sin();
                fr.circle(x, y + 2, 4, mix(0xff4010, 0xffe060, glow));
                return;
            }
            Kind::Flare => {
                for (i, &(bx, by)) in flare_balls(f).iter().enumerate() {
                    if (by - f.y).abs() < 3.0 && i != 0 && i != 8 { continue; }
                    let r = 5 - (i as i32 - 4).abs() / 3;
                    fr.circle(self.sx(bx), by as i32, r, if (i + odd) % 2 == 0 { 0xff7010 } else { 0xffb030 });
                    fr.circle(self.sx(bx), by as i32, r - 2, 0xfff0a0);
                }
                return;
            }
            Kind::Shutter => {
                let hard = f.a > 0.5;
                for (x0, y0, x1, y1) in shutter_rects(f, &self.land, self.cam) {
                    if y1 - y0 < 1.0 { continue; }
                    let (px, py, h) = (self.sx(x0), y0 as i32, (y1 - y0) as i32);
                    let c = if f.flash > 0.0 { WHITE } else if hard { 0x707a94 } else { 0xa06a48 };
                    panel(fr, px, py, (x1 - x0) as i32, h, c);
                    for k in (4..h).step_by(8) { fr.hline(px + 2, py + k, 10, tint(c, 0.7)); }
                    let end = if y0 < 2.0 || y0 <= self.land.at(self.cam + f.x).1 + 1.0 { py + h - 3 } else { py };
                    fr.rect(px + 1, end, 12, 3, if hard { 0xffd040 } else { 0xff9040 });
                }
                return;
            }
        };
        if f.flash > 0.0 { put_white(fr, s, x, y); } else { put(fr, s, x, y); }
        if f.carrier && odd == 0 { fr.put(x, y, 0xffffff); }
    }

    fn draw_boss(&self, f: &mut Frame, b: &Boss) {
        let (x, y) = (self.sx(b.x), b.y.round() as i32);
        let flash = b.flash > 0.0;
        match b.kind {
            BossKind::Core => {
                let hull = match self.stage { 0 => 0x8a94a8, 2 => 0x6ab4d8, _ => 0x8a7ac8 };
                for s in [-1i32, 1] {
                    let wy = y + s * 26;
                    panel(f, x - 34, wy - 8, 96, 16, hull);
                    for i in 0..16 {
                        let d = (i as f32 - 7.5).abs();
                        let len = (18.0 - d * 2.2).max(0.0) as i32;
                        f.hline(x - 34 - len, wy - 8 + i, len, tint(hull, 1.15 - i as f32 * 0.03));
                    }
                    f.rect(x - 56, wy - 2, 12, 4, 0x40485a);
                    if b.fire < 0.3 { f.rect(x - 59, wy - 1, 3, 2, 0x90d0ff); }
                    let flick = if (self.time * 20.0) as i32 % 2 == 0 { 0xff9a30 } else { 0xffe070 };
                    f.rect(x + 62, wy - 3, 4, 6, flick);
                }
                panel(f, x - 2, y - 18, 64, 36, tint(hull, 0.8));
                panel(f, x - 50, y - 13, 50, 4, tint(hull, 1.1));
                panel(f, x - 50, y + 9, 50, 4, tint(hull, 1.1));
                f.rect(x - 50, y - 9, 50, 18, 0x080a12);
                for (i, &p) in b.plates.iter().enumerate() {
                    if p <= 0 { continue; }
                    let px = x - 46 + i as i32 * 8;
                    let c = mix(0x4a3a60, 0xc0d0ff, p as f32 / b.full as f32);
                    f.rect(px, y - 8, 4, 16, c);
                    f.vline(px, y - 8, 16, tint(c, 1.3));
                }
                let pulse = 0.5 + 0.5 * (b.t * 6.0).sin();
                let open = b.plates.iter().all(|&p| p <= 0);
                let cc = if flash && open { WHITE } else { mix(0x2a50ff, 0xa8e8ff, pulse) };
                f.circle(x + 2, y, 9, 0x10182c);
                f.circle(x + 2, y, 7, cc);
                f.circle(x, y - 2, 2, WHITE);
            }
            BossKind::Head => {
                let s = &self.art.big_head[boss_open(b) as usize];
                if flash { put_white(f, s, x, y); } else { put(f, s, x, y); }
            }
            BossKind::Eye => {
                let r = 32 + (2.0 * (b.t * 3.0).sin()) as i32;
                f.circle(x, y, r + 2, 0x5a1020);
                f.circle(x, y, r, 0xb04060);
                f.circle(x + 4, y - 4, r - 8, 0xd06080);
                for k in 0..7 {
                    let a = k as f32 * 0.9 + b.t * 0.3;
                    f.circle(x + (a.cos() * 18.0) as i32, y + (a.sin() * 18.0) as i32, 3, 0x802040);
                }
                let (ex, ey) = (x - 26, y);
                f.circle(ex, ey, 12, 0x3a0810);
                if boss_open(b) {
                    f.circle(ex, ey, 11, if flash { WHITE } else { 0xf8f0f0 });
                    let a = (self.ship.1 - b.y).atan2(self.ship.0 - b.x + 26.0);
                    let (ax, ay) = ((a.cos() * 4.0) as i32, (a.sin() * 4.0) as i32);
                    f.circle(ex + ax, ey + ay, 6, 0xd02020);
                    f.circle(ex + ax, ey + ay, 3, 0x100000);
                } else {
                    f.circle(ex, ey, 11, 0x902848);
                    f.hline(ex - 10, ey, 21, 0x3a0810);
                }
            }
            BossKind::Serpent => {
                let parts: Vec<(f32, f32)> = serpent_parts(b).collect();
                for (i, &(px, py)) in parts.iter().enumerate().rev() {
                    let (px, py) = (self.sx(px), py as i32);
                    f.circle(px, py, 8, 0x8a2800);
                    f.circle(px, py, 7, if i % 2 == 0 { 0xff7a10 } else { 0xffb030 });
                    f.circle(px - 2, py - 2, 2, 0xfff0a0);
                }
                f.circle(x, y, 12, 0x8a2800);
                f.circle(x, y, 11, if flash { WHITE } else { 0xffa020 });
                f.circle(x - 4, y - 4, 3, WHITE);
                f.circle(x - 5, y - 4, 1, 0x100000);
                f.line(x - 11, y + 3, x - 3, y + 6, 0x6a1800);
                f.circle(x + 3, y - 3, 3, 0xffe080);
            }
            BossKind::Brain => {
                for &(ox, oy, hp) in &b.blocks {
                    if hp > 0 { panel(f, x + ox as i32, y + oy as i32, 12, 12, mix(0x5a5a70, 0x9aa0b8, hp as f32 / b.full as f32)); }
                }
                let r = 26 + (1.5 * (b.t * 2.5).sin()) as i32;
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy > r * r { continue; }
                        let v = (dx as f32 * 0.45).sin() * (dy as f32 * 0.5 + b.t).cos();
                        let c = if flash { WHITE } else { mix(0xf0a0c0, 0x904060, v * 0.5 + 0.5) };
                        f.put(x + dx, y + dy, c);
                    }
                }
                f.circle(x - 6, y, 5, 0xffe0f0);
                f.circle(x - 7, y, 2, 0x401020);
            }
        }
    }

    fn draw_ship(&self, f: &mut Frame) {
        if matches!(self.mode, Mode::Dying(_) | Mode::Over(_) | Mode::Title) { return; }
        for k in (0..self.options).rev() {
            let (ox, oy) = self.option_at(k);
            put(f, &self.art.option[((self.time * 10.0) as usize + k) % 2], ox as i32, oy as i32);
        }
        if self.inv > 0.0 && self.shield == 0 && (self.time * 14.0) as i32 % 2 == 0 { return; }
        let (x, y) = (self.ship.0 as i32, self.ship.1 as i32);
        let flick = (self.time * 30.0) as i32 % 3;
        f.rect(x - 20 - flick, y - 1, 4 + flick, 3, 0xff9a30);
        f.rect(x - 18, y, 2, 1, 0xffffa0);
        put(f, &self.art.ship, x, y);
        if self.shield > 0 {
            let c = mix(0x2a60ff, 0xa0f0ff, 0.5 + 0.5 * (self.time * 9.0).sin());
            ring(f, x + 2, y, 15, c);
            if self.shield > 2 { ring(f, x + 2, y, 14, tint(c, 0.6)); }
        }
    }

    fn draw_shots(&self, f: &mut Frame) {
        for s in &self.shots {
            let (x, y) = (s.x.round() as i32, s.y.round() as i32);
            match s.kind {
                ShotKind::Shot => {
                    f.rect(x - 6, y - 1, 7, 2, 0xffe890);
                    f.rect(x, y - 1, 2, 2, WHITE);
                }
                ShotKind::Double => for k in 0..4 {
                    f.put(x - k, y + k, 0xffe890);
                    f.put(x - k + 1, y + k, 0xffe890);
                },
                ShotKind::Laser => {
                    f.rect(x - 36, y - 1, 36, 3, 0x40a0ff);
                    f.hline(x - 36, y, 36, 0xd0f8ff);
                }
                ShotKind::Missile => {
                    f.rect(x - 3, y - 1, 5, 3, 0xff8a30);
                    f.put(x + 2, y, WHITE);
                    f.put(x - 5, y, 0x904010);
                }
            }
        }
        let blink = (self.time * 16.0) as i32 % 2 == 0;
        for b in &self.bullets {
            let (x, y) = (b.x.round() as i32, b.y.round() as i32);
            if b.beam {
                f.rect(x, y - 1, 28, 3, 0x2a60ff);
                f.hline(x, y, 28, 0xc0e0ff);
            } else {
                f.rect(x - 1, y - 1, 3, 3, if blink { 0xff5a8a } else { 0xffa0c0 });
                f.put(x, y, WHITE);
            }
        }
    }

    fn draw_booms(&self, f: &mut Frame) {
        for &(x, y, t, r) in &self.booms {
            let k = t / 0.5;
            let rr = (r * (0.3 + 0.7 * k.sqrt())) as i32;
            let (cx, cy) = (self.sx(x), y as i32);
            f.circle(cx, cy, rr, mix(0xff8a20, 0x401008, k));
            if k < 0.6 { f.circle(cx, cy, (rr as f32 * 0.65) as i32, mix(0xffe070, 0xff6010, k / 0.6)); }
            if k < 0.3 { f.circle(cx, cy, (rr as f32 * 0.35) as i32, WHITE); }
        }
    }

    fn draw_hud(&self, f: &mut Frame) {
        let st = &STAGES[self.stage];
        f.rect(0, BOT, W, H - BOT, 0x000000);
        f.hline(0, BOT, W, tint(st.rim, 0.45));
        let (sw, gap) = (66, 4);
        let x0 = (W - 6 * sw - 5 * gap) / 2;
        let blink = (self.time * 6.0) as i32 % 2 == 0;
        for i in 0..6 {
            let (x, y) = (x0 + i * (sw + gap), BOT + 3);
            let (edge, fill, ink) = if self.bar == i {
                (0xffe0a0, if blink { 0xff8a20 } else { 0xe06010 }, 0x000000)
            } else {
                (0x3a4a90, 0x0c1430, 0xa0b8ff)
            };
            f.rect(x, y, sw, 11, edge);
            f.rect(x + 1, y + 1, sw - 2, 9, fill);
            if self.slot_open(i as usize) { f.text_centered(x + sw / 2, y + 3, BAR[i as usize], ink, false, 1); }
        }
        let y = BOT + 17;
        f.text_big(6, y, &format!("1P {:07}", self.score), TEXT);
        let hi = format!("HI {:07}", self.high.max(self.score));
        f.text_big(W / 2 - Frame::text_width(&hi, true, 1) / 2, y, &hi, 0x9090b0);
        let right = format!("STAGE {}  SHIPS {}", self.stage + 1 + self.round as usize * STAGES.len(), self.lives.saturating_sub(1));
        f.text_big(W - 6 - Frame::text_width(&right, true, 1), y, &right, st.rim);
    }
}

impl Game for Salvo {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        self.particles.update(dt);
        for b in &mut self.booms { b.2 += dt; }
        self.booms.retain(|b| b.2 < 0.5);
        self.white = (self.white - dt).max(0.0);
        let s = self.scroll;
        for st in &mut self.stars {
            st.0 -= (s * [0.15, 0.35, 0.6][st.2 as usize] + [6.0, 10.0, 16.0][st.2 as usize]) * dt;
            if st.0 < 0.0 { st.0 += W as f32; }
        }
        if let Some((_, t)) = self.note.as_mut() {
            *t -= dt;
            if *t <= 0.0 { self.note = None; }
        }
        match self.mode {
            Mode::Title => {
                self.scroll = 40.0;
                self.cam = (self.cam + 40.0 * dt) % (STAGES[0].length - W as f32);
                if input.pressed(Key::Space) || input.pressed(Key::Enter) { self.start_game(); }
            }
            Mode::Play => {
                if input.pressed(Key::Char('p')) { self.mode = Mode::Paused; } else { self.play(input, dt); }
            }
            Mode::Paused => if input.pressed(Key::Char('p')) || input.pressed(Key::Space) { self.mode = Mode::Play; },
            Mode::Dying(t) => {
                let t = t - dt;
                if t > 0.0 {
                    self.mode = Mode::Dying(t);
                } else {
                    self.lives -= 1;
                    if self.lives == 0 {
                        if funkey::store::record_score(GAME, self.score) { self.high = self.score; }
                        self.mode = Mode::Over(5.0);
                        self.audio.stop(1);
                    } else {
                        self.respawn();
                        self.mode = Mode::Play;
                    }
                }
            }
            Mode::Clear(t) => {
                let t = t - dt;
                if t > 0.0 {
                    self.mode = Mode::Clear(t);
                } else {
                    let mut n = self.stage + 1;
                    if n == STAGES.len() {
                        n = 0;
                        self.round += 1;
                    }
                    self.load(n);
                    self.enter();
                    self.mode = Mode::Play;
                }
            }
            Mode::Over(t) => {
                let t = t - dt;
                if t <= 0.0 || input.pressed(Key::Space) {
                    self.mode = Mode::Title;
                    self.load(0);
                    self.note = None;
                    self.audio.play_loop(1, &self.m_title, 0.8);
                } else {
                    self.mode = Mode::Over(t);
                }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        self.draw_back(f);
        self.draw_land(f);
        if self.mode == Mode::Title {
            f.dim(0.55);
            fancy_text(f, W / 2, 26 + ((self.time * 2.0).sin() * 4.0) as i32, "SALVO", 10, self.time);
            f.text_centered(W / 2, 104, "A TRIBUTE TO GRADIUS", TEXT, true, 2);
            f.text_centered(W / 2, 124, "KONAMI 1985", 0x9090b0, false, 2);
            f.blit_scaled(&self.art.ship, W / 2 - self.art.ship.w * 3 / 2, 140, 3, false);
            f.text_centered(W / 2, 198, &format!("HIGH SCORE {:07}", self.high), TEXT, true, 2);
            if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 220, "PRESS SPACE", GOLD, true, 2); }
            f.text_centered(W / 2, 244, "ARROWS FLY  SPACE FIRES  Z TAKES THE LIT POWER-UP  P PAUSES", 0x8080a0, true, 1);
            f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x505070);
            return;
        }
        for foe in &self.foes { self.draw_foe(f, foe); }
        let bob = ((self.time * 5.0).sin() * 1.5) as i32;
        for c in &self.caps { put(f, &self.art.cap[c.blue as usize], c.x as i32, c.y as i32 + bob); }
        if let Some(b) = &self.boss { self.draw_boss(f, b); }
        self.draw_shots(f);
        self.draw_ship(f);
        self.draw_booms(f);
        self.particles.draw(f, 0, 0);
        if self.white > 0.0 {
            let k = self.white / 0.3 * 0.7;
            for p in f.px[..(BOT * W) as usize].iter_mut() { *p = mix(*p, WHITE, k); }
        }
        self.draw_hud(f);
        if let Some((s, _)) = &self.note { f.text_centered(W / 2, 30, s, GOLD, true, 2); }
        if self.warning > 0.0 && (self.time * 4.0) as i32 % 2 == 0 {
            f.text_centered(W / 2, 96, "WARNING", 0xff3030, true, 4);
        }
        match self.mode {
            Mode::Paused => {
                f.dim(0.5);
                f.text_centered(W / 2, 110, "PAUSED", TEXT, true, 3);
            }
            Mode::Clear(_) => {
                fancy_text(f, W / 2, 80, "STAGE CLEAR", 5, self.time);
                f.text_centered(W / 2, 128, &format!("BONUS {}", 10000 * (self.stage + 1)), TEXT, true, 2);
            }
            Mode::Over(_) => {
                f.dim(0.5);
                f.text_centered(W / 2, 100, "GAME OVER", 0xe03030, true, 4);
                if self.score >= self.high && self.score > 0 { f.text_centered(W / 2, 140, "NEW HIGH SCORE", GOLD, true, 2); }
            }
            _ => {}
        }
    }
}

/// A sprite centred on a point.
fn put(f: &mut Frame, s: &Sprite, x: i32, y: i32) { f.blit(s, x - s.w / 2, y - s.h / 2); }

/// A sprite's shape in white, for the moment it is hit.
fn put_white(f: &mut Frame, s: &Sprite, x: i32, y: i32) {
    let (x0, y0) = (x - s.w / 2, y - s.h / 2);
    for yy in 0..s.h {
        for xx in 0..s.w {
            if s.px[(yy * s.w + xx) as usize] >> 24 != 0 { f.put(x0 + xx, y0 + yy, WHITE); }
        }
    }
}

fn ring(f: &mut Frame, cx: i32, cy: i32, r: i32, c: Rgb) {
    let n = (r * 7).max(12);
    for i in 0..n {
        let a = i as f32 / n as f32 * 2.0 * PI;
        f.put(cx + (a.cos() * r as f32).round() as i32, cy + (a.sin() * r as f32).round() as i32, c);
    }
}

/// A block of hull: lit on the top and left edges, dark on the others.
fn panel(f: &mut Frame, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
    f.rect(x, y, w, h, c);
    f.hline(x, y, w, tint(c, 1.35));
    f.vline(x, y, h, tint(c, 1.2));
    f.hline(x, y + h - 1, w, tint(c, 0.55));
    f.vline(x + w - 1, y, h, tint(c, 0.7));
}

/// A crystal, taller than wide, each quarter a facet lit from the top left.
fn crystal(r: i32) -> Sprite {
    let (w, h) = (r * 2 + 1, r * 3 + 1);
    let (cx, cy) = (r as f32, (r * 3) as f32 / 2.0);
    let mut px = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            if dx.abs() / (r as f32 + 0.5) + dy.abs() / (cy + 0.5) > 1.0 { continue; }
            let c = match (dx < 0.0, dy < 0.0) {
                (true, true) => 0xd8f8ff,
                (false, true) => 0x78c8ec,
                (true, false) => 0x4aa0d0,
                (false, false) => 0x2a6a9a,
            };
            px[(y * w + x) as usize] = 0xff00_0000 | c;
        }
    }
    outline(&Sprite { w, h, px }, 0x0c1830)
}

fn vflip(s: &Sprite) -> Sprite {
    let mut px = Vec::with_capacity(s.px.len());
    for y in (0..s.h).rev() { px.extend_from_slice(&s.px[(y * s.w) as usize..((y + 1) * s.w) as usize]); }
    Sprite { w: s.w, h: s.h, px }
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
            px[(y * w + x) as usize] = if on(x, y) {
                s.px[((y - 1) * s.w + x - 1) as usize]
            } else if on(x - 1, y) || on(x + 1, y) || on(x, y - 1) || on(x, y + 1) {
                0xff00_0000 | c
            } else {
                0
            };
        }
    }
    Sprite { w, h, px }
}

/// A small drawing made ready for the big screen: doubled, lit, outlined.
fn fancy(rows: &[&str], palette: &[(char, Rgb)]) -> Sprite {
    outline(&shade(&scale2x(&Sprite::from_rows(rows, palette))), 0x100818)
}

/// Big letters in a fire gradient, with a light sweeping across them and
/// a shadow under them.
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
            let mut c = mix(0xffe890, 0xe04010, yy as f32 / h as f32);
            let d = (xx + yy - band).abs();
            if d < scale * 2 { c = mix(c, WHITE, 1.0 - d as f32 / (scale * 2) as f32); }
            f.put(x0 + xx, y + yy, c);
        }
    }
}

/// The game with its sound on and the title tune playing.
fn salvo() -> Salvo {
    let mut game = Salvo::new();
    game.audio = Audio::open();
    game.audio.play_loop(1, &game.m_title, 0.8);
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let mut game = Salvo::new();
    // SALVO_BENCH=<frames> plays that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("SALVO_BENCH") {
        let n: u32 = n.parse().unwrap_or(1000);
        game.start_game();
        let mut f = Frame::new(W, H);
        let mut input = Input::new();
        input.inject(Key::Space);
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            game.update(&input, 1.0 / 60.0);
            game.draw(&mut f);
        }
        eprintln!("{:.3} ms a frame at {}x{} over {} frames", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n);
        return;
    }
    run(&mut salvo(), Config { width: W, height: H, fps: 60 });
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(salvo(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_leaves_room_to_fly() {
        for n in 0..STAGES.len() {
            let land = Land::new(n);
            for x in 0..land.floor.len() {
                let gap = land.floor[x] - land.ceil[x];
                assert!(gap >= 80, "stage {} at {}: only {} pixels open", n + 1, x, gap);
            }
            // Where the walls step, the openings on both sides overlap.
            for x in 1..land.floor.len() {
                let top = land.ceil[x].max(land.ceil[x - 1]);
                let bottom = land.floor[x].min(land.floor[x - 1]);
                assert!(bottom - top >= 50, "stage {} at {}: the opening jumps", n + 1, x);
            }
        }
    }

    #[test]
    fn capsules_light_the_bar_and_a_key_takes_it() {
        let mut g = Salvo::new();
        g.start_game();
        g.collect(false);
        assert_eq!(g.bar, 0);
        g.take_power();
        assert_eq!((g.speed, g.bar), (1, -1));
        for _ in 0..3 { g.collect(false); }
        g.take_power();
        assert_eq!(g.weapon, Weapon::Double);
        for _ in 0..4 { g.collect(false); }
        assert_eq!(g.bar, 3);
        g.take_power();
        assert_eq!(g.weapon, Weapon::Laser);
        // A slot that is full cannot be taken; the bar stays lit.
        g.options = MAX_OPTIONS;
        g.bar = 4;
        g.take_power();
        assert_eq!((g.options, g.bar), (MAX_OPTIONS, 4));
        // Past the shield, the bar starts over.
        g.bar = 5;
        g.collect(false);
        assert_eq!(g.bar, 0);
    }

    #[test]
    fn a_death_leaves_one_step_of_the_bar() {
        let mut g = Salvo::new();
        g.start_game();
        // Capsules on the bar, nothing taken.
        for _ in 0..4 { g.collect(false); }
        g.respawn();
        assert_eq!((g.bar, g.speed), (0, 0));
        // A power-up taken, the bar dark.
        g.take_power();
        assert_eq!((g.bar, g.speed), (-1, 1));
        g.respawn();
        assert_eq!((g.bar, g.speed), (0, 0));
        // Nothing lit and nothing taken: nothing to keep.
        g.bar = -1;
        g.respawn();
        assert_eq!(g.bar, -1);
    }

    #[test]
    fn options_follow_the_ships_path() {
        let mut g = Salvo::new();
        g.start_game();
        g.options = 2;
        let mut input = Input::new();
        input.inject(Key::Up);
        for _ in 0..40 { g.fly(&input, 1.0 / 60.0); }
        input.release_all();
        input.inject(Key::Right);
        for _ in 0..40 { g.fly(&input, 1.0 / 60.0); }
        let (a, b) = (g.option_at(0), g.option_at(1));
        assert!(a.0 < g.ship.0 && b.0 < a.0, "options trail behind: {:?} {:?} {:?}", g.ship, a, b);
        // The second option is still on the climb, left of where the ship turned.
        assert!(b.1 > a.1 - 0.01);
    }

    #[test]
    fn every_boss_can_be_beaten() {
        for n in 0..STAGES.len() {
            let mut g = Salvo::new();
            g.load(n);
            g.mode = Mode::Play;
            g.start_boss();
            let b = g.boss.as_mut().unwrap();
            b.x = b.tx;
            for _ in 0..600 {
                let b = g.boss.as_mut().unwrap();
                b.t = 2.0; // mouths and eyes open
                let (bx, by) = (b.x, b.y);
                let mut x = bx - 170.0;
                while x < bx + 40.0 && !g.boss_hit(x, by, 1) { x += 3.0; }
                if g.boss.as_ref().unwrap().hp <= 0 { break; }
            }
            assert!(g.boss.as_ref().unwrap().hp <= 0, "stage {} boss survives", n + 1);
        }
    }

    #[test]
    fn every_stage_plays_to_its_boss() {
        for n in 0..STAGES.len() {
            let mut g = Salvo::new();
            g.load(n);
            g.respawn();
            g.mode = Mode::Play;
            let mut input = Input::new();
            input.inject(Key::Space);
            // A ship that cannot be hit, kept in the middle of the opening.
            let mut ticks = 0;
            while g.boss.is_none() && ticks < 60 * 200 {
                g.inv = 1.0;
                let (fl, ce) = g.land.at(g.cam + g.ship.0);
                g.ship.1 = (fl + ce) / 2.0;
                let (fl, ce) = g.land.at(g.cam + g.ship.0 + 12.0);
                g.ship.1 = g.ship.1.clamp(ce + 5.0, fl - 5.0);
                g.cells.clear();
                g.foes.retain(|f| f.kind != Kind::Shutter);
                g.update(&input, 1.0 / 60.0);
                if g.mode != Mode::Play { g.mode = Mode::Play; }
                ticks += 1;
            }
            assert!(g.boss.is_some(), "stage {} never reached its boss", n + 1);
        }
    }
}
