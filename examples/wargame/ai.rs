//! The computer's side. It plans with the rules' own tools: where each
//! unit can go, the odds an attack would get, where the enemy could
//! strike next, and how far the enemy has to go to reach a city.
//!
//! Before it moves, it lines units up next to enemies it can beat, and
//! more so next to enemies with nowhere to fall back. Then each unit,
//! the ones nearest the enemy first, picks the hex that scores best:
//!
//! - A defender stands where it lengthens the enemy's road to our cities
//!   the most, in a line beside its friends, in our threatened cities,
//!   and on ground where it is hard to beat or cut off.
//! - An attacker heads for the city it wants, and its tanks look for
//!   hexes that cut enemies off from supply; neither drives on alone.
//! - A unit with nothing to do near it marches toward the front.

use super::rules::*;
use super::scenario::Kind;
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

/// What each city is worth to a side: its points, and for a port it
/// holds half a point more for each step of its units only that port
/// supplies.
fn worth(w: &War, side: usize) -> Vec<f32> {
    let fed = |ok: &[bool]| -> i32 { w.on_map(side).filter(|&u| ok[w.units[u].hex.unwrap()]).map(|u| w.units[u].steps as i32).sum() };
    let now = fed(&w.supplied(side));
    w.sc.cities
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut v = c.vp as f32;
            if c.port && w.owner[i] == side {
                let mut t = w.clone();
                t.owner[i] = 1 - side;
                v += 0.5 * (now - fed(&t.supplied(side))) as f32;
            }
            v
        })
        .collect()
}

/// How long the enemy's roads to our cities are, weighed by what each
/// city is worth.
fn block(w: &War, side: usize, worth: &[f32]) -> f32 {
    let foe = 1 - side;
    let src: Vec<(usize, i32)> = w.on_map(foe).map(|u| (w.units[u].hex.unwrap(), 0)).collect();
    let d = field(w, foe, &src);
    let mut s = 0.0;
    for (i, c) in w.sc.cities.iter().enumerate() {
        if worth[i] > 0.0 && w.owner[i] == side {
            let h = w.map.at(c.col, c.row).unwrap();
            s += worth[i] * d[h].min(CAP) as f32;
        }
    }
    s
}

/// What the enemy could do next turn: the attack it could bring against
/// each hex, and the hexes it could step into.
fn threat(w: &War, side: usize) -> (Vec<i32>, Vec<bool>) {
    let foe = 1 - side;
    let zoc = w.zoc_map(foe);
    let mut t = vec![0; w.map.len()];
    let mut reach = vec![false; w.map.len()];
    let mut stamp = vec![usize::MAX; w.map.len()];
    for e in w.on_map(foe).collect::<Vec<_>>() {
        let r = w.reach_as(e, &zoc, w.full_mp(e), true);
        let a = if w.units[e].hungry > 0 { w.attack(e) / 2 } else { w.attack(e) };
        for x in 0..r.len() {
            if r[x].0 == i32::MAX { continue; }
            reach[x] = true;
            for (_, h) in w.map.around(x) {
                if stamp[h] != e {
                    stamp[h] = e;
                    t[h] += a;
                }
            }
        }
    }
    (t, reach)
}

/// Steps the unit would lose on average if `att` attacked it at `h`;
/// a unit that cannot fall back loses a step where it would retreat.
fn risk(w: &War, u: usize, h: usize, att: i32, trapped: bool) -> f32 {
    if att == 0 { return 0.0; }
    let def = w.defence(u);
    let Some(raw) = (0..8).rev().find(|&c| att * ODDS[c].1 >= def * ODDS[c].0) else { return 0.0 };
    let c = w.map.city[h];
    let city = if c >= 0 { w.sc.cities[c as usize].size as i32 } else { 0 };
    let ground = match w.map.ground[h] {
        super::map::Ground::Forest | super::map::Ground::Rough | super::map::Ground::Marsh => 1,
        super::map::Ground::Mountain => 2,
        _ => 0,
    };
    let col = (raw as i32 - city.max(ground)).clamp(0, 7) as usize;
    let steps = w.units[u].steps as f32;
    CRT[col].iter().map(|&r| def_loss(r, steps, trapped)).sum::<f32>() / 6.0
}

/// The steps a defender with `steps` loses to a result.
fn def_loss(r: Res, steps: f32, trapped: bool) -> f32 {
    match r {
        Res::De => steps,
        Res::D1 if trapped => steps.min(2.0),
        Res::D1 | Res::Ex => 1.0,
        Res::Dr if trapped => 1.0,
        Res::Dr => 0.25,
        _ => 0.0,
    }
}

/// The steps an attack is likely to cost each side: (defender, attacker).
fn expect(col: usize, steps: u8, trapped: bool) -> (f32, f32) {
    let s = steps as f32;
    let mut d = 0.0;
    let mut a = 0.0;
    for &r in &CRT[col] {
        d += def_loss(r, s, trapped);
        a += match r {
            Res::A2 => 2.0,
            Res::A1 | Res::Ex => 1.0,
            _ => 0.0,
        };
    }
    (d / 6.0, a / 6.0)
}

/// Whether the unit at `h` could be closed in: enemies stand or could
/// stand on four of the hexes around it, and no friend is beside it.
fn trapped(w: &War, u: usize, h: usize, ereach: &[bool]) -> bool {
    let side = w.side_of(u);
    let mut ring = 0;
    for (_, n) in w.map.around(h) {
        match w.occ[n] {
            Some(o) if o != u && w.side_of(o) == side => return false,
            Some(o) if o != u => ring += 1,
            _ => ring += ereach[n] as i32,
        }
    }
    ring >= 4
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

/// Where a side's units are needed: the hexes one or two from an enemy
/// that it can supply, and its cities the enemy is near. Each starts as
/// far off as it has friends close by, so reserves go where few stand.
fn posts(w: &War, side: usize, dfoe: &[i32], ok: &[bool]) -> Vec<(usize, i32)> {
    let ours: Vec<usize> = w.on_map(side).map(|u| w.units[u].hex.unwrap()).collect();
    (0..w.map.len())
        .filter(|&h| w.map.land(h) && ok[h] && w.occ[h].map_or(true, |o| w.side_of(o) == side))
        .filter(|&h| {
            let c = w.map.city[h];
            let city = c >= 0 && w.sc.cities[c as usize].vp > 0 && w.owner[c as usize] == side;
            (1..=2).contains(&dfoe[h]) || (city && dfoe[h] <= 8)
        })
        .map(|h| (h, 3 * ours.iter().filter(|&&o| w.map.distance(o, h) <= 2).count() as i32))
        .collect()
}

/// Steps from every hex to the nearest one in supply, around enemies.
fn to_supply(w: &War, side: usize, ok: &[bool]) -> Vec<i32> {
    let mut d = vec![FAR; w.map.len()];
    let mut q: std::collections::VecDeque<usize> = (0..w.map.len()).filter(|&h| ok[h]).collect();
    for &h in &q { d[h] = 0; }
    while let Some(h) = q.pop_front() {
        for (_, n) in w.map.around(h) {
            if d[n] == FAR && w.map.land(n) && w.occ[n].map_or(true, |o| w.side_of(o) == side) {
                d[n] = d[h] + 1;
                q.push_back(n);
            }
        }
    }
    d
}

/// Steps over land from a side's supply, its edge and its ports, to
/// every hex.
fn home(w: &War, side: usize) -> Vec<i32> {
    let mut d = vec![FAR; w.map.len()];
    let mut q = std::collections::VecDeque::new();
    for h in 0..w.map.len() {
        let (c, r) = w.map.cr(h);
        let ci = w.map.city[h];
        let port = ci >= 0 && w.sc.cities[ci as usize].port && w.owner[ci as usize] == side;
        if w.map.land(h) && ((w.sc.supply)(side, c, r) || port) {
            d[h] = 0;
            q.push_back(h);
        }
    }
    while let Some(h) = q.pop_front() {
        for (_, n) in w.map.around(h) {
            if d[n] == FAR && w.map.land(n) {
                d[n] = d[h] + 1;
                q.push_back(n);
            }
        }
    }
    d
}

/// For every hex, how far an enemy within five hexes stands between it
/// and our supply: an enemy in our rear.
fn rear(w: &War, side: usize, home: &[i32]) -> Vec<i32> {
    let foes: Vec<usize> = w.on_map(1 - side).map(|e| w.units[e].hex.unwrap()).collect();
    (0..w.map.len())
        .map(|h| foes.iter().filter(|&&f| w.map.distance(f, h) <= 5).map(|&f| home[h] - home[f] - 2).max().unwrap_or(0).max(0))
        .collect()
}

/// What an attack on `e` is worth beyond the steps: it can open the road
/// to supply for our units cut off near it, and drive back an enemy that
/// stands in our rear.
fn rescue(w: &War, e: usize, ok: &[bool], home: &[i32]) -> f32 {
    let eh = w.units[e].hex.unwrap();
    let ours: Vec<usize> = w.on_map(w.side).map(|u| w.units[u].hex.unwrap()).collect();
    let cut = ours.iter().filter(|&&h| !ok[h] && w.map.distance(h, eh) <= 2).count();
    let behind = ours.iter().filter(|&&h| w.map.distance(h, eh) <= 5 && home[h] - home[eh] > 2).count();
    0.25 * cut.min(2) as f32 + 0.15 * behind.min(4) as f32
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
    let (threat, ereach) = threat(&sim, side);
    let goals = if attack { goals(&sim, side) } else { Vec::new() };
    // How many units already head for each city.
    let mut load = vec![0; goals.len()];
    let foes: Vec<usize> = sim.on_map(1 - side).map(|e| sim.units[e].hex.unwrap()).collect();
    let dfoe: Vec<i32> = (0..sim.map.len()).map(|h| foes.iter().map(|&f| sim.map.distance(f, h)).min().unwrap_or(99)).collect();
    let toward = field(&sim, side, &posts(&sim, side, &dfoe, &ok));
    let back = to_supply(&sim, side, &ok);
    let home = home(&sim, side);
    let rear = rear(&sim, side, &home);
    let mut order: Vec<usize> = sim.on_map(side).filter(|&u| !sim.units[u].moved && sim.units[u].mp > 0).collect();
    order.sort_by_key(|&u| (dfoe[sim.units[u].hex.unwrap()], Reverse(sim.attack(u))));
    let base_cut = if attack { cut_off(&sim, side) } else { 0 };
    let worth = worth(&sim, side);
    let base_block = block(&sim, side, &worth);
    for u in order {
        let r = sim.reach_with(u, &zoc);
        let here = sim.units[u].hex.unwrap();
        let mut spots = sim.targets(u, &r);
        spots.push(here);
        let mech = sim.kit(u).kind.mech();
        // Weak foot units guard the flanks and the cities behind, even in
        // an attack.
        let guard = !attack || (!mech && sim.attack(u) <= 3);
        // The city an attacker goes for: near, rich, and not crowded.
        let aim = (0..goals.len()).filter(|_| !guard).min_by_key(|&k| goals[k].1[here].min(200) + 3 * load[k] - 3 * goals[k].0);
        if let Some(k) = aim { load[k] += 1; }
        let mut best = (f32::MIN, here);
        for h in spots {
            let mut s = 0.0;
            let pals = friends(&sim, u, h);
            let front = dfoe[h] <= 3;
            if guard {
                s += try_at(&mut sim, u, h, |w| block(w, side, &worth)) - base_block;
                let c = sim.map.city[h];
                if c >= 0 {
                    // Stand in our cities near the enemy; take back one the
                    // enemy left empty.
                    let (vp, ours) = (worth[c as usize], sim.owner[c as usize] == side);
                    s += if !ours { 4.0 * vp } else if dfoe[h] <= 6 { 3.0 * vp } else { 0.5 * vp };
                }
                // A line: a friend on each side, no more.
                if front { s += 1.5 * pals.min(2) as f32; }
                s -= 0.4 * toward[h].min(80) as f32;
            } else {
                let w_off = if mech { 3.0 } else { 1.5 };
                if let Some(k) = aim { s -= w_off * goals[k].1[h].min(200) as f32; }
                s += 0.5 * pals.min(2) as f32;
                if mech {
                    s += 6.0 * (try_at(&mut sim, u, h, |w| cut_off(w, side)) - base_cut) as f32;
                }
                if goals.is_empty() { s -= 0.4 * toward[h].min(80) as f32; }
            }
            let shut = trapped(&sim, u, h, &ereach);
            s -= risk(&sim, u, h, threat[h], shut) * if guard { 6.0 } else { 4.0 };
            if shut && threat[h] > 0 { s -= if guard { 2.0 } else { 8.0 }; }
            if !ok[h] { s -= 40.0; }
            s -= if guard { 1.5 } else { 1.0 } * rear[h].min(4) as f32;
            // Cut off: head back toward supply.
            if !ok[here] { s -= 3.0 * back[h].min(30) as f32; }
            if h == here { s += 0.3; }
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

/// Whether the defender at `eh` would have nowhere to fall back with our
/// units standing at `ours` as well as where they are.
fn no_way_back(w: &War, eh: usize, ours: &[usize], gone: &[usize]) -> bool {
    let side = w.side;
    let mine = |n: usize| ours.contains(&n) || w.occ[n].map_or(false, |o| w.side_of(o) == side && !gone.contains(&o));
    let theirs = |n: usize| w.occ[n].map_or(false, |o| w.side_of(o) != side);
    for (_, n) in w.map.around(eh) {
        // A friend beside it lets it fall back through.
        if theirs(n) { return false; }
        if !w.map.land(n) || mine(n) { continue; }
        if !w.map.around(n).any(|(_, m)| mine(m)) { return false; }
    }
    true
}

/// Move units next to enemies they can beat, one target at a time.
fn set_up_attacks(sim: &mut War, out: &mut Vec<(usize, Vec<usize>, i32)>, attack: bool) {
    let side = sim.side;
    let zoc = sim.zoc_map(side);
    let ours: Vec<usize> = sim.on_map(side).collect();
    let reach: Vec<Vec<(i32, usize)>> = ours.iter().map(|&u| sim.reach_with(u, &zoc)).collect();
    let mut used = vec![false; sim.units.len()];
    let mut hit = vec![false; sim.units.len()];
    let margin = if attack { 0.15 } else { 0.4 };
    let ok = sim.supplied(side);
    let home = home(sim, side);
    loop {
        let mut best: Option<(f32, usize, Vec<(usize, usize, usize)>)> = None;
        for e in sim.on_map(1 - side).collect::<Vec<_>>() {
            if hit[e] { continue; }
            let eh = sim.units[e].hex.unwrap();
            // Who could stand where: (attack, already there, unit, slot,
            // its place in ours).
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
                if col.map_or(false, |c| c >= 5) { break; }
            }
            let Some(c) = col else { continue };
            let slots: Vec<usize> = pick.iter().map(|p| p.1).collect();
            let movers: Vec<usize> = pick.iter().map(|p| p.0).collect();
            let shut = no_way_back(sim, eh, &slots, &movers);
            let (d, a) = expect(c, sim.units[e].steps, shut);
            let cv = sim.map.city[eh];
            let vp = if cv >= 0 { sim.sc.cities[cv as usize].vp } else { 0 };
            let gain = d - a + if attack && vp > 0 { 0.2 } else { 0.0 } + rescue(sim, e, &ok, &home);
            if gain < margin { continue; }
            let score = gain * 10.0 + vp as f32 * 3.0 - pick.len() as f32 * 0.5;
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
    let attack = w.attacking(w.side);
    let margin = if attack { 0.15 } else { 0.4 };
    let ok = w.supplied(w.side);
    let home = home(w, w.side);
    let mut best: Option<(f32, Vec<usize>, usize)> = None;
    for e in w.on_map(1 - w.side).collect::<Vec<_>>() {
        let mut att = w.attackers_of(e);
        if att.is_empty() { continue; }
        att.sort_by_key(|&a| Reverse(w.attack(a)));
        let Some(col) = w.odds(&att, e).col else { continue };
        while att.len() > 1 && w.odds(&att[..att.len() - 1], e).col == Some(col) { att.pop(); }
        let h = w.units[e].hex.unwrap();
        let (d, a) = expect(col, w.units[e].steps, no_way_back(w, h, &[], &[]));
        let cv = w.map.city[h];
        let vp = if cv >= 0 { w.sc.cities[cv as usize].vp } else { 0 };
        let gain = d - a + if attack && vp > 0 { 0.2 } else { 0.0 } + rescue(w, e, &ok, &home);
        if gain < margin { continue; }
        let score = gain * 10.0 + vp as f32 * 3.0 + if w.units[e].steps == 1 { 2.0 } else { 0.0 };
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
