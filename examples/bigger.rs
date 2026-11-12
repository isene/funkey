//! bigger: a dot eats dots. Then the picture pulls back, and the dot
//! turns out to be something else, in a world with rules of its own.
//!
//!     cargo run --release --example bigger
//!
//! The arrows steer, P pauses, Q quits. Eat what is smaller than you and
//! keep away from what is bigger.
//!
//! `BIGGER_START=<1 to 5>` starts at another size (5 is the ending);
//! `BIGGER_BENCH=<frames>` plays that many frames with no terminal and
//! prints the time one takes, and `BIGGER_SHOT=<file.ppm>` keeps the
//! last picture of that run.

use funkey::noise::{hash, noise};
use funkey::*;
use std::f32::consts::TAU;

const W: i32 = 320;
const H: i32 = 200;
const CX: f32 = 160.0;
const CY: f32 = 100.0;
const GAME: &str = "bigger";
/// The game's own version; the engine has its own.
const VERSION: &str = "0.1";

/// The pull of the giant the moon goes round: a round orbit 70 pixels
/// out takes 45 pixels a second.
const GM: f32 = 141_750.0;
/// The giant's radius.
const GIANT: f32 = 22.0;
/// The widest orbit that stays on the screen.
const FAR: f32 = 92.0;
/// How far out a disc must reach to cover the whole picture.
const COVER: f32 = 190.0;
/// Seconds a pull-back takes.
const PULL: f32 = 3.4;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Size { Dot, Cell, Fish, Moon }

impl Size {
    fn n(self) -> u32 { match self { Size::Dot => 1, Size::Cell => 2, Size::Fish => 3, Size::Moon => 4 } }
    fn name(self) -> &'static str { match self { Size::Dot => "DOT", Size::Cell => "CELL", Size::Fish => "FISH", Size::Moon => "MOON" } }
    /// The radius it starts with, and the radius that ends the size.
    fn start_r(self) -> f32 { match self { Size::Dot => 3.0, Size::Cell => 5.0, Size::Fish => 5.0, Size::Moon => 4.0 } }
    fn goal_r(self) -> f32 { match self { Size::Dot => 3.0, Size::Cell => 14.0, Size::Fish => 15.0, Size::Moon => 10.0 } }
    fn next(self) -> Option<Size> {
        match self { Size::Dot => Some(Size::Cell), Size::Cell => Some(Size::Fish), Size::Fish => Some(Size::Moon), Size::Moon => None }
    }
    /// What the pull-back shows, and the new rule.
    fn lines(self) -> (&'static str, &'static str) {
        match self {
            Size::Dot => ("EAT THE DOTS.", ""),
            Size::Cell => ("YOU WERE INSIDE A CELL.", "EAT THE SMALL. FLEE THE BIG."),
            Size::Fish => ("THE CELL WAS INSIDE A FISH.", "SAME RULES. WETTER. JELLYFISH STING."),
            Size::Moon => ("THE FISH LIVED ON A MOON.", "YOU ARE THE MOON. MIND THE GRAVITY."),
        }
    }
}

/// What a picture shows: a size being played, or one of the three
/// pictures the ending pulls back through.
#[derive(Clone, Copy, PartialEq, Debug)]
enum View { Stage(Size), Stars, Galaxy, Void }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Play, Pull, End, Over, Won }

/// Everything that is not the player: food while it is smaller, a
/// danger while it is bigger.
#[derive(Clone, Copy, Debug)]
struct Thing {
    x: f32, y: f32, vx: f32, vy: f32, r: f32,
    /// It stings at any size and is never food: a jellyfish.
    sting: bool,
    color: Rgb,
    /// Where it is in its own wobble.
    phase: f32,
    /// Seconds since it came; a new one fades in and bites nobody yet.
    age: f32,
    /// A rock's orbit: how far out, the angle now, and the turn a second.
    orbit: f32, angle: f32, turn: f32,
}

#[derive(Clone, Copy, Debug)]
struct Me {
    x: f32, y: f32, vx: f32, vy: f32, r: f32,
    /// Seconds left in which nothing can bite.
    safe: f32,
    /// 1 looking right, -1 looking left.
    face: f32,
}

/// A pull-back: the old picture shrinks into a disc that ends as the
/// thing at (`x`, `y`) with radius `r` in the new picture.
#[derive(Clone, Copy, Debug)]
struct Pull { t: f32, x: f32, y: f32, r: f32 }

struct Sounds {
    eat: Vec<Sample>,
    bit: Sample,
    pull: Sample,
    big: Sample,
    end: Sample,
    music: [Sample; 3],
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
        let big = Sample::synth(2.5, |t, r| ((t * 55.0 * TAU).sin() * 0.5 + r * 0.05) * (t * 2.0).min(1.0) * (2.5 - t).min(1.0));
        let cell = Tune::parse("168 c4/8 g4/8 e4/8 g4/8 a3/8 e4/8 c4/8 e4/8 f3/8 c4/8 a3/8 c4/8 g3/8 d4/8 b3/8 d4/8", Wave::Triangle, 0.16).render();
        let fish = Tune::parse("132 d4 a4/8 f4/8 a4 - c4 g4/8 e4/8 g4 - bb3 f4/8 d4/8 f4 - a3 e4/8 c#4/8 e4 -", Wave::Sine, 0.2).render();
        let moon = Tune::parse("60 a2/2 e3/2 c3/2 g3/2 f2/2 c3/2 e2/2 b2/2", Wave::Sine, 0.24).render()
            .mixed(&Tune::parse("60 - e5/8 - a5/8 - c5/8 - g5/8 - a4/8 - c5/8 - b4/8 - e5/8", Wave::Triangle, 0.07).render());
        Sounds {
            eat,
            bit: Sample::sweep(Wave::Saw, 300.0, 70.0, 0.35, 0.3),
            pull, big,
            end: Tune::parse("200 c5/8 g4/8 e4/8 c4/2", Wave::Triangle, 0.3).render(),
            music: [cell, fish, moon],
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
    bits: Particles,
    rng: Rng,
    audio: Audio,
    snd: Sounds,
    /// A test's hand on the arrows.
    #[cfg(test)]
    bot: bool,
}

fn smooth(t: f32) -> f32 { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

/// A disc with a soft edge, laid over what is there.
fn blend_disc(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
    let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
    for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            if d > r { continue; }
            let edge = (r - d).min(1.0);
            let under = f.get(x, y);
            f.put(x, y, mix(under, c, a * edge));
        }
    }
}

/// A ball lit from the upper left.
fn ball(f: &mut Frame, cx: f32, cy: f32, r: f32, c: Rgb, a: f32) {
    let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
    for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r { continue; }
            let light = (0.75 - (dx + dy) / (r * 2.2)).clamp(0.25, 1.15);
            let under = f.get(x, y);
            f.put(x, y, mix(under, tint(c, light), a));
        }
    }
}

impl Bigger {
    fn new() -> Bigger {
        Bigger {
            size: Size::Dot, view: View::Void,
            me: Me { x: CX, y: CY, vx: 0.0, vy: 0.0, r: 3.0, safe: 0.0, face: 1.0 },
            things: Vec::new(), eaten: 0, combo: 0, combo_t: 0.0, score: 0,
            high: funkey::scores::best(GAME), lives: 3, mode: Mode::Title,
            timer: 0.0, stage_t: 0.0, total_t: 0.0, time: 0.0,
            pull: Pull { t: 0.0, x: CX, y: CY, r: 3.0 }, step: 0,
            snap: Frame::new(W, H), bits: Particles::new(),
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
        self.me = Me { x: CX, y: CY, vx: 0.0, vy: 0.0, r: size.start_r(), safe: 1.5, face: 1.0 };
        if size == Size::Moon { self.to_orbit(); }
        self.things.clear();
        let n = match size { Size::Dot => 6, Size::Cell => 14, Size::Fish => 11, Size::Moon => 12 };
        for i in 0..n {
            let mut t = self.spawn(i);
            t.age = 1.0;
            // The first of a size are spread over the picture; later
            // ones come in from its edges.
            if size == Size::Cell || size == Size::Fish {
                t.x = self.rng.range(10.0, W as f32 - 10.0);
                if (t.x - CX).abs() < 50.0 && (t.y - CY).abs() < 40.0 { t.x += 110.0 * if t.x < CX { -1.0 } else { 1.0 }; }
            }
            self.things.push(t);
        }
    }

    /// The moon back on a round orbit, to the right of the giant.
    fn to_orbit(&mut self) {
        let a = 70.0;
        (self.me.x, self.me.y) = (CX + a, CY);
        (self.me.vx, self.me.vy) = (0.0, (GM / a).sqrt());
    }

    /// A radius for a new thing: mostly food, sometimes a danger, always
    /// measured against the player as it is now.
    fn pick_r(&mut self, nth: usize) -> f32 {
        let big = nth % 3 == 2;
        let k = if big { self.rng.range(1.3, 1.8) } else { self.rng.range(0.35, 0.8) };
        (self.me.r * k).clamp(1.5, 24.0)
    }

    /// One new thing for the size being played. `nth` decides its sort,
    /// so that two in three are food.
    fn spawn(&mut self, nth: usize) -> Thing {
        let mut t = Thing { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, r: 2.0, sting: false, color: WHITE,
            phase: self.rng.range(0.0, TAU), age: 0.0, orbit: 0.0, angle: 0.0, turn: 0.0 };
        match self.size {
            Size::Dot => {
                // Six dots on a ring round the middle, none under the player.
                let a = nth as f32 / 6.0 * TAU + self.rng.range(-0.3, 0.3);
                let d = self.rng.range(45.0, 80.0);
                (t.x, t.y) = (CX + a.cos() * d * 1.5, CY + a.sin() * d);
            }
            Size::Cell => {
                t.r = self.pick_r(nth);
                // In from an edge, drifting inward.
                let side = self.rng.below(4);
                let along = self.rng.float();
                (t.x, t.y) = match side {
                    0 => (-t.r, along * H as f32),
                    1 => (W as f32 + t.r, along * H as f32),
                    2 => (along * W as f32, -t.r),
                    _ => (along * W as f32, H as f32 + t.r),
                };
                let a = self.rng.range(0.0, TAU);
                (t.vx, t.vy) = (a.cos() * 8.0, a.sin() * 8.0);
                t.color = *self.rng.pick(&[0x70d0a0, 0xc0d060, 0x60c0d8, 0xd09070, 0xb080d8]).unwrap_or(&WHITE);
            }
            Size::Fish => {
                t.sting = nth % 6 == 5;
                let right = self.rng.chance(0.5);
                if t.sting {
                    // A jellyfish rises from the sea floor.
                    t.r = 5.0;
                    (t.x, t.y) = (self.rng.range(20.0, W as f32 - 20.0), H as f32 + 10.0);
                    (t.vx, t.vy) = (0.0, -self.rng.range(7.0, 12.0));
                    t.color = 0xf080c0;
                } else {
                    t.r = self.pick_r(nth);
                    let speed = self.rng.range(18.0, 40.0);
                    t.x = if right { -t.r * 2.0 } else { W as f32 + t.r * 2.0 };
                    t.y = self.rng.range(20.0, H as f32 - 28.0);
                    t.vx = if right { speed } else { -speed };
                    t.color = *self.rng.pick(&[0xf0c040, 0x60d0b0, 0xe07050, 0xa0b0f0, 0xd0d0d8]).unwrap_or(&WHITE);
                }
            }
            Size::Moon => {
                t.r = self.pick_r(nth).clamp(2.0, 8.0);
                t.orbit = self.rng.range(GIANT + 22.0, FAR - 2.0);
                // Away from the player, so that nothing appears on top of it.
                let mine = (self.me.y - CY).atan2(self.me.x - CX);
                t.angle = mine + self.rng.range(1.2, TAU - 1.2);
                t.turn = (GM / t.orbit.powi(3)).sqrt();
                (t.x, t.y) = (CX + t.angle.cos() * t.orbit, CY + t.angle.sin() * t.orbit);
                t.color = *self.rng.pick(&[0x908880, 0xa09078, 0x787880, 0xb0a090]).unwrap_or(&WHITE);
            }
        }
        t
    }

    fn music(&mut self) {
        self.audio.stop(1);
        let i = match self.size { Size::Dot => return, Size::Cell => 0, Size::Fish => 1, Size::Moon => 2 };
        if self.mode == Mode::Play { self.audio.play_loop(1, &self.snd.music[i], 1.0); }
    }

    /// Which way the arrows point, each part -1, 0 or 1.
    fn arrows(&self, input: &Input) -> (f32, f32) {
        #[cfg(test)]
        if self.bot { return self.bot_arrows(); }
        (input.axis_x() as f32, input.axis_y() as f32)
    }

    /// Move the player by the rules of its size.
    fn steer(&mut self, ax: f32, ay: f32, dt: f32) {
        let me = &mut self.me;
        if ax != 0.0 { me.face = ax; }
        match self.size {
            // A dot goes where it is told and stops when it is not.
            Size::Dot => {
                (me.vx, me.vy) = (ax * 70.0, ay * 70.0);
            }
            // A cell is pushed about and drifts on.
            Size::Cell | Size::Fish => {
                let (push, drag) = if self.size == Size::Cell { (230.0, 2.4) } else { (320.0, 3.2) };
                me.vx += (ax * push - me.vx * drag) * dt;
                me.vy += (ay * push - me.vy * drag) * dt;
            }
            // A moon falls round the giant. The arrows push it; let go,
            // and its path settles into a round orbit where it is.
            Size::Moon => {
                let (dx, dy) = (me.x - CX, me.y - CY);
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                let g = GM / (d * d);
                me.vx += (-dx / d * g + ax * 110.0) * dt;
                me.vy += (-dy / d * g + ay * 110.0) * dt;
                // The speed of a round orbit here, the way it goes now.
                let way = if dx * me.vy - dy * me.vx >= 0.0 { 1.0 } else { -1.0 };
                let v = (GM / d).sqrt();
                let (rx, ry) = (-dy / d * v * way, dx / d * v * way);
                me.vx += (rx - me.vx) * 0.5 * dt;
                me.vy += (ry - me.vy) * 0.5 * dt;
                // Too far out, and the giant calls it back.
                if d > FAR { me.vx -= dx / d * 120.0 * dt; me.vy -= dy / d * 120.0 * dt; }
            }
        }
        me.x += me.vx * dt;
        me.y += me.vy * dt;
        match self.size {
            Size::Dot | Size::Fish => {
                let top = if self.size == Size::Fish { 14.0 } else { 0.0 };
                me.x = me.x.clamp(me.r, W as f32 - me.r);
                me.y = me.y.clamp(top + me.r, H as f32 - me.r - if self.size == Size::Fish { 10.0 } else { 0.0 });
            }
            // The drop of water has no edge: out one side, in the other.
            Size::Cell => {
                me.x = me.x.rem_euclid(W as f32);
                me.y = me.y.rem_euclid(H as f32);
            }
            Size::Moon => {}
        }
    }

    /// Move everything else.
    fn drift(&mut self, dt: f32) {
        let me = self.me;
        let size = self.size;
        let time = self.time;
        for t in &mut self.things {
            t.age += dt;
            let (dx, dy) = (me.x - t.x, me.y - t.y);
            let d = (dx * dx + dy * dy).sqrt().max(0.1);
            let bigger = t.r > me.r * 1.1;
            match size {
                Size::Dot => {}
                Size::Cell => {
                    // Small ones shy away, big ones come slowly closer,
                    // and the water jostles them all.
                    let pull = if bigger { 16.0 } else if d < 45.0 { -22.0 } else { 0.0 };
                    t.vx += (dx / d * pull + (time * 0.9 + t.phase).sin() * 6.0 - t.vx * 0.8) * dt;
                    t.vy += (dy / d * pull + (time * 0.7 + t.phase * 2.0).cos() * 6.0 - t.vy * 0.8) * dt;
                    t.x = (t.x + t.vx * dt).rem_euclid(W as f32);
                    t.y = (t.y + t.vy * dt).rem_euclid(H as f32);
                }
                Size::Fish => {
                    if t.sting {
                        t.x += (time * 1.3 + t.phase).sin() * 6.0 * dt;
                    } else if bigger && d < 70.0 {
                        // A big fish that sees the player goes for it.
                        t.vy += (dy / d * 60.0 - t.vy * 2.0) * dt;
                        if dx * t.vx > 0.0 { t.x += t.vx * 0.7 * dt; }
                    } else if !bigger && d < 40.0 {
                        t.vy += (-dy / d * 90.0 - t.vy * 2.0) * dt;
                    } else {
                        t.vy -= t.vy * 2.0 * dt;
                    }
                    t.x += t.vx * dt;
                    t.y = (t.y + t.vy * dt).clamp(16.0 - if t.sting { 60.0 } else { 0.0 }, H as f32 + 12.0);
                    if !t.sting { t.y = t.y.min(H as f32 - 14.0); }
                }
                Size::Moon => {
                    t.angle += t.turn * dt;
                    (t.x, t.y) = (CX + t.angle.cos() * t.orbit, CY + t.angle.sin() * t.orbit);
                }
            }
        }
        // A fish that has crossed the sea, or a jellyfish at the top, is
        // gone, and a new one comes.
        if size == Size::Fish {
            for i in 0..self.things.len() {
                let t = self.things[i];
                let out = if t.sting { t.y < -20.0 } else { t.x < -60.0 || t.x > W as f32 + 60.0 };
                if out { self.things[i] = self.spawn(i); }
            }
        }
    }

    /// The player meets things: it eats the smaller and is bitten by
    /// the bigger.
    fn meet(&mut self) {
        for i in 0..self.things.len() {
            // An eaten dot leaves the list, so the list may end early.
            let Some(&t) = self.things.get(i) else { break };
            if t.age < 0.6 { continue; }
            let (dx, dy) = (t.x - self.me.x, t.y - self.me.y);
            let d = (dx * dx + dy * dy).sqrt();
            let food = !t.sting && self.me.r > t.r * 1.1 || self.size == Size::Dot;
            if food && d < self.me.r + t.r * 0.3 {
                self.eat(i);
            } else if !food && (t.sting || t.r > self.me.r * 1.1) && d < t.r.max(self.me.r) * 0.9 && self.me.safe <= 0.0 {
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
        self.score += 10 * self.size.n() * (1 + self.combo.min(7));
        let note = (self.combo as usize).min(self.snd.eat.len() - 1);
        let snd = self.snd.eat[note].clone();
        self.audio.play(&snd, 1.0);
        self.bits.burst(t.x, t.y, 6 + t.r as usize, 40.0, 0.5, t.color);
        if self.size == Size::Dot {
            self.things.remove(i);
            return;
        }
        // The meal's area, most of it, joins the player's.
        self.me.r = (self.me.r * self.me.r + t.r * t.r * 0.62).sqrt().min(self.size.goal_r());
        self.things[i] = self.spawn(i);
    }

    fn bitten(&mut self, dx: f32, dy: f32, d: f32) {
        self.lives = self.lives.saturating_sub(1);
        let snd = self.snd.bit.clone();
        self.audio.play(&snd, 1.0);
        self.bits.burst(self.me.x, self.me.y, 24, 70.0, 0.8, 0xff6060);
        self.me.r = (self.me.r * 0.85).max(self.size.start_r());
        self.me.safe = 2.5;
        self.combo = 0;
        if self.size != Size::Moon {
            self.me.vx = -dx / d.max(0.1) * 120.0;
            self.me.vy = -dy / d.max(0.1) * 120.0;
        }
        if self.lives == 0 {
            self.audio.stop(1);
            if !cfg!(test) { funkey::scores::record(GAME, self.score); }
            self.high = self.high.max(self.score);
            (self.mode, self.timer) = (Mode::Over, 0.0);
        }
    }

    /// Start a pull-back: keep the picture as it is, then set up what
    /// comes next around it.
    fn pull_back(&mut self) {
        let mut snap = std::mem::replace(&mut self.snap, Frame::new(1, 1));
        self.scene(&mut snap, true);
        self.snap = snap;
        self.audio.stop(1);
        let snd = self.snd.pull.clone();
        self.audio.play(&snd, 1.0);
        let stage = if self.step == 0 { self.size.next() } else { None };
        match stage {
            Some(next) => {
                self.score += 500 * self.size.n();
                self.lives = (self.lives + 1).min(5);
                self.enter(next);
                self.pull = Pull { t: 0.0, x: self.me.x, y: self.me.y, r: self.me.r };
            }
            // Past the moon the picture pulls back three times more, and
            // nobody is asked to play.
            None => {
                self.step += 1;
                self.things.clear();
                (self.view, self.pull) = match self.step {
                    1 => (View::Stars, Pull { t: 0.0, x: 196.0, y: 112.0, r: 1.5 }),
                    2 => (View::Galaxy, Pull { t: 0.0, x: 214.0, y: 84.0, r: 1.5 }),
                    _ => (View::Void, Pull { t: 0.0, x: CX, y: CY, r: 3.0 }),
                };
                if self.step == 1 { self.score += 500 * self.size.n(); }
            }
        }
        (self.mode, self.timer) = (Mode::Pull, 0.0);
    }

    fn step(&mut self, input: &Input, dt: f32) {
        self.time += dt;
        self.timer += dt;
        self.bits.update(dt);
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
                if done { self.pull_back(); }
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
            // The last joke plays by the clock; see `ending`.
            Mode::End => {
                if self.timer - dt < 3.0 && self.timer >= 3.0 {
                    let snd = self.snd.big.clone();
                    self.audio.play(&snd, 1.0);
                }
                if self.timer >= 10.0 {
                    let fast = (3000.0 - self.total_t * 6.0).max(0.0) as u32;
                    self.score += self.lives * 1000 + fast;
                    if !cfg!(test) { funkey::scores::record(GAME, self.score); }
                    self.high = self.high.max(self.score);
                    let snd = self.snd.end.clone();
                    self.audio.play(&snd, 1.0);
                    (self.mode, self.timer) = (Mode::Won, 0.0);
                }
            }
            Mode::Over | Mode::Won => if self.timer > 12.0 { self.mode = Mode::Title; },
        }
    }

    // ---- pictures ----

    /// The picture of the view, with no score or words on it. `with_me`
    /// is false while a shrinking disc stands in for the player.
    fn scene(&self, f: &mut Frame, with_me: bool) {
        match self.view {
            View::Void => {
                f.clear(BLACK);
                for t in &self.things { f.circle(t.x as i32, t.y as i32, 2, WHITE); }
                if with_me { f.circle(self.me.x as i32, self.me.y as i32, 3, WHITE); }
            }
            View::Stage(Size::Dot) => {
                f.clear(BLACK);
                for t in &self.things { f.circle(t.x as i32, t.y as i32, 2, WHITE); }
                if with_me { f.circle(self.me.x as i32, self.me.y as i32, 3, WHITE); }
            }
            View::Stage(Size::Cell) => self.pond(f, with_me),
            View::Stage(Size::Fish) => self.sea(f, with_me),
            View::Stage(Size::Moon) => self.space(f, with_me),
            View::Stars => stars(f, self.time),
            View::Galaxy => galaxy(f, self.time),
        }
        self.bits.draw(f, 0, 0);
    }

    /// How much of the player shows: it blinks while nothing can bite.
    fn shown(&self) -> f32 {
        if self.mode == Mode::Play && self.me.safe > 0.0 && (self.time * 8.0) as i32 % 2 == 0 { 0.35 } else { 1.0 }
    }

    /// A drop of pond water.
    fn pond(&self, f: &mut Frame, with_me: bool) {
        for y in (0..H).step_by(4) {
            for x in (0..W).step_by(4) {
                let n = noise(x as f32 / 40.0 + self.time * 0.05, y as f32 / 40.0, 64, 3);
                f.rect(x, y, 4, 4, mix(0x06262a, 0x0e4840, n));
            }
        }
        for t in &self.things {
            let a = (t.age * 2.0).min(1.0);
            let r = t.r * (1.0 + (self.time * 2.0 + t.phase).sin() * 0.05);
            let danger = t.r > self.me.r * 1.1;
            cell(f, t.x, t.y, r, if danger { mix(t.color, 0xe04040, 0.45) } else { t.color }, a);
        }
        if with_me {
            blend_disc(f, self.me.x, self.me.y, self.me.r + 1.5, WHITE, 0.35 * self.shown());
            cell(f, self.me.x, self.me.y, self.me.r, 0xfff8a0, self.shown());
        }
    }

    /// The sea, seen from the side.
    fn sea(&self, f: &mut Frame, with_me: bool) {
        // Light comes down in slanted, slowly moving bands. A band's
        // strength is worked out once for each slanted line, not for
        // each pixel.
        let rays: Vec<f32> = (0..W + H).map(|i| (noise(i as f32 / 30.0 + self.time * 0.08, 0.5, 32, 5) - 0.55).max(0.0) * 0.5).collect();
        for y in 0..H {
            let deep = y as f32 / H as f32;
            let c = mix(0x2080b0, 0x061838, deep);
            for x in 0..W {
                let lit = rays[(x + y * 3 / 5) as usize] * (1.0 - deep);
                f.put(x, y, if lit > 0.0 { mix(c, 0xc0f0ff, lit) } else { c });
            }
        }
        for x in 0..W {
            let top = H - 9 - (noise(x as f32 / 24.0, 0.0, 64, 9) * 6.0) as i32;
            f.vline(x, top, H - top, mix(0x6a5a38, 0x8a7a4c, hash(x, 1, 2)));
        }
        for t in &self.things {
            let a = (t.age * 2.0).min(1.0);
            if t.sting {
                jelly(f, t.x, t.y, t.r, t.color, self.time * 3.0 + t.phase);
            } else {
                let danger = t.r > self.me.r * 1.1;
                fish(f, t.x, t.y, t.r, t.vx.signum(), if danger { tint(t.color, 0.55) } else { t.color }, danger, a, self.time * 8.0 + t.phase);
            }
        }
        if with_me { fish(f, self.me.x, self.me.y, self.me.r, self.me.face, 0xff9030, false, self.shown(), self.time * 8.0); }
    }

    /// The giant, its rocks and the moon.
    fn space(&self, f: &mut Frame, with_me: bool) {
        f.clear(0x04040c);
        for i in 0..90 {
            let (x, y) = ((hash(i, 0, 7) * W as f32) as i32, (hash(i, 1, 7) * H as f32) as i32);
            f.put(x, y, tint(WHITE, 0.3 + hash(i, 2, 7) * 0.6));
        }
        // The giant: bands of cloud that drift, lit from the upper left.
        let r = GIANT as i32;
        for dy in -r..=r {
            let half = ((r * r - dy * dy) as f32).sqrt() as i32;
            for dx in -half..=half {
                let band = noise(dy as f32 / 5.0, (dx as f32 + self.time * 2.0) / 30.0, 64, 4);
                let c = mix(0xd8a060, 0x9a5830, band);
                let light = (0.8 - (dx + dy) as f32 / (GIANT * 2.4)).clamp(0.2, 1.1);
                f.put(CX as i32 + dx, CY as i32 + dy, tint(c, light));
            }
        }
        for t in &self.things {
            let a = (t.age * 2.0).min(1.0);
            let danger = t.r > self.me.r * 1.1;
            ball(f, t.x, t.y, t.r, if danger { mix(t.color, 0xc04030, 0.5) } else { t.color }, a);
        }
        if with_me {
            ball(f, self.me.x, self.me.y, self.me.r, 0x3090d0, self.shown());
            blend_disc(f, self.me.x - self.me.r * 0.2, self.me.y + self.me.r * 0.25, self.me.r * 0.35, 0x50b060, self.shown() * 0.8);
        }
    }

    /// The old picture as a disc on its way to being one small thing.
    fn disc(&self, f: &mut Frame) {
        let p = self.pull;
        let k = (p.t / PULL).clamp(0.0, 1.0);
        let e = smooth(k);
        let r = COVER * (p.r / COVER).powf(e);
        let (cx, cy) = (CX + (p.x - CX) * e, CY + (p.y - CY) * e);
        let fade = ((k - 0.82) / 0.18).clamp(0.0, 1.0);
        let scale = COVER / r;
        let (x0, x1) = (((cx - r).floor() as i32).max(0), ((cx + r).ceil() as i32).min(W - 1));
        for y in ((cy - r).floor() as i32).max(0)..=((cy + r).ceil() as i32).min(H - 1) {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy > r * r { continue; }
                // A shrunken picture is the mean of four spots, or it
                // would flicker as it goes.
                let c = if scale < 1.5 {
                    mirrored(&self.snap, CX + dx * scale, CY + dy * scale)
                } else {
                    let mut sum = (0u32, 0u32, 0u32);
                    for (ox, oy) in [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)] {
                        let (r, g, b) = parts(mirrored(&self.snap, CX + (dx + ox) * scale, CY + (dy + oy) * scale));
                        sum = (sum.0 + r as u32, sum.1 + g as u32, sum.2 + b as u32);
                    }
                    rgb((sum.0 / 4) as u8, (sum.1 / 4) as u8, (sum.2 / 4) as u8)
                };
                // A pale rim, so that a dark picture shows against a dark one.
                let rim = if r - (dx * dx + dy * dy).sqrt() < 1.2 { 0.35 } else { 0.0 };
                let under = f.get(x, y);
                f.put(x, y, mix(mix(c, WHITE, rim), under, fade));
            }
        }
    }

    /// Two lines of words low in the picture, on a dark band.
    fn say(&self, f: &mut Frame, a: &str, b: &str, show: f32) {
        if show <= 0.0 { return; }
        let c = mix(BLACK, WHITE, show.min(1.0));
        f.text_centered(W / 2 + 1, H - 39, a, BLACK, true, 1);
        f.text_centered(W / 2, H - 40, a, c, true, 1);
        f.text_centered(W / 2 + 1, H - 25, b, BLACK, false, 1);
        f.text_centered(W / 2, H - 26, b, mix(BLACK, 0xa0e0ff, show.min(1.0)), false, 1);
    }

    fn hud(&self, f: &mut Frame) {
        // A dot has no score to look at. That comes with the cell.
        if self.size == Size::Dot { return; }
        f.text_big(5, 4, &format!("{:06}", self.score), WHITE);
        for k in 0..self.lives as i32 { f.circle(W - 8 - k * 9, 7, 3, 0xff6060); }
        let (from, to) = (self.size.start_r(), self.size.goal_r());
        let done = ((self.me.r - from) / (to - from)).clamp(0.0, 1.0);
        f.text(W / 2 - 52, 5, self.size.name(), 0xa0a0b8);
        f.rect(W / 2 - 30, 5, 62, 5, 0x303040);
        f.rect(W / 2 - 29, 6, (60.0 * done) as i32, 3, 0x60e080);
    }

    fn title(&self, f: &mut Frame) {
        f.clear(BLACK);
        f.text_centered(W / 2, 44, "BIGGER", WHITE, true, 4);
        f.circle(W / 2, 98, 3, WHITE);
        f.text_centered(W / 2, 116, "A GAME ABOUT A DOT", 0xa0a0b8, false, 1);
        if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 140, "PRESS SPACE", WHITE, true, 1); }
        f.text_centered(W / 2, 166, "ARROWS STEER  P PAUSES  Q QUITS", 0x707088, false, 1);
        if self.high > 0 { f.text_centered(W / 2, 180, &format!("BEST {}", self.high), 0x707088, false, 1); }
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x404050);
    }

    /// The end: the dot alone, then a bigger dot.
    fn ending(&self, f: &mut Frame) {
        f.clear(BLACK);
        let t = self.timer;
        // The big one rolls in from the right and stops on the small one.
        let k = smooth((t - 3.0) / 3.0);
        let bx = W as f32 + 70.0 - (W as f32 + 70.0 - CX - 30.0) * k;
        if bx - 60.0 > CX + 3.0 || t < 3.0 { f.circle(CX as i32, CY as i32, 3, WHITE); }
        if t >= 3.0 { f.circle(bx as i32, CY as i32, 60, WHITE); }
        if t < 3.0 {
            self.say(f, "YOU ARE A DOT.", "", t - 0.6);
        } else if t > 6.6 {
            f.text_centered(W / 2, H - 24, "THERE IS ALWAYS A BIGGER DOT.", WHITE, true, 1);
        }
    }
}

/// The old picture at a spot, also past its edges: there it is laid out
/// again as in a mirror, so a round cut of it has no empty corners.
fn mirrored(snap: &Frame, x: f32, y: f32) -> Rgb {
    let fold = |v: i32, n: i32| { let m = v.rem_euclid(2 * n); if m < n { m } else { 2 * n - 1 - m } };
    snap.get(fold(x.floor() as i32, snap.w), fold(y.floor() as i32, snap.h))
}

/// A cell: a soft body, a skin and a core.
fn cell(f: &mut Frame, x: f32, y: f32, r: f32, c: Rgb, a: f32) {
    blend_disc(f, x, y, r, c, 0.9 * a);
    if r > 2.5 {
        blend_disc(f, x, y, r - 1.5, tint(c, 0.55), 0.6 * a);
        blend_disc(f, x + r * 0.15, y - r * 0.1, r * 0.35, tint(c, 0.3), 0.9 * a);
    }
}

/// A fish looking the way `face` says. A dangerous one shows its teeth.
#[allow(clippy::too_many_arguments)]
fn fish(f: &mut Frame, x: f32, y: f32, r: f32, face: f32, c: Rgb, teeth: bool, a: f32, swim: f32) {
    let face = if face < 0.0 { -1.0 } else { 1.0 };
    let (rx, ry) = (r * 1.35, r * 0.8);
    for dy in -(ry.ceil() as i32)..=ry.ceil() as i32 {
        for dx in -(rx.ceil() as i32)..=rx.ceil() as i32 {
            let (u, v) = (dx as f32 / rx, dy as f32 / ry);
            if u * u + v * v > 1.0 { continue; }
            let shade = 1.05 - v * 0.35;
            let (px, py) = ((x + dx as f32) as i32, (y + dy as f32) as i32);
            let under = f.get(px, py);
            f.put(px, py, mix(under, tint(c, shade), a));
        }
    }
    // The tail swings.
    let tail = (swim.sin() * r * 0.25) as i32;
    let len = (r * 0.9).ceil() as i32;
    for i in 0..len {
        let half = (i as f32 * 0.8) as i32 + 1;
        let tx = (x - face * (rx + i as f32 - 1.0)) as i32;
        for dy in -half..=half {
            let under = f.get(tx, y as i32 + dy + tail * i / len.max(1));
            f.put(tx, y as i32 + dy + tail * i / len.max(1), mix(under, tint(c, 0.8), a));
        }
    }
    let (ex, ey) = ((x + face * rx * 0.55) as i32, (y - ry * 0.25) as i32);
    if r >= 3.0 && a > 0.5 {
        f.put(ex, ey, if teeth { 0xff3030 } else { BLACK });
        if r >= 6.0 { f.put(ex, ey - 1, WHITE); }
    }
    if teeth && r >= 6.0 && a > 0.5 {
        for i in 0..3 { f.put((x + face * (rx - 1.0 - i as f32 * 2.0)) as i32, (y + ry * 0.3) as i32, WHITE); }
    }
}

/// A jellyfish: a bell that beats, and threads under it.
fn jelly(f: &mut Frame, x: f32, y: f32, r: f32, c: Rgb, beat: f32) {
    let wide = r * (1.0 + beat.sin() * 0.12);
    for dy in -(r as i32)..=0 {
        let half = (wide * (1.0 - (dy as f32 / r).powi(2)).max(0.0).sqrt()) as i32;
        for dx in -half..=half {
            let (px, py) = (x as i32 + dx, y as i32 + dy);
            let under = f.get(px, py);
            f.put(px, py, mix(under, c, 0.75));
        }
    }
    for k in -2..=2 {
        for i in 1..(r * 1.6) as i32 {
            let sway = ((beat * 0.5 + i as f32 * 0.5 + k as f32).sin() * 1.2) as i32;
            let (px, py) = (x as i32 + k * 2 + sway, y as i32 + i);
            let under = f.get(px, py);
            f.put(px, py, mix(under, c, 0.45));
        }
    }
}

/// A star with its planets, among other stars.
fn stars(f: &mut Frame, time: f32) {
    f.clear(0x020208);
    for i in 0..260 {
        let (x, y) = ((hash(i, 0, 21) * W as f32) as i32, (hash(i, 1, 21) * H as f32) as i32);
        f.put(x, y, tint(mix(0xa0c0ff, 0xffe0a0, hash(i, 3, 21)), 0.25 + hash(i, 2, 21) * 0.7));
    }
    blend_disc(f, CX, CY, 16.0, 0xffb040, 0.25);
    blend_disc(f, CX, CY, 9.0, 0xffe080, 0.9);
    blend_disc(f, CX, CY, 6.0, 0xfffff0, 1.0);
    for (i, a) in [22.0f32, 38.0, 60.0].iter().enumerate() {
        let th = time * 6.0 / a + i as f32 * 2.1;
        f.put((CX + th.cos() * a) as i32, (CY + th.sin() * a * 0.6) as i32, 0xc0c0d0);
    }
}

/// A spiral of stars, slowly turning.
fn galaxy(f: &mut Frame, time: f32) {
    f.clear(0x010104);
    for i in 0..120 {
        let (x, y) = ((hash(i, 0, 31) * W as f32) as i32, (hash(i, 1, 31) * H as f32) as i32);
        f.put(x, y, tint(WHITE, 0.15 + hash(i, 2, 31) * 0.3));
    }
    for i in 0..2600 {
        let d = hash(i, 0, 33).powf(0.7) * 88.0;
        let arm = (i % 2) as f32 * std::f32::consts::PI;
        let th = arm + d / 22.0 + (hash(i, 1, 33) - 0.5) * (1.4 - d / 120.0) + time * 0.05;
        let (x, y) = (CX + th.cos() * d, CY + th.sin() * d * 0.62);
        let c = mix(0xfff0c0, 0x80a8ff, (d / 80.0).min(1.0));
        let under = f.get(x as i32, y as i32);
        f.put(x as i32, y as i32, mix(under, c, 0.35 + hash(i, 2, 33) * 0.4));
    }
    blend_disc(f, CX, CY, 7.0, 0xfff4d0, 0.8);
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
                self.hud(f);
                if self.size == Size::Dot {
                    let line = match self.eaten { 0 | 1 => "EAT THE DOTS.", 2 | 3 => "THAT IS THE WHOLE GAME.", _ => "REALLY." };
                    self.say(f, line, "", self.stage_t);
                } else {
                    let (a, b) = self.size.lines();
                    self.say(f, a, b, (5.0 - self.timer).min(1.0));
                }
            }
            Mode::Pull => {
                let k = self.pull.t / PULL;
                self.scene(f, k > 0.82);
                self.disc(f);
                let (a, b) = match self.view {
                    View::Stage(s) => s.lines(),
                    View::Stars => ("THE MOON WENT ROUND A STAR.", ""),
                    View::Galaxy => ("THE STAR WAS IN A GALAXY.", ""),
                    View::Void => ("THE GALAXY WAS", ""),
                };
                self.say(f, a, b, (k - 0.3) * 4.0);
            }
            Mode::End => self.ending(f),
            Mode::Over => {
                self.scene(f, false);
                f.dim(0.4);
                f.text_centered(W / 2, 60, "EATEN", 0xff6060, true, 3);
                f.text_centered(W / 2, 100, &format!("YOU GOT AS FAR AS {}", self.size.name()), WHITE, false, 1);
                f.text_centered(W / 2, 116, &format!("SCORE {}", self.score), WHITE, true, 1);
            }
            Mode::Won => {
                f.clear(WHITE);
                f.text_centered(W / 2, 56, "THE END", BLACK, true, 3);
                f.text_centered(W / 2, 100, &format!("SCORE {}", self.score), BLACK, true, 1);
                f.text_centered(W / 2, 118, "MORE SIZES ARE ON THEIR WAY", 0x606070, false, 1);
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

/// `BIGGER_START`: 1 to 4 start at that size, 5 at the ending.
fn start_at(game: &mut Bigger) {
    let Some(n) = std::env::var("BIGGER_START").ok().and_then(|s| s.trim().parse::<u32>().ok()) else { return };
    game.begin(match n { 2 => Size::Cell, 3 => Size::Fish, 4 | 5 => Size::Moon, _ => Size::Dot });
    if n >= 5 { game.me.r = Size::Moon.goal_r(); }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = Config { width: W, height: H, fps: 60 };
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
funkey::web!(bigger(), Config { width: W, height: H, fps: 60 });

#[cfg(test)]
impl Bigger {
    /// A careful player: toward the nearest food, away from anything
    /// that bites, and a moon keeps clear of the giant.
    fn bot_arrows(&self) -> (f32, f32) {
        let me = self.me;
        let (mut fx, mut fy) = (0.0f32, 0.0f32);
        let mut best = f32::MAX;
        for t in &self.things {
            let (dx, dy) = (t.x - me.x, t.y - me.y);
            let d = (dx * dx + dy * dy).sqrt().max(0.1);
            let food = self.size == Size::Dot || !t.sting && me.r > t.r * 1.1;
            if food {
                if d < best { best = d; }
            } else if d < t.r + me.r + 30.0 {
                fx -= dx / d * 4.0;
                fy -= dy / d * 4.0;
            }
        }
        for t in &self.things {
            let (dx, dy) = (t.x - me.x, t.y - me.y);
            let d = (dx * dx + dy * dy).sqrt().max(0.1);
            let food = self.size == Size::Dot || !t.sting && me.r > t.r * 1.1;
            if food && d <= best { fx += dx / d; fy += dy / d; }
        }
        if self.size == Size::Moon {
            let (dx, dy) = (me.x - CX, me.y - CY);
            let d = (dx * dx + dy * dy).sqrt();
            if d < GIANT + 24.0 { fx += dx / d * 6.0; fy += dy / d * 6.0; }
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

    #[test]
    fn the_bot_plays_every_size_to_the_end() {
        for seed in [11, 22, 44] {
            let mut g = game(seed);
            g.bot = true;
            g.begin(Size::Dot);
            let input = Input::new();
            let (mut bites, mut t) = (0, 0.0);
            while t < 600.0 && g.mode != Mode::Won {
                let before = g.lives;
                g.update(&input, 1.0 / 60.0);
                t += 1.0 / 60.0;
                // The bot never runs out of lives; its bites are counted.
                if g.lives < before {
                    bites += 1;
                    g.lives = before;
                }
            }
            assert_eq!(g.mode, Mode::Won, "seed {seed}: stuck as a {:?} with radius {} after {t} s", g.size, g.me.r);
            assert!(bites <= 10, "seed {seed}: the bot was bitten {bites} times");
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
        assert!((d - 70.0).abs() < 8.0, "the moon is {d} from the giant");
        assert_eq!(g.lives, 3);
    }

    #[test]
    fn the_old_picture_shrinks_to_the_new_player() {
        let mut g = game(7);
        g.begin(Size::Dot);
        g.things.clear();
        run_for(&mut g, 0.1, |_| false);
        assert_eq!(g.mode, Mode::Pull);
        assert_eq!(g.size, Size::Cell);
        // Half way, the black picture of the dot still covers the middle.
        run_for(&mut g, PULL * 0.3, |_| false);
        let mut f = Frame::new(W, H);
        g.draw(&mut f);
        assert_eq!(f.get(CX as i32 + 20, CY as i32 - 30), BLACK);
        run_for(&mut g, PULL, |g| g.mode == Mode::Play);
        assert_eq!(g.mode, Mode::Play);
        g.draw(&mut f);
        assert_ne!(f.get(CX as i32 + 20, CY as i32 - 30), BLACK);
    }

    #[test]
    fn the_ending_leaves_a_dot_and_brings_a_bigger_one() {
        let mut g = game(8);
        g.begin(Size::Moon);
        g.me.r = Size::Moon.goal_r();
        run_for(&mut g, 60.0, |g| g.mode == Mode::End);
        assert_eq!(g.mode, Mode::End);
        let mut f = Frame::new(W, H);
        run_for(&mut g, 1.0, |_| false);
        g.draw(&mut f);
        assert_eq!(f.get(CX as i32, CY as i32), WHITE);
        assert_eq!(f.get(CX as i32 + 40, CY as i32), BLACK);
        run_for(&mut g, 6.0, |_| false);
        g.draw(&mut f);
        assert_eq!(f.get(CX as i32 + 40, CY as i32), WHITE);
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
        let mut f = Frame::new(W, H);
        for i in 0..60 * 240 {
            g.update(&input, 1.0 / 60.0);
            g.lives = 5;
            if i % 15 == 0 {
                g.draw(&mut f);
                let _ = std::fs::write(format!("{dir}/f{:04}.ppm", i / 15), f.to_ppm());
            }
            if g.mode == Mode::Won { break; }
        }
    }
}
