//! A software 3D rasterizer, twice over.
//!
//! [`Raster`] draws meshes of flat-shaded triangles straight into the
//! frame, one thread, with a depth buffer, near-plane clipping, a light
//! direction and fog. Enough for a road over hills and a car on it.
//!
//! [`Scene`] takes [`Model`]s of textured triangles with a baked light
//! per vertex, collects a frame's worth, then paints the rows on every
//! core at once. Textures wrap, have holes ([`CUTOUT`]) and shrink into
//! mip levels so far ground does not shimmer. Enough for a castle.

use crate::frame::{parts, rgb, Frame, Rgb};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct V3 { pub x: f32, pub y: f32, pub z: f32 }

impl V3 {
    pub const fn new(x: f32, y: f32, z: f32) -> V3 { V3 { x, y, z } }
    pub fn add(self, o: V3) -> V3 { V3::new(self.x + o.x, self.y + o.y, self.z + o.z) }
    pub fn sub(self, o: V3) -> V3 { V3::new(self.x - o.x, self.y - o.y, self.z - o.z) }
    pub fn mul(self, k: f32) -> V3 { V3::new(self.x * k, self.y * k, self.z * k) }
    pub fn dot(self, o: V3) -> f32 { self.x * o.x + self.y * o.y + self.z * o.z }
    pub fn cross(self, o: V3) -> V3 { V3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x) }
    pub fn len(self) -> f32 { self.dot(self).sqrt() }
    pub fn norm(self) -> V3 { let l = self.len(); if l > 0.0 { self.mul(1.0 / l) } else { self } }
    pub fn lerp(self, o: V3, t: f32) -> V3 { self.add(o.sub(self).mul(t)) }
}

/// A 4 by 4 matrix, row major, for moving and turning things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M4(pub [[f32; 4]; 4]);

impl M4 {
    pub fn identity() -> M4 { M4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]) }
    pub fn translate(v: V3) -> M4 { let mut m = M4::identity(); m.0[0][3] = v.x; m.0[1][3] = v.y; m.0[2][3] = v.z; m }
    pub fn scale(k: f32) -> M4 { let mut m = M4::identity(); m.0[0][0] = k; m.0[1][1] = k; m.0[2][2] = k; m }
    pub fn rotate_x(a: f32) -> M4 { let (s, c) = a.sin_cos(); let mut m = M4::identity(); m.0[1][1] = c; m.0[1][2] = -s; m.0[2][1] = s; m.0[2][2] = c; m }
    pub fn rotate_y(a: f32) -> M4 { let (s, c) = a.sin_cos(); let mut m = M4::identity(); m.0[0][0] = c; m.0[0][2] = s; m.0[2][0] = -s; m.0[2][2] = c; m }
    pub fn rotate_z(a: f32) -> M4 { let (s, c) = a.sin_cos(); let mut m = M4::identity(); m.0[0][0] = c; m.0[0][1] = -s; m.0[1][0] = s; m.0[1][1] = c; m }

    /// This, then `o`: `a.then(b)` applies a first.
    pub fn then(&self, o: &M4) -> M4 { o.mul(self) }

    pub fn mul(&self, o: &M4) -> M4 {
        let mut r = [[0.0f32; 4]; 4];
        for i in 0..4 { for j in 0..4 { for k in 0..4 { r[i][j] += self.0[i][k] * o.0[k][j]; } } }
        M4(r)
    }

    /// A point through the matrix.
    pub fn apply(&self, v: V3) -> V3 {
        let m = &self.0;
        V3::new(m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z + m[0][3],
                m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z + m[1][3],
                m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z + m[2][3])
    }

    /// A direction through the matrix: turned, not moved.
    pub fn apply_dir(&self, v: V3) -> V3 {
        let m = &self.0;
        V3::new(m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
                m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
                m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z)
    }
}

/// Triangles with a colour each. Faces are counter-clockwise seen from
/// outside.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub verts: Vec<V3>,
    pub tris: Vec<[u32; 3]>,
    pub colors: Vec<Rgb>,
}

impl Mesh {
    pub fn new() -> Mesh { Mesh::default() }

    pub fn tri(&mut self, a: V3, b: V3, c: V3, color: Rgb) {
        let n = self.verts.len() as u32;
        self.verts.extend_from_slice(&[a, b, c]);
        self.tris.push([n, n + 1, n + 2]);
        self.colors.push(color);
    }

    pub fn quad(&mut self, a: V3, b: V3, c: V3, d: V3, color: Rgb) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }

    /// A box centred on the origin.
    pub fn cuboid(w: f32, h: f32, d: f32, color: Rgb) -> Mesh {
        let (x, y, z) = (w / 2.0, h / 2.0, d / 2.0);
        let p = |sx: f32, sy: f32, sz: f32| V3::new(sx * x, sy * y, sz * z);
        let mut m = Mesh::new();
        m.quad(p(-1.0, -1.0, 1.0), p(1.0, -1.0, 1.0), p(1.0, 1.0, 1.0), p(-1.0, 1.0, 1.0), color);
        m.quad(p(1.0, -1.0, -1.0), p(-1.0, -1.0, -1.0), p(-1.0, 1.0, -1.0), p(1.0, 1.0, -1.0), color);
        m.quad(p(-1.0, -1.0, -1.0), p(-1.0, -1.0, 1.0), p(-1.0, 1.0, 1.0), p(-1.0, 1.0, -1.0), color);
        m.quad(p(1.0, -1.0, 1.0), p(1.0, -1.0, -1.0), p(1.0, 1.0, -1.0), p(1.0, 1.0, 1.0), color);
        m.quad(p(-1.0, 1.0, 1.0), p(1.0, 1.0, 1.0), p(1.0, 1.0, -1.0), p(-1.0, 1.0, -1.0), color);
        m.quad(p(-1.0, -1.0, -1.0), p(1.0, -1.0, -1.0), p(1.0, -1.0, 1.0), p(-1.0, -1.0, 1.0), color);
        m
    }

    pub fn extend(&mut self, o: &Mesh, m: &M4) {
        let base = self.verts.len() as u32;
        self.verts.extend(o.verts.iter().map(|&v| m.apply(v)));
        self.tris.extend(o.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        self.colors.extend_from_slice(&o.colors);
    }
}

/// Where the eye is and which way it looks. Yaw turns about the up
/// axis, pitch nods; both in radians. `focal` is the distance from the
/// eye to the screen in pixels: the width for a 53 degree view.
#[derive(Clone, Copy, Debug)]
pub struct Cam3 { pub pos: V3, pub yaw: f32, pub pitch: f32, pub focal: f32 }

impl Cam3 {
    /// World to view: x right, y up, z into the screen.
    pub fn view(&self) -> M4 {
        M4::translate(self.pos.mul(-1.0)).then(&M4::rotate_y(-self.yaw)).then(&M4::rotate_x(self.pitch))
    }

    pub fn forward(&self) -> V3 { V3::new(self.yaw.sin(), 0.0, self.yaw.cos()) }
}

pub struct Raster {
    pub w: i32,
    pub h: i32,
    zbuf: Vec<f32>,
    pub near: f32,
    /// Direction the light comes from, in world space.
    pub light: V3,
    pub ambient: f32,
    /// Fog colour and the distance at which it is complete.
    pub fog: Option<(Rgb, f32)>,
}

impl Raster {
    pub fn new(w: i32, h: i32) -> Raster {
        Raster { w, h, zbuf: vec![f32::INFINITY; (w * h) as usize], near: 0.5, light: V3::new(0.4, 1.0, -0.3).norm(), ambient: 0.35, fog: None }
    }

    /// Start a frame: forget the depths.
    pub fn clear(&mut self) { self.zbuf.fill(f32::INFINITY); }

    /// Draw a mesh placed by `model` as seen by the camera.
    pub fn draw(&mut self, f: &mut Frame, mesh: &Mesh, model: &M4, cam: &Cam3) {
        let mv = model.then(&cam.view());
        let world_verts: Vec<V3> = mesh.verts.iter().map(|&v| model.apply(v)).collect();
        let view_verts: Vec<V3> = mesh.verts.iter().map(|&v| mv.apply(v)).collect();
        for (t, &color) in mesh.tris.iter().zip(&mesh.colors) {
            let (a, b, c) = (view_verts[t[0] as usize], view_verts[t[1] as usize], view_verts[t[2] as usize]);
            // Facing away: the winding seen from the eye is clockwise.
            let n = b.sub(a).cross(c.sub(a));
            if n.dot(a) >= 0.0 { continue; }
            if a.z < self.near && b.z < self.near && c.z < self.near { continue; }
            // Light in world space, flat over the face.
            let (wa, wb, wc) = (world_verts[t[0] as usize], world_verts[t[1] as usize], world_verts[t[2] as usize]);
            let wn = wb.sub(wa).cross(wc.sub(wa)).norm();
            let lit = self.ambient + (1.0 - self.ambient) * wn.dot(self.light).max(0.0);
            let shaded = shade(color, lit);
            // Clip against the near plane, then fan.
            let poly = clip_near(&[a, b, c], self.near);
            if poly.len() < 3 { continue; }
            let pts: Vec<(f32, f32, f32)> = poly.iter().map(|v| self.project(*v)).collect();
            for i in 1..pts.len() - 1 { self.fill(f, pts[0], pts[i], pts[i + 1], shaded); }
        }
    }

    fn project(&self, v: V3) -> (f32, f32, f32) {
        let z = v.z.max(self.near);
        (self.w as f32 / 2.0 + v.x * self.focal_of() / z, self.h as f32 / 2.0 - v.y * self.focal_of() / z, z)
    }

    fn focal_of(&self) -> f32 { self.w as f32 }

    fn fill(&mut self, f: &mut Frame, p0: (f32, f32, f32), p1: (f32, f32, f32), p2: (f32, f32, f32), color: Rgb) {
        let mut v = [p0, p1, p2];
        v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let (t, m, b) = (v[0], v[1], v[2]);
        // A pixel belongs to the triangle when its centre does, so two
        // triangles sharing an edge share no pixel and leave no gap.
        let y0 = ((t.1 - 0.5).ceil() as i32).max(0);
        let y1 = ((b.1 - 0.5).ceil() as i32 - 1).min(self.h - 1);
        // Interpolate x and 1/z down both edges.
        let edge = |a: (f32, f32, f32), c: (f32, f32, f32), y: f32| {
            let s = if c.1 - a.1 > 1e-6 { ((y - a.1) / (c.1 - a.1)).clamp(0.0, 1.0) } else { 1.0 };
            (a.0 + (c.0 - a.0) * s, 1.0 / a.2 + (1.0 / c.2 - 1.0 / a.2) * s)
        };
        for y in y0..=y1 {
            let yc = y as f32 + 0.5;
            let (xa, za) = edge(t, b, yc);
            let (xb, zb) = if yc < m.1 { edge(t, m, yc) } else { edge(m, b, yc) };
            let ((xl, zl), (xr, zr)) = if xa < xb { ((xa, za), (xb, zb)) } else { ((xb, zb), (xa, za)) };
            let x0 = ((xl - 0.5).ceil() as i32).max(0);
            let x1 = ((xr - 0.5).ceil() as i32 - 1).min(self.w - 1);
            if x0 > x1 { continue; }
            let row = (y * self.w) as usize;
            for x in x0..=x1 {
                let s = if xr - xl > 0.001 { (x as f32 + 0.5 - xl) / (xr - xl) } else { 0.0 };
                let iz = zl + (zr - zl) * s;
                let i = row + x as usize;
                if iz > 1.0 / self.zbuf[i] {
                    self.zbuf[i] = 1.0 / iz;
                    f.px[i] = match self.fog { Some((fc, d)) => blend(color, fc, (1.0 / iz / d).clamp(0.0, 1.0)), None => color };
                }
            }
        }
    }
}

fn shade3(c: Rgb, k: [f32; 3]) -> Rgb {
    let (r, g, b) = parts(c);
    rgb((r as f32 * k[0]).clamp(0.0, 255.0) as u8, (g as f32 * k[1]).clamp(0.0, 255.0) as u8, (b as f32 * k[2]).clamp(0.0, 255.0) as u8)
}

fn shade(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    rgb((r as f32 * k).min(255.0) as u8, (g as f32 * k).min(255.0) as u8, (b as f32 * k).min(255.0) as u8)
}

pub fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let (ar, ag, ab) = parts(a);
    let (br, bg, bb) = parts(b);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

/// Cut a polygon at z = near, keeping the far side.
fn clip_near(poly: &[V3], near: f32) -> Vec<V3> {
    let mut out = Vec::with_capacity(4);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ina, inb) = (a.z >= near, b.z >= near);
        if ina { out.push(a); }
        if ina != inb {
            let t = (near - a.z) / (b.z - a.z);
            out.push(a.lerp(b, t));
        }
    }
    out
}

// ─────────────────────────── textured scenes ───────────────────────────

/// Texels of this colour are holes: the sky between a tree's branches.
pub const CUTOUT: Rgb = 0xff00ff;

/// A picture wrapped over triangles. Width and height are powers of two,
/// so a coordinate past the edge wraps round. Smaller copies are made at
/// once, each half the size, and a triangle far away reads the copy
/// whose texels are about a pixel wide.
#[derive(Clone, Debug)]
pub struct Texture {
    pub w: u32,
    pub h: u32,
    levels: Vec<Vec<Rgb>>,
}

impl Texture {
    /// `f(x, y)` gives each texel. `w` and `h` must be powers of two.
    pub fn from_fn(w: u32, h: u32, f: impl Fn(u32, u32) -> Rgb) -> Texture {
        assert!(w.is_power_of_two() && h.is_power_of_two(), "texture sides must be powers of two");
        let base: Vec<Rgb> = (0..w * h).map(|i| f(i % w, i / w)).collect();
        let mut levels = vec![base];
        let (mut lw, mut lh) = (w, h);
        while lw > 1 && lh > 1 {
            let prev = levels.last().unwrap();
            let (nw, nh) = (lw / 2, lh / 2);
            let mut next = Vec::with_capacity((nw * nh) as usize);
            for y in 0..nh {
                for x in 0..nw {
                    let s = [prev[((2 * y) * lw + 2 * x) as usize], prev[((2 * y) * lw + 2 * x + 1) as usize],
                             prev[((2 * y + 1) * lw + 2 * x) as usize], prev[((2 * y + 1) * lw + 2 * x + 1) as usize]];
                    let solid: Vec<Rgb> = s.iter().copied().filter(|&c| c != CUTOUT).collect();
                    if solid.len() < 2 { next.push(CUTOUT); continue; }
                    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                    for &c in &solid { let (cr, cg, cb) = parts(c); r += cr as u32; g += cg as u32; b += cb as u32; }
                    let n = solid.len() as u32;
                    next.push(rgb((r / n) as u8, (g / n) as u8, (b / n) as u8));
                }
            }
            levels.push(next);
            lw = nw;
            lh = nh;
        }
        Texture { w, h, levels }
    }

    /// One colour all over.
    pub fn solid(c: Rgb) -> Texture { Texture::from_fn(1, 1, |_, _| c) }

    pub fn levels(&self) -> usize { self.levels.len() }

    /// The colour under (u, v) blended from the four texels round it, so
    /// a wall seen up close is smooth instead of blocky. Next to a hole
    /// the nearest texel is used as it is, so the hole's colour never
    /// bleeds.
    #[inline]
    pub fn smooth_texel(&self, u: f32, v: f32) -> Rgb {
        let (fx, fy) = (u * self.w as f32 - 0.5, v * self.h as f32 - 0.5);
        let (x0f, y0f) = (fx.floor(), fy.floor());
        let (tx, ty) = (((fx - x0f) * 256.0) as u32, ((fy - y0f) * 256.0) as u32);
        let (mx, my) = (self.w - 1, self.h - 1);
        let (x0, y0) = (x0f as i64 as u32 & mx, y0f as i64 as u32 & my);
        let (x1, y1) = ((x0 + 1) & mx, (y0 + 1) & my);
        let l = &self.levels[0];
        let c = [l[(y0 * self.w + x0) as usize], l[(y0 * self.w + x1) as usize], l[(y1 * self.w + x0) as usize], l[(y1 * self.w + x1) as usize]];
        if c.iter().any(|&t| t == CUTOUT) { return c[(if tx > 128 { 1 } else { 0 }) + if ty > 128 { 2 } else { 0 }]; }
        let w = [(256 - tx) * (256 - ty), tx * (256 - ty), (256 - tx) * ty, tx * ty];
        let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
        for i in 0..4 { r += ((c[i] >> 16) & 255) * w[i]; g += ((c[i] >> 8) & 255) * w[i]; b += (c[i] & 255) * w[i]; }
        ((r >> 16) << 16) | ((g >> 16) << 8) | (b >> 16)
    }

    /// The texel under (u, v) at a mip level, wrapping. u and v run 0..1
    /// over the picture once.
    #[inline]
    pub fn texel(&self, level: usize, u: f32, v: f32) -> Rgb {
        let level = level.min(self.levels.len() - 1);
        let (w, h) = (self.w >> level, self.h >> level);
        let x = (u * w as f32).floor() as i64 as u32 & (w - 1);
        let y = (v * h as f32).floor() as i64 as u32 & (h - 1);
        self.levels[level][(y * w + x) as usize]
    }
}

/// A corner of a textured triangle: where it is, where on the picture
/// it reads, and how lit it is, red, green and blue apart: 1 is the
/// texture as drawn, more glows, and a shadow can lean blue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vert { pub p: V3, pub u: f32, pub v: f32, pub lit: [f32; 3] }

impl Vert {
    pub const fn new(p: V3, u: f32, v: f32, lit: f32) -> Vert { Vert { p, u, v, lit: [lit, lit, lit] } }
    pub fn lerp(self, o: Vert, t: f32) -> Vert {
        let l = |i: usize| self.lit[i] + (o.lit[i] - self.lit[i]) * t;
        Vert { p: self.p.lerp(o.p, t), u: self.u + (o.u - self.u) * t, v: self.v + (o.v - self.v) * t, lit: [l(0), l(1), l(2)] }
    }
}

/// What a triangle is painted with: one colour, or a texture in the
/// scene's list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mat { Flat(Rgb), Tex(usize) }

/// Textured triangles with a light per corner. Faces wind like
/// [`Mesh::cuboid`]'s: `(b - a) x (c - a)` points outward. `bound` fills in the sphere the
/// scene uses to skip a model that is off screen.
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub verts: Vec<Vert>,
    pub tris: Vec<([u32; 3], Mat)>,
    pub centre: V3,
    pub radius: f32,
}

impl Model {
    pub fn new() -> Model { Model::default() }

    pub fn tri(&mut self, a: Vert, b: Vert, c: Vert, mat: Mat) {
        let n = self.verts.len() as u32;
        self.verts.extend_from_slice(&[a, b, c]);
        self.tris.push(([n, n + 1, n + 2], mat));
    }

    pub fn quad(&mut self, a: Vert, b: Vert, c: Vert, d: Vert, mat: Mat) {
        let n = self.verts.len() as u32;
        self.verts.extend_from_slice(&[a, b, c, d]);
        self.tris.push(([n, n + 1, n + 2], mat));
        self.tris.push(([n, n + 2, n + 3], mat));
    }

    /// A quad cut into `nu` by `nv` pieces, so a baked light or a shadow
    /// edge can change across it. Corners go round: a, b along u; d, c
    /// above them.
    pub fn patch(&mut self, a: Vert, b: Vert, c: Vert, d: Vert, nu: u32, nv: u32, mat: Mat) {
        let (nu, nv) = (nu.max(1), nv.max(1));
        let at = |i: u32, j: u32| {
            let (s, t) = (i as f32 / nu as f32, j as f32 / nv as f32);
            a.lerp(b, s).lerp(d.lerp(c, s), t)
        };
        for j in 0..nv {
            for i in 0..nu {
                self.quad(at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1), mat);
            }
        }
    }

    /// A box from `min` to `max`, textured by where its faces are in the
    /// world: `scale` metres of wall per repeat of the picture, so the
    /// pattern runs on round corners. Faces are cut every `seg` metres.
    pub fn block(min: V3, max: V3, mat: Mat, scale: f32, seg: f32) -> Model {
        let mut m = Model::new();
        let k = 1.0 / scale;
        let n = |len: f32| ((len / seg).ceil() as u32).max(1);
        let v = |p: V3, u: f32, vv: f32| Vert::new(p, u * k, vv * k, 1.0);
        let (a, b) = (min, max);
        // front (-z), back (+z), left (-x), right (+x), top, bottom
        m.patch(v(V3::new(b.x, a.y, a.z), -b.x, a.y), v(V3::new(a.x, a.y, a.z), -a.x, a.y), v(V3::new(a.x, b.y, a.z), -a.x, b.y), v(V3::new(b.x, b.y, a.z), -b.x, b.y), n(b.x - a.x), n(b.y - a.y), mat);
        m.patch(v(V3::new(a.x, a.y, b.z), a.x, a.y), v(V3::new(b.x, a.y, b.z), b.x, a.y), v(V3::new(b.x, b.y, b.z), b.x, b.y), v(V3::new(a.x, b.y, b.z), a.x, b.y), n(b.x - a.x), n(b.y - a.y), mat);
        m.patch(v(V3::new(a.x, a.y, a.z), a.z, a.y), v(V3::new(a.x, a.y, b.z), b.z, a.y), v(V3::new(a.x, b.y, b.z), b.z, b.y), v(V3::new(a.x, b.y, a.z), a.z, b.y), n(b.z - a.z), n(b.y - a.y), mat);
        m.patch(v(V3::new(b.x, a.y, b.z), -b.z, a.y), v(V3::new(b.x, a.y, a.z), -a.z, a.y), v(V3::new(b.x, b.y, a.z), -a.z, b.y), v(V3::new(b.x, b.y, b.z), -b.z, b.y), n(b.z - a.z), n(b.y - a.y), mat);
        m.patch(v(V3::new(a.x, b.y, b.z), a.x, b.z), v(V3::new(b.x, b.y, b.z), b.x, b.z), v(V3::new(b.x, b.y, a.z), b.x, a.z), v(V3::new(a.x, b.y, a.z), a.x, a.z), n(b.x - a.x), n(b.z - a.z), mat);
        m.patch(v(V3::new(a.x, a.y, a.z), a.x, a.z), v(V3::new(b.x, a.y, a.z), b.x, a.z), v(V3::new(b.x, a.y, b.z), b.x, b.z), v(V3::new(a.x, a.y, b.z), a.x, b.z), n(b.x - a.x), n(b.z - a.z), mat);
        m
    }

    /// A tube standing on `base`, `n` sides, open at both ends. The
    /// picture repeats every `scale` metres round and up.
    pub fn cylinder(base: V3, r: f32, h: f32, n: u32, mat: Mat, scale: f32) -> Model {
        let mut m = Model::new();
        let n = n.max(3);
        let round = std::f32::consts::TAU * r;
        for i in 0..n {
            let (a0, a1) = (i as f32 / n as f32 * std::f32::consts::TAU, (i + 1) as f32 / n as f32 * std::f32::consts::TAU);
            let (u0, u1) = (i as f32 / n as f32 * round / scale, (i + 1) as f32 / n as f32 * round / scale);
            let p0 = V3::new(base.x + a0.sin() * r, base.y, base.z + a0.cos() * r);
            let p1 = V3::new(base.x + a1.sin() * r, base.y, base.z + a1.cos() * r);
            m.patch(Vert::new(p0, u0, base.y / scale, 1.0), Vert::new(p1, u1, base.y / scale, 1.0),
                    Vert::new(V3::new(p1.x, base.y + h, p1.z), u1, (base.y + h) / scale, 1.0),
                    Vert::new(V3::new(p0.x, base.y + h, p0.z), u0, (base.y + h) / scale, 1.0), 1, ((h / scale * 2.0).ceil() as u32).max(1), mat);
        }
        m
    }

    /// A cone standing on `base`, its point `h` up.
    pub fn cone(base: V3, r: f32, h: f32, n: u32, mat: Mat, scale: f32) -> Model {
        let mut m = Model::new();
        let n = n.max(3);
        let round = std::f32::consts::TAU * r;
        let slant = (r * r + h * h).sqrt();
        let top = V3::new(base.x, base.y + h, base.z);
        for i in 0..n {
            let (a0, a1) = (i as f32 / n as f32 * std::f32::consts::TAU, (i + 1) as f32 / n as f32 * std::f32::consts::TAU);
            let (u0, u1) = (i as f32 / n as f32 * round / scale, (i + 1) as f32 / n as f32 * round / scale);
            let p0 = V3::new(base.x + a0.sin() * r, base.y, base.z + a0.cos() * r);
            let p1 = V3::new(base.x + a1.sin() * r, base.y, base.z + a1.cos() * r);
            m.tri(Vert::new(p0, u0, 0.0, 1.0), Vert::new(p1, u1, 0.0, 1.0), Vert::new(top, (u0 + u1) / 2.0, slant / scale, 1.0), mat);
        }
        m
    }

    /// Another model placed by `m`, added to this one.
    pub fn extend(&mut self, o: &Model, m: &M4) {
        let base = self.verts.len() as u32;
        self.verts.extend(o.verts.iter().map(|v| Vert { p: m.apply(v.p), ..*v }));
        self.tris.extend(o.tris.iter().map(|(t, mat)| ([t[0] + base, t[1] + base, t[2] + base], *mat)));
    }

    /// Set every corner's light from where it is and which way its face
    /// points: the place to bake a sun and its shadows.
    pub fn relight(&mut self, f: impl Fn(V3, V3) -> [f32; 3]) {
        let mut normals = vec![V3::default(); self.verts.len()];
        for (t, _) in &self.tris {
            let (a, b, c) = (self.verts[t[0] as usize].p, self.verts[t[1] as usize].p, self.verts[t[2] as usize].p);
            let n = b.sub(a).cross(c.sub(a)).norm();
            for &i in t { normals[i as usize] = normals[i as usize].add(n); }
        }
        for (v, n) in self.verts.iter_mut().zip(normals) { v.lit = f(v.p, n.norm()); }
    }

    /// Work out the sphere round the model, for culling.
    pub fn bound(&mut self) {
        if self.verts.is_empty() { return; }
        let mut c = V3::default();
        for v in &self.verts { c = c.add(v.p); }
        c = c.mul(1.0 / self.verts.len() as f32);
        self.centre = c;
        self.radius = self.verts.iter().map(|v| v.p.sub(c).len()).fold(0.0, f32::max);
    }
}

/// A corner on the screen: position, and the depth and texture
/// coordinates divided by depth, which is what varies evenly across
/// the pixels.
#[derive(Clone, Copy, Debug)]
struct SV { x: f32, y: f32, iz: f32, uz: f32, vz: f32, lit: [f32; 3] }

impl SV {
    fn lerp(self, o: SV, t: f32) -> SV {
        let l = |i: usize| self.lit[i] + (o.lit[i] - self.lit[i]) * t;
        SV { x: self.x + (o.x - self.x) * t, y: self.y + (o.y - self.y) * t, iz: self.iz + (o.iz - self.iz) * t,
             uz: self.uz + (o.uz - self.uz) * t, vz: self.vz + (o.vz - self.vz) * t, lit: [l(0), l(1), l(2)] }
    }
}

struct STri { v: [SV; 3], mat: Mat, level: usize, mag: bool, y0: i32, y1: i32 }

/// A frame's worth of textured triangles, painted on every core.
///
/// Each frame: [`begin`](Scene::begin) with the camera, [`push`](Scene::push)
/// the models, then [`render`](Scene::render) into the frame with a sky.
pub struct Scene {
    pub w: i32,
    pub h: i32,
    pub near: f32,
    /// Fog colour and the distance over which it thickens (it never
    /// quite completes: at three times the distance a twentieth is left).
    pub fog: Option<(Rgb, f32)>,
    pub textures: Vec<Texture>,
    /// Blend texels on surfaces close enough that a texel covers more
    /// than a pixel. Costs a little on those surfaces only.
    pub smooth: bool,
    /// Cores to paint with; 0 means all of them.
    pub threads: usize,
    tris: Vec<STri>,
    zbuf: Vec<f32>,
    view: M4,
    focal: f32,
    right: V3,
    up: V3,
    fwd: V3,
    scratch: Vec<V3>,
}

impl Scene {
    pub fn new(w: i32, h: i32) -> Scene {
        Scene { w, h, near: 0.3, fog: None, textures: Vec::new(), smooth: false, threads: 0, tris: Vec::new(), zbuf: vec![0.0; (w * h) as usize],
                view: M4::identity(), focal: w as f32, right: V3::new(1.0, 0.0, 0.0), up: V3::new(0.0, 1.0, 0.0), fwd: V3::new(0.0, 0.0, 1.0), scratch: Vec::new() }
    }

    /// Add a texture; the index is what `Mat::Tex` names.
    pub fn texture(&mut self, t: Texture) -> usize { self.textures.push(t); self.textures.len() - 1 }

    /// Start a frame as seen by the camera.
    pub fn begin(&mut self, cam: &Cam3) {
        self.tris.clear();
        self.view = cam.view();
        self.focal = cam.focal;
        let back = M4::rotate_x(-cam.pitch).then(&M4::rotate_y(cam.yaw));
        self.right = back.apply_dir(V3::new(1.0, 0.0, 0.0));
        self.up = back.apply_dir(V3::new(0.0, 1.0, 0.0));
        self.fwd = back.apply_dir(V3::new(0.0, 0.0, 1.0));
    }

    /// The world-space direction a pixel looks along.
    pub fn ray(&self, x: f32, y: f32) -> V3 {
        self.fwd.mul(self.focal).add(self.right.mul(x - self.w as f32 / 2.0)).add(self.up.mul(self.h as f32 / 2.0 - y)).norm()
    }

    /// A model placed by `m`. One that is off screen costs a sphere test.
    pub fn push(&mut self, model: &Model, m: &M4) {
        let mv = m.then(&self.view);
        if model.radius > 0.0 {
            let c = mv.apply(model.centre);
            let r = model.radius * scale_of(m);
            if c.z + r < self.near { return; }
            let hw = (c.z + r) * (self.w as f32 / 2.0) / self.focal;
            let hh = (c.z + r) * (self.h as f32 / 2.0) / self.focal;
            if c.x - r > hw || c.x + r < -hw || c.y - r > hh || c.y + r < -hh { return; }
        }
        self.scratch.clear();
        self.scratch.extend(model.verts.iter().map(|v| mv.apply(v.p)));
        for (t, mat) in &model.tris {
            let (a, b, c) = (self.scratch[t[0] as usize], self.scratch[t[1] as usize], self.scratch[t[2] as usize]);
            if a.z < self.near && b.z < self.near && c.z < self.near { continue; }
            let n = b.sub(a).cross(c.sub(a));
            if n.dot(a) >= 0.0 { continue; }
            let va = Vert { p: a, ..model.verts[t[0] as usize] };
            let vb = Vert { p: b, ..model.verts[t[1] as usize] };
            let vc = Vert { p: c, ..model.verts[t[2] as usize] };
            self.push_view_tri(va, vb, vc, *mat);
        }
    }

    /// A picture standing up and facing the camera, `w` by `h`, its
    /// bottom edge centred on `pos`: a tree, a flame.
    pub fn billboard(&mut self, pos: V3, w: f32, h: f32, mat: Mat, lit: [f32; 3]) {
        let c = self.view.apply(pos);
        if c.z + w < self.near { return; }
        let (l, r) = (c.x - w / 2.0, c.x + w / 2.0);
        let a = Vert { p: V3::new(l, c.y, c.z), u: 0.0, v: 1.0, lit };
        let b = Vert { p: V3::new(r, c.y, c.z), u: 1.0, v: 1.0, lit };
        let cc = Vert { p: V3::new(r, c.y + h, c.z), u: 1.0, v: 0.0, lit };
        let d = Vert { p: V3::new(l, c.y + h, c.z), u: 0.0, v: 0.0, lit };
        self.push_view_tri(b, a, d, mat);
        self.push_view_tri(b, d, cc, mat);
    }

    fn push_view_tri(&mut self, a: Vert, b: Vert, c: Vert, mat: Mat) {
        let poly = clip_near_verts(&[a, b, c], self.near);
        if poly.len() < 3 { return; }
        let sv: Vec<SV> = poly.iter().map(|v| {
            let iz = 1.0 / v.p.z;
            SV { x: self.w as f32 / 2.0 + v.p.x * self.focal * iz, y: self.h as f32 / 2.0 - v.p.y * self.focal * iz, iz, uz: v.u * iz, vz: v.v * iz, lit: v.lit }
        }).collect();
        // Which mip level: texels per pixel over the whole triangle.
        let mut mag = false;
        let level = match mat {
            Mat::Tex(t) => {
                let tex = &self.textures[t];
                let (du1, dv1, du2, dv2) = (b.u - a.u, b.v - a.v, c.u - a.u, c.v - a.v);
                let texels = (du1 * dv2 - dv1 * du2).abs() * tex.w as f32 * tex.h as f32;
                let (p, q, r) = (sv[0], sv[1], sv[sv.len() - 1]);
                let mut pixels = ((q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)).abs();
                if poly.len() > 3 {
                    for i in 2..poly.len() - 1 { let (q, r) = (sv[i - 1], sv[i]); pixels += ((q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)).abs(); }
                }
                mag = self.smooth && pixels > 1e-3 && texels < pixels * 0.7;
                if pixels < 1e-3 { tex.levels() - 1 } else { ((texels / pixels).max(1.0).log2() * 0.5) as usize }.min(tex.levels() - 1)
            }
            Mat::Flat(_) => 0,
        };
        for i in 1..sv.len() - 1 {
            let v = [sv[0], sv[i], sv[i + 1]];
            let y0 = v.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            let y1 = v.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
            if v.iter().all(|p| p.x < 0.0) || v.iter().all(|p| p.x >= self.w as f32) || y1 < 0.0 || y0 >= self.h as f32 { continue; }
            self.tris.push(STri { v, mat, level, mag, y0: (y0 - 0.5).ceil() as i32, y1: (y1 - 0.5).ceil() as i32 - 1 });
        }
    }

    /// Paint the frame: the sky through every pixel first, from the
    /// direction it looks along, then the triangles over it. The rows are
    /// split into bands and the bands over the cores.
    pub fn render(&mut self, f: &mut Frame, sky: &(dyn Fn(V3) -> Rgb + Sync)) {
        assert!(f.w == self.w && f.h == self.h, "the frame must be the scene's size");
        // The depth buffer steps out of the scene for the frame, so the
        // threads can hold the scene read-only and their own rows of it.
        let mut zbuf = std::mem::take(&mut self.zbuf);
        zbuf.fill(0.0);
        let threads = if self.threads > 0 { self.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) }.clamp(1, 64);
        let band = ((self.h as usize + threads * 4 - 1) / (threads * 4)).max(1);
        let w = self.w as usize;
        let bands: Vec<(usize, &mut [Rgb], &mut [f32])> = f.px.chunks_mut(band * w).zip(zbuf.chunks_mut(band * w)).enumerate()
            .map(|(i, (p, z))| (i * band, p, z)).collect();
        let mut groups: Vec<Vec<(usize, &mut [Rgb], &mut [f32])>> = (0..threads).map(|_| Vec::new()).collect();
        for (i, b) in bands.into_iter().enumerate() { groups[i % threads].push(b); }
        let me = &*self;
        std::thread::scope(|s| {
            for group in groups {
                s.spawn(move || for (y0, px, zb) in group { me.paint(y0, px, zb, sky); });
            }
        });
        self.zbuf = zbuf;
    }

    fn paint(&self, y0: usize, px: &mut [Rgb], zb: &mut [f32], sky: &(dyn Fn(V3) -> Rgb + Sync)) {
        let w = self.w as usize;
        let rows = px.len() / w;
        for r in 0..rows {
            let y = (y0 + r) as f32 + 0.5;
            for x in 0..w {
                px[r * w + x] = sky(self.ray(x as f32 + 0.5, y));
            }
        }
        let (by0, by1) = (y0 as i32, (y0 + rows) as i32 - 1);
        let fog = self.fog;
        for t in &self.tris {
            if t.y1 < by0 || t.y0 > by1 { continue; }
            let mut v = t.v;
            v.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
            let (top, mid, bot) = (v[0], v[1], v[2]);
            let edge = |a: SV, c: SV, y: f32| { let s = if c.y - a.y > 1e-6 { ((y - a.y) / (c.y - a.y)).clamp(0.0, 1.0) } else { 1.0 }; a.lerp(c, s) };
            let tex = match t.mat { Mat::Tex(i) => Some(&self.textures[i]), Mat::Flat(_) => None };
            let flat = match t.mat { Mat::Flat(c) => c, _ => 0 };
            for y in t.y0.max(by0).max(0)..=t.y1.min(by1).min(self.h - 1) {
                let yc = y as f32 + 0.5;
                let a = edge(top, bot, yc);
                let b = if yc < mid.y { edge(top, mid, yc) } else { edge(mid, bot, yc) };
                let (l, rr) = if a.x < b.x { (a, b) } else { (b, a) };
                let x0 = ((l.x - 0.5).ceil() as i32).max(0);
                let x1 = ((rr.x - 0.5).ceil() as i32 - 1).min(self.w - 1);
                if x0 > x1 { continue; }
                let span = rr.x - l.x;
                let inv = if span > 1e-4 { 1.0 / span } else { 0.0 };
                let d = SV { x: 0.0, y: 0.0, iz: (rr.iz - l.iz) * inv, uz: (rr.uz - l.uz) * inv, vz: (rr.vz - l.vz) * inv,
                             lit: [(rr.lit[0] - l.lit[0]) * inv, (rr.lit[1] - l.lit[1]) * inv, (rr.lit[2] - l.lit[2]) * inv] };
                let s0 = x0 as f32 + 0.5 - l.x;
                let (mut iz, mut uz, mut vz) = (l.iz + d.iz * s0, l.uz + d.uz * s0, l.vz + d.vz * s0);
                let mut lit = [l.lit[0] + d.lit[0] * s0, l.lit[1] + d.lit[1] * s0, l.lit[2] + d.lit[2] * s0];
                let row = (y - by0) as usize * w;
                for x in x0..=x1 {
                    let i = row + x as usize;
                    if iz > zb[i] {
                        let z = 1.0 / iz;
                        let c = match tex {
                            Some(tex) if t.mag => tex.smooth_texel(uz * z, vz * z),
                            Some(tex) => tex.texel(t.level, uz * z, vz * z),
                            None => flat,
                        };
                        if c != CUTOUT {
                            zb[i] = iz;
                            let c = shade3(c, lit);
                            px[i] = match fog { Some((fc, dist)) => blend(c, fc, 1.0 - (-z / dist).exp()), None => c };
                        }
                    }
                    iz += d.iz; uz += d.uz; vz += d.vz;
                    lit[0] += d.lit[0]; lit[1] += d.lit[1]; lit[2] += d.lit[2];
                }
            }
        }
    }
}

/// How much a matrix stretches things, for a bounding radius.
fn scale_of(m: &M4) -> f32 {
    let r = &m.0;
    [r[0][0] * r[0][0] + r[1][0] * r[1][0] + r[2][0] * r[2][0], r[0][1] * r[0][1] + r[1][1] * r[1][1] + r[2][1] * r[2][1], r[0][2] * r[0][2] + r[1][2] * r[1][2] + r[2][2] * r[2][2]]
        .into_iter().fold(0.0f32, f32::max).sqrt().max(1e-6)
}

fn clip_near_verts(poly: &[Vert], near: f32) -> Vec<Vert> {
    let mut out = Vec::with_capacity(4);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ina, inb) = (a.p.z >= near, b.p.z >= near);
        if ina { out.push(a); }
        if ina != inb {
            let t = (near - a.p.z) / (b.p.z - a.p.z);
            out.push(a.lerp(b, t));
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_in_front_of_the_camera_lands_in_the_middle() {
        let mut f = Frame::new(64, 64);
        let mut r = Raster::new(64, 64);
        let cam = Cam3 { pos: V3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, focal: 64.0 };
        r.draw(&mut f, &Mesh::cuboid(2.0, 2.0, 2.0, 0xff0000), &M4::translate(V3::new(0.0, 0.0, 6.0)), &cam);
        assert_ne!(f.get(32, 32), 0, "the cube covers the centre");
        assert_eq!(f.get(2, 2), 0, "and not the corner");
        let mut behind = Frame::new(64, 64);
        r.clear();
        r.draw(&mut behind, &Mesh::cuboid(2.0, 2.0, 2.0, 0xff0000), &M4::translate(V3::new(0.0, 0.0, -6.0)), &cam);
        assert!(behind.px.iter().all(|&p| p == 0), "nothing behind the eye is drawn");
    }

    #[test]
    fn a_negative_pitch_looks_down() {
        let cam = Cam3 { pos: V3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: -0.3, focal: 64.0 };
        let below = cam.view().apply(V3::new(0.0, -6.0, 20.0));
        assert!(below.y.abs() < 2.0, "a point below and ahead comes to the centre: {:?}", below);
    }

    #[test]
    fn two_triangles_sharing_an_edge_leave_no_gap() {
        let mut f = Frame::new(40, 40);
        let mut r = Raster::new(40, 40);
        let cam = Cam3 { pos: V3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, focal: 40.0 };
        let mut m = Mesh::new();
        // A wall of stacked quads straight ahead, with fractional edges.
        for i in 0..6 {
            let (y0, y1) = (-3.0 + i as f32 * 1.07, -3.0 + (i + 1) as f32 * 1.07);
            m.quad(V3::new(3.0, y0, 5.0), V3::new(-3.0, y0, 5.0), V3::new(-3.0, y1, 5.0), V3::new(3.0, y1, 5.0), 0x00ff00);
        }
        r.draw(&mut f, &m, &M4::identity(), &cam);
        let holes = (5..35).filter(|&y| f.get(20, y) == 0).count();
        assert_eq!(holes, 0, "rows through the wall must all be painted");
    }

    #[test]
    fn a_texture_wraps_and_shrinks() {
        let t = Texture::from_fn(4, 4, |x, y| if (x + y) % 2 == 0 { 0xffffff } else { 0x000000 });
        assert_eq!(t.levels(), 3);
        assert_eq!(t.texel(0, 0.0, 0.0), 0xffffff);
        assert_eq!(t.texel(0, 0.25, 0.0), 0x000000);
        assert_eq!(t.texel(0, 1.25, 0.0), 0x000000, "past the edge wraps");
        assert_eq!(t.texel(0, -0.75, 0.0), 0x000000, "and so does before it");
        assert_eq!(t.texel(2, 0.5, 0.5), 0x7f7f7f, "the smallest level is the average");
    }

    #[test]
    fn a_textured_wall_shows_its_texels_in_place_and_its_holes() {
        let mut f = Frame::new(64, 64);
        let mut sc = Scene::new(64, 64);
        sc.threads = 3;
        // Left half red, right half blue, a hole at the bottom right.
        let t = sc.texture(Texture::from_fn(2, 2, |x, y| if x == 0 { 0xff0000 } else if y == 0 { 0x0000ff } else { CUTOUT }));
        let mut m = Model::new();
        let v = |x: f32, y: f32, u: f32, vv: f32| Vert::new(V3::new(x, y, 8.0), u, vv, 1.0);
        m.quad(v(2.0, -2.0, 1.0, 1.0), v(-2.0, -2.0, 0.0, 1.0), v(-2.0, 2.0, 0.0, 0.0), v(2.0, 2.0, 1.0, 0.0), Mat::Tex(t));
        m.bound();
        let cam = Cam3 { pos: V3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, focal: 64.0 };
        sc.begin(&cam);
        sc.push(&m, &M4::identity());
        sc.render(&mut f, &|_| 0x00ff00);
        // The wall covers the middle 32 pixels each way.
        assert_eq!(f.get(24, 24), 0xff0000, "top left of the wall is red");
        assert_eq!(f.get(40, 24), 0x0000ff, "top right is blue");
        assert_eq!(f.get(40, 40), 0x00ff00, "the hole shows the sky");
        assert_eq!(f.get(24, 40), 0xff0000, "bottom left is red again");
        assert_eq!(f.get(1, 1), 0x00ff00, "and the corner of the frame is sky");
    }

    #[test]
    fn a_model_behind_the_camera_is_skipped_by_its_sphere() {
        let mut f = Frame::new(32, 32);
        let mut sc = Scene::new(32, 32);
        let mut m = Model::block(V3::new(-1.0, -1.0, -5.0), V3::new(1.0, 1.0, -3.0), Mat::Flat(0xffffff), 1.0, 1.0);
        m.bound();
        assert!(m.radius > 1.0);
        let cam = Cam3 { pos: V3::new(0.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, focal: 32.0 };
        sc.begin(&cam);
        sc.push(&m, &M4::identity());
        sc.render(&mut f, &|_| 0);
        assert!(f.px.iter().all(|&p| p == 0));
        let mut ahead = Model::block(V3::new(-1.0, -1.0, 3.0), V3::new(1.0, 1.0, 5.0), Mat::Flat(0xffffff), 1.0, 1.0);
        ahead.bound();
        sc.begin(&cam);
        sc.push(&ahead, &M4::identity());
        sc.render(&mut f, &|_| 0);
        assert_eq!(f.get(16, 16), 0xffffff);
    }

    #[test]
    fn matrices_compose_in_reading_order() {
        let m = M4::translate(V3::new(1.0, 0.0, 0.0)).then(&M4::scale(2.0));
        let p = m.apply(V3::new(1.0, 0.0, 0.0));
        assert!((p.x - 4.0).abs() < 1e-5, "moved then scaled: {}", p.x);
    }
}
