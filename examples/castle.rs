//! castle: a walk from the hills to a castle and in through its gate,
//! on funkey's textured rasterizer. Stone, slate and wood as pictures
//! wrapped over the walls, the sun and its shadows baked into every
//! corner, banners in the wind, torches at the gate, pines on the
//! slopes, mountains, fog and clouds. A demo of what real pixels can
//! look like in a terminal; there is nothing to win.
//!
//!     cargo run --release --example castle
//!
//! Up and Down walk, Left and Right turn, A and D sidestep, W and S
//! change the pace, Space hands the walk back to the autopilot, Q quits.
//! `CASTLE_BENCH=100` draws that many frames with no terminal and
//! prints the time a frame takes.

use funkey::*;
use std::f32::consts::{PI, TAU};

const W: i32 = 960;
const H: i32 = 600;
/// The demo's own version; the engine has its own.
const VERSION: &str = "1.0";
/// The castle stands on a rise this high.
const H0: f32 = 14.0;
const WATER_Y: f32 = H0 - 1.4;
const ZENITH: Rgb = 0x3468b8;
const HAZE: Rgb = 0xdcd4c8;
const SUNCOL: Rgb = 0xfff1c9;
const CLOUD: usize = 256;
const I: M4 = M4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);

fn sun() -> V3 { V3::new(-0.45, 0.55, -0.70).norm() }

// ─────────────────────────────── noise ───────────────────────────────

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ seed.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13; h = h.wrapping_mul(0x5bd1e995); h ^= h >> 15;
    (h & 0xffffff) as f32 / 16777216.0
}

fn smooth(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

/// Value noise on a lattice that repeats every `px` by `py` cells.
fn noise2(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (smooth(x - xi as f32), smooth(y - yi as f32));
    let g = |ix: i32, iy: i32| hash(ix.rem_euclid(px), iy.rem_euclid(py), seed);
    let a = g(xi, yi) + (g(xi + 1, yi) - g(xi, yi)) * fx;
    let b = g(xi, yi + 1) + (g(xi + 1, yi + 1) - g(xi, yi + 1)) * fx;
    a + (b - a) * fy
}

fn noise(x: f32, y: f32, period: i32, seed: u32) -> f32 { noise2(x, y, period, period, seed) }

/// Layers of noise, each twice as fine and half as tall. Repeats every
/// 8 units.
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += noise(x * freq, y * freq, 8 << o, seed + o) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Ridges: noise folded over, for mountains.
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

/// Tileable noise over a texture, `u` and `v` in 0..1.
fn tnoise(u: f32, v: f32, octaves: u32, seed: u32) -> f32 { fbm(u * 8.0, v * 8.0, octaves, seed) }

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb { funkey::raster::blend(a, b, t.clamp(0.0, 1.0)) }
fn scale(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    rgb((r as f32 * k).clamp(0.0, 255.0) as u8, (g as f32 * k).clamp(0.0, 255.0) as u8, (b as f32 * k).clamp(0.0, 255.0) as u8)
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }
fn mul3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] * b[0], a[1] * b[1], a[2] * b[2]] }

// ───────────────────────────── textures ──────────────────────────────

fn stone_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let row = y / 32;
        let xx = (x + if row % 2 == 0 { 0 } else { 32 }) % 256;
        let col = xx / 64;
        let (bx, by) = (xx % 64, y % 32);
        let (u, v) = (x as f32 / 256.0, y as f32 / 256.0);
        let n = tnoise(u, v, 5, 17);
        let stain = tnoise(u * 0.5, v * 0.5, 3, 18);
        // Courses are straight; the joints between blocks wander.
        let jp = (hash(col as i32, row as i32, 13) * 12.0) as u32;
        let joint = (bx >= jp && bx < jp + 3) || by < 3;
        if joint { return scale(lerp(0x4e4740, 0x6a6258, n), 0.9 + stain * 0.2); }
        let id = hash(col as i32, row as i32, 5);
        let mut c = lerp(0x968b7c, 0xb9ae9a, id);
        c = lerp(c, 0xa08c74, hash(col as i32, row as i32, 7) * 0.5);
        if hash(col as i32, row as i32, 9) > 0.86 { c = lerp(c, 0x6f7f56, 0.45); }
        if stain > 0.58 { c = lerp(c, 0x4f4a44, (stain - 0.58) * 2.2); }
        let bevel = if by < 6 || bx < jp + 6 { 1.1 } else if by > 28 || bx > 60 { 0.84 } else { 1.0 };
        scale(c, (0.78 + n * 0.44) * bevel)
    })
}

fn bush_tex() -> Texture {
    Texture::from_fn(64, 64, |x, y| {
        let (u, v) = ((x as f32 + 0.5) / 64.0, (y as f32 + 0.5) / 64.0);
        let (dx, dy) = ((u - 0.5) * 1.1, v - 0.62);
        let n = noise2(u * 8.0, v * 8.0, 8, 8, 101);
        if (dx * dx + dy * dy).sqrt() > 0.36 + n * 0.1 || v > 0.97 { return CUTOUT; }
        let g = noise2(u * 16.0, v * 16.0, 16, 16, 103);
        scale(lerp(0x2c5a26, 0x5a9440, g), 0.6 + 0.5 * (1.0 - v) + 0.2 * (0.5 - dx))
    })
}

fn rock_bb_tex() -> Texture {
    Texture::from_fn(64, 32, |x, y| {
        let (u, v) = ((x as f32 + 0.5) / 64.0, (y as f32 + 0.5) / 32.0);
        let (dx, dy) = ((u - 0.5) * 1.05, (v - 0.75) * 1.6);
        let n = noise2(u * 8.0, v * 8.0, 8, 8, 111);
        if (dx * dx + dy * dy).sqrt() > 0.42 + n * 0.08 { return CUTOUT; }
        let g = noise2(u * 16.0, v * 16.0, 16, 16, 113);
        scale(lerp(0x6e6a64, 0x9b968e, g), 0.75 + 0.35 * (1.0 - v) + 0.15 * (0.5 - dx))
    })
}

fn slate_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let row = y / 16;
        let xx = (x + if row % 2 == 0 { 0 } else { 16 }) % 256;
        let (col, bx, by) = (xx / 32, xx % 32, y % 16);
        let n = tnoise(x as f32 / 256.0, y as f32 / 256.0, 3, 19);
        let base = lerp(0x424d5b, 0x5c6877, hash(col as i32, row as i32, 3));
        let mut k = 0.85 + n * 0.3;
        if by >= 14 { k *= 0.5; }
        if bx < 1 { k *= 0.65; }
        if by < 2 { k *= 1.12; }
        scale(base, k)
    })
}

fn wood_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let pw = 256.0 / 6.0;
        let plank = (x as f32 / pw) as i32;
        let bx = x as f32 - plank as f32 * pw;
        let id = hash(plank, 0, 21);
        let (u, v) = (x as f32 / 256.0, y as f32 / 256.0);
        let grain = noise2(u * 48.0 + id * 7.0, v * 6.0, 48, 6, 23);
        let band = (80..96).contains(&y) || (160..176).contains(&y);
        if band {
            let rivet = (x + 10) % 40 < 6 && (84..92).contains(&(y % 80));
            return if rivet { 0x8a8a90 } else { lerp(0x2a2a32, 0x3c3c46, grain) };
        }
        let base = lerp(0x6a4b2c, 0x8d6c46, id);
        scale(base, if bx < 2.0 { 0.35 } else { 0.78 + grain * 0.4 })
    })
}

fn grass_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let (u, v) = (x as f32 / 256.0, y as f32 / 256.0);
        let n1 = tnoise(u, v, 5, 31);
        let n2 = tnoise(u, v, 3, 33);
        let n3 = noise2(u * 64.0, v * 64.0, 64, 64, 35);
        let mut c = lerp(0x4a7a2c, 0x7fa546, n1);
        if n2 < 0.42 { c = lerp(c, 0x365c22, (0.42 - n2) * 6.0); }
        if n2 > 0.66 { c = lerp(c, 0x8a7a48, (n2 - 0.66) * 4.0); }
        scale(c, 0.85 + n3 * 0.3)
    })
}

fn dirt_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let (u, v) = (x as f32 / 256.0, y as f32 / 256.0);
        let n1 = tnoise(u, v, 5, 41);
        let n3 = noise2(u * 64.0, v * 64.0, 64, 64, 45);
        let mut c = lerp(0x6b5538, 0x8f7652, n1);
        if hash(x as i32, y as i32, 47) > 0.992 { c = 0x9a948a; }
        scale(c, 0.85 + n3 * 0.3)
    })
}

fn rock_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let (u, v) = (x as f32 / 256.0, y as f32 / 256.0);
        let n1 = tnoise(u, v, 6, 61);
        let n2 = noise2(u * 32.0, v * 32.0, 32, 32, 63);
        scale(lerp(0x7d7770, 0x9c968c, n1), 0.8 + n2 * 0.4)
    })
}

fn water_tex() -> Texture {
    Texture::from_fn(256, 256, |x, y| {
        let (u, v) = (x as f32 / 256.0, y as f32 / 256.0);
        let n = tnoise(u, v, 4, 51);
        let r = noise2(u * 32.0, v * 32.0, 32, 32, 53);
        lerp(0x27496e, 0x6f9fc4, (n * 0.6 + r * 0.4).powf(1.6))
    })
}

fn banner_tex() -> Texture {
    Texture::from_fn(64, 64, |x, y| {
        let (u, v) = (x as f32 / 64.0, y as f32 / 64.0);
        if u > 0.7 && (v - 0.5).abs() < (u - 0.7) * 1.7 { return CUTOUT; }
        let n = noise2(u * 8.0, v * 8.0, 8, 8, 81);
        if x < 3 || y < 3 || y > 60 { return lerp(0xd6ae3a, 0xf0cc5a, n); }
        let (dx, dy) = ((x as i32 - 22).abs(), (y as i32 - 32).abs());
        if dx + dy < 12 { return lerp(0xdcb23c, 0xf4d060, n); }
        lerp(0x9c1a1a, 0xc02828, n)
    })
}

fn flame_tex(frame: u32) -> Texture {
    Texture::from_fn(32, 64, move |x, y| {
        let (u, v) = ((x as f32 + 0.5) / 32.0, (y as f32 + 0.5) / 64.0);
        let cx = (u - 0.5) * 2.0;
        let t = 1.0 - v;
        let wob = (noise2(u * 4.0 + frame as f32 * 1.7, v * 8.0 + frame as f32 * 2.3, 4, 8, 90 + frame) - 0.5) * 0.6 * t;
        let r = (1.0 - t).powf(0.35) * (t * 3.0).min(1.0) * 0.85;
        let dist = (cx - wob).abs() / r.max(0.02);
        if dist > 1.0 || t > 0.98 || t < 0.03 { return CUTOUT; }
        let core = 1.0 - dist;
        if core > 0.55 && t < 0.6 { 0xfff3b0 } else if core > 0.3 { 0xffa030 } else { 0xe04010 }
    })
}

fn pine_tex() -> Texture {
    Texture::from_fn(128, 256, |x, y| {
        let (u, v) = ((x as f32 + 0.5) / 128.0, (y as f32 + 0.5) / 256.0);
        let cx = (u - 0.5) * 2.0;
        if v > 0.86 { return if cx.abs() < 0.07 { lerp(0x4a3320, 0x6b4a2e, hash(x as i32, y as i32, 3)) } else { CUTOUT }; }
        if v < 0.04 { return CUTOUT; }
        let t = (v - 0.04) / 0.82;
        let saw = (t * 6.0).fract();
        let width = t.powf(0.8) * 0.95 * (0.55 + 0.45 * saw);
        let edge = noise2(u * 16.0, v * 32.0, 16, 32, 71) * 0.14;
        if cx.abs() > width + edge - 0.07 { return CUTOUT; }
        let n = noise2(u * 32.0, v * 64.0, 32, 64, 73);
        let base = lerp(0x22441a, 0x3f6b2a, n);
        scale(base, (1.0 - 0.3 * cx) * (0.72 + 0.28 * saw))
    })
}

fn window_tex(lit: bool) -> Texture {
    Texture::from_fn(32, 64, move |x, y| {
        if x < 3 || x > 28 || y < 3 || y > 60 { return lerp(0xa89e8c, 0xc4bba8, hash(x as i32, y as i32, 2)); }
        if lit {
            if (15..17).contains(&x) || (31..33).contains(&y) { return 0x3a2a18; }
            return lerp(0xe8a840, 0xffd070, hash(x as i32 / 4, y as i32 / 4, 4));
        }
        lerp(0x0e0e14, 0x1c1c26, hash(x as i32, y as i32, 6))
    })
}

fn crow_tex(up: bool) -> Texture {
    Texture::from_fn(16, 16, move |x, y| {
        let dx = (x as f32 - 7.5).abs();
        let wing = if up { 8.0 - dx * 0.6 } else { 6.0 + dx * 0.6 };
        let body = dx < 1.6 && (y as f32 - 7.5).abs() < 1.6;
        if body || (y as f32 - wing).abs() < 0.9 { 0x101014 } else { CUTOUT }
    })
}

struct Tx { stone: usize, slate: usize, wood: usize, grass: usize, dirt: usize, rock: usize, water: usize, banner: usize, flame: [usize; 4], pine: usize, bush: usize, boulder: usize, win_dark: usize, win_lit: usize, crow: [usize; 2] }

// ─────────────────────────────── the land ────────────────────────────

/// The land as it would be without the castle.
fn wild(x: f32, z: f32) -> f32 {
    let (fx, fz) = (x / 480.0 + 5.3, z / 480.0 + 2.1);
    let hills = fbm(fx, fz, 6, 11);
    let d = (x * x + z * z).sqrt();
    let far = smooth(((d - 450.0) / 800.0).clamp(0.0, 1.0));
    let mtn = ridged(fx * 0.7, fz * 0.7, 5, 29);
    hills * 60.0 - 14.0 + mtn * mtn * far * 520.0 + 0.045 * (-z - 120.0).max(0.0)
}

/// Distance from a rounded square round the castle: zero inside it.
fn rsq(x: f32, z: f32) -> f32 {
    let (qx, qz) = ((x.abs() - 28.0).max(0.0), (z.abs() - 28.0).max(0.0));
    (qx * qx + qz * qz).sqrt()
}

/// The ground: the wild land, a flat rise for the castle, a moat round it.
fn ground(x: f32, z: f32) -> f32 {
    let d = rsq(x, z);
    let t = smooth(((d - 30.0) / 110.0).clamp(0.0, 1.0));
    let h = H0 + (wild(x, z) - H0) * t;
    let m = (1.0 - ((d - 18.0).abs() - 2.0).max(0.0) / 2.0).clamp(0.0, 1.0);
    h - smooth(m) * 3.4
}

/// Where feet land: the ground, or the drawbridge over the moat.
fn floor_at(x: f32, z: f32) -> f32 {
    let g = ground(x, z);
    if x.abs() < 2.7 && (-52.5..=-40.5).contains(&z) { g.max(H0 + 0.15) } else { g }
}

fn normal_at(x: f32, z: f32, e: f32) -> V3 {
    let (dx, dz) = (ground(x + e, z) - ground(x - e, z), ground(x, z + e) - ground(x, z - e));
    V3::new(-dx, 2.0 * e, -dz).norm()
}

type Aabb = (V3, V3);

fn ray_hits_box(o: V3, d: V3, b: &Aabb) -> bool {
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    for i in 0..3 {
        let (oi, di, lo, hi) = match i { 0 => (o.x, d.x, b.0.x, b.1.x), 1 => (o.y, d.y, b.0.y, b.1.y), _ => (o.z, d.z, b.0.z, b.1.z) };
        if di.abs() < 1e-6 { if oi < lo || oi > hi { return false; } continue; }
        let (mut a, mut c) = ((lo - oi) / di, (hi - oi) / di);
        if a > c { std::mem::swap(&mut a, &mut c); }
        t0 = t0.max(a);
        t1 = t1.min(c);
        if t0 > t1 { return false; }
    }
    true
}

/// How much sun reaches a point: 1 in the open, less behind a hill or a
/// wall. `land` says whether to march over the ground too.
fn shadow(p: V3, casters: &[Aabb], land: bool) -> f32 {
    let s = sun();
    for b in casters { if ray_hits_box(p, s, b) { return 0.0; } }
    if land {
        let mut q = p;
        let step = 4.0;
        for _ in 0..60 {
            q = q.add(s.mul(step));
            if ground(q.x, q.z) > q.y { return 0.0; }
        }
    }
    1.0
}

/// The light at a surface: warm sun where it falls, cool sky elsewhere.
fn light(n: V3, shade: f32, ao: f32) -> [f32; 3] {
    let d = n.dot(sun()).max(0.0) * shade * 1.05;
    let sky = 0.42 * ao;
    [sky * 0.62 + d, sky * 0.68 + d * 0.94, sky * 0.82 + d * 0.82]
}

/// A patch of ground as a grid of triangles, lit and tinted per corner.
fn land_chunk(x0: f32, z0: f32, nx: usize, nz: usize, cell: f32, fine: bool, hole: Option<(f32, f32, f32, f32)>, rim: (f32, f32, f32, f32), casters: &[Aabb], tx: &Tx) -> Model {
    let mut m = Model::new();
    let (mat, tscale) = if fine { (Mat::Tex(tx.grass), 6.0) } else { (Mat::Tex(tx.rock), 36.0) };
    let corner = |ix: usize, iz: usize| {
        let (x, z) = (x0 + ix as f32 * cell, z0 + iz as f32 * cell);
        let h = ground(x, z);
        let n = normal_at(x, z, cell * 0.5);
        let mut lit = light(n, shadow(V3::new(x, h + 0.4, z), casters, true), 1.0);
        if fine {
            // Patches of drier grass, so a meadow is not one green.
            let patch = fbm(x / 90.0 + 3.0, z / 90.0 + 7.0, 3, 121);
            lit = mul3(lit, lerp3([0.9, 0.95, 0.9], [1.14, 1.06, 0.8], smooth(((patch - 0.42) * 3.5).clamp(0.0, 1.0))));
        } else {
            let slope = 1.0 - n.y;
            let snow = smooth(((h - 250.0) / 90.0).clamp(0.0, 1.0)) * (1.0 - slope * 1.2).clamp(0.0, 1.0);
            let grassy = smooth(((120.0 - h) / 80.0).clamp(0.0, 1.0)) * (1.0 - slope * 1.6).clamp(0.0, 1.0);
            let base = lerp3(lerp3([0.78, 0.74, 0.7], [0.55, 0.78, 0.38], grassy), [1.55, 1.55, 1.65], snow);
            lit = mul3(lit, base);
        }
        Vert { p: V3::new(x, h, z), u: x / tscale, v: z / tscale, lit }
    };
    let verts: Vec<Vert> = (0..=nz).flat_map(|iz| (0..=nx).map(move |ix| (ix, iz))).map(|(ix, iz)| corner(ix, iz)).collect();
    let at = |ix: usize, iz: usize| verts[iz * (nx + 1) + ix];
    for iz in 0..nz {
        for ix in 0..nx {
            if let Some((hx0, hz0, hx1, hz1)) = hole {
                let (cx, cz) = (x0 + (ix as f32 + 0.5) * cell, z0 + (iz as f32 + 0.5) * cell);
                if cx > hx0 && cx < hx1 && cz > hz0 && cz < hz1 { continue; }
            }
            m.quad(at(ix, iz + 1), at(ix + 1, iz + 1), at(ix + 1, iz), at(ix, iz), mat);
        }
    }
    if fine {
        // A skirt hanging from the outer edge of the fine land hides any
        // crack against the coarser land beyond.
        let drop = |v: Vert| Vert { p: V3::new(v.p.x, v.p.y - 10.0, v.p.z), ..v };
        let (x1, z1) = (x0 + nx as f32 * cell, z0 + nz as f32 * cell);
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        for ix in 0..nx {
            if near(z0, rim.1) { let (a, b) = (at(ix, 0), at(ix + 1, 0)); m.quad(b, a, drop(a), drop(b), mat); }
            if near(z1, rim.3) { let (a, b) = (at(ix + 1, nz), at(ix, nz)); m.quad(b, a, drop(a), drop(b), mat); }
        }
        for iz in 0..nz {
            if near(x0, rim.0) { let (a, b) = (at(0, iz + 1), at(0, iz)); m.quad(b, a, drop(a), drop(b), mat); }
            if near(x1, rim.2) { let (a, b) = (at(nx, iz), at(nx, iz + 1)); m.quad(b, a, drop(a), drop(b), mat); }
        }
    }
    m.bound();
    m
}

/// The fine land round the walk, the coarse land to the mountains, in
/// chunks the scene can skip, built on every core.
fn build_land(casters: &[Aabb], tx: &Tx) -> Vec<Model> {
    const FINE: (f32, f32, f32, f32) = (-160.0, -480.0, 160.0, 120.0);
    let mut jobs: Vec<(f32, f32, usize, usize, f32, bool)> = Vec::new();
    let (cell, per) = (4.0, 20usize);
    let (nxc, nzc) = (((FINE.2 - FINE.0) / cell) as usize / per, ((FINE.3 - FINE.1) / cell) as usize / per);
    for cz in 0..nzc { for cx in 0..nxc { jobs.push((FINE.0 + (cx * per) as f32 * cell, FINE.1 + (cz * per) as f32 * cell, per, per, cell, true)); } }
    let (cell, per, span) = (30.0, 15usize, 1800.0);
    let n = (2.0 * span / cell) as usize / per;
    for cz in 0..n { for cx in 0..n { jobs.push((-span + (cx * per) as f32 * cell, -span + (cz * per) as f32 * cell, per, per, cell, false)); } }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 32);
    let mut out: Vec<Vec<(usize, Model)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads).map(|t| {
            let jobs = &jobs;
            s.spawn(move || jobs.iter().enumerate().skip(t).step_by(threads)
                .map(|(i, &(x0, z0, nx, nz, cell, fine))| (i, land_chunk(x0, z0, nx, nz, cell, fine, (!fine).then_some(FINE), FINE, casters, tx))).collect::<Vec<_>>())
        }).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut all: Vec<(usize, Model)> = out.drain(..).flatten().collect();
    all.sort_by_key(|(i, _)| *i);
    all.into_iter().map(|(_, m)| m).collect()
}

// ─────────────────────────────── the castle ──────────────────────────

/// A wall face from `p0` to `p1` on the ground plane, `y0` up to `y1`.
/// Its outside is on the left as you walk from p0 to p1.
fn wall(m: &mut Model, p0: (f32, f32), p1: (f32, f32), y0: f32, y1: f32, mat: Mat, scale: f32, seg: f32) {
    let (dx, dz) = (p1.0 - p0.0, p1.1 - p0.1);
    let len = (dx * dx + dz * dz).sqrt();
    let (ux, uz) = (dx / len, dz / len);
    let u = |p: (f32, f32)| (p.0 * ux + p.1 * uz) / scale;
    let v = |p: (f32, f32), y: f32| Vert::new(V3::new(p.0, y, p.1), u(p), y / scale, 1.0);
    m.patch(v(p0, y0), v(p1, y0), v(p1, y1), v(p0, y1), ((len / seg).ceil() as u32).max(1), (((y1 - y0) / seg).ceil() as u32).max(1), mat);
}

fn add(m: &mut Model, o: Model, at: &M4) { m.extend(&o, at); }

/// The underside of a roof's overhang: a disc facing down.
fn disc_down(m: &mut Model, c: V3, r: f32, n: u32, mat: Mat, turn: f32) {
    for i in 0..n {
        let (a0, a1) = (i as f32 / n as f32 * TAU + turn, (i + 1) as f32 / n as f32 * TAU + turn);
        let p0 = V3::new(c.x + a0.sin() * r, c.y, c.z + a0.cos() * r);
        let p1 = V3::new(c.x + a1.sin() * r, c.y, c.z + a1.cos() * r);
        m.tri(Vert::new(p1, p1.x / 4.0, p1.z / 4.0, 1.0), Vert::new(p0, p0.x / 4.0, p0.z / 4.0, 1.0), Vert::new(c, c.x / 4.0, c.z / 4.0, 1.0), mat);
    }
}

/// A box in the castle; the big ones also throw shadows.
fn block(m: &mut Model, casters: &mut Vec<Aabb>, a: V3, b: V3, mat: Mat, scale: f32, cast: bool) {
    add(m, Model::block(a, b, mat, scale, 2.5), &I);
    if cast { casters.push((a, b)); }
}

fn build_castle(tx: &Tx) -> (Model, Vec<Aabb>) {
    let mut m = Model::new();
    let stone = Mat::Tex(tx.stone);
    let slate = Mat::Tex(tx.slate);
    let wood = Mat::Tex(tx.wood);
    let mut casters: Vec<Aabb> = Vec::new();
    let (half, wt, wh) = (35.0, 2.5, 9.0);
    let (y0, top) = (H0 - 1.0, H0 + wh);
    // Curtain walls; the front one leaves the gate open.
    block(&mut m, &mut casters, V3::new(-half, y0, -half - wt / 2.0), V3::new(-2.6, top, -half + wt / 2.0), stone, 4.0, true);
    block(&mut m, &mut casters, V3::new(2.6, y0, -half - wt / 2.0), V3::new(half, top, -half + wt / 2.0), stone, 4.0, true);
    block(&mut m, &mut casters, V3::new(-half, y0, half - wt / 2.0), V3::new(half, top, half + wt / 2.0), stone, 4.0, true);
    block(&mut m, &mut casters, V3::new(-half - wt / 2.0, y0, -half), V3::new(-half + wt / 2.0, top, half), stone, 4.0, true);
    block(&mut m, &mut casters, V3::new(half - wt / 2.0, y0, -half), V3::new(half + wt / 2.0, top, half), stone, 4.0, true);
    // The arch over the gate, and the wall above it.
    block(&mut m, &mut casters, V3::new(-2.6, H0 + 6.0, -half - wt / 2.0), V3::new(2.6, top, -half + wt / 2.0), stone, 4.0, false);
    {
        // A round arch: the wall above it in strips, and a curved underside.
        let (zf, zb) = (-half - wt / 2.0, -half + wt / 2.0);
        let n = 16;
        let arc = |i: usize| { let th = PI * i as f32 / n as f32; (2.6 * th.cos(), H0 + 3.4 + 2.6 * th.sin()) };
        let sv = |x: f32, y: f32, z: f32, u: f32, v: f32| Vert::new(V3::new(x, y, z), u, v, 1.0);
        for i in 0..n {
            let ((x0, y0), (x1, y1)) = (arc(i), arc(i + 1));
            let yt = H0 + 6.0;
            m.quad(sv(x0, y0, zf, -x0 / 4.0, y0 / 4.0), sv(x1, y1, zf, -x1 / 4.0, y1 / 4.0), sv(x1, yt, zf, -x1 / 4.0, yt / 4.0), sv(x0, yt, zf, -x0 / 4.0, yt / 4.0), stone);
            m.quad(sv(x1, y1, zb, x1 / 4.0, y1 / 4.0), sv(x0, y0, zb, x0 / 4.0, y0 / 4.0), sv(x0, yt, zb, x0 / 4.0, yt / 4.0), sv(x1, yt, zb, x1 / 4.0, yt / 4.0), stone);
            let (a0, a1) = (2.6 * PI * i as f32 / n as f32, 2.6 * PI * (i + 1) as f32 / n as f32);
            m.quad(sv(x0, y0, zf, a0 / 4.0, zf / 4.0), sv(x0, y0, zb, a0 / 4.0, zb / 4.0), sv(x1, y1, zb, a1 / 4.0, zb / 4.0), sv(x1, y1, zf, a1 / 4.0, zf / 4.0), stone);
        }
    }
    // Battlements along the walls, on the outer edge.
    let merlon = |m: &mut Model, x: f32, z: f32, along_x: bool| {
        let (w, d) = if along_x { (0.6, 0.4) } else { (0.4, 0.6) };
        add(m, Model::block(V3::new(x - w, top, z - d), V3::new(x + w, top + 1.5, z + d), stone, 4.0, 2.5), &I);
    };
    let mut p = -half + 3.5;
    while p < half - 3.0 {
        if p.abs() > 7.5 { merlon(&mut m, p, -half - wt / 2.0 + 0.4, true); }
        merlon(&mut m, p, half + wt / 2.0 - 0.4, true);
        merlon(&mut m, -half - wt / 2.0 + 0.4, p, false);
        merlon(&mut m, half + wt / 2.0 - 0.4, p, false);
        p += 2.4;
    }
    // Corner towers with slate cones and arrow slits.
    let slits = |m: &mut Model, cx: f32, cz: f32, r: f32, ys: &[f32], n: usize| {
        for k in 0..n {
            let a = k as f32 / n as f32 * TAU + PI / n as f32;
            let (sx, sz) = (a.sin(), a.cos());
            for &y in ys {
                let c = (cx + sx * (r + 0.03), cz + sz * (r + 0.03));
                let t = (a.cos(), -a.sin());
                wall(m, (c.0 - t.0 * 0.18, c.1 - t.1 * 0.18), (c.0 + t.0 * 0.18, c.1 + t.1 * 0.18), y, y + 1.4, Mat::Tex(tx.win_dark), 1.0, 5.0);
            }
        }
    };
    for (cx, cz) in [(-half, -half), (half, -half), (-half, half), (half, half)] {
        let base = V3::new(cx, y0, cz);
        add(&mut m, Model::cylinder(base, 5.5, 15.0, 24, stone, 4.0), &I);
        add(&mut m, Model::cone(V3::new(cx, H0 + 14.0, cz), 6.3, 7.5, 24, slate, 2.0), &I);
        disc_down(&mut m, V3::new(cx, H0 + 14.0, cz), 6.3, 24, stone, 0.0);
        slits(&mut m, cx, cz, 5.5, &[H0 + 4.0, H0 + 8.5], 8);
        casters.push((V3::new(cx - 3.9, y0, cz - 3.9), V3::new(cx + 3.9, H0 + 19.0, cz + 3.9)));
    }
    // The gatehouse: two round towers with battlements, torches between.
    for x in [-5.6f32, 5.6] {
        let base = V3::new(x, y0, -half);
        add(&mut m, Model::cylinder(base, 3.3, 13.0, 20, stone, 4.0), &I);
        add(&mut m, Model::cone(V3::new(x, H0 + 12.0, -half), 3.3, 0.3, 20, stone, 4.0), &I);
        for k in 0..10 {
            let a = k as f32 / 10.0 * TAU;
            let piece = Model::block(V3::new(-0.5, 0.0, -0.3), V3::new(0.5, 1.3, 0.3), stone, 4.0, 2.5);
            add(&mut m, piece, &M4::rotate_y(a).then(&M4::translate(V3::new(x + a.sin() * 2.9, H0 + 12.0, -half + a.cos() * 2.9))));
        }
        slits(&mut m, x, -half, 3.3, &[H0 + 4.5, H0 + 8.5], 6);
        casters.push((V3::new(x - 2.3, y0, -half - 2.3), V3::new(x + 2.3, H0 + 13.0, -half + 2.3)));
    }
    // The keep: a square tower with a pyramid roof, windows, a door.
    let (kx, kz, kh) = (11.0, 8.0, 22.0);
    block(&mut m, &mut casters, V3::new(-kx, y0, kz - kx), V3::new(kx, H0 + kh, kz + kx), stone, 4.0, true);
    add(&mut m, Model::cone(V3::new(0.0, H0 + kh, kz), kx * 1.414 + 1.2, 9.0, 4, slate, 3.0), &M4::translate(V3::new(0.0, -(H0 + kh), -kz)).then(&M4::rotate_y(PI / 4.0)).then(&M4::translate(V3::new(0.0, H0 + kh, kz))));
    disc_down(&mut m, V3::new(0.0, H0 + kh, kz), kx * 1.414 + 1.2, 4, stone, PI / 4.0);
    for (row, y) in [H0 + 5.0, H0 + 11.0, H0 + 17.0].into_iter().enumerate() {
        for wx in [-5.5f32, 5.5] {
            let mat = Mat::Tex(if row == 2 || (row == 1 && wx > 0.0) { tx.win_lit } else { tx.win_dark });
            wall(&mut m, (wx + 0.5, kz - kx - 0.03), (wx - 0.5, kz - kx - 0.03), y, y + 1.8, mat, 1.0, 5.0);
            wall(&mut m, (wx - 0.5, kz + kx + 0.03), (wx + 0.5, kz + kx + 0.03), y, y + 1.8, mat, 1.0, 5.0);
            wall(&mut m, (-kx - 0.03, kz + wx - 0.5), (-kx - 0.03, kz + wx + 0.5), y, y + 1.8, mat, 1.0, 5.0);
            wall(&mut m, (kx + 0.03, kz + wx + 0.5), (kx + 0.03, kz + wx - 0.5), y, y + 1.8, mat, 1.0, 5.0);
        }
    }
    wall(&mut m, (1.1, kz - kx - 0.03), (-1.1, kz - kx - 0.03), H0, H0 + 3.4, wood, 2.0, 5.0);
    block(&mut m, &mut casters, V3::new(-1.7, H0 - 0.5, kz - kx - 1.3), V3::new(1.7, H0 + 0.3, kz - kx), stone, 4.0, false);
    // Flagpoles on the keep and the gate towers.
    for (x, y, z) in [(0.0, H0 + kh + 9.0, kz), (-5.6, H0 + 12.3, -half), (5.6, H0 + 12.3, -half)] {
        block(&mut m, &mut casters, V3::new(x - 0.08, y, z - 0.08), V3::new(x + 0.08, y + 5.0, z + 0.08), Mat::Flat(0x3a3a40), 1.0, false);
    }
    // Torch brackets at the gate and the keep door.
    for (x, y, z) in torch_spots() {
        block(&mut m, &mut casters, V3::new(x - 0.08, y - 0.45, z - 0.08), V3::new(x + 0.08, y - 0.05, z + 0.08), Mat::Flat(0x2a2a30), 1.0, false);
    }
    // The drawbridge and the well.
    block(&mut m, &mut casters, V3::new(-2.7, H0 - 0.15, -52.5), V3::new(2.7, H0 + 0.15, -40.5), wood, 2.0, false);
    add(&mut m, Model::cylinder(V3::new(-13.0, H0 - 0.3, -13.0), 1.1, 1.1, 14, stone, 4.0), &I);
    for dx in [-0.9f32, 0.9] { block(&mut m, &mut casters, V3::new(-13.0 + dx - 0.07, H0, -13.0 - 0.07), V3::new(-13.0 + dx + 0.07, H0 + 2.4, -13.0 + 0.07), wood, 2.0, false); }
    add(&mut m, Model::cone(V3::new(-13.0, H0 + 2.3, -13.0), 1.7, 0.9, 4, slate, 1.5), &I);
    // Bake the sun: shadows from the big shapes, darker near the ground
    // and inside the gate.
    let cast = casters.clone();
    m.relight(|p, n| {
        let sh = shadow(p.add(n.mul(0.08)).add(sun().mul(0.06)), &cast, false);
        let mut ao = if n.y > 0.5 { 1.0 } else { 0.62 + 0.38 * smooth(((p.y - H0) / 7.0).clamp(0.0, 1.0)) };
        if p.x.abs() < 2.7 && (p.z + half).abs() < wt && p.y < H0 + 6.2 { ao *= 0.55; }
        light(n, sh, ao)
    });
    m.bound();
    (m, casters)
}

fn torch_spots() -> [(f32, f32, f32); 4] {
    [(-5.6 + 2.4, H0 + 5.2, -35.0 - 2.4), (5.6 - 2.4, H0 + 5.2, -35.0 - 2.4), (-2.6, H0 + 3.6, -3.35), (2.6, H0 + 3.6, -3.35)]
}

/// The gate's two leaves, open into the yard.
fn build_doors(tx: &Tx, casters: &[Aabb]) -> Model {
    let mut m = Model::new();
    for side in [-1.0f32, 1.0] {
        let mut leaf = Model::new();
        let (a, b) = ((0.0, 0.0), (side * 2.55, 0.0));
        wall(&mut leaf, a, b, 0.0, 5.9, Mat::Tex(tx.wood), 2.0, 5.0);
        wall(&mut leaf, b, a, 0.0, 5.9, Mat::Tex(tx.wood), 2.0, 5.0);
        m.extend(&leaf, &M4::rotate_y(side * -1.25).then(&M4::translate(V3::new(side * 2.6, H0, -35.0 + 1.0))));
    }
    m.relight(|p, n| light(n, shadow(p.add(n.mul(0.05)), casters, false), 0.7));
    m.bound();
    m
}

/// The path from the hills to the gate and on to the keep's door.
const WAY: [(f32, f32); 9] = [(-40.0, -430.0), (-28.0, -330.0), (-8.0, -232.0), (5.0, -150.0), (2.0, -90.0), (0.0, -60.0), (0.0, -46.0), (0.0, -30.0), (0.0, -16.0)];

fn build_path(tx: &Tx, casters: &[Aabb]) -> Model {
    let mut m = Model::new();
    let mut pts: Vec<(f32, f32)> = WAY.to_vec();
    pts.push((0.0, -4.5));
    let mut along = 0.0f32;
    for w in pts.windows(2) {
        let ((x0, z0), (x1, z1)) = (w[0], w[1]);
        let (dx, dz) = (x1 - x0, z1 - z0);
        let len = (dx * dx + dz * dz).sqrt();
        let (ux, uz) = (dx / len, dz / len);
        let (nx, nz) = (uz * 1.8, -ux * 1.8);
        let n = (len / 3.0).ceil() as usize;
        for i in 0..n {
            let (s0, s1) = (i as f32 / n as f32 * len, (i + 1) as f32 / n as f32 * len);
            let mid = (x0 + ux * (s0 + s1) / 2.0, z0 + uz * (s0 + s1) / 2.0);
            if mid.0.abs() < 3.0 && (-53.0..=-40.0).contains(&mid.1) { continue; }
            let at = |s: f32, side: f32| {
                let (x, z) = (x0 + ux * s + nx * side, z0 + uz * s + nz * side);
                let y = ground(x, z) + 0.06;
                Vert { p: V3::new(x, y, z), u: side * 0.5 + 0.5, v: (along + s) / 4.0, lit: light(normal_at(x, z, 2.0), shadow(V3::new(x, y + 0.3, z), casters, true), 1.0) }
            };
            m.quad(at(s1, -1.0), at(s1, 1.0), at(s0, 1.0), at(s0, -1.0), Mat::Tex(tx.dirt));
        }
        along += len;
    }
    m.bound();
    m
}

fn dist_to_path(x: f32, z: f32) -> f32 {
    let mut best = f32::INFINITY;
    for w in WAY.windows(2) {
        let ((x0, z0), (x1, z1)) = (w[0], w[1]);
        let (dx, dz) = (x1 - x0, z1 - z0);
        let t = (((x - x0) * dx + (z - z0) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
        let (px, pz) = (x0 + dx * t - x, z0 + dz * t - z);
        best = best.min((px * px + pz * pz).sqrt());
    }
    best
}

/// Something standing on the ground as a picture: a pine, a bush, a rock.
struct Tree { pos: V3, w: f32, h: f32, kind: u8, lit: [f32; 3] }

fn plant_trees(casters: &[Aabb]) -> Vec<Tree> {
    let mut rng = Rng::new(7);
    let mut out = Vec::new();
    let lit_at = |x: f32, h: f32, z: f32, up: f32| {
        let l = light(V3::new(0.0, 1.0, 0.0), shadow(V3::new(x, h + up, z), casters, true), 1.0);
        [l[0] * 0.9, l[1] * 0.9, l[2] * 0.9]
    };
    for _ in 0..3200 {
        let (x, z) = (rng.range(-900.0, 900.0), rng.range(-950.0, 650.0));
        if rsq(x, z) < 40.0 || dist_to_path(x, z) < 6.0 { continue; }
        let h = ground(x, z);
        if h > 210.0 || normal_at(x, z, 3.0).y < 0.8 { continue; }
        let density = fbm(x / 260.0 + 9.0, z / 260.0 + 4.0, 3, 91);
        if density < 0.44 || rng.float() > (density - 0.44) * 4.0 { continue; }
        let height = rng.range(10.0, 17.0);
        out.push(Tree { pos: V3::new(x, h - 0.4, z), w: height * 0.5, h: height, kind: 0, lit: lit_at(x, h, z, 6.0) });
    }
    for _ in 0..700 {
        let (x, z) = (rng.range(-120.0, 120.0), rng.range(-470.0, 100.0));
        let d = dist_to_path(x, z);
        if rsq(x, z) < 26.0 || d < 2.6 || d > 60.0 { continue; }
        let h = ground(x, z);
        let rock = rng.chance(0.3);
        let w = if rock { rng.range(0.8, 2.2) } else { rng.range(1.2, 2.4) };
        out.push(Tree { pos: V3::new(x, h - 0.15, z), w, h: if rock { w * 0.5 } else { w * 0.9 }, kind: if rock { 2 } else { 1 }, lit: lit_at(x, h, z, 0.8) });
    }
    out
}

// ─────────────────────────────── the walk ────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Phase { Approach, Court, Reset }

struct Castle {
    scene: Scene,
    tx: Tx,
    land: Vec<Model>,
    castle: Model,
    doors: Model,
    path: Model,
    trees: Vec<Tree>,
    clouds: Vec<f32>,
    cum: Vec<f32>,
    s: f32,
    x: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    speed: f32,
    pace: f32,
    auto: bool,
    idle: f32,
    time: f32,
    phase: Phase,
    phase_t: f32,
    fade: f32,
    step_t: f32,
    audio: Audio,
    steps: Sample,
    paint_ms: f64,
}

impl Castle {
    fn new() -> Castle {
        let t0 = std::time::Instant::now();
        let mut scene = Scene::new(W, H);
        scene.fog = Some((HAZE, 1100.0));
        scene.smooth = true;
        let tx = Tx {
            stone: scene.texture(stone_tex()), slate: scene.texture(slate_tex()), wood: scene.texture(wood_tex()),
            grass: scene.texture(grass_tex()), dirt: scene.texture(dirt_tex()), rock: scene.texture(rock_tex()),
            water: scene.texture(water_tex()), banner: scene.texture(banner_tex()),
            flame: [scene.texture(flame_tex(0)), scene.texture(flame_tex(1)), scene.texture(flame_tex(2)), scene.texture(flame_tex(3))],
            pine: scene.texture(pine_tex()), bush: scene.texture(bush_tex()), boulder: scene.texture(rock_bb_tex()),
            win_dark: scene.texture(window_tex(false)), win_lit: scene.texture(window_tex(true)),
            crow: [scene.texture(crow_tex(true)), scene.texture(crow_tex(false))],
        };
        let (castle, casters) = build_castle(&tx);
        let doors = build_doors(&tx, &casters);
        let path = build_path(&tx, &casters);
        let land = build_land(&casters, &tx);
        let trees = plant_trees(&casters);
        let clouds = (0..CLOUD * CLOUD).map(|i| fbm((i % CLOUD) as f32 / CLOUD as f32 * 8.0, (i / CLOUD) as f32 / CLOUD as f32 * 8.0, 5, 5)).collect();
        let mut cum = vec![0.0f32];
        for w in WAY.windows(2) { cum.push(cum.last().unwrap() + ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()); }
        let mut audio = Audio::open();
        let mut wind = Vec::with_capacity(22050);
        let (mut v, mut x) = (0.0f32, 0x1234_5678u32);
        for _ in 0..22050 { x ^= x << 13; x ^= x >> 17; x ^= x << 5; let r = (x >> 16) as f32 / 32768.0 - 1.0; v += (r - v) * 0.06; wind.push((v * 11000.0) as i16); }
        audio.play_loop(1, &Sample::from_i16(wind), 0.3);
        let steps = Sample::noise(0.05, 0.16);
        if std::env::var_os("CASTLE_BENCH").is_some() { eprintln!("built in {:.2} s", t0.elapsed().as_secs_f64()); }
        let mut c = Castle { scene, tx, land, castle, doors, path, trees, clouds, cum, s: 0.0, x: 0.0, z: 0.0, yaw: 0.0, pitch: 0.0, speed: 0.0, pace: 1.0,
            auto: true, idle: 0.0, time: 0.0, phase: Phase::Approach, phase_t: 0.0, fade: 1.0, step_t: 0.0, audio, steps, paint_ms: 0.0 };
        let (x, z) = c.at(0.0);
        c.x = x; c.z = z;
        c.yaw = c.heading(0.0);
        // `CASTLE_START=metres` begins that far along the walk, for shots.
        if let Some(s) = std::env::var("CASTLE_START").ok().and_then(|v| v.parse::<f32>().ok()) {
            c.s = s.min(c.total());
            let (x, z) = c.at(c.s);
            c.x = x; c.z = z;
            c.yaw = c.heading(c.s + 10.0);
            c.speed = 6.0;
            c.fade = 0.0;
            if c.s >= c.total() { c.phase = Phase::Court; }
        }
        c
    }

    fn total(&self) -> f32 { *self.cum.last().unwrap() }

    /// The point `s` metres along the path.
    fn at(&self, s: f32) -> (f32, f32) {
        let s = s.clamp(0.0, self.total());
        for i in 0..WAY.len() - 1 {
            if s <= self.cum[i + 1] {
                let t = (s - self.cum[i]) / (self.cum[i + 1] - self.cum[i]).max(1e-3);
                return (WAY[i].0 + (WAY[i + 1].0 - WAY[i].0) * t, WAY[i].1 + (WAY[i + 1].1 - WAY[i].1) * t);
            }
        }
        WAY[WAY.len() - 1]
    }

    fn heading(&self, s: f32) -> f32 {
        let (a, b) = (self.at(s), self.at(s + 6.0));
        (b.0 - a.0).atan2(b.1 - a.1)
    }

    fn nearest_s(&self) -> f32 {
        let mut best = (f32::INFINITY, 0.0);
        let mut s = 0.0;
        while s <= self.total() {
            let (x, z) = self.at(s);
            let d = (x - self.x).powi(2) + (z - self.z).powi(2);
            if d < best.0 { best = (d, s); }
            s += 2.0;
        }
        best.1
    }

    fn eye(&self) -> V3 {
        let bob = (self.step_t * TAU).sin() * 0.05 * (self.speed / 5.0).min(1.0);
        V3::new(self.x, floor_at(self.x, self.z) + 1.7 + bob, self.z)
    }
}

fn turn_toward(yaw: &mut f32, target: f32, k: f32) {
    let d = (target - *yaw + PI).rem_euclid(TAU) - PI;
    *yaw += d * k.min(1.0);
}

impl Game for Castle {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        let keys = [Key::Left, Key::Right, Key::Up, Key::Down, Key::Char('a'), Key::Char('d'), Key::Char('w'), Key::Char('s')];
        let steering = keys.iter().any(|&k| input.held(k));
        if steering {
            if self.auto { self.fade = 0.0; }
            self.auto = false;
            self.idle = 0.0;
        } else {
            self.idle += dt;
        }
        if (input.pressed(Key::Space) || self.idle > 25.0) && !self.auto {
            self.auto = true;
            self.s = self.nearest_s();
            self.phase = if self.s > self.total() - 1.0 { Phase::Court } else { Phase::Approach };
            self.phase_t = 0.0;
        }
        if self.auto {
            self.phase_t += dt;
            match self.phase {
                Phase::Approach => {
                    let left = self.total() - self.s;
                    let want = if left < 110.0 { 2.6 + 4.0 * smooth((left / 110.0).clamp(0.0, 1.0)) } else { 6.6 };
                    self.speed += (want - self.speed) * dt * 0.8;
                    self.s += self.speed * dt;
                    let (x, z) = self.at(self.s);
                    self.x = x; self.z = z;
                    let target = self.heading(self.s + 10.0);
                    turn_toward(&mut self.yaw, target, dt * 1.2);
                    // Look a little down a slope, and up at the keep near the gate.
                    let ahead = self.at(self.s + 20.0);
                    let slope = ((floor_at(ahead.0, ahead.1) - floor_at(self.x, self.z)) / 20.0).atan();
                    let up = if left < 60.0 { 0.12 } else { 0.03 };
                    self.pitch += ((slope * 0.6 + up).clamp(-0.25, 0.3) - self.pitch) * dt * 1.5;
                    self.fade = (self.fade - dt * 0.7).max(0.0);
                    if self.s >= self.total() { self.phase = Phase::Court; self.phase_t = 0.0; }
                }
                Phase::Court => {
                    self.speed += (0.0 - self.speed) * dt * 2.0;
                    self.yaw += 0.13 * dt;
                    self.pitch += (0.16 - self.pitch) * dt;
                    if self.phase_t > 42.0 { self.phase = Phase::Reset; self.phase_t = 0.0; }
                }
                Phase::Reset => {
                    self.fade = (self.phase_t / 1.5).min(1.0);
                    if self.phase_t >= 1.6 {
                        self.s = 0.0;
                        let (x, z) = self.at(0.0);
                        self.x = x; self.z = z;
                        self.yaw = self.heading(0.0);
                        self.pitch = 0.0;
                        self.speed = 0.0;
                        self.phase = Phase::Approach;
                        self.phase_t = 0.0;
                    }
                }
            }
        } else {
            let turn = (input.held(Key::Right) as i32 - input.held(Key::Left) as i32) as f32;
            self.yaw += turn * 1.3 * dt;
            if input.held(Key::Char('w')) { self.pace = (self.pace + dt).min(4.0); }
            if input.held(Key::Char('s')) { self.pace = (self.pace - dt).max(0.3); }
            let fwd = (input.held(Key::Up) as i32 - input.held(Key::Down) as i32) as f32;
            let side = (input.held(Key::Char('d')) as i32 - input.held(Key::Char('a')) as i32) as f32;
            let v = 4.5 * self.pace;
            let (sy, cy) = self.yaw.sin_cos();
            self.x += (sy * fwd + cy * side) * v * dt;
            self.z += (cy * fwd - sy * side) * v * dt;
            self.speed = ((fwd.abs() + side.abs()).min(1.0)) * v;
            self.pitch += (0.0 - self.pitch) * dt * 0.5;
        }
        if self.speed > 0.3 {
            let before = self.step_t;
            self.step_t += dt * (0.9 + self.speed * 0.18);
            if before.floor() != self.step_t.floor() { self.audio.play_on(2, &self.steps, 0.5); }
        }
        self.audio.volume(1, 0.22 + 0.02 * (self.time * 0.7).sin());
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        let eye = self.eye();
        let cam = Cam3 { pos: eye, yaw: self.yaw, pitch: self.pitch, focal: W as f32 * 0.72 };
        let scene = &mut self.scene;
        scene.begin(&cam);
        for chunk in &self.land { scene.push(chunk, &I); }
        scene.push(&self.castle, &I);
        scene.push(&self.doors, &I);
        scene.push(&self.path, &I);
        // The moat: one sheet of water, the picture drifting.
        let mut water = Model::new();
        let drift = self.time * 0.02;
        let wv = |x: f32, z: f32| Vert { p: V3::new(x, WATER_Y, z), u: x / 8.0 + drift, v: z / 8.0 + drift * 0.6, lit: [0.85, 0.95, 1.1] };
        water.quad(wv(-58.0, 58.0), wv(58.0, 58.0), wv(58.0, -58.0), wv(-58.0, -58.0), Mat::Tex(self.tx.water));
        scene.push(&water, &I);
        // Banners in the wind.
        let mut cloth = Model::new();
        for (x, y, z) in [(0.0, H0 + 22.0 + 9.0 + 4.9, 8.0), (-5.6, H0 + 12.3 + 4.9, -35.0), (5.6, H0 + 12.3 + 4.9, -35.0)] {
            let t = self.time * 5.0 + x;
            let (len, drop) = (2.4f32, 1.3f32);
            let pt = |s: f32, d: f32| {
                let wave = (s * 5.0 - t).sin() * 0.28 * s + (s * 9.0 - t * 1.6).sin() * 0.08 * s;
                Vert { p: V3::new(x + 0.1 + s * len, y - d * drop - s * s * 0.25, z + wave), u: s, v: d, lit: [1.05 - d * 0.25, 1.0 - d * 0.25, 0.9 - d * 0.2] }
            };
            let (nu, nv) = (10, 4);
            for j in 0..nv {
                for i in 0..nu {
                    let (s0, s1, d0, d1) = (i as f32 / nu as f32, (i + 1) as f32 / nu as f32, j as f32 / nv as f32, (j + 1) as f32 / nv as f32);
                    cloth.quad(pt(s0, d1), pt(s1, d1), pt(s1, d0), pt(s0, d0), Mat::Tex(self.tx.banner));
                    cloth.quad(pt(s1, d1), pt(s0, d1), pt(s0, d0), pt(s1, d0), Mat::Tex(self.tx.banner));
                }
            }
        }
        scene.push(&cloth, &I);
        // Trees, flames and crows stand up facing the eye.
        for t in &self.trees {
            let d = (t.pos.x - eye.x).powi(2) + (t.pos.z - eye.z).powi(2);
            let (mat, far) = match t.kind { 0 => (self.tx.pine, 650.0f32), 1 => (self.tx.bush, 160.0), _ => (self.tx.boulder, 160.0) };
            if d < far * far { scene.billboard(t.pos, t.w, t.h, Mat::Tex(mat), t.lit); }
        }
        let frame = ((self.time * 11.0) as usize) % 4;
        for (i, (x, y, z)) in torch_spots().into_iter().enumerate() {
            let flick = 1.0 + 0.12 * ((self.time * 17.0 + i as f32 * 1.7).sin());
            scene.billboard(V3::new(x, y - 0.1, z), 0.5 * flick, 1.0 * flick, Mat::Tex(self.tx.flame[(frame + i) % 4]), [1.5, 1.35, 1.1]);
        }
        for i in 0..6 {
            let a = self.time * 0.35 + i as f32 * 1.05;
            let r = 26.0 + 6.0 * (a * 0.5).sin();
            let p = V3::new(a.sin() * r, H0 + 44.0 + 4.0 * (a * 1.3 + i as f32).sin(), 8.0 + a.cos() * r);
            let flap = ((self.time * 9.0 + i as f32) as usize) % 2;
            scene.billboard(p, 1.3, 1.3, Mat::Tex(self.tx.crow[flap]), [0.25, 0.25, 0.3]);
        }
        // The sky through every pixel: a gradient, the sun, clouds on a
        // sheet high above.
        let clouds = &self.clouds;
        let (time, sundir) = (self.time, sun());
        let sky = move |d: V3| {
            let e = d.y;
            let mut c = lerp(HAZE, ZENITH, e.max(0.0).powf(0.55));
            let s = d.dot(sundir).max(0.0);
            if s > 0.99975 { return SUNCOL; }
            c = lerp(c, SUNCOL, s.powi(40) * 0.9 + s.powi(5) * 0.22);
            if e > 0.015 {
                let dist = (1500.0 - eye.y) / e;
                let (px, pz) = (eye.x + d.x * dist + time * 9.0, eye.z + d.z * dist);
                let (tx, tz) = (px / 2600.0 * CLOUD as f32, pz / 2600.0 * CLOUD as f32);
                let (xi, zi) = (tx.floor(), tz.floor());
                let (fx, fz) = (tx - xi, tz - zi);
                let m = CLOUD - 1;
                let (x0, z0) = (xi as i64 as usize & m, zi as i64 as usize & m);
                let (x1, z1) = ((x0 + 1) & m, (z0 + 1) & m);
                let a = clouds[z0 * CLOUD + x0] + (clouds[z0 * CLOUD + x1] - clouds[z0 * CLOUD + x0]) * fx;
                let b = clouds[z1 * CLOUD + x0] + (clouds[z1 * CLOUD + x1] - clouds[z1 * CLOUD + x0]) * fx;
                let n = a + (b - a) * fz;
                let fade = (1.0 - dist / 16000.0).clamp(0.0, 1.0);
                let cover = ((n - 0.5) * 3.4).clamp(0.0, 1.0) * fade;
                if cover > 0.0 { c = lerp(c, 0xfaf5ec, cover * 0.85); }
            }
            c
        };
        let t0 = std::time::Instant::now();
        scene.render(f, &sky);
        self.paint_ms += t0.elapsed().as_secs_f64() * 1000.0;
        if self.auto { f.text_big(8, H - 14, "AUTO", 0xe8e0d0); }
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x9a9080);
        if self.fade > 0.0 { f.dim(1.0 - self.fade); }
    }
}

fn main() {
    let mut game = Castle::new();
    if let Ok(n) = std::env::var("CASTLE_BENCH") {
        let n: u32 = n.parse().unwrap_or(100);
        let mut f = Frame::new(W, H);
        let input = Input::new();
        let t0 = std::time::Instant::now();
        for _ in 0..n { game.update(&input, 1.0 / 30.0); game.draw(&mut f); }
        eprintln!("{:.2} ms a frame at {}x{} over {} frames ({:.1} ms of it painting)", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n, game.paint_ms / n as f64);
        if let Ok(p) = std::env::var("FUNKEY_SHOT") { let _ = std::fs::write(p, f.to_ppm()); }
        return;
    }
    run(&mut game, Config { width: W, height: H, fps: 30 });
}
