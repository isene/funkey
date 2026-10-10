//! vector: a tribute to Tempest (Atari, 1981). Glowing lines on black: a
//! web seen down its length, a claw on its rim, and what climbs up the
//! lanes toward it. Sixteen webs, then the same again in new colours,
//! faster.
//!
//!     cargo run --release --example vector
//!
//! Left and Right move the claw along the rim, Space fires down the lane.
//! Z or Down is the superzapper: once a web it clears the web, a second
//! time it kills one. P pauses, Q quits.
//!
//! Flippers climb and flip from lane to lane; on the rim they come for
//! the claw, and can be shot only as they flip in. A tanker splits in two.
//! Spikers leave spikes, to shoot away or to steer clear of on the flight
//! down to the next web. A fuseball rides the lane edges and can be hit
//! only while it crosses a lane. A pulsar charges the lane it sits in.
//!
//! `VECTOR_START=<level>` starts at another web; `VECTOR_BENCH=<frames>`
//! times the game with no terminal. Left alone on the title, it plays
//! itself. Everything here is new: the webs, the letters and the sounds.

use funkey::*;
use std::f32::consts::TAU;

const W: i32 = 640;
const H: i32 = 400;
const GAME: &str = "vector";
const VERSION: &str = "1.0";
/// How small the far end of a web is beside its rim.
const FAR: f32 = 0.133;
/// The middle of the screen, and the size of a web on it.
const CX: f32 = 320.0;
const CY: f32 = 212.0;
const RX: f32 = 196.0;
const RY: f32 = 160.0;
/// How many of the claw's shots fly at once, and how fast: with the fire
/// key held, eight leave in a burst and then there is a gap, as in the
/// arcade.
const SHOTS: usize = 8;
const SHOT: f32 = 1.7;
/// Seconds from one pulse of a pulsar to the next.
const PERIOD: f32 = 2.6;
/// The score that gives another claw, and each one after it.
const EXTRA: u32 = 20000;
const WEBS: u32 = 16;

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

/// An angle into -PI..PI.
fn turn(a: f32) -> f32 { (a + TAU / 2.0).rem_euclid(TAU) - TAU / 2.0 }

fn far(a: (f32, f32), b: (f32, f32)) -> f32 { (a.0 - b.0).hypot(a.1 - b.1) }

/// A number from 0 to 1 out of a whole number, the same each time.
fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    x ^= x >> 15; x = x.wrapping_mul(0x2c1b_3c6d); x ^= x >> 12;
    (x >> 8) as f32 / 16_777_216.0
}

/// One colour shaded toward another.
fn shade(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let (a, b, t) = (parts(a), parts(b), t.clamp(0.0, 1.0));
    rgb(lerp(a.0 as f32, b.0 as f32, t) as u8, lerp(a.1 as f32, b.1 as f32, t) as u8, lerp(a.2 as f32, b.2 as f32, t) as u8)
}

/// The strokes of a letter on a grid four wide and six high: each stroke
/// a run of points, a point two digits, across then down.
fn glyph(c: char) -> &'static str {
    match c.to_ascii_uppercase() {
        'A' => "0602204246 0444", 'B' => "06003041423303 3344453606", 'C' => "4130100105163645", 'D' => "06003041453606",
        'E' => "40000646 0333", 'F' => "400006 0333", 'G' => "41301001051636454323", 'H' => "0006 4046 0343", 'I' => "1030 2026 1636",
        'J' => "4045361605", 'K' => "0006 400346", 'L' => "000646", 'M' => "0600234046", 'N' => "06004640",
        'O' => "103041453616050110", 'P' => "06003041423303", 'Q' => "103041453616050110 2446", 'R' => "06003041423303 2346",
        'S' => "413010010213334445361605", 'T' => "0040 2026", 'U' => "000516364540", 'V' => "002640", 'W' => "0016233640",
        'X' => "0046 4006", 'Y' => "002340 2326", 'Z' => "00400646",
        '0' => "0040460600", '1' => "112026 1636", '2' => "004043030646", '3' => "00404606 0343", '4' => "000343 4046",
        '5' => "400003434606", '6' => "400006464303", '7' => "004046", '8' => "0040460600 0343", '9' => "430300404606",
        '-' => "0343", '.' => "2526", ':' => "2122 2425", '!' => "2024 2526", '/' => "0640", '<' => "400346", '>' => "004306",
        _ => "",
    }
}

/// The screen as a vector tube shows it: lines add their light where they
/// cross, each has a glow about it, and what was lit fades over a few
/// frames.
#[derive(Default)]
struct Glow {
    /// What this frame drew, and what the tube still shows.
    now: Vec<[f32; 3]>,
    held: Vec<[f32; 3]>,
    /// All that is drawn is this much brighter or dimmer.
    gain: f32,
}

impl Glow {
    fn new() -> Glow {
        let n = (W * H) as usize;
        Glow { now: vec![[0.0; 3]; n], held: vec![[0.0; 3]; n], gain: 1.0 }
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), c: Rgb, k: f32) {
        let (w, h) = (W as f32, H as f32);
        if (a.0 < -4.0 && b.0 < -4.0) || (a.1 < -4.0 && b.1 < -4.0) || (a.0 > w + 4.0 && b.0 > w + 4.0) || (a.1 > h + 4.0 && b.1 > h + 4.0) { return; }
        let (r, g, bl) = parts(c);
        let k = k * self.gain;
        let col = [r as f32 * k, g as f32 * k, bl as f32 * k];
        // Walk the long way of the line: `m` runs along it, `n` across.
        let steep = (b.1 - a.1).abs() > (b.0 - a.0).abs();
        let (mut a, mut b) = if steep { ((a.1, a.0), (b.1, b.0)) } else { (a, b) };
        if a.0 > b.0 { std::mem::swap(&mut a, &mut b); }
        let run = b.0 - a.0;
        let (slope, thin) = if run > 0.001 { ((b.1 - a.1) / run, run / run.hypot(b.1 - a.1)) } else { (0.0, 1.0) };
        let (ms, ns) = if steep { (H, W) } else { (W, H) };
        let (m0, m1) = (((a.0 - 4.0).floor() as i32).max(0), ((b.0 + 4.0).ceil() as i32).min(ms - 1));
        for m in m0..=m1 {
            // Past an end the light falls off round it.
            let on = (m as f32).clamp(a.0, b.0);
            let past = m as f32 - on;
            let mid = a.1 + (on - a.0) * slope;
            let (n0, n1) = (((mid - 4.0).floor() as i32).max(0), ((mid + 4.0).ceil() as i32).min(ns - 1));
            for n in n0..=n1 {
                let across = (n as f32 - mid) * thin;
                let d2 = across * across + past * past;
                // A sharp core one pixel wide, and the glow about it.
                let halo = 1.0 + d2 * 0.25;
                let v = (1.1 - d2.sqrt()).clamp(0.0, 1.0) + 0.45 / (halo * halo);
                if v < 0.02 { continue; }
                let p = &mut self.now[if steep { m * W + n } else { n * W + m } as usize];
                p[0] += col[0] * v; p[1] += col[1] * v; p[2] += col[2] * v;
            }
        }
    }

    /// A run of lines from point to point.
    fn path(&mut self, pts: &[(f32, f32)], c: Rgb, k: f32) {
        for p in pts.windows(2) { self.line(p[0], p[1], c, k); }
    }

    /// Text in letters `size` pixels high, from its top left corner.
    fn text(&mut self, x: f32, y: f32, s: &str, size: f32, c: Rgb, k: f32) {
        let u = size / 6.0;
        for (i, ch) in s.chars().enumerate() {
            let x0 = x + i as f32 * size;
            for stroke in glyph(ch).split(' ') {
                let b = stroke.as_bytes();
                let pt = |j: usize| (x0 + (b[j] - b'0') as f32 * u, y + (b[j + 1] - b'0') as f32 * u);
                for j in (0..b.len().saturating_sub(3)).step_by(2) { self.line(pt(j), pt(j + 2), c, k); }
            }
        }
    }

    fn width(s: &str, size: f32) -> f32 { (s.chars().count() as f32 * 6.0 - 2.0).max(0.0) * size / 6.0 }

    fn centered(&mut self, cx: f32, y: f32, s: &str, size: f32, c: Rgb, k: f32) {
        self.text(cx - Glow::width(s, size) / 2.0, y, s, size, c, k);
    }

    /// Onto the frame: the fresh light over what is left of the old, which
    /// keeps `keep` of itself. What is brighter than the tube can show
    /// turns white.
    fn show(&mut self, f: &mut Frame, keep: f32) {
        for ((o, h), n) in f.px.iter_mut().zip(self.held.iter_mut()).zip(self.now.iter_mut()) {
            let p = [(h[0] * keep).max(n[0]), (h[1] * keep).max(n[1]), (h[2] * keep).max(n[2])];
            *h = p;
            *n = [0.0; 3];
            let top = p[0].max(p[1]).max(p[2]);
            if top < 1.0 { *o = 0; continue; }
            let over = (top - 255.0).max(0.0) * 0.5;
            *o = rgb((p[0] + over).min(255.0) as u8, (p[1] + over).min(255.0) as u8, (p[2] + over).min(255.0) as u8);
        }
    }
}

/// A web: the points of its rim, seen from the front, in a square from -1
/// to 1. A lane runs between two points next to each other.
struct Web {
    pts: Vec<(f32, f32)>,
    /// A closed web runs all the way round; an open one has two ends.
    closed: bool,
    /// Where its far end lies.
    far: (f32, f32),
    lanes: usize,
}

/// Points round a middle, the first straight up, each as far out as `r`
/// says for its angle.
fn ring(n: usize, r: impl Fn(usize, f32) -> f32) -> Vec<(f32, f32)> {
    (0..n).map(|i| { let a = i as f32 / n as f32 * TAU; let k = r(i, a); (k * a.sin(), -k * a.cos()) }).collect()
}

/// Points along a run of corners, with so many lanes to each side. With a
/// lane count for every corner the run closes on itself.
fn sides(corners: &[(f32, f32)], lanes: &[usize]) -> Vec<(f32, f32)> {
    let mut pts = Vec::new();
    for (i, &n) in lanes.iter().enumerate() {
        let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
        for k in 0..n { let t = k as f32 / n as f32; pts.push((lerp(a.0, b.0, t), lerp(a.1, b.1, t))); }
    }
    if lanes.len() < corners.len() { pts.push(corners[corners.len() - 1]); }
    pts
}

impl Web {
    /// The sixteen webs, in the order they are played.
    fn new(i: u32) -> Web {
        let shut = |pts: Vec<(f32, f32)>, far: (f32, f32)| Web { lanes: pts.len(), pts, closed: true, far };
        let open = |pts: Vec<(f32, f32)>, far: (f32, f32)| Web { lanes: pts.len() - 1, pts, closed: false, far };
        match i % WEBS {
            0 => shut(ring(16, |_, _| 0.98), (0.0, 0.1)),
            1 => shut(sides(&[(-0.9, -0.9), (0.9, -0.9), (0.9, 0.9), (-0.9, 0.9)], &[4, 4, 4, 4]), (0.0, 0.1)),
            2 => shut(sides(&[(-0.36, -1.0), (0.36, -1.0), (0.36, -0.36), (1.0, -0.36), (1.0, 0.36), (0.36, 0.36), (0.36, 1.0), (-0.36, 1.0),
                (-0.36, 0.36), (-1.0, 0.36), (-1.0, -0.36), (-0.36, -0.36)], &[2, 1, 1, 2, 1, 1, 2, 1, 1, 2, 1, 1]), (0.0, 0.08)),
            3 => open(sides(&[(-1.0, -0.8), (0.0, 0.85), (1.0, -0.8)], &[8, 8]), (0.0, -0.15)),
            4 => shut(sides(&[(-1.0, -0.85), (0.0, -0.3), (1.0, -0.85), (1.0, 0.85), (0.0, 0.3), (-1.0, 0.85)], &[3, 3, 2, 3, 3, 2]), (0.0, 0.05)),
            5 => shut(ring(16, |_, a| 0.64 + 0.36 * (4.0 * a).cos()), (0.0, 0.08)),
            6 => open((0..=16).map(|i| (-1.0 + i as f32 / 8.0, 0.78)).collect(), (0.0, -0.5)),
            7 => shut(sides(&[(0.0, -1.0), (1.0, 0.8), (-1.0, 0.8)], &[5, 6, 5]), (0.0, 0.2)),
            8 => {
                // A stair of eight steps.
                let (mut pts, mut x, mut y) = (Vec::new(), -1.0f32, -0.72f32);
                for _ in 0..8 { pts.push((x, y)); x += 0.25; pts.push((x, y)); y += 0.19; }
                pts.push((x, y));
                open(pts, (0.25, -0.3))
            }
            9 => shut(ring(16, |_, a| 0.5 + 0.5 * (2.0 * a).sin().abs()), (0.0, 0.05)),
            10 => {
                // A U: two straight sides and a round bottom.
                let mut pts = vec![(-0.9, -0.85), (-0.9, -0.42)];
                pts.extend((0..=12).map(|k| { let a = TAU / 2.0 - k as f32 * TAU / 24.0; (0.9 * a.cos(), 0.85 * a.sin()) }));
                pts.extend([(0.9, -0.42), (0.9, -0.85)]);
                open(pts, (0.0, -0.35))
            }
            11 => shut((0..16).map(|i| {
                let a = i as f32 / 16.0 * TAU;
                (a.sin().powi(3), -(13.0 * a.cos() - 5.0 * (2.0 * a).cos() - 2.0 * (3.0 * a).cos() - (4.0 * a).cos()) / 16.0 * 0.95 - 0.12)
            }).collect(), (0.0, 0.1)),
            12 => open(sides(&[(-1.0, -0.05), (-0.5, 0.78), (0.0, 0.15), (0.5, 0.78), (1.0, -0.05)], &[4, 4, 4, 4]), (0.0, -0.6)),
            13 => shut(ring(16, |i, _| if i % 2 == 0 { 1.0 } else { 0.62 }), (0.0, 0.08)),
            14 => open((0..=16).map(|i| { let a = (-100.0 + i as f32 * 12.5f32).to_radians(); (0.98 * a.sin(), 0.3 - 0.98 * a.cos()) }).collect(), (0.0, 0.42)),
            _ => shut(ring(16, |_, a| 0.58 + 0.42 * a.sin().abs()), (0.0, 0.06)),
        }
    }

    /// The point of the rim `u` lanes along it.
    fn rim(&self, u: f32) -> (f32, f32) {
        let n = self.pts.len();
        let (i, f) = if self.closed {
            let u = u.rem_euclid(n as f32);
            ((u as usize).min(n - 1), u.fract())
        } else {
            let u = u.clamp(0.0, self.lanes as f32);
            let i = (u as usize).min(self.lanes - 1);
            (i, u - i as f32)
        };
        let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
        (lerp(a.0, b.0, f), lerp(a.1, b.1, f))
    }

    /// The lane so many lanes on, if the web runs that far.
    fn step(&self, lane: usize, by: i32) -> Option<usize> {
        let to = lane as i32 + by;
        if self.closed { Some(to.rem_euclid(self.lanes as i32) as usize) }
        else if (0..self.lanes as i32).contains(&to) { Some(to as usize) } else { None }
    }

    /// How many lanes from one to another and which way; the short way
    /// round a closed web.
    fn dist(&self, from: usize, to: usize) -> i32 {
        let (d, n) = (to as i32 - from as i32, self.lanes as i32);
        if !self.closed { d } else if d > n / 2 { d - n } else if d < -(n / 2) { d + n } else { d }
    }

    /// How many lane edges the web has.
    fn edges(&self) -> usize { if self.closed { self.lanes } else { self.lanes + 1 } }

    /// The lane on one side of an edge, if there is one.
    fn beside(&self, edge: usize, side: i32) -> Option<usize> {
        if side > 0 { if self.closed || edge < self.lanes { Some(edge % self.lanes) } else { None } }
        else if self.closed { Some((edge + self.lanes - 1) % self.lanes) } else { edge.checked_sub(1) }
    }

    /// The edge on the other side of that lane.
    fn across(&self, edge: usize, side: i32) -> usize {
        let to = edge as i32 + side.signum();
        if self.closed { to.rem_euclid(self.lanes as i32) as usize } else { to.clamp(0, self.lanes as i32) as usize }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Flipper, Tanker, Spiker, Fuse, Pulsar }

#[derive(Clone, Copy)]
struct Foe {
    kind: Kind,
    /// The lane it is in; for a fuseball, the edge it rides.
    lane: usize,
    /// How far up the web: 0 at the far end, 1 on the rim.
    t: f32,
    /// A flip to the next lane, or a fuseball's crossing: which way, and
    /// how far along from 0 to 1.
    flip: Option<(i32, f32)>,
    /// Seconds to its next flip.
    wait: f32,
    /// Seconds to its next shot.
    fire: f32,
    /// A spiker: up or down.
    dir: f32,
    /// A spiker and a fuseball: the depth they head for.
    goal: f32,
    /// What a tanker carries.
    cargo: Kind,
    /// Up on the rim, and after the claw.
    rim: bool,
    /// A pulsar's clock.
    pulse: f32,
}

impl Foe {
    /// Where in its pulse a pulsar is, 0 to 1: it warns from 0.55 and
    /// kills from 0.75.
    fn phase(&self) -> f32 { (self.pulse / PERIOD).fract() }

    fn live(&self) -> bool { self.kind == Kind::Pulsar && self.phase() >= 0.75 && self.t > 0.12 }

    /// The lane a shot must fly down to hit it. A fuseball on its edge
    /// cannot be hit at all.
    fn target(&self, web: &Web) -> Option<usize> {
        match (self.kind, self.flip) {
            (Kind::Fuse, Some((side, _))) => web.beside(self.lane, side),
            (Kind::Fuse, None) => None,
            _ => Some(self.lane),
        }
    }

    /// How far along the rim it is, in lanes.
    fn along(&self) -> f32 {
        match (self.kind, self.flip) {
            (Kind::Fuse, Some((side, p))) => self.lane as f32 + side as f32 * p,
            (Kind::Fuse, None) => self.lane as f32,
            _ => self.lane as f32 + 0.5,
        }
    }
}

#[derive(Clone, Copy)]
struct Shot { lane: usize, t: f32, mine: bool }

/// A burst of rays where something blew up.
struct Burst { u: f32, t: f32, age: f32, life: f32, size: f32, color: Rgb, rays: u32, spin: f32 }

/// The colours of a round of sixteen webs.
struct Colors { web: Rgb, claw: Rgb, flipper: Rgb, tanker: Rgb, spiker: Rgb, pulsar: Rgb }

static COLORS: [Colors; 5] = [
    Colors { web: 0x2848ff, claw: 0xffe020, flipper: 0xff3030, tanker: 0xb050ff, spiker: 0x30e050, pulsar: 0xffff60 },
    Colors { web: 0xff3030, claw: 0x40ff60, flipper: 0xc060ff, tanker: 0x4080ff, spiker: 0x40e0e0, pulsar: 0xffe020 },
    Colors { web: 0xffe020, claw: 0x4080ff, flipper: 0x40ff60, tanker: 0x40e0e0, spiker: 0xff3030, pulsar: 0xc060ff },
    Colors { web: 0x30e0e0, claw: 0xffe020, flipper: 0x40ff60, tanker: 0xb050ff, spiker: 0xff3030, pulsar: 0xff8030 },
    // The fifth round's webs are all but dark.
    Colors { web: 0x16161e, claw: 0xffe020, flipper: 0xff3030, tanker: 0xb050ff, spiker: 0x30e050, pulsar: 0xffff60 },
];

/// What comes up a web: more of everything, and new kinds, with the level.
fn roster(level: u32) -> Vec<Kind> {
    let l = level as i32;
    let some = |from: i32, first: i32, every: i32, most: i32| if l < from { 0 } else { (first + (l - from) / every).min(most) as usize };
    let mut all = vec![Kind::Flipper; (6 + l).min(22) as usize];
    all.extend(vec![Kind::Tanker; some(3, 1, 2, 6)]);
    all.extend(vec![Kind::Spiker; some(4, 2, 4, 5)]);
    all.extend(vec![Kind::Fuse; some(6, 1, 3, 4)]);
    all.extend(vec![Kind::Pulsar; some(8, 2, 3, 6)]);
    all
}

const LEAD: &str = "132 e5/1 g5/1 f#5/1 d#5/2 b4/2";
const ARP: &str = "132 e3/8 b3/8 e4/8 b3/8 g3/8 b3/8 e4/8 b3/8 c3/8 g3/8 c4/8 g3/8 e3/8 g3/8 c4/8 g3/8 \
    d3/8 a3/8 d4/8 a3/8 f#3/8 a3/8 d4/8 a3/8 b2/8 f#3/8 b3/8 f#3/8 d#3/8 f#3/8 b3/8 f#3/8";

struct Sounds { title: Sample, fire: Sample, boom: Sample, shot: Sample, death: Sample, zap: Sample, warp: Sample, spike: Sample, life: Sample,
    start: Sample }

impl Sounds {
    fn new() -> Sounds {
        let step = TAU / funkey::audio::RATE as f32;
        // The lead over its arpeggio, mixed into one loop so the two never drift.
        let (a, b) = (Tune::parse(LEAD, Wave::Square, 0.10).render(), Tune::parse(ARP, Wave::Triangle, 0.30).render());
        let at = |s: &Sample, i: usize| *s.data.get(i).unwrap_or(&0) as i32;
        let title = Sample::from_i16((0..a.data.len().max(b.data.len())).map(|i| (at(&a, i) + at(&b, i)).clamp(-32768, 32767) as i16).collect());
        let mut ph = 0.0f32;
        let boom = Sample::synth(0.22, |t, r| { ph += step * (320.0 - 1100.0 * t).max(40.0); (r * 0.7 + ph.sin() * 0.5) * (-t * 14.0).exp() });
        // The claw's end: a tone that falls a long way, breaking up.
        let mut ph = 0.0f32;
        let death = Sample::synth(1.3, |t, r| {
            ph += step * (60.0 + 800.0 * (1.0 - t / 1.3).powi(2));
            (ph.sin().signum() * 0.45 + r * 0.35) * (1.0 - t / 1.3)
        });
        let mut ph = 0.0f32;
        let zap = Sample::synth(0.6, |t, r| { ph += step * (2400.0 - 3600.0 * t).max(180.0); (ph.sin() * 0.5 + r * 0.5) * (1.0 - t / 0.6) });
        // The flight down: a tone that climbs.
        let mut ph = 0.0f32;
        let warp = Sample::synth(1.7, |t, _| { ph += step * (150.0 + 1500.0 * (t / 1.7).powi(2)); ph.sin() * 0.45 * (t * 8.0).min(1.0) * (1.0 - t / 1.7).sqrt() });
        Sounds { title, boom, death, zap, warp,
            fire: Sample::sweep(Wave::Square, 1500.0, 400.0, 0.07, 0.22),
            shot: Sample::sweep(Wave::Triangle, 250.0, 700.0, 0.12, 0.3),
            spike: Sample::tone(Wave::Square, 1800.0, 0.02, 0.15),
            life: Tune::parse("260 c5/8 e5/8 g5/8 c6/8 e6/4", Wave::Square, 0.3).render(),
            start: Tune::parse("240 e4/8 a4/8 e5/4", Wave::Triangle, 0.4).render() }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Arrive, Play, Dying, Warp, Over }

/// What ended a claw.
#[derive(Clone, Copy, PartialEq, Debug)]
enum How { Grab, Shot, Pulse, Fuse, Spike }

/// What the player, or the game playing itself, asks of the claw.
#[derive(Clone, Copy, Default)]
struct Ctl { dir: i32, fire: bool, zap: bool }

struct Vector {
    mode: Mode,
    /// The title: the game plays itself under its name.
    demo: bool,
    level: u32,
    /// The level a new game begins with.
    start: u32,
    web: Web,
    /// How large the web is drawn: it grows on arrival, and rushes past
    /// on the flight down.
    zoom: f32,
    lane: usize,
    /// Where the claw is drawn: it slides to its lane.
    pos: f32,
    /// How deep the claw is: 1 on the rim.
    depth: f32,
    steer: i32, repeat: f32, cool: f32,
    /// How often the superzapper has been used on this web.
    zaps: u8,
    zapping: f32, zap_tick: f32,
    foes: Vec<Foe>,
    /// What has yet to come up.
    pool: Vec<Kind>,
    spawn: f32,
    /// The last climbers are gone: spikers and fuseballs go back down.
    retreat: bool,
    shots: Vec<Shot>,
    /// How high the spike in each lane stands.
    spikes: Vec<f32>,
    fx: Vec<Burst>,
    how: How,
    /// The claw died on the flight down.
    warped: bool,
    score: u32, high: u32, lives: u32, extra: u32,
    time: f32, timer: f32,
    /// Seconds since the frame was last drawn, for the fading.
    since: f32,
    flash: f32,
    glow: Glow,
    rng: Rng,
    audio: Audio,
    snd: Sounds,
}

impl Vector {
    fn new() -> Vector {
        let mut game = Vector { mode: Mode::Arrive, demo: true, level: 1, start: 1, web: Web::new(0), zoom: 1.0, lane: 0, pos: 0.5,
            depth: 1.0, steer: 0, repeat: 0.0, cool: 0.0, zaps: 0, zapping: 0.0, zap_tick: 0.0, foes: Vec::new(), pool: Vec::new(), spawn: 0.0,
            retreat: false, shots: Vec::new(), spikes: Vec::new(), fx: Vec::new(), how: How::Shot, warped: false, score: 0,
            high: funkey::scores::best(GAME), lives: 3, extra: EXTRA, time: 0.0, timer: 0.0, since: 0.0, flash: 0.0, glow: Glow::new(),
            rng: Rng::from_time(), audio: Audio::off(), snd: Sounds::new() };
        game.enter(1);
        game
    }

    /// Onto a web: it comes up out of the dark, with all its foes below.
    fn enter(&mut self, level: u32) {
        self.level = level.max(1);
        self.web = Web::new(self.level - 1);
        self.spikes = vec![0.0; self.web.lanes];
        self.pool = roster(self.level);
        // Shuffled, so no two games bring them up in the same order.
        for i in (1..self.pool.len()).rev() { let j = self.rng.below(i as u32 + 1) as usize; self.pool.swap(i, j); }
        self.foes.clear();
        self.shots.clear();
        self.lane = self.web.lanes / 2;
        self.pos = self.lane as f32 + 0.5;
        (self.depth, self.zoom, self.zaps, self.zapping, self.retreat, self.warped) = (1.0, 0.02, 0, 0.0, false, false);
        (self.mode, self.timer, self.spawn) = (Mode::Arrive, 0.0, 0.7);
    }

    /// A new game.
    fn begin(&mut self) {
        (self.demo, self.score, self.lives, self.extra) = (false, 0, 3, EXTRA);
        self.fx.clear();
        self.enter(self.start);
        self.audio.stop(1);
        self.audio.play(&self.snd.start, 0.7);
    }

    /// Back to the title, the game playing itself on some web.
    fn title(&mut self) {
        self.demo = true;
        let level = 1 + self.rng.below(12);
        self.enter(level);
    }

    /// Where on the screen a point of the web lies: `u` lanes along the
    /// rim, `t` of the way up from the far end.
    fn at(&self, u: f32, t: f32) -> (f32, f32) { self.lay(self.web.rim(u), t) }

    /// The middle of the web at a depth.
    fn hub(&self, t: f32) -> (f32, f32) { self.lay((0.0, 0.0), t) }

    fn lay(&self, p: (f32, f32), t: f32) -> (f32, f32) {
        // Each step up the web makes it larger by the same share: slow far
        // down, fast near the rim, and still seen to move all the way.
        let s = FAR.powf(1.0 - t.min(1.1));
        let k = (s - FAR) / (1.0 - FAR);
        let far = self.web.far;
        let (x, y) = (far.0 * (1.0 - k) + p.0 * s, far.1 * (1.0 - k) + p.1 * s);
        (CX + (far.0 + (x - far.0) * self.zoom) * RX, CY + (far.1 + (y - far.1) * self.zoom) * RY)
    }

    fn tint(&self, kind: Kind) -> Rgb {
        let c = &COLORS[((self.level - 1) / WEBS % 5) as usize];
        match kind {
            Kind::Flipper => c.flipper, Kind::Tanker => c.tanker, Kind::Spiker => c.spiker, Kind::Pulsar => c.pulsar,
            Kind::Fuse => [0xff3030, 0xffe020, 0x40ff60, 0x40e0e0, 0xc060ff][(self.time * 12.0) as usize % 5],
        }
    }

    fn colors(&self) -> &'static Colors { &COLORS[((self.level - 1) / WEBS % 5) as usize] }

    /// A foe as it comes up, or out of a tanker.
    fn hatch(&mut self, kind: Kind, lane: usize, t: f32) -> Foe {
        let l = self.level as f32;
        let mut f = Foe { kind, lane, t, flip: None, wait: 0.0, fire: self.rng.range(1.5, 4.5) / (1.0 + 0.06 * l), dir: 1.0, goal: 0.0,
            cargo: Kind::Flipper, rim: false, pulse: self.rng.range(0.0, PERIOD) };
        match kind {
            Kind::Flipper | Kind::Pulsar => f.wait = self.rng.range(1.0, 2.4) / (1.0 + 0.08 * l) * if self.level < 2 { 2.0 } else { 1.0 },
            Kind::Tanker => if self.level >= 14 && self.rng.chance(0.25) { f.cargo = Kind::Fuse } else if self.level >= 20 && self.rng.chance(0.3) { f.cargo = Kind::Pulsar },
            Kind::Spiker => f.goal = self.rng.range(0.45, 0.85),
            Kind::Fuse => f.goal = self.rng.range(0.3, 1.0),
        }
        f
    }

    fn come_up(&mut self, dt: f32) {
        if self.pool.is_empty() { return; }
        self.spawn -= dt;
        if self.spawn > 0.0 || self.foes.len() >= (4 + self.level as usize / 2).min(12) { return; }
        self.spawn = (1.5 - 0.05 * self.level as f32).max(0.45) * self.rng.range(0.7, 1.3);
        let Some(kind) = self.pool.pop() else { return };
        let lane = self.rng.below(if kind == Kind::Fuse { self.web.edges() } else { self.web.lanes } as u32) as usize;
        let f = self.hatch(kind, lane, 0.0);
        self.foes.push(f);
    }

    /// The claw moves a lane at a press, and runs while the key is held.
    fn drive(&mut self, ctl: Ctl, dt: f32) {
        if ctl.dir != 0 {
            let mut steps = 0;
            if ctl.dir != self.steer { steps = 1; self.repeat = 0.15; } else {
                self.repeat -= dt;
                while self.repeat <= 0.0 { steps += 1; self.repeat += 0.045; }
            }
            for _ in 0..steps { if let Some(l) = self.web.step(self.lane, ctl.dir) { self.lane = l; } }
        }
        self.steer = ctl.dir;
        let n = self.web.lanes as f32;
        let mut d = self.lane as f32 + 0.5 - self.pos;
        if self.web.closed { d = (d + n / 2.0).rem_euclid(n) - n / 2.0; }
        self.pos += d * (dt * 28.0).min(1.0);
        if self.web.closed { self.pos = self.pos.rem_euclid(n); }
    }

    fn fire(&mut self, ctl: Ctl, dt: f32) {
        self.cool = (self.cool - dt).max(0.0);
        if ctl.fire && self.cool == 0.0 && self.shots.iter().filter(|s| s.mine).count() < SHOTS {
            self.cool = 0.055;
            self.shots.push(Shot { lane: self.lane, t: self.depth, mine: true });
            if !self.demo { self.audio.play_on(2, &self.snd.fire, 0.5); }
        }
    }

    /// The foe highest up the web.
    fn highest(&self) -> Option<usize> {
        (0..self.foes.len()).max_by(|&a, &b| self.foes[a].t.total_cmp(&self.foes[b].t))
    }

    /// The superzapper: all on the web the first time, one the second.
    fn zap(&mut self, ctl: Ctl, dt: f32) {
        if ctl.zap && self.zaps < 2 {
            self.zaps += 1;
            self.flash = 1.0;
            if self.zaps == 1 { self.zapping = 0.05 * (self.foes.len() as f32 + 4.0); self.zap_tick = 0.0; }
            else if let Some(j) = self.highest() { self.kill(j); }
            if !self.demo { self.audio.play_on(3, &self.snd.zap, 0.7); }
        }
        if self.zapping > 0.0 {
            self.zapping -= dt;
            self.zap_tick -= dt;
            if self.zap_tick <= 0.0 {
                self.zap_tick = 0.05;
                if let Some(j) = self.highest() { self.kill(j); }
            }
        }
    }

    fn burst(&mut self, u: f32, t: f32, color: Rgb, size: f32, rays: u32) {
        if self.fx.len() > 80 { self.fx.remove(0); }
        let spin = self.rng.range(0.0, TAU);
        self.fx.push(Burst { u, t, age: 0.0, life: 0.35 + size * 0.12, size, color, rays, spin });
    }

    fn add(&mut self, points: u32) {
        self.score += points;
        if self.demo { return; }
        self.high = self.high.max(self.score);
        if self.score >= self.extra {
            self.extra += EXTRA;
            self.lives = (self.lives + 1).min(6);
            self.audio.play(&self.snd.life, 0.7);
        }
    }

    /// A foe is destroyed. A tanker lets out the two it carried.
    fn kill(&mut self, j: usize) {
        let f = self.foes.swap_remove(j);
        let color = self.tint(f.kind);
        self.burst(f.along(), f.t, color, 1.0, 10);
        self.add(match f.kind {
            Kind::Flipper => 150, Kind::Tanker => 100, Kind::Spiker => 50, Kind::Pulsar => 200,
            Kind::Fuse => if f.t > 0.66 { 750 } else if f.t > 0.33 { 500 } else { 250 },
        });
        if f.kind == Kind::Tanker { self.split(&f); }
        if !self.demo { self.audio.play(&self.snd.boom, 0.6); }
    }

    fn split(&mut self, f: &Foe) {
        for side in [-1, 1] {
            let lane = if f.cargo == Kind::Fuse { if side < 0 { f.lane } else { self.web.across(f.lane, 1) } }
                else { self.web.step(f.lane, side).unwrap_or(f.lane) };
            let mut young = self.hatch(f.cargo, lane, f.t.min(0.97));
            young.wait = 0.4;
            self.foes.push(young);
        }
    }

    /// What every foe does in a frame.
    fn foes_move(&mut self, dt: f32) {
        let l = self.level as f32;
        let climb = (0.15 + 0.012 * l).min(0.45);
        let rim_flip = (0.34 - 0.012 * l).max(0.16);
        let cap = (1 + self.level as usize / 3).min(4);
        let mut flying = self.shots.iter().filter(|s| !s.mine).count();
        let me = self.lane;
        let (mut burst, mut fired): (Vec<Foe>, bool) = (Vec::new(), false);
        {
            let Vector { foes, web, rng, spikes, shots, retreat, .. } = self;
            let mut i = 0;
            while i < foes.len() {
                let f = &mut foes[i];
                let mut gone = false;
                match f.kind {
                    Kind::Flipper | Kind::Pulsar => {
                        f.pulse += dt;
                        if !f.rim {
                            f.t += climb * if f.kind == Kind::Pulsar { 0.8 } else { 1.0 } * dt;
                            if f.t >= 1.0 { f.t = 1.0; f.rim = true; f.wait = 0.2; }
                        }
                        if let Some((dir, p)) = f.flip {
                            // Half way over, it is in the next lane.
                            let np = p + dt / if f.rim { rim_flip } else { 0.26 };
                            if p < 0.5 && np >= 0.5 { if let Some(to) = web.step(f.lane, dir) { f.lane = to; } }
                            if np >= 1.0 {
                                f.flip = None;
                                f.wait = if f.rim { 0.1 } else { rng.range(1.0, 2.4) / (1.0 + 0.08 * l) * if l < 2.0 { 2.0 } else { 1.0 } };
                            } else { f.flip = Some((dir, np)); }
                        } else {
                            f.wait -= dt;
                            if f.wait <= 0.0 {
                                // On the rim it comes for the claw; below, it mostly does.
                                let toward = web.dist(f.lane, me).signum();
                                let dir = if f.rim || (toward != 0 && rng.chance(0.6)) { toward } else if rng.chance(0.5) { 1 } else { -1 };
                                let dir = if web.step(f.lane, dir).is_some() { dir } else { -dir };
                                if dir != 0 && f.t > 0.1 && web.step(f.lane, dir).is_some() { f.flip = Some((dir, 0.0)); } else { f.wait = 0.1; }
                            }
                        }
                    }
                    Kind::Tanker => {
                        f.t += climb * 0.7 * dt;
                        if f.t >= 0.93 { burst.push(*f); gone = true; }
                    }
                    Kind::Spiker => {
                        if *retreat { f.dir = -1.0; }
                        f.t += f.dir * 0.42 * dt;
                        if f.dir > 0.0 {
                            spikes[f.lane] = spikes[f.lane].max(f.t);
                            if f.t >= f.goal { f.dir = -1.0; }
                        } else if f.t <= 0.0 {
                            // At the bottom it picks another lane, or leaves.
                            if *retreat { gone = true; } else {
                                (f.t, f.dir, f.goal) = (0.0, 1.0, rng.range(0.45, 0.88));
                                f.lane = rng.below(web.lanes as u32) as usize;
                            }
                        }
                    }
                    Kind::Fuse => {
                        if *retreat && f.flip.is_none() { f.goal = -0.1; }
                        if let Some((side, p)) = f.flip {
                            let np = p + dt / 0.32;
                            if np >= 1.0 {
                                f.lane = web.across(f.lane, side);
                                f.flip = None;
                                f.wait = 0.0;
                                // From the rim it goes back down; from below it may come up.
                                f.goal = if f.t > 0.9 { rng.range(0.2, 0.7) } else if rng.chance(0.35 + 0.02 * l) { 1.0 } else { rng.range(0.25, 0.95) };
                            } else { f.flip = Some((side, np)); }
                        } else if (f.t - f.goal).abs() > 0.01 {
                            f.t += (f.goal - f.t).clamp(-0.5 * dt, 0.5 * dt);
                            if f.t <= 0.0 { gone = true; }
                        } else {
                            // Where it wanted to be: a moment's wait, then across a lane.
                            f.wait += dt;
                            if f.wait > if f.t > 0.95 { 0.45 } else { 0.12 } {
                                let side = if rng.chance(0.5) { 1 } else { -1 };
                                let side = if web.beside(f.lane, side).is_some() { side } else { -side };
                                (f.flip, f.wait) = (Some((side, 0.0)), 0.0);
                            }
                        }
                    }
                }
                // Those that climb also shoot up their lane.
                if !gone && !f.rim && f.flip.is_none() && matches!(f.kind, Kind::Flipper | Kind::Tanker | Kind::Pulsar) {
                    f.fire -= dt;
                    if f.fire <= 0.0 {
                        if f.t > 0.12 && f.t < 0.8 && flying < cap {
                            shots.push(Shot { lane: f.lane, t: f.t, mine: false });
                            flying += 1;
                            fired = true;
                            f.fire = rng.range(1.8, 4.5) / (1.0 + 0.06 * l);
                        } else { f.fire = 0.3; }
                    }
                }
                if gone { foes.swap_remove(i); } else { i += 1; }
            }
        }
        for f in &burst {
            let color = self.tint(Kind::Tanker);
            self.burst(f.along(), f.t, color, 0.8, 8);
            self.split(f);
        }
        if fired && !self.demo { self.audio.play_on(4, &self.snd.shot, 0.4); }
    }

    fn shots_move(&mut self, dt: f32) {
        let speed = (0.55 + 0.02 * self.level as f32).min(0.95);
        let mut i = 0;
        while i < self.shots.len() {
            let s = self.shots[i];
            if !s.mine {
                let to = s.t + speed * dt;
                if to >= 1.0 {
                    self.shots.swap_remove(i);
                    if self.mode == Mode::Play && s.lane == self.lane { self.die(How::Shot); }
                    continue;
                }
                self.shots[i].t = to;
                i += 1;
                continue;
            }
            let (from, to) = (s.t, s.t - SHOT * dt);
            // One of theirs coming up the lane: the two cancel.
            if let Some(j) = self.shots.iter().position(|o| !o.mine && o.lane == s.lane && o.t <= from + 0.02 && o.t >= to - 0.03) {
                let t = self.shots[j].t;
                self.shots.swap_remove(i.max(j));
                self.shots.swap_remove(i.min(j));
                self.burst(s.lane as f32 + 0.5, t, 0xffffff, 0.4, 5);
                continue;
            }
            if let Some(j) = self.foes.iter().position(|f| f.target(&self.web) == Some(s.lane) && f.t <= from + 0.02 && f.t >= to - 0.02) {
                self.shots.swap_remove(i);
                self.kill(j);
                continue;
            }
            // The tip of a spike: each shot takes a piece off it.
            let spike = self.spikes[s.lane];
            if spike > 0.0 && to <= spike {
                self.spikes[s.lane] = if spike < 0.12 { 0.0 } else { spike - 0.09 };
                self.shots.swap_remove(i);
                self.add(2);
                let color = self.tint(Kind::Spiker);
                self.burst(s.lane as f32 + 0.5, spike, color, 0.3, 4);
                if !self.demo { self.audio.play_on(5, &self.snd.spike, 0.5); }
                continue;
            }
            if to <= 0.0 { self.shots.swap_remove(i); continue; }
            self.shots[i].t = to;
            i += 1;
        }
    }

    /// The claw is lost.
    fn die(&mut self, how: How) {
        if self.mode == Mode::Dying { return; }
        self.warped = self.mode == Mode::Warp;
        (self.mode, self.timer, self.how, self.zapping) = (Mode::Dying, 0.0, how, 0.0);
        if how != How::Grab {
            let (u, t, claw) = (self.pos, self.depth, self.colors().claw);
            self.burst(u, t, 0xffffff, 2.2, 16);
            self.burst(u, t, claw, 3.2, 12);
            self.burst(u, t, 0xff4030, 4.2, 9);
        }
        if !self.demo { self.audio.play_on(3, &self.snd.death, 0.8); }
    }

    /// What on the rim kills the claw by touching it.
    fn touch(&mut self) {
        let (me, web) = (self.lane, &self.web);
        let how = self.foes.iter().find_map(|f| match f.kind {
            Kind::Flipper | Kind::Pulsar if f.rim && f.flip.is_none() && f.lane == me => Some(How::Grab),
            Kind::Pulsar if f.live() && f.lane == me => Some(How::Pulse),
            Kind::Fuse if f.t >= 0.97 && (web.beside(f.lane, 1) == Some(me) || web.beside(f.lane, -1) == Some(me)) => Some(How::Fuse),
            _ => None,
        });
        if let Some(how) = how { self.die(how); }
    }

    fn play(&mut self, ctl: Ctl, dt: f32) {
        self.drive(ctl, dt);
        self.fire(ctl, dt);
        self.zap(ctl, dt);
        self.come_up(dt);
        self.foes_move(dt);
        self.shots_move(dt);
        if self.mode == Mode::Play { self.touch(); }
        if self.mode != Mode::Play || !self.pool.is_empty() || self.zapping > 0.0 { return; }
        // All have come up. When none is left climbing, the rest go back
        // down, and the claw flies on.
        if !self.foes.iter().any(|f| !f.rim && matches!(f.kind, Kind::Flipper | Kind::Tanker | Kind::Pulsar)) { self.retreat = true; }
        if self.foes.iter().all(|f| f.rim) && self.shots.iter().all(|s| s.mine) {
            (self.mode, self.timer) = (Mode::Warp, 0.0);
            self.foes.clear();
            if !self.demo { self.audio.play_on(3, &self.snd.warp, 0.7); }
        }
    }

    /// The flight down the web to the next one. A spike in the claw's
    /// lane ends it.
    fn warp(&mut self, ctl: Ctl, dt: f32) {
        self.timer += dt;
        self.drive(ctl, dt);
        self.fire(ctl, dt);
        self.shots_move(dt);
        self.depth -= (0.12 + self.timer * 0.55) * dt;
        self.zoom = 1.0 + (1.0 - self.depth).max(0.0).powi(2) * 7.0;
        let spike = self.spikes[self.lane];
        if spike > 0.0 && self.depth <= spike { self.die(How::Spike); return; }
        if self.depth <= 0.0 { self.enter(self.level + 1); }
    }

    fn dying(&mut self, dt: f32) {
        self.timer += dt;
        if self.how == How::Grab {
            // Dragged down the lane by what caught it.
            self.depth = (self.depth - 0.7 * dt).max(0.05);
            let (me, d) = (self.lane, self.depth);
            if let Some(f) = self.foes.iter_mut().find(|f| f.rim && f.lane == me) { f.t = d; }
        }
        if self.timer < 1.5 { return; }
        if self.demo { return self.title(); }
        self.lives -= 1;
        if self.lives == 0 {
            (self.mode, self.timer) = (Mode::Over, 0.0);
            if !cfg!(test) { funkey::scores::record(GAME, self.score); }
        } else if self.warped {
            self.enter(self.level + 1);
        } else {
            // The web is swept clean: what was on it comes up again.
            let back: Vec<Kind> = self.foes.drain(..).map(|f| f.kind).collect();
            self.pool.extend(back);
            self.shots.clear();
            (self.mode, self.depth, self.spawn, self.retreat) = (Mode::Play, 1.0, 1.0, false);
        }
    }

    /// The game playing itself: go for what is highest up the web, keep
    /// out of the lanes that kill, fire all the time. Like a player, it
    /// does not see what is still small and far down.
    fn bot(&self) -> Ctl {
        let (web, me, n) = (&self.web, self.lane, self.web.lanes);
        let mut bad = vec![false; n];
        if self.mode == Mode::Warp { for (l, s) in self.spikes.iter().enumerate() { bad[l] = *s > 0.0; } }
        for f in &self.foes {
            match f.kind {
                Kind::Pulsar if f.phase() > 0.5 && f.t > 0.1 => bad[f.lane] = true,
                // A fuseball near the rim: the lanes beside its edge, and
                // beside the edge it is crossing to.
                Kind::Fuse if f.t > 0.75 => for edge in [Some(f.lane), f.flip.map(|(side, _)| web.across(f.lane, side))].into_iter().flatten() {
                    for side in [-1, 1] { if let Some(l) = web.beside(edge, side) { bad[l] = true; } }
                },
                _ => {}
            }
        }
        let (mut goal, mut best) = (me, f32::MIN);
        for f in &self.foes {
            let Some(l) = f.target(web) else { continue };
            if bad[l] || f.rim || f.t < 0.35 { continue; }
            let score = f.t - 0.04 * web.dist(me, l).abs() as f32 - if f.kind == Kind::Spiker { 0.6 } else { 0.0 };
            if score > best { (best, goal) = (score, l); }
        }
        // Nothing to shoot: clear the nearest spike.
        if best == f32::MIN && self.mode == Mode::Play {
            if let Some(l) = (0..n).filter(|&l| self.spikes[l] > 0.0 && !bad[l]).min_by_key(|&l| web.dist(me, l).abs()) { goal = l; }
        }
        let mut dir = web.dist(me, goal).signum();
        if bad[me] {
            // Out of a lane that kills, the short way.
            let out = (1..n as i32).flat_map(|d| [d, -d]).find(|&d| web.step(me, d).is_some_and(|l| !bad[l]));
            dir = out.map(|d| d.signum()).unwrap_or(dir);
        } else if dir != 0 && web.step(me, dir).is_some_and(|l| bad[l]) { dir = 0; }
        let zap = self.mode == Mode::Play && self.zaps == 0 && self.foes.iter().filter(|f| f.rim).count() >= 2;
        Ctl { dir, fire: true, zap }
    }
}

impl Vector {
    /// The two ends of a foe that lies across its lane. One that flips
    /// swings about the edge between its lane and the next, over the
    /// inside of the web.
    fn span(&self, f: &Foe) -> ((f32, f32), (f32, f32)) {
        let Some((dir, p)) = f.flip else { return (self.at(f.lane as f32, f.t), self.at(f.lane as f32 + 1.0, f.t)) };
        let from = if p < 0.5 { f.lane as f32 } else { f.lane as f32 - dir as f32 };
        let (pivot, a, b) = if dir > 0 { (from + 1.0, from, from + 2.0) } else { (from, from + 1.0, from - 1.0) };
        let (pv, a, b) = (self.at(pivot, f.t), self.at(a, f.t), self.at(b, f.t));
        let (a0, a1) = ((a.1 - pv.1).atan2(a.0 - pv.0), (b.1 - pv.1).atan2(b.0 - pv.0));
        let mut d = turn(a1 - a0);
        let (hub, mid) = (self.hub(f.t), a0 + d / 2.0);
        if mid.cos() * (hub.0 - pv.0) + mid.sin() * (hub.1 - pv.1) < 0.0 { d -= d.signum() * TAU; }
        let (ang, len) = (a0 + d * p, lerp(far(a, pv), far(b, pv), p));
        (pv, (pv.0 + ang.cos() * len, pv.1 + ang.sin() * len))
    }

    fn scene(&self, g: &mut Glow) {
        let c = self.colors();
        let n = self.web.lanes;
        let web = shade(c.web, 0xffffff, self.flash);
        let alive = self.mode != Mode::Dying && self.mode != Mode::Over;
        // The web: a rail up each edge, the rim and the far end.
        for e in 0..self.web.edges() {
            let mine = alive && (self.web.beside(e, 1) == Some(self.lane) || self.web.beside(e, -1) == Some(self.lane));
            g.line(self.at(e as f32, 0.0), self.at(e as f32, 1.0), if mine { c.claw } else { web }, if mine { 0.85 } else { 0.5 });
        }
        for l in 0..n {
            g.line(self.at(l as f32, 1.0), self.at(l as f32 + 1.0, 1.0), web, 0.8);
            g.line(self.at(l as f32, 0.0), self.at(l as f32 + 1.0, 0.0), web, 0.4);
        }
        // What waits below: dots circling under the far end.
        for (i, &kind) in self.pool.iter().enumerate().take(28) {
            let p = self.at(i as f32 * 2.4 + self.time * 0.7, -0.3 - (i % 5) as f32 * 0.14);
            g.line(p, p, self.tint(kind), 0.9);
        }
        for (l, &h) in self.spikes.iter().enumerate() {
            if h <= 0.0 { continue; }
            let (a, b) = (self.at(l as f32 + 0.5, 0.0), self.at(l as f32 + 0.5, h));
            g.line(a, b, c.spiker, 0.75);
            g.line(b, b, 0xffffff, 1.0);
        }
        for f in &self.foes {
            let color = self.tint(f.kind);
            let k = 0.6 + 0.6 * f.t.clamp(0.0, 1.0);
            match f.kind {
                Kind::Flipper => {
                    // A bow: two triangles that meet in the middle.
                    let (p, q) = self.span(f);
                    let thick = (11.0 / far(p, q).max(1.0)).min(0.24);
                    let (nx, ny) = (-(q.1 - p.1) * thick, (q.0 - p.0) * thick);
                    g.path(&[(p.0 + nx, p.1 + ny), (q.0 - nx, q.1 - ny), (q.0 + nx, q.1 + ny), (p.0 - nx, p.1 - ny), (p.0 + nx, p.1 + ny)], color, k);
                }
                Kind::Tanker => {
                    // A diamond, with what it carries inside.
                    let (p, q) = (self.at(f.lane as f32, f.t), self.at(f.lane as f32 + 1.0, f.t));
                    let half = (16.0 / far(p, q).max(1.0)).min(0.42);
                    let (m, hx, hy) = (((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0), (q.0 - p.0) * half, (q.1 - p.1) * half);
                    let (nx, ny) = (-hy * 0.75, hx * 0.75);
                    for (s, col) in [(1.0, color), (0.45, self.tint(f.cargo))] {
                        g.path(&[(m.0 - hx * s, m.1 - hy * s), (m.0 + nx * s, m.1 + ny * s), (m.0 + hx * s, m.1 + hy * s), (m.0 - nx * s, m.1 - ny * s),
                            (m.0 - hx * s, m.1 - hy * s)], col, k);
                    }
                }
                Kind::Spiker => {
                    // A coil that turns.
                    let (p, q, m) = (self.at(f.lane as f32, f.t), self.at(f.lane as f32 + 1.0, f.t), self.at(f.lane as f32 + 0.5, f.t));
                    let r = (far(p, q) * 0.36).min(12.0);
                    let pts: Vec<(f32, f32)> = (0..=12).map(|i| {
                        let (a, d) = (self.time * 7.0 + i as f32 * 0.95, r * i as f32 / 12.0);
                        (m.0 + a.cos() * d, m.1 + a.sin() * d)
                    }).collect();
                    g.path(&pts, color, k);
                }
                Kind::Fuse => {
                    // Sparks that jump about a middle, a new colour each.
                    let m = self.at(f.along(), f.t);
                    let r = (far(self.at(1.0, f.t), self.at(2.0, f.t)) * 0.42).min(13.0);
                    let tick = (self.time * 20.0) as u32;
                    for i in 0..5u32 {
                        let a = i as f32 * TAU / 5.0 + hash(tick * 7 + i) * 1.2;
                        let (bend, reach) = (a + (hash(tick * 13 + i) - 0.5) * 1.6, r * (0.7 + 0.5 * hash(tick * 17 + i)));
                        let elbow = (m.0 + bend.cos() * reach * 0.5, m.1 + bend.sin() * reach * 0.5);
                        let col = [0xff3030, 0xffe020, 0x40ff60, 0x40e0e0, 0xc060ff][((tick + i) % 5) as usize];
                        g.path(&[m, elbow, (m.0 + a.cos() * reach, m.1 + a.sin() * reach)], col, k);
                    }
                }
                Kind::Pulsar => {
                    // A line across the lane that folds into a zigzag as it charges.
                    let (p, q) = self.span(f);
                    let ph = f.phase();
                    let fold = if ph >= 0.75 { 0.34 } else { 0.05 + 0.25 * (ph / 0.75).powi(2) } * (30.0 / far(p, q).max(1.0)).min(1.0);
                    let (nx, ny) = (-(q.1 - p.1) * fold, (q.0 - p.0) * fold);
                    let pts: Vec<(f32, f32)> = (0..=6).map(|i| {
                        let (t, s) = (i as f32 / 6.0, if i == 0 || i == 6 { 0.0 } else if i % 2 == 1 { 1.0 } else { -1.0 });
                        (lerp(p.0, q.0, t) + nx * s, lerp(p.1, q.1, t) + ny * s)
                    }).collect();
                    g.path(&pts, if f.live() { 0xffffff } else { color }, if f.live() { 1.5 } else { k });
                    // Its lane lights up before it kills, and blazes while it does.
                    if ph > 0.55 && f.t > 0.12 && f.flip.is_none() && (f.live() || (self.time * 16.0) as i32 % 2 == 0) {
                        let (col, lit) = if f.live() { (0xffffff, 1.3) } else { (color, 0.6) };
                        for e in [f.lane as f32, f.lane as f32 + 1.0] { g.line(self.at(e, 0.0), self.at(e, 1.0), col, lit); }
                    }
                }
            }
        }
        for s in &self.shots {
            let u = s.lane as f32 + 0.5;
            if s.mine {
                g.line(self.at(u, s.t), self.at(u, (s.t + 0.05).min(1.0)), 0xffffa0, 1.3);
            } else {
                // Theirs: a small cross that spins.
                let (m, r) = (self.at(u, s.t), 2.0 + 3.0 * s.t);
                for a in [self.time * 14.0, self.time * 14.0 + TAU / 4.0] {
                    g.line((m.0 - a.cos() * r, m.1 - a.sin() * r), (m.0 + a.cos() * r, m.1 + a.sin() * r), if (self.time * 20.0) as i32 % 2 == 0 { 0xffffff } else { 0xff5040 }, 1.1);
                }
            }
        }
        if alive || (self.mode == Mode::Dying && self.how == How::Grab) { self.claw(g, self.pos, self.depth, c.claw, 1.25); }
        for b in &self.fx {
            let (m, t) = (self.at(b.u, b.t), b.age / b.life);
            let r = b.size * (5.0 + 22.0 * t) * (0.4 + 0.6 * b.t.clamp(0.0, 1.0));
            for i in 0..b.rays {
                let a = b.spin + i as f32 * TAU / b.rays as f32;
                let (r0, r1) = (r * (0.35 + 0.3 * hash(i + b.rays)), r * (0.8 + 0.4 * hash(i * 3 + 1)));
                g.line((m.0 + a.cos() * r0, m.1 + a.sin() * r0), (m.0 + a.cos() * r1, m.1 + a.sin() * r1), b.color, 1.4 * (1.0 - t));
            }
        }
        // On the flight down the dark rushes past.
        if self.mode == Mode::Warp {
            let hub = self.hub(0.0);
            for i in 0..40u32 {
                let (a, d) = (hash(i) * TAU, (self.timer * (0.6 + hash(i + 99)) + hash(i + 50)).fract());
                let (r0, r1) = (d * d * 420.0, d * d * 420.0 * 1.08 + 2.0);
                g.line((hub.0 + a.cos() * r0, hub.1 + a.sin() * r0), (hub.0 + a.cos() * r1, hub.1 + a.sin() * r1), 0xc0c8ff, d);
            }
        }
    }

    /// The claw, astride its lane on the rim: its back away from the web,
    /// two fingers into the lane.
    fn claw(&self, g: &mut Glow, pos: f32, depth: f32, color: Rgb, k: f32) {
        let (a, b, hub) = (self.at(pos - 0.5, depth), self.at(pos + 0.5, depth), self.hub(depth));
        let (m, wide) = (((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), far(a, b).max(0.001));
        // Square to the lane, on the side away from the middle of the web.
        let (mut nx, mut ny) = (-(b.1 - a.1) / wide, (b.0 - a.0) / wide);
        if nx * (m.0 - hub.0) + ny * (m.1 - hub.1) < 0.0 { (nx, ny) = (-nx, -ny); }
        let r = (wide * 0.28).clamp(5.0, 18.0);
        // A point so far along the lane and so far out from it.
        let at = |s: f32, h: f32| (lerp(a.0, b.0, s) + nx * h * r, lerp(a.1, b.1, s) + ny * h * r);
        g.path(&[a, at(0.1, 1.0), at(0.5, 0.42), at(0.9, 1.0), b], color, k);
        g.line(a, at(0.24, -0.6), color, k);
        g.line(b, at(0.76, -0.6), color, k);
    }

    fn hud(&self, g: &mut Glow) {
        let c = self.colors();
        g.text(14.0, 10.0, &format!("{:06}", self.score), 16.0, 0x40ff60, 1.0);
        g.centered(CX, 10.0, &format!("HIGH {:06}", self.high.max(self.score)), 10.0, 0x6080ff, 0.8);
        let level = format!("LEVEL {}", self.level);
        g.text(W as f32 - 14.0 - Glow::width(&level, 12.0), 10.0, &level, 12.0, 0x40e0e0, 0.9);
        if self.zaps < 2 { g.text(W as f32 - 14.0 - Glow::width("ZAP", 10.0), 30.0, "ZAP", 10.0, 0xffe020, if self.zaps == 0 { 1.0 } else { 0.35 }); }
        // The claws in reserve.
        for i in 0..self.lives.saturating_sub(1) {
            let x = 18.0 + i as f32 * 20.0;
            g.path(&[(x - 6.0, 44.0), (x - 8.0, 36.0), (x, 40.0), (x + 8.0, 36.0), (x + 6.0, 44.0)], c.claw, 0.9);
        }
        let blink = (self.time * 4.0) as i32 % 2 == 0;
        match self.mode {
            Mode::Arrive => g.centered(CX, 52.0, &format!("LEVEL {}", self.level), 20.0, 0x40e0e0, 1.0),
            Mode::Warp if self.timer < 1.2 && blink && self.spikes.iter().any(|&s| s > 0.0) => g.centered(CX, 52.0, "AVOID SPIKES", 16.0, 0xff4030, 1.1),
            Mode::Over => {
                g.centered(CX, 150.0, "GAME OVER", 40.0, 0xff3030, 1.2);
                g.centered(CX, 220.0, &format!("SCORE {:06}", self.score), 18.0, 0xffe020, 1.0);
            }
            _ => {}
        }
    }

    /// The name over the game playing itself.
    fn name(&self, g: &mut Glow) {
        let x = CX - Glow::width("VECTOR", 64.0) / 2.0;
        for (i, ch) in "VECTOR".chars().enumerate() {
            let col = [0xff3030, 0xff8030, 0xffe020, 0x40ff60, 0x40e0e0, 0x6070ff][i];
            // Each letter twice, a step apart: a line with weight.
            for d in [0.0, 1.5] { g.text(x + i as f32 * 64.0 + d, 50.0 + d, &ch.to_string(), 64.0, col, 1.15); }
        }
        g.centered(CX, 130.0, "A TRIBUTE TO TEMPEST", 12.0, 0xc0c8ff, 0.9);
        if (self.time * 2.0) as i32 % 2 == 0 { g.centered(CX, 190.0, "SPACE TO PLAY", 20.0, 0xffffff, 1.1); }
        g.centered(CX, 236.0, &format!("< START AT LEVEL {} >", self.start), 11.0, 0x40e0e0, 0.9);
        g.centered(CX, 266.0, &format!("HIGH {:06}", self.high), 12.0, 0xffe020, 0.9);
        g.centered(CX, 346.0, "LEFT RIGHT MOVE   SPACE FIRE   Z SUPERZAPPER", 9.0, 0x8090c0, 0.8);
        g.centered(CX, 364.0, "P PAUSE   Q QUIT", 9.0, 0x8090c0, 0.8);
        g.text(W as f32 - 8.0 - Glow::width(VERSION, 8.0), H as f32 - 14.0, VERSION, 8.0, 0x8090c0, 0.7);
    }
}

impl Game for Vector {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        let go = input.pressed(Key::Space) || input.pressed(Key::Enter);
        if self.demo {
            if input.pressed(Key::Left) { self.start = self.start.saturating_sub(1).max(1); }
            if input.pressed(Key::Right) { self.start = (self.start + 1).min(WEBS); }
            if go { self.begin(); return Flow::Continue; }
        } else if input.pressed(Key::Char('p')) && matches!(self.mode, Mode::Play | Mode::Warp) { return Flow::Pause; }
        let ctl = if self.demo { self.bot() } else {
            let side = |k: Key| input.pressed(k) || input.motion(k);
            Ctl { dir: side(Key::Right) as i32 - side(Key::Left) as i32, fire: input.held(Key::Space),
                zap: input.pressed(Key::Char('z')) || input.pressed(Key::Down) }
        };
        self.step(ctl, go, dt);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        let mut g = std::mem::take(&mut self.glow);
        // Under the name the game is dimmed.
        g.gain = if self.demo { 0.5 } else { 1.0 };
        self.scene(&mut g);
        g.gain = 1.0;
        if self.demo { self.name(&mut g); } else { self.hud(&mut g); }
        g.show(f, 0.55f32.powf(self.since * 60.0));
        self.since = 0.0;
        self.glow = g;
    }
}

impl Vector {
    /// One frame of the game, with what is asked of the claw.
    fn step(&mut self, ctl: Ctl, go: bool, dt: f32) {
        self.time += dt;
        self.since += dt;
        self.flash = (self.flash - dt * 3.0).max(0.0);
        for b in self.fx.iter_mut() { b.age += dt; }
        self.fx.retain(|b| b.age < b.life);
        match self.mode {
            Mode::Arrive => {
                self.timer += dt;
                self.drive(ctl, dt);
                let t = (self.timer / 0.7).min(1.0);
                self.zoom = 0.02 + 0.98 * (1.0 - (1.0 - t).powi(3));
                if t >= 1.0 { (self.mode, self.zoom) = (Mode::Play, 1.0); }
            }
            Mode::Play => self.play(ctl, dt),
            Mode::Warp => self.warp(ctl, dt),
            Mode::Dying => self.dying(dt),
            Mode::Over => {
                self.timer += dt;
                if self.timer > 6.0 || (go && self.timer > 1.0) {
                    self.title();
                    self.audio.play_loop(1, &self.snd.title, 0.5);
                }
            }
        }
    }
}

/// The game as it is played: with sound and the title tune, or straight
/// into the level `VECTOR_START` names.
fn vector() -> Vector {
    let mut game = Vector::new();
    game.audio = Audio::open();
    match std::env::var("VECTOR_START").ok().and_then(|s| s.trim().parse::<u32>().ok()) {
        Some(level) => { game.start = level.max(1); game.begin(); }
        None => game.audio.play_loop(1, &game.snd.title, 0.5),
    }
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // VECTOR_BENCH=<frames> plays that many frames by itself with no
    // terminal and prints the time one takes.
    if let Ok(n) = std::env::var("VECTOR_BENCH") {
        bench(&mut Vector::new(), &Input::new(), Config { width: W, height: H, fps: 60 }, n.parse().unwrap_or(600));
        return;
    }
    run(&mut vector(), Config { width: W, height: H, fps: 60 });
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(vector(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
mod tests {
    use super::*;

    /// A game on a web, past the arrival, with nothing coming up.
    fn on(level: u32) -> Vector {
        let mut game = Vector::new();
        game.start = level;
        game.begin();
        (game.mode, game.zoom) = (Mode::Play, 1.0);
        game.pool.clear();
        game
    }

    fn foe(game: &mut Vector, kind: Kind, lane: usize, t: f32) {
        let mut f = game.hatch(kind, lane, t);
        (f.wait, f.fire) = (99.0, 99.0);
        game.foes.push(f);
    }

    fn held(key: Key) -> Input { let mut i = Input::new(); i.inject(key); i }

    #[test]
    fn every_letter_is_drawn_from_whole_points() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-.:!/<>".chars() {
            let g = glyph(c);
            assert!(!g.is_empty(), "{} has no strokes", c);
            for stroke in g.split(' ') {
                assert!(stroke.len() >= 4 && stroke.len() % 2 == 0, "{}: the stroke {} is not points", c, stroke);
                for (i, b) in stroke.bytes().enumerate() {
                    assert!(b.is_ascii_digit() && b - b'0' <= if i % 2 == 0 { 4 } else { 6 }, "{}: {} is off the grid", c, stroke);
                }
            }
        }
    }

    #[test]
    fn every_web_is_sound() {
        for i in 0..WEBS {
            let web = Web::new(i);
            assert!(web.lanes >= 12 && web.lanes <= 17, "web {} has {} lanes", i, web.lanes);
            assert_eq!(web.pts.len(), if web.closed { web.lanes } else { web.lanes + 1 });
            for l in 0..web.lanes {
                let (a, b) = (web.rim(l as f32), web.rim(l as f32 + 1.0));
                assert!(far(a, b) > 0.11, "web {}: lane {} is too narrow", i, l);
                assert!(a.0.abs() <= 1.0001 && a.1.abs() <= 1.0001, "web {}: lane {} is off the screen", i, l);
            }
            // Round a closed web the short way; an open one has ends.
            let last = web.lanes - 1;
            if web.closed {
                assert_eq!((web.step(last, 1), web.dist(0, last), web.beside(0, -1)), (Some(0), -1, Some(last)));
            } else {
                assert_eq!((web.step(last, 1), web.dist(0, last), web.beside(0, -1)), (None, last as i32, None));
            }
        }
    }

    #[test]
    fn a_web_narrows_toward_its_far_end() {
        let game = on(1);
        let wide = |t: f32| far(game.at(0.0, t), game.at(1.0, t));
        assert!(wide(1.0) > wide(0.5) && wide(0.5) > wide(0.0) && wide(0.0) > 2.0);
        assert!((wide(0.0) / wide(1.0) - FAR).abs() < 0.01);
    }

    #[test]
    fn a_shot_kills_what_is_in_its_lane() {
        let mut game = on(1);
        let lane = game.lane;
        foe(&mut game, Kind::Flipper, lane, 0.5);
        let other = (lane + 3) % game.web.lanes;
        foe(&mut game, Kind::Flipper, other, 0.5);
        let fire = held(Key::Space);
        for _ in 0..20 { game.update(&fire, 1.0 / 60.0); }
        assert_eq!(game.foes.len(), 1, "the flipper ahead still climbs");
        assert_eq!(game.score, 150);
    }

    #[test]
    fn a_tanker_lets_out_two() {
        let mut game = on(3);
        foe(&mut game, Kind::Tanker, 5, 0.5);
        game.kill(0);
        assert_eq!(game.foes.iter().map(|f| (f.kind, f.lane)).collect::<Vec<_>>(), [(Kind::Flipper, 4), (Kind::Flipper, 6)]);
        assert_eq!(game.score, 100);
    }

    #[test]
    fn a_flipper_on_the_rim_comes_for_the_claw() {
        let mut game = on(1);
        let lane = (game.lane + 3) % game.web.lanes;
        foe(&mut game, Kind::Flipper, lane, 0.99);
        game.foes[0].wait = 0.0;
        // One more far below, or the web would be done.
        foe(&mut game, Kind::Tanker, lane, 0.0);
        let idle = Input::new();
        for _ in 0..240 { game.update(&idle, 1.0 / 60.0); if game.mode == Mode::Dying { break; } }
        assert_eq!((game.mode, game.how), (Mode::Dying, How::Grab));
    }

    #[test]
    fn a_flipper_can_be_shot_as_it_flips_in() {
        let mut game = on(1);
        let lane = (game.lane + 2) % game.web.lanes;
        foe(&mut game, Kind::Flipper, lane, 0.99);
        game.foes[0].wait = 0.0;
        let below = (lane + 6) % game.web.lanes;
        foe(&mut game, Kind::Tanker, below, 0.0);
        let fire = held(Key::Space);
        for _ in 0..240 { game.update(&fire, 1.0 / 60.0); if game.foes.len() == 1 { break; } }
        assert!(game.foes.len() == 1 && game.mode == Mode::Play && game.score == 150, "it got through the fire");
    }

    #[test]
    fn the_superzapper_clears_the_web_once() {
        let mut game = on(5);
        for l in 0..6 { foe(&mut game, Kind::Flipper, l, 0.2 + l as f32 * 0.1); }
        game.update(&held(Key::Char('z')), 1.0 / 60.0);
        let idle = Input::new();
        for _ in 0..60 { game.update(&idle, 1.0 / 60.0); }
        assert!(game.foes.is_empty() && game.zaps == 1);
        // The second time it takes one.
        for l in 0..3 { foe(&mut game, Kind::Flipper, l + 8, 0.3); }
        (game.mode, game.depth, game.zoom) = (Mode::Play, 1.0, 1.0);
        game.update(&held(Key::Char('z')), 1.0 / 60.0);
        for _ in 0..30 { game.update(&idle, 1.0 / 60.0); }
        assert_eq!((game.foes.len(), game.zaps), (2, 2));
    }

    #[test]
    fn a_spike_ends_the_flight_down_and_a_clear_lane_does_not() {
        for spiked in [true, false] {
            let mut game = on(1);
            if spiked { game.spikes[game.lane] = 0.6; }
            let idle = Input::new();
            for _ in 0..400 { game.update(&idle, 1.0 / 60.0); if game.mode == Mode::Dying || game.level == 2 { break; } }
            if spiked { assert_eq!((game.mode, game.how), (Mode::Dying, How::Spike)); } else { assert_eq!((game.level, game.mode), (2, Mode::Arrive)); }
        }
    }

    #[test]
    fn a_fuseball_can_be_hit_only_while_it_crosses() {
        let game = on(6);
        let mut f = Foe { kind: Kind::Fuse, lane: 4, t: 0.5, flip: None, wait: 0.0, fire: 9.0, dir: 1.0, goal: 0.5, cargo: Kind::Flipper, rim: false, pulse: 0.0 };
        assert_eq!(f.target(&game.web), None);
        f.flip = Some((1, 0.3));
        assert_eq!(f.target(&game.web), Some(4));
        f.flip = Some((-1, 0.3));
        assert_eq!(f.target(&game.web), Some(3));
    }

    #[test]
    fn later_levels_bring_more_and_new_kinds() {
        let has = |l: u32, k: Kind| roster(l).iter().filter(|&&x| x == k).count();
        assert_eq!((has(1, Kind::Tanker), has(1, Kind::Spiker), has(1, Kind::Fuse), has(1, Kind::Pulsar)), (0, 0, 0, 0));
        assert!(has(3, Kind::Tanker) > 0 && has(4, Kind::Spiker) > 0 && has(6, Kind::Fuse) > 0 && has(8, Kind::Pulsar) > 0);
        for l in 1..40 { assert!(roster(l + 1).len() >= roster(l).len()); }
    }

    /// Not a test: the bot plays real games, to see how hard they are and
    /// what they look like. `VECTOR_FILM=<folder>,<level>,<every>,<frames>`
    /// saves every so-many-th frame of one game; without it, forty games
    /// are played and how they went is printed.
    #[test]
    #[ignore]
    fn the_bot_plays() {
        if let Ok(film) = std::env::var("VECTOR_FILM") {
            let p: Vec<&str> = film.split(',').collect();
            let (every, frames): (usize, usize) = (p[2].parse().unwrap(), p[3].parse().unwrap());
            let (mut game, mut f) = (Vector::new(), Frame::new(W, H));
            game.start = p[1].parse().unwrap();
            game.begin();
            for n in 1..=every * frames {
                let ctl = game.bot();
                game.step(ctl, false, 1.0 / 60.0);
                game.draw(&mut f);
                if n % every == 0 { std::fs::write(format!("{}/f-{:04}.ppm", p[0], n / every), f.to_ppm()).unwrap(); }
            }
            return;
        }
        let (mut games, mut how) = (Vec::new(), std::collections::BTreeMap::new());
        for _ in 0..40 {
            let mut game = Vector::new();
            game.begin();
            let mut last = Mode::Arrive;
            while game.mode != Mode::Over && game.time < 3600.0 {
                let ctl = game.bot();
                game.step(ctl, false, 1.0 / 60.0);
                if game.mode == Mode::Dying && last != Mode::Dying { *how.entry(format!("{:?}", game.how)).or_insert(0) += 1; }
                last = game.mode;
            }
            games.push((game.level, game.score, (game.time / 60.0) as u32));
        }
        games.sort();
        println!("level, score, minutes: {:?}", games);
        println!("ends: {:?}", how);
    }

    #[test]
    fn the_game_plays_itself_through_webs() {
        // Left alone it must neither stall nor fall over, on any web.
        let mut game = Vector::new();
        let (idle, mut f) = (Input::new(), Frame::new(W, H));
        let mut seen = 0;
        for level in 1..=32 {
            game.enter(level);
            for n in 0..3600 {
                game.update(&idle, 1.0 / 60.0);
                if n % 240 == 0 { game.draw(&mut f); }
                if game.level != level || game.mode == Mode::Dying { break; }
            }
            if game.level > level { seen += 1; }
        }
        assert!(game.demo && seen >= 8, "it finished only {} webs of 32", seen);
    }
}
