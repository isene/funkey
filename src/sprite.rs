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
