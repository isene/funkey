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

/// A colour between two: `a` at 0, `b` at 1.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let ((ar, ag, ab), (br, bg, bb)) = (parts(a), parts(b));
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    rgb(l(ar, br), l(ag, bg), l(ab, bb))
}

/// A colour made brighter (`k` above 1) or darker (below 1).
pub fn tint(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    let s = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    rgb(s(r), s(g), s(b))
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
        if x0 >= x1 { return; }
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

    /// Text in the 5 by 7 font, upper and lower case, 6 pixels per
    /// character. Returns the width drawn.
    pub fn text_big(&mut self, x: i32, y: i32, s: &str, c: Rgb) -> i32 { self.text_scaled(x, y, s, c, true, 1) }

    /// Either font, each pixel `scale` wide: titles and game-over screens.
    pub fn text_scaled(&mut self, x: i32, y: i32, s: &str, c: Rgb, big: bool, scale: i32) -> i32 {
        let scale = scale.max(1);
        let (cw, adv) = if big { (5, 6) } else { (3, 4) };
        let mut cx = x;
        for chr in s.chars() {
            let rows: Vec<u8> = if big { font::glyph7(chr).map(|g| g.to_vec()).unwrap_or_default() } else { font::glyph(chr).map(|g| g.to_vec()).unwrap_or_default() };
            for (ry, row) in rows.iter().enumerate() {
                for rx in 0..cw {
                    if row & (1 << (cw - 1 - rx)) != 0 { self.rect(cx + rx * scale, y + ry as i32 * scale, scale, scale, c); }
                }
            }
            cx += adv * scale;
        }
        cx - x
    }

    /// The width `text_scaled` would draw, for centring.
    pub fn text_width(s: &str, big: bool, scale: i32) -> i32 { s.chars().count() as i32 * if big { 6 } else { 4 } * scale.max(1) }

    /// Text centred on a column.
    pub fn text_centered(&mut self, cx: i32, y: i32, s: &str, c: Rgb, big: bool, scale: i32) {
        let w = Frame::text_width(s, big, scale);
        self.text_scaled(cx - w / 2, y, s, c, big, scale);
    }

    /// Big text for a title, centred on a column: `colors.0` at the top
    /// fading to `colors.1` at the bottom, a black shadow under it, and a
    /// white gleam that sweeps across every two seconds of `time`.
    pub fn fancy_text(&mut self, cx: i32, y: i32, s: &str, scale: i32, time: f32, colors: (Rgb, Rgb)) {
        let (w, h) = (Frame::text_width(s, true, scale), 7 * scale);
        let mut m = Frame::new(w + 1, h + 1);
        m.text_scaled(0, 0, s, WHITE, true, scale);
        let x0 = cx - w / 2;
        let lit = |xx: i32, yy: i32| m.get(xx, yy) != BLACK;
        for yy in 0..h {
            for xx in 0..w {
                if lit(xx, yy) { self.put(x0 + xx + scale / 2 + 1, y + yy + scale / 2 + 1, BLACK); }
            }
        }
        let band = ((time * 0.5).fract() * (w + h) as f32 * 1.6) as i32 - h;
        for yy in 0..h {
            for xx in 0..w {
                if !lit(xx, yy) { continue; }
                let mut c = mix(colors.0, colors.1, yy as f32 / h as f32);
                let d = (xx + yy - band).abs();
                if d < scale * 2 { c = mix(c, WHITE, 1.0 - d as f32 / (scale * 2) as f32); }
                self.put(x0 + xx, y + yy, c);
            }
        }
    }

    /// A filled circle.
    pub fn circle(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f32).sqrt() as i32;
            self.hline(cx - half, cy + dy, half * 2 + 1, c);
        }
    }

    /// Darken everything by a factor 0..1, for a pause or a game over.
    pub fn dim(&mut self, k: f32) {
        let k = k.clamp(0.0, 1.0);
        for p in self.px.iter_mut() {
            let (r, g, b) = parts(*p);
            *p = rgb((r as f32 * k) as u8, (g as f32 * k) as u8, (b as f32 * k) as u8);
        }
    }

    /// A sprite drawn `scale` times bigger.
    pub fn blit_scaled(&mut self, s: &Sprite, x: i32, y: i32, scale: i32, flip_x: bool) {
        let scale = scale.max(1);
        for sy in 0..s.h {
            for sx in 0..s.w {
                let src = if flip_x { s.w - 1 - sx } else { sx };
                let p = s.px[(sy * s.w + src) as usize];
                if p & 0xff00_0000 == 0 { continue; }
                self.rect(x + sx * scale, y + sy * scale, scale, scale, p & 0xff_ffff);
            }
        }
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
    fn colours_mix_and_tint() {
        assert_eq!(mix(0x000000, 0xffffff, 0.0), 0x000000);
        assert_eq!(mix(0x000000, 0xffffff, 1.0), 0xffffff);
        assert_eq!(mix(0x102030, 0x304050, 0.5), 0x203040);
        assert_eq!(tint(0x406080, 0.5), 0x203040);
        assert_eq!(tint(0x808080, 4.0), 0xffffff, "a channel stops at full");
    }

    #[test]
    fn fancy_text_fades_from_top_to_bottom_over_a_shadow() {
        let mut f = Frame::new(60, 20);
        f.clear(0x00ff00);
        f.fancy_text(30, 2, "HI", 2, 0.9, (0xff0000, 0x0000ff));
        let col = (0..60).find(|&x| f.get(x, 2) != 0x00ff00).expect("the top row of the text is drawn");
        assert_eq!(f.get(col, 2), 0xff0000, "the top row has the first colour");
        assert!(f.px.iter().any(|&p| p == BLACK), "and a shadow lies under it");
        assert!(f.px.iter().any(|&p| parts(p).2 > 200 && parts(p).0 < 60), "the bottom rows near the second colour");
    }

    #[test]
    fn text_uses_four_pixels_a_character() {
        let mut f = Frame::new(20, 6);
        assert_eq!(f.text(0, 0, "AB", WHITE), 8);
        assert!(f.px.iter().any(|&p| p == WHITE));
    }
}
