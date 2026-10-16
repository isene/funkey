//! The board: hexes in columns with a flat side up, every odd column half
//! a hex lower. Sides of a hex are numbered from north, clockwise: N, NE,
//! SE, S, SW, NW. A river runs along hex sides, so a unit crosses it when
//! it steps over one of those sides.

use crate::scenario::{Land, Scenario};
use std::collections::HashMap;

/// A hex's corner radius, and its height, in pixels.
pub const S: f32 = 18.0;
pub const HH: f32 = 31.176915;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ground { Clear, Forest, Rough, Marsh, Mountain, Sea }

impl Ground {
    fn from(c: char) -> Ground {
        match c {
            'f' => Ground::Forest,
            'r' => Ground::Rough,
            'm' => Ground::Marsh,
            '^' => Ground::Mountain,
            '~' => Ground::Sea,
            _ => Ground::Clear,
        }
    }

    /// What the ground is called in a scenario's land: the desert has
    /// farmland, salt marsh and sand sea where the steppe has forest,
    /// marsh and mountains.
    pub fn name(self, land: Land) -> &'static str {
        match (land, self) {
            (Land::Desert, Ground::Clear) => "DESERT",
            (Land::Desert, Ground::Forest) => "FARMLAND",
            (Land::Desert, Ground::Marsh) => "SALT MARSH",
            (Land::Desert, Ground::Mountain) => "SAND SEA",
            (_, Ground::Clear) => "CLEAR",
            (_, Ground::Forest) => "FOREST",
            (_, Ground::Rough) => "ROUGH",
            (_, Ground::Marsh) => "MARSH",
            (_, Ground::Mountain) => "MOUNTAINS",
            (_, Ground::Sea) => "SEA",
        }
    }
}

pub struct Map {
    pub cols: i32,
    pub rows: i32,
    pub ground: Vec<Ground>,
    /// Bit d set: a river along side d.
    pub river: Vec<u8>,
    /// Which river runs along a hex, as its place in the scenario's list
    /// plus one, or 0.
    pub river_of: Vec<u8>,
    /// The city in a hex, as its place in the scenario's list, or -1.
    pub city: Vec<i32>,
    /// Every river side as a line in pixels, for drawing.
    pub river_lines: Vec<((f32, f32), (f32, f32))>,
}

/// Steps to the six neighbours, for even and for odd columns.
const EVEN: [(i32, i32); 6] = [(0, -1), (1, -1), (1, 0), (0, 1), (-1, 0), (-1, -1)];
const ODD: [(i32, i32); 6] = [(0, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0)];

impl Map {
    pub fn new(sc: &Scenario) -> Map {
        let rows = sc.ground.len() as i32;
        let cols = sc.ground[0].chars().count() as i32;
        let ground = sc.ground.iter().flat_map(|r| r.chars().map(Ground::from)).collect::<Vec<_>>();
        assert_eq!(ground.len(), (rows * cols) as usize, "every row of {} has {} hexes", sc.name, cols);
        let mut city = vec![-1; ground.len()];
        for (i, c) in sc.cities.iter().enumerate() {
            city[(c.row * cols + c.col) as usize] = i as i32;
        }
        let mut map = Map { cols, rows, ground, river: vec![0; (rows * cols) as usize], river_of: vec![0; (rows * cols) as usize], city, river_lines: Vec::new() };
        map.lay_rivers(sc);
        map
    }

    pub fn len(&self) -> usize { (self.cols * self.rows) as usize }

    pub fn at(&self, c: i32, r: i32) -> Option<usize> {
        (c >= 0 && r >= 0 && c < self.cols && r < self.rows).then(|| (r * self.cols + c) as usize)
    }

    pub fn cr(&self, i: usize) -> (i32, i32) { (i as i32 % self.cols, i as i32 / self.cols) }

    /// The neighbour across side `d`, if it is on the map.
    pub fn step(&self, i: usize, d: usize) -> Option<usize> {
        let (c, r) = self.cr(i);
        let (dc, dr) = if c & 1 == 0 { EVEN[d] } else { ODD[d] };
        self.at(c + dc, r + dr)
    }

    /// The neighbours on the map, with the side each lies across.
    pub fn around(&self, i: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..6).filter_map(move |d| self.step(i, d).map(|n| (d, n)))
    }

    pub fn land(&self, i: usize) -> bool { self.ground[i] != Ground::Sea }

    /// True when a river runs between two neighbours.
    pub fn river_between(&self, a: usize, b: usize) -> bool {
        self.around(a).any(|(d, n)| n == b && self.river[a] & (1 << d) != 0)
    }

    pub fn distance(&self, a: usize, b: usize) -> i32 {
        let cube = |i: usize| {
            let (c, r) = self.cr(i);
            let z = r - (c - (c & 1)) / 2;
            (c, -c - z, z)
        };
        let (a, b) = (cube(a), cube(b));
        (a.0 - b.0).abs().max((a.1 - b.1).abs()).max((a.2 - b.2).abs())
    }


    /// The size of the whole board in pixels.
    pub fn pixels(&self) -> (i32, i32) {
        ((2.0 * S + 1.5 * S * (self.cols - 1) as f32).ceil() as i32 + 1, (HH * self.rows as f32 + HH / 2.0).ceil() as i32 + 1)
    }

    /// The middle of a hex in pixels.
    pub fn center(&self, i: usize) -> (f32, f32) {
        let (c, r) = self.cr(i);
        (S + 1.5 * S * c as f32, HH * (r as f32 + 0.5 * (c & 1) as f32) + HH / 2.0)
    }

    /// Corner k of a hex, k from 0 at the east going clockwise.
    pub fn corner(&self, i: usize, k: usize) -> (f32, f32) {
        let (x, y) = self.center(i);
        let a = (k as f32 * 60.0).to_radians();
        (x + S * a.cos(), y + S * a.sin())
    }

    /// Rivers follow hex sides: from each point of a river to the next,
    /// walk the corners of the hexes, always to the corner nearest the
    /// point, and mark every side walked as a river.
    fn lay_rivers(&mut self, sc: &Scenario) {
        // A corner's place on the lattice all corners share: x in ninths
        // of a hex, y in half hex heights.
        let key = |p: (f32, f32)| ((p.0 / 9.0).round() as i32, (p.1 / (HH / 2.0)).round() as i32);
        let mut spot: HashMap<(i32, i32), (f32, f32)> = HashMap::new();
        let mut next: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
        let mut sides: HashMap<((i32, i32), (i32, i32)), Vec<(usize, usize)>> = HashMap::new();
        for i in 0..self.len() {
            for k in 0..6 {
                let (p, q) = (self.corner(i, k), self.corner(i, (k + 1) % 6));
                let (kp, kq) = (key(p), key(q));
                spot.insert(kp, p);
                let n = next.entry(kp).or_default();
                if !n.contains(&kq) { n.push(kq); }
                let n = next.entry(kq).or_default();
                if !n.contains(&kp) { n.push(kp); }
                // Side k of a hex faces the direction (k + 2) mod 6.
                sides.entry((kp.min(kq), kp.max(kq))).or_default().push((i, (k + 2) % 6));
            }
        }
        let (lon0, lat0, dlon, dlat) = sc.geo;
        let dist = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        for (ri, river) in sc.rivers.iter().enumerate() {
            let pts: Vec<(f32, f32)> = river.points.iter().map(|&(lon, lat)| ((lon - lon0) / dlon, (lat0 - lat) / dlat)).collect();
            let Some(mut at) = spot.iter().min_by(|a, b| dist(*a.1, pts[0]).total_cmp(&dist(*b.1, pts[0]))).map(|(k, _)| *k) else { continue };
            for &p in &pts[1..] {
                for _ in 0..60 {
                    let here = dist(spot[&at], p);
                    if here < S * 0.9 { break; }
                    let Some(&to) = next[&at].iter().min_by(|a, b| dist(spot[a], p).total_cmp(&dist(spot[b], p))) else { break };
                    if dist(spot[&to], p) >= here { break; }
                    for &(h, d) in &sides[&(at.min(to), at.max(to))] {
                        self.river[h] |= 1 << d;
                        self.river_of[h] = ri as u8 + 1;
                    }
                    self.river_lines.push((spot[&at], spot[&to]));
                    at = to;
                }
            }
        }
        // A river side between land and sea is coast, not a crossing.
        for i in 0..self.len() {
            for d in 0..6 {
                if let Some(n) = self.step(i, d) {
                    if !self.land(i) || !self.land(n) { self.river[i] &= !(1 << d); }
                }
            }
        }
    }
}
