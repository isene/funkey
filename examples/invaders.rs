//! invaders: fifty-five of them, marching down. Space Invaders in spirit.
//! Shields crumble where they are hit, the march quickens as the rows
//! thin, a mystery ship crosses the top now and then, and each wave
//! starts a little lower.
//!
//!     cargo run --release --example invaders
//!
//! Left and Right move, Space fires, Q quits.

use funkey::*;

const W: i32 = 224;
const H: i32 = 256;
const GAME: &str = "invaders";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.0";

const COLS: usize = 11;
const ROWS: usize = 5;
const GROUND: i32 = 239;
const PLAYER_Y: i32 = 216;
const GREEN: Rgb = 0x40e040;
const RED: Rgb = 0xff4040;
const CYAN: Rgb = 0x40e0e0;

#[derive(Clone, Copy, PartialEq)]
enum Mode { Title, Play, Dying(f32), Wave(f32), Over(f32) }

struct Shot { x: f32, y: f32, vy: f32, kind: u8 }
struct Boom { x: i32, y: i32, t: f32 }

struct Invaders {
    alive: [[bool; COLS]; ROWS],
    fx: f32, fy: f32, dir: f32, step_timer: f32, frame: bool,
    player_x: f32,
    shot: Option<Shot>,
    bombs: Vec<Shot>,
    bomb_timer: f32,
    shields: Vec<(i32, Sprite)>,
    ufo: Option<(f32, f32, u32)>,
    ufo_timer: f32,
    ufo_hit: Option<(i32, u32, f32)>,
    booms: Vec<Boom>,
    score: u32, high: u32, lives: u32, wave: u32,
    mode: Mode, time: f32, beat: usize,
    rng: Rng,
    audio: Audio,
    inv: [[Sprite; 2]; 3], player: Sprite, ufo_s: Sprite, boom_s: Sprite, shield: Sprite, cannon_boom: [Sprite; 2],
    s_shoot: Sample, s_kill: Sample, s_die: Sample, s_ufo: Sample, s_ufo_hit: Sample, s_beat: [Sample; 4],
}

impl Invaders {
    fn new() -> Invaders {
        let pal = [('X', 0xffffff)];
        let inv = [
            [Sprite::from_rows(&["...XX...", "..XXXX..", ".XXXXXX.", "XX.XX.XX", "XXXXXXXX", "..X..X..", ".X.XX.X.", "X.X..X.X"], &pal),
             Sprite::from_rows(&["...XX...", "..XXXX..", ".XXXXXX.", "XX.XX.XX", "XXXXXXXX", ".X.XX.X.", "X......X", ".X....X."], &pal)],
            [Sprite::from_rows(&["..X.....X..", "...X...X...", "..XXXXXXX..", ".XX.XXX.XX.", "XXXXXXXXXXX", "X.XXXXXXX.X", "X.X.....X.X", "...XX.XX..."], &pal),
             Sprite::from_rows(&["..X.....X..", "X..X...X..X", "X.XXXXXXX.X", "XXX.XXX.XXX", "XXXXXXXXXXX", ".XXXXXXXXX.", "..X.....X..", ".X.......X."], &pal)],
            [Sprite::from_rows(&["....XXXX....", ".XXXXXXXXXX.", "XXXXXXXXXXXX", "XXX..XX..XXX", "XXXXXXXXXXXX", "...XX..XX...", "..XX.XX.XX..", "XX........XX"], &pal),
             Sprite::from_rows(&["....XXXX....", ".XXXXXXXXXX.", "XXXXXXXXXXXX", "XXX..XX..XXX", "XXXXXXXXXXXX", "..XXX..XXX..", ".XX..XX..XX.", "..XX....XX.."], &pal)],
        ];
        let green = [('X', GREEN)];
        let player = Sprite::from_rows(&["......X......", ".....XXX.....", ".....XXX.....", ".XXXXXXXXXXX.", "XXXXXXXXXXXXX", "XXXXXXXXXXXXX", "XXXXXXXXXXXXX", "XXXXXXXXXXXXX"], &green);
        let ufo_s = Sprite::from_rows(&[".....XXXXXX.....", "...XXXXXXXXXX...", "..XXXXXXXXXXXX..", ".XX.XX.XX.XX.XX.", "XXXXXXXXXXXXXXXX", "...XXX....XXX...", "....X......X...."], &[('X', RED)]);
        let boom_s = Sprite::from_rows(&["....X...X....", ".X...X.X...X.", "..X.......X..", "...X.....X...", "XX.........XX", "...X.....X...", "..X..X.X..X..", ".X..X...X..X."], &pal);
        let shield = Sprite::from_rows(&["....XXXXXXXXXXXXXX....", "...XXXXXXXXXXXXXXXX...", "..XXXXXXXXXXXXXXXXXX..", ".XXXXXXXXXXXXXXXXXXXX.",
            "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXXXXXXXXXXXXXXXXX",
            "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXXXXXXXXXXXXXXXXX", "XXXXXXX........XXXXXXX", "XXXXXX..........XXXXXX",
            "XXXXX............XXXXX", "XXXXX............XXXXX"], &green);
        let cannon_boom = [
            Sprite::from_rows(&["....X....X...", ".X....X......", "...X....X..X.", "X....XX......", "..XXXXXXX.X..", ".XXXXXXXXXX..", "XXXXXXXXXXXXX", "XXXXXXXXXXXXX"], &green),
            Sprite::from_rows(&["X......X.....", "...X.....X...", ".....X.X....X", ".X..X....X...", "...XXXXXX..X.", "X.XXXXXXXX...", ".XXXXXXXXXXX.", "XXXXXXXXXXXXX"], &green),
        ];
        let mut audio = Audio::open();
        let s_ufo = Sample::sweep(Wave::Sine, 520.0, 980.0, 0.18, 0.25).then(&Sample::sweep(Wave::Sine, 980.0, 520.0, 0.18, 0.25));
        let _ = &mut audio;
        let mut g = Invaders {
            alive: [[true; COLS]; ROWS], fx: 24.0, fy: 48.0, dir: 1.0, step_timer: 0.0, frame: false,
            player_x: (W / 2 - 6) as f32, shot: None, bombs: Vec::new(), bomb_timer: 1.5, shields: Vec::new(),
            ufo: None, ufo_timer: 18.0, ufo_hit: None, booms: Vec::new(),
            score: 0, high: funkey::store::high_score(GAME), lives: 3, wave: 1, mode: Mode::Title, time: 0.0, beat: 0,
            rng: Rng::from_time(), audio,
            inv, player, ufo_s, boom_s, shield: shield.clone(), cannon_boom,
            s_shoot: Sample::sweep(Wave::Square, 900.0, 250.0, 0.1, 0.3),
            s_kill: Sample::noise(0.14, 0.5),
            s_die: Sample::noise(0.5, 0.6).then(&Sample::sweep(Wave::Saw, 200.0, 40.0, 0.4, 0.4)),
            s_ufo, s_ufo_hit: Sample::sweep(Wave::Square, 300.0, 1400.0, 0.3, 0.4),
            s_beat: [Sample::tone(Wave::Square, 110.0, 0.09, 0.35), Sample::tone(Wave::Square, 104.0, 0.09, 0.35),
                     Sample::tone(Wave::Square, 98.0, 0.09, 0.35), Sample::tone(Wave::Square, 92.0, 0.09, 0.35)],
        };
        g.reset_shields();
        g
    }

    fn reset_shields(&mut self) {
        self.shields = (0..4).map(|i| (28 + i * 46, self.shield.clone())).collect();
    }

    fn start_wave(&mut self, wave: u32) {
        self.wave = wave;
        self.alive = [[true; COLS]; ROWS];
        self.fx = 24.0;
        self.fy = 48.0 + ((wave - 1) as f32 * 8.0).min(56.0);
        self.dir = 1.0;
        self.step_timer = 0.0;
        self.shot = None;
        self.bombs.clear();
        self.ufo = None;
        self.ufo_timer = self.rng.range(15.0, 25.0);
        self.mode = Mode::Play;
    }

    fn start_game(&mut self) {
        self.score = 0;
        self.lives = 3;
        self.reset_shields();
        self.player_x = (W / 2 - 6) as f32;
        self.start_wave(1);
    }

    fn count(&self) -> usize { self.alive.iter().flatten().filter(|&&a| a).count() }

    fn kind_of(row: usize) -> usize { match row { 0 => 0, 1 | 2 => 1, _ => 2 } }
    fn points(row: usize) -> u32 { match row { 0 => 30, 1 | 2 => 20, _ => 10 } }

    /// Screen box of an invader: its sprite centred in a 16-wide cell.
    fn inv_box(&self, r: usize, c: usize) -> (i32, i32, i32, i32) {
        let s = &self.inv[Self::kind_of(r)][0];
        let x = self.fx as i32 + c as i32 * 16 + (16 - s.w) / 2;
        let y = self.fy as i32 + r as i32 * 16;
        (x, y, s.w, s.h)
    }

    fn step_formation(&mut self) {
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        for r in 0..ROWS { for c in 0..COLS { if self.alive[r][c] { let (x, _, w, _) = self.inv_box(r, c); min_x = min_x.min(x); max_x = max_x.max(x + w); } } }
        if (self.dir > 0.0 && max_x + 2 >= W - 4) || (self.dir < 0.0 && min_x - 2 <= 4) {
            self.dir = -self.dir;
            self.fy += 8.0;
        } else {
            self.fx += self.dir * 2.0;
        }
        self.frame = !self.frame;
        let b = self.s_beat[self.beat].clone();
        self.audio.play(&b, 1.0);
        self.beat = (self.beat + 1) % 4;
    }

    /// Erode a shield where something hits it. True when it did.
    fn hit_shield(&mut self, x: i32, y: i32, from_above: bool) -> bool {
        for (sx, s) in self.shields.iter_mut() {
            let (lx, ly) = (x - *sx, y - 192);
            if lx < 0 || ly < 0 || lx >= s.w || ly >= s.h { continue; }
            if s.px[(ly * s.w + lx) as usize] & 0xff00_0000 == 0 { continue; }
            let dy = if from_above { 1 } else { -1 };
            for oy in -2..=3 {
                for ox in -2..=2 {
                    let (px, py) = (lx + ox, ly + oy * dy);
                    if px < 0 || py < 0 || px >= s.w || py >= s.h { continue; }
                    if ox.abs() + oy.abs() > 3 && self.rng.chance(0.5) { continue; }
                    s.px[(py * s.w + px) as usize] = 0;
                }
            }
            return true;
        }
        false
    }

    fn play(&mut self, input: &Input, dt: f32) {
        // The cannon.
        let ax = input.axis_x();
        self.player_x = (self.player_x + ax as f32 * 80.0 * dt).clamp(4.0, (W - 17) as f32);
        if input.pressed(Key::Space) && self.shot.is_none() {
            self.shot = Some(Shot { x: self.player_x + 6.0, y: PLAYER_Y as f32 - 2.0, vy: -300.0, kind: 0 });
            self.audio.play(&self.s_shoot, 1.0);
        }
        // The march: faster as they fall.
        let n = self.count();
        let interval = 0.03 + 0.62 * (n as f32 / 55.0).powf(1.3);
        self.step_timer -= dt;
        if self.step_timer <= 0.0 { self.step_timer += interval; self.step_formation(); }
        // Bombs from the lowest invader of a random column.
        self.bomb_timer -= dt;
        if self.bomb_timer <= 0.0 && self.bombs.len() < 3 {
            self.bomb_timer = self.rng.range(0.4, 1.4) * (1.0 - self.wave as f32 * 0.05).max(0.4);
            let cols: Vec<usize> = (0..COLS).filter(|&c| (0..ROWS).any(|r| self.alive[r][c])).collect();
            if let Some(&c) = self.rng.pick(&cols) {
                let r = (0..ROWS).rev().find(|&r| self.alive[r][c]).unwrap_or(0);
                let (x, y, w, h) = self.inv_box(r, c);
                self.bombs.push(Shot { x: (x + w / 2) as f32, y: (y + h) as f32, vy: 90.0 + self.wave as f32 * 8.0, kind: 1 + self.rng.below(2) as u8 });
            }
        }
        // The mystery ship.
        self.ufo_timer -= dt;
        if self.ufo.is_none() && self.ufo_timer <= 0.0 && n > 7 {
            let from_left = self.rng.chance(0.5);
            let points = *self.rng.pick(&[50u32, 100, 150, 300]).unwrap_or(&100);
            self.ufo = Some((if from_left { -16.0 } else { W as f32 }, if from_left { 42.0 } else { -42.0 }, points));
            self.audio.play_loop(2, &self.s_ufo, 1.0);
        }
        if let Some((x, vx, _)) = &mut self.ufo {
            *x += *vx * dt;
            if *x < -20.0 || *x > W as f32 + 4.0 { self.ufo = None; self.audio.stop(2); self.ufo_timer = self.rng.range(15.0, 30.0); }
        }
        // The player's shot.
        if let Some(mut s) = self.shot.take() {
            s.y += s.vy * dt;
            let (sx, sy) = (s.x as i32, s.y as i32);
            let mut gone = sy < 20;
            if !gone && self.hit_shield(sx, sy, false) { gone = true; }
            if !gone {
                'hit: for r in 0..ROWS { for c in 0..COLS {
                    if !self.alive[r][c] { continue; }
                    let (x, y, w, h) = self.inv_box(r, c);
                    if sx >= x && sx < x + w && sy >= y && sy < y + h + 2 {
                        self.alive[r][c] = false;
                        self.score += Self::points(r);
                        self.booms.push(Boom { x: x + w / 2 - 6, y, t: 0.25 });
                        self.audio.play(&self.s_kill, 1.0);
                        gone = true;
                        break 'hit;
                    }
                } }
            }
            if !gone {
                if let Some((ux, _, pts)) = self.ufo {
                    if sy < 32 && sx >= ux as i32 && sx < ux as i32 + 16 {
                        self.score += pts;
                        self.ufo_hit = Some((ux as i32, pts, 1.2));
                        self.ufo = None;
                        self.audio.stop(2);
                        self.audio.play(&self.s_ufo_hit, 1.0);
                        self.ufo_timer = self.rng.range(15.0, 30.0);
                        gone = true;
                    }
                }
            }
            if !gone {
                if let Some(k) = self.bombs.iter().position(|b| (b.x - s.x).abs() < 3.0 && (b.y - s.y).abs() < 6.0) {
                    self.bombs.remove(k);
                    gone = true;
                }
            }
            if !gone { self.shot = Some(s); }
        }
        // Their bombs.
        let (px, py) = (self.player_x as i32, PLAYER_Y);
        let mut died = false;
        let mut keep = Vec::new();
        let bombs = std::mem::take(&mut self.bombs);
        for mut b in bombs {
            b.y += b.vy * dt;
            let (bx, by) = (b.x as i32, b.y as i32);
            if by >= GROUND { continue; }
            if self.hit_shield(bx, by, true) { continue; }
            if bx >= px && bx < px + 13 && by >= py && by < py + 8 { died = true; continue; }
            keep.push(b);
        }
        self.bombs = keep;
        // Have they landed?
        let lowest = (0..ROWS).rev().find(|&r| (0..COLS).any(|c| self.alive[r][c])).map(|r| self.fy as i32 + r as i32 * 16 + 8).unwrap_or(0);
        if lowest >= PLAYER_Y { died = true; self.lives = 1; }
        if died {
            self.mode = Mode::Dying(1.5);
            self.audio.stop(2);
            self.audio.play(&self.s_die, 1.0);
        } else if n == 0 {
            self.mode = Mode::Wave(2.0);
            self.audio.stop(2);
        }
    }
}

impl Game for Invaders {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        for b in &mut self.booms { b.t -= dt; }
        self.booms.retain(|b| b.t > 0.0);
        if let Some((_, _, t)) = &mut self.ufo_hit { *t -= dt; if *t <= 0.0 { self.ufo_hit = None; } }
        match self.mode {
            Mode::Title => { if input.pressed(Key::Space) || input.pressed(Key::Enter) { self.start_game(); } }
            Mode::Play => self.play(input, dt),
            Mode::Dying(t) => {
                if t - dt > 0.0 { self.mode = Mode::Dying(t - dt); }
                else {
                    self.lives -= 1;
                    self.bombs.clear();
                    self.shot = None;
                    if self.lives == 0 {
                        if funkey::store::record_score(GAME, self.score) { self.high = self.score; }
                        self.mode = Mode::Over(5.0);
                    } else { self.player_x = (W / 2 - 6) as f32; self.mode = Mode::Play; }
                }
            }
            Mode::Wave(t) => { if t - dt > 0.0 { self.mode = Mode::Wave(t - dt); } else { let w = self.wave + 1; self.start_wave(w); } }
            Mode::Over(t) => { if t - dt <= 0.0 || input.pressed(Key::Space) { self.mode = Mode::Title; } else { self.mode = Mode::Over(t - dt); } }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        f.clear(0x000000);
        f.text_big(8, 4, "SCORE", WHITE);
        f.text_big(8, 14, &format!("{:05}", self.score), WHITE);
        f.text_big(W - 8 - Frame::text_width("HI-SCORE", true, 1), 4, "HI-SCORE", WHITE);
        f.text_big(W - 8 - Frame::text_width("00000", true, 1), 14, &format!("{:05}", self.high.max(self.score)), WHITE);
        if self.mode == Mode::Title {
            f.text_centered(W / 2, 60, "INVADERS", WHITE, true, 3);
            let rows: [(usize, &str); 3] = [(0, "= 30 POINTS"), (1, "= 20 POINTS"), (2, "= 10 POINTS")];
            for (k, (kind, txt)) in rows.iter().enumerate() {
                let y = 110 + k as i32 * 18;
                f.blit(&self.inv[*kind][0], 60, y);
                f.text_big(80, y, txt, WHITE);
            }
            f.blit(&self.ufo_s, 58, 164);
            f.text_big(80, 164, "= ? MYSTERY", RED);
            if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 200, "PRESS SPACE", CYAN, true, 1); }
            f.text_centered(W / 2, 222, "LEFT RIGHT MOVE  SPACE FIRES", 0x8080a0, false, 1);
            f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 6, VERSION, 0x404060);
            return;
        }
        for r in 0..ROWS { for c in 0..COLS {
            if !self.alive[r][c] { continue; }
            let (x, y, _, _) = self.inv_box(r, c);
            f.blit(&self.inv[Self::kind_of(r)][self.frame as usize], x, y);
        } }
        for b in &self.booms { f.blit(&self.boom_s, b.x, b.y); }
        if let Some((x, _, _)) = self.ufo { f.blit(&self.ufo_s, x as i32, 24); }
        if let Some((x, pts, _)) = self.ufo_hit { f.text_big(x, 24, &format!("{}", pts), RED); }
        for (sx, s) in &self.shields { f.blit(s, *sx, 192); }
        if let Some(s) = &self.shot { f.vline(s.x as i32, s.y as i32, 4, WHITE); }
        for b in &self.bombs {
            let (x, y) = (b.x as i32, b.y as i32);
            let wiggle = ((self.time * 20.0) as i32 + y / 3) % 2;
            if b.kind == 1 { f.put(x + wiggle, y, WHITE); f.put(x + 1 - wiggle, y + 1, WHITE); f.put(x + wiggle, y + 2, WHITE); f.put(x + 1 - wiggle, y + 3, WHITE); }
            else { f.vline(x, y, 4, WHITE); f.hline(x - 1, y + 1 + wiggle, 3, WHITE); }
        }
        match self.mode {
            Mode::Dying(t) => f.blit(&self.cannon_boom[((t * 12.0) as usize) % 2], self.player_x as i32, PLAYER_Y),
            _ => f.blit(&self.player, self.player_x as i32, PLAYER_Y),
        }
        f.hline(0, GROUND, W, GREEN);
        f.text_big(8, GROUND + 4, &format!("{}", self.lives), WHITE);
        for k in 0..self.lives.saturating_sub(1) { f.blit(&self.player, 24 + k as i32 * 16, GROUND + 4); }
        f.text_big(W - 8 - Frame::text_width("WAVE 00", true, 1), GROUND + 4, &format!("WAVE {:2}", self.wave), WHITE);
        match self.mode {
            Mode::Wave(_) => f.text_centered(W / 2, 120, "WAVE CLEARED", CYAN, true, 2),
            Mode::Over(_) => { f.text_centered(W / 2, 110, "GAME OVER", RED, true, 2); if self.score >= self.high && self.score > 0 { f.text_centered(W / 2, 134, "NEW HI-SCORE", CYAN, true, 1); } }
            _ => {}
        }
    }
}

fn main() {
    run(&mut Invaders::new(), Config { width: W, height: H, fps: 60 });
}
