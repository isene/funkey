//! The terminal as a pixel display, two ways:
//!
//! - Half blocks, in any truecolor terminal: each cell shows two pixels,
//!   the upper half block `▀` with the top pixel as foreground and the
//!   bottom one as background. Only cells that changed since the last
//!   frame are sent, in one write. A frame smaller than the display is
//!   scaled up by a whole number and centred; a larger one is scaled
//!   down to fit.
//! - Real pixels through the kitty graphics protocol, where the terminal
//!   has it: the whole frame goes out every time and the terminal scales
//!   it to the window. `FUNKEY_PIXELS=kitty` picks this. Inside glass the
//!   pixels travel over shared memory; elsewhere as base64.

use crate::frame::{parts, Frame, Rgb, BLACK};
use crust::cursor::{seq, Cursor};
use crust::style;
use crust::Crust;
use std::io::Write;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Backend {
    HalfBlocks,
    Kitty,
    /// A bare console: the pixels of the screen itself, written through
    /// glow. Chosen on its own where there is no terminal to ask.
    Screen,
}

pub struct Screen {
    pub backend: Backend,
    /// The console screen, opened once and kept for the whole game.
    fb: Option<glow::fb::Screen>,
    /// Cells across and down.
    pub cols: i32,
    pub rows: i32,
    /// Pixels the display holds: cols by rows * 2.
    pub w: i32,
    pub h: i32,
    last: Vec<(Rgb, Rgb)>,
    scaled: Frame,
    out: String,
    rgba: Vec<u8>,
    big: bool,
}

impl Screen {
    /// Take over the terminal: raw mode, the alternate screen, no cursor.
    pub fn open() -> Screen {
        Crust::init();
        Crust::enable_key_release();
        let backend = match std::env::var("FUNKEY_PIXELS").as_deref() {
            Ok("kitty") => Backend::Kitty,
            Ok("blocks") => Backend::HalfBlocks,
            // On a console there is no protocol to speak, but the screen
            // is right there. Real pixels beat blocks every time.
            _ if glow::fb::there() => Backend::Screen,
            _ => Backend::HalfBlocks,
        };
        let fb = if backend == Backend::Screen { glow::fb::Screen::open() } else { None };
        let backend = if backend == Backend::Screen && fb.is_none() { Backend::HalfBlocks } else { backend };
        let mut s = Screen { backend, fb, cols: 0, rows: 0, w: 0, h: 0, last: Vec::new(), scaled: Frame::new(1, 1), out: String::new(), rgba: Vec::new(), big: false };
        s.resize();
        s
    }

    /// Read the terminal size again and forget the last frame, so the
    /// next one is drawn in full.
    pub fn resize(&mut self) {
        let (c, r) = Crust::terminal_size();
        self.cols = c.max(1) as i32;
        self.rows = r.max(1) as i32;
        if let Some(fb) = &self.fb {
            // The display is the screen, pixel for pixel.
            self.w = fb.w as i32;
            self.h = fb.h as i32;
            self.last = Vec::new();
            return;
        }
        self.w = self.cols;
        self.h = self.rows * 2;
        self.last = vec![(0xffff_ffff, 0xffff_ffff); (self.cols * self.rows) as usize];
        let mut so = std::io::stdout();
        let _ = so.write_all(seq::ERASE_ALL.as_bytes());
        let _ = so.flush();
    }

    /// Show a frame. The frame may be any size; see the module notes.
    pub fn present(&mut self, frame: &Frame) {
        if self.backend == Backend::Screen { return self.present_screen(frame); }
        if self.backend == Backend::Kitty { return self.present_pixels(frame); }
        let needs_scale = frame.w != self.w || frame.h != self.h;
        if needs_scale { self.scale(frame); }
        // Take the scaled frame out while the cells are read, so the
        // borrow checker sees one frame and one screen.
        let scaled = std::mem::replace(&mut self.scaled, Frame::new(1, 1));
        let src = if needs_scale { &scaled } else { frame };
        self.out.clear();
        let (mut cur_fg, mut cur_bg): (Option<Rgb>, Option<Rgb>) = (None, None);
        let mut cursor_at: Option<(i32, i32)> = None;
        for row in 0..self.rows {
            for col in 0..self.cols {
                let top = src.get(col, row * 2);
                let bot = src.get(col, row * 2 + 1);
                let i = (row * self.cols + col) as usize;
                if self.last[i] == (top, bot) { continue; }
                self.last[i] = (top, bot);
                if cursor_at != Some((col, row)) {
                    self.out.push_str(&Cursor::at(col as u16 + 1, row as u16 + 1));
                }
                if cur_fg != Some(top) {
                    let (r, g, b) = parts(top);
                    self.out.push_str(&style::set_fg_rgb(r, g, b));
                    cur_fg = Some(top);
                }
                if cur_bg != Some(bot) {
                    let (r, g, b) = parts(bot);
                    self.out.push_str(&style::set_bg_rgb(r, g, b));
                    cur_bg = Some(bot);
                }
                self.out.push('▀');
                cursor_at = if col + 1 < self.cols { Some((col + 1, row)) } else { None };
            }
        }
        self.scaled = scaled;
        if self.out.is_empty() { return; }
        self.out.push_str(style::RESET);
        let mut so = std::io::stdout();
        let _ = so.write_all(self.out.as_bytes());
        let _ = so.flush();
    }

    /// The frame on a bare console: scaled to the screen, keeping its
    /// shape, and written straight to the pixels. No terminal is asked
    /// anything, so nothing is drawn per cell and nothing is encoded.
    fn present_screen(&mut self, frame: &Frame) {
        if frame.w != self.w || frame.h != self.h {
            self.scale(frame);
        }
        let shown: &Frame = if frame.w != self.w || frame.h != self.h { &self.scaled } else { frame };
        self.rgba.clear();
        self.rgba.reserve((self.w * self.h * 4) as usize);
        for &p in &shown.px {
            let (r, g, b) = parts(p);
            self.rgba.extend_from_slice(&[r, g, b, 255]);
        }
        if let Some(fb) = &self.fb {
            fb.blit(0, 0, self.w as usize, self.h as usize, &self.rgba);
        }
    }

    /// The frame as real pixels: one kitty image, replaced every frame,
    /// stretched over the cells that keep the frame's shape.
    fn present_pixels(&mut self, frame: &Frame) {
        // The terminal scales the placement into the cells asked for, so
        // the frame goes out at its own size. The cells keep the frame's
        // shape, from the terminal's cell size in pixels (10 by 20 when it
        // does not say).
        let (cw, ch) = glow::get_cell_size();
        let (cw, ch) = (cw.max(1) as f32, ch.max(1) as f32);
        self.rgba.clear();
        self.rgba.reserve((frame.w * frame.h * 3) as usize);
        for &p in &frame.px {
            let (r, g, b) = parts(p);
            self.rgba.extend_from_slice(&[r, g, b]);
        }
        let fit = (self.cols as f32 * cw / frame.w as f32).min(self.rows as f32 * ch / frame.h as f32);
        let cols = ((frame.w as f32 * fit / cw).round() as i32).clamp(1, self.cols);
        let rows = ((frame.h as f32 * fit / ch).round() as i32).clamp(1, self.rows);
        let (ox, oy) = ((self.cols - cols) / 2, (self.rows - rows) / 2);
        let (w, h) = (frame.w, frame.h);
        self.big = cols * rows * 2 > self.cols * self.rows;
        if std::env::var_os("FUNKEY_DEBUG").is_some() {
            let _ = std::fs::write(crate::debug_path(), format!("terminal {}x{} cells of {}x{} px; frame {}x{} over {}x{} cells at {},{}\n",
                self.cols, self.rows, cw, ch, w, h, cols, rows, ox, oy));
        }
        self.out.clear();
        self.out.push_str(&Cursor::at(ox as u16 + 1, oy as u16 + 1));
        self.out.push_str(&glow::kitty_frame_rgb(1, w as u32, h as u32, cols as u16, rows as u16, &self.rgba));
        let mut so = std::io::stdout();
        let _ = so.write_all(self.out.as_bytes());
        let _ = so.flush();
    }

    /// True when real pixels cover more than half the window, where a
    /// lower frame rate saves the display server real work.
    pub fn big(&self) -> bool { self.backend == Backend::Kitty && self.big }

    /// Nearest-neighbour scaling into the display, keeping the frame's
    /// shape, black around it.
    fn scale(&mut self, frame: &Frame) {
        if self.scaled.w != self.w || self.scaled.h != self.h { self.scaled = Frame::new(self.w, self.h); }
        let up = (self.w / frame.w).min(self.h / frame.h);
        let (dw, dh) = if up >= 1 {
            (frame.w * up, frame.h * up)
        } else {
            // Shrink to fit, keeping the shape.
            let sx = self.w as f32 / frame.w as f32;
            let sy = self.h as f32 / frame.h as f32;
            let s = sx.min(sy);
            (((frame.w as f32) * s) as i32, ((frame.h as f32) * s) as i32)
        };
        let (ox, oy) = ((self.w - dw) / 2, (self.h - dh) / 2);
        self.scaled.clear(BLACK);
        for y in 0..dh {
            let sy = (y as i64 * frame.h as i64 / dh as i64) as i32;
            for x in 0..dw {
                let sx = (x as i64 * frame.w as i64 / dw as i64) as i32;
                self.scaled.px[((oy + y) * self.w + ox + x) as usize] = frame.px[(sy * frame.w + sx) as usize];
            }
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        if let Some(fb) = &self.fb {
            // Leave the console its own screen back, black.
            fb.fill(0, 0, fb.w, fb.h, (0, 0, 0));
        }
        if self.backend == Backend::Kitty {
            let mut so = std::io::stdout();
            let _ = so.write_all(glow::kitty_forget(1).as_bytes());
            let _ = so.flush();
        }
        Crust::disable_modifier_keys();
        Crust::cleanup();
    }
}
