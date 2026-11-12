//! bigger: a dot eats dots. Then the picture pulls back, and the dot
//! turns out to be something else, in a world with rules of its own.
//!
//!     cargo run --release --example bigger
//!
//! The arrows steer, P pauses, Q quits. Eat what is smaller than you and
//! keep away from what is bigger. Seven sizes: a dot, a cell, a fish, a
//! moon, a star, a neutron star and a black hole. Each moves its own way.
//!
//! The world is 480 by 300 units and the picture 960 by 600 pixels, so
//! every shape is painted with four pixels to a unit.
//!
//! `BIGGER_START=<1 to 8>` starts at another size (8 is the ending);
//! `BIGGER_BENCH=<frames>` plays that many frames with no terminal and
//! prints the time one takes, and `BIGGER_SHOT=<file.ppm>` keeps the
//! last picture of that run.

use funkey::noise::{fbm, hash, noise};
use funkey::particles::Particle;
use funkey::*;
use std::f32::consts::{PI, TAU};

const W: i32 = 480;
const H: i32 = 300;
const CX: f32 = 240.0;
const CY: f32 = 150.0;
/// The world is 480 by 300; the picture has this many pixels to each
/// unit of it, each way.
#[cfg(not(target_arch = "wasm32"))]
const S: i32 = 2;
/// A web page paints on one core, so there a unit is one pixel.
#[cfg(target_arch = "wasm32")]
const S: i32 = 1;
const SF: f32 = S as f32;
const RW: i32 = W * S;
const RH: i32 = H * S;
const GAME: &str = "bigger";
/// The game's own version; the engine has its own.
const VERSION: &str = "0.3";

/// The pull of the giant the moon goes round: a round orbit 105 pixels
/// out takes 68 pixels a second.
const GM: f32 = 485_520.0;
/// The giant's radius.
const GIANT: f32 = 33.0;
/// The widest orbit that stays on the screen.
const FAR: f32 = 138.0;
/// How strongly stars pull at each other, and a black hole at stars.
const STAR_PULL: f32 = 1300.0;
const HOLE_PULL: f32 = 2600.0;
/// How far a neutron star's beam reaches.
const BEAM: f32 = 130.0;
/// How far out a disc must reach to cover the whole picture.
const COVER: f32 = 285.0;
/// Seconds a pull-back takes, and the two other ways to change size.
const PULL: f32 = 3.4;
const NOVA: f32 = 5.0;
const NOVA_BANG: f32 = 1.4;
const FALL: f32 = 4.4;
const FALL_DONE: f32 = 1.9;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Size { Dot, Cell, Fish, Moon, Star, Pulsar, Hole }

impl Size {
    fn n(self) -> u32 { self as u32 + 1 }
    fn name(self) -> &'static str {
        match self {
            Size::Dot => "DOT", Size::Cell => "CELL", Size::Fish => "FISH", Size::Moon => "MOON",
            Size::Star => "STAR", Size::Pulsar => "NEUTRON STAR", Size::Hole => "BLACK HOLE",
        }
    }
    /// The radius it starts with, and the radius that ends the size.
    fn start_r(self) -> f32 {
        match self { Size::Dot => 3.0, Size::Cell | Size::Fish | Size::Star => 7.0, Size::Moon | Size::Pulsar => 6.0, Size::Hole => 8.0 }
    }
    fn goal_r(self) -> f32 {
        match self {
            Size::Dot => 3.0, Size::Cell => 21.0, Size::Fish => 22.0, Size::Moon => 15.0,
            Size::Star => 20.0, Size::Pulsar => 15.0, Size::Hole => 24.0,
        }
    }
    fn next(self) -> Option<Size> {
        match self {
            Size::Dot => Some(Size::Cell), Size::Cell => Some(Size::Fish), Size::Fish => Some(Size::Moon),
            Size::Moon => Some(Size::Star), Size::Star => Some(Size::Pulsar), Size::Pulsar => Some(Size::Hole), Size::Hole => None,
        }
    }
    /// How many things share the picture with the player.
    fn crowd(self) -> usize {
        match self { Size::Dot => 6, Size::Cell => 16, Size::Fish => 13, Size::Moon => 12, Size::Star => 13, Size::Pulsar => 12, Size::Hole => 16 }
    }
    /// What the change of size showed, and the new rule.
    fn lines(self) -> (&'static str, &'static str) {
        match self {
            Size::Dot => ("EAT THE DOTS.", ""),
            Size::Cell => ("YOU WERE INSIDE A CELL.", "EAT THE SMALL. FLEE THE BIG."),
            Size::Fish => ("THE CELL WAS INSIDE A FISH.", "SAME RULES. WETTER. JELLYFISH STING."),
            Size::Moon => ("THE FISH LIVED ON A MOON.", "YOU ARE THE MOON. MIND THE GRAVITY."),
            Size::Star => ("THE MOON WENT ROUND A STAR.", "YOU ARE THE STAR. EVERYTHING PULLS EVERYTHING."),
            Size::Pulsar => ("YOU GOT TOO BIG. YOU BLEW UP.", "LEFT AND RIGHT TURN. UP PUSHES. THE BEAM BREAKS THE BIG ONES."),
            Size::Hole => ("TOO HEAVY. YOU FELL IN ON YOURSELF.", "YOU ARE A BLACK HOLE. EAT ANYTHING. STOP EATING AND YOU FADE."),
        }
    }
}

/// What a picture shows: a size being played, or one of the three
/// pictures the ending pulls back through.
#[derive(Clone, Copy, PartialEq, Debug)]
enum View { Stage(Size), Galaxy, Web, Void }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Play, Pull, Nova, Fall, End, Over, Won }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// Food while it is smaller, a danger while it is bigger.
    Plain,
    /// It stings at any size and is never food: a jellyfish.
    Sting,
    /// Another black hole.
    Rival,
}

/// Everything that is not the player.
#[derive(Clone, Copy, Debug)]
struct Thing {
    x: f32, y: f32, vx: f32, vy: f32, r: f32,
    kind: Kind,
    color: Rgb,
    /// Where it is in its own wobble.
    phase: f32,
    /// Seconds since it came; a new one fades in and bites nobody yet.
    age: f32,
    /// A rock's orbit: how far out, the angle now, and the turn a second.
    orbit: f32, angle: f32, turn: f32,
    /// How long a neutron star's beam has been on it.
    heat: f32,
}

#[derive(Clone, Copy, Debug)]
struct Me {
    x: f32, y: f32, vx: f32, vy: f32, r: f32,
    /// Seconds left in which nothing can bite.
    safe: f32,
    /// 1 looking right, -1 looking left.
    face: f32,
    /// The way a neutron star's beam points.
    ang: f32,
}

/// A pull-back: the old picture shrinks into a disc that ends as the
/// thing at (`x`, `y`) with radius `r` in the new picture.
#[derive(Clone, Copy, Debug)]
struct Pull { t: f32, x: f32, y: f32, r: f32, with_me: bool }

/// A number that floats up from a meal.
struct Pop { x: f32, y: f32, t: f32, text: String }

/// A ring that spreads from a meal or a bite.
struct Ring { x: f32, y: f32, t: f32, life: f32, reach: f32, color: Rgb }

struct Sounds {
    eat: Vec<Sample>,
    bit: Sample,
    pull: Sample,
    nova: Sample,
    fall: Sample,
    crack: Sample,
    big: Sample,
    end: Sample,
    music: [Sample; 6],
}

impl Sounds {
    fn new() -> Sounds {
        let eat = (0..8).map(|i| {
            let hz = 420.0 * 2f32.powf(i as f32 / 6.0);
            Sample::sweep(Wave::Triangle, hz, hz * 1.5, 0.07, 0.35)
        }).collect();
        // A pull-back sounds like a long fall: the pitch drops as the
        // world shrinks.
        let pull = Sample::synth(PULL, |t, _| {
            let k = t / PULL;
            let hz = 660.0 * (0.12f32).powf(k);
            (t * hz * TAU).sin() * 0.22 * (1.0 - k) * (t * 8.0).min(1.0)
        });
        // A star that blows up: a rising whine, then a long roar.
        let nova = Sample::synth(NOVA, |t, r| {
            if t < NOVA_BANG {
                (t * (200.0 + t * 500.0) * TAU).sin() * 0.2 * t / NOVA_BANG
            } else {
                let k = (t - NOVA_BANG) / (NOVA - NOVA_BANG);
                (r * 0.5 + (t * 48.0 * TAU).sin() * 0.4) * (1.0 - k).powi(2)
            }
        });
        let fall = Sample::synth(FALL, |t, r| {
            let k = t / FALL;
            let hz = 300.0 * (0.08f32).powf(k);
            ((t * hz * TAU).sin() * 0.35 + r * 0.03) * (1.0 - k) * (t * 6.0).min(1.0)
        });
        let big = Sample::synth(2.5, |t, r| ((t * 55.0 * TAU).sin() * 0.5 + r * 0.05) * (t * 2.0).min(1.0) * (2.5 - t).min(1.0));
        let tri = Wave::Triangle;
        let cell = Tune::parse("168 c4/8 g4/8 e4/8 g4/8 a3/8 e4/8 c4/8 e4/8 f3/8 c4/8 a3/8 c4/8 g3/8 d4/8 b3/8 d4/8", tri, 0.16).render();
        let fish = Tune::parse("132 d4 a4/8 f4/8 a4 - c4 g4/8 e4/8 g4 - bb3 f4/8 d4/8 f4 - a3 e4/8 c#4/8 e4 -", Wave::Sine, 0.2).render();
        let moon = Tune::parse("60 a2/2 e3/2 c3/2 g3/2 f2/2 c3/2 e2/2 b2/2", Wave::Sine, 0.24).render()
            .mixed(&Tune::parse("60 - e5/8 - a5/8 - c5/8 - g5/8 - a4/8 - c5/8 - b4/8 - e5/8", tri, 0.07).render());
        let star = Tune::parse("96 d3/2 a3/2 f#3/2 a3/2 b2/2 f#3/2 g2/2 d3/2", Wave::Sine, 0.22).render()
            .mixed(&Tune::parse("96 f#5 a5/8 d6/8 a5 - d5 f#5/8 b5/8 f#5 - b4 d5/8 g5/8 d5 - a4 d5/8 f#5/8 a5 -", tri, 0.09).render());
        let pulsar = Tune::parse("224 e3/8 e4/8 e3/8 b3/8 e3/8 e4/8 g3/8 b3/8 d3/8 d4/8 d3/8 a3/8 c3/8 c4/8 b2/8 b3/8", Wave::Square, 0.07).render();
        let hole = Tune::parse("40 c2/2 g1/2 bb1/2 f1/2", Wave::Sine, 0.34).render()
            .mixed(&Tune::parse("40 - eb4/8 - - g3/8 - - - d4/8 - - f3/8 - - - -", tri, 0.06).render());
        Sounds {
            eat,
            bit: Sample::sweep(Wave::Saw, 300.0, 70.0, 0.35, 0.3),
            pull, nova, fall,
            crack: Sample::synth(0.25, |t, r| r * 0.4 * (1.0 - t / 0.25).powi(2)),
            big,
            end: Tune::parse("200 c5/8 g4/8 e4/8 c4/2", tri, 0.3).render(),
            music: [cell, fish, moon, star, pulsar, hole],
        }
    }
}

struct Bigger {
    size: Size,
    view: View,
    me: Me,
    things: Vec<Thing>,
    /// Things eaten at this size, and in a row with no long gap.
    eaten: u32,
    combo: u32,
    combo_t: f32,
    score: u32,
    high: u32,
    lives: u32,
    mode: Mode,
    /// Seconds in the mode, in the size, and in the whole game.
    timer: f32,
    stage_t: f32,
    total_t: f32,
    time: f32,
    pull: Pull,
    /// Which picture of the ending the pull-back is on its way to.
    step: u8,
    /// The old picture, while it shrinks.
    snap: Frame,
    /// The sky behind a size in space, painted once when the size starts.
    sky: Frame,
    bits: Particles,
    pops: Vec<Pop>,
    rings: Vec<Ring>,
    /// Where the player has just been, newest last.
    trail: Vec<(f32, f32)>,
    /// Seconds the picture still trembles after a bite.
    shake: f32,
    rng: Rng,
    audio: Audio,
    snd: Sounds,
    /// A test's hand on the arrows.
    #[cfg(test)]
    bot: bool,
}

fn smooth(t: f32) -> f32 { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

/// One colour laid over another, worked out in whole numbers: quick
/// enough to do for every pixel of a picture.
#[inline]
fn lay(under: Rgb, over: Rgb, a: f32) -> Rgb { lay_k(under, over, (a.clamp(0.0, 1.0) * 256.0) as u32) }

/// The same, with the share of the upper colour as a count out of 256.
#[inline]
fn lay_k(under: Rgb, over: Rgb, k: u32) -> Rgb {
    let rb = ((((under & 0xff00ff) * (256 - k)) >> 8) + (((over & 0xff00ff) * k) >> 8)) & 0xff00ff;
    let g = ((((under & 0x00ff00) * (256 - k)) >> 8) + (((over & 0x00ff00) * k) >> 8)) & 0x00ff00;
    rb | g
}

/// A colour with light added to it: light never darkens.
#[inline]
fn lit(under: Rgb, c: Rgb, k: f32) -> Rgb {
    let k = (k.clamp(0.0, 4.0) * 256.0) as u32;
    let ch = |sh: u32| (((under >> sh) & 255) + ((((c >> sh) & 255) * k) >> 8)).min(255) << sh;
    ch(16) | ch(8) | ch(0)
}

/// Light added to one pixel of the picture.
#[inline]
fn add(f: &mut Frame, x: i32, y: i32, c: Rgb, k: f32) {
    if x < 0 || y < 0 || x >= f.w || y >= f.h || k <= 0.0 { return; }
    let i = (y * f.w + x) as usize;
    f.px[i] = lit(f.px[i], c, k);
}

/// Paint every row of a picture, the rows shared out over the cores.
fn rows(f: &mut Frame, paint: impl Fn(i32, &mut [Rgb]) + Sync) {
    let w = f.w.max(1) as usize;
    #[cfg(target_arch = "wasm32")]
    for (y, row) in f.px.chunks_mut(w).enumerate() { paint(y as i32, row); }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
        let per = (f.h.max(1) as usize).div_ceil(n);
        std::thread::scope(|s| {
            for (i, band) in f.px.chunks_mut(per * w).enumerate() {
                let paint = &paint;
                s.spawn(move || for (j, row) in band.chunks_mut(w).enumerate() { paint((i * per + j) as i32, row); });
            }
        });
    }
}

// From here on a place and a size are given in the world's own units,
// and each shape works out the pixels itself.

/// A soft light round a point, strongest in the middle.
fn glow(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, k: f32) {
    let (cx, cy, r) = (cx * SF, cy * SF, r * SF);
    if r < 1.0 || k <= 0.0 { return; }
    let (x0, x1) = (((cx - r) as i32).max(0), ((cx + r) as i32 + 1).min(f.w - 1));
    for y in ((cy - r) as i32).max(0)..=((cy + r) as i32 + 1).min(f.h - 1) {
        let row = (y * f.w) as usize;
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt() / r;
            if d < 1.0 { let i = row + x as usize; f.px[i] = lit(f.px[i], c, (1.0 - d) * (1.0 - d) * k); }
        }
    }
}

/// A fine point of light: a far star, a spark.
fn spot(f: &mut Frame, x: f32, y: f32, c: Rgb, k: f32) { glow(f, x, y, 0.95, c, k * 1.5); }

/// A disc with a crisp edge, laid over what is there.
fn blend_disc(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
    let (cx, cy, r) = (cx * SF, cy * SF, r * SF);
    let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
    for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            if d > r { continue; }
            let under = f.get(x, y);
            f.put(x, y, lay(under, c, a * (r - d).min(1.0)));
        }
    }
}

/// A disc that fades out toward its edge: something out of focus.
fn soft(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
    let (cx, cy, r) = (cx * SF, cy * SF, r * SF);
    let (x0, x1) = (((cx - r) as i32).max(0), ((cx + r) as i32 + 1).min(f.w - 1));
    for y in ((cy - r) as i32).max(0)..=((cy + r) as i32 + 1).min(f.h - 1) {
        let row = (y * f.w) as usize;
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt() / r;
            if d < 1.0 { let i = row + x as usize; f.px[i] = lay(f.px[i], c, a * smooth(1.0 - d)); }
        }
    }
}

/// A rock in space: a ball lit from the upper left, rough, with craters.
fn rock(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, a: f32, seed: i32) {
    let (cx, cy, r) = (cx * SF, cy * SF, r * SF);
    // Craters: where each is on the face, and how wide.
    let n = if r < 7.0 { 0 } else { 2 + (seed.unsigned_abs() % 4) as i32 };
    let pits: Vec<(f32, f32, f32)> = (0..n).map(|k| {
        let (th, far) = (hash(k, seed, 81) * TAU, hash(k, seed, 82).sqrt() * 0.62);
        (th.cos() * far * r, th.sin() * far * r, r * (0.13 + 0.2 * hash(k, seed, 83)))
    }).collect();
    let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
    for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r { continue; }
            let (nx, ny) = (dx / r, dy / r);
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            let mut light = 0.1 + 1.05 * (nx * -0.52 + ny * -0.55 + nz * 0.65).max(0.0);
            if r > 4.0 {
                let rough = fbm(nx * 2.4 + seed as f32 * 0.37 + 40.0, ny * 2.4 + 40.0, 3, 84, 64);
                light *= 0.7 + 0.6 * rough;
            }
            for &(px, py, pr) in &pits {
                let (qx, qy) = (dx - px, dy - py);
                let q = (qx * qx + qy * qy).sqrt() / pr;
                // The wall away from the light is bright, the near one dark.
                if q < 1.0 { light *= 0.72 + 0.4 * (qx + qy) / pr * smooth(q * 1.4); }
                else if q < 1.25 { light *= 1.12; }
            }
            let under = f.get(x, y);
            f.put(x, y, lay(under, tint(c, light), a * (r - d2.sqrt()).min(1.0)));
        }
    }
}

/// A small world of sea, land, ice and cloud that turns, with thin air
/// glowing round it.
fn planet(f: &mut Frame, cx: f32, cy: f32, r: f32, a: f32, time: f32) {
    glow(f, cx, cy, r * 1.9, 0x3c8cff, 0.32 * a);
    let (cx, cy, r) = (cx * SF, cy * SF, r * SF);
    let reach = r.ceil() as i32 + S;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (fx, fy) = (dx as f32 + 0.5, dy as f32 + 0.5);
            let d = (fx * fx + fy * fy).sqrt();
            let (px, py) = (cx as i32 + dx, cy as i32 + dy);
            if d > r {
                let k = (1.0 - (d - r) / (1.6 * SF)).max(0.0);
                add(f, px, py, 0x70b8ff, k * k * 0.7 * a);
                continue;
            }
            let (nx, ny) = (fx / r, fy / r);
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            let (lon, lat) = (nx.atan2(nz) + time * 0.25, ny.asin());
            let high = fbm(lon * 1.3 + 30.0, lat * 2.0 + 30.0, 4, 21, 64);
            let mut c = if high > 0.52 {
                lay(0x3c8a3c, 0x9a8250, ((high - 0.52) * 7.0).min(1.0))
            } else {
                lay(0x0c3c8c, 0x2c7cd0, (high / 0.52).powi(3))
            };
            if lat.abs() > 1.12 { c = lay(c, 0xf0f8ff, ((lat.abs() - 1.12) * 8.0).min(1.0)); }
            let cloud = fbm(lon * 1.7 + time * 0.06 + 50.0, lat * 3.2 + 50.0, 3, 22, 64);
            if cloud > 0.55 { c = lay(c, WHITE, ((cloud - 0.55) * 6.0).min(0.9)); }
            let facing = (nx * -0.52 + ny * -0.55 + nz * 0.65).max(0.0);
            let mut col = tint(c, 0.08 + 1.1 * facing);
            // The sun glints on the sea.
            if high <= 0.52 { col = lit(col, WHITE, facing.powi(24) * 0.6); }
            let under = f.get(px, py);
            f.put(px, py, lay(under, col, a * (r - d).min(1.0)));
        }
    }
}

/// A thin circle of light.
fn circle_of_light(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, k: f32) {
    let (cx, cy, r) = (cx * SF, cy * SF, r * SF);
    if r < 0.5 || k <= 0.0 { return; }
    let n = ((r * TAU * 1.3) as i32).clamp(8, 9000);
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let (ca, sa) = (a.cos(), a.sin());
        add(f, (cx + ca * r) as i32, (cy + sa * r) as i32, c, k);
        add(f, (cx + ca * (r + 1.0)) as i32, (cy + sa * (r + 1.0)) as i32, c, k * 0.5);
    }
}

/// Every pixel moved a little toward a colour: a slow swell of light.
fn veil(f: &mut Frame, c: Rgb, a: f32) {
    if a <= 0.0 { return; }
    for p in f.px.iter_mut() { *p = lay(*p, c, a); }
}

/// Space is bent round a black hole: what lies behind it shows as a
/// ring round the dark middle. This one counts in pixels.
fn lens(f: &mut Frame, cx: f32, cy: f32, r: f32) {
    let reach = (r * 3.4) as i32;
    let (bx, by) = (cx as i32 - reach, cy as i32 - reach);
    let side = reach * 2 + 1;
    let mut old = vec![BLACK; (side * side) as usize];
    for y in 0..side { for x in 0..side { old[(y * side + x) as usize] = f.get(bx + x, by + y); } }
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = ((x - reach) as f32, (y - reach) as f32);
            let d = (dx * dx + dy * dy).sqrt();
            if d >= reach as f32 { continue; }
            if d < r { f.put(bx + x, by + y, BLACK); continue; }
            // Near the hole the picture is taken from the far side.
            let s = 1.0 - (r * 1.45 / d).powi(2) * smooth((reach as f32 - d) / (reach as f32 * 0.5));
            let (sx, sy) = ((reach as f32 + dx * s) as i32, (reach as f32 + dy * s) as i32);
            f.put(bx + x, by + y, old[(sy.clamp(0, side - 1) * side + sx.clamp(0, side - 1)) as usize]);
        }
    }
}

/// A black hole: the bent sky, the dark middle, and the disc of hot gas
/// that goes round it. The disc is seen nearly edge on. Its far half
/// shows twice: flat behind the hole, and bent over the top and under
/// the bottom of it, since the light from there curves round.
fn hole(f: &mut Frame, wx: f32, wy: f32, wr: f32, c: Rgb, turn: f32, a: f32) {
    let (x, y, r) = (wx * SF, wy * SF, wr * SF);
    lens(f, x, y, r);
    const TILT: f32 = 0.27;
    let (inner, outer) = (r * 1.45, r * 3.2);
    let hot = mix(c, WHITE, 0.85);
    // Gas at an angle and a distance: streaks that go round, the inner
    // ones faster than the outer ones.
    let gas = |th: f32, rho: f32, p: f32| {
        let (u, v) = ((th / TAU + 0.5) * 16.0, rho / r * 4.5);
        let fast = noise(u + turn * 2.6, v, 16, 91);
        let slow = noise(u + turn * 1.3, v + 5.0, 16, 92);
        let fine = noise(u * 2.0 + turn * 4.0, v * 3.0, 32, 93);
        let hair = noise(u * 4.0 + turn * 7.0, v * 7.0, 64, 94);
        (fast * (1.0 - p) + slow * p) * 0.65 + fine * 0.22 + hair * 0.13
    };
    let reach = outer.ceil() as i32;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (fx, fy) = (dx as f32, dy as f32);
            let d = (fx * fx + fy * fy).sqrt();
            let (px, py) = (x as i32 + dx, y as i32 + dy);
            // The far half of the disc, bent round the hole: a bright
            // rim that hugs the dark middle, thickest over the top.
            if d >= r && d < r * 1.5 {
                let th = fy.atan2(fx);
                let over = 0.55 + 0.45 * (-fy / d);
                let thick = r * (0.22 + 0.24 * over);
                let k = (1.0 - (d - r) / thick).max(0.0);
                let streak = 0.55 + 0.6 * gas(th, r * 1.6 + (d - r) * 3.0, 0.2);
                // The gas that comes toward the eye, on the left, shines more.
                let side = 0.7 + 0.5 * (-fx / d);
                add(f, px, py, mix(hot, c, (d - r) / thick), k * streak * side * over * 2.0 * a);
                // And right at the edge, a thin ring of light that has
                // gone all the way round.
                let ring = (1.0 - (d - r * 1.03).abs() / 1.6).max(0.0);
                add(f, px, py, WHITE, ring * 0.9 * (wr / 14.0).min(1.0) * a);
            }
            // The disc itself, squeezed flat by the angle it is seen at.
            let (ex, ey) = (fx, fy / TILT);
            let rho = (ex * ex + ey * ey).sqrt();
            if rho < inner || rho > outer { continue; }
            // Its far half is hidden where the hole stands in front.
            if fy < 0.0 && d < r * 1.02 { continue; }
            let p = (rho - inner) / (outer - inner);
            let th = ey.atan2(ex);
            let heat = (1.0 - p).powf(1.15) * (p * 14.0).min(1.0);
            let streak = 0.25 + 1.1 * gas(th, rho, p);
            let side = 0.55 + 0.75 * (0.5 - 0.5 * th.cos());
            let col = if p < 0.35 { mix(hot, c, p / 0.35) } else { mix(c, tint(c, 0.45), (p - 0.35) / 0.65) };
            add(f, px, py, col, heat * streak * side * 2.3 * a);
        }
    }
    // A soft light round it all.
    glow(f, wx, wy, wr * 4.2, c, 0.13 * a);
}

/// The old picture at a spot, also past its edges: there it is laid out
/// again as in a mirror, so a round cut of it has no empty corners.
fn mirrored(snap: &Frame, x: f32, y: f32) -> Rgb {
    let fold = |v: i32, n: i32| { let m = v.rem_euclid(2 * n); if m < n { m } else { 2 * n - 1 - m } };
    snap.get(fold(x.floor() as i32, snap.w), fold(y.floor() as i32, snap.h))
}

/// The shortest way from `a` to `b` in a world that wraps at `n`.
fn wrapped(a: f32, b: f32, n: f32) -> f32 { (b - a + n * 0.5).rem_euclid(n) - n * 0.5 }

impl Bigger {
    fn new() -> Bigger {
        Bigger {
            size: Size::Dot, view: View::Void,
            me: Me { x: CX, y: CY, vx: 0.0, vy: 0.0, r: 3.0, safe: 0.0, face: 1.0, ang: 0.0 },
            things: Vec::new(), eaten: 0, combo: 0, combo_t: 0.0, score: 0,
            high: funkey::scores::best(GAME), lives: 3, mode: Mode::Title,
            timer: 0.0, stage_t: 0.0, total_t: 0.0, time: 0.0,
            pull: Pull { t: 0.0, x: CX, y: CY, r: 3.0, with_me: false }, step: 0,
            snap: Frame::new(RW, RH), sky: Frame::new(RW, RH), bits: Particles::new(),
            pops: Vec::new(), rings: Vec::new(), trail: Vec::new(), shake: 0.0,
            rng: match std::env::var("FUNKEY_SEED").ok().and_then(|s| s.parse().ok()) { Some(s) => Rng::new(s), None => Rng::from_time() },
            audio: Audio::off(), snd: Sounds::new(),
            #[cfg(test)]
            bot: false,
        }
    }

    /// A new game, from the size asked for.
    fn begin(&mut self, size: Size) {
        (self.score, self.lives, self.total_t, self.step) = (0, 3, 0.0, 0);
        self.enter(size);
        (self.mode, self.timer) = (Mode::Play, 0.0);
        self.music();
    }

    /// Set the size up: the player in its place, the world filled.
    fn enter(&mut self, size: Size) {
        (self.size, self.view) = (size, View::Stage(size));
        (self.eaten, self.combo, self.stage_t) = (0, 0, 0.0);
        self.bits = Particles::new();
        self.bits.gravity = 0.0;
        self.pops.clear();
        self.rings.clear();
        self.trail.clear();
        self.me = Me { x: CX, y: CY, vx: 0.0, vy: 0.0, r: size.start_r(), safe: 1.5, face: 1.0, ang: -PI / 2.0 };
        if size == Size::Moon { self.to_orbit(); }
        self.sky = sky(size);
        self.things.clear();
        for i in 0..size.crowd() {
            let mut t = self.spawn(i);
            t.age = 1.0;
            // The first of a size are spread over the picture; later
            // ones come in from its edges.
            if !matches!(size, Size::Dot | Size::Moon) && t.kind == Kind::Plain {
                t.x = self.rng.range(15.0, W as f32 - 15.0);
                t.y = self.rng.range(24.0, H as f32 - 30.0);
                if (t.x - CX).abs() < 80.0 && (t.y - CY).abs() < 70.0 { t.x += 170.0 * if t.x < CX { -1.0 } else { 1.0 }; }
            }
            self.things.push(t);
        }
    }

    /// The moon back on a round orbit, to the right of the giant.
    fn to_orbit(&mut self) {
        let a = 105.0;
        (self.me.x, self.me.y) = (CX + a, CY);
        (self.me.vx, self.me.vy) = (0.0, (GM / a).sqrt());
        self.trail.clear();
    }

    /// A radius for a new thing: mostly food, sometimes a danger, always
    /// measured against the player as it is now.
    fn pick_r(&mut self, nth: usize) -> f32 {
        let big = nth % 3 == 2;
        let k = if big { self.rng.range(1.3, 1.8) } else { self.rng.range(0.35, 0.8) };
        (self.me.r * k).clamp(2.0, 34.0)
    }

    /// A spot just outside the picture, and a speed that carries a thing
    /// from there into it.
    fn from_edge(&mut self, r: f32, speed: f32) -> (f32, f32, f32, f32) {
        let along = self.rng.float();
        let (x, y) = match self.rng.below(4) {
            0 => (-r, along * H as f32),
            1 => (W as f32 + r, along * H as f32),
            2 => (along * W as f32, -r),
            _ => (along * W as f32, H as f32 + r),
        };
        // Toward a spot near the middle, so that it crosses the picture.
        let (tx, ty) = (CX + self.rng.range(-120.0, 120.0), CY + self.rng.range(-80.0, 80.0));
        let d = ((tx - x).powi(2) + (ty - y).powi(2)).sqrt().max(1.0);
        (x, y, (tx - x) / d * speed, (ty - y) / d * speed)
    }

    /// One new thing for the size being played. `nth` decides its sort,
    /// so that two in three are food.
    fn spawn(&mut self, nth: usize) -> Thing {
        let mut t = Thing { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, r: 2.0, kind: Kind::Plain, color: WHITE,
            phase: self.rng.range(0.0, TAU), age: 0.0, orbit: 0.0, angle: 0.0, turn: 0.0, heat: 0.0 };
        match self.size {
            Size::Dot => {
                // Six dots on a ring round the middle, none under the player.
                let a = nth as f32 / 6.0 * TAU + self.rng.range(-0.3, 0.3);
                let d = self.rng.range(65.0, 120.0);
                (t.x, t.y) = (CX + a.cos() * d * 1.5, CY + a.sin() * d);
            }
            Size::Cell => {
                t.r = self.pick_r(nth);
                (t.x, t.y, t.vx, t.vy) = self.from_edge(t.r, 12.0);
                t.color = *self.rng.pick(&[0x70d0a0, 0xc0d060, 0x60c0d8, 0xd09070, 0xb080d8]).unwrap_or(&WHITE);
            }
            Size::Fish => {
                let right = self.rng.chance(0.5);
                if nth % 6 == 5 {
                    // A jellyfish rises from the sea floor.
                    (t.kind, t.r) = (Kind::Sting, 8.0);
                    (t.x, t.y) = (self.rng.range(30.0, W as f32 - 30.0), H as f32 + 16.0);
                    (t.vx, t.vy) = (0.0, -self.rng.range(10.0, 18.0));
                    t.color = 0xf080c0;
                } else {
                    t.r = self.pick_r(nth);
                    let speed = self.rng.range(27.0, 60.0);
                    t.x = if right { -t.r * 2.0 } else { W as f32 + t.r * 2.0 };
                    t.y = self.rng.range(30.0, H as f32 - 42.0);
                    t.vx = if right { speed } else { -speed };
                    t.color = *self.rng.pick(&[0xf0c040, 0x60d0b0, 0xe07050, 0xa0b0f0, 0xd0d0d8]).unwrap_or(&WHITE);
                }
            }
            Size::Moon => {
                t.r = self.pick_r(nth).clamp(3.0, 12.0);
                t.orbit = self.rng.range(GIANT + 33.0, FAR - 3.0);
                // Away from the player, so that nothing appears on top of it.
                let mine = (self.me.y - CY).atan2(self.me.x - CX);
                t.angle = mine + self.rng.range(1.2, TAU - 1.2);
                t.turn = (GM / t.orbit.powi(3)).sqrt();
                (t.x, t.y) = (CX + t.angle.cos() * t.orbit, CY + t.angle.sin() * t.orbit);
                t.color = *self.rng.pick(&[0x908880, 0xa09078, 0x787880, 0xb0a090]).unwrap_or(&WHITE);
            }
            Size::Star => {
                t.r = self.pick_r(nth).min(30.0);
                let speed = self.rng.range(14.0, 30.0);
                (t.x, t.y, t.vx, t.vy) = self.from_edge(t.r * 2.0, speed);
            }
            Size::Pulsar => {
                t.r = self.pick_r(nth);
                let speed = self.rng.range(18.0, 46.0);
                (t.x, t.y, t.vx, t.vy) = self.from_edge(t.r, speed);
                t.color = *self.rng.pick(&[0x8880a0, 0x7090a0, 0xa08890, 0x9098b0]).unwrap_or(&WHITE);
            }
            Size::Hole => {
                if nth == 7 {
                    (t.kind, t.r) = (Kind::Rival, (self.me.r * 1.5).min(34.0));
                    (t.x, t.y, t.vx, t.vy) = self.from_edge(t.r * 3.0, 18.0);
                    t.color = 0xb090ff;
                } else {
                    t.r = self.rng.range(2.5, 8.0);
                    let speed = self.rng.range(20.0, 44.0);
                    (t.x, t.y, t.vx, t.vy) = self.from_edge(t.r * 2.0, speed);
                    t.color = *self.rng.pick(&[0xffd070, 0xff9050, 0xa0c8ff, 0xfff0d0]).unwrap_or(&WHITE);
                }
            }
        }
        t
    }

    fn music(&mut self) {
        self.audio.stop(1);
        if self.size == Size::Dot || self.mode != Mode::Play { return; }
        self.audio.play_loop(1, &self.snd.music[self.size as usize - 1], 1.0);
    }

    fn sound(&mut self, pick: impl Fn(&Sounds) -> &Sample) {
        let s = pick(&self.snd).clone();
        self.audio.play(&s, 1.0);
    }

    /// Which way the arrows point, each part -1, 0 or 1.
    fn arrows(&self, input: &Input) -> (f32, f32) {
        #[cfg(test)]
        if self.bot { return self.bot_arrows(); }
        (input.axis_x() as f32, input.axis_y() as f32)
    }

    /// Move the player by the rules of its size.
    fn steer(&mut self, ax: f32, ay: f32, dt: f32) {
        let size = self.size;
        // What the others do to the player by their weight alone.
        let (mut gx, mut gy) = (0.0, 0.0);
        if size == Size::Star || size == Size::Hole {
            for t in &self.things {
                if size == Size::Hole && (t.kind != Kind::Rival || t.r < self.me.r) { continue; }
                let (dx, dy) = (t.x - self.me.x, t.y - self.me.y);
                let d2 = dx * dx + dy * dy + 900.0;
                let pull = STAR_PULL * t.r * t.r / d2 * (t.age * 0.5).min(1.0) * if size == Size::Hole { 0.4 } else { 1.0 };
                gx += dx / d2.sqrt() * pull;
                gy += dy / d2.sqrt() * pull;
            }
        }
        let me = &mut self.me;
        if ax != 0.0 { me.face = ax; }
        match size {
            // A dot goes where it is told and stops when it is not.
            Size::Dot => {
                (me.vx, me.vy) = (ax * 105.0, ay * 105.0);
            }
            // A cell is pushed about and drifts on. A fish is quicker.
            Size::Cell | Size::Fish => {
                let (push, drag) = if size == Size::Cell { (345.0, 2.4) } else { (480.0, 3.2) };
                me.vx += (ax * push - me.vx * drag) * dt;
                me.vy += (ay * push - me.vy * drag) * dt;
            }
            // A moon falls round the giant. The arrows push it; let go,
            // and its path settles into a round orbit where it is.
            Size::Moon => {
                let (dx, dy) = (me.x - CX, me.y - CY);
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                let g = GM / (d * d);
                me.vx += (-dx / d * g + ax * 165.0) * dt;
                me.vy += (-dy / d * g + ay * 165.0) * dt;
                // The speed of a round orbit here, the way it goes now.
                let way = if dx * me.vy - dy * me.vx >= 0.0 { 1.0 } else { -1.0 };
                let v = (GM / d).sqrt();
                let (rx, ry) = (-dy / d * v * way, dx / d * v * way);
                me.vx += (rx - me.vx) * 0.5 * dt;
                me.vy += (ry - me.vy) * 0.5 * dt;
                // Too far out, and the giant calls it back.
                if d > FAR { me.vx -= dx / d * 180.0 * dt; me.vy -= dy / d * 180.0 * dt; }
            }
            // A star is pushed by the arrows and pulled by every other
            // star, the big ones most.
            Size::Star => {
                me.vx += (ax * 190.0 + gx - me.vx * 0.9) * dt;
                me.vy += (ay * 190.0 + gy - me.vy * 0.9) * dt;
            }
            // A neutron star turns, and only pushes the way it points.
            Size::Pulsar => {
                me.ang += ax * 3.8 * dt;
                let push = if ay < 0.0 { 210.0 } else { 0.0 };
                me.vx += (me.ang.cos() * push - me.vx * 0.7) * dt;
                me.vy += (me.ang.sin() * push - me.vy * 0.7) * dt;
            }
            // A black hole is heavy: slow to start and slow to stop.
            Size::Hole => {
                me.vx += (ax * 150.0 + gx - me.vx * 1.1) * dt;
                me.vy += (ay * 150.0 + gy - me.vy * 1.1) * dt;
            }
        }
        me.x += me.vx * dt;
        me.y += me.vy * dt;
        match size {
            Size::Dot | Size::Fish | Size::Star | Size::Hole => {
                let (top, floor) = if size == Size::Fish { (20.0, 16.0) } else { (0.0, 0.0) };
                let (x, y) = (me.x.clamp(me.r, W as f32 - me.r), me.y.clamp(top + me.r, H as f32 - me.r - floor));
                if x != me.x { me.vx = 0.0; }
                if y != me.y { me.vy = 0.0; }
                (me.x, me.y) = (x, y);
            }
            // A drop of water has no edge, and neither has empty space:
            // out one side, in the other.
            Size::Cell | Size::Pulsar => {
                me.x = me.x.rem_euclid(W as f32);
                me.y = me.y.rem_euclid(H as f32);
            }
            Size::Moon => {}
        }
        if matches!(size, Size::Moon | Size::Star | Size::Pulsar) {
            self.trail.push((self.me.x, self.me.y));
            if self.trail.len() > 70 { self.trail.remove(0); }
        }
    }

    /// Move everything else.
    fn drift(&mut self, dt: f32) {
        let me = self.me;
        let size = self.size;
        let time = self.time;
        let (wf, hf) = (W as f32, H as f32);
        let mut broken: Vec<usize> = Vec::new();
        for (i, t) in self.things.iter_mut().enumerate() {
            t.age += dt;
            let (dx, dy) = (me.x - t.x, me.y - t.y);
            let d = (dx * dx + dy * dy).sqrt().max(0.1);
            let bigger = t.r > me.r * 1.1;
            match size {
                Size::Dot => {}
                Size::Cell => {
                    // Small ones shy away, big ones come slowly closer,
                    // and the water jostles them all.
                    let pull = if bigger { 24.0 } else if d < 68.0 { -33.0 } else { 0.0 };
                    t.vx += (dx / d * pull + (time * 0.9 + t.phase).sin() * 9.0 - t.vx * 0.8) * dt;
                    t.vy += (dy / d * pull + (time * 0.7 + t.phase * 2.0).cos() * 9.0 - t.vy * 0.8) * dt;
                    t.x = (t.x + t.vx * dt).rem_euclid(wf);
                    t.y = (t.y + t.vy * dt).rem_euclid(hf);
                }
                Size::Fish => {
                    if t.kind == Kind::Sting {
                        t.x += (time * 1.3 + t.phase).sin() * 9.0 * dt;
                        t.y += t.vy * dt * (0.6 + 0.6 * (time * 2.0 + t.phase).sin().max(0.0));
                    } else {
                        if bigger && d < 105.0 {
                            // A big fish that sees the player goes for it.
                            t.vy += (dy / d * 90.0 - t.vy * 2.0) * dt;
                            if dx * t.vx > 0.0 { t.x += t.vx * 0.7 * dt; }
                        } else if !bigger && d < 60.0 {
                            t.vy += (-dy / d * 135.0 - t.vy * 2.0) * dt;
                        } else {
                            t.vy -= t.vy * 2.0 * dt;
                        }
                        t.x += t.vx * dt;
                        t.y = (t.y + t.vy * dt).clamp(24.0, hf - 22.0);
                    }
                }
                Size::Moon => {
                    t.angle += t.turn * dt;
                    (t.x, t.y) = (CX + t.angle.cos() * t.orbit, CY + t.angle.sin() * t.orbit);
                }
                Size::Star => {
                    // The player's pull. A star heavier than the player
                    // hardly feels it.
                    let pull = STAR_PULL * me.r * me.r / (d * d + 900.0) * (me.r / t.r).powi(2).min(1.0);
                    t.vx += (dx / d * pull - t.vx * 0.15) * dt;
                    t.vy += (dy / d * pull - t.vy * 0.15) * dt;
                    // The edges of the picture push back, softly.
                    if t.age > 4.0 {
                        if t.x < 10.0 { t.vx += 40.0 * dt; } else if t.x > wf - 10.0 { t.vx -= 40.0 * dt; }
                        if t.y < 10.0 { t.vy += 40.0 * dt; } else if t.y > hf - 10.0 { t.vy -= 40.0 * dt; }
                    }
                    t.x += t.vx * dt;
                    t.y += t.vy * dt;
                }
                Size::Pulsar => {
                    // Along the beam, either way from the neutron star.
                    let (bx, by) = (me.ang.cos(), me.ang.sin());
                    let (ox, oy) = (wrapped(me.x, t.x, wf), wrapped(me.y, t.y, hf));
                    let along = ox * bx + oy * by;
                    let off = (ox * by - oy * bx).abs();
                    if along.abs() < BEAM + t.r && off < t.r + 2.0 && t.age > 0.6 {
                        t.heat += dt * 1.6;
                        if t.heat >= 1.0 && bigger { broken.push(i); }
                    } else {
                        t.heat = (t.heat - dt * 0.5).max(0.0);
                    }
                    t.x = (t.x + t.vx * dt).rem_euclid(wf);
                    t.y = (t.y + t.vy * dt).rem_euclid(hf);
                }
                Size::Hole => {
                    if t.kind == Kind::Rival {
                        // A bigger hole comes for the player; a smaller one runs.
                        let way = if t.r > me.r { 1.0 } else { -1.0 };
                        // It bends its path a little, and never turns back:
                        // it crosses the picture and is gone.
                        t.vx += dx / d * 7.0 * way * dt;
                        t.vy += dy / d * 7.0 * way * dt;
                        let v = (t.vx * t.vx + t.vy * t.vy).sqrt();
                        if v > 26.0 { t.vx *= 26.0 / v; t.vy *= 26.0 / v; }
                    } else {
                        // A star falls toward the hole, and round it.
                        let pull = HOLE_PULL * me.r * me.r / (d * d + 400.0);
                        t.vx += (dx / d * pull - dy / d * pull * 0.6 - t.vx * 0.3) * dt;
                        t.vy += (dy / d * pull + dx / d * pull * 0.6 - t.vy * 0.3) * dt;
                    }
                    t.x += t.vx * dt;
                    t.y += t.vy * dt;
                }
            }
        }
        // A rock the beam has cracked falls into three, each small
        // enough to eat.
        for &i in broken.iter().rev() {
            let t = self.things[i];
            self.bits.burst(t.x, t.y, 30, 90.0, 0.8, 0xffc070);
            self.rings.push(Ring { x: t.x, y: t.y, t: 0.0, life: 0.5, reach: t.r * 3.0, color: 0xffc070 });
            for k in 0..3 {
                let a = t.phase + k as f32 * TAU / 3.0;
                let piece = Thing { x: t.x + a.cos() * t.r * 0.5, y: t.y + a.sin() * t.r * 0.5, vx: t.vx + a.cos() * 40.0, vy: t.vy + a.sin() * 40.0,
                    r: (t.r * 0.5).min(me.r * 0.85), heat: 0.0, age: 0.3, phase: a, ..t };
                if k == 0 { self.things[i] = piece; } else { self.things.push(piece); }
            }
            self.sound(|s| &s.crack);
        }
        // What has left the picture for good is gone, and a new one comes.
        for i in 0..self.things.len() {
            let t = self.things[i];
            let out = match size {
                Size::Fish if t.kind == Kind::Sting => t.y < -30.0,
                Size::Fish => t.x < -90.0 || t.x > wf + 90.0,
                Size::Star | Size::Hole => t.age > 4.0 && (t.x < -80.0 || t.x > wf + 80.0 || t.y < -80.0 || t.y > hf + 80.0),
                _ => false,
            };
            if out { self.things[i] = self.spawn(i); }
        }
    }

    /// The player meets things: it eats the smaller and is bitten by
    /// the bigger.
    fn meet(&mut self) {
        let (wf, hf) = (W as f32, H as f32);
        let wraps = matches!(self.size, Size::Cell | Size::Pulsar);
        for i in 0..self.things.len() {
            // An eaten dot leaves the list, so the list may end early.
            let Some(&t) = self.things.get(i) else { break };
            if t.age < 0.6 { continue; }
            let (dx, dy) = if wraps { (wrapped(self.me.x, t.x, wf), wrapped(self.me.y, t.y, hf)) } else { (t.x - self.me.x, t.y - self.me.y) };
            let d = (dx * dx + dy * dy).sqrt();
            let smaller = self.me.r > t.r * 1.1;
            let food = match (self.size, t.kind) {
                (Size::Dot, _) => true,
                (_, Kind::Sting) => false,
                // A black hole eats any star, whatever its size.
                (Size::Hole, Kind::Plain) => true,
                _ => smaller,
            };
            if food && d < self.me.r + t.r * 0.3 {
                self.eat(i);
            } else if !food && (t.kind == Kind::Sting || t.r > self.me.r * 1.1) && d < t.r.max(self.me.r) * 0.9 && self.me.safe <= 0.0 {
                self.bitten(dx, dy, d);
            }
        }
        if self.size == Size::Moon {
            let (dx, dy) = (self.me.x - CX, self.me.y - CY);
            if (dx * dx + dy * dy).sqrt() < GIANT + self.me.r * 0.6 && self.me.safe <= 0.0 {
                self.bitten(0.0, 0.0, 1.0);
                if self.mode == Mode::Play { self.to_orbit(); }
            }
        }
    }

    fn eat(&mut self, i: usize) {
        let t = self.things[i];
        self.eaten += 1;
        self.combo = if self.combo_t > 0.0 { self.combo + 1 } else { 0 };
        self.combo_t = 1.6;
        let gain = 10 * self.size.n() * (1 + self.combo.min(7));
        self.score += gain;
        let note = (self.combo as usize).min(self.snd.eat.len() - 1);
        let snd = self.snd.eat[note].clone();
        self.audio.play(&snd, 1.0);
        // The splash: bits fly, a ring spreads, the points float up.
        let splash = match self.size { Size::Dot => WHITE, Size::Star | Size::Hole => 0xffd080, Size::Moon | Size::Pulsar => 0xd0c8c0, _ => t.color };
        self.bits.burst(t.x, t.y, 10 + (t.r * 2.5) as usize + 4 * self.combo.min(7) as usize, 70.0 + t.r * 4.0, 0.7, splash);
        if self.size != Size::Dot {
            self.rings.push(Ring { x: t.x, y: t.y, t: 0.0, life: 0.45, reach: 14.0 + t.r * 2.5, color: splash });
            let text = if self.combo >= 2 { format!("{} X{}", gain, self.combo + 1) } else { gain.to_string() };
            self.pops.push(Pop { x: t.x, y: t.y - t.r - 4.0, t: 0.0, text });
        }
        if self.size == Size::Dot {
            self.things.remove(i);
            return;
        }
        // The meal's area, most of it, joins the player's.
        let k = if t.kind == Kind::Rival { 0.3 } else { 0.62 };
        self.me.r = (self.me.r * self.me.r + t.r * t.r * k).sqrt().min(self.size.goal_r());
        if self.things.len() > self.size.crowd() { self.things.remove(i); } else { self.things[i] = self.spawn(i); }
    }

    fn bitten(&mut self, dx: f32, dy: f32, d: f32) {
        self.lives = self.lives.saturating_sub(1);
        self.sound(|s| &s.bit);
        self.bits.burst(self.me.x, self.me.y, 50, 120.0, 0.9, 0xff6060);
        self.rings.push(Ring { x: self.me.x, y: self.me.y, t: 0.0, life: 0.6, reach: 60.0, color: 0xff6060 });
        self.shake = 0.35;
        self.me.r = (self.me.r * 0.85).max(self.size.start_r());
        self.me.safe = 2.5;
        self.combo = 0;
        if self.size != Size::Moon {
            self.me.vx = -dx / d.max(0.1) * 180.0;
            self.me.vy = -dy / d.max(0.1) * 180.0;
        }
        if self.lives == 0 {
            self.audio.stop(1);
            if !cfg!(test) { funkey::scores::record(GAME, self.score); }
            self.high = self.high.max(self.score);
            (self.mode, self.timer) = (Mode::Over, 0.0);
        }
    }

    /// The size is done: points, a life, and no more music for now.
    fn grown(&mut self) {
        self.score += 500 * self.size.n();
        self.lives = (self.lives + 1).min(5);
        self.audio.stop(1);
    }

    /// Start a pull-back: keep the picture as it is, then set up what
    /// comes next around it.
    fn pull_back(&mut self) {
        let mut snap = std::mem::replace(&mut self.snap, Frame::new(1, 1));
        self.scene(&mut snap, true);
        self.snap = snap;
        self.sound(|s| &s.pull);
        let stage = if self.step == 0 { self.size.next() } else { None };
        match stage {
            Some(next) => {
                self.grown();
                self.enter(next);
                self.pull = Pull { t: 0.0, x: self.me.x, y: self.me.y, r: self.me.r, with_me: false };
                // The moon and its giant end as a speck beside the star.
                if next == Size::Star { self.pull = Pull { t: 0.0, x: self.me.x + self.me.r + 9.0, y: self.me.y, r: 1.5, with_me: true }; }
            }
            // Past the black hole the picture pulls back three times
            // more, and nobody is asked to play.
            None => {
                if self.step == 0 { self.grown(); }
                self.step += 1;
                self.things.clear();
                self.bits.list.clear();
                (self.view, self.pull) = match self.step {
                    1 => (View::Galaxy, Pull { t: 0.0, x: CX, y: CY, r: 2.0, with_me: false }),
                    2 => (View::Web, Pull { t: 0.0, x: 300.0, y: 126.0, r: 2.0, with_me: false }),
                    _ => (View::Void, Pull { t: 0.0, x: CX, y: CY, r: 3.0, with_me: false }),
                };
            }
        }
        (self.mode, self.timer) = (Mode::Pull, 0.0);
    }

    fn step(&mut self, input: &Input, dt: f32) {
        self.time += dt;
        self.timer += dt;
        self.shake -= dt;
        // In a black hole's world the bits fall into the hole too.
        if self.size == Size::Hole && self.view == View::Stage(Size::Hole) {
            let me = self.me;
            for p in &mut self.bits.list {
                let (dx, dy) = (me.x - p.x, me.y - p.y);
                let d = (dx * dx + dy * dy).sqrt().max(4.0);
                p.vx += (dx / d * 260.0 - dy / d * 120.0 - p.vx * 1.5) * dt;
                p.vy += (dy / d * 260.0 + dx / d * 120.0 - p.vy * 1.5) * dt;
            }
        }
        self.bits.update(dt);
        self.pops.retain_mut(|p| { p.t += dt; p.y -= 22.0 * dt; p.t < 0.9 });
        self.rings.retain_mut(|r| { r.t += dt; r.t < r.life });
        match self.mode {
            Mode::Title => {}
            Mode::Play => {
                self.total_t += dt;
                self.stage_t += dt;
                self.combo_t -= dt;
                self.me.safe -= dt;
                let (ax, ay) = self.arrows(input);
                self.steer(ax, ay, dt);
                self.drift(dt);
                self.meet();
                if self.mode != Mode::Play { return; }
                let done = match self.size { Size::Dot => self.things.is_empty(), s => self.me.r >= s.goal_r() };
                if !done { self.ambience(dt); }
                if self.mode != Mode::Play { return; }
                if done {
                    match self.size {
                        Size::Star => {
                            self.grown();
                            self.sound(|s| &s.nova);
                            (self.mode, self.timer) = (Mode::Nova, 0.0);
                        }
                        Size::Pulsar => {
                            self.grown();
                            self.sound(|s| &s.fall);
                            (self.mode, self.timer) = (Mode::Fall, 0.0);
                        }
                        _ => self.pull_back(),
                    }
                }
            }
            Mode::Pull => {
                self.pull.t += dt;
                if self.pull.t >= PULL {
                    if self.step == 0 {
                        (self.mode, self.timer) = (Mode::Play, 0.0);
                        self.music();
                    } else if self.step < 3 {
                        self.pull_back();
                    } else {
                        (self.mode, self.timer) = (Mode::End, 0.0);
                    }
                }
            }
            // The star swells, then bursts, and a neutron star is left
            // where it stood.
            Mode::Nova => {
                if self.timer - dt < NOVA_BANG && self.timer >= NOVA_BANG {
                    let (x, y) = (self.me.x, self.me.y);
                    self.enter(Size::Pulsar);
                    (self.me.x, self.me.y) = (x, y);
                    for i in 0..700 {
                        let a = self.rng.range(0.0, TAU);
                        let s = self.rng.range(40.0, 420.0);
                        let color = *[0xfff8e0, 0xffd070, 0xff9040, 0xa0d0ff].get(i % 4).unwrap_or(&WHITE);
                        let life = self.rng.range(1.0, 3.2);
                        self.bits.spawn(Particle { x, y, vx: a.cos() * s, vy: a.sin() * s, life, color, size: 1 + (i % 3 == 0) as i32 });
                    }
                }
                if self.timer >= NOVA {
                    (self.mode, self.timer) = (Mode::Play, 0.0);
                    self.music();
                }
            }
            // The neutron star draws everything in, and falls in on itself.
            Mode::Fall => {
                if self.timer < FALL_DONE {
                    let me = self.me;
                    for t in &mut self.things {
                        t.x += (me.x - t.x) * 2.5 * dt;
                        t.y += (me.y - t.y) * 2.5 * dt;
                        t.r = (t.r - 4.0 * dt).max(0.5);
                    }
                    self.me.ang += 14.0 * dt;
                }
                if self.timer - dt < FALL_DONE && self.timer >= FALL_DONE {
                    let (x, y) = (self.me.x.clamp(60.0, W as f32 - 60.0), self.me.y.clamp(60.0, H as f32 - 60.0));
                    self.enter(Size::Hole);
                    (self.me.x, self.me.y) = (x, y);
                    self.rings.push(Ring { x, y, t: 0.0, life: 1.6, reach: 330.0, color: 0x8060ff });
                }
                if self.timer >= FALL {
                    (self.mode, self.timer) = (Mode::Play, 0.0);
                    self.music();
                }
            }
            // The last joke plays by the clock; see `ending`.
            Mode::End => {
                if self.timer - dt < 3.0 && self.timer >= 3.0 { self.sound(|s| &s.big); }
                if self.timer >= 10.0 {
                    let fast = (6000.0 - self.total_t * 6.0).max(0.0) as u32;
                    self.score += self.lives * 1000 + fast;
                    if !cfg!(test) { funkey::scores::record(GAME, self.score); }
                    self.high = self.high.max(self.score);
                    self.sound(|s| &s.end);
                    (self.mode, self.timer) = (Mode::Won, 0.0);
                }
            }
            Mode::Over | Mode::Won => if self.timer > 12.0 { self.mode = Mode::Title; },
        }
    }

    /// What a size does by itself: bubbles, sparks, and a black hole
    /// that fades when it is not fed.
    fn ambience(&mut self, dt: f32) {
        let me = self.me;
        match self.size {
            Size::Fish => {
                if self.rng.chance(dt * 9.0) {
                    let x = self.rng.range(0.0, W as f32);
                    let (vx, vy) = (self.rng.range(-4.0, 4.0), -self.rng.range(18.0, 40.0));
                    self.bits.spawn(Particle { x, y: H as f32 - 14.0, vx, vy, life: 7.0, color: 0x90d0f0, size: 1 });
                }
                // The swimmer leaves bubbles behind when it hurries.
                if me.vx.abs() + me.vy.abs() > 90.0 && self.rng.chance(dt * 20.0) {
                    self.bits.spawn(Particle { x: me.x - me.face * me.r, y: me.y, vx: -me.vx * 0.2, vy: -18.0, life: 1.2, color: 0xc0f0ff, size: 1 });
                }
            }
            Size::Star => {
                // Small stars near the player bleed light into it.
                for i in 0..self.things.len() {
                    let t = self.things[i];
                    let (dx, dy) = (me.x - t.x, me.y - t.y);
                    let d = (dx * dx + dy * dy).sqrt().max(1.0);
                    if me.r > t.r * 1.1 && d < 90.0 && self.rng.chance(dt * 25.0) {
                        self.bits.spawn(Particle { x: t.x, y: t.y, vx: dx / d * 110.0, vy: dy / d * 110.0, life: d / 110.0, color: 0xffc060, size: 1 });
                    }
                }
            }
            Size::Pulsar => {
                // Sparks fly off both ends of the beam.
                if self.rng.chance(dt * 40.0) {
                    let way = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
                    let far = self.rng.range(me.r, BEAM);
                    let side = self.rng.range(-30.0, 30.0);
                    self.bits.spawn(Particle { x: me.x + me.ang.cos() * far * way, y: me.y + me.ang.sin() * far * way,
                        vx: -me.ang.sin() * side, vy: me.ang.cos() * side, life: 0.4, color: 0x80e0ff, size: 1 });
                }
            }
            Size::Hole => {
                // It gives off a little of itself all the time.
                self.me.r -= 0.15 * dt;
                if self.me.r < Size::Hole.start_r() * 0.75 {
                    self.bitten(0.0, 0.0, 1.0);
                    self.me.r = Size::Hole.start_r();
                }
            }
            _ => {}
        }
    }

    // ---- pictures ----

    /// The picture of the view, with no score or words on it. `with_me`
    /// is false while a shrinking disc stands in for the player.
    fn scene(&self, f: &mut Frame, with_me: bool) {
        match self.view {
            View::Void | View::Stage(Size::Dot) => {
                f.clear(BLACK);
                for t in &self.things { blend_disc(f, t.x, t.y, 2.0, WHITE, 1.0); }
                if with_me { blend_disc(f, self.me.x, self.me.y, 3.0, WHITE, 1.0); }
            }
            View::Stage(Size::Cell) => self.pond(f, with_me),
            View::Stage(Size::Fish) => self.sea(f, with_me),
            View::Stage(Size::Moon) => self.space(f, with_me),
            View::Stage(Size::Star) => self.cluster(f, with_me),
            View::Stage(Size::Pulsar) => self.wreck(f, with_me),
            View::Stage(Size::Hole) => self.abyss(f, with_me),
            View::Galaxy => galaxy(f, self.time),
            View::Web => web(f),
        }
        for p in &self.bits.list {
            let k = (p.life * 2.0).min(1.0);
            match self.view {
                View::Stage(Size::Dot) | View::Void => f.rect((p.x * SF) as i32, (p.y * SF) as i32, S, S, p.color),
                // In the sea the bits are bubbles.
                View::Stage(Size::Fish) => bubble(f, p.x, p.y, 0.7 + p.size as f32 * 0.5, k),
                _ => {
                    spot(f, p.x, p.y, p.color, k);
                    if p.size > 1 { glow(f, p.x, p.y, 3.2, p.color, 0.5 * k); }
                }
            }
        }
        for r in &self.rings {
            let k = r.t / r.life;
            let far = 4.0 + r.reach * smooth(k);
            circle_of_light(f, r.x, r.y, far, r.color, (1.0 - k) * 0.8);
            circle_of_light(f, r.x, r.y, far * 0.8, r.color, (1.0 - k) * 0.3);
        }
    }

    /// How much of the player shows: it blinks while nothing can bite.
    fn shown(&self) -> f32 {
        if self.mode == Mode::Play && self.me.safe > 0.0 && (self.time * 8.0) as i32 % 2 == 0 { 0.35 } else { 1.0 }
    }

    /// Whether a thing is big enough to bite.
    fn danger(&self, t: &Thing) -> bool { t.r > self.me.r * 1.1 }

    /// The stars' sky, with a slow twinkle.
    fn night(&self, f: &mut Frame) {
        f.px.copy_from_slice(&self.sky.px);
        for i in 0..110 {
            let (x, y) = (hash(i, 0, 77) * W as f32, hash(i, 1, 77) * H as f32);
            spot(f, x, y, WHITE, 0.3 + 0.3 * (self.time * (1.0 + hash(i, 2, 77) * 2.0) + i as f32).sin());
        }
    }

    /// The player's path behind it, fading.
    fn wake(&self, f: &mut Frame, c: Rgb, wide: f32) {
        let n = self.trail.len();
        for (i, &(x, y)) in self.trail.iter().enumerate() {
            let k = i as f32 / n.max(1) as f32;
            if wide > 1.5 { glow(f, x, y, wide * k, c, 0.12 * k); } else { spot(f, x, y, c, 0.4 * k); }
        }
    }

    /// A drop of pond water under the microscope.
    fn pond(&self, f: &mut Frame, with_me: bool) {
        let t = self.time;
        // The water is worked out for every second unit of the world
        // each way, and smoothed up to the picture: light in water has
        // no hard edges.
        let g = 2 * S;
        let mut water = Frame::new(W / 2 + 2, H / 2 + 2);
        rows(&mut water, |y, row| {
            let wy = (y * 2) as f32;
            for (x, p) in row.iter_mut().enumerate() {
                let wx = (x * 2) as f32;
                let n = noise(wx / 60.0 + t * 0.04, wy / 60.0, 64, 3);
                // Light from above is broken by the water's skin into
                // two nets that slide over each other.
                let a = (1.0 - (noise(wx / 27.0 - t * 0.12, wy / 27.0 + t * 0.09, 64, 8) - 0.5).abs() * 5.5).max(0.0);
                let b = (1.0 - (noise(wx / 17.0 + t * 0.1, wy / 17.0 - t * 0.07, 64, 9) - 0.5).abs() * 5.5).max(0.0);
                let net = a * a * 0.3 + a * b * 0.55;
                // The glass darkens toward its edges.
                let (ex, ey) = ((wx - CX) / CX, (wy - CY) / CY);
                *p = lay(lit(lay(0x041c22, 0x0f5046, n), 0x70f0c0, net * 0.42), 0x01080a, (ex * ex + ey * ey) * 0.32);
            }
        });
        let (water, gw) = (&water.px, water.w as usize);
        rows(f, |y, row| {
            let (gy, kv) = ((y / g) as usize, (y % g * 256 / g) as u32);
            for (x, p) in row.iter_mut().enumerate() {
                let (gx, ku) = (x / g as usize, (x as i32 % g * 256 / g) as u32);
                let i = gy * gw + gx;
                *p = lay_k(lay_k(water[i], water[i + 1], ku), lay_k(water[i + gw], water[i + gw + 1], ku), kv);
            }
        });
        // Far cells, out of focus.
        for i in 0..12 {
            let deep = 0.4 + hash(i, 2, 14) * 0.6;
            let x = (hash(i, 0, 14) * W as f32 + t * 2.5 * deep).rem_euclid(W as f32 + 60.0) - 30.0;
            let y = hash(i, 1, 14) * H as f32 + (t * 0.2 + i as f32).sin() * 6.0;
            soft(f, x, y, 9.0 + hash(i, 3, 14) * 16.0, lay(0x1c7458, 0x4a8440, hash(i, 4, 14)), 0.2 * deep);
        }
        // Specks adrift, some near, some far.
        for i in 0..170 {
            let deep = 0.3 + hash(i, 2, 13) * 0.7;
            let x = (hash(i, 0, 13) * W as f32 + t * 6.0 * deep).rem_euclid(W as f32);
            let y = (hash(i, 1, 13) * H as f32 + (t * 0.5 + i as f32).sin() * 4.0).rem_euclid(H as f32);
            spot(f, x, y, 0x80ffd0, 0.22 * deep);
        }
        for th in &self.things {
            let a = (th.age * 2.0).min(1.0);
            let c = if self.danger(th) { mix(th.color, 0xe83030, 0.5) } else { th.color };
            blob(f, th.x, th.y, th.r, c, a, t * 2.0 + th.phase, self.danger(th));
        }
        if with_me {
            glow(f, self.me.x, self.me.y, self.me.r * 2.6, 0xffffa0, 0.32 * self.shown());
            blob(f, self.me.x, self.me.y, self.me.r, 0xfff8a0, self.shown(), t * 3.0, false);
        }
    }

    /// The sea, seen from the side.
    fn sea(&self, f: &mut Frame, with_me: bool) {
        let t = self.time;
        // Light comes down in slanted, slowly moving bands: wide ones
        // and fine ones. A band's strength is worked out once for each
        // slanted line, not for each pixel.
        let rays: Vec<f32> = (0..RW + RH).map(|i| {
            let u = i as f32 / SF;
            (noise(u / 42.0 + t * 0.08, 0.5, 32, 5) - 0.5).max(0.0) * 0.75 + (noise(u / 10.0 - t * 0.06, 3.5, 64, 15) - 0.58).max(0.0) * 0.4
        }).collect();
        // For each column: where the surface is and how it glitters,
        // the two rows of far hills, and the top of the sand.
        let cols: Vec<(f32, f32, i32, i32, i32)> = (0..RW).map(|x| {
            let wx = x as f32 / SF;
            let surf = 3.0 + 2.5 * noise(wx / 9.0 + t * 0.8, t * 0.3, 64, 6) + 1.2 * (wx / 14.0 + t * 1.5).sin();
            let glint = noise(wx / 3.0 + t * 1.4, t * 0.5, 64, 16);
            let far = H as f32 - 44.0 - noise(wx / 95.0, 7.0, 64, 12) * 58.0 - noise(wx / 17.0, 7.0, 64, 12) * 6.0;
            let near = H as f32 - 24.0 - noise(wx / 70.0, 3.0, 64, 11) * 40.0 - noise(wx / 11.0, 3.0, 64, 11) * 4.0;
            let top = H as f32 - 12.0 - noise(wx / 36.0, 0.0, 64, 9) * 8.0;
            (surf * SF, glint, (far * SF) as i32, (near * SF) as i32, (top * SF) as i32)
        }).collect();
        rows(f, |y, row| {
            let deep = y as f32 / RH as f32;
            let water = lay(0x2a90c0, 0x04122c, deep.powf(0.8));
            let fade = (1.0 - deep) * (1.0 - deep);
            let wy = y as f32 / SF;
            for (x, p) in row.iter_mut().enumerate() {
                let (surf, glint, far, near, top) = cols[x];
                if (y as f32) < surf { *p = lay(0x58b8e4, 0xf0ffff, glint * glint); continue; }
                let mut c = lit(water, 0xc0f0ff, rays[x + y as usize * 3 / 5] * fade);
                // Right under the surface the water is bright.
                let high = 1.0 - (y as f32 - surf) / (16.0 * SF);
                if high > 0.0 { c = lit(c, 0x90e0ff, high * high * 0.4); }
                if y >= far { c = lay(c, 0x0a2c48, 0.38); }
                if y >= near { c = lay(c, 0x04122a, 0.55); }
                if y >= top {
                    // Sand: ripples, grains, and light that dances on it.
                    let d = (y - top) as f32 / SF;
                    let wx = x as f32 / SF;
                    let ripple = 0.5 + 0.5 * (d * 1.6 + noise(wx / 12.0, 0.0, 64, 17) * 6.0).sin();
                    let dance = (noise(wx / 9.0 + t * 0.35, wy / 4.0 - t * 0.2, 64, 18) - 0.55).max(0.0);
                    c = lay(0xb09c68, 0x54462c, (d / 14.0 + hash(x as i32, y, 2) * 0.14 + ripple * 0.12).min(1.0));
                    c = lit(c, 0xfff0c0, dance * 0.9);
                    if d < 1.0 { c = lit(c, WHITE, 0.14); }
                }
                *p = c;
            }
        });
        // A school of fish far off.
        for i in 0..11 {
            let way = if i % 3 == 0 { -1.0 } else { 1.0 };
            let x = (hash(i, 0, 23) * W as f32 + way * t * (5.0 + hash(i, 2, 23) * 5.0)).rem_euclid(W as f32 + 40.0) - 20.0;
            let y = 50.0 + hash(i, 1, 23) * 150.0 + (t * 0.4 + i as f32).sin() * 5.0;
            fish(f, x, y, 2.5 + hash(i, 3, 23) * 2.5, way, 0x0a2c48, false, 0.4, t * 6.0 + i as f32);
        }
        // Weed that sways, each blade wide at the root and thin at the tip.
        for i in 0..38 {
            let plant = i / 2;
            let wx = (hash(plant, 0, 19) * W as f32 + (i % 2) as f32 * 3.0).min(W as f32 - 1.0);
            let x0 = (wx * SF) as i32;
            let tall = ((26.0 + hash(i, 1, 19) * 74.0) * SF) as i32;
            let root = cols[x0.clamp(0, RW - 1) as usize].4 + S;
            let green = lay(0x14482c, 0x2c8850, hash(i, 2, 19));
            for k in 0..tall {
                let s = k as f32 / tall as f32;
                let sway = ((t * 1.2 + plant as f32 + i as f32 * 0.4 + k as f32 * 0.035).sin() * k as f32 * 0.12) as i32;
                let wide = 1 + ((1.0 - s) * 2.6) as i32;
                let x = x0 + sway - wide / 2;
                f.rect(x, root - k, wide, 1, tint(green, 0.7 + 0.5 * s));
                f.put(x, root - k, tint(green, 0.5));
                // Leaves, left and right in turn.
                let step = 9 * S;
                if k % step < 4 * S && k > step && wide > 1 {
                    let out = (k % step) + 1;
                    let side = if (k / step) % 2 == 0 { -1 } else { 1 };
                    f.rect(x + if side < 0 { -out } else { wide }, root - k - out / 2, out, 1, tint(green, 1.15));
                }
            }
        }
        // Bubbles on their way up.
        for i in 0..26 {
            let speed = 9.0 + hash(i, 2, 27) * 14.0;
            let y = H as f32 - (t * speed + hash(i, 1, 27) * H as f32).rem_euclid(H as f32);
            let x = hash(i, 0, 27) * W as f32 + (t * 1.5 + i as f32).sin() * 2.5;
            bubble(f, x, y, 0.6 + hash(i, 3, 27) * 1.3, 0.55);
        }
        for th in &self.things {
            let a = (th.age * 2.0).min(1.0);
            if th.kind == Kind::Sting {
                glow(f, th.x, th.y, th.r * 2.6, th.color, 0.22);
                jelly(f, th.x, th.y, th.r, th.color, t * 3.0 + th.phase);
            } else {
                let danger = self.danger(th);
                fish(f, th.x, th.y, th.r, th.vx.signum(), if danger { mix(th.color, 0x602020, 0.6) } else { th.color }, danger, a, t * 8.0 + th.phase);
            }
        }
        if with_me { fish(f, self.me.x, self.me.y, self.me.r, self.me.face, 0xff9030, false, self.shown(), t * 10.0); }
    }

    /// The giant, its rocks and the moon.
    fn space(&self, f: &mut Frame, with_me: bool) {
        self.night(f);
        giant(f, self.time);
        if with_me { self.wake(f, 0x60b0ff, 1.0); }
        for t in &self.things {
            let a = (t.age * 2.0).min(1.0);
            let c = if self.danger(t) { mix(t.color, 0xd04030, 0.5) } else { t.color };
            rock(f, t.x, t.y, t.r, c, a, (t.phase * 100.0) as i32);
        }
        if with_me { planet(f, self.me.x, self.me.y, self.me.r, self.shown(), self.time); }
    }

    /// A crowd of stars.
    fn cluster(&self, f: &mut Frame, with_me: bool) {
        self.night(f);
        if with_me { self.wake(f, 0xffe090, 7.0); }
        for t in &self.things {
            let a = (t.age * 2.0).min(1.0);
            // Small stars burn red, big ones blue.
            let k = (t.r / self.me.r - 0.4).clamp(0.0, 1.4) / 1.4;
            let c = if k < 0.5 { mix(0xff5030, 0xffd060, k * 2.0) } else { mix(0xffd060, 0x90c0ff, k * 2.0 - 1.0) };
            star(f, t.x, t.y, t.r, c, a, self.time + t.phase);
        }
        if with_me {
            let (x, y, r, a) = (self.me.x, self.me.y, self.me.r, self.shown());
            star(f, x, y, r, 0xfff0a0, a, self.time);
            // The speck that goes round it used to be a whole game.
            let th = self.stage_t * 1.2;
            circle_of_light(f, x, y, r + 9.0, 0x8098c0, 0.1 * a);
            spot(f, x + th.cos() * (r + 9.0), y + th.sin() * (r + 9.0), 0xc0d8ff, a);
        }
    }

    /// What a burst star leaves: a cloud, rubble, and a neutron star
    /// with its beam.
    fn wreck(&self, f: &mut Frame, with_me: bool) {
        self.night(f);
        for t in &self.things {
            let a = (t.age * 2.0).min(1.0);
            let c = if self.danger(t) { mix(t.color, 0xd04030, 0.45) } else { t.color };
            if t.heat > 0.0 { glow(f, t.x, t.y, t.r * 2.2, 0xff8030, t.heat.min(1.0) * 0.8); }
            rock(f, t.x, t.y, t.r, mix(c, 0xffb050, t.heat.min(1.0) * 0.7), a, (t.phase * 100.0) as i32);
        }
        if with_me {
            let (x, y, r, a) = (self.me.x, self.me.y, self.me.r, self.shown());
            self.wake(f, 0x80c0ff, 4.0);
            let (bx, by) = (self.me.ang.cos(), self.me.ang.sin());
            // The lines of its field: loops from one pole to the other.
            for (n, reach) in [2.4, 3.6, 5.0].into_iter().enumerate() {
                let steps = (reach * r * 5.0) as i32;
                for i in 1..steps {
                    let th = i as f32 / steps as f32 * PI;
                    let far = reach * r * th.sin() * th.sin();
                    let (along, across) = (far * th.cos(), far * th.sin());
                    let k = (0.2 - n as f32 * 0.04) * (0.7 + 0.3 * (self.time * 3.0 - th * 4.0 + n as f32).sin()) * a;
                    for side in [-1.0, 1.0] {
                        spot(f, x + bx * along - by * across * side, y + by * along + bx * across * side, 0x70b0ff, k);
                    }
                }
            }
            // The beam, both ways: a hot thread in a cone of light that
            // widens and thins toward its ends.
            let mut far = r;
            while far < BEAM {
                let k = 1.0 - far / BEAM;
                let flicker = 0.75 + 0.25 * (self.time * 30.0 + far * 0.3).sin();
                for way in [-1.0, 1.0] {
                    let (px, py) = (x + bx * far * way, y + by * far * way);
                    spot(f, px, py, 0xe0f8ff, k * flicker * a);
                    glow(f, px, py, 1.6 + far * 0.045, 0x40a0ff, 0.3 * k * flicker * a);
                }
                far += 0.6;
            }
            glow(f, x, y, r * 4.5, 0x4080ff, 0.35 * a);
            glow(f, x, y, r * 2.2, 0xa0d0ff, 0.7 * a);
            blend_disc(f, x, y, r, 0xc8e4ff, a);
            blend_disc(f, x, y, r * 0.7, WHITE, a);
        }
    }

    /// Stars falling into a black hole.
    fn abyss(&self, f: &mut Frame, with_me: bool) {
        self.night(f);
        for t in self.things.iter().filter(|t| t.kind == Kind::Plain) {
            star(f, t.x, t.y, t.r * 0.8, t.color, (t.age * 2.0).min(1.0), self.time + t.phase);
        }
        for t in self.things.iter().filter(|t| t.kind == Kind::Rival) {
            let c = if self.danger(t) { 0xff5040 } else { t.color };
            hole(f, t.x, t.y, t.r, c, -self.time * 1.5 + t.phase, (t.age * 2.0).min(1.0));
        }
        if with_me { hole(f, self.me.x, self.me.y, self.me.r, 0xffb050, self.time * 2.0, self.shown()); }
    }

    /// The old picture as a disc on its way to being one small thing.
    fn disc(&self, f: &mut Frame) {
        let p = self.pull;
        let k = (p.t / PULL).clamp(0.0, 1.0);
        let e = smooth(k);
        let cover = COVER * SF;
        let r = cover * (p.r / COVER).powf(e);
        let (cx, cy) = ((CX + (p.x - CX) * e) * SF, (CY + (p.y - CY) * e) * SF);
        let fade = ((k - 0.82) / 0.18).clamp(0.0, 1.0);
        let scale = cover / r;
        let (mx, my) = (CX * SF, CY * SF);
        let (x0, x1) = (((cx - r).floor() as i32).max(0), ((cx + r).ceil() as i32).min(RW - 1));
        let snap = &self.snap;
        rows(f, |y, row| {
            let dy = y as f32 + 0.5 - cy;
            if dy.abs() > r { return; }
            for x in x0..=x1 {
                let dx = x as f32 + 0.5 - cx;
                let d2 = dx * dx + dy * dy;
                if d2 > r * r { continue; }
                // A shrunken picture is the mean of four spots, or it
                // would flicker as it goes.
                let c = if scale < 1.5 {
                    mirrored(snap, mx + dx * scale, my + dy * scale)
                } else {
                    let mut sum = (0u32, 0u32, 0u32);
                    for (ox, oy) in [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)] {
                        let c = mirrored(snap, mx + (dx + ox) * scale, my + (dy + oy) * scale);
                        sum = (sum.0 + (c >> 16 & 255), sum.1 + (c >> 8 & 255), sum.2 + (c & 255));
                    }
                    ((sum.0 / 4) << 16) | ((sum.1 / 4) << 8) | (sum.2 / 4)
                };
                // A pale rim, so that a dark picture shows against a dark one.
                let edge = r - d2.sqrt();
                let rim = if edge < 1.5 * SF { 0.4 } else { 0.0 };
                let i = x as usize;
                row[i] = lay(lay(lay(c, WHITE, rim), row[i], fade), row[i], 1.0 - edge.min(1.0));
            }
        });
    }

    /// Two lines of words low in the picture.
    fn say(&self, f: &mut Frame, a: &str, b: &str, show: f32) {
        if show <= 0.0 { return; }
        let k = show.min(1.0);
        f.text_centered(RW / 2 + S, (H - 61) * S + S, a, BLACK, true, 2 * S);
        f.text_centered(RW / 2, (H - 62) * S, a, mix(BLACK, WHITE, k), true, 2 * S);
        f.text_centered(RW / 2 + S, (H - 37) * S + 1, b, BLACK, true, S);
        f.text_centered(RW / 2, (H - 38) * S, b, mix(BLACK, 0xa0e0ff, k), true, S);
    }

    fn hud(&self, f: &mut Frame) {
        // A dot has no score to look at. That comes with the cell.
        if self.size == Size::Dot { return; }
        let score = format!("{:06}", self.score);
        f.text_scaled(7 * S, 6 * S, &score, BLACK, true, S);
        f.text_scaled(6 * S, 5 * S, &score, WHITE, true, S);
        for k in 0..self.lives as i32 {
            let x = (W - 11 - k * 11) as f32;
            blend_disc(f, x + 1.0, 9.0, 4.0, BLACK, 0.8);
            blend_disc(f, x, 8.0, 3.2, 0xff6060, 1.0);
            blend_disc(f, x - 1.0, 7.0, 1.0, 0xffc0c0, 1.0);
        }
        let (from, to) = (self.size.start_r(), self.size.goal_r());
        let done = ((self.me.r - from) / (to - from)).clamp(0.0, 1.0);
        let name = self.size.name();
        f.text_scaled((W / 2 - 52) * S - Frame::text_width(name, true, S), 5 * S, name, 0xc0c0d8, true, S);
        f.rect((W / 2 - 46) * S, 5 * S, 94 * S, 7 * S, 0x202030);
        f.rect((W / 2 - 45) * S, 6 * S, (92.0 * SF * done) as i32, 5 * S, mix(0x40c070, 0xffe060, done));
        f.rect((W / 2 - 45) * S, 6 * S, (92.0 * SF * done) as i32, S, mix(0x90e0a0, 0xfff0a0, done));
        for p in &self.pops {
            let c = mix(0xffff80, 0xff8040, p.t);
            let (x, y) = ((p.x * SF) as i32, (p.y * SF) as i32);
            f.text_centered(x + S, y + S, &p.text, BLACK, true, S);
            f.text_centered(x, y, &p.text, c, true, S);
        }
    }

    fn title(&self, f: &mut Frame) {
        f.clear(BLACK);
        f.text_centered(RW / 2, 66 * S, "BIGGER", WHITE, true, 6 * S);
        blend_disc(f, CX, 148.0, 3.0, WHITE, 1.0);
        f.text_centered(RW / 2, 172 * S, "A GAME ABOUT A DOT", 0xa0a0b8, true, S);
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(RW / 2, 208 * S, "PRESS SPACE", WHITE, true, 2 * S); }
        f.text_centered(RW / 2, 250 * S, "ARROWS STEER  P PAUSES  Q QUITS", 0x707088, true, S);
        if self.high > 0 { f.text_centered(RW / 2, 268 * S, &format!("BEST {}", self.high), 0x707088, true, S); }
        f.text_scaled(RW - 4 * S - Frame::text_width(VERSION, false, S), (H - 7) * S, VERSION, 0x404050, false, S);
    }

    /// The end: the dot alone, then a bigger dot.
    fn ending(&self, f: &mut Frame) {
        f.clear(BLACK);
        let t = self.timer;
        // The big one rolls in from the right and stops on the small one.
        let k = smooth((t - 3.0) / 3.0);
        let bx = W as f32 + 100.0 - (W as f32 + 100.0 - CX - 45.0) * k;
        if bx - 90.0 > CX + 3.0 || t < 3.0 { blend_disc(f, CX, CY, 3.0, WHITE, 1.0); }
        if t >= 3.0 { blend_disc(f, bx, CY, 90.0, WHITE, 1.0); }
        if t < 3.0 {
            self.say(f, "YOU ARE A DOT.", "", t - 0.6);
        } else if t > 6.6 {
            f.text_centered(RW / 2, (H - 34) * S, "THERE IS ALWAYS A BIGGER DOT.", WHITE, true, 2 * S);
        }
    }
}

/// A bubble: a thin ring with a glint.
fn bubble(f: &mut Frame, x: f32, y: f32, r: f32, a: f32) {
    circle_of_light(f, x, y, r, 0xc0f0ff, 0.5 * a);
    spot(f, x - r * 0.4, y - r * 0.4, WHITE, 0.6 * a);
}

/// A cell: a skin that wobbles, a grainy body you can see through, a
/// core and small parts. A hunter bristles and has hairs that beat.
#[allow(clippy::too_many_arguments)]
fn blob(f: &mut Frame, x: f32, y: f32, r: f32, c: Rgb, a: f32, wob: f32, spiky: bool) {
    let (cx, cy, pr) = (x * SF, y * SF, r * SF);
    let (bx, by) = (cx.floor() as i32, cy.floor() as i32);
    let skin = lay(c, WHITE, 0.45);
    let grainy = (c & 0xff) as f32;
    let edge_at = |th: f32| {
        let mut e = pr * (1.0 + 0.07 * (3.0 * th + wob).sin() + 0.05 * (5.0 * th - wob * 1.7).sin() + 0.02 * (9.0 * th + wob * 2.3).sin());
        if spiky { e += pr * 0.12 * (11.0 * th + wob * 0.5).sin().max(0.0).powi(2); }
        e
    };
    if spiky && r > 3.0 {
        // Hairs round the skin.
        for i in 0..26 {
            let th = i as f32 / 26.0 * TAU;
            let from = edge_at(th);
            let long = pr * 0.24;
            let mut far = 0.0;
            while far < long {
                let bend = (wob * 3.0 + i as f32 * 0.9).sin() * 0.35 * far / long;
                add(f, (cx + (th + bend).cos() * (from + far)) as i32, (cy + (th + bend).sin() * (from + far)) as i32, skin, 0.5 * a * (1.0 - far / long));
                far += 1.0;
            }
        }
    }
    // Where the skin is, for 256 ways out from the middle.
    let mut skin_at = [0.0f32; 257];
    for (i, e) in skin_at.iter_mut().enumerate() { *e = edge_at(i as f32 / 256.0 * TAU - PI); }
    let reach = (pr * 1.25).ceil() as i32 + 1;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (fx, fy) = ((bx + dx) as f32 + 0.5 - cx, (by + dy) as f32 + 0.5 - cy);
            let d = (fx * fx + fy * fy).sqrt();
            if d > pr * 1.25 { continue; }
            let at = (fy.atan2(fx) + PI) / TAU * 256.0;
            let (i, part) = ((at as usize).min(255), at.fract());
            let edge = skin_at[i] * (1.0 - part) + skin_at[i + 1] * part;
            if d > edge { continue; }
            let k = d / edge;
            let (px, py) = (bx + dx, by + dy);
            let under = f.get(px, py);
            // A bright skin, a dark line inside it, then the body.
            let (mut col, al) = if k > 0.9 || edge - d < 1.6 {
                (skin, 0.95)
            } else if k > 0.82 {
                (tint(c, 0.4), 0.8)
            } else {
                let grain = noise(fx / pr * 3.5 + grainy, fy / pr * 3.5 + wob * 0.05, 64, 61);
                (tint(c, 0.4 + 0.3 * k + 0.3 * grain), 0.68)
            };
            // Light falls on it from the upper left.
            let sheen = ((-fx - fy) / (pr * 1.41)).max(0.0) * k;
            col = lit(col, WHITE, sheen * sheen * 0.4);
            f.put(px, py, lay(under, col, a * al * (edge - d).min(1.0)));
        }
    }
    if r > 3.0 {
        // The core, with a core of its own.
        soft(f, x + r * 0.15, y - r * 0.1, r * 0.46, tint(c, 0.2), 0.95 * a);
        blend_disc(f, x + r * 0.15, y - r * 0.1, r * 0.3, tint(c, 0.3), 0.7 * a);
        blend_disc(f, x + r * 0.09, y - r * 0.16, r * 0.11, lay(c, WHITE, 0.5), 0.85 * a);
    }
    if r > 5.0 {
        let n = 4 + (r / 3.0) as i32;
        for i in 0..n {
            let th = i as f32 * 2.4 + wob * 0.15;
            let far = r * (0.5 + 0.22 * hash(i, 7, 62));
            let (ox, oy) = (x + th.cos() * far, y + th.sin() * far);
            if i % 3 == 0 {
                // A bubble of sap.
                soft(f, ox, oy, r * 0.15, lay(c, WHITE, 0.7), 0.35 * a);
                circle_of_light(f, ox, oy, r * 0.13, WHITE, 0.2 * a);
            } else {
                blend_disc(f, ox, oy, r * (0.06 + 0.04 * hash(i, 8, 62)), lay(c, WHITE, 0.3), 0.6 * a);
            }
        }
    }
}

/// A fish looking the way `face` says: a forked tail that swings, fins,
/// scales, a gill and an eye. A dangerous one shows its teeth.
#[allow(clippy::too_many_arguments)]
fn fish(f: &mut Frame, x: f32, y: f32, r: f32, face: f32, c: Rgb, teeth: bool, a: f32, swim: f32) {
    let face = if face < 0.0 { -1.0 } else { 1.0 };
    let (cx, cy, pr) = (x * SF, y * SF, r * SF);
    let (bx, by) = (cx.floor() as i32, cy.floor() as i32);
    let (rx, ry) = (pr * 1.35, pr * 0.8);
    let swing = swim.sin() * pr * 0.35;
    let tail_from = rx * 0.78;
    let tail_len = pr * 1.15;
    let fine = r >= 5.0;
    let (x0, x1) = (-(rx + tail_len).ceil() as i32 - 1, rx.ceil() as i32 + 1);
    let tall = (pr * 1.5).ceil() as i32;
    for dy in -tall..=tall {
        for lx in x0..=x1 {
            // `u` runs along the fish, 1 at the nose; `v` runs down.
            let (fxu, fy) = (lx as f32 + 0.5, (by + dy) as f32 + 0.5 - cy);
            let (u, v) = (fxu / rx, fy / ry);
            let px = bx + (fxu * face) as i32;
            let py = by + dy;
            let e = u * u + v * v;
            let mut col = None;
            let mut al = 1.0;
            if e <= 1.0 {
                // A dark back, a pale belly, and a line of light between.
                let mut k = if v > 0.3 { lay(c, WHITE, (v - 0.3) * 0.85) } else { tint(c, 1.0 - v * 0.25) };
                if v < -0.5 { k = tint(k, 0.75 + (v + 1.0) * 0.5); }
                if fine {
                    if u < 0.4 {
                        let scale = ((fxu * 7.0 / pr).sin() * (fy * 9.0 / pr).cos()).abs();
                        k = tint(k, 0.86 + 0.24 * scale);
                    }
                    if r >= 8.0 && ((u + 1.0) * 3.2).fract() < 0.28 && u < 0.4 && v < 0.35 { k = tint(k, 0.7); }
                    // The gill: a dark arc behind the head.
                    let gill = ((u - 0.18).powi(2) + v * v * 0.45).sqrt();
                    if (gill - 0.3).abs() < 0.035 && u > 0.36 { k = tint(k, 0.6); }
                    // The side fin, beating.
                    let (qx, qy) = ((u - 0.12) / 0.26, (v - 0.32 - 0.06 * (swim * 0.7).sin()) / 0.2);
                    if qx * qx + qy * qy < 1.0 { k = tint(c, 0.62 + 0.2 * (qx * 9.0).sin().abs()); }
                    let sheen = (-((v + 0.45) / 0.16).powi(2)).exp();
                    k = lit(k, WHITE, sheen * 0.22);
                    // The mouth of one that bites.
                    if teeth && u > 0.62 && (v - 0.28).abs() < (u - 0.62) * 0.5 { k = 0x280808; }
                }
                col = Some(k);
                al = ((1.0 - e) * pr * 0.45).min(1.0);
            } else if -fxu > tail_from && -fxu < tail_from + tail_len {
                // The tail: it widens, swings, and is cut in at the end.
                let s = (-fxu - tail_from) / tail_len;
                let q = (fy - swing * s).abs();
                let half = ry * (0.16 + 0.95 * s);
                let cut = half * 0.5 * ((s - 0.5) / 0.5).max(0.0);
                if q < half && q >= cut {
                    let ray = 0.5 + 0.5 * (q / half * 14.0).cos();
                    col = Some(tint(c, 0.62 + 0.2 * ray - 0.15 * s));
                    al = (half - q).min(1.0) * if fine { 0.92 } else { 1.0 };
                }
            } else if fine && fy < 0.0 && u > -0.55 && u < 0.4 {
                // The fin on the back, highest toward the tail.
                let top = -ry * (1.0 - u * u).sqrt();
                let s = (u + 0.55) / 0.95;
                let high = ry * 0.62 * (s * PI).sin().powf(0.6) * (1.25 - s);
                if fy > top - high {
                    let ray = 0.5 + 0.5 * (fxu * 14.0 / pr + fy * 3.0 / pr).cos();
                    col = Some(tint(c, 0.58 + 0.22 * ray));
                    al = (fy - (top - high)).min(1.0) * 0.92;
                }
            } else if fine && fy > 0.0 && u > -0.35 && u < 0.05 {
                // A small fin under the belly.
                let low = ry * (1.0 - u * u).sqrt();
                let s = (u + 0.35) / 0.4;
                if fy < low + ry * 0.3 * (s * PI).sin() * (1.2 - s) { col = Some(tint(c, 0.6)); al = 0.9; }
            }
            if let Some(k) = col {
                let under = f.get(px, py);
                f.put(px, py, lay(under, k, a * al));
            }
        }
    }
    if a <= 0.5 { return; }
    let (ex, ey) = (x + face * r * 1.35 * 0.58, y - r * 0.8 * 0.22);
    if r >= 3.5 {
        let e = (r * 0.17).max(0.8);
        blend_disc(f, ex, ey, e + 0.5, 0xf8f8f0, 1.0);
        blend_disc(f, ex + face * e * 0.25, ey, e * 0.62, if teeth { 0xc01010 } else { 0x101018 }, 1.0);
        spot(f, ex - e * 0.3, ey - e * 0.35, WHITE, 0.8);
    } else {
        blend_disc(f, ex, ey, 0.6, BLACK, 1.0);
    }
    if teeth && r >= 5.0 {
        // Teeth along both lips.
        let n = (r * 0.5) as i32;
        for i in 0..n {
            let s = (i as f32 + 0.5) / n as f32;
            let tx = x + face * r * 1.35 * (0.66 + 0.3 * s);
            let gap = r * 0.8 * 0.36 * s * 0.5;
            let size = (r * 0.045).max(0.4);
            blend_disc(f, tx, y + r * 0.8 * 0.28 - gap, size, WHITE, 1.0);
            blend_disc(f, tx, y + r * 0.8 * 0.28 + gap, size, WHITE, 1.0);
        }
    }
}

/// A jellyfish: a bell you can see through that beats, ribs in it, a
/// frilled hem, and long threads under it.
fn jelly(f: &mut Frame, x: f32, y: f32, r: f32, c: Rgb, beat: f32) {
    let (cx, cy, pr) = (x * SF, y * SF, r * SF);
    let (bx, by) = (cx.floor() as i32, cy.floor() as i32);
    let wide = pr * (1.0 + beat.sin() * 0.14);
    let tall = pr * (1.0 - beat.sin() * 0.08);
    let pale = lay(c, WHITE, 0.55);
    // The threads hang behind the bell.
    for k in 0..9 {
        let off = (k as f32 - 4.0) / 4.0;
        let long = pr * (2.2 + 0.9 * hash(k, 3, 71));
        let mut i = 0.0;
        while i < long {
            let s = i / long;
            let sway = (beat * 0.5 + i * 0.09 + k as f32 * 1.3).sin() * (1.5 + s * pr * 0.3);
            add(f, (cx + off * wide * 0.8 + sway) as i32, (cy + i) as i32, c, 0.55 * (1.0 - s));
            i += 1.0;
        }
    }
    // Four frilled arms in the middle.
    for k in 0..4 {
        let off = (k as f32 - 1.5) * 0.16;
        let long = pr * 1.5;
        let mut i = 0.0;
        while i < long {
            let s = i / long;
            let sway = (beat * 0.6 + i * 0.16 + k as f32 * 2.1).sin() * (1.0 + s * pr * 0.14);
            let half = ((1.0 - s) * pr * 0.07) as i32 + 1;
            for w in -half..=half { add(f, (cx + off * wide + sway) as i32 + w, (cy + i) as i32, pale, 0.3 * (1.0 - s)); }
            i += 1.0;
        }
    }
    let reach = wide.ceil() as i32 + 1;
    for dy in -(tall.ceil() as i32) - 1..=(tall * 0.2) as i32 + 1 {
        for dx in -reach..=reach {
            let (fx, fy) = ((bx + dx) as f32 + 0.5 - cx, (by + dy) as f32 + 0.5 - cy);
            let (u, v) = (fx / wide, fy / tall);
            if u.abs() >= 1.0 { continue; }
            let k = (u * u + v * v).sqrt();
            // Above: a dome. Below: a hem in scallops.
            let inside = if v <= 0.0 { k <= 1.0 } else { v < 0.17 * (0.35 + 0.65 * (u * 11.0).cos().abs()) * (1.0 - u * u).sqrt() };
            if !inside { continue; }
            let rib = (u.atan2(-v.min(0.0) + 0.25) * 5.5).cos().abs().powi(10);
            let rim = k.min(1.0).powi(6);
            let top = (-((u + 0.25).powi(2) + (v + 0.7).powi(2)) * 14.0).exp();
            let col = lit(lay(c, pale, (rim * 0.8 + rib * 0.35).min(1.0)), WHITE, top * 0.5);
            let al = (0.42 + 0.45 * rim + 0.15 * rib).min(0.95) * if v <= 0.0 { ((1.0 - k) * pr).min(1.0) } else { 1.0 };
            let (px, py) = (bx + dx, by + dy);
            let under = f.get(px, py);
            f.put(px, py, lay(under, col, al));
        }
    }
    // What glows inside it.
    for k in [-1.0, 1.0] { soft(f, x + k * r * 0.26, y - r * 0.42, r * 0.2, pale, 0.5); }
}

/// The giant the moon goes round: bands of cloud that drift and curl, a
/// storm, day on its upper left, and thin air at its rim.
fn giant(f: &mut Frame, time: f32) {
    glow(f, CX, CY, GIANT * 2.1, 0xff9040, 0.3);
    let (cx, cy, r) = (CX * SF, CY * SF, GIANT * SF);
    let reach = r as i32 + 3 * S;
    let storm = -0.2 + 0.12 * (time * 0.05).sin();
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (fx, fy) = (dx as f32 + 0.5, dy as f32 + 0.5);
            let d = (fx * fx + fy * fy).sqrt();
            let (px, py) = (cx as i32 + dx, cy as i32 + dy);
            if d > r {
                let k = (1.0 - (d - r) / (2.5 * SF)).max(0.0);
                add(f, px, py, 0xffb070, k * k * 0.55 * (0.4 - 0.6 * (fx + fy) / (d * 1.41)).clamp(0.0, 1.0));
                continue;
            }
            let (nx, ny) = (fx / r, fy / r);
            let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
            let (lon, lat) = (nx.atan2(nz), ny.asin());
            // Each band drifts at its own speed, and their edges curl.
            let flow = time * 0.05 * (1.0 + (lat * 7.0).sin());
            let curl = fbm(lon * 1.6 + flow + 20.0, lat * 5.0 + 20.0, 3, 4, 64) - 0.5;
            let band = 0.5 + 0.5 * ((lat + curl * 0.24) * 11.0).sin();
            let fine = noise(lon * 6.0 + flow * 2.0 + 40.0, lat * 18.0 + 40.0, 64, 5);
            let k = (band * 0.8 + fine * 0.25).clamp(0.0, 1.0);
            let mut c = if k < 0.5 { lay(0x7a3c20, 0xc88850, k * 2.0) } else { lay(0xc88850, 0xf0d8a8, k * 2.0 - 1.0) };
            // The storm: a red oval with a pale ring round it.
            let (sx, sy) = ((lon - storm) / 0.36, (lat - 0.3) / 0.15);
            let q = sx * sx + sy * sy;
            if q < 1.0 {
                let whirl = 0.5 + 0.5 * (q * 9.0 - sy.atan2(sx) * 2.0 - time * 0.4).sin();
                c = lay(c, lay(0xb03818, 0xe07040, whirl), (1.0 - q).sqrt() * 0.9);
            } else if q < 1.5 {
                c = lay(c, 0xf8e0b8, (1.5 - q) * 0.8);
            }
            let facing = (nx * -0.52 + ny * -0.55 + nz * 0.65).max(0.0);
            let col = tint(c, 0.05 + 1.15 * facing.powf(0.75));
            let under = f.get(px, py);
            f.put(px, py, lay(under, col, (r - d).min(1.0)));
        }
    }
}

/// A star: a wide glow, a ball that seethes, flames round its rim, and
/// eight rays that breathe.
fn star(f: &mut Frame, x: f32, y: f32, r: f32, c: Rgb, a: f32, time: f32) {
    glow(f, x, y, r * 3.4, c, 0.5 * a);
    let (cx, cy, pr) = (x * SF, y * SF, r * SF);
    let hot = lay(c, WHITE, 0.82);
    if pr < 5.0 {
        blend_disc(f, x, y, r, c, a);
        blend_disc(f, x, y, r * 0.65, hot, a);
        return;
    }
    let (bx, by) = (cx.floor() as i32, cy.floor() as i32);
    let reach = (pr * 1.75).ceil() as i32;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (fx, fy) = ((bx + dx) as f32 + 0.5 - cx, (by + dy) as f32 + 0.5 - cy);
            let d = (fx * fx + fy * fy).sqrt();
            let (px, py) = (bx + dx, by + dy);
            if d < pr {
                // Hot in the middle, its own colour at the rim, and
                // grains that come and go.
                let limb = (1.0 - d * d / (pr * pr)).sqrt();
                let grain = noise(fx / pr * 5.0 + time * 0.25 + 9.0, fy / pr * 5.0 - time * 0.2 + 9.0, 64, 71);
                let col = lay(c, hot, (limb * 0.95 + (grain - 0.5) * 0.55).clamp(0.0, 1.0));
                let under = f.get(px, py);
                f.put(px, py, lay(under, col, a * (pr - d).min(1.0)));
            } else if d < pr * 1.75 {
                let flame = noise(fy.atan2(fx) / TAU * 24.0 + 12.0, time * 0.6 - d / pr * 1.5, 24, 72);
                let k = (1.0 - (d - pr) / (pr * 0.75 * (0.3 + flame))).max(0.0);
                add(f, px, py, c, k * k * 0.9 * a);
            }
        }
    }
    let long = pr * (2.7 + 0.3 * (time * 2.0).sin());
    for (ux, uy, len) in [(1.0, 0.0, 1.0), (-1.0, 0.0, 1.0), (0.0, 1.0, 1.0), (0.0, -1.0, 1.0),
                          (0.707, 0.707, 0.55), (-0.707, 0.707, 0.55), (0.707, -0.707, 0.55), (-0.707, -0.707, 0.55f32)] {
        let end = long * len;
        let mut far = pr * 0.9;
        while far < end {
            let k = 1.0 - far / end;
            let k = k * k * 0.75 * a;
            let (qx, qy) = (cx + ux * far, cy + uy * far);
            add(f, qx as i32, qy as i32, hot, k);
            add(f, (qx - uy) as i32, (qy + ux) as i32, c, k * 0.4);
            add(f, (qx + uy) as i32, (qy - ux) as i32, c, k * 0.4);
            far += 1.0;
        }
    }
}

/// The sky behind a size in space: clouds of gas in the size's own
/// colours with bright threads and dark dust in them, and stars.
fn sky(size: Size) -> Frame {
    let mut f = Frame::new(RW, RH);
    let (a, b, seed) = match size {
        Size::Moon => (0x101838, 0x301040, 41),
        Size::Star => (0x402070, 0x104060, 42),
        Size::Pulsar => (0x105060, 0x602050, 43),
        Size::Hole => (0x502810, 0x281040, 44),
        _ => return f,
    };
    rows(&mut f, |y, row| {
        for (x, p) in row.iter_mut().enumerate() {
            let (wx, wy) = (x as f32 / SF, y as f32 / SF);
            let (u, v) = (wx / 90.0, wy / 90.0);
            // The gas is pushed about by a second field: that gives it
            // folds instead of plain blots.
            let (qx, qy) = (fbm(u + 3.0, v + 9.0, 3, seed + 5, 64) - 0.5, fbm(u + 11.0, v + 2.0, 3, seed + 6, 64) - 0.5);
            let (gu, gv) = (u + qx * 1.6, v + qy * 1.6);
            let cloud = fbm(gu, gv, 5, seed, 64);
            let mut gas = (cloud - 0.4).max(0.0) * 1.7;
            let which = fbm(gu + 7.0, gv + 3.0, 3, seed + 9, 64);
            // What a burst star leaves is a ring of gas round the middle.
            if size == Size::Pulsar {
                let d = ((wx - CX).powi(2) + (wy - CY).powi(2) * 1.6).sqrt();
                gas = gas * 0.5 + (1.0 - ((d - 150.0) / 70.0).powi(2)).max(0.0) * fbm(gu * 2.0, gv * 2.0, 4, seed + 3, 64) * 0.9;
            }
            let mut c = lay(0x020208, lay(a, b, which), gas.min(1.0));
            // Bright threads where the gas folds.
            let thread = (1.0 - (fbm(gu * 2.0 + 5.0, gv * 2.0 + 5.0, 4, seed + 11, 64) - 0.5).abs() * 9.0).max(0.0);
            c = lit(c, lay(b, WHITE, 0.35), thread * thread * gas.min(1.0) * 0.55);
            // Dust in front of it.
            let dust = (fbm(u * 1.4 + 20.0 + qy, v * 1.4 + 20.0 + qx, 4, seed + 13, 64) - 0.55).max(0.0) * 3.0;
            *p = lay(c, 0x010104, dust.min(0.8));
        }
    });
    let mut rng = Rng::new(seed as u64);
    for _ in 0..1700 {
        let (x, y) = (rng.float() * W as f32, rng.float() * H as f32);
        let bright = rng.float();
        let c = lay(0xa0c0ff, 0xffe0a0, rng.float());
        add(&mut f, (x * SF) as i32, (y * SF) as i32, c, 0.15 + bright.powi(3) * 0.7);
        if bright > 0.9 { spot(&mut f, x, y, c, 0.5); }
        if bright > 0.988 {
            // The brightest have a glow and a small cross.
            glow(&mut f, x, y, 3.5, c, 0.5);
            for k in 1..7 * S {
                let fade = 0.5 * (1.0 - k as f32 / (7 * S) as f32);
                for (dx, dy) in [(k, 0), (-k, 0), (0, k), (0, -k)] { add(&mut f, (x * SF) as i32 + dx, (y * SF) as i32 + dy, c, fade); }
            }
        }
    }
    f
}

/// A spiral of stars, slowly turning: a warm heart, two blue arms with
/// pink clouds and dark dust in them, and a haze of old stars between.
fn galaxy(f: &mut Frame, time: f32) {
    f.clear(0x010104);
    // The same stars every time: the stream starts from the same seed.
    let mut rng = Rng::new(33);
    for _ in 0..900 {
        let (x, y) = ((rng.float() * RW as f32) as i32, (rng.float() * RH as f32) as i32);
        f.put(x, y, tint(WHITE, 0.1 + rng.float().powi(3) * 0.5));
    }
    glow(f, CX, CY, 190.0, 0x283870, 0.45);
    const FLAT: f32 = 0.6;
    let turn = time * 0.05;
    // An arm winds outward: the angle grows with the log of the distance.
    let wind = |d: f32| (1.0 + d / 14.0).ln() * 2.5;
    for i in 0..34000 {
        // Many near the middle, few far out.
        let d = (-(1.0 - rng.float() * 0.97).ln() * 54.0).min(185.0);
        let between = i % 4 == 0;
        // Three numbers added lie mostly near their middle: an arm is
        // dense along its spine and thins out to both sides.
        let off = (rng.float() + rng.float() + rng.float() - 1.5) / 1.5;
        let th = if between { rng.float() * TAU } else { (i % 2) as f32 * PI + wind(d) + off * (0.95 - d / 400.0) } + turn;
        let (x, y) = (CX + th.cos() * d, CY + th.sin() * d * FLAT);
        let young = rng.float();
        let c = if between { 0xffe0b0 } else { lay(0xfff0c0, 0x88b0ff, (d / 90.0).min(1.0) * (0.5 + 0.5 * young)) };
        let k = (0.12 + rng.float() * 0.42) * if between { 0.45 } else { 1.0 - off.abs() * 0.7 };
        add(f, (x * SF) as i32, (y * SF) as i32, c, k);
        if between { continue; }
        // Pink clouds where stars are born, and the odd bright young star.
        if i % 150 == 1 && d > 25.0 && off.abs() < 0.4 { glow(f, x, y, 2.0 + young * 2.5, 0xff4c80, 0.45); }
        if i % 90 == 2 && d > 15.0 { spot(f, x, y, 0xd0e0ff, 0.5 + young * 0.4); }
    }
    // Dust lies along the inside of each arm.
    for i in 0..2600 {
        let d = 16.0 + rng.float() * 120.0;
        let th = (i % 2) as f32 * PI + wind(d) - 0.3 + (rng.float() + rng.float() - 1.0) * 0.16 + turn;
        soft(f, CX + th.cos() * d, CY + th.sin() * d * FLAT, 1.2 + rng.float() * 1.6, 0x010104, 0.3);
    }
    // The heart: old yellow stars in a ball.
    for _ in 0..5000 {
        let d = (rng.float() + rng.float() + rng.float()) / 3.0 * 44.0 * rng.float();
        let th = rng.float() * TAU;
        add(f, ((CX + th.cos() * d) * SF) as i32, ((CY + th.sin() * d * 0.8) * SF) as i32, 0xffe8b8, 0.2 + rng.float() * 0.3);
    }
    glow(f, CX, CY, 52.0, 0xffd8a0, 0.45);
    glow(f, CX, CY, 16.0, 0xfff4d0, 0.9);
}

/// Galaxies in their thousands, strung along threads, with crowds where
/// the threads meet. It does not move, so it is painted once and kept.
fn web(f: &mut Frame) {
    static KEPT: std::sync::OnceLock<Frame> = std::sync::OnceLock::new();
    let kept = KEPT.get_or_init(|| {
        let mut f = Frame::new(RW, RH);
        f.clear(0x010103);
        let mut rng = Rng::new(51);
        for _ in 0..90000 {
            let (x, y) = (rng.float() * W as f32, rng.float() * H as f32);
            // Threads run where the noise crosses its middle, and they
            // are thick where a second field is high.
            let ridge = 1.0 - (fbm(x / 85.0, y / 85.0, 4, 52, 64) - 0.5).abs() * 9.0;
            let rich = fbm(x / 120.0 + 9.0, y / 120.0 + 4.0, 3, 53, 64);
            let pick = rng.float();
            if ridge < pick * 0.9 + 0.08 - (rich - 0.5) * 0.5 {
                // Between the threads, a galaxy here and there.
                if pick > 0.985 { add(&mut f, (x * SF) as i32, (y * SF) as i32, 0x8088b0, 0.3); }
                continue;
            }
            let c = lay(0x8090ff, 0xffd0a0, rng.float());
            add(&mut f, (x * SF) as i32, (y * SF) as i32, c, 0.2 + ridge.max(0.0) * 0.5);
            let big = rng.float();
            if big > 0.97 { spot(&mut f, x, y, c, 0.6); }
            if big > 0.994 && ridge > 0.5 { glow(&mut f, x, y, 3.0 + rich * 5.0, c, 0.5); }
        }
        f
    });
    f.px.copy_from_slice(&kept.px);
}

impl Game for Bigger {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        if input.pressed(Key::Char('p')) && self.mode == Mode::Play { return Flow::Pause; }
        if input.pressed(Key::Space) || input.pressed(Key::Enter) {
            match self.mode {
                Mode::Title => self.begin(Size::Dot),
                Mode::Over | Mode::Won if self.timer > 1.5 => self.mode = Mode::Title,
                _ => {}
            }
        }
        self.step(input, dt);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        match self.mode {
            Mode::Title => self.title(f),
            Mode::Play => {
                self.scene(f, true);
                // A bite makes the picture tremble for a moment.
                if self.shake > 0.0 {
                    let far = 3.0 * SF * self.shake / 0.35;
                    let (sx, sy) = (((self.time * 61.0).sin() * far) as i32, ((self.time * 47.0).cos() * far) as i32);
                    let old = f.px.clone();
                    for y in 0..RH { for x in 0..RW {
                        f.px[(y * RW + x) as usize] = old[((y + sy).clamp(0, RH - 1) * RW + (x + sx).clamp(0, RW - 1)) as usize];
                    } }
                }
                self.hud(f);
                if self.size == Size::Dot {
                    let line = match self.eaten { 0 | 1 => "EAT THE DOTS.", 2 | 3 => "THAT IS THE WHOLE GAME.", _ => "REALLY." };
                    self.say(f, line, "", self.stage_t);
                } else {
                    let (a, b) = self.size.lines();
                    self.say(f, a, b, (6.0 - self.timer).min(1.0));
                }
            }
            Mode::Pull => {
                let k = self.pull.t / PULL;
                self.scene(f, k > 0.82 || self.pull.with_me);
                self.disc(f);
                let (a, b) = match self.view {
                    View::Stage(s) => s.lines(),
                    View::Galaxy => ("THE HOLE WAS THE HEART OF A GALAXY.", ""),
                    View::Web => ("THE GALAXY WAS ONE OF MANY.", ""),
                    View::Void => ("ALL OF THEM WERE", ""),
                };
                self.say(f, a, b, (k - 0.3) * 4.0);
            }
            Mode::Nova => {
                let t = self.timer;
                self.scene(f, true);
                if t < NOVA_BANG {
                    // The star swells and whitens. One slow swell: nothing blinks.
                    let k = t / NOVA_BANG;
                    glow(f, self.me.x, self.me.y, self.me.r * (3.0 + 14.0 * k * k), 0xfff0c0, 1.2 * k);
                    veil(f, 0xfff4e0, 0.8 * k * k * k);
                } else {
                    let k = (t - NOVA_BANG) / (NOVA - NOVA_BANG);
                    for (lag, c) in [(0.0, 0xfff0c0), (0.08, 0xff9040), (0.16, 0x8060ff)] {
                        let reach = ((k - lag).max(0.0)).sqrt() * 420.0;
                        for w in 0..8 { circle_of_light(f, self.me.x, self.me.y, reach - w as f32, c, (1.0 - k) * 0.45); }
                    }
                    veil(f, 0xfff4e0, 0.8 * (1.0 - k * 2.2).max(0.0).powi(2));
                    let (a, b) = Size::Pulsar.lines();
                    self.say(f, a, b, (k - 0.25) * 4.0);
                }
            }
            Mode::Fall => {
                let t = self.timer;
                self.scene(f, true);
                if t < FALL_DONE {
                    // Everything is drawn in, and the light goes.
                    let k = t / FALL_DONE;
                    glow(f, self.me.x, self.me.y, self.me.r * (6.0 - 5.0 * k), 0xc0e0ff, k);
                    veil(f, BLACK, 0.85 * k * k);
                } else {
                    let k = (t - FALL_DONE) / (FALL - FALL_DONE);
                    veil(f, BLACK, 0.85 * (1.0 - k * 2.5).max(0.0));
                    let (a, b) = Size::Hole.lines();
                    self.say(f, a, b, (k - 0.2) * 4.0);
                }
            }
            Mode::End => self.ending(f),
            Mode::Over => {
                self.scene(f, false);
                f.dim(0.4);
                f.text_centered(RW / 2, 84 * S, "EATEN", 0xff6060, true, 5 * S);
                f.text_centered(RW / 2, 150 * S, &format!("YOU GOT AS FAR AS {}", self.size.name()), WHITE, true, S);
                f.text_centered(RW / 2, 174 * S, &format!("SCORE {}", self.score), WHITE, true, 2 * S);
            }
            Mode::Won => {
                f.clear(WHITE);
                f.text_centered(RW / 2, 84 * S, "THE END", BLACK, true, 5 * S);
                f.text_centered(RW / 2, 156 * S, &format!("SCORE {}", self.score), BLACK, true, 2 * S);
            }
        }
    }
}

/// The game as it is played: with sound, from the title or straight
/// into the size `BIGGER_START` names.
fn bigger() -> Bigger {
    let mut game = Bigger::new();
    game.audio = Audio::open();
    start_at(&mut game);
    game
}

/// `BIGGER_START`: 1 to 7 start at that size, 8 at the ending.
fn start_at(game: &mut Bigger) {
    let Some(n) = std::env::var("BIGGER_START").ok().and_then(|s| s.trim().parse::<u32>().ok()) else { return };
    let size = match n { 2 => Size::Cell, 3 => Size::Fish, 4 => Size::Moon, 5 => Size::Star, 6 => Size::Pulsar, 7 | 8 => Size::Hole, _ => Size::Dot };
    game.begin(size);
    if n >= 8 { game.me.r = Size::Hole.goal_r(); }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = Config { width: RW, height: RH, fps: 60 };
    if let Ok(n) = std::env::var("BIGGER_BENCH") {
        let mut game = Bigger::new();
        game.begin(Size::Dot);
        start_at(&mut game);
        let last = bench(&mut game, &Input::new(), cfg, n.parse().unwrap_or(600));
        if let Ok(path) = std::env::var("BIGGER_SHOT") { let _ = std::fs::write(path, last.to_ppm()); }
        return;
    }
    run(&mut bigger(), cfg);
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(bigger(), Config { width: RW, height: RH, fps: 60 });

#[cfg(test)]
impl Bigger {
    /// A careful player: toward the nearest food, away from anything
    /// that bites, and a moon keeps clear of the giant.
    fn bot_arrows(&self) -> (f32, f32) {
        let me = self.me;
        let wraps = matches!(self.size, Size::Cell | Size::Pulsar);
        let seen: Vec<(f32, f32, f32, bool, f32)> = self.things.iter().map(|t| {
            let (dx, dy) = if wraps { (wrapped(me.x, t.x, W as f32), wrapped(me.y, t.y, H as f32)) } else { (t.x - me.x, t.y - me.y) };
            let food = match (self.size, t.kind) {
                (Size::Dot, _) => true,
                (_, Kind::Sting) => false,
                (Size::Hole, Kind::Plain) => true,
                _ => me.r > t.r * 1.1,
            };
            (dx, dy, (dx * dx + dy * dy).sqrt().max(0.1), food, t.r)
        }).collect();
        let (mut fx, mut fy) = (0.0f32, 0.0f32);
        let wary = match self.size { Size::Star => 75.0, Size::Hole => 90.0, _ => 45.0 };
        for &(dx, dy, d, food, r) in &seen {
            if !food && d < r + me.r + wary { fx -= dx / d * 4.0; fy -= dy / d * 4.0; }
        }
        if let Some(&(dx, dy, d, ..)) = seen.iter().filter(|s| s.3).min_by(|a, b| a.2.total_cmp(&b.2)) {
            fx += dx / d;
            fy += dy / d;
        }
        if self.size == Size::Moon {
            let (dx, dy) = (me.x - CX, me.y - CY);
            let d = (dx * dx + dy * dy).sqrt();
            if d < GIANT + 36.0 { fx += dx / d * 6.0; fy += dy / d * 6.0; }
        }
        // A neutron star turns to face the way it wants to go, and
        // pushes when it does.
        if self.size == Size::Pulsar {
            if fx == 0.0 && fy == 0.0 { return (0.0, 0.0); }
            let turn = wrapped(me.ang, fy.atan2(fx), TAU);
            let speed = (me.vx * me.vx + me.vy * me.vy).sqrt();
            return (if turn > 0.12 { 1.0 } else if turn < -0.12 { -1.0 } else { 0.0 }, if turn.abs() < 0.5 && speed < 110.0 { -1.0 } else { 0.0 });
        }
        let dead = 0.3;
        (if fx > dead { 1.0 } else if fx < -dead { -1.0 } else { 0.0 }, if fy > dead { 1.0 } else if fy < -dead { -1.0 } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(seed: u64) -> Bigger {
        let mut g = Bigger::new();
        g.rng = Rng::new(seed);
        g
    }

    fn run_for(g: &mut Bigger, secs: f32, stop: impl Fn(&Bigger) -> bool) -> f32 {
        let input = Input::new();
        let mut t = 0.0;
        while t < secs && !stop(g) {
            g.update(&input, 1.0 / 60.0);
            t += 1.0 / 60.0;
        }
        t
    }

    /// The bot's game on a seed: where it ended, and its bites by size.
    fn bot_game(seed: u64, secs: f32) -> (Bigger, [u32; 7], [f32; 7]) {
        let mut g = game(seed);
        g.bot = true;
        g.begin(Size::Dot);
        let input = Input::new();
        let (mut bites, mut took, mut t) = ([0; 7], [0.0; 7], 0.0);
        while t < secs && g.mode != Mode::Won {
            let before = g.lives;
            g.update(&input, 1.0 / 60.0);
            t += 1.0 / 60.0;
            if g.mode == Mode::Play { took[g.size as usize] += 1.0 / 60.0; }
            // The bot never runs out of lives; its bites are counted.
            if g.lives < before {
                bites[g.size as usize] += 1;
                g.lives = before;
            }
        }
        (g, bites, took)
    }

    #[test]
    fn the_bot_plays_every_size_to_the_end() {
        for seed in [11, 22, 44] {
            let (g, bites, took) = bot_game(seed, 1200.0);
            eprintln!("seed {seed}: bites {bites:?}, seconds {:?}", took.map(|t| t as u32));
            assert_eq!(g.mode, Mode::Won, "seed {seed}: stuck as a {:?} with radius {}, bites {bites:?}", g.size, g.me.r);
            assert!(bites.iter().sum::<u32>() <= 20, "seed {seed}: the bot was bitten {bites:?} times");
        }
    }

    #[test]
    fn a_meal_makes_it_bigger_and_a_bite_smaller() {
        let mut g = game(4);
        g.begin(Size::Cell);
        let before = g.me.r;
        g.things[0] = Thing { x: g.me.x, y: g.me.y, r: 3.0, age: 2.0, ..g.things[0] };
        g.meet();
        assert!(g.me.r > before);
        let fed = g.me.r;
        g.me.safe = 0.0;
        g.things[1] = Thing { x: g.me.x, y: g.me.y, r: fed * 2.0, age: 2.0, ..g.things[1] };
        g.meet();
        assert!(g.me.r < fed);
        assert_eq!(g.lives, 2);
        assert!(g.me.safe > 0.0);
    }

    #[test]
    fn three_bites_end_the_game() {
        let mut g = game(5);
        g.begin(Size::Cell);
        for _ in 0..3 {
            g.me.safe = 0.0;
            g.bitten(1.0, 0.0, 1.0);
        }
        assert_eq!(g.mode, Mode::Over);
    }

    #[test]
    fn a_moon_left_alone_stays_in_orbit() {
        let mut g = game(6);
        g.begin(Size::Moon);
        g.things.clear();
        run_for(&mut g, 60.0, |_| false);
        let d = ((g.me.x - CX).powi(2) + (g.me.y - CY).powi(2)).sqrt();
        assert!((d - 105.0).abs() < 12.0, "the moon is {d} from the giant");
        assert_eq!(g.lives, 3);
    }

    #[test]
    fn a_big_star_pulls_the_player_and_the_player_pulls_a_small_one() {
        let mut g = game(9);
        g.begin(Size::Star);
        let one = g.things[0];
        g.things = vec![Thing { x: CX + 80.0, y: CY, vx: 0.0, vy: 0.0, r: 20.0, age: 9.0, ..one }, Thing { x: CX, y: CY - 70.0, vx: 0.0, vy: 0.0, r: 3.0, age: 9.0, ..one }];
        run_for(&mut g, 0.5, |_| false);
        assert!(g.me.x > CX + 1.0, "the player was not pulled: {}", g.me.x);
        assert!(g.things[1].y > CY - 70.0 + 0.5, "the small star was not pulled: {}", g.things[1].y);
    }

    #[test]
    fn the_beam_breaks_a_big_rock_into_food() {
        let mut g = game(10);
        g.begin(Size::Pulsar);
        let one = g.things[0];
        // The beam points up at first; a big rock sits in it.
        g.things = vec![Thing { x: CX, y: CY - 70.0, vx: 0.0, vy: 0.0, r: 12.0, age: 9.0, heat: 0.0, ..one }];
        run_for(&mut g, 1.5, |_| false);
        assert_eq!(g.things.len(), 3);
        assert!(g.things.iter().all(|t| g.me.r > t.r * 1.1), "a piece is too big to eat");
    }

    #[test]
    fn a_star_blows_up_and_a_neutron_star_falls_in() {
        let mut g = game(12);
        g.begin(Size::Star);
        g.me.r = Size::Star.goal_r();
        run_for(&mut g, 0.1, |_| false);
        assert_eq!(g.mode, Mode::Nova);
        run_for(&mut g, NOVA + 0.5, |g| g.mode == Mode::Play);
        assert_eq!((g.mode, g.size), (Mode::Play, Size::Pulsar));
        g.me.r = Size::Pulsar.goal_r();
        run_for(&mut g, 0.1, |_| false);
        assert_eq!(g.mode, Mode::Fall);
        run_for(&mut g, FALL + 0.5, |g| g.mode == Mode::Play);
        assert_eq!((g.mode, g.size), (Mode::Play, Size::Hole));
    }

    #[test]
    fn a_black_hole_that_is_not_fed_fades() {
        let mut g = game(13);
        g.begin(Size::Hole);
        g.things.clear();
        run_for(&mut g, 4.0, |_| false);
        assert!(g.me.r < Size::Hole.start_r());
        run_for(&mut g, 10.0, |g| g.lives < 3);
        assert_eq!(g.lives, 2);
    }

    #[test]
    fn the_old_picture_shrinks_to_the_new_player() {
        let mut g = game(7);
        g.begin(Size::Dot);
        g.things.clear();
        run_for(&mut g, 0.1, |_| false);
        assert_eq!(g.mode, Mode::Pull);
        assert_eq!(g.size, Size::Cell);
        // A third of the way, the black picture of the dot still covers
        // the middle.
        run_for(&mut g, PULL * 0.3, |_| false);
        let mut f = Frame::new(RW, RH);
        g.draw(&mut f);
        assert_eq!(f.get((CX as i32 + 30) * S, (CY as i32 - 45) * S), BLACK);
        run_for(&mut g, PULL, |g| g.mode == Mode::Play);
        assert_eq!(g.mode, Mode::Play);
        g.draw(&mut f);
        assert_ne!(f.get((CX as i32 + 30) * S, (CY as i32 - 45) * S), BLACK);
    }

    #[test]
    fn the_ending_leaves_a_dot_and_brings_a_bigger_one() {
        let mut g = game(8);
        g.begin(Size::Hole);
        g.me.r = Size::Hole.goal_r();
        run_for(&mut g, 60.0, |g| g.mode == Mode::End);
        assert_eq!(g.mode, Mode::End);
        let mut f = Frame::new(RW, RH);
        run_for(&mut g, 1.0, |_| false);
        g.draw(&mut f);
        assert_eq!(f.get(CX as i32 * S, CY as i32 * S), WHITE);
        assert_eq!(f.get((CX as i32 + 60) * S, CY as i32 * S), BLACK);
        run_for(&mut g, 6.0, |_| false);
        g.draw(&mut f);
        assert_eq!(f.get((CX as i32 + 60) * S, CY as i32 * S), WHITE);
        run_for(&mut g, 5.0, |g| g.mode == Mode::Won);
        assert_eq!(g.mode, Mode::Won);
        assert!(g.score > 3000);
    }

    /// `BIGGER_FILM=<folder>` gets pictures of the bot's game, four a
    /// second: `cargo test --release --example bigger film -- --ignored`.
    #[test]
    #[ignore]
    fn film() {
        let Ok(dir) = std::env::var("BIGGER_FILM") else { return };
        let mut g = game(22);
        g.bot = true;
        g.begin(Size::Dot);
        let input = Input::new();
        let mut f = Frame::new(RW, RH);
        let mut log = String::new();
        let mut was = (g.mode, g.size);
        for i in 0..60 * 600 {
            g.update(&input, 1.0 / 60.0);
            g.lives = 5;
            if (g.mode, g.size) != was {
                was = (g.mode, g.size);
                log.push_str(&format!("f{:04} {:?} {:?}\n", i / 15, g.mode, g.size));
            }
            // A picture four times a second while the size changes, and
            // one every four seconds of play.
            if i % if g.mode == Mode::Play { 240 } else { 15 } == 0 {
                g.draw(&mut f);
                let _ = std::fs::write(format!("{dir}/f{:04}.ppm", i / 15), f.to_ppm());
            }
            if g.mode == Mode::Won { break; }
        }
        let _ = std::fs::write(format!("{dir}/log.txt"), log);
    }
}
