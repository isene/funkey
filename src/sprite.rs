//! Sprites: small pictures with transparent pixels. Drawn in code as
//! rows of characters with a palette, so a game needs no image files.

use crate::frame::Rgb;

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
}
