//! kart: eight karts, four tracks and a box of tricks, in the spirit of
//! Super Mario Kart (Nintendo, 1992). The ground is flat and painted the
//! way that game painted it: every pixel under the horizon looks up its
//! own spot on the track. On it stand the karts, the barriers and the
//! boxes as 3D models, and the trees as pictures. A cup is four races of
//! three laps, and the best finishes take the most points.
//!
//!     cargo run --release --example kart
//!
//! Up is the gas, Down the brake, Left and Right steer. Space hops, and
//! held through a bend it drifts: let go when the sparks turn blue or
//! orange for a push. X (or Z, F, Enter) uses the item from a box.
//! P pauses, Q quits. Where the terminal reports no key releases the gas
//! is always on, and a drift cannot be held.
//!
//! `KART_START=<track>[,<class>]` starts on a track at once,
//! `KART_AUTO=1` lets the computer drive your kart, and
//! `KART_BENCH=<frames>` times the game with no terminal. Everything
//! here is new: the tracks, the karts, the items and the sounds.

use funkey::raster::blend;
use funkey::*;
use std::f32::consts::{PI, TAU};

const W: i32 = 640;
const H: i32 = 400;
const GAME: &str = "kart";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";
const FOCAL: f32 = W as f32 * 0.62;
/// The world is this many metres a side, and as many cells.
const SIZE: usize = 512;
/// A cell with no track near it.
const NONE: u16 = u16::MAX;
const KARTS: usize = 8;
const LAPS: i32 = 3;
/// The striped strip beside the road, and the grass from it to the barrier.
const KERB: f32 = 1.2;
const VERGE: f32 = 6.0;
/// A kart is a circle this wide when it meets a barrier or another kart.
const RADIUS: f32 = 0.9;
/// How long a hop is in the air.
const HOP: f32 = 0.3;
/// A drift held this long pays a push, and this long a longer one.
const BLUE: f32 = 0.9;
const ORANGE: f32 = 2.0;
const POINTS: [u32; KARTS] = [10, 8, 6, 5, 4, 3, 2, 1];
const NAMES: [&str; KARTS] = ["YOU", "REX", "IVY", "OTTO", "MIRA", "DUKE", "NOVA", "ZIP"];
const PAINT: [Rgb; KARTS] = [0xe02828, 0x2c9c3c, 0xf0c020, 0x2860d8, 0xf070b0, 0xf08020, 0x8040c0, 0x30c0c8];
const SUIT: [Rgb; KARTS] = [0x2848b0, 0xf0f0f0, 0x283048, 0xf0f0f0, 0x6030a0, 0x283048, 0xf0c020, 0xf0f0f0];
const TEXT: Rgb = 0xf8f8f8;
const GOLD: Rgb = 0xffd040;

fn turn(a: f32) -> f32 { (a + PI).rem_euclid(TAU) - PI }

fn mixf(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

/// How far a pattern `period` metres long has melted into its average,
/// where one pixel covers `far` metres of ground.
fn blur(far: f32, period: f32) -> f32 { ((far * (1.0 / period) - 0.3) * (1.0 / 0.45)).clamp(0.0, 1.0) }

/// How fast a kart can turn at a speed, in radians a second.
fn rate_at(v: f32) -> f32 { 1.75 - 0.75 * (v / 30.0).clamp(0.0, 1.0) }

fn clock(t: f32) -> String { format!("{}:{:05.2}", (t / 60.0) as u32, t % 60.0) }

/// The angle of a line of sight round the compass, `atan2(x, z)`, to
/// within a hundred-thousandth: the sky asks for it at every pixel.
fn bearing(x: f32, z: f32) -> f32 {
    let (ax, az) = (x.abs(), z.abs());
    let t = ax.min(az) / ax.max(az).max(1e-9);
    let t2 = t * t;
    let a = t * (0.999_866 + t2 * (-0.330_299_5 + t2 * (0.180_141 + t2 * (-0.085_133 + t2 * 0.020_835_1))));
    let a = if ax > az { PI / 2.0 - a } else { a };
    let a = if z < 0.0 { PI - a } else { a };
    if x < 0.0 { -a } else { a }
}

/// A colour between two, counted in whole numbers. The ground and the
/// sky ask for several at every pixel, where `blend` would take most of
/// the frame.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let w = ((t * 256.0) as u32).min(256);
    let part = |mask: u32| (((a & mask) * (256 - w) + (b & mask) * w) >> 8) & mask;
    part(0xff00ff) | part(0x00ff00)
}

/// `exp(-x)` for a fog, near enough and with no division: `(1 - x/32)`
/// to the power of 32.
fn fade(x: f32) -> f32 {
    let b = (1.0 - x * (1.0 / 32.0)).max(0.0);
    let b = b * b * b * b;
    let b = b * b * b * b;
    b * b
}

/// What the ground and the sky are painted from: smooth noise that
/// repeats every 64 steps, and how high the near and the far hills stand
/// at 1024 points round the compass. Both are read at every pixel, so
/// their sizes are fixed and no index needs a check.
struct Paint { grain: [f32; 4096], ridge: [[f32; 2]; 1024] }

impl Paint {
    fn new() -> Paint {
        let mut r = Rng::new(7);
        let mut p = Paint { grain: [0.0; 4096], ridge: [[0.0; 2]; 1024] };
        for g in &mut p.grain { *g = r.float(); }
        for (i, h) in p.ridge.iter_mut().enumerate() {
            let a = (i as f32 / 1024.0 - 0.5) * TAU;
            *h = [0.3 + 0.15 * (a * 5.0 + 1.0).sin() + 0.05 * (a * 13.0).sin(), 0.62 + 0.26 * (a * 3.0).sin() + 0.12 * (a * 11.0 + 2.0).sin()];
        }
        p
    }

    fn at(&self, x: f32, y: f32) -> f32 {
        // Counted in 256ths: one cast a coordinate, and a shift that
        // rounds down below zero as well.
        let (fx, fy) = ((x * 256.0) as i32, (y * 256.0) as i32);
        let (ix, iy, tx, ty) = (fx >> 8, fy >> 8, (fx & 255) as f32 * (1.0 / 256.0), (fy & 255) as f32 * (1.0 / 256.0));
        let g = |a: i32, b: i32| self.grain[(((b & 63) << 6) | (a & 63)) as usize];
        let (p, q) = (mixf(g(ix, iy), g(ix + 1, iy), tx), mixf(g(ix, iy + 1), g(ix + 1, iy + 1), tx));
        mixf(p, q, ty)
    }

    /// The near and the far skyline at a bearing, as parts of a theme's rise.
    fn hills(&self, a: f32) -> [f32; 2] {
        let at = (a * (1.0 / TAU) + 0.5) * 1024.0;
        let (i, t) = (at as usize, at - (at as usize) as f32);
        let (p, q) = (self.ridge[i & 1023], self.ridge[(i + 1) & 1023]);
        [mixf(p[0], q[0], t), mixf(p[1], q[1], t)]
    }
}

/// What a track looks like and how it drives.
struct Theme {
    /// The sky overhead and at the horizon.
    sky: [Rgb; 2],
    /// The far and the near line of hills, and how high they stand.
    hills: [Rgb; 2],
    rise: f32,
    road: [Rgb; 2],
    line: Rgb,
    kerb: [Rgb; 2],
    verge: [Rgb; 2],
    land: [Rgb; 2],
    wall: [Rgb; 2],
    /// How fast a kart's travel follows its nose: low is ice.
    grip: f32,
    /// What is left of the top speed off the road.
    drag: f32,
    tree: usize,
    fog: f32,
}

static THEMES: [Theme; 4] = [
    Theme { sky: [0x3c78d8, 0xb8d8f0], hills: [0x7ca8c8, 0x4c8c48], rise: 0.11, road: [0x62626e, 0x565662], line: 0xf0f0f0, kerb: [0xd83030, 0xf0f0f0],
            verge: [0x48a030, 0x5cb440], land: [0x2c7828, 0x3c8c30], wall: [0xe8e8e8, 0xd03030], grip: 10.0, drag: 0.5, tree: 0, fog: 300.0 },
    Theme { sky: [0x2890e0, 0xc8ecf8], hills: [0x2068b8, 0x30a0a0], rise: 0.05, road: [0x8c7c64, 0x80705a], line: 0xfff8e0, kerb: [0x2878d0, 0xf8f0d8],
            verge: [0xe8d498, 0xd8c484], land: [0xd8c080, 0xc4ac6c], wall: [0xf8f0d8, 0x2878d0], grip: 9.0, drag: 0.45, tree: 1, fog: 340.0 },
    Theme { sky: [0x6088c0, 0xdce8f4], hills: [0xb8c8dc, 0xe8f0f8], rise: 0.16, road: [0xa8c4dc, 0x98b4d0], line: 0x4070b0, kerb: [0x3060b0, 0xf8f8ff],
            verge: [0xf0f4fc, 0xdce6f4], land: [0xd8e4f0, 0xc4d4ea], wall: [0x3060b0, 0xf0f0f8], grip: 3.4, drag: 0.55, tree: 2, fog: 240.0 },
    Theme { sky: [0x503878, 0xf8a860], hills: [0xa85840, 0x783828], rise: 0.14, road: [0x5c4c48, 0x504240], line: 0xf0d8a0, kerb: [0xe07020, 0x403030],
            verge: [0xc88048, 0xb47040], land: [0xa86838, 0x945830], wall: [0x403030, 0xe07020], grip: 10.0, drag: 0.5, tree: 3, fog: 280.0 },
];

/// A track as drawn up: the points its middle runs through, in metres,
/// where the rows of boxes stand (as parts of a lap), and the push pads
/// (a part of a lap, and metres right of the middle).
struct Plan { name: &'static str, theme: usize, half: f32, pts: &'static [(f32, f32)], boxes: &'static [f32], pads: &'static [(f32, f32)] }

static PLANS: [Plan; 4] = [
    Plan { name: "MEADOW RING", theme: 0, half: 8.0, boxes: &[0.2, 0.62], pads: &[(0.42, 0.0)],
           pts: &[(170.0, 150.0), (280.0, 144.0), (370.0, 176.0), (400.0, 256.0), (346.0, 326.0), (266.0, 310.0), (200.0, 346.0), (116.0, 318.0), (88.0, 232.0), (104.0, 166.0)] },
    Plan { name: "SANDY BAY", theme: 1, half: 8.0, boxes: &[0.14, 0.48, 0.8], pads: &[(0.3, -3.0), (0.68, 3.0)],
           pts: &[(140.0, 380.0), (260.0, 388.0), (370.0, 360.0), (410.0, 270.0), (350.0, 200.0), (390.0, 120.0), (320.0, 70.0), (230.0, 100.0), (230.0, 190.0), (160.0, 225.0),
                  (85.0, 240.0), (65.0, 315.0)] },
    Plan { name: "FROST PASS", theme: 2, half: 9.0, boxes: &[0.18, 0.5, 0.78], pads: &[(0.36, 0.0), (0.9, 0.0)],
           pts: &[(256.0, 90.0), (370.0, 100.0), (430.0, 180.0), (370.0, 260.0), (400.0, 350.0), (330.0, 420.0), (230.0, 400.0), (220.0, 300.0), (150.0, 268.0), (88.0, 232.0),
                  (72.0, 160.0), (135.0, 104.0)] },
    Plan { name: "DUSTY MESA", theme: 3, half: 7.0, boxes: &[0.1, 0.36, 0.62, 0.86], pads: &[(0.25, 0.0), (0.74, 2.5)],
           pts: &[(80.0, 280.0), (84.0, 170.0), (150.0, 90.0), (240.0, 110.0), (250.0, 210.0), (330.0, 230.0), (350.0, 130.0), (420.0, 80.0), (450.0, 180.0), (420.0, 290.0),
                  (320.0, 340.0), (250.0, 410.0), (150.0, 390.0)] },
];

/// A closed curve through the points.
fn spline(p: &[(f32, f32)]) -> Vec<[f32; 2]> {
    let n = p.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (p0, p1, p2, p3) = (p[(k + n - 1) % n], p[k], p[(k + 1) % n], p[(k + 2) % n]);
        for j in 0..24 {
            let t = j as f32 / 24.0;
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t + (3.0 * b - a - 3.0 * c + d) * t * t * t);
            out.push([f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)]);
        }
    }
    out
}

/// The same closed line as points a metre apart.
fn even(q: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let n = q.len();
    let seg: Vec<f32> = (0..n).map(|i| (q[(i + 1) % n][0] - q[i][0]).hypot(q[(i + 1) % n][1] - q[i][1])).collect();
    let len: f32 = seg.iter().sum();
    let m = len.round() as usize;
    let (mut i, mut done) = (0, 0.0f32);
    (0..m).map(|k| {
        let d = k as f32 * len / m as f32;
        while i + 1 < n && done + seg[i] < d { done += seg[i]; i += 1; }
        let (a, b, t) = (q[i], q[(i + 1) % n], if seg[i] > 0.0 { (d - done) / seg[i] } else { 0.0 });
        [mixf(a[0], b[0], t), mixf(a[1], b[1], t)]
    }).collect()
}

/// Every point as the mean of its neighbours: it rounds the corners.
fn rounded(q: &[[f32; 2]], reach: usize) -> Vec<[f32; 2]> {
    let n = q.len();
    (0..n).map(|i| {
        let (mut x, mut z) = (0.0, 0.0);
        for k in 0..=2 * reach { let p = q[(i + n + k - reach) % n]; x += p[0]; z += p[1]; }
        [x / (2 * reach + 1) as f32, z / (2 * reach + 1) as f32]
    }).collect()
}

/// A quad that faces the point `out`, with one light all over.
fn face(m: &mut Model, p: [V3; 4], out: V3, mat: Mat, lit: f32) {
    let n = p[1].sub(p[0]).cross(p[2].sub(p[0]));
    let p = if n.dot(out.sub(p[0])) < 0.0 { [p[3], p[2], p[1], p[0]] } else { p };
    let v = |q: V3| Vert::new(q, 0.0, 0.0, lit);
    m.quad(v(p[0]), v(p[1]), v(p[2]), v(p[3]), mat);
}

/// Light a model from above, the same from both sides, and bound it.
fn shade(m: &mut Model) {
    m.relight(|_, n| { let k = 0.62 + 0.4 * n.y.max(0.0) + 0.14 * n.y.min(0.0) + 0.08 * n.z.abs(); [k, k, k] });
    m.bound();
}

/// The pictures the scene paints with.
struct Tex { puff: usize, check: usize, chest: usize, trees: [usize; 4] }

/// How wide a tree is to its height, by kind.
const SLIM: [f32; 4] = [1.0, 1.0, 0.5, 0.5];

fn textures(scene: &mut Scene) -> Tex {
    let puff = scene.texture(Texture::from_fn(16, 16, |x, y| {
        let r = (x as f32 - 7.5).hypot(y as f32 - 7.5);
        if r < 7.6 { blend(0xffffff, 0xa8a8a8, (r / 7.6).powi(2)) } else { CUTOUT }
    }));
    let check = scene.texture(Texture::from_fn(16, 16, |x, y| if (x / 8 + y / 8) & 1 == 0 { 0xf4f4f4 } else { 0x181818 }));
    // The box's mark sits on the picture's corner, which is the middle of
    // each side of the box.
    let chest = scene.texture(Texture::from_fn(32, 32, |x, y| {
        let (u, v) = (((x + 16) % 32) as i32 - 16, ((y + 16) % 32) as i32 - 16);
        if u.abs().max(v.abs()) >= 13 { 0x8c4c10 } else if u.abs() + v.abs() < 8 { 0xffffff } else { blend(0xffd040, 0xf06020, (u + v + 26) as f32 / 52.0) }
    }));
    let leaf = scene.texture(Texture::from_fn(64, 64, |x, y| {
        let (dx, dy) = (x as f32 - 31.5, y as f32 - 24.0);
        if dx.hypot(dy) < 21.0 + 2.5 * (dy.atan2(dx) * 7.0).sin() { blend(0x1c6420, 0x5cb444, ((-dx - dy) / 44.0 + 0.5).clamp(0.0, 1.0)) }
        else if (x as i32 - 32).abs() < 4 && y > 38 { if x < 32 { 0x6c4424 } else { 0x503018 } }
        else { CUTOUT }
    }));
    let palm = scene.texture(Texture::from_fn(64, 64, |x, y| {
        let (fx, fy) = (x as f32, y as f32);
        let (dx, dy) = (fx - 32.0, fy - 18.0);
        let r = dx.hypot(dy);
        if r < 4.0 || (r < 27.0 && (dy.atan2(dx) * 3.5 + 0.4).sin().abs() > 0.5 + r / 60.0 && dy < 15.0) {
            return blend(0x1c7830, 0x74c44c, (0.6 - dy / 40.0).clamp(0.0, 1.0));
        }
        let mid = 32.0 + 5.0 * ((fy - 18.0) / 46.0 * 2.2).sin();
        if fy > 18.0 && (fx - mid).abs() < 2.5 { if (y / 4) % 2 == 0 { 0x8c6434 } else { 0x74502c } } else { CUTOUT }
    }));
    let pine = scene.texture(Texture::from_fn(32, 64, |x, y| {
        let (dx, fy) = ((x as f32 - 15.5).abs(), y as f32);
        for tier in 0..3 {
            let (top, foot, wide) = (4.0 + tier as f32 * 14.0, 24.0 + tier as f32 * 15.0, 8.0 + tier as f32 * 3.5);
            if fy >= top && fy < foot && dx < (fy - top) / (foot - top) * wide {
                return if fy - top < 4.0 + dx * 0.5 { 0xf4f8ff } else { blend(0x144c2c, 0x2c7c44, x as f32 / 32.0) };
            }
        }
        if fy >= 54.0 && dx < 2.5 { 0x5c3c20 } else { CUTOUT }
    }));
    let cactus = scene.texture(Texture::from_fn(32, 64, |x, y| {
        let (x, y) = (x as i32, y as i32);
        let body = ((x - 16).abs() < 4 && y > 8) || ((6..=12).contains(&x) && (34..40).contains(&y)) || ((5..9).contains(&x) && (22..40).contains(&y))
            || ((20..=26).contains(&x) && (26..32).contains(&y)) || ((23..27).contains(&x) && (14..32).contains(&y));
        if !body { CUTOUT } else if x % 3 == 0 { 0x2c6c30 } else { 0x48984c }
    }));
    Tex { puff, check, chest, trees: [leaf, palm, pine, cactus] }
}

/// A track as it is driven and drawn.
struct Track {
    plan: &'static Plan,
    /// The middle of the road, a point every `step` metres, the way the
    /// road runs at each, and how hard it bends there: more than zero is
    /// a bend to the right.
    pts: Vec<[f32; 2]>,
    dir: Vec<[f32; 2]>,
    curv: Vec<f32>,
    /// For every cell of the world, the point of the middle nearest it.
    near: Vec<u16>,
    step: f32,
    len: f32,
    half: f32,
    /// How far from the middle the barrier stands.
    limit: f32,
    walls: Vec<Model>,
    trees: Vec<(V3, f32)>,
    crates: Vec<[f32; 2]>,
    /// Where each push pad begins along the lap, and its middle.
    pads: Vec<(f32, f32)>,
    /// The track on the small map: its dots, and how the world maps to it.
    dots: Vec<(i32, i32)>,
    chart: (f32, f32, f32),
}

/// The small map's place and size on the screen.
const MAP: (i32, i32, f32) = (W - 98, 10, 88.0);

impl Track {
    fn build(plan: &'static Plan, tex: &Tex) -> Track {
        let pts = even(&rounded(&rounded(&even(&spline(plan.pts)), 14), 14));
        let n = pts.len();
        let seg = |i: usize| (pts[(i + 1) % n][0] - pts[i][0]).hypot(pts[(i + 1) % n][1] - pts[i][1]);
        let len: f32 = (0..n).map(seg).sum();
        let step = len / n as f32;
        let dir: Vec<[f32; 2]> = (0..n).map(|i| {
            let (a, b) = (pts[(i + n - 1) % n], pts[(i + 1) % n]);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-6);
            [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
        }).collect();
        let way = |i: usize| dir[i][0].atan2(dir[i][1]);
        let curv: Vec<f32> = (0..n).map(|i| turn(way((i + 1) % n) - way((i + n - 1) % n)) / (2.0 * step)).collect();
        let limit = plan.half + KERB + VERGE;
        // Every cell learns which point of the middle is nearest.
        let reach = (limit + 16.0) as i32;
        let mut near = vec![NONE; SIZE * SIZE];
        let mut best = vec![f32::MAX; SIZE * SIZE];
        for (i, p) in pts.iter().enumerate() {
            let (cx, cz) = (p[0] as i32, p[1] as i32);
            for z in (cz - reach).max(0)..=(cz + reach).min(SIZE as i32 - 1) {
                for x in (cx - reach).max(0)..=(cx + reach).min(SIZE as i32 - 1) {
                    let d = (x as f32 + 0.5 - p[0]).powi(2) + (z as f32 + 0.5 - p[1]).powi(2);
                    let k = z as usize * SIZE + x as usize;
                    if d < best[k] && d < (reach * reach) as f32 { best[k] = d; near[k] = i as u16; }
                }
            }
        }
        let th = &THEMES[plan.theme];
        let off = |i: usize, lat: f32, y: f32| V3::new(pts[i][0] + dir[i][1] * lat, y, pts[i][1] - dir[i][0] * lat);
        // The barrier, in pieces short enough to be skipped when off screen.
        let mut walls = Vec::new();
        for c in (0..n).step_by(8) {
            let mut m = Model::new();
            for i in c..(c + 8).min(n) {
                let j = (i + 1) % n;
                for side in [-limit, limit] {
                    let quad = [off(i, side, 0.0), off(j, side, 0.0), off(j, side, 0.8), off(i, side, 0.8)];
                    let col = Mat::Flat(th.wall[(i / 2) & 1]);
                    face(&mut m, quad, off(i, 0.0, 0.4), col, 0.95);
                    face(&mut m, quad, off(i, side * 2.0, 0.4), col, 0.7);
                }
            }
            m.bound();
            walls.push(m);
        }
        // The arch over the line.
        let post = plan.half + KERB + 0.8;
        let mut arch = Model::new();
        for side in [-post, post] {
            arch.extend(&Model::block(V3::new(side - 0.3, 0.0, -0.3), V3::new(side + 0.3, 5.8, 0.3), Mat::Flat(0xd0d0d8), 1.0, 99.0), &M4::identity());
        }
        arch.extend(&Model::block(V3::new(-post, 4.4, -0.25), V3::new(post, 5.8, 0.25), Mat::Tex(tex.check), 1.4, 99.0), &M4::identity());
        shade(&mut arch);
        let mut over = Model::new();
        over.extend(&arch, &M4::rotate_y(way(0)).then(&M4::translate(V3::new(pts[0][0], 0.0, pts[0][1]))));
        over.bound();
        walls.push(over);
        let spot = |f: f32| (f * n as f32) as usize % n;
        let crates = plan.boxes.iter().flat_map(|&f| [-0.56f32, -0.19, 0.19, 0.56].map(|lane| { let p = off(spot(f), lane * plan.half, 0.0); [p.x, p.z] })).collect();
        let pads = plan.pads.iter().map(|&(f, mid)| (spot(f) as f32 * step, mid)).collect();
        let (lo_x, hi_x) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])));
        let (lo_z, hi_z) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
        let k = MAP.2 / (hi_x - lo_x).max(hi_z - lo_z);
        let chart = (lo_x - (MAP.2 / k - (hi_x - lo_x)) / 2.0, hi_z + (MAP.2 / k - (hi_z - lo_z)) / 2.0, k);
        let mut t = Track { plan, pts, dir, curv, near, step, len, half: plan.half, limit, walls, trees: Vec::new(), crates, pads, dots: Vec::new(), chart };
        t.dots = (0..n).step_by(3).map(|i| t.dot(t.pts[i][0], t.pts[i][1])).collect();
        // Trees beyond the barrier, never where another stretch of road runs.
        let mut rng = Rng::new(plan.theme as u64 * 77 + 5);
        let mut i = 0;
        while i < n {
            for side in [-1.0f32, 1.0] {
                let p = t.off(i, side * (limit + rng.range(2.5, 14.0)));
                let clear = t.at(p.x, p.z).map(|(lat, _, _)| lat.abs() > limit + 1.5).unwrap_or(true);
                if clear && rng.chance(0.8) { t.trees.push((p, rng.range(4.5, 8.0))); }
            }
            i += rng.range(5.0, 11.0) as usize;
        }
        t
    }

    /// The spot on the ground `lat` metres right of the middle at a point.
    fn off(&self, i: usize, lat: f32) -> V3 { V3::new(self.pts[i][0] + self.dir[i][1] * lat, 0.0, self.pts[i][1] - self.dir[i][0] * lat) }

    /// Where a spot is by the road: metres right of the middle, metres
    /// along the lap, and the nearest point of the middle. None far from
    /// any road.
    fn at(&self, x: f32, z: f32) -> Option<(f32, f32, usize)> {
        if !(x >= 0.0 && z >= 0.0 && x < SIZE as f32 && z < SIZE as f32) { return None; }
        let i = self.near[z as i32 as usize * SIZE + x as i32 as usize];
        if i == NONE { return None; }
        let i = i as usize;
        let (dx, dz, d) = (x - self.pts[i][0], z - self.pts[i][1], self.dir[i]);
        let s = i as f32 * self.step + dx * d[0] + dz * d[1];
        Some((dx * d[1] - dz * d[0], if s < 0.0 { s + self.len } else if s >= self.len { s - self.len } else { s }, i))
    }

    /// A spot of the world on the small map.
    fn dot(&self, x: f32, z: f32) -> (i32, i32) {
        (MAP.0 + ((x - self.chart.0) * self.chart.2) as i32, MAP.1 + ((self.chart.1 - z) * self.chart.2) as i32)
    }

    /// The colour of the ground at a spot. One pixel covers `far` metres
    /// there, so a pattern finer than that is painted as its average.
    fn ground(&self, th: &Theme, paint: &Paint, x: f32, z: f32, far: f32, time: f32) -> Rgb {
        let land = || mix(th.land[0], th.land[1], paint.at(x * 0.13, z * 0.13));
        let Some((lat, s, _)) = self.at(x, z) else { return land() };
        let a = lat.abs();
        if a > self.limit { return land(); }
        if a > self.half + KERB {
            let band = mixf(((s * (1.0 / 6.0)) as i32 & 1) as f32, 0.5, blur(far, 6.0));
            let fine = mixf(paint.at(x * 0.9, z * 0.9), 0.5, blur(far, 1.2));
            return mix(th.verge[0], th.verge[1], band * 0.5 + paint.at(x * 0.13, z * 0.13) * 0.3 + fine * 0.2);
        }
        if a > self.half { return mix(th.kerb[0], th.kerb[1], mixf(((s * 0.5) as i32 & 1) as f32, 0.5, blur(far, 2.0))); }
        if s < 1.6 {
            let k = (((s * 1.25) as i32 + ((lat + 64.0) * 1.25) as i32) & 1) as f32;
            return mix(0x181818, 0xf8f8f8, mixf(k, 0.5, blur(far, 0.8)));
        }
        for &(from, mid) in &self.pads {
            let (w, u) = (if s < from { s - from + self.len } else { s - from }, (lat - mid).abs());
            if w < 7.0 && u < 1.8 {
                let k = ((w + u * 0.9) / 1.6 - time * 3.0).rem_euclid(1.0);
                return mix(0xf05010, 0xfff040, mixf(if k < 0.5 { 1.0 } else { 0.0 }, 0.5, blur(far, 1.6)));
            }
        }
        let c = mix(th.road[0], th.road[1], mixf(paint.at(x * 0.9, z * 0.9), 0.5, blur(far, 1.2)));
        if a > self.half - 0.3 || (a < 0.14 && (s * (1.0 / 3.0)) as i32 & 1 == 0 && far < 2.0) { mix(c, th.line, 0.85) } else { c }
    }
}

/// The sky along a line of sight: two rows of hills, and clouds.
fn sky(th: &Theme, paint: &Paint, d: V3) -> Rgb {
    let (a, up) = (bearing(d.x, d.z), d.y);
    let [near, far] = paint.hills(a);
    if up < th.rise * near { return mix(th.hills[1], th.sky[1], 0.25); }
    if up < th.rise * far { return mix(th.hills[0], th.sky[1], 0.45); }
    let cloud = ((paint.at(a * (64.0 / TAU), up * 26.0) - 0.56) * 6.0).clamp(0.0, 1.0) * (up * 8.0).min(1.0);
    mix(mix(th.sky[1], th.sky[0], (up * 2.4).min(1.0)), 0xffffff, cloud * 0.8)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Item { Boost, Oil, Bolt, Seeker, Star, Zap }

const ITEMS: [Item; 6] = [Item::Boost, Item::Oil, Item::Bolt, Item::Seeker, Item::Star, Item::Zap];

/// What a box gives, by place: the leader gets what guards a lead, and
/// the last get what wins one back. Odds out of 20, in the order of ITEMS.
fn roll(place: usize, rng: &mut Rng) -> Item {
    let odds: [u32; 6] = match place { 0 => [3, 9, 8, 0, 0, 0], 1 | 2 => [6, 4, 6, 4, 0, 0], 3..=5 => [7, 2, 3, 5, 3, 0], _ => [6, 0, 1, 5, 5, 3] };
    let mut r = rng.below(20);
    for (item, &o) in ITEMS.iter().zip(&odds) {
        if r < o { return *item; }
        r -= o;
    }
    Item::Boost
}

/// Each item as a picture of 11 by 11 dots for the box on the screen.
const ICONS: [([&str; 11], Rgb, Rgb); 6] = [
    ([".....#.....", "....###....", "...#####...", "..###.###..", ".###...###.", ".....#.....", "....###....", "...#####...", "..###.###..", ".###...###.", "..........."], 0xff7010, 0),
    ([".....#.....", "....###....", "....###....", "...#####...", "..#######..", ".#########.", ".#########.", ".##o######.", ".###o#####.", "..#######..", "...#####..."], 0x181820, 0x8088a0),
    (["...#####...", "..#######..", ".###oo####.", ".##oo#####.", ".#########.", ".#########.", ".#########.", ".#########.", ".#########.", "..#######..", "...#####..."], 0x30c040, 0xc0ffc0),
    (["...#####...", "..#######..", ".###oo####.", ".##oo#####.", ".#########.", "ooooooooooo", ".#########.", ".#########.", ".#########.", "..#######..", "...#####..."], 0xe02828, 0xffffff),
    ([".....#.....", ".....#.....", "....###....", "....###....", "###########", ".#########.", "..#######..", "...#####...", "..###.###..", "..##...##..", ".##.....##."], 0xffd020, 0),
    (["......###..", ".....###...", "....###....", "...###.....", "..#######..", ".....###...", "....###....", "...###.....", "..###......", ".###.......", ".#........."], 0xfff040, 0),
];

/// What a driver asks of the kart in one step.
#[derive(Clone, Copy, Default)]
struct Ctl { gas: bool, brake: bool, steer: f32, hop: bool, hold: bool, fire: bool }

/// A puff of dust, a spark or a flame.
struct Puff { p: V3, v: V3, age: f32, life: f32, size: f32, tint: [f32; 3], hot: bool }

/// One kart on the track.
#[derive(Clone, Default)]
struct Car {
    x: f32,
    z: f32,
    /// Where the nose points, and where the kart is going: on ice and in
    /// a drift the second lags behind the first.
    head: f32,
    way: f32,
    v: f32,
    steer: f32,
    /// Seconds left of the hop, the spin, the push, the star, being small
    /// and standing still.
    air: f32,
    spin: f32,
    boost: f32,
    star: f32,
    small: f32,
    stall: f32,
    /// The side of the drift, 0 for none, and how long it has been held.
    drift: f32,
    charge: f32,
    item: Option<Item>,
    /// Seconds until the box has picked an item.
    wheel: f32,
    /// Seconds until a computer driver uses what it has.
    wait: f32,
    lap: i32,
    s: f32,
    lat: f32,
    at: usize,
    done: Option<f32>,
    lap_from: f32,
    lane: f32,
    skill: f32,
}

impl Car {
    /// Find the kart by the road again, and count a lap when it crosses
    /// the line: one more going forward, one fewer going back.
    fn locate(&mut self, t: &Track) {
        let Some((lat, s, at)) = t.at(self.x, self.z) else { return };
        if s - self.s < -t.len / 2.0 { self.lap += 1; } else if s - self.s > t.len / 2.0 { self.lap -= 1; }
        (self.lat, self.s, self.at) = (lat, s, at);
    }

    /// How far the kart has come in the race, in metres.
    fn far(&self, t: &Track) -> f32 { self.lap as f32 * t.len + self.s }

    /// A hit: the kart spins and loses its speed. A star shrugs it off.
    fn hit(&mut self) {
        if self.star > 0.0 || self.spin > 0.0 { return; }
        (self.spin, self.drift, self.charge, self.boost) = (1.2, 0.0, 0.0, 0.0);
        self.v *= 0.4;
    }

    /// One step of driving: `top` is the kart's top speed on a clear road.
    fn drive(&mut self, c: Ctl, t: &Track, th: &Theme, top: f32, dt: f32, fx: &mut Vec<Puff>, rng: &mut Rng) {
        for left in [&mut self.boost, &mut self.star, &mut self.small, &mut self.stall, &mut self.wait] { *left = (*left - dt).max(0.0); }
        let slow = self.lat.abs() > t.half + KERB && self.boost <= 0.0 && self.star <= 0.0;
        let base = top;
        let top = top * if self.small > 0.0 { 0.7 } else { 1.0 } * if self.star > 0.0 { 1.18 } else { 1.0 } * if self.boost > 0.0 { 1.32 } else { 1.0 } * if slow { th.drag } else { 1.0 };
        if self.spin > 0.0 {
            self.spin -= dt;
            self.v -= self.v * 2.0 * dt;
        } else {
            if c.brake && self.boost <= 0.0 {
                self.v = if self.v > 0.0 { (self.v - 30.0 * dt).max(0.0) } else { (self.v - 9.0 * dt).max(-7.0) };
            } else if self.boost > 0.0 || (c.gas && self.stall <= 0.0) {
                let push = if self.boost > 0.0 { 36.0 } else { 13.0 };
                if self.v < top { self.v = (self.v + push * (1.0 - 0.55 * (self.v / top).max(0.0)) * dt).min(top); }
            } else {
                self.v -= self.v * 0.7 * dt;
            }
            if self.v > top { self.v -= (self.v - top) * if slow { 3.5 } else { 1.6 } * dt; }
            self.steer += (c.steer - self.steer) * (12.0 * dt).min(1.0);
            if c.hop && self.air <= 0.0 && self.drift == 0.0 { self.air = HOP; }
            let up = self.air > 0.0;
            self.air = (self.air - dt).max(0.0);
            // Landing a hop with the key down and the wheel turned starts a drift.
            if up && self.air <= 0.0 && c.hold && c.steer != 0.0 && self.v > base * 0.5 && !slow { (self.drift, self.charge) = (c.steer.signum(), 0.0); }
            let rate = rate_at(self.v.abs());
            let by = if self.drift != 0.0 {
                // The wheel sets how hard the drift bites: against it the kart
                // runs almost straight, with it the bend is tight and the
                // sparks come sooner.
                let bite = 0.8 + 0.7 * self.steer * self.drift;
                self.charge += dt * bite.max(0.3);
                let by = self.drift * rate * bite;
                if !c.hold || self.v < base * 0.4 || slow {
                    if !c.hold { self.boost = self.boost.max(if self.charge > ORANGE { 1.3 } else if self.charge > BLUE { 0.6 } else { 0.0 }); }
                    self.drift = 0.0;
                }
                by
            } else {
                self.steer * rate * (self.v / 5.0).clamp(-1.0, 1.0)
            };
            self.head += by * dt;
        }
        // Travel follows the nose as fast as the ground grips.
        let grip = th.grip * if self.drift != 0.0 { 0.4 } else if slow { 0.6 } else { 1.0 };
        self.way += turn(self.head - self.way) * (grip * dt).min(1.0);
        let slip = turn(self.head - self.way);
        if slip.abs() > 0.7 { self.way = self.head - 0.7 * slip.signum(); }
        self.x += self.way.sin() * self.v * dt;
        self.z += self.way.cos() * self.v * dt;
        self.locate(t);
        // The barrier turns the kart along itself and takes speed for it.
        let edge = t.limit - RADIUS;
        if self.lat.abs() > edge {
            let (d, side) = (t.dir[self.at], self.lat.signum());
            let over = self.lat.abs() - edge;
            self.x -= d[1] * over * side;
            self.z += d[0] * over * side;
            self.lat = edge * side;
            let along = d[0].atan2(d[1]);
            let rel = turn(self.way - along);
            if rel.sin() * side * self.v > 0.0 {
                self.v *= 1.0 - 0.7 * rel.sin().abs();
                self.way = if rel.cos() >= 0.0 { along } else { along + PI };
                self.head += turn(self.way - self.head) * 0.5;
                (self.drift, self.charge) = (0.0, 0.0);
            }
        }
        if fx.len() > 400 { return; }
        let (sn, cs) = self.head.sin_cos();
        let rear = |side: f32| V3::new(self.x - sn * 0.8 + cs * 0.65 * side, 0.05, self.z - cs * 0.8 - sn * 0.65 * side);
        // Sparks and flames keep most of the kart's speed, dust stays behind.
        let with = |rng: &mut Rng, keep: f32, k: f32| V3::new(self.way.sin() * self.v * keep + rng.range(-k, k), rng.range(0.2, 1.0) * k, self.way.cos() * self.v * keep + rng.range(-k, k));
        if self.drift != 0.0 && self.charge > BLUE {
            let tint = if self.charge > ORANGE { [1.0, 0.5, 0.1] } else { [0.3, 0.6, 1.0] };
            fx.push(Puff { p: rear(-self.drift), v: with(rng, 0.8, 2.2), age: 0.0, life: 0.2, size: 0.22, tint, hot: false });
        } else if self.drift != 0.0 && rng.chance(0.5) {
            fx.push(Puff { p: rear(-self.drift), v: with(rng, 0.3, 1.0), age: 0.0, life: 0.35, size: 0.4, tint: [0.9, 0.9, 0.9], hot: false });
        }
        if slow && self.v > 4.0 && rng.chance(0.4) {
            let (r, g, b) = parts(th.verge[0]);
            fx.push(Puff { p: rear(rng.range(-1.0, 1.0)), v: with(rng, 0.2, 1.2), age: 0.0, life: 0.5, size: 0.7, tint: [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0], hot: false });
        }
        if self.boost > 0.0 {
            for side in [-0.2, 0.2] { fx.push(Puff { p: rear(side).add(V3::new(0.0, 0.35, 0.0)), v: with(rng, 0.72, 0.5), age: 0.0, life: 0.16, size: 0.34, tint: [1.0; 3], hot: true }); }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Slick, Bolt, Seeker }

/// What lies or flies on the track: oil, a bolt, a seeker.
struct Loose { kind: Kind, x: f32, z: f32, way: f32, s: f32, lat: f32, age: f32, life: f32, from: usize, to: usize, hops: u32 }

/// How fast a bolt and a seeker fly, in metres a second.
const BOLT: f32 = 48.0;
const SEEK: f32 = 50.0;

/// The models that are the same on every track.
struct Models { karts: Vec<Model>, shadow: Model, slick: Model, chest: Model }

/// A wheel: an eight-sided drum on its side.
fn wheel(m: &mut Model, x: f32, z: f32, r: f32, w: f32) {
    let rim = |k: u32, x: f32| { let a = k as f32 / 8.0 * TAU; V3::new(x, r + a.cos() * r, z + a.sin() * r) };
    for k in 0..8 {
        face(m, [rim(k, x - w / 2.0), rim(k + 1, x - w / 2.0), rim(k + 1, x + w / 2.0), rim(k, x + w / 2.0)], rim(k, x).add(rim(k, x).sub(V3::new(x, r, z))), Mat::Flat(0x202024), 1.0);
        for side in [-1.0f32, 1.0] {
            let hub = V3::new(x + side * w / 2.0, r, z);
            face(m, [hub, rim(k, hub.x), rim(k + 1, hub.x), rim(k + 1, hub.x)], V3::new(x + side * 9.0, r, z), Mat::Flat(0x44444c), 1.0);
        }
    }
}

/// A flat eight-sided patch on the ground.
fn patch(r: f32, c: Rgb) -> Model {
    let mut m = Model::new();
    let rim = |k: u32| { let a = k as f32 / 8.0 * TAU; V3::new(a.cos() * r, 0.0, a.sin() * r) };
    for k in 0..8 { face(&mut m, [V3::default(), rim(k), rim(k + 1), rim(k + 1)], V3::new(0.0, 9.0, 0.0), Mat::Flat(c), 1.0); }
    m.bound();
    m
}

fn kart_model(paint: Rgb, suit: Rgb) -> Model {
    let b = |x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32, c: Rgb| Model::block(V3::new(x0, y0, z0), V3::new(x1, y1, z1), Mat::Flat(c), 1.0, 99.0);
    let mut m = Model::new();
    for part in [
        b(-0.5, 0.16, -0.95, 0.5, 0.4, 0.75, paint),        // the floor
        b(-0.28, 0.2, 0.75, 0.28, 0.36, 1.15, paint),       // the nose
        b(-0.62, 0.14, 1.05, 0.62, 0.24, 1.25, 0xe8e8e8),   // the bumper
        b(-0.34, 0.4, -0.72, 0.34, 0.92, -0.58, 0x303038),  // the seat
        b(-0.3, 0.4, -1.0, 0.3, 0.66, -0.72, 0x70747c),     // the engine
        b(-0.24, 0.4, -0.58, 0.24, 0.86, -0.24, suit),      // the driver
        b(-0.18, 0.86, -0.54, 0.18, 1.16, -0.24, 0xf0c098), // the head
        b(-0.2, 0.92, -0.6, 0.2, 1.24, -0.42, paint),       // the helmet, and its peak
        b(-0.2, 1.08, -0.42, 0.2, 1.24, -0.12, paint),
        b(-0.04, 0.4, 0.1, 0.04, 0.72, 0.16, 0x303038),     // the wheel and its column
        b(-0.2, 0.66, 0.06, 0.2, 0.74, 0.14, 0x202020),
    ] { m.extend(&part, &M4::identity()); }
    for (x, z, r, w) in [(-0.66, 0.62, 0.24, 0.2), (0.66, 0.62, 0.24, 0.2), (-0.7, -0.62, 0.3, 0.28), (0.7, -0.62, 0.3, 0.28)] { wheel(&mut m, x, z, r, w); }
    shade(&mut m);
    m
}

impl Models {
    fn new(tex: &Tex) -> Models {
        let mut chest = Model::block(V3::new(-0.55, -0.55, -0.55), V3::new(0.55, 0.55, 0.55), Mat::Tex(tex.chest), 1.1, 99.0);
        shade(&mut chest);
        Models { karts: (0..KARTS).map(|n| kart_model(PAINT[n], SUIT[n])).collect(), shadow: patch(1.0, 0x181c18), slick: patch(1.2, 0x101018), chest }
    }
}

/// A sample from a wave: `wave(seconds, noise)` gives -1 to 1.
fn synth(secs: f32, mut wave: impl FnMut(f32, f32) -> f32) -> Sample {
    let rate = funkey::audio::RATE as f32;
    let mut x = 0x2545_f491u32;
    Sample::from_i16((0..(secs * rate) as usize).map(|i| {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        (wave(i as f32 / rate, (x >> 8) as f32 / 8388608.0 - 1.0).clamp(-1.0, 1.0) * 32000.0) as i16
    }).collect())
}

/// Two tunes of the same length as one sample, so they never drift apart.
fn duet(lead: &str, bass: &str) -> Sample {
    let (a, b) = (Tune::parse(lead, Wave::Square, 0.09).render(), Tune::parse(bass, Wave::Triangle, 0.28).render());
    let at = |s: &Sample, i: usize| *s.data.get(i).unwrap_or(&0) as i32;
    Sample::from_i16((0..a.data.len().max(b.data.len())).map(|i| (at(&a, i) + at(&b, i)).clamp(-32768, 32767) as i16).collect())
}

const TITLE: [&str; 2] = ["168 g5/8 b5/8 d6/8 b5/8 g5/8 b5/8 d6/4 e6/8 d6/8 b5/8 g5/8 a5/4 d5/4 \
    g5/8 b5/8 d6/8 b5/8 g5/8 b5/8 d6/4 c6/8 b5/8 a5/8 f#5/8 g5/2",
    "168 g2/4 d3/4 g2/4 d3/4 c3/4 g3/4 d3/4 d3/4 g2/4 d3/4 g2/4 d3/4 c3/4 d3/4 g2/2"];

struct Sounds { title: Sample, engine: Sample, skid: Sample, tick: Sample, go: Sample, pick: Sample, got: Sample, boost: Sample, hit: Sample, fire: Sample,
                star: Sample, zap: Sample, lap: Sample, last: Sample, win: Sample }

impl Sounds {
    fn new() -> Sounds {
        Sounds {
            title: duet(TITLE[0], TITLE[1]),
            engine: Sample::loop_tone(Wave::Saw, 62.0, 0.5, 0.22),
            skid: synth(0.4, |_, r| r * 0.25),
            tick: Sample::tone(Wave::Square, 520.0, 0.12, 0.3),
            go: Sample::tone(Wave::Square, 1040.0, 0.45, 0.3),
            pick: Sample::sweep(Wave::Triangle, 500.0, 1200.0, 0.12, 0.4),
            got: Tune::parse("360 c6/16 e6/16 g6/16 c7/8", Wave::Square, 0.22).render(),
            boost: Sample::sweep(Wave::Saw, 200.0, 900.0, 0.35, 0.3),
            hit: Sample::sweep(Wave::Saw, 500.0, 60.0, 0.5, 0.4),
            fire: Sample::sweep(Wave::Square, 900.0, 300.0, 0.15, 0.3),
            star: Tune::parse("300 c5/16 e5/16 g5/16 c6/16 e6/16 g6/16 c7/8", Wave::Square, 0.25).render(),
            zap: synth(0.4, |t, r| r * (1.0 - t / 0.4) * 0.6),
            lap: Tune::parse("240 g5/8 c6/8 e6/4", Wave::Triangle, 0.4).render(),
            last: Tune::parse("300 g5/16 g5/16 g5/16 c6/8 g5/16 c6/16 e6/4", Wave::Square, 0.25).render(),
            win: Tune::parse("200 c5/8 e5/8 g5/8 c6/4 a5/8 b5/8 c6/2", Wave::Triangle, 0.45).render(),
        }
    }
}

/// An engine size: the top speed in metres a second, and how much of it
/// the computer's drivers get.
struct Class { name: &'static str, top: f32, cpu: f32 }

static CLASSES: [Class; 3] = [Class { name: "50cc", top: 21.0, cpu: 0.93 }, Class { name: "100cc", top: 25.0, cpu: 0.965 }, Class { name: "150cc", top: 29.0, cpu: 0.99 }];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Grid, Race, Flag, Table, Cup }

struct Kart {
    scene: Scene,
    tex: Tex,
    models: Models,
    paint: Paint,
    track: Track,
    class: usize,
    race: usize,
    cars: Vec<Car>,
    /// Each kart's place in the race, 0 for the leader.
    place: [usize; KARTS],
    /// The karts in the order they finished, and the order they line up in.
    order: Vec<usize>,
    grid: [usize; KARTS],
    points: [u32; KARTS],
    gained: [u32; KARTS],
    loose: Vec<Loose>,
    fx: Vec<Puff>,
    /// Seconds until each box is back.
    crates: Vec<f32>,
    mode: Mode,
    time: f32,
    clock: f32,
    count: f32,
    timer: f32,
    /// How long the gas has been down on the grid.
    rev: f32,
    /// The camera's heading, and the kart it follows.
    cam: f32,
    eye: usize,
    paused: bool,
    auto: bool,
    flash: f32,
    wrong: f32,
    note: Option<(String, f32)>,
    /// The best lap of this race, the best ever on each track, and the
    /// best place in a cup of each class (0 for none yet).
    lap_best: f32,
    best: [f32; 4],
    cups: [u32; 3],
    audio: Audio,
    snd: Sounds,
    rng: Rng,
}

impl Kart {
    fn new() -> Kart {
        let mut scene = Scene::new(W, H);
        scene.smooth = true;
        let tex = textures(&mut scene);
        let kept = |key: String| funkey::store::load(GAME, &key).and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.0);
        let mut game = Kart {
            models: Models::new(&tex), track: Track::build(&PLANS[0], &tex), scene, tex, paint: Paint::new(), class: 1, race: 0, cars: Vec::new(), place: [0; KARTS],
            order: Vec::new(), grid: [0; KARTS], points: [0; KARTS], gained: [0; KARTS], loose: Vec::new(), fx: Vec::new(), crates: Vec::new(), mode: Mode::Title,
            time: 0.0, clock: 0.0, count: 0.0, timer: 0.0, rev: 0.0, cam: 0.0, eye: 0, paused: false, auto: false, flash: 0.0, wrong: 0.0, note: None, lap_best: 0.0,
            best: [0, 1, 2, 3].map(|i| kept(format!("lap{i}"))), cups: [0, 1, 2].map(|i| kept(format!("cup{i}")) as u32),
            audio: Audio::off(), snd: Sounds::new(), rng: Rng::from_time(),
        };
        game.title();
        game
    }

    fn theme(&self) -> &'static Theme { &THEMES[self.track.plan.theme] }

    /// The title: the computer's drivers race behind the words.
    fn title(&mut self) {
        (self.grid, self.mode, self.eye) = ([0, 1, 2, 3, 4, 5, 6, 7], Mode::Title, 1);
        let which = self.rng.below(4) as usize;
        self.line_up(which);
        self.audio.stop(1);
        self.audio.stop(2);
        self.audio.play_loop(3, &self.snd.title, 0.6);
    }

    /// Build a track and stand the karts on its grid.
    fn line_up(&mut self, which: usize) {
        let t = Track::build(&PLANS[which], &self.tex);
        self.cars = vec![Car::default(); KARTS];
        for (slot, &who) in self.grid.iter().enumerate() {
            let s = t.len - 5.0 - slot as f32 * 4.5;
            let i = (s / t.step) as usize % t.pts.len();
            let lat = if slot % 2 == 0 { -2.6 } else { 2.6 };
            let c = &mut self.cars[who];
            (c.x, c.z) = (t.pts[i][0] + t.dir[i][1] * lat, t.pts[i][1] - t.dir[i][0] * lat);
            c.head = t.dir[i][0].atan2(t.dir[i][1]);
            (c.way, c.lap, c.s, c.lat, c.at) = (c.head, -1, s, lat, i);
            c.skill = 1.0 - 0.014 * who as f32;
            c.lane = lat * self.rng.range(0.3, 1.3);
            c.stall = if who == 0 { 0.0 } else { self.rng.range(0.0, 0.3) };
        }
        self.crates = vec![0.0; t.crates.len()];
        self.scene.fog = Some((THEMES[t.plan.theme].sky[1], THEMES[t.plan.theme].fog));
        self.track = t;
        self.loose.clear();
        self.fx.clear();
        self.order.clear();
        (self.clock, self.lap_best, self.wrong, self.flash, self.note) = (0.0, 0.0, 0.0, 0.0, None);
        self.cam = self.cars[self.eye].head;
        self.rank();
    }

    /// A new cup: you start the first race from the back.
    fn cup(&mut self) {
        (self.points, self.grid, self.eye) = ([0; KARTS], [1, 2, 3, 4, 5, 6, 7, 0], 0);
        self.audio.stop(3);
        self.begin(0);
    }

    fn begin(&mut self, race: usize) {
        self.race = race;
        self.line_up(race);
        (self.mode, self.count, self.rev) = (Mode::Grid, 3.6, 0.0);
        self.audio.play_loop(1, &self.snd.engine, 0.12);
        self.audio.play_loop(2, &self.snd.skid, 0.0);
    }

    fn say(&mut self, s: &str) { self.note = Some((s.to_string(), 2.0)); }

    /// Who is where: the finished in the order they came in, the rest by
    /// how far they have come.
    fn rank(&mut self) {
        let t = &self.track;
        let mut out: Vec<usize> = (0..KARTS).filter(|n| !self.order.contains(n)).collect();
        out.sort_by(|&a, &b| self.cars[b].far(t).partial_cmp(&self.cars[a].far(t)).unwrap_or(std::cmp::Ordering::Equal));
        for (p, &n) in self.order.iter().chain(&out).enumerate() { self.place[n] = p; }
    }

    /// A kart's top speed. The computer's drivers get a little more when
    /// they trail you and a little less when they lead, so the race stays
    /// close.
    fn top(&self, n: usize) -> f32 {
        let class = &CLASSES[self.class];
        if self.mode == Mode::Title { return class.top * self.cars[n].skill; }
        if n == 0 { return class.top; }
        let behind = ((self.cars[0].far(&self.track) - self.cars[n].far(&self.track)) / 80.0).clamp(-1.0, 1.0);
        class.top * class.cpu * self.cars[n].skill * (1.0 + 0.05 * behind)
    }

    /// What the computer does with a kart: aim at a point down the road,
    /// slow for a bend too tight for the speed, drift through a long one.
    fn think(&self, n: usize) -> Ctl {
        let (k, t) = (&self.cars[n], &self.track);
        let ahead = |d: f32| (k.at + (d / t.step) as usize) % t.pts.len();
        let (j, here, soon) = (ahead(8.0 + k.v.abs() * 0.45), t.curv[ahead(3.0)], t.curv[ahead(4.0 + k.v.abs() * 0.6)]);
        // Lean to the inside of the bend that is coming.
        let mut lane = k.lane + (soon * 45.0).clamp(-1.0, 1.0) * t.half * 0.5;
        // Pass a kart close ahead on the side this one is already on.
        for (m, o) in self.cars.iter().enumerate() {
            let gap = (o.s - k.s + t.len * 1.5).rem_euclid(t.len) - t.len * 0.5;
            if m != n && gap > 0.0 && gap < 5.0 + k.v * 0.3 && (o.lat - k.lat).abs() < 2.4 && o.lat.abs() < t.half {
                lane = o.lat + if (k.lat, n) > (o.lat, m) { 2.6 } else { -2.6 };
            }
        }
        let lane = lane.clamp(1.5 - t.half, t.half - 1.5);
        let aim = [t.pts[j][0] + t.dir[j][1] * lane, t.pts[j][1] - t.dir[j][0] * lane];
        let err = turn((aim[0] - k.x).atan2(aim[1] - k.z) - k.head);
        let mut c = Ctl { gas: true, steer: (err * 2.4).clamp(-1.0, 1.0), ..Ctl::default() };
        let (mut tight, mut d) = (0.0f32, 0.0);
        while d < k.v * 1.1 + 6.0 {
            tight = tight.max(t.curv[ahead(d)].abs());
            d += 3.0;
        }
        let can = rate_at(k.v) * if k.drift != 0.0 { 1.5 } else { 1.0 };
        if tight * k.v > can * 0.95 { (c.gas, c.brake) = (false, tight * k.v > can * 1.25); }
        // A drift is for a bend that asks for most of what the wheel can
        // give, and it ends where the bend does or at the inside kerb.
        let hard = here.abs() * k.v > rate_at(k.v) * 0.55 && here * soon > 0.0;
        c.hop = hard && k.drift == 0.0 && k.air <= 0.0 && k.spin <= 0.0 && k.lat.abs() < t.half - 2.0;
        if c.hop || k.air > 0.0 { c.steer = here.signum() * c.steer.abs().max(0.25); }
        let side = if k.drift != 0.0 { k.drift } else { here.signum() };
        c.hold = (c.hop || k.air > 0.0 || k.drift != 0.0) && here * side > 0.012 && k.lat * side < t.half - 1.5;
        c.fire = k.item.is_some() && k.wait <= 0.0;
        c
    }

    fn use_item(&mut self, n: usize) {
        let Some(item) = self.cars[n].item.take() else { return };
        let k = &self.cars[n];
        let (sn, cs) = k.head.sin_cos();
        let loose = |kind: Kind, d: f32, life: f32, to: usize| Loose { kind, x: k.x + sn * d, z: k.z + cs * d, way: k.head, s: k.s, lat: k.lat, age: 0.0, life, from: n, to, hops: 0 };
        let sound = match item {
            Item::Boost => { self.cars[n].boost = 1.5; &self.snd.boost }
            Item::Star => { self.cars[n].star = 7.0; &self.snd.star }
            Item::Oil => { self.loose.push(loose(Kind::Slick, -2.6, 45.0, n)); &self.snd.fire }
            Item::Bolt => { self.loose.push(loose(Kind::Bolt, 2.4, 6.0, n)); &self.snd.fire }
            Item::Seeker => {
                let to = (0..KARTS).find(|&m| self.place[m] + 1 == self.place[n]).unwrap_or(n);
                self.loose.push(loose(Kind::Seeker, 2.4, 9.0, to));
                &self.snd.fire
            }
            Item::Zap => {
                for (m, c) in self.cars.iter_mut().enumerate() {
                    if m != n && c.star <= 0.0 { (c.small, c.spin, c.item, c.drift) = (5.0, c.spin.max(0.6), None, 0.0); }
                }
                self.flash = 0.3;
                &self.snd.zap
            }
        };
        if n == 0 || item == Item::Zap { self.audio.play(sound, 1.0); }
    }

    /// What the track does to a kart after it has moved: the push pads,
    /// the boxes, and the line.
    fn after(&mut self, n: usize, lap: i32, dt: f32) {
        let (t, me) = (&self.track, n == 0 && self.mode == Mode::Race);
        let car = &mut self.cars[n];
        if car.air <= 0.0 && t.pads.iter().any(|&(from, mid)| (car.s - from).rem_euclid(t.len) < 7.0 && (car.lat - mid).abs() < 1.8) {
            if me && car.boost < 0.5 { self.audio.play(&self.snd.boost, 1.0); }
            car.boost = car.boost.max(1.0);
        }
        for (i, p) in t.crates.iter().enumerate() {
            if self.crates[i] <= 0.0 && (car.x - p[0]).hypot(car.z - p[1]) < 1.6 {
                self.crates[i] = 2.0;
                if car.item.is_none() && car.wheel <= 0.0 {
                    car.wheel = 1.3;
                    if me { self.audio.play(&self.snd.pick, 1.0); }
                }
            }
        }
        if car.wheel > 0.0 {
            car.wheel -= dt;
            if car.wheel <= 0.0 {
                (car.item, car.wait) = (Some(roll(self.place[n], &mut self.rng)), self.rng.range(1.0, 4.5));
                if me { self.audio.play(&self.snd.got, 1.0); }
            }
        }
        if car.lap <= lap || self.mode == Mode::Title { return; }
        if car.lap >= LAPS && car.done.is_none() {
            car.done = Some(self.clock);
            self.order.push(n);
        }
        if !me { return; }
        let took = self.clock - car.lap_from;
        car.lap_from = self.clock;
        if lap >= 0 {
            if self.lap_best == 0.0 || took < self.lap_best { self.lap_best = took; }
            let i = self.track.plan.theme;
            if self.best[i] == 0.0 || took < self.best[i] {
                self.best[i] = took;
                if !cfg!(test) { funkey::store::save(GAME, &format!("lap{i}"), &format!("{took:.2}")); }
            }
        }
        if car.lap == LAPS - 1 {
            self.audio.play(&self.snd.last, 1.0);
            self.note = Some(("FINAL LAP".to_string(), 2.0));
        } else if car.lap > 0 && car.lap < LAPS {
            self.audio.play(&self.snd.lap, 1.0);
            self.note = Some((format!("LAP {}", clock(took)), 2.0));
        }
    }

    /// Karts that touch push each other apart; a star knocks the other one
    /// into a spin.
    fn bump(&mut self) {
        for b in 1..KARTS {
            let (left, right) = self.cars.split_at_mut(b);
            let kb = &mut right[0];
            for ka in left.iter_mut() {
                let (dx, dz) = (kb.x - ka.x, kb.z - ka.z);
                // Nose to tail they meet sooner than side by side.
                let (sn, cs) = ka.head.sin_cos();
                let d = ((dx * sn + dz * cs) * 0.75).hypot(dx * cs - dz * sn);
                let min = RADIUS * (if ka.small > 0.0 { 0.6 } else { 1.0 } + if kb.small > 0.0 { 0.6 } else { 1.0 });
                if d >= min || d < 1e-3 { continue; }
                let far = dx.hypot(dz);
                let (px, pz) = (dx / far * (min - d) / 2.0, dz / far * (min - d) / 2.0);
                (ka.x, ka.z, kb.x, kb.z) = (ka.x - px, ka.z - pz, kb.x + px, kb.z + pz);
                if ka.star > 0.0 { kb.hit(); }
                if kb.star > 0.0 { ka.hit(); }
                let mean = (ka.v + kb.v) / 2.0;
                (ka.v, kb.v) = (mixf(ka.v, mean, 0.2), mixf(kb.v, mean, 0.2));
            }
        }
    }

    /// Oil lies, a bolt flies straight and bounces off the barrier, a
    /// seeker follows the road to the kart ahead. Each stops at the first
    /// kart it meets.
    fn things(&mut self, dt: f32) {
        let (t, cars) = (&self.track, &self.cars);
        let mut struck = Vec::new();
        self.loose.retain_mut(|o| {
            o.age += dt;
            if o.age > o.life { return false; }
            match o.kind {
                Kind::Slick => {}
                Kind::Bolt => {
                    o.x += o.way.sin() * BOLT * dt;
                    o.z += o.way.cos() * BOLT * dt;
                    let Some((lat, _, i)) = t.at(o.x, o.z) else { return false };
                    let over = lat.abs() - (t.limit - 0.4);
                    if over > 0.0 {
                        let d = t.dir[i];
                        o.x -= d[1] * over * lat.signum();
                        o.z += d[0] * over * lat.signum();
                        o.way = 2.0 * d[0].atan2(d[1]) - o.way;
                        o.hops += 1;
                        if o.hops > 5 { return false; }
                    }
                }
                Kind::Seeker => {
                    o.s = (o.s + SEEK * dt).rem_euclid(t.len);
                    let to = &cars[o.to];
                    let aim = if o.to != o.from && (to.s - o.s).rem_euclid(t.len) < 30.0 { to.lat } else { 0.0 };
                    o.lat += (aim - o.lat) * (5.0 * dt).min(1.0);
                    let i = (o.s / t.step) as usize % t.pts.len();
                    (o.x, o.z) = (t.pts[i][0] + t.dir[i][1] * o.lat, t.pts[i][1] - t.dir[i][0] * o.lat);
                }
            }
            let reach = if o.kind == Kind::Slick { 1.3 } else { 1.5 };
            // Its own kart is safe from it for the first half second.
            let met = cars.iter().enumerate().position(|(n, k)| {
                (n != o.from || o.age > 0.5) && (k.x - o.x).hypot(k.z - o.z) < reach && !(o.kind == Kind::Slick && k.air > 0.0)
            });
            struck.extend(met);
            met.is_none()
        });
        for n in struck {
            if n == 0 && self.cars[0].star <= 0.0 && self.mode == Mode::Race { self.audio.play(&self.snd.hit, 1.0); }
            self.cars[n].hit();
        }
    }

    /// One step of the race. `me` is what your keys ask; None lets the
    /// computer drive every kart.
    fn step(&mut self, me: Option<Ctl>, dt: f32) {
        self.clock += dt;
        self.rank();
        let th = self.theme();
        for n in 0..KARTS {
            let c = match me { Some(c) if n == 0 => c, _ => self.think(n) };
            if c.fire && self.cars[n].spin <= 0.0 { self.use_item(n); }
            let (top, lap) = (self.top(n), self.cars[n].lap);
            self.cars[n].drive(c, &self.track, th, top, dt, &mut self.fx, &mut self.rng);
            self.after(n, lap, dt);
        }
        self.bump();
        self.things(dt);
        for p in &mut self.fx {
            p.age += dt;
            p.p = p.p.add(p.v.mul(dt));
        }
        self.fx.retain(|p| p.age < p.life);
        for c in &mut self.crates { *c = (*c - dt).max(0.0); }
    }

    /// A frame of racing, as two steps: a kart at full speed must not
    /// jump a box or a barrier.
    fn run(&mut self, me: Option<Ctl>, dt: f32) {
        self.step(me, dt / 2.0);
        self.step(me.map(|c| Ctl { hop: false, fire: false, ..c }), dt / 2.0);
        let k = &self.cars[self.eye];
        self.cam += turn(k.way - self.cam) * (5.0 * dt).min(1.0);
        self.flash = (self.flash - dt).max(0.0);
        if let Some((_, left)) = &mut self.note {
            *left -= dt;
            if *left <= 0.0 { self.note = None; }
        }
        let k = &self.cars[0];
        if self.mode != Mode::Title {
            self.audio.rate(1, 0.6 + k.v.abs() / 20.0);
            self.audio.volume(1, 0.12 + k.v.abs() / 250.0);
            self.audio.volume(2, if k.drift != 0.0 || k.spin > 0.0 { 0.3 } else { 0.0 });
        }
        // Facing back up the road for a while brings a warning.
        let d = self.track.dir[k.at];
        self.wrong = if turn(k.head - d[0].atan2(d[1])).cos() < -0.3 && k.v > 3.0 { self.wrong + dt } else { 0.0 };
    }

    /// What your keys ask of your kart.
    fn keys(&self, input: &Input) -> Ctl {
        if self.auto { return self.think(0); }
        Ctl {
            gas: if input.exact() { input.axis_y() < 0 } else { input.axis_y() <= 0 },
            brake: input.axis_y() > 0,
            steer: input.axis_x() as f32,
            hop: input.pressed(Key::Space),
            hold: input.held(Key::Space),
            fire: [Key::Char('x'), Key::Char('z'), Key::Char('f'), Key::Enter].into_iter().any(|k| input.pressed(k)),
        }
    }

    /// The race is over: those still out are placed by how far they came,
    /// and everyone gets the points of the place.
    fn tally(&mut self) {
        self.rank();
        self.order = (0..KARTS).collect();
        self.order.sort_by_key(|&n| self.place[n]);
        for (p, &n) in self.order.iter().enumerate() {
            self.gained[n] = POINTS[p];
            self.points[n] += POINTS[p];
        }
        self.grid.copy_from_slice(&self.order);
        (self.mode, self.timer) = (Mode::Table, 0.0);
        self.audio.stop(1);
        self.audio.stop(2);
    }

    /// The karts by their points in the cup, best first.
    fn standing(&self) -> Vec<usize> {
        let mut by: Vec<usize> = self.grid.to_vec();
        by.sort_by(|&a, &b| self.points[b].cmp(&self.points[a]));
        by
    }
}

/// Text with a dark edge, so it reads on snow and on asphalt.
fn ink(f: &mut Frame, x: i32, y: i32, s: &str, c: Rgb, scale: i32) {
    let d = (scale + 1) / 2;
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (d, d)] { f.text_scaled(x + dx, y + dy, s, 0x101018, true, scale); }
    f.text_scaled(x, y, s, c, true, scale);
}

fn ink_mid(f: &mut Frame, y: i32, s: &str, c: Rgb, scale: i32) { ink(f, W / 2 - Frame::text_width(s, true, scale) / 2, y, s, c, scale); }

fn nth(place: usize) -> String { format!("{}{}", place + 1, ["ST", "ND", "RD", "TH"][place.min(3)]) }

impl Kart {
    /// The track and all that is on it, from behind the kart the camera follows.
    fn view(&mut self, f: &mut Frame) {
        let (th, k) = (self.theme(), &self.cars[self.eye]);
        let back = 5.6 + k.boost.min(0.6) * 1.5;
        let cam = Cam3 { pos: V3::new(k.x - self.cam.sin() * back, 2.4, k.z - self.cam.cos() * back), yaw: self.cam, pitch: -0.2, focal: FOCAL };
        let (sn, cs) = self.cam.sin_cos();
        let seen = |x: f32, z: f32, far: f32| { let d = (x - cam.pos.x) * sn + (z - cam.pos.z) * cs; d > -3.0 && d < far };
        self.scene.begin(&cam);
        for m in &self.track.walls { if seen(m.centre.x, m.centre.z, 320.0) { self.scene.push(m, &M4::identity()); } }
        for &(p, size) in &self.track.trees {
            if seen(p.x, p.z, 280.0) { self.scene.billboard(p, size * SLIM[th.tree], size, Mat::Tex(self.tex.trees[th.tree]), [1.0; 3]); }
        }
        for (i, p) in self.track.crates.iter().enumerate() {
            if self.crates[i] > 0.0 || !seen(p[0], p[1], 200.0) { continue; }
            let lift = 1.0 + (self.time * 3.0 + i as f32).sin() * 0.12;
            self.scene.push(&self.models.chest, &M4::rotate_y(self.time * 2.0 + i as f32).then(&M4::translate(V3::new(p[0], lift, p[1]))));
        }
        for o in &self.loose {
            match o.kind {
                Kind::Slick => self.scene.push(&self.models.slick, &M4::translate(V3::new(o.x, 0.03, o.z))),
                Kind::Bolt => self.scene.billboard(V3::new(o.x, 0.1, o.z), 0.9, 0.9, Mat::Tex(self.tex.puff), [0.3, 1.7, 0.4]),
                Kind::Seeker => self.scene.billboard(V3::new(o.x, 0.1, o.z), 0.9, 0.9, Mat::Tex(self.tex.puff), [2.0, 0.3, 0.3]),
            }
        }
        for (n, k) in self.cars.iter().enumerate() {
            if !seen(k.x, k.z, 250.0) { continue; }
            let size = if k.small > 0.0 { 0.6 } else { 1.0 };
            let m = M4::scale(size).then(&M4::rotate_z(k.steer * 0.06 + k.drift * 0.08)).then(&M4::rotate_y(k.head + k.spin / 1.2 * 2.0 * TAU + k.drift * 0.25))
                .then(&M4::translate(V3::new(k.x, (k.air / HOP * PI).sin() * 0.45, k.z)));
            if k.star > 0.0 {
                // A star runs through the colours.
                let (mut lit, h) = (self.models.karts[n].clone(), self.time * 9.0 + n as f32);
                for v in &mut lit.verts { for (i, l) in v.lit.iter_mut().enumerate() { *l *= 1.2 + 0.6 * (h + i as f32 * 2.1).sin(); } }
                self.scene.push(&lit, &m);
            } else {
                self.scene.push(&self.models.karts[n], &m);
            }
            self.scene.push(&self.models.shadow, &M4::scale(size).then(&M4::translate(V3::new(k.x, 0.02, k.z))));
        }
        for p in &self.fx {
            let age = p.age / p.life;
            // A flame goes from white-yellow to red as it cools.
            let tint = if p.hot { [2.4, 2.2 - 1.8 * age, 1.2 - 1.1 * age] } else { p.tint };
            self.scene.billboard(p.p, p.size * (1.0 - age * 0.7), p.size * (1.0 - age * 0.7), Mat::Tex(self.tex.puff), tint);
        }
        // The ground is the "sky" under the horizon: each pixel's line of
        // sight is followed down to the flat ground and coloured there.
        let (track, paint, time, eye) = (&self.track, &self.paint, self.time, cam.pos);
        let (wide, thick) = (1.0 / (FOCAL * eye.y), 1.0 / th.fog);
        self.scene.render(f, &|d: V3| {
            if d.y > -0.004 { return sky(th, paint, d); }
            let t = eye.y / -d.y;
            let c = track.ground(th, paint, eye.x + d.x * t, eye.z + d.z * t, t * t * wide, time);
            mix(th.sky[1], c, fade(t * thick))
        });
        if self.flash > 0.0 { for p in &mut f.px { *p = blend(*p, 0xffffff, self.flash * 2.0); } }
    }

    /// The item box at the top of the screen.
    fn item_box(&self, f: &mut Frame) {
        let (k, x, y) = (&self.cars[0], W / 2 - 21, 8);
        f.rect(x, y, 43, 43, 0x101018);
        f.rect(x + 2, y + 2, 39, 39, 0x5070b0);
        let show = if k.wheel > 0.0 { Some(ITEMS[(self.time * 12.0) as usize % ITEMS.len()]) } else { k.item };
        let Some(item) = show else { return };
        let (rows, ink, light) = &ICONS[ITEMS.iter().position(|&i| i == item).unwrap_or(0)];
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if ch != '.' { f.rect(x + 5 + i as i32 * 3, y + 5 + j as i32 * 3, 3, 3, if ch == 'o' { *light } else { *ink }); }
            }
        }
    }

    /// The small map, with every kart on it.
    fn map(&self, f: &mut Frame) {
        for &(x, y) in &self.track.dots {
            f.rect(x, y, 3, 3, 0x101018);
        }
        for &(x, y) in &self.track.dots { f.rect(x, y, 2, 2, 0xe8e8f0); }
        for n in (0..KARTS).rev() {
            let (x, y) = self.track.dot(self.cars[n].x, self.cars[n].z);
            if n == 0 { f.rect(x - 2, y - 2, 6, 6, 0xffffff); }
            f.rect(x - 1, y - 1, 4, 4, PAINT[n]);
        }
    }

    fn hud(&self, f: &mut Frame) {
        let k = &self.cars[0];
        ink(f, 12, 10, &format!("LAP {}/{}", (k.lap + 1).clamp(1, LAPS), LAPS), TEXT, 2);
        ink(f, 12, 30, &clock(self.clock), TEXT, 2);
        if self.lap_best > 0.0 { ink(f, 12, 50, &format!("BEST {}", clock(self.lap_best)), GOLD, 1); }
        let place = self.place[0];
        ink(f, 12, H - 48, &nth(place), if place == 0 { GOLD } else { TEXT }, 5);
        let speed = format!("{:3.0} KM/H", k.v.abs() * 3.6);
        ink(f, W - 12 - Frame::text_width(&speed, true, 2), H - 24, &speed, TEXT, 2);
        self.item_box(f);
        self.map(f);
        if k.drift != 0.0 {
            let (c, s) = if k.charge > ORANGE { (0xff8020, "DRIFT!!") } else if k.charge > BLUE { (0x60b0ff, "DRIFT!") } else { (TEXT, "DRIFT") };
            ink_mid(f, H - 24, s, c, 2);
        }
        if self.wrong > 1.0 && (self.time * 3.0) as i32 % 2 == 0 { ink_mid(f, 120, "WRONG WAY", 0xff4030, 3); }
        else if let Some((s, _)) = &self.note { ink_mid(f, 120, s, GOLD, 3); }
    }

    /// The lights over the grid.
    fn lights(&self, f: &mut Frame) {
        for i in 0..3 {
            let on = self.mode == Mode::Race || self.count < 3.0 - i as f32;
            let c = if self.mode == Mode::Race { 0x30e050 } else if on { 0xf03020 } else { 0x402020 };
            f.rect(W / 2 - 62 + i * 44, 68, 36, 36, 0x101018);
            f.circle(W / 2 - 44 + i * 44, 86, 14, c);
            for r in 0..14 { f.circle(W / 2 - 44 + i * 44, 86, r, c); }
        }
    }

    /// A list of the karts: the place, the name, and what `cell` says.
    fn table(&self, f: &mut Frame, title: &str, rows: &[usize], cell: impl Fn(usize, usize) -> String) {
        f.dim(0.35);
        ink_mid(f, 34, title, GOLD, 3);
        for (p, &n) in rows.iter().enumerate() {
            let (y, c) = (84 + p as i32 * 30, if n == 0 { GOLD } else { TEXT });
            if n == 0 { f.rect(120, y - 6, W - 240, 26, 0x283048); }
            ink(f, 136, y, &nth(p), c, 2);
            f.rect(206, y - 1, 16, 16, PAINT[n]);
            ink(f, 234, y, NAMES[n], c, 2);
            let s = cell(p, n);
            ink(f, W - 136 - Frame::text_width(&s, true, 2), y, &s, c, 2);
        }
        if (self.time * 2.0) as i32 % 2 == 0 { ink_mid(f, H - 40, "PRESS SPACE", TEXT, 2); }
    }

    fn title_screen(&self, f: &mut Frame) {
        f.dim(0.75);
        let x = W / 2 - Frame::text_width("KART", true, 12) / 2;
        f.text_scaled(x + 5, 49, "KART", 0x801010, true, 12);
        f.text_scaled(x, 44, "KART", 0xffd040, true, 12);
        ink_mid(f, 150, "FOUR TRACKS  EIGHT KARTS  THREE LAPS", TEXT, 2);
        let class = format!("<  {}  >", CLASSES[self.class].name);
        ink_mid(f, 196, &class, GOLD, 3);
        let cup = match self.cups[self.class] { 0 => "NO CUP YET".to_string(), 1 => "BEST: GOLD CUP".to_string(), 2 => "BEST: SILVER CUP".to_string(), 3 => "BEST: BRONZE CUP".to_string(), p => format!("BEST: {}", nth(p as usize - 1)) };
        ink_mid(f, 232, &cup, TEXT, 1);
        if (self.time * 2.0) as i32 % 2 == 0 { ink_mid(f, 268, "PRESS SPACE", TEXT, 2); }
        ink_mid(f, 318, "UP GAS  DOWN BRAKE  LEFT RIGHT STEER", 0xc0c8d8, 1);
        ink_mid(f, 334, "SPACE HOP, HOLD IT IN A BEND TO DRIFT", 0xc0c8d8, 1);
        ink_mid(f, 350, "X ITEM  P PAUSE  Q QUIT", 0xc0c8d8, 1);
        f.text(W - 6 - Frame::text_width(VERSION, false, 1), H - 8, VERSION, 0x8090a8);
    }
}

impl Game for Kart {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        self.timer += dt;
        let go = input.pressed(Key::Space) || input.pressed(Key::Enter);
        match self.mode {
            Mode::Title => {
                self.run(None, dt);
                if input.pressed(Key::Left) { self.class = (self.class + 2) % 3; }
                if input.pressed(Key::Right) { self.class = (self.class + 1) % 3; }
                if go { self.cup(); }
            }
            Mode::Grid => {
                let before = self.count;
                self.count -= dt;
                // The gas pressed in the last half second pays a push off the line.
                self.rev = if input.exact() && input.axis_y() < 0 { self.rev + dt } else { 0.0 };
                for at in [3.0, 2.0, 1.0] { if before > at && self.count <= at { self.audio.play(&self.snd.tick, 1.0); } }
                if self.count <= 0.0 {
                    self.audio.play(&self.snd.go, 1.0);
                    if self.rev > 0.0 && self.rev < 0.5 {
                        self.cars[0].boost = 1.0;
                        self.say("GOOD START");
                    }
                    (self.mode, self.clock) = (Mode::Race, 0.0);
                }
            }
            Mode::Race => {
                if input.pressed(Key::Char('p')) { self.paused = !self.paused; }
                if self.paused { return Flow::Continue; }
                let me = self.keys(input);
                self.run(Some(me), dt);
                // You are in, or everyone else is: the race is over.
                if self.cars[0].done.is_none() && self.order.len() == KARTS - 1 {
                    self.cars[0].done = Some(self.clock);
                    self.order.push(0);
                }
                if self.cars[0].done.is_some() {
                    (self.mode, self.timer) = (Mode::Flag, 0.0);
                    self.rank();
                    self.audio.play(&self.snd.win, 1.0);
                }
            }
            Mode::Flag => {
                self.run(None, dt);
                if self.timer > 7.0 || (go && self.timer > 1.5) { self.tally(); }
            }
            Mode::Table => {
                if go && self.timer > 0.5 {
                    if self.race + 1 < PLANS.len() { self.begin(self.race + 1); } else {
                        let place = self.standing().iter().position(|&n| n == 0).unwrap_or(KARTS - 1) as u32 + 1;
                        if self.cups[self.class] == 0 || place < self.cups[self.class] {
                            self.cups[self.class] = place;
                            if !cfg!(test) { funkey::store::save(GAME, &format!("cup{}", self.class), &place.to_string()); }
                        }
                        (self.mode, self.timer) = (Mode::Cup, 0.0);
                        self.audio.play_loop(3, &self.snd.title, 0.6);
                    }
                }
            }
            Mode::Cup => if go && self.timer > 0.5 { self.title(); },
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        self.view(f);
        match self.mode {
            Mode::Title => self.title_screen(f),
            Mode::Grid => {
                self.hud(f);
                self.lights(f);
                ink_mid(f, 120, self.track.plan.name, TEXT, 3);
                let best = self.best[self.track.plan.theme];
                if best > 0.0 { ink_mid(f, 152, &format!("BEST LAP {}", clock(best)), GOLD, 1); }
            }
            Mode::Race => {
                self.hud(f);
                if self.clock < 1.0 { self.lights(f); }
                if self.paused {
                    f.dim(0.5);
                    ink_mid(f, 170, "PAUSED", TEXT, 4);
                }
            }
            Mode::Flag => {
                self.map(f);
                ink_mid(f, 110, "FINISH", GOLD, 6);
                ink_mid(f, 180, &nth(self.place[0]), TEXT, 5);
            }
            Mode::Table => {
                let title = format!("RACE {}  {}", self.race + 1, self.track.plan.name);
                self.table(f, &title, &self.order, |_, n| format!("+{:2}  {:3}", self.gained[n], self.points[n]));
            }
            Mode::Cup => {
                let by = self.standing();
                let me = by.iter().position(|&n| n == 0).unwrap_or(KARTS - 1);
                let title = match me { 0 => "THE GOLD CUP IS YOURS".to_string(), 1 => "THE SILVER CUP IS YOURS".to_string(), 2 => "THE BRONZE CUP IS YOURS".to_string(), p => format!("{} IN THE CUP", nth(p)) };
                self.table(f, &title, &by, |_, n| format!("{:3}", self.points[n]));
            }
        }
    }
}

/// The game as it is played: with sound, and from the track asked for.
/// `KART_START=<track>[,<class>]` skips the title; `KART_AUTO=1` lets the
/// computer drive your kart.
fn kart() -> Kart {
    let mut game = Kart::new();
    game.audio = Audio::open();
    game.auto = std::env::var("KART_AUTO").is_ok();
    let from = std::env::var("KART_START").unwrap_or_default();
    let mut it = from.split(',').map(|s| s.trim().parse::<usize>().ok());
    match it.next().flatten() {
        Some(n) => {
            if let Some(c) = it.next().flatten() { game.class = c.clamp(1, 3) - 1; }
            game.cup();
            game.begin(n.clamp(1, PLANS.len()) - 1);
        }
        None => game.audio.play_loop(3, &game.snd.title, 0.6),
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // KART_BENCH=<frames> races that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("KART_BENCH") {
        let n: u32 = n.parse().unwrap_or(300);
        let mut game = Kart::new();
        let (mut f, input) = (Frame::new(W, H), Input::new());
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            game.update(&input, 1.0 / 30.0);
            game.draw(&mut f);
        }
        eprintln!("{:.2} ms a frame at {}x{} over {} frames", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n);
        return;
    }
    run(&mut kart(), Config { width: W, height: H, fps: 30 });
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(kart(), Config { width: W, height: H, fps: 30 });

#[cfg(test)]
mod tests {
    use super::*;

    /// A race on a track with the computer in every kart, and no title.
    fn race(track: usize, class: usize) -> Kart {
        let mut game = Kart::new();
        (game.class, game.auto) = (class, true);
        game.cup();
        game.begin(track);
        (game.mode, game.rng) = (Mode::Race, Rng::new(3));
        game
    }

    /// Your kart alone on the first track: the others stand parked far
    /// from the road.
    fn alone() -> Kart {
        let mut game = race(0, 1);
        for n in 1..KARTS { (game.cars[n].x, game.cars[n].z, game.cars[n].stall) = (5.0 + n as f32 * 3.0, 5.0, 1e9); }
        game
    }

    /// Stand a kart on the middle of the road at a point, on lap one.
    fn put(game: &mut Kart, n: usize, i: usize) {
        let (p, d) = (game.track.pts[i], game.track.dir[i]);
        let k = &mut game.cars[n];
        (k.x, k.z, k.head) = (p[0], p[1], d[0].atan2(d[1]));
        k.locate(&game.track);
        (k.way, k.lap) = (k.head, 0);
    }

    fn held(key: Key) -> Input {
        let mut input = Input::new();
        input.set_exact(true);
        input.key(Key::Up, true);
        input.key(key, true);
        input
    }

    #[test]
    fn every_track_is_a_clean_loop() {
        let mut scene = Scene::new(W, H);
        let tex = textures(&mut scene);
        for plan in &PLANS {
            let t = Track::build(plan, &tex);
            let n = t.pts.len();
            assert!(t.len > 700.0 && t.len < 1500.0, "{}: {} m round", plan.name, t.len);
            let tight = t.curv.iter().fold(0.0f32, |a, c| a.max(c.abs()));
            assert!(1.0 / tight > t.limit + 2.0, "{}: a bend of {:.1} m is tighter than its barrier allows", plan.name, 1.0 / tight);
            for p in &t.pts { assert!(p[0] > t.limit + 16.0 && p[1] > t.limit + 16.0 && p[0] < SIZE as f32 - t.limit - 16.0 && p[1] < SIZE as f32 - t.limit - 16.0, "{}: off the map", plan.name); }
            // Two stretches of road never share a barrier.
            for i in (0..n).step_by(2) {
                for j in (i + 2..n).step_by(2) {
                    let along = (j - i).min(n - (j - i)) as f32 * t.step;
                    let gap = (t.pts[i][0] - t.pts[j][0]).hypot(t.pts[i][1] - t.pts[j][1]);
                    assert!(along < 3.0 * t.limit || gap > 2.0 * t.limit + 2.0, "{}: {:.1} m between two stretches at {i} and {j}", plan.name, gap);
                }
            }
            // Every box and pad is on the road, and every cell by the road knows its place.
            for c in &t.crates { assert!(t.at(c[0], c[1]).unwrap().0.abs() < t.half - 1.0); }
            for i in 0..n { assert!(t.at(t.pts[i][0], t.pts[i][1]).unwrap().0.abs() < 0.8); }
        }
    }

    #[test]
    fn the_computer_finishes_every_track() {
        for track in 0..PLANS.len() {
            for class in [0, 2] {
                let mut game = race(track, class);
                let (mut steps, mut off) = (0u32, [0u32; KARTS]);
                while game.order.len() < KARTS && steps < 60 * 400 {
                    game.step(None, 1.0 / 60.0);
                    for n in 0..KARTS { if game.cars[n].done.is_none() && game.cars[n].lat.abs() > game.track.half + KERB { off[n] += 1; } }
                    steps += 1;
                }
                let name = PLANS[track].name;
                assert_eq!(game.order.len(), KARTS, "{name} at {}: only {:?} came in", CLASSES[class].name, game.order);
                let won = game.cars[game.order[0]].done.unwrap();
                let flat = game.track.len * LAPS as f32 / CLASSES[class].top;
                assert!(won < flat * 1.25, "{name} at {}: the winner took {won:.0} s, flat out is {flat:.0} s", CLASSES[class].name);
                for n in 0..KARTS { assert!((off[n] as f32) < steps as f32 * 0.03, "{name} at {}: {} spent {} of {steps} steps off the road", CLASSES[class].name, NAMES[n], off[n]); }
            }
        }
    }

    #[test]
    fn a_lap_counts_forward_and_uncounts_back() {
        let mut game = alone();
        let t = &game.track;
        let k = &mut game.cars[0];
        assert_eq!(k.lap, -1);
        (k.x, k.z) = (t.pts[4][0], t.pts[4][1]);
        k.locate(t);
        assert_eq!(k.lap, 0);
        let back = t.pts.len() - 4;
        (k.x, k.z) = (t.pts[back][0], t.pts[back][1]);
        k.locate(t);
        assert_eq!(k.lap, -1);
    }

    #[test]
    fn the_barrier_keeps_a_kart_in() {
        let mut game = alone();
        let (th, t) = (game.theme(), &game.track);
        let k = &mut game.cars[0];
        k.head += 1.2;
        k.way = k.head;
        for _ in 0..600 {
            k.drive(Ctl { gas: true, steer: 0.3, ..Ctl::default() }, t, th, 25.0, 1.0 / 60.0, &mut game.fx, &mut game.rng);
            assert!(k.lat.abs() <= t.limit - RADIUS + 0.01, "{} m from the middle", k.lat);
        }
    }

    #[test]
    fn a_drift_pays_a_push_by_its_length() {
        let mut game = alone();
        let (th, t) = (game.theme(), &game.track);
        let k = &mut game.cars[0];
        k.v = 22.0;
        let c = Ctl { gas: true, steer: 1.0, hold: true, ..Ctl::default() };
        let mut go = |k: &mut Car, c: Ctl, steps: u32| for _ in 0..steps { k.drive(c, t, th, 25.0, 1.0 / 60.0, &mut game.fx, &mut game.rng); };
        go(k, Ctl { hop: true, ..c }, 1);
        assert!(k.air > 0.0 && k.drift == 0.0);
        go(k, c, (HOP * 60.0) as u32 + 2);
        assert_eq!(k.drift, 1.0, "a hop with the wheel turned lands in a drift");
        for (charge, push) in [(0.5, 0.0), (BLUE + 0.1, 0.6), (ORANGE + 0.1, 1.3)] {
            (k.drift, k.charge, k.boost) = (1.0, charge, 0.0);
            go(k, Ctl { hold: false, ..c }, 1);
            assert!(k.drift == 0.0 && k.boost == push, "a drift of {charge} paid {} s of push", k.boost);
        }
        // A hop with the wheel straight is only a hop.
        k.boost = 0.0;
        go(k, Ctl { hop: true, steer: 0.0, ..c }, 1);
        go(k, Ctl { steer: 0.0, ..c }, (HOP * 60.0) as u32 + 2);
        assert_eq!(k.drift, 0.0);
    }

    #[test]
    fn the_keys_drive_the_kart() {
        let mut game = alone();
        game.auto = false;
        let (x, z, nose) = (game.cars[0].x, game.cars[0].z, game.cars[0].head);
        for _ in 0..30 { game.update(&held(Key::Up), 1.0 / 30.0); }
        let k = &game.cars[0];
        assert!((k.x - x).hypot(k.z - z) > 4.0 && k.v > 8.0, "a second of gas moved it {} m", (k.x - x).hypot(k.z - z));
        for _ in 0..10 { game.update(&held(Key::Right), 1.0 / 30.0); }
        assert!(turn(game.cars[0].head - nose) > 0.2, "Right turns right");
        let mut brake = Input::new();
        brake.set_exact(true);
        brake.key(Key::Down, true);
        for _ in 0..30 { game.update(&brake, 1.0 / 30.0); }
        assert!(game.cars[0].v <= 0.0, "Down stops it");
    }

    #[test]
    fn items_do_what_they_say() {
        // A bolt spins the kart ahead; a star shrugs oil off.
        let mut game = alone();
        put(&mut game, 0, 5);
        put(&mut game, 1, 25);
        game.cars[0].head = (game.cars[1].x - game.cars[0].x).atan2(game.cars[1].z - game.cars[0].z);
        game.cars[0].item = Some(Item::Bolt);
        game.use_item(0);
        for _ in 0..90 { game.things(1.0 / 60.0); }
        assert!(game.cars[1].spin > 0.0 && game.loose.is_empty(), "the bolt hit the kart ahead");
        game.cars[1].spin = 0.0;
        game.cars[1].item = Some(Item::Oil);
        game.use_item(1);
        game.cars[0].star = 5.0;
        (game.cars[0].x, game.cars[0].z) = (game.loose[0].x, game.loose[0].z);
        game.things(1.0 / 60.0);
        assert!(game.cars[0].spin == 0.0 && game.loose.is_empty(), "a star drives through oil");
        // A seeker follows the road to the kart one place ahead.
        put(&mut game, 1, 200);
        put(&mut game, 0, 10);
        game.rank();
        game.cars[0].item = Some(Item::Seeker);
        game.use_item(0);
        for _ in 0..(60.0 * 200.0 / SEEK * 1.2) as u32 { game.things(1.0 / 60.0); }
        assert!(game.cars[1].spin > 0.0, "the seeker found the kart 190 m up the road");
        // A zap makes every other kart small and takes its item.
        game.cars[2].item = Some(Item::Star);
        game.cars[0].item = Some(Item::Zap);
        game.use_item(0);
        assert!(game.cars[2].small > 0.0 && game.cars[2].item.is_none() && game.cars[0].small == 0.0);
        // The leader never draws a star, and the last never oil.
        let mut rng = Rng::new(1);
        for _ in 0..500 {
            assert!(!matches!(roll(0, &mut rng), Item::Star | Item::Zap | Item::Seeker));
            assert_ne!(roll(7, &mut rng), Item::Oil);
        }
    }

    #[test]
    fn a_cup_is_four_races_and_the_points_add_up() {
        let mut game = Kart::new();
        (game.auto, game.class) = (true, 2);
        let mut input = Input::new();
        input.inject(Key::Space);
        let idle = Input::new();
        game.update(&input, 1.0 / 30.0);
        assert_eq!(game.mode, Mode::Grid);
        assert_eq!(game.grid[KARTS - 1], 0, "you start the first race last");
        let mut f = Frame::new(W, H);
        let mut seen = Vec::new();
        for _ in 0..30 * 60 * 20 {
            if !seen.contains(&game.mode) {
                game.draw(&mut f);
                seen.push(game.mode);
            }
            let go = matches!(game.mode, Mode::Flag | Mode::Table) && game.timer > 2.0;
            game.update(if go { &input } else { &idle }, 1.0 / 30.0);
            if game.mode == Mode::Table { for (p, &n) in game.order.iter().enumerate() { assert_eq!(game.place[n], p, "the place on screen is the place in the table"); } }
            if game.mode == Mode::Cup { break; }
        }
        assert_eq!(game.mode, Mode::Cup, "four races in twenty minutes");
        assert_eq!(game.points.iter().sum::<u32>(), 4 * POINTS.iter().sum::<u32>());
        assert_eq!(seen, [Mode::Grid, Mode::Race, Mode::Flag, Mode::Table], "every screen was drawn");
        game.draw(&mut f);
        game.update(&input, 1.0 / 30.0);
        assert_eq!(game.mode, Mode::Cup, "the cup stays up for half a second");
        for _ in 0..20 { game.update(&idle, 1.0 / 30.0); }
        game.update(&input, 1.0 / 30.0);
        assert_eq!(game.mode, Mode::Title);
    }
}
