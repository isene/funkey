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
pub enum Backend { HalfBlocks, Kitty }

pub struct Screen {
    pub backend: Backend,
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
}

impl Screen {
    /// Take over the terminal: raw mode, the alternate screen, no cursor.
    pub fn open() -> Screen {
        Crust::init();
        let backend = match std::env::var("FUNKEY_PIXELS").as_deref() {
            Ok("kitty") => Backend::Kitty,
            _ => Backend::HalfBlocks,
        };
        let mut s = Screen { backend, cols: 0, rows: 0, w: 0, h: 0, last: Vec::new(), scaled: Frame::new(1, 1), out: String::new(), rgba: Vec::new() };
        s.resize();
        s
    }

    /// Read the terminal size again and forget the last frame, so the
    /// next one is drawn in full.
    pub fn resize(&mut self) {
        let (c, r) = Crust::terminal_size();
        self.cols = c.max(1) as i32;
        self.rows = r.max(1) as i32;
        self.w = self.cols;
        self.h = self.rows * 2;
        self.last = vec![(0xffff_ffff, 0xffff_ffff); (self.cols * self.rows) as usize];
        let mut so = std::io::stdout();
        let _ = so.write_all(seq::ERASE_ALL.as_bytes());
        let _ = so.flush();
    }

    /// Show a frame. The frame may be any size; see the module notes.
    pub fn present(&mut self, frame: &Frame) {
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

    /// The frame as real pixels: one kitty image, replaced every frame,
    /// stretched over the cells that keep the frame's shape.
    fn present_pixels(&mut self, frame: &Frame) {
        self.rgba.clear();
        self.rgba.reserve((frame.w * frame.h * 3) as usize);
        for &p in &frame.px {
            let (r, g, b) = parts(p);
            self.rgba.extend_from_slice(&[r, g, b]);
        }
        // Cells are about twice as tall as wide: a frame of w by h pixels
        // keeps its shape over w by h/2 cells, scaled to fit the window.
        let fit = (self.cols as f32 / frame.w as f32).min(self.rows as f32 * 2.0 / frame.h as f32);
        let cols = ((frame.w as f32 * fit) as i32).clamp(1, self.cols);
        let rows = ((frame.h as f32 * fit / 2.0) as i32).clamp(1, self.rows);
        let (ox, oy) = ((self.cols - cols) / 2, (self.rows - rows) / 2);
        self.out.clear();
        self.out.push_str(&Cursor::at(ox as u16 + 1, oy as u16 + 1));
        self.out.push_str(&glow::kitty_frame_rgb(1, frame.w as u32, frame.h as u32, cols as u16, rows as u16, &self.rgba));
        let mut so = std::io::stdout();
        let _ = so.write_all(self.out.as_bytes());
        let _ = so.flush();
    }

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
        if self.backend == Backend::Kitty {
            let mut so = std::io::stdout();
            let _ = so.write_all(glow::kitty_forget(1).as_bytes());
            let _ = so.flush();
        }
        Crust::cleanup();
    }
}
