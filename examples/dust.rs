//! dust: a way down through caves where every pixel is a grain of
//! something. Sand pours, water finds its level, oil floats and burns,
//! acid eats the rock, lava meets water and turns to stone. Nothing is
//! scripted: the cave is what its grains do.
//!
//!     cargo run --release --example dust
//!
//! Left and right walk, Up lifts you while the lamp oil lasts, Down aims
//! down. Z or Space fires the wand, X or 1 to 4 picks one: the bore
//! digs, the flame burns, the spout pours water, the bomb blows a hole.
//! Find the gate at the bottom of each cave. P pauses, Q quits.
//!
//! `DUST_START=<depth>` starts deeper; `DUST_BENCH=<frames>` plays that
//! many frames with no terminal and prints the time one takes.

use funkey::*;
use funkey::noise::{fbm, hash};

const W: i32 = 320;
const H: i32 = 180;
/// The cave: as wide as the screen, a little over three screens deep.
const WW: i32 = 320;
const WH: i32 = 600;
const N: usize = (WW * WH) as usize;
const GAME: &str = "dust";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";

// The kinds of grain. The order matters: 1 to 4 never move, 6 to 8 are
// powders, 9 to 12 liquids, 13 to 15 gases.
const AIR: u8 = 0;
const ROCK: u8 = 1;
const HARD: u8 = 2;
const DIRT: u8 = 3;
const WOOD: u8 = 4;
/// Wood on fire.
const EMBER: u8 = 5;
const SAND: u8 = 6;
const GOLD: u8 = 7;
const POWDER: u8 = 8;
const WATER: u8 = 9;
const OIL: u8 = 10;
const ACID: u8 = 11;
const LAVA: u8 = 12;
const FIRE: u8 = 13;
const SMOKE: u8 = 14;
const STEAM: u8 = 15;

/// How heavy a kind is: a grain sinks through whatever is lighter.
const DENS: [u8; 16] = [0, 9, 9, 9, 9, 9, 6, 6, 6, 3, 2, 4, 5, 0, 0, 0];

/// A grain can pass through it: air, a liquid or a gas.
fn fluid(k: u8) -> bool { k == AIR || k >= WATER }
fn liquid(k: u8) -> bool { (WATER..=LAVA).contains(&k) }
/// A body stands on it and a shot stops at it.
fn solid(k: u8) -> bool { (ROCK..=SAND).contains(&k) || k == POWDER }

/// The five caves, met in this order and then again, deeper.
struct Biome { name: &'static str, rock: Rgb, dirt: Rgb, pools: [(u8, u32); 3], powder: u32, wood: u32, gold: u32 }

const BIOMES: [Biome; 5] = [
    Biome { name: "THE WARREN", rock: 0x6a6258, dirt: 0x6e4a2c, pools: [(WATER, 9), (OIL, 1), (AIR, 0)], powder: 0, wood: 7, gold: 10 },
    Biome { name: "THE OIL SEEPS", rock: 0x4e5262, dirt: 0x4a3c34, pools: [(OIL, 9), (WATER, 3), (AIR, 0)], powder: 7, wood: 10, gold: 12 },
    Biome { name: "THE ACID MIRE", rock: 0x4a6050, dirt: 0x3c4a2c, pools: [(ACID, 8), (WATER, 4), (AIR, 0)], powder: 2, wood: 4, gold: 14 },
    Biome { name: "THE FORGE", rock: 0x6a4640, dirt: 0x50302a, pools: [(LAVA, 9), (WATER, 3), (OIL, 3)], powder: 5, wood: 3, gold: 16 },
    Biome { name: "THE VAULT", rock: 0x58506e, dirt: 0x40385a, pools: [(WATER, 4), (LAVA, 4), (ACID, 3)], powder: 4, wood: 5, gold: 34 },
];

fn biome(depth: u32) -> &'static Biome { &BIOMES[(depth.max(1) as usize - 1) % BIOMES.len()] }

/// The cave as grains, and the rules they move by.
struct World {
    kind: Vec<u8>,
    /// A grain's shade, or the ticks a flame or a puff has left.
    aux: Vec<u8>,
    /// The tick a grain last moved in, so none moves twice in one.
    stamp: Vec<u8>,
    /// The wall behind the air, as a shade.
    back: Vec<u8>,
    tick: u8,
    seed: u32,
    /// Blasts asked for by the grains: where, and how wide.
    booms: Vec<(i32, i32, i32)>,
}

impl World {
    fn new(seed: u32) -> World {
        World { kind: vec![AIR; N], aux: vec![0; N], stamp: vec![0; N], back: vec![0; N], tick: 0, seed: seed | 1, booms: Vec::new() }
    }

    fn rnd(&mut self) -> u32 {
        let mut x = self.seed;
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        self.seed = x;
        x
    }

    /// One time in `n`.
    fn one_in(&mut self, n: u32) -> bool { self.rnd() % n == 0 }

    fn inside(x: i32, y: i32) -> bool { x > 0 && y > 0 && x < WW - 1 && y < WH - 1 }

    fn get(&self, x: i32, y: i32) -> u8 { if World::inside(x, y) { self.kind[(y * WW + x) as usize] } else { HARD } }

    /// Put a grain there. The edge of the cave stays as it is.
    fn set(&mut self, x: i32, y: i32, k: u8) {
        if World::inside(x, y) { self.put((y * WW + x) as usize, k); }
    }

    fn put(&mut self, i: usize, k: u8) {
        let r = self.rnd();
        self.kind[i] = k;
        self.aux[i] = match k {
            FIRE => 14 + (r % 26) as u8,
            SMOKE => 40 + (r % 60) as u8,
            STEAM => 90 + (r % 90) as u8,
            EMBER => 90 + (r % 120) as u8,
            _ => (r >> 8) as u8,
        };
        self.stamp[i] = self.tick;
    }

    fn swap(&mut self, i: usize, j: usize) {
        self.kind.swap(i, j);
        self.aux.swap(i, j);
        self.stamp[i] = self.tick;
        self.stamp[j] = self.tick;
    }

    /// One tick for the rows `y0..y1`, from the bottom up.
    fn step(&mut self, y0: i32, y1: i32) {
        self.tick = self.tick.wrapping_add(1);
        let (y0, y1) = (y0.max(1), y1.min(WH - 1));
        let left = self.tick & 1 == 0;
        for y in (y0..y1).rev() {
            for n in 1..WW - 1 {
                let x = if left { n } else { WW - 1 - n };
                let i = (y * WW + x) as usize;
                let k = self.kind[i];
                if k < EMBER || self.stamp[i] == self.tick { continue; }
                match k {
                    SAND | GOLD | POWDER => self.powder(i),
                    WATER | OIL => self.flow(i, k, if k == WATER { 5 } else { 3 }),
                    ACID => self.acid(i),
                    LAVA => self.lava(i),
                    FIRE => self.fire(i, x, y),
                    EMBER => self.ember(i),
                    _ => self.gas(i, k),
                }
            }
        }
    }

    fn powder(&mut self, i: usize) {
        let w = WW as usize;
        let below = self.kind[i + w];
        if fluid(below) {
            if below == AIR || below >= FIRE || self.one_in(2) { self.swap(i, i + w); }
            return;
        }
        let (a, b) = if self.one_in(2) { (i + w - 1, i + w + 1) } else { (i + w + 1, i + w - 1) };
        for j in [a, b] {
            let t = self.kind[j];
            if t == AIR || t >= FIRE || (liquid(t) && self.one_in(3)) { self.swap(i, j); return; }
        }
    }

    /// A liquid: down, down to a side, or along the floor up to `reach`.
    fn flow(&mut self, i: usize, k: u8, reach: usize) {
        let w = WW as usize;
        let me = DENS[k as usize];
        if DENS[self.kind[i + w] as usize] < me { self.swap(i, i + w); return; }
        let right = self.one_in(2);
        let (a, b) = if right { (i + w + 1, i + w - 1) } else { (i + w - 1, i + w + 1) };
        for j in [a, b] {
            if DENS[self.kind[j] as usize] < me { self.swap(i, j); return; }
        }
        for dir in [right, !right] {
            let mut j = i;
            for _ in 0..reach {
                let n = if dir { j + 1 } else { j - 1 };
                let t = self.kind[n];
                if t != AIR && t < FIRE { break; }
                j = n;
            }
            if j != i { self.swap(i, j); return; }
        }
    }

    fn around(i: usize) -> [usize; 4] { [i - 1, i + 1, i - WW as usize, i + WW as usize] }

    /// Acid eats what it touches, and is used up by it.
    fn acid(&mut self, i: usize) {
        let j = World::around(i)[(self.rnd() % 4) as usize];
        if matches!(self.kind[j], ROCK | DIRT | WOOD | SAND | POWDER) && self.one_in(7) {
            let puff = self.one_in(3);
            self.put(j, if puff { SMOKE } else { AIR });
            if self.one_in(2) { self.put(i, AIR); return; }
        }
        self.flow(i, ACID, 3);
    }

    /// Lava sets fire to what burns and turns to stone in water.
    fn lava(&mut self, i: usize) {
        for j in World::around(i) {
            match self.kind[j] {
                WATER => { self.put(j, STEAM); self.put(i, ROCK); return; }
                OIL => self.put(j, FIRE),
                WOOD => if self.one_in(10) { self.put(j, EMBER) },
                POWDER => self.light_powder(j),
                AIR => if self.one_in(400) { self.put(j, FIRE) },
                _ => {}
            }
        }
        if self.one_in(3) { self.flow(i, LAVA, 1); }
    }

    /// Black powder goes off: a flame, and now and then a blast.
    fn light_powder(&mut self, j: usize) {
        self.put(j, FIRE);
        if self.one_in(5) && self.booms.len() < 24 {
            self.booms.push(((j % WW as usize) as i32, (j / WW as usize) as i32, 5));
        }
    }

    fn fire(&mut self, i: usize, x: i32, y: i32) {
        for j in World::around(i) {
            match self.kind[j] {
                WATER => { self.put(i, STEAM); return; }
                OIL => if self.one_in(5) {
                    // Oil burns long, and throws a flame up as it goes.
                    self.put(j, FIRE);
                    self.aux[j] = 40 + (self.rnd() % 40) as u8;
                    let up = i - WW as usize;
                    if self.kind[up] == AIR { self.put(up, FIRE); }
                },
                WOOD => if self.one_in(24) { self.put(j, EMBER) },
                POWDER => self.light_powder(j),
                _ => {}
            }
        }
        if self.aux[i] <= 1 {
            let smoke = self.one_in(5);
            self.put(i, if smoke { SMOKE } else { AIR });
            return;
        }
        self.aux[i] -= 1;
        // A flame on oil stays on it, so a pool burns to the last drop.
        if self.kind[i + WW as usize] != OIL && self.one_in(2) {
            let j = ((y - 1) * WW + x + (self.rnd() % 3) as i32 - 1) as usize;
            if self.kind[j] == AIR { self.swap(i, j); }
        }
    }

    /// Burning wood: it feeds flames into the air round it for a while.
    fn ember(&mut self, i: usize) {
        let j = World::around(i)[(self.rnd() % 4) as usize];
        match self.kind[j] {
            AIR => if self.one_in(4) { self.put(j, FIRE) },
            WATER => { self.put(j, STEAM); self.put(i, WOOD); return; }
            WOOD => if self.one_in(18) { self.put(j, EMBER) },
            OIL => self.put(j, FIRE),
            POWDER => self.light_powder(j),
            _ => {}
        }
        if self.tick & 1 == 0 {
            if self.aux[i] <= 1 { self.put(i, FIRE); } else { self.aux[i] -= 1; }
        }
    }

    /// Smoke and steam rise, through liquids too, and thin out.
    fn gas(&mut self, i: usize, k: u8) {
        if self.aux[i] <= 1 {
            let rain = k == STEAM && self.one_in(3);
            self.put(i, if rain { WATER } else { AIR });
            return;
        }
        self.aux[i] -= 1;
        let w = WW as usize;
        let r = self.rnd() % 4;
        let j = match r { 0 => i - w - 1, 1 => i - w + 1, _ => i - w };
        let t = self.kind[j];
        if t == AIR || liquid(t) { self.swap(i, j); return; }
        let j = if r & 1 == 0 { i - 1 } else { i + 1 };
        if self.kind[j] == AIR { self.swap(i, j); }
    }

    /// A blast: the middle is blown clear, the rim is left as rubble
    /// and flames. Gold is never lost, and the cave's edge never breaks.
    fn blast(&mut self, cx: i32, cy: i32, r: i32) {
        for dy in -r..=r {
            for dx in -r..=r {
                let d2 = dx * dx + dy * dy;
                if d2 > r * r { continue; }
                let (x, y) = (cx + dx, cy + dy);
                if !World::inside(x, y) { continue; }
                let i = (y * WW + x) as usize;
                let rim = d2 * 2 > r * r;
                match self.kind[i] {
                    HARD | GOLD => {}
                    POWDER => if d2 > 2 { self.light_powder(i) } else { self.put(i, FIRE) },
                    WATER => if !rim || self.one_in(2) { self.put(i, STEAM) },
                    LAVA | ACID | OIL if rim => {}
                    ROCK | DIRT if rim => { let s = self.one_in(3); self.put(i, if s { SAND } else { AIR }) },
                    _ => { let f = self.one_in(if rim { 2 } else { 4 }); self.put(i, if f { FIRE } else { AIR }) },
                }
            }
        }
    }

    /// The bore's bite: a round hole, with a little rubble left to fall.
    fn carve(&mut self, cx: i32, cy: i32, r: i32) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r { continue; }
                let (x, y) = (cx + dx, cy + dy);
                match self.get(x, y) {
                    ROCK => { let s = self.one_in(5); self.set(x, y, if s { SAND } else { AIR }) },
                    DIRT | WOOD | EMBER | SAND => self.set(x, y, AIR),
                    POWDER => if self.one_in(3) { self.set(x, y, AIR) },
                    _ => {}
                }
            }
        }
    }

    /// Something solid in the box.
    fn hits(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        for yy in y..y + h { for xx in x..x + w { if solid(self.get(xx, yy)) { return true; } } }
        false
    }

    /// How many grains of each kind lie in the box.
    fn count(&self, x: i32, y: i32, w: i32, h: i32) -> [u32; 16] {
        let mut c = [0; 16];
        for yy in y..y + h { for xx in x..x + w { c[self.get(xx, yy) as usize] += 1; } }
        c
    }

    /// A disc of one kind, laid only over the kinds in `over`.
    fn blob(&mut self, cx: i32, cy: i32, r: i32, k: u8, over: &[u8]) {
        for dy in -r..=r {
            for dx in -r..=r {
                let wob = (hash(cx + dx, cy + dy, 9) * 2.0) as i32;
                if dx * dx + dy * dy > (r - wob) * (r - wob) { continue; }
                if over.contains(&self.get(cx + dx, cy + dy)) { self.set(cx + dx, cy + dy, k); }
            }
        }
    }

    /// Pour a pool: the air that can be reached from a spot on the
    /// floor without rising more than `deep` above it.
    fn pool(&mut self, x: i32, mut y: i32, deep: i32, k: u8) {
        if self.get(x, y) != AIR { return; }
        while self.get(x, y + 1) == AIR { y += 1; }
        let top = y - deep;
        let mut todo = vec![(x, y)];
        let mut filled = 0;
        while let Some((x, y)) = todo.pop() {
            if y < top || self.get(x, y) != AIR || filled > 1400 { continue; }
            self.set(x, y, k);
            filled += 1;
            todo.extend([(x - 1, y), (x + 1, y), (x, y + 1), (x, y - 1)]);
        }
    }

    /// A cave grown from noise: rock with holes in it, veins and pools
    /// by the biome, a room to start in and a room for the gate.
    fn cave(depth: u32, rng: &mut Rng) -> (World, (i32, i32), (i32, i32)) {
        let b = biome(depth);
        let mut w = World::new(rng.next_u32());
        let seed = rng.next_u32();
        for y in 0..WH {
            for x in 0..WW {
                let i = (y * WW + x) as usize;
                let (fx, fy) = (x as f32, y as f32);
                w.back[i] = (22.0 + 34.0 * fbm(fx / 24.0, fy / 24.0, 3, seed, 256) + 14.0 * hash(x, y, seed)) as u8;
                w.aux[i] = (255.0 * hash(x, y, seed ^ 5)) as u8;
                if x < 3 || y < 3 || x >= WW - 3 || y >= WH - 3 { w.kind[i] = HARD; continue; }
                // Winding tunnels where one noise crosses its middle, and
                // halls where another runs high.
                let worm = (fbm(fx / 70.0, fy / 52.0, 4, seed ^ 1, 256) - 0.5).abs();
                let hall = fbm(fx / 60.0, fy / 44.0, 4, seed ^ 2, 256);
                if worm < 0.034 || hall > 0.585 { continue; }
                let earth = fbm(fx / 50.0, fy / 30.0, 3, seed ^ 3, 256) > 0.6;
                w.kind[i] = if earth { DIRT } else { ROCK };
            }
        }
        let start = (WW / 2, 26);
        let gate = (rng.range(40.0, (WW - 40) as f32) as i32, WH - 26);
        let spot = |rng: &mut Rng| (rng.range(8.0, (WW - 8) as f32) as i32, rng.range(50.0, (WH - 12) as f32) as i32);
        for _ in 0..16 { let (x, y) = spot(rng); w.blob(x, y, rng.range(5.0, 12.0) as i32, SAND, &[ROCK, DIRT]); }
        for _ in 0..b.powder { let (x, y) = spot(rng); w.blob(x, y, rng.range(4.0, 8.0) as i32, POWDER, &[ROCK, DIRT]); }
        let mut veins = 0;
        for _ in 0..4000 {
            // A vein of gold lies in the rock, never loose in the air.
            let (x, y) = spot(rng);
            if veins < b.gold + depth && matches!(w.get(x, y), ROCK | DIRT) { w.blob(x, y, rng.range(3.0, 6.0) as i32, GOLD, &[ROCK, DIRT, SAND]); veins += 1; }
        }
        for _ in 0..b.wood {
            let (x, y) = spot(rng);
            for dx in 0..rng.range(18.0, 50.0) as i32 {
                for dy in 0..3 { if w.get(x + dx, y + dy) == AIR { w.set(x + dx, y + dy, WOOD); } }
            }
        }
        for (cx, cy) in [start, gate] {
            w.blob(cx, cy, 15, AIR, &[ROCK, DIRT, SAND, POWDER, WOOD, GOLD]);
            for dx in -12..=12 { for dy in 9..12 { if w.get(cx + dx, cy + dy) == AIR { w.set(cx + dx, cy + dy, ROCK); } } }
        }
        for (k, n) in b.pools {
            for _ in 0..n * 3 {
                let (x, y) = spot(rng);
                // No pool is poured into the two rooms.
                if (y - start.1).abs() < 40 && (x - start.0).abs() < 60 || y > WH - 60 && (x - gate.0).abs() < 40 { continue; }
                if k != AIR { w.pool(x, y, rng.range(5.0, 16.0) as i32, k); }
            }
        }
        (w, start, gate)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Shot { Bore, Flame, Drop, Bomb, Spit }

struct Bolt { x: f32, y: f32, vx: f32, vy: f32, kind: Shot, life: f32 }

#[derive(Clone, Copy, PartialEq, Debug)]
enum FoeKind { Slime, Bat, Imp }

struct Foe { kind: FoeKind, x: f32, y: f32, vx: f32, vy: f32, hp: f32, t: f32, hurt: f32 }

impl Foe {
    fn size(&self) -> (i32, i32) { match self.kind { FoeKind::Slime => (7, 5), FoeKind::Bat => (7, 4), FoeKind::Imp => (6, 8) } }
}

#[derive(Clone, Copy, PartialEq)]
enum Find { Heart, Bombs }

struct Me { x: f32, y: f32, vx: f32, vy: f32, face: i32, hp: f32, mana: f32, oil: f32, safe: f32, ground: bool, walk: f32 }

const ME_W: i32 = 5;
const ME_H: i32 = 9;
const WANDS: [&str; 4] = ["BORE", "FLAME", "SPOUT", "BOMB"];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Play, Down, Dead }

struct Sounds { bore: Sample, dig: Sample, flame: Sample, spout: Sample, boom: Sample, gold: Sample, hurt: Sample, die: Sample, gate: Sample, kill: Sample, find: Sample, hum: Sample }

impl Sounds {
    fn new() -> Sounds {
        use Wave::*;
        Sounds {
            bore: Sample::sweep(Square, 900.0, 300.0, 0.07, 0.18),
            dig: Sample::noise(0.12, 0.35),
            flame: Sample::synth(0.5, |_, r| r * 0.16),
            spout: Sample::synth(0.5, |t, r| r * 0.07 * (0.6 + 0.4 * (t * 40.0).sin())),
            boom: Sample::synth(0.7, |t, r| (r * 0.8 + (t * 55.0 * std::f32::consts::TAU).sin() * 0.6) * (1.0 - t / 0.7).powi(2)),
            gold: Sample::tone(Triangle, 1320.0, 0.04, 0.3).then(&Sample::tone(Triangle, 1760.0, 0.07, 0.3)),
            hurt: Sample::sweep(Saw, 300.0, 120.0, 0.15, 0.35),
            die: Sample::sweep(Saw, 400.0, 40.0, 0.9, 0.4),
            gate: Sample::sweep(Sine, 220.0, 880.0, 0.7, 0.4).mixed(&Sample::sweep(Triangle, 330.0, 1320.0, 0.7, 0.25)),
            kill: Sample::sweep(Square, 500.0, 90.0, 0.18, 0.25),
            find: Sample::tone(Triangle, 660.0, 0.08, 0.35).then(&Sample::tone(Triangle, 990.0, 0.16, 0.35)),
            hum: Sample::synth(4.0, |t, r| {
                let tau = std::f32::consts::TAU;
                ((t * 55.0 * tau).sin() * 0.10 + (t * 82.5 * tau).sin() * 0.06 + r * 0.012) * (0.75 + 0.25 * (t * 0.5 * tau).sin())
            }),
        }
    }
}

struct Dust {
    world: World,
    me: Me,
    wand: usize, bombs: u32, cool: f32,
    bolts: Vec<Bolt>, foes: Vec<Foe>, finds: Vec<(Find, i32, i32)>,
    gate: (i32, i32),
    parts: Particles,
    cam: f32, shake: f32,
    depth: u32, start: u32, score: u32, gold: u32, high: u32,
    mode: Mode, timer: f32, time: f32,
    /// Which of the two loops is playing: the flame, the spout.
    loops: [bool; 2],
    light: Vec<[f32; 3]>,
    rng: Rng, audio: Audio, snd: Sounds,
}

/// The light map has one point for every eight pixels each way.
const LW: usize = (W / 8 + 1) as usize;
const LH: usize = (H / 8 + 2) as usize;

impl Dust {
    fn new() -> Dust {
        let mut rng = Rng::from_time();
        let mut game = Dust {
            world: World::new(rng.next_u32()),
            me: Me { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, face: 1, hp: 100.0, mana: 100.0, oil: 1.0, safe: 0.0, ground: false, walk: 0.0 },
            wand: 0, bombs: 3, cool: 0.0, bolts: Vec::new(), foes: Vec::new(), finds: Vec::new(), gate: (0, 0),
            parts: Particles::new(), cam: 0.0, shake: 0.0,
            depth: 1, start: 1, score: 0, gold: 0, high: funkey::scores::best(GAME),
            mode: Mode::Title, timer: 0.0, time: 0.0, loops: [false; 2], light: vec![[0.0; 3]; LW * LH],
            rng, audio: Audio::off(), snd: Sounds::new(),
        };
        game.title_world();
        game
    }

    /// The title: the game's name as stone, for the grains to fall on.
    fn title_world(&mut self) {
        self.world = World::new(self.rng.next_u32());
        let mut f = Frame::new(W, H);
        f.text_centered(W / 2, 46, "DUST", WHITE, true, 9);
        for y in 0..H {
            for x in 0..W {
                let i = (y * WW + x) as usize;
                self.world.back[i] = 26 + (10.0 * hash(x, y, 3)) as u8;
                self.world.aux[i] = (255.0 * hash(x, y, 4)) as u8;
                if f.get(x, y) != BLACK || y >= H - 3 || x < 2 || x >= W - 2 { self.world.kind[i] = HARD; }
            }
        }
        for x in 0..WW { self.world.kind[(H * WW + x) as usize] = HARD; }
        (self.cam, self.timer) = (0.0, 0.0);
    }

    /// What falls on the title, by turns: sand, water, oil, then lava
    /// to set the oil alight.
    fn title_rain(&mut self) {
        let t = self.timer;
        let drop = |g: &mut Dust, k: u8, n: u32, x0: f32, x1: f32| {
            for _ in 0..n { let x = g.rng.range(x0, x1) as i32; if g.world.get(x, 2) == AIR { g.world.set(x, 2, k); } }
        };
        if t < 12.0 { drop(self, SAND, 5, 30.0, 290.0); }
        if t < 7.0 { drop(self, GOLD, 1, 60.0, 260.0); }
        if (8.0..18.0).contains(&t) { drop(self, WATER, 14, 6.0, 314.0); }
        if (18.0..24.0).contains(&t) { drop(self, OIL, 10, 6.0, 314.0); }
        if (27.0..28.0).contains(&t) && self.world.tick % 4 == 0 { drop(self, LAVA, 1, 20.0, 300.0); }
        if (40.0..47.0).contains(&t) { drop(self, ACID, 3, 40.0, 280.0); }
        if t > 56.0 { self.title_world(); }
    }

    fn begin(&mut self) {
        (self.score, self.gold, self.bombs, self.wand) = (0, 0, 3, 0);
        (self.me.hp, self.me.mana, self.me.oil) = (100.0, 100.0, 1.0);
        self.enter(self.start);
        self.audio.play_loop(1, &self.snd.hum, 1.0);
    }

    /// A new cave, with its creatures and the things to find.
    fn enter(&mut self, depth: u32) {
        let (world, start, gate) = World::cave(depth, &mut self.rng);
        (self.world, self.gate, self.depth) = (world, gate, depth);
        self.me.x = (start.0 - ME_W / 2) as f32;
        self.me.y = (start.1 + 9 - ME_H) as f32;
        (self.me.vx, self.me.vy, self.me.safe) = (0.0, 0.0, 2.0);
        self.cam = 0.0;
        self.bolts.clear();
        self.foes.clear();
        self.finds.clear();
        self.parts.list.clear();
        let kinds: &[FoeKind] = match depth {
            1 => &[FoeKind::Slime, FoeKind::Slime, FoeKind::Bat],
            2 => &[FoeKind::Slime, FoeKind::Bat, FoeKind::Bat, FoeKind::Imp],
            _ => &[FoeKind::Slime, FoeKind::Bat, FoeKind::Imp],
        };
        let mut tries = 0;
        while self.foes.len() < (5 + depth * 2).min(22) as usize && tries < 4000 {
            tries += 1;
            let (x, y) = (self.rng.range(10.0, (WW - 18) as f32) as i32, self.rng.range(120.0, (WH - 50) as f32) as i32);
            let kind = *self.rng.pick(kinds).unwrap();
            let foe = Foe { kind, x: x as f32, y: y as f32, vx: 0.0, vy: 0.0, hp: match kind { FoeKind::Slime => 3.0, FoeKind::Bat => 1.0, FoeKind::Imp => 4.0 }, t: self.rng.range(0.5, 2.0), hurt: 0.0 };
            let (w, h) = foe.size();
            if self.world.count(x - 1, y - 1, w + 2, h + 2)[AIR as usize] == ((w + 2) * (h + 2)) as u32 { self.foes.push(foe); }
        }
        let mut tries = 0;
        while self.finds.len() < 5 && tries < 4000 {
            tries += 1;
            let (x, y) = (self.rng.range(10.0, (WW - 14) as f32) as i32, self.rng.range(90.0, (WH - 40) as f32) as i32);
            if self.world.count(x, y, 7, 7)[AIR as usize] == 49 && solid(self.world.get(x + 3, y + 8)) {
                self.finds.push((if self.finds.len() < 2 { Find::Heart } else { Find::Bombs }, x, y));
            }
        }
        (self.mode, self.timer) = (Mode::Down, 2.2);
    }

    fn sound(&mut self, s: fn(&Sounds) -> &Sample, vol: f32) { self.audio.play(s(&self.snd), vol); }

    /// Where the wand points: the way you face, or up or down with the
    /// arrows, slanted when you walk as well.
    fn aim(&self, input: &Input) -> (f32, f32) {
        let (ax, ay) = (input.axis_x(), input.held(Key::Down) as i32 - input.held(Key::Up) as i32);
        if ay == 0 { return (self.me.face as f32, 0.0); }
        if ax == 0 { return (0.0, ay as f32); }
        (ax as f32 * 0.707, ay as f32 * 0.707)
    }

    fn shoot(&mut self, input: &Input) {
        let (dx, dy) = self.aim(input);
        // From your middle, so no thin wall beside you is skipped.
        let (x, y) = (self.me.x.round() + 2.0, self.me.y.round() + 4.0);
        let mut r = |a: f32| self.rng.range(-a, a);
        match self.wand {
            0 if self.me.mana >= 4.0 => {
                self.me.mana -= 4.0;
                self.cool = 0.16;
                self.bolts.push(Bolt { x, y, vx: dx * 230.0, vy: dy * 230.0, kind: Shot::Bore, life: 0.5 });
                self.sound(|s| &s.bore, 0.6);
            }
            1 if self.me.mana >= 1.0 => {
                self.me.mana -= 0.9;
                self.cool = 0.02;
                for _ in 0..2 {
                    let (jx, jy, l) = (r(26.0), r(26.0), 0.3 + r(0.1));
                    self.bolts.push(Bolt { x, y, vx: dx * 150.0 + jx, vy: dy * 150.0 + jy - 10.0, kind: Shot::Flame, life: l });
                }
            }
            2 if self.me.mana >= 1.0 => {
                self.me.mana -= 0.7;
                self.cool = 0.02;
                for _ in 0..2 {
                    let (jx, jy) = (r(16.0), r(16.0));
                    self.bolts.push(Bolt { x, y, vx: dx * 170.0 + jx, vy: dy * 170.0 + jy - 30.0, kind: Shot::Drop, life: 1.2 });
                }
            }
            3 if self.bombs > 0 => {
                self.bombs -= 1;
                self.cool = 0.45;
                self.bolts.push(Bolt { x, y, vx: dx * 150.0 + self.me.vx * 0.5, vy: dy * 150.0 - 60.0, kind: Shot::Bomb, life: 1.6 });
                self.sound(|s| &s.bore, 0.9);
            }
            _ => {}
        }
    }

    fn hurt(&mut self, amount: f32, hard: bool) {
        if self.mode != Mode::Play || (hard && self.me.safe > 0.0) { return; }
        self.me.hp -= amount;
        if hard {
            self.me.safe = 0.7;
            self.sound(|s| &s.hurt, 1.0);
        }
        if self.me.hp <= 0.0 {
            self.me.hp = 0.0;
            self.parts.burst(self.me.x + 2.5, self.me.y + 4.0, 60, 90.0, 1.2, 0x60a8ff);
            self.sound(|s| &s.die, 1.0);
            self.quiet();
            self.audio.stop(1);
            if !cfg!(test) { funkey::scores::record(GAME, self.score); }
            self.high = self.high.max(self.score);
            (self.mode, self.timer) = (Mode::Dead, 5.0);
        }
    }

    /// Stop the flame's and the spout's sound.
    fn quiet(&mut self) {
        for (n, on) in self.loops.iter_mut().enumerate() { if *on { self.audio.stop(2 + n as u8); *on = false; } }
    }

    fn boom(&mut self, x: i32, y: i32, r: i32) {
        self.world.blast(x, y, r);
        let reach = r as f32 + 6.0;
        let near = |px: f32, py: f32| ((px - x as f32).powi(2) + (py - y as f32).powi(2)).sqrt() < reach;
        if near(self.me.x + 2.5, self.me.y + 4.5) { self.hurt(r as f32 * 2.2, true); }
        for f in &mut self.foes { if near(f.x + 3.0, f.y + 3.0) { f.hp -= r as f32; f.hurt = 0.2; } }
        self.parts.burst(x as f32, y as f32, (r * 3) as usize, 30.0 + r as f32 * 8.0, 0.6, 0xffc040);
        self.shake = self.shake.max(r as f32 * 0.35);
        let vol = (r as f32 / 14.0).min(1.0);
        self.sound(|s| &s.boom, vol);
    }

    fn body(&mut self, input: &Input, dt: f32) {
        let ax = input.axis_x();
        let up = input.held(Key::Up);
        let (bx, by) = (self.me.x.round() as i32, self.me.y.round() as i32);
        let c = self.world.count(bx, by, ME_W, ME_H);
        let wet = (c[WATER as usize] + c[OIL as usize] + c[ACID as usize] + c[LAVA as usize]) as f32 / (ME_W * ME_H) as f32;
        if ax != 0 { self.me.face = ax; self.me.walk += dt * 10.0; }
        let speed = 58.0 * (1.0 - wet * 0.45);
        self.me.vx += (ax as f32 * speed - self.me.vx) * (dt * 14.0).min(1.0);
        self.me.vy += (420.0 - wet * 330.0) * dt;
        if up && self.me.oil > 0.0 {
            self.me.vy -= 900.0 * dt;
            self.me.oil -= dt * 0.55;
            if self.world.tick % 2 == 0 {
                self.parts.spawn(funkey::particles::Particle { x: self.me.x + 2.5, y: self.me.y + 9.0, vx: self.rng.range(-12.0, 12.0), vy: 50.0, life: 0.25, color: 0xffb040, size: 1 });
            }
        } else if up && wet > 0.3 {
            self.me.vy -= 620.0 * dt;
        }
        if self.me.ground || wet > 0.5 { self.me.oil = (self.me.oil + dt * 1.6).min(1.0); } else if !up { self.me.oil = (self.me.oil + dt * 0.25).min(1.0); }
        if wet > 0.2 { self.me.vy *= 1.0 - (dt * 4.0).min(0.9); }
        self.me.vy = self.me.vy.clamp(-85.0, 240.0);

        // Buried: sand that fell on you gives way, and you rise out of it.
        if self.world.hits(bx, by, ME_W, ME_H) {
            for y in by..by + ME_H { for x in bx..bx + ME_W { if matches!(self.world.get(x, y), SAND | POWDER) { self.world.set(x, y, AIR); } } }
            if self.world.hits(bx, by, ME_W, ME_H) && !self.world.hits(bx, by - 1, ME_W, ME_H) { self.me.y -= 1.0; }
        }
        let (mut x, mut y) = (self.me.x.round() as i32, self.me.y.round() as i32);
        let (nx, ny) = ((self.me.x + self.me.vx * dt).round() as i32, (self.me.y + self.me.vy * dt).round() as i32);
        let w = &self.world;
        let mut blocked = false;
        while x != nx {
            let s = (nx - x).signum();
            if !w.hits(x + s, y, ME_W, ME_H) { x += s; continue; }
            // A step up of three grains or less is walked over.
            match (1..=3).find(|&k| !w.hits(x + s, y - k, ME_W, ME_H) && !w.hits(x, y - k, ME_W, ME_H)) {
                Some(k) if self.me.vy >= -1.0 => { x += s; y -= k; }
                _ => { blocked = true; break; }
            }
        }
        if blocked { (self.me.x, self.me.vx) = (x as f32, 0.0); } else { self.me.x += self.me.vx * dt; }
        let lift = self.me.y.round() as i32 - y;
        let ny = ny - lift;
        self.me.ground = false;
        let mut stopped = false;
        while y != ny {
            let s = (ny - y).signum();
            if w.hits(x, y + s, ME_W, ME_H) { stopped = true; self.me.ground = s > 0; break; }
            y += s;
        }
        if stopped { (self.me.y, self.me.vy) = (y as f32, 0.0); } else { self.me.y += self.me.vy * dt - lift as f32; }
        if !self.me.ground && self.me.vy >= 0.0 && w.hits(x, y + 1, ME_W, ME_H) { self.me.ground = true; }

        // What the grains round you do to you, and the gold you pick up.
        let c = self.world.count(x, y, ME_W, ME_H);
        let burn = (c[FIRE as usize] + c[EMBER as usize]).min(6) as f32 * 6.0 + c[LAVA as usize].min(8) as f32 * 14.0 + c[ACID as usize].min(8) as f32 * 4.0;
        if burn > 0.0 {
            self.hurt(burn * dt, false);
            if self.world.tick % 12 == 0 { self.sound(|s| &s.hurt, 0.5); }
        }
        let mut got = 0;
        for yy in y - 2..y + ME_H + 2 {
            for xx in x - 2..x + ME_W + 2 { if self.world.get(xx, yy) == GOLD { self.world.set(xx, yy, AIR); got += 1; } }
        }
        if got > 0 {
            self.gold += got;
            self.score += got * 10;
            if self.world.tick % 3 == 0 { self.sound(|s| &s.gold, 0.7); }
        }
        let (mx, my) = (x + 2, y + 4);
        let mut taken = None;
        for (n, &(what, fx, fy)) in self.finds.iter().enumerate() {
            if (fx + 3 - mx).abs() < 7 && (fy + 3 - my).abs() < 8 { taken = Some((n, what)); }
        }
        if let Some((n, what)) = taken {
            self.finds.remove(n);
            match what { Find::Heart => self.me.hp = (self.me.hp + 35.0).min(100.0), Find::Bombs => self.bombs += 2 }
            self.score += 50;
            self.sound(|s| &s.find, 1.0);
        }
        if (self.gate.0 - mx).abs() < 7 && (self.gate.1 - my).abs() < 9 {
            self.score += 500 * self.depth;
            self.me.hp = (self.me.hp + 20.0).min(100.0);
            self.quiet();
            self.sound(|s| &s.gate, 1.0);
            self.enter(self.depth + 1);
        }
    }

    fn fly(&mut self, dt: f32) {
        let mut bolts = std::mem::take(&mut self.bolts);
        let (px, py) = (self.me.x, self.me.y);
        bolts.retain_mut(|b| {
            b.life -= dt;
            if matches!(b.kind, Shot::Drop | Shot::Bomb) { b.vy += 300.0 * dt; }
            let steps = ((b.vx.abs().max(b.vy.abs()) * dt).ceil() as i32).max(1);
            let (sx, sy) = (b.vx * dt / steps as f32, b.vy * dt / steps as f32);
            let mut hit = b.life <= 0.0;
            for _ in 0..steps {
                if hit { break; }
                let (nx, ny) = (b.x + sx, b.y + sy);
                let (cx, cy) = (nx.round() as i32, ny.round() as i32);
                let k = self.world.get(cx, cy);
                // The bore is as wide as you are, so it never slips past
                // the ledge you stand on.
                let wide = b.kind == Shot::Bore && if sx.abs() < sy.abs() { (-2..=2).any(|n| solid(self.world.get(cx + n, cy))) } else { (-4..=4).any(|n| solid(self.world.get(cx, cy + n))) };
                let stops = wide || solid(k) || k == GOLD || (b.kind == Shot::Flame && liquid(k)) || (b.kind == Shot::Drop && k == LAVA);
                if stops { hit = true; break; }
                (b.x, b.y) = (nx, ny);
                if b.kind == Shot::Spit {
                    if nx > px - 1.0 && nx < px + 6.0 && ny > py - 1.0 && ny < py + 10.0 { hit = true; self.hurt(9.0, true); }
                } else {
                    for f in &mut self.foes {
                        let (w, h) = f.size();
                        if nx > f.x - 1.0 && nx < f.x + w as f32 + 1.0 && ny > f.y - 1.0 && ny < f.y + h as f32 + 1.0 {
                            match b.kind {
                                Shot::Bore => { f.hp -= 2.0; f.hurt = 0.15; hit = true; }
                                Shot::Flame => if f.kind != FoeKind::Imp { f.hp -= 0.12; f.hurt = 0.1; },
                                Shot::Drop => if f.kind == FoeKind::Imp { f.hp -= 0.25; f.hurt = 0.1; },
                                _ => hit = true,
                            }
                        }
                    }
                }
            }
            let (x, y) = (b.x.round() as i32, b.y.round() as i32);
            match b.kind {
                Shot::Flame | Shot::Spit => if self.world.get(x, y) == AIR && (hit || self.world.one_in(3)) { self.world.set(x, y, FIRE); },
                _ => {}
            }
            if !hit { return true; }
            match b.kind {
                Shot::Bore => {
                    let (cx, cy) = ((b.x + sx * 2.0).round() as i32, (b.y + sy * 2.0).round() as i32);
                    self.world.carve(cx, cy, 5);
                    self.parts.burst(b.x, b.y, 6, 40.0, 0.3, 0xa0e0ff);
                    if b.life > 0.0 { self.audio.play(&self.snd.dig, 0.5); }
                }
                Shot::Drop => if self.world.get(x, y) == AIR { self.world.set(x, y, WATER) },
                Shot::Bomb => self.world.booms.push((x, y, 13)),
                _ => {}
            }
            false
        });
        self.bolts = bolts;
    }

    fn creatures(&mut self, dt: f32) {
        let (px, py) = (self.me.x + 2.5, self.me.y + 4.5);
        let mut foes = std::mem::take(&mut self.foes);
        let mut spits = Vec::new();
        for f in &mut foes {
            if (f.y - py).abs() > 200.0 { continue; }
            let (w, h) = f.size();
            let (dx, dy) = (px - (f.x + w as f32 / 2.0), py - (f.y + h as f32 / 2.0));
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            f.t -= dt;
            f.hurt -= dt;
            match f.kind {
                FoeKind::Slime => {
                    f.vy = (f.vy + 420.0 * dt).min(220.0);
                    let ground = self.world.hits(f.x.round() as i32, f.y.round() as i32 + 1, w, h);
                    if ground {
                        f.vx = 0.0;
                        if f.t <= 0.0 && dist < 130.0 {
                            (f.vx, f.vy, f.t) = (dx.signum() * self.rng.range(35.0, 60.0), -self.rng.range(90.0, 135.0), self.rng.range(0.6, 1.3));
                        }
                    }
                }
                FoeKind::Bat => {
                    let (tx, ty) = if dist < 120.0 { (dx / dist * 62.0, dy / dist * 62.0) } else { ((self.time * 2.0 + f.x).sin() * 20.0, (self.time * 3.0 + f.x).cos() * 12.0) };
                    f.vx += (tx - f.vx) * (dt * 3.0).min(1.0);
                    f.vy += (ty + (self.time * 9.0 + f.x).sin() * 26.0 - f.vy) * (dt * 3.0).min(1.0);
                }
                FoeKind::Imp => {
                    let (tx, ty) = if dist < 150.0 { ((dx - dx.signum() * 46.0).clamp(-40.0, 40.0), (dy + 22.0).clamp(-40.0, 40.0)) } else { (0.0, (self.time * 2.0 + f.x).sin() * 8.0) };
                    f.vx += (tx - f.vx) * (dt * 2.0).min(1.0);
                    f.vy += (ty - f.vy) * (dt * 2.0).min(1.0);
                    if f.t <= 0.0 && dist < 130.0 {
                        f.t = self.rng.range(1.4, 2.4);
                        spits.push(Bolt { x: f.x + 3.0, y: f.y + 3.0, vx: dx / dist * 95.0, vy: dy / dist * 95.0, kind: Shot::Spit, life: 1.6 });
                    }
                }
            }
            let (ox, oy) = (f.x.round() as i32, f.y.round() as i32);
            let (nx, ny) = (f.x + f.vx * dt, f.y + f.vy * dt);
            if !self.world.hits(nx.round() as i32, oy, w, h) { f.x = nx; }
            else if f.kind == FoeKind::Slime && !self.world.hits(nx.round() as i32, oy - 2, w, h) { f.x = nx; f.y -= 2.0; }
            else { f.vx = -f.vx * 0.5; }
            if !self.world.hits(f.x.round() as i32, ny.round() as i32, w, h) { f.y = ny; } else { f.vy = if f.kind == FoeKind::Slime { 0.0 } else { -f.vy * 0.5 }; }
            // Buried in sand, it climbs out; shut in rock, it is lost.
            if self.world.hits(ox, oy, w, h) && self.world.hits(f.x.round() as i32, f.y.round() as i32, w, h) {
                if self.world.hits(ox, oy - 1, w, h) { f.hp -= dt * 3.0; } else { f.y -= 1.0; }
            }
            let c = self.world.count(f.x.round() as i32, f.y.round() as i32, w, h);
            let heat = (c[FIRE as usize] + c[EMBER as usize] + c[LAVA as usize] * 3) as f32;
            match f.kind {
                FoeKind::Imp => f.hp -= c[WATER as usize] as f32 * dt * 1.2,
                FoeKind::Slime => f.hp -= heat * dt * 0.9,
                FoeKind::Bat => f.hp -= (heat + c[ACID as usize] as f32) * dt * 0.9,
            }
            if dx.abs() < w as f32 / 2.0 + 2.5 && dy.abs() < h as f32 / 2.0 + 4.5 {
                let bite = match f.kind { FoeKind::Slime => 9.0, FoeKind::Bat => 6.0, FoeKind::Imp => 8.0 };
                if self.me.safe <= 0.0 && self.mode == Mode::Play {
                    self.me.vx = -dx.signum() * 90.0;
                    self.me.vy = -70.0;
                }
                self.hurt(bite, true);
            }
        }
        let mut dead = Vec::new();
        foes.retain(|f| { if f.hp <= 0.0 { dead.push((f.kind, f.x as i32 + 3, f.y as i32 + 2)); } f.hp > 0.0 });
        self.foes = foes;
        self.bolts.extend(spits);
        for (kind, x, y) in dead {
            match kind {
                FoeKind::Slime => {
                    self.score += 50;
                    for _ in 0..7 { let (ax, ay) = (x + self.rng.range(-3.0, 3.0) as i32, y + self.rng.range(-3.0, 2.0) as i32); if self.world.get(ax, ay) == AIR { self.world.set(ax, ay, ACID); } }
                    self.parts.burst(x as f32, y as f32, 14, 50.0, 0.5, 0x70e050);
                }
                FoeKind::Bat => { self.score += 30; self.parts.burst(x as f32, y as f32, 10, 50.0, 0.5, 0xb080d0); }
                FoeKind::Imp => { self.score += 100; self.world.booms.push((x, y, 6)); }
            }
            self.sound(|s| &s.kill, 0.8);
        }
    }

    fn play(&mut self, input: &Input, dt: f32) {
        self.me.safe -= dt;
        self.cool -= dt;
        self.me.mana = (self.me.mana + dt * 24.0).min(100.0);
        if input.pressed(Key::Char('x')) || input.pressed(Key::Tab) { self.wand = (self.wand + 1) % 4; }
        for n in 0..4 { if input.pressed(Key::Char((b'1' + n) as char)) { self.wand = n as usize; } }
        let fire = input.held(Key::Char('z')) || input.held(Key::Space);
        if fire && self.cool <= 0.0 { self.shoot(input); }
        for (n, wand) in [1usize, 2].into_iter().enumerate() {
            let on = fire && self.wand == wand && self.me.mana >= 1.0;
            if on && !self.loops[n] { self.audio.play_loop(2 + n as u8, if n == 0 { &self.snd.flame } else { &self.snd.spout }, 1.0); }
            if !on && self.loops[n] { self.audio.stop(2 + n as u8); }
            self.loops[n] = on;
        }
        self.body(input, dt);
        if self.mode != Mode::Play { return; }
        self.fly(dt);
        self.creatures(dt);
    }

    /// One tick of the cave round the screen, and the blasts it set off.
    fn grains(&mut self) {
        let top = self.cam as i32;
        self.world.step(top - 70, top + H + 100);
        let booms: Vec<_> = self.world.booms.drain(..).collect();
        for (x, y, r) in booms.into_iter().take(8) { self.boom(x, y, r); }
    }

    fn step(&mut self, input: &Input, dt: f32) {
        self.time += dt;
        self.timer += if self.mode == Mode::Title { dt } else { -dt };
        self.shake = (self.shake - dt * 14.0).max(0.0);
        self.parts.update(dt);
        match self.mode {
            Mode::Title => {
                self.title_rain();
                self.world.step(0, H);
                self.world.booms.clear();
            }
            Mode::Down => {
                self.grains();
                if self.timer <= 0.0 { self.mode = Mode::Play; }
            }
            Mode::Play => {
                self.play(input, dt);
                self.grains();
            }
            Mode::Dead => {
                self.grains();
                if self.timer <= 0.0 { self.mode = Mode::Title; self.title_world(); }
            }
        }
        if self.mode != Mode::Title {
            let want = (self.me.y - H as f32 / 2.0 + 10.0 + self.me.vy * 0.15).clamp(0.0, (WH - H) as f32);
            self.cam += (want - self.cam) * (dt * 6.0).min(1.0);
        }
    }

    /// A grain's colour, before the light falls on it.
    fn color(&self, k: u8, a: u8, x: i32, y: i32, b: &Biome) -> Rgb {
        let shade = 0.78 + a as f32 / 255.0 * 0.34;
        let wave = ((x + y * 3) as f32 * 0.35 + self.time * 5.0).sin() * 0.5 + 0.5;
        // A lit top edge and a dark underside give the rock its shape.
        let edge = |w: &World| if w.get(x, y - 1) == AIR { 1.45 } else if w.get(x, y + 1) == AIR { 0.62 } else { 1.0 };
        match k {
            ROCK => tint(b.rock, shade * edge(&self.world)),
            HARD => if self.mode == Mode::Title && y < 115 && y > 40 && x > 10 && x < W - 10 { mix(0xffe070, 0xff6028, (y - 46) as f32 / 64.0) } else { tint(0x2c2a34, shade) },
            DIRT => tint(b.dirt, shade * edge(&self.world)),
            WOOD => tint(0x8a5a30, 0.8 + ((y % 3) as f32) * 0.1 + a as f32 / 1200.0),
            EMBER => mix(0xff5010, 0xffc040, wave),
            SAND => tint(0xd8b870, shade * edge(&self.world).max(0.8)),
            GOLD => if (a as f32 + self.time * 90.0) as i32 % 97 < 5 { 0xfffff0 } else { tint(0xffc820, shade) },
            POWDER => tint(0x484850, shade),
            WATER => mix(0x2860c8, 0x4088e8, wave * 0.5),
            OIL => mix(0x2a2018, 0x4a3828, wave * 0.4),
            ACID => mix(0x50d020, 0x90ff40, wave),
            LAVA => mix(0xff4808, 0xffb020, (wave + a as f32 / 255.0) * 0.5),
            FIRE => if a > 26 { 0xfff0a0 } else if a > 12 { 0xffa020 } else { 0xe04010 },
            SMOKE => tint(0x50505a, 0.5 + a as f32 / 200.0),
            _ => tint(0xa8b8c8, 0.5 + a as f32 / 360.0),
        }
    }

    /// The cave as it is lit: by flames, lava and acid, by the gate and
    /// by your lamp. The title is lit all over.
    fn paint(&mut self, f: &mut Frame, cam_x: i32, cam_y: i32) {
        let b = biome(self.depth);
        let title = self.mode == Mode::Title;
        for l in self.light.iter_mut() { *l = [0.0; 3]; }
        for sy in 0..H {
            let wy = sy + cam_y;
            for sx in 0..W {
                let wx = sx + cam_x;
                let (k, a, back) = if wx < 0 || wx >= WW || wy < 0 || wy >= WH { (HARD, 128, 0) } else {
                    let i = (wy * WW + wx) as usize;
                    (self.world.kind[i], self.world.aux[i], self.world.back[i])
                };
                let glow: [f32; 3] = match k {
                    FIRE => [0.10, 0.055, 0.015],
                    LAVA => [0.05, 0.02, 0.004],
                    EMBER => [0.06, 0.025, 0.006],
                    ACID => [0.004, 0.016, 0.004],
                    _ => [0.0; 3],
                };
                if glow[0] > 0.0 {
                    let l = &mut self.light[((sy >> 3) as usize + 1) * LW + (sx >> 3) as usize];
                    for c in 0..3 { l[c] += glow[c]; }
                }
                f.px[(sy * W + sx) as usize] = if k == AIR { tint(if title { 0x20263a } else { b.rock }, back as f32 / 255.0) } else { self.color(k, a, wx, wy, b) } | (k as u32) << 24;
            }
        }
        // Spread the light: two passes over the small map.
        for _ in 0..2 {
            let old = self.light.clone();
            for y in 1..LH - 1 {
                for x in 1..LW - 1 {
                    let i = y * LW + x;
                    for c in 0..3 {
                        self.light[i][c] = (old[i][c] * 4.0 + (old[i - 1][c] + old[i + 1][c] + old[i - LW][c] + old[i + LW][c]) * 2.0
                            + old[i - LW - 1][c] + old[i - LW + 1][c] + old[i + LW - 1][c] + old[i + LW + 1][c]) / 13.0;
                    }
                }
            }
        }
        let lamp = (self.me.x + 2.5 - cam_x as f32, self.me.y + 3.0 - cam_y as f32);
        let gate = ((self.gate.0 - cam_x) as f32, (self.gate.1 - cam_y) as f32);
        let alive = self.mode != Mode::Dead && !title;
        for y in 0..LH {
            for x in 0..LW {
                let (px, py) = (x as f32 * 8.0 + 4.0, y as f32 * 8.0 - 4.0);
                let l = &mut self.light[y * LW + x];
                if alive {
                    let d = ((px - lamp.0).powi(2) + (py - lamp.1).powi(2)).sqrt();
                    let v = (1.0 - d / 110.0).max(0.0).powi(2) * 1.1;
                    l[0] += v; l[1] += v * 0.92; l[2] += v * 0.78;
                }
                if title { continue; }
                let d = ((px - gate.0).powi(2) + (py - gate.1).powi(2)).sqrt();
                let v = (1.0 - d / 50.0).max(0.0).powi(2) * (0.9 + 0.2 * (self.time * 5.0).sin());
                l[0] += v * 0.7; l[1] += v * 0.3; l[2] += v;
            }
        }
        for sy in 0..H {
            let (ly, ty) = (((sy + 4) >> 3) as usize, ((sy + 4) & 7) as f32 / 8.0);
            for sx in 0..W {
                let (lx, tx) = if sx < 4 { (0, 0.0) } else { (((sx - 4) >> 3) as usize, ((sx - 4) & 7) as f32 / 8.0) };
                let i = ly * LW + lx.min(LW - 2);
                let (a, b2, c, d) = (self.light[i], self.light[i + 1], self.light[i + LW], self.light[i + LW + 1]);
                let p = &mut f.px[(sy * W + sx) as usize];
                let k = (*p >> 24) as u8;
                let (r, g, bl) = parts(*p & 0xffffff);
                let own = matches!(k, FIRE | LAVA | EMBER);
                let dark = if title { 0.9 } else if k == GOLD { 0.7 } else if k == AIR { 0.5 } else { 0.3 };
                let mut out = [0u8; 3];
                for (n, v) in [r, g, bl].into_iter().enumerate() {
                    let l = (a[n] * (1.0 - tx) + b2[n] * tx) * (1.0 - ty) + (c[n] * (1.0 - tx) + d[n] * tx) * ty;
                    let lit = if own { 1.0 } else { dark + l.min(1.5) };
                    out[n] = (v as f32 * lit + l.min(1.2) * if k == AIR { 70.0 } else { 30.0 }).min(255.0) as u8;
                }
                *p = rgb(out[0], out[1], out[2]);
            }
        }
    }

    fn bar(f: &mut Frame, x: i32, y: i32, w: i32, part: f32, c: Rgb) {
        f.rect(x - 1, y - 1, w + 2, 5, 0x101018);
        f.rect(x, y, w, 3, tint(c, 0.25));
        f.rect(x, y, (w as f32 * part.clamp(0.0, 1.0)) as i32, 3, c);
    }

    fn title(&self, f: &mut Frame) {
        let mut line = |y: i32, text: &str, c: Rgb, big: bool| {
            f.text_centered(W / 2 + 1, y + 1, text, BLACK, big, 1);
            f.text_centered(W / 2, y, text, c, big, 1);
        };
        line(118, "EVERY PIXEL IS A GRAIN OF SOMETHING", 0xf0e8d0, false);
        if (self.time * 2.0) as i32 % 2 == 0 { line(130, "PRESS SPACE", 0x60e0ff, true); }
        line(146, "ARROWS WALK, LIFT AND AIM   Z FIRES   X OR 1-4 PICKS THE WAND", 0xd0d0e0, false);
        line(155, "BORE DIGS  FLAME BURNS  SPOUT POURS  BOMB BLASTS   P PAUSES  Q QUITS", 0xd0d0e0, false);
        if self.high > 0 { line(166, &format!("BEST {}", self.high), 0xffd060, false); }
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), 4, VERSION, 0x8080a0);
    }
}

impl Game for Dust {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        let quit = input.pressed(Key::Char('q')) || input.pressed(Key::Escape);
        if quit && self.mode == Mode::Title { return Flow::Quit; }
        if quit {
            self.quiet();
            self.audio.stop(1);
            self.mode = Mode::Title;
            self.title_world();
            return Flow::Continue;
        }
        if input.pressed(Key::Char('p')) && self.mode == Mode::Play { self.quiet(); return Flow::Pause; }
        if input.pressed(Key::Space) || input.pressed(Key::Enter) {
            match self.mode { Mode::Title => self.begin(), Mode::Dead if self.timer < 4.0 => { self.mode = Mode::Title; self.title_world(); } _ => {} }
        }
        self.step(input, dt.min(0.05));
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        let shake = self.shake.min(4.0);
        let cam_x = (self.rng.range(-shake, shake)) as i32;
        let cam_y = self.cam as i32 + (self.rng.range(-shake, shake)) as i32;
        self.paint(f, cam_x, cam_y);
        if self.mode == Mode::Title { self.title(f); return; }

        // The gate: a ring of sparks that turns.
        let (gx, gy) = (self.gate.0 - cam_x, self.gate.1 - cam_y);
        for n in 0..14 {
            let a = self.time * 2.5 + n as f32 * 0.449;
            let r = 6.0 + (self.time * 4.0 + n as f32).sin() * 1.5;
            f.put(gx + (a.cos() * r * 0.7) as i32, gy + (a.sin() * r) as i32, if n % 2 == 0 { 0xe0a0ff } else { 0x8040ff });
        }
        for &(what, x, y) in &self.finds {
            let (x, y) = (x - cam_x, y - cam_y + ((self.time * 3.0 + x as f32).sin() * 1.5) as i32);
            match what {
                Find::Heart => { f.rect(x + 1, y + 1, 2, 2, 0xff4060); f.rect(x + 4, y + 1, 2, 2, 0xff4060); f.rect(x + 1, y + 2, 5, 2, 0xff4060); f.rect(x + 2, y + 4, 3, 1, 0xff4060); f.put(x + 3, y + 5, 0xff4060); }
                Find::Bombs => { f.circle(x + 3, y + 4, 2, 0x202028); f.put(x + 4, y + 1, 0xffc040); f.put(x + 2, y + 3, 0x8080a0); }
            }
        }
        for foe in &self.foes {
            let (x, y) = (foe.x.round() as i32 - cam_x, foe.y.round() as i32 - cam_y);
            let white = foe.hurt > 0.0;
            match foe.kind {
                FoeKind::Slime => {
                    let c = if white { WHITE } else { 0x58d040 };
                    f.rect(x + 1, y, 5, 1, c); f.rect(x, y + 1, 7, 4, c);
                    f.put(x + 2, y + 2, BLACK); f.put(x + 4, y + 2, BLACK);
                }
                FoeKind::Bat => {
                    let c = if white { WHITE } else { 0x9060c0 };
                    let flap = ((self.time * 14.0 + foe.x) as i32 % 2) * 2;
                    f.rect(x + 2, y + 1, 3, 3, c); f.rect(x, y + flap, 2, 2, c); f.rect(x + 5, y + flap, 2, 2, c);
                    f.put(x + 2, y + 2, 0xff4040); f.put(x + 4, y + 2, 0xff4040);
                }
                FoeKind::Imp => {
                    let c = if white { WHITE } else { 0xff6020 };
                    f.rect(x + 1, y + 1, 4, 6, c); f.put(x, y, c); f.put(x + 5, y, c); f.rect(x + 2, y + 7, 2, 1, 0xffc040);
                    f.put(x + 2, y + 3, 0xffff80); f.put(x + 4, y + 3, 0xffff80);
                }
            }
        }
        for b in &self.bolts {
            let (x, y) = (b.x.round() as i32 - cam_x, b.y.round() as i32 - cam_y);
            match b.kind {
                Shot::Bore => { f.rect(x - 1, y - 1, 2, 2, 0xc0f0ff); }
                Shot::Flame => f.put(x, y, 0xffd060),
                Shot::Drop => f.put(x, y, 0x60a0ff),
                Shot::Bomb => { f.circle(x, y, 2, 0x18181c); f.put(x + 1, y - 2, if (self.time * 20.0) as i32 % 2 == 0 { 0xffe060 } else { 0xff6020 }); }
                Shot::Spit => { f.rect(x - 1, y - 1, 2, 2, 0xffe080); }
            }
        }
        if self.mode != Mode::Dead && (self.me.safe <= 0.0 || (self.time * 20.0) as i32 % 2 == 0) {
            let (x, y) = (self.me.x.round() as i32 - cam_x, self.me.y.round() as i32 - cam_y);
            let leg = if self.me.ground { (self.me.walk as i32) % 2 } else { 0 };
            f.rect(x + 1, y, 3, 2, 0xe0b030);
            f.put(x + 2 + self.me.face, y + 1, 0xffffd0);
            f.rect(x + 1, y + 2, 3, 2, 0xf0c8a0);
            f.rect(x, y + 4, 5, 3, 0x3880e0);
            f.rect(x + 1 - leg, y + 7, 1, 2, 0x504840);
            f.rect(x + 3 + leg, y + 7, 1, 2, 0x504840);
            f.put(x + 2 + self.me.face * 3, y + 5, 0xc0f0ff);
        }
        self.parts.draw(f, cam_x, cam_y);

        Dust::bar(f, 5, 5, 60, self.me.hp / 100.0, 0xe03040);
        Dust::bar(f, 5, 11, 60, self.me.mana / 100.0, 0x40a0ff);
        Dust::bar(f, 5, 17, 60, self.me.oil, 0xffb040);
        let mut x = 72;
        for (n, name) in WANDS.iter().enumerate() {
            let label = if n == 3 { format!("{} {}", name, self.bombs) } else { name.to_string() };
            let w = Frame::text_width(&label, false, 1);
            if n == self.wand { f.rect(x - 2, 3, w + 3, 9, 0x283050); }
            f.text(x, 5, &label, if n == self.wand { WHITE } else { 0x8088a0 });
            x += w + 7;
        }
        let score = format!("{:06}", self.score);
        f.text_big(W - 5 - Frame::text_width(&score, true, 1), 4, &score, WHITE);
        let deep = format!("DEPTH {}", self.depth);
        f.text(W - 5 - Frame::text_width(&deep, false, 1), 14, &deep, 0xa0a0b8);

        if self.mode == Mode::Down {
            f.fancy_text(W / 2, 60, biome(self.depth).name, 2, self.time, (0xe0a0ff, 0x8040ff));
            f.text_centered(W / 2, 84, &format!("DEPTH {}", self.depth), 0xc0c0d8, true, 1);
        }
        if self.mode == Mode::Dead {
            f.fancy_text(W / 2, 62, "LOST IN THE DUST", 2, self.time, (0xff6040, 0xffc040));
            f.text_centered(W / 2, 88, &format!("DEPTH {}   GOLD {}   SCORE {}", self.depth, self.gold, self.score), WHITE, true, 1);
        }
    }
}

/// The game as it is played: with sound, from the title or straight
/// into the depth `DUST_START` names.
fn dust() -> Dust {
    let mut game = Dust::new();
    game.audio = Audio::open();
    if let Some(depth) = std::env::var("DUST_START").ok().and_then(|s| s.trim().parse::<u32>().ok()) {
        game.start = depth.max(1);
        game.begin();
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = Config { width: W, height: H, fps: 60 };
    // DUST_BENCH=<frames> plays that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("DUST_BENCH") {
        let mut game = Dust::new();
        game.start = std::env::var("DUST_START").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        game.begin();
        let mut input = Input::new();
        input.key(Key::Char('z'), true);
        input.key(Key::Down, true);
        let last = bench(&mut game, &input, cfg, n.parse().unwrap_or(600));
        if let Ok(path) = std::env::var("DUST_SHOT") { let _ = std::fs::write(path, last.to_ppm()); }
        return;
    }
    run(&mut dust(), cfg);
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(dust(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
mod tests {
    use super::*;

    /// A box of stone, open inside, for a rule to be tried in.
    fn jar() -> World {
        let mut w = World::new(7);
        for y in 0..WH { for x in 0..WW { if !(10..60).contains(&x) || !(10..60).contains(&y) { w.kind[(y * WW + x) as usize] = HARD; } } }
        w
    }

    fn run(w: &mut World, ticks: u32) { for _ in 0..ticks { w.step(0, 80); } }

    #[test]
    fn sand_falls_and_lies_in_a_heap() {
        let mut w = jar();
        for y in 12..30 { w.set(35, y, SAND); }
        run(&mut w, 200);
        assert_eq!(w.count(10, 10, 50, 40)[SAND as usize], 0, "sand is still in the air");
        assert_eq!(w.count(10, 50, 50, 10)[SAND as usize], 18);
        assert!(w.get(35, 59) == SAND && w.get(35, 54) == AIR, "a column stood where a heap should lie");
    }

    #[test]
    fn water_finds_its_level() {
        let mut w = jar();
        for y in 20..50 { for x in 12..16 { w.set(x, y, WATER); } }
        run(&mut w, 1500);
        assert_eq!(w.count(10, 10, 50, 50)[WATER as usize], 120);
        assert_eq!(w.count(10, 10, 50, 46)[WATER as usize], 0, "water stands above its level");
    }

    #[test]
    fn oil_floats_on_water() {
        let mut w = jar();
        for x in 10..60 { for y in 50..55 { w.set(x, y, OIL); } for y in 55..60 { w.set(x, y, WATER); } }
        for x in 10..60 { for y in 40..45 { w.set(x, y, WATER); } }
        run(&mut w, 2500);
        assert_eq!(w.count(10, 50, 50, 10)[OIL as usize], 0, "oil lies under the water");
        assert_eq!(w.count(10, 10, 50, 50)[OIL as usize], 250);
    }

    #[test]
    fn lava_and_water_make_stone() {
        let mut w = jar();
        for x in 10..60 { w.set(x, 59, LAVA); w.set(x, 58, WATER); }
        run(&mut w, 300);
        let c = w.count(10, 10, 50, 50);
        assert_eq!(c[LAVA as usize], 0);
        assert_eq!(c[ROCK as usize], 50);
    }

    #[test]
    fn a_flame_burns_the_oil_away_and_water_stops_it() {
        let mut w = jar();
        for x in 10..60 { for y in 56..60 { w.set(x, y, OIL); } }
        for x in 20..40 { w.set(x, 55, FIRE); }
        run(&mut w, 1500);
        // A drop or two may run clear of the last flame.
        assert!(w.count(10, 10, 50, 50)[OIL as usize] < 10, "oil is left");
        let mut w = jar();
        for x in 10..60 { w.set(x, 59, WOOD); }
        w.set(12, 59, EMBER);
        for x in 10..60 { for y in 50..58 { w.set(x, y, WATER); } }
        run(&mut w, 2000);
        assert!(w.count(10, 10, 50, 50)[WOOD as usize] >= 48, "wood burned under water");
    }

    #[test]
    fn powder_blows_a_hole_but_gold_is_kept() {
        let mut w = jar();
        for x in 10..60 { for y in 50..60 { w.set(x, y, ROCK); } }
        for x in 30..36 { for y in 52..58 { w.set(x, y, POWDER); } }
        w.set(20, 55, GOLD);
        w.blast(33, 55, 13);
        assert!(w.count(10, 10, 50, 50)[ROCK as usize] < 420);
        assert_eq!(w.count(10, 10, 50, 50)[GOLD as usize], 1);
        assert!(!w.booms.is_empty(), "the powder did not go off");
    }

    #[test]
    fn every_cave_has_a_room_to_start_in_and_a_gate() {
        for depth in 1..=10 {
            let mut rng = Rng::new(depth as u64 * 31);
            let (w, start, gate) = World::cave(depth, &mut rng);
            assert!(!w.hits(start.0 - 2, start.1, ME_W, ME_H), "depth {depth}: the start is in rock");
            assert!(w.hits(start.0 - 2, start.1 + 9, ME_W, 1), "depth {depth}: no floor at the start");
            assert!(!w.hits(gate.0 - 2, gate.1 - 4, ME_W, ME_H), "depth {depth}: the gate is in rock");
            let c = w.count(0, 0, WW, WH);
            assert!(c[AIR as usize] > 30_000 && c[GOLD as usize] > 100, "depth {depth}: {c:?}");
            for x in 0..WW { assert_eq!(w.get(x, WH - 2), HARD); }
        }
    }

    /// A player that only digs: down, and toward the gate.
    fn dig(game: &mut Dust, frames: u32) {
        let from = game.depth;
        for n in 0..frames {
            let mut input = Input::new();
            let dx = game.gate.0 - game.me.x as i32 - 2;
            let low = game.me.y as i32 + 9 >= game.gate.1 + 8;
            if dx.abs() > 3 { input.key(if dx > 0 { Key::Right } else { Key::Left }, true); }
            // On a ledge the bore misses the floor: a step to the side now and then.
            else if n % 90 < 6 { input.key(Key::Right, true); } else if n % 90 >= 45 && n % 90 < 51 { input.key(Key::Left, true); }
            // Half the time it bores level, to get past an overhang.
            if !low && (dx.abs() <= 3 || n % 60 < 30) { input.key(Key::Down, true); }
            input.key(Key::Char('z'), true);
            (game.me.hp, game.foes) = (100.0, Vec::new());
            game.update(&input, 1.0 / 60.0);
            if game.depth > from { println!("depth {from} dug through in {n} frames"); return; }
        }
        let (x, y) = (game.me.x as i32, game.me.y as i32);
        for yy in y - 3..y + 16 { println!("{}", (x - 12..x + 17).map(|xx| b" #H%w*.$p~o!L^'\""[game.world.get(xx, yy) as usize] as char).collect::<String>()); }
        panic!("the digger never reached the gate of depth {from}: at {},{} gate {:?} mana {} vy {} ground {}", game.me.x, game.me.y, game.gate, game.me.mana, game.me.vy, game.me.ground);
    }

    #[test]
    fn a_digger_reaches_every_gate() {
        let mut game = Dust::new();
        game.rng = Rng::new(5);
        game.begin();
        for _ in 0..5 { dig(&mut game, 60 * 240); }
        assert_eq!(game.depth, 6);
        assert!(game.score >= 500 * 15);
    }

    #[test]
    fn any_keys_can_be_pressed_and_the_game_goes_on() {
        let keys = [Key::Left, Key::Right, Key::Up, Key::Down, Key::Char('z'), Key::Char('x'), Key::Char('1'), Key::Char('2'), Key::Char('3'), Key::Char('4'), Key::Space];
        let mut frame = Frame::new(W, H);
        for seed in 1..=5u64 {
            let mut game = Dust::new();
            game.rng = Rng::new(seed);
            let mut rng = Rng::new(seed * 77);
            game.start = seed as u32;
            game.begin();
            let mut held = Vec::new();
            for n in 0..1500 {
                if n % 20 == 0 { held = keys.iter().filter(|_| rng.chance(0.3)).copied().collect(); }
                let mut input = Input::new();
                for &k in &held { input.key(k, true); }
                game.update(&input, 1.0 / 60.0);
                if n % 10 == 0 { game.draw(&mut frame); }
            }
        }
    }

    /// `DUST_FILM=<folder>` writes pictures of a digger at work, and of
    /// the title, as PPM files.
    #[test]
    #[ignore]
    fn the_bot_plays() {
        let Ok(dir) = std::env::var("DUST_FILM") else { return };
        let mut frame = Frame::new(W, H);
        let mut game = Dust::new();
        game.rng = Rng::new(std::env::var("DUST_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(5));
        game.title_world();
        for n in 0..60 * 50 {
            game.update(&Input::new(), 1.0 / 60.0);
            if n % 300 == 299 { game.draw(&mut frame); std::fs::write(format!("{dir}/title{:02}.ppm", n / 300), frame.to_ppm()).unwrap(); }
            if (1640..2240).contains(&n) && n % 100 == 0 { game.draw(&mut frame); std::fs::write(format!("{dir}/burn{:02}.ppm", (n - 1640) / 100), frame.to_ppm()).unwrap(); }
        }
        game.start = std::env::var("DUST_START").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        game.begin();
        for n in 0..60 * 60 {
            let mut input = Input::new();
            let wand = (n / 240) % 3;
            input.key(Key::Char((b'1' + wand as u8) as char), true);
            input.key(Key::Char('z'), true);
            if wand == 0 { input.key(Key::Down, true); }
            if n % 400 < 200 { input.key(Key::Right, true); } else { input.key(Key::Left, true); }
            game.me.hp = 100.0;
            game.update(&input, 1.0 / 60.0);
            if n % 120 == 119 { game.draw(&mut frame); std::fs::write(format!("{dir}/play{:02}.ppm", n / 120), frame.to_ppm()).unwrap(); }
        }
    }
}
