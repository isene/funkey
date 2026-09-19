//! What monsters do, frame by frame: look, chase, attack, die. Each
//! action is Doom's, simplified where the terminal cannot tell.

use crate::info::{self, Act, Attack};
use crate::world::*;
use std::f32::consts::PI;

pub fn action(w: &mut World, i: usize, act: Act) {
    match act {
        Act::Look => look(w, i),
        Act::Chase => chase(w, i),
        Act::Face => face(w, i),
        Act::Attack => attack(w, i),
        Act::Melee => melee(w, i),
        Act::Combo => if in_melee_range(w, i) { melee(w, i) } else { attack(w, i) },
        Act::Pain => { let (s, at) = { let m = &w.mobjs[i]; (m.mon.map(|m| m.s_pain).unwrap_or(""), Some((m.x, m.y))) }; w.sound(s, at); }
        Act::Scream => {
            let (list, at) = { let m = &w.mobjs[i]; (m.mon.map(|m| m.s_death).unwrap_or(&[]), if matches!(m.thing_kind, 7 | 16) { None } else { Some((m.x, m.y)) }) };
            w.sound_of(list, at);
        }
        Act::XScream => { let at = at(w, i); w.sound("SLOP", at); }
        Act::Fall => { w.mobjs[i].flags &= !F_SOLID; }
        Act::Explode => { let (x, y, z, owner) = { let m = &w.mobjs[i]; (m.x, m.y, m.z + m.height * 0.5, m.owner) }; w.radius_damage(x, y, z, 128.0, owner); }
        Act::Refire => refire(w, i),
        Act::SkullAttack => skull_attack(w, i),
        Act::PainAttack => { let a = w.mobjs[i].angle; pain_shoot(w, i, a); }
        Act::PainDie => { let a = w.mobjs[i].angle; for k in 1..4 { pain_shoot(w, i, a + k as f32 * PI / 2.0); } }
        Act::Tracer => tracer(w, i),
        Act::Whoosh => { face(w, i); let at = at(w, i); w.sound("SKESWG", at); }
        Act::FatRaise => { face(w, i); let at = at(w, i); w.sound("MANATK", at); }
        Act::Fat1 => fat_pair(w, i, 0.0, FATSPREAD),
        Act::Fat2 => fat_pair(w, i, 0.0, -FATSPREAD),
        Act::Fat3 => fat_pair(w, i, -FATSPREAD / 2.0, FATSPREAD / 2.0),
        Act::VileStart => { let at = at(w, i); w.sound("VILATK", at); }
        Act::VileTarget => vile_target(w, i),
        Act::VileAttack => vile_attack(w, i),
        Act::Hoof => { let at = at(w, i); w.sound("HOOF", at); chase(w, i); }
        Act::Metal => { let at = at(w, i); w.sound("METAL", at); chase(w, i); }
        Act::BabyMetal => { let at = at(w, i); w.sound("BSPWLK", at); chase(w, i); }
        Act::BossDeath => crate::specials::boss_death(w, i),
        Act::KeenDie => crate::specials::keen_die(w),
        Act::Remove => { w.mobjs[i].removed = true; }
        Act::BFGSpray => bfg_spray(w),
        _ => {}
    }
}

const FATSPREAD: f32 = PI / 16.0;

fn at(w: &World, i: usize) -> SoundAt { let m = &w.mobjs[i]; Some((m.x, m.y)) }

/// Idle: wake up on seeing the player, or on noise heard in the sector.
fn look(w: &mut World, i: usize) {
    let (sector, flags, kind, x, y, angle) = { let m = &w.mobjs[i]; (m.sector, m.flags, m.thing_kind, m.x, m.y, m.angle) };
    let Some(mon) = w.mobjs[i].mon else { return };
    if mon.see.is_empty() { return; }
    w.mobjs[i].threshold = 0;
    if w.player.dead { return; }
    let heard = w.sound_level[sector] > 0;
    let wake = if heard && flags & F_AMBUSH == 0 {
        true
    } else if heard {
        w.sees_player(i)
    } else {
        let dist = w.player_dist(x, y);
        let to = (w.player.y - y).atan2(w.player.x - x);
        let in_front = angle_diff(angle, to).abs() < PI / 2.0 || dist < MELEERANGE;
        in_front && w.sees_player(i)
    };
    if !wake { return; }
    let m = &mut w.mobjs[i];
    m.awake = true;
    m.threshold = 0;
    let at = if matches!(kind, 7 | 16) { None } else { Some((x, y)) };
    w.sound_of(mon.s_sight, at);
    w.set_state(i, St::See, mon.see);
}

/// Walk toward the player, and attack when the odds say so.
fn chase(w: &mut World, i: usize) {
    let Some(mon) = w.mobjs[i].mon else { return };
    {
        let m = &mut w.mobjs[i];
        if m.reaction > 0 { m.reaction -= 1; }
        if m.threshold > 0 { m.threshold -= 1; }
        if m.move_dir < 8 {
            let target = m.move_dir as f32 * PI / 4.0;
            let d = angle_diff(m.angle, target);
            m.angle += d.clamp(-PI / 4.0, PI / 4.0);
        }
    }
    if w.player.dead || !w.mobjs[i].awake {
        w.mobjs[i].awake = false;
        w.set_state(i, St::Spawn, mon.spawn);
        return;
    }
    if w.mobjs[i].flags & F_JUSTATTACKED != 0 {
        w.mobjs[i].flags &= !F_JUSTATTACKED;
        new_chase_dir(w, i);
        return;
    }
    if !mon.melee.is_empty() && in_melee_range(w, i) {
        w.set_state(i, St::Melee, mon.melee);
        return;
    }
    if !mon.missile.is_empty() && w.mobjs[i].move_count == 0 && check_missile_range(w, i) {
        w.set_state(i, St::Missile, mon.missile);
        w.mobjs[i].flags |= F_JUSTATTACKED;
        return;
    }
    w.mobjs[i].move_count -= 1;
    if w.mobjs[i].move_count < 0 || !do_move(w, i) { new_chase_dir(w, i); }
    if w.rng.next() < 3 { let at = at(w, i); w.sound(mon.s_active, at); }
}

pub fn face(w: &mut World, i: usize) {
    let (px, py) = (w.player.x, w.player.y);
    let fuzzy = w.player.powers[crate::player::PW_INVIS] > 0;
    let spread = if fuzzy { w.rng.spread() / 255.0 * PI / 8.0 } else { 0.0 };
    let m = &mut w.mobjs[i];
    m.angle = (py - m.y).atan2(px - m.x) + spread;
    // The arch-vile's fire follows its target.
    if let Some(f) = m.fire_for {
        if w.sees_player(i) {
            let (pz, px, py) = (w.player.z, w.player.x, w.player.y);
            let fire = &mut w.mobjs[f];
            if !fire.removed { fire.x = px; fire.y = py; fire.z = pz; }
        }
    }
}

pub fn in_melee_range(w: &World, i: usize) -> bool {
    let m = &w.mobjs[i];
    !w.player.dead && w.player_dist(m.x, m.y) < MELEERANGE - 20.0 + PLAYER_RADIUS
}

fn check_missile_range(w: &mut World, i: usize) -> bool {
    if !w.sees_player(i) { return false; }
    let (kind, x, y, has_melee) = { let m = &w.mobjs[i]; (m.thing_kind, m.x, m.y, m.mon.map(|m| !m.melee.is_empty()).unwrap_or(false)) };
    {
        let m = &mut w.mobjs[i];
        if m.flags & F_JUSTHIT != 0 { m.flags &= !F_JUSTHIT; return true; }
        if m.reaction > 0 { return false; }
    }
    let mut dist = w.player_dist(x, y) - 64.0;
    if !has_melee { dist -= 128.0; }
    match kind {
        64 => if dist > 14.0 * 64.0 { return false; },
        66 => { if dist < 196.0 { return false; } dist /= 2.0; }
        16 | 7 | 3006 => dist /= 2.0,
        _ => {}
    }
    if dist > 200.0 { dist = 200.0; }
    if kind == 16 && dist > 160.0 { dist = 160.0; }
    (w.rng.next() as f32) >= dist
}

/// One step in the move direction. A door in the way gets opened.
fn do_move(w: &mut World, i: usize) -> bool {
    let (dir, x, y, speed, flags) = { let m = &w.mobjs[i]; (m.move_dir, m.x, m.y, m.mon.map(|m| m.speed).unwrap_or(0.0), m.flags) };
    if dir >= 8 || speed <= 0.0 { return false; }
    let ang = dir as f32 * PI / 4.0;
    let (nx, ny) = (x + ang.cos() * speed, y + ang.sin() * speed);
    let r = w.try_move(Some(i), nx, ny);
    let got = { let m = &w.mobjs[i]; ((m.x - x).powi(2) + (m.y - y).powi(2)).sqrt() };
    if !r.moved || got < speed * 0.5 {
        if let Some(li) = r.hit_line {
            if crate::specials::monster_use_door(w, li) { w.mobjs[i].move_dir = 8; return true; }
        }
        return false;
    }
    if flags & F_FLOAT == 0 { let m = &mut w.mobjs[i]; m.z = m.floorz; }
    for (li, side) in r.crossed { crate::specials::cross_line(w, li, side, Some(i)); }
    true
}

fn try_walk(w: &mut World, i: usize, dir: u8) -> bool {
    w.mobjs[i].move_dir = dir;
    if !do_move(w, i) { return false; }
    w.mobjs[i].move_count = (w.rng.next() & 15) as i32;
    true
}

/// Doom's choice of a new direction: toward the player, diagonals first,
/// then sideways, never straight back unless nothing else works.
fn new_chase_dir(w: &mut World, i: usize) {
    let (dx, dy, olddir) = { let m = &w.mobjs[i]; (w.player.x - m.x, w.player.y - m.y, m.move_dir) };
    let turnaround = if olddir < 8 { (olddir + 4) % 8 } else { 8 };
    let mut d1 = if dx > 10.0 { 0 } else if dx < -10.0 { 4 } else { 8 };
    let mut d2 = if dy < -10.0 { 6 } else if dy > 10.0 { 2 } else { 8 };
    if d1 != 8 && d2 != 8 {
        let diag = match (dx > 0.0, dy > 0.0) { (true, true) => 1, (false, true) => 3, (false, false) => 5, (true, false) => 7 };
        if diag != turnaround && try_walk(w, i, diag) { return; }
    }
    if w.rng.next() > 200 || dy.abs() > dx.abs() { std::mem::swap(&mut d1, &mut d2); }
    if d1 == turnaround { d1 = 8; }
    if d2 == turnaround { d2 = 8; }
    if d1 != 8 && try_walk(w, i, d1) { return; }
    if d2 != 8 && try_walk(w, i, d2) { return; }
    if olddir != 8 && try_walk(w, i, olddir) { return; }
    if w.rng.next() & 1 != 0 {
        for d in 0..8u8 { if d != turnaround && try_walk(w, i, d) { return; } }
    } else {
        for d in (0..8u8).rev() { if d != turnaround && try_walk(w, i, d) { return; } }
    }
    if turnaround != 8 && try_walk(w, i, turnaround) { return; }
    w.mobjs[i].move_dir = 8;
}

/// The ranged attack from the monster's table entry.
fn attack(w: &mut World, i: usize) {
    face(w, i);
    let Some(mon) = w.mobjs[i].mon else { return };
    let (x, y, z, angle) = { let m = &w.mobjs[i]; (m.x, m.y, m.z + 32.0, m.angle) };
    match mon.attack {
        Attack::Hitscan(n) => {
            w.sound(mon.s_attack, Some((x, y)));
            for _ in 0..n {
                let a = angle + w.rng.spread() / 255.0 * PI / 8.0;
                let dmg = ((w.rng.next() % 5) as i32 + 1) * 3;
                w.monster_bullet(i, a, dmg);
            }
        }
        Attack::Missile(p) => {
            let (px, py, pz) = (w.player.x, w.player.y, w.player_mid());
            w.spawn_missile(Some(i), p, x, y, z, angle, px, py, pz);
        }
        Attack::None => {}
    }
}

fn melee(w: &mut World, i: usize) {
    face(w, i);
    if !in_melee_range(w, i) { return; }
    let Some(mon) = w.mobjs[i].mon else { return };
    let (x, y) = { let m = &w.mobjs[i]; (m.x, m.y) };
    w.sound(mon.s_attack, Some((x, y)));
    let (dice, mult) = mon.melee_dmg;
    if dice == 0 { return; }
    let dmg = ((w.rng.next() as i32) % dice + 1) * mult;
    w.damage_player(dmg, Some((x, y)));
}

/// Keep firing while the player is in sight: back to the second frame.
fn refire(w: &mut World, i: usize) {
    face(w, i);
    if w.rng.next() < 40 { w.mobjs[i].idx = 0; return; }
    if w.player.dead || !w.sees_player(i) {
        if let Some(mon) = w.mobjs[i].mon { w.set_state(i, St::See, mon.see); }
        return;
    }
    w.mobjs[i].idx = 0;
}

fn skull_attack(w: &mut World, i: usize) {
    if w.player.dead { return; }
    face(w, i);
    let (px, py, pz) = (w.player.x, w.player.y, w.player_mid());
    let m = &mut w.mobjs[i];
    m.flags |= F_SKULLFLY;
    let at = Some((m.x, m.y));
    const SKULLSPEED: f32 = 20.0;
    m.mx = m.angle.cos() * SKULLSPEED;
    m.my = m.angle.sin() * SKULLSPEED;
    let dist = m.dist_to(px, py);
    let tics = (dist / SKULLSPEED).max(1.0);
    m.mz = (pz - (m.z + m.height * 0.5)) / tics;
    w.sound("SKLATK", at);
}

/// A pain elemental spits out a lost soul, unless twenty are already about.
fn pain_shoot(w: &mut World, i: usize, angle: f32) {
    if w.mobjs.iter().filter(|m| !m.removed && m.thing_kind == 3006 && m.health > 0).count() >= 20 { return; }
    let (x, y, z, r) = { let m = &w.mobjs[i]; (m.x, m.y, m.z + 8.0, m.radius) };
    let prestep = 4.0 + 3.0 * (r + 16.0) / 2.0;
    let (sx, sy) = (x + angle.cos() * prestep, y + angle.sin() * prestep);
    let Some(j) = w.spawn_thing(3006, x, y, angle, false) else { return };
    w.mobjs[j].z = z;
    let r = w.try_move(Some(j), sx, sy);
    if !r.moved || w.mobjs[j].dist_to(sx, sy) > 4.0 { w.mobjs[j].removed = true; return; }
    w.mobjs[j].awake = true;
    w.total_kills += 0;
    skull_attack(w, j);
}

/// The revenant's rocket turns toward the player, a little at a time.
fn tracer(w: &mut World, i: usize) {
    if w.tic & 3 != 0 { return; }
    let (x, y, z, mx, my) = { let m = &w.mobjs[i]; (m.x, m.y, m.z, m.mx, m.my) };
    w.spawn_effect(*b"PUFF", info::PUFF, x - mx, y - my, z);
    if w.player.dead { return; }
    let (px, py, pz) = (w.player.x, w.player.y, w.player.z);
    let speed = info::TRACER.speed;
    let m = &mut w.mobjs[i];
    let exact = (py - m.y).atan2(px - m.x);
    let d = angle_diff(m.angle, exact);
    const TRACEANGLE: f32 = 0.2945;
    m.angle += d.clamp(-TRACEANGLE, TRACEANGLE);
    m.mx = m.angle.cos() * speed;
    m.my = m.angle.sin() * speed;
    let dist = m.dist_to(px, py);
    let tics = (dist / speed).max(1.0);
    let slope = (pz + 40.0 - m.z) / tics;
    if slope < m.mz { m.mz -= 0.125; } else { m.mz += 0.125; }
}

/// Two mancubus fireballs, at these angles off the line to the player.
fn fat_pair(w: &mut World, i: usize, a1: f32, a2: f32) {
    face(w, i);
    let (x, y, z, angle) = { let m = &w.mobjs[i]; (m.x, m.y, m.z + 32.0, m.angle) };
    let (px, py, pz) = (w.player.x, w.player.y, w.player_mid());
    let dist = ((px - x).powi(2) + (py - y).powi(2)).sqrt().max(1.0);
    let mz = (pz - z) / (dist / info::FATSHOT.speed).max(1.0);
    w.launch(Some(i), &info::FATSHOT, x, y, z, angle + a1, mz);
    w.launch(Some(i), &info::FATSHOT, x, y, z, angle + a2, mz);
}

fn vile_target(w: &mut World, i: usize) {
    face(w, i);
    if w.player.dead { return; }
    let (px, py, pz) = (w.player.x, w.player.y, w.player.z);
    let f = w.spawn_effect(*b"FIRE", info::VILE_FIRE, px, py, pz);
    w.mobjs[i].fire_for = Some(f);
}

fn vile_attack(w: &mut World, i: usize) {
    face(w, i);
    if w.player.dead || !w.sees_player(i) { return; }
    let (x, y) = { let m = &w.mobjs[i]; (m.x, m.y) };
    w.sound("BAREXP", Some((x, y)));
    w.damage_player(20, Some((x, y)));
    w.player.mz = 10.0;
    let (fx, fy, fz) = match w.mobjs[i].fire_for {
        Some(f) if !w.mobjs[f].removed => { let m = &w.mobjs[f]; (m.x, m.y, m.z) }
        _ => (w.player.x, w.player.y, w.player.z),
    };
    w.radius_damage(fx, fy, fz, 70.0, Some(i));
}

/// The BFG's forty rays, from the player toward wherever the ball burst.
fn bfg_spray(w: &mut World) {
    let (px, py, pz, angle) = (w.player.x, w.player.y, w.player.z + 32.0, w.player.angle);
    for n in 0..40 {
        let a = angle - PI / 4.0 + PI / 2.0 * n as f32 / 40.0;
        let Some((_, j)) = w.aim(px, py, pz, a, 1024.0, None, false) else { continue };
        let (x, y, z, h) = { let m = &w.mobjs[j]; (m.x, m.y, m.z, m.height) };
        w.spawn_effect(*b"BFE2", info::BFG_SPRAY, x, y, z + h / 4.0);
        let dmg: i32 = (0..15).map(|_| (w.rng.next() % 8) as i32 + 1).sum();
        w.damage(j, dmg, None, None);
    }
}
