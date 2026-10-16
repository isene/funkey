//! wargame: an old-school hex wargame, two sides taking turns on a map of
//! a real campaign: North Africa 1942 and Stalingrad 1942. Pick a
//! scenario, a side, or two players, or watch the computer play itself.
//!
//!     cargo run --release --example wargame
//!
//! The arrows (or HJKL) move the cursor. Enter or Space picks a unit and
//! then the hex it goes to; Tab jumps to the next unit that can still
//! move; U takes back a move; N ends the phase. To attack, press Enter on
//! an enemy, then Enter on your units next to it to leave some out, and
//! Enter on the enemy again to roll. ? shows the rules, Q saves and
//! leaves.
//!
//! `WARGAME_BOT=<seed>` plays a whole war, computer against computer,
//! with no terminal and prints how it went. `WARGAME_BOARD=<file.ppm>`
//! paints the whole board with the units at the start, or after
//! `WARGAME_TURN=<n>` turns of such a war. `WARGAME_SCEN=<n>` picks the
//! scenario for both, counted from 0 as on the title.

mod ai;
mod desert;
mod draw;
mod map;
mod rules;
mod scenario;
mod stalingrad;

use draw::Look;
use funkey::*;
use rules::{Phase, War, CRT, ODDS_NAME};
use scenario::Scenario;
use std::collections::VecDeque;

const W: i32 = 960;
const H: i32 = 600;
/// The map's part of the screen; the panel has the rest.
const MAP_W: i32 = 700;
/// The legend's strip over the map.
const TOP: i32 = 18;
const GAME: &str = "wargame";
/// The game's own version; the engine has its own.
const VERSION: &str = "0.2";

const SCENARIOS: &[&Scenario] = &[&desert::SCENARIO, &stalingrad::SCENARIO];

const PANEL: Rgb = 0x1d2127;
const TEXT: Rgb = 0xe8e2d0;
const SOFT: Rgb = 0x9aa0a8;
const GOLD: Rgb = 0xf0c060;
const RED: Rgb = 0xe0604a;

#[derive(Clone, Copy, PartialEq)]
enum Screen { Title, Play, Help, Over }

/// A unit on its way along a path, drawn between hexes.
struct Walk {
    unit: usize,
    from: usize,
    path: Vec<usize>,
    cost: i32,
    t: f32,
}

struct Wargame {
    screen: Screen,
    menu: usize,
    scen: usize,
    war: Option<War>,
    /// True for a side a person plays.
    human: [bool; 2],
    board: Frame,
    board_of: usize,
    cursor: usize,
    view: (i32, i32),
    picked: Option<usize>,
    reach: Vec<(i32, usize)>,
    /// An attack being put together: the defender and the attackers.
    attack: Option<(usize, Vec<usize>)>,
    /// A hex an attack emptied, and who may take it.
    advance: Option<(usize, Vec<usize>)>,
    undo: Vec<War>,
    walk: Option<Walk>,
    /// The computer's moves for this phase, and whether it has made them.
    plan: VecDeque<(usize, Vec<usize>, i32)>,
    planned: bool,
    fast: bool,
    /// A battle's result over its hex, for a moment.
    flash: Option<(usize, String, f32)>,
    repeat: f32,
    help_page: usize,
    time: f32,
}

fn seed() -> u64 { Rng::from_time().next_u32() as u64 | 1 }

impl Wargame {
    fn new() -> Wargame {
        let sc = SCENARIOS[0];
        let map = map::Map::new(sc);
        Wargame {
            screen: Screen::Title,
            menu: 0,
            scen: 0,
            war: None,
            human: [true, false],
            board: draw::board(sc, &map),
            board_of: 0,
            cursor: 0,
            view: (0, 0),
            picked: None,
            reach: Vec::new(),
            attack: None,
            advance: None,
            undo: Vec::new(),
            walk: None,
            plan: VecDeque::new(),
            planned: false,
            fast: false,
            flash: None,
            repeat: 0.0,
            help_page: 0,
            time: 0.0,
        }
    }

    fn saved() -> Option<(usize, [bool; 2], War)> {
        let text = store::load(GAME, "war")?;
        let (head, rest) = text.split_once('\n')?;
        let v: Vec<usize> = head.split(' ').skip(1).map(|x| x.parse().ok()).collect::<Option<_>>()?;
        // The scenario by its name, so a new one in the list moves nothing.
        let s = SCENARIOS.iter().position(|sc| rest.starts_with(&format!("WARGAME 1 {}\n", sc.name)))?;
        let war = War::load(SCENARIOS[s], rest)?;
        (!war.over).then_some((s, [v.get(1) == Some(&1), v.get(2) == Some(&1)], war))
    }

    fn save(&self) {
        if cfg!(test) { return; }
        if let Some(w) = &self.war {
            let head = format!("MODE {} {} {}\n", self.scen, self.human[0] as u8, self.human[1] as u8);
            store::save(GAME, "war", &(head + &w.save()));
        }
    }

    /// The painted board of a scenario, made when it changes.
    fn board_for(&mut self, scen: usize) {
        if self.board_of != scen {
            self.board = draw::board(SCENARIOS[scen], &map::Map::new(SCENARIOS[scen]));
            self.board_of = scen;
        }
    }

    fn start(&mut self, scen: usize, human: [bool; 2], war: War) {
        self.board_for(scen);
        self.scen = scen;
        self.human = human;
        self.cursor = war.units.iter().find_map(|u| u.hex).unwrap_or(0);
        self.war = Some(war);
        self.screen = Screen::Play;
        self.fresh_phase();
        self.look_at(self.cursor, true);
    }

    fn fresh_phase(&mut self) {
        self.picked = None;
        self.attack = None;
        self.advance = None;
        self.undo.clear();
        self.plan.clear();
        self.planned = false;
    }

    fn war(&self) -> &War { self.war.as_ref().unwrap() }

    /// Scroll so a hex is on screen; `middle` puts it in the middle.
    fn look_at(&mut self, hex: usize, middle: bool) {
        let (x, y) = self.war().map.center(hex);
        let (bw, bh) = (self.board.w, self.board.h);
        let (mut vx, mut vy) = self.view;
        let (m, vh) = (70, H - TOP);
        if middle || (x as i32) < vx + m || x as i32 > vx + MAP_W - m || (y as i32) < vy + m || y as i32 > vy + vh - m {
            if middle {
                vx = x as i32 - MAP_W / 2;
                vy = y as i32 - vh / 2;
            } else {
                vx = vx.clamp(x as i32 - MAP_W + m, x as i32 - m);
                vy = vy.clamp(y as i32 - vh + m, y as i32 - m);
            }
        }
        self.view = (vx.clamp(0, (bw - MAP_W).max(0)), vy.clamp(0, (bh - vh).max(0)));
    }

    fn end_phase(&mut self) {
        let side = self.war().side;
        self.war.as_mut().unwrap().next_phase();
        self.fresh_phase();
        let now = self.war().side;
        if now != side {
            self.fast = false;
            let w = self.war();
            let first = w.on_map(now).find_map(|u| w.units[u].hex);
            if let Some(h) = first.filter(|_| self.human[now]) {
                self.cursor = h;
                self.look_at(h, true);
            }
        }
        if self.war().over { self.screen = Screen::Over; }
        self.save();
    }

    fn update_play(&mut self, input: &Input, dt: f32) -> Flow {
        if let Some(f) = &mut self.flash {
            f.2 -= dt;
            if f.2 <= 0.0 { self.flash = None; }
        }
        if self.walk.is_some() {
            let step = if self.fast { 0.02 } else if self.human[self.war().side] { 0.06 } else { 0.11 };
            let w = self.walk.as_mut().unwrap();
            w.t += dt / step;
            if w.t >= w.path.len() as f32 {
                let w = self.walk.take().unwrap();
                self.war.as_mut().unwrap().go(w.unit, &w.path, w.cost);
            }
        }
        if input.pressed(Key::Char('q')) {
            self.save();
            self.screen = Screen::Title;
            return Flow::Continue;
        }
        if input.pressed(Key::Char('?')) {
            self.screen = Screen::Help;
            return Flow::Continue;
        }
        let side = self.war().side;
        if !self.human[side] {
            if input.any_pressed() { self.fast = true; }
            if self.walk.is_none() && self.flash.as_ref().map_or(true, |f| self.fast && f.2 < 0.6) {
                self.computer();
            }
            return Flow::Continue;
        }
        self.keys(input, dt);
        if self.walk.is_some() || self.flash.is_some() || self.arrow_held(input) { Flow::Continue } else { Flow::Wait }
    }

    fn arrow_held(&self, input: &Input) -> bool {
        [Key::Up, Key::Down, Key::Left, Key::Right, Key::Char('h'), Key::Char('j'), Key::Char('k'), Key::Char('l')].iter().any(|&k| input.held(k))
    }

    /// One thing the computer does: a move, an attack, or the end of the
    /// phase.
    fn computer(&mut self) {
        let w = self.war.as_mut().unwrap();
        match w.phase {
            Phase::Move | Phase::Exploit => {
                if !self.planned {
                    self.plan = ai::plan_moves(w).into();
                    self.planned = true;
                }
                match self.plan.pop_front() {
                    Some((u, path, cost)) => {
                        let from = w.units[u].hex.unwrap();
                        if self.fast {
                            w.go(u, &path, cost);
                        } else {
                            let to = *path.last().unwrap();
                            self.walk = Some(Walk { unit: u, from, path, cost, t: 0.0 });
                            self.look_at(to, false);
                        }
                    }
                    None => self.end_phase(),
                }
            }
            Phase::Combat => match ai::pick_attack(w) {
                Some((att, def)) => {
                    let b = w.fight(&att, def);
                    let text = format!("{} {} {}", ODDS_NAME[b.col], b.die + 1, b.res.name());
                    if b.empty {
                        if let Some(a) = ai::pick_advance(w, &att, b.hex) { w.advance(a, b.hex); }
                    }
                    self.flash = Some((b.hex, text, if self.fast { 0.7 } else { 1.1 }));
                    if !self.fast { self.look_at(b.hex, false); }
                }
                None => self.end_phase(),
            },
        }
    }

    fn keys(&mut self, input: &Input, dt: f32) {
        let pressed = |c: char| input.pressed(Key::Char(c));
        // The cursor: a press steps once, a held key keeps stepping.
        let dirs = [(Key::Up, 'k', 0), (Key::Down, 'j', 1), (Key::Left, 'h', 2), (Key::Right, 'l', 3)];
        let mut go = None;
        for &(k, c, d) in &dirs {
            if input.pressed(k) || pressed(c) {
                go = Some(d);
                self.repeat = 0.3;
            }
        }
        if go.is_none() {
            self.repeat -= dt;
            if self.repeat <= 0.0 {
                for &(k, c, d) in &dirs {
                    if input.repeating(k) || input.repeating(Key::Char(c)) {
                        go = Some(d);
                        self.repeat = 0.06;
                    }
                }
            }
        }
        if let Some(d) = go {
            let map = &self.war().map;
            let (c, r) = map.cr(self.cursor);
            let to = match d {
                0 => map.at(c, r - 1),
                1 => map.at(c, r + 1),
                2 => map.at(c - 1, r),
                _ => map.at(c + 1, r),
            };
            if let Some(h) = to {
                self.cursor = h;
                self.look_at(h, false);
            }
        }
        if self.walk.is_some() { return; }
        let phase = self.war().phase;
        if let Some((hex, who)) = self.advance.clone() {
            if input.pressed(Key::Escape) || pressed('n') {
                self.advance = None;
            } else if input.pressed(Key::Enter) || input.pressed(Key::Space) {
                let w = self.war.as_mut().unwrap();
                if let Some(&u) = who.iter().find(|&&u| w.units[u].hex == Some(self.cursor)) {
                    w.advance(u, hex);
                    self.advance = None;
                }
            }
            return;
        }
        if pressed('n') {
            self.end_phase();
            return;
        }
        if input.pressed(Key::Escape) {
            self.picked = None;
            self.attack = None;
            return;
        }
        if pressed('u') && phase != Phase::Combat {
            if let Some(w) = self.undo.pop() {
                self.war = Some(w);
                self.picked = None;
            }
            return;
        }
        if input.pressed(Key::Tab) {
            let w = self.war();
            let ready: Vec<usize> = w.on_map(w.side).filter(|&u| self.can_act(u)).collect();
            if let Some(&u) = ready.iter().find(|&&u| w.units[u].hex.unwrap() > self.cursor).or(ready.first()) {
                let h = w.units[u].hex.unwrap();
                self.cursor = h;
                self.look_at(h, false);
                if phase != Phase::Combat { self.pick(u); }
            }
            return;
        }
        if !(input.pressed(Key::Enter) || input.pressed(Key::Space)) { return; }
        let here = self.war().occ[self.cursor];
        let side = self.war().side;
        match phase {
            Phase::Move | Phase::Exploit => {
                if let Some(u) = here.filter(|&u| self.war().side_of(u) == side) {
                    if self.picked == Some(u) { self.picked = None } else if self.can_act(u) { self.pick(u) }
                } else if let Some(u) = self.picked {
                    let w = self.war();
                    if here.is_none() && self.reach[self.cursor].0 != i32::MAX {
                        let path = War::path(&self.reach, self.cursor);
                        let cost = self.reach[self.cursor].0;
                        let from = w.units[u].hex.unwrap();
                        let snap = w.clone();
                        self.undo.push(snap);
                        self.walk = Some(Walk { unit: u, from, path, cost, t: 0.0 });
                        self.picked = None;
                    }
                }
            }
            Phase::Combat => {
                let w = self.war();
                if let Some((def, att)) = self.attack.clone() {
                    if here == Some(def) {
                        if att.is_empty() || w.odds(&att, def).col.is_none() { return; }
                        let w = self.war.as_mut().unwrap();
                        let b = w.fight(&att, def);
                        let text = format!("{} {} {}", ODDS_NAME[b.col], b.die + 1, b.res.name());
                        self.flash = Some((b.hex, text, 1.4));
                        self.attack = None;
                        if b.empty {
                            let who: Vec<usize> = att.iter().copied().filter(|&a| w.units[a].steps > 0).collect();
                            if !who.is_empty() { self.advance = Some((b.hex, who)); }
                        }
                    } else if let Some(u) = here.filter(|&u| w.attackers_of(def).contains(&u)) {
                        let mut att = att;
                        if let Some(k) = att.iter().position(|&a| a == u) { att.remove(k); } else { att.push(u); }
                        self.attack = Some((def, att));
                    } else if let Some(e) = here.filter(|&e| w.side_of(e) != side && !w.attackers_of(e).is_empty()) {
                        self.attack = Some((e, w.attackers_of(e)));
                    }
                } else if let Some(e) = here.filter(|&e| w.side_of(e) != side && !w.attackers_of(e).is_empty()) {
                    self.attack = Some((e, w.attackers_of(e)));
                }
            }
        }
    }

    /// True for a unit of the side to play that can still do something
    /// this phase.
    fn can_act(&self, u: usize) -> bool {
        let w = self.war();
        let un = &w.units[u];
        match w.phase {
            Phase::Combat => !un.fought && w.units[u].hex.map_or(false, |h| w.map.around(h).any(|(_, n)| w.occ[n].map_or(false, |o| w.side_of(o) != w.side))),
            _ => un.mp > 0 && !un.moved,
        }
    }

    fn pick(&mut self, u: usize) {
        self.picked = Some(u);
        self.reach = self.war().reach(u);
    }

    fn draw_play(&mut self, fr: &mut Frame) {
        let w = self.war.as_ref().unwrap();
        let (vx, vy) = self.view;
        draw::show_board(fr, &self.board, vx, vy, TOP, MAP_W, H - TOP);
        let on = |x: f32, y: f32| x > -30.0 && y > -30.0 && x < MAP_W as f32 + 30.0 && y < H as f32 + 30.0;
        let at = |h: usize| {
            let (x, y) = w.map.center(h);
            (x - vx as f32, y - vy as f32 + TOP as f32)
        };
        // Who holds each city with points, as a small flag.
        for (i, c) in w.sc.cities.iter().enumerate() {
            if c.vp == 0 { continue; }
            let h = w.map.at(c.col, c.row).unwrap();
            let (x, y) = at(h);
            if !on(x, y) { continue; }
            let owner = w.owner[i];
            let paper = w.sc.nations.iter().find(|n| n.side == owner).map(|n| n.paper).unwrap_or(WHITE);
            fr.rect(x as i32 + 13, y as i32 - 4, 5, 8, 0x000000);
            fr.rect(x as i32 + 14, y as i32 - 3, 3, 6, paper);
        }
        if let Some(u) = self.picked {
            for h in w.targets(u, &self.reach) {
                let (x, y) = at(h);
                if on(x, y) { draw::wash(fr, x, y, WHITE, 0.35); }
            }
        }
        if let Some((def, att)) = &self.attack {
            let (x, y) = at(w.units[*def].hex.unwrap());
            draw::wash(fr, x, y, RED, 0.5);
            draw::ring(fr, x, y, RED, 3.0);
            for a in w.attackers_of(*def) {
                let (x, y) = at(w.units[a].hex.unwrap());
                draw::ring(fr, x, y, if att.contains(&a) { GOLD } else { SOFT }, 3.0);
            }
        }
        if let Some((hex, who)) = &self.advance {
            let (x, y) = at(*hex);
            draw::wash(fr, x, y, GOLD, 0.4);
            for &a in who {
                if let Some(h) = w.units[a].hex {
                    let (x, y) = at(h);
                    draw::ring(fr, x, y, GOLD, 3.0);
                }
            }
        }
        let walking = self.walk.as_ref().map(|wk| wk.unit);
        for u in 0..w.units.len() {
            let Some(h) = w.units[u].hex else { continue };
            if Some(u) == walking { continue; }
            let (x, y) = at(h);
            if on(x, y) { self.counter(fr, u, x as i32, y as i32); }
        }
        if let Some(wk) = &self.walk {
            let k = wk.t.floor() as usize;
            let a = if k == 0 { wk.from } else { wk.path[(k - 1).min(wk.path.len() - 1)] };
            let b = wk.path[k.min(wk.path.len() - 1)];
            let (ax, ay) = at(a);
            let (bx, by) = at(b);
            let t = wk.t.fract();
            self.counter(fr, wk.unit, (ax + (bx - ax) * t) as i32, (ay + (by - ay) * t) as i32);
        }
        let (cx, cy) = at(self.cursor);
        draw::ring(fr, cx, cy, WHITE, 2.0);
        if let Some((h, text, _)) = &self.flash {
            let (x, y) = at(*h);
            let tw = Frame::text_width(text, true, 1);
            let bx = (x as i32 - tw / 2 - 4).clamp(2, MAP_W - tw - 10);
            let by = if y < TOP as f32 + 40.0 { y as i32 + 18 } else { y as i32 - 32 };
            fr.rect(bx, by, tw + 8, 13, 0x000000);
            fr.rect(bx + 1, by + 1, tw + 6, 11, 0xfff4d6);
            fr.text_big(bx + 4, by + 3, text, 0x8a1408);
        }
        draw::legend(fr, w.sc, MAP_W, TOP, PANEL, SOFT);
        self.panel(fr);
    }

    fn counter(&self, fr: &mut Frame, u: usize, x: i32, y: i32) {
        let w = self.war();
        let k = w.kit(u);
        let nat = &w.sc.nations[k.nat];
        let un = &w.units[u];
        let mine = w.side_of(u) == w.side;
        let spent = mine && match w.phase {
            Phase::Combat => un.fought,
            Phase::Exploit => !k.kind.mech() || un.moved || un.mp == 0,
            Phase::Move => un.moved,
        };
        let look = Look {
            paper: nat.paper,
            ink: nat.ink,
            kind: k.kind,
            size: k.size,
            numbers: format!("{}-{}-{}", w.attack(u), w.defence(u), k.m),
            reduced: un.steps < 2,
            spent,
            hungry: un.hungry > 0,
            picked: self.picked == Some(u),
        };
        draw::counter(fr, x, y, &look);
    }

    fn panel(&self, fr: &mut Frame) {
        let w = self.war();
        fr.rect(MAP_W, 0, W - MAP_W, H, PANEL);
        fr.vline(MAP_W, 0, H, 0x3a4048);
        let x = MAP_W + 12;
        let mut y = 12;
        let line = |fr: &mut Frame, y: &mut i32, s: &str, c: Rgb| {
            fr.text_big(x, *y, s, c);
            *y += 10;
        };
        line(fr, &mut y, w.sc.name, GOLD);
        let year = w.sc.name.rsplit(' ').next().unwrap_or("");
        line(fr, &mut y, &format!("TURN {} OF {}  {} {}", w.turn + 1, w.sc.dates.len(), w.date(), year), TEXT);
        let weather = match w.weather() {
            b'm' => "MUD",
            b's' => "SNOW",
            _ => "DRY",
        };
        line(fr, &mut y, &format!("WEATHER {}", weather), if weather == "DRY" { SOFT } else { 0x9fd0ff });
        y += 4;
        let who = if self.human[w.side] { "" } else { "  (COMPUTER)" };
        line(fr, &mut y, &format!("{} {}{}", w.sc.sides[w.side], w.phase.name(), who), WHITE);
        line(fr, &mut y, &format!("POINTS {} {}  {} {}", w.sc.sides[0], w.vp(0), w.sc.sides[1], w.vp(1)), SOFT);
        y += 6;
        fr.hline(x, y, W - x - 12, 0x3a4048);
        y += 8;
        // The hex under the cursor.
        let h = self.cursor;
        let (c, r) = w.map.cr(h);
        let mut what = format!("{:02}{:02} {}", c, r, w.map.ground[h].name(w.sc.land));
        if w.map.river[h] != 0 && w.map.river_of[h] > 0 {
            what += ", ";
            what += w.sc.rivers[w.map.river_of[h] as usize - 1].name;
        }
        line(fr, &mut y, &what, TEXT);
        let ci = w.map.city[h];
        if ci >= 0 {
            let city = &w.sc.cities[ci as usize];
            let size = ["TOWN", "CITY", "GREAT CITY"][city.size.min(2) as usize];
            let pts = if city.vp > 0 { format!(" {} PT", city.vp) } else { String::new() };
            line(fr, &mut y, &format!("{} {}{}", city.name, size, pts), GOLD);
            line(fr, &mut y, &format!("HELD BY {}", w.sc.sides[w.owner[ci as usize]]), SOFT);
        }
        if let Some(u) = w.occ[h] {
            let k = w.kit(u);
            y += 4;
            line(fr, &mut y, &format!("{} {}", w.name(u), k.what), WHITE);
            let un = &w.units[u];
            let state = if un.steps < 2 { "REDUCED" } else { "FULL STRENGTH" };
            line(fr, &mut y, &format!("{} {}-{}-{} {}", w.sc.nations[k.nat].name, w.attack(u), w.defence(u), k.m, state), TEXT);
            if w.side_of(u) == w.side && w.phase != Phase::Combat {
                line(fr, &mut y, &format!("{} MOVEMENT POINTS LEFT", un.mp), SOFT);
            }
            if un.hungry > 0 { line(fr, &mut y, "OUT OF SUPPLY", 0xffd400); }
        }
        y += 6;
        fr.hline(x, y, W - x - 12, 0x3a4048);
        y += 8;
        if let Some((def, att)) = &self.attack {
            let o = w.odds(att, *def);
            line(fr, &mut y, &format!("ATTACK {} AGAINST {}", o.att2 as f32 / 2.0, o.def2 / 2), WHITE);
            for (why, n) in &o.why {
                line(fr, &mut y, &format!("{} {:+}", why, n), SOFT);
            }
            match o.col {
                Some(col) => {
                    line(fr, &mut y, &format!("ODDS {}", ODDS_NAME[col]), GOLD);
                    let row: Vec<&str> = CRT[col].iter().map(|r| r.name()).collect();
                    line(fr, &mut y, &format!("DIE 1-6: {}", row.join(" ")), TEXT);
                    line(fr, &mut y, "ENTER ON THE ENEMY ROLLS", SOFT);
                }
                None => line(fr, &mut y, "TOO WEAK TO ATTACK", RED),
            }
            line(fr, &mut y, "ENTER ON YOUR UNIT: IN OR OUT", SOFT);
            y += 6;
        } else if self.advance.is_some() {
            line(fr, &mut y, "THE HEX IS EMPTY.", WHITE);
            line(fr, &mut y, "ENTER ON AN ATTACKER TAKES IT,", TEXT);
            line(fr, &mut y, "ESC STAYS.", TEXT);
            y += 6;
        }
        // The log, newest last.
        let room = ((H - 70 - y) / 10).max(0) as usize;
        for l in w.log.iter().rev().take(room).collect::<Vec<_>>().into_iter().rev() {
            for part in wrap(l, 40) {
                if y > H - 70 { break; }
                line(fr, &mut y, &part, SOFT);
            }
        }
        let keys = if self.human[w.side] {
            match w.phase {
                Phase::Combat => ["ENTER ON AN ENEMY MARKS IT", "N NEXT PHASE  ? RULES  Q QUIT"],
                _ => ["ENTER PICKS AND MOVES  TAB NEXT", "U UNDO  N NEXT PHASE  ? RULES"],
            }
        } else {
            ["ANY KEY: FASTER", "? RULES  Q SAVE AND QUIT"]
        };
        fr.hline(x, H - 34, W - x - 12, 0x3a4048);
        fr.text_big(x, H - 26, keys[0], SOFT);
        fr.text_big(x, H - 15, keys[1], SOFT);
    }

    fn update_title(&mut self, input: &Input) -> Flow {
        let items = self.title_items();
        let n = items.len();
        if input.pressed(Key::Up) || input.pressed(Key::Char('k')) { self.menu = (self.menu + n - 1) % n; }
        if input.pressed(Key::Down) || input.pressed(Key::Char('j')) { self.menu = (self.menu + 1) % n; }
        if SCENARIOS.len() > 1 {
            if input.pressed(Key::Left) || input.pressed(Key::Char('h')) { self.scen = (self.scen + SCENARIOS.len() - 1) % SCENARIOS.len(); }
            if input.pressed(Key::Right) || input.pressed(Key::Char('l')) { self.scen = (self.scen + 1) % SCENARIOS.len(); }
            self.board_for(self.scen);
        }
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        if input.pressed(Key::Char('?')) {
            self.screen = Screen::Help;
            return Flow::Continue;
        }
        if input.pressed(Key::Enter) || input.pressed(Key::Space) {
            let sc = SCENARIOS[self.scen];
            match self.menu.min(n - 1) {
                0 => self.start(self.scen, [true, false], War::new(sc, seed())),
                1 => self.start(self.scen, [false, true], War::new(sc, seed())),
                2 => self.start(self.scen, [true, true], War::new(sc, seed())),
                3 => self.start(self.scen, [false, false], War::new(sc, seed())),
                _ => {
                    if let Some((s, h, w)) = Wargame::saved() { self.start(s, h, w); }
                }
            }
            return Flow::Continue;
        }
        Flow::Wait
    }

    fn title_items(&self) -> Vec<String> {
        let sc = SCENARIOS[self.scen];
        let mut v = vec![
            format!("PLAY {}", sc.sides[0]),
            format!("PLAY {}", sc.sides[1]),
            "TWO PLAYERS".to_string(),
            "WATCH THE COMPUTER".to_string(),
        ];
        if Wargame::saved().is_some() { v.push("GO ON WITH THE SAVED WAR".to_string()); }
        v
    }

    fn draw_title(&mut self, fr: &mut Frame) {
        fr.clear(0x14171c);
        // The board, small, as the picture behind the menu.
        let b = &self.board;
        let (ox, oy, k) = (W - 430, 70, (b.w + 409) / 410);
        for y in 0..(b.h / k).min(H - oy - 20) {
            for x in 0..(b.w / k).min(410) {
                fr.put(ox + x, oy + y, tint(b.get(x * k, y * k), 0.75));
            }
        }
        fr.fancy_text(260, 40, "WARGAME", 6, self.time, (0xf3e3b0, 0xb07a30));
        let sc = SCENARIOS[self.scen];
        let mut y = 120;
        fr.text_scaled(60, y, sc.name, GOLD, true, 2);
        y += 26;
        for l in sc.about {
            fr.text_big(60, y, l, TEXT);
            y += 11;
        }
        if SCENARIOS.len() > 1 {
            fr.text_big(60, y + 4, "LEFT AND RIGHT: ANOTHER SCENARIO", SOFT);
        }
        y += 34;
        for (i, item) in self.title_items().iter().enumerate() {
            let sel = i == self.menu;
            if sel { fr.rect(50, y - 4, 280, 18, 0x3a3020); }
            fr.text_scaled(60, y, item, if sel { WHITE } else { SOFT }, true, 1);
            y += 22;
        }
        fr.text_big(60, H - 40, "ENTER STARTS   ? RULES   Q QUITS", SOFT);
        fr.text(60, H - 22, &format!("WARGAME {}  FUNKEY {}", VERSION, env!("CARGO_PKG_VERSION")), 0x606670);
    }

    fn draw_help(&self, fr: &mut Frame) {
        fr.clear(0x14171c);
        let page = HELP[self.help_page.min(HELP.len() - 1)];
        let mut y = 30;
        fr.text_scaled(40, y, page[0], GOLD, true, 3);
        y += 40;
        for l in &page[1..] {
            for part in wrap(l, 72) {
                fr.text_scaled(40, y, &part, TEXT, true, 2);
                y += 20;
            }
            y += 8;
        }
        fr.text_big(40, H - 26, &format!("PAGE {} OF {}   LEFT AND RIGHT TURN PAGES   ESC GOES BACK", self.help_page + 1, HELP.len()), SOFT);
    }

    fn draw_over(&mut self, fr: &mut Frame) {
        self.draw_play(fr);
        fr.dim(0.35);
        let w = self.war();
        let cx = MAP_W / 2;
        fr.text_centered(cx, 200, "THE WAR IS OVER", WHITE, true, 3);
        fr.text_centered(cx, 250, w.result(), GOLD, true, 2);
        fr.text_centered(cx, 290, &format!("{} HOLD {} POINTS, {} {}", w.sc.sides[0], w.vp(0), w.sc.sides[1], w.vp(1)), TEXT, true, 1);
        fr.text_centered(cx, 305, &format!("STEPS LOST: {} {}, {} {}", w.sc.sides[0], w.lost[0], w.sc.sides[1], w.lost[1]), TEXT, true, 1);
        fr.text_centered(cx, 340, "ANY KEY", SOFT, true, 1);
    }
}

/// Cut a line into parts of at most `n` characters, at spaces.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        if !cur.is_empty() && cur.len() + 1 + word.len() > n {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() { cur.push(' '); }
        cur.push_str(word);
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

const HELP: &[&[&str]] = &[
    &[
        "HOW A TURN GOES",
        "Each turn both sides play in turn. A side moves its units, then attacks, then moves its tanks and motorised troops again with half their movement points.",
        "One unit to a hex. A full unit has two steps; a hit takes one off and the unit is weaker on its reduced side, shown by a pale band.",
        "The numbers on a counter are attack, defence and movement.",
        "Hold the cities with points when the last turn ends. The panel shows what each side holds.",
    ],
    &[
        "MOVING",
        "Each hex costs movement points: clear 1, forest and rough 2, marsh and mountains 3. In the desert, farmland costs 2, salt marsh and sand sea 3. Crossing a river costs 1 more. Tanks pay 1 more off clear ground; mountain troops pay 2 in the mountains.",
        "A unit must stop when it steps next to an enemy. A unit that starts next to one may step away, but not straight into another hex next to an enemy.",
        "A unit that has not moved may always go one hex. Mud halves movement, and the snow slows the side that suffers in it.",
        "Pick a unit with Enter: the hexes it can reach light up. Enter on one of them moves it. U takes a move back.",
    ],
    &[
        "ATTACKING",
        "Units next to an enemy may attack it together, each once a turn. Their attack against its defence gives the odds, from 1:2 to 6:1.",
        "The odds go down a column for forest, rough or marsh, a city, or when every attacker is across a river; two for mountains or a great city. Mud costs a column, and so does snow for the side that suffers in it. Tanks in the open move the odds one column up.",
        "A die gives the result: A1 or A2, the attackers lose steps; DR, the defender falls back a hex; D1, it loses a step and falls back; EX, both lose a step; DE, it is destroyed.",
        "A unit that must fall back and has nowhere to go loses a step instead. When the hex is left empty, one attacker may take it.",
        "Enter on an enemy marks it and lights your units that can join: gold are in, grey are out. Enter on one of yours puts it in or out. Enter on the enemy again rolls the die.",
    ],
    &[
        "SUPPLY",
        "A unit is in supply when a path runs from it to its side's edge of the map, or to a port its side holds, never through an enemy or a hex next to one, unless a friend stands there. In the desert that path may be at most ten hexes long.",
        "Supply is checked at the start of a side's turn. Out of supply a unit moves and attacks at half strength, and from its second turn out of supply it loses a step every turn.",
        "So cutting the enemy off is the way to destroy whole armies: get behind them, and close the ring.",
    ],
];

impl Game for Wargame {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        self.time += dt;
        match self.screen {
            Screen::Title => self.update_title(input),
            Screen::Help => {
                if input.pressed(Key::Right) || input.pressed(Key::Char('l')) || input.pressed(Key::Space) {
                    self.help_page = (self.help_page + 1).min(HELP.len() - 1);
                } else if input.pressed(Key::Left) || input.pressed(Key::Char('h')) {
                    self.help_page = self.help_page.saturating_sub(1);
                } else if input.any_pressed() {
                    self.screen = if self.war.as_ref().map_or(false, |w| !w.over) { Screen::Play } else { Screen::Title };
                }
                Flow::Wait
            }
            Screen::Over => {
                if input.any_pressed() {
                    self.screen = Screen::Title;
                    self.war = None;
                    self.menu = 0;
                }
                Flow::Wait
            }
            Screen::Play => self.update_play(input, dt),
        }
    }

    fn draw(&mut self, fr: &mut Frame) {
        match self.screen {
            Screen::Title => self.draw_title(fr),
            Screen::Help => self.draw_help(fr),
            Screen::Over => self.draw_over(fr),
            Screen::Play => self.draw_play(fr),
        }
    }
}

/// A war played by the computer on both sides, up to the start of turn
/// `stop` (counted from 1) or to the end, with a line for each turn.
fn bot(scen: usize, seed: u64, stop: usize, quiet: bool) -> War {
    let mut w = War::new(SCENARIOS[scen], seed);
    while !w.over && w.turn + 1 < stop {
        match w.phase {
            Phase::Move | Phase::Exploit => {
                for (u, path, cost) in ai::plan_moves(&w) { w.go(u, &path, cost); }
            }
            Phase::Combat => {
                while let Some((att, def)) = ai::pick_attack(&w) {
                    let b = w.fight(&att, def);
                    if b.empty {
                        if let Some(a) = ai::pick_advance(&w, &att, b.hex) { w.advance(a, b.hex); }
                    }
                }
            }
        }
        let (turn, side, phase) = (w.turn, w.side, w.phase);
        w.next_phase();
        if !quiet && side == 1 && phase == Phase::Exploit {
            let units = |s: usize| w.on_map(s).count();
            println!("{:>7} points {:>2} {:>2}  lost {:>3} {:>3}  units {:>2} {:>2}", w.sc.dates[turn], w.vp(0), w.vp(1), w.lost[0], w.lost[1], units(0), units(1));
        }
    }
    w
}

/// The whole board with the units on it, as a picture.
#[cfg(not(target_arch = "wasm32"))]
fn paint(scen: usize, w: War, path: &str) {
    let mut g = Wargame::new();
    g.start(scen, [true, true], w);
    let w = g.war();
    let mut fr = Frame::new(g.board.w, g.board.h);
    fr.px.copy_from_slice(&g.board.px);
    for u in 0..w.units.len() {
        let Some(h) = w.units[u].hex else { continue };
        let (x, y) = w.map.center(h);
        g.counter(&mut fr, u, x as i32, y as i32);
    }
    if std::env::var("WARGAME_GRID").is_ok() {
        for h in 0..w.map.len() {
            let (c, r) = w.map.cr(h);
            let (x, y) = w.map.center(h);
            fr.text(x as i32 - 7, y as i32 - 14, &format!("{}.{}", c, r), 0x0000a0);
        }
    }
    let _ = std::fs::write(path, fr.to_ppm());
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let var = |k: &str| std::env::var(k).ok();
    let scen = var("WARGAME_SCEN").and_then(|n| n.parse().ok()).unwrap_or(0usize).min(SCENARIOS.len() - 1);
    if let Some(s) = var("WARGAME_BOT") {
        let t = std::time::Instant::now();
        let stop = var("WARGAME_TURN").and_then(|n| n.parse().ok()).unwrap_or(99);
        let w = bot(scen, s.parse::<u64>().unwrap_or(1) * 2 + 1, stop, false);
        println!("{}: {} points to {}, {:.1} s", w.result(), w.vp(0), w.vp(1), t.elapsed().as_secs_f32());
        if let Some(p) = var("WARGAME_BOARD") { paint(scen, w, &p); }
        return;
    }
    if let Some(p) = var("WARGAME_BOARD") {
        paint(scen, War::new(SCENARIOS[scen], 1), &p);
        return;
    }
    let mut game = Wargame::new();
    run(&mut game, Config { width: W, height: H, fps: 30 });
}

#[cfg(target_arch = "wasm32")]
funkey::web!(Wargame::new(), Config { width: W, height: H, fps: 30 });

#[cfg(test)]
mod tests {
    use super::*;
    use map::Ground;

    fn unit(w: &War, name: &str) -> usize { (0..w.units.len()).find(|&u| w.name(u) == name).unwrap() }

    /// Take every unit off the board, then stand the named ones on hexes.
    fn only(w: &mut War, at: &[(&str, usize)]) {
        for u in 0..w.units.len() {
            if let Some(h) = w.units[u].hex.take() { w.occ[h] = None; }
        }
        for &(name, h) in at {
            let u = unit(w, name);
            w.units[u].hex = Some(h);
            w.occ[h] = Some(u);
        }
    }

    /// Open ground: a clear hex with no town and six clear neighbours, no
    /// river between any of them.
    fn open(w: &War) -> usize {
        let plain = |h: usize| w.map.ground[h] == Ground::Clear && w.map.city[h] < 0;
        (0..w.map.len())
            .find(|&h| {
                plain(h) && w.map.around(h).count() == 6 && w.map.around(h).all(|(_, n)| plain(n) && !w.map.river_between(h, n)
                    && w.map.around(n).count() == 6 && w.map.around(n).all(|(_, m)| plain(m)))
            })
            .unwrap()
    }

    fn ring(w: &War, h: usize) -> Vec<usize> { w.map.around(h).map(|(_, n)| n).collect() }

    #[test]
    fn odds_follow_the_numbers_and_the_tanks() {
        let mut w = War::new(SCENARIOS[1], 7);
        let h = open(&w);
        let r = ring(&w, h);
        only(&mut w, &[("48", h), ("LV", r[0])]);
        let (lv, ussr) = (unit(&w, "LV"), unit(&w, "48"));
        // 6 against 5 is 1:1.
        assert_eq!(w.odds(&[lv], ussr).col, Some(1));
        only(&mut w, &[("48", h), ("LV", r[0]), ("VII", r[1]), ("XIII", r[2])]);
        let att = [lv, unit(&w, "VII"), unit(&w, "XIII")];
        // 18 against 5 is 3:1.
        assert_eq!(w.odds(&att, ussr).col, Some(4));
        only(&mut w, &[("48", h), ("LV", r[0]), ("VII", r[1]), ("XXIV PZ", r[2])]);
        let att = [lv, unit(&w, "VII"), unit(&w, "XXIV PZ")];
        // 21 against 5 is 4:1, and tanks in the open make it 5:1.
        assert_eq!(w.odds(&att, ussr).col, Some(6));
    }

    #[test]
    fn zones_of_control_stop_and_hold() {
        let mut w = War::new(SCENARIOS[1], 7);
        let h = open(&w);
        let r = ring(&w, h);
        only(&mut w, &[("48", h), ("LV", r[0])]);
        let lv = unit(&w, "LV");
        let zoc = w.zoc_map(0);
        let reach = w.reach(lv);
        for n in 0..w.map.len() {
            if reach[n].0 == i32::MAX || n == r[0] { continue; }
            let from = reach[n].1;
            // No step goes on from a hex next to the enemy, and none goes
            // straight from one such hex to another.
            if from != r[0] { assert!(!zoc[from], "through {:?}", w.map.cr(from)); }
            if from == r[0] { assert!(!zoc[n], "slid to {:?}", w.map.cr(n)); }
        }
    }

    #[test]
    fn a_river_costs_one_more() {
        let w = War::new(SCENARIOS[1], 7);
        let (a, b) = (0..w.map.len())
            .flat_map(|a| w.map.around(a).map(move |(_, b)| (a, b)))
            .find(|&(a, b)| w.map.river_between(a, b) && w.map.ground[b] == Ground::Clear)
            .unwrap();
        assert_eq!(w.step_cost(scenario::Kind::Inf, a, b), Some(2));
        assert_eq!(w.step_cost(scenario::Kind::Arm, a, b), Some(2));
    }

    #[test]
    fn a_ring_of_enemies_cuts_supply() {
        let mut w = War::new(SCENARIOS[1], 7);
        let h = open(&w);
        let r = ring(&w, h);
        only(&mut w, &[("48", h)]);
        assert!(w.supplied(1)[h]);
        let names = ["LV", "VII", "XIII", "XXIX", "XVII", "VIII"];
        let at: Vec<(&str, usize)> = std::iter::once(("48", h)).chain(names.iter().copied().zip(r)).collect();
        only(&mut w, &at);
        assert!(!w.supplied(1)[h]);
    }

    /// Let the computer move side `side` once, as at the start of a turn.
    fn computer_moves(w: &mut War, side: usize) {
        w.side = side;
        w.phase = Phase::Move;
        for u in 0..w.units.len() {
            w.units[u].mp = w.full_mp(u);
            w.units[u].moved = false;
        }
        for (u, path, cost) in ai::plan_moves(w) { w.go(u, &path, cost); }
    }

    #[test]
    fn reserves_far_from_the_fighting_march_to_it() {
        // Three Romanian corps side by side at the west edge, the Soviets
        // far to the east: the clump must not stay put.
        let mut w = War::new(SCENARIOS[1], 7);
        let at = |c, r| w.map.at(c, r).unwrap();
        let (front, clump) = (at(24, 14), [at(0, 12), at(0, 13), at(1, 12)]);
        only(&mut w, &[("ROM I", clump[0]), ("ROM II", clump[1]), ("ROM IV", clump[2]), ("48", front)]);
        computer_moves(&mut w, 0);
        for name in ["ROM I", "ROM II", "ROM IV"] {
            let h = w.units[unit(&w, name)].hex.unwrap();
            assert!(!clump.contains(&h), "{name} stayed at {:?}", w.map.cr(h));
        }
    }

    #[test]
    fn a_unit_cut_off_heads_back_to_supply() {
        let mut w = War::new(SCENARIOS[0], 7);
        for (i, c) in w.sc.cities.iter().enumerate() {
            if c.port { w.owner[i] = 0; }
        }
        let (lost, foe) = (w.map.at(20, 12).unwrap(), w.map.at(2, 2).unwrap());
        only(&mut w, &[("50", lost), ("15 PZ", foe)]);
        assert!(!w.supplied(1)[lost]);
        computer_moves(&mut w, 1);
        let (c, _) = w.map.cr(w.units[unit(&w, "50")].hex.unwrap());
        assert!(c > 20, "it went to column {c}");
    }

    #[test]
    fn the_odds_name_desert_ground_by_its_desert_name() {
        let mut w = War::new(SCENARIOS[0], 7);
        let h = (0..w.map.len()).find(|&h| w.map.ground[h] == Ground::Mountain && w.map.city[h] < 0).unwrap();
        let n = ring(&w, h).into_iter().find(|&n| w.map.ground[n] != Ground::Sea).unwrap();
        only(&mut w, &[("50", h), ("15 PZ", n)]);
        let why: Vec<&str> = w.odds(&[unit(&w, "15 PZ")], unit(&w, "50")).why.iter().map(|r| r.0).collect();
        assert!(why.contains(&"SAND SEA"), "{why:?}");
    }

    #[test]
    fn a_saved_war_comes_back_the_same() {
        for scen in 0..SCENARIOS.len() {
            let w = bot(scen, 5, 3, true);
            let text = w.save();
            let back = War::load(SCENARIOS[scen], &text).unwrap();
            assert_eq!(back.save(), text);
        }
    }

    #[test]
    fn desert_supply_runs_ten_hexes_from_the_edge_or_a_port() {
        let mut w = War::new(SCENARIOS[0], 7);
        let at = |c, r| w.map.at(c, r).unwrap();
        let (near, far, past_tobruk) = (at(10, 12), at(12, 12), at(17, 9));
        only(&mut w, &[("15 PZ", near)]);
        assert!(w.supplied(0)[near]);
        assert!(!w.supplied(0)[far]);
        assert!(!w.supplied(0)[past_tobruk]);
        // Tobruk's harbour feeds whoever holds it.
        let tobruk = w.sc.cities.iter().position(|c| c.name == "TOBRUK").unwrap();
        w.owner[tobruk] = 0;
        assert!(w.supplied(0)[past_tobruk]);
    }

    /// A whole war, computer against computer: `cargo test --release
    /// --example wargame -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_bot_plays() {
        for scen in 0..SCENARIOS.len() {
            for seed in 1..=5 {
                let t = std::time::Instant::now();
                let w = bot(scen, seed * 2 + 1, 99, true);
                println!("{}: {} to {}, steps lost {} {}, {:.1} s", w.result(), w.vp(0), w.vp(1), w.lost[0], w.lost[1], t.elapsed().as_secs_f32());
                assert!(w.over && !w.result().is_empty());
                assert!(w.lost[0] > 0 && w.lost[1] > 0);
                assert!(t.elapsed().as_secs() < 10);
            }
        }
    }
}
