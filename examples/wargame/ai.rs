//! The computer's side. It plans with the rules' own tools: where each
//! unit can go, the odds an attack would get, where the enemy could
//! strike next, and how far the enemy has to go to reach a city.
//!
//! A side that attacks first moves units next to enemies it can beat at
//! 3:1, then drives the rest toward the cities it wants; its tanks look
//! for hexes that cut enemies off from supply. A side that defends puts
//! each unit where it lengthens the enemy's road to its own cities the
//! most, on ground where it is hard to beat.

use crate::rules::*;
use crate::scenario::Kind;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A cost no road reaches.
const FAR: i32 = 1 << 20;
/// Beyond this the enemy's road to a city no longer counts.
const CAP: i32 = 24;

/// The cost from the nearest source to every hex for a unit of `side`:
/// its enemies block, and a hex next to one costs 3 more.
fn field(w: &War, side: usize, sources: &[(usize, i32)]) -> Vec<i32> {
    let zoc = w.zoc_map(side);
    let mut d = vec![FAR; w.map.len()];
    let mut heap = BinaryHeap::new();
    for &(h, c) in sources {
        if c < d[h] {
            d[h] = c;
            heap.push(Reverse((c, h)));
        }
    }
    while let Some(Reverse((c, h))) = heap.pop() {
        if c > d[h] { continue; }
        for (_, n) in w.map.around(h) {
            if w.occ[n].map_or(false, |o| w.side_of(o) != side) { continue; }
            let Some(step) = w.step_cost(Kind::Inf, h, n) else { continue };
            let nc = c + step + if zoc[n] { 3 } else { 0 };
            if nc < d[n] {
                d[n] = nc;
                heap.push(Reverse((nc, n)));
            }
        }
    }
    d
}

/// How long the enemy's roads to our cities are, weighed by what each
/// city is worth.
fn block(w: &War, side: usize) -> f32 {
    let foe = 1 - side;
    let src: Vec<(usize, i32)> = w.on_map(foe).map(|u| (w.units[u].hex.unwrap(), 0)).collect();
    let d = field(w, foe, &src);
    let mut s = 0.0;
    for (i, c) in w.sc.cities.iter().enumerate() {
        if c.vp > 0 && w.owner[i] == side {
            let h = w.map.at(c.col, c.row).unwrap();
            s += c.vp as f32 * d[h].min(CAP) as f32;
        }
    }
    s
}

/// How much attack the enemy could bring against each hex next turn.
fn threat(w: &War, side: usize) -> Vec<i32> {
    let foe = 1 - side;
    let zoc = w.zoc_map(foe);
    let mut t = vec![0; w.map.len()];
    let mut stamp = vec![usize::MAX; w.map.len()];
    for e in w.on_map(foe).collect::<Vec<_>>() {
        let r = w.reach_as(e, &zoc, w.full_mp(e), true);
        let a = if w.units[e].hungry > 0 { w.attack(e) / 2 } else { w.attack(e) };
        for x in 0..r.len() {
            if r[x].0 == i32::MAX { continue; }
            for (_, h) in w.map.around(x) {
                if stamp[h] != e {
                    stamp[h] = e;
                    t[h] += a;
                }
            }
        }
    }
    t
}

/// Steps the unit would lose on average if `att` attacked it at `h`.
fn risk(w: &War, u: usize, h: usize, att: i32) -> f32 {
    if att == 0 { return 0.0; }
    let def = w.defence(u);
    let Some(raw) = (0..8).rev().find(|&c| att * ODDS[c].1 >= def * ODDS[c].0) else { return 0.0 };
    let c = w.map.city[h];
    let city = if c >= 0 { w.sc.cities[c as usize].size as i32 } else { 0 };
    let ground = match w.map.ground[h] {
        crate::map::Ground::Forest | crate::map::Ground::Rough | crate::map::Ground::Marsh => 1,
        crate::map::Ground::Mountain => 2,
        _ => 0,
    };
    let col = (raw as i32 - city.max(ground)).clamp(0, 7) as usize;
    let steps = w.units[u].steps as f32;
    CRT[col]
        .iter()
        .map(|r| match r {
            Res::De => steps,
            Res::D1 | Res::Ex => 1.0,
            Res::Dr => 0.25,
            _ => 0.0,
        })
        .sum::<f32>()
        / 6.0
}

/// Enemy steps out of supply.
fn cut_off(w: &War, side: usize) -> i32 {
    let foe = 1 - side;
    let ok = w.supplied(foe);
    w.on_map(foe).filter(|&e| !ok[w.units[e].hex.unwrap()]).map(|e| w.units[e].steps as i32).sum()
}

/// The cities a side wants to take, each with its points and the road
/// to it from every hex.
fn goals(w: &War, side: usize) -> Vec<(i32, Vec<i32>)> {
    w.sc.cities
        .iter()
        .enumerate()
        .filter(|(i, c)| c.vp > 0 && w.owner[*i] != side)
        .map(|(_, c)| (c.vp, field(w, side, &[(w.map.at(c.col, c.row).unwrap(), 0)])))
        .collect()
}

/// Friends of `u` next to hex `h`.
fn friends(w: &War, u: usize, h: usize) -> i32 {
    let side = w.side_of(u);
    w.map.around(h).filter(|&(_, n)| w.occ[n].map_or(false, |o| o != u && w.side_of(o) == side)).count() as i32
}

/// Stand the unit at `h` for a moment and see what `f` makes of it.
fn try_at<T>(w: &mut War, u: usize, h: usize, f: impl FnOnce(&War) -> T) -> T {
    let old = w.units[u].hex.unwrap();
    w.occ[old] = None;
    w.occ[h] = Some(u);
    w.units[u].hex = Some(h);
    let t = f(w);
    w.occ[h] = None;
    w.occ[old] = Some(u);
    w.units[u].hex = Some(old);
    t
}

/// The moves for this movement or exploitation phase, in order.
pub fn plan_moves(w: &War) -> Vec<(usize, Vec<usize>, i32)> {
    let mut sim = w.clone();
    let side = w.side;
    let exploit = w.phase == Phase::Exploit;
    let attack = w.attacking(side);
    let mut out = Vec::new();
    if !exploit { set_up_attacks(&mut sim, &mut out, attack); }
    let zoc = sim.zoc_map(side);
    let ok = sim.supplied(side);
    let threat = threat(&sim, side);
    let goals = goals(&sim, side);
    // How many units already head for each city.
    let mut load = vec![0; goals.len()];
    let foes: Vec<usize> = sim.on_map(1 - side).map(|e| sim.units[e].hex.unwrap()).collect();
    let near = |w: &War, h: usize| foes.iter().map(|&f| w.map.distance(f, h)).min().unwrap_or(99);
    let mut order: Vec<usize> = sim.on_map(side).filter(|&u| !sim.units[u].moved && sim.units[u].mp > 0).collect();
    order.sort_by_key(|&u| (near(&sim, sim.units[u].hex.unwrap()), Reverse(sim.attack(u))));
    let base_cut = if attack || exploit { cut_off(&sim, side) } else { 0 };
    for u in order {
        let r = sim.reach_with(u, &zoc);
        let here = sim.units[u].hex.unwrap();
        let mut spots = sim.targets(u, &r);
        spots.push(here);
        let mech = sim.kit(u).kind.mech();
        // Weak foot units guard the flanks and the cities behind, even in
        // an attack.
        let guard = !exploit && (!attack || (!mech && sim.attack(u) <= 3));
        // The city an attacker goes for: near, rich, and not crowded.
        let aim = (0..goals.len()).filter(|_| !guard).min_by_key(|&k| goals[k].1[here].min(200) + 3 * load[k] - 3 * goals[k].0);
        if let Some(k) = aim { load[k] += 1; }
        let mut best = (f32::MIN, here);
        for h in spots {
            let mut s = 0.0;
            if !guard {
                let w_off = if mech { 3.0 } else { 1.5 };
                if let Some(k) = aim { s -= w_off * goals[k].1[h].min(200) as f32; }
                s += 0.5 * friends(&sim, u, h) as f32;
                if mech {
                    s += 6.0 * (try_at(&mut sim, u, h, |w| cut_off(w, side)) - base_cut) as f32;
                }
            } else {
                s += try_at(&mut sim, u, h, |w| block(w, side));
                s -= 0.3 * near(&sim, h) as f32;
                let c = sim.map.city[h];
                if c >= 0 {
                    // Stand in our cities; take back one the enemy left empty.
                    let vp = sim.sc.cities[c as usize].vp as f32;
                    s += if sim.owner[c as usize] == side { 3.0 * vp } else { 4.0 * vp };
                }
                s += 1.5 * friends(&sim, u, h) as f32;
            }
            s -= risk(&sim, u, h, threat[h]) * if guard { 6.0 } else { 5.0 };
            if !ok[h] { s -= 40.0; }
            if h == here { s += 0.5; }
            if s > best.0 { best = (s, h); }
        }
        if best.1 != here {
            let path = War::path(&r, best.1);
            let cost = r[best.1].0;
            sim.go(u, &path, cost);
            out.push((u, path, cost));
        }
    }
    out
}

/// Move units next to enemies they can beat, one target at a time.
fn set_up_attacks(sim: &mut War, out: &mut Vec<(usize, Vec<usize>, i32)>, attack: bool) {
    let side = sim.side;
    let need = if attack { 4 } else { 5 };
    let zoc = sim.zoc_map(side);
    let ours: Vec<usize> = sim.on_map(side).collect();
    let reach: Vec<Vec<(i32, usize)>> = ours.iter().map(|&u| sim.reach_with(u, &zoc)).collect();
    let mut used = vec![false; sim.units.len()];
    let mut hit = vec![false; sim.units.len()];
    loop {
        let mut best: Option<(i32, usize, Vec<(usize, usize, usize)>)> = None;
        for e in sim.on_map(1 - side).collect::<Vec<_>>() {
            if hit[e] { continue; }
            let eh = sim.units[e].hex.unwrap();
            // Who could stand where: (attack, unit, slot, its place in ours).
            let mut cands: Vec<(i32, bool, usize, usize, usize)> = Vec::new();
            for (_, slot) in sim.map.around(eh) {
                if !sim.map.land(slot) { continue; }
                match sim.occ[slot] {
                    Some(o) if sim.side_of(o) == side => {
                        if !used[o] { cands.push((sim.attack(o), true, o, slot, usize::MAX)); }
                    }
                    Some(_) => {}
                    None => {
                        for (k, &u) in ours.iter().enumerate() {
                            if !used[u] && !sim.units[u].moved && reach[k][slot].0 != i32::MAX {
                                cands.push((sim.attack(u), false, u, slot, k));
                            }
                        }
                    }
                }
            }
            cands.sort_by_key(|c| (Reverse(c.1), Reverse(c.0)));
            let mut pick: Vec<(usize, usize, usize)> = Vec::new();
            let mut col = None;
            for &(_, _, u, slot, k) in &cands {
                if pick.iter().any(|p| p.0 == u || p.1 == slot) { continue; }
                pick.push((u, slot, k));
                // The odds as if every unit stood in its slot.
                let att: Vec<usize> = pick.iter().map(|p| p.0).collect();
                let olds: Vec<Option<usize>> = att.iter().map(|&a| sim.units[a].hex).collect();
                for p in &pick { sim.units[p.0].hex = Some(p.1); }
                col = sim.odds(&att, e).col;
                for (a, o) in att.iter().zip(olds) { sim.units[*a].hex = o; }
                if col.map_or(false, |c| c > need) { break; }
            }
            let Some(c) = col else { continue };
            let cv = sim.map.city[eh];
            let vp = if cv >= 0 { sim.sc.cities[cv as usize].vp } else { 0 };
            if c < need - (attack && vp > 0) as usize { continue; }
            let score = c as i32 * 10 + vp * 15 + if sim.units[e].steps == 1 { 5 } else { 0 } - pick.len() as i32 * 2;
            if best.as_ref().map_or(true, |b| score > b.0) { best = Some((score, e, pick)); }
        }
        let Some((_, e, pick)) = best else { break };
        hit[e] = true;
        for (u, slot, k) in pick {
            used[u] = true;
            if sim.units[u].hex == Some(slot) {
                // It stays for the attack; mark it so it is not sent away.
                sim.units[u].moved = true;
                continue;
            }
            let path = War::path(&reach[k], slot);
            let cost = reach[k][slot].0;
            sim.go(u, &path, cost);
            out.push((u, path, cost));
        }
    }
}

/// The best attack to make now, if any is good enough.
pub fn pick_attack(w: &War) -> Option<(Vec<usize>, usize)> {
    let need = if w.attacking(w.side) { 3 } else { 4 };
    let mut best: Option<(i32, Vec<usize>, usize)> = None;
    for e in w.on_map(1 - w.side).collect::<Vec<_>>() {
        let mut att = w.attackers_of(e);
        if att.is_empty() { continue; }
        att.sort_by_key(|&a| Reverse(w.attack(a)));
        let Some(col) = w.odds(&att, e).col else { continue };
        let h = w.units[e].hex.unwrap();
        let cv = w.map.city[h];
        let vp = if cv >= 0 { w.sc.cities[cv as usize].vp } else { 0 };
        if col < need - (w.attacking(w.side) && vp > 0) as usize { continue; }
        while att.len() > 1 && w.odds(&att[..att.len() - 1], e).col == Some(col) { att.pop(); }
        let score = col as i32 * 10 + vp * 10 + if w.units[e].steps == 1 { 4 } else { 0 };
        if best.as_ref().map_or(true, |b| score > b.0) { best = Some((score, att, e)); }
    }
    best.map(|b| (b.1, b.2))
}

/// Which attacker, if any, should take the hex the defender left.
pub fn pick_advance(w: &War, att: &[usize], hex: usize) -> Option<usize> {
    let alive: Vec<usize> = att.iter().copied().filter(|&a| w.units[a].steps > 0).collect();
    let c = w.map.city[hex];
    let city = c >= 0 && w.sc.cities[c as usize].vp > 0;
    if !city && !w.attacking(w.side) { return None; }
    alive.iter().copied().max_by_key(|&a| (!w.kit(a).kind.mech(), w.defence(a)))
}
