//! raid: a gunship over fractal mountains, in the spirit of Comanche
//! (NovaLogic, 1992). The land is drawn the way soar draws it: a height
//! map ray-cast a column at a time, on every core. On it stand tanks,
//! guns, missile sites and boats, as small 3D models tested for depth
//! against the hills. Six missions from dawn to night; after the sixth it
//! all begins again, harder.
//!
//!     cargo run --release --example raid
//!
//! Left and Right turn, Up and Down set the speed, W and S set the height
//! over the ground, A and D slide sideways. Space fires the gun, F a
//! rocket, E a guided missile at the boxed target. P pauses, Q quits.
//! Hills are cover: what cannot see the gunship cannot shoot it. Hover
//! low over the pad to rearm and repair.
//!
//! `RAID_START=<mission>[,<metres>]` starts at another mission, that far
//! from its targets; `RAID_BENCH=<frames>` times the game with no
//! terminal. Everything here is new: the land, the models and the sounds.

use funkey::*;
use std::f32::consts::{PI, TAU};

const W: i32 = 640;
const H: i32 = 400;
/// The instrument panel under the view.
const PANEL: i32 = 46;
/// The height of the view out of the cockpit.
const VH: i32 = H - PANEL;
const MAP: usize = 1024;
const MASK: usize = MAP - 1;
/// The land is this wide, then it repeats.
const WORLD: f32 = MAP as f32;
const WATER: f32 = 0.34;
const HSCALE: f32 = 260.0;
/// The full map and four coarser copies of it.
const LEVELS: usize = 5;
const FOCAL: f32 = W as f32 * 0.55;
/// Nothing nearer the eye than this is drawn.
const NEAR: f32 = 1.0;
/// Where the sun stands, as a heading, and the way its light comes:
/// east, up, north.
const SUN_AZ: f32 = 0.66;
const SUN: V3 = V3::new(0.557, 0.416, 0.718);
/// Steps of the sky from the horizon up.
const GRAD: usize = 512;
const GAME: &str = "raid";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";
const ROCKETS: u32 = 38;
const MISSILES: u32 = 8;
const ROUNDS: u32 = 600;
/// The gunship's top speed, in metres a second: a step of the map is a metre.
const TOP: f32 = 75.0;
/// How far off a target can be boxed.
const REACH: f32 = 520.0;
/// How fast the gunship can climb, in metres a second.
const CLIMB: f32 = 60.0;
/// The horizon's place down the view when the gunship hangs still.
const LEVEL: f32 = 0.44;
/// The row of the gunsight.
const SIGHT: i32 = VH / 2;
const HUD: Rgb = 0x50ff80;
const AMBER: Rgb = 0xffc040;
const RED: Rgb = 0xff4838;
const TEXT: Rgb = 0xf0f0f0;

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ seed.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13; h = h.wrapping_mul(0x5bd1e995); h ^= h >> 15;
    (h & 0xffffff) as f32 / 16777216.0
}

fn smooth(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

/// Value noise on a lattice that repeats every `period` cells.
fn noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (smooth(x - xi as f32), smooth(y - yi as f32));
    let g = |ix: i32, iy: i32| hash(ix.rem_euclid(period), iy.rem_euclid(period), seed);
    let a = g(xi, yi) + (g(xi + 1, yi) - g(xi, yi)) * fx;
    let b = g(xi, yi + 1) + (g(xi + 1, yi + 1) - g(xi, yi + 1)) * fx;
    a + (b - a) * fy
}

/// Layers of noise, each twice as fine and half as tall. The coarsest
/// repeats every `period` cells, so the land meets itself at its edges.
fn fbm(x: f32, y: f32, octaves: u32, seed: u32, period: i32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += noise(x * freq, y * freq, period << o, seed + o) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Ridges: the absolute value of a noise folded over, for mountains.
fn ridged(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        let n = 1.0 - (noise(x * freq, y * freq, 6 << o, seed + o) * 2.0 - 1.0).abs();
        sum += n * n * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb { funkey::raster::blend(a, b, t.clamp(0.0, 1.0)) }

fn scale(c: Rgb, k: f32) -> Rgb { tinted(c, [k, k, k]) }

fn tinted(c: Rgb, k: [f32; 3]) -> Rgb {
    let (r, g, b) = parts(c);
    rgb((r as f32 * k[0]).min(255.0) as u8, (g as f32 * k[1]).min(255.0) as u8, (b as f32 * k[2]).min(255.0) as u8)
}

/// The short way from one place on the map to another: the land repeats.
fn wrap(d: f32) -> f32 { (d + WORLD / 2.0).rem_euclid(WORLD) - WORLD / 2.0 }

/// An angle brought into -PI..PI.
fn turn(a: f32) -> f32 { (a + PI).rem_euclid(TAU) - PI }

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (dx, dy, dh) = (wrap(b[0] - a[0]), wrap(b[1] - a[1]), b[2] - a[2]);
    (dx * dx + dy * dy + dh * dh).sqrt()
}

/// The way from one point to another, one step long.
fn toward(from: [f32; 3], to: [f32; 3]) -> [f32; 3] {
    let (dx, dy, dh) = (wrap(to[0] - from[0]), wrap(to[1] - from[1]), to[2] - from[2]);
    let l = (dx * dx + dy * dy + dh * dh).sqrt().max(0.001);
    [dx / l, dy / l, dh / l]
}

fn cores() -> usize { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) }

/// Fill a map row by row, the rows shared out over the cores.
fn rows<T: Send>(data: &mut [T], fill: impl Fn(usize, &mut [T]) + Sync) {
    let per = MAP.div_ceil(cores()) * MAP;
    std::thread::scope(|s| {
        for (i, part) in data.chunks_mut(per).enumerate() {
            let fill = &fill;
            s.spawn(move || for (j, row) in part.chunks_mut(MAP).enumerate() { fill(i * per / MAP + j, row); });
        }
    });
}

/// The map at one level of detail. Each level up is half as fine, every
/// cell the average of four below it, so far ground is read from a map
/// as coarse as the steps the rays take there.
struct Level { size: usize, height: Vec<f32>, color: Vec<Rgb> }

impl Level {
    fn halve(&self) -> Level {
        let (s, n) = (self.size, self.size / 2);
        let (mut height, mut color) = (Vec::with_capacity(n * n), Vec::with_capacity(n * n));
        for y in 0..n {
            for x in 0..n {
                let i = [2 * y * s + 2 * x, 2 * y * s + 2 * x + 1, (2 * y + 1) * s + 2 * x, (2 * y + 1) * s + 2 * x + 1];
                height.push(i.iter().map(|&j| self.height[j]).sum::<f32>() / 4.0);
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for &j in &i { let (cr, cg, cb) = parts(self.color[j]); r += cr as u32; g += cg as u32; b += cb as u32; }
                color.push(rgb((r / 4) as u8, (g / 4) as u8, (b / 4) as u8));
            }
        }
        Level { size: n, height, color }
    }

    /// The four cells around a point of the full map, and the weights
    /// between them.
    #[inline]
    fn cell(&self, x: f32, y: f32) -> ([usize; 4], f32, f32) {
        let k = self.size as f32 / MAP as f32;
        let (x, y) = ((x + 0.5) * k - 0.5, (y + 0.5) * k - 0.5);
        let (xi, yi) = (x.floor(), y.floor());
        let m = self.size - 1;
        let (x0, y0) = (xi as i64 as usize & m, yi as i64 as usize & m);
        let (x1, y1) = ((x0 + 1) & m, (y0 + 1) & m);
        let s = self.size;
        ([y0 * s + x0, y0 * s + x1, y1 * s + x0, y1 * s + x1], x - xi, y - yi)
    }

    #[inline]
    fn height_at(&self, x: f32, y: f32) -> f32 {
        let (i, fx, fy) = self.cell(x, y);
        let a = self.height[i[0]] + (self.height[i[1]] - self.height[i[0]]) * fx;
        let b = self.height[i[2]] + (self.height[i[3]] - self.height[i[2]]) * fx;
        a + (b - a) * fy
    }

    #[inline]
    fn color_at(&self, x: f32, y: f32) -> Rgb {
        let (i, fx, fy) = self.cell(x, y);
        lerp(lerp(self.color[i[0]], self.color[i[1]], fx), lerp(self.color[i[2]], self.color[i[3]], fx), fy)
    }
}

/// The land: its levels in the light of the hour, the same in plain
/// daylight to tint again from, and the clouds over it.
struct Terrain {
    levels: Vec<Level>,
    day: Vec<Vec<Rgb>>,
    clouds: Vec<f32>,
    /// Where each mission is fought, and the pad it is flown from. The
    /// ground is levelled at both.
    sites: Vec<(f32, f32)>,
    pads: Vec<(f32, f32)>,
}

const CLOUD: usize = 256;

/// Bilinear read of a tileable square texture.
fn tex(t: &[f32], size: usize, x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let m = size - 1;
    let (x0, y0) = (xi as i64 as usize & m, yi as i64 as usize & m);
    let (x1, y1) = ((x0 + 1) & m, (y0 + 1) & m);
    let a = t[y0 * size + x0] + (t[y0 * size + x1] - t[y0 * size + x0]) * fx;
    let b = t[y1 * size + x0] + (t[y1 * size + x1] - t[y1 * size + x0]) * fx;
    a + (b - a) * fy
}

impl Terrain {
    fn generate() -> Terrain {
        let n = MAP * MAP;
        let mut height = vec![0.0f32; n];
        let s = 8.0 / MAP as f32;
        rows(&mut height, |y, row| for (x, h) in row.iter_mut().enumerate() {
            let (fx, fy) = (x as f32 * s, y as f32 * s);
            let base = fbm(fx, fy, 7, 11, 8);
            let mountains = ridged(fx * 0.75, fy * 0.75, 6, 29);
            let mask = smooth((fbm(fx * 0.5, fy * 0.5, 3, 41, 4) - 0.35).clamp(0.0, 0.35) / 0.35);
            *h = base * 0.55 + mountains * mask * 0.75;
        });
        // Stretch so the water covers about a third.
        let mut sorted = height.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let (lo, hi) = (sorted[n / 100], sorted[n - n / 400]);
        for h in height.iter_mut() { *h = ((*h - lo) / (hi - lo)).clamp(0.0, 1.0); }
        let (sites, pads) = plan(&mut height);
        let mut color = vec![0u32; n];
        let hm = &height;
        rows(&mut color, |y, row| for (x, out) in row.iter_mut().enumerate() {
            let h = hm[y * MAP + x];
            let at = |dx: i32, dy: i32| hm[((y as i32 + dy) as usize & MASK) * MAP + ((x as i32 + dx) as usize & MASK)];
            let (dhx, dhy) = ((at(1, 0) - at(-1, 0)) * HSCALE / 2.0, (at(0, 1) - at(0, -1)) * HSCALE / 2.0);
            let normal = V3::new(-dhx, 1.0, -dhy).norm();
            let slope = 1.0 - normal.y;
            let (fx, fy) = (x as f32 / 16.0, y as f32 / 16.0);
            let grain = noise(fx * 4.0, fy * 4.0, 256, 77);
            let c = if h < WATER {
                lerp(0x3d9bb0, 0x0a2450, (WATER - h) * 9.0)
            } else if h < WATER + 0.012 {
                lerp(0xe0d29a, 0xc8b47a, grain)
            } else if h > 0.78 + grain * 0.06 - slope * 0.1 {
                lerp(0xf2f5fa, 0xc8d4e2, slope * 2.0)
            } else if slope > 0.42 + grain * 0.15 {
                lerp(0x6e655e, 0x9a8f84, grain)
            } else if h > 0.62 {
                lerp(0x8a8a5a, 0x6e6e4c, grain)
            } else if fbm(fx * 0.25, fy * 0.25, 3, 91, 16) > 0.52 && h > WATER + 0.03 {
                lerp(0x2c5a26, 0x3f7a30, grain)
            } else {
                lerp(0x5d9a3c, 0x8ab34e, grain)
            };
            // Sun, and the shadow of what stands between.
            let lit = 0.42 + 0.58 * normal.dot(SUN).max(0.0);
            let mut shade = 1.0;
            if h >= WATER {
                let (mut px, mut py, mut pz) = (x as f32, y as f32, h * HSCALE);
                for _ in 0..70 {
                    px += SUN.x * 1.5; py += SUN.z * 1.5; pz += SUN.y * 1.5;
                    let ground = hm[(py as i64 as usize & MASK) * MAP + (px as i64 as usize & MASK)] * HSCALE;
                    if ground > pz + 0.5 { shade = 0.62; break; }
                    if pz > HSCALE { break; }
                }
            }
            *out = scale(c, lit * shade);
        });
        let clouds = (0..CLOUD * CLOUD).map(|i| fbm((i % CLOUD) as f32 / CLOUD as f32 * 8.0, (i / CLOUD) as f32 / CLOUD as f32 * 8.0, 5, 5, 8)).collect();
        let mut levels = vec![Level { size: MAP, height, color }];
        while levels.len() < LEVELS { let up = levels[levels.len() - 1].halve(); levels.push(up); }
        let day = levels.iter().map(|l| l.color.clone()).collect();
        Terrain { levels, day, clouds, sites, pads }
    }

    /// Colour the land for an hour of the day.
    fn light(&mut self, tint: [f32; 3]) {
        for (level, day) in self.levels.iter_mut().zip(&self.day) {
            for (c, &d) in level.color.iter_mut().zip(day) { *c = tinted(d, tint); }
        }
    }

    /// The height at a point, `lod` levels up from the full map. Between
    /// two levels both are read and mixed, so no seam shows where one
    /// hands over to the next.
    fn height(&self, x: f32, y: f32, lod: f32) -> f32 {
        let l = (lod as usize).min(LEVELS - 1);
        let a = self.levels[l].height_at(x, y);
        let t = lod - l as f32;
        if t <= 0.0 || l + 1 == LEVELS { return a; }
        a + (self.levels[l + 1].height_at(x, y) - a) * t
    }

    fn color(&self, x: f32, y: f32, lod: f32) -> Rgb {
        let l = (lod as usize).min(LEVELS - 1);
        let a = self.levels[l].color_at(x, y);
        let t = lod - l as f32;
        if t <= 0.0 || l + 1 == LEVELS { return a; }
        lerp(a, self.levels[l + 1].color_at(x, y), t)
    }

    /// The height of the map at a point, 0 to 1, the lake beds too.
    fn raw(&self, x: f32, y: f32) -> f32 { self.levels[0].height_at(x, y) }

    /// How high the ground or the water lies at a point.
    fn ground(&self, x: f32, y: f32) -> f32 { self.raw(x, y).max(WATER) * HSCALE }

    fn land(&self, x: f32, y: f32) -> bool { self.raw(x, y) > WATER + 0.012 }

    /// How steep the ground is: the rise over one step along.
    fn slope(&self, x: f32, y: f32) -> f32 {
        let e = 2.0;
        let dx = (self.raw(x + e, y) - self.raw(x - e, y)) * HSCALE / (2.0 * e);
        let dy = (self.raw(x, y + e) - self.raw(x, y - e)) * HSCALE / (2.0 * e);
        dx.hypot(dy)
    }

    /// True when no ground stands between two points.
    fn sees(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        let (dx, dy) = (wrap(b[0] - a[0]), wrap(b[1] - a[1]));
        let n = (dx.hypot(dy) / 3.0).ceil().max(1.0) as i32;
        (1..n - 1).all(|i| {
            let t = i as f32 / n as f32;
            self.ground(a[0] + dx * t, a[1] + dy * t) < a[2] + (b[2] - a[2]) * t
        })
    }

    /// What lies in a circle: the share that is dry land, and how high
    /// it lies, 0 to 1.
    fn survey(&self, x: f32, y: f32, r: f32) -> (f32, f32) {
        let n = 60;
        let (mut land, mut high) = (0.0, 0.0);
        for i in 0..n {
            let (d, a) = (r * ((i as f32 + 0.5) / n as f32).sqrt(), i as f32 * 2.4);
            let (px, py) = (x + a.sin() * d, y + a.cos() * d);
            if self.land(px, py) { land += 1.0; }
            high += self.raw(px, py);
        }
        (land / n as f32, high / n as f32)
    }
}

/// An hour of the day: the sky overhead and at the horizon, what the
/// light does to every colour, the sun or the moon, how far one sees.
struct Sky { zenith: Rgb, haze: Rgb, tint: [f32; 3], sun: Rgb, cloud: Rgb, fog: f32, night: bool,
    /// How high the sun or the moon is drawn.
    rise: f32 }

const DAWN: usize = 0;
const DAY: usize = 1;
const MIST: usize = 2;
const DUSK: usize = 3;
const NIGHT: usize = 4;
static SKIES: [Sky; 5] = [
    Sky { zenith: 0x40508c, haze: 0xf2b68c, tint: [1.0, 0.84, 0.74], sun: 0xffd9a0, cloud: 0xf6c8a8, fog: 520.0, night: false, rise: 0.07 },
    Sky { zenith: 0x3568b8, haze: 0xe6d2b4, tint: [1.0, 1.0, 1.0], sun: 0xfff2c8, cloud: 0xf8f4ee, fog: 620.0, night: false, rise: 0.22 },
    Sky { zenith: 0x6f8fb4, haze: 0xd4dad8, tint: [0.90, 0.94, 0.96], sun: 0xffffff, cloud: 0xe4e8e8, fog: 360.0, night: false, rise: 0.22 },
    Sky { zenith: 0x2c2a66, haze: 0xf07a48, tint: [1.0, 0.70, 0.52], sun: 0xffb060, cloud: 0xf09a6a, fog: 520.0, night: false, rise: 0.05 },
    Sky { zenith: 0x04060f, haze: 0x141c34, tint: [0.26, 0.33, 0.52], sun: 0xdfe6ff, cloud: 0x10141f, fog: 560.0, night: true, rise: 0.30 },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind { Truck, Flak, Sam, Tank, Boat, Heli, Radar, Fuel, Bunker, Hangar, Tower }

impl Kind {
    /// Its name, for one and for many.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Kind::Truck => ("truck", "trucks"), Kind::Flak => ("flak gun", "flak guns"), Kind::Sam => ("missile site", "missile sites"),
            Kind::Tank => ("tank", "tanks"), Kind::Boat => ("gunboat", "gunboats"), Kind::Heli => ("gunship", "gunships"),
            Kind::Radar => ("radar", "radars"), Kind::Fuel => ("fuel tank", "fuel tanks"), Kind::Bunker => ("bunker", "bunkers"),
            Kind::Hangar => ("hangar", "hangars"), Kind::Tower => ("radio mast", "radio masts"),
        }
    }

    fn armour(self) -> f32 {
        match self {
            Kind::Truck => 12.0, Kind::Flak => 30.0, Kind::Sam => 35.0, Kind::Tank => 70.0, Kind::Boat => 50.0, Kind::Heli => 45.0,
            Kind::Radar => 40.0, Kind::Fuel => 25.0, Kind::Bunker => 220.0, Kind::Hangar => 110.0, Kind::Tower => 30.0,
        }
    }

    fn score(self) -> u32 {
        match self {
            Kind::Truck => 100, Kind::Flak => 300, Kind::Sam => 500, Kind::Tank => 400, Kind::Boat => 400, Kind::Heli => 800,
            Kind::Radar => 500, Kind::Fuel => 200, Kind::Bunker => 1500, Kind::Hangar => 800, Kind::Tower => 300,
        }
    }

    /// The ball a shot must enter to hit it: how wide, and how high its
    /// middle sits over the thing's foot.
    fn size(self) -> (f32, f32) {
        match self {
            Kind::Truck => (3.8, 1.6), Kind::Flak => (3.4, 1.6), Kind::Sam => (3.6, 1.8), Kind::Tank => (4.4, 1.8), Kind::Boat => (5.2, 1.4),
            Kind::Heli => (4.2, 0.0), Kind::Radar => (3.8, 3.2), Kind::Fuel => (4.4, 2.2), Kind::Bunker => (8.0, 2.2),
            Kind::Hangar => (8.5, 3.0), Kind::Tower => (5.0, 8.0),
        }
    }

    /// How far it shoots; 0 for what cannot.
    fn range(self) -> f32 {
        match self { Kind::Flak => 210.0, Kind::Sam => 620.0, Kind::Tank => 270.0, Kind::Boat => 200.0, Kind::Heli => 230.0, _ => 0.0 }
    }
}

/// "1 radar", "4 trucks".
fn count(kind: Kind, n: u32) -> String {
    let (one, many) = kind.names();
    format!("{} {}", n, if n == 1 { one } else { many })
}

/// One of theirs.
struct Foe {
    kind: Kind,
    x: f32, y: f32, h: f32,
    /// How fast it moves over the map.
    vx: f32, vy: f32,
    heading: f32,
    /// Where its gun, its launcher or its dish points.
    aim: f32,
    armour: f32,
    dead: bool,
    /// The mission is to destroy it.
    primary: bool,
    /// Seconds until it may fire again, shots left in the burst, seconds
    /// to the next of them.
    cool: f32, burst: u32, gap: f32,
    /// Whether it sees the gunship, for how long, and when it looks next.
    los: bool, seen: f32, look: f32,
    /// A boat's circle: the middle, how wide, where on it.
    cx: f32, cy: f32, orbit: f32, phase: f32,
    /// A gunship's height over the ground, and how fast its wreck falls;
    /// below zero once it lies still.
    agl: f32, fall: f32,
    /// Seconds a wreck keeps smoking.
    smoke: f32,
}

const KINDS: [Kind; 11] = [Kind::Truck, Kind::Flak, Kind::Sam, Kind::Tank, Kind::Boat, Kind::Heli, Kind::Radar, Kind::Fuel, Kind::Bunker,
    Kind::Hangar, Kind::Tower];

impl Foe {
    fn new(kind: Kind, primary: bool) -> Foe {
        Foe { kind, x: 0.0, y: 0.0, h: 0.0, vx: 0.0, vy: 0.0, heading: 0.0, aim: 0.0, armour: kind.armour(), dead: false, primary,
            cool: 0.0, burst: 0, gap: 0.0, los: false, seen: 0.0, look: 0.0, cx: 0.0, cy: 0.0, orbit: 0.0, phase: 0.0, agl: 0.0, fall: 0.0, smoke: 0.0 }
    }

    /// The middle of it: what a shot is aimed at.
    fn mid(&self) -> [f32; 3] { [self.x, self.y, self.h + self.kind.size().1] }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ammo { Round, Rocket, Missile, Shell, Tracer, Sam }

struct Shot {
    ammo: Ammo,
    at: [f32; 3],
    vel: [f32; 3],
    life: f32,
    /// Fired by the gunship.
    mine: bool,
    /// What a missile is after.
    target: Option<usize>,
}

impl Shot {
    fn new(ammo: Ammo, at: [f32; 3], dir: [f32; 3], speed: f32, life: f32, mine: bool, target: Option<usize>) -> Shot {
        Shot { ammo, at, vel: [dir[0] * speed, dir[1] * speed, dir[2] * speed], life, mine, target }
    }

    fn speed(&self) -> f32 { (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1] + self.vel[2] * self.vel[2]).sqrt() }

    /// Turn toward a point, by no more than `max` radians, at a new speed.
    fn steer(&mut self, to: [f32; 3], max: f32, speed: f32) {
        let v = self.speed().max(0.001);
        let (d, w) = ([self.vel[0] / v, self.vel[1] / v, self.vel[2] / v], toward(self.at, to));
        let angle = (d[0] * w[0] + d[1] * w[1] + d[2] * w[2]).clamp(-1.0, 1.0).acos();
        // Part of the way round the arc from the one heading to the other.
        let t = if angle > max { max / angle } else { 1.0 };
        let (a, b) = if angle < 0.001 { (1.0 - t, t) } else { (((1.0 - t) * angle).sin() / angle.sin(), (t * angle).sin() / angle.sin()) };
        for k in 0..3 { self.vel[k] = (d[k] * a + w[k] * b) * speed; }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Puff { Fire, Smoke, Spark, Dust, Foam, Trail }

/// A ball of fire, smoke, dust or foam.
struct Fx { puff: Puff, at: [f32; 3], vel: [f32; 3], age: f32, life: f32, size: f32, grow: f32 }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Site { Plain, Lake, Peaks }

struct Mission {
    name: &'static str,
    orders: [&'static str; 2],
    site: Site,
    sky: usize,
    /// What stands there: the kind, how many, and whether the mission is
    /// to destroy them.
    force: &'static [(Kind, u32, bool)],
}

static MISSIONS: [Mission; 6] = [
    Mission { name: "First Light", orders: ["A radar post watches the valley.", "Take out the radar and its trucks."], site: Site::Plain, sky: DAWN,
        force: &[(Kind::Radar, 1, true), (Kind::Truck, 4, true), (Kind::Flak, 2, false)] },
    Mission { name: "Iron Valley", orders: ["Six tanks are dug in on the plain.", "Leave none of them."], site: Site::Plain, sky: DAY,
        force: &[(Kind::Tank, 6, true), (Kind::Flak, 2, false), (Kind::Truck, 2, false)] },
    Mission { name: "Lake Patrol", orders: ["Gunboats patrol the lake, their fuel is on the shore.", "Sink the boats and burn the fuel."], site: Site::Lake, sky: MIST,
        force: &[(Kind::Boat, 4, true), (Kind::Fuel, 4, true), (Kind::Flak, 2, false), (Kind::Tank, 1, false)] },
    Mission { name: "Weasel", orders: ["Missile sites guard the high pass.", "Stay low. Kill the launchers and their radar."], site: Site::Peaks, sky: DUSK,
        force: &[(Kind::Sam, 3, true), (Kind::Radar, 1, true), (Kind::Flak, 2, false), (Kind::Tank, 1, false)] },
    Mission { name: "Air Cover", orders: ["Gunships guard a supply column.", "Shoot them down, then burn the trucks."], site: Site::Plain, sky: DAY,
        force: &[(Kind::Heli, 3, true), (Kind::Truck, 5, true), (Kind::Flak, 1, false), (Kind::Sam, 1, false)] },
    Mission { name: "The Fortress", orders: ["Their command sits under concrete.", "Break the bunker, the hangars and the mast."], site: Site::Peaks, sky: NIGHT,
        force: &[(Kind::Bunker, 1, true), (Kind::Hangar, 2, true), (Kind::Tower, 1, true), (Kind::Fuel, 3, false), (Kind::Sam, 2, false),
            (Kind::Flak, 3, false), (Kind::Tank, 3, false), (Kind::Heli, 2, false)] },
];

/// Where a mission is fought: of many places tried, the one that fits
/// its kind best and lies away from the others.
fn find_site(t: &Terrain, site: Site, taken: &[(f32, f32)], rng: &mut Rng) -> (f32, f32) {
    let mut best = ((0.0, 0.0), f32::MIN);
    for _ in 0..300 {
        let (x, y) = (rng.range(0.0, WORLD), rng.range(0.0, WORLD));
        let (land, high) = t.survey(x, y, 110.0);
        let mut score = match site {
            Site::Plain => land - (high - 0.45).abs() * 4.0,
            Site::Lake => 1.0 - (land - 0.5).abs() * 3.0,
            Site::Peaks => land - (high - 0.72).abs() * 4.0,
        };
        if taken.iter().any(|&(tx, ty)| wrap(tx - x).hypot(wrap(ty - y)) < 280.0) { score -= 10.0; }
        if score > best.1 { best = ((x, y), score); }
    }
    best.0
}

/// Where the gunship starts: dry ground a short flight from the site,
/// not on a mountain and not on a shore.
fn find_pad(t: &Terrain, site: (f32, f32), rng: &mut Rng) -> (f32, f32) {
    let mut best = (site, f32::MAX);
    for _ in 0..200 {
        let (a, d) = (rng.range(0.0, TAU), rng.range(380.0, 440.0));
        let (x, y) = (site.0 + a.sin() * d, site.1 + a.cos() * d);
        if !t.land(x, y) { continue; }
        let (land, high) = t.survey(x, y, 40.0);
        let score = t.slope(x, y) * 0.3 + (1.0 - land) * 5.0 + (high - 0.45).abs() * 3.0;
        if score < best.1 { best = ((x.rem_euclid(WORLD), y.rem_euclid(WORLD)), score); }
    }
    best.0
}

/// Level the ground about a point: flat out to `inner`, then easing back
/// into the land as it was by `outer`. On a shore the hills are cut down
/// to just over the water, and the lake and its beaches stay.
fn level(height: &mut [f32], c: (f32, f32), inner: f32, outer: f32, shore: bool) {
    let cells = |r: f32| {
        let n = r.ceil() as i32;
        (-n..=n).flat_map(move |dy| (-n..=n).map(move |dx| (dx, dy))).filter(move |&(dx, dy)| ((dx * dx + dy * dy) as f32).sqrt() < r)
    };
    let at = |dx: i32, dy: i32| ((c.1 as i32 + dy).rem_euclid(MAP as i32) as usize) * MAP + (c.0 as i32 + dx).rem_euclid(MAP as i32) as usize;
    let (mut sum, mut n) = (0.0, 0.0);
    for (dx, dy) in cells(inner) { sum += height[at(dx, dy)]; n += 1.0; }
    let dry = WATER + 0.025;
    let goal = if shore { dry } else { (sum / n).max(dry) };
    for (dx, dy) in cells(outer) {
        let h = &mut height[at(dx, dy)];
        if shore && *h <= goal { continue; }
        let d = ((dx * dx + dy * dy) as f32).sqrt();
        *h += (goal - *h) * smooth(((outer - d) / (outer - inner)).clamp(0.0, 1.0));
    }
}

/// Choose where the missions are fought and flown from, and level the
/// ground there, before the land gets its colours.
fn plan(height: &mut Vec<f32>) -> (Vec<(f32, f32)>, Vec<(f32, f32)>) {
    let mut rng = Rng::new(7);
    let (mut sites, mut pads) = (Vec::new(), Vec::new());
    for m in &MISSIONS {
        // The land so far, heights only, to look for a place in.
        let t = Terrain { levels: vec![Level { size: MAP, height: std::mem::take(height), color: Vec::new() }], day: Vec::new(), clouds: Vec::new(),
            sites: Vec::new(), pads: Vec::new() };
        let site = find_site(&t, m.site, &sites, &mut rng);
        let pad = find_pad(&t, site, &mut rng);
        *height = t.levels.into_iter().next().unwrap().height;
        level(height, site, 85.0, 200.0, m.site == Site::Lake);
        level(height, pad, 16.0, 60.0, false);
        sites.push(site);
        pads.push(pad);
    }
    (sites, pads)
}

/// A place to stand near a point: of some tries on dry land clear of
/// what stands there already, the flattest. The circle widens until one
/// is found.
fn spot(t: &Terrain, rng: &mut Rng, c: (f32, f32), r: f32, used: &[(f32, f32)], gap: f32) -> (f32, f32) {
    let mut r = r;
    for _ in 0..5 {
        let mut best = (c, f32::MAX);
        for _ in 0..60 {
            let (a, d) = (rng.range(0.0, TAU), r * rng.float().sqrt());
            let (x, y) = (c.0 + a.sin() * d, c.1 + a.cos() * d);
            if !t.land(x, y) || used.iter().any(|&(ux, uy)| wrap(ux - x).hypot(wrap(uy - y)) < gap) { continue; }
            let s = t.slope(x, y);
            if s < best.1 { best = ((x, y), s); }
        }
        if best.1 < f32::MAX { return best.0; }
        r *= 1.6;
    }
    c
}

/// Water for a boat near a point: the middle of a circle it can sail,
/// and how wide. A boat with no room lies still.
fn pond(t: &Terrain, rng: &mut Rng, c: (f32, f32), used: &[(f32, f32)]) -> (f32, f32, f32) {
    let deep = |x: f32, y: f32| t.raw(x, y) < WATER - 0.02;
    for orbit in [30.0, 20.0, 12.0, 0.0] {
        for _ in 0..200 {
            let (a, d) = (rng.range(0.0, TAU), 160.0 * rng.float().sqrt());
            let (x, y) = (c.0 + a.sin() * d, c.1 + a.cos() * d);
            if used.iter().any(|&(ux, uy)| wrap(ux - x).hypot(wrap(uy - y)) < orbit * 2.0 + 12.0) { continue; }
            if deep(x, y) && (0..10).all(|k| { let b = k as f32 * TAU / 10.0; deep(x + b.sin() * (orbit + 6.0), y + b.cos() * (orbit + 6.0)) }) { return (x, y, orbit); }
        }
    }
    (c.0, c.1, 0.0)
}

/// A box standing on a point: so wide, so tall, so deep.
fn slab(m: &mut Mesh, w: f32, h: f32, d: f32, at: (f32, f32, f32), c: Rgb) {
    m.extend(&Mesh::cuboid(w, h, d, c), &M4::translate(V3::new(at.0, at.1 + h / 2.0, at.2)));
}

/// A beam that runs ahead from a point, its far end lifted by an angle:
/// a barrel, a rail.
fn beam(m: &mut Mesh, w: f32, h: f32, d: f32, at: (f32, f32, f32), lift: f32, c: Rgb) {
    let t = M4::translate(V3::new(0.0, 0.0, d / 2.0)).then(&M4::rotate_x(-lift)).then(&M4::translate(V3::new(at.0, at.1, at.2)));
    m.extend(&Mesh::cuboid(w, h, d, c), &t);
}

/// A flat face of three or four corners, turned to look away from `inside`.
fn face(m: &mut Mesh, p: &[V3], inside: V3, c: Rgb) {
    let n = p[1].sub(p[0]).cross(p[2].sub(p[0]));
    let q: Vec<V3> = if n.dot(p[0].sub(inside)) >= 0.0 { p.to_vec() } else { p.iter().rev().copied().collect() };
    if q.len() == 3 { m.tri(q[0], q[1], q[2], c); } else { m.quad(q[0], q[1], q[2], q[3], c); }
}

/// A block whose top is another size than its foot: sloped walls, or a
/// roof with a ridge when the top has no width.
fn taper(m: &mut Mesh, foot: (f32, f32), top: (f32, f32), h: f32, at: (f32, f32, f32), c: Rgb) {
    let p = |s: (f32, f32), y: f32, i: usize| {
        let (sx, sz) = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)][i];
        V3::new(at.0 + sx * s.0 / 2.0, at.1 + y, at.2 + sz * s.1 / 2.0)
    };
    let mid = V3::new(at.0, at.1 + h / 2.0, at.2);
    for i in 0..4 {
        let j = (i + 1) % 4;
        face(m, &[p(foot, 0.0, i), p(foot, 0.0, j), p(top, h, j), p(top, h, i)], mid, c);
    }
    face(m, &[p(top, h, 0), p(top, h, 1), p(top, h, 2), p(top, h, 3)], mid, c);
    face(m, &[p(foot, 0.0, 0), p(foot, 0.0, 1), p(foot, 0.0, 2), p(foot, 0.0, 3)], mid, c);
}

/// A round tank: flat sides about an upright axis, and a lid.
fn drum(m: &mut Mesh, r: f32, h: f32, sides: u32, at: (f32, f32, f32), wall: Rgb, lid: Rgb) {
    let p = |i: u32, y: f32| { let a = i as f32 * TAU / sides as f32; V3::new(at.0 + a.sin() * r, at.1 + y, at.2 + a.cos() * r) };
    for i in 0..sides {
        m.quad(p(i, 0.0), p(i + 1, 0.0), p(i + 1, h), p(i, h), wall);
        m.tri(V3::new(at.0, at.1 + h, at.2), p(i, h), p(i + 1, h), lid);
    }
}

/// What a thing looks like: its body, and the part that turns on it.
/// A model looks north; x is its right, y is up, z is ahead.
fn model(kind: Kind) -> (Mesh, Mesh) {
    let (mut b, mut t) = (Mesh::new(), Mesh::new());
    match kind {
        Kind::Truck => {
            slab(&mut b, 2.0, 0.5, 5.6, (0.0, 0.5, 0.0), 0x2c2c28);
            slab(&mut b, 2.0, 1.4, 1.5, (0.0, 1.0, 2.0), 0x5a6a3a);
            slab(&mut b, 1.7, 0.6, 0.1, (0.0, 1.6, 2.76), 0x9fc0d0);
            slab(&mut b, 2.2, 1.9, 3.6, (0.0, 1.0, -0.9), 0x8c8a5c);
            for (x, z) in [(-1.0, 1.9), (1.0, 1.9), (-1.0, -1.7), (1.0, -1.7)] { slab(&mut b, 0.45, 0.9, 0.9, (x, 0.0, z), 0x18181a); }
        }
        Kind::Tank => {
            for x in [-1.45, 1.45] { slab(&mut b, 0.85, 0.95, 6.4, (x, 0.0, 0.0), 0x26261f); }
            taper(&mut b, (2.5, 6.0), (2.3, 5.0), 0.75, (0.0, 0.55, 0.0), 0x4f5a36);
            taper(&mut t, (2.1, 2.7), (1.5, 2.0), 0.8, (0.0, 1.3, -0.2), 0x5a6640);
            beam(&mut t, 0.28, 0.28, 3.9, (0.0, 1.7, 0.9), 0.05, 0x33382a);
        }
        Kind::Flak => {
            taper(&mut b, (4.4, 4.4), (3.6, 3.6), 0.9, (0.0, 0.0, 0.0), 0x9c8f6a);
            slab(&mut t, 0.9, 1.0, 0.9, (0.0, 0.9, 0.0), 0x30342c);
            slab(&mut t, 1.7, 0.9, 0.14, (0.0, 1.5, 0.55), 0x4a5142);
            for x in [-0.3, 0.3] { beam(&mut t, 0.18, 0.18, 3.2, (x, 1.9, 0.3), 0.55, 0x22251f); }
        }
        Kind::Sam => {
            for x in [-1.5, 1.5] { slab(&mut b, 0.7, 0.7, 4.9, (x, 0.0, 0.0), 0x26261f); }
            slab(&mut b, 3.0, 0.8, 4.6, (0.0, 0.4, 0.0), 0x4a5540);
            slab(&mut t, 0.9, 0.7, 0.9, (0.0, 1.2, 0.0), 0x3c4236);
            beam(&mut t, 1.7, 0.25, 4.2, (0.0, 1.7, -1.7), 0.5, 0x3c4236);
            for x in [-0.45, 0.45] { beam(&mut t, 0.4, 0.4, 3.9, (x, 2.05, -1.6), 0.5, 0xe8e8e0); }
        }
        Kind::Boat => {
            taper(&mut b, (2.0, 8.0), (3.0, 9.0), 1.3, (0.0, -0.2, -0.5), 0x6f7a84);
            let (l, r, tip, keel) = (V3::new(-1.5, 1.1, 4.0), V3::new(1.5, 1.1, 4.0), V3::new(0.0, 1.1, 6.8), V3::new(0.0, -0.2, 5.4));
            let inside = V3::new(0.0, 0.6, 4.6);
            face(&mut b, &[l, r, tip], inside, 0x4c545c);
            face(&mut b, &[l, tip, keel], inside, 0x6f7a84);
            face(&mut b, &[r, tip, keel], inside, 0x6f7a84);
            slab(&mut b, 2.0, 1.3, 2.8, (0.0, 1.1, -1.6), 0x9aa4ac);
            slab(&mut b, 1.6, 0.4, 0.1, (0.0, 1.8, -0.18), 0x26333f);
            slab(&mut b, 0.15, 1.7, 0.15, (0.0, 2.4, -1.6), 0x30343a);
            slab(&mut t, 1.0, 0.6, 1.0, (0.0, 1.1, 0.0), 0x30343a);
            beam(&mut t, 0.2, 0.2, 2.3, (0.0, 1.5, 0.2), 0.15, 0x1c1e22);
        }
        Kind::Heli => {
            taper(&mut b, (1.3, 4.2), (1.5, 4.6), 0.9, (0.0, -1.0, 0.3), 0x3a3f34);
            taper(&mut b, (1.5, 4.6), (1.0, 3.0), 0.9, (0.0, -0.1, 0.0), 0x3a3f34);
            slab(&mut b, 1.0, 0.7, 1.2, (0.0, -0.1, 1.4), 0x7fa6bc);
            slab(&mut b, 0.36, 0.42, 4.8, (0.0, -0.3, -4.2), 0x33382e);
            slab(&mut b, 0.16, 1.7, 0.9, (0.0, -0.2, -6.3), 0x4a5142);
            slab(&mut b, 3.8, 0.14, 0.9, (0.0, -0.4, 0.2), 0x33382e);
            for x in [-1.7, 1.7] { slab(&mut b, 0.4, 0.4, 1.6, (x, -0.8, 0.2), 0x22251f); }
            for x in [-0.8, 0.8] { slab(&mut b, 0.12, 0.12, 3.4, (x, -1.5, 0.4), 0x18181a); }
            slab(&mut t, 0.25, 0.5, 0.25, (0.0, 0.8, 0.0), 0x22251f);
            slab(&mut t, 10.0, 0.06, 0.4, (0.0, 1.3, 0.0), 0x18181a);
            slab(&mut t, 0.4, 0.06, 10.0, (0.0, 1.3, 0.0), 0x18181a);
        }
        Kind::Radar => {
            slab(&mut b, 3.2, 3.4, 3.2, (0.0, -1.2, 0.0), 0x8a8f86);
            slab(&mut b, 0.3, 1.5, 0.3, (0.0, 2.2, 0.0), 0x4a4e50);
            slab(&mut t, 4.2, 2.6, 0.25, (0.0, 3.6, 0.2), 0xc8ccc4);
            slab(&mut t, 0.16, 0.16, 1.3, (0.0, 4.8, 0.3), 0x4a4e50);
        }
        Kind::Fuel => {
            drum(&mut b, 2.2, 5.0, 10, (0.0, -1.0, 0.0), 0xc4c8c0, 0xa4a8a0);
            drum(&mut b, 2.26, 0.6, 10, (0.0, 2.0, 0.0), 0xb03a2a, 0xb03a2a);
        }
        Kind::Bunker => {
            taper(&mut b, (14.0, 11.0), (9.5, 6.5), 6.0, (0.0, -2.6, 0.0), 0x8c8a80);
            slab(&mut b, 5.0, 0.5, 0.6, (0.0, 1.9, 3.5), 0x18181a);
            slab(&mut b, 2.2, 0.4, 2.2, (0.0, 3.4, 0.0), 0x6c6a62);
        }
        Kind::Hangar => {
            slab(&mut b, 12.0, 6.5, 15.0, (0.0, -2.5, 0.0), 0x7c8288);
            taper(&mut b, (12.5, 15.4), (0.0, 15.4), 2.4, (0.0, 4.0, 0.0), 0x5d646c);
            slab(&mut b, 8.0, 3.2, 0.2, (0.0, 0.0, 7.5), 0x2a2e33);
        }
        Kind::Tower => {
            slab(&mut b, 2.8, 3.4, 2.8, (2.4, -1.2, 0.0), 0x8a8f86);
            for i in 0..4 { slab(&mut b, 0.5, 4.0, 0.5, (0.0, i as f32 * 4.0, 0.0), if i % 2 == 0 { 0xc03020 } else { 0xe4e4e0 }); }
            slab(&mut b, 0.9, 0.3, 0.9, (0.0, 16.0, 0.0), 0xc03020);
        }
    }
    (b, t)
}

/// Where the turning part of a thing sits, ahead of its middle.
fn pivot(kind: Kind) -> f32 { if kind == Kind::Boat { 2.4 } else { 0.0 } }

struct Models { body: Vec<Mesh>, top: Vec<Mesh>, pad: Mesh }

impl Models {
    fn new() -> Models {
        let (mut body, mut top) = (Vec::new(), Vec::new());
        for kind in KINDS { let (b, t) = model(kind); body.push(b); top.push(t); }
        // The pad the gunship flies from, with an H on it.
        let mut pad = Mesh::new();
        slab(&mut pad, 17.0, 6.0, 17.0, (0.0, -6.0, 0.0), 0x5a5e62);
        for x in [-2.2, 2.2] { slab(&mut pad, 1.0, 0.1, 6.6, (x, 0.0, 0.0), 0xe8e8e0); }
        slab(&mut pad, 4.4, 0.1, 1.0, (0.0, 0.0, 0.0), 0xe8e8e0);
        Models { body, top, pad }
    }
}

/// The eye: where it is, which way it looks, where the horizon falls and
/// how much it leans.
#[derive(Clone, Copy)]
struct Eye { x: f32, y: f32, alt: f32, yaw: f32, sin: f32, cos: f32, hz: f32, tilt: f32 }

impl Eye {
    /// A point of the world as the eye has it: to the right, up, ahead.
    fn cam(&self, p: [f32; 3]) -> V3 {
        let (dx, dy) = (wrap(p[0] - self.x), wrap(p[1] - self.y));
        V3::new(dx * self.cos - dy * self.sin, p[2] - self.alt, dx * self.sin + dy * self.cos)
    }

    /// The row of the horizon in a column.
    fn horizon(&self, sx: f32) -> f32 { self.hz + (sx - W as f32 / 2.0) * self.tilt }

    /// Where on the screen a point ahead of the eye falls.
    fn screen(&self, c: V3) -> (f32, f32) {
        let sx = W as f32 / 2.0 + c.x * FOCAL / c.z;
        (sx, self.horizon(sx) - c.y * FOCAL / c.z)
    }
}

/// Two colours mixed in whole numbers, `t` from 0 to 256: fast enough
/// for a cloud of smoke that fills the view.
fn mix(a: Rgb, b: Rgb, t: u32) -> Rgb {
    let rb = (((a & 0xff00ff) * (256 - t) + (b & 0xff00ff) * t) >> 8) & 0xff00ff;
    let g = (((a & 0x00ff00) * (256 - t) + (b & 0x00ff00) * t) >> 8) & 0x00ff00;
    rb | g
}

/// What stands on the land is painted here: on the frame, behind the
/// land where the land is nearer, and in front of one another.
struct Paint<'a> {
    f: &'a mut Frame,
    /// How far off the land is in each pixel, a column at a time.
    land: &'a [f32],
    /// How far off the nearest thing painted so far is.
    over: &'a mut [f32],
    eye: Eye,
    sky: &'static Sky,
    /// The way to the sun, as the eye has it.
    sun: V3,
    v: Vec<V3>,
}

impl Paint<'_> {
    /// How far behind the land a thing may lie and still show. The land's
    /// depth is only as fine as the steps its rays took.
    fn slack(z: f32) -> f32 { 1.0 + z * 0.012 }

    /// A model at a place, turned to a heading; `dark` below 1 for a wreck.
    fn mesh(&mut self, m: &Mesh, at: [f32; 3], heading: f32, dark: f32) {
        let o = self.eye.cam(at);
        if o.z < -40.0 || o.z > 1400.0 || o.x.abs() > o.z.max(0.0) * 1.2 + 40.0 { return; }
        let (s, c) = (heading - self.eye.yaw).sin_cos();
        let fog = 1.0 - (-o.z.max(0.0) / self.sky.fog).exp();
        let mut v = std::mem::take(&mut self.v);
        v.clear();
        v.extend(m.verts.iter().map(|p| V3::new(o.x + p.x * c + p.z * s, o.y + p.y, o.z - p.x * s + p.z * c)));
        for (t, &col) in m.tris.iter().zip(&m.colors) {
            let p = [v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]];
            let n = p[1].sub(p[0]).cross(p[2].sub(p[0]));
            if n.dot(p[0]) >= 0.0 { continue; }
            let lit = 0.42 + 0.58 * n.norm().dot(self.sun).max(0.0);
            self.poly(&p, lerp(tinted(scale(col, lit * dark), self.sky.tint), self.sky.haze, fog));
        }
        self.v = v;
    }

    /// A triangle in the eye's space, cut where it comes too near.
    fn poly(&mut self, p: &[V3; 3], c: Rgb) {
        let mut q = [(0.0f32, 0.0f32, 0.0f32); 4];
        let mut n = 0;
        for i in 0..3 {
            let (a, b) = (p[i], p[(i + 1) % 3]);
            if a.z >= NEAR { let (x, y) = self.eye.screen(a); q[n] = (x, y, 1.0 / a.z); n += 1; }
            if (a.z >= NEAR) != (b.z >= NEAR) {
                let m = a.lerp(b, (NEAR - a.z) / (b.z - a.z));
                let (x, y) = self.eye.screen(m);
                q[n] = (x, y, 1.0 / m.z);
                n += 1;
            }
        }
        if n < 3 { return; }
        self.fill(q[0], q[1], q[2], c);
        if n == 4 { self.fill(q[0], q[2], q[3], c); }
    }

    /// A triangle on the screen: column, row and one over the distance.
    fn fill(&mut self, a: (f32, f32, f32), b: (f32, f32, f32), c: (f32, f32, f32), col: Rgb) {
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area.abs() < 1e-4 { return; }
        let (x0, x1) = (a.0.min(b.0).min(c.0).floor().max(0.0) as i32, a.0.max(b.0).max(c.0).ceil().min(W as f32 - 1.0) as i32);
        let (y0, y1) = (a.1.min(b.1).min(c.1).floor().max(0.0) as i32, a.1.max(b.1).max(c.1).ceil().min(VH as f32 - 1.0) as i32);
        let inv = 1.0 / area;
        for y in y0..=y1 {
            let py = y as f32 + 0.5;
            for x in x0..=x1 {
                let px = x as f32 + 0.5;
                let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) * inv;
                let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 { continue; }
                let z = 1.0 / (w0 * a.2 + w1 * b.2 + w2 * c.2);
                let i = (x * VH + y) as usize;
                if z < self.over[i] && z < self.land[i] + Paint::slack(z) {
                    self.over[i] = z;
                    self.f.px[(y * W + x) as usize] = col;
                }
            }
        }
    }

    /// A soft ball of colour: smoke, fire, a shell. `a` is how solid, 0 to 1.
    fn ball(&mut self, at: [f32; 3], r: f32, c: Rgb, a: f32) {
        let p = self.eye.cam(at);
        if p.z < NEAR { return; }
        let (sx, sy) = self.eye.screen(p);
        let (mut pr, mut a) = ((r * FOCAL / p.z).max(1.0), a);
        // A ball that fills the view is thinned, so flying through smoke
        // does not turn the screen one colour.
        if pr > 160.0 { a *= 160.0 / pr; pr = 160.0; }
        if sx + pr < 0.0 || sx - pr >= W as f32 || sy + pr < 0.0 || sy - pr >= VH as f32 { return; }
        let slack = Paint::slack(p.z);
        for y in ((sy - pr) as i32).max(0)..=((sy + pr) as i32).min(VH - 1) {
            for x in ((sx - pr) as i32).max(0)..=((sx + pr) as i32).min(W - 1) {
                let (dx, dy) = (x as f32 + 0.5 - sx, y as f32 + 0.5 - sy);
                let k = 1.0 - (dx * dx + dy * dy) / (pr * pr);
                if k <= 0.0 && pr > 1.0 { continue; }
                let i = (x * VH + y) as usize;
                if p.z < self.land[i] + slack && p.z < self.over[i] + 1.0 {
                    let t = (a * if pr > 1.0 { (k * 2.5).min(1.0) } else { 1.0 } * 256.0) as u32;
                    let j = (y * W + x) as usize;
                    self.f.px[j] = mix(self.f.px[j], c, t.min(256));
                }
            }
        }
    }

    /// A line between two points of the world: a tracer.
    fn streak(&mut self, a: [f32; 3], b: [f32; 3], c: Rgb) {
        let (mut p, mut q) = (self.eye.cam(a), self.eye.cam(b));
        if p.z < NEAR && q.z < NEAR { return; }
        if p.z < NEAR { p = q.lerp(p, (q.z - NEAR) / (q.z - p.z)); }
        if q.z < NEAR { q = p.lerp(q, (p.z - NEAR) / (p.z - q.z)); }
        let ((ax, ay), (bx, by)) = (self.eye.screen(p), self.eye.screen(q));
        let n = (bx - ax).abs().max((by - ay).abs()).ceil().clamp(1.0, 1500.0) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let (x, y) = ((ax + (bx - ax) * t) as i32, (ay + (by - ay) * t) as i32);
            if x < 0 || x >= W || y < 0 || y >= VH { continue; }
            let z = 1.0 / (1.0 / p.z + (1.0 / q.z - 1.0 / p.z) * t);
            let i = (x * VH + y) as usize;
            if z < self.land[i] + Paint::slack(z) && z < self.over[i] + 0.5 { self.f.px[(y * W + x) as usize] = c; }
        }
    }
}

/// How near a point comes to the stretch flown from `a` to `b`.
fn pass(a: [f32; 3], b: [f32; 3], p: [f32; 3]) -> f32 {
    let d = [wrap(b[0] - a[0]), wrap(b[1] - a[1]), b[2] - a[2]];
    let w = [wrap(p[0] - a[0]), wrap(p[1] - a[1]), p[2] - a[2]];
    let l2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let t = if l2 > 0.0 { ((w[0] * d[0] + w[1] * d[1] + w[2] * d[2]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let e = [w[0] - d[0] * t, w[1] - d[1] * t, w[2] - d[2] * t];
    (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt()
}

/// Who stands where in a mission. The same every time it is flown.
fn muster(t: &Terrain, n: usize, site: (f32, f32)) -> Vec<Foe> {
    let mut rng = Rng::new(4000 + n as u64);
    let mut foes = Vec::new();
    let mut used: Vec<(f32, f32)> = Vec::new();
    for &(kind, many, primary) in MISSIONS[n].force {
        // Where the first of its kind stands: trucks and fuel tanks keep
        // together.
        let mut first: Option<(f32, f32)> = None;
        for _ in 0..many {
            let mut foe = Foe::new(kind, primary);
            match kind {
                Kind::Boat => {
                    let (cx, cy, orbit) = pond(t, &mut rng, site, &used);
                    used.push((cx, cy));
                    foe.cx = cx; foe.cy = cy; foe.orbit = orbit; foe.phase = rng.range(0.0, TAU);
                    foe.x = (cx + foe.phase.sin() * orbit).rem_euclid(WORLD);
                    foe.y = (cy + foe.phase.cos() * orbit).rem_euclid(WORLD);
                    foe.h = WATER * HSCALE;
                    foe.heading = foe.phase + PI / 2.0;
                }
                Kind::Heli => {
                    let (a, d) = (rng.range(0.0, TAU), rng.range(30.0, 110.0));
                    foe.x = (site.0 + a.sin() * d).rem_euclid(WORLD);
                    foe.y = (site.1 + a.cos() * d).rem_euclid(WORLD);
                    foe.agl = rng.range(22.0, 38.0);
                    foe.h = t.ground(foe.x, foe.y) + foe.agl;
                    foe.heading = rng.range(-PI, PI);
                }
                _ => {
                    let (c, r, gap) = match (kind, first) {
                        (Kind::Fuel, Some(g)) => (g, 14.0, 6.5),
                        (Kind::Truck, Some(g)) => (g, 28.0, 8.0),
                        (Kind::Bunker | Kind::Hangar, _) => (site, 70.0, 28.0),
                        (Kind::Flak | Kind::Sam | Kind::Tank, _) if !primary => (site, 130.0, 22.0),
                        _ => (site, 80.0, 20.0),
                    };
                    let p = spot(t, &mut rng, c, r, &used, gap);
                    used.push(p);
                    first.get_or_insert(p);
                    foe.x = p.0.rem_euclid(WORLD);
                    foe.y = p.1.rem_euclid(WORLD);
                    foe.h = t.ground(foe.x, foe.y);
                    foe.heading = rng.range(-PI, PI);
                }
            }
            foe.aim = foe.heading;
            foe.look = rng.float() * 0.3;
            foe.cool = rng.range(0.5, 2.0);
            foes.push(foe);
        }
    }
    foes
}

/// A sound made a sample at a time: the time in seconds and a random
/// number from -1 to 1 go in, the loudness from -1 to 1 comes out.
fn synth(secs: f32, mut wave: impl FnMut(f32, f32) -> f32) -> Sample {
    let rate = funkey::audio::RATE as f32;
    let mut x = 0x2545_f491u32;
    Sample::from_i16((0..(secs * rate) as usize).map(|i| {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        let r = (x >> 8) as f32 / 8388608.0 - 1.0;
        (wave(i as f32 / rate, r).clamp(-1.0, 1.0) * 32000.0) as i16
    }).collect())
}

const TUNE: &str = "112 a4/2 e5/4 d5/8 c5/8 b4/2 g4/2 a4/2 e5/4 f5/8 e5/8 d5/1 \
    c5/2 g5/4 f5/8 e5/8 d5/2 b4/2 c5/4 b4/4 a4/4 g#4/4 a4/1";
const BASS: &str = "112 a2/8 a2/8 a2/8 a2/8 a2/8 a2/8 a2/8 a2/8 g2/8 g2/8 g2/8 g2/8 g2/8 g2/8 g2/8 g2/8 \
    f2/8 f2/8 f2/8 f2/8 f2/8 f2/8 f2/8 f2/8 d2/8 d2/8 d2/8 d2/8 e2/8 e2/8 e2/8 e2/8 \
    c3/8 c3/8 c3/8 c3/8 c3/8 c3/8 c3/8 c3/8 g2/8 g2/8 g2/8 g2/8 g2/8 g2/8 g2/8 g2/8 \
    f2/8 f2/8 f2/8 f2/8 e2/8 e2/8 e2/8 e2/8 a2/8 a2/8 a3/8 a2/8 a2/8 a2/8 e2/8 e2/8";

struct Sounds { title: Sample, rotor: Sample, gun: Sample, rocket: Sample, missile: Sample, boom: Sample, clank: Sample, thud: Sample,
    lock: Sample, launch: Sample, tick: Sample, win: Sample, lose: Sample }

impl Sounds {
    fn new() -> Sounds {
        // The tune over its bass, mixed into one loop so the two never drift.
        let (a, b) = (Tune::parse(TUNE, Wave::Square, 0.13).render(), Tune::parse(BASS, Wave::Triangle, 0.30).render());
        let at = |s: &Sample, i: usize| *s.data.get(i).unwrap_or(&0) as i32;
        let title = Sample::from_i16((0..a.data.len().max(b.data.len())).map(|i| (at(&a, i) + at(&b, i)).clamp(-32768, 32767) as i16).collect());
        // The rotor: twelve blade beats a second over a thin whine. One
        // second long, and every part of it fits that second whole.
        let mut lp = 0.0f32;
        let rotor = synth(1.0, |t, r| {
            let beat = (-(t * 12.0).fract() * 7.0).exp();
            lp += (r - lp) * 0.12;
            lp * beat * 1.7 + (TAU * 48.0 * t).sin() * beat * 0.35 + (TAU * 300.0 * t).sin() * 0.03 + lp * 0.15
        });
        let gun = synth(0.08, |t, r| (r * 0.7 + if (t * 150.0).fract() < 0.5 { 0.5 } else { -0.5 }) * (-t * 40.0).exp());
        let mut lp = 0.0f32;
        let rocket = synth(0.7, |t, r| { lp += (r - lp) * (0.08 + t * 0.3); lp * (1.0 - t / 0.7) * 1.8 });
        let mut lp = 0.0f32;
        let missile = synth(1.0, |t, r| { lp += (r - lp) * 0.25; (lp * 1.2 + (TAU * (180.0 + 250.0 * t) * t).sin() * 0.15) * (1.0 - t).powf(0.7) });
        let mut lp = 0.0f32;
        let boom = synth(1.4, |t, r| {
            lp += (r - lp) * (0.05 + 0.25 * (-t * 6.0).exp());
            lp * 2.6 * (-t * 3.0).exp() + (TAU * (62.0 - 11.0 * t) * t).sin() * 0.7 * (-t * 4.5).exp()
        });
        let clank = synth(0.12, |t, r| ((TAU * (900.0 - 2000.0 * t) * t).sin() * 0.6 + r * 0.4) * (-t * 28.0).exp());
        let mut lp = 0.0f32;
        let thud = synth(0.09, |t, r| { lp += (r - lp) * 0.2; lp * 2.0 * (-t * 30.0).exp() });
        Sounds { title, rotor, gun, rocket, missile, boom, clank, thud,
            lock: Sample::tone(Wave::Square, 990.0, 0.07, 0.18),
            launch: Sample::tone(Wave::Square, 1480.0, 0.05, 0.22),
            tick: Sample::tone(Wave::Square, 220.0, 0.04, 0.2),
            win: Tune::parse("200 c5/8 e5/8 g5/8 c6/4 g5/8 c6/2", Wave::Triangle, 0.5).render(),
            lose: Tune::parse("100 e4/4 d4/4 c4/4 a3/2", Wave::Triangle, 0.5).render() }
    }
}

/// One column of land, near to far, into `col` from the top down, and
/// how far off each of its pixels is into `dep`. Returns the row where
/// the land begins.
#[allow(clippy::too_many_arguments)]
fn column(t: &Terrain, sky: &Sky, time: f32, dir: (f32, f32), x: usize, col: &mut [Rgb], dep: &mut [f32], eye: &Eye) -> i32 {
    let k = (x as f32 + 0.5 - W as f32 / 2.0) / FOCAL;
    let horizon = eye.hz + (x as f32 + 0.5 - W as f32 / 2.0) * eye.tilt;
    let glare = turn(eye.yaw + k.atan() - SUN_AZ).cos().max(0.0).powi(10) * if sky.night { 0.5 } else { 1.0 };
    let stretch = (1.0 + k * k).sqrt();
    dep.fill(f32::INFINITY);
    let (mut z, mut dz, mut ymin) = (1.0f32, 0.4f32, VH);
    // The colour at the top of the last span, while the ground runs on
    // unbroken; a span shades from its own colour down to it.
    let mut below: Option<(Rgb, f32)> = None;
    while z < 1400.0 {
        let (px, py) = (eye.x + dir.0 * z * stretch, eye.y + dir.1 * z * stretch);
        // Read the map as coarse as the step along the ground.
        let lod = (dz * stretch).log2().max(0.0);
        let mut h = t.height(px, py, lod);
        let water = h < WATER;
        if water { h = WATER; }
        let fy = horizon + (eye.alt - h * HSCALE) * FOCAL / z;
        let sy = fy as i32;
        if sy < ymin {
            let mut c = t.color(px, py, lod);
            if water {
                // Waves, and the glitter of the sun or the moon on them.
                let wave = noise(px * 0.35 + time * 1.3, py * 0.35 - time * 0.9, 256, 3);
                c = lerp(c, sky.sun, (glare * (wave - 0.35).clamp(0.0, 1.0) * 1.6).min(1.0));
                c = lerp(c, sky.zenith, 0.25);
            } else if z < 48.0 {
                // Close to, the ground gets a grain finer than the map.
                c = scale(c, 1.0 + (noise(px * 3.0, py * 3.0, 3072, 9) - 0.5) * 0.34 * (1.0 - z / 48.0));
            }
            let c = lerp(c, sky.haze, 1.0 - (-z / sky.fog).exp());
            let (y0, y1) = (sy.max(0), ymin.min(VH));
            match below {
                Some((b, by)) if by > fy => for y in y0..y1 { col[y as usize] = lerp(c, b, (y as f32 - fy) / (by - fy)); },
                _ => for y in y0..y1 { col[y as usize] = c; },
            }
            for y in y0..y1 { dep[y as usize] = z; }
            ymin = sy;
            below = Some((c, fy));
        } else {
            below = None;
        }
        if ymin <= 0 { break; }
        z += dz;
        dz *= 1.005;
    }
    ymin
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Brief, Fly, Down, Won, Debrief, Over }

struct Raid {
    terrain: Terrain,
    models: Models,
    sites: Vec<(f32, f32)>,
    pads: Vec<(f32, f32)>,
    mode: Mode,
    mission: usize,
    /// The mission a new game begins with.
    first: usize,
    /// How many times all six have been flown.
    lap: u32,
    sky: usize,
    // The gunship: where it is, how it moves, how it leans.
    x: f32, y: f32, alt: f32, yaw: f32,
    spin: f32, speed: f32, throttle: f32, side: f32, climb: f32,
    /// The height over the ground it keeps.
    want: f32,
    pitch: f32, bank: f32,
    /// Seconds Down has been held at a hover.
    hold: f32,
    /// The land ahead rises faster than the gunship can climb.
    steep: bool,
    vel: [f32; 3],
    armour: f32, lives: u32, rounds: u32, rockets: u32, missiles: u32,
    gun_cool: f32, burst: u32, pod: bool, rocket_cool: f32, missile_cool: f32,
    /// Seconds until the ground can hurt again, and seconds over the pad.
    bump: f32, rearm: f32,
    /// The target in the box.
    lock: Option<usize>,
    /// The pad: where, and how high its top lies.
    pad: [f32; 3],
    foes: Vec<Foe>,
    shots: Vec<Shot>,
    fx: Vec<Fx>,
    /// Blasts that wait: seconds to go, where, how wide, how hard.
    blasts: Vec<(f32, [f32; 3], f32, f32)>,
    score: u32, high: u32, kills: u32,
    /// What a mission paid: for flying it, for the armour left, for speed.
    bonus: [u32; 3],
    /// Seconds into the mission, since the game began, and in this mode.
    clock: f32, time: f32, timer: f32,
    note: Option<(String, f32)>,
    /// 1 while a missile site is taking aim, 2 while a missile flies.
    warn: u8,
    beep: f32, flash: f32, shake: f32, hit: f32,
    crashed: bool, paused: bool,
    rng: Rng,
    audio: Audio,
    snd: Sounds,
    // Kept between frames so nothing is allocated while flying.
    dirs: Vec<(f32, f32)>, hzs: Vec<f32>, tops: Vec<i32>, cols: Vec<Rgb>, deps: Vec<f32>, over: Vec<f32>,
    grad: Vec<Rgb>,
    /// Stars: a heading, how high, how bright.
    stars: Vec<(f32, f32, f32)>,
    order: Vec<(f32, usize)>,
}

impl Raid {
    fn new() -> Raid {
        let terrain = Terrain::generate();
        let (sites, pads) = (terrain.sites.clone(), terrain.pads.clone());
        let mut rng = Rng::new(7);
        let stars = (0..320).map(|_| (rng.range(-PI, PI), rng.range(0.02, 0.9), rng.range(0.25, 1.0))).collect();
        let n = (W * VH) as usize;
        let mut game = Raid { terrain, models: Models::new(), sites, pads, mode: Mode::Title, mission: 0, first: 0, lap: 0, sky: DAY,
            x: 0.0, y: 0.0, alt: 0.0, yaw: 0.0, spin: 0.0, speed: 0.0, throttle: 0.0, side: 0.0, climb: 0.0, want: 8.0, pitch: 0.0, bank: 0.0,
            hold: 0.0, steep: false, vel: [0.0; 3], armour: 100.0, lives: 3, rounds: ROUNDS, rockets: ROCKETS, missiles: MISSILES, gun_cool: 0.0, burst: 0,
            pod: false, rocket_cool: 0.0, missile_cool: 0.0, bump: 0.0, rearm: 0.0, lock: None, pad: [0.0; 3], foes: Vec::new(),
            shots: Vec::new(), fx: Vec::new(), blasts: Vec::new(), score: 0, high: funkey::store::high_score(GAME), kills: 0, bonus: [0; 3],
            clock: 0.0, time: 0.0, timer: 0.0, note: None, warn: 0, beep: 0.0, flash: 0.0, shake: 0.0, hit: 0.0, crashed: false,
            paused: false, rng: Rng::from_time(), audio: Audio::off(), snd: Sounds::new(), dirs: vec![(0.0, 1.0); W as usize],
            hzs: vec![0.0; W as usize], tops: vec![VH; W as usize], cols: vec![0; n], deps: vec![f32::INFINITY; n],
            over: vec![f32::INFINITY; n], grad: Vec::new(), stars, order: Vec::new() };
        game.begin(0);
        game
    }

    /// Set a mission up: its light, who stands there, the gunship on its pad.
    fn begin(&mut self, n: usize) {
        self.mission = n;
        self.sky = MISSIONS[n].sky;
        let sky = &SKIES[self.sky];
        self.terrain.light(sky.tint);
        self.grad = (0..GRAD).map(|i| lerp(sky.haze, sky.zenith, (i as f32 / (GRAD - 1) as f32).powf(0.8))).collect();
        self.foes = muster(&self.terrain, n, self.sites[n]);
        for f in self.foes.iter_mut() { f.armour *= 1.0 + 0.25 * self.lap as f32; }
        let (px, py) = self.pads[n];
        let top = [(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0), (0.0, 0.0)].iter()
            .map(|&(dx, dy)| self.terrain.ground(px + dx, py + dy)).fold(f32::MIN, f32::max);
        self.pad = [px, py, top + 0.3];
        self.fx.clear();
        self.blasts.clear();
        self.clock = 0.0;
        self.kills = 0;
        self.spawn();
    }

    /// A fresh gunship on the pad, its nose to the targets.
    fn spawn(&mut self) {
        let site = self.sites[self.mission];
        self.x = self.pad[0];
        self.y = self.pad[1];
        self.alt = self.pad[2] + 5.0;
        self.yaw = wrap(site.0 - self.x).atan2(wrap(site.1 - self.y));
        (self.spin, self.speed, self.throttle, self.side, self.climb, self.pitch, self.bank, self.hold) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        self.want = 8.0;
        self.vel = [0.0; 3];
        self.armour = 100.0;
        (self.rounds, self.rockets, self.missiles) = (ROUNDS, ROCKETS, MISSILES);
        (self.gun_cool, self.burst, self.rocket_cool, self.missile_cool, self.bump, self.rearm) = (0.4, 0, 0.0, 0.0, 0.0, 0.0);
        self.lock = None;
        self.shots.clear();
        self.crashed = false;
        self.steep = false;
        self.warn = 0;
        for f in self.foes.iter_mut() { f.seen = 0.0; f.burst = 0; }
    }

    /// A new game.
    fn start(&mut self) {
        self.score = 0;
        self.lives = 3;
        self.lap = 0;
        self.begin(self.first);
        self.mode = Mode::Brief;
        self.timer = 0.0;
        self.paused = false;
    }

    /// Put the gunship so far from the targets, on the line to its pad.
    fn place(&mut self, far: f32) {
        let site = self.sites[self.mission];
        let (dx, dy) = (wrap(self.pad[0] - site.0), wrap(self.pad[1] - site.1));
        let l = dx.hypot(dy).max(1.0);
        self.x = (site.0 + dx / l * far).rem_euclid(WORLD);
        self.y = (site.1 + dy / l * far).rem_euclid(WORLD);
        self.alt = self.floor(self.x, self.y) + self.want;
    }

    fn me(&self) -> [f32; 3] { [self.x, self.y, self.alt] }

    /// The ground under the gunship: the land, the water, or the pad.
    fn floor(&self, x: f32, y: f32) -> f32 {
        let g = self.terrain.ground(x, y);
        if wrap(x - self.pad[0]).abs() < 8.5 && wrap(y - self.pad[1]).abs() < 8.5 { g.max(self.pad[2]) } else { g }
    }

    /// The row of the horizon straight ahead.
    fn level(&self) -> f32 { VH as f32 * LEVEL + self.pitch * FOCAL }

    fn eye(&self) -> Eye {
        let (sin, cos) = self.yaw.sin_cos();
        let jolt = (self.time * 61.0).sin() * self.shake * 6.0;
        Eye { x: self.x, y: self.y, alt: self.alt, yaw: self.yaw, sin, cos, hz: self.level() + jolt, tilt: -self.bank }
    }

    fn say(&mut self, s: &str) { self.note = Some((s.to_string(), 2.5)); }

    /// The click of a weapon with nothing to fire.
    fn dry(&mut self) { self.audio.play_on(4, &self.snd.tick, 0.6); }

    /// The title's slow flight over the land.
    fn drift(&mut self, dt: f32) {
        self.yaw = turn(self.yaw + ((self.time * 0.11).sin() * 0.3 + (self.time * 0.037).sin() * 0.2) * dt);
        let (s, c) = self.yaw.sin_cos();
        self.x = (self.x + s * 24.0 * dt).rem_euclid(WORLD);
        self.y = (self.y + c * 24.0 * dt).rem_euclid(WORLD);
        let here = self.terrain.ground(self.x, self.y);
        let high = here.max(self.terrain.ground(self.x + s * 40.0, self.y + c * 40.0));
        self.alt = (self.alt + (high + 42.0 - self.alt) * dt * 0.9).max(here + 5.0);
        self.pitch += (-0.05 - self.pitch) * dt;
        self.bank = 0.0;
    }

    /// A frame of flying.
    fn fly(&mut self, input: &Input, dt: f32) {
        self.clock += dt;
        let steer = (input.held(Key::Right) as i32 - input.held(Key::Left) as i32) as f32;
        self.spin += (steer * 1.15 - self.spin) * (dt * 6.0).min(1.0);
        self.yaw = turn(self.yaw + self.spin * dt);
        if input.held(Key::Up) {
            self.throttle = (self.throttle + 30.0 * dt).min(TOP);
            self.hold = 0.0;
        } else if input.held(Key::Down) {
            // Down slows to a hover and stops there; held on, it backs up.
            if self.throttle > 0.0 {
                self.throttle = (self.throttle - 40.0 * dt).max(0.0);
                self.hold = 0.0;
            } else {
                self.hold += dt;
                if self.hold > 0.4 { self.throttle = (self.throttle - 20.0 * dt).max(-14.0); }
            }
        } else {
            self.hold = 0.0;
        }
        let slide = (input.held(Key::Char('d')) as i32 - input.held(Key::Char('a')) as i32) as f32;
        self.side += (slide * 16.0 - self.side) * (dt * 3.0).min(1.0);
        let lift = (input.held(Key::Char('w')) as i32 - input.held(Key::Char('s')) as i32) as f32;
        self.want = (self.want + lift * 30.0 * dt).clamp(4.0, 120.0);
        self.speed += (self.throttle - self.speed) * (dt * 1.5).min(1.0);
        let (s, c) = self.yaw.sin_cos();
        let (vx, vy) = (s * self.speed + c * self.side, c * self.speed - s * self.side);
        self.x = (self.x + vx * dt).rem_euclid(WORLD);
        self.y = (self.y + vy * dt).rem_euclid(WORLD);
        // Keep the height over the ground, and look ahead for what rises.
        let here = self.floor(self.x, self.y);
        let mut rate = (here + self.want - self.alt) * 2.2;
        for k in [0.3, 0.6, 1.0, 1.5, 2.0] {
            let rise = (self.floor(self.x + vx * k, self.y + vy * k) + self.want - self.alt) / k;
            if rise > rate { rate = rise; }
        }
        self.steep = rate > CLIMB * 1.25 && self.speed > 25.0;
        self.climb += (rate.clamp(-24.0, CLIMB) - self.climb) * (dt * 5.0).min(1.0);
        self.alt += self.climb * dt;
        self.vel = [vx, vy, self.climb];
        self.bump = (self.bump - dt).max(0.0);
        if self.alt < here + 2.2 {
            self.alt = here + 2.2;
            self.climb = self.climb.max(0.0);
            if self.bump == 0.0 && self.speed.abs() > 14.0 {
                self.bump = 0.6;
                let harm = 3.0 + self.speed.abs() * 0.1;
                self.speed *= 0.5;
                self.throttle *= 0.5;
                self.say("TERRAIN");
                self.hurt(harm);
            }
        }
        // The nose dips with speed, the gunship leans into a turn.
        let nose = -(self.speed / TOP) * 0.11 - (self.throttle - self.speed) * 0.0022 + self.climb * 0.0012;
        self.pitch += (nose - self.pitch) * (dt * 4.0).min(1.0);
        self.bank += (self.spin * 0.11 + self.side * 0.006 - self.bank) * (dt * 5.0).min(1.0);
        self.audio.rate(1, 0.94 + self.speed.abs() / TOP * 0.12 + self.climb.max(0.0) / CLIMB * 0.08);
        if self.mode == Mode::Fly {
            self.seek();
            self.shoot(input, dt);
            self.service(dt);
        }
        self.world(dt);
        if self.mode == Mode::Won {
            self.timer += dt;
            if self.timer > 3.0 { self.debrief(); }
        }
    }

    /// Box the target nearest the gunsight that can be seen.
    fn seek(&mut self) {
        let me = self.me();
        let mut best: Option<(usize, f32)> = None;
        for (i, f) in self.foes.iter().enumerate() {
            if f.dead { continue; }
            let p = f.mid();
            let (dx, dy) = (wrap(p[0] - me[0]), wrap(p[1] - me[1]));
            let far = dx.hypot(dy);
            if !(4.0..REACH).contains(&far) { continue; }
            let off = turn(dx.atan2(dy) - self.yaw).abs();
            if off > 0.3 { continue; }
            // The one in the box keeps it against a near rival.
            let rank = off + far / 4000.0 - if self.lock == Some(i) { 0.05 } else { 0.0 };
            if best.is_none_or(|b| rank < b.1) && self.terrain.sees(me, p) { best = Some((i, rank)); }
        }
        self.lock = best.map(|b| b.0);
    }

    /// The way a shot leaves: at the boxed target when it lies within
    /// `cone` of the nose, or else straight along the gunsight.
    fn aim(&self, from: [f32; 3], speed: f32, cone: f32) -> [f32; 3] {
        if let Some(i) = self.lock {
            let f = &self.foes[i];
            let mut p = f.mid();
            let t = apart(from, p) / speed;
            p[0] += f.vx * t;
            p[1] += f.vy * t;
            let d = toward(from, p);
            if turn(d[0].atan2(d[1]) - self.yaw).abs() < cone { return d; }
        }
        let rise = (self.level() - SIGHT as f32) / FOCAL;
        let (s, c) = self.yaw.sin_cos();
        let l = (1.0 + rise * rise).sqrt();
        [s / l, c / l, rise / l]
    }

    fn shoot(&mut self, input: &Input, dt: f32) {
        self.gun_cool = (self.gun_cool - dt).max(0.0);
        self.rocket_cool = (self.rocket_cool - dt).max(0.0);
        self.missile_cool = (self.missile_cool - dt).max(0.0);
        // A tap is a burst of four; held, the gun keeps firing.
        if input.pressed(Key::Space) { self.burst = self.burst.max(4); }
        if input.held(Key::Space) { self.burst = self.burst.max(1); }
        let (s, c) = self.yaw.sin_cos();
        let me = self.me();
        if self.burst > 0 && self.gun_cool == 0.0 {
            self.burst -= 1;
            if self.rounds == 0 {
                self.burst = 0;
                self.gun_cool = 0.3;
                self.dry();
            } else {
                self.rounds -= 1;
                self.gun_cool = 0.085;
                let from = [me[0] + s * 2.0, me[1] + c * 2.0, me[2] - 1.3];
                let mut d = self.aim(from, 430.0, 0.45);
                for k in d.iter_mut() { *k += self.rng.range(-0.007, 0.007); }
                self.shots.push(Shot::new(Ammo::Round, from, d, 430.0, 1.3, true, None));
                self.audio.play_on(3, &self.snd.gun, 0.7);
            }
        }
        // Rockets and missiles leave from a pod on either side, in turn.
        let off = if self.pod { 1.9 } else { -1.9 };
        let from = [me[0] + c * off + s * 1.5, me[1] - s * off + c * 1.5, me[2] - 1.0];
        if input.held(Key::Char('f')) && self.rocket_cool == 0.0 {
            self.rocket_cool = 0.28;
            if self.rockets == 0 { self.dry(); } else {
                self.rockets -= 1;
                self.pod = !self.pod;
                let d = self.aim(from, 230.0, 0.2);
                self.shots.push(Shot::new(Ammo::Rocket, from, d, 230.0, 3.0, true, None));
                self.audio.play(&self.snd.rocket, 0.7);
            }
        }
        if input.pressed(Key::Char('e')) && self.missile_cool == 0.0 {
            self.missile_cool = 0.6;
            match self.lock {
                Some(i) if self.missiles > 0 => {
                    self.missiles -= 1;
                    self.pod = !self.pod;
                    self.shots.push(Shot::new(Ammo::Missile, from, [s, c, 0.02], 70.0, 9.0, true, Some(i)));
                    self.audio.play(&self.snd.missile, 0.8);
                }
                Some(_) => self.dry(),
                None => { self.say("NO TARGET"); self.dry(); }
            }
        }
    }

    /// Hovering low over the pad mends the armour and fills the pods.
    fn service(&mut self, dt: f32) {
        let over = wrap(self.x - self.pad[0]).hypot(wrap(self.y - self.pad[1])) < 12.0 && self.alt - self.pad[2] < 10.0 && self.speed.abs() < 8.0;
        let short = self.armour < 100.0 || self.rounds < ROUNDS || self.rockets < ROCKETS || self.missiles < MISSILES;
        if !(over && short) { self.rearm = 0.0; return; }
        self.rearm += dt;
        self.armour = (self.armour + 30.0 * dt).min(100.0);
        if self.rearm > 1.5 {
            (self.rounds, self.rockets, self.missiles) = (ROUNDS, ROCKETS, MISSILES);
            if self.armour >= 100.0 { self.say("REARMED"); }
        }
    }

    /// Everything that moves besides the gunship.
    fn world(&mut self, dt: f32) {
        // Blasts that waited their turn: fuel tanks go off one by one.
        let mut due = Vec::new();
        self.blasts.retain_mut(|b| { b.0 -= dt; if b.0 <= 0.0 { due.push((b.1, b.2, b.3)); false } else { true } });
        for (at, reach, harm) in due { self.blast(at, reach, harm); }
        self.foes_act(dt);
        self.shots_fly(dt);
        for p in self.fx.iter_mut() {
            p.age += dt;
            for k in 0..3 { p.at[k] += p.vel[k] * dt; }
            p.size += p.grow * dt;
            if p.puff == Puff::Spark { p.vel[2] -= 30.0 * dt; }
        }
        self.fx.retain(|p| p.age < p.life);
        if self.fx.len() > 700 { let cut = self.fx.len() - 700; self.fx.drain(..cut); }
        self.flash = (self.flash - dt * 2.5).max(0.0);
        self.shake = (self.shake - dt * 2.0).max(0.0);
        self.hit = (self.hit - dt).max(0.0);
        if let Some(n) = &mut self.note { n.1 -= dt; }
        if self.note.as_ref().is_some_and(|n| n.1 <= 0.0) { self.note = None; }
        self.beep = (self.beep - dt).max(0.0);
        if self.warn > 0 && self.beep == 0.0 && self.mode == Mode::Fly {
            if self.warn == 2 { self.audio.play_on(4, &self.snd.launch, 0.8); self.beep = 0.16; }
            else { self.audio.play_on(4, &self.snd.lock, 0.7); self.beep = 0.5; }
        }
    }

    /// What every one of theirs does in a frame: move, look, take aim, fire.
    fn foes_act(&mut self, dt: f32) {
        let me = self.me();
        let (vel, peace) = (self.vel, self.mode != Mode::Fly);
        let low = self.alt - self.terrain.ground(self.x, self.y) < 10.0;
        let radar = self.foes.iter().any(|f| f.kind == Kind::Radar && !f.dead);
        let quick = 0.85f32.powi(self.lap as i32);
        let (mut shots, mut puffs, mut crashes): (Vec<Shot>, Vec<Fx>, Vec<[f32; 3]>) = (Vec::new(), Vec::new(), Vec::new());
        let (mut nearest, mut locking) = (f32::MAX, false);
        let (terrain, rng) = (&self.terrain, &mut self.rng);
        for f in self.foes.iter_mut() {
            if f.dead {
                if f.fall > 0.0 {
                    // A gunship shot down spins to the ground.
                    f.fall += 28.0 * dt;
                    f.h -= f.fall * dt;
                    f.heading += 5.0 * dt;
                    f.x = (f.x + f.vx * dt).rem_euclid(WORLD);
                    f.y = (f.y + f.vy * dt).rem_euclid(WORLD);
                    let g = terrain.ground(f.x, f.y) + 1.6;
                    if f.h <= g {
                        f.h = g;
                        (f.fall, f.vx, f.vy, f.smoke) = (-1.0, 0.0, 0.0, 30.0);
                        crashes.push([f.x, f.y, f.h]);
                    }
                }
                // A boat settles in the water.
                if f.kind == Kind::Boat { f.h = (f.h - 0.35 * dt).max(WATER * HSCALE - 1.5); }
                if f.smoke > 0.0 || f.fall > 0.0 {
                    f.smoke -= dt;
                    f.gap -= dt;
                    if f.gap <= 0.0 {
                        f.gap = 0.16;
                        let p = [f.x + rng.range(-1.0, 1.0), f.y + rng.range(-1.0, 1.0), f.h + f.kind.size().1 + 0.5];
                        if f.smoke > 22.0 { puffs.push(Fx { puff: Puff::Fire, at: p, vel: [0.0, 0.0, 3.0], age: 0.0, life: 0.5, size: 1.4, grow: 1.0 }); }
                        puffs.push(Fx { puff: Puff::Smoke, at: p, vel: [1.5, 0.8, rng.range(4.0, 7.0)], age: 0.0, life: rng.range(2.5, 4.5), size: 1.2,
                            grow: 1.6 });
                    }
                }
                continue;
            }
            let (dx, dy) = (wrap(me[0] - f.x), wrap(me[1] - f.y));
            let (flat, bearing) = (dx.hypot(dy), dx.atan2(dy));
            match f.kind {
                Kind::Boat if f.orbit > 0.0 => {
                    f.phase += 6.0 / f.orbit * dt;
                    let (s, c) = f.phase.sin_cos();
                    f.x = (f.cx + s * f.orbit).rem_euclid(WORLD);
                    f.y = (f.cy + c * f.orbit).rem_euclid(WORLD);
                    f.heading = f.phase + PI / 2.0;
                    (f.vx, f.vy) = (c * 6.0, -s * 6.0);
                    if rng.chance(dt * 7.0) {
                        puffs.push(Fx { puff: Puff::Foam, at: [f.x - c * 4.5, f.y + s * 4.5, f.h + 0.2], vel: [0.0; 3], age: 0.0, life: 1.6, size: 0.9,
                            grow: 0.9 });
                    }
                }
                Kind::Heli => {
                    // Come at the gunship, break off when close, come again.
                    let goal = if f.orbit > 0.0 { f.orbit -= dt; bearing + 2.5 } else { if flat < 75.0 { f.orbit = 3.5; } bearing };
                    f.heading = turn(f.heading + turn(goal - f.heading).clamp(-0.9 * dt, 0.9 * dt));
                    let (s, c) = f.heading.sin_cos();
                    (f.vx, f.vy) = (s * 30.0, c * 30.0);
                    f.x = (f.x + f.vx * dt).rem_euclid(WORLD);
                    f.y = (f.y + f.vy * dt).rem_euclid(WORLD);
                    let g = terrain.ground(f.x, f.y).max(terrain.ground(f.x + f.vx, f.y + f.vy));
                    f.h += (g + f.agl - f.h).clamp(-18.0 * dt, 24.0 * dt);
                    f.aim += 24.0 * dt;
                }
                Kind::Radar => f.aim += 1.5 * dt,
                _ => {}
            }
            let range = f.kind.range();
            if range == 0.0 { continue; }
            let m = f.mid();
            let gun = [m[0], m[1], m[2] + 0.9];
            // Look for the gunship a few times a second; hills hide it,
            // and a missile site cannot find it close to the ground.
            f.look -= dt;
            if f.look <= 0.0 {
                f.look = 0.2 + rng.float() * 0.1;
                f.los = flat < range * 1.2 && terrain.sees(gun, me) && !(f.kind == Kind::Sam && low && flat > 150.0);
            }
            if f.los && !peace { f.seen += dt; } else { f.seen = (f.seen - dt * 2.0).max(0.0); }
            if f.kind != Kind::Heli && f.los { f.aim = turn(f.aim + turn(bearing - f.aim).clamp(-1.8 * dt, 1.8 * dt)); }
            f.cool = (f.cool - dt).max(0.0);
            let far = apart(gun, me);
            let facing = turn(bearing - if f.kind == Kind::Heli { f.heading } else { f.aim }).abs() < 0.22;
            let ready = f.los && !peace && far < range && facing;
            // Where the gunship will be when a shot at this speed gets there.
            let ahead = |speed: f32| { let t = far / speed; [me[0] + vel[0] * t, me[1] + vel[1] * t, me[2] + vel[2] * t] };
            match f.kind {
                Kind::Flak | Kind::Boat | Kind::Heli => {
                    if f.burst > 0 {
                        f.gap -= dt;
                        if f.gap <= 0.0 {
                            f.gap = 0.09;
                            f.burst -= 1;
                            let mut d = toward(gun, ahead(170.0));
                            for k in d.iter_mut() { *k += rng.range(-0.04, 0.04); }
                            shots.push(Shot::new(Ammo::Tracer, gun, d, 170.0, 2.0, false, None));
                            nearest = nearest.min(far);
                        }
                    } else if ready && f.cool == 0.0 && f.seen > 0.8 {
                        f.burst = if f.kind == Kind::Flak { 5 } else { 3 };
                        f.cool = if f.kind == Kind::Flak { 1.9 } else { 2.5 } * quick;
                        f.gap = 0.0;
                    }
                }
                Kind::Tank => if ready && f.cool == 0.0 && f.seen > 1.2 {
                    f.cool = 3.6 * quick;
                    let mut d = toward(gun, ahead(150.0));
                    for k in d.iter_mut() { *k += rng.range(-0.016, 0.016); }
                    shots.push(Shot::new(Ammo::Shell, gun, d, 150.0, 3.0, false, None));
                    puffs.push(Fx { puff: Puff::Fire, at: [gun[0] + d[0] * 4.0, gun[1] + d[1] * 4.0, gun[2]], vel: [0.0; 3], age: 0.0, life: 0.15, size: 1.2,
                        grow: 4.0 });
                    nearest = nearest.min(far);
                },
                Kind::Sam => {
                    if f.los && !peace && f.seen > 0.3 { locking = true; }
                    // With their radar up the launchers aim in half the time.
                    let need = if radar { 1.8 } else { 3.4 };
                    if f.los && !peace && f.cool == 0.0 && f.seen > need && far < range && far > 70.0 {
                        f.cool = 9.0 * quick;
                        f.seen = 0.0;
                        let d = toward(gun, me);
                        shots.push(Shot::new(Ammo::Sam, gun, [d[0] * 0.7, d[1] * 0.7, d[2] * 0.7 + 0.7], 55.0, 10.0, false, None));
                        nearest = nearest.min(far * 0.5);
                    }
                }
                _ => {}
            }
        }
        self.shots.extend(shots);
        self.fx.extend(puffs);
        for at in crashes { self.burst_at(at, 9.0); }
        if nearest < 500.0 { self.audio.play_on(5, &self.snd.thud, (1.0 - nearest / 500.0).max(0.08)); }
        let flying = self.shots.iter().any(|s| s.ammo == Ammo::Sam);
        self.warn = if peace { 0 } else if flying { 2 } else if locking { 1 } else { 0 };
    }

    fn shots_fly(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|s| self.shot(s, dt));
        shots.append(&mut self.shots);
        self.shots = shots;
    }

    /// Move one shot a frame on. False when it is spent.
    fn shot(&mut self, s: &mut Shot, dt: f32) -> bool {
        let from = s.at;
        let body = [self.x, self.y, self.alt - 0.8];
        let coast = [s.at[0] + s.vel[0], s.at[1] + s.vel[1], s.at[2] + s.vel[2]];
        match s.ammo {
            Ammo::Missile => {
                let v = (s.speed() + 260.0 * dt).min(300.0);
                match s.target.filter(|&t| !self.foes[t].dead) {
                    Some(t) => s.steer(self.foes[t].mid(), 2.8 * dt, v),
                    None => s.steer(coast, 0.0, v),
                }
            }
            Ammo::Sam => {
                // It climbs away first, then turns after the gunship for
                // as long as it can see it.
                let v = (s.speed() + 70.0 * dt).min(120.0 + 12.0 * self.lap as f32);
                if self.mode == Mode::Fly && s.life < 9.4 && self.terrain.sees(s.at, body) { s.steer(body, dt, v); } else { s.steer(coast, 0.0, v); }
            }
            _ => {}
        }
        if matches!(s.ammo, Ammo::Rocket | Ammo::Missile | Ammo::Sam) {
            self.fx.push(Fx { puff: Puff::Trail, at: from, vel: [0.0, 0.0, 0.6], age: 0.0, life: if s.ammo == Ammo::Rocket { 0.6 } else { 1.3 }, size: 0.5,
                grow: 1.3 });
        }
        for k in 0..3 { s.at[k] += s.vel[k] * dt; }
        s.at[0] = s.at[0].rem_euclid(WORLD);
        s.at[1] = s.at[1].rem_euclid(WORLD);
        s.life -= dt;
        if s.life <= 0.0 {
            if matches!(s.ammo, Ammo::Rocket | Ammo::Missile | Ammo::Sam) { self.burst_at(s.at, 4.0); }
            return false;
        }
        if s.mine {
            let hit = self.foes.iter().position(|f| !f.dead && pass(from, s.at, f.mid()) < f.kind.size().0 * 0.5 + 0.9);
            if let Some(j) = hit {
                match s.ammo {
                    Ammo::Round => { self.sparks(s.at, 3); self.damage(j, 7.0); }
                    Ammo::Rocket => self.blast(s.at, 9.0, 50.0),
                    _ => self.blast(s.at, 11.0, 160.0),
                }
                return false;
            }
        } else if self.mode == Mode::Fly && pass(from, s.at, body) < if s.ammo == Ammo::Sam { 6.0 } else { 2.6 } {
            match s.ammo {
                Ammo::Sam => { self.burst_at(s.at, 8.0); self.hurt(34.0); }
                Ammo::Shell => { self.sparks(s.at, 8); self.hurt(15.0); }
                _ => self.hurt(3.0),
            }
            return false;
        }
        let ground = self.terrain.ground(s.at[0], s.at[1]);
        if s.at[2] > ground { return true; }
        let p = [s.at[0], s.at[1], ground + 0.3];
        match s.ammo {
            Ammo::Round | Ammo::Tracer => self.dust(p, 1),
            Ammo::Shell => self.dust(p, 5),
            Ammo::Rocket => self.blast(p, 9.0, 50.0),
            Ammo::Missile => self.blast(p, 11.0, 160.0),
            Ammo::Sam => self.burst_at(p, 7.0),
        }
        false
    }

    /// Fire, smoke and sparks, and the bang.
    fn burst_at(&mut self, at: [f32; 3], size: f32) {
        let n = (size * 1.5) as u32 + 5;
        for _ in 0..n {
            let v = [self.rng.range(-0.5, 0.5) * size, self.rng.range(-0.5, 0.5) * size, self.rng.range(0.1, 0.6) * size];
            let p = [at[0] + v[0] * 0.3, at[1] + v[1] * 0.3, at[2] + v[2] * 0.3];
            self.fx.push(Fx { puff: Puff::Fire, at: p, vel: v, age: 0.0, life: self.rng.range(0.45, 1.0), size: size * self.rng.range(0.2, 0.42),
                grow: size * 0.45 });
        }
        for _ in 0..n / 2 {
            let v = [self.rng.range(-1.5, 1.5), self.rng.range(-1.5, 1.5), self.rng.range(3.0, 7.0)];
            self.fx.push(Fx { puff: Puff::Smoke, at, vel: v, age: 0.0, life: self.rng.range(2.5, 5.0), size: size * 0.35, grow: 1.8 });
        }
        self.sparks(at, n);
        let far = apart(at, self.me());
        self.audio.play(&self.snd.boom, ((1.0 - far / 700.0).max(0.08) * (0.5 + size / 16.0)).min(1.0));
        let weight = (size / 12.0).min(1.0);
        self.flash = self.flash.max((1.0 - far / 150.0).max(0.0) * weight * 0.8);
        self.shake = self.shake.max((1.0 - far / 100.0).max(0.0) * weight);
    }

    fn sparks(&mut self, at: [f32; 3], n: u32) {
        for _ in 0..n {
            let v = [self.rng.range(-14.0, 14.0), self.rng.range(-14.0, 14.0), self.rng.range(2.0, 22.0)];
            self.fx.push(Fx { puff: Puff::Spark, at, vel: v, age: 0.0, life: self.rng.range(0.4, 1.2), size: 0.22, grow: 0.0 });
        }
    }

    /// What a shot throws up from the ground, or from the water.
    fn dust(&mut self, at: [f32; 3], n: u32) {
        let puff = if self.terrain.land(at[0], at[1]) { Puff::Dust } else { Puff::Foam };
        for _ in 0..n {
            let v = [self.rng.range(-1.5, 1.5), self.rng.range(-1.5, 1.5), self.rng.range(1.5, 5.0)];
            self.fx.push(Fx { puff, at, vel: v, age: 0.0, life: self.rng.range(0.5, 1.1), size: 0.6, grow: 2.2 });
        }
    }

    /// A blast: it harms all of theirs within reach, less the farther off.
    fn blast(&mut self, at: [f32; 3], reach: f32, harm: f32) {
        self.burst_at(at, reach * 0.8);
        for j in 0..self.foes.len() {
            let f = &self.foes[j];
            if f.dead { continue; }
            let far = (apart(at, f.mid()) - f.kind.size().0 * 0.5).max(0.0);
            if far < reach { self.damage(j, harm * (1.0 - far / reach)); }
        }
    }

    fn damage(&mut self, j: usize, harm: f32) {
        if self.foes[j].dead { return; }
        self.foes[j].armour -= harm;
        if self.foes[j].armour <= 0.0 { self.kill(j); }
    }

    fn kill(&mut self, j: usize) {
        let (kind, mid) = (self.foes[j].kind, self.foes[j].mid());
        let f = &mut self.foes[j];
        (f.dead, f.burst, f.gap, f.los, f.seen) = (true, 0, 0.0, false, 0.0);
        if kind == Kind::Heli { f.fall = 1.0; } else { f.smoke = 30.0; }
        self.score += kind.score() * (2 + self.lap) / 2;
        self.high = self.high.max(self.score);
        self.kills += 1;
        if self.lock == Some(j) { self.lock = None; }
        self.burst_at(mid, match kind { Kind::Fuel => 13.0, Kind::Bunker | Kind::Hangar => 15.0, Kind::Heli => 5.0, Kind::Truck => 6.0, _ => 8.0 });
        // Burning fuel sets off the tank beside it.
        if kind == Kind::Fuel { self.blasts.push((0.25, mid, 20.0, 80.0)); }
        self.say(&format!("{} DESTROYED", kind.names().0.to_uppercase()));
        if self.mode == Mode::Fly && self.foes.iter().all(|f| !f.primary || f.dead) {
            self.mode = Mode::Won;
            self.timer = 0.0;
            self.warn = 0;
            self.say("MISSION COMPLETE");
            self.audio.play_on(7, &self.snd.win, 0.8);
        }
    }

    /// The gunship is hit.
    fn hurt(&mut self, harm: f32) {
        if self.mode != Mode::Fly { return; }
        self.armour -= harm;
        self.hit = 0.3;
        self.shake = self.shake.max(0.4 + harm / 40.0);
        self.audio.play_on(6, &self.snd.clank, 0.8);
        if self.armour <= 0.0 {
            self.armour = 0.0;
            self.mode = Mode::Down;
            self.timer = 0.0;
            self.crashed = false;
            self.lock = None;
            self.warn = 0;
            self.climb = self.climb.min(0.0);
            self.say("GOING DOWN");
        }
    }

    /// Shot down: the gunship spins to the ground, and the next one
    /// lifts from the pad if there is one.
    fn fall(&mut self, dt: f32) {
        self.timer += dt;
        self.spin += (4.0 - self.spin) * dt;
        self.yaw = turn(self.yaw + self.spin * dt);
        self.bank += (0.25 - self.bank) * dt;
        if !self.crashed {
            self.timer = self.timer.min(1.9);
            self.x = (self.x + self.vel[0] * dt).rem_euclid(WORLD);
            self.y = (self.y + self.vel[1] * dt).rem_euclid(WORLD);
            self.climb -= 20.0 * dt;
            self.alt += self.climb * dt;
            let floor = self.terrain.ground(self.x, self.y) + 2.0;
            if self.alt <= floor {
                self.alt = floor;
                self.crashed = true;
                self.timer = 2.0;
                self.spin = 0.0;
                self.vel = [0.0; 3];
                let (s, c) = self.yaw.sin_cos();
                self.burst_at([self.x + s * 7.0, self.y + c * 7.0, self.alt], 14.0);
                self.flash = 1.0;
                self.audio.stop(1);
            }
        }
        self.world(dt);
        if self.timer < 3.8 { return; }
        if self.lives > 1 {
            self.lives -= 1;
            self.spawn();
            self.mode = Mode::Fly;
            self.audio.play_loop(1, &self.snd.rotor, 0.5);
            let left = self.lives;
            self.say(&if left == 1 { "LAST GUNSHIP".to_string() } else { format!("{} GUNSHIPS LEFT", left) });
        } else {
            self.lives = 0;
            self.mode = Mode::Over;
            self.timer = 0.0;
            if !cfg!(test) { funkey::store::record_score(GAME, self.score); }
            self.audio.play_on(7, &self.snd.lose, 0.8);
        }
    }

    /// The mission is flown: count what it paid.
    fn debrief(&mut self) {
        self.bonus = [1000 * (1 + self.lap), self.armour as u32 * 10, (240.0 - self.clock).max(0.0) as u32 * 10];
        self.score += self.bonus.iter().sum::<u32>();
        self.high = self.high.max(self.score);
        self.mode = Mode::Debrief;
        self.timer = 0.0;
        self.shots.clear();
    }

    /// On to the next mission; after the last, the first again, harder.
    fn next(&mut self) {
        let mut n = self.mission + 1;
        if n == MISSIONS.len() { n = 0; self.lap += 1; }
        self.begin(n);
        self.mode = Mode::Brief;
        self.timer = 0.0;
    }
}

impl Raid {
    /// The view out of the cockpit: sky, land, and what stands on it.
    fn scene(&mut self, f: &mut Frame, eye: &Eye) {
        let sky = &SKIES[self.sky];
        let (w, vh) = (W as usize, VH as usize);
        for x in 0..w {
            let k = x as f32 + 0.5 - W as f32 / 2.0;
            let ang = eye.yaw + (k / FOCAL).atan();
            self.dirs[x] = (ang.sin(), ang.cos());
            self.hzs[x] = eye.hz + k * eye.tilt;
        }
        // The sky: its colours, the stars at night, the clouds, the sun.
        for y in 0..vh {
            for (x, p) in f.px[y * w..(y + 1) * w].iter_mut().enumerate() {
                let t = ((self.hzs[x] - y as f32) / (VH as f32 * 0.9)).clamp(0.0, 1.0);
                *p = self.grad[(t * (GRAD - 1) as f32) as usize];
            }
        }
        if sky.night {
            for &(az, rise, bright) in &self.stars {
                let rel = turn(az - eye.yaw);
                if rel.abs() > 0.9 { continue; }
                let sx = W as f32 / 2.0 + rel.tan() * FOCAL;
                let sy = eye.horizon(sx) - rise * FOCAL / rel.cos();
                if sx >= 0.0 && sx < W as f32 && sy >= 0.0 && sy < VH as f32 {
                    let i = sy as usize * w + sx as usize;
                    f.px[i] = lerp(f.px[i], 0xe8ecff, bright);
                }
            }
        }
        // The clouds lie on a flat layer far above.
        let scroll = self.time * 24.0;
        let cover = if self.sky == MIST { 0.42 } else { 0.5 };
        for y in 0..self.hzs[0].max(self.hzs[w - 1]).clamp(0.0, VH as f32) as usize {
            for x in 0..w {
                let rise = (self.hzs[x] - y as f32) / FOCAL;
                if rise < 0.02 { continue; }
                let d = 900.0 / rise;
                if d > 12000.0 { continue; }
                let (sa, ca) = self.dirs[x];
                let n = tex(&self.terrain.clouds, CLOUD, (eye.x + sa * d + scroll) / 1900.0 * CLOUD as f32, (eye.y + ca * d) / 1900.0 * CLOUD as f32);
                let c = ((n - cover) * 3.2).clamp(0.0, 1.0) * (1.0 - d / 12000.0);
                if c > 0.0 { f.px[y * w + x] = lerp(f.px[y * w + x], sky.cloud, c * 0.85); }
            }
        }
        let rel = turn(SUN_AZ - eye.yaw);
        if rel.abs() < 1.2 {
            let sx = W as f32 / 2.0 + rel.tan() * FOCAL;
            let (sx, sy) = (sx as i32, (eye.horizon(sx) - sky.rise * FOCAL) as i32);
            let (disc, halo) = if sky.night { (8.0, 22.0) } else { (11.0, 50.0) };
            let r = (disc + halo) as i32;
            for y in (sy - r).max(0)..(sy + r).min(VH) {
                for x in (sx - r).max(0)..(sx + r).min(W) {
                    let d = (((x - sx) * (x - sx) + (y - sy) * (y - sy)) as f32).sqrt();
                    let glow = if d < disc { 1.0 } else { (1.0 - (d - disc) / halo).clamp(0.0, 1.0).powi(3) * 0.7 };
                    if glow > 0.0 { let i = (y * W + x) as usize; f.px[i] = lerp(f.px[i], sky.sun, glow); }
                }
            }
        }
        // The land: strips of columns over the cores, each core taking
        // the next strip when it is done.
        let mut cols = std::mem::take(&mut self.cols);
        let mut deps = std::mem::take(&mut self.deps);
        let mut tops = std::mem::take(&mut self.tops);
        let strip = 8;
        let next = std::sync::Mutex::new(cols.chunks_mut(vh * strip).zip(deps.chunks_mut(vh * strip)).zip(tops.chunks_mut(strip)).enumerate());
        let (terrain, dirs, time) = (&self.terrain, &self.dirs, self.time);
        std::thread::scope(|s| {
            for _ in 0..cores() {
                s.spawn(|| loop {
                    let job = next.lock().unwrap().next();
                    let Some((i, ((col, dep), top))) = job else { break };
                    for (k, t) in top.iter_mut().enumerate() {
                        let x = i * strip + k;
                        *t = column(terrain, sky, time, dirs[x], x, &mut col[k * vh..(k + 1) * vh], &mut dep[k * vh..(k + 1) * vh], eye);
                    }
                });
            }
        });
        drop(next);
        for y in 0..vh {
            for x in 0..w {
                if y as i32 >= tops[x] { f.px[y * w + x] = cols[x * vh + y]; }
            }
        }
        // What stands on it, flies over it and burns on it.
        let mut over = std::mem::take(&mut self.over);
        over.fill(f32::INFINITY);
        let mut order = std::mem::take(&mut self.order);
        {
            let sun = V3::new(SUN.x * eye.cos - SUN.z * eye.sin, SUN.y, SUN.x * eye.sin + SUN.z * eye.cos);
            let mut paint = Paint { f: &mut *f, land: &deps, over: &mut over, eye: *eye, sky, sun, v: Vec::new() };
            paint.mesh(&self.models.pad, self.pad, 0.0, 1.0);
            for foe in &self.foes {
                let (k, dark) = (foe.kind as usize, if foe.dead { 0.26 } else { 1.0 });
                paint.mesh(&self.models.body[k], [foe.x, foe.y, foe.h], foe.heading, dark);
                // A wreck on the ground has lost its rotor.
                if foe.dead && foe.kind == Kind::Heli && foe.fall < 0.0 { continue; }
                let (s, c) = foe.heading.sin_cos();
                let ahead = pivot(foe.kind);
                paint.mesh(&self.models.top[k], [foe.x + s * ahead, foe.y + c * ahead, foe.h], foe.aim, dark);
            }
            for s in &self.shots {
                let back = |k: f32| [s.at[0] - s.vel[0] * k, s.at[1] - s.vel[1] * k, s.at[2] - s.vel[2] * k];
                match s.ammo {
                    Ammo::Round => paint.streak(back(0.02), s.at, 0xfff2a0),
                    Ammo::Tracer => { paint.streak(back(0.04), s.at, 0xff5a2a); paint.ball(s.at, 0.25, 0xffd0a0, 1.0); }
                    Ammo::Shell => paint.ball(s.at, 0.4, 0xffd890, 1.0),
                    _ => {
                        paint.ball(back(0.006), 0.6, 0xffb040, 0.9);
                        paint.ball(s.at, 0.4, if s.ammo == Ammo::Sam { 0xf0f0f0 } else { 0xa0a498 }, 1.0);
                    }
                }
            }
            // Smoke and fire from the far ones to the near ones.
            order.clear();
            order.extend(self.fx.iter().enumerate().map(|(i, p)| (eye.cam(p.at).z, i)));
            order.sort_by(|a, b| b.0.total_cmp(&a.0));
            for &(_, i) in &order {
                let p = &self.fx[i];
                let t = p.age / p.life;
                let (c, a) = match p.puff {
                    Puff::Fire => (if t < 0.3 { lerp(0xfff6c0, 0xffb030, t / 0.3) } else if t < 0.7 { lerp(0xffb030, 0xd04010, (t - 0.3) / 0.4) }
                        else { lerp(0xd04010, 0x301810, (t - 0.7) / 0.3) }, (1.0 - t).powf(0.7) * 0.95),
                    Puff::Smoke => (tinted(lerp(0x2a2826, 0x5a554e, t), sky.tint), 0.55 * (1.0 - t)),
                    Puff::Spark => (0xffe080, 1.0 - t),
                    Puff::Dust => (tinted(0xb8a070, sky.tint), 0.6 * (1.0 - t)),
                    Puff::Foam => (tinted(0xe8f4ff, sky.tint), 0.7 * (1.0 - t)),
                    Puff::Trail => (tinted(0xd8d8d0, sky.tint), 0.5 * (1.0 - t)),
                };
                paint.ball(p.at, p.size, c, a);
            }
        }
        // A blast close by whitens the view for a moment.
        if self.flash > 0.02 {
            let t = ((self.flash * 150.0) as u32).min(200);
            for p in f.px[..vh * w].iter_mut() { *p = mix(*p, 0xfff0d0, t); }
        }
        (self.cols, self.deps, self.tops, self.over, self.order) = (cols, deps, tops, over, order);
    }

    /// The green figures on the glass: the heading, the gunsight, the box
    /// on the target, speed and height, the warnings.
    fn hud(&self, f: &mut Frame, eye: &Eye) {
        let (cx, cy) = (W / 2, SIGHT);
        for (x, y, w, h) in [(cx - 10, cy, 7, 1), (cx + 4, cy, 7, 1), (cx, cy - 10, 1, 7), (cx, cy + 4, 1, 7), (cx, cy, 1, 1)] { f.rect(x, y, w, h, HUD); }
        // The heading tape: three pixels a degree, a mark every five.
        let deg = self.yaw.to_degrees().rem_euclid(360.0);
        for t in ((deg - 34.0) / 5.0).ceil() as i32..=((deg + 34.0) / 5.0).floor() as i32 {
            let (h, x) = ((t * 5).rem_euclid(360), cx + ((t as f32 * 5.0 - deg) * 3.0).round() as i32);
            f.vline(x, 16, if h % 10 == 0 { 6 } else { 3 }, HUD);
            if h % 30 == 0 {
                let name = match h { 0 => "N".to_string(), 90 => "E".to_string(), 180 => "S".to_string(), 270 => "W".to_string(), _ => format!("{:02}", h / 10) };
                f.text_centered(x, 8, &name, HUD, false, 1);
            }
        }
        for (dx, dy) in [(0, 0), (-1, 1), (1, 1), (-2, 2), (2, 2)] { f.put(cx + dx, 24 + dy, HUD); }
        // On the tape: the way to the nearest target, and to the pad.
        let mark = |f: &mut Frame, x: f32, y: f32, s: &str, c: Rgb| {
            let rel = turn(wrap(x - self.x).atan2(wrap(y - self.y)) - self.yaw).to_degrees();
            let out = rel.abs() > 34.0;
            f.text_centered(cx + (rel.clamp(-34.0, 34.0) * 3.0) as i32, 29, if out { if rel < 0.0 { "<" } else { ">" } } else { s }, c, false, 1);
        };
        let nearest = self.foes.iter().filter(|t| t.primary && !t.dead)
            .min_by(|a, b| wrap(a.x - self.x).hypot(wrap(a.y - self.y)).total_cmp(&wrap(b.x - self.x).hypot(wrap(b.y - self.y))));
        if let Some(t) = nearest { mark(f, t.x, t.y, "V", AMBER); }
        if self.armour < 60.0 || self.rounds < 100 || self.rockets == 0 { mark(f, self.pad[0], self.pad[1], "H", HUD); }
        // Speed in kilometres an hour, height over the ground in metres.
        let agl = (self.alt - self.floor(self.x, self.y)).max(0.0);
        f.text(cx - 168, cy - 12, "SPEED", HUD);
        f.text_big(cx - 168, cy - 4, &format!("{:3.0}", self.speed * 3.6), HUD);
        f.text(cx + 148, cy - 12, "HEIGHT", HUD);
        f.text_big(cx + 148, cy - 4, &format!("{:3.0}", agl), HUD);
        f.text(cx + 148, cy + 8, &format!("SET {:3.0}", self.want), HUD);
        if let Some(t) = self.lock.map(|i| &self.foes[i]) {
            let p = eye.cam(t.mid());
            if p.z > NEAR {
                let (sx, sy) = eye.screen(p);
                let (sx, sy, r) = (sx as i32, sy as i32, (t.kind.size().0 * FOCAL / p.z * 0.9).clamp(9.0, 60.0) as i32);
                let c = if t.primary { AMBER } else { HUD };
                for (ax, ay) in [(-1, -1), (1, -1), (1, 1), (-1, 1)] {
                    f.hline((sx + ax * r).min(sx + ax * (r - 5)), sy + ay * r, 6, c);
                    f.vline(sx + ax * r, (sy + ay * r).min(sy + ay * (r - 5)), 6, c);
                }
                if sy + r + 12 < VH {
                    f.text_centered(sx, sy + r + 4, &format!("{} {:.0}M", t.kind.names().0.to_uppercase(), apart(self.me(), t.mid())), c, false, 1);
                }
            }
        }
        let blink = (self.time * 5.0) as i32 % 2 == 0;
        if self.warn == 2 { if blink { f.text_centered(cx, 44, "MISSILE", RED, true, 2); } }
        else if self.warn == 1 && blink { f.text_centered(cx, 46, "MISSILE SITE HAS YOU", AMBER, true, 1); }
        if self.steep && blink && self.mode == Mode::Fly { f.text_centered(cx, cy + 30, "TERRAIN AHEAD", AMBER, true, 1); }
        if let Some((s, _)) = &self.note { f.text_centered(cx, 70, s, TEXT, true, 1); }
        if self.rearm > 0.0 { f.text_centered(cx, 84, "ON THE PAD", HUD, true, 1); }
        // A hit rims the glass in red.
        if self.hit > 0.0 {
            for i in 0..3 { for (x, y, w, h) in [(i, i, W - 2 * i, 1), (i, VH - 1 - i, W - 2 * i, 1), (i, i, 1, VH - 2 * i), (W - 1 - i, i, 1, VH - 2 * i)] { f.rect(x, y, w, h, RED); } }
        }
    }

    /// The instruments under the view.
    fn panel(&self, f: &mut Frame, eye: &Eye) {
        const DIM: Rgb = 0x8a949e;
        f.rect(0, VH, W, PANEL, 0x14171b);
        f.hline(0, VH, W, 0x4a525c);
        f.hline(0, VH + 1, W, 0x0a0c0e);
        f.text(12, VH + 8, "ARMOUR", DIM);
        f.rect(12, VH + 16, 98, 9, 0x2a3038);
        let c = if self.armour > 50.0 { HUD } else if self.armour > 25.0 { AMBER } else { RED };
        f.rect(13, VH + 17, (self.armour.max(0.0) / 100.0 * 96.0) as i32, 7, c);
        f.text(12, VH + 32, "GUNSHIPS", DIM);
        for i in 0..self.lives as i32 { f.rect(50 + i * 9, VH + 32, 6, 5, HUD); }
        let left = |n: u32| if n == 0 { RED } else { TEXT };
        for (i, (name, n)) in [("GUN", self.rounds), ("ROCKETS", self.rockets), ("MISSILES", self.missiles)].into_iter().enumerate() {
            let y = VH + 8 + i as i32 * 12;
            f.text(132, y, name, DIM);
            f.text_big(170, y - 1, &format!("{:3}", n), left(n));
        }
        let (total, gone) = self.foes.iter().filter(|t| t.primary).fold((0, 0), |a, t| (a.0 + 1, a.1 + t.dead as u32));
        f.text(380, VH + 8, &format!("MISSION {}", self.mission + 1), DIM);
        f.text_big(380, VH + 16, MISSIONS[self.mission].name, TEXT);
        f.text(380, VH + 32, &format!("TARGETS {} OF {}", gone, total), if gone == total { HUD } else { AMBER });
        f.text(540, VH + 8, "SCORE", DIM);
        f.text_big(540, VH + 16, &format!("{:06}", self.score), TEXT);
        f.text(540, VH + 32, &format!("HIGH {:06}", self.high.max(self.score)), DIM);
        // The scope: what lies within 600 metres, the nose at the top.
        let (sx, sy, r) = (W / 2, H - 28, 25);
        f.circle(sx, sy, r + 2, 0x4a525c);
        f.circle(sx, sy, r, 0x061008);
        f.vline(sx, sy - r, 2 * r + 1, 0x12301a);
        f.hline(sx - r, sy, 2 * r + 1, 0x12301a);
        let blip = |f: &mut Frame, x: f32, y: f32, c: Rgb, big: bool, edge: bool| {
            let (dx, dy) = (wrap(x - self.x), wrap(y - self.y));
            let (mut px, mut py) = ((dx * eye.cos - dy * eye.sin) / 24.0, -(dx * eye.sin + dy * eye.cos) / 24.0);
            let far = px.hypot(py);
            if far > r as f32 - 2.0 {
                if !edge { return; }
                px *= (r as f32 - 2.0) / far;
                py *= (r as f32 - 2.0) / far;
            }
            let s = if big { 3 } else { 2 };
            f.rect(sx + px as i32 - s / 2, sy + py as i32 - s / 2, s, s, c);
        };
        blip(f, self.pad[0], self.pad[1], HUD, true, true);
        for t in self.foes.iter().filter(|t| !t.dead) {
            if t.primary { if (self.time * 3.0) as i32 % 2 == 0 { blip(f, t.x, t.y, AMBER, true, true); } }
            else { blip(f, t.x, t.y, if t.kind.range() > 0.0 { RED } else { DIM }, false, false); }
        }
        f.rect(sx - 1, sy - 2, 3, 1, TEXT);
        f.put(sx, sy - 3, TEXT);
        f.rect(sx, sy - 1, 1, 4, TEXT);
    }

    fn title(&self, f: &mut Frame) {
        let cx = W / 2;
        f.rect(0, VH, W, PANEL, 0x08090b);
        f.hline(0, VH, W, 0x4a525c);
        f.text_centered(cx + 4, 64, "RAID", 0x101418, true, 8);
        f.text_centered(cx, 60, "RAID", 0xffd060, true, 8);
        f.text_centered(cx, 130, "A gunship over fractal mountains", TEXT, true, 1);
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(cx, 196, "SPACE TO FLY", WHITE, true, 2); }
        f.text_centered(cx, 240, &format!("HIGH {:06}", self.high), AMBER, true, 1);
        for (i, line) in ["LEFT RIGHT TURN    UP DOWN SPEED    W S HEIGHT    A D SLIDE", "SPACE GUN    F ROCKET    E MISSILE AT THE BOXED TARGET",
            "HOVER LOW OVER THE PAD TO REARM    P PAUSE    Q QUIT"].iter().enumerate() {
            f.text_centered(cx, VH + 9 + i as i32 * 11, line, 0xa0a8b0, false, 1);
        }
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), VH - 8, VERSION, 0xd0c8b8);
    }

    /// The orders for the mission, over a dimmed view from the pad.
    fn brief(&self, f: &mut Frame) {
        let (m, cx) = (&MISSIONS[self.mission], W / 2);
        f.text_centered(cx, 62, &format!("MISSION {}", self.mission + 1), AMBER, true, 1);
        f.text_centered(cx, 80, &m.name.to_uppercase(), WHITE, true, 3);
        f.text_centered(cx, 124, m.orders[0], TEXT, true, 1);
        f.text_centered(cx, 138, m.orders[1], TEXT, true, 1);
        let list = |primary: bool| m.force.iter().filter(|x| x.2 == primary && (primary || x.0.range() > 0.0)).map(|x| count(x.0, x.1))
            .collect::<Vec<_>>().join(", ");
        f.text_centered(cx, 170, &format!("Targets: {}", list(true)), AMBER, true, 1);
        f.text_centered(cx, 184, &format!("Guarded by: {}", list(false)), 0xff9080, true, 1);
        if self.lap > 0 { f.text_centered(cx, 204, &format!("Round {}: they shoot faster and take more", self.lap + 1), TEXT, true, 1); }
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(cx, 240, "SPACE TO FLY", WHITE, true, 2); }
    }

    /// What the mission paid.
    fn tally(&self, f: &mut Frame) {
        let cx = W / 2;
        f.text_centered(cx, 70, "MISSION COMPLETE", WHITE, true, 3);
        let rows = [("Destroyed", self.kills), ("Mission", self.bonus[0]), ("Armour left", self.bonus[1]), ("Speed", self.bonus[2]), ("Score", self.score)];
        for (i, (name, n)) in rows.iter().enumerate() {
            let (y, c) = (124 + i as i32 * 14 + if i == 4 { 8 } else { 0 }, if i == 4 { AMBER } else { TEXT });
            f.text_big(cx - 90, y, name, c);
            let s = n.to_string();
            f.text_big(cx + 90 - Frame::text_width(&s, true, 1), y, &s, c);
        }
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(cx, 240, "SPACE FOR THE NEXT", WHITE, true, 2); }
    }
}

impl Game for Raid {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        let go = input.pressed(Key::Space) || input.pressed(Key::Enter);
        match self.mode {
            Mode::Title => {
                self.drift(dt);
                if go { self.start(); }
            }
            Mode::Brief => {
                self.timer += dt;
                if go && self.timer > 0.3 {
                    self.mode = Mode::Fly;
                    self.audio.play_loop(1, &self.snd.rotor, 0.5);
                    if self.mission == 0 && self.lap == 0 { self.say("HILLS ARE COVER. STAY LOW"); }
                }
            }
            Mode::Fly | Mode::Won => {
                if input.pressed(Key::Char('p')) { self.paused = !self.paused; }
                if !self.paused { self.fly(input, dt); }
            }
            Mode::Down => self.fall(dt),
            Mode::Debrief => {
                self.timer += dt;
                if go && self.timer > 0.5 { self.next(); }
            }
            Mode::Over => {
                self.timer += dt;
                self.world(dt);
                if go && self.timer > 1.0 {
                    self.lap = 0;
                    self.begin(self.first);
                    self.mode = Mode::Title;
                    self.audio.play_loop(1, &self.snd.title, 0.7);
                }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        let eye = self.eye();
        self.scene(f, &eye);
        let cx = W / 2;
        match self.mode {
            Mode::Title => self.title(f),
            Mode::Brief => { f.dim(0.4); self.brief(f); self.panel(f, &eye); }
            Mode::Fly | Mode::Won | Mode::Down => {
                self.hud(f, &eye);
                self.panel(f, &eye);
                if self.paused { f.dim(0.5); f.text_centered(cx, 150, "PAUSED", WHITE, true, 3); }
            }
            Mode::Debrief => { f.dim(0.4); self.tally(f); self.panel(f, &eye); }
            Mode::Over => {
                f.dim(0.4);
                f.text_centered(cx, 110, "GAME OVER", WHITE, true, 4);
                f.text_centered(cx, 170, &format!("SCORE {:06}", self.score), AMBER, true, 2);
                self.panel(f, &eye);
            }
        }
    }
}

/// The game as it is played: with sound, and at the mission asked for.
/// `RAID_START=<mission>[,<metres>]` skips the title and the orders, and
/// can put the gunship that far from its targets.
fn raid(sound: bool) -> Raid {
    let mut game = Raid::new();
    if sound { game.audio = Audio::open(); }
    let from = std::env::var("RAID_START").unwrap_or_default();
    let mut it = from.split(',').map(|s| s.trim().parse::<f32>().ok());
    if let Some(n) = it.next().flatten() {
        game.first = (n as usize).clamp(1, MISSIONS.len()) - 1;
        game.start();
        game.mode = Mode::Fly;
        if let Some(far) = it.next().flatten() { game.place(far); }
        game.audio.play_loop(1, &game.snd.rotor, 0.5);
    } else {
        game.audio.play_loop(1, &game.snd.title, 0.7);
    }
    game
}

fn main() {
    // RAID_BENCH=<frames> flies that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("RAID_BENCH") {
        let n: u32 = n.parse().unwrap_or(300);
        let mut game = raid(false);
        if game.mode == Mode::Title { game.start(); game.mode = Mode::Fly; }
        let mut f = Frame::new(W, H);
        let mut input = Input::new();
        input.inject(Key::Up);
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            game.update(&input, 1.0 / 30.0);
            game.draw(&mut f);
        }
        eprintln!("{:.2} ms a frame at {}x{} over {} frames", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n);
        return;
    }
    run(&mut raid(true), Config { width: W, height: H, fps: 30 });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game in the air over mission one, with nobody on the ground.
    fn alone() -> Raid {
        let mut game = Raid::new();
        game.start();
        game.mode = Mode::Fly;
        game.foes.clear();
        game
    }

    #[test]
    fn every_mission_stands_where_it_should() {
        let game = Raid::new();
        for n in 0..MISSIONS.len() {
            let (site, foes) = (game.sites[n], muster(&game.terrain, n, game.sites[n]));
            assert_eq!(foes.len() as u32, MISSIONS[n].force.iter().map(|f| f.1).sum::<u32>(), "mission {}", n + 1);
            assert!(foes.iter().any(|f| f.primary), "mission {} has nothing to destroy", n + 1);
            for f in &foes {
                let name = f.kind.names().0;
                match f.kind {
                    Kind::Boat => assert!(game.terrain.raw(f.x, f.y) < WATER, "mission {}: a {} on dry land", n + 1, name),
                    Kind::Heli => assert!(f.h > game.terrain.ground(f.x, f.y) + 10.0, "mission {}: a {} on the ground", n + 1, name),
                    _ => assert!(game.terrain.land(f.x, f.y), "mission {}: a {} in the water", n + 1, name),
                }
            }
            // The pad is dry and level, and out of reach of the guns.
            let (px, py) = game.pads[n];
            assert!(game.terrain.land(px, py) && game.terrain.slope(px, py) < 0.35, "mission {}: no good pad", n + 1);
            assert!(wrap(px - site.0).hypot(wrap(py - site.1)) > 370.0, "mission {}: the pad is too near", n + 1);
            for other in &game.sites[..n] { assert!(wrap(other.0 - site.0).hypot(wrap(other.1 - site.1)) > 250.0, "mission {} shares its ground", n + 1); }
        }
    }

    #[test]
    fn what_stands_on_the_ground_is_drawn_on_the_ground() {
        let mut game = alone();
        // Look at a point of the ground from 120 metres off and well above.
        let (tx, ty) = game.sites[0];
        let th = game.terrain.ground(tx, ty);
        let mut seen = 0;
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            (game.x, game.y, game.yaw) = ((tx - a.sin() * 120.0).rem_euclid(WORLD), (ty - a.cos() * 120.0).rem_euclid(WORLD), a);
            game.alt = th.max(game.terrain.ground(game.x, game.y)) + 40.0;
            if !game.terrain.sees(game.me(), [tx, ty, th + 1.0]) { continue; }
            seen += 1;
            let mut f = Frame::new(W, H);
            game.draw(&mut f);
            let eye = game.eye();
            let p = eye.cam([tx, ty, th]);
            let (sx, sy) = eye.screen(p);
            assert!((0.0..W as f32).contains(&sx) && (0.0..VH as f32).contains(&sy), "the point is off the screen at {} {}", sx, sy);
            let z = game.deps[sx as usize * VH as usize + sy as usize];
            assert!((z - p.z).abs() < Paint::slack(p.z) + 3.0, "from heading {}: the land is {} off, the point {}", a, z, p.z);
        }
        assert!(seen > 0, "the point is never in sight");
    }

    #[test]
    fn the_gunship_clears_the_hills_at_full_speed() {
        // No gun fires here, so all harm comes from the ground.
        for k in 0..6 {
            let mut game = alone();
            game.yaw = turn(k as f32 * TAU / 6.0);
            let mut input = Input::new();
            input.inject(Key::Up);
            for _ in 0..450 { game.update(&input, 1.0 / 30.0); }
            assert_eq!(game.mode, Mode::Fly);
            assert!(game.armour >= 80.0, "heading {}: the ground took {} of the armour", k, 100.0 - game.armour);
        }
    }

    #[test]
    fn the_gun_kills_what_is_in_the_box() {
        let mut game = alone();
        let (s, c) = game.yaw.sin_cos();
        let mut truck = Foe::new(Kind::Truck, true);
        (truck.x, truck.y) = ((game.x + s * 120.0).rem_euclid(WORLD), (game.y + c * 120.0).rem_euclid(WORLD));
        truck.h = game.terrain.ground(truck.x, truck.y);
        game.foes.push(truck);
        // Climb until it is in sight, then hold the trigger.
        let mut input = Input::new();
        input.inject(Key::Char('w'));
        for _ in 0..90 { game.update(&input, 1.0 / 30.0); }
        assert_eq!(game.lock, Some(0), "the truck is not boxed");
        let mut input = Input::new();
        input.inject(Key::Space);
        for _ in 0..90 { game.update(&input, 1.0 / 30.0); }
        assert!(game.foes[0].dead, "the truck stands, with {} armour", game.foes[0].armour);
        assert_eq!(game.mode, Mode::Won);
        assert!(game.score >= 100 && game.rounds < ROUNDS);
    }

    #[test]
    fn fuel_tanks_go_off_one_after_another() {
        let mut game = alone();
        for i in 0..4 {
            let mut tank = Foe::new(Kind::Fuel, i > 0);
            (tank.x, tank.y) = ((game.sites[0].0 + i as f32 * 9.0).rem_euclid(WORLD), game.sites[0].1);
            tank.h = game.terrain.ground(tank.x, tank.y);
            game.foes.push(tank);
        }
        game.damage(0, 100.0);
        assert!(game.foes[0].dead && !game.foes[1].dead);
        let input = Input::new();
        for _ in 0..60 { game.update(&input, 1.0 / 30.0); }
        assert!(game.foes.iter().all(|f| f.dead), "{} tanks stand", game.foes.iter().filter(|f| !f.dead).count());
    }

    #[test]
    fn a_missile_turns_no_faster_than_it_may() {
        let mut s = Shot::new(Ammo::Missile, [0.0; 3], [0.0, 1.0, 0.0], 100.0, 5.0, true, None);
        s.steer([100.0, 0.0, 0.0], 0.1, 200.0);
        assert!((s.speed() - 200.0).abs() < 0.01);
        let off = (s.vel[1] / 200.0).acos();
        assert!((off - 0.1).abs() < 0.01, "it turned {}", off);
        // What lies nearly ahead is turned to in one go.
        s.steer([s.vel[0], s.vel[1] + 1.0, 0.0], 0.1, 200.0);
        assert!(pass([0.0; 3], [0.0, 10.0, 0.0], [3.0, 5.0, 4.0]) == 5.0);
    }

    #[test]
    fn names_and_angles() {
        assert_eq!(count(Kind::Radar, 1), "1 radar");
        assert_eq!(count(Kind::Truck, 4), "4 trucks");
        assert_eq!(wrap(1000.0), -24.0);
        assert!((turn(3.0 * PI) - PI).abs() < 1e-4 || (turn(3.0 * PI) + PI).abs() < 1e-4);
        assert_eq!(KINDS.iter().map(|&k| k as usize).collect::<Vec<_>>(), (0..KINDS.len()).collect::<Vec<_>>());
    }
}
