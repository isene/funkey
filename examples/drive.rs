//! drive: a car on a winding road over the hills, on funkey's 3D
//! rasterizer. Packages lie along the road and off it; fetch them all
//! before the clock runs out, then more of them, farther apart, with
//! less time. Smuggler's Run in spirit: go fast, leave the road, regret it.
//!
//!     cargo run --release --example drive
//!
//! Up and Down for the gas and the brake, Left and Right steer, Space
//! starts, Q quits. The arrow at the top points at the nearest package.

use funkey::*;

const W: i32 = 320;
const H: i32 = 200;
const ROAD_HALF: f32 = 9.0;
const CELL: f32 = 6.0;
const GAME: &str = "drive";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";

/// Where the road's middle is at a given distance: it winds.
fn road_x(z: f32) -> f32 { 38.0 * (z / 95.0).sin() + 22.0 * (z / 41.0).sin() }

/// The ground: rolling hills, flat where the road runs.
fn ground(x: f32, z: f32) -> f32 {
    let hills = 4.0 * (x / 41.0).sin() * (z / 53.0).cos() + 2.0 * (x / 13.0 + z / 17.0).sin() + 1.2 * (z / 7.0).sin() * (x / 9.0).cos();
    let road = 2.5 * (z / 60.0).sin();
    let off = (x - road_x(z)).abs();
    let t = ((off - ROAD_HALF) / 14.0).clamp(0.0, 1.0);
    road + (hills - road) * t * t * (3.0 - 2.0 * t)
}

fn on_road(x: f32, z: f32) -> bool { (x - road_x(z)).abs() < ROAD_HALF }

#[derive(Clone, Copy, PartialEq)]
enum Mode { Title, Play, Clear(f32), Over(f32) }

struct Package { x: f32, z: f32, taken: bool }

struct Drive {
    raster: Raster,
    car: Mesh,
    post: Mesh,
    package: Mesh,
    x: f32, z: f32, heading: f32, speed: f32,
    distance: f32,
    time: f32,
    audio: Audio,
    rng: Rng,
    mode: Mode,
    level: u32,
    packages: Vec<Package>,
    time_left: f32,
    score: u32,
    high: u32,
    last_tick: i32,
    s_pick: Sample, s_clear: Sample, s_over: Sample, s_tick: Sample,
}

impl Drive {
    fn new() -> Drive {
        let mut car = Mesh::cuboid(3.2, 1.0, 5.6, 0xd82020);
        car.extend(&Mesh::cuboid(2.6, 0.9, 2.6, 0x203040), &M4::translate(V3::new(0.0, 0.95, -0.3)));
        for (sx, sz) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            car.extend(&Mesh::cuboid(0.6, 1.1, 1.1, 0x181818), &M4::translate(V3::new(sx * 1.6, -0.35, sz * 1.9)));
        }
        let mut package = Mesh::cuboid(2.4, 2.0, 2.4, 0xa0642c);
        package.extend(&Mesh::cuboid(2.5, 0.5, 2.5, 0xf0c020), &M4::identity());
        let mut raster = Raster::new(W, H);
        raster.fog = Some((0x9fb8d8, 180.0));
        raster.light = V3::new(0.3, 1.0, -0.5).norm();
        let mut audio = Audio::open();
        audio.play_loop(1, &Sample::loop_tone(Wave::Saw, 55.0, 0.5, 0.25), 0.15);
        Drive {
            raster, car, post: Mesh::cuboid(0.5, 1.6, 0.5, 0xf0f0f0), package,
            x: 0.0, z: 0.0, heading: 0.0, speed: 0.0, distance: 0.0, time: 0.0, audio, rng: Rng::from_time(),
            mode: Mode::Title, level: 0, packages: Vec::new(), time_left: 0.0, score: 0, high: funkey::store::high_score(GAME), last_tick: 0,
            s_pick: Sample::sweep(Wave::Triangle, 600.0, 1300.0, 0.14, 0.5),
            s_clear: Tune::parse("190 c5 e5 g5 c6/2 -/8 g5/8 c6/2", Wave::Square, 0.35).render(),
            s_over: Sample::sweep(Wave::Saw, 300.0, 50.0, 0.9, 0.4),
            s_tick: Sample::tone(Wave::Square, 1000.0, 0.04, 0.3),
        }
    }

    /// A level: packages ahead along the road, some off it, and a clock.
    fn start_level(&mut self, level: u32) {
        self.level = level;
        let n = 4 + level as usize * 2;
        let span = 220.0 + level as f32 * 90.0;
        self.packages.clear();
        for k in 0..n {
            let z = self.z + 50.0 + span * (k as f32 + self.rng.range(0.2, 0.8)) / n as f32;
            let off = if self.rng.chance(0.6) { self.rng.range(-6.0, 6.0) } else { self.rng.range(12.0, 32.0) * if self.rng.chance(0.5) { -1.0 } else { 1.0 } };
            self.packages.push(Package { x: road_x(z) + off, z, taken: false });
        }
        self.time_left = 30.0 + n as f32 * 4.0;
        self.mode = Mode::Play;
    }

    fn nearest(&self) -> Option<&Package> {
        self.packages.iter().filter(|p| !p.taken).min_by(|a, b| {
            let da = (a.x - self.x).powi(2) + (a.z - self.z).powi(2);
            let db = (b.x - self.x).powi(2) + (b.z - self.z).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn drive(&mut self, input: &Input, dt: f32) {
        let on_road = on_road(self.x, self.z);
        let top = if on_road { 70.0 } else { 26.0 };
        if input.held(Key::Up) { self.speed += 22.0 * dt; }
        if input.held(Key::Down) { self.speed -= 40.0 * dt; }
        self.speed -= self.speed * (if on_road { 0.12 } else { 0.9 }) * dt;
        self.speed = self.speed.clamp(0.0, top);
        let steer = (input.held(Key::Right) as i32 - input.held(Key::Left) as i32) as f32;
        self.heading += steer * 1.4 * dt * (self.speed / 40.0).clamp(0.25, 1.0);
        self.x += self.heading.sin() * self.speed * dt;
        self.z += self.heading.cos() * self.speed * dt;
        self.distance += self.speed * dt;
        self.audio.rate(1, 0.7 + self.speed / 45.0);
        self.audio.volume(1, 0.15 + self.speed / 300.0);
    }
}

impl Game for Drive {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        match self.mode {
            Mode::Title => {
                self.speed = 0.0;
                if input.pressed(Key::Space) || input.pressed(Key::Enter) { self.score = 0; self.start_level(1); }
            }
            Mode::Play => {
                self.drive(input, dt);
                self.time_left -= dt;
                let (x, z) = (self.x, self.z);
                let mut got = 0;
                for p in self.packages.iter_mut() {
                    if !p.taken && (p.x - x).powi(2) + (p.z - z).powi(2) < 4.5 * 4.5 { p.taken = true; got += 1; }
                }
                if got > 0 { self.score += 100 * got; self.audio.play(&self.s_pick, 1.0); }
                if self.packages.iter().all(|p| p.taken) {
                    self.score += (self.time_left * 10.0) as u32;
                    self.mode = Mode::Clear(3.0);
                    self.audio.play(&self.s_clear, 1.0);
                } else if self.time_left <= 0.0 {
                    self.time_left = 0.0;
                    if funkey::store::record_score(GAME, self.score) { self.high = self.score; }
                    self.mode = Mode::Over(4.0);
                    self.audio.play(&self.s_over, 1.0);
                } else if self.time_left < 10.0 && self.time_left as i32 != self.last_tick {
                    self.last_tick = self.time_left as i32;
                    self.audio.play(&self.s_tick, 1.0);
                }
            }
            Mode::Clear(t) => {
                self.drive(input, dt);
                if t - dt <= 0.0 { let next = self.level + 1; self.start_level(next); } else { self.mode = Mode::Clear(t - dt); }
            }
            Mode::Over(t) => {
                self.speed *= 1.0 - 2.0 * dt;
                if t - dt <= 0.0 || input.pressed(Key::Space) { self.mode = Mode::Title; } else { self.mode = Mode::Over(t - dt); }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        // Sky: a gradient.
        for y in 0..H / 2 { let t = y as f32 / (H / 2) as f32; f.hline(0, y, W, funkey::raster::blend(0x4a78c8, 0x9fb8d8, t)); }
        f.rect(0, H / 2, W, H / 2, 0x9fb8d8);
        self.raster.clear();
        let car_y = ground(self.x, self.z) + 0.9;
        let cam = Cam3 {
            pos: V3::new(self.x - self.heading.sin() * 14.0, car_y + 5.0, self.z - self.heading.cos() * 14.0),
            yaw: self.heading, pitch: -0.22, focal: W as f32,
        };
        // The ground around the car, one mesh a frame.
        let mut ground_mesh = Mesh::new();
        let (cx, cz) = ((self.x / CELL).floor() * CELL, (self.z / CELL).floor() * CELL);
        let mut k = 0;
        for iz in -4..32 {
            for ix in -18..18 {
                let (x0, z0) = (cx + ix as f32 * CELL, cz + iz as f32 * CELL);
                let (x1, z1) = (x0 + CELL, z0 + CELL);
                let mid = x0 + CELL / 2.0;
                let off = mid - road_x(z0 + CELL / 2.0);
                let colour = if off.abs() < ROAD_HALF {
                    if off.abs() < 1.5 && (z0 / CELL) as i32 % 2 == 0 { 0xd8d8b0 } else { 0x505058 }
                } else {
                    let h = ground(mid, z0 + CELL / 2.0);
                    if k % 2 == 0 { funkey::raster::blend(0x4c8c30, 0x9c9c5c, (h / 8.0).clamp(0.0, 1.0)) } else { funkey::raster::blend(0x448028, 0x94945c, (h / 8.0).clamp(0.0, 1.0)) }
                };
                k += 1;
                let a = V3::new(x0, ground(x0, z0), z0);
                let b = V3::new(x1, ground(x1, z0), z0);
                let c = V3::new(x1, ground(x1, z1), z1);
                let d = V3::new(x0, ground(x0, z1), z1);
                ground_mesh.quad(a, d, c, b, colour);
            }
        }
        self.raster.draw(f, &ground_mesh, &M4::identity(), &cam);
        // Posts along both edges, so the bends read.
        let z_first = ((self.z - 20.0) / 12.0).floor() * 12.0;
        for k in 0..16 {
            let z = z_first + k as f32 * 12.0;
            for side in [-1.0f32, 1.0] {
                let x = road_x(z) + side * (ROAD_HALF + 1.5);
                let colour = if (k + if side > 0.0 { 1 } else { 0 }) % 2 == 0 { 0xf0f0f0 } else { 0xe03030 };
                let mut p = self.post.clone();
                for c in p.colors.iter_mut() { *c = colour; }
                self.raster.draw(f, &p, &M4::translate(V3::new(x, ground(x, z) + 0.8, z)), &cam);
            }
        }
        // Packages, bobbing and turning.
        for (i, p) in self.packages.iter().enumerate() {
            if p.taken || (p.z - self.z).abs() > 200.0 { continue; }
            let bob = (self.time * 3.0 + i as f32).sin() * 0.4;
            let m = M4::rotate_y(self.time + i as f32).then(&M4::translate(V3::new(p.x, ground(p.x, p.z) + 1.6 + bob, p.z)));
            self.raster.draw(f, &self.package, &m, &cam);
        }
        // The car, leaning into the slope.
        let ahead = ground(self.x + self.heading.sin() * 2.0, self.z + self.heading.cos() * 2.0);
        let behind = ground(self.x - self.heading.sin() * 2.0, self.z - self.heading.cos() * 2.0);
        let pitch = ((ahead - behind) / 4.0).atan();
        let model = M4::rotate_x(-pitch).then(&M4::rotate_y(self.heading)).then(&M4::translate(V3::new(self.x, car_y, self.z)));
        self.raster.draw(f, &self.car, &model, &cam);

        match self.mode {
            Mode::Title => {
                f.dim(0.7);
                f.text_centered(W / 2, 40, "DRIVE", 0xffd040, true, 5);
                f.text_centered(W / 2, 90, "FETCH THE PACKAGES BEFORE THE CLOCK RUNS OUT", WHITE, false, 1);
                f.text_centered(W / 2, 110, &format!("HIGH SCORE {:06}", self.high), WHITE, true, 1);
                if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 140, "PRESS SPACE", WHITE, true, 1); }
                f.text_centered(W / 2, 170, "UP GAS  DOWN BRAKE  LEFT RIGHT STEER", 0xc0c0d0, false, 1);
                f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 6, VERSION, 0x506080);
                return;
            }
            Mode::Clear(_) => { f.text_centered(W / 2, 70, "ALL DELIVERED", 0xffd040, true, 2); f.text_centered(W / 2, 92, &format!("TIME BONUS {}", (self.time_left * 10.0) as u32), WHITE, true, 1); }
            Mode::Over(_) => { f.text_centered(W / 2, 70, "OUT OF TIME", 0xe03030, true, 2); f.text_centered(W / 2, 92, &format!("SCORE {}", self.score), WHITE, true, 1); }
            Mode::Play => {}
        }
        // The arrow to the nearest package, and how far.
        if let Some(p) = self.nearest() {
            let (dx, dz) = (p.x - self.x, p.z - self.z);
            let bearing = dx.atan2(dz) - self.heading;
            let dist = (dx * dx + dz * dz).sqrt();
            let (ax, ay, r) = (W / 2, 14, 9.0f32);
            let tip = (ax + (bearing.sin() * r) as i32, ay - (bearing.cos() * r) as i32);
            let tail = (ax - (bearing.sin() * r) as i32, ay + (bearing.cos() * r) as i32);
            f.line(tail.0, tail.1, tip.0, tip.1, 0xffd040);
            for side in [-1.0f32, 1.0] {
                let a = bearing + std::f32::consts::PI + side * 0.5;
                f.line(tip.0, tip.1, tip.0 + (a.sin() * 6.0) as i32, tip.1 - (a.cos() * 6.0) as i32, 0xffd040);
            }
            f.text_centered(W / 2, 26, &format!("{:.0} m", dist), 0xffd040, false, 1);
        }
        let left = self.packages.iter().filter(|p| !p.taken).count();
        f.text_big(4, 4, &format!("LEVEL {}  PACKAGES {}/{}", self.level, self.packages.len() - left, self.packages.len()), WHITE);
        let clock = if self.time_left < 10.0 { 0xff4040 } else { WHITE };
        f.text_big(4, 14, &format!("TIME {:2.0}", self.time_left.ceil()), clock);
        f.text_big(W - 4 - Frame::text_width("SCORE 000000", true, 1), 4, &format!("SCORE {:06}", self.score), WHITE);
        f.text_big(4, H - 12, &format!("{:3.0} km/h", self.speed * 3.6), WHITE);
        if !on_road(self.x, self.z) { f.text_centered(W / 2, H - 20, "OFF ROAD", 0xffd040, true, 1); }
    }
}

fn main() {
    run(&mut Drive::new(), Config { width: W, height: H, fps: 30 });
}
