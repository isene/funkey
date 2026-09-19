//! The world: the level, everything moving in it, the player, and the
//! physics they share. Movement slides along walls and other things,
//! shots trace lines, and damage goes through one door.

use crate::info::{self, Act, Fr, Mon, Pickup, Proj};
use crate::player::Player;
use crate::specials::{Button, Light, Mover};
use funkey::wad::{Level, Linedef, ML_BLOCKING, ML_TWOSIDED};
use std::f32::consts::{PI, TAU};

pub const TICRATE: i32 = 35;
pub const MELEERANGE: f32 = 64.0;
pub const MISSILERANGE: f32 = 32.0 * 64.0;
pub const MAXSTEP: f32 = 24.0;
pub const GRAVITY: f32 = 1.0;
pub const FRICTION: f32 = 0.90625;
pub const PLAYER_RADIUS: f32 = 16.0;
pub const PLAYER_HEIGHT: f32 = 56.0;

pub const F_SOLID: u32 = 1;
pub const F_SHOOTABLE: u32 = 2;
pub const F_FLOAT: u32 = 4;
pub const F_NOGRAVITY: u32 = 8;
pub const F_COUNTKILL: u32 = 16;
pub const F_COUNTITEM: u32 = 32;
pub const F_CORPSE: u32 = 64;
pub const F_FUZZY: u32 = 128;
pub const F_AMBUSH: u32 = 256;
pub const F_SKULLFLY: u32 = 512;
pub const F_HANG: u32 = 1024;
pub const F_DROPPED: u32 = 2048;
pub const F_JUSTHIT: u32 = 4096;
pub const F_NOBLOOD: u32 = 8192;
pub const F_JUSTATTACKED: u32 = 16384;
pub const F_NOSPLASH: u32 = 32768;
pub const F_MISSILE: u32 = 65536;

/// Doom's random numbers: a byte at a time.
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Rng { Rng(seed | 1) }
    pub fn next(&mut self) -> u8 {
        let mut x = self.0;
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        self.0 = x;
        (x >> 24) as u8
    }
    /// A spread of -255..255, centred on zero.
    pub fn spread(&mut self) -> f32 { self.next() as f32 - self.next() as f32 }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind { Monster, Missile, Pickup, Deco, Effect }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum St { Spawn, See, Melee, Missile, Pain, Death, XDeath }

pub struct Mobj {
    pub x: f32, pub y: f32, pub z: f32, pub angle: f32,
    pub mx: f32, pub my: f32, pub mz: f32,
    pub radius: f32, pub height: f32,
    pub health: i32,
    pub kind: Kind,
    pub sprite: [u8; 4],
    pub seq: &'static [Fr],
    pub idx: usize,
    pub tics: i32,
    pub st: St,
    pub flags: u32,
    pub mon: Option<&'static Mon>,
    pub proj: Option<&'static Proj>,
    pub pickup: Option<Pickup>,
    pub thing_kind: u16,
    /// Has a target: the player.
    pub awake: bool,
    pub move_dir: u8,
    pub move_count: i32,
    pub reaction: i32,
    pub threshold: i32,
    /// A missile's shooter: None for the player.
    pub owner: Option<usize>,
    pub removed: bool,
    pub floorz: f32,
    pub ceilingz: f32,
    pub sector: usize,
    /// For the arch-vile's fire: the thing it burns at.
    pub fire_for: Option<usize>,
}

impl Mobj {
    pub fn frame(&self) -> &Fr { &self.seq[self.idx.min(self.seq.len().saturating_sub(1))] }
    pub fn solid(&self) -> bool { self.flags & F_SOLID != 0 }
    pub fn shootable(&self) -> bool { self.flags & F_SHOOTABLE != 0 && !self.removed }
    pub fn dist_to(&self, x: f32, y: f32) -> f32 { ((self.x - x).powi(2) + (self.y - y).powi(2)).sqrt() }
}

/// Where a sound plays from; None is the player's own ears.
pub type SoundAt = Option<(f32, f32)>;

pub struct World {
    pub level: Level,
    pub mobjs: Vec<Mobj>,
    pub player: Player,
    pub rng: Rng,
    pub tic: i32,
    pub sounds: Vec<(&'static str, SoundAt)>,
    pub movers: Vec<Mover>,
    pub lights: Vec<Light>,
    pub buttons: Vec<Button>,
    pub anims: Vec<crate::specials::Anim>,
    pub texture_order: Vec<String>,
    pub flat_order: Vec<String>,
    pub exit: Option<bool>,
    pub total_kills: i32,
    pub total_items: i32,
    pub total_secrets: i32,
    /// Teleport landing spots: position, angle.
    pub telespots: Vec<(f32, f32, f32, usize)>,
    /// Which sectors touch which, through two-sided lines.
    pub neighbours: Vec<Vec<usize>>,
    /// Sound reach per sector for waking monsters, as Doom's flood.
    pub sound_level: Vec<u8>,
    pub skill: u8,
    pub sky_map: bool,
}

pub struct Trace {
    pub hit_line: Option<usize>,
    pub hit_thing: Option<usize>,
    /// Where the shot stopped.
    pub x: f32, pub y: f32, pub z: f32,
    pub dist: f32,
}

impl World {
    pub fn new(level: Level, texture_order: Vec<String>, flat_order: Vec<String>, skill: u8) -> World {
        let n = level.sectors.len();
        let mut w = World {
            level, mobjs: Vec::new(), player: Player::new(), rng: Rng::new(0x9e37_79b9), tic: 0, sounds: Vec::new(),
            movers: Vec::new(), lights: Vec::new(), buttons: Vec::new(), anims: Vec::new(), texture_order, flat_order,
            exit: None, total_kills: 0, total_items: 0, total_secrets: 0, telespots: Vec::new(),
            neighbours: vec![Vec::new(); n], sound_level: vec![0; n], skill, sky_map: false,
        };
        w.build_neighbours();
        w.spawn_things();
        w.init_specials();
        w
    }

    fn build_neighbours(&mut self) {
        for l in &self.level.linedefs {
            let (Some(f), Some(b)) = (l.front, l.back) else { continue };
            let (fs, bs) = (self.level.sidedefs[f].sector, self.level.sidedefs[b].sector);
            if fs == bs { continue; }
            if !self.neighbours[fs].contains(&bs) { self.neighbours[fs].push(bs); }
            if !self.neighbours[bs].contains(&fs) { self.neighbours[bs].push(fs); }
        }
    }

    /// Everything the THINGS lump asks for, at the chosen skill.
    fn spawn_things(&mut self) {
        let skill_bit = match self.skill { 0 | 1 => 1, 2 => 2, _ => 4 };
        let things = self.level.things.clone();
        for t in &things {
            if t.kind == 1 {
                self.player.x = t.x; self.player.y = t.y; self.player.angle = t.angle;
                let s = self.level.sector_at(t.x, t.y);
                self.player.sector = s;
                self.player.z = self.level.sectors[s].floor;
                self.player.floorz = self.player.z;
                self.player.ceilingz = self.level.sectors[s].ceiling;
                self.player.viewz = self.player.z + 41.0;
                continue;
            }
            if t.kind == 14 { self.telespots.push((t.x, t.y, t.angle, self.level.sector_at(t.x, t.y))); continue; }
            if t.flags & 16 != 0 || t.flags & skill_bit == 0 { continue; }
            self.spawn_thing(t.kind, t.x, t.y, t.angle, t.flags & 8 != 0);
        }
        for m in &self.mobjs {
            if m.flags & F_COUNTKILL != 0 { self.total_kills += 1; }
            if m.flags & F_COUNTITEM != 0 { self.total_items += 1; }
        }
        self.total_secrets = self.level.sectors.iter().filter(|s| s.special == 9).count() as i32;
    }

    pub fn spawn_thing(&mut self, kind: u16, x: f32, y: f32, angle: f32, ambush: bool) -> Option<usize> {
        if let Some(mon) = info::monster(kind) {
            let mut flags = F_SOLID | F_SHOOTABLE;
            if kind != 2035 && kind != 72 { flags |= F_COUNTKILL; }
            if mon.float { flags |= F_FLOAT | F_NOGRAVITY; }
            if kind == 58 { flags |= F_FUZZY; }
            if kind == 2035 || kind == 72 { flags |= F_NOBLOOD; }
            if kind == 72 { flags |= F_HANG | F_NOGRAVITY; }
            if kind == 16 || kind == 7 { flags |= F_NOSPLASH; }
            if ambush { flags |= F_AMBUSH; }
            let i = self.push(Mobj {
                x, y, z: 0.0, angle, mx: 0.0, my: 0.0, mz: 0.0, radius: mon.radius, height: mon.height, health: mon.health,
                kind: Kind::Monster, sprite: mon.sprite, seq: mon.spawn, idx: 0, tics: 0, st: St::Spawn, flags,
                mon: Some(mon), proj: None, pickup: None, thing_kind: kind, awake: false, move_dir: 8, move_count: 0,
                reaction: 8, threshold: 0, owner: None, removed: false, floorz: 0.0, ceilingz: 0.0, sector: 0, fire_for: None,
            });
            // Start somewhere along the idle frames, so a crowd is not in step.
            let n = self.mobjs[i].seq.len();
            if n > 1 { let r = self.rng.next() as usize % n; self.mobjs[i].idx = r; }
            self.mobjs[i].tics = self.mobjs[i].frame().1 as i32;
            return Some(i);
        }
        if let Some(p) = info::pickup(kind) {
            let mut flags = 0;
            if p.count { flags |= F_COUNTITEM; }
            let i = self.push(Mobj {
                x, y, z: 0.0, angle, mx: 0.0, my: 0.0, mz: 0.0, radius: 20.0, height: 16.0, health: 1000,
                kind: Kind::Pickup, sprite: p.sprite, seq: p.frames, idx: 0, tics: 0, st: St::Spawn, flags,
                mon: None, proj: None, pickup: Some(p), thing_kind: kind, awake: false, move_dir: 8, move_count: 0,
                reaction: 0, threshold: 0, owner: None, removed: false, floorz: 0.0, ceilingz: 0.0, sector: 0, fire_for: None,
            });
            self.mobjs[i].tics = self.mobjs[i].frame().1 as i32;
            return Some(i);
        }
        if let Some(d) = info::decoration(kind) {
            let mut flags = 0;
            if d.solid { flags |= F_SOLID; }
            if d.hang { flags |= F_HANG | F_NOGRAVITY; }
            let i = self.push(Mobj {
                x, y, z: 0.0, angle, mx: 0.0, my: 0.0, mz: 0.0, radius: d.radius, height: d.height, health: 1000,
                kind: Kind::Deco, sprite: d.sprite, seq: d.frames, idx: 0, tics: 0, st: St::Spawn, flags,
                mon: None, proj: None, pickup: None, thing_kind: kind, awake: false, move_dir: 8, move_count: 0,
                reaction: 0, threshold: 0, owner: None, removed: false, floorz: 0.0, ceilingz: 0.0, sector: 0, fire_for: None,
            });
            self.mobjs[i].tics = self.mobjs[i].frame().1 as i32;
            return Some(i);
        }
        None
    }

    /// Add a thing, on the floor or the ceiling of its sector.
    fn push(&mut self, mut m: Mobj) -> usize {
        let s = self.level.sector_at(m.x, m.y);
        m.sector = s;
        m.floorz = self.level.sectors[s].floor;
        m.ceilingz = self.level.sectors[s].ceiling;
        m.z = if m.flags & F_HANG != 0 { m.ceilingz - m.height } else { m.floorz };
        if let Some(slot) = self.mobjs.iter().position(|o| o.removed) {
            self.mobjs[slot] = m;
            slot
        } else {
            self.mobjs.push(m);
            self.mobjs.len() - 1
        }
    }

    /// A short-lived sprite: a puff, blood, teleport fog.
    pub fn spawn_effect(&mut self, sprite: [u8; 4], seq: &'static [Fr], x: f32, y: f32, z: f32) -> usize {
        let i = self.push(Mobj {
            x, y, z: 0.0, angle: 0.0, mx: 0.0, my: 0.0, mz: 0.0, radius: 1.0, height: 1.0, health: 1,
            kind: Kind::Effect, sprite, seq, idx: 0, tics: 0, st: St::Spawn, flags: F_NOGRAVITY,
            mon: None, proj: None, pickup: None, thing_kind: 0, awake: false, move_dir: 8, move_count: 0,
            reaction: 0, threshold: 0, owner: None, removed: false, floorz: 0.0, ceilingz: 0.0, sector: 0, fire_for: None,
        });
        let m = &mut self.mobjs[i];
        m.z = z.clamp(m.floorz, m.ceilingz);
        m.tics = m.frame().1 as i32;
        i
    }

    pub fn sound(&mut self, name: &'static str, at: SoundAt) {
        if !name.is_empty() { self.sounds.push((name, at)); }
    }

    pub fn sound_of(&mut self, list: &'static [&'static str], at: SoundAt) {
        if list.is_empty() { return; }
        let n = self.rng.next() as usize % list.len();
        self.sound(list[n], at);
    }

    pub fn message(&mut self, text: &str) {
        self.player.message = text.to_string();
        self.player.message_tics = 4 * TICRATE;
    }

    // ------------------------------------------------------------ frames

    /// Put a thing into a sequence at its first frame.
    pub fn set_state(&mut self, i: usize, st: St, seq: &'static [Fr]) {
        if seq.is_empty() { return; }
        let m = &mut self.mobjs[i];
        m.st = st;
        m.seq = seq;
        m.idx = 0;
        m.tics = seq[0].1 as i32;
        self.run_action(i);
        // A frame of no length runs and moves straight on.
        let mut guard = 0;
        while !self.mobjs[i].removed && self.mobjs[i].tics == 0 && guard < 8 {
            self.advance(i);
            guard += 1;
        }
    }

    /// Step to the next frame, or to the sequence that follows.
    fn advance(&mut self, i: usize) {
        let m = &mut self.mobjs[i];
        if m.idx + 1 < m.seq.len() {
            m.idx += 1;
            m.tics = m.seq[m.idx].1 as i32;
            self.run_action(i);
            return;
        }
        match m.st {
            St::Spawn => { m.idx = 0; m.tics = m.seq[0].1 as i32; self.run_action(i); }
            St::See => { m.idx = 0; m.tics = m.seq[0].1 as i32; self.run_action(i); }
            St::Missile if m.flags & F_SKULLFLY != 0 && m.seq.len() >= 2 => {
                m.idx = m.seq.len() - 2;
                m.tics = m.seq[m.idx].1 as i32;
            }
            St::Melee | St::Missile | St::Pain => {
                let mon = m.mon;
                let awake = m.awake;
                match mon {
                    Some(mon) if awake && !mon.see.is_empty() => self.set_state(i, St::See, mon.see),
                    Some(mon) => self.set_state(i, St::Spawn, mon.spawn),
                    None => { m.removed = true; }
                }
            }
            St::Death | St::XDeath => { m.tics = -1; }
        }
    }

    fn run_action(&mut self, i: usize) {
        let act = self.mobjs[i].frame().2;
        if act != Act::None { crate::ai::action(self, i, act); }
    }

    // ---------------------------------------------------------- geometry

    pub fn line_points(&self, l: &Linedef) -> (f32, f32, f32, f32) {
        let (x1, y1) = self.level.vertexes[l.v1];
        let (x2, y2) = self.level.vertexes[l.v2];
        (x1, y1, x2, y2)
    }

    /// Both sectors of a line, front then back.
    pub fn line_sectors(&self, l: &Linedef) -> (Option<usize>, Option<usize>) {
        (l.front.map(|s| self.level.sidedefs[s].sector), l.back.map(|s| self.level.sidedefs[s].sector))
    }

    /// The gap a two-sided line leaves: highest floor, lowest ceiling.
    pub fn opening(&self, l: &Linedef) -> Option<(f32, f32, f32)> {
        let (Some(f), Some(b)) = self.line_sectors(l) else { return None };
        let (fs, bs) = (&self.level.sectors[f], &self.level.sectors[b]);
        Some((fs.floor.max(bs.floor), fs.ceiling.min(bs.ceiling), fs.floor.min(bs.floor)))
    }

    /// Which side of a line a point is on: 0 front, 1 back.
    pub fn point_side(&self, l: &Linedef, x: f32, y: f32) -> usize {
        let (x1, y1, x2, y2) = self.line_points(l);
        if (x - x1) * (y2 - y1) - (y - y1) * (x2 - x1) <= 0.0 { 0 } else { 1 }
    }

    /// Whether a circle at (x, y) touches the line, and the push that
    /// would clear it.
    fn circle_line(&self, l: &Linedef, x: f32, y: f32, r: f32) -> Option<(f32, f32)> {
        let (x1, y1, x2, y2) = self.line_points(l);
        let (ex, ey) = (x2 - x1, y2 - y1);
        let len2 = ex * ex + ey * ey;
        if len2 == 0.0 { return None; }
        let t = (((x - x1) * ex + (y - y1) * ey) / len2).clamp(0.0, 1.0);
        let (px, py) = (x1 + ex * t, y1 + ey * t);
        let (ox, oy) = (x - px, y - py);
        let d2 = ox * ox + oy * oy;
        if d2 >= r * r { return None; }
        if d2 < 1e-6 {
            // Dead centre: push off the line's front.
            let len = len2.sqrt();
            return Some((ey / len * r, -ex / len * r));
        }
        let d = d2.sqrt();
        Some((ox / d * (r - d), oy / d * (r - d)))
    }

    /// Does this line stop a body of this height, standing at z, with
    /// its feet on floorz? Monsters also refuse a drop taller than a step.
    fn line_blocks(&self, l: &Linedef, z: f32, height: f32, floorz: f32, monster: bool, missile: bool) -> bool {
        if l.flags & ML_TWOSIDED == 0 || l.back.is_none() || l.front.is_none() { return true; }
        if !missile && l.flags & ML_BLOCKING != 0 { return true; }
        if monster && l.flags & 2 != 0 { return true; }
        let Some((bottom, top, low)) = self.opening(l) else { return true };
        if missile { return z + height > top || z < bottom; }
        if top - bottom < height { return true; }
        if bottom - z > MAXSTEP { return true; }
        if monster && floorz - low > MAXSTEP { return true; }
        false
    }

    /// Move a body to (nx, ny) if it fits, sliding along what it meets.
    /// `me` is the mobj index, or None for the player. Returns whether
    /// it moved at all, the new floor and ceiling, the lines crossed and
    /// the thing bumped into.
    pub fn try_move(&mut self, me: Option<usize>, nx: f32, ny: f32) -> MoveResult {
        let (ox, oy, z, radius, height, floorz, flags, noclip, owner) = match me {
            Some(i) => { let m = &self.mobjs[i]; (m.x, m.y, m.z, m.radius, m.height, m.floorz, m.flags, false, m.owner) }
            None => { let p = &self.player; (p.x, p.y, p.z, PLAYER_RADIUS, PLAYER_HEIGHT, p.floorz, 0, p.noclip, None) }
        };
        let monster = me.is_some() && flags & F_FLOAT == 0 && flags & F_MISSILE == 0;
        let missile = flags & F_MISSILE != 0 || flags & F_SKULLFLY != 0;
        let (mut x, mut y) = (nx, ny);
        let mut res = MoveResult { moved: false, floorz, ceilingz: f32::MAX, hit_line: None, hit_thing: None, crossed: Vec::new(), pickups: Vec::new() };
        let reach = radius + 1.0;
        if !noclip {
            // Slide out of walls, a few times over, since one push can land
            // in the next wall.
            let mut blocked = false;
            for pass in 0..4 {
                let mut pushed = false;
                for (li, l) in self.level.linedefs.iter().enumerate() {
                    let (x1, y1, x2, y2) = self.line_points(l);
                    if x1.min(x2) > x + reach || x1.max(x2) < x - reach || y1.min(y2) > y + reach || y1.max(y2) < y - reach { continue; }
                    let Some((px, py)) = self.circle_line(l, x, y, radius) else { continue };
                    if !self.line_blocks(l, z, height, floorz, monster, missile) { continue; }
                    if missile { res.hit_line = Some(li); return res; }
                    if pass == 3 { blocked = true; break; }
                    x += px; y += py;
                    pushed = true;
                    res.hit_line = Some(li);
                }
                if !pushed { break; }
            }
            if blocked { return res; }
            // Other things: solid ones push back, pickups are noted.
            for j in 0..self.mobjs.len() {
                if Some(j) == me || (missile && Some(j) == owner) { continue; }
                let o = &self.mobjs[j];
                if o.removed { continue; }
                let rr = o.radius + radius;
                let (dx, dy) = (x - o.x, y - o.y);
                if dx.abs() >= rr || dy.abs() >= rr { continue; }
                if me.is_none() && o.kind == Kind::Pickup { res.pickups.push(j); continue; }
                if missile {
                    if (o.shootable() || o.solid()) && z < o.z + o.height && z + height > o.z { res.hit_thing = Some(j); return res; }
                    continue;
                }
                if !o.solid() { continue; }
                // Something a monster can step onto, or under, does not block.
                if z >= o.z + o.height || z + height <= o.z { continue; }
                let d = (dx * dx + dy * dy).sqrt();
                if d < 0.01 { x += rr; } else { x += dx / d * (rr - d); y += dy / d * (rr - d); }
                res.hit_thing = Some(j);
            }
            // Pushed back into a wall? Then stay put.
            for l in &self.level.linedefs {
                let (x1, y1, x2, y2) = self.line_points(l);
                if x1.min(x2) > x + reach || x1.max(x2) < x - reach || y1.min(y2) > y + reach || y1.max(y2) < y - reach { continue; }
                if self.circle_line(l, x, y, radius).is_some() && self.line_blocks(l, z, height, floorz, monster, missile) { return res; }
            }
            // Monsters are blocked by the player; a monster's missile or a
            // charging lost soul hits them. The player's own missiles pass.
            if me.is_some() {
                let p = &self.player;
                let rr = radius + PLAYER_RADIUS;
                let overlap = !p.dead && (x - p.x).abs() < rr && (y - p.y).abs() < rr && z < p.z + PLAYER_HEIGHT && z + height > p.z;
                if overlap {
                    if missile {
                        if owner.is_some() || flags & F_SKULLFLY != 0 { res.hit_thing = Some(usize::MAX); return res; }
                    } else {
                        return res;
                    }
                }
            }
        }
        if (x - nx).abs() > radius || (y - ny).abs() > radius { return res; }
        // Heights at the new spot: the highest floor and lowest ceiling of
        // the sectors the body touches.
        let s = self.level.sector_at(x, y);
        let (mut fz, mut cz) = (self.level.sectors[s].floor, self.level.sectors[s].ceiling);
        if !noclip {
            for l in &self.level.linedefs {
                let (x1, y1, x2, y2) = self.line_points(l);
                if x1.min(x2) > x + reach || x1.max(x2) < x - reach || y1.min(y2) > y + reach || y1.max(y2) < y - reach { continue; }
                if self.circle_line(l, x, y, radius).is_none() { continue; }
                if let Some((bottom, top, _)) = self.opening(l) { fz = fz.max(bottom); cz = cz.min(top); }
            }
        }
        // Lines crossed by the centre, for their specials.
        for (li, l) in self.level.linedefs.iter().enumerate() {
            if l.special == 0 { continue; }
            let (x1, y1, x2, y2) = self.line_points(l);
            if segments_cross(ox, oy, x, y, x1, y1, x2, y2).is_some() {
                res.crossed.push((li, self.point_side(l, ox, oy)));
            }
        }
        res.moved = true;
        res.floorz = fz;
        res.ceilingz = cz;
        match me {
            Some(i) => { let m = &mut self.mobjs[i]; m.x = x; m.y = y; m.floorz = fz; m.ceilingz = cz; m.sector = s; }
            None => { let p = &mut self.player; p.x = x; p.y = y; p.floorz = fz; p.ceilingz = cz; p.sector = s; }
        }
        res
    }

    /// Can a point see another? Walls block, and so does any two-sided
    /// line whose gap the sight line does not pass through.
    pub fn sight(&self, x1: f32, y1: f32, z1: f32, x2: f32, y2: f32, z2: f32) -> bool {
        for l in &self.level.linedefs {
            let (ax, ay, bx, by) = self.line_points(l);
            if ax.min(bx) > x1.max(x2) || ax.max(bx) < x1.min(x2) || ay.min(by) > y1.max(y2) || ay.max(by) < y1.min(y2) { continue; }
            let Some(t) = segments_cross(x1, y1, x2, y2, ax, ay, bx, by) else { continue };
            let Some((bottom, top, _)) = self.opening(l) else { return false };
            if top <= bottom { return false; }
            let z = z1 + (z2 - z1) * t;
            if z < bottom || z > top { return false; }
        }
        true
    }

    pub fn sees_player(&self, i: usize) -> bool {
        let m = &self.mobjs[i];
        let p = &self.player;
        self.sight(m.x, m.y, m.z + m.height * 0.75, p.x, p.y, p.z + PLAYER_HEIGHT * 0.5)
    }

    /// Trace a shot: from (x, y, z) along `angle`, climbing `slope` per
    /// unit, up to `range`. Stops at the first wall or shootable thing.
    pub fn trace(&self, x: f32, y: f32, z: f32, angle: f32, slope: f32, range: f32, skip: Option<usize>) -> Trace {
        let (dx, dy) = (angle.cos(), angle.sin());
        let (ex, ey) = (x + dx * range, y + dy * range);
        let mut best = range;
        let mut hit_line = None;
        for (li, l) in self.level.linedefs.iter().enumerate() {
            let (ax, ay, bx, by) = self.line_points(l);
            if ax.min(bx) > x.max(ex) || ax.max(bx) < x.min(ex) || ay.min(by) > y.max(ey) || ay.max(by) < y.min(ey) { continue; }
            let Some(t) = segments_cross(x, y, ex, ey, ax, ay, bx, by) else { continue };
            let d = t * range;
            if d >= best { continue; }
            let zt = z + slope * d;
            let blocks = match self.opening(l) {
                None => true,
                Some((bottom, top, _)) => zt <= bottom || zt >= top,
            };
            if blocks { best = d; hit_line = Some(li); }
        }
        let mut hit_thing = None;
        for (j, m) in self.mobjs.iter().enumerate() {
            if Some(j) == skip || !m.shootable() { continue; }
            // Distance along the ray and off it.
            let (ox, oy) = (m.x - x, m.y - y);
            let along = ox * dx + oy * dy;
            if along <= 0.0 || along >= best { continue; }
            let off = (ox * dy - oy * dx).abs();
            if off > m.radius { continue; }
            let zt = z + slope * along;
            if zt < m.z || zt > m.z + m.height { continue; }
            best = along;
            hit_thing = Some(j);
            hit_line = None;
        }
        let d = if hit_thing.is_some() { best } else { (best - 4.0).max(0.0) };
        Trace { hit_line, hit_thing, x: x + dx * d, y: y + dy * d, z: z + slope * d, dist: best }
    }

    /// Doom's aim: the nearest shootable thing near the line of fire,
    /// and the slope to its middle. None when nothing is there.
    pub fn aim(&self, x: f32, y: f32, z: f32, angle: f32, range: f32, skip: Option<usize>, at_player: bool) -> Option<(f32, usize)> {
        let (dx, dy) = (angle.cos(), angle.sin());
        let mut best: Option<(f32, f32, usize)> = None;
        if at_player {
            let p = &self.player;
            if !p.dead {
                let (ox, oy) = (p.x - x, p.y - y);
                let along = ox * dx + oy * dy;
                let off = (ox * dy - oy * dx).abs();
                if along > 0.0 && along < range && off <= PLAYER_RADIUS + 8.0 {
                    let slope = (p.z + PLAYER_HEIGHT * 0.5 - z) / along;
                    best = Some((along, slope, usize::MAX));
                }
            }
        } else {
            for (j, m) in self.mobjs.iter().enumerate() {
                if Some(j) == skip || !m.shootable() { continue; }
                let (ox, oy) = (m.x - x, m.y - y);
                let along = ox * dx + oy * dy;
                if along <= 0.0 || along >= range { continue; }
                let off = (ox * dy - oy * dx).abs();
                if off > m.radius + 8.0 { continue; }
                if best.map(|b| along >= b.0).unwrap_or(false) { continue; }
                let slope = (m.z + m.height * 0.5 - z) / along;
                if slope.abs() > 0.6 { continue; }
                best = Some((along, slope, j));
            }
        }
        let (along, slope, j) = best?;
        // A wall in the way means no aim.
        let (tx, ty) = (x + dx * along, y + dy * along);
        if !self.sight(x, y, z, tx, ty, z + slope * along) { return None; }
        Some((slope, j))
    }

    /// One bullet from a shooter: aim, trace, damage, puff or blood.
    pub fn bullet(&mut self, x: f32, y: f32, z: f32, angle: f32, slope: f32, range: f32, damage: i32, shooter: Option<usize>) {
        let t = self.trace(x, y, z, angle, slope, range, shooter);
        if let Some(j) = t.hit_thing {
            let noblood = self.mobjs[j].flags & F_NOBLOOD != 0;
            self.damage(j, damage, shooter, Some((x, y)));
            if noblood { self.spawn_effect(*b"PUFF", info::PUFF, t.x, t.y, t.z); }
            else { self.spawn_blood(t.x, t.y, t.z, damage); }
            return;
        }
        if let Some(li) = t.hit_line {
            let l = &self.level.linedefs[li];
            // Into the sky: no puff.
            let (f, b) = self.line_sectors(l);
            let sky = |s: Option<usize>| s.map(|s| self.level.sectors[s].ceiling_flat == "F_SKY1").unwrap_or(false);
            if (sky(f) || sky(b)) && t.z > f.map(|s| self.level.sectors[s].ceiling).unwrap_or(f32::MAX).min(b.map(|s| self.level.sectors[s].ceiling).unwrap_or(f32::MAX)) { return; }
            if shooter.is_none() { crate::specials::shoot_line(self, li); }
            let dz = self.rng.spread() / 64.0;
            self.spawn_effect(*b"PUFF", info::PUFF, t.x, t.y, t.z + dz);
        }
    }

    /// A monster shooting at the player: hit or miss by the spread.
    pub fn monster_bullet(&mut self, i: usize, angle: f32, damage: i32) {
        let m = &self.mobjs[i];
        let (x, y, z) = (m.x, m.y, m.z + m.height * 0.5 + 8.0);
        let p = &self.player;
        let (dx, dy) = (angle.cos(), angle.sin());
        let (ox, oy) = (p.x - x, p.y - y);
        let along = ox * dx + oy * dy;
        let off = (ox * dy - oy * dx).abs();
        let slope = if along > 0.0 { (p.z + PLAYER_HEIGHT * 0.5 - z) / along } else { 0.0 };
        let t = self.trace(x, y, z, angle, slope, MISSILERANGE, Some(i));
        if !p.dead && along > 0.0 && along < t.dist && off <= PLAYER_RADIUS {
            let (px, py, pz) = (p.x, p.y, p.z + 32.0);
            self.damage_player(damage, Some((x, y)));
            let (jx, jy) = (self.rng.spread() / 32.0, self.rng.spread() / 32.0);
            self.spawn_blood(px + jx, py + jy, pz, damage);
            return;
        }
        if let Some(j) = t.hit_thing {
            // Monsters do not hurt their own kind with bullets.
            let same = self.mobjs[j].thing_kind == self.mobjs[i].thing_kind;
            if !same { self.damage(j, damage, Some(i), Some((x, y))); self.spawn_blood(t.x, t.y, t.z, damage); }
            return;
        }
        if t.hit_line.is_some() { self.spawn_effect(*b"PUFF", info::PUFF, t.x, t.y, t.z); }
    }

    pub fn spawn_blood(&mut self, x: f32, y: f32, z: f32, damage: i32) {
        let seq = if damage <= 8 { info::BLOOD_SMALL } else if damage <= 12 { info::BLOOD_MID } else { info::BLOOD_BIG };
        let dz = self.rng.spread() / 64.0;
        let i = self.spawn_effect(*b"BLUD", seq, x, y, z + dz);
        self.mobjs[i].mz = 2.0;
        self.mobjs[i].flags &= !F_NOGRAVITY;
    }

    /// Fire a missile from a thing toward a point at the given height.
    pub fn spawn_missile(&mut self, from: Option<usize>, proj: &'static Proj, x: f32, y: f32, z: f32, angle: f32, tx: f32, ty: f32, tz: f32) -> usize {
        let dist = ((tx - x).powi(2) + (ty - y).powi(2)).sqrt().max(1.0);
        let tics = (dist / proj.speed).max(1.0);
        let mz = (tz - z) / tics;
        self.launch(from, proj, x, y, z, angle, mz)
    }

    pub fn launch(&mut self, from: Option<usize>, proj: &'static Proj, x: f32, y: f32, z: f32, angle: f32, mz: f32) -> usize {
        let i = self.push(Mobj {
            x, y, z: 0.0, angle, mx: angle.cos() * proj.speed, my: angle.sin() * proj.speed, mz,
            radius: proj.radius, height: proj.height, health: 1, kind: Kind::Missile, sprite: proj.sprite, seq: proj.spawn,
            idx: 0, tics: 0, st: St::Spawn, flags: F_MISSILE | F_NOGRAVITY, mon: None, proj: Some(proj), pickup: None,
            thing_kind: 0, awake: true, move_dir: 8, move_count: 0, reaction: 0, threshold: 0, owner: from, removed: false,
            floorz: 0.0, ceilingz: 0.0, sector: 0, fire_for: None,
        });
        let m = &mut self.mobjs[i];
        m.z = z.clamp(m.floorz, m.ceilingz - m.height);
        m.tics = m.frame().1 as i32;
        // Start a little ahead, clear of the shooter.
        m.x += m.mx; m.y += m.my;
        self.sound(proj.s_fire, Some((x, y)));
        i
    }

    /// A missile's end: it becomes its explosion.
    pub fn explode_missile(&mut self, i: usize) {
        let Some(proj) = self.mobjs[i].proj else { return };
        let m = &mut self.mobjs[i];
        m.mx = 0.0; m.my = 0.0; m.mz = 0.0;
        m.flags &= !F_MISSILE;
        m.kind = Kind::Effect;
        m.sprite = proj.death_sprite;
        let at = Some((m.x, m.y));
        self.set_state(i, St::Death, proj.death);
        self.sound(proj.s_hit, at);
    }

    /// Damage to a thing, from a source (a monster index, or None for
    /// the player), with the spot the blow came from.
    pub fn damage(&mut self, i: usize, damage: i32, source: Option<usize>, from: Option<(f32, f32)>) {
        if !self.mobjs[i].shootable() { return; }
        let mon = self.mobjs[i].mon;
        // Knockback, from the blow's direction.
        if let Some((fx, fy)) = from {
            let m = &mut self.mobjs[i];
            let mass = mon.map(|m| m.mass).unwrap_or(100.0);
            let thrust = damage as f32 * 12.5 / mass;
            let ang = (m.y - fy).atan2(m.x - fx);
            if m.flags & F_SKULLFLY == 0 && m.kind != Kind::Missile { m.mx += ang.cos() * thrust; m.my += ang.sin() * thrust; }
        }
        let m = &mut self.mobjs[i];
        m.health -= damage;
        if m.health <= 0 { self.kill(i, damage); return; }
        let Some(mon) = mon else { return };
        let pain = (self.rng.next() as u32) < mon.pain as u32;
        let m = &mut self.mobjs[i];
        if pain && m.flags & F_SKULLFLY == 0 {
            m.flags |= F_JUSTHIT;
            let at = Some((m.x, m.y));
            self.set_state(i, St::Pain, mon.pain_st);
            if source.is_none() { self.mobjs[i].awake = true; self.mobjs[i].threshold = 100; }
            let _ = at;
        }
        let m = &mut self.mobjs[i];
        m.reaction = 0;
        // Hurt by the player: it comes for you.
        if source.is_none() && !m.awake && m.mon.map(|mm| mm.speed > 0.0).unwrap_or(false) {
            m.awake = true;
            m.threshold = 100;
            if m.st == St::Spawn { self.set_state(i, St::See, mon.see); }
        }
    }

    fn kill(&mut self, i: usize, damage: i32) {
        let m = &mut self.mobjs[i];
        m.flags &= !(F_SHOOTABLE | F_FLOAT | F_SKULLFLY);
        if m.thing_kind != 2035 && m.thing_kind != 72 { m.flags &= !F_NOGRAVITY; }
        m.flags |= F_CORPSE;
        m.height /= 4.0;
        if m.flags & F_COUNTKILL != 0 { self.player.kills += 1; }
        let Some(mon) = m.mon else { m.removed = true; return };
        let at = Some((m.x, m.y));
        let (x, y, angle) = (m.x, m.y, m.angle);
        let gib = !mon.xdeath.is_empty() && m.health < -mon.health;
        let _ = damage;
        if gib { self.set_state(i, St::XDeath, mon.xdeath); } else { self.set_state(i, St::Death, mon.death); }
        if mon.drop != 0 {
            if let Some(d) = self.spawn_thing(mon.drop, x, y, angle, false) {
                self.mobjs[d].flags |= F_DROPPED;
                self.mobjs[d].mz = 4.0;
            }
        }
        let _ = at;
    }

    /// Splash damage around a spot, to everything in reach that can be
    /// seen from it, the player included.
    pub fn radius_damage(&mut self, x: f32, y: f32, z: f32, damage: f32, source: Option<usize>) {
        for j in 0..self.mobjs.len() {
            let m = &self.mobjs[j];
            if !m.shootable() || m.flags & F_NOSPLASH != 0 { continue; }
            let dist = ((m.x - x).abs().max((m.y - y).abs()) - m.radius).max(0.0);
            if dist >= damage { continue; }
            let (mx, my, mz) = (m.x, m.y, m.z + m.height * 0.5);
            if !self.sight(x, y, z, mx, my, mz) { continue; }
            self.damage(j, (damage - dist) as i32, source, Some((x, y)));
        }
        let p = &self.player;
        if !p.dead {
            let dist = ((p.x - x).abs().max((p.y - y).abs()) - PLAYER_RADIUS).max(0.0);
            if dist < damage && self.sight(x, y, z, p.x, p.y, p.z + 28.0) {
                self.damage_player((damage - dist) as i32, Some((x, y)));
            }
        }
    }

    /// The player takes a hit: armour first, then health.
    pub fn damage_player(&mut self, damage: i32, from: Option<(f32, f32)>) {
        let p = &mut self.player;
        if p.dead { return; }
        let mut damage = damage;
        if p.powers[crate::player::PW_INVULN] > 0 || p.god { return; }
        if p.armor > 0 {
            let mut saved = if p.armor_type == 1 { damage / 3 } else { damage / 2 };
            if p.armor <= saved { saved = p.armor; p.armor_type = 0; }
            p.armor -= saved;
            damage -= saved;
        }
        p.health -= damage;
        p.damage_count = (p.damage_count + damage).min(100);
        p.attacker = from;
        if let Some((fx, fy)) = from {
            let ang = (fy - p.y).atan2(fx - p.x);
            let thrust = damage as f32 * 12.5 / 100.0;
            p.mx -= ang.cos() * thrust; p.my -= ang.sin() * thrust;
        }
        p.face_hurt(damage);
        if p.health <= 0 {
            p.health = 0;
            crate::player::die(self);
        } else {
            self.sound("PLPAIN", None);
        }
    }

    // ----------------------------------------------------------- per tic

    /// Advance the world one tic: the player, every thing, the movers.
    pub fn tick(&mut self, input: &funkey::Input) {
        self.tic += 1;
        crate::player::think(self, input);
        for i in 0..self.mobjs.len() {
            if self.mobjs[i].removed { continue; }
            self.think(i);
        }
        crate::specials::tick(self);
        self.player.message_tics -= 1;
    }

    fn think(&mut self, i: usize) {
        // Momentum, with friction on the floor.
        let m = &self.mobjs[i];
        if m.mx != 0.0 || m.my != 0.0 {
            let (nx, ny) = (m.x + m.mx, m.y + m.my);
            let missile = m.flags & F_MISSILE != 0 || m.flags & F_SKULLFLY != 0;
            let r = self.try_move(Some(i), nx, ny);
            if missile {
                if r.hit_thing.is_some() || r.hit_line.is_some() || !r.moved { self.missile_hit(i, r.hit_thing, r.hit_line); return; }
            } else if !r.moved {
                let m = &mut self.mobjs[i];
                m.mx = 0.0; m.my = 0.0;
            } else {
                for (li, side) in r.crossed { crate::specials::cross_line(self, li, side, Some(i)); }
            }
            let m = &mut self.mobjs[i];
            if !missile && m.z <= m.floorz {
                m.mx *= FRICTION; m.my *= FRICTION;
                if m.mx.abs() < 0.1 && m.my.abs() < 0.1 { m.mx = 0.0; m.my = 0.0; }
            }
        }
        // Height.
        let m = &mut self.mobjs[i];
        let missile = m.flags & F_MISSILE != 0;
        m.z += m.mz;
        if m.flags & F_FLOAT != 0 && m.awake && m.flags & F_SKULLFLY == 0 && m.flags & F_CORPSE == 0 {
            let (px, py, pz) = (self.player.x, self.player.y, self.player.z);
            let dist = m.dist_to(px, py);
            let delta = pz + PLAYER_HEIGHT * 0.5 - (m.z + m.height * 0.5);
            if delta < 0.0 && dist < -delta * 3.0 { m.z -= 4.0; } else if delta > 0.0 && dist < delta * 3.0 { m.z += 4.0; }
        }
        if m.z <= m.floorz {
            if missile && m.mz < 0.0 { m.z = m.floorz; self.explode_missile(i); return; }
            m.z = m.floorz;
            if m.mz < 0.0 { m.mz = 0.0; }
        } else if m.flags & F_NOGRAVITY == 0 {
            m.mz = if m.mz == 0.0 { -GRAVITY * 2.0 } else { m.mz - GRAVITY };
        }
        if m.z + m.height > m.ceilingz {
            m.z = m.ceilingz - m.height;
            if m.mz > 0.0 { m.mz = 0.0; }
            if missile {
                let sky = self.level.sectors[m.sector].ceiling_flat == "F_SKY1";
                if sky { m.removed = true; } else { self.explode_missile(i); }
                return;
            }
        }
        // Frames.
        let m = &mut self.mobjs[i];
        if m.tics > 0 {
            m.tics -= 1;
            if m.tics == 0 { self.advance(i); }
        }
    }

    fn missile_hit(&mut self, i: usize, thing: Option<usize>, _line: Option<usize>) {
        let m = &self.mobjs[i];
        let skull = m.flags & F_SKULLFLY != 0;
        let (x, y) = (m.x, m.y);
        if let Some(j) = thing {
            let damage_mult = m.proj.map(|p| p.damage).unwrap_or(3);
            let owner = m.owner;
            let dmg = (0..damage_mult).map(|_| (self.rng.next() % 8) as i32 + 1).sum::<i32>();
            if j == usize::MAX {
                self.damage_player(dmg, Some((x, y)));
            } else {
                let same = owner.map(|o| self.mobjs[o].thing_kind == self.mobjs[j].thing_kind).unwrap_or(false) && !skull;
                if !same { self.damage(j, dmg, owner, Some((x, y))); }
            }
        }
        if skull {
            let m = &mut self.mobjs[i];
            m.flags &= !F_SKULLFLY;
            m.mx = 0.0; m.my = 0.0; m.mz = 0.0;
            if let Some(mon) = m.mon { self.set_state(i, St::See, mon.see); }
            return;
        }
        self.explode_missile(i);
    }

    /// The sprites to draw this frame.
    pub fn visible(&self) -> Vec<funkey::doom::Vis> {
        let mut out = Vec::with_capacity(self.mobjs.len());
        for m in &self.mobjs {
            if m.removed { continue; }
            let f = m.frame();
            out.push(funkey::doom::Vis { x: m.x, y: m.y, z: m.z, angle: m.angle, sprite: m.sprite, frame: info::letter(f.0),
                bright: info::bright(f.0), fuzzy: m.flags & F_FUZZY != 0 });
        }
        out
    }

    /// Distance from the player.
    pub fn player_dist(&self, x: f32, y: f32) -> f32 { ((self.player.x - x).powi(2) + (self.player.y - y).powi(2)).sqrt() }

    /// The player's height above the floor, for aiming.
    pub fn player_mid(&self) -> f32 { self.player.z + PLAYER_HEIGHT * 0.5 }
}

pub struct MoveResult {
    pub moved: bool,
    pub floorz: f32,
    pub ceilingz: f32,
    pub hit_line: Option<usize>,
    pub hit_thing: Option<usize>,
    /// Lines with specials crossed, with the side they were crossed from.
    pub crossed: Vec<(usize, usize)>,
    pub pickups: Vec<usize>,
}

/// Where segment a-b crosses c-d, as a fraction along a-b.
pub fn segments_cross(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32, dx: f32, dy: f32) -> Option<f32> {
    let (r_x, r_y) = (bx - ax, by - ay);
    let (s_x, s_y) = (dx - cx, dy - cy);
    let den = r_x * s_y - r_y * s_x;
    if den.abs() < 1e-6 { return None; }
    let t = ((cx - ax) * s_y - (cy - ay) * s_x) / den;
    let u = ((cx - ax) * r_y - (cy - ay) * r_x) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) { Some(t) } else { None }
}

/// The smallest turn from one angle to another, in -PI..PI.
pub fn angle_diff(from: f32, to: f32) -> f32 {
    let d = (to - from).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}
