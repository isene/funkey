//! The player: moving, using, the weapon in hand, pickups, powers, and
//! the face in the status bar.

use crate::info::{self, Act, AmmoKind, Effect, Fr, WEAPONS, W_BFG, W_CHAINGUN, W_CHAINSAW, W_FIST, W_PISTOL, W_PLASMA, W_ROCKET, W_SHOTGUN, W_SSG};
use crate::world::*;
use funkey::{Input, Key};
use std::f32::consts::PI;

pub const PW_INVULN: usize = 0;
pub const PW_BERSERK: usize = 1;
pub const PW_INVIS: usize = 2;
pub const PW_SUIT: usize = 3;
pub const PW_MAP: usize = 4;
pub const PW_VISOR: usize = 5;

pub const WEAPONTOP: f32 = 32.0;
pub const WEAPONBOTTOM: f32 = 128.0;
const VIEWHEIGHT: f32 = 41.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WState { Ready, Attack, Lower, Raise }

pub struct Player {
    pub x: f32, pub y: f32, pub z: f32, pub angle: f32,
    pub mx: f32, pub my: f32, pub mz: f32,
    pub viewz: f32, pub viewheight: f32, pub deltaviewheight: f32, pub bob: f32,
    pub floorz: f32, pub ceilingz: f32, pub sector: usize,
    pub health: i32, pub armor: i32, pub armor_type: i32,
    pub ammo: [i32; 4], pub max_ammo: [i32; 4],
    pub weapons: [bool; 9], pub weapon: usize, pub pending: Option<usize>,
    pub keys: [bool; 6],
    pub powers: [i32; 6],
    pub wstate: WState,
    pub psp_seq: &'static [Fr], pub psp_idx: usize, pub psp_tics: i32,
    pub psp_x: f32, pub psp_y: f32,
    pub flash: Option<(&'static [Fr], usize, i32)>,
    pub refire: i32,
    pub attack_down: bool,
    pub dead: bool,
    pub attacker: Option<(f32, f32)>,
    pub damage_count: i32, pub bonus_count: i32, pub extra_light: i32,
    pub kills: i32, pub items: i32, pub secrets: i32,
    pub god: bool, pub noclip: bool,
    pub message: String, pub message_tics: i32,
    pub reaction: i32,
    // The face.
    pub look_idx: usize, pub look_tics: i32, pub ouch_tics: i32, pub evil_tics: i32,
    pub hurt_tics: i32, pub hurt_dir: i32, pub rampage: i32,
    pub backpack: bool,
    pub level_time: i32,
}

impl Player {
    pub fn new() -> Player {
        let mut weapons = [false; 9];
        weapons[W_FIST] = true;
        weapons[W_PISTOL] = true;
        Player {
            x: 0.0, y: 0.0, z: 0.0, angle: 0.0, mx: 0.0, my: 0.0, mz: 0.0,
            viewz: VIEWHEIGHT, viewheight: VIEWHEIGHT, deltaviewheight: 0.0, bob: 0.0,
            floorz: 0.0, ceilingz: 0.0, sector: 0,
            health: 100, armor: 0, armor_type: 0, ammo: [50, 0, 0, 0], max_ammo: info::MAX_AMMO,
            weapons, weapon: W_PISTOL, pending: None, keys: [false; 6], powers: [0; 6],
            wstate: WState::Raise, psp_seq: WEAPONS[W_PISTOL].ready, psp_idx: 0, psp_tics: 1, psp_x: 1.0, psp_y: WEAPONBOTTOM,
            flash: None, refire: 0, attack_down: false, dead: false, attacker: None,
            damage_count: 0, bonus_count: 0, extra_light: 0, kills: 0, items: 0, secrets: 0,
            god: false, noclip: false, message: String::new(), message_tics: 0, reaction: 0,
            look_idx: 0, look_tics: 0, ouch_tics: 0, evil_tics: 0, hurt_tics: 0, hurt_dir: 0, rampage: 0,
            backpack: false, level_time: 0,
        }
    }

    /// What carries over to the next level: health, armour, weapons, ammo.
    pub fn inherit(&mut self, old: &Player) {
        self.health = old.health; self.armor = old.armor; self.armor_type = old.armor_type;
        self.ammo = old.ammo; self.max_ammo = old.max_ammo; self.weapons = old.weapons;
        self.weapon = old.weapon; self.backpack = old.backpack;
        self.psp_seq = WEAPONS[self.weapon].ready;
        self.god = old.god; self.noclip = old.noclip;
    }

    pub fn face_hurt(&mut self, damage: i32) {
        if damage >= 20 { self.ouch_tics = 35; }
        self.hurt_tics = 35;
        self.hurt_dir = match self.attacker {
            Some((ax, ay)) => {
                let d = angle_diff(self.angle, (ay - self.y).atan2(ax - self.x));
                if d.abs() < PI / 4.0 { 0 } else if d > 0.0 { -1 } else { 1 }
            }
            None => 0,
        };
    }

    pub fn has_key(&self, colour: usize) -> bool { self.keys[colour] || self.keys[colour + 3] }

    /// The face picture for the status bar.
    pub fn face_pic(&self) -> String {
        if self.dead { return "STFDEAD0".into(); }
        if self.god || self.powers[PW_INVULN] > 0 { return "STFGOD0".into(); }
        let p = ((100 - self.health.clamp(0, 100)) * 5 / 101).clamp(0, 4);
        if self.evil_tics > 0 { return format!("STFEVL{}0", p); }
        if self.ouch_tics > 0 { return format!("STFOUCH{}0", p); }
        if self.hurt_tics > 0 && self.hurt_dir != 0 { return format!("STF{}{}0", if self.hurt_dir < 0 { "TL" } else { "TR" }, p); }
        if self.rampage >= 70 { return format!("STFKILL{}0", p); }
        format!("STFST{}{}", p, self.look_idx)
    }
}

pub fn think(w: &mut World, input: &Input) {
    let dead = w.player.dead;
    w.player.level_time += 1;
    powers(w);
    if dead {
        death_think(w);
    } else {
        movement(w, input);
        if w.player.reaction > 0 { w.player.reaction -= 1; }
        if input.pressed(Key::Char('e')) || input.pressed(Key::Enter) { crate::specials::use_lines(w); }
        weapon_keys(w, input);
        w.player.attack_down = input.held(Key::Space) || input.held(Key::Char('f'));
        sector_special(w);
    }
    psprites(w);
    face_tick(w);
    calc_height(w);
    let p = &mut w.player;
    if p.damage_count > 0 { p.damage_count -= 1; }
    if p.bonus_count > 0 { p.bonus_count -= 1; }
}

fn powers(w: &mut World) {
    let p = &mut w.player;
    for k in [PW_INVULN, PW_INVIS, PW_SUIT, PW_VISOR] { if p.powers[k] > 0 { p.powers[k] -= 1; } }
    if p.powers[PW_BERSERK] > 0 { p.powers[PW_BERSERK] += 1; }
}

fn movement(w: &mut World, input: &Input) {
    let turn = 7f32.to_radians();
    let p = &mut w.player;
    if p.reaction == 0 {
        if input.tapped(Key::Left) { p.angle += 10f32.to_radians(); }
        if input.tapped(Key::Right) { p.angle -= 10f32.to_radians(); }
        if input.motion(Key::Left) { p.angle += turn; }
        if input.motion(Key::Right) { p.angle -= turn; }
        let (fx, fy) = (p.angle.cos(), p.angle.sin());
        let on_ground = p.z <= p.floorz;
        let fwd = 1.5625;
        let side = 1.25;
        let mut thrust = |key: Key, ax: f32, ay: f32, amount: f32| {
            if input.tapped(key) { p.mx += ax * amount * 6.0; p.my += ay * amount * 6.0; }
            if input.motion(key) && on_ground { p.mx += ax * amount; p.my += ay * amount; }
        };
        thrust(Key::Up, fx, fy, fwd); thrust(Key::Char('w'), fx, fy, fwd);
        thrust(Key::Down, -fx, -fy, fwd); thrust(Key::Char('s'), -fx, -fy, fwd);
        thrust(Key::Char('a'), fy, -fx, side); thrust(Key::Char(','), fy, -fx, side);
        thrust(Key::Char('d'), -fy, fx, side); thrust(Key::Char('.'), -fy, fx, side);
    }
    let speed = (p.mx * p.mx + p.my * p.my).sqrt();
    if speed > 30.0 { p.mx *= 30.0 / speed; p.my *= 30.0 / speed; }
    let (ox, oy) = (p.x, p.y);
    let (nx, ny) = (p.x + p.mx, p.y + p.my);
    let wanted = ((nx - ox).powi(2) + (ny - oy).powi(2)).sqrt();
    if wanted > 0.0 {
        let r = w.try_move(None, nx, ny);
        let p = &mut w.player;
        let got = ((p.x - ox).powi(2) + (p.y - oy).powi(2)).sqrt();
        if !r.moved { p.mx = 0.0; p.my = 0.0; }
        else if got < wanted * 0.5 { p.mx = p.x - ox; p.my = p.y - oy; }
        for (li, side) in r.crossed { crate::specials::cross_line(w, li, side, None); }
        for j in r.pickups { touch(w, j); }
    }
    let p = &mut w.player;
    if p.z <= p.floorz {
        p.mx *= FRICTION; p.my *= FRICTION;
        if p.mx.abs() < 0.0625 && p.my.abs() < 0.0625 { p.mx = 0.0; p.my = 0.0; }
    }
    // Height.
    p.z += p.mz;
    if p.z <= p.floorz {
        if p.mz < -8.0 { p.deltaviewheight = p.mz / 8.0; w.sound("OOF", None); }
        let p = &mut w.player;
        p.z = p.floorz;
        p.mz = 0.0;
    } else {
        p.mz = if p.mz == 0.0 { -GRAVITY * 2.0 } else { p.mz - GRAVITY };
    }
    let p = &mut w.player;
    if p.z + PLAYER_HEIGHT > p.ceilingz { p.z = (p.ceilingz - PLAYER_HEIGHT).max(p.floorz); if p.mz > 0.0 { p.mz = 0.0; } }
}

/// Doom's view bob and the dip after a fall.
fn calc_height(w: &mut World) {
    let tic = w.tic;
    let p = &mut w.player;
    p.bob = ((p.mx * p.mx + p.my * p.my) / 4.0).min(16.0);
    if p.dead {
        p.viewz = p.z + p.viewheight;
        return;
    }
    let angle = tic as f32 * std::f32::consts::TAU / 20.0;
    let bob = p.bob / 2.0 * angle.sin();
    p.viewheight += p.deltaviewheight;
    if p.viewheight > VIEWHEIGHT { p.viewheight = VIEWHEIGHT; p.deltaviewheight = 0.0; }
    if p.viewheight < VIEWHEIGHT / 2.0 { p.viewheight = VIEWHEIGHT / 2.0; if p.deltaviewheight <= 0.0 { p.deltaviewheight = 1.0; } }
    if p.deltaviewheight != 0.0 { p.deltaviewheight += 0.25; if p.deltaviewheight == 0.0 { p.deltaviewheight = 1.0; } }
    p.viewz = (p.z + p.viewheight + bob).min(p.ceilingz - 4.0);
}

fn death_think(w: &mut World) {
    let p = &mut w.player;
    if p.viewheight > 6.0 { p.viewheight -= 1.0; }
    p.deltaviewheight = 0.0;
    if let Some((ax, ay)) = p.attacker {
        let d = angle_diff(p.angle, (ay - p.y).atan2(ax - p.x));
        p.angle += d.clamp(-5f32.to_radians(), 5f32.to_radians());
    }
    if p.damage_count > 0 { p.damage_count -= 1; }
    p.mx *= FRICTION; p.my *= FRICTION;
}

pub fn die(w: &mut World) {
    let p = &mut w.player;
    p.dead = true;
    p.pending = None;
    p.wstate = WState::Lower;
    p.powers[PW_INVIS] = 0;
    w.sound("PLDETH", None);
}

fn weapon_keys(w: &mut World, input: &Input) {
    let p = &mut w.player;
    let mut pick = |slot: u8| {
        let want = match slot {
            1 => if p.weapons[W_CHAINSAW] && p.weapon != W_CHAINSAW { W_CHAINSAW } else { W_FIST },
            2 => W_PISTOL,
            3 => if p.weapons[W_SSG] && p.weapon != W_SSG { W_SSG } else { W_SHOTGUN },
            4 => W_CHAINGUN, 5 => W_ROCKET, 6 => W_PLASMA, 7 => W_BFG,
            _ => return,
        };
        if p.weapons[want] && want != p.weapon { p.pending = Some(want); }
    };
    for c in '1'..='7' { if input.pressed(Key::Char(c)) { pick(c as u8 - b'0'); } }
}

/// Enough ammo for the weapon in hand? If not, pick another.
fn check_ammo(w: &mut World) -> bool {
    let p = &mut w.player;
    let wp = &WEAPONS[p.weapon];
    let ok = match wp.ammo { None => true, Some(a) => p.ammo[a as usize] >= wp.per_shot };
    if ok { return true; }
    let order = [W_PLASMA, W_SSG, W_CHAINGUN, W_SHOTGUN, W_PISTOL, W_CHAINSAW, W_ROCKET, W_BFG, W_FIST];
    for &c in &order {
        if !p.weapons[c] { continue; }
        let wc = &WEAPONS[c];
        let has = match wc.ammo { None => true, Some(a) => p.ammo[a as usize] >= wc.per_shot };
        if has { p.pending = Some(c); break; }
    }
    p.wstate = WState::Lower;
    false
}

/// The weapon's frames, raising, lowering, firing.
fn psprites(w: &mut World) {
    let tic = w.tic;
    match w.player.wstate {
        WState::Lower => {
            let p = &mut w.player;
            p.psp_y += 6.0;
            if p.psp_y >= WEAPONBOTTOM {
                p.psp_y = WEAPONBOTTOM;
                if p.dead { return; }
                if let Some(n) = p.pending.take() { p.weapon = n; }
                p.wstate = WState::Raise;
                p.psp_seq = WEAPONS[p.weapon].ready; p.psp_idx = 0; p.psp_tics = -1;
                if p.weapon == W_CHAINSAW { w.sound("SAWUP", None); }
            }
            return;
        }
        WState::Raise => {
            let p = &mut w.player;
            p.psp_y -= 6.0;
            if p.psp_y <= WEAPONTOP {
                p.psp_y = WEAPONTOP;
                p.wstate = WState::Ready;
                p.psp_seq = WEAPONS[p.weapon].ready; p.psp_idx = 0; p.psp_tics = p.psp_seq[0].1 as i32;
            }
            return;
        }
        _ => {}
    }
    // Flash frames.
    if let Some((seq, idx, tics)) = w.player.flash {
        if tics > 0 {
            let t = tics - 1;
            if t == 0 {
                if idx + 1 < seq.len() { w.player.flash = Some((seq, idx + 1, seq[idx + 1].1 as i32)); weapon_action(w, seq[idx + 1].2); }
                else { w.player.flash = None; w.player.extra_light = 0; }
            } else { w.player.flash = Some((seq, idx, t)); }
        }
    }
    // The weapon's own frames.
    let p = &mut w.player;
    if p.psp_tics > 0 { p.psp_tics -= 1; }
    if p.psp_tics == 0 {
        if p.psp_idx + 1 < p.psp_seq.len() {
            p.psp_idx += 1;
            p.psp_tics = p.psp_seq[p.psp_idx].1 as i32;
            let act = p.psp_seq[p.psp_idx].2;
            weapon_action(w, act);
        } else {
            let ready = WEAPONS[p.weapon].ready;
            p.wstate = WState::Ready;
            p.psp_seq = ready; p.psp_idx = 0; p.psp_tics = ready[0].1 as i32;
            weapon_action(w, ready[0].2);
        }
    } else if p.wstate == WState::Ready && p.psp_tics < 0 {
        weapon_action(w, Act::Ready);
    }
    // Zero-length frames run on the spot.
    let mut guard = 0;
    while w.player.psp_tics == 0 && w.player.wstate == WState::Attack && guard < 4 {
        let p = &mut w.player;
        if p.psp_idx + 1 < p.psp_seq.len() {
            p.psp_idx += 1;
            p.psp_tics = p.psp_seq[p.psp_idx].1 as i32;
            let act = p.psp_seq[p.psp_idx].2;
            weapon_action(w, act);
        } else {
            let ready = WEAPONS[p.weapon].ready;
            p.wstate = WState::Ready;
            p.psp_seq = ready; p.psp_idx = 0; p.psp_tics = ready[0].1 as i32;
        }
        guard += 1;
    }
    let _ = tic;
}

fn start_attack(w: &mut World) {
    let p = &mut w.player;
    p.wstate = WState::Attack;
    p.psp_seq = WEAPONS[p.weapon].attack;
    p.psp_idx = 0;
    p.psp_tics = p.psp_seq[0].1 as i32;
    if p.weapon == W_CHAINSAW { w.sound("SAWFUL", None); }
    let act = w.player.psp_seq[0].2;
    weapon_action(w, act);
}

fn flash(w: &mut World, idx: usize) {
    let p = &mut w.player;
    let seq = WEAPONS[p.weapon].flash;
    if seq.is_empty() { return; }
    let idx = idx.min(seq.len() - 1);
    p.flash = Some((seq, idx, seq[idx].1 as i32));
    let act = seq[idx].2;
    weapon_action(w, act);
}

fn use_ammo(w: &mut World) {
    let p = &mut w.player;
    if let Some(a) = WEAPONS[p.weapon].ammo { p.ammo[a as usize] = (p.ammo[a as usize] - WEAPONS[p.weapon].per_shot).max(0); }
}

/// Bullets from the player: aim, then trace with spread.
fn gunshot(w: &mut World, accurate: bool, count: u32, wide: bool) {
    let (x, y, z, angle) = (w.player.x, w.player.y, w.player.z + 32.0, w.player.angle);
    let aimed = w.aim(x, y, z, angle, 16.0 * 64.0, None, false);
    let slope0 = aimed.map(|a| a.0).unwrap_or(0.0);
    for _ in 0..count {
        let dmg = 5 * ((w.rng.next() % 3) as i32 + 1);
        let a = if accurate { angle } else if wide { angle + w.rng.spread() / 255.0 * PI / 16.0 } else { angle + w.rng.spread() / 255.0 * PI / 32.0 };
        let slope = if wide { slope0 + w.rng.spread() / 255.0 * 0.125 } else { slope0 };
        w.bullet(x, y, z, a, slope, MISSILERANGE, dmg, None);
    }
    crate::specials::noise_alert(w);
}

fn player_missile(w: &mut World, proj: &'static info::Proj) {
    let (x, y, z, angle) = (w.player.x, w.player.y, w.player.z + 32.0, w.player.angle);
    let slope = w.aim(x, y, z, angle, 16.0 * 64.0, None, false).map(|a| a.0).unwrap_or(0.0);
    w.launch(None, proj, x, y, z, angle, slope * proj.speed);
    crate::specials::noise_alert(w);
}

fn weapon_action(w: &mut World, act: Act) {
    match act {
        Act::Ready => {
            let p = &mut w.player;
            if p.weapon == W_CHAINSAW && p.psp_idx == 0 && p.psp_tics == WEAPONS[W_CHAINSAW].ready[0].1 as i32 { w.sound("SAWIDL", None); }
            let p = &mut w.player;
            if p.pending.is_some() || p.dead { p.wstate = WState::Lower; return; }
            if p.attack_down {
                if check_ammo(w) { start_attack(w); }
                return;
            }
            let p = &mut w.player;
            let a = w.tic as f32 * std::f32::consts::TAU / 64.0;
            p.psp_x = 1.0 + p.bob * a.cos();
            p.psp_y = WEAPONTOP + p.bob * (a % PI).sin().abs();
        }
        Act::ReFire => {
            let p = &mut w.player;
            if p.attack_down && p.pending.is_none() && !p.dead {
                p.refire += 1;
                if check_ammo(w) { start_attack(w); }
            } else {
                p.refire = 0;
                check_ammo(w);
            }
        }
        Act::Punch => {
            let mut dmg = ((w.rng.next() % 10) as i32 + 1) * 2;
            if w.player.powers[PW_BERSERK] > 0 { dmg *= 10; }
            let (x, y, z) = (w.player.x, w.player.y, w.player.z + 32.0);
            let angle = w.player.angle + w.rng.spread() / 255.0 * PI / 32.0;
            if let Some((slope, j)) = w.aim(x, y, z, angle, MELEERANGE, None, false) {
                w.bullet(x, y, z, angle, slope, MELEERANGE, dmg, None);
                w.sound("PUNCH", None);
                let (tx, ty) = (w.mobjs[j].x, w.mobjs[j].y);
                w.player.angle = (ty - y).atan2(tx - x);
            }
        }
        Act::Saw => {
            let dmg = ((w.rng.next() % 10) as i32 + 1) * 2;
            let (x, y, z) = (w.player.x, w.player.y, w.player.z + 32.0);
            let angle = w.player.angle + w.rng.spread() / 255.0 * PI / 32.0;
            match w.aim(x, y, z, angle, MELEERANGE + 1.0, None, false) {
                Some((slope, j)) => {
                    w.bullet(x, y, z, angle, slope, MELEERANGE + 1.0, dmg, None);
                    w.sound("SAWHIT", None);
                    let (tx, ty) = (w.mobjs[j].x, w.mobjs[j].y);
                    w.player.angle = (ty - y).atan2(tx - x);
                }
                None => w.sound("SAWFUL", None),
            }
        }
        Act::FirePistol => { w.sound("PISTOL", None); flash(w, 0); use_ammo(w); let acc = w.player.refire == 0; gunshot(w, acc, 1, false); }
        Act::FireShotgun => { w.sound("SHOTGN", None); flash(w, 0); use_ammo(w); gunshot(w, false, 7, false); }
        Act::FireShotgun2 => { w.sound("DSHTGN", None); flash(w, 0); use_ammo(w); gunshot(w, false, 20, true); }
        Act::FireCGun => {
            if w.player.ammo[AmmoKind::Bullets as usize] <= 0 { return; }
            w.sound("PISTOL", None);
            let idx = w.player.psp_idx;
            flash(w, idx);
            use_ammo(w);
            let acc = w.player.refire == 0;
            gunshot(w, acc, 1, false);
        }
        Act::FireMissile => { use_ammo(w); player_missile(w, &info::ROCKET); }
        Act::FirePlasma => { let n = (w.rng.next() & 1) as usize; flash(w, n); use_ammo(w); player_missile(w, &info::PLASMA); }
        Act::BFGSound => w.sound("BFG", None),
        Act::FireBFG => { use_ammo(w); player_missile(w, &info::BFGBALL); }
        Act::GunFlash => flash(w, 0),
        Act::OpenShotgun2 => w.sound("DBOPN", None),
        Act::LoadShotgun2 => w.sound("DBLOAD", None),
        Act::CloseShotgun2 => { w.sound("DBCLS", None); weapon_action(w, Act::ReFire); }
        Act::CheckReload => { check_ammo(w); }
        Act::Light0 => w.player.extra_light = 0,
        Act::Light1 => w.player.extra_light = 1,
        Act::Light2 => w.player.extra_light = 2,
        _ => {}
    }
}

/// Picking something up.
pub fn touch(w: &mut World, j: usize) {
    let Some(p) = w.mobjs[j].pickup else { return };
    if w.mobjs[j].removed { return; }
    let dropped = w.mobjs[j].flags & F_DROPPED != 0;
    let (mz, z) = (w.mobjs[j].z, w.player.z);
    // Out of reach above or below.
    if mz > z + PLAYER_HEIGHT || mz + 8.0 < z { return; }
    let pl = &mut w.player;
    let mut msg = p.msg;
    let taken = match p.effect {
        Effect::Health(n, max) => {
            if pl.health >= max { false } else {
                if n == 25 && pl.health < 25 { msg = "Picked up a medikit that you REALLY need!"; }
                pl.health = (pl.health + n).min(max); true
            }
        }
        Effect::Armor(n, t) => { if pl.armor >= n { false } else { pl.armor = n; pl.armor_type = t; true } }
        Effect::ArmorBonus => { pl.armor = (pl.armor + 1).min(200); if pl.armor_type == 0 { pl.armor_type = 1; } true }
        Effect::Key(k) => { pl.keys[k] = true; true }
        Effect::Ammo(kind, n) => give_ammo(pl, kind, if dropped { n / 2 } else { n }, w.skill),
        Effect::Backpack => {
            if !pl.backpack { pl.backpack = true; for m in pl.max_ammo.iter_mut() { *m *= 2; } }
            for (k, n) in [(AmmoKind::Bullets, 10), (AmmoKind::Shells, 4), (AmmoKind::Cells, 20), (AmmoKind::Rockets, 1)] { give_ammo(pl, k, n, w.skill); }
            true
        }
        Effect::Weapon(wi) => {
            let (kind, n) = match wi {
                W_SHOTGUN | W_SSG => (Some(AmmoKind::Shells), 8), W_CHAINGUN => (Some(AmmoKind::Bullets), 20),
                W_ROCKET => (Some(AmmoKind::Rockets), 2), W_PLASMA | W_BFG => (Some(AmmoKind::Cells), 40), _ => (None, 0),
            };
            let gave_ammo = match kind { Some(k) => give_ammo(pl, k, if dropped { n / 2 } else { n }, w.skill), None => false };
            let gave_weapon = !pl.weapons[wi];
            if gave_weapon { pl.weapons[wi] = true; pl.pending = Some(wi); pl.evil_tics = 70; }
            gave_weapon || gave_ammo
        }
        Effect::Berserk => { pl.powers[PW_BERSERK] = 1; if pl.health < 100 { pl.health = 100; } if pl.weapon != W_FIST { pl.pending = Some(W_FIST); } true }
        Effect::Invuln => { pl.powers[PW_INVULN] = 30 * TICRATE; true }
        Effect::Invis => { pl.powers[PW_INVIS] = 60 * TICRATE; true }
        Effect::Suit => { pl.powers[PW_SUIT] = 60 * TICRATE; true }
        Effect::Map => { pl.powers[PW_MAP] = 1; true }
        Effect::Visor => { pl.powers[PW_VISOR] = 120 * TICRATE; true }
        Effect::Mega => { pl.health = 200; pl.armor = 200; pl.armor_type = 2; true }
    };
    if !taken { return; }
    if p.count { pl.items += 1; }
    pl.bonus_count += 6;
    w.mobjs[j].removed = true;
    w.message(msg);
    w.sound(p.sound, None);
}

fn give_ammo(pl: &mut Player, kind: AmmoKind, n: i32, skill: u8) -> bool {
    let k = kind as usize;
    if pl.ammo[k] >= pl.max_ammo[k] { return false; }
    let n = if skill == 0 || skill == 4 { n * 2 } else { n };
    let had = pl.ammo[k];
    pl.ammo[k] = (pl.ammo[k] + n).min(pl.max_ammo[k]);
    // Out of ammo a moment ago: bring up something that shoots.
    if had == 0 && (pl.weapon == W_FIST || pl.weapon == W_PISTOL && kind != AmmoKind::Bullets) {
        let want = match kind {
            AmmoKind::Bullets => if pl.weapons[W_CHAINGUN] { W_CHAINGUN } else { W_PISTOL },
            AmmoKind::Shells => if pl.weapons[W_SHOTGUN] { W_SHOTGUN } else if pl.weapons[W_SSG] { W_SSG } else { return true },
            AmmoKind::Cells => if pl.weapons[W_PLASMA] { W_PLASMA } else { return true },
            AmmoKind::Rockets => if pl.weapons[W_ROCKET] { W_ROCKET } else { return true },
        };
        if pl.weapons[want] { pl.pending = Some(want); }
    }
    true
}

/// Damage floors, secret sectors, the level that ends when you are nearly dead.
fn sector_special(w: &mut World) {
    let p = &w.player;
    if p.z > p.floorz { return; }
    let special = w.level.sectors[p.sector].special;
    if special == 0 { return; }
    let suit = w.player.powers[PW_SUIT] > 0;
    let every32 = w.tic & 31 == 0;
    match special {
        9 => { w.player.secrets += 1; w.level.sectors[w.player.sector].special = 0; }
        5 => if !suit && every32 { w.damage_player(10, None); },
        7 => if !suit && every32 { w.damage_player(5, None); },
        16 | 4 => if every32 && (!suit || w.rng.next() < 5) { w.damage_player(20, None); },
        11 => {
            w.player.god = false;
            if every32 { w.damage_player(20, None); }
            if w.player.health <= 10 { w.exit = Some(false); }
        }
        _ => {}
    }
}

fn face_tick(w: &mut World) {
    let tic = w.tic;
    let p = &mut w.player;
    if p.ouch_tics > 0 { p.ouch_tics -= 1; }
    if p.evil_tics > 0 { p.evil_tics -= 1; }
    if p.hurt_tics > 0 { p.hurt_tics -= 1; }
    if p.attack_down && p.wstate == WState::Attack { p.rampage += 1; } else { p.rampage = 0; }
    if p.look_tics > 0 { p.look_tics -= 1; }
    else {
        p.look_tics = 17;
        p.look_idx = if tic % 3 == 0 { (w.rng.next() % 3) as usize } else { 0 };
    }
}
