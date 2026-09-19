//! A software 3D rasterizer: meshes of flat-shaded triangles through a
//! camera into the frame, with a depth buffer, near-plane clipping, a
//! light direction and fog. Enough for a road over hills and a car on it.

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
        if b.1 - t.1 < 0.5 { return; }
        let y0 = (t.1.ceil() as i32).max(0);
        let y1 = (b.1.ceil() as i32 - 1).min(self.h - 1);
        // Interpolate x and 1/z down both edges.
        let edge = |a: (f32, f32, f32), c: (f32, f32, f32), y: f32| {
            let s = ((y - a.1) / (c.1 - a.1)).clamp(0.0, 1.0);
            (a.0 + (c.0 - a.0) * s, 1.0 / a.2 + (1.0 / c.2 - 1.0 / a.2) * s)
        };
        for y in y0..=y1 {
            let yc = y as f32 + 0.5;
            let (xa, za) = edge(t, b, yc);
            let (xb, zb) = if yc < m.1 { edge(t, m, yc) } else { edge(m, b, yc) };
            let ((xl, zl), (xr, zr)) = if xa < xb { ((xa, za), (xb, zb)) } else { ((xb, zb), (xa, za)) };
            let x0 = (xl.ceil() as i32).max(0);
            let x1 = (xr.ceil() as i32 - 1).min(self.w - 1);
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
    fn matrices_compose_in_reading_order() {
        let m = M4::translate(V3::new(1.0, 0.0, 0.0)).then(&M4::scale(2.0));
        let p = m.apply(V3::new(1.0, 0.0, 0.0));
        assert!((p.x - 4.0).abs() < 1e-5, "moved then scaled: {}", p.x);
    }
}
