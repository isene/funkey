//! Line and sector specials: doors, lifts, floors, ceilings, crushers,
//! stairs, teleports, switches, exits, lights, and the animated flats
//! and walls. Doom's numbers throughout.

use crate::info;
use crate::world::*;
use std::rc::Rc;

pub const VDOORSPEED: f32 = 2.0;
pub const VDOORWAIT: i32 = 150;
pub const PLATWAIT: i32 = 105;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DoorKind { Normal, Open, Close, Close30Open, Raise5Min }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PlatKind { DownWaitUp, Perpetual, RaiseAndChange, RaiseToNearestAndChange }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PlatStatus { Up, Down, Wait }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CeilKind { LowerToFloor, RaiseToHighest, LowerAndCrush, CrushAndRaise, FastCrushAndRaise, SilentCrushAndRaise }

pub enum Mover {
    Door { sector: usize, kind: DoorKind, top: f32, speed: f32, dir: i32, count: i32, blaze: bool },
    Plat { sector: usize, kind: PlatKind, low: f32, high: f32, speed: f32, wait: i32, count: i32, status: PlatStatus, tag: u16 },
    Floor { sector: usize, dest: f32, speed: f32, dir: i32, crush: bool, change: Option<(String, u16)> },
    Ceiling { sector: usize, kind: CeilKind, top: f32, bottom: f32, speed: f32, dir: i32, crush: bool, tag: u16 },
}

impl Mover {
    pub fn sector(&self) -> usize {
        match self { Mover::Door { sector, .. } | Mover::Plat { sector, .. } | Mover::Floor { sector, .. } | Mover::Ceiling { sector, .. } => *sector }
    }
}

pub enum Light {
    Flicker { sector: usize, count: i32, max: i32, min: i32 },
    Flash { sector: usize, count: i32, max: i32, min: i32, maxtime: i32, mintime: i32 },
    Strobe { sector: usize, count: i32, min: i32, max: i32, dark: i32, bright: i32 },
    Glow { sector: usize, min: i32, max: i32, dir: i32 },
}

pub struct Button { pub side: usize, pub slot: u8, pub old: String, pub tics: i32 }

pub enum Target { Floor(usize), Ceiling(usize), Upper(usize), Lower(usize), Middle(usize), Scroll(usize) }

pub struct Anim { pub target: Target, pub names: Rc<Vec<String>>, pub offset: usize }

#[derive(PartialEq)]
enum Plane { Ok, Crushed, PastDest }

const FLAT_ANIMS: &[(&str, &str)] = &[("NUKAGE1", "NUKAGE3"), ("FWATER1", "FWATER4"), ("SWATER1", "SWATER4"), ("LAVA1", "LAVA4"),
    ("BLOOD1", "BLOOD3"), ("RROCK05", "RROCK08"), ("SLIME01", "SLIME04"), ("SLIME05", "SLIME08"), ("SLIME09", "SLIME12")];
const TEX_ANIMS: &[(&str, &str)] = &[("BLODGR1", "BLODGR4"), ("SLADRIP1", "SLADRIP3"), ("BLODRIP1", "BLODRIP4"), ("FIREWALA", "FIREWALL"),
    ("GSTFONT1", "GSTFONT3"), ("FIRELAV3", "FIRELAVA"), ("FIREMAG1", "FIREMAG3"), ("FIREBLU1", "FIREBLU2"), ("ROCKRED1", "ROCKRED3"),
    ("BFALL1", "BFALL4"), ("SFALL1", "SFALL4"), ("WFALL1", "WFALL4"), ("DBRAIN1", "DBRAIN4")];

fn sequence(order: &[String], start: &str, end: &str) -> Option<Rc<Vec<String>>> {
    let s = order.iter().position(|n| n == start)?;
    let e = order.iter().position(|n| n == end)?;
    if e < s { return None; }
    Some(Rc::new(order[s..=e].to_vec()))
}

impl World {
    /// Lights, animations and timed doors the level starts with.
    pub fn init_specials(&mut self) {
        for s in 0..self.level.sectors.len() {
            let special = self.level.sectors[s].special;
            let light = self.level.sectors[s].light;
            let min = self.min_light_around(s);
            match special {
                1 => self.lights.push(Light::Flash { sector: s, count: (self.rng.next() & 64) as i32 + 1, max: light, min, maxtime: 64, mintime: 7 }),
                2 | 4 => self.lights.push(Light::Strobe { sector: s, count: 1, min: if min == light { 0 } else { min }, max: light, dark: 15, bright: 5 }),
                3 | 12 => self.lights.push(Light::Strobe { sector: s, count: 1, min: if min == light { 0 } else { min }, max: light, dark: 35, bright: 5 }),
                13 => self.lights.push(Light::Strobe { sector: s, count: 1, min: if min == light { 0 } else { min }, max: light, dark: 15, bright: 5 }),
                8 => self.lights.push(Light::Glow { sector: s, min, max: light, dir: -1 }),
                17 => self.lights.push(Light::Flicker { sector: s, count: 4, max: light, min: min + 16 }),
                10 => { let floor = self.level.sectors[s].floor; let _ = floor; self.movers.push(Mover::Door { sector: s, kind: DoorKind::Close, top: 0.0, speed: VDOORSPEED, dir: 0, count: 30 * TICRATE, blaze: false }); }
                14 => { let top = self.lowest_ceiling_around(s) - 4.0; self.movers.push(Mover::Door { sector: s, kind: DoorKind::Raise5Min, top, speed: VDOORSPEED, dir: 2, count: 5 * 60 * TICRATE, blaze: false }); }
                _ => {}
            }
        }
        // Animated flats and walls.
        let mut flat_seqs = Vec::new();
        for (a, b) in FLAT_ANIMS { if let Some(seq) = sequence(&self.flat_order, a, b) { flat_seqs.push(seq); } }
        let mut tex_seqs = Vec::new();
        for (a, b) in TEX_ANIMS { if let Some(seq) = sequence(&self.texture_order, a, b) { tex_seqs.push(seq); } }
        for (i, sec) in self.level.sectors.iter().enumerate() {
            for seq in &flat_seqs {
                if let Some(o) = seq.iter().position(|n| *n == sec.floor_flat) { self.anims.push(Anim { target: Target::Floor(i), names: seq.clone(), offset: o }); }
                if let Some(o) = seq.iter().position(|n| *n == sec.ceiling_flat) { self.anims.push(Anim { target: Target::Ceiling(i), names: seq.clone(), offset: o }); }
            }
        }
        for (i, side) in self.level.sidedefs.iter().enumerate() {
            for seq in &tex_seqs {
                if let Some(o) = seq.iter().position(|n| *n == side.upper) { self.anims.push(Anim { target: Target::Upper(i), names: seq.clone(), offset: o }); }
                if let Some(o) = seq.iter().position(|n| *n == side.lower) { self.anims.push(Anim { target: Target::Lower(i), names: seq.clone(), offset: o }); }
                if let Some(o) = seq.iter().position(|n| *n == side.middle) { self.anims.push(Anim { target: Target::Middle(i), names: seq.clone(), offset: o }); }
            }
        }
        let empty = Rc::new(Vec::new());
        for l in &self.level.linedefs {
            if l.special == 48 { if let Some(f) = l.front { self.anims.push(Anim { target: Target::Scroll(f), names: empty.clone(), offset: 0 }); } }
        }
    }

    // ------------------------------------------------- sector questions

    pub fn min_light_around(&self, s: usize) -> i32 {
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].light).fold(self.level.sectors[s].light, i32::min)
    }
    pub fn max_light_around(&self, s: usize) -> i32 {
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].light).fold(0, i32::max)
    }
    pub fn lowest_ceiling_around(&self, s: usize) -> f32 {
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].ceiling).fold(f32::MAX, f32::min)
    }
    pub fn highest_ceiling_around(&self, s: usize) -> f32 {
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].ceiling).fold(0.0, f32::max)
    }
    pub fn lowest_floor_around(&self, s: usize) -> f32 {
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].floor).fold(self.level.sectors[s].floor, f32::min)
    }
    pub fn highest_floor_around(&self, s: usize) -> f32 {
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].floor).fold(-500.0, f32::max)
    }
    /// The lowest neighbouring floor above this one; the floor itself when none.
    pub fn next_highest_floor(&self, s: usize) -> f32 {
        let cur = self.level.sectors[s].floor;
        self.neighbours[s].iter().map(|&n| self.level.sectors[n].floor).filter(|&f| f > cur).fold(f32::MAX, f32::min).min(if self.neighbours[s].iter().any(|&n| self.level.sectors[n].floor > cur) { f32::MAX } else { cur })
    }
    /// The shortest lower texture on the sector's lines, for the floors
    /// that rise by it.
    pub fn shortest_lower_texture(&self, s: usize, heights: &dyn Fn(&str) -> Option<f32>) -> f32 {
        let mut min = 32000.0f32;
        for l in &self.level.linedefs {
            let (Some(f), Some(b)) = (l.front, l.back) else { continue };
            if self.level.sidedefs[f].sector != s && self.level.sidedefs[b].sector != s { continue; }
            for side in [f, b] {
                if let Some(h) = heights(&self.level.sidedefs[side].lower) { if h > 0.0 { min = min.min(h); } }
            }
        }
        min
    }

    pub fn sector_moving(&self, s: usize) -> bool { self.movers.iter().any(|m| m.sector() == s) }

    pub fn tagged(&self, tag: u16) -> Vec<usize> {
        if tag == 0 { return Vec::new(); }
        (0..self.level.sectors.len()).filter(|&s| self.level.sectors[s].tag == tag).collect()
    }

    // ------------------------------------------------------ plane moves

    /// Move a floor or ceiling toward `dest`, minding what stands there.
    fn move_plane(&mut self, s: usize, speed: f32, dest: f32, crush: bool, ceiling: bool, dir: i32) -> Plane {
        let (floor, ceil) = (self.level.sectors[s].floor, self.level.sectors[s].ceiling);
        let cur = if ceiling { ceil } else { floor };
        let dest = if ceiling && dir < 0 { dest.max(floor) } else if !ceiling && dir > 0 { dest.min(ceil) } else { dest };
        let (next, past) = if dir < 0 { if cur - speed <= dest { (dest, true) } else { (cur - speed, false) } }
            else if cur + speed >= dest { (dest, true) } else { (cur + speed, false) };
        if ceiling { self.level.sectors[s].ceiling = next; } else { self.level.sectors[s].floor = next; }
        let nofit = self.change_sector(s, crush);
        // Something in the way: a floor going down never crushes, so any
        // misfit backs it up; a crusher keeps pressing.
        if nofit && !crush && (dir > 0 || ceiling) {
            if ceiling { self.level.sectors[s].ceiling = cur; } else { self.level.sectors[s].floor = cur; }
            self.change_sector(s, false);
            return Plane::Crushed;
        }
        if past { Plane::PastDest } else if nofit { Plane::Crushed } else { Plane::Ok }
    }

    /// After a height change: things stand on the new floor; anything that
    /// no longer fits is crushed, or reports that it does not fit.
    fn change_sector(&mut self, s: usize, crush: bool) -> bool {
        let (floor, ceil) = (self.level.sectors[s].floor, self.level.sectors[s].ceiling);
        let mut nofit = false;
        let hurt = crush && self.tic & 3 == 0;
        for j in 0..self.mobjs.len() {
            let m = &mut self.mobjs[j];
            if m.removed || m.sector != s { continue; }
            m.floorz = floor;
            m.ceilingz = ceil;
            if m.flags & F_HANG != 0 { m.z = ceil - m.height; continue; }
            if m.z < floor || (m.flags & F_NOGRAVITY == 0 && m.z <= floor + 1.0) { m.z = floor; }
            if m.z + m.height <= ceil { continue; }
            if m.flags & F_CORPSE != 0 {
                // Crushed flat: a pool of gibs.
                m.sprite = *b"POL5"; m.seq = info::GIBS; m.idx = 0; m.tics = -1; m.height = 0.0; m.flags &= !F_SOLID;
                continue;
            }
            if m.kind == Kind::Pickup && m.flags & F_DROPPED != 0 { m.removed = true; continue; }
            if !m.shootable() { continue; }
            nofit = true;
            if hurt {
                let (x, y, z) = (m.x, m.y, m.z + m.height * 0.5);
                self.damage(j, 10, None, None);
                self.spawn_blood(x, y, z, 10);
            }
        }
        let p = &mut self.player;
        if p.sector == s {
            p.floorz = floor; p.ceilingz = ceil;
            if p.z < floor { p.z = floor; }
            if p.z + PLAYER_HEIGHT > ceil && !p.dead {
                nofit = true;
                if hurt { self.damage_player(10, None); }
            }
        }
        nofit
    }

    // ------------------------------------------------------------ doors

    /// A door in front of the player, or one a monster walks into.
    fn manual_door(&mut self, li: usize, by_player: bool) -> bool {
        let l = self.level.linedefs[li].clone();
        let Some(back) = l.back else { return false };
        let sector = self.level.sidedefs[back].sector;
        if by_player {
            let need = match l.special { 26 | 32 => Some((0, "You need a blue key to open this door")), 27 | 34 => Some((1, "You need a yellow key to open this door")),
                28 | 33 => Some((2, "You need a red key to open this door")), _ => None };
            if let Some((k, msg)) = need {
                if !self.player.has_key(k) { self.message(msg); self.sound("OOF", None); return false; }
            }
        }
        // Already moving: a normal door reverses.
        for m in &mut self.movers {
            if let Mover::Door { sector: s, kind, dir, count: _, blaze, .. } = m {
                if *s != sector { continue; }
                if *kind != DoorKind::Normal { return false; }
                if *dir == -1 { *dir = 1; self.sounds.push((if *blaze { "BDOPN" } else { "DOROPN" }, None)); }
                else if by_player { *dir = -1; self.sounds.push((if *blaze { "BDCLS" } else { "DORCLS" }, None)); }
                return true;
            }
        }
        let blaze = matches!(l.special, 117 | 118);
        let kind = match l.special { 1 | 26 | 27 | 28 | 117 => DoorKind::Normal, _ => DoorKind::Open };
        let top = self.lowest_ceiling_around(sector) - 4.0;
        self.movers.push(Mover::Door { sector, kind, top, speed: if blaze { VDOORSPEED * 4.0 } else { VDOORSPEED }, dir: 1, count: 0, blaze });
        let at = self.line_mid(li);
        self.sound(if blaze { "BDOPN" } else { "DOROPN" }, Some(at));
        if matches!(l.special, 31 | 32 | 33 | 34 | 118) { self.level.linedefs[li].special = 0; }
        true
    }

    fn tagged_door(&mut self, tag: u16, kind: DoorKind, blaze: bool) -> bool {
        let mut any = false;
        for s in self.tagged(tag) {
            if self.sector_moving(s) { continue; }
            any = true;
            let top = self.lowest_ceiling_around(s) - 4.0;
            let speed = if blaze { VDOORSPEED * 4.0 } else { VDOORSPEED };
            let (dir, count) = match kind { DoorKind::Close | DoorKind::Close30Open => (-1, 0), _ => (1, 0) };
            let at = self.sector_mid(s);
            match kind {
                DoorKind::Close | DoorKind::Close30Open => self.sound(if blaze { "BDCLS" } else { "DORCLS" }, Some(at)),
                _ => self.sound(if blaze { "BDOPN" } else { "DOROPN" }, Some(at)),
            }
            self.movers.push(Mover::Door { sector: s, kind, top, speed, dir, count, blaze });
        }
        any
    }

    fn tick_door(&mut self, i: usize) -> bool {
        let Mover::Door { sector, kind, top, speed, dir, count, blaze } = &self.movers[i] else { return false };
        let (sector, kind, top, speed, mut dir, mut count, blaze) = (*sector, *kind, *top, *speed, *dir, *count, *blaze);
        let mut done = false;
        match dir {
            0 => { count -= 1; if count <= 0 { match kind {
                DoorKind::Close30Open => { dir = 1; self.sound(if blaze { "BDOPN" } else { "DOROPN" }, Some(self.sector_mid(sector))); }
                DoorKind::Close => { dir = -1; self.sound("DORCLS", Some(self.sector_mid(sector))); }
                _ => { dir = -1; self.sound(if blaze { "BDCLS" } else { "DORCLS" }, Some(self.sector_mid(sector))); } } } }
            2 => { count -= 1; if count <= 0 { dir = 1; self.sound("DOROPN", Some(self.sector_mid(sector))); } }
            -1 => {
                let floor = self.level.sectors[sector].floor;
                match self.move_plane(sector, speed, floor, false, true, -1) {
                    Plane::PastDest => match kind { DoorKind::Close30Open => { dir = 0; count = 30 * TICRATE; } _ => done = true },
                    Plane::Crushed => match kind { DoorKind::Close => {} _ => { dir = 1; self.sound(if blaze { "BDOPN" } else { "DOROPN" }, Some(self.sector_mid(sector))); } },
                    Plane::Ok => {}
                }
            }
            _ => {
                if self.move_plane(sector, speed, top, false, true, 1) == Plane::PastDest {
                    match kind { DoorKind::Normal => { dir = 0; count = VDOORWAIT; } _ => done = true }
                }
            }
        }
        if let Mover::Door { dir: d, count: c, kind: k, .. } = &mut self.movers[i] { *d = dir; *c = count; if kind == DoorKind::Raise5Min && dir == 1 { *k = DoorKind::Open; } }
        done
    }

    // ------------------------------------------------------------ plats

    fn do_plat(&mut self, li: usize, tag: u16, kind: PlatKind, amount: f32) -> bool {
        let mut any = false;
        let line_sector = self.level.linedefs[li].front.map(|f| self.level.sidedefs[f].sector);
        for s in self.tagged(tag) {
            if self.sector_moving(s) { continue; }
            any = true;
            let floor = self.level.sectors[s].floor;
            let at = self.sector_mid(s);
            let mover = match kind {
                PlatKind::RaiseToNearestAndChange | PlatKind::RaiseAndChange => {
                    if let Some(ls) = line_sector { let flat = self.level.sectors[ls].floor_flat.clone(); self.level.sectors[s].floor_flat = flat; }
                    if kind == PlatKind::RaiseToNearestAndChange { self.level.sectors[s].special = 0; }
                    let high = if kind == PlatKind::RaiseAndChange { floor + amount } else { self.next_highest_floor(s) };
                    self.sound("STNMOV", Some(at));
                    Mover::Plat { sector: s, kind, low: floor, high, speed: 0.5, wait: 0, count: 0, status: PlatStatus::Up, tag }
                }
                PlatKind::DownWaitUp => {
                    let low = self.lowest_floor_around(s).min(floor);
                    self.sound("PSTART", Some(at));
                    Mover::Plat { sector: s, kind, low, high: floor, speed: amount, wait: PLATWAIT, count: 0, status: PlatStatus::Down, tag }
                }
                PlatKind::Perpetual => {
                    let low = self.lowest_floor_around(s).min(floor);
                    let high = self.highest_floor_around(s).max(floor);
                    self.sound("PSTART", Some(at));
                    let status = if self.rng.next() & 1 == 0 { PlatStatus::Up } else { PlatStatus::Down };
                    Mover::Plat { sector: s, kind, low, high, speed: 1.0, wait: PLATWAIT, count: 0, status, tag }
                }
            };
            self.movers.push(mover);
        }
        any
    }

    fn tick_plat(&mut self, i: usize) -> bool {
        let Mover::Plat { sector, kind, low, high, speed, wait, count, status, .. } = &self.movers[i] else { return false };
        let (sector, kind, low, high, speed, wait, mut count, mut status) = (*sector, *kind, *low, *high, *speed, *wait, *count, *status);
        let mut done = false;
        let at = self.sector_mid(sector);
        match status {
            PlatStatus::Up => {
                let crush = matches!(kind, PlatKind::RaiseAndChange | PlatKind::RaiseToNearestAndChange);
                let res = self.move_plane(sector, speed, high, crush, false, 1);
                if crush && self.tic & 7 == 0 { self.sound("STNMOV", Some(at)); }
                match res {
                    Plane::Crushed if !crush => { count = wait; status = PlatStatus::Down; self.sound("PSTART", Some(at)); }
                    Plane::PastDest => {
                        count = wait; status = PlatStatus::Wait; self.sound("PSTOP", Some(at));
                        if kind != PlatKind::Perpetual { done = true; }
                    }
                    _ => {}
                }
            }
            PlatStatus::Down => {
                if self.move_plane(sector, speed, low, false, false, -1) == Plane::PastDest {
                    count = wait; status = PlatStatus::Wait; self.sound("PSTOP", Some(at));
                }
            }
            PlatStatus::Wait => {
                count -= 1;
                if count <= 0 {
                    status = if self.level.sectors[sector].floor <= low { PlatStatus::Up } else { PlatStatus::Down };
                    self.sound("PSTART", Some(at));
                }
            }
        }
        if let Mover::Plat { count: c, status: s, .. } = &mut self.movers[i] { *c = count; *s = status; }
        done
    }

    // ----------------------------------------------------------- floors

    fn do_floor(&mut self, li: usize, tag: u16, what: u16) -> bool {
        let mut any = false;
        let line_sector = self.level.linedefs[li].front.map(|f| self.level.sidedefs[f].sector);
        for s in self.tagged(tag) {
            if self.sector_moving(s) { continue; }
            any = true;
            let floor = self.level.sectors[s].floor;
            let (dest, speed, dir, crush, change): (f32, f32, i32, bool, Option<(String, u16)>) = match what {
                19 | 83 | 102 | 45 => (self.highest_floor_around(s), 1.0, -1, false, None),
                23 | 38 | 60 | 82 => (self.lowest_floor_around(s), 1.0, -1, false, None),
                36 | 70 | 71 | 98 => { let h = self.highest_floor_around(s); (if h != floor { h + 8.0 } else { h }, 4.0, -1, false, None) }
                5 | 24 | 64 | 91 | 101 => (self.lowest_ceiling_around(s).min(self.level.sectors[s].ceiling), 1.0, 1, false, None),
                55 | 56 | 65 | 94 => (self.lowest_ceiling_around(s).min(self.level.sectors[s].ceiling) - 8.0, 1.0, 1, true, None),
                18 | 69 | 119 | 128 => (self.next_highest_floor(s), 1.0, 1, false, None),
                129 | 130 | 131 | 132 => (self.next_highest_floor(s), 4.0, 1, false, None),
                58 | 92 => (floor + 24.0, 1.0, 1, false, None),
                140 => (floor + 512.0, 1.0, 1, false, None),
                59 | 93 => {
                    let ch = line_sector.map(|ls| (self.level.sectors[ls].floor_flat.clone(), self.level.sectors[ls].special));
                    (floor + 24.0, 1.0, 1, false, ch)
                }
                30 | 96 => {
                    let order = self.texture_order.clone();
                    let h = self.shortest_lower_texture(s, &|n: &str| if order.iter().any(|t| t == n) { Some(crate::TEXTURE_HEIGHTS.with(|m| m.borrow().get(n).copied().unwrap_or(0.0))) } else { None });
                    (floor + if h > 30000.0 { 0.0 } else { h }, 1.0, 1, false, None)
                }
                37 | 84 => {
                    let dest = self.lowest_floor_around(s);
                    let ch = self.neighbours[s].iter().find(|&&n| self.level.sectors[n].floor == dest)
                        .map(|&n| (self.level.sectors[n].floor_flat.clone(), self.level.sectors[n].special));
                    (dest, 1.0, -1, false, ch)
                }
                _ => continue,
            };
            self.movers.push(Mover::Floor { sector: s, dest, speed, dir, crush, change });
        }
        any
    }

    fn tick_floor(&mut self, i: usize) -> bool {
        let Mover::Floor { sector, dest, speed, dir, crush, .. } = &self.movers[i] else { return false };
        let (sector, dest, speed, dir, crush) = (*sector, *dest, *speed, *dir, *crush);
        let at = self.sector_mid(sector);
        let res = self.move_plane(sector, speed, dest, crush, false, dir);
        if self.tic & 7 == 0 { self.sound("STNMOV", Some(at)); }
        if res == Plane::PastDest {
            if let Mover::Floor { change: Some((flat, special)), .. } = &self.movers[i] {
                let (flat, special) = (flat.clone(), *special);
                self.level.sectors[sector].floor_flat = flat;
                self.level.sectors[sector].special = special;
            }
            self.sound("PSTOP", Some(at));
            return true;
        }
        false
    }

    fn build_stairs(&mut self, tag: u16, step: f32, speed: f32) -> bool {
        let mut any = false;
        for s in self.tagged(tag) {
            if self.sector_moving(s) { continue; }
            any = true;
            let texture = self.level.sectors[s].floor_flat.clone();
            let mut height = self.level.sectors[s].floor + step;
            self.movers.push(Mover::Floor { sector: s, dest: height, speed, dir: 1, crush: false, change: None });
            let mut cur = s;
            loop {
                let mut next = None;
                for l in &self.level.linedefs {
                    let (Some(f), Some(b)) = (l.front, l.back) else { continue };
                    let (fs, bs) = (self.level.sidedefs[f].sector, self.level.sidedefs[b].sector);
                    if fs != cur { continue; }
                    if self.level.sectors[bs].floor_flat != texture || self.sector_moving(bs) { continue; }
                    next = Some(bs);
                    break;
                }
                let Some(n) = next else { break };
                height += step;
                self.movers.push(Mover::Floor { sector: n, dest: height, speed, dir: 1, crush: false, change: None });
                cur = n;
            }
        }
        any
    }

    // --------------------------------------------------------- ceilings

    fn do_ceiling(&mut self, tag: u16, kind: CeilKind) -> bool {
        let mut any = false;
        for s in self.tagged(tag) {
            if self.sector_moving(s) { continue; }
            any = true;
            let (floor, ceil) = (self.level.sectors[s].floor, self.level.sectors[s].ceiling);
            let m = match kind {
                CeilKind::FastCrushAndRaise => Mover::Ceiling { sector: s, kind, top: ceil, bottom: floor + 8.0, speed: 2.0, dir: -1, crush: true, tag },
                CeilKind::CrushAndRaise | CeilKind::SilentCrushAndRaise => Mover::Ceiling { sector: s, kind, top: ceil, bottom: floor + 8.0, speed: 1.0, dir: -1, crush: true, tag },
                CeilKind::LowerAndCrush => Mover::Ceiling { sector: s, kind, top: ceil, bottom: floor + 8.0, speed: 1.0, dir: -1, crush: true, tag },
                CeilKind::LowerToFloor => Mover::Ceiling { sector: s, kind, top: ceil, bottom: floor, speed: 1.0, dir: -1, crush: false, tag },
                CeilKind::RaiseToHighest => Mover::Ceiling { sector: s, kind, top: self.highest_ceiling_around(s), bottom: floor, speed: 1.0, dir: 1, crush: false, tag },
            };
            self.movers.push(m);
        }
        any
    }

    fn tick_ceiling(&mut self, i: usize) -> bool {
        let Mover::Ceiling { sector, kind, top, bottom, speed, dir, crush, .. } = &self.movers[i] else { return false };
        let (sector, kind, top, bottom, mut speed, mut dir, crush) = (*sector, *kind, *top, *bottom, *speed, *dir, *crush);
        let at = self.sector_mid(sector);
        let silent = kind == CeilKind::SilentCrushAndRaise;
        let mut done = false;
        if dir > 0 {
            let res = self.move_plane(sector, speed, top, false, true, 1);
            if !silent && self.tic & 7 == 0 { self.sound("STNMOV", Some(at)); }
            if res == Plane::PastDest {
                match kind { CeilKind::RaiseToHighest => done = true, _ => { dir = -1; if silent { self.sound("PSTOP", Some(at)); } } }
            }
        } else {
            let res = self.move_plane(sector, speed, bottom, crush, true, -1);
            if !silent && self.tic & 7 == 0 { self.sound("STNMOV", Some(at)); }
            match res {
                Plane::PastDest => match kind {
                    CeilKind::LowerToFloor | CeilKind::LowerAndCrush => done = true,
                    _ => { dir = 1; if silent { self.sound("PSTOP", Some(at)); } else { speed = match kind { CeilKind::FastCrushAndRaise => 2.0, _ => 1.0 }; } }
                },
                Plane::Crushed => if kind != CeilKind::FastCrushAndRaise { speed = 0.125; },
                Plane::Ok => {}
            }
        }
        if let Mover::Ceiling { dir: d, speed: sp, .. } = &mut self.movers[i] { *d = dir; *sp = speed; }
        done
    }

    // ----------------------------------------------------------- lights

    fn light_turn_on(&mut self, tag: u16, level: i32) {
        for s in self.tagged(tag) {
            let l = if level == 0 { self.max_light_around(s) } else { level };
            self.level.sectors[s].light = l;
        }
    }

    fn lights_off(&mut self, tag: u16) {
        for s in self.tagged(tag) { let l = self.min_light_around(s); self.level.sectors[s].light = l; }
    }

    fn start_strobing(&mut self, tag: u16) {
        for s in self.tagged(tag) {
            if self.lights.iter().any(|l| matches!(l, Light::Strobe { sector, .. } if *sector == s)) { continue; }
            let light = self.level.sectors[s].light;
            let min = self.min_light_around(s);
            self.lights.push(Light::Strobe { sector: s, count: 1, min: if min == light { 0 } else { min }, max: light, dark: 35, bright: 5 });
        }
    }

    fn tick_lights(&mut self) {
        for k in 0..self.lights.len() {
            let r = self.rng.next() as i32;
            match &mut self.lights[k] {
                Light::Flicker { sector, count, max, min } => {
                    *count -= 1;
                    if *count > 0 { continue; }
                    let amount = (r & 3) * 16;
                    self.level.sectors[*sector].light = if *max - amount < *min { *min } else { *max - amount };
                    *count = 4;
                }
                Light::Flash { sector, count, max, min, maxtime, mintime } => {
                    *count -= 1;
                    if *count > 0 { continue; }
                    let sec = &mut self.level.sectors[*sector];
                    if sec.light == *max { sec.light = *min; *count = (r & (*mintime - 1).max(1)) + 1; }
                    else { sec.light = *max; *count = (r & (*maxtime - 1)) + 1; }
                }
                Light::Strobe { sector, count, min, max, dark, bright } => {
                    *count -= 1;
                    if *count > 0 { continue; }
                    let sec = &mut self.level.sectors[*sector];
                    if sec.light == *min { sec.light = *max; *count = *bright; } else { sec.light = *min; *count = *dark; }
                }
                Light::Glow { sector, min, max, dir } => {
                    let sec = &mut self.level.sectors[*sector];
                    sec.light += *dir * 8;
                    if sec.light <= *min { sec.light = *min; *dir = 1; }
                    if sec.light >= *max { sec.light = *max; *dir = -1; }
                }
            }
        }
    }

    // ---------------------------------------------------------- helpers

    pub fn line_mid(&self, li: usize) -> (f32, f32) {
        let (x1, y1, x2, y2) = self.line_points(&self.level.linedefs[li]);
        ((x1 + x2) / 2.0, (y1 + y2) / 2.0)
    }

    pub fn sector_mid(&self, s: usize) -> (f32, f32) {
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for l in &self.level.linedefs {
            let (f, b) = self.line_sectors(l);
            if f != Some(s) && b != Some(s) { continue; }
            let (x1, y1, x2, y2) = self.line_points(l);
            sx += x1 + x2; sy += y1 + y2; n += 2.0;
        }
        if n == 0.0 { (0.0, 0.0) } else { (sx / n, sy / n) }
    }

    /// Flip a switch texture SW1 to SW2 or back on the line's front side.
    fn switch_texture(&mut self, li: usize, repeat: bool) {
        let Some(side) = self.level.linedefs[li].front else { return };
        let sd = &self.level.sidedefs[side];
        let names = [sd.upper.clone(), sd.middle.clone(), sd.lower.clone()];
        for (slot, name) in names.iter().enumerate() {
            let swapped = if let Some(rest) = name.strip_prefix("SW1") { format!("SW2{}", rest) }
                else if let Some(rest) = name.strip_prefix("SW2") { format!("SW1{}", rest) } else { continue };
            if !self.texture_order.iter().any(|t| *t == swapped) { continue; }
            let sd = &mut self.level.sidedefs[side];
            match slot { 0 => sd.upper = swapped, 1 => sd.middle = swapped, _ => sd.lower = swapped }
            if repeat { self.buttons.push(Button { side, slot: slot as u8, old: name.clone(), tics: 35 }); }
            return;
        }
    }

    fn tick_buttons(&mut self) {
        let mut k = 0;
        while k < self.buttons.len() {
            self.buttons[k].tics -= 1;
            if self.buttons[k].tics > 0 { k += 1; continue; }
            let b = self.buttons.remove(k);
            let sd = &mut self.level.sidedefs[b.side];
            match b.slot { 0 => sd.upper = b.old, 1 => sd.middle = b.old, _ => sd.lower = b.old }
            self.sounds.push(("SWTCHN", None));
        }
    }

    fn tick_anims(&mut self) {
        let frame = (self.tic / 8) as usize;
        for a in &self.anims {
            if let Target::Scroll(side) = a.target { self.level.sidedefs[side].x_off += 1.0; continue; }
            if a.names.is_empty() || self.tic % 8 != 0 { continue; }
            let name = a.names[(frame + a.offset) % a.names.len()].clone();
            match a.target {
                Target::Floor(s) => self.level.sectors[s].floor_flat = name,
                Target::Ceiling(s) => self.level.sectors[s].ceiling_flat = name,
                Target::Upper(s) => self.level.sidedefs[s].upper = name,
                Target::Lower(s) => self.level.sidedefs[s].lower = name,
                Target::Middle(s) => self.level.sidedefs[s].middle = name,
                Target::Scroll(_) => {}
            }
        }
    }

    // --------------------------------------------------------- teleport

    fn teleport(&mut self, li: usize, side: usize, who: Option<usize>) -> bool {
        if side == 1 { return false; }
        let tag = self.level.linedefs[li].tag;
        let Some(&(tx, ty, tangle, tsector)) = self.telespots.iter().find(|s| self.level.sectors[s.3].tag == tag) else { return false };
        let floor = self.level.sectors[tsector].floor;
        // Whatever stands on the pad dies.
        for j in 0..self.mobjs.len() {
            if Some(j) == who { continue; }
            let m = &self.mobjs[j];
            if !m.shootable() || (m.x - tx).abs() >= m.radius + 20.0 || (m.y - ty).abs() >= m.radius + 20.0 { continue; }
            self.damage(j, 10000, None, None);
        }
        let (ox, oy, oz) = match who {
            Some(i) => { let m = &mut self.mobjs[i]; let o = (m.x, m.y, m.z); m.x = tx; m.y = ty; m.z = floor; m.floorz = floor; m.ceilingz = self.level.sectors[tsector].ceiling; m.sector = tsector; m.angle = tangle; m.mx = 0.0; m.my = 0.0; m.mz = 0.0; m.reaction = 18; o }
            None => { let p = &mut self.player; let o = (p.x, p.y, p.z); p.x = tx; p.y = ty; p.z = floor; p.floorz = floor; p.ceilingz = self.level.sectors[tsector].ceiling; p.sector = tsector; p.angle = tangle; p.mx = 0.0; p.my = 0.0; p.mz = 0.0; p.reaction = 18; p.viewz = floor + 41.0; o }
        };
        self.spawn_effect(*b"TFOG", info::TELEFOG, ox, oy, oz);
        self.sound("TELEPT", Some((ox, oy)));
        self.spawn_effect(*b"TFOG", info::TELEFOG, tx + tangle.cos() * 20.0, ty + tangle.sin() * 20.0, floor);
        self.sound("TELEPT", Some((tx, ty)));
        true
    }
}

// ------------------------------------------------------------ dispatch

fn is_repeatable(special: u16) -> bool {
    matches!(special, 1 | 26 | 27 | 28 | 117 | 42 | 43 | 45 | 60 | 61 | 62 | 63 | 64 | 65 | 66 | 67 | 68 | 69 | 70 | 72 | 73 | 74 | 75 | 76 | 77
        | 79 | 80 | 81 | 82 | 83 | 84 | 86 | 87 | 88 | 89 | 90 | 91 | 92 | 93 | 94 | 95 | 96 | 97 | 98 | 99 | 105 | 106 | 107 | 114 | 115 | 116
        | 120 | 123 | 126 | 128 | 129 | 132 | 134 | 136 | 138 | 139 | 46)
}

/// A key the player needs for a tagged door switch, if any.
fn key_for(special: u16) -> Option<(usize, &'static str)> {
    match special {
        99 | 133 => Some((0, "You need a blue key to activate this object")),
        134 | 135 => Some((2, "You need a red key to activate this object")),
        136 | 137 => Some((1, "You need a yellow key to activate this object")),
        _ => None,
    }
}

/// The player presses use on a line.
fn use_special(w: &mut World, li: usize, _side: usize) -> bool {
    let (special, tag) = { let l = &w.level.linedefs[li]; (l.special, l.tag) };
    if let Some((k, msg)) = key_for(special) {
        if !w.player.has_key(k) { w.message(msg); w.sound("OOF", None); return false; }
    }
    let ok = match special {
        1 | 26 | 27 | 28 | 31 | 32 | 33 | 34 | 117 | 118 => return w.manual_door(li, true),
        29 | 63 => w.tagged_door(tag, DoorKind::Normal, false),
        50 | 42 => w.tagged_door(tag, DoorKind::Close, false),
        103 | 61 | 133 | 135 | 137 | 99 | 134 | 136 => w.tagged_door(tag, DoorKind::Open, matches!(special, 133 | 135 | 137 | 99 | 134 | 136)),
        111 | 114 => w.tagged_door(tag, DoorKind::Normal, true),
        112 | 115 => w.tagged_door(tag, DoorKind::Open, true),
        113 | 116 => w.tagged_door(tag, DoorKind::Close, true),
        21 | 62 => w.do_plat(li, tag, PlatKind::DownWaitUp, 4.0),
        122 | 123 => w.do_plat(li, tag, PlatKind::DownWaitUp, 8.0),
        14 | 67 => w.do_plat(li, tag, PlatKind::RaiseAndChange, 32.0),
        15 | 66 => w.do_plat(li, tag, PlatKind::RaiseAndChange, 24.0),
        20 | 68 => w.do_plat(li, tag, PlatKind::RaiseToNearestAndChange, 0.0),
        18 | 23 | 45 | 55 | 60 | 64 | 65 | 69 | 70 | 71 | 101 | 102 | 131 | 132 | 140 => w.do_floor(li, tag, special),
        7 => w.build_stairs(tag, 8.0, 0.25),
        127 => w.build_stairs(tag, 16.0, 4.0),
        41 | 43 => w.do_ceiling(tag, CeilKind::LowerToFloor),
        49 => w.do_ceiling(tag, CeilKind::CrushAndRaise),
        138 => { w.light_turn_on(tag, 255); true }
        139 => { w.light_turn_on(tag, 35); true }
        11 => { w.exit = Some(false); w.switch_texture(li, false); w.sound("SWTCHX", None); return true; }
        51 => { w.exit = Some(true); w.switch_texture(li, false); w.sound("SWTCHX", None); return true; }
        _ => return false,
    };
    if ok {
        let repeat = is_repeatable(special);
        w.switch_texture(li, repeat);
        let at = w.line_mid(li);
        w.sound("SWTCHN", Some(at));
        if !repeat { w.level.linedefs[li].special = 0; }
    }
    ok
}

/// Something walked over a line with a special.
pub fn cross_line(w: &mut World, li: usize, side: usize, who: Option<usize>) {
    let (special, tag) = { let l = &w.level.linedefs[li]; (l.special, l.tag) };
    if special == 0 { return; }
    if let Some(i) = who {
        let m = &w.mobjs[i];
        if m.kind != Kind::Monster || !matches!(special, 4 | 10 | 39 | 88 | 97 | 125 | 126) { return; }
    } else if matches!(special, 125 | 126) { return; }
    let ok = match special {
        2 | 86 => w.tagged_door(tag, DoorKind::Open, false),
        3 | 75 => w.tagged_door(tag, DoorKind::Close, false),
        4 | 90 => w.tagged_door(tag, DoorKind::Normal, false),
        16 | 76 => w.tagged_door(tag, DoorKind::Close30Open, false),
        108 | 105 => w.tagged_door(tag, DoorKind::Normal, true),
        109 | 106 => w.tagged_door(tag, DoorKind::Open, true),
        110 | 107 => w.tagged_door(tag, DoorKind::Close, true),
        10 | 88 => w.do_plat(li, tag, PlatKind::DownWaitUp, 4.0),
        121 | 120 => w.do_plat(li, tag, PlatKind::DownWaitUp, 8.0),
        22 | 95 => w.do_plat(li, tag, PlatKind::RaiseToNearestAndChange, 0.0),
        53 | 87 => w.do_plat(li, tag, PlatKind::Perpetual, 0.0),
        54 | 89 => { w.movers.retain(|m| !matches!(m, Mover::Plat { tag: t, .. } if *t == tag)); true }
        5 | 19 | 30 | 36 | 37 | 38 | 56 | 58 | 59 | 82 | 83 | 84 | 91 | 92 | 93 | 94 | 96 | 98 | 119 | 128 | 129 | 130 => w.do_floor(li, tag, special),
        8 => w.build_stairs(tag, 8.0, 0.25),
        100 => w.build_stairs(tag, 16.0, 4.0),
        40 => w.do_ceiling(tag, CeilKind::RaiseToHighest),
        44 | 72 => w.do_ceiling(tag, CeilKind::LowerAndCrush),
        6 | 77 => w.do_ceiling(tag, CeilKind::FastCrushAndRaise),
        25 | 73 => w.do_ceiling(tag, CeilKind::CrushAndRaise),
        141 => w.do_ceiling(tag, CeilKind::SilentCrushAndRaise),
        57 | 74 => { w.movers.retain(|m| !matches!(m, Mover::Ceiling { tag: t, .. } if *t == tag)); true }
        12 | 80 => { w.light_turn_on(tag, 0); true }
        13 | 81 => { w.light_turn_on(tag, 255); true }
        35 | 79 | 104 => { w.lights_off(tag); true }
        17 => { w.start_strobing(tag); true }
        39 | 97 | 125 | 126 => w.teleport(li, side, who),
        52 => { if who.is_none() { w.exit = Some(false); } true }
        124 => { if who.is_none() { w.exit = Some(true); } true }
        _ => false,
    };
    if ok && !is_repeatable(special) { w.level.linedefs[li].special = 0; }
}

/// A bullet hit a line with a special.
pub fn shoot_line(w: &mut World, li: usize) {
    let (special, tag) = { let l = &w.level.linedefs[li]; (l.special, l.tag) };
    let ok = match special {
        24 => w.do_floor(li, tag, 24),
        46 => w.tagged_door(tag, DoorKind::Open, false),
        47 => w.do_plat(li, tag, PlatKind::RaiseToNearestAndChange, 0.0),
        _ => return,
    };
    if ok {
        w.switch_texture(li, special == 46);
        let at = w.line_mid(li);
        w.sound("SWTCHN", Some(at));
        if special != 46 { w.level.linedefs[li].special = 0; }
    }
}

/// A monster walked into a door: plain doors open for them.
pub fn monster_use_door(w: &mut World, li: usize) -> bool {
    if w.level.linedefs[li].special != 1 { return false; }
    w.manual_door(li, false)
}

/// The player presses use: the first line within reach that does something.
pub fn use_lines(w: &mut World) {
    let p = &w.player;
    let (x, y, angle) = (p.x, p.y, p.angle);
    let (ex, ey) = (x + angle.cos() * 64.0, y + angle.sin() * 64.0);
    let mut hits: Vec<(f32, usize)> = Vec::new();
    for (li, l) in w.level.linedefs.iter().enumerate() {
        let (x1, y1, x2, y2) = w.line_points(l);
        if let Some(t) = segments_cross(x, y, ex, ey, x1, y1, x2, y2) { hits.push((t, li)); }
    }
    hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_, li) in hits {
        let l = w.level.linedefs[li].clone();
        if l.special == 0 {
            let closed = match w.opening(&l) { None => true, Some((bottom, top, _)) => top <= bottom };
            if closed { w.sound("NOWAY", None); return; }
            continue;
        }
        let side = w.point_side(&l, x, y);
        use_special(w, li, side);
        return;
    }
}

/// Gunfire: monsters in earshot wake up. The sound floods from sector
/// to sector through open gaps, and one sound-blocking line dampens it.
pub fn noise_alert(w: &mut World) {
    let start = w.player.sector;
    for l in w.sound_level.iter_mut() { *l = 0; }
    let mut stack = vec![(start, 1u8)];
    w.sound_level[start] = 1;
    while let Some((s, level)) = stack.pop() {
        for l in &w.level.linedefs {
            let (Some(f), Some(b)) = w.line_sectors(l) else { continue };
            if f != s && b != s { continue; }
            let other = if f == s { b } else { f };
            let Some((bottom, top, _)) = w.opening(l) else { continue };
            if top <= bottom { continue; }
            let next = if l.flags & 64 != 0 { if level == 2 { continue; } 2 } else { level };
            if w.sound_level[other] != 0 && w.sound_level[other] <= next { continue; }
            w.sound_level[other] = next;
            stack.push((other, next));
        }
    }
}

/// A boss fell: some maps react.
pub fn boss_death(w: &mut World, i: usize) {
    let kind = w.mobjs[i].thing_kind;
    let name = w.level.name.clone();
    let action: u8 = match (name.as_str(), kind) {
        ("E1M8", 3003) | ("E4M8", 7) | ("MAP07", 67) => 1,
        ("E2M8", 16) | ("E3M8", 7) => 2,
        ("E4M6", 16) => 3,
        ("MAP07", 68) => 4,
        _ => return,
    };
    if w.mobjs.iter().enumerate().any(|(j, m)| j != i && !m.removed && m.thing_kind == kind && m.health > 0) { return; }
    match action {
        1 => { for s in w.tagged(666) { if !w.sector_moving(s) { let dest = w.lowest_floor_around(s); w.movers.push(Mover::Floor { sector: s, dest, speed: 1.0, dir: -1, crush: false, change: None }); } } }
        2 => w.exit = Some(false),
        3 => { w.tagged_door(666, DoorKind::Open, true); }
        _ => { for s in w.tagged(667) { if !w.sector_moving(s) {
            let order = w.texture_order.clone();
            let h = w.shortest_lower_texture(s, &|n: &str| if order.iter().any(|t| t == n) { Some(crate::TEXTURE_HEIGHTS.with(|m| m.borrow().get(n).copied().unwrap_or(0.0))) } else { None });
            let dest = w.level.sectors[s].floor + if h > 30000.0 { 0.0 } else { h };
            w.movers.push(Mover::Floor { sector: s, dest, speed: 1.0, dir: 1, crush: false, change: None });
        } } }
    }
}

pub fn keen_die(w: &mut World) {
    if w.mobjs.iter().any(|m| !m.removed && m.thing_kind == 72 && m.health > 0) { return; }
    w.tagged_door(666, DoorKind::Open, false);
}

/// Everything that moves on its own each tic.
pub fn tick(w: &mut World) {
    let mut k = 0;
    while k < w.movers.len() {
        let done = match &w.movers[k] {
            Mover::Door { .. } => w.tick_door(k),
            Mover::Plat { .. } => w.tick_plat(k),
            Mover::Floor { .. } => w.tick_floor(k),
            Mover::Ceiling { .. } => w.tick_ceiling(k),
        };
        if done { w.movers.remove(k); } else { k += 1; }
    }
    w.tick_lights();
    w.tick_buttons();
    w.tick_anims();
}
