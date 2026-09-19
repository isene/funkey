//! doom: the game, on funkey's sector renderer. Monsters, weapons,
//! pickups, keys, doors, lifts, switches, teleports, exits and the
//! status bar, from any Doom-format WAD.
//!
//!     cargo run --release --example doom [--skill N] [--no-sound] [wad] [map]
//!     cargo run --release --example doom -- --shot out.ppm [wad] [map]
//!
//! The WAD defaults to ~/.funkey/freedoom1.wad and the map to its first.
//! Arrows turn and walk, A and D sidestep, Space fires, E or Enter uses,
//! 1-7 pick a weapon, Tab shows the map, Escape quits. The old cheats
//! work.
//!
//! `DOOM_SCRIPT="up*70,space*3"` runs those keys for that many tics with
//! no terminal, then prints where things stand; `DOOM_SHOT=file.ppm`
//! writes the last frame.

mod ai;
mod hud;
mod info;
mod player;
mod sound;
mod specials;
mod world;

use funkey::doom::{blit_pic, Camera, Renderer};
use funkey::wad::{Art, Level, Wad, ML_TWOSIDED};
use funkey::*;
use hud::{Hud, Inter};
use info::WEAPONS;
use player::{PW_BERSERK, PW_INVULN, PW_MAP, PW_SUIT, PW_VISOR, WEAPONBOTTOM};
use sound::Sound;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use world::World;

thread_local! {
    /// Wall texture heights, for the floors that rise by their lower texture.
    pub static TEXTURE_HEIGHTS: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}

const W: i32 = 320;
const H: i32 = 240;
const VIEW_H: i32 = 202;

enum Screen { Title, Play, Inter(Inter), End }

struct Doom {
    wad: Wad,
    art: Art,
    renderer: Renderer,
    world: World,
    hud: Hud,
    sound: Sound,
    screen: Screen,
    automap: bool,
    map_scale: f32,
    typed: String,
    skill: u8,
    texture_order: Vec<String>,
    flat_order: Vec<String>,
    dead_tics: i32,
    /// Sounds played, by tic: the sound track of a scripted run.
    sound_log: Option<Vec<(i32, String, f32)>>,
}

fn texture_names(wad: &Wad) -> Vec<String> {
    let mut out = Vec::new();
    for tl in ["TEXTURE1", "TEXTURE2"] {
        let Some(t) = wad.lump(tl) else { continue };
        if t.len() < 4 { continue; }
        let n = u32::from_le_bytes([t[0], t[1], t[2], t[3]]) as usize;
        for i in 0..n {
            let a = 4 + i * 4;
            if a + 4 > t.len() { break; }
            let at = u32::from_le_bytes([t[a], t[a + 1], t[a + 2], t[a + 3]]) as usize;
            if at + 8 > t.len() { continue; }
            out.push(funkey::wad::name8(&t[at..at + 8]));
        }
    }
    out
}

fn flat_names(wad: &Wad) -> Vec<String> {
    wad.between("F_START", "F_END").into_iter().chain(wad.between("FF_START", "FF_END")).map(|i| wad.lumps[i].name.clone()).collect()
}

impl Doom {
    fn new(wad_path: &PathBuf, map: Option<String>, skill: u8, sound_on: bool) -> Result<Doom, String> {
        let wad = Wad::open(wad_path)?;
        let map = map.unwrap_or_else(|| wad.lumps.iter().map(|l| l.name.as_str()).find(|n| is_map_name(n)).unwrap_or("E1M1").to_string());
        let art = Art::load(&wad);
        TEXTURE_HEIGHTS.with(|m| { let mut m = m.borrow_mut(); for (n, p) in &art.textures { m.insert(n.clone(), p.h as f32); } });
        let texture_order = texture_names(&wad);
        let flat_order = flat_names(&wad);
        let level = Level::load(&wad, &map)?;
        let world = World::new(level, texture_order.clone(), flat_order.clone(), skill);
        let hud = Hud::load(&wad);
        let sound = Sound::new(&wad, sound_on);
        Ok(Doom { wad, art, renderer: Renderer::new(W, VIEW_H), world, hud, sound, screen: Screen::Title, automap: false,
            map_scale: 0.09, typed: String::new(), skill, texture_order, flat_order, dead_tics: 0, sound_log: None })
    }

    fn load_level(&mut self, name: &str, keep: bool) -> Result<(), String> {
        let level = Level::load(&self.wad, name)?;
        let mut world = World::new(level, self.texture_order.clone(), self.flat_order.clone(), self.skill);
        if keep { world.player.inherit(&self.world.player); }
        self.world = world;
        self.renderer.seen.clear();
        self.automap = false;
        self.dead_tics = 0;
        Ok(())
    }

    fn play(&mut self, name: &str, vol: f32) {
        if let Some(log) = &mut self.sound_log { log.push((self.world.tic, name.to_string(), vol)); }
        self.sound.play(name, vol);
    }

    fn drain_sounds(&mut self) {
        let (px, py) = (self.world.player.x, self.world.player.y);
        let sounds: Vec<_> = self.world.sounds.drain(..).collect();
        for (name, at) in sounds {
            let vol = match at {
                None => 1.0,
                Some((x, y)) => ((1200.0 - ((x - px).powi(2) + (y - py).powi(2)).sqrt()) / 1000.0).clamp(0.0, 1.0),
            };
            if vol > 0.0 { self.play(name, vol); }
        }
    }

    fn start_intermission(&mut self, secret: bool) {
        let w = &self.world;
        let pct = |a: i32, b: i32| if b == 0 { 100 } else { (a * 100 / b).min(100) };
        self.screen = Screen::Inter(Inter {
            finished: w.level.name.clone(), next: info::next_map(&w.level.name, secret),
            kills: pct(w.player.kills, w.total_kills), items: pct(w.player.items, w.total_items), secrets: pct(w.player.secrets, w.total_secrets),
            time: w.player.level_time / 35, shown: [0; 4], stage: 0, tics: 0,
        });
    }

    /// The old cheats, typed while playing.
    fn cheats(&mut self, input: &Input) {
        for c in ('a'..='z').chain('0'..='9') { if input.pressed(Key::Char(c)) { self.typed.push(c); } }
        if self.typed.len() > 16 { let cut = self.typed.len() - 16; self.typed.drain(..cut); }
        let t = self.typed.clone();
        let p = &mut self.world.player;
        let msg: Option<String> = if t.ends_with("iddqd") {
            p.god = !p.god;
            if p.god { p.health = 100; }
            Some(if p.god { "Degreelessness Mode On".into() } else { "Degreelessness Mode Off".into() })
        } else if t.ends_with("idkfa") || t.ends_with("idfa") {
            for w in p.weapons.iter_mut() { *w = true; }
            if !p.backpack { p.backpack = true; for m in p.max_ammo.iter_mut() { *m *= 2; } }
            p.ammo = p.max_ammo;
            p.armor = 200; p.armor_type = 2;
            if t.ends_with("idkfa") { for k in p.keys.iter_mut() { *k = true; } Some("Very Happy Ammo Added".into()) } else { Some("Ammo (no keys) Added".into()) }
        } else if t.ends_with("idclip") || t.ends_with("idspispopd") {
            p.noclip = !p.noclip;
            Some(if p.noclip { "No Clipping Mode ON".into() } else { "No Clipping Mode OFF".into() })
        } else if t.ends_with("iddt") {
            self.world.sky_map = !self.world.sky_map;
            Some(String::new())
        } else if t.len() >= 8 && &t[t.len() - 8..t.len() - 2] == "idclev" && t[t.len() - 2..].chars().all(|c| c.is_ascii_digit()) {
            let d = &t[t.len() - 2..];
            let name = if self.world.level.name.starts_with('E') { format!("E{}M{}", &d[..1], &d[1..]) } else { format!("MAP{}", d) };
            self.typed.clear();
            if self.load_level(&name, true).is_ok() { self.world.message("Changing Level..."); }
            return;
        } else if t.len() >= 9 && &t[t.len() - 9..t.len() - 1] == "idbehold" {
            let k = match t.chars().last() { Some('v') => Some(PW_INVULN), Some('s') => Some(PW_BERSERK), Some('i') => Some(player::PW_INVIS),
                Some('r') => Some(PW_SUIT), Some('a') => Some(PW_MAP), Some('l') => Some(PW_VISOR), _ => None };
            match k { Some(k) => { p.powers[k] = if p.powers[k] > 0 { 0 } else if k == PW_BERSERK || k == PW_MAP { 1 } else { 60 * 35 }; Some("Power-up Toggled".into()) } None => None }
        } else { None };
        if let Some(m) = msg {
            self.typed.clear();
            if !m.is_empty() { self.world.message(&m); }
        }
    }

    fn draw_play(&mut self, f: &mut Frame) {
        let p = &self.world.player;
        // Red for pain, yellow for pickups, green in the suit.
        let mut cnt = p.damage_count;
        if p.powers[PW_BERSERK] > 0 { let bz = 12 - (p.powers[PW_BERSERK] >> 6); if bz > cnt { cnt = bz; } }
        let pal = if cnt > 0 { (((cnt + 7) >> 3).min(7) + 1) as usize }
            else if p.bonus_count > 0 { (((p.bonus_count + 7) >> 3).min(3) + 9) as usize }
            else if p.powers[PW_SUIT] > 128 || p.powers[PW_SUIT] & 8 != 0 { 13 } else { 0 };
        self.art.use_palette(pal);
        let maps = self.art.colormaps.len();
        self.renderer.fixed = if p.powers[PW_INVULN] > 0 && maps > 32 { Some(32) } else if p.powers[PW_VISOR] > 0 { Some(0) } else { None };
        self.renderer.extra_light = p.extra_light;
        let cam = Camera { x: p.x, y: p.y, z: p.viewz, angle: p.angle };
        if self.automap {
            self.draw_automap(f);
        } else {
            let vis = self.world.visible();
            self.renderer.render(&self.world.level, &self.art, &cam, f, &vis);
            self.draw_weapon(f);
        }
        self.hud.draw_status(f, &self.art, &self.world);
    }

    fn draw_weapon(&mut self, f: &mut Frame) {
        let p = &self.world.player;
        if p.dead && p.psp_y >= WEAPONBOTTOM { return; }
        let wp = &WEAPONS[p.weapon];
        let fr = p.psp_seq[p.psp_idx.min(p.psp_seq.len() - 1)];
        let light = self.renderer.light_for(self.world.level.sectors[p.sector].light);
        let cm = if info::bright(fr.0) { 0 } else { light }.min(self.art.colormaps.len() - 1);
        if let Some((pic, _)) = self.art.sprite(&wp.sprite, info::letter(fr.0), 1) {
            let x = p.psp_x - pic.left as f32;
            let y = (p.psp_y - pic.top as f32) * hud::SCALE_Y;
            blit_pic(f, &self.art, pic, x, y, 1.0, hud::SCALE_Y, Some(&self.art.colormaps[cm]));
        }
        if let Some((seq, idx, _)) = p.flash {
            let fr = seq[idx.min(seq.len() - 1)];
            if let Some((pic, _)) = self.art.sprite(&wp.flash_sprite, info::letter(fr.0), 1) {
                let x = p.psp_x - pic.left as f32;
                let y = (p.psp_y - pic.top as f32) * hud::SCALE_Y;
                blit_pic(f, &self.art, pic, x, y, 1.0, hud::SCALE_Y, Some(&self.art.colormaps[0]));
            }
        }
    }

    fn draw_automap(&self, f: &mut Frame) {
        f.rect(0, 0, W, VIEW_H, 0x000000);
        let w = &self.world;
        let p = &w.player;
        let s = self.map_scale;
        let (cx, cy) = (W as f32 / 2.0, VIEW_H as f32 / 2.0);
        let at = |x: f32, y: f32| ((cx + (x - p.x) * s) as i32, (cy - (y - p.y) * s) as i32);
        let all = w.sky_map || p.powers[PW_MAP] > 0;
        for (li, l) in w.level.linedefs.iter().enumerate() {
            if l.flags & 128 != 0 { continue; }
            let seen = self.renderer.seen.get(li).copied().unwrap_or(false);
            if !seen && !all { continue; }
            let colour = if !seen { 0x6c6c6c }
                else if l.flags & ML_TWOSIDED == 0 || l.flags & 32 != 0 { 0xfc0000 }
                else {
                    match w.line_sectors(l) {
                        (Some(a), Some(b)) => {
                            let (sa, sb) = (&w.level.sectors[a], &w.level.sectors[b]);
                            if sa.floor != sb.floor { 0xbc7c4c } else if sa.ceiling != sb.ceiling { 0xfcfc00 } else if all { 0x6c6c6c } else { continue }
                        }
                        _ => 0xfc0000,
                    }
                };
            let (x1, y1, x2, y2) = w.line_points(l);
            let (a, b) = (at(x1, y1), at(x2, y2));
            f.line(a.0, a.1, b.0, b.1, colour);
        }
        if w.sky_map {
            for m in &w.mobjs {
                if m.removed || m.kind != world::Kind::Monster || m.health <= 0 { continue; }
                let (x, y) = at(m.x, m.y);
                f.rect(x - 1, y - 1, 3, 3, 0x00fc00);
            }
        }
        // The player: an arrow.
        let (ax, ay) = at(p.x, p.y);
        let len = 8.0;
        let tip = (ax + (p.angle.cos() * len) as i32, ay - (p.angle.sin() * len) as i32);
        f.line(ax - (p.angle.cos() * len) as i32, ay + (p.angle.sin() * len) as i32, tip.0, tip.1, WHITE);
        for side in [-1.0f32, 1.0] {
            let a = p.angle + side * 2.5;
            f.line(tip.0, tip.1, tip.0 + (a.cos() * 5.0) as i32, tip.1 - (a.sin() * 5.0) as i32, WHITE);
        }
    }
}

impl Game for Doom {
    fn update(&mut self, input: &Input, _dt: f32) -> Flow {
        match self.screen {
            Screen::Title => { if input.any_pressed() { self.screen = Screen::Play; } }
            Screen::End => { if input.any_pressed() { return Flow::Quit; } }
            Screen::Play => {
                if input.pressed(Key::Escape) || (input.pressed(Key::Char('q')) && !self.typed.ends_with("idd")) { return Flow::Quit; }
                if input.pressed(Key::Tab) { self.automap = !self.automap; }
                if self.automap {
                    if input.pressed(Key::Char('-')) { self.map_scale /= 1.25; }
                    if input.pressed(Key::Char('=')) || input.pressed(Key::Char('+')) { self.map_scale *= 1.25; }
                }
                self.cheats(input);
                self.world.tick(input);
                self.drain_sounds();
                if let Some(secret) = self.world.exit { self.start_intermission(secret); return Flow::Continue; }
                if self.world.player.dead {
                    self.dead_tics += 1;
                    if self.dead_tics > 35 && (input.pressed(Key::Space) || input.pressed(Key::Char('e')) || input.pressed(Key::Enter)) {
                        let name = self.world.level.name.clone();
                        let _ = self.load_level(&name, false);
                    }
                }
            }
            Screen::Inter(_) => {
                let mut next_level: Option<Option<String>> = None;
                let mut to_play: Vec<(&str, f32)> = Vec::new();
                if let Screen::Inter(inter) = &mut self.screen {
                    inter.tics += 1;
                    let targets = [inter.kills, inter.items, inter.secrets, inter.time];
                    let stage = inter.stage as usize;
                    if stage < 3 {
                        if inter.tics & 1 == 0 {
                            inter.shown[stage] = (inter.shown[stage] + 2).min(targets[stage]);
                            if inter.shown[stage] < targets[stage] { to_play.push(("PISTOL", 0.6)); }
                            else if inter.tics > 20 { to_play.push(("BAREXP", 0.8)); inter.stage += 1; inter.tics = 0; }
                        }
                    } else if stage == 3 {
                        inter.shown[3] = (inter.shown[3] + 3).min(targets[3]);
                        if inter.shown[3] < targets[3] { if inter.tics & 3 == 0 { to_play.push(("PISTOL", 0.6)); } }
                        else if inter.tics > 20 { to_play.push(("BAREXP", 0.8)); inter.stage = 4; inter.tics = 0; }
                    }
                    if input.any_pressed() && inter.tics > 8 {
                        match inter.stage {
                            0..=3 => { inter.shown = targets; inter.stage = 4; inter.tics = 0; to_play.push(("BAREXP", 0.8)); }
                            4 => { inter.stage = 5; inter.tics = 0; to_play.push(("SGCOCK", 0.8)); }
                            _ => next_level = Some(inter.next.clone()),
                        }
                    }
                }
                for (n, v) in to_play { self.play(n, v); }
                if let Some(next) = next_level {
                    match next {
                        Some(name) => match self.load_level(&name, true) {
                            Ok(()) => self.screen = Screen::Play,
                            Err(_) => self.screen = Screen::End,
                        },
                        None => self.screen = Screen::End,
                    }
                }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        match &self.screen {
            Screen::Title => { self.art.use_palette(0); if !self.hud.full(f, &self.art, "TITLEPIC") { f.clear(0); } }
            Screen::Inter(inter) => { self.art.use_palette(0); self.hud.draw_intermission(f, &self.art, inter); }
            Screen::End => {
                self.art.use_palette(0);
                if !self.hud.full(f, &self.art, "CREDIT") { f.clear(0); self.hud.text(f, &self.art, 130.0, 90.0, "THE END"); }
            }
            Screen::Play => self.draw_play(f),
        }
    }
}

fn is_map_name(n: &str) -> bool {
    let b = n.as_bytes();
    (b.len() == 4 && b[0] == b'E' && b[1].is_ascii_digit() && b[2] == b'M' && b[3].is_ascii_digit())
        || (b.len() == 5 && n.starts_with("MAP") && b[3].is_ascii_digit() && b[4].is_ascii_digit())
}

fn script_key(name: &str) -> Option<Key> {
    Some(match name {
        "up" => Key::Up, "down" => Key::Down, "left" => Key::Left, "right" => Key::Right, "space" => Key::Space,
        "enter" => Key::Enter, "tab" => Key::Tab, "esc" => Key::Escape, "none" | "-" => return None,
        s if s.chars().count() == 1 => Key::Char(s.chars().next().unwrap()),
        _ => return None,
    })
}

/// Run a key script with no terminal, then say where things stand.
fn scripted(game: &mut Doom, script: &str) {
    let mut input = Input::new();
    game.screen = if std::env::var_os("DOOM_TITLE").is_some() { Screen::Title } else { Screen::Play };
    game.sound_log = Some(Vec::new());
    let mut frame = Frame::new(W, H);
    let mut frames_written = 0u32;
    for item in script.split(',') {
        let (name, n) = item.split_once('*').unwrap_or((item, "1"));
        let n: i32 = n.trim().parse().unwrap_or(1);
        // Test hooks: "exit" ends the level, "kill" takes the player's
        // health, "door" or "line:N" puts the player in front of a line.
        if name.trim() == "exit" { game.world.exit = Some(false); }
        if name.trim() == "kill" { game.world.damage_player(1000, None); }
        let li = if name.trim() == "door" { game.world.level.linedefs.iter().position(|l| l.special == 1) }
            else { name.trim().strip_prefix("line:").and_then(|n| n.parse::<usize>().ok()) };
        if let Some(li) = li {
            let w = &mut game.world;
            let (x1, y1, x2, y2) = w.line_points(&w.level.linedefs[li]);
            let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
            let len = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt().max(1.0);
            // The front is to the right of the line's direction.
            let (nx, ny) = ((y2 - y1) / len, -(x2 - x1) / len);
            let p = &mut w.player;
            p.x = mx + nx * 40.0; p.y = my + ny * 40.0; p.angle = (-ny).atan2(-nx); p.mx = 0.0; p.my = 0.0;
            p.sector = w.level.sector_at(p.x, p.y);
            p.z = w.level.sectors[p.sector].floor; p.floorz = p.z; p.ceilingz = w.level.sectors[p.sector].ceiling;
            eprintln!("at line {} special {} tag {}", li, w.level.linedefs[li].special, w.level.linedefs[li].tag);
        }
        let key = script_key(name.trim());
        input.release_all();
        for _ in 0..n {
            input.clear_pressed();
            if let Some(k) = key { input.inject(k); }
            if game.update(&input, 1.0 / 35.0) == Flow::Quit { break; }
            if let Ok(every) = std::env::var("DOOM_SHOT_EVERY") {
                let n: u32 = every.parse().unwrap_or(35);
                frames_written += 1;
                if n > 0 && frames_written % n == 0 {
                    game.draw(&mut frame);
                    let _ = std::fs::write(format!("{}-{:06}.ppm", std::env::var("DOOM_SHOT").unwrap_or("shot".into()), frames_written / n), frame.to_ppm());
                }
            }
        }
    }
    game.draw(&mut frame);
    if let Ok(p) = std::env::var("DOOM_SHOT") { let _ = std::fs::write(p, frame.to_ppm()); }
    if let (Ok(p), Some(log)) = (std::env::var("DOOM_WAV"), &game.sound_log) {
        if let Err(e) = game.sound.render_wav(log, &p) { eprintln!("doom: {}: {}", p, e); }
        eprintln!("{} sounds in the track", log.len());
    }
    let w = &game.world;
    let p = &w.player;
    let alive = w.mobjs.iter().filter(|m| !m.removed && m.kind == world::Kind::Monster && m.health > 0 && m.flags & world::F_COUNTKILL != 0).count();
    let screen = match game.screen { Screen::Play => "play", Screen::Inter(_) => "intermission", Screen::End => "end", Screen::Title => "title" };
    for m in &w.movers {
        let s = m.sector();
        let kind = match m { specials::Mover::Door { .. } => "door", specials::Mover::Plat { .. } => "plat", specials::Mover::Floor { .. } => "floor", specials::Mover::Ceiling { .. } => "ceiling" };
        let inside = w.mobjs.iter().filter(|m| !m.removed && m.sector == s).count();
        let dir = match m { specials::Mover::Door { dir, count, .. } => format!("dir {} count {}", dir, count), _ => String::new() };
        eprintln!("mover {} sector {} floor {} ceiling {} things inside {} {}", kind, s, w.level.sectors[s].floor, w.level.sectors[s].ceiling, inside, dir);
    }
    eprintln!("tic {} screen {} pos {:.0},{:.0} z {:.0} angle {:.0} sector {} health {} armor {} ammo {:?} weapon {} kills {}/{} items {}/{} secrets {}/{} exit {:?} dead {} alive {} movers {} msg {:?}",
        w.tic, screen, p.x, p.y, p.z, p.angle.to_degrees(), p.sector, p.health, p.armor, p.ammo, p.weapon, p.kills, w.total_kills, p.items, w.total_items,
        p.secrets, w.total_secrets, w.exit, p.dead, alive, w.movers.len(), p.message);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut shot: Option<PathBuf> = None;
    let mut skill: u8 = 2;
    let mut sound_on = true;
    if let Some(i) = args.iter().position(|a| a == "--shot") {
        args.remove(i);
        if i < args.len() { shot = Some(PathBuf::from(args.remove(i))); }
    }
    if let Some(i) = args.iter().position(|a| a == "--skill") {
        args.remove(i);
        if i < args.len() { skill = args.remove(i).parse::<u8>().unwrap_or(3).clamp(1, 5) - 1; }
    }
    if let Some(i) = args.iter().position(|a| a == "--no-sound") { args.remove(i); sound_on = false; }
    let wad = args.first().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".funkey/freedoom1.wad")
    });
    let map = args.get(1).cloned();
    let script = std::env::var("DOOM_SCRIPT").ok();
    let mut game = match Doom::new(&wad, map, skill, sound_on && script.is_none() && shot.is_none()) {
        Ok(g) => g,
        Err(e) => { eprintln!("doom: {}", e); std::process::exit(1); }
    };
    if let Some(s) = script { scripted(&mut game, &s); return; }
    if std::env::var_os("DOOM_BENCH").is_some() {
        let mut frame = Frame::new(W, H);
        game.screen = Screen::Play;
        let t0 = std::time::Instant::now();
        for i in 0..200 { game.world.player.angle += 0.02; if i % 50 == 0 { game.world.player.x += 8.0; } game.draw(&mut frame); }
        eprintln!("{:.2} ms a frame at {}x{}", t0.elapsed().as_secs_f64() * 1000.0 / 200.0, W, H);
        return;
    }
    if let Some(path) = shot {
        let mut frame = Frame::new(W, H);
        game.screen = Screen::Play;
        game.draw(&mut frame);
        if let Err(e) = std::fs::write(&path, frame.to_ppm()) { eprintln!("doom: {}: {}", path.display(), e); }
        return;
    }
    run(&mut game, Config { width: W, height: H, fps: 35 });
}
