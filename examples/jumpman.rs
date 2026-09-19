//! jumpman: a Jumpman Junior kind of game. Girders, ladders, ropes,
//! bombs to collect, bullets to jump, robots to dodge, and a fall from
//! too high is the end of you.
//!
//!     cargo run --release --example jumpman
//!
//! Left and Right run, Space jumps, Up and Down climb, R restarts a life,
//! Q quits. Take every bomb to finish a level. Some bombs do things.

use funkey::*;

const W: i32 = 256;
const H: i32 = 192;
const TILE: i32 = 8;
const TOP: i32 = 16;
const GAME: &str = "jumpman";

struct Level { name: &'static str, rows: [&'static str; 22], bullets: bool }

const LEVELS: [Level; 5] = [
    Level { name: "EASY DOES IT", bullets: false, rows: [
        "                                ",
        "                                ",
        "    *        *        *         ",
        "  ##########H#########H####     ",
        "            H         H         ",
        "     *      H    *    H      *  ",
        "  ####H#####H#########H#######  ",
        "      H                         ",
        "      H     *          *   *    ",
        "  ####H#############H########## ",
        "                    H           ",
        "   *     *          H      *    ",
        "  #######H##########H#########  ",
        "         H                      ",
        "         H    *        *        ",
        "  #######H###########H########  ",
        "                     H          ",
        "    *                H     *    ",
        "  ###H###############H########  ",
        "     H                          ",
        "  P  H                     *    ",
        "################################",
    ]},
    Level { name: "ROPE TRICK", bullets: true, rows: [
        "                                ",
        "                                ",
        "     *   |            |   *     ",
        "  #######|###      ###|#######  ",
        "         |            |         ",
        "         |  *      *  |         ",
        "         |            |         ",
        "     ####|####    ####|####     ",
        "                                ",
        "  *          |    |          *  ",
        "  ###H#######|####|#######H###  ",
        "     H       |    |       H     ",
        "     H   *   |    |   *   H     ",
        "     H       |    |       H     ",
        "  ###H####   |    |   ####H###  ",
        "             |    |             ",
        "    *     ###|####|###     *    ",
        "  ########   |    |   ########  ",
        "             |    |             ",
        "       *     |    |     *       ",
        "  P    ######|####|######       ",
        "################################",
    ]},
    Level { name: "HIDDEN LADDERS", bullets: false, rows: [
        "                                ",
        "                                ",
        "  *     *      @      *     *   ",
        "  #########h#####h##########    ",
        "           h     h              ",
        "           h     h              ",
        "  *        h  *  h         *    ",
        "  ####h####h#####h######h####   ",
        "      h                 h       ",
        "      h                 h       ",
        "   *  h      *    *     h   *   ",
        "  ####h#########H#######h#####  ",
        "                H               ",
        "                H               ",
        "     *          H          *    ",
        "  ######H#######H######H######  ",
        "        H              H        ",
        "        H              H        ",
        "   *    H       *      H    *   ",
        "  ######H##############H######  ",
        "  P                             ",
        "################################",
    ]},
    Level { name: "VANISHING ACT", bullets: true, rows: [
        "                                ",
        "                                ",
        "     *     *   %   *     *      ",
        "  ###H####=====H=====#####H###  ",
        "     H                    H     ",
        "     H  *              *  H     ",
        "  ###H###            ######H##  ",
        "                                ",
        "   *  ======  *  *  ======  *   ",
        "  ####H#####================### ",
        "      H                         ",
        "      H  *      *      *        ",
        "  ####H########H#########H####  ",
        "               H         H      ",
        "               H         H      ",
        "    *          H    *    H  *   ",
        "  ######H######H#####H###H####  ",
        "        H            H          ",
        "        H            H          ",
        "  P  *  H     *      H   *      ",
        "  ######H############H########  ",
        "################################",
    ]},
    Level { name: "ROBOT RUN", bullets: true, rows: [
        "                                ",
        "                                ",
        "   *    r      *      r    *    ",
        "  #####H###########H#########   ",
        "       H           H            ",
        "       H  *     *  H            ",
        "  ##H##H###########H#####H###   ",
        "    H                    H      ",
        "    H   r    *      r    H      ",
        "  ##H####################H###   ",
        "                                ",
        "      *   |          |   *      ",
        "  ####H###|##########|###H####  ",
        "      H   |          |   H      ",
        "      H   |  *    *  |   H      ",
        "  ####H###|##########|###H####  ",
        "      H                  H      ",
        "      H    r    *   r    H      ",
        "  ####H##################H####  ",
        "                                ",
        "  P   *          *         *    ",
        "################################",
    ]},
];

const GIRDER: Rgb = 0xd04010;
const GIRDER_DARK: Rgb = 0x802008;
const LADDER: Rgb = 0xe8c040;
const ROPE: Rgb = 0xc89060;
const SKY: Rgb = 0x101020;
const TEXT: Rgb = 0xf0f0f0;
const GOLD: Rgb = 0xffd040;

#[derive(Clone, Copy, PartialEq)]
enum Mode { Title, Play, Dying(f32), Clear(f32), Over(f32) }

struct Bullet { x: f32, y: f32, dir: f32 }
struct Robot { body: Body, dir: f32 }

struct Jumpman {
    map: Tilemap,
    level: usize,
    player: Body,
    face_left: bool,
    climbing: bool,
    hanging: bool,
    fall_from: f32,
    start: (f32, f32),
    bombs_left: usize,
    bullets: Vec<Bullet>,
    bullet_timer: f32,
    robots: Vec<Robot>,
    score: u32,
    high: u32,
    bonus: f32,
    lives: u32,
    mode: Mode,
    time: f32,
    rng: Rng,
    particles: Particles,
    audio: Audio,
    s_jump: Sample, s_bomb: Sample, s_die: Sample, s_clear: Sample, s_bullet: Sample, s_reveal: Sample, s_title: Sample,
    hero: Vec<Sprite>, bomb: Sprite, robot: Sprite, girder: Sprite, ladder: Sprite, rope: Sprite, bullet: Sprite,
}

impl Jumpman {
    fn new() -> Jumpman {
        let pal = [('b', 0x3080f0), ('s', 0xffd7b0), ('d', 0x80d0ff), ('r', 0xe03030), ('w', 0xffffff), ('k', 0x101010), ('y', GOLD), ('o', 0xc06010), ('g', 0x90a0b0), ('e', 0x40ff40)];
        let hero = vec![
            Sprite::from_rows(&[".sss.", ".sks.", ".sss.", "rbbbr", ".bbb.", ".bbb.", ".d.d.", ".d.d.", ".d.d.", "dd.dd"], &pal),
            Sprite::from_rows(&[".sss.", ".sks.", ".sss.", "rbbbr", ".bbb.", ".bbb.", ".d.d.", "d...d", "d...d", "d...d"], &pal),
            Sprite::from_rows(&["r.s.r", "rsssr", ".sks.", ".sss.", ".bbb.", ".bbb.", ".bbb.", ".d.d.", ".d.d.", ".d.d."], &pal),
            Sprite::from_rows(&[".sss.", ".sks.", "rsssr", "rbbbr", ".bbb.", ".bbb.", "d...d", "d...d", ".....", "....."], &pal),
            Sprite::from_rows(&["..r..", "..r..", ".sss.", ".sks.", ".sss.", ".bbb.", ".bbb.", ".d.d.", ".d.d.", ".d.d."], &pal),
        ];
        let bomb = Sprite::from_rows(&["...w.", "..o..", ".ggg.", "gwggg", "ggggg", "ggggg", ".ggg."], &[('w', 0xffffff), ('o', 0xff9020), ('g', 0x8090c0)]);
        let robot = Sprite::from_rows(&[".gggg.", "gegeg.", "gggggg", ".gggg.", "g.gg.g", ".g..g."], &pal);
        let girder = Sprite::from_rows(&["GGGGGGGG", "GkGGGGkG", "GGGGGGGG", "DDDDDDDD", "GGGGGGGG", "GkGGGGkG", "GGGGGGGG", "DDDDDDDD"], &[('G', GIRDER), ('D', GIRDER_DARK), ('k', 0xff9060)]);
        let ladder = Sprite::from_rows(&["L......L", "L......L", "LLLLLLLL", "L......L", "L......L", "L......L", "LLLLLLLL", "L......L"], &[('L', LADDER)]);
        let rope = Sprite::from_rows(&["...R....", "....R...", "...R....", "....R...", "...R....", "....R...", "...R....", "....R..."], &[('R', ROPE)]);
        let bullet = Sprite::from_rows(&["wyyw", "yyyy"], &pal);
        let mut audio = Audio::open();
        let s_title = Tune::parse("150 c4 e4 g4 e4 a4 g4 e4 c4 d4 f4 a4 f4 g4/2 -/4 e4 g4 c5 g4 a4 c5 e5 c5 d5 b4 g4 f4 e4/2 -/4", Wave::Square, 0.35).render();
        let s_clear = Tune::parse("200 c5 e5 g5 c6/2 g5/8 c6/2", Wave::Triangle, 0.5).render();
        let mut j = Jumpman {
            map: Tilemap::from_rows(&LEVELS[0].rows, TILE), level: 0, player: Body::new(0.0, 0.0, 5.0, 10.0),
            face_left: false, climbing: false, hanging: false, fall_from: 0.0, start: (0.0, 0.0), bombs_left: 0,
            bullets: Vec::new(), bullet_timer: 3.0, robots: Vec::new(),
            score: 0, high: funkey::store::high_score(GAME), bonus: 0.0, lives: 4, mode: Mode::Title, time: 0.0,
            rng: Rng::from_time(), particles: Particles::new(),
            s_jump: Sample::sweep(Wave::Square, 260.0, 620.0, 0.12, 0.35),
            s_bomb: Sample::tone(Wave::Square, 880.0, 0.05, 0.4).then(&Sample::tone(Wave::Square, 1320.0, 0.09, 0.4)),
            s_die: Sample::sweep(Wave::Saw, 420.0, 60.0, 0.6, 0.45).then(&Sample::noise(0.25, 0.4)),
            s_clear, s_bullet: Sample::noise(0.06, 0.12), s_reveal: Sample::sweep(Wave::Triangle, 300.0, 1200.0, 0.4, 0.4),
            s_title, hero, bomb, robot, girder, ladder, rope, bullet,
            audio: Audio::off(),
        };
        audio.play_loop(1, &j.s_title, 1.0);
        j.audio = audio;
        j.load(0);
        j
    }

    fn load(&mut self, level: usize) {
        self.level = level % LEVELS.len();
        let l = &LEVELS[self.level];
        self.map = Tilemap::from_rows(&l.rows, TILE);
        self.map.one_way = vec!['H'];
        let (sx, sy) = self.map.find('P').first().map(|&(x, y)| (x as f32 * 8.0 + 1.0, y as f32 * 8.0 - 2.0)).unwrap_or((8.0, 8.0));
        for (x, y) in self.map.find('P') { self.map.set(x, y, ' '); }
        self.robots = self.map.find('r').iter().map(|&(x, y)| Robot { body: Body::new(x as f32 * 8.0 + 1.0, y as f32 * 8.0 + 2.0, 6.0, 6.0), dir: 1.0 }).collect();
        for (x, y) in self.map.find('r') { self.map.set(x, y, ' '); }
        self.bombs_left = self.map.find('*').len() + self.map.find('@').len() + self.map.find('%').len();
        self.start = (sx, sy);
        self.bullets.clear();
        self.bullet_timer = 3.0;
        self.bonus = 1000.0;
        self.respawn();
    }

    fn respawn(&mut self) {
        self.player = Body::new(self.start.0, self.start.1, 5.0, 10.0);
        self.climbing = false;
        self.hanging = false;
        self.fall_from = self.player.y;
        self.face_left = false;
    }

    fn tiles_under(&self, c: char) -> bool {
        let p = &self.player;
        self.map.under(p.x + 1.0, p.y, p.w - 2.0, p.h).iter().any(|&(_, _, t)| t == c)
    }

    fn tile_below(&self, c: char) -> bool {
        let p = &self.player;
        self.map.under(p.x + 1.0, p.y + p.h, p.w - 2.0, 1.0).iter().any(|&(_, _, t)| t == c)
    }

    fn die(&mut self) {
        if !matches!(self.mode, Mode::Play) { return; }
        self.mode = Mode::Dying(1.2);
        let (x, y) = (self.player.x + 2.0, self.player.y + 5.0);
        self.particles.burst(x, y, 24, 70.0, 0.9, 0xe03030);
        self.particles.burst(x, y, 12, 50.0, 0.9, 0xffd7b0);
        self.audio.play(&self.s_die, 1.0);
    }

    fn start_game(&mut self) {
        self.score = 0;
        self.lives = 4;
        self.load(0);
        self.mode = Mode::Play;
        self.audio.stop(1);
    }

    fn take_bomb(&mut self, tx: i32, ty: i32, kind: char) {
        self.map.set(tx, ty, ' ');
        self.score += 100;
        self.bombs_left -= 1;
        self.particles.burst(tx as f32 * 8.0 + 4.0, ty as f32 * 8.0 + 4.0, 10, 40.0, 0.5, GOLD);
        self.audio.play(&self.s_bomb, 1.0);
        match kind {
            '@' => { for (x, y) in self.map.find('h') { self.map.set(x, y, 'H'); } self.audio.play(&self.s_reveal, 1.0); }
            '%' => { for (x, y) in self.map.find('=') { self.map.set(x, y, ' '); } self.audio.play(&self.s_reveal, 1.0); }
            _ => {}
        }
        if self.bombs_left == 0 {
            self.score += self.bonus as u32;
            self.mode = Mode::Clear(2.5);
            self.audio.play(&self.s_clear, 1.0);
        }
    }

    fn play(&mut self, input: &Input, dt: f32) {
        self.bonus = (self.bonus - 10.0 * dt).max(0.0);
        let ax = input.axis_x();
        let ay = input.axis_y();
        let jump = input.pressed(Key::Space);

        let on_ladder = self.tiles_under('H');
        let ladder_below = self.tile_below('H');
        let on_rope = self.tiles_under('|');
        if !self.climbing && !self.hanging && ay != 0 && ((ay < 0 && on_ladder) || (ay > 0 && (ladder_below || on_ladder))) { self.climbing = true; }
        if !self.climbing && !self.hanging && ay != 0 && on_rope && !self.player.on_ground { self.hanging = true; }
        if !self.climbing && !self.hanging && ay < 0 && on_rope { self.hanging = true; }

        let p = &mut self.player;
        if self.climbing {
            if !on_ladder && !ladder_below { self.climbing = false; }
            let col = ((p.x + p.w / 2.0) / 8.0).floor() * 8.0;
            p.x += ((col + (8.0 - p.w) / 2.0) - p.x) * 0.5;
            p.vx = 0.0;
            p.vy = ay as f32 * 32.0;
            if ay == 0 && ax != 0 && p.on_ground { self.climbing = false; }
            p.step_through(&self.map, dt, ay > 0);
            let grounded = p.on_ground;
            if grounded && ay >= 0 && !self.tile_below('H') && !self.tiles_under('H') { self.climbing = false; }
            self.fall_from = self.player.y;
        } else if self.hanging {
            let col = ((p.x + p.w / 2.0) / 8.0).floor() * 8.0;
            p.x += ((col + (8.0 - p.w) / 2.0) - p.x) * 0.5;
            p.vx = 0.0;
            p.vy = ay as f32 * 28.0;
            if ax != 0 || jump {
                // Let go: a small hop off the rope.
                self.hanging = false;
                p.vx = ax as f32 * 44.0;
                p.vy = -60.0;
                if jump { p.vy = -95.0; }
                self.audio.play(&self.s_jump, 0.7);
            } else {
                p.step_through(&self.map, dt, true);
                if !self.tiles_under('|') { self.hanging = false; self.player.vy = 0.0; }
            }
            self.fall_from = self.player.y;
        } else {
            if p.on_ground {
                p.vx = ax as f32 * 44.0;
                self.fall_from = p.y;
            } else {
                // A little steering in the air.
                p.vx += ax as f32 * 60.0 * dt;
                p.vx = p.vx.clamp(-50.0, 50.0);
            }
            p.vy = (p.vy + 320.0 * dt).min(200.0);
            if jump && p.on_ground { p.vy = -108.0; self.audio.play(&self.s_jump, 1.0); }
            let was_ground = p.on_ground;
            p.step(&self.map, dt);
            if p.vy < 0.0 || !was_ground && p.y < self.fall_from { self.fall_from = self.fall_from.min(p.y); }
            if p.on_ground && !was_ground {
                // Landed: how far did we fall?
                if p.y - self.fall_from > 34.0 { self.die(); return; }
                self.fall_from = p.y;
            }
            if p.y > (H - TOP) as f32 + 8.0 { self.die(); return; }
        }
        if ax != 0 { self.face_left = ax < 0; }

        // Bombs.
        let p = &self.player;
        let hits: Vec<(i32, i32, char)> = self.map.under(p.x, p.y, p.w, p.h).into_iter().filter(|&(_, _, c)| c == '*' || c == '@' || c == '%').collect();
        for (x, y, c) in hits { self.take_bomb(x, y, c); }

        // Bullets: from the sides at a girder's height, now and then.
        if LEVELS[self.level].bullets {
            self.bullet_timer -= dt;
            if self.bullet_timer <= 0.0 {
                self.bullet_timer = self.rng.range(2.5, 4.5);
                let rows: Vec<i32> = (1..self.map.h - 1).filter(|&r| (0..self.map.w).any(|c| self.map.at(c, r) == ' ' && self.map.is_solid(c, r + 1))).collect();
                if let Some(&row) = self.rng.pick(&rows) {
                    let from_left = self.rng.chance(0.5);
                    self.bullets.push(Bullet { x: if from_left { -6.0 } else { W as f32 + 2.0 }, y: row as f32 * 8.0 + 4.0, dir: if from_left { 1.0 } else { -1.0 } });
                    self.audio.play(&self.s_bullet, 1.0);
                }
            }
        }
        for b in &mut self.bullets { b.x += b.dir * 38.0 * dt; }
        self.bullets.retain(|b| b.x > -10.0 && b.x < W as f32 + 10.0);
        let p = &self.player;
        if self.bullets.iter().any(|b| b.x < p.x + p.w && b.x + 4.0 > p.x && b.y < p.y + p.h && b.y + 2.0 > p.y) { self.die(); return; }

        // Robots patrol and turn at edges and walls.
        for r in &mut self.robots {
            r.body.vx = r.dir * 22.0;
            r.body.vy = 60.0;
            r.body.step(&self.map, dt);
            let ahead = if r.dir > 0.0 { r.body.x + r.body.w + 1.0 } else { r.body.x - 1.0 };
            let edge = !self.map.blocked(ahead, r.body.y + r.body.h + 1.0, 1.0, 1.0)
                && self.map.lands_on_one_way(ahead, 1.0, r.body.y + r.body.h, r.body.y + r.body.h + 1.0).is_none();
            if r.body.hit_wall || edge { r.dir = -r.dir; }
        }
        if self.robots.iter().any(|r| r.body.overlaps(&self.player)) { self.die(); }
    }
}

impl Game for Jumpman {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        self.particles.update(dt);
        match self.mode {
            Mode::Title => { if input.pressed(Key::Space) || input.pressed(Key::Enter) { self.start_game(); } }
            Mode::Play => {
                if input.pressed(Key::Char('r')) { self.die(); } else { self.play(input, dt); }
            }
            Mode::Dying(t) => {
                let t = t - dt;
                if t > 0.0 { self.mode = Mode::Dying(t); }
                else {
                    self.lives -= 1;
                    if self.lives == 0 {
                        if funkey::store::record_score(GAME, self.score) { self.high = self.score; }
                        self.mode = Mode::Over(4.0);
                    } else { self.respawn(); self.mode = Mode::Play; }
                }
            }
            Mode::Clear(t) => {
                let t = t - dt;
                if t > 0.0 { self.mode = Mode::Clear(t); } else { let next = self.level + 1; self.load(next); self.mode = Mode::Play; }
            }
            Mode::Over(t) => {
                let t = t - dt;
                if t <= 0.0 || input.pressed(Key::Space) { self.mode = Mode::Title; self.audio.play_loop(1, &self.s_title, 1.0); } else { self.mode = Mode::Over(t); }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        f.clear(SKY);
        if self.mode == Mode::Title {
            let bob = ((self.time * 2.0).sin() * 3.0) as i32;
            f.text_centered(W / 2, 30 + bob, "JUMPMAN", GOLD, true, 4);
            f.text_centered(W / 2, 66, "JUNIOR", GIRDER, true, 2);
            f.blit_scaled(&self.hero[0], W / 2 - 10, 92, 4, false);
            f.text_centered(W / 2, 140, &format!("HIGH SCORE {:06}", self.high), TEXT, true, 1);
            if (self.time * 2.0) as i32 % 2 == 0 { f.text_centered(W / 2, 160, "PRESS SPACE", TEXT, true, 1); }
            f.text_centered(W / 2, 178, "ARROWS RUN AND CLIMB  SPACE JUMPS", 0x8080a0, false, 1);
            return;
        }
        // The level, below the HUD.
        let cam_y = -TOP;
        let tiles = [('#', &self.girder), ('=', &self.girder), ('H', &self.ladder), ('|', &self.rope)];
        self.map.draw(f, 0, cam_y, &tiles);
        for (x, y) in self.map.find('*').into_iter().chain(self.map.find('@')).chain(self.map.find('%')) {
            f.blit(&self.bomb, x * 8 + 1, y * 8 + 1 - cam_y);
        }
        for r in &self.robots { f.blit_flip(&self.robot, r.body.x as i32 - 1, r.body.y as i32 - cam_y, r.dir < 0.0); }
        for b in &self.bullets { f.blit(&self.bullet, b.x as i32, b.y as i32 - cam_y); }
        if !matches!(self.mode, Mode::Dying(_)) {
            let p = &self.player;
            let frame = if self.climbing { if (p.y / 6.0) as i32 % 2 == 0 { 2 } else { 4 } }
                else if self.hanging { 4 }
                else if !p.on_ground { 3 }
                else if p.vx != 0.0 && (self.time * 10.0) as i32 % 2 == 0 { 1 } else { 0 };
            f.blit_flip(&self.hero[frame], p.x as i32, p.y as i32 - cam_y, self.face_left);
        }
        self.particles.draw(f, 0, cam_y);
        // HUD.
        f.rect(0, 0, W, TOP, 0x000000);
        f.text_big(2, 1, &format!("SCORE {:06}", self.score), TEXT);
        f.text_big(W - 2 - Frame::text_width("LIVES 0", true, 1), 1, &format!("LIVES {}", self.lives.saturating_sub(1)), GOLD);
        f.text_big(2, 9, &format!("BONUS {:04}", self.bonus as u32), TEXT);
        let name = LEVELS[self.level].name;
        f.text_big(W - 2 - Frame::text_width(name, true, 1), 9, name, GIRDER);
        match self.mode {
            Mode::Clear(_) => { f.dim(0.6); f.text_centered(W / 2, 80, "LEVEL CLEAR", GOLD, true, 2); f.text_centered(W / 2, 100, &format!("BONUS {}", self.bonus as u32), TEXT, true, 1); }
            Mode::Over(_) => { f.dim(0.5); f.text_centered(W / 2, 80, "GAME OVER", 0xe03030, true, 2); if self.score >= self.high && self.score > 0 { f.text_centered(W / 2, 100, "NEW HIGH SCORE", GOLD, true, 1); } }
            _ => {}
        }
    }
}

fn main() {
    run(&mut Jumpman::new(), Config { width: W, height: H, fps: 60 });
}
