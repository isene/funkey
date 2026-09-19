//! climb: a Jumpman-style platformer, the first game on funkey.
//! Run, jump, climb ladders, take every coin, mind the blobs.
//!
//!     cargo run --release --example climb
//!
//! Arrows or WASD move, Space or Up jumps, Up and Down climb a ladder,
//! R restarts, Q quits.

use funkey::*;

const TILE: i32 = 8;
const LEVEL: [&str; 12] = [
    "################",
    "#o     o     o #",
    "##H#########H###",
    "# H   o     H  #",
    "# H  ####   H  #",
    "######H######H##",
    "#  o  H   b  H #",
    "#   ###H###  H #",
    "#H     H o   H #",
    "#H########H#####",
    "#Ho  b  # H  P #",
    "################",
];

const SKY: Rgb = 0x1a1a2e;
const BRICK: Rgb = 0xb7410e;
const BRICK_DARK: Rgb = 0x7a2a08;
const LADDER: Rgb = 0xe0b93a;
const COIN: Rgb = 0xffd24a;
const TEXT: Rgb = 0xf4ead8;

struct Blob { body: Body, dir: f32 }

struct Climb {
    map: Tilemap,
    player: Body,
    face_left: bool,
    climbing: bool,
    blobs: Vec<Blob>,
    start: (f32, f32),
    score: u32,
    lives: u32,
    time: f32,
    coins_left: usize,
    done: Option<&'static str>,
    flash: f32,
    /// Seconds of grace after a respawn, shown as blinking.
    grace: f32,
    hero: Sprite,
    blob: Sprite,
    coin: Sprite,
}

impl Climb {
    fn new() -> Climb {
        let mut map = Tilemap::from_rows(&LEVEL, TILE);
        map.one_way = vec!['H'];
        let (sx, sy) = map.find('P').first().map(|&(x, y)| (x as f32 * 8.0 + 1.0, y as f32 * 8.0 + 1.0)).unwrap_or((8.0, 8.0));
        for (x, y) in map.find('P') { map.set(x, y, ' '); }
        let blobs = map.find('b').iter().map(|&(x, y)| Blob { body: Body::new(x as f32 * 8.0 + 1.0, y as f32 * 8.0 + 3.0, 6.0, 5.0), dir: 1.0 }).collect();
        for (x, y) in map.find('b') { map.set(x, y, ' '); }
        let coins_left = map.find('o').len();
        let pal = [('b', 0x4ec3ff), ('s', 0xffd7b0), ('d', 0x2a6f9e), ('r', 0xe05252), ('w', 0xffffff), ('k', 0x000000), ('y', COIN), ('o', 0xc99a1a)];
        Climb {
            player: Body::new(sx, sy, 5.0, 7.0),
            face_left: false, climbing: false, blobs, start: (sx, sy),
            score: 0, lives: 3, time: 0.0, coins_left, done: None, flash: 0.0, grace: 1.0,
            hero: Sprite::from_rows(&[".sss.", ".sks.", "bbbbb", ".bbb.", ".ddd.", ".d.d.", ".d.d."], &pal),
            blob: Sprite::from_rows(&[".rrrr.", "rwrrwr", "rkrrkr", "rrrrrr", ".r..r."], &pal),
            coin: Sprite::from_rows(&[".yy.", "yoyy", "yyoy", ".yy."], &pal),
            map,
        }
    }

    fn die(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        self.flash = 0.4;
        if self.lives == 0 { self.done = Some("GAME OVER"); return; }
        self.player.x = self.start.0;
        self.player.y = self.start.1;
        self.player.vx = 0.0;
        self.player.vy = 0.0;
        self.climbing = false;
        self.grace = 1.5;
    }

    fn on_ladder(&self) -> bool {
        let p = &self.player;
        self.map.under(p.x + 1.0, p.y, p.w - 2.0, p.h).iter().any(|&(_, _, c)| c == 'H')
    }

    fn ladder_below(&self) -> bool {
        let p = &self.player;
        self.map.under(p.x + 1.0, p.y + p.h, p.w - 2.0, 1.0).iter().any(|&(_, _, c)| c == 'H')
    }
}

impl Game for Climb {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        if input.pressed(Key::Char('r')) { *self = Climb::new(); return Flow::Continue; }
        if self.done.is_some() { return Flow::Continue; }
        self.time += dt;
        self.flash = (self.flash - dt).max(0.0);
        self.grace = (self.grace - dt).max(0.0);

        let ax = input.axis_x();
        let ay = input.axis_y();
        let jump = input.pressed(Key::Space) || (input.pressed(Key::Up) && !self.on_ladder() && !self.ladder_below());

        // Ladders: Up or Down while over one takes the player onto it.
        if !self.climbing && ay != 0 && ((ay < 0 && self.on_ladder()) || (ay > 0 && (self.ladder_below() || self.on_ladder()))) {
            self.climbing = true;
        }
        if self.climbing {
            if !self.on_ladder() && !self.ladder_below() { self.climbing = false; }
            if jump && input.pressed(Key::Space) { self.climbing = false; }
        }

        let p = &mut self.player;
        if self.climbing {
            // Snap to the ladder's column and move at climbing speed.
            let col = ((p.x + p.w / 2.0) / 8.0).floor() * 8.0;
            p.x += ((col + (8.0 - p.w) / 2.0) - p.x) * 0.5;
            p.vx = 0.0;
            p.vy = ay as f32 * 28.0;
            if ay == 0 && ax != 0 && p.on_ground { self.climbing = false; }
            p.step_through(&self.map, dt, ay > 0);
            let (px, foot, grounded) = (p.x, p.y + p.h, p.on_ground);
            if grounded && ay >= 0 && !self.on_ladder_at(px, foot) { self.climbing = false; }
        } else {
            p.vx = ax as f32 * 42.0;
            p.vy = (p.vy + 260.0 * dt).min(160.0);
            if jump && p.on_ground { p.vy = -98.0; }
            p.step(&self.map, dt);
        }
        if ax != 0 { self.face_left = ax < 0; }

        // Coins under the player.
        let p = &self.player;
        let taken: Vec<(i32, i32)> = self.map.under(p.x, p.y, p.w, p.h).iter().filter(|&&(_, _, c)| c == 'o').map(|&(x, y, _)| (x, y)).collect();
        for (x, y) in taken {
            self.map.set(x, y, ' ');
            self.score += 100;
            self.coins_left -= 1;
        }
        if self.coins_left == 0 { self.done = Some("YOU WIN"); self.score += (300.0 - self.time).max(0.0) as u32; }

        // Blobs patrol their platform and turn at walls and edges.
        for b in &mut self.blobs {
            b.body.vx = b.dir * 18.0;
            b.body.vy = 60.0;
            b.body.step(&self.map, dt);
            let ahead = if b.dir > 0.0 { b.body.x + b.body.w + 1.0 } else { b.body.x - 1.0 };
            let edge = !self.map.blocked(ahead, b.body.y + b.body.h + 1.0, 1.0, 1.0)
                && self.map.lands_on_one_way(ahead, 1.0, b.body.y + b.body.h, b.body.y + b.body.h + 1.0).is_none();
            if b.body.hit_wall || edge { b.dir = -b.dir; }
        }
        let hit = self.grace == 0.0 && self.blobs.iter().any(|b| b.body.overlaps(&self.player));
        if hit { self.die(); }
        Flow::Continue
    }

    fn draw(&self, f: &mut Frame) {
        f.clear(if self.flash > 0.0 { 0x5a1a1a } else { SKY });
        for ty in 0..self.map.h {
            for tx in 0..self.map.w {
                let (x, y) = (tx * TILE, ty * TILE);
                match self.map.at(tx, ty) {
                    '#' => {
                        f.rect(x, y, TILE, TILE, BRICK);
                        f.hline(x, y + 3, TILE, BRICK_DARK);
                        f.hline(x, y + 7, TILE, BRICK_DARK);
                        f.vline(x + 3, y, 4, BRICK_DARK);
                        f.vline(x + 7, y + 4, 4, BRICK_DARK);
                    }
                    'H' => {
                        f.vline(x + 1, y, TILE, LADDER);
                        f.vline(x + 6, y, TILE, LADDER);
                        f.hline(x + 1, y + 2, 6, LADDER);
                        f.hline(x + 1, y + 6, 6, LADDER);
                    }
                    'o' => f.blit(&self.coin, x + 2, y + 2),
                    _ => {}
                }
            }
        }
        for b in &self.blobs { f.blit_flip(&self.blob, b.body.x as i32, b.body.y as i32, b.dir < 0.0); }
        let blink = self.grace > 0.0 && (self.grace * 8.0) as i32 % 2 == 0;
        if !blink { f.blit_flip(&self.hero, self.player.x as i32, self.player.y as i32, self.face_left); }

        let hud = self.map.pixel_h() + 1;
        f.text(2, hud, &format!("SCORE {}", self.score), TEXT);
        f.text(52, hud, &format!("LIVES {}", self.lives), COIN);
        f.text(92, hud, &format!("TIME {}", self.time as u32), TEXT);
        if let Some(msg) = self.done {
            let w = msg.len() as i32 * 4;
            f.rect(64 - w / 2 - 6, 40, w + 12, 21, BLACK);
            f.text(64 - w / 2, 44, msg, COIN);
            f.text(64 - 40, 52, "R AGAIN  Q QUIT", TEXT);
        }
    }
}

impl Climb {
    fn on_ladder_at(&self, x: f32, y: f32) -> bool {
        self.map.under(x + 1.0, y, self.player.w - 2.0, 1.0).iter().any(|&(_, _, c)| c == 'H')
    }
}

fn main() {
    let mut game = Climb::new();
    run(&mut game, Config { width: 128, height: 104, fps: 60 });
}
