//! Sprites: small pictures with transparent pixels. Drawn in code as
//! rows of characters with a palette, so a game needs no image files.

use crate::frame::{tint, Rgb};

#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub w: i32,
    pub h: i32,
    /// 0xAARRGGBB; alpha 0 is transparent, anything else is opaque.
    pub px: Vec<u32>,
}

impl Sprite {
    /// Rows of characters; `.` and space are transparent, every other
    /// character takes its colour from the palette. Rows may differ in
    /// length; short ones are padded with transparency.
    pub fn from_rows(rows: &[&str], palette: &[(char, Rgb)]) -> Sprite {
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut px = vec![0u32; (w * h) as usize];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == '.' || ch == ' ' { continue; }
                if let Some((_, c)) = palette.iter().find(|(p, _)| *p == ch) {
                    px[y * w as usize + x] = 0xff00_0000 | c;
                }
            }
        }
        Sprite { w, h, px }
    }

    pub fn solid(w: i32, h: i32, c: Rgb) -> Sprite {
        Sprite { w, h, px: vec![0xff00_0000 | c; (w * h) as usize] }
    }

    /// A PNG's pixels; alpha under half is transparent.
    pub fn from_png(bytes: &[u8]) -> Option<Sprite> {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (w, h) = (img.width() as i32, img.height() as i32);
        let px = img.pixels().map(|p| if p[3] < 128 { 0 } else { 0xff00_0000 | ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32 }).collect();
        Some(Sprite { w, h, px })
    }

    /// A PNG file.
    pub fn load(path: &str) -> Option<Sprite> { Sprite::from_png(&std::fs::read(path).ok()?) }

    /// The part from (x, y) of size (w, h).
    pub fn sub(&self, x: i32, y: i32, w: i32, h: i32) -> Sprite {
        let mut px = Vec::with_capacity((w * h) as usize);
        for sy in y..y + h {
            for sx in x..x + w {
                px.push(if sx >= 0 && sy >= 0 && sx < self.w && sy < self.h { self.px[(sy * self.w + sx) as usize] } else { 0 });
            }
        }
        Sprite { w, h, px }
    }

    /// A sheet cut into cells of (w, h), left to right, top to bottom.
    pub fn sheet(&self, w: i32, h: i32) -> Vec<Sprite> {
        let mut out = Vec::new();
        for y in (0..self.h - h + 1).step_by(h.max(1) as usize) {
            for x in (0..self.w - w + 1).step_by(w.max(1) as usize) { out.push(self.sub(x, y, w, h)); }
        }
        out
    }

    /// Every pixel `n` times bigger.
    pub fn scaled(&self, n: i32) -> Sprite {
        let n = n.max(1);
        let (w, h) = (self.w * n, self.h * n);
        let mut px = Vec::with_capacity((w * h) as usize);
        for y in 0..h { for x in 0..w { px.push(self.px[((y / n) * self.w + x / n) as usize]); } }
        Sprite { w, h, px }
    }

    /// The same shape in one colour: a flash when hit, a shadow.
    pub fn tinted(&self, c: Rgb) -> Sprite {
        Sprite { w: self.w, h: self.h, px: self.px.iter().map(|&p| if p & 0xff00_0000 == 0 { 0 } else { 0xff00_0000 | c }).collect() }
    }

    /// Twice the size with the stair steps of diagonals smoothed (the
    /// Scale2x rule): small pixel art made big without getting blocky.
    pub fn scale2x(&self) -> Sprite {
        let (w, h) = (self.w, self.h);
        let at = |x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { 0 } else { self.px[(y * w + x) as usize] };
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

    /// Lit from the top left: the edge pixels there a little brighter,
    /// the ones at the bottom right a little darker.
    pub fn shaded(&self) -> Sprite {
        let on = |x: i32, y: i32| x >= 0 && y >= 0 && x < self.w && y < self.h && self.px[(y * self.w + x) as usize] >> 24 != 0;
        let mut out = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                let i = (y * self.w + x) as usize;
                if self.px[i] >> 24 == 0 { continue; }
                let k = if !on(x, y - 1) || !on(x - 1, y) { 1.25 } else if !on(x, y + 1) || !on(x + 1, y) { 0.72 } else { 1.0 };
                out.px[i] = 0xff00_0000 | tint(self.px[i] & 0xff_ffff, k);
            }
        }
        out
    }

    /// With a line of colour `c` one pixel wide around the shape. The
    /// sprite grows by a pixel on every side.
    pub fn outlined(&self, c: Rgb) -> Sprite {
        let (w, h) = (self.w + 2, self.h + 2);
        let on = |x: i32, y: i32| x >= 1 && y >= 1 && x <= self.w && y <= self.h && self.px[((y - 1) * self.w + x - 1) as usize] >> 24 != 0;
        let mut px = vec![0u32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                px[(y * w + x) as usize] = if on(x, y) { self.px[((y - 1) * self.w + x - 1) as usize] }
                    else if on(x - 1, y) || on(x + 1, y) || on(x, y - 1) || on(x, y + 1) { 0xff00_0000 | c } else { 0 };
            }
        }
        Sprite { w, h, px }
    }

    pub fn flipped(&self) -> Sprite {
        let mut px = Vec::with_capacity(self.px.len());
        for y in 0..self.h {
            for x in (0..self.w).rev() { px.push(self.px[(y * self.w + x) as usize]); }
        }
        Sprite { w: self.w, h: self.h, px }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_become_pixels_with_transparency() {
        let s = Sprite::from_rows(&["X.", ".XX"], &[('X', 0xff0000)]);
        assert_eq!((s.w, s.h), (3, 2));
        assert_eq!(s.px[0], 0xffff0000);
        assert_eq!(s.px[1], 0);
        assert_eq!(s.px[2], 0, "a short row is padded");
        assert_eq!(s.flipped().px[0], 0);
        assert_eq!(s.flipped().px[2], 0xffff0000);
    }

    #[test]
    fn a_sprite_grows_gets_light_and_a_line_around_it() {
        let s = Sprite::solid(3, 3, 0x808080);
        let big = s.scale2x();
        assert_eq!((big.w, big.h), (6, 6));
        assert_eq!(big.px[0], 0, "a corner is rounded off");
        assert_eq!(big.px[14], 0xff808080, "the inside stays plain");
        let lit = s.shaded();
        assert_eq!(lit.px[0], 0xffa0a0a0, "the top left edge is brighter");
        assert_eq!(lit.px[4], 0xff808080, "the middle is as it was");
        assert_eq!(lit.px[8], 0xff5c5c5c, "the bottom right edge is darker");
        let ring = s.outlined(0x0000ff);
        assert_eq!((ring.w, ring.h), (5, 5));
        assert_eq!(ring.px[0], 0, "a corner stays clear");
        assert_eq!(ring.px[1], 0xff0000ff);
        assert_eq!(ring.px[6], 0xff808080);
    }
}
