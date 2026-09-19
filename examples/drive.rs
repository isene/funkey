//! drive: a car on a road over the hills, on funkey's 3D rasterizer.
//! Smuggler's Run in spirit: go fast, leave the road, regret it.
//!
//!     cargo run --release --example drive
//!
//! Up and Down for the gas and the brake, Left and Right steer, Q quits.

use funkey::*;

const W: i32 = 320;
const H: i32 = 200;
const ROAD_HALF: f32 = 9.0;
const CELL: f32 = 6.0;

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

struct Drive {
    raster: Raster,
    car: Mesh,
    post: Mesh,
    x: f32, z: f32, heading: f32, speed: f32,
    distance: f32,
    audio: Audio,
    engine_on: bool,
    time: f32,
}

impl Drive {
    fn new() -> Drive {
        let mut car = Mesh::cuboid(3.2, 1.0, 5.6, 0xd82020);
        car.extend(&Mesh::cuboid(2.6, 0.9, 2.6, 0x203040), &M4::translate(V3::new(0.0, 0.95, -0.3)));
        for (sx, sz) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            car.extend(&Mesh::cuboid(0.6, 1.1, 1.1, 0x181818), &M4::translate(V3::new(sx * 1.6, -0.35, sz * 1.9)));
        }
        let mut raster = Raster::new(W, H);
        raster.fog = Some((0x9fb8d8, 180.0));
        raster.light = V3::new(0.3, 1.0, -0.5).norm();
        let mut audio = Audio::open();
        let hum = Sample::loop_tone(Wave::Saw, 55.0, 0.5, 0.25);
        audio.play_loop(1, &hum, 0.25);
        let post = Mesh::cuboid(0.5, 1.6, 0.5, 0xf0f0f0);
        Drive { raster, car, post, x: 0.0, z: 0.0, heading: 0.0, speed: 0.0, distance: 0.0, audio, engine_on: true, time: 0.0 }
    }
}

impl Game for Drive {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        let on_road = on_road(self.x, self.z);
        let top = if on_road { 70.0 } else { 24.0 };
        if input.held(Key::Up) { self.speed += 22.0 * dt; }
        if input.held(Key::Down) { self.speed -= 40.0 * dt; }
        self.speed -= self.speed * (if on_road { 0.12 } else { 0.9 }) * dt;
        self.speed = self.speed.clamp(0.0, top);
        let steer = (input.held(Key::Right) as i32 - input.held(Key::Left) as i32) as f32;
        self.heading += steer * 1.4 * dt * (self.speed / 40.0).clamp(0.25, 1.0);
        self.x += self.heading.sin() * self.speed * dt;
        self.z += self.heading.cos() * self.speed * dt;
        self.distance += self.speed * dt;
        if self.engine_on { self.audio.rate(1, 0.7 + self.speed / 45.0); self.audio.volume(1, 0.15 + self.speed / 300.0); }
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
        // The car, leaning into the slope.
        let ahead = ground(self.x + self.heading.sin() * 2.0, self.z + self.heading.cos() * 2.0);
        let behind = ground(self.x - self.heading.sin() * 2.0, self.z - self.heading.cos() * 2.0);
        let pitch = ((ahead - behind) / 4.0).atan();
        let model = M4::rotate_x(-pitch).then(&M4::rotate_y(self.heading)).then(&M4::translate(V3::new(self.x, car_y, self.z)));
        self.raster.draw(f, &self.car, &model, &cam);
        f.text_big(4, 4, &format!("{:3.0} km/h", self.speed * 3.6), WHITE);
        f.text_big(4, 14, &format!("{:5.0} m", self.distance), WHITE);
        if !on_road(self.x, self.z) { f.text_centered(W / 2, 30, "OFF ROAD", 0xffd040, true, 1); }
    }
}

fn main() {
    run(&mut Drive::new(), Config { width: W, height: H, fps: 30 });
}
