//! Pictures: the board painted once, the counters, the marks on hexes.

use super::map::{Ground, Map, HH, S};
use super::scenario::{Kind, Land, Scenario};
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

fn base(land: Land, g: Ground) -> Rgb {
    match (land, g) {
        (Land::Desert, Ground::Clear) => 0xe8d3a2,
        (Land::Desert, Ground::Forest) => 0xa9c47c,
        (Land::Desert, Ground::Rough) => 0xd4b47e,
        (Land::Desert, Ground::Marsh) => 0xdcd8cc,
        (Land::Desert, Ground::Mountain) => 0xecc888,
        (_, Ground::Clear) => 0xdcd19e,
        (_, Ground::Forest) => 0xa9bc80,
        (_, Ground::Rough) => 0xd2bd8c,
        (_, Ground::Marsh) => 0xbcd2ab,
        (_, Ground::Mountain) => 0xbfae90,
        (_, Ground::Sea) => 0x76a5cc,
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

/// The board as it never changes, and the same board small for the
/// title, at most `mw` by `mh` pixels.
pub fn boards(sc: &Scenario, map: &Map, mw: i32, mh: i32) -> (Frame, Frame) {
    let mut b = ground(sc, map);
    let small = mini(sc, map, &b, mw, mh);
    names(&mut b, sc, map);
    (b, small)
}

/// The ground, the grid, the coast and the rivers.
fn ground(sc: &Scenario, map: &Map) -> Frame {
    let (w, h) = map.pixels();
    let mut fr = Frame::new(w, h);
    fr.clear(OUTSIDE);
    for i in 0..map.len() {
        let g = map.ground[i];
        let (cx, cy) = map.center(i);
        let b = base(sc.land, g);
        let desert = sc.land == Land::Desert;
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
            Ground::Forest if desert => dots(4, &mut |fr, x, y| {
                for k in 0..3 { fr.hline(x - 4, y - 2 + 2 * k, 8, 0x7f9e57); }
            }),
            Ground::Forest => dots(7, &mut |fr, x, y| {
                fr.circle(x, y, 3, 0x5e7d45);
                fr.put(x - 1, y - 1, 0x86a661);
            }),
            Ground::Rough => dots(5, &mut |fr, x, y| {
                fr.line(x - 3, y + 1, x, y - 2, 0x9c8158);
                fr.line(x, y - 2, x + 3, y + 1, 0x9c8158);
            }),
            Ground::Marsh if desert => dots(7, &mut |fr, x, y| {
                fr.hline(x - 2, y, 4, 0xf6f4ee);
                fr.put(x + 1, y + 1, 0xb4ab98);
            }),
            Ground::Mountain if desert => dots(3, &mut |fr, x, y| {
                // A dune: a crest with its shadow under it.
                for k in -5..=5i32 {
                    let h = (k * k) / 6;
                    fr.put(x + k, y - 2 + h, 0xb88a4c);
                    fr.put(x + k, y - 1 + h, 0xd9ad6a);
                }
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
            Ground::Clear if desert => dots(5, &mut |fr, x, y| fr.put(x, y, 0xc4ad80)),
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
    fr
}

/// The cities and their names.
fn names(fr: &mut Frame, sc: &Scenario, map: &Map) {
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
}

/// The ground shrunk smoothly, each small pixel the mean of the big ones
/// under it, then sharp on top: the rivers, the cities, the forces of the
/// first turn and the names of the cities with points.
fn mini(sc: &Scenario, map: &Map, g: &Frame, mw: i32, mh: i32) -> Frame {
    let s = (mw as f32 / g.w as f32).min(mh as f32 / g.h as f32);
    let (w, h) = ((g.w as f32 * s) as i32, (g.h as f32 * s) as i32);
    let mut fr = Frame::new(w, h);
    let k = 1.0 / s;
    for y in 0..h {
        let (y0, y1) = (y as f32 * k, (y + 1) as f32 * k);
        for x in 0..w {
            let (x0, x1) = (x as f32 * k, (x + 1) as f32 * k);
            let mut sum = [0.0f32; 4];
            for sy in y0 as i32..(y1.ceil() as i32).min(g.h) {
                let wy = y1.min(sy as f32 + 1.0) - y0.max(sy as f32);
                for sx in x0 as i32..(x1.ceil() as i32).min(g.w) {
                    let wt = wy * (x1.min(sx as f32 + 1.0) - x0.max(sx as f32));
                    let (r, gr, b) = parts(g.px[(sy * g.w + sx) as usize]);
                    sum[0] += wt * r as f32;
                    sum[1] += wt * gr as f32;
                    sum[2] += wt * b as f32;
                    sum[3] += wt;
                }
            }
            let c = |v: f32| (v / sum[3]).round().clamp(0.0, 255.0) as u8;
            fr.px[(y * w + x) as usize] = rgb(c(sum[0]), c(sum[1]), c(sum[2]));
        }
    }
    let at = |(x, y): (f32, f32)| (x * s, y * s);
    let hex = |col, row| {
        let (x, y) = at(map.center(map.at(col, row).unwrap()));
        (x as i32, y as i32)
    };
    for &(a, b) in &map.river_lines {
        thick(&mut fr, at(a), at(b), 2, RIVER);
    }
    for c in sc.cities {
        let (x, y) = hex(c.col, c.row);
        let r = 1 + c.size.min(2) as i32;
        fr.rect(x - r, y - r, 2 * r, 2 * r, INK);
    }
    for f in sc.forces.iter().filter(|f| f.turn == 1) {
        let n = &sc.nations[f.kit.nat];
        let (x, y) = hex(f.col, f.row);
        fr.rect(x - 3, y - 3, 7, 7, n.ink);
        fr.rect(x - 2, y - 2, 5, 5, n.paper);
    }
    for c in sc.cities.iter().filter(|c| c.vp > 0) {
        let (x, y) = hex(c.col, c.row);
        let tw = Frame::text_width(c.name, false, 1);
        let (tx, ty) = (x - tw / 2, y + 5);
        fr.rect(tx - 1, ty - 1, tw + 1, 7, 0xf1ead2);
        fr.text(tx, ty, c.name, 0x9a1b10);
    }
    fr
}

/// Copy the part of the board under the view into the frame, `w` by `h`
/// pixels from row `top` down.
pub fn show_board(fr: &mut Frame, board: &Frame, vx: i32, vy: i32, top: i32, w: i32, h: i32) {
    for y in 0..h.min(fr.h - top) {
        let by = y + vy;
        for x in 0..w.min(fr.w) {
            let bx = x + vx;
            let p = if bx >= 0 && bx < board.w && by >= 0 && by < board.h { board.px[(by * board.w + bx) as usize] } else { OUTSIDE };
            fr.px[((y + top) * fr.w + x) as usize] = p;
        }
    }
}

pub struct Look {
    pub paper: Rgb,
    pub ink: Rgb,
    pub kind: Kind,
    /// How many marks go over the sign: 2 a division, 3 a corps.
    pub size: u8,
    pub numbers: String,
    pub reduced: bool,
    pub spent: bool,
    pub hungry: bool,
    pub picked: bool,
}

/// Darken a rectangle by `k`.
fn shade(fr: &mut Frame, x: i32, y: i32, w: i32, h: i32, k: f32) {
    for yy in y..y + h {
        for xx in x..x + w {
            let p = fr.get(xx, yy);
            fr.put(xx, yy, tint(p, 1.0 - k));
        }
    }
}

/// A unit's sign, as on a military map, in a box `w` by `h` with its
/// corner at (bx, by): a cross for foot troops, an oval for tanks, both
/// for mechanised troops, a stroke for horse and a peak for mountain
/// troops.
pub fn sign(fr: &mut Frame, bx: i32, by: i32, w: i32, h: i32, kind: Kind, field: Rgb, ink: Rgb) {
    fr.rect(bx, by, w, h, ink);
    fr.rect(bx + 1, by + 1, w - 2, h - 2, field);
    let (x1, y1) = (bx + w - 1, by + h - 1);
    let cross = |fr: &mut Frame| {
        fr.line(bx, by, x1, y1, ink);
        fr.line(bx, y1, x1, by, ink);
    };
    let oval = |fr: &mut Frame| {
        let (ow, oh) = (w * 5 / 9, h / 2);
        let (ox, oy) = (bx + (w - ow) / 2, by + (h - oh) / 2);
        fr.hline(ox + 1, oy, ow - 2, ink);
        fr.hline(ox + 1, oy + oh - 1, ow - 2, ink);
        fr.vline(ox, oy + 1, oh - 2, ink);
        fr.vline(ox + ow - 1, oy + 1, oh - 2, ink);
    };
    match kind {
        Kind::Inf => cross(fr),
        Kind::Arm => oval(fr),
        Kind::Mot => {
            cross(fr);
            oval(fr);
        }
        Kind::Cav => fr.line(bx, y1, x1, by, ink),
        Kind::Mtn => {
            cross(fr);
            let cx = bx + w / 2;
            for k in 0..3 { fr.hline(cx - 1 - k, y1 - 3 + k, 2 + 2 * k, ink); }
        }
    }
}

/// A counter, 26 pixels square, centred at (x, y): the marks for its
/// size, the unit's sign, and its attack, defence and movement.
pub fn counter(fr: &mut Frame, x: i32, y: i32, l: &Look) {
    const N: i32 = 26;
    let (x0, y0) = (x - N / 2, y - N / 2);
    let paper = if l.spent { tint(l.paper, 0.68) } else { l.paper };
    let ink = if l.spent { tint(l.ink, 0.8) } else { l.ink };
    shade(fr, x0 + 2, y0 + N, N, 2, 0.45);
    shade(fr, x0 + N, y0 + 2, 2, N - 2, 0.45);
    fr.rect(x0, y0, N, N, tint(paper, 0.5));
    fr.rect(x0 + 1, y0 + 1, N - 2, N - 2, paper);
    // Cardboard: light on the top and left, dark on the bottom and right.
    let (light, dark) = (mix(paper, WHITE, 0.45), tint(paper, 0.78));
    fr.hline(x0 + 1, y0 + 1, N - 2, light);
    fr.vline(x0 + 1, y0 + 1, N - 2, light);
    fr.hline(x0 + 1, y0 + N - 2, N - 2, dark);
    fr.vline(x0 + N - 2, y0 + 2, N - 3, dark);
    // A reduced unit shows its pale band, as on the back of a counter.
    if l.reduced { fr.rect(x0 + 2, y0 + 2, N - 4, 5, mix(paper, WHITE, 0.6)); }
    // The size: a small x for each mark.
    let n = l.size as i32;
    let mx = x - (4 * n - 1) / 2;
    for k in 0..n {
        let px = mx + 4 * k;
        for (dx, dy) in [(0, 0), (2, 0), (1, 1), (0, 2), (2, 2)] { fr.put(px + dx, y0 + 3 + dy, ink); }
    }
    sign(fr, x0 + 4, y0 + 7, 18, 11, l.kind, mix(paper, WHITE, 0.35), ink);
    let tw = Frame::text_width(&l.numbers, false, 1) - 1;
    fr.text(x - tw / 2, y0 + 19, &l.numbers, ink);
    let edge = |fr: &mut Frame, k: i32, c: Rgb| {
        fr.hline(x0 + k, y0 + k, N - 2 * k, c);
        fr.hline(x0 + k, y0 + N - 1 - k, N - 2 * k, c);
        fr.vline(x0 + k, y0 + k, N - 2 * k, c);
        fr.vline(x0 + N - 1 - k, y0 + k, N - 2 * k, c);
    };
    // Out of supply: a yellow edge.
    if l.hungry {
        for k in 0..2 { edge(fr, k, 0xffd400); }
    }
    if l.picked {
        for k in -2..0 { edge(fr, k, WHITE); }
    }
}

/// The key to the counters, as a strip `h` pixels high: each nation's
/// colour, then each kind of unit the scenario has.
pub fn legend(fr: &mut Frame, sc: &Scenario, w: i32, h: i32, bg: Rgb, text: Rgb) {
    fr.rect(0, 0, w, h, bg);
    fr.hline(0, h - 1, w, 0x3a4048);
    let ty = (h - 7) / 2;
    let mut x = 8;
    for n in sc.nations {
        fr.rect(x, ty - 1, 12, 9, tint(n.paper, 0.5));
        fr.rect(x + 1, ty, 10, 7, n.paper);
        x += 15;
        x += fr.text_big(x, ty, n.name, text) + 9;
    }
    x += 6;
    for (kind, name) in [(Kind::Inf, "INFANTRY"), (Kind::Arm, "TANKS"), (Kind::Mot, "MECHANISED"), (Kind::Cav, "CAVALRY"), (Kind::Mtn, "MOUNTAIN")] {
        if !sc.forces.iter().any(|f| f.kit.kind == kind) { continue; }
        sign(fr, x, ty - 1, 14, 9, kind, 0xd8d2c0, 0x1a1a1a);
        x += 17;
        x += fr.text_big(x, ty, name, text) + 9;
    }
}
