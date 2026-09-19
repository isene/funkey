//! Sparks, dust, confetti: little squares that fly, fall and fade.

use crate::frame::{Frame, Rgb};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub x: f32, pub y: f32,
    pub vx: f32, pub vy: f32,
    /// Seconds left.
    pub life: f32,
    pub color: Rgb,
    pub size: i32,
}

pub struct Particles {
    pub list: Vec<Particle>,
    /// Pixels a second squared, down.
    pub gravity: f32,
    rng: Rng,
}

impl Default for Particles {
    fn default() -> Self { Particles::new() }
}

impl Particles {
    pub fn new() -> Particles { Particles { list: Vec::new(), gravity: 120.0, rng: Rng::new(11) } }

    pub fn spawn(&mut self, p: Particle) { self.list.push(p); }

    /// `n` particles from a point, flying every way at up to `speed`.
    pub fn burst(&mut self, x: f32, y: f32, n: usize, speed: f32, life: f32, color: Rgb) {
        for _ in 0..n {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let s = self.rng.range(speed * 0.3, speed);
            self.list.push(Particle { x, y, vx: a.cos() * s, vy: a.sin() * s, life: self.rng.range(life * 0.5, life), color, size: 1 });
        }
    }

    pub fn update(&mut self, dt: f32) {
        let g = self.gravity;
        for p in &mut self.list {
            p.vy += g * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.list.retain(|p| p.life > 0.0);
    }

    pub fn draw(&self, f: &mut Frame, cam_x: i32, cam_y: i32) {
        for p in &self.list {
            f.rect(p.x as i32 - cam_x, p.y as i32 - cam_y, p.size, p.size, p.color);
        }
    }

    pub fn is_empty(&self) -> bool { self.list.is_empty() }
}
