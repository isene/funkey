//! A Doom-style renderer: sectors with a floor and a ceiling, walls
//! between them, drawn front to back along the level's own BSP tree.
//! Walls are textured columns, floors and ceilings are flats read with
//! one perspective division per pixel, things are sprites tested against
//! a depth buffer. Lighting comes from the sector and the distance
//! through the WAD's colour maps.

use crate::frame::Frame;
use crate::wad::{Art, Level, Picture, Seg, ML_DONTPEGBOTTOM, ML_DONTPEGTOP, NF_SUBSECTOR, NO_PIXEL};

/// A sprite the game wants drawn: something standing at a spot, showing
/// one frame of one sprite.
#[derive(Clone, Copy, Debug)]
pub struct Vis {
    pub x: f32,
    pub y: f32,
    /// The sprite's origin, absolute: the feet of a monster.
    pub z: f32,
    /// Which way it faces, for sprites with rotations.
    pub angle: f32,
    pub sprite: [u8; 4],
    /// The frame letter, `b'A'` on.
    pub frame: u8,
    /// Lit in full whatever the sector light: fireballs, muzzle flashes.
    pub bright: bool,
    /// Drawn as a shimmer: spectres, a player with partial invisibility.
    pub fuzzy: bool,
}

const NEAR: f32 = 0.5;
const EYE: f32 = 41.0;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    /// Eye height, absolute.
    pub z: f32,
    pub angle: f32,
}

impl Camera {
    pub fn standing(level: &Level, x: f32, y: f32, angle: f32) -> Camera {
        let floor = level.sectors[level.sector_at(x, y)].floor;
        Camera { x, y, z: floor + EYE, angle }
    }
}

/// What a thing looks like: its sprite name and frame letter.
pub fn sprite_of(kind: u16) -> Option<(&'static str, char)> {
    Some(match kind {
        3004 => ("POSS", 'A'), 9 => ("SPOS", 'A'), 3001 => ("TROO", 'A'), 3002 | 58 => ("SARG", 'A'),
        3005 => ("HEAD", 'A'), 3003 => ("BOSS", 'A'), 3006 => ("SKUL", 'A'), 7 => ("SPID", 'A'),
        16 => ("CYBR", 'A'), 64 => ("VILE", 'A'), 65 => ("CPOS", 'A'), 66 => ("SKEL", 'A'), 67 => ("FATT", 'A'),
        68 => ("BSPI", 'A'), 69 => ("BOS2", 'A'), 71 => ("PAIN", 'A'), 84 => ("SSWV", 'A'), 72 => ("KEEN", 'A'),
        2001 => ("SHOT", 'A'), 2002 => ("MGUN", 'A'), 2003 => ("LAUN", 'A'), 2004 => ("PLAS", 'A'),
        2005 => ("CSAW", 'A'), 2006 => ("BFUG", 'A'), 2007 => ("CLIP", 'A'), 2008 => ("SHEL", 'A'),
        2010 => ("ROCK", 'A'), 2046 => ("BROK", 'A'), 2047 => ("CELL", 'A'), 17 => ("CELP", 'A'),
        2048 => ("AMMO", 'A'), 2049 => ("SBOX", 'A'), 2011 => ("STIM", 'A'), 2012 => ("MEDI", 'A'),
        2013 => ("SOUL", 'A'), 2014 => ("BON1", 'A'), 2015 => ("BON2", 'A'), 2018 => ("ARM1", 'A'),
        2019 => ("ARM2", 'A'), 2022 => ("PINV", 'A'), 2023 => ("PSTR", 'A'), 2024 => ("PINS", 'A'),
        2025 => ("SUIT", 'A'), 2026 => ("PMAP", 'A'), 2045 => ("PVIS", 'A'), 83 => ("MEGA", 'A'),
        5 => ("BKEY", 'A'), 6 => ("YKEY", 'A'), 13 => ("RKEY", 'A'), 38 => ("RSKU", 'A'), 39 => ("YSKU", 'A'),
        40 => ("BSKU", 'A'), 8 => ("BPAK", 'A'), 2035 => ("BAR1", 'A'), 2028 => ("COLU", 'A'),
        30 => ("COL1", 'A'), 31 => ("COL2", 'A'), 32 => ("COL3", 'A'), 33 => ("COL4", 'A'), 37 => ("COL6", 'A'),
        36 => ("COL5", 'A'), 41 => ("CEYE", 'A'), 42 => ("FSKU", 'A'), 43 => ("TRE1", 'A'), 44 => ("TBLU", 'A'),
        45 => ("TGRN", 'A'), 46 => ("TRED", 'A'), 47 => ("SMIT", 'A'), 48 => ("ELEC", 'A'), 54 => ("TRE2", 'A'),
        55 => ("SMBT", 'A'), 56 => ("SMGT", 'A'), 57 => ("SMRT", 'A'), 34 => ("CAND", 'A'), 35 => ("CBRA", 'A'),
        70 => ("FCAN", 'A'), 85 => ("TLMP", 'A'), 86 => ("TLP2", 'A'), 10 | 12 => ("PLAY", 'W'), 15 => ("PLAY", 'N'),
        18 => ("POSS", 'L'), 19 => ("SPOS", 'L'), 20 => ("TROO", 'M'), 21 => ("SARG", 'N'), 22 => ("HEAD", 'L'),
        23 => ("SKUL", 'K'), 24 => ("POL5", 'A'), 25 => ("POL1", 'A'), 26 => ("POL6", 'A'), 27 => ("POL4", 'A'),
        28 => ("POL2", 'A'), 29 => ("POL3", 'A'), 49 => ("GOR1", 'A'), 50 => ("GOR2", 'A'), 51 => ("GOR3", 'A'),
        52 => ("GOR4", 'A'), 53 => ("GOR5", 'A'), 59 => ("GOR2", 'A'), 60 => ("GOR4", 'A'), 61 => ("GOR3", 'A'),
        62 => ("GOR5", 'A'), 63 => ("GOR1", 'A'), 73 => ("HDB1", 'A'), 74 => ("HDB2", 'A'), 75 => ("HDB3", 'A'),
        76 => ("HDB4", 'A'), 77 => ("HDB5", 'A'), 78 => ("HDB6", 'A'), 79 => ("POB1", 'A'), 80 => ("POB2", 'A'),
        81 => ("BRS1", 'A'),
        _ => return None,
    })
}

pub struct Renderer {
    pub w: i32,
    pub h: i32,
    focal: f32,
    focal_y: f32,
    ceil_clip: Vec<i32>,
    floor_clip: Vec<i32>,
    zbuf: Vec<f32>,
    open: i32,
    // The camera in a form the inner loops like.
    cx: f32, cy: f32, cz: f32,
    fx: f32, fy: f32, rx: f32, ry: f32,
    angle: f32,
    sky: String,
    /// Lines drawn so far, by linedef: what an automap has seen.
    pub seen: Vec<bool>,
    /// One colour map for everything: 32 for invulnerability, 0 for a light visor.
    pub fixed: Option<usize>,
    /// Steps of extra brightness, from a muzzle flash.
    pub extra_light: i32,
}

impl Renderer {
    pub fn new(w: i32, h: i32) -> Renderer {
        Renderer {
            w, h, focal: w as f32 / 2.0, focal_y: w as f32 / 2.0 * 1.2,
            ceil_clip: vec![-1; w as usize], floor_clip: vec![h; w as usize], zbuf: vec![f32::INFINITY; (w * h) as usize],
            open: w, cx: 0.0, cy: 0.0, cz: 0.0, fx: 1.0, fy: 0.0, rx: 0.0, ry: -1.0, angle: 0.0, sky: "SKY1".into(),
            seen: Vec::new(), fixed: None, extra_light: 0,
        }
    }

    /// Draw the level as seen from the camera, then the sprites.
    pub fn render(&mut self, level: &Level, art: &Art, cam: &Camera, frame: &mut Frame, vis: &[Vis]) {
        self.ceil_clip.fill(-1);
        self.floor_clip.fill(self.h);
        self.zbuf.fill(f32::INFINITY);
        self.open = self.w;
        self.cx = cam.x; self.cy = cam.y; self.cz = cam.z; self.angle = cam.angle;
        self.fx = cam.angle.cos(); self.fy = cam.angle.sin();
        self.rx = self.fy; self.ry = -self.fx;
        self.sky = sky_for(&level.name);
        if self.seen.len() != level.linedefs.len() { self.seen = vec![false; level.linedefs.len()]; }
        let root = (level.nodes.len() - 1) as u16;
        self.walk(level, art, frame, root);
        self.sprites(level, art, frame, vis);
    }

    /// The colour map for something right in front of the camera in a
    /// sector of the given light: the weapon in hand.
    pub fn light_for(&self, light: i32) -> usize { self.light_index(light, 0.0) }

    fn walk(&mut self, level: &Level, art: &Art, frame: &mut Frame, id: u16) {
        if self.open <= 0 { return; }
        if id & NF_SUBSECTOR != 0 {
            let ss = &level.subsectors[(id & !NF_SUBSECTOR) as usize];
            for i in ss.first..ss.first + ss.count {
                if let Some(seg) = level.segs.get(i) { self.seg(level, art, frame, seg); }
            }
            return;
        }
        let n = &level.nodes[id as usize];
        let side = level.node_side(n, self.cx, self.cy);
        self.walk(level, art, frame, n.child[side]);
        self.walk(level, art, frame, n.child[side ^ 1]);
    }

    /// View-space depth (forward) and side (right) of a world point.
    #[inline]
    fn view(&self, x: f32, y: f32) -> (f32, f32) {
        let (dx, dy) = (x - self.cx, y - self.cy);
        (dx * self.fx + dy * self.fy, dx * self.rx + dy * self.ry)
    }

    #[inline]
    fn row_of(&self, z: f32, depth: f32) -> f32 {
        self.h as f32 / 2.0 - (z - self.cz) * self.focal_y / depth
    }

    fn light_index(&self, light: i32, depth: f32) -> usize {
        if let Some(f) = self.fixed { return f; }
        let base = ((255 - light.clamp(0, 255)) / 8) as f32;
        (base + depth / 160.0 - self.extra_light as f32 * 2.0).clamp(0.0, 31.0) as usize
    }

    fn seg(&mut self, level: &Level, art: &Art, frame: &mut Frame, seg: &Seg) {
        let (x1, y1) = level.vertexes[seg.v1];
        let (x2, y2) = level.vertexes[seg.v2];
        // Only the front of a seg is drawn: the camera must be to its right.
        if (self.cx - x1) * (y2 - y1) - (self.cy - y1) * (x2 - x1) <= 0.0 { return; }
        let line = &level.linedefs[seg.linedef];
        let side_id = if seg.back_side { line.back } else { line.front };
        let Some(side_id) = side_id else { return };
        let side = &level.sidedefs[side_id];
        let front = &level.sectors[side.sector];
        let back_id = if seg.back_side { line.front } else { line.back };
        let back = back_id.map(|s| &level.sectors[level.sidedefs[s].sector]);

        let (mut d1, mut s1) = self.view(x1, y1);
        let (mut d2, mut s2) = self.view(x2, y2);
        if d1 < NEAR && d2 < NEAR { return; }
        let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
        let (mut u1, mut u2) = (seg.offset + side.x_off, seg.offset + side.x_off + len);
        if d1 < NEAR {
            let t = (NEAR - d1) / (d2 - d1);
            s1 += (s2 - s1) * t; u1 += (u2 - u1) * t; d1 = NEAR;
        } else if d2 < NEAR {
            let t = (NEAR - d2) / (d1 - d2);
            s2 += (s1 - s2) * t; u2 += (u1 - u2) * t; d2 = NEAR;
        }
        let half = self.w as f32 / 2.0;
        let px1 = half + s1 * self.focal / d1;
        let px2 = half + s2 * self.focal / d2;
        if px1 >= px2 { return; }
        let xa = (px1.ceil() as i32).max(0);
        let xb = ((px2.ceil() as i32) - 1).min(self.w - 1);
        if xa > xb { return; }
        self.seen[seg.linedef] = true;

        // Textures and pegging.
        let sky_ceiling = front.ceiling_flat == "F_SKY1";
        let both_sky = sky_ceiling && back.map(|b| b.ceiling_flat == "F_SKY1").unwrap_or(false);
        let tex_mid = art.textures.get(&side.middle);
        let tex_up = art.textures.get(&side.upper);
        let tex_low = art.textures.get(&side.lower);
        let light = front.light + fake_contrast(x1, y1, x2, y2);
        let mid_anchor = if line.flags & ML_DONTPEGBOTTOM != 0 { front.floor + tex_mid.map(|t| t.h as f32).unwrap_or(0.0) } else { front.ceiling };

        for x in xa..=xb {
            if self.ceil_clip[x as usize] + 1 >= self.floor_clip[x as usize] { continue; }
            // The exact hit of this column's ray on the seg: perspective-correct.
            let k = (x as f32 + 0.5 - half) / self.focal;
            let den = (s2 - s1) - k * (d2 - d1);
            if den.abs() < 1e-6 { continue; }
            let t = ((k * d1 - s1) / den).clamp(0.0, 1.0);
            let depth = d1 + t * (d2 - d1);
            if depth < NEAR { continue; }
            let u = u1 + t * (u2 - u1);
            let top = self.row_of(front.ceiling, depth);
            let bottom = self.row_of(front.floor, depth);
            let ceil_row = top.round() as i32;
            let floor_row = bottom.round() as i32;
            let (c0, f0) = (self.ceil_clip[x as usize], self.floor_clip[x as usize]);

            // Ceiling above the wall, floor below it.
            let ceil_end = (ceil_row - 1).min(f0 - 1);
            if ceil_end > c0 {
                if sky_ceiling { self.sky_column(art, frame, x, c0 + 1, ceil_end); }
                else { self.flat_column(art, frame, x, c0 + 1, ceil_end, front.ceiling, &front.ceiling_flat, front.light); }
            }
            let floor_start = (floor_row + 1).max(c0 + 1);
            if floor_start < f0 {
                self.flat_column(art, frame, x, floor_start, f0 - 1, front.floor, &front.floor_flat, front.light);
            }

            match back {
                None => {
                    let (y0, y1) = (ceil_row.max(c0 + 1), floor_row.min(f0 - 1));
                    if y0 <= y1 {
                        if let Some(t) = tex_mid { self.wall_column(art, frame, x, y0, y1, depth, u, mid_anchor, side.y_off, t, light, false); }
                    }
                    self.ceil_clip[x as usize] = self.h;
                    self.floor_clip[x as usize] = -1;
                    self.open -= 1;
                }
                Some(b) => {
                    let mut new_c = c0.max(ceil_row - 1);
                    let mut new_f = f0.min(floor_row + 1);
                    // Upper wall, unless both ceilings are sky.
                    if b.ceiling < front.ceiling && !both_sky {
                        let brow = self.row_of(b.ceiling, depth).round() as i32;
                        let (y0, y1) = (ceil_row.max(c0 + 1), (brow - 1).min(f0 - 1));
                        if y0 <= y1 {
                            let anchor = if line.flags & ML_DONTPEGTOP != 0 { front.ceiling } else { b.ceiling + tex_up.map(|t| t.h as f32).unwrap_or(0.0) };
                            match tex_up {
                                Some(t) => self.wall_column(art, frame, x, y0, y1, depth, u, anchor, side.y_off, t, light, false),
                                None => self.fill_column(frame, x, y0, y1, depth, 0),
                            }
                        }
                        new_c = new_c.max(brow - 1);
                    } else if b.ceiling < front.ceiling && both_sky {
                        let brow = self.row_of(b.ceiling, depth).round() as i32;
                        let (y0, y1) = (ceil_row.max(c0 + 1), (brow - 1).min(f0 - 1));
                        if y0 <= y1 { self.sky_column(art, frame, x, y0, y1); }
                        new_c = new_c.max(brow - 1);
                    }
                    // Lower wall.
                    if b.floor > front.floor {
                        let brow = self.row_of(b.floor, depth).round() as i32;
                        let (y0, y1) = (brow.max(c0 + 1), floor_row.min(f0 - 1));
                        if y0 <= y1 {
                            let anchor = if line.flags & ML_DONTPEGBOTTOM != 0 { front.ceiling } else { b.floor };
                            match tex_low {
                                Some(t) => self.wall_column(art, frame, x, y0, y1, depth, u, anchor, side.y_off, t, light, false),
                                None => self.fill_column(frame, x, y0, y1, depth, 0),
                            }
                        }
                        new_f = new_f.min(brow);
                    }
                    // A see-through middle texture, drawn once, not repeated.
                    if let Some(t) = tex_mid {
                        let (lo_c, hi_f) = (front.ceiling.min(b.ceiling), front.floor.max(b.floor));
                        let anchor = if line.flags & ML_DONTPEGBOTTOM != 0 { hi_f + t.h as f32 } else { lo_c };
                        let y0 = self.row_of(anchor.min(lo_c), depth).round() as i32;
                        let y1 = self.row_of((anchor - t.h as f32).max(hi_f), depth).round() as i32 - 1;
                        let (y0, y1) = (y0.max(new_c + 1), y1.min(new_f - 1));
                        if y0 <= y1 { self.wall_column(art, frame, x, y0, y1, depth, u, anchor, side.y_off, t, light, true); }
                    }
                    self.ceil_clip[x as usize] = new_c;
                    self.floor_clip[x as usize] = new_f;
                    if new_c + 1 >= new_f { self.open -= 1; }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wall_column(&mut self, art: &Art, frame: &mut Frame, x: i32, y0: i32, y1: i32, depth: f32, u: f32,
                   anchor: f32, y_off: f32, tex: &Picture, light: i32, masked: bool) {
        let cm = &art.colormaps[self.light_index(light, depth)];
        let tx = (u as i32).rem_euclid(tex.w);
        let col = &tex.px[(tx * tex.h) as usize..((tx + 1) * tex.h) as usize];
        let dz = depth / self.focal_y;
        // World height at the centre of row y0, then one dz down per row.
        let mut z = self.cz + (self.h as f32 / 2.0 - (y0 as f32 + 0.5)) * dz;
        let w = self.w as usize;
        for y in y0..=y1 {
            let v = anchor - z + y_off;
            let i = y as usize * w + x as usize;
            let ty = if masked { let r = v as i32; if r < 0 || r >= tex.h { z -= dz; continue; } r } else { (v as i32).rem_euclid(tex.h) };
            let c = col[ty as usize];
            if c != NO_PIXEL && depth < self.zbuf[i] {
                frame.px[i] = art.palette[cm[c as usize] as usize];
                self.zbuf[i] = depth;
            }
            z -= dz;
        }
    }

    fn fill_column(&mut self, frame: &mut Frame, x: i32, y0: i32, y1: i32, depth: f32, color: u32) {
        let w = self.w as usize;
        for y in y0..=y1 {
            let i = y as usize * w + x as usize;
            if depth < self.zbuf[i] { frame.px[i] = color; self.zbuf[i] = depth; }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn flat_column(&mut self, art: &Art, frame: &mut Frame, x: i32, y0: i32, y1: i32, z: f32, name: &str, light: i32) {
        let Some(flat) = art.flats.get(name) else { return self.fill_column(frame, x, y0, y1, 1.0, 0x202020) };
        let half_h = self.h as f32 / 2.0;
        let k = (x as f32 + 0.5 - self.w as f32 / 2.0) / self.focal;
        let dz = z - self.cz;
        let w = self.w as usize;
        for y in y0..=y1 {
            let dy = y as f32 + 0.5 - half_h;
            if dy.abs() < 0.01 { continue; }
            let dist = -dz * self.focal_y / dy;
            if dist <= 0.0 { continue; }
            let i = y as usize * w + x as usize;
            if dist >= self.zbuf[i] { continue; }
            let wx = self.cx + self.fx * dist + self.rx * dist * k;
            let wy = self.cy + self.fy * dist + self.ry * dist * k;
            let c = flat.at(wx as i32, -(wy as i32));
            let cm = &art.colormaps[self.light_index(light, dist)];
            frame.px[i] = art.palette[cm[c as usize] as usize];
            self.zbuf[i] = dist;
        }
    }

    fn sky_column(&mut self, art: &Art, frame: &mut Frame, x: i32, y0: i32, y1: i32) {
        let w = self.w as usize;
        let Some(sky) = art.textures.get(&self.sky) else { return self.fill_column(frame, x, y0, y1, 1e9, 0x304060) };
        let ang = self.angle - ((x as f32 + 0.5 - self.w as f32 / 2.0) / self.focal).atan();
        let tx = ((-ang / std::f32::consts::TAU * 1024.0) as i32).rem_euclid(sky.w);
        for y in y0..=y1 {
            let i = y as usize * w + x as usize;
            if self.zbuf[i] != f32::INFINITY { continue; }
            let ty = ((y as f32 * 200.0 / self.h as f32) as i32).clamp(0, sky.h - 1);
            let c = sky.px[(tx * sky.h + ty) as usize];
            frame.px[i] = art.palette[c as usize];
            self.zbuf[i] = 1e9;
        }
    }

    /// Sprites, far to near, tested per pixel against the depth buffer.
    fn sprites(&mut self, level: &Level, art: &Art, frame: &mut Frame, vis: &[Vis]) {
        let mut seen: Vec<(f32, f32, &Vis)> = vis.iter()
            .filter_map(|v| { let (d, side) = self.view(v.x, v.y); if d > NEAR { Some((d, side, v)) } else { None } })
            .collect();
        seen.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let w = self.w as usize;
        for (depth, side, v) in seen {
            // Rotation as seen from the camera: 1 faces us, then clockwise.
            let to_cam = (self.cy - v.y).atan2(self.cx - v.x);
            let rel = (to_cam - v.angle).rem_euclid(std::f32::consts::TAU);
            let rot = (((rel + std::f32::consts::PI / 8.0) / (std::f32::consts::PI / 4.0)) as u8 % 8) + 1;
            let Some((pic, mirror)) = art.sprite(&v.sprite, v.frame, rot) else { continue };
            let sx = self.w as f32 / 2.0 + side * self.focal / depth;
            let scale = self.focal / depth;
            let scale_y = self.focal_y / depth;
            let left = sx - pic.left as f32 * scale;
            // The picture's top offset says how far above the origin it starts.
            let top = self.row_of(v.z + pic.top as f32, depth);
            let bottom = top + pic.h as f32 * scale_y;
            let x0 = (left.round() as i32).max(0);
            let x1 = ((left + pic.w as f32 * scale).round() as i32 - 1).min(self.w - 1);
            let y0 = (top.round() as i32).max(0);
            let y1 = (bottom.round() as i32 - 1).min(self.h - 1);
            if x0 > x1 || y0 > y1 { continue; }
            let light = if let Some(f) = self.fixed { f } else if v.bright { 0 } else if v.fuzzy { 6 } else {
                self.light_index(level.sectors[level.sector_at(v.x, v.y)].light, depth)
            };
            let cm = &art.colormaps[light.min(art.colormaps.len() - 1)];
            for x in x0..=x1 {
                let mut tx = ((x as f32 + 0.5 - left) / scale) as i32;
                if mirror { tx = pic.w - 1 - tx; }
                if tx < 0 || tx >= pic.w { continue; }
                let col = &pic.px[(tx * pic.h) as usize..((tx + 1) * pic.h) as usize];
                for y in y0..=y1 {
                    if v.fuzzy && (x + y) & 1 == 0 { continue; }
                    let ty = ((y as f32 + 0.5 - top) / scale_y) as i32;
                    if ty < 0 || ty >= pic.h { continue; }
                    let c = col[ty as usize];
                    if c == NO_PIXEL { continue; }
                    let i = y as usize * w + x as usize;
                    if depth < self.zbuf[i] {
                        frame.px[i] = art.palette[cm[c as usize] as usize];
                        self.zbuf[i] = depth;
                    }
                }
            }
        }
    }
}

/// Draw a WAD picture into the frame with its top-left corner at (x, y),
/// scaled by `sx` and `sy`, through a colour map when one is given. For
/// the weapon in hand, the status bar, menus: nothing checks depth.
#[allow(clippy::too_many_arguments)]
pub fn blit_pic(frame: &mut Frame, art: &Art, pic: &Picture, x: f32, y: f32, sx: f32, sy: f32, cm: Option<&[u8; 256]>) {
    let x0 = (x.round() as i32).max(0);
    let y0 = (y.round() as i32).max(0);
    let x1 = ((x + pic.w as f32 * sx).round() as i32).min(frame.w);
    let y1 = ((y + pic.h as f32 * sy).round() as i32).min(frame.h);
    for px in x0..x1 {
        let tx = ((px as f32 + 0.5 - x) / sx) as i32;
        if tx < 0 || tx >= pic.w { continue; }
        let col = &pic.px[(tx * pic.h) as usize..((tx + 1) * pic.h) as usize];
        for py in y0..y1 {
            let ty = ((py as f32 + 0.5 - y) / sy) as i32;
            if ty < 0 || ty >= pic.h { continue; }
            let c = col[ty as usize];
            if c == NO_PIXEL { continue; }
            let c = cm.map(|m| m[c as usize]).unwrap_or(c);
            frame.px[(py * frame.w + px) as usize] = art.palette[c as usize];
        }
    }
}

/// Doom's fake contrast: walls along the y axis a little brighter, along
/// the x axis a little darker, so corners read.
fn fake_contrast(x1: f32, y1: f32, x2: f32, y2: f32) -> i32 {
    if (x1 - x2).abs() < 0.5 { 16 } else if (y1 - y2).abs() < 0.5 { -16 } else { 0 }
}

/// The sky texture for a level name: SKY1 for E1 and MAP01-11, and so on.
pub fn sky_for(name: &str) -> String {
    let b = name.as_bytes();
    if b.len() >= 4 && b[0] == b'E' && b[1].is_ascii_digit() {
        let e = (b[1] - b'0').clamp(1, 4);
        return format!("SKY{}", e);
    }
    if let Some(n) = name.strip_prefix("MAP").and_then(|n| n.parse::<u32>().ok()) {
        return if n <= 11 { "SKY1".into() } else if n <= 20 { "SKY2".into() } else { "SKY3".into() };
    }
    "SKY1".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_follows_the_episode_or_the_map_number() {
        assert_eq!(sky_for("E1M1"), "SKY1");
        assert_eq!(sky_for("E3M7"), "SKY3");
        assert_eq!(sky_for("MAP15"), "SKY2");
        assert_eq!(sky_for("MAP30"), "SKY3");
    }

    #[test]
    fn things_have_sprites_and_players_do_not() {
        assert_eq!(sprite_of(3004), Some(("POSS", 'A')));
        assert_eq!(sprite_of(2035), Some(("BAR1", 'A')));
        assert_eq!(sprite_of(1), None);
    }
}
