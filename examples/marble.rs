//! marble: a glass ball down sloping courses against the clock, in the
//! spirit of Marble Madness (Atari, 1984). Six courses hang in the dark,
//! seen from above and from the side. The marble rolls as a ball does: it
//! gathers speed downhill, will not stop at once, flies off a ramp, and
//! breaks after a long fall. Steel balls shove it, green springs eat it
//! and pools of acid melt it. Each loss costs time, and time is all there
//! is: the seconds left at a goal go on to the next course.
//!
//!     cargo run --release --example marble
//!
//! The arrows roll the marble along the tiles: Right is down to the right,
//! Down is down to the left, Up and Left are the ways back. Two at once
//! roll it straight. P pauses, Q quits.
//!
//! Each course is drawn once, into a tall picture with the depth of every
//! pixel beside it. A frame is a window of that picture, and the marble
//! and its foes are drawn into it as lit balls, hidden where a wall
//! stands before them.
//!
//! `MARBLE_START=<course>` starts at another course; `MARBLE_BENCH=<frames>`
//! times the game with no terminal. Everything here is new: the courses,
//! the look and the sounds.

use funkey::*;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

const W: i32 = 640;
const H: i32 = 400;
const GAME: &str = "marble";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";
const COURSES: usize = 6;
/// Half a tile across and half a tile down on the screen, in pixels.
const TW: f32 = 24.0;
const TH: f32 = 12.0;
/// One tile of length seen square on, and one tile of height, in pixels.
const UNIT: f32 = 33.94;
const ZH: f32 = 29.39;
/// The ways right, up and out of the screen, in the course's own x, y
/// and height.
const RIGHT: [f32; 3] = [0.7071, -0.7071, 0.0];
const UP: [f32; 3] = [-0.3536, -0.3536, 0.866];
const OUT: [f32; 3] = [0.6124, 0.6124, 0.5];
/// Where the light comes from on a ball, and halfway from there to the eye.
const LIGHT: [f32; 3] = [-0.456, 0.659, 0.608];
const HALF: [f32; 3] = [-0.254, 0.368, 0.897];
/// The marble's radius, in tiles.
const R: f32 = 0.36;
/// What a key adds to the speed each second, and what a slope of one in
/// one adds.
const PUSH: f32 = 10.0;
const SLOPE: f32 = 15.0;
const GRAV: f32 = 24.0;
/// The share of its speed a rolling marble loses each second.
const DRAG: f32 = 1.6;
const TOP: f32 = 9.0;
/// The highest edge a marble rolls up, and the height of a rail.
const STEP: f32 = 0.2;
const RAIL: f32 = 0.5;
/// How far the floor may fall away under a marble that stays on it.
const SNAP: f32 = 0.06;
const BOUNCE: f32 = 0.45;
/// How hard a landing dazes the marble, and how hard breaks it.
const DAZE: f32 = 9.0;
const BREAK: f32 = 12.5;
/// How far down the walls go under a course before they fade out.
const SKIRT: f32 = 3.0;
/// The room over the highest tile in a course's picture, in pixels.
const HEAD: f32 = 120.0;
const BLUE: Rgb = 0x2458e0;
const PALE: Rgb = 0x86b4ff;
const GOLD: Rgb = 0xffe060;
const RED: Rgb = 0xff5040;
const TEXT: Rgb = 0xd0d8e8;
const X: (i32, i32) = (1, 0);
const Y: (i32, i32) = (0, 1);

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb { funkey::raster::blend(a, b, t.clamp(0.0, 1.0)) }

fn scale(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    let f = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
    rgb(f(r), f(g), f(b))
}

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Surf { Plain, Rail, Ice, Goal }

/// One square of a course: the height of its four corners, at x and y of
/// (0,0), (1,0), (0,1) and (1,1). It is two flat triangles.
#[derive(Clone, Copy)]
struct Tile { h: [f32; 4], surf: Surf }

impl Tile {
    /// True when the fold between the two triangles runs from the first
    /// corner to the last.
    fn fold(&self) -> bool { (self.h[0] - self.h[3]).abs() <= (self.h[1] - self.h[2]).abs() }

    /// The height at a point of the tile, and how it climbs along x and y.
    fn at(&self, u: f32, v: f32) -> (f32, (f32, f32)) {
        let [a, b, c, d] = self.h;
        if self.fold() {
            if u >= v { (a + (b - a) * u + (d - b) * v, (b - a, d - b)) } else { (a + (d - c) * u + (c - a) * v, (d - c, c - a)) }
        } else if u + v <= 1.0 {
            (a + (b - a) * u + (c - a) * v, (b - a, c - a))
        } else {
            (d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v), (d - c, d - b))
        }
    }
}

/// The colours of a course: the tiles, the lines between them, the wall
/// faces to the left and to the right, and the rails.
#[derive(Clone, Copy)]
struct Pal { top: Rgb, line: Rgb, left: Rgb, right: Rgb, rail: Rgb }

/// A point on the way down a course, and how fast the game takes it when
/// it plays itself. `tight` marks a narrow stretch with no rails, `fast`
/// the run up to a leap, where no new marble is set down.
#[derive(Clone, Copy)]
struct Way { x: f32, y: f32, z: f32, pace: f32, tight: bool, fast: bool }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Steelie, Slinky, Acid }

#[derive(Clone, Copy)]
struct Spawn { kind: Kind, x: f32, y: f32, to: (f32, f32) }

struct Course {
    name: &'static str,
    w: i32,
    h: i32,
    tiles: Vec<Option<Tile>>,
    way: Vec<Way>,
    spawns: Vec<Spawn>,
    /// The seconds this course adds to the clock.
    time: f32,
    pal: Pal,
    /// The height of the lowest tile.
    low: f32,
    /// What centres the course across the picture and sets its top row.
    mid: f32,
    top: f32,
    /// The picture: its rows, its colours and how near each pixel is.
    ih: i32,
    img: Vec<Rgb>,
    depth: Vec<f32>,
}

fn edge(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 { (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0) }

/// Every pixel of a triangle in a picture of so many rows, with how much
/// of each corner it has.
fn fill(rows: i32, p: [(f32, f32); 3], mut at: impl FnMut(usize, [f32; 3])) {
    let area = edge(p[0], p[1], p[2]);
    if area.abs() < 1e-4 { return; }
    let span = |f: fn(&(f32, f32)) -> f32, most: i32| {
        let (lo, hi) = p.iter().map(f).fold((f32::MAX, f32::MIN), |m, v| (m.0.min(v), m.1.max(v)));
        ((lo.floor() as i32).max(0), (hi.ceil() as i32).min(most - 1))
    };
    let ((x0, x1), (y0, y1)) = (span(|q| q.0, W), span(|q| q.1, rows));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let q = (x as f32 + 0.5, y as f32 + 0.5);
            let (a, b) = (edge(p[1], p[2], q) / area, edge(p[2], p[0], q) / area);
            let c = 1.0 - a - b;
            // A little over the edge, so no crack shows between two triangles.
            if a < -0.02 || b < -0.02 || c < -0.02 { continue; }
            at((y * W + x) as usize, [a, b, c]);
        }
    }
}

impl Course {
    fn tile(&self, x: i32, y: i32) -> Option<&Tile> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { return None; }
        self.tiles[(y * self.w + x) as usize].as_ref()
    }

    /// The floor under a point: its height, how it climbs, and its kind.
    fn floor(&self, x: f32, y: f32) -> Option<(f32, (f32, f32), Surf)> {
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        let t = self.tile(cx, cy)?;
        let (h, g) = t.at(x - cx as f32, y - cy as f32);
        Some((h, g, t.surf))
    }

    /// True when the way from one point to another runs into a face too
    /// high to roll up, for a marble at height `z`.
    fn wall(&self, from: (f32, f32), to: (f32, f32), z: f32) -> bool {
        let cell = |p: (f32, f32)| (p.0.floor() as i32, p.1.floor() as i32);
        let (a, b) = (cell(from), cell(to));
        if a == b { return false; }
        let Some(there) = self.tile(b.0, b.1) else { return false };
        // Where the way crosses into the far tile.
        let seam = (from.0.clamp(b.0 as f32, b.0 as f32 + 1.0), from.1.clamp(b.1 as f32, b.1 as f32 + 1.0));
        let far = there.at(seam.0 - b.0 as f32, seam.1 - b.1 as f32).0;
        if far <= z + STEP { return false; }
        match self.tile(a.0, a.1) {
            Some(here) => far > here.at((seam.0 - a.0 as f32).clamp(0.0, 1.0), (seam.1 - a.1 as f32).clamp(0.0, 1.0)).0 + STEP,
            None => true,
        }
    }

    /// Where a point of the course is in its picture.
    fn spot(&self, x: f32, y: f32, z: f32) -> (f32, f32) {
        (W as f32 / 2.0 + (x - y - self.mid) * TW, (x + y) * TH - z * ZH + self.top)
    }

    /// The point on the way nearest to a place.
    fn nearest(&self, p: [f32; 3]) -> usize {
        let far = |w: &Way| (w.x - p[0]).powi(2) + (w.y - p[1]).powi(2) + (2.0 * (w.z - p[2])).powi(2);
        (0..self.way.len()).min_by(|&a, &b| far(&self.way[a]).total_cmp(&far(&self.way[b]))).unwrap_or(0)
    }

    /// Draws the whole course into its picture, once.
    fn paint(&mut self) {
        let (rows, n) = (self.ih, (W * self.ih) as usize);
        let mut img: Vec<Rgb> = (0..n).map(|i| {
            let (x, y) = ((i as i32) % W, (i as i32) / W);
            let h = hash(x, y);
            // A few faint stars in the dark behind.
            if h % 1700 == 0 { scale(0x8090c0, 0.25 + (h >> 20 & 7) as f32 * 0.07) } else { mix(0x04040a, 0x0a0a18, y as f32 / rows as f32) }
        }).collect();
        let mut depth = vec![f32::MIN; n];
        let pal = self.pal;
        for cy in 0..self.h {
            for cx in 0..self.w {
                let Some(t) = self.tile(cx, cy).copied() else { continue };
                let (fx, fy) = (cx as f32, cy as f32);
                let [a, b, c, d] = t.h;
                // The top, as its two triangles.
                let tris: [[(f32, f32, f32); 3]; 2] = if t.fold() {
                    [[(0.0, 0.0, a), (1.0, 0.0, b), (1.0, 1.0, d)], [(0.0, 0.0, a), (0.0, 1.0, c), (1.0, 1.0, d)]]
                } else {
                    [[(0.0, 0.0, a), (1.0, 0.0, b), (0.0, 1.0, c)], [(1.0, 0.0, b), (0.0, 1.0, c), (1.0, 1.0, d)]]
                };
                for tri in tris {
                    fill(rows, tri.map(|(u, v, h)| self.spot(fx + u, fy + v, h)), |i, k| {
                        let (u, v) = (k[0] * tri[0].0 + k[1] * tri[1].0 + k[2] * tri[2].0, k[0] * tri[0].1 + k[1] * tri[1].1 + k[2] * tri[2].1);
                        let near = fx + fy + u + v;
                        if near < depth[i] { return; }
                        // Light from the upper left: a slope down to the right is
                        // darkest. The slope is taken smoothly over the tile, so a
                        // twisted one shows no fold.
                        let g = (lerp(b - a, d - c, v), lerp(c - a, d - b, u));
                        let lum = (0.22 + 0.98 * (0.55 * g.0 + 0.25 * g.1 + 0.8) / (1.0 + g.0 * g.0 + g.1 * g.1).sqrt()).clamp(0.35, 1.2);
                        let seam = u.min(1.0 - u).min(v).min(1.0 - v) < 0.04;
                        let color = match t.surf {
                            Surf::Goal => if ((u * 2.0) as i32 + (v * 2.0) as i32) & 1 == 0 { 0xf0f0f0 } else { 0x202028 },
                            Surf::Ice => {
                                let streak = ((u + v) * 2.0 + (hash(cx, cy) & 15) as f32 * 0.07).fract() < 0.1;
                                if seam { 0x7fb4d8 } else if streak { 0xffffff } else { 0xc4ecff }
                            }
                            Surf::Rail => if seam { pal.line } else { pal.rail },
                            Surf::Plain => if seam { pal.line } else { pal.top },
                        };
                        (depth[i], img[i]) = (near, scale(color, lum));
                    });
                }
                // The two faces turned to the eye: at the far x and the far y.
                // Each goes down to the tile beside it, or fades out under
                // the course where there is none.
                let sides = [
                    ((fx + 1.0, fy), (0.0, 1.0), (b, d), self.tile(cx + 1, cy).map(|n| (n.h[0], n.h[2])), pal.right),
                    ((fx, fy + 1.0), (1.0, 0.0), (c, d), self.tile(cx, cy + 1).map(|n| (n.h[0], n.h[1])), pal.left),
                ];
                for (from, by, tops, next, color) in sides {
                    let (void, foot) = match next { Some(f) => (false, (f.0.min(tops.0), f.1.min(tops.1))), None => (true, (tops.0 - SKIRT, tops.1 - SKIRT)) };
                    if tops.0 - foot.0 < 0.01 && tops.1 - foot.1 < 0.01 { continue; }
                    let quad = [(0.0, tops.0), (1.0, tops.1), (1.0, foot.1), (0.0, foot.0)];
                    for tri in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                        fill(rows, tri.map(|(s, z)| self.spot(from.0 + by.0 * s, from.1 + by.1 * s, z)), |i, k| {
                            let (s, z) = (k[0] * tri[0].0 + k[1] * tri[1].0 + k[2] * tri[2].0, k[0] * tri[0].1 + k[1] * tri[1].1 + k[2] * tri[2].1);
                            let near = from.0 + from.1 + s;
                            if near < depth[i] { return; }
                            let below = lerp(tops.0, tops.1, s) - z;
                            let fade = if void { (1.0 - below / SKIRT).max(0.0).powf(1.5) } else { (1.0 - below * 0.1).max(0.5) };
                            let seam = s < 0.04 || s > 0.96 || (z - z.round()).abs() < 0.03;
                            (depth[i], img[i]) = (near, scale(if seam { mix(color, pal.line, 0.5) } else { color }, fade));
                        });
                    }
                }
            }
        }
        (self.img, self.depth) = (img, depth);
    }
}

/// How a stretch of track is bent across its width: flat, up at both
/// edges, down at both edges, or up at one edge and down at the other.
#[derive(Clone, Copy)]
enum Shape { Flat, Gutter(f32), Crown(f32), Tilt(f32) }

/// A course being laid, a stretch at a time, from where the last ended.
struct Plan {
    tiles: HashMap<(i32, i32), Tile>,
    way: Vec<Way>,
    spawns: Vec<Spawn>,
    /// The middle tile of the row to lay next, and its height.
    x: i32,
    y: i32,
    z: f32,
    dir: (i32, i32),
    wide: i32,
    rails: bool,
    shape: Shape,
    /// The pace for the game playing itself; 0 leaves it to the width.
    pace: f32,
    surf: Surf,
}

impl Plan {
    fn new() -> Plan {
        Plan { tiles: HashMap::new(), way: Vec::new(), spawns: Vec::new(), x: 0, y: 0, z: 0.0, dir: X, wide: 3, rails: true, shape: Shape::Flat,
            pace: 0.0, surf: Surf::Plain }
    }

    fn wide(&mut self, n: i32) -> &mut Plan { self.wide = n; self }
    fn rails(&mut self, on: bool) -> &mut Plan { self.rails = on; self }
    fn shape(&mut self, s: Shape) -> &mut Plan { self.shape = s; self }
    fn pace(&mut self, p: f32) -> &mut Plan { self.pace = p; self }
    fn surf(&mut self, s: Surf) -> &mut Plan { self.surf = s; self }

    /// How many tiles of a row lie to each side of its middle one.
    fn span(&self) -> (i32, i32) { ((self.wide - 1) / 2, self.wide / 2) }

    /// The way across the track.
    fn across(&self) -> (i32, i32) { (self.dir.1.abs(), self.dir.0.abs()) }

    /// The middle of the row about to be laid.
    fn mid(&self) -> (f32, f32) {
        let ((lo, hi), p) = (self.span(), self.across());
        let c = (hi + 1 - lo) as f32 / 2.0;
        (self.x as f32 + 0.5 * p.1 as f32 + c * p.0 as f32, self.y as f32 + 0.5 * p.0 as f32 + c * p.1 as f32)
    }

    /// One tile, `q` to the side of the middle one. `h` gives the height
    /// of a corner from how far along and how far across it is, 0 or 1.
    /// A rail never takes the place of track.
    fn put(&mut self, q: i32, surf: Surf, h: impl Fn(f32, f32) -> f32) {
        let (p, dir) = (self.across(), self.dir);
        let at = (self.x + q * p.0, self.y + q * p.1);
        if surf == Surf::Rail && self.tiles.get(&at).is_some_and(|t| t.surf != Surf::Rail) { return; }
        let corner = |u: f32, v: f32| match dir {
            (1, 0) => h(u, v),
            (-1, 0) => h(1.0 - u, v),
            (0, 1) => h(v, u),
            _ => h(1.0 - v, u),
        };
        self.tiles.insert(at, Tile { h: [corner(0.0, 0.0), corner(1.0, 0.0), corner(0.0, 1.0), corner(1.0, 1.0)], surf });
    }

    /// A stretch of so many rows that goes down by `drop` over its length.
    fn run(&mut self, len: i32, drop: f32) -> &mut Plan {
        let ((lo, hi), w) = (self.span(), self.wide);
        let bend: Vec<f32> = (0..=w).map(|k| {
            let t = k as f32 / w as f32 * 2.0 - 1.0;
            match self.shape { Shape::Flat => 0.0, Shape::Gutter(d) => d * t * t, Shape::Crown(d) => -d * t * t, Shape::Tilt(d) => d * t / 2.0 }
        }).collect();
        let pace = if self.pace > 0.0 { self.pace } else {
            match (self.rails, w) { (true, 1) => 3.0, (true, _) => 4.5, (false, 1) => 2.0, (false, 2) => 2.8, (false, _) => 3.6 }
        };
        for i in 0..len {
            let (z0, z1) = (self.z - drop * i as f32 / len as f32, self.z - drop * (i + 1) as f32 / len as f32);
            // The bend comes in over the first row and goes out over the last.
            let full = |i: i32| if i == 0 || i == len { 0.0 } else { 1.0 };
            let (f0, f1) = (full(i), full(i + 1));
            for q in -lo..=hi {
                let (e0, e1) = (bend[(q + lo) as usize], bend[(q + lo + 1) as usize]);
                self.put(q, self.surf, |a, s| lerp(z0, z1, a) + lerp(f0, f1, a) * lerp(e0, e1, s));
            }
            if self.rails {
                let (e0, e1) = (bend[0], bend[w as usize]);
                self.put(-lo - 1, Surf::Rail, |a, _| lerp(z0, z1, a) + lerp(f0, f1, a) * e0 + RAIL);
                self.put(hi + 1, Surf::Rail, |a, _| lerp(z0, z1, a) + lerp(f0, f1, a) * e1 + RAIL);
            }
            let (x, y) = self.mid();
            self.way.push(Way { x, y, z: (z0 + z1) / 2.0, pace, tight: w < 3 && !self.rails, fast: pace >= 6.0 });
            (self.x, self.y) = (self.x + self.dir.0, self.y + self.dir.1);
        }
        self.z -= drop;
        self
    }

    /// A rail across the track, where the next row would lie.
    fn cap(&mut self) -> &mut Plan {
        let ((lo, hi), z) = (self.span(), self.z + RAIL);
        for q in -lo - 1..=hi + 1 { self.put(q, Surf::Rail, |_, _| z); }
        self
    }

    /// A rail across the track behind the row to lay next: the back of a
    /// starting place.
    fn back(&mut self) -> &mut Plan {
        (self.x, self.y) = (self.x - self.dir.0, self.y - self.dir.1);
        self.cap();
        (self.x, self.y) = (self.x + self.dir.0, self.y + self.dir.1);
        self
    }

    /// A square of flat track, and from its side the track goes on
    /// another way.
    fn turn(&mut self, to: (i32, i32)) -> &mut Plan {
        let ((lo, hi), w, old, from) = (self.span(), self.wide, self.dir, (self.x, self.y));
        let (shape, pace) = (self.shape, self.pace);
        let slow = if self.rails { 3.2 } else if w == 1 { 1.8 } else { 2.4 };
        (self.shape, self.pace) = (Shape::Flat, if pace > 0.0 { pace.min(slow) } else { slow });
        self.run(w, 0.0);
        // The way turns in the middle of the square.
        self.way.truncate(self.way.len() - (w / 2) as usize);
        if self.rails { self.cap(); }
        (self.shape, self.pace) = (shape, pace);
        let first = if old.0 + old.1 > 0 { 0 } else { 1 - w };
        if old.0 != 0 {
            self.x = from.0 + first + lo;
            self.y = if to.1 > 0 { from.1 + hi + 1 } else { from.1 - lo - 1 };
        } else {
            self.y = from.1 + first + lo;
            self.x = if to.0 > 0 { from.0 + hi + 1 } else { from.0 - lo - 1 };
        }
        self.dir = to;
        self
    }

    /// A hole of so many rows, and a step down by `drop`.
    fn gap(&mut self, len: i32, drop: f32) -> &mut Plan {
        (self.x, self.y, self.z) = (self.x + self.dir.0 * len, self.y + self.dir.1 * len, self.z - drop);
        self
    }

    /// A foe on the row to lay next. Acid slides from one edge of the
    /// track to the other, so there is always a way past it.
    fn foe(&mut self, kind: Kind) -> &mut Plan {
        let ((x, y), p) = (self.mid(), self.across());
        let k = if kind == Kind::Acid { self.wide as f32 / 2.0 - 0.5 } else { 0.0 };
        self.spawns.push(Spawn { kind, x: x - p.0 as f32 * k, y: y - p.1 as f32 * k, to: (x + p.0 as f32 * k, y + p.1 as f32 * k) });
        self
    }

    /// The chequered square a course ends on.
    fn goal(&mut self) -> &mut Plan {
        (self.surf, self.shape, self.pace) = (Surf::Goal, Shape::Flat, 0.0);
        self.run(self.wide.max(2), 0.0);
        if self.rails { self.cap(); }
        self
    }

    /// The finished course, moved to start at the first tile and drawn.
    fn done(&mut self, name: &'static str, time: f32, pal: Pal) -> Course {
        let lo = |f: fn(&(i32, i32)) -> i32| self.tiles.keys().map(f).min().unwrap_or(0) - 1;
        let hi = |f: fn(&(i32, i32)) -> i32| self.tiles.keys().map(f).max().unwrap_or(0) + 2;
        let ((x0, y0), (x1, y1)) = ((lo(|k| k.0), lo(|k| k.1)), (hi(|k| k.0), hi(|k| k.1)));
        let (w, h) = (x1 - x0, y1 - y0);
        let mut tiles = vec![None; (w * h) as usize];
        let (mut side, mut rows, mut low) = ((f32::MAX, f32::MIN), (f32::MAX, f32::MIN), f32::MAX);
        for (&(x, y), t) in &self.tiles {
            let (cx, cy) = (x - x0, y - y0);
            tiles[(cy * w + cx) as usize] = Some(*t);
            for (i, &z) in t.h.iter().enumerate() {
                let (px, py) = ((cx + (i as i32 & 1)) as f32, (cy + (i as i32 >> 1)) as f32);
                side = (side.0.min(px - py), side.1.max(px - py));
                rows = (rows.0.min((px + py) * TH - z * ZH), rows.1.max((px + py) * TH - (z - SKIRT) * ZH));
                low = low.min(z);
            }
        }
        assert!((side.1 - side.0) * TW <= W as f32 - 8.0, "{} is {} half tiles across, too many for the screen", name, side.1 - side.0);
        let shift = |x: f32, y: f32| (x - x0 as f32, y - y0 as f32);
        let way = self.way.iter().map(|p| { let (x, y) = shift(p.x, p.y); Way { x, y, ..*p } }).collect();
        let spawns = self.spawns.iter().map(|s| { let ((x, y), to) = (shift(s.x, s.y), shift(s.to.0, s.to.1)); Spawn { x, y, to, ..*s } }).collect();
        let mut c = Course { name, w, h, tiles, way, spawns, time, pal, low, mid: (side.0 + side.1) / 2.0, top: HEAD - rows.0,
            ih: ((rows.1 - rows.0 + HEAD) as i32 + 24).max(H), img: Vec::new(), depth: Vec::new() };
        c.paint();
        c
    }
}

/// The six courses. Each starts in the middle of the screen and runs down
/// it in legs to the right and to the left.
fn course(n: usize) -> Course {
    use Kind::*;
    use Shape::*;
    use Surf::{Ice, Plain};
    let mut p = Plan::new();
    match n {
        // Rails all the way: a place to learn how the marble rolls.
        0 => {
            p.back().run(4, 0.0).run(5, 1.0).turn(Y).run(6, 1.0).run(2, 0.0).run(5, 1.5).turn(X).run(4, 0.0).run(6, 1.5).turn(Y);
            p.run(5, 0.5).goal();
            p.done("WARM UP", 30.0, Pal { top: 0xb4bcd4, line: 0x5a628c, left: 0x6c74ac, right: 0x3e4474, rail: 0x8890c8 })
        }
        // Open edges, a chute, a step down, a steel ball and a row of humps.
        1 => {
            p.back().run(3, 0.0).rails(false).run(5, 1.0).turn(Y).run(2, 0.0);
            p.shape(Gutter(0.6)).run(9, 3.5).shape(Flat).rails(true).run(4, 0.0).turn(X);
            p.rails(false).run(3, 0.0).wide(2).run(4, 1.0).foe(Steelie).run(4, 0.0).gap(0, 1.0).wide(3).run(3, 0.0).turn(Y);
            p.run(3, 1.0).run(1, -0.3).run(1, 0.3).run(1, -0.3).run(1, 0.3).run(4, 1.0).turn(X);
            p.rails(true).run(5, 1.0).turn(Y).run(2, 0.0).goal();
            p.done("THE CHUTE", 40.0, Pal { top: 0xa8d0b0, line: 0x4c7c5c, left: 0x5c9c70, right: 0x346048, rail: 0x7cc090 })
        }
        // Bridges one tile wide and a road that falls away to both sides.
        2 => {
            p.back().run(3, 0.0).rails(false).wide(2).run(6, 1.0).turn(Y);
            p.wide(1).run(7, 0.0).wide(3).run(2, 0.0).foe(Slinky).run(4, 1.0).run(3, 0.0).turn(X);
            p.shape(Crown(0.35)).run(8, 1.5).shape(Flat).run(2, 0.0).turn(Y);
            p.wide(1).run(4, 0.0).turn(X).run(4, 0.0).turn(Y).run(3, 0.5).wide(2).run(2, 0.0).foe(Acid).run(5, 1.0).turn(X);
            p.wide(3).foe(Slinky).run(6, 1.0).rails(true).run(3, 0.0).turn(Y).run(2, 0.0).goal();
            p.done("NARROWS", 60.0, Pal { top: 0xd8c8a0, line: 0x8c7448, left: 0xb09058, right: 0x705830, rail: 0xc8a868 })
        }
        // A leap over a hole, roads that lean, and two steel balls.
        3 => {
            p.back().run(3, 0.0).rails(false).run(5, 0.5).turn(Y);
            p.rails(true).pace(7.0).run(7, 2.8).run(2, -0.5).gap(2, 1.3).pace(0.0).run(4, 0.0).turn(X);
            p.rails(false).shape(Tilt(0.5)).run(7, 1.0).shape(Flat).foe(Steelie).run(4, 0.0).turn(Y);
            p.wide(2).shape(Tilt(-0.4)).run(6, 1.0).shape(Flat).wide(3).foe(Steelie).run(4, 0.0).turn(X);
            p.run(3, 0.5).wide(1).run(5, 0.0).wide(3).run(3, 0.5).turn(Y);
            p.rails(true).run(6, 2.0).run(2, 0.0).goal();
            p.done("THE LEAP", 45.0, Pal { top: 0xe0b090, line: 0x905038, left: 0xc07050, right: 0x783828, rail: 0xd88860 })
        }
        // Ice: the keys do little on it, and nothing slows the marble.
        4 => {
            p.back().run(2, 0.0).surf(Ice).run(5, 1.0).surf(Plain).run(2, 0.0).turn(Y);
            p.rails(false).run(3, 0.5).foe(Acid).run(5, 1.0).surf(Ice).run(3, 0.0).surf(Plain).run(2, 0.0).turn(X);
            p.wide(2).surf(Ice).run(6, 0.5).surf(Plain).wide(3).run(2, 0.0).foe(Slinky).run(4, 0.5).turn(Y);
            p.rails(true).surf(Ice).shape(Gutter(0.5)).run(9, 3.0).shape(Flat).surf(Plain).run(3, 0.0).turn(X);
            p.rails(false).foe(Steelie).run(4, 0.0).wide(2).surf(Ice).run(5, 0.0).surf(Plain).wide(3).run(3, 0.0).turn(Y);
            p.foe(Acid).run(5, 1.0).rails(true).run(2, 0.0).goal();
            p.done("GLASS", 45.0, Pal { top: 0xc8b8e0, line: 0x685890, left: 0x8870b8, right: 0x504078, rail: 0xa890d8 })
        }
        // All of it, and little to hold on to.
        _ => {
            p.back().run(2, 0.0).rails(false).wide(2).run(6, 1.0).turn(Y);
            p.wide(1).run(5, 0.5).turn(X).run(3, 0.0).turn(Y).run(3, 0.5).wide(3).run(2, 0.0).foe(Steelie);
            p.shape(Crown(0.4)).run(6, 1.5).shape(Flat).turn(X);
            p.foe(Slinky).run(3, 0.5).wide(2).surf(Ice).run(5, 0.0).surf(Plain).foe(Acid).run(5, 0.5).turn(Y);
            p.wide(1).run(1, 0.0).wide(3).rails(true).pace(7.0).run(5, 2.2).run(2, -0.5).gap(2, 1.3).pace(0.0).run(4, 0.0).turn(X);
            p.rails(false).wide(2).foe(Steelie).run(4, 1.0).shape(Tilt(0.5)).run(5, 0.5).shape(Flat).wide(3).run(2, 0.0).turn(Y);
            p.foe(Slinky).run(4, 0.5).wide(1).run(4, 0.0).wide(3).rails(true).run(2, 0.0).goal();
            p.done("THE EDGE", 65.0, Pal { top: 0xc0c0c8, line: 0x505058, left: 0x80808c, right: 0x484850, rail: 0xe06050 })
        }
    }
}

/// A ball on a course: the marble, or a steel one.
#[derive(Clone, Copy)]
struct Ball {
    x: f32,
    y: f32,
    /// The height of its foot.
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    air: bool,
    /// The ball's own three axes as they lie now; rolling turns them.
    turn: [[f32; 3]; 3],
}

/// What happened to a ball in one step.
#[derive(Default)]
struct Roll { bump: f32, land: f32, broke: bool }

impl Ball {
    fn at(x: f32, y: f32, z: f32) -> Ball {
        Ball { x, y, z, vx: 0.0, vy: 0.0, vz: 0.0, air: false, turn: [[0.8, 0.0, 0.6], [0.0, 1.0, 0.0], [-0.6, 0.0, 0.8]] }
    }

    fn speed(&self) -> f32 { self.vx.hypot(self.vy) }

    /// One step: the push of the keys, the pull of the slope, the walls
    /// in the way and the floor underneath.
    fn roll(&mut self, c: &Course, push: (f32, f32), dt: f32) -> Roll {
        let mut out = Roll::default();
        match (self.air, c.floor(self.x, self.y)) {
            (false, Some((_, g, surf))) => {
                let (grip, drag) = if surf == Surf::Ice { (0.2, 0.12) } else { (1.0, DRAG) };
                self.vx += (push.0 * PUSH * grip - g.0 * SLOPE) * dt;
                self.vy += (push.1 * PUSH * grip - g.1 * SLOPE) * dt;
                let k = (-drag * dt).exp();
                (self.vx, self.vy) = (self.vx * k, self.vy * k);
            }
            // In the air the keys do little.
            _ => (self.vx, self.vy) = (self.vx + push.0 * PUSH * 0.12 * dt, self.vy + push.1 * PUSH * 0.12 * dt),
        }
        let v = self.speed();
        if v > TOP { (self.vx, self.vy) = (self.vx * TOP / v, self.vy * TOP / v); }
        // Sideways one way at a time, so a wall stops only the way into it.
        let (from, nx) = ((self.x, self.y), self.x + self.vx * dt);
        if c.wall(from, (nx + R * self.vx.signum(), self.y), self.z) || c.wall(from, (nx, self.y), self.z) {
            out.bump = self.vx.abs();
            self.vx *= -BOUNCE;
        } else {
            self.x = nx;
        }
        let (from, ny) = ((self.x, self.y), self.y + self.vy * dt);
        if c.wall(from, (self.x, ny + R * self.vy.signum()), self.z) || c.wall(from, (self.x, ny), self.z) {
            out.bump = out.bump.max(self.vy.abs());
            self.vy *= -BOUNCE;
        } else {
            self.y = ny;
        }
        self.spin(dt);
        match c.floor(self.x, self.y) {
            Some((h, g, _)) => {
                // How fast the floor rises or falls under the marble as it goes.
                let follow = g.0 * self.vx + g.1 * self.vy;
                if !self.air && h >= self.z + self.vz * dt - SNAP {
                    (self.z, self.vz) = (h, follow);
                } else {
                    self.air = true;
                    self.vz -= GRAV * dt;
                    self.z += self.vz * dt;
                    if self.z <= h {
                        let hit = follow - self.vz;
                        (self.z, out.land, out.broke) = (h, hit, hit > BREAK);
                        if hit > 4.0 { self.vz = follow + hit * 0.3; } else { (self.air, self.vz) = (false, follow); }
                    }
                }
            }
            None => {
                self.air = true;
                self.vz -= GRAV * dt;
                self.z += self.vz * dt;
            }
        }
        out
    }

    /// Turns the ball's own axes as far as it rolled.
    fn spin(&mut self, dt: f32) {
        let v = self.speed();
        if v < 0.01 { return; }
        let (k, (s, c)) = ([-self.vy / v, self.vx / v], (v * dt / R).sin_cos());
        for p in self.turn.iter_mut() {
            let d = k[0] * p[0] + k[1] * p[1];
            let x = [k[1] * p[2], -k[0] * p[2], k[0] * p[1] - k[1] * p[0]];
            *p = [p[0] * c + x[0] * s + k[0] * d * (1.0 - c), p[1] * c + x[1] * s + k[1] * d * (1.0 - c), p[2] * c + x[2] * s];
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            *p = [p[0] / len, p[1] / len, p[2] / len];
        }
    }
}

/// Two balls that touch push each other off; the answer is how fast they met.
fn knock(a: &mut Ball, b: &mut Ball, ma: f32, mb: f32) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let d = dx.hypot(dy);
    if d >= 2.0 * R || d < 1e-4 || (b.z - a.z).abs() > 2.0 * R { return 0.0; }
    let (nx, ny, over) = (dx / d, dy / d, 2.0 * R - d);
    (a.x, a.y) = (a.x - nx * over * mb / (ma + mb), a.y - ny * over * mb / (ma + mb));
    (b.x, b.y) = (b.x + nx * over * ma / (ma + mb), b.y + ny * over * ma / (ma + mb));
    let closing = (a.vx - b.vx) * nx + (a.vy - b.vy) * ny;
    if closing <= 0.0 { return 0.0; }
    let j = 1.6 * closing / (1.0 / ma + 1.0 / mb);
    (a.vx, a.vy) = (a.vx - j / ma * nx, a.vy - j / ma * ny);
    (b.vx, b.vy) = (b.vx + j / mb * nx, b.vy + j / mb * ny);
    closing
}

struct Foe {
    kind: Kind,
    b: Ball,
    home: (f32, f32),
    /// The far end of a pool of acid's slide.
    to: (f32, f32),
    /// The time into a spring's hop, or along the acid's slide.
    t: f32,
    /// Where a spring hops from and to.
    from: [f32; 3],
    dest: [f32; 3],
    alive: bool,
    /// Seconds since the marble struck it, counted down.
    hit: f32,
}

struct Fx { at: [f32; 3], vel: [f32; 3], age: f32, life: f32, color: Rgb, size: i32, weight: f32 }

struct Pop { text: String, at: [f32; 3], age: f32, color: Rgb }

/// The frame as a window on the course's picture, for what moves.
struct View<'a> { c: &'a Course, cam: i32, f: &'a mut Frame }

impl View<'_> {
    /// One pixel, if nothing of the course stands before it there.
    fn dot(&mut self, x: i32, y: i32, near: f32, c: Rgb, a: f32) {
        if x < 0 || x >= W || y < 0 || y >= H || a <= 0.0 { return; }
        if near + 0.05 < self.c.depth[((y + self.cam) * W + x) as usize] { return; }
        let p = &mut self.f.px[(y * W + x) as usize];
        *p = if a >= 1.0 { c } else { mix(*p, c, a) };
    }

    /// A lit ball with a band round it and a spot at each pole, so its
    /// rolling shows.
    fn ball(&mut self, b: &Ball, r: f32, base: Rgb, band: Rgb, a: f32) {
        let (sx, sy) = self.c.spot(b.x, b.y, b.z + r);
        let (sy, rp) = (sy - self.cam as f32, r * UNIT);
        for py in (sy - rp - 1.0) as i32..=(sy + rp + 1.0) as i32 {
            for px in (sx - rp - 1.0) as i32..=(sx + rp + 1.0) as i32 {
                let (dx, dy) = (px as f32 + 0.5 - sx, py as f32 + 0.5 - sy);
                let d = dx.hypot(dy);
                if d > rp + 0.5 { continue; }
                let (u, v) = (dx / UNIT, -dy / UNIT);
                let s = (r * r - u * u - v * v).max(0.0).sqrt();
                let n = [u / r, v / r, s / r];
                let w = [0, 1, 2].map(|i| n[0] * RIGHT[i] + n[1] * UP[i] + n[2] * OUT[i]);
                let own = b.turn[0][0] * w[0] + b.turn[0][1] * w[1] + b.turn[0][2] * w[2];
                let c = if own.abs() < 0.2 || own.abs() > 0.93 { band } else { base };
                let lit = (n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]).max(0.0);
                let shine = (n[0] * HALF[0] + n[1] * HALF[1] + n[2] * HALF[2]).max(0.0).powi(40);
                let c = mix(scale(c, 0.3 + 0.85 * lit), WHITE, shine * 0.9);
                self.dot(px, py, b.x + b.y - 0.7071 * v + 1.2247 * s, c, a * (rp + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }

    /// Something flat on the floor at a height: every pixel of an oval
    /// that lies on that floor, with how far out it is, 0 to 1, and the
    /// angle round it.
    fn flat(&mut self, x: f32, y: f32, h: f32, r: f32, mut paint: impl FnMut(Rgb, f32, f32) -> Rgb) {
        let (sx, sy) = self.c.spot(x, y, h);
        let (sy, rx, ry) = (sy - self.cam as f32, r * UNIT, r * UNIT / 2.0);
        for py in (sy - ry) as i32..=(sy + ry) as i32 {
            for px in (sx - rx) as i32..=(sx + rx) as i32 {
                let (dx, dy) = ((px as f32 + 0.5 - sx) / rx, (py as f32 + 0.5 - sy) / ry);
                let q = dx * dx + dy * dy;
                if q >= 1.0 || px < 0 || px >= W || py < 0 || py >= H { continue; }
                // Only where the picture shows this floor, not a wall before it.
                if (self.c.depth[((py + self.cam) * W + px) as usize] - (x + y + dy * ry / TH)).abs() > 0.5 { continue; }
                let p = &mut self.f.px[(py * W + px) as usize];
                *p = paint(*p, q.sqrt(), dy.atan2(dx));
            }
        }
    }

    /// A small square at a place in the course.
    fn speck(&mut self, at: [f32; 3], size: i32, c: Rgb, a: f32) {
        let (sx, sy) = self.c.spot(at[0], at[1], at[2]);
        let (x, y) = (sx as i32, sy as i32 - self.cam);
        for oy in 0..size { for ox in 0..size { self.dot(x + ox - size / 2, y + oy - size / 2, at[0] + at[1] + 0.3, c, a); } }
    }
}

/// Two tunes of the same length as one sample, so they never drift apart.
fn duet(lead: &str, bass: &str) -> Sample {
    Tune::parse(lead, Wave::Square, 0.09).render().mixed(&Tune::parse(bass, Wave::Triangle, 0.28).render())
}

const TITLE: [&str; 2] = ["144 e5/8 g5/8 c6/8 g5/8 e5/8 g5/8 c6/4 f5/8 a5/8 c6/8 a5/8 f5/8 a5/8 c6/4 \
    g5/8 b5/8 d6/8 b5/8 g5/8 b5/8 d6/4 c6/8 g5/8 e5/8 g5/8 c5/2",
    "144 c3/4 g3/4 c3/4 g3/4 f3/4 c4/4 f3/4 c4/4 g3/4 d4/4 g3/4 d4/4 c3/4 g3/4 c3/2"];
const TUNE: [&str; 2] = ["160 a5/2 c6/2 f5/2 a5/2 g5/2 b5/2 e5/1",
    "160 a3/8 e4/8 a4/8 e4/8 c4/8 e4/8 a4/8 e4/8 f3/8 c4/8 f4/8 c4/8 a3/8 c4/8 f4/8 c4/8 \
    g3/8 d4/8 g4/8 d4/8 b3/8 d4/8 g4/8 d4/8 e3/8 b3/8 e4/8 b3/8 g#3/8 b3/8 e4/8 b3/8"];

struct Sounds { title: Sample, tune: Sample, roll: Sample, bump: Sample, land: Sample, fall: Sample, smash: Sample, gulp: Sample, fizz: Sample,
    clack: Sample, goal: Sample, tick: Sample, warn: Sample, over: Sample, start: Sample, daze: Sample }

impl Sounds {
    fn new() -> Sounds {
        let step = TAU / funkey::audio::RATE as f32;
        // The rumble of a rolling ball: noise with the top taken off.
        let mut low = 0.0f32;
        let roll = Sample::synth(0.6, |_, r| { low += (r - low) * 0.06; low * 2.6 });
        let mut ph = 0.0f32;
        let land = Sample::synth(0.16, |t, r| { ph += step * (150.0 - 500.0 * t).max(50.0); (ph.sin() * 0.7 + r * 0.2) * (-t * 22.0).exp() });
        // Glass breaking: a crack, then high notes that ring out.
        let smash = Sample::synth(0.7, |t, r| {
            let ring: f32 = [2100.0f32, 3170.0, 4420.0].iter().map(|hz| (t * hz * TAU).sin()).sum();
            r * 0.7 * (-t * 30.0).exp() + ring * 0.14 * (-t * 7.0).exp()
        });
        let fizz = Sample::synth(0.9, |t, r| r * 0.4 * (1.0 - t / 0.9) * (0.6 + 0.4 * (t * 90.0).sin()));
        let mut ph = 0.0f32;
        let daze = Sample::synth(0.5, |t, _| { ph += step * (700.0 + 250.0 * (t * 40.0).sin()); ph.sin() * 0.3 * (1.0 - t / 0.5) });
        Sounds { title: duet(TITLE[0], TITLE[1]), tune: duet(TUNE[0], TUNE[1]), roll, land, smash, fizz, daze,
            bump: Sample::tone(Wave::Square, 190.0, 0.03, 0.3),
            fall: Sample::sweep(Wave::Sine, 1100.0, 140.0, 0.8, 0.3),
            gulp: Sample::sweep(Wave::Triangle, 320.0, 80.0, 0.16, 0.5).then(&Sample::sweep(Wave::Triangle, 80.0, 220.0, 0.12, 0.5)),
            clack: Sample::tone(Wave::Square, 950.0, 0.025, 0.35),
            goal: Tune::parse("220 c5/8 e5/8 g5/8 c6/4 g5/8 c6/2", Wave::Square, 0.25).render(),
            tick: Sample::tone(Wave::Square, 1300.0, 0.015, 0.2),
            warn: Sample::tone(Wave::Square, 880.0, 0.07, 0.25),
            over: Tune::parse("110 e4/4 d4/4 c4/4 a3/2", Wave::Triangle, 0.4).render(),
            start: Tune::parse("240 c5/8 g5/8 c6/4", Wave::Triangle, 0.4).render() }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Ready, Play, Goal, Over, Won }

/// What took a marble.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Lost { Fell, Broke, Eaten, Melted }

struct Marble {
    audio: Audio,
    snd: Sounds,
    rng: Rng,
    mode: Mode,
    /// The game plays itself under its name.
    demo: bool,
    /// The game plays itself in a real game, for tests and films.
    auto: bool,
    /// No foes, to try a course by itself.
    calm: bool,
    /// Seconds in this mode, and seconds in all.
    t: f32,
    time: f32,
    score: u32,
    high: u32,
    /// The course to begin at, and the one now played.
    start: usize,
    level: usize,
    course: Course,
    /// The seconds left.
    clock: f32,
    ball: Ball,
    /// A marble lost, why, and how long until the next one.
    gone: Option<(Lost, f32)>,
    /// The point on the way the next marble is set down at.
    back: usize,
    /// Seconds a new marble cannot be eaten or melted, and seconds a
    /// dazed one will not answer the keys.
    safe: f32,
    daze: f32,
    /// Where the marble last touched the floor.
    last: [f32; 3],
    falling: bool,
    foes: Vec<Foe>,
    fx: Vec<Fx>,
    pops: Vec<Pop>,
    /// The picture's row at the top of the screen.
    cam: f32,
    /// The point on the way the game is at when it plays itself.
    at: usize,
    /// Points for the goal still to be counted up.
    bonus: u32,
    /// The rumble's loudness as last sent, in steps.
    hum: i32,
}

impl Marble {
    fn new() -> Marble {
        let mut game = Marble { audio: Audio::off(), snd: Sounds::new(), rng: if cfg!(test) { Rng::new(7) } else { Rng::from_time() },
            mode: Mode::Play, demo: true, auto: false, calm: false, t: 0.0, time: 0.0, score: 0,
            high: funkey::scores::best(GAME), start: 0, level: 0, course: course(0), clock: 0.0, ball: Ball::at(0.0, 0.0, 0.0), gone: None,
            back: 0, safe: 0.0, daze: 0.0, last: [0.0; 3], falling: false, foes: Vec::new(), fx: Vec::new(), pops: Vec::new(), cam: 0.0, at: 0,
            bonus: 0, hum: -1 };
        game.settle();
        game.clock = game.course.time;
        game
    }

    /// A sound, unless the game is only showing itself.
    fn play(&mut self, pick: impl Fn(&Sounds) -> &Sample, vol: f32) {
        if self.demo { return; }
        let s = pick(&self.snd).clone();
        self.audio.play(&s, vol.min(1.0));
    }

    /// The rumble of the rolling marble, louder and higher the faster it goes.
    fn rumble(&mut self, speed: f32) {
        let step = if self.demo { 0 } else { (speed * 2.0) as i32 };
        if step == self.hum { return; }
        self.hum = step;
        self.audio.volume(2, (step as f32 / 16.0).min(1.0) * 0.5);
        self.audio.rate(2, 0.6 + step as f32 / 14.0);
    }

    /// The marble and the foes at their places on the course as it is.
    fn settle(&mut self) {
        let c = &self.course;
        let w = c.way[0];
        let z = c.floor(w.x, w.y).map_or(w.z, |f| f.0);
        self.ball = Ball::at(w.x, w.y, z);
        self.foes = if self.calm { Vec::new() } else {
            c.spawns.iter().map(|s| {
                let z = c.floor(s.x, s.y).map_or(0.0, |f| f.0);
                Foe { kind: s.kind, b: Ball::at(s.x, s.y, z), home: (s.x, s.y), to: s.to, t: 0.0, from: [s.x, s.y, z], dest: [s.x, s.y, z],
                    alive: true, hit: 0.0 }
            }).collect()
        };
        (self.gone, self.safe, self.daze, self.last, self.falling, self.at, self.back) = (None, 0.0, 0.0, [w.x, w.y, z], false, 0, 0);
        self.fx.clear();
        self.pops.clear();
        self.cam = self.aim();
    }

    /// Another course.
    fn enter(&mut self, level: usize) {
        (self.level, self.course) = (level, course(level));
        self.settle();
        (self.mode, self.t) = (Mode::Ready, 0.0);
    }

    /// A new game.
    fn begin(&mut self) {
        (self.demo, self.score) = (false, 0);
        self.enter(self.start.min(COURSES - 1));
        self.clock = self.course.time;
        self.audio.play_loop(1, &self.snd.tune, 0.3);
        self.audio.play_loop(2, &self.snd.roll, 0.0);
        self.hum = -1;
        self.play(|s| &s.start, 0.7);
    }

    /// Back to the name, with the game playing itself behind it.
    fn title(&mut self) {
        self.demo = true;
        self.enter(0);
        (self.mode, self.clock) = (Mode::Play, self.course.time);
        self.audio.stop(2);
        self.audio.play_loop(1, &self.snd.title, 0.5);
    }

    /// The end of a game.
    fn end(&mut self, mode: Mode) {
        (self.mode, self.t) = (mode, 0.0);
        self.rumble(0.0);
        self.audio.stop(1);
        if !cfg!(test) { funkey::scores::record(GAME, self.score); }
        if mode == Mode::Over { self.play(|s| &s.over, 0.8); }
    }

    /// Where the window on the picture wants to be.
    fn aim(&self) -> f32 {
        let c = &self.course;
        let p = match self.gone {
            Some((why, _)) if why != Lost::Fell => { let w = c.way[self.back]; [w.x, w.y, w.z] }
            _ => [self.ball.x, self.ball.y, self.ball.z.max(self.last[2] - 2.0)],
        };
        (c.spot(p[0], p[1], p[2]).1 - H as f32 * 0.45).clamp(0.0, (c.ih - H) as f32)
    }

    /// A marble is lost: time goes by before the next is set down.
    fn lose(&mut self, why: Lost) {
        let b = self.ball;
        self.back = self.course.nearest(if why == Lost::Fell { self.last } else { [b.x, b.y, b.z] });
        // A leap needs its run-up: back to where that begins.
        while self.back > 0 && self.course.way[self.back].fast { self.back -= 1; }
        self.gone = Some((why, match why { Lost::Fell => 1.0, Lost::Broke => 1.6, _ => 1.8 }));
        self.rumble(0.0);
        let at = [b.x, b.y, b.z + R];
        match why {
            Lost::Fell => {}
            Lost::Broke => {
                for _ in 0..18 {
                    let (a, v) = (self.rng.range(0.0, TAU), self.rng.range(1.0, 4.5));
                    self.fx.push(Fx { at, vel: [a.cos() * v, a.sin() * v, self.rng.range(2.0, 7.0)], age: 0.0, life: self.rng.range(0.5, 1.0),
                        color: if self.rng.chance(0.4) { WHITE } else { PALE }, size: 2 + self.rng.below(2) as i32, weight: 1.0 });
                }
                self.play(|s| &s.smash, 0.9);
            }
            Lost::Eaten => {
                self.pops.push(Pop { text: "GULP".into(), at, age: 0.0, color: 0x60ff70 });
                self.play(|s| &s.gulp, 0.9);
            }
            Lost::Melted => {
                for _ in 0..14 {
                    let a = self.rng.range(0.0, TAU);
                    self.fx.push(Fx { at, vel: [a.cos() * 0.6, a.sin() * 0.6, self.rng.range(0.5, 2.0)], age: 0.0, life: self.rng.range(0.6, 1.4),
                        color: 0x70ff60, size: 2, weight: -0.05 });
                }
                self.play(|s| &s.fizz, 0.8);
            }
        }
    }

    /// The next marble, dropped in on the way.
    fn place(&mut self) {
        let w = self.course.way[self.back];
        let z = self.course.floor(w.x, w.y).map_or(w.z, |f| f.0);
        self.ball = Ball { z: z + 1.0, air: true, ..Ball::at(w.x, w.y, z) };
        (self.gone, self.safe, self.daze, self.last, self.falling, self.at) = (None, 1.5, 0.0, [w.x, w.y, z], false, self.back);
    }

    /// What the game presses when it plays itself: toward a point a
    /// little ahead on the way, at the pace set for that stretch.
    fn bot(&mut self) -> (f32, f32) {
        let (b, way) = (self.ball, &self.course.way);
        if self.gone.is_some() || b.air { return (0.0, 0.0); }
        let far = |i: usize| (way[i].x - b.x).powi(2) + (way[i].y - b.y).powi(2) + (2.0 * (way[i].z - b.z)).powi(2);
        let last = way.len() - 1;
        let mut at = (self.at.saturating_sub(2)..=(self.at + 6).min(last)).min_by(|&i, &j| far(i).total_cmp(&far(j))).unwrap_or(0);
        // After a fall it may be anywhere.
        if far(at) > 4.0 { at = self.course.nearest([b.x, b.y, b.z]); }
        self.at = at;
        let aim = way[(at + if way[at].tight { 1 } else { 2 }).min(last)];
        // The slowest pace of the next few points, but not of those past a hole.
        let mut pace = way[at].pace;
        for i in at + 1..=(at + 3).min(last) {
            if (way[i].x - way[i - 1].x).hypot(way[i].y - way[i - 1].y) > 1.6 { break; }
            pace = pace.min(way[i].pace);
        }
        let (dx, dy) = (aim.x - b.x, aim.y - b.y);
        let d = dx.hypot(dy).max(0.01);
        let key = |want: f32, v: f32| if want - v > 0.5 { 1.0 } else if want - v < -0.5 { -1.0 } else { 0.0 };
        (key(dx / d * pace, b.vx), key(dy / d * pace, b.vy))
    }

    /// One step of the foes.
    fn stir(&mut self, dt: f32) {
        let Marble { foes, ball, course, rng, gone, score, pops, demo, .. } = self;
        let seen = gone.is_none();
        for f in foes.iter_mut().filter(|f| f.alive) {
            f.hit = (f.hit - dt).max(0.0);
            let (dx, dy) = (ball.x - f.b.x, ball.y - f.b.y);
            let d = dx.hypot(dy).max(0.01);
            match f.kind {
                // A steel ball charges the marble from a little way off, and rolls
                // home when the marble is far. Right beside the marble it only
                // rolls, so a steady push always gets past it. A knock stuns it
                // for a second, and for the first of that it will go over an edge.
                Kind::Steelie => {
                    let (hx, hy) = (f.home.0 - f.b.x, f.home.1 - f.b.y);
                    let h = hx.hypot(hy);
                    let mut push = if f.hit > 2.0 { (0.0, 0.0) }
                        else if seen && d < 6.0 && (ball.z - f.b.z).abs() < 1.5 { if d > 1.0 { (dx / d * 0.5, dy / d * 0.5) } else { (0.0, 0.0) } }
                        else if h > 0.6 { (hx / h * 0.5, hy / h * 0.5) }
                        else { (-f.b.vx.clamp(-1.0, 1.0), -f.b.vy.clamp(-1.0, 1.0)) };
                    if f.hit <= 2.4 {
                        if course.floor(f.b.x + (push.0 + f.b.vx * 0.3).signum() * 0.9, f.b.y).is_none() { push.0 = -f.b.vx.clamp(-1.0, 1.0); }
                        if course.floor(f.b.x, f.b.y + (push.1 + f.b.vy * 0.3).signum() * 0.9).is_none() { push.1 = -f.b.vy.clamp(-1.0, 1.0); }
                    }
                    f.b.roll(course, push, dt);
                    if f.b.z < course.low - 5.0 {
                        f.alive = false;
                        if f.hit > 0.0 && !*demo {
                            *score += 1000;
                            pops.push(Pop { text: "1000".into(), at: [ball.x, ball.y, ball.z + 1.0], age: 0.0, color: GOLD });
                        }
                    }
                }
                // A spring hops from tile to tile, after the marble when it is near.
                Kind::Slinky => {
                    f.t += dt;
                    if f.t >= 0.8 {
                        (f.t, f.from) = (0.0, f.dest);
                        let mut best = (f32::MAX, f.dest);
                        for (ox, oy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                            let (x, y) = (f.dest[0].floor() + 0.5 + ox, f.dest[1].floor() + 0.5 + oy);
                            let Some((h, _, surf)) = course.floor(x, y) else { continue };
                            if surf == Surf::Rail || (h - f.dest[2]).abs() > 0.6 || (x - f.home.0).hypot(y - f.home.1) > 3.5 { continue; }
                            let rank = if seen && d < 6.0 { (x - ball.x).hypot(y - ball.y) } else { rng.float() * 9.0 };
                            if rank < best.0 { best = (rank, [x, y, h]); }
                        }
                        f.dest = best.1;
                    }
                    let e = (f.t / 0.5).min(1.0);
                    (f.b.x, f.b.y, f.b.z) = (lerp(f.from[0], f.dest[0], e), lerp(f.from[1], f.dest[1], e), lerp(f.from[2], f.dest[2], e) + 0.6 * (e * PI).sin());
                }
                // Acid slides to and fro.
                Kind::Acid => {
                    f.t += dt;
                    let e = 0.5 - 0.5 * (f.t * 1.1).cos();
                    (f.b.x, f.b.y) = (lerp(f.home.0, f.to.0, e), lerp(f.home.1, f.to.1, e));
                    f.b.z = course.floor(f.b.x, f.b.y).map_or(f.b.z, |g| g.0);
                }
            }
        }
    }

    /// One step of a course being played.
    fn roll(&mut self, push: (f32, f32), dt: f32) {
        self.clock -= dt;
        if self.clock <= 0.0 {
            if self.demo { self.settle(); self.clock = self.course.time; return; }
            self.clock = 0.0;
            self.end(Mode::Over);
            return;
        }
        if !self.demo && self.clock < 10.0 && (self.clock + dt).ceil() != self.clock.ceil() { self.play(|s| &s.warn, 0.6); }
        (self.safe, self.daze) = ((self.safe - dt).max(0.0), (self.daze - dt).max(0.0));
        self.stir(dt);
        if let Some((why, left)) = self.gone {
            if why == Lost::Fell { self.ball.roll(&self.course, (0.0, 0.0), dt); }
            if left <= dt { self.place(); } else { self.gone = Some((why, left - dt)); }
            return;
        }
        let len = push.0.hypot(push.1).max(1.0);
        let push = if self.daze > 0.0 { (0.0, 0.0) } else { (push.0 / len, push.1 / len) };
        let roll = self.ball.roll(&self.course, push, dt);
        if roll.bump > 1.0 { self.play(|s| &s.bump, roll.bump / 6.0); }
        if roll.broke { self.lose(Lost::Broke); return; }
        if roll.land > 1.5 {
            self.play(|s| &s.land, roll.land / 8.0);
            if roll.land > DAZE { self.daze = 0.9; self.play(|s| &s.daze, 0.7); }
            for _ in 0..if roll.land > 5.0 { 6 } else { 0 } {
                let a = self.rng.range(0.0, TAU);
                self.fx.push(Fx { at: [self.ball.x, self.ball.y, self.ball.z + 0.05], vel: [a.cos() * 1.5, a.sin() * 1.5, 0.6], age: 0.0, life: 0.4,
                    color: 0xa0a0a8, size: 2, weight: 0.1 });
            }
        }
        let b = self.ball;
        if b.air {
            if !self.falling && b.z < self.last[2] - 1.2 && self.course.floor(b.x, b.y).is_none() {
                self.falling = true;
                self.play(|s| &s.fall, 0.7);
            }
            if b.z < self.last[2] - 7.0 || b.z < self.course.low - 3.0 { self.lose(Lost::Fell); return; }
            self.rumble(0.0);
        } else {
            (self.last, self.falling) = ([b.x, b.y, b.z], false);
            self.rumble(b.speed());
            if self.course.floor(b.x, b.y).is_some_and(|f| f.2 == Surf::Goal) {
                (self.mode, self.t) = (Mode::Goal, 0.0);
                self.rumble(0.0);
                let last = self.level + 1 == COURSES;
                self.bonus = if self.demo { 0 } else { 1000 * (self.level as u32 + 1) + self.clock as u32 * if last { 300 } else { 100 } };
                self.play(|s| &s.goal, 0.8);
                return;
            }
        }
        // What the foes do to it.
        let mut hit = (0.0f32, None);
        for f in self.foes.iter_mut().filter(|f| f.alive) {
            let (d, dz) = ((f.b.x - self.ball.x).hypot(f.b.y - self.ball.y), (f.b.z - self.ball.z).abs());
            match f.kind {
                Kind::Steelie => {
                    let met = knock(&mut self.ball, &mut f.b, 1.0, 1.3);
                    if met > 0.5 { f.hit = 3.0; hit.0 = hit.0.max(met); }
                }
                Kind::Slinky => if self.safe <= 0.0 && d < 0.55 && dz < 0.7 { hit.1 = Some(Lost::Eaten); },
                Kind::Acid => if self.safe <= 0.0 && !self.ball.air && d < 0.5 && dz < 0.5 { hit.1 = Some(Lost::Melted); },
            }
        }
        if hit.0 > 0.0 { self.play(|s| &s.clack, hit.0 / 5.0); }
        if let Some(why) = hit.1 { self.lose(why); }
    }

    /// One frame of the game, with the way the keys push.
    fn step(&mut self, push: (f32, f32), go: bool, dt: f32) {
        (self.time, self.t) = (self.time + dt, self.t + dt);
        for p in self.fx.iter_mut() {
            p.age += dt;
            p.vel[2] -= GRAV * p.weight * dt;
            for i in 0..3 { p.at[i] += p.vel[i] * dt; }
        }
        self.fx.retain(|p| p.age < p.life);
        for p in self.pops.iter_mut() { p.age += dt; p.at[2] += dt * 0.8; }
        self.pops.retain(|p| p.age < 1.2);
        match self.mode {
            Mode::Ready => if self.t > 1.6 { (self.mode, self.t) = (Mode::Play, 0.0); },
            Mode::Play => {
                let push = if self.demo || self.auto { self.bot() } else { push };
                self.roll(push, dt);
            }
            Mode::Goal => {
                // The points for the goal are counted up, then the next course.
                if self.t > 0.7 && self.bonus > 0 {
                    let n = self.bonus.min(((dt * 3000.0) as u32).max(10));
                    (self.bonus, self.score) = (self.bonus - n, self.score + n);
                    if (self.time * 20.0) as i32 != ((self.time - dt) * 20.0) as i32 { self.play(|s| &s.tick, 0.5); }
                }
                if self.bonus == 0 && self.t > if self.demo { 1.5 } else { 3.2 } {
                    if self.demo {
                        self.enter((self.level + 1) % COURSES);
                        (self.mode, self.clock) = (Mode::Play, self.course.time);
                    } else if self.level + 1 == COURSES {
                        self.end(Mode::Won);
                    } else {
                        self.enter(self.level + 1);
                        self.clock += self.course.time;
                    }
                }
            }
            Mode::Over | Mode::Won => if self.t > 7.0 || (go && self.t > 1.0) { self.high = self.high.max(self.score); self.title(); },
        }
        let want = self.aim();
        self.cam += (want - self.cam) * (dt * 6.0).min(1.0);
    }

    /// The course and all that moves on it.
    fn scene(&self, f: &mut Frame) {
        let (c, cam) = (&self.course, (self.cam as i32).clamp(0, self.course.ih - H));
        let from = (cam * W) as usize;
        f.px.copy_from_slice(&c.img[from..from + (W * H) as usize]);
        let mut v = View { c, cam, f };
        let shown = match self.gone { None => true, Some((why, _)) => why == Lost::Fell };
        // Shadows and pools first: they lie on the floor.
        let shade = |v: &mut View, b: &Ball, r: f32| {
            let Some((h, _, _)) = c.floor(b.x, b.y) else { return };
            if h > b.z + 0.05 { return; }
            let k = 0.5 / (1.0 + (b.z - h) * 0.8);
            v.flat(b.x + 0.1, b.y + 0.03, h, r * 1.05, |p, q, _| scale(p, 1.0 - k * (1.0 - q * q)));
        };
        if shown { shade(&mut v, &self.ball, R); }
        for foe in self.foes.iter().filter(|f| f.alive) {
            match foe.kind {
                Kind::Acid => {
                    let t = self.time;
                    v.flat(foe.b.x, foe.b.y, foe.b.z, 0.62, |p, q, a| {
                        let edge = 0.8 + 0.12 * (a * 3.0 + t * 4.0).sin() + 0.06 * (a * 5.0 - t * 6.0).sin();
                        if q > edge { p } else if q > edge - 0.14 { 0x1c7c24 } else if (q - 0.35).abs() < 0.08 && a < -1.0 && a > -2.4 { 0xc8ffb0 } else { 0x44e040 }
                    });
                }
                Kind::Steelie => shade(&mut v, &foe.b, R),
                Kind::Slinky => shade(&mut v, &Ball { z: foe.dest[2], ..foe.b }, 0.22),
            }
        }
        // Then the balls, the far ones first.
        let mut order: Vec<(f32, usize)> = self.foes.iter().enumerate().filter(|(_, f)| f.alive && f.kind != Kind::Acid)
            .map(|(i, f)| (f.b.x + f.b.y, i + 1)).collect();
        if shown { order.push((self.ball.x + self.ball.y, 0)); }
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, i) in order {
            if i == 0 {
                let blink = self.safe > 0.5 && (self.time * 12.0) as i32 % 2 == 0;
                v.ball(&self.ball, R, BLUE, PALE, if blink { 0.45 } else { 1.0 });
                // Stars round a dazed marble.
                for k in 0..if self.daze > 0.0 { 3 } else { 0 } {
                    let a = self.time * 7.0 + k as f32 * TAU / 3.0;
                    v.speck([self.ball.x + a.cos() * 0.3, self.ball.y + a.sin() * 0.3, self.ball.z + 0.85], 2, GOLD, 1.0);
                }
                continue;
            }
            let foe = &self.foes[i - 1];
            if foe.kind == Kind::Steelie {
                v.ball(&foe.b, R, 0x30303a, 0x565664, 1.0);
                continue;
            }
            // A spring: five coils, the top one first over in a hop.
            let e = (foe.t / 0.5).min(1.0);
            for k in 0..5 {
                let ek = (e * 1.5 - (4 - k) as f32 * 0.125).clamp(0.0, 1.0);
                let at = [lerp(foe.from[0], foe.dest[0], ek), lerp(foe.from[1], foe.dest[1], ek),
                    lerp(foe.from[2], foe.dest[2], ek) + 0.6 * (ek * PI).sin() + k as f32 * 0.09];
                v.ball(&Ball { x: at[0], y: at[1], z: at[2], ..foe.b }, 0.2, if k == 4 { 0x70f060 } else { 0x2ca838 }, 0x186420, 1.0);
            }
        }
        for p in &self.fx { v.speck(p.at, p.size, p.color, (1.0 - p.age / p.life) * 1.5); }
        for p in &self.pops {
            let (x, y) = c.spot(p.at[0], p.at[1], p.at[2]);
            v.f.text_centered(x as i32, y as i32 - cam, &p.text, p.color, true, 1);
        }
    }

    /// The score, the clock and the words over a game.
    fn hud(&self, f: &mut Frame) {
        let cx = W / 2;
        label(f, 8 + Frame::text_width("000000", true, 2) / 2, 8, &format!("{:06}", self.score), WHITE, 2);
        label(f, W - 8 - Frame::text_width("HIGH 000000", true, 1) / 2, 8, &format!("HIGH {:06}", self.high.max(self.score)), TEXT, 1);
        let secs = self.clock.ceil() as i32;
        let low = secs <= 10 && self.mode == Mode::Play;
        if !low || (self.time * 4.0) as i32 % 2 == 0 { label(f, cx, 6, &secs.to_string(), if low { RED } else { GOLD }, 3); }
        let name = format!("{} {}", self.level + 1, self.course.name);
        label(f, 8 + Frame::text_width(&name, true, 1) / 2, 28, &name, TEXT, 1);
        match self.mode {
            Mode::Ready => {
                label(f, cx, 120, &format!("COURSE {}", self.level + 1), GOLD, 2);
                label(f, cx, 146, self.course.name, WHITE, 5);
            }
            Mode::Goal => {
                label(f, cx, 110, "GOAL", GOLD, 6);
                if self.t > 0.7 && self.bonus > 0 { label(f, cx, 176, &format!("BONUS {}", self.bonus), WHITE, 2); }
            }
            Mode::Over => {
                label(f, cx, 120, "OUT OF TIME", RED, 5);
                label(f, cx, 180, &format!("SCORE {:06}", self.score), WHITE, 2);
            }
            Mode::Won => {
                label(f, cx, 110, "ALL SIX COURSES", GOLD, 4);
                label(f, cx, 160, &format!("SCORE {:06}", self.score), WHITE, 3);
                if self.score > self.high { label(f, cx, 200, "A NEW HIGH SCORE", GOLD, 2); }
            }
            Mode::Play => {}
        }
    }

    /// The name, over the game playing itself.
    fn name(&self, f: &mut Frame) {
        f.dim(0.5);
        let cx = W / 2;
        f.text_centered(cx + 5, 61, "MARBLE", 0x081030, true, 11);
        f.text_centered(cx, 56, "MARBLE", 0x5c98ff, true, 11);
        f.text_centered(cx, 148, "A tribute to Marble Madness", TEXT, true, 2);
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(cx, 206, "SPACE TO ROLL", WHITE, true, 3); }
        f.text_centered(cx, 252, &format!("HIGH {:06}", self.high), GOLD, true, 2);
        for (i, line) in ["THE ARROWS ROLL THE MARBLE ALONG THE TILES", "RIGHT IS DOWN TO THE RIGHT    DOWN IS DOWN TO THE LEFT",
            "TWO AT ONCE ROLL IT STRAIGHT    P PAUSES    Q QUITS"].iter().enumerate() {
            f.text_centered(cx, 318 + i as i32 * 14, line, 0xa8b0c4, true, 1);
        }
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 8, VERSION, 0x8088a0);
    }
}

/// Words centred on a column, with a dark edge so they read over a course.
fn label(f: &mut Frame, cx: i32, y: i32, s: &str, c: Rgb, size: i32) {
    f.text_centered(cx + size.min(2), y + size.min(2), s, 0x000008, true, size);
    f.text_centered(cx, y, s, c, true, size);
}

impl Game for Marble {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        let go = input.pressed(Key::Space) || input.pressed(Key::Enter);
        if self.demo {
            if go { self.begin(); return Flow::Continue; }
        } else if input.pressed(Key::Char('p')) && self.mode == Mode::Play {
            return Flow::Pause;
        }
        self.step((input.axis_x() as f32, input.axis_y() as f32), go, dt);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        self.scene(f);
        if self.demo { self.name(f); } else { self.hud(f); }
    }
}

fn marble() -> Marble {
    let mut game = Marble::new();
    game.audio = Audio::open();
    match std::env::var("MARBLE_START").ok().and_then(|s| s.trim().parse::<usize>().ok()) {
        Some(n) => { game.start = n.max(1) - 1; game.begin(); }
        None => game.audio.play_loop(1, &game.snd.title, 0.5),
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // MARBLE_BENCH=<frames> plays that many frames by itself with no
    // terminal and prints the time one takes.
    if let Ok(n) = std::env::var("MARBLE_BENCH") {
        bench(&mut Marble::new(), &Input::new(), Config { width: W, height: H, fps: 60 }, n.parse().unwrap_or(600));
        return;
    }
    let mut game = marble();
    run(&mut game, Config { width: W, height: H, fps: 60 });
}

#[cfg(target_arch = "wasm32")]
funkey::web!(marble(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
mod tests {
    use super::*;

    const PAL: Pal = Pal { top: 0xb0b0b0, line: 0x505050, left: 0x808080, right: 0x404040, rail: 0x909090 };
    const DT: f32 = 1.0 / 60.0;

    /// A game on a course of the test's own, with no foes but those it adds.
    fn on(c: Course) -> Marble {
        let mut game = Marble::new();
        (game.demo, game.calm, game.course) = (false, true, c);
        game.settle();
        (game.mode, game.clock) = (Mode::Play, 60.0);
        game
    }

    fn secs(game: &mut Marble, push: (f32, f32), secs: f32) {
        for _ in 0..(secs * 60.0) as usize { game.step(push, false, DT); }
    }

    #[test]
    fn every_course_fits_the_screen_and_has_a_way_to_its_goal() {
        for n in 0..COURSES {
            let c = course(n);
            assert!(c.ih >= H && c.ih < 3600, "{}: {} rows", c.name, c.ih);
            assert_eq!(c.img.len(), (W * c.ih) as usize);
            for (i, w) in c.way.iter().enumerate() {
                let f = c.floor(w.x, w.y).unwrap_or_else(|| panic!("{}: point {} of the way is over nothing", c.name, i));
                assert!(f.2 != Surf::Rail, "{}: point {} of the way is on a rail", c.name, i);
            }
            let end = c.way.last().unwrap();
            assert_eq!(c.floor(end.x, end.y).unwrap().2, Surf::Goal, "{}", c.name);
            for s in &c.spawns { assert!(c.floor(s.x, s.y).is_some() && c.floor(s.to.0, s.to.1).is_some(), "{}: a foe over nothing", c.name); }
        }
    }

    #[test]
    fn two_tiles_of_a_slope_meet_at_one_height() {
        let c = Plan::new().rails(false).run(6, 3.0).done("T", 9.0, PAL);
        let w = c.way[2];
        let (a, b) = (c.floor(w.x + 0.4999, w.y).unwrap(), c.floor(w.x + 0.5001, w.y).unwrap());
        assert!((a.0 - b.0).abs() < 0.001, "{} and {}", a.0, b.0);
        assert!((a.1.0 + 0.5).abs() < 0.001, "the slope is {}", a.1.0);
    }

    #[test]
    fn a_marble_at_rest_on_the_flat_stays_and_one_on_a_slope_rolls_down() {
        let mut game = on(Plan::new().wide(5).rails(false).run(8, 0.0).done("T", 9.0, PAL));
        let x = game.ball.x;
        secs(&mut game, (0.0, 0.0), 1.0);
        assert!((game.ball.x - x).abs() < 0.001 && !game.ball.air);
        let mut game = on(Plan::new().wide(5).rails(false).run(12, 4.0).done("T", 9.0, PAL));
        let (x, z) = (game.ball.x, game.ball.z);
        secs(&mut game, (0.0, 0.0), 1.0);
        assert!(game.ball.x > x + 1.0 && game.ball.z < z - 0.3, "it went from {} to {}", x, game.ball.x);
    }

    #[test]
    fn the_keys_push_along_the_tiles_and_two_keys_push_no_harder() {
        let flat = || on(Plan::new().wide(9).rails(false).run(9, 0.0).done("T", 9.0, PAL));
        let (mut a, mut b) = (flat(), flat());
        secs(&mut a, (1.0, 0.0), 0.4);
        secs(&mut b, (1.0, 1.0), 0.4);
        assert!(a.ball.vx > 2.0 && a.ball.vy.abs() < 0.001);
        assert!((b.ball.speed() - a.ball.speed()).abs() < 0.01, "{} and {}", b.ball.speed(), a.ball.speed());
    }

    #[test]
    fn a_rail_turns_the_marble_back() {
        let mut game = on(Plan::new().run(6, 0.0).done("T", 9.0, PAL));
        secs(&mut game, (0.0, 1.0), 2.0);
        let f = game.course.floor(game.ball.x, game.ball.y).unwrap();
        assert!(f.2 == Surf::Plain && !game.ball.air && game.gone.is_none(), "the marble is on {:?}", f.2);
    }

    #[test]
    fn a_marble_over_the_edge_falls_and_another_is_set_down_on_the_way() {
        let mut game = on(Plan::new().rails(false).run(6, 0.0).done("T", 9.0, PAL));
        secs(&mut game, (0.0, 1.0), 1.5);
        assert_eq!(game.gone.map(|g| g.0), Some(Lost::Fell));
        let clock = game.clock;
        secs(&mut game, (0.0, 0.0), 2.5);
        assert!(game.gone.is_none() && !game.ball.air && game.course.floor(game.ball.x, game.ball.y).is_some());
        assert!(game.clock < clock - 2.0, "the fall cost no time");
    }

    #[test]
    fn a_long_fall_breaks_the_marble_and_a_short_one_does_not() {
        let step = |drop: f32| {
            let mut game = on(Plan::new().wide(5).rails(false).run(3, 0.0).gap(0, drop).run(6, 0.0).done("T", 9.0, PAL));
            secs(&mut game, (1.0, 0.0), 1.6);
            game
        };
        let (short, long) = (step(1.2), step(4.5));
        assert!(short.gone.is_none() && short.ball.z < -1.0, "a short fall: {:?}", short.gone);
        assert_eq!(long.gone.map(|g| g.0), Some(Lost::Broke));
    }

    #[test]
    fn a_marble_leaps_a_gap_at_speed_and_drops_into_it_when_slow() {
        let leap = |push: f32| {
            let mut game = on(Plan::new().wide(3).run(8, 2.5).run(2, -0.5).gap(2, 1.3).run(8, 0.0).done("T", 9.0, PAL));
            for _ in 0..240 {
                game.step((push, 0.0), false, DT);
                if game.gone.is_some() || game.ball.x > 14.0 { break; }
            }
            game
        };
        let (fast, slow) = (leap(1.0), leap(-0.8));
        assert!(fast.gone.is_none() && fast.ball.x > 14.0, "the fast marble: {:?} at {}", fast.gone, fast.ball.x);
        assert!(slow.gone.is_some(), "the slow marble got over, to {}", slow.ball.x);
    }

    #[test]
    fn a_steel_ball_shoves_the_marble_and_can_be_knocked_off() {
        let mut game = on(Plan::new().wide(3).rails(false).run(9, 0.0).done("T", 9.0, PAL));
        let w = game.course.way[4];
        let z = game.course.floor(w.x, w.y).unwrap().0;
        game.foes.push(Foe { kind: Kind::Steelie, b: Ball::at(w.x, w.y + 1.0, z), home: (w.x, w.y + 1.0), to: (0.0, 0.0), t: 0.0, from: [0.0; 3],
            dest: [0.0; 3], alive: true, hit: 0.0 });
        (game.ball.x, game.ball.y, game.ball.vy) = (w.x, w.y - 0.5, 8.0);
        for _ in 0..90 {
            game.step((0.0, 0.0), false, DT);
            if !game.foes[0].alive { break; }
        }
        assert!(!game.foes[0].alive, "the steel ball is still at {}, {}", game.foes[0].b.x, game.foes[0].b.y);
        assert_eq!(game.score, 1000);
    }

    #[test]
    fn a_spring_eats_the_marble_and_acid_melts_it() {
        for (kind, why) in [(Kind::Slinky, Lost::Eaten), (Kind::Acid, Lost::Melted)] {
            let mut game = on(Plan::new().wide(3).run(9, 0.0).done("T", 9.0, PAL));
            let w = game.course.way[3];
            let z = game.course.floor(w.x, w.y).unwrap().0;
            game.foes.push(Foe { kind, b: Ball::at(w.x, w.y, z), home: (w.x, w.y), to: (w.x, w.y), t: 0.0, from: [w.x, w.y, z], dest: [w.x, w.y, z],
                alive: true, hit: 0.0 });
            for _ in 0..240 {
                game.step((1.0, 0.0), false, DT);
                if game.gone.is_some() { break; }
            }
            assert_eq!(game.gone.map(|g| g.0), Some(why));
            // The next marble cannot be taken at once.
            secs(&mut game, (0.0, 0.0), 2.2);
            assert!(game.gone.is_none());
        }
    }

    #[test]
    fn the_goal_pays_for_the_time_left_and_the_next_course_adds_its_own() {
        let mut game = Marble::new();
        (game.calm, game.auto) = (true, true);
        game.begin();
        while game.mode != Mode::Goal { game.step((0.0, 0.0), false, DT); assert!(game.time < 60.0, "the first goal was never reached"); }
        let (left, due) = (game.clock, game.bonus);
        assert_eq!(due, 1000 + left as u32 * 100);
        while game.level == 0 { game.step((0.0, 0.0), false, DT); }
        assert_eq!(game.score, due);
        assert!((game.clock - left - game.course.time).abs() < 0.01 && game.mode == Mode::Ready);
    }

    #[test]
    fn the_game_ends_when_the_clock_runs_out() {
        let mut game = Marble::new();
        game.begin();
        game.clock = 3.0;
        secs(&mut game, (0.0, 0.0), 5.0);
        assert_eq!(game.mode, Mode::Over);
        secs(&mut game, (0.0, 0.0), 8.0);
        assert!(game.demo, "the name did not come back");
    }

    #[test]
    fn the_game_plays_every_course_by_itself_with_time_to_spare() {
        for n in 0..COURSES {
            let mut game = Marble::new();
            (game.calm, game.auto, game.start) = (true, true, n);
            game.begin();
            let mut lost = 0;
            while game.mode != Mode::Goal {
                let was = game.gone.is_some();
                game.step((0.0, 0.0), false, DT);
                if game.gone.is_some() && !was { lost += 1; }
                assert!(game.mode != Mode::Over, "{}: the clock ran out at point {} of {}", game.course.name, game.at, game.course.way.len());
            }
            let used = game.course.time - game.clock;
            assert!(lost == 0 && used < game.course.time * 0.7, "{}: {} marbles lost, {:.1} of {} seconds used", game.course.name, lost, used,
                game.course.time);
        }
    }

    /// Not a test: the game plays itself, to see how the courses go and
    /// what they look like. `MARBLE_MAP=<folder>` saves each course's whole
    /// picture; `MARBLE_FILM=<folder>,<course>,<every>,<frames>[,calm]`
    /// saves every so-many-th frame of one game; with neither, each course
    /// is played twenty times and how it went is printed.
    #[test]
    #[ignore]
    fn the_bot_plays() {
        if let Ok(folder) = std::env::var("MARBLE_MAP") {
            for n in 0..COURSES {
                let c = course(n);
                let mut f = Frame::new(W, c.ih);
                f.px.copy_from_slice(&c.img);
                std::fs::write(format!("{}/map-{}.ppm", folder, n + 1), f.to_ppm()).unwrap();
            }
            return;
        }
        if let Ok(film) = std::env::var("MARBLE_FILM") {
            let p: Vec<&str> = film.split(',').collect();
            let (every, frames): (usize, usize) = (p[2].parse().unwrap(), p[3].parse().unwrap());
            let (mut game, mut f) = (Marble::new(), Frame::new(W, H));
            (game.auto, game.calm, game.start) = (true, p.len() > 4, p[1].parse::<usize>().unwrap() - 1);
            game.begin();
            for n in 1..=every * frames {
                game.step((0.0, 0.0), false, DT);
                if n % every == 0 {
                    game.draw(&mut f);
                    std::fs::write(format!("{}/f-{:04}.ppm", p[0], n / every), f.to_ppm()).unwrap();
                }
            }
            return;
        }
        for n in 0..COURSES {
            let (mut times, mut how) = (Vec::new(), std::collections::BTreeMap::new());
            for seed in 0..20 {
                let mut game = Marble::new();
                (game.auto, game.start, game.rng) = (true, n, Rng::new(seed));
                game.begin();
                game.clock = 600.0;
                while game.mode != Mode::Goal && game.clock > 0.5 {
                    let was = game.gone.is_some();
                    game.step((0.0, 0.0), false, DT);
                    if let (Some((why, _)), false) = (game.gone, was) { *how.entry(format!("{:?} at {}", why, game.back)).or_insert(0) += 1; }
                }
                times.push((600.0 - game.clock) as u32);
            }
            times.sort();
            println!("{} ({} s, {} points): seconds {:?}, lost {:?}", course(n).name, course(n).time, course(n).way.len(), times, how);
        }
    }
}
