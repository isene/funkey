//! soar: a flight over fractal mountains. A height map ray-cast a column
//! at a time, the way Comanche did it in 1992, with the sun, shadows,
//! fog, water and clouds baked and blended on top. A demo of what the
//! terminal can look like with real pixels; there is nothing to win.
//!
//!     cargo run --release --example soar
//!
//! Left and Right turn, Up and Down climb and dive, W and S change speed,
//! Space hands the controls back to the autopilot, Q quits.
//! `SOAR_BENCH=<frames>` times the flight with no terminal.

use funkey::*;

const W: i32 = 640;
const H: i32 = 400;
const MAP: usize = 1024;
const MASK: usize = MAP - 1;
const WATER: f32 = 0.34;
const HSCALE: f32 = 260.0;
/// The full map and four coarser copies of it.
const LEVELS: usize = 5;
/// Where the sun stands, as a heading.
const SUN_AZ: f32 = 0.66;
const ZENITH: Rgb = 0x3568b8;
const HAZE: Rgb = 0xe6d2b4;
/// The game's own version; the engine has its own.
const VERSION: &str = "1.1";

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ seed.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13; h = h.wrapping_mul(0x5bd1e995); h ^= h >> 15;
    (h & 0xffffff) as f32 / 16777216.0
}

fn smooth(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

/// Value noise on a lattice that repeats every `period` cells.
fn noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (smooth(x - xi as f32), smooth(y - yi as f32));
    let g = |ix: i32, iy: i32| hash(ix.rem_euclid(period), iy.rem_euclid(period), seed);
    let a = g(xi, yi) + (g(xi + 1, yi) - g(xi, yi)) * fx;
    let b = g(xi, yi + 1) + (g(xi + 1, yi + 1) - g(xi, yi + 1)) * fx;
    a + (b - a) * fy
}

/// Layers of noise, each twice as fine and half as tall.
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        let period = (8 << o).max(1);
        sum += noise(x * freq, y * freq, period, seed + o) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Ridges: the absolute value of a noise folded over, for mountains.
fn ridged(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        let period = (6 << o).max(1);
        let n = 1.0 - (noise(x * freq, y * freq, period, seed + o) * 2.0 - 1.0).abs();
        sum += n * n * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb { funkey::raster::blend(a, b, t.clamp(0.0, 1.0)) }
fn scale(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    rgb((r as f32 * k).min(255.0) as u8, (g as f32 * k).min(255.0) as u8, (b as f32 * k).min(255.0) as u8)
}

/// The map at one level of detail. Each level up is half as fine, every
/// cell the average of four below it, so far ground is read from a map
/// as coarse as the steps the rays take there.
struct Level { size: usize, height: Vec<f32>, color: Vec<Rgb> }

impl Level {
    fn halve(&self) -> Level {
        let (s, n) = (self.size, self.size / 2);
        let (mut height, mut color) = (Vec::with_capacity(n * n), Vec::with_capacity(n * n));
        for y in 0..n {
            for x in 0..n {
                let i = [2 * y * s + 2 * x, 2 * y * s + 2 * x + 1, (2 * y + 1) * s + 2 * x, (2 * y + 1) * s + 2 * x + 1];
                height.push(i.iter().map(|&j| self.height[j]).sum::<f32>() / 4.0);
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for &j in &i { let (cr, cg, cb) = parts(self.color[j]); r += cr as u32; g += cg as u32; b += cb as u32; }
                color.push(rgb((r / 4) as u8, (g / 4) as u8, (b / 4) as u8));
            }
        }
        Level { size: n, height, color }
    }

    /// The four cells around a point of the full map, and the weights
    /// between them.
    #[inline]
    fn cell(&self, x: f32, y: f32) -> ([usize; 4], f32, f32) {
        let k = self.size as f32 / MAP as f32;
        let (x, y) = ((x + 0.5) * k - 0.5, (y + 0.5) * k - 0.5);
        let (xi, yi) = (x.floor(), y.floor());
        let m = self.size - 1;
        let (x0, y0) = (xi as i64 as usize & m, yi as i64 as usize & m);
        let (x1, y1) = ((x0 + 1) & m, (y0 + 1) & m);
        let s = self.size;
        ([y0 * s + x0, y0 * s + x1, y1 * s + x0, y1 * s + x1], x - xi, y - yi)
    }

    #[inline]
    fn height_at(&self, x: f32, y: f32) -> f32 {
        let (i, fx, fy) = self.cell(x, y);
        let a = self.height[i[0]] + (self.height[i[1]] - self.height[i[0]]) * fx;
        let b = self.height[i[2]] + (self.height[i[3]] - self.height[i[2]]) * fx;
        a + (b - a) * fy
    }

    #[inline]
    fn color_at(&self, x: f32, y: f32) -> Rgb {
        let (i, fx, fy) = self.cell(x, y);
        lerp(lerp(self.color[i[0]], self.color[i[1]], fx), lerp(self.color[i[2]], self.color[i[3]], fx), fy)
    }
}

struct Terrain { levels: Vec<Level>, clouds: Vec<f32> }

const CLOUD: usize = 256;

/// Bilinear read of a tileable square texture.
fn tex(t: &[f32], size: usize, x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let m = size - 1;
    let (x0, y0) = (xi as i64 as usize & m, yi as i64 as usize & m);
    let (x1, y1) = ((x0 + 1) & m, (y0 + 1) & m);
    let a = t[y0 * size + x0] + (t[y0 * size + x1] - t[y0 * size + x0]) * fx;
    let b = t[y1 * size + x0] + (t[y1 * size + x1] - t[y1 * size + x0]) * fx;
    a + (b - a) * fy
}

impl Terrain {
    fn generate() -> Terrain {
        let n = MAP * MAP;
        let mut height = vec![0.0f32; n];
        let s = 1.0 / MAP as f32 * 8.0;
        for y in 0..MAP {
            for x in 0..MAP {
                let (fx, fy) = (x as f32 * s, y as f32 * s);
                let base = fbm(fx, fy, 7, 11);
                let mountains = ridged(fx * 0.75, fy * 0.75, 6, 29);
                let mask = smooth((fbm(fx * 0.5, fy * 0.5, 3, 41) - 0.35).clamp(0.0, 0.35) / 0.35);
                let h = base * 0.55 + mountains * mask * 0.75;
                height[y * MAP + x] = h;
            }
        }
        // Stretch so the water covers about a third.
        let mut sorted = height.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (lo, hi) = (sorted[n / 100], sorted[n - n / 400]);
        for h in height.iter_mut() { *h = ((*h - lo) / (hi - lo)).clamp(0.0, 1.0); }
        let sun = V3::new(-0.55, 0.42, 0.72).norm();
        let mut color = vec![0u32; n];
        for y in 0..MAP {
            for x in 0..MAP {
                let i = y * MAP + x;
                let h = height[i];
                let at = |dx: i32, dy: i32| height[((y as i32 + dy) as usize & MASK) * MAP + ((x as i32 + dx) as usize & MASK)];
                let (dhx, dhy) = ((at(1, 0) - at(-1, 0)) * HSCALE / 2.0, (at(0, 1) - at(0, -1)) * HSCALE / 2.0);
                let normal = V3::new(-dhx, 1.0, -dhy).norm();
                let slope = 1.0 - normal.y;
                let (fx, fy) = (x as f32 / 16.0, y as f32 / 16.0);
                let grain = noise(fx * 4.0, fy * 4.0, 256, 77);
                let mut c = if h < WATER {
                    let depth = ((WATER - h) * 9.0).clamp(0.0, 1.0);
                    lerp(0x3d9bb0, 0x0a2450, depth)
                } else if h < WATER + 0.012 {
                    lerp(0xe0d29a, 0xc8b47a, grain)
                } else if h > 0.78 + grain * 0.06 - slope * 0.1 {
                    lerp(0xf2f5fa, 0xc8d4e2, slope * 2.0)
                } else if slope > 0.42 + grain * 0.15 {
                    lerp(0x6e655e, 0x9a8f84, grain)
                } else if h > 0.62 {
                    lerp(0x8a8a5a, 0x6e6e4c, grain)
                } else if fbm(fx * 0.3, fy * 0.3, 3, 91) > 0.52 && h > WATER + 0.03 {
                    lerp(0x2c5a26, 0x3f7a30, grain)
                } else {
                    lerp(0x5d9a3c, 0x8ab34e, grain)
                };
                // Sun, and the shadow of what stands between.
                let lit = 0.30 + 0.70 * normal.dot(sun).max(0.0);
                let mut shade = 1.0;
                if h >= WATER {
                    let (mut px, mut py, mut pz) = (x as f32, y as f32, h * HSCALE);
                    for _ in 0..70 {
                        px -= sun.x * 1.5; py -= sun.z * 1.5; pz += sun.y * 1.5;
                        let ground = height[(py as usize & MASK) * MAP + (px as usize & MASK)] * HSCALE;
                        if ground > pz + 0.5 { shade = 0.55; break; }
                        if pz > HSCALE { break; }
                    }
                }
                c = scale(c, lit * shade);
                color[i] = c;
            }
        }
        let clouds = (0..CLOUD * CLOUD).map(|i| fbm((i % CLOUD) as f32 / CLOUD as f32 * 8.0, (i / CLOUD) as f32 / CLOUD as f32 * 8.0, 5, 5)).collect();
        let mut levels = vec![Level { size: MAP, height, color }];
        while levels.len() < LEVELS { let up = levels[levels.len() - 1].halve(); levels.push(up); }
        Terrain { levels, clouds }
    }

    /// The height at a point, `lod` levels up from the full map. Between
    /// two levels both are read and mixed, so no seam shows where one
    /// hands over to the next.
    fn height(&self, x: f32, y: f32, lod: f32) -> f32 {
        let l = (lod as usize).min(LEVELS - 1);
        let a = self.levels[l].height_at(x, y);
        let t = lod - l as f32;
        if t <= 0.0 || l + 1 == LEVELS { return a; }
        a + (self.levels[l + 1].height_at(x, y) - a) * t
    }

    fn color(&self, x: f32, y: f32, lod: f32) -> Rgb {
        let l = (lod as usize).min(LEVELS - 1);
        let a = self.levels[l].color_at(x, y);
        let t = lod - l as f32;
        if t <= 0.0 || l + 1 == LEVELS { return a; }
        lerp(a, self.levels[l + 1].color_at(x, y), t)
    }
}

struct Soar {
    terrain: Terrain,
    x: f32, y: f32, alt: f32, yaw: f32, pitch: f32, speed: f32,
    auto: bool, idle: f32, time: f32,
    /// Per column: the direction it looks along (sine, cosine), and the
    /// row where its ground begins.
    dirs: Vec<(f32, f32)>,
    tops: Vec<i32>,
    /// The ground, a column after a column, painted over the cores.
    cols: Vec<Rgb>,
    audio: Audio,
}

impl Soar {
    fn new() -> Soar {
        let terrain = Terrain::generate();
        let mut audio = Audio::open();
        // Wind: noise smoothed into a hush, looped.
        let mut wind = Vec::with_capacity(22050);
        let (mut v, mut x) = (0.0f32, 0x1234_5678u32);
        for _ in 0..22050 { x ^= x << 13; x ^= x >> 17; x ^= x << 5; let r = (x >> 16) as f32 / 32768.0 - 1.0; v += (r - v) * 0.08; wind.push((v * 12000.0) as i16); }
        audio.play_loop(1, &Sample::from_i16(wind), 0.4);
        Soar { terrain, x: 300.0, y: 200.0, alt: 90.0, yaw: 0.4, pitch: 0.0, speed: 26.0, auto: true, idle: 0.0, time: 0.0,
            dirs: vec![(0.0, 1.0); W as usize], tops: vec![H; W as usize], cols: vec![0; (W * H) as usize], audio }
    }

    fn ground(&self) -> f32 { self.terrain.height(self.x, self.y, 0.0).max(WATER) * HSCALE }

    /// One column of ground, near to far, into `col` (top to bottom).
    /// Returns the row where the ground begins.
    fn column(&self, x: usize, col: &mut [Rgb], horizon: f32, focal: f32) -> i32 {
        let k = (x as f32 + 0.5 - W as f32 / 2.0) / focal;
        let (dx, dy) = self.dirs[x];
        let ang = self.yaw + k.atan();
        let toward = ((ang - SUN_AZ).cos()).max(0.0).powi(10);
        let stretch = (1.0 + k * k).sqrt();
        let far = 1400.0;
        let mut z = 1.0f32;
        let mut dz = 0.4f32;
        let mut ymin = H;
        // The colour at the top of the last span, while the ground runs
        // on unbroken; a span shades from its own colour down to it.
        let mut below: Option<(Rgb, f32)> = None;
        while z < far {
            let (px, py) = (self.x + dx * z * stretch, self.y + dy * z * stretch);
            // Read the map as coarse as the step along the ground.
            let lod = (dz * stretch).log2().max(0.0);
            let mut h = self.terrain.height(px, py, lod);
            let water = h < WATER;
            if water { h = WATER; }
            let fy = horizon + (self.alt - h * HSCALE) * focal / z;
            let sy = fy as i32;
            if sy < ymin {
                let mut c = self.terrain.color(px, py, lod);
                if water {
                    // Waves and the sun's glitter on them.
                    let wave = noise(px * 0.35 + self.time * 1.3, py * 0.35 - self.time * 0.9, 256, 3);
                    let glint = toward * (wave - 0.35).clamp(0.0, 1.0) * 1.6;
                    c = lerp(c, 0xfff4d0, glint.min(1.0));
                    c = lerp(c, ZENITH, 0.25);
                }
                let c = lerp(c, HAZE, 1.0 - (-z / 620.0).exp());
                let (y0, y1) = (sy.max(0), ymin.min(H));
                match below {
                    Some((b, by)) if by > fy => {
                        for y in y0..y1 { col[y as usize] = lerp(c, b, (y as f32 - fy) / (by - fy)); }
                    }
                    _ => for y in y0..y1 { col[y as usize] = c; },
                }
                ymin = sy;
                below = Some((c, fy));
            } else {
                below = None;
            }
            if ymin <= 0 { break; }
            z += dz;
            dz *= 1.005;
        }
        ymin
    }
}

impl Game for Soar {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        if input.pressed(Key::Char('q')) || input.pressed(Key::Escape) { return Flow::Quit; }
        self.time += dt;
        let steering = input.held(Key::Left) || input.held(Key::Right) || input.held(Key::Up) || input.held(Key::Down) || input.held(Key::Char('w')) || input.held(Key::Char('s'));
        if steering { self.auto = false; self.idle = 0.0; } else { self.idle += dt; }
        if input.pressed(Key::Space) || self.idle > 12.0 { self.auto = true; }
        if self.auto {
            // A lazy course: long turns, altitude following the ground.
            let turn = (self.time * 0.11).sin() * 0.35 + (self.time * 0.037).sin() * 0.25;
            self.yaw += turn * dt;
            let want = self.ground() + 70.0 + 25.0 * (self.time * 0.2).sin();
            self.alt += (want - self.alt) * dt * 0.8;
            self.pitch += (-0.04 - self.pitch) * dt;
            self.speed += (26.0 - self.speed) * dt;
        } else {
            let steer = (input.held(Key::Right) as i32 - input.held(Key::Left) as i32) as f32;
            self.yaw += steer * 0.9 * dt;
            let climb = (input.held(Key::Up) as i32 - input.held(Key::Down) as i32) as f32;
            self.alt += climb * 45.0 * dt;
            self.pitch += (climb * 0.25 - self.pitch) * dt * 3.0;
            if input.held(Key::Char('w')) { self.speed += 20.0 * dt; }
            if input.held(Key::Char('s')) { self.speed -= 20.0 * dt; }
            self.speed = self.speed.clamp(4.0, 80.0);
        }
        self.x += self.yaw.sin() * self.speed * dt;
        self.y += self.yaw.cos() * self.speed * dt;
        let floor = self.ground() + 6.0;
        if self.alt < floor { self.alt = floor; }
        self.audio.volume(1, 0.2 + self.speed / 200.0);
        self.audio.rate(1, 0.8 + self.speed / 120.0);
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        let focal = W as f32 * 0.55;
        let horizon = H as f32 * 0.5 + self.pitch * focal;
        for x in 0..W as usize {
            let ang = self.yaw + ((x as f32 + 0.5 - W as f32 / 2.0) / focal).atan();
            self.dirs[x] = (ang.sin(), ang.cos());
        }
        // Sky: a gradient, clouds, the sun.
        for y in 0..H {
            let t = ((horizon - y as f32) / (H as f32 * 0.9)).clamp(0.0, 1.0);
            let c = lerp(HAZE, ZENITH, t.powf(0.8));
            f.hline(0, y, W, c);
        }
        let scroll = self.time * 24.0;
        for y in 0..(horizon as i32).min(H).max(0) {
            let elev = (horizon - y as f32) / focal;
            if elev < 0.02 { continue; }
            // Clouds live on a flat layer far above: project by elevation.
            let d = 900.0 / elev;
            if d > 12000.0 { continue; }
            let fade = (1.0 - d / 12000.0).clamp(0.0, 1.0);
            for x in 0..W {
                let (sa, ca) = self.dirs[x as usize];
                let (cx, cy) = (self.x + sa * d + scroll, self.y + ca * d);
                let n = tex(&self.terrain.clouds, CLOUD, cx / 1900.0 * CLOUD as f32, cy / 1900.0 * CLOUD as f32);
                let cover = ((n - 0.5) * 3.2).clamp(0.0, 1.0) * fade;
                if cover > 0.0 {
                    let i = (y * W + x) as usize;
                    f.px[i] = lerp(f.px[i], 0xf8f4ee, cover * 0.85);
                }
            }
        }
        let rel = (SUN_AZ - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        if rel.abs() < 1.2 {
            let sx = (W as f32 / 2.0 + rel.tan() * focal) as i32;
            let sy = (horizon - 0.19 * focal) as i32;
            for y in (sy - 60).max(0)..(sy + 60).min(H) {
                for x in (sx - 60).max(0)..(sx + 60).min(W) {
                    let d = (((x - sx) * (x - sx) + (y - sy) * (y - sy)) as f32).sqrt();
                    let glow = if d < 11.0 { 1.0 } else { (1.0 - (d - 11.0) / 50.0).clamp(0.0, 1.0).powi(3) * 0.7 };
                    if glow > 0.0 { let i = (y * W + x) as usize; f.px[i] = lerp(f.px[i], 0xfff2c8, glow); }
                }
            }
        }
        // The ground: strips of columns over the cores, each core taking
        // the next strip when it is done.
        let (w, h) = (W as usize, H as usize);
        let mut cols = std::mem::take(&mut self.cols);
        let mut tops = std::mem::take(&mut self.tops);
        let strip = 8;
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        let next = std::sync::Mutex::new(cols.chunks_mut(h * strip).zip(tops.chunks_mut(strip)).enumerate());
        let me = &*self;
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| loop {
                    let job = next.lock().unwrap().next();
                    let Some((i, (col, top))) = job else { break };
                    for (k, t) in top.iter_mut().enumerate() { *t = me.column(i * strip + k, &mut col[k * h..(k + 1) * h], horizon, focal); }
                });
            }
        });
        for y in 0..h {
            for x in 0..w {
                if y as i32 >= tops[x] { f.px[y * w + x] = cols[x * h + y]; }
            }
        }
        self.cols = cols;
        self.tops = tops;
        let alt = (self.alt - self.ground()).max(0.0);
        f.text_big(8, H - 14, &format!("ALT {:4.0}  SPD {:3.0}{}", alt, self.speed, if self.auto { "  AUTO" } else { "" }), 0xe8e0d0);
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x9a9080);
    }
}

fn main() {
    let mut game = Soar::new();
    // SOAR_BENCH=<frames> flies that many frames with no terminal and
    // prints the time one takes.
    if let Ok(n) = std::env::var("SOAR_BENCH") {
        let n: u32 = n.parse().unwrap_or(300);
        let mut f = Frame::new(W, H);
        let input = Input::new();
        let t0 = std::time::Instant::now();
        for _ in 0..n { game.update(&input, 1.0 / 30.0); game.draw(&mut f); }
        eprintln!("{:.2} ms a frame at {}x{} over {} frames", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, W, H, n);
        return;
    }
    run(&mut game, Config { width: W, height: H, fps: 30 });
}
