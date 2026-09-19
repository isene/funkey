//! The framebuffer a game draws into: a grid of RGB pixels, and the
//! drawing calls that write into it. The screen backend turns it into
//! terminal cells; the game never sees the terminal.

use crate::font;
use crate::sprite::Sprite;

/// A colour as 0xRRGGBB.
pub type Rgb = u32;

pub const BLACK: Rgb = 0x000000;
pub const WHITE: Rgb = 0xffffff;

pub fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

pub fn parts(c: Rgb) -> (u8, u8, u8) {
    ((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub struct Frame {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgb>,
}

impl Frame {
    pub fn new(w: i32, h: i32) -> Frame {
        Frame { w: w.max(1), h: h.max(1), px: vec![BLACK; (w.max(1) * h.max(1)) as usize] }
    }

    pub fn clear(&mut self, c: Rgb) {
        self.px.fill(c);
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        if x >= 0 && y >= 0 && x < self.w && y < self.h { self.px[(y * self.w + x) as usize] } else { BLACK }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(self.w), (y + h).min(self.h));
        for yy in y0..y1 {
            let row = (yy * self.w) as usize;
            self.px[row + x0 as usize..row + x1 as usize].fill(c);
        }
    }

    /// A straight line between two points.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    pub fn hline(&mut self, x: i32, y: i32, w: i32, c: Rgb) { self.rect(x, y, w, 1, c); }
    pub fn vline(&mut self, x: i32, y: i32, h: i32, c: Rgb) { self.rect(x, y, 1, h, c); }

    /// Draw a sprite with its top-left corner at (x, y). Transparent
    /// pixels leave what is under them.
    pub fn blit(&mut self, s: &Sprite, x: i32, y: i32) {
        self.blit_flip(s, x, y, false);
    }

    pub fn blit_flip(&mut self, s: &Sprite, x: i32, y: i32, flip_x: bool) {
        for sy in 0..s.h {
            let dy = y + sy;
            if dy < 0 || dy >= self.h { continue; }
            for sx in 0..s.w {
                let src = if flip_x { s.w - 1 - sx } else { sx };
                let p = s.px[(sy * s.w + src) as usize];
                if p & 0xff00_0000 == 0 { continue; }
                let dx = x + sx;
                if dx >= 0 && dx < self.w { self.px[(dy * self.w + dx) as usize] = p & 0xff_ffff; }
            }
        }
    }

    /// Text in the built-in 3 by 5 font, upper case only, 4 pixels per
    /// character. Returns the width drawn.
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: Rgb) -> i32 {
        let mut cx = x;
        for ch in s.chars() {
            if let Some(rows) = font::glyph(ch) {
                for (ry, row) in rows.iter().enumerate() {
                    for rx in 0..3 {
                        if row & (4 >> rx) != 0 { self.put(cx + rx, y + ry as i32, c); }
                    }
                }
            }
            cx += 4;
        }
        cx - x
    }

    /// The frame as a binary PPM, for a screenshot or a test.
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.w, self.h).into_bytes();
        for &p in &self.px {
            let (r, g, b) = parts(p);
            out.extend_from_slice(&[r, g, b]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_stays_inside_the_frame() {
        let mut f = Frame::new(4, 3);
        f.rect(-2, -2, 4, 4, WHITE);
        f.put(10, 10, WHITE);
        assert_eq!(f.get(0, 0), WHITE);
        assert_eq!(f.get(1, 1), WHITE);
        assert_eq!(f.get(2, 2), BLACK);
        assert_eq!(f.px.iter().filter(|&&p| p == WHITE).count(), 4);
    }

    #[test]
    fn text_uses_four_pixels_a_character() {
        let mut f = Frame::new(20, 6);
        assert_eq!(f.text(0, 0, "AB", WHITE), 8);
        assert!(f.px.iter().any(|&p| p == WHITE));
    }
}
