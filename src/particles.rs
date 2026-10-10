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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particles_fly_fall_and_die() {
        let mut p = Particles::new();
        p.burst(10.0, 10.0, 20, 50.0, 1.0, 0xff0000);
        assert_eq!(p.list.len(), 20);
        assert!(p.list.iter().all(|q| (0.5..=1.0).contains(&q.life)), "each lives half the time to all of it");
        let mut f = Frame::new(20, 20);
        p.draw(&mut f, 0, 0);
        assert_eq!(f.get(10, 10), 0xff0000, "they start where the burst was");
        let before: f32 = p.list.iter().map(|q| q.vy).sum();
        p.update(0.1);
        let after: f32 = p.list.iter().map(|q| q.vy).sum();
        assert!((after - before - 20.0 * 12.0).abs() < 0.01, "gravity pulls each one down");
        assert!(p.list.iter().any(|q| q.x != 10.0 || q.y != 10.0));
        p.update(1.0);
        assert!(p.is_empty(), "past their time they are gone");
    }
}
