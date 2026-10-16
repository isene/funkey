//! Pictures: the board painted once, the counters, the marks on hexes.

use crate::map::{Ground, Map, HH, S};
use crate::scenario::{Kind, Scenario};
use funkey::*;

pub const INK: Rgb = 0x2a2116;
const RIVER: Rgb = 0x3570c0;
const OUTSIDE: Rgb = 0x1c2026;

/// A number from a pixel or a hex, the same every time.
fn hash(a: i32, b: i32, s: u32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1) ^ (b as u32).wrapping_mul(0x85eb_ca77) ^ s.wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h
}

fn base(g: Ground) -> Rgb {
    match g {
        Ground::Clear => 0xdcd19e,
        Ground::Forest => 0xa9bc80,
        Ground::Rough => 0xd2bd8c,
        Ground::Marsh => 0xbcd2ab,
        Ground::Mountain => 0xbfae90,
        Ground::Sea => 0x76a5cc,
    }
}

/// True for a pixel inside a hex centred at (cx, cy), a little smaller
/// by `inset` pixels.
pub fn inside(px: f32, py: f32, cx: f32, cy: f32, inset: f32) -> bool {
    let (dx, dy) = ((px - cx).abs(), (py - cy).abs());
    dy <= HH / 2.0 - inset && dx <= S - inset - dy / 3f32.sqrt()
}

/// Every pixel of a hex, as the frame sees it with the board's corner at
/// (ox, oy).
pub fn each_pixel(cx: f32, cy: f32, inset: f32, mut f: impl FnMut(i32, i32)) {
    for y in (cy - HH / 2.0).floor() as i32..=(cy + HH / 2.0).ceil() as i32 {
        for x in (cx - S).floor() as i32..=(cx + S).ceil() as i32 {
            if inside(x as f32 + 0.5, y as f32 + 0.5, cx, cy, inset) { f(x, y); }
        }
    }
}

/// Lighten or colour a whole hex: `k` of `c` mixed into what is there.
pub fn wash(fr: &mut Frame, cx: f32, cy: f32, c: Rgb, k: f32) {
    each_pixel(cx, cy, 1.0, |x, y| {
        let p = fr.get(x, y);
        fr.put(x, y, mix(p, c, k));
    });
}

/// A hex's outline, `w` pixels thick, inside its edge.
pub fn ring(fr: &mut Frame, cx: f32, cy: f32, c: Rgb, w: f32) {
    each_pixel(cx, cy, 0.0, |x, y| {
        if !inside(x as f32 + 0.5, y as f32 + 0.5, cx, cy, w) { fr.put(x, y, c); }
    });
}

/// A line `w` pixels wide.
fn thick(fr: &mut Frame, a: (f32, f32), b: (f32, f32), w: i32, c: Rgb) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0) as i32 + 1;
    for s in 0..=n {
        let t = s as f32 / n as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let (x0, y0) = ((x - w as f32 / 2.0).round() as i32, (y - w as f32 / 2.0).round() as i32);
        fr.rect(x0, y0, w, w, c);
    }
}

/// The board as it never changes: ground, rivers, cities and names.
pub fn board(sc: &Scenario, map: &Map) -> Frame {
    let (w, h) = map.pixels();
    let mut fr = Frame::new(w, h);
    fr.clear(OUTSIDE);
    for i in 0..map.len() {
        let g = map.ground[i];
        let (cx, cy) = map.center(i);
        let b = base(g);
        each_pixel(cx, cy, 0.0, |x, y| {
            let n = hash(x, y, 1) % 9;
            fr.put(x, y, tint(b, 0.96 + n as f32 * 0.01));
        });
        let (c, r) = map.cr(i);
        let mut dots = |n: u32, f: &mut dyn FnMut(&mut Frame, i32, i32)| {
            for k in 0..n {
                let hx = hash(c, r, 10 + k);
                let (x, y) = ((hx % 23) as i32 - 11, ((hx >> 8) % 19) as i32 - 9);
                f(&mut fr, cx as i32 + x, cy as i32 + y);
            }
        };
        match g {
            Ground::Forest => dots(7, &mut |fr, x, y| {
                fr.circle(x, y, 3, 0x5e7d45);
                fr.put(x - 1, y - 1, 0x86a661);
            }),
            Ground::Rough => dots(5, &mut |fr, x, y| {
                fr.line(x - 3, y + 1, x, y - 2, 0x9c8158);
                fr.line(x, y - 2, x + 3, y + 1, 0x9c8158);
            }),
            Ground::Marsh => dots(6, &mut |fr, x, y| {
                fr.hline(x - 3, y, 6, 0x5f9496);
                fr.put(x - 1, y - 2, 0x5f9496);
                fr.put(x + 1, y - 2, 0x5f9496);
            }),
            Ground::Mountain => dots(3, &mut |fr, x, y| {
                for k in 0..6 {
                    fr.hline(x - k, y - 5 + k, 2 * k + 1, if k < 2 { 0xf2efe8 } else { 0x7d6c57 });
                }
            }),
            Ground::Sea => dots(4, &mut |fr, x, y| {
                fr.hline(x - 3, y, 3, 0x93bcdf);
                fr.hline(x, y - 1, 3, 0x93bcdf);
            }),
            Ground::Clear => {}
        }
    }
    // The grid, a little darker than the ground under it.
    for i in 0..map.len() {
        let (cx, cy) = map.center(i);
        each_pixel(cx, cy, 0.0, |x, y| {
            if !inside(x as f32 + 0.5, y as f32 + 0.5, cx, cy, 0.8) {
                let p = fr.get(x, y);
                fr.put(x, y, tint(p, 0.82));
            }
        });
    }
    // The coast.
    for i in 0..map.len() {
        if !map.land(i) { continue; }
        for (d, n) in map.around(i) {
            if map.land(n) { continue; }
            let k = (d + 4) % 6;
            thick(&mut fr, map.corner(i, k), map.corner(i, (k + 1) % 6), 2, 0x4a7aa6);
        }
    }
    for &(a, b) in &map.river_lines {
        thick(&mut fr, a, b, 3, RIVER);
    }
    for c in sc.cities {
        let i = map.at(c.col, c.row).unwrap();
        let (cx, cy) = map.center(i);
        let (x, y) = (cx as i32, cy as i32);
        match c.size {
            0 => fr.rect(x - 2, y - 2, 4, 4, INK),
            1 => {
                fr.rect(x - 4, y - 4, 8, 8, INK);
                fr.rect(x - 3, y - 3, 6, 6, 0x8a7a66);
            }
            _ => {
                fr.rect(x - 7, y - 5, 14, 10, INK);
                fr.rect(x - 6, y - 4, 5, 4, 0x8a7a66);
                fr.rect(x + 1, y - 4, 5, 4, 0x8a7a66);
                fr.rect(x - 3, y + 1, 6, 3, 0x8a7a66);
            }
        }
        let name = if c.vp > 0 { format!("{} {}", c.name, c.vp) } else { c.name.to_string() };
        let tw = Frame::text_width(&name, false, 1);
        let (tx, ty) = (x - tw / 2, y + 8);
        fr.rect(tx - 1, ty - 1, tw + 1, 7, 0xf1ead2);
        fr.text(tx, ty, &name, if c.vp > 0 { 0x9a1b10 } else { INK });
    }
    fr
}

/// Copy the part of the board under the view into the frame.
pub fn show_board(fr: &mut Frame, board: &Frame, vx: i32, vy: i32, w: i32, h: i32) {
    for y in 0..h.min(fr.h) {
        let by = y + vy;
        if by < 0 || by >= board.h { continue; }
        for x in 0..w.min(fr.w) {
            let bx = x + vx;
            let p = if bx >= 0 && bx < board.w { board.px[(by * board.w + bx) as usize] } else { OUTSIDE };
            fr.px[(y * fr.w + x) as usize] = p;
        }
    }
}

pub struct Look {
    pub paper: Rgb,
    pub ink: Rgb,
    pub kind: Kind,
    pub numbers: String,
    pub reduced: bool,
    pub spent: bool,
    pub hungry: bool,
    pub picked: bool,
}

/// A counter, 22 pixels square, centred at (x, y): the unit's sign in
/// a box, and its attack, defence and movement under it.
pub fn counter(fr: &mut Frame, x: i32, y: i32, l: &Look) {
    let (x0, y0) = (x - 11, y - 11);
    let paper = if l.spent { tint(l.paper, 0.68) } else { l.paper };
    let ink = if l.spent { tint(l.ink, 0.8) } else { l.ink };
    fr.rect(x0 + 1, y0 + 1, 22, 22, 0x000000);
    fr.rect(x0, y0, 22, 22, tint(paper, 0.7));
    fr.rect(x0 + 1, y0 + 1, 20, 20, paper);
    if l.reduced { fr.rect(x0 + 1, y0 + 1, 20, 3, mix(paper, WHITE, 0.6)); }
    // The sign: a box with a cross for foot troops, an oval for tanks,
    // both for motorised troops, a stroke for horse and a peak for the
    // mountains.
    let (bx, by, bw, bh) = (x0 + 4, y0 + 5, 14, 8);
    fr.hline(bx, by, bw, ink);
    fr.hline(bx, by + bh - 1, bw, ink);
    fr.vline(bx, by, bh, ink);
    fr.vline(bx + bw - 1, by, bh, ink);
    let cross = |fr: &mut Frame| {
        fr.line(bx, by, bx + bw - 1, by + bh - 1, ink);
        fr.line(bx, by + bh - 1, bx + bw - 1, by, ink);
    };
    let oval = |fr: &mut Frame| {
        fr.hline(bx + 4, by + 2, 6, ink);
        fr.hline(bx + 4, by + 5, 6, ink);
        fr.vline(bx + 3, by + 3, 2, ink);
        fr.vline(bx + 10, by + 3, 2, ink);
    };
    match l.kind {
        Kind::Inf => cross(fr),
        Kind::Arm => oval(fr),
        Kind::Mot => {
            cross(fr);
            oval(fr);
        }
        Kind::Cav => fr.line(bx, by + bh - 1, bx + bw - 1, by, ink),
        Kind::Mtn => {
            cross(fr);
            fr.rect(bx + 6, by + bh - 3, 2, 2, ink);
            fr.hline(bx + 5, by + bh - 2, 4, ink);
        }
    }
    let tw = Frame::text_width(&l.numbers, false, 1) - 1;
    fr.text(x - tw / 2, y0 + 15, &l.numbers, ink);
    if l.hungry {
        // Out of supply: a yellow edge.
        for k in 0..2 {
            fr.hline(x0 + k, y0 + k, 22 - 2 * k, 0xffd400);
            fr.hline(x0 + k, y0 + 21 - k, 22 - 2 * k, 0xffd400);
            fr.vline(x0 + k, y0 + k, 22 - 2 * k, 0xffd400);
            fr.vline(x0 + 21 - k, y0 + k, 22 - 2 * k, 0xffd400);
        }
    }
    if l.picked {
        for k in -2..0 {
            fr.hline(x0 + k, y0 + k, 22 - 2 * k, WHITE);
            fr.hline(x0 + k, y0 + 21 - k, 22 - 2 * k, WHITE);
            fr.vline(x0 + k, y0 + k, 22 - 2 * k, WHITE);
            fr.vline(x0 + 21 - k, y0 + k, 22 - 2 * k, WHITE);
        }
    }
}
