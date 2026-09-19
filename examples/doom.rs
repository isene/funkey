//! doom: walk through a Doom level on funkey's sector renderer.
//!
//!     cargo run --release --example doom [wad] [map]
//!     cargo run --release --example doom -- --shot out.ppm [wad] [map]
//!
//! The WAD defaults to ~/.funkey/freedoom1.wad and the map to its first.
//! Arrows turn and walk, A and D sidestep, W and S walk, Space or E
//! opens a door, Q quits. `--shot` draws one frame to a file and exits.

use funkey::doom::{Camera, Renderer};
use funkey::wad::{Art, Level, Wad, ML_BLOCKING, ML_TWOSIDED};
use funkey::*;
use std::path::PathBuf;

const RADIUS: f32 = 16.0;
const STEP: f32 = 24.0;
const HEADROOM: f32 = 56.0;

struct Door { sector: usize, target: f32, closed: f32, wait: f32, opening: bool, stays: bool }

struct Doom {
    level: Level,
    art: Art,
    renderer: Renderer,
    cam: Camera,
    doors: Vec<Door>,
    message: String,
    message_for: f32,
}

impl Doom {
    fn new(wad_path: &PathBuf, map: Option<String>, w: i32, h: i32) -> Result<Doom, String> {
        let wad = Wad::open(wad_path)?;
        let map = map.unwrap_or_else(|| {
            wad.lumps.iter().map(|l| l.name.as_str()).find(|n| is_map_name(n)).unwrap_or("E1M1").to_string()
        });
        let art = Art::load(&wad);
        let level = Level::load(&wad, &map)?;
        let start = level.player_start().ok_or("no player start")?;
        let cam = Camera::standing(&level, start.x, start.y, start.angle);
        Ok(Doom { level, art, renderer: Renderer::new(w, h), cam, doors: Vec::new(), message: format!("{} {}", map, wad_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()), message_for: 3.0 })
    }

    /// Move the camera by (dx, dy) in world units, sliding along walls.
    fn walk(&mut self, dx: f32, dy: f32) {
        let (mut nx, mut ny) = (self.cam.x + dx, self.cam.y + dy);
        for _ in 0..3 {
            let mut pushed = false;
            for line in &self.level.linedefs {
                let (x1, y1) = self.level.vertexes[line.v1];
                let (x2, y2) = self.level.vertexes[line.v2];
                // Distance from the new position to the line segment.
                let (ex, ey) = (x2 - x1, y2 - y1);
                let len2 = ex * ex + ey * ey;
                if len2 == 0.0 { continue; }
                let t = (((nx - x1) * ex + (ny - y1) * ey) / len2).clamp(0.0, 1.0);
                let (px, py) = (x1 + ex * t, y1 + ey * t);
                let (ox, oy) = (nx - px, ny - py);
                let d2 = ox * ox + oy * oy;
                if d2 >= RADIUS * RADIUS || d2 == 0.0 { continue; }
                if !self.blocks(line) { continue; }
                let d = d2.sqrt();
                nx += ox / d * (RADIUS - d);
                ny += oy / d * (RADIUS - d);
                pushed = true;
            }
            if !pushed { break; }
        }
        self.cam.x = nx;
        self.cam.y = ny;
        let floor = self.level.sectors[self.level.sector_at(nx, ny)].floor;
        let target = floor + 41.0;
        self.cam.z += (target - self.cam.z) * 0.35;
    }

    fn blocks(&self, line: &funkey::wad::Linedef) -> bool {
        if line.flags & ML_TWOSIDED == 0 || line.flags & ML_BLOCKING != 0 { return true; }
        let (Some(f), Some(b)) = (line.front, line.back) else { return true };
        let fs = &self.level.sectors[self.level.sidedefs[f].sector];
        let bs = &self.level.sectors[self.level.sidedefs[b].sector];
        let here = self.level.sectors[self.level.sector_at(self.cam.x, self.cam.y)].floor;
        let floor = fs.floor.max(bs.floor);
        let ceiling = fs.ceiling.min(bs.ceiling);
        floor - here > STEP || ceiling - floor < HEADROOM
    }

    /// Space: the first door line in front, within reach, starts opening.
    fn use_line(&mut self) {
        let (fx, fy) = (self.cam.angle.cos(), self.cam.angle.sin());
        let (ax, ay) = (self.cam.x, self.cam.y);
        let (bx, by) = (ax + fx * 64.0, ay + fy * 64.0);
        let mut best: Option<(f32, usize)> = None;
        for (i, line) in self.level.linedefs.iter().enumerate() {
            if !is_door(line.special) { continue; }
            let (x1, y1) = self.level.vertexes[line.v1];
            let (x2, y2) = self.level.vertexes[line.v2];
            if let Some(t) = segments_cross(ax, ay, bx, by, x1, y1, x2, y2) {
                if best.map(|(bt, _)| t < bt).unwrap_or(true) { best = Some((t, i)); }
            }
        }
        let Some((_, i)) = best else { return };
        let line = &self.level.linedefs[i];
        let Some(back) = line.back else { return };
        let sector = self.level.sidedefs[back].sector;
        if self.doors.iter().any(|d| d.sector == sector) { return; }
        // The door opens to just under the lowest neighbouring ceiling.
        let mut top = f32::MAX;
        for l in &self.level.linedefs {
            let (Some(f), Some(b)) = (l.front, l.back) else { continue };
            let (fsec, bsec) = (self.level.sidedefs[f].sector, self.level.sidedefs[b].sector);
            if fsec == sector && bsec != sector { top = top.min(self.level.sectors[bsec].ceiling); }
            if bsec == sector && fsec != sector { top = top.min(self.level.sectors[fsec].ceiling); }
        }
        if top == f32::MAX { return; }
        let closed = self.level.sectors[sector].ceiling;
        self.doors.push(Door { sector, target: top - 4.0, closed, wait: 4.0, opening: true, stays: matches!(line.special, 31 | 32 | 33 | 34 | 118) });
    }

    fn run_doors(&mut self, dt: f32) {
        let speed = 140.0 * dt;
        let mut done = Vec::new();
        for (i, d) in self.doors.iter_mut().enumerate() {
            let s = &mut self.level.sectors[d.sector];
            if d.opening {
                s.ceiling = (s.ceiling + speed).min(d.target);
                if s.ceiling >= d.target {
                    if d.stays { done.push(i); } else { d.wait -= dt; if d.wait <= 0.0 { d.opening = false; } }
                }
            } else {
                s.ceiling = (s.ceiling - speed).max(d.closed);
                if s.ceiling <= d.closed { done.push(i); }
            }
        }
        for i in done.into_iter().rev() { self.doors.remove(i); }
    }
}

impl Game for Doom {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        // A tap is one step; a key that keeps repeating, or is held where
        // the terminal reports releases, moves continuously.
        let tap_turn = 10f32.to_radians();
        let turn = 2.4 * dt;
        if input.tapped(Key::Left) { self.cam.angle += tap_turn; }
        if input.tapped(Key::Right) { self.cam.angle -= tap_turn; }
        if input.motion(Key::Left) { self.cam.angle += turn; }
        if input.motion(Key::Right) { self.cam.angle -= turn; }
        let (fx, fy) = (self.cam.angle.cos(), self.cam.angle.sin());
        let (mut dx, mut dy) = (0.0, 0.0);
        let mut go = |key: Key, ax: f32, ay: f32| {
            if input.tapped(key) { dx += ax * 24.0; dy += ay * 24.0; }
            if input.motion(key) { dx += ax * 260.0 * dt; dy += ay * 260.0 * dt; }
        };
        go(Key::Up, fx, fy); go(Key::Char('w'), fx, fy);
        go(Key::Down, -fx, -fy); go(Key::Char('s'), -fx, -fy);
        go(Key::Char('a'), -fy, fx);
        go(Key::Char('d'), fy, -fx);
        if dx != 0.0 || dy != 0.0 { self.walk(dx, dy); }
        if input.pressed(Key::Space) || input.pressed(Key::Char('e')) { self.use_line(); }
        self.run_doors(dt);
        self.message_for = (self.message_for - dt).max(0.0);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        self.renderer.render(&self.level, &self.art, &self.cam, f);
        if self.message_for > 0.0 { f.text(2, 2, &self.message, WHITE); }
    }
}

fn is_map_name(n: &str) -> bool {
    let b = n.as_bytes();
    (b.len() == 4 && b[0] == b'E' && b[1].is_ascii_digit() && b[2] == b'M' && b[3].is_ascii_digit())
        || (b.len() == 5 && n.starts_with("MAP") && b[3].is_ascii_digit() && b[4].is_ascii_digit())
}

fn is_door(special: u16) -> bool {
    matches!(special, 1 | 26 | 27 | 28 | 31 | 32 | 33 | 34 | 117 | 118)
}

/// Where segment a-b crosses c-d, as a fraction along a-b.
fn segments_cross(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32, dx: f32, dy: f32) -> Option<f32> {
    let (r_x, r_y) = (bx - ax, by - ay);
    let (s_x, s_y) = (dx - cx, dy - cy);
    let den = r_x * s_y - r_y * s_x;
    if den.abs() < 1e-6 { return None; }
    let t = ((cx - ax) * s_y - (cy - ay) * s_x) / den;
    let u = ((cx - ax) * r_y - (cy - ay) * r_x) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) { Some(t) } else { None }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut shot: Option<PathBuf> = None;
    if let Some(i) = args.iter().position(|a| a == "--shot") {
        args.remove(i);
        if i < args.len() { shot = Some(PathBuf::from(args.remove(i))); }
    }
    let wad = args.first().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".funkey/freedoom1.wad")
    });
    let map = args.get(1).cloned();
    let (w, h) = (320, 240);
    let mut game = match Doom::new(&wad, map, w, h) {
        Ok(g) => g,
        Err(e) => { eprintln!("doom: {}", e); std::process::exit(1); }
    };
    if std::env::var_os("DOOM_INFO").is_some() {
        let l = &game.level;
        let sec = &l.sectors[l.sector_at(game.cam.x, game.cam.y)];
        eprintln!("start {:.0},{:.0} angle {:.2} sector floor {} ceiling {} flats {}/{} light {}",
            game.cam.x, game.cam.y, game.cam.angle, sec.floor, sec.ceiling, sec.floor_flat, sec.ceiling_flat, sec.light);
        eprintln!("textures {} flats {} sprites {} colormaps {} sky {}", game.art.textures.len(), game.art.flats.len(),
            game.art.sprites.len(), game.art.colormaps.len(), game.art.textures.contains_key("SKY1"));
        eprintln!("vertexes {} linedefs {} sectors {} segs {} subsectors {} nodes {} things {}",
            l.vertexes.len(), l.linedefs.len(), l.sectors.len(), l.segs.len(), l.subsectors.len(), l.nodes.len(), l.things.len());
        let missing: Vec<&str> = l.sectors.iter().map(|s| s.floor_flat.as_str()).filter(|f| !game.art.flats.contains_key(*f)).take(5).collect();
        eprintln!("missing flats: {:?}", missing);
        if let Ok(a) = std::env::var("DOOM_ANGLE") { game.cam.angle = a.parse::<f32>().unwrap_or(0.0).to_radians(); }
    }
    if std::env::var_os("DOOM_BENCH").is_some() {
        let mut frame = Frame::new(w, h);
        let t0 = std::time::Instant::now();
        for i in 0..200 { game.cam.angle += 0.02; if i % 50 == 0 { game.cam.x += 8.0; } game.draw(&mut frame); }
        eprintln!("{:.2} ms a frame at {}x{}", t0.elapsed().as_secs_f64() * 1000.0 / 200.0, w, h);
        return;
    }
    if let Some(path) = shot {
        let mut frame = Frame::new(w, h);
        game.draw(&mut frame);
        if let Err(e) = std::fs::write(&path, frame.to_ppm()) { eprintln!("doom: {}: {}", path.display(), e); }
        return;
    }
    run(&mut game, Config { width: w, height: h, fps: 60 });
}
