//! The status bar, messages and the intermission, drawn from the WAD's
//! own pictures at Doom's 320 by 200 positions, stretched to our rows.

use crate::info::{AmmoKind, WEAPONS};
use crate::world::World;
use funkey::doom::blit_pic;
use funkey::wad::{Art, Picture, Wad};
use funkey::Frame;
use std::collections::HashMap;

pub const SCALE_Y: f32 = 1.2;

pub struct Hud { pics: HashMap<String, Picture> }

pub struct Inter {
    pub finished: String,
    pub next: Option<String>,
    pub kills: i32, pub items: i32, pub secrets: i32,
    /// Seconds.
    pub time: i32,
    pub shown: [i32; 4],
    /// 0-2 counting kills, items, secrets; 3 the time; 4 done; 5 "entering".
    pub stage: u8,
    pub tics: i32,
}

impl Hud {
    pub fn load(wad: &Wad) -> Hud {
        let mut names: Vec<String> = ["STBAR", "STARMS", "STTPRCNT", "STTMINUS", "STFGOD0", "STFDEAD0", "WIF", "WIENTER", "WIOSTK", "WIOSTI",
            "WISCRT2", "WITIME", "WIPCNT", "WICOLON", "INTERPIC", "TITLEPIC", "CREDIT", "WIMAP0", "WIMAP1", "WIMAP2"].iter().map(|s| s.to_string()).collect();
        for i in 0..10 { names.push(format!("STTNUM{}", i)); names.push(format!("STYSNUM{}", i)); names.push(format!("WINUM{}", i)); }
        for i in 2..8 { names.push(format!("STGNUM{}", i)); }
        for i in 0..6 { names.push(format!("STKEYS{}", i)); }
        for p in 0..5 {
            for i in 0..3 { names.push(format!("STFST{}{}", p, i)); }
            for s in ["STFTR", "STFTL", "STFOUCH", "STFEVL", "STFKILL"] { names.push(format!("{}{}0", s, p)); }
        }
        for c in 33..96 { names.push(format!("STCFN{:03}", c)); }
        for e in 0..4 { for m in 0..9 { names.push(format!("WILV{}{}", e, m)); } }
        for m in 0..32 { names.push(format!("CWILV{:02}", m)); }
        let mut pics = HashMap::new();
        for n in names { if let Some(p) = wad.picture(&n) { pics.insert(n, p); } }
        Hud { pics }
    }

    /// Draw a picture at Doom's coordinates, minding its own offsets.
    pub fn put(&self, f: &mut Frame, art: &Art, name: &str, x: f32, y: f32) {
        if let Some(p) = self.pics.get(name) { blit_pic(f, art, p, x - p.left as f32, (y - p.top as f32) * SCALE_Y, 1.0, SCALE_Y, None); }
    }

    /// A picture stretched over the whole screen.
    pub fn full(&self, f: &mut Frame, art: &Art, name: &str) -> bool {
        let Some(p) = self.pics.get(name) else { return false };
        blit_pic(f, art, p, 0.0, 0.0, f.w as f32 / p.w as f32, f.h as f32 / p.h as f32, None);
        true
    }

    /// A number with its right edge at x, in the given digit set.
    fn number(&self, f: &mut Frame, art: &Art, set: &str, x: f32, y: f32, n: i32) {
        let digits = n.abs().to_string();
        let w = self.pics.get(&format!("{}0", set)).map(|p| p.w as f32).unwrap_or(14.0);
        let mut cx = x - w * digits.len() as f32;
        if n < 0 { self.put(f, art, "STTMINUS", cx - w, y); }
        for d in digits.chars() { self.put(f, art, &format!("{}{}", set, d), cx, y); cx += w; }
    }

    pub fn text_width(&self, s: &str) -> i32 {
        s.chars().map(|c| match self.pics.get(&format!("STCFN{:03}", c.to_ascii_uppercase() as u32)) { Some(p) => p.w, None => 4 }).sum()
    }

    /// Text in Doom's small font, at Doom coordinates.
    pub fn text(&self, f: &mut Frame, art: &Art, x: f32, y: f32, s: &str) {
        let mut cx = x;
        for c in s.chars() {
            let name = format!("STCFN{:03}", c.to_ascii_uppercase() as u32);
            match self.pics.get(&name) {
                Some(p) => { blit_pic(f, art, p, cx, y * SCALE_Y, 1.0, SCALE_Y, None); cx += p.w as f32; }
                None => cx += 4.0,
            }
        }
    }

    pub fn draw_status(&self, f: &mut Frame, art: &Art, w: &World) {
        let p = &w.player;
        self.put(f, art, "STBAR", 0.0, 168.0);
        self.put(f, art, "STARMS", 104.0, 168.0);
        if let Some(a) = WEAPONS[p.weapon].ammo { self.number(f, art, "STTNUM", 44.0, 171.0, p.ammo[a as usize]); }
        self.number(f, art, "STTNUM", 90.0, 171.0, p.health);
        self.put(f, art, "STTPRCNT", 90.0, 171.0);
        self.number(f, art, "STTNUM", 221.0, 171.0, p.armor);
        self.put(f, art, "STTPRCNT", 221.0, 171.0);
        for slot in 2..8u8 {
            let owned = WEAPONS.iter().any(|wp| wp.slot == slot && p.weapons[WEAPONS.iter().position(|q| std::ptr::eq(q, wp)).unwrap_or(0)]);
            let x = 111.0 + ((slot - 2) % 3) as f32 * 12.0;
            let y = 172.0 + ((slot - 2) / 3) as f32 * 10.0;
            self.put(f, art, &format!("{}{}", if owned { "STYSNUM" } else { "STGNUM" }, slot), x, y);
        }
        self.put(f, art, &p.face_pic(), 143.0, 168.0);
        for k in 0..3 {
            let which = if p.keys[k + 3] { Some(k + 3) } else if p.keys[k] { Some(k) } else { None };
            if let Some(n) = which { self.put(f, art, &format!("STKEYS{}", n), 239.0, 171.0 + k as f32 * 10.0); }
        }
        for (row, kind) in [AmmoKind::Bullets, AmmoKind::Shells, AmmoKind::Rockets, AmmoKind::Cells].iter().enumerate() {
            let y = 173.0 + row as f32 * 6.0;
            self.number(f, art, "STYSNUM", 288.0, y, p.ammo[*kind as usize]);
            self.number(f, art, "STYSNUM", 314.0, y, p.max_ammo[*kind as usize]);
        }
        if p.message_tics > 0 { self.text(f, art, 0.0, 0.0, &p.message); }
    }

    pub fn level_pic(&self, name: &str) -> Option<String> {
        let b = name.as_bytes();
        let pic = if b.len() == 4 && b[0] == b'E' { format!("WILV{}{}", b[1] - b'1', b[3] - b'1') }
            else { format!("CWILV{:02}", name.strip_prefix("MAP")?.parse::<u32>().ok()?.saturating_sub(1)) };
        if self.pics.contains_key(&pic) { Some(pic) } else { None }
    }

    pub fn draw_intermission(&self, f: &mut Frame, art: &Art, inter: &Inter) {
        let b = inter.finished.as_bytes();
        let bg = if b[0] == b'E' && b[1] <= b'3' { format!("WIMAP{}", b[1] - b'1') } else { "INTERPIC".to_string() };
        if !self.full(f, art, &bg) { f.clear(0); }
        let centre = |hud: &Hud, name: &str| hud.pics.get(name).map(|p| 160.0 - p.w as f32 / 2.0).unwrap_or(0.0);
        if inter.stage == 5 {
            self.put(f, art, "WIENTER", centre(self, "WIENTER"), 2.0);
            if let Some(next) = &inter.next {
                match self.level_pic(next) {
                    Some(pic) => self.put(f, art, &pic, centre(self, &pic), 24.0),
                    None => self.text(f, art, 160.0 - self.text_width(next) as f32 / 2.0, 24.0, next),
                }
            }
            return;
        }
        match self.level_pic(&inter.finished) {
            Some(pic) => self.put(f, art, &pic, centre(self, &pic), 2.0),
            None => self.text(f, art, 160.0 - self.text_width(&inter.finished) as f32 / 2.0, 2.0, &inter.finished),
        }
        self.put(f, art, "WIF", centre(self, "WIF"), 24.0);
        let rows = [("WIOSTK", 0), ("WIOSTI", 1), ("WISCRT2", 2)];
        for (k, (name, idx)) in rows.iter().enumerate() {
            let y = 50.0 + k as f32 * 33.0;
            self.put(f, art, name, 50.0, y);
            if inter.stage > *idx as u8 || inter.shown[*idx] > 0 || inter.stage == 4 {
                self.put(f, art, "WIPCNT", 270.0, y);
                self.number(f, art, "WINUM", 270.0, y, inter.shown[*idx]);
            }
        }
        if inter.stage >= 3 {
            self.put(f, art, "WITIME", 16.0, 168.0);
            let t = inter.shown[3];
            let (m, s) = (t / 60, t % 60);
            self.number(f, art, "WINUM", 160.0, 168.0, s);
            self.put(f, art, "WICOLON", 133.0, 168.0);
            self.number(f, art, "WINUM", 132.0, 168.0, m);
            if s < 10 { self.number(f, art, "WINUM", 148.0, 168.0, 0); }
        }
    }
}
