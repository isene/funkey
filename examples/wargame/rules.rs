//! The rules. A turn is two player turns, side 0 first. In a player turn
//! the side moves, then fights, then its tanks and motorised troops move
//! again. One unit to a hex.
//!
//! - Moving: each hex costs movement points by its ground, one more over
//!   a river. A unit stops when it steps next to an enemy, and a unit
//!   that starts next to one may not step straight to another hex next
//!   to one. A unit that has not moved may always go one hex.
//! - Fighting: units next to an enemy may attack it together. Their
//!   attack against its defence gives the odds; ground, cities, rivers,
//!   mud and snow move the odds down, tanks in the open move them up. One
//!   die then gives the result. A defender that must give ground and
//!   cannot loses a step instead. A hex left empty may be taken by one of
//!   the attackers.
//! - Supply: a unit is in supply when a path runs from it to its side's
//!   map edge, or a port its side holds, without crossing an enemy or a
//!   hex next to one, unless a friend stands there. A scenario may limit
//!   how long that path is. Out of supply a unit moves and attacks at
//!   half, and from its second turn so it loses a step every turn.

use super::map::{Ground, Map};
use super::scenario::{Kind, Kit, Scenario};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase { Move, Combat, Exploit }

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Move => "MOVEMENT",
            Phase::Combat => "COMBAT",
            Phase::Exploit => "EXPLOITATION",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    /// Its line in the scenario's forces.
    pub f: usize,
    pub hex: Option<usize>,
    /// 2 at full strength, 1 reduced, 0 gone.
    pub steps: u8,
    pub mp: i32,
    pub moved: bool,
    pub fought: bool,
    /// Its own turns out of supply in a row.
    pub hungry: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Res { A2, A1, No, Dr, Ex, D1, De }

impl Res {
    pub fn name(self) -> &'static str {
        match self {
            Res::A2 => "A2",
            Res::A1 => "A1",
            Res::No => "--",
            Res::Dr => "DR",
            Res::Ex => "EX",
            Res::D1 => "D1",
            Res::De => "DE",
        }
    }

    pub fn meaning(self) -> &'static str {
        match self {
            Res::A2 => "ATTACKERS LOSE TWO STEPS",
            Res::A1 => "ATTACKERS LOSE A STEP",
            Res::No => "NO RESULT",
            Res::Dr => "DEFENDER FALLS BACK",
            Res::Ex => "BOTH LOSE A STEP",
            Res::D1 => "DEFENDER LOSES A STEP AND FALLS BACK",
            Res::De => "DEFENDER DESTROYED",
        }
    }
}

/// The odds columns, as attack to defence.
pub const ODDS: [(i32, i32); 8] = [(1, 2), (1, 1), (3, 2), (2, 1), (3, 1), (4, 1), (5, 1), (6, 1)];
pub const ODDS_NAME: [&str; 8] = ["1:2", "1:1", "3:2", "2:1", "3:1", "4:1", "5:1", "6:1"];
use Res::*;
/// The results table: one row per odds column, one result per die face.
pub const CRT: [[Res; 6]; 8] = [
    [A2, A1, A1, A1, No, Dr],
    [A1, A1, No, Dr, Dr, Ex],
    [A1, No, Dr, Dr, Ex, D1],
    [A1, Dr, Dr, Ex, D1, D1],
    [No, Dr, Ex, D1, D1, De],
    [Dr, Ex, D1, D1, De, De],
    [Ex, D1, D1, De, De, De],
    [D1, D1, De, De, De, De],
];

pub struct Odds {
    /// The column used, None when the attack may not be made.
    pub col: Option<usize>,
    /// Attack and defence, doubled so a halved attack stays whole.
    pub att2: i32,
    pub def2: i32,
    /// What moved the odds, and by how many columns (down is negative).
    pub why: Vec<(&'static str, i32)>,
}

pub struct Battle {
    pub hex: usize,
    pub col: usize,
    pub die: usize,
    pub res: Res,
    /// True when the defender's hex is empty afterwards.
    pub empty: bool,
}

#[derive(Clone)]
pub struct War {
    pub sc: &'static Scenario,
    pub map: Rc<Map>,
    pub units: Vec<Unit>,
    /// Which side holds each city.
    pub owner: Vec<usize>,
    /// From 0.
    pub turn: usize,
    pub side: usize,
    pub phase: Phase,
    /// The unit in each hex.
    pub occ: Vec<Option<usize>>,
    pub seed: u64,
    pub log: Vec<String>,
    /// Steps each side has lost.
    pub lost: [i32; 2],
    pub over: bool,
    /// Added to the attack and defence of each side's units: the
    /// computer's side gets 1 against a person.
    pub boost: [i32; 2],
}

impl War {
    pub fn new(sc: &'static Scenario, seed: u64) -> War {
        let map = Rc::new(Map::new(sc));
        let mut w = War {
            sc,
            occ: vec![None; map.len()],
            map,
            units: Vec::new(),
            owner: sc.cities.iter().map(|c| c.owner).collect(),
            turn: 0,
            side: 0,
            phase: Phase::Move,
            seed: seed | 1,
            log: Vec::new(),
            lost: [0, 0],
            over: false,
            boost: [0, 0],
        };
        for (f, force) in sc.forces.iter().enumerate() {
            w.units.push(Unit { f, hex: None, steps: 2, mp: 0, moved: false, fought: false, hungry: 0 });
            if force.turn <= 1 {
                let h = w.map.at(force.col, force.row).expect("a force starts on the map");
                assert!(w.occ[h].is_none() && w.map.land(h), "{} starts on a free land hex", force.name);
                w.put(f, h);
            }
        }
        w.begin();
        w
    }

    /// A die: 0 to 5.
    pub fn roll(&mut self) -> usize {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed = x;
        ((x >> 32) % 6) as usize
    }

    pub fn say(&mut self, s: String) {
        self.log.push(s);
        if self.log.len() > 60 { self.log.remove(0); }
    }

    pub fn kit(&self, u: usize) -> &'static Kit { &self.sc.forces[self.units[u].f].kit }
    pub fn name(&self, u: usize) -> &'static str { self.sc.forces[self.units[u].f].name }
    pub fn side_of(&self, u: usize) -> usize { self.sc.nations[self.kit(u).nat].side }
    pub fn weather(&self) -> u8 { self.sc.weather.as_bytes()[self.turn.min(self.sc.weather.len() - 1)] }
    pub fn date(&self) -> &'static str { self.sc.dates[self.turn.min(self.sc.dates.len() - 1)] }
    pub fn attacking(&self, side: usize) -> bool { self.sc.posture[side].as_bytes().get(self.turn) == Some(&b'a') }

    pub fn attack(&self, u: usize) -> i32 {
        let k = self.kit(u);
        self.boost[self.side_of(u)] + if self.units[u].steps >= 2 { k.a } else { (k.a + 1) / 2 }
    }

    pub fn defence(&self, u: usize) -> i32 {
        let k = self.kit(u);
        self.boost[self.side_of(u)] + if self.units[u].steps >= 2 { k.d } else { (2 * k.d + 1) / 3 }
    }

    /// Movement points for a whole movement phase, after the weather and
    /// supply.
    pub fn full_mp(&self, u: usize) -> i32 {
        let mut m = self.kit(u).m;
        if self.units[u].hungry > 0 { m /= 2; }
        match self.weather() {
            b'm' => m = (m + 1) / 2,
            b's' if self.side_of(u) == self.sc.cold => m -= m / 4,
            _ => {}
        }
        m.max(1)
    }

    pub fn on_map(&self, side: usize) -> impl Iterator<Item = usize> + '_ {
        (0..self.units.len()).filter(move |&u| self.units[u].hex.is_some() && self.side_of(u) == side)
    }

    fn put(&mut self, u: usize, h: usize) {
        if let Some(old) = self.units[u].hex { self.occ[old] = None; }
        self.units[u].hex = Some(h);
        self.occ[h] = Some(u);
        let c = self.map.city[h];
        if c >= 0 {
            let side = self.side_of(u);
            if self.owner[c as usize] != side {
                self.owner[c as usize] = side;
                let city = &self.sc.cities[c as usize];
                if city.vp > 0 { self.say(format!("{} TAKE {}", self.sc.sides[side], city.name)); }
            }
        }
    }

    /// The hexes next to a unit of the side that is not `side`.
    pub fn zoc_map(&self, side: usize) -> Vec<bool> {
        let mut z = vec![false; self.map.len()];
        for u in 0..self.units.len() {
            let Some(h) = self.units[u].hex else { continue };
            if self.side_of(u) == side { continue; }
            for (_, n) in self.map.around(h) { z[n] = true; }
        }
        z
    }

    /// What it costs a unit to step from one hex to its neighbour.
    pub fn step_cost(&self, kind: Kind, from: usize, to: usize) -> Option<i32> {
        let g = self.map.ground[to];
        let mut c = match g {
            Ground::Sea => return None,
            Ground::Clear => 1,
            Ground::Forest | Ground::Rough => 2,
            Ground::Marsh => 3,
            Ground::Mountain => if kind == Kind::Mtn { 2 } else { 3 },
        };
        if kind.mech() && g != Ground::Clear { c += 1; }
        if self.map.river_between(from, to) { c += 1; }
        Some(c)
    }

    /// Where a unit can go now: for every hex the cheapest cost and the
    /// hex it is reached from; i32::MAX where it cannot go.
    pub fn reach(&self, u: usize) -> Vec<(i32, usize)> {
        let zoc = self.zoc_map(self.side_of(u));
        self.reach_with(u, &zoc)
    }

    pub fn reach_with(&self, u: usize, zoc: &[bool]) -> Vec<(i32, usize)> {
        self.reach_as(u, zoc, self.units[u].mp, !self.units[u].moved)
    }

    /// Where a unit could go with `mp` points; `fresh` when it has not
    /// moved yet this phase.
    pub fn reach_as(&self, u: usize, zoc: &[bool], mp: i32, fresh: bool) -> Vec<(i32, usize)> {
        let n = self.map.len();
        let mut best = vec![(i32::MAX, usize::MAX); n];
        let Some(start) = self.units[u].hex else { return best };
        let (side, kind) = (self.side_of(u), self.kit(u).kind);
        best[start] = (0, start);
        if mp <= 0 { return best; }
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0, start)));
        while let Some(Reverse((c, h))) = heap.pop() {
            if c > best[h].0 { continue; }
            if h != start && zoc[h] { continue; }
            for (_, nb) in self.map.around(h) {
                if let Some(o) = self.occ[nb] {
                    if self.side_of(o) != side { continue; }
                }
                let Some(cost) = self.step_cost(kind, h, nb) else { continue };
                if h == start && zoc[start] && zoc[nb] { continue; }
                let mut nc = c + cost;
                if nc > mp {
                    if h == start && fresh { nc = mp } else { continue }
                }
                if nc < best[nb].0 {
                    best[nb] = (nc, h);
                    heap.push(Reverse((nc, nb)));
                }
            }
        }
        best
    }

    /// The hexes a unit can end its move in.
    pub fn targets(&self, u: usize, reach: &[(i32, usize)]) -> Vec<usize> {
        (0..reach.len()).filter(|&h| reach[h].0 != i32::MAX && self.occ[h].is_none() && Some(h) != self.units[u].hex).collect()
    }

    /// The hexes from the start, not counted, to `to`.
    pub fn path(reach: &[(i32, usize)], to: usize) -> Vec<usize> {
        let mut p = vec![to];
        let mut h = to;
        while reach[h].1 != h && reach[h].1 != usize::MAX {
            h = reach[h].1;
            p.push(h);
        }
        p.pop();
        p.reverse();
        p
    }

    /// Move a unit along a path, taking the cities it passes.
    pub fn go(&mut self, u: usize, path: &[usize], cost: i32) {
        for &h in path {
            if self.occ[h].is_none() || self.occ[h] == Some(u) {
                self.put(u, h);
            } else {
                // Through a friend: the city still changes hands.
                let c = self.map.city[h];
                if c >= 0 { self.owner[c as usize] = self.side_of(u); }
            }
        }
        if let Some(&last) = path.last() { self.put(u, last); }
        self.units[u].mp = (self.units[u].mp - cost).max(0);
        self.units[u].moved = true;
    }

    /// The units that could join an attack on `def` now.
    pub fn attackers_of(&self, def: usize) -> Vec<usize> {
        let Some(h) = self.units[def].hex else { return Vec::new() };
        self.map
            .around(h)
            .filter_map(|(_, n)| self.occ[n])
            .filter(|&a| self.side_of(a) == self.side && self.side_of(def) != self.side && !self.units[a].fought)
            .collect()
    }

    pub fn odds(&self, att: &[usize], def: usize) -> Odds {
        let h = self.units[def].hex.unwrap();
        let att2: i32 = att.iter().map(|&a| if self.units[a].hungry > 0 { self.attack(a) } else { 2 * self.attack(a) }).sum();
        let def2 = 2 * self.defence(def);
        let raw = (0..8).rev().find(|&c| att2 * ODDS[c].1 >= def2 * ODDS[c].0);
        let mut why = Vec::new();
        let g = self.map.ground[h];
        let all_mtn = !att.is_empty() && att.iter().all(|&a| self.kit(a).kind == Kind::Mtn);
        let ground = match g {
            Ground::Forest | Ground::Rough | Ground::Marsh => 1,
            Ground::Mountain => if all_mtn { 1 } else { 2 },
            _ => 0,
        };
        let c = self.map.city[h];
        let city = if c >= 0 { self.sc.cities[c as usize].size as i32 } else { 0 };
        if city > 0 && city >= ground {
            why.push((if city > 1 { "GREAT CITY" } else { "CITY" }, -city));
        } else if ground > 0 {
            why.push((g.name(self.sc.land), -ground));
        }
        if !att.is_empty() && att.iter().all(|&a| self.map.river_between(self.units[a].hex.unwrap(), h)) {
            why.push(("RIVER", -1));
        }
        match self.weather() {
            b'm' => why.push(("MUD", -1)),
            b's' if self.side == self.sc.cold => why.push(("SNOW", -1)),
            _ => {}
        }
        if g == Ground::Clear && c < 0 && self.weather() != b'm' && att.iter().any(|&a| self.kit(a).kind == Kind::Arm) {
            why.push(("TANKS IN THE OPEN", 1));
        }
        let shift: i32 = why.iter().map(|w| w.1).sum();
        let col = raw.map(|r| (r as i32 + shift).clamp(0, 7) as usize);
        Odds { col, att2, def2, why }
    }

    pub fn lose_step(&mut self, u: usize) {
        let side = self.side_of(u);
        self.lost[side] += 1;
        self.units[u].steps -= 1;
        if self.units[u].steps == 0 {
            if let Some(h) = self.units[u].hex.take() { self.occ[h] = None; }
            self.say(format!("{} DESTROYED", self.name(u)));
        }
    }

    fn lose_attackers(&mut self, att: &[usize], n: usize) {
        for _ in 0..n {
            let alive: Vec<usize> = att.iter().copied().filter(|&a| self.units[a].steps > 0).collect();
            let Some(&a) = alive.iter().max_by_key(|&&a| (self.units[a].steps, -self.attack(a))) else { return };
            self.lose_step(a);
        }
    }

    /// Fall back one hex, away from the attackers, or two through a
    /// friend; with nowhere to go, lose a step.
    fn retreat(&mut self, u: usize, att: &[usize]) {
        let from = self.units[u].hex.unwrap();
        let side = self.side_of(u);
        let zoc = self.zoc_map(side);
        let from_hexes: Vec<usize> = att.iter().filter_map(|&a| self.units[a].hex).collect();
        let away = |w: &War, h: usize| from_hexes.iter().map(|&a| w.map.distance(a, h)).min().unwrap_or(0);
        let free = |w: &War, h: usize| w.map.land(h) && w.occ[h].is_none() && !zoc[h];
        let mut best: Option<(i32, usize)> = None;
        for (_, n) in self.map.around(from) {
            if free(self, n) {
                let s = away(self, n) * 10 - self.map.river_between(from, n) as i32;
                if best.map_or(true, |b| s > b.0) { best = Some((s, n)); }
            }
        }
        if best.is_none() {
            for (_, n) in self.map.around(from) {
                let friend = self.occ[n].map_or(false, |o| self.side_of(o) == side);
                if !friend { continue; }
                for (_, m) in self.map.around(n) {
                    if m != from && free(self, m) {
                        let s = away(self, m);
                        if best.map_or(true, |b| s > b.0) { best = Some((s, m)); }
                    }
                }
            }
        }
        match best {
            Some((_, h)) => self.put(u, h),
            None => {
                self.say(format!("{} CANNOT FALL BACK", self.name(u)));
                self.lose_step(u);
            }
        }
    }

    pub fn fight(&mut self, att: &[usize], def: usize) -> Battle {
        let hex = self.units[def].hex.unwrap();
        let col = self.odds(att, def).col.expect("an attack at 1:2 or better");
        let die = self.roll();
        let res = CRT[col][die];
        for &a in att { self.units[a].fought = true; }
        self.say(format!("ATTACK ON {} AT {}, DIE {}: {}", self.name(def), ODDS_NAME[col], die + 1, res.meaning()));
        match res {
            A2 => self.lose_attackers(att, 2),
            A1 => self.lose_attackers(att, 1),
            No => {}
            Dr => self.retreat(def, att),
            Ex => {
                self.lose_step(def);
                self.lose_attackers(att, 1);
            }
            D1 => {
                self.lose_step(def);
                if self.units[def].steps > 0 { self.retreat(def, att); }
            }
            De => {
                while self.units[def].steps > 0 { self.lose_step(def); }
            }
        }
        Battle { hex, col, die, res, empty: self.occ[hex].is_none() }
    }

    /// An attacker takes the hex the defender left.
    pub fn advance(&mut self, u: usize, hex: usize) { self.put(u, hex); }

    /// The hexes from which a side can trace supply.
    pub fn supplied(&self, side: usize) -> Vec<bool> {
        let zoc = self.zoc_map(side);
        let n = self.map.len();
        let pass = |h: usize| self.map.land(h) && self.occ[h].map_or(!zoc[h], |o| self.side_of(o) == side);
        let port = |h: usize| self.map.city[h] >= 0 && self.sc.cities[self.map.city[h] as usize].port && self.owner[self.map.city[h] as usize] == side;
        let mut ok = vec![false; n];
        let mut q = VecDeque::new();
        for h in 0..n {
            let (c, r) = self.map.cr(h);
            if ((self.sc.supply)(side, c, r) || port(h)) && pass(h) {
                ok[h] = true;
                q.push_back((h, 0));
            }
        }
        let far = if self.sc.range > 0 { self.sc.range } else { i32::MAX };
        while let Some((h, d)) = q.pop_front() {
            if d >= far { continue; }
            for (_, m) in self.map.around(h) {
                if !ok[m] && pass(m) {
                    ok[m] = true;
                    q.push_back((m, d + 1));
                }
            }
        }
        ok
    }

    /// The start of a player turn: new units come in, supply is traced,
    /// every unit gets its movement points.
    pub fn begin(&mut self) {
        let side = self.side;
        let zoc = self.zoc_map(side);
        for u in 0..self.units.len() {
            let force = &self.sc.forces[self.units[u].f];
            if self.side_of(u) != side || self.units[u].hex.is_some() || self.units[u].steps == 0 || force.turn > self.turn + 1 {
                continue;
            }
            let Some(entry) = self.map.at(force.col, force.row) else { continue };
            // The entry hex, or the nearest free one that is not under the
            // enemy's guns.
            let mut seen = vec![false; self.map.len()];
            let mut q = VecDeque::from([(entry, 0)]);
            seen[entry] = true;
            let mut spot = None;
            while let Some((h, d)) = q.pop_front() {
                if self.occ[h].is_none() && !zoc[h] {
                    spot = Some(h);
                    break;
                }
                if d >= 4 { continue; }
                for (_, m) in self.map.around(h) {
                    let enemy = self.occ[m].map_or(false, |o| self.side_of(o) != side);
                    if !seen[m] && self.map.land(m) && !enemy {
                        seen[m] = true;
                        q.push_back((m, d + 1));
                    }
                }
            }
            if let Some(h) = spot {
                self.put(u, h);
                self.say(format!("{} ARRIVES", force.name));
            }
        }
        let ok = self.supplied(side);
        for u in self.on_map(side).collect::<Vec<_>>() {
            if ok[self.units[u].hex.unwrap()] {
                self.units[u].hungry = 0;
            } else {
                self.units[u].hungry = self.units[u].hungry.saturating_add(1);
                if self.units[u].hungry >= 2 {
                    self.say(format!("{} STARVES", self.name(u)));
                    self.lose_step(u);
                }
            }
        }
        for u in 0..self.units.len() {
            if self.side_of(u) == side {
                self.units[u].mp = self.full_mp(u);
                self.units[u].moved = false;
                self.units[u].fought = false;
            }
        }
        self.phase = Phase::Move;
    }

    pub fn next_phase(&mut self) {
        match self.phase {
            Phase::Move => self.phase = Phase::Combat,
            Phase::Combat => {
                self.phase = Phase::Exploit;
                for u in 0..self.units.len() {
                    if self.side_of(u) == self.side {
                        self.units[u].mp = if self.kit(u).kind.mech() { self.full_mp(u) / 2 } else { 0 };
                        self.units[u].moved = false;
                    }
                }
            }
            Phase::Exploit => {
                if self.side == 0 {
                    self.side = 1;
                } else {
                    self.side = 0;
                    self.turn += 1;
                    if self.turn >= self.sc.dates.len() {
                        self.over = true;
                        self.turn = self.sc.dates.len() - 1;
                        return;
                    }
                }
                self.begin();
            }
        }
    }

    /// Victory points held by a side.
    pub fn vp(&self, side: usize) -> i32 {
        self.sc.cities.iter().zip(&self.owner).filter(|(_, &o)| o == side).map(|(c, _)| c.vp).sum()
    }

    pub fn result(&self) -> &'static str {
        let vp = self.vp(0);
        self.sc.levels.iter().find(|l| vp >= l.0).map(|l| l.1).unwrap_or("")
    }

    /// The whole war as text, to be picked up again later.
    pub fn save(&self) -> String {
        let mut s = format!("WARGAME 1 {}\n", self.sc.name);
        s += &format!("{} {} {} {} {} {} {}\n", self.turn, self.side, self.phase as u8, self.seed, self.lost[0], self.lost[1], self.over as u8);
        s += &self.owner.iter().map(|o| o.to_string()).collect::<String>();
        s += "\n";
        for u in &self.units {
            let h = u.hex.map(|h| h as i64).unwrap_or(-1);
            s += &format!("{} {} {} {} {} {}\n", h, u.steps, u.mp, u.moved as u8, u.fought as u8, u.hungry);
        }
        for l in self.log.iter().rev().take(12).rev() {
            s += &format!("L {}\n", l);
        }
        s
    }

    pub fn load(sc: &'static Scenario, text: &str) -> Option<War> {
        let mut lines = text.lines();
        if lines.next()? != format!("WARGAME 1 {}", sc.name) { return None; }
        let mut w = War::new(sc, 1);
        let n: Vec<u64> = lines.next()?.split(' ').map(|x| x.parse().ok()).collect::<Option<_>>()?;
        if n.len() != 7 { return None; }
        w.turn = n[0] as usize;
        w.side = n[1] as usize;
        w.phase = [Phase::Move, Phase::Combat, Phase::Exploit][(n[2] as usize).min(2)];
        w.seed = n[3].max(1);
        w.lost = [n[4] as i32, n[5] as i32];
        w.over = n[6] != 0;
        let own = lines.next()?;
        if own.len() != w.owner.len() { return None; }
        w.owner = own.bytes().map(|b| (b - b'0') as usize).collect();
        w.occ = vec![None; w.map.len()];
        for u in 0..w.units.len() {
            let v: Vec<i64> = lines.next()?.split(' ').map(|x| x.parse().ok()).collect::<Option<_>>()?;
            if v.len() != 6 { return None; }
            let hex = (v[0] >= 0 && (v[0] as usize) < w.map.len()).then_some(v[0] as usize);
            w.units[u] = Unit { f: u, hex, steps: v[1] as u8, mp: v[2] as i32, moved: v[3] != 0, fought: v[4] != 0, hungry: v[5] as u8 };
            if let Some(h) = hex { w.occ[h] = Some(u); }
        }
        w.log = lines.filter_map(|l| l.strip_prefix("L ").map(String::from)).collect();
        Some(w)
    }
}
