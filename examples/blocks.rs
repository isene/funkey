//! blocks: a small world of blocks to dig and build in. Minecraft in
//! spirit, on funkey's textured rasterizer: hills, tunnels and groves
//! grown from a seed, a walker with weight, and nine kinds of block to
//! build with. There is nothing to win.
//!
//!     cargo run --release --example blocks
//!
//! W and S walk, A and D sidestep, the arrows look round, Space jumps.
//! F digs the block in the frame, E builds on it, 1 to 9 pick the kind.
//! P pauses, Q quits. The world is kept in ~/.funkey/blocks/world: its
//! seed and every block that was changed. `BLOCKS_BENCH=100` draws that
//! many frames from the top of a tower, with no terminal, and prints the
//! time a frame takes.

use funkey::noise::{fbm, hash, noise, smooth};
use funkey::*;
use std::collections::HashMap;

const W: i32 = 640;
const H: i32 = 400;
const GAME: &str = "blocks";
/// The game's own version; the engine has its own.
const VERSION: &str = "0.1";

/// The world's size in blocks: across, up and deep.
const SX: i32 = 256;
const SY: i32 = 64;
const SZ: i32 = 256;
/// The world is meshed in columns this wide, so a change redraws little.
const CHUNK: i32 = 16;
const ACROSS: i32 = SX / CHUNK;
/// The land generator's number, kept with the world: changed blocks from
/// an older generator are not laid over this one's land.
const LAND: u32 = 1;
/// Land lower than this is sand.
const SAND_LINE: i32 = 27;

const ZENITH: Rgb = 0x3d7fd8;
const HAZE: Rgb = 0xc4dcf0;
const SUN: V3 = V3 { x: 0.36, y: 0.80, z: 0.48 };
/// The haze thickens over this many blocks, and a chunk farther off than
/// `FAR` is lost in it and not drawn.
const FOG: f32 = 85.0;
const FAR: f32 = 210.0;
const FOCAL: f32 = W as f32 * 0.78;
const NEAR: f32 = 0.1;

/// The walker is a box this wide from its middle and this tall, with
/// eyes near the top.
const HALF: f32 = 0.3;
const TALL: f32 = 1.8;
const EYE: f32 = 1.62;
/// How far an arm reaches, in blocks.
const REACH: f32 = 5.0;
const WALK: f32 = 4.4;
/// A jump clears one block and not two.
const JUMP: f32 = 8.6;
const PULL: f32 = 28.0;

const AIR: u8 = 0;
const GRASS: u8 = 1;
const DIRT: u8 = 2;
const STONE: u8 = 3;
const SAND: u8 = 4;
const LOG: u8 = 5;
const LEAVES: u8 = 6;
const PLANKS: u8 = 7;
const BRICK: u8 = 8;
const GLASS: u8 = 9;
/// The floor of the world, which nothing digs.
const BEDROCK: u8 = 10;

/// The kinds the keys 1 to 9 pick.
const KINDS: [(u8, &str); 9] = [
    (GRASS, "GRASS"), (DIRT, "DIRT"), (STONE, "STONE"), (SAND, "SAND"), (LOG, "LOG"),
    (LEAVES, "LEAVES"), (PLANKS, "PLANKS"), (BRICK, "BRICK"), (GLASS, "GLASS"),
];

/// The faces behind these show through them.
fn clear(b: u8) -> bool { matches!(b, AIR | LEAVES | GLASS) }

/// Which picture a block wears on a face: 0 is its top, 1 its bottom.
fn skin(b: u8, face: usize) -> usize {
    match (b, face) {
        (GRASS, 0) => 0,
        (GRASS, 1) => 2,
        (GRASS, _) => 1,
        (DIRT, _) => 2,
        (STONE, _) => 3,
        (SAND, _) => 4,
        (LOG, 0 | 1) => 6,
        (LOG, _) => 5,
        (LEAVES, _) => 7,
        (PLANKS, _) => 8,
        (BRICK, _) => 9,
        (GLASS, _) => 10,
        _ => 11,
    }
}

/// The twelve pictures the blocks wear, 16 by 16, in `skin`'s order.
fn pictures() -> Vec<Texture> {
    // A colour made a little lighter or darker from texel to texel.
    fn grain(c: Rgb, x: u32, y: u32, seed: u32, k: f32) -> Rgb { tint(c, 1.0 - k / 2.0 + hash(x as i32, y as i32, seed) * k) }
    fn dirt(x: u32, y: u32) -> Rgb { grain(0x8a623c, x, y, 1, 0.35) }
    fn grass(x: u32, y: u32) -> Rgb { grain(0x5fa845, x, y, 2, 0.3) }
    fn bark(x: u32, y: u32) -> Rgb { tint(0x6e4c2c, 0.72 + hash(x as i32, 0, 8) * 0.3 + hash(x as i32, (y / 4) as i32, 9) * 0.2) }
    let pic = |f: fn(u32, u32) -> Rgb| Texture::from_fn(16, 16, f);
    vec![
        pic(grass),
        // Dirt with the grass hanging over its top edge.
        pic(|x, y| if y < 3 + (hash(x as i32, 0, 3) * 3.0) as u32 { grass(x, y) } else { dirt(x, y) }),
        pic(dirt),
        pic(|x, y| tint(0x868a8e, 0.76 + noise(x as f32 / 4.0, y as f32 / 4.0, 4, 5) * 0.34 + hash(x as i32, y as i32, 6) * 0.1)),
        pic(|x, y| grain(0xdccb8c, x, y, 7, 0.16)),
        pic(bark),
        // The cut end of a log: rings inside the bark.
        pic(|x, y| {
            let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs());
            if d > 6.0 { bark(x, y) } else { grain(if d as u32 % 2 == 0 { 0xb8935a } else { 0x9a7642 }, x, y, 15, 0.1) }
        }),
        pic(|x, y| if hash(x as i32, y as i32, 10) > 0.8 { CUTOUT } else { grain(0x3f8f36, x, y, 11, 0.5) }),
        // Four boards, their ends not in line.
        pic(|x, y| if y % 4 == 3 || (x + y / 4 * 5) % 16 == 0 { 0x7d5a30 } else { grain(0xba8c50, x, y / 4, 12, 0.12) }),
        // Bricks of 8 by 4 in mortar, each course half a brick along.
        pic(|x, y| {
            let along = x + y / 4 % 2 * 4;
            if y % 4 == 3 || along % 8 == 7 { 0xbdb5a6 } else { tint(grain(0xa24c38, x, y, 13, 0.14), 0.85 + hash((along / 8) as i32, (y / 4) as i32, 14) * 0.3) }
        }),
        // A frame, a glint, and nothing between.
        pic(|x, y| {
            if x == 0 || y == 0 || x == 15 || y == 15 { 0xcfe6f0 } else if (3..8).contains(&x) && (x + y == 9 || x + y == 10) { 0xeaf6fb } else { CUTOUT }
        }),
        pic(|x, y| tint(0x4a4c50, 0.5 + hash((x / 2) as i32, (y / 2) as i32, 16) * 0.9)),
    ]
}

/// A picture as a sprite, for the row of kinds at the bottom.
fn icon(t: &Texture) -> Sprite {
    let px = (0..256).map(|i| t.texel(0, (i % 16) as f32 / 16.0 + 0.03, (i / 16) as f32 / 16.0 + 0.03)).map(|c| if c == CUTOUT { 0 } else { 0xff00_0000 | c }).collect();
    Sprite { w: 16, h: 16, px }
}

/// The six faces of a block: which way each looks, its four corners as
/// steps from the block's low corner (the bottom two first, going round),
/// and how light a face that way is.
const FACES: [([i32; 3], [[i32; 3]; 4], f32); 6] = [
    ([0, 1, 0], [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]], 1.0),
    ([0, -1, 0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]], 0.5),
    ([0, 0, -1], [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]], 0.72),
    ([0, 0, 1], [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]], 0.72),
    ([-1, 0, 0], [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]], 0.86),
    ([1, 0, 0], [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]], 0.86),
];
/// Where on the picture the four corners read.
const UV: [(f32, f32); 4] = [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
/// A corner is darker the more blocks crowd round it: none, one, two, or
/// one on each side.
const CROWD: [f32; 4] = [1.0, 0.82, 0.66, 0.5];
/// How much light is left under a roof.
const SHADE: f32 = 0.42;

fn at(x: i32, y: i32, z: i32) -> usize { ((x * SZ + z) * SY + y) as usize }
fn inside(x: i32, y: i32, z: i32) -> bool { (0..SX).contains(&x) && (0..SY).contains(&y) && (0..SZ).contains(&z) }

/// How high the land stands at a place.
fn ground(x: f32, z: f32, seed: u32) -> i32 {
    let wide = noise(x / 110.0, z / 110.0, 4096, seed + 20);
    let hills = fbm(x / 56.0, z / 56.0, 4, seed, 4096);
    (14.0 + wide * 14.0 + hills * 24.0) as i32
}

struct World {
    seed: u32,
    cells: Vec<u8>,
    /// The highest block in each column that stops the sky's light.
    roof: Vec<i32>,
    /// The blocks that differ from the land as it grew: place and kind.
    changed: HashMap<u32, u8>,
}

impl World {
    /// The block at a place. Past the sides and under the floor the
    /// world is bedrock; over the top it is air.
    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if inside(x, y, z) { self.cells[at(x, y, z)] } else if y >= SY { AIR } else { BEDROCK }
    }

    fn roof_at(&self, x: i32, z: i32) -> i32 { self.roof[(x * SZ + z) as usize] }

    /// Does the sky's light reach a place straight down?
    fn lit(&self, x: i32, y: i32, z: i32) -> bool {
        !(0..SX).contains(&x) || !(0..SZ).contains(&z) || y > self.roof_at(x, z)
    }

    fn find_roof(&self, x: i32, z: i32) -> i32 {
        (0..SY).rev().find(|&y| !matches!(self.cells[at(x, y, z)], AIR | GLASS)).unwrap_or(0)
    }

    fn roofs(&mut self) {
        for x in 0..SX {
            for z in 0..SZ { self.roof[(x * SZ + z) as usize] = self.find_roof(x, z); }
        }
    }

    /// Grow the land from a seed: stone hills under dirt and grass, sand
    /// in the low places, tunnels through the stone, and groves of trees.
    fn grow(seed: u32) -> World {
        let mut w = World { seed, cells: vec![AIR; (SX * SY * SZ) as usize], roof: vec![0; (SX * SZ) as usize], changed: HashMap::new() };
        for x in 0..SX {
            for z in 0..SZ {
                let (fx, fz) = (x as f32, z as f32);
                let h = ground(fx, fz, seed);
                // A tunnel runs where one noise crosses its middle value,
                // at the height a second one gives, as wide as a third.
                let vein = (noise(fx / 44.0, fz / 44.0, 4096, seed + 31) - 0.5).abs() / 0.03;
                let mid = 6.0 + noise(fx / 60.0, fz / 60.0, 4096, seed + 32) * 34.0;
                let bore = if vein < 1.0 { (1.6 + noise(fx / 9.0, fz / 9.0, 4096, seed + 33) * 2.2) * (1.0 - vein * vein).sqrt() } else { 0.0 };
                for y in 0..h {
                    w.cells[at(x, y, z)] = if y == 0 {
                        BEDROCK
                    } else if (y as f32 + 0.5 - mid).abs() < bore {
                        AIR
                    } else if y < h - 4 {
                        STONE
                    } else if h < SAND_LINE {
                        SAND
                    } else if y < h - 1 {
                        DIRT
                    } else {
                        GRASS
                    };
                }
            }
        }
        // Trees stand on grass, close together in the groves a coarse
        // noise marks and nowhere between them.
        for x in 3..SX - 3 {
            for z in 3..SZ - 3 {
                let grove = smooth(((noise(x as f32 / 36.0, z as f32 / 36.0, 4096, seed + 40) - 0.5) * 5.0).clamp(0.0, 1.0));
                if hash(x, z, seed + 41) >= grove * 0.035 { continue; }
                let y = ground(x as f32, z as f32, seed);
                if w.get(x, y - 1, z) == GRASS { w.tree(x, y, z, 4 + (hash(x, z, seed + 42) * 3.0) as i32); }
            }
        }
        w.roofs();
        w
    }

    /// A trunk `tall` blocks high standing at a place, under a crown two
    /// blocks out at the bottom and one at the top.
    fn tree(&mut self, x: i32, y: i32, z: i32, tall: i32) {
        for up in tall - 2..=tall + 1 {
            let r: i32 = if up < tall { 2 } else { 1 };
            for dx in -r..=r {
                for dz in -r..=r {
                    // The corners are cut, all of them at the top.
                    let corner = dx.abs() == r && dz.abs() == r;
                    if corner && (up >= tall || hash(x + dx, z + dz + up, self.seed + 43) < 0.6) { continue; }
                    let (cx, cy, cz) = (x + dx, y + up, z + dz);
                    if inside(cx, cy, cz) && self.cells[at(cx, cy, cz)] == AIR { self.cells[at(cx, cy, cz)] = LEAVES; }
                }
            }
        }
        for up in 0..tall {
            if inside(x, y + up, z) { self.cells[at(x, y + up, z)] = LOG; }
        }
    }

    /// Change one block, and remember that it was changed.
    fn put(&mut self, c: [i32; 3], b: u8) {
        let i = at(c[0], c[1], c[2]);
        self.cells[i] = b;
        self.changed.insert(i as u32, b);
        self.roof[(c[0] * SZ + c[2]) as usize] = self.find_roof(c[0], c[2]);
    }

    /// The world as text: the generator's number, the seed, then every
    /// changed block as its place and its kind.
    fn text(&self) -> String {
        let mut changed: Vec<_> = self.changed.iter().collect();
        changed.sort();
        let mut t = format!("{LAND} {}", self.seed);
        for (i, b) in changed { t.push_str(&format!(" {i} {b}")); }
        t
    }

    /// The world that text was written from. Text from another generator,
    /// or that is not a world at all, gives none.
    fn from_text(t: &str) -> Option<World> {
        let mut n = t.split_whitespace().map(|w| w.parse::<u32>().ok());
        if n.next()?? != LAND { return None; }
        let mut w = World::grow(n.next()??);
        while let Some(i) = n.next() {
            let (i, b) = (i?, n.next()??);
            if i as usize >= w.cells.len() || b > BEDROCK as u32 { return None; }
            w.cells[i as usize] = b as u8;
            w.changed.insert(i, b as u8);
        }
        w.roofs();
        Some(w)
    }

    /// The faces that show in one chunk, lit corner by corner: a corner
    /// is darker where blocks crowd round it, and darker under a roof.
    fn mesh(&self, cx: i32, cz: i32) -> Model {
        let mut m = Model::new();
        for x in cx * CHUNK..(cx + 1) * CHUNK {
            for z in cz * CHUNK..(cz + 1) * CHUNK {
                for y in 0..SY {
                    let b = self.cells[at(x, y, z)];
                    if b == AIR { continue; }
                    for (f, (n, corners, shade)) in FACES.iter().enumerate() {
                        let into = [x + n[0], y + n[1], z + n[2]];
                        let next = self.get(into[0], into[1], into[2]);
                        // Hidden by a block, or by more of the same leaves or glass.
                        if !clear(next) || next == b { continue; }
                        // The two directions that run along the face.
                        let (t1, t2) = if n[0] != 0 { (1, 2) } else if n[1] != 0 { (0, 2) } else { (0, 1) };
                        let mut v = [Vert::new(V3::default(), 0.0, 0.0, 0.0); 4];
                        for (i, c) in corners.iter().enumerate() {
                            // The place the face looks into, and the three
                            // beside it round this corner.
                            let mut near = [into; 4];
                            near[1][t1] += 2 * c[t1] - 1;
                            near[2][t2] += 2 * c[t2] - 1;
                            near[3][t1] += 2 * c[t1] - 1;
                            near[3][t2] += 2 * c[t2] - 1;
                            let open = near.map(|q| clear(self.get(q[0], q[1], q[2])));
                            let crowd = if !open[1] && !open[2] { 3 } else { open[1..].iter().filter(|o| !**o).count() };
                            let (mut sky, mut count) = (0.0, 0.0);
                            for (q, o) in near.iter().zip(open) {
                                if !o { continue; }
                                count += 1.0;
                                if self.lit(q[0], q[1], q[2]) { sky += 1.0; }
                            }
                            let light = shade * CROWD[crowd] * (SHADE + (1.0 - SHADE) * sky / count);
                            v[i] = Vert::new(V3::new((x + c[0]) as f32, (y + c[1]) as f32, (z + c[2]) as f32), UV[i].0, UV[i].1, light);
                        }
                        // Cut the face along the diagonal that keeps one
                        // dark corner in a triangle of its own.
                        let mat = Mat::Tex(skin(b, f));
                        if v[0].lit[0] + v[2].lit[0] < v[1].lit[0] + v[3].lit[0] { m.quad(v[1], v[2], v[3], v[0], mat); } else { m.quad(v[0], v[1], v[2], v[3], mat); }
                    }
                }
            }
        }
        m.bound();
        m
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Ask, Play }

struct Sounds { dig: Sample, build: Sample }

struct Blocks {
    world: World,
    scene: Scene,
    /// A mesh for each chunk, row by row.
    chunks: Vec<Model>,
    /// The walker: where its feet are, how fast it rises, where it looks.
    pos: V3,
    rise: f32,
    yaw: f32,
    pitch: f32,
    grounded: bool,
    /// Which of the `KINDS` is built.
    pick: usize,
    /// Seconds a look key has been down.
    looking: f32,
    /// Seconds until a held dig or build key acts again.
    again: f32,
    mode: Mode,
    /// Is the world read from the store and written back? Not in a test
    /// or a bench.
    stored: bool,
    unsaved: bool,
    /// Seconds since the last change.
    since: f32,
    time: f32,
    icons: Vec<Sprite>,
    audio: Audio,
    snd: Sounds,
}

impl Blocks {
    fn new(world: World) -> Blocks {
        let mut scene = Scene::new(W, H);
        scene.near = NEAR;
        scene.fog = Some((HAZE, FOG));
        let pics = pictures();
        let icons = KINDS.iter().map(|&(kind, _)| icon(&pics[skin(kind, 2)])).collect();
        for p in pics { scene.texture(p); }
        let snd = Sounds { dig: Sample::noise(0.07, 0.5), build: Sample::sweep(Wave::Triangle, 230.0, 90.0, 0.07, 0.6) };
        let mut b = Blocks {
            world, scene, chunks: Vec::new(), pos: V3::default(), rise: 0.0, yaw: 0.0, pitch: 0.0, grounded: false, pick: 2, looking: 0.0, again: 0.0,
            mode: Mode::Title, stored: false, unsaved: false, since: 0.0, time: 0.0, icons, audio: Audio::off(), snd,
        };
        b.mesh_all();
        b.spawn();
        b
    }

    fn mesh_all(&mut self) {
        self.chunks = (0..ACROSS * (SZ / CHUNK)).map(|i| self.world.mesh(i % ACROSS, i / ACROSS)).collect();
    }

    /// Stand on open ground near the middle of the world: on grass where
    /// there is some, and not under a tree.
    fn spawn(&mut self) {
        let z = SZ / 2;
        let top = |x: i32| self.world.get(x, self.world.roof_at(x, z), z);
        let x = (SX / 2..SX - 2).find(|&x| top(x) == GRASS).or_else(|| (SX / 2..SX - 2).find(|&x| top(x) == SAND)).unwrap_or(SX / 2);
        self.pos = V3::new(x as f32 + 0.5, self.world.roof_at(x, z) as f32 + 1.0, z as f32 + 0.5);
        (self.rise, self.yaw, self.pitch) = (0.0, 0.0, -0.1);
    }

    /// A new world from a seed, in place of this one.
    fn fresh(&mut self, seed: u32) {
        self.world = World::grow(seed);
        self.mesh_all();
        self.spawn();
        self.keep();
    }

    /// Write the world and the walker to the store.
    fn keep(&mut self) {
        self.unsaved = false;
        if !self.stored { return; }
        store::save(GAME, "world", &self.world.text());
        store::save(GAME, "walker", &format!("{} {} {} {} {} {}", self.pos.x, self.pos.y, self.pos.z, self.yaw, self.pitch, self.pick));
    }

    /// Put the walker where `keep` wrote it was, if it fits there.
    fn stand(&mut self, t: &str) {
        let n: Vec<f32> = t.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        if n.len() != 6 || n.iter().any(|v| !v.is_finite()) { return; }
        let p = V3::new(n[0], n[1], n[2]);
        if self.blocked(p) { return; }
        (self.pos, self.yaw, self.pitch, self.pick) = (p, n[3], n[4].clamp(-1.5, 1.5), (n[5] as usize).min(KINDS.len() - 1));
    }

    fn eye(&self) -> V3 { self.pos.add(V3::new(0.0, EYE, 0.0)) }

    /// The way the walker looks, a step long.
    fn gaze(&self) -> V3 { M4::rotate_x(-self.pitch).then(&M4::rotate_y(self.yaw)).apply_dir(V3::new(0.0, 0.0, 1.0)) }

    /// Would the walker's box, its feet at `p`, be in a block or past
    /// the sides of the world?
    fn blocked(&self, p: V3) -> bool {
        if p.x < HALF || p.z < HALF || p.x > SX as f32 - HALF || p.z > SZ as f32 - HALF { return true; }
        let span = |lo: f32, hi: f32| lo.floor() as i32..=hi.floor() as i32;
        span(p.x - HALF, p.x + HALF).any(|x| span(p.y, p.y + TALL).any(|y| span(p.z - HALF, p.z + HALF).any(|z| self.world.get(x, y, z) != AIR)))
    }

    /// Would a block at `c` be where the walker stands?
    fn touches(&self, c: [i32; 3]) -> bool {
        let (p, c) = (self.pos, c.map(|v| v as f32));
        c[0] < p.x + HALF && c[0] + 1.0 > p.x - HALF && c[1] < p.y + TALL && c[1] + 1.0 > p.y && c[2] < p.z + HALF && c[2] + 1.0 > p.z - HALF
    }

    /// Move the walker for `dt` seconds: `ahead` and `side` are -1 to 1.
    fn walk(&mut self, ahead: f32, side: f32, jump: bool, dt: f32) {
        let (s, c) = self.yaw.sin_cos();
        let (mut dx, mut dz) = (s * ahead + c * side, c * ahead - s * side);
        let len = (dx * dx + dz * dz).sqrt();
        if len > 1.0 { dx /= len; dz /= len; }
        if jump && self.grounded { self.rise = JUMP; }
        self.rise = (self.rise - PULL * dt).max(-24.0);
        // One direction at a time, so the walker slides along a wall.
        let mut p = self.pos;
        for step in [V3::new(dx * WALK * dt, 0.0, 0.0), V3::new(0.0, 0.0, dz * WALK * dt)] {
            if !self.blocked(p.add(step)) { p = p.add(step); }
        }
        let up = p.add(V3::new(0.0, self.rise * dt, 0.0));
        self.grounded = false;
        if !self.blocked(up) {
            p = up;
        } else {
            if self.rise < 0.0 {
                // Landed: the feet go on top of the block they fell into.
                let top = V3::new(p.x, up.y.floor() + 1.0, p.z);
                if !self.blocked(top) { p = top; }
                self.grounded = true;
            }
            self.rise = 0.0;
        }
        self.pos = p;
    }

    /// The block in the middle of the picture, if an arm reaches it, and
    /// which way the side that is looked at faces.
    fn aim(&self) -> Option<([i32; 3], [i32; 3])> {
        let (eye, gaze) = (self.eye(), self.gaze());
        let (o, d) = ([eye.x, eye.y, eye.z], [gaze.x, gaze.y, gaze.z]);
        let mut c = o.map(|v| v.floor() as i32);
        let step = d.map(|v| if v > 0.0 { 1 } else { -1 });
        // How far along the line of sight the next wall of the grid is in
        // each direction, and how far it is from one wall to the next.
        let pace = d.map(|v| if v == 0.0 { f32::INFINITY } else { 1.0 / v.abs() });
        let mut next = [0.0; 3];
        for i in 0..3 {
            let wall = if d[i] > 0.0 { c[i] as f32 + 1.0 } else { c[i] as f32 };
            next[i] = if d[i] == 0.0 { f32::INFINITY } else { (wall - o[i]) / d[i] };
        }
        loop {
            let i = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b])).unwrap();
            if next[i] > REACH { return None; }
            c[i] += step[i];
            next[i] += pace[i];
            if self.world.get(c[0], c[1], c[2]) == AIR { continue; }
            let mut face = [0; 3];
            face[i] = -step[i];
            return inside(c[0], c[1], c[2]).then_some((c, face));
        }
    }

    /// Change a block and mesh again what it touches: its own chunk, and
    /// the next one when the block is on the edge, for the light it
    /// gives or takes there.
    fn set(&mut self, c: [i32; 3], b: u8) {
        self.world.put(c, b);
        let span = |v: i32, most: i32| (v - 1).max(0) / CHUNK..=((v + 1) / CHUNK).min(most - 1);
        for cz in span(c[2], SZ / CHUNK) {
            for cx in span(c[0], ACROSS) { self.chunks[(cz * ACROSS + cx) as usize] = self.world.mesh(cx, cz); }
        }
        (self.unsaved, self.since) = (true, 0.0);
    }

    fn play(&mut self, input: &Input, dt: f32) {
        let held = |k: Key| input.held(k) as i32 as f32;
        for i in 0..KINDS.len() {
            if input.pressed(Key::Char((b'1' + i as u8) as char)) { self.pick = i; }
        }
        // A look key starts slow, to aim at one block, and speeds up.
        let (turn, nod) = (held(Key::Right) - held(Key::Left), held(Key::Up) - held(Key::Down));
        self.looking = if turn != 0.0 || nod != 0.0 { self.looking + dt } else { 0.0 };
        let rate = (0.8 + self.looking * 5.0).min(2.6);
        self.yaw += turn * rate * dt;
        self.pitch = (self.pitch + nod * rate * 0.7 * dt).clamp(-1.5, 1.5);

        let (ahead, side) = (held(Key::Char('w')) - held(Key::Char('s')), held(Key::Char('d')) - held(Key::Char('a')));
        // Short steps, so a fall never passes through a floor.
        let steps = (dt * 60.0).ceil().max(1.0);
        for _ in 0..steps as usize { self.walk(ahead, side, input.held(Key::Space), dt / steps); }

        // A held key digs or builds again after a moment.
        self.again -= dt;
        let acts = |k: Key| input.pressed(k) || (input.held(k) && self.again <= 0.0);
        let (digs, builds) = (acts(Key::Char('f')), acts(Key::Char('e')));
        if digs || builds {
            self.again = 0.22;
            if let Some((c, face)) = self.aim() {
                let to = [c[0] + face[0], c[1] + face[1], c[2] + face[2]];
                if digs && self.world.get(c[0], c[1], c[2]) != BEDROCK {
                    self.set(c, AIR);
                    self.audio.play(&self.snd.dig, 0.5);
                } else if builds && inside(to[0], to[1], to[2]) && self.world.get(to[0], to[1], to[2]) == AIR && !self.touches(to) {
                    self.set(to, KINDS[self.pick].0);
                    self.audio.play(&self.snd.build, 0.6);
                }
            }
        }
        self.since += dt;
        if self.unsaved && self.since > 5.0 { self.keep(); }
    }

    /// The edges of the side that is looked at, drawn over the picture.
    fn frame_aim(&self, f: &mut Frame, cam: &Cam3) {
        let Some((c, face)) = self.aim() else { return };
        let Some((_, corners, _)) = FACES.iter().find(|side| side.0 == face) else { return };
        let view = cam.view();
        let p = corners.map(|k| view.apply(V3::new((c[0] + k[0]) as f32, (c[1] + k[1]) as f32, (c[2] + k[2]) as f32)));
        for i in 0..4 {
            let (mut a, mut b) = (p[i], p[(i + 1) % 4]);
            // Cut the edge where it passes behind the eye.
            if a.z < NEAR && b.z < NEAR { continue; }
            if a.z < NEAR { a = a.lerp(b, (NEAR - a.z) / (b.z - a.z)); }
            if b.z < NEAR { b = b.lerp(a, (NEAR - b.z) / (a.z - b.z)); }
            let on = |q: V3| ((W as f32 / 2.0 + q.x * FOCAL / q.z) as i32, (H as f32 / 2.0 - q.y * FOCAL / q.z) as i32);
            let ((x0, y0), (x1, y1)) = (on(a), on(b));
            f.line(x0, y0, x1, y1, WHITE);
        }
    }

    /// The nine kinds along the bottom, the picked one framed in white.
    fn bar(&self, f: &mut Frame) {
        let x0 = (W - KINDS.len() as i32 * 22) / 2;
        let y = H - 26;
        for (i, icon) in self.icons.iter().enumerate() {
            let x = x0 + i as i32 * 22;
            f.rect(x, y, 20, 20, if i == self.pick { WHITE } else { 0x20242c });
            f.rect(x + 1, y + 1, 18, 18, 0x101218);
            f.blit(icon, x + 2, y + 2);
            f.text(x + 8, y - 7, &(i + 1).to_string(), if i == self.pick { WHITE } else { 0x9098a8 });
        }
        f.text_centered(W / 2, y - 17, KINDS[self.pick].1, WHITE, false, 1);
    }
}

/// The sky along a line of sight: blue overhead, pale at the horizon,
/// and the sun.
fn sky(d: V3) -> Rgb {
    if d.dot(SUN) > 0.9985 { return 0xfff6d8; }
    mix(HAZE, ZENITH, (d.y.max(0.0) * 1.7).min(1.0))
}

impl Game for Blocks {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        self.time += dt;
        let key = |c: char| input.pressed(Key::Char(c));
        if key('q') || input.pressed(Key::Escape) {
            self.keep();
            return Flow::Quit;
        }
        match self.mode {
            Mode::Title => {
                if input.pressed(Key::Space) || input.pressed(Key::Enter) { self.mode = Mode::Play; }
                if key('n') { self.mode = Mode::Ask; }
            }
            Mode::Ask => {
                if key('y') { self.fresh(Rng::from_time().next_u32()); }
                if input.any_pressed() { self.mode = Mode::Title; }
            }
            Mode::Play => {
                if key('p') {
                    self.keep();
                    return Flow::Pause;
                }
                self.play(input, dt);
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        // The title looks slowly round from where the walker stands.
        let yaw = if self.mode == Mode::Play { self.yaw } else { self.yaw + self.time * 0.12 };
        let cam = Cam3 { pos: self.eye(), yaw, pitch: self.pitch, focal: FOCAL };
        self.scene.begin(&cam);
        let still = M4::identity();
        for (i, chunk) in self.chunks.iter().enumerate() {
            let (cx, cz) = (i as i32 % ACROSS, i as i32 / ACROSS);
            let (dx, dz) = ((cx * CHUNK + CHUNK / 2) as f32 - cam.pos.x, (cz * CHUNK + CHUNK / 2) as f32 - cam.pos.z);
            if dx * dx + dz * dz < (FAR + CHUNK as f32) * (FAR + CHUNK as f32) { self.scene.push(chunk, &still); }
        }
        self.scene.render(f, &sky);

        if self.mode == Mode::Play {
            self.frame_aim(f, &cam);
            f.rect(W / 2 - 4, H / 2, 9, 1, WHITE);
            f.rect(W / 2, H / 2 - 4, 1, 9, WHITE);
            self.bar(f);
            return;
        }
        f.dim(0.55);
        let cx = W / 2;
        if self.mode == Mode::Ask {
            f.text_centered(cx, 170, "A NEW WORLD?", WHITE, true, 2);
            f.text_centered(cx, 200, "THE ONE YOU HAVE IS LOST.", 0xd0d4e0, true, 1);
            f.text_centered(cx, 226, "Y MAKES IT   ANY OTHER KEY KEEPS THE OLD", 0x70e0d0, true, 1);
            return;
        }
        f.fancy_text(cx, 70, "BLOCKS", 8, self.time, (0x9be06a, 0x4a9a3c));
        f.text_centered(cx, 172, "W A S D WALK   ARROWS LOOK   SPACE JUMPS", 0xd0d4e0, true, 1);
        f.text_centered(cx, 188, "F DIGS   E BUILDS   1-9 PICK A BLOCK", 0xd0d4e0, true, 1);
        f.text_centered(cx, 204, "P PAUSES   Q QUITS", 0xd0d4e0, true, 1);
        f.text_centered(cx, 250, "SPACE: PLAY", 0x70e0d0, true, 2);
        f.text_centered(cx, 286, "N: A NEW WORLD", 0x9098a8, true, 1);
        f.text(W - 18, H - 9, VERSION, 0x9098a8);
    }
}

/// The game with the world from the store, or a new one.
fn blocks() -> Blocks {
    let world = store::load(GAME, "world").and_then(|t| World::from_text(&t));
    let new = world.is_none();
    let mut b = Blocks::new(world.unwrap_or_else(|| World::grow(Rng::from_time().next_u32())));
    b.stored = true;
    b.unsaved = new;
    if !new {
        if let Some(t) = store::load(GAME, "walker") { b.stand(&t); }
    }
    b.audio = Audio::open();
    b
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = Config { width: W, height: H, fps: 30 };
    // BLOCKS_BENCH=<frames> looks round a world from the top of a tower,
    // with no terminal, and prints the time a frame takes.
    if let Ok(n) = std::env::var("BLOCKS_BENCH") {
        let t0 = std::time::Instant::now();
        let mut game = Blocks::new(World::grow(7));
        game.mode = Mode::Play;
        eprintln!("built in {:.2} s", t0.elapsed().as_secs_f64());
        let at = [game.pos.x as i32, 0, game.pos.z as i32];
        for y in game.pos.y as i32..SY { game.set([at[0], y, at[2]], STONE); }
        game.pos.y = SY as f32;
        let mut input = Input::new();
        input.key(Key::Right, true);
        let f = bench(&mut game, &input, cfg, n.parse().unwrap_or(100));
        if let Ok(p) = std::env::var("FUNKEY_SHOT") { let _ = std::fs::write(p, f.to_ppm()); }
        return;
    }
    run(&mut blocks(), cfg);
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(blocks(), Config { width: W, height: H, fps: 30 });

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat stone up to a height, with nothing on it.
    fn flat(h: i32) -> World {
        let mut w = World { seed: 0, cells: vec![AIR; (SX * SY * SZ) as usize], roof: vec![0; (SX * SZ) as usize], changed: HashMap::new() };
        for x in 0..SX {
            for z in 0..SZ {
                for y in 0..h { w.cells[at(x, y, z)] = if y == 0 { BEDROCK } else { STONE }; }
            }
        }
        w.roofs();
        w
    }

    /// A game on flat stone ten blocks deep, the walker in play at the
    /// middle, looking along z.
    fn on_flat() -> Blocks {
        let mut b = Blocks::new(flat(10));
        b.mode = Mode::Play;
        b.pos = V3::new(100.5, 10.0, 100.5);
        (b.yaw, b.pitch) = (0.0, 0.0);
        b
    }

    fn held(keys: &[Key]) -> Input {
        let mut i = Input::new();
        for &k in keys { i.inject(k); }
        i.clear_pressed();
        i
    }

    fn pressed(key: Key) -> Input {
        let mut i = Input::new();
        i.inject(key);
        i
    }

    fn run_for(b: &mut Blocks, input: &Input, secs: f32) {
        for _ in 0..(secs * 30.0) as usize { b.update(input, 1.0 / 30.0); }
    }

    #[test]
    fn a_seed_grows_the_same_world_every_time() {
        let (a, b, c) = (World::grow(5), World::grow(5), World::grow(6));
        assert!(a.cells == b.cells && a.cells != c.cells);
        // Land, air over it, tunnels in it and trees on it.
        let count = |kind| a.cells.iter().filter(|&&k| k == kind).count();
        assert!(count(GRASS) > 20_000 && count(SAND) > 1000 && count(LOG) > 300 && count(LEAVES) > 3000, "{} {} {} {}", count(GRASS), count(SAND), count(LOG), count(LEAVES));
        let holes = (0..SX).flat_map(|x| (0..SZ).map(move |z| (x, z))).filter(|&(x, z)| (1..a.roof_at(x, z)).any(|y| a.get(x, y, z) == AIR && a.get(x, y + 1, z) == STONE)).count();
        assert!(holes > 500, "{holes} columns with a tunnel");
        // The floor is whole, and the top layer is free to build in.
        assert!((0..SX).all(|x| (0..SZ).all(|z| a.get(x, 0, z) == BEDROCK && a.get(x, SY - 1, z) == AIR)));
    }

    #[test]
    fn the_walker_falls_to_the_ground_and_stands_on_it() {
        let mut b = on_flat();
        b.pos.y = 14.3;
        run_for(&mut b, &Input::new(), 2.0);
        assert_eq!((b.pos.y, b.grounded), (10.0, true));
        // A long fall does not pass through the floor either.
        b.pos.y = 60.0;
        run_for(&mut b, &Input::new(), 6.0);
        assert_eq!((b.pos.y, b.grounded), (10.0, true));
    }

    #[test]
    fn a_wall_stops_the_walker_and_it_slides_along() {
        let mut b = on_flat();
        for x in 90..110 {
            for y in 10..13 { b.set([x, y, 104], STONE); }
        }
        run_for(&mut b, &held(&[Key::Char('w')]), 3.0);
        assert!(b.pos.z > 103.5 && b.pos.z <= 104.0 - HALF, "z {}", b.pos.z);
        // Walking into it at a slant still moves the walker along it.
        b.yaw = 0.6;
        let x = b.pos.x;
        run_for(&mut b, &held(&[Key::Char('w')]), 1.0);
        assert!(b.pos.x > x + 1.0 && b.pos.z <= 104.0 - HALF);
    }

    #[test]
    fn a_jump_clears_one_block_and_not_two() {
        let mut b = on_flat();
        b.set([100, 10, 103], STONE);
        run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 1.5);
        assert!(b.pos.z > 103.0 && b.pos.y >= 11.0, "over one block: {:?}", b.pos);

        let mut b = on_flat();
        for y in 10..12 { b.set([100, y, 103], STONE); }
        run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 1.5);
        assert!(b.pos.z < 103.0 - HALF + 0.01, "not over two: {:?}", b.pos);
    }

    #[test]
    fn nobody_walks_out_of_the_world() {
        let mut b = on_flat();
        b.pos = V3::new(2.5, 10.0, 2.5);
        b.yaw = -2.4;
        run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 4.0);
        assert!(b.pos.x >= HALF && b.pos.z >= HALF && b.pos.y >= 10.0, "{:?}", b.pos);
        // Nor over the top of it, from a tower at the edge.
        for y in 10..SY { b.set([1, y, 1], STONE); }
        b.pos = V3::new(1.5, SY as f32, 1.5);
        run_for(&mut b, &held(&[Key::Char('w')]), 2.0);
        assert!(b.pos.x >= HALF && b.pos.z >= HALF, "{:?}", b.pos);
    }

    #[test]
    fn f_digs_the_block_in_the_frame_and_e_builds_on_it() {
        let mut b = on_flat();
        // Looking down at the ground ahead.
        b.pitch = -0.9;
        let (c, face) = b.aim().expect("the ground is within reach");
        assert_eq!((c, face), ([100, 9, 101], [0, 1, 0]));
        b.pick = 7;
        b.update(&pressed(Key::Char('e')), 1.0 / 30.0);
        assert_eq!(b.world.get(100, 10, 101), BRICK);
        // Now the brick is in the frame; F takes it, and then the stone.
        assert_eq!(b.aim().unwrap().0, [100, 10, 101]);
        b.update(&pressed(Key::Char('f')), 1.0 / 30.0);
        assert_eq!(b.world.get(100, 10, 101), AIR);
        b.update(&pressed(Key::Char('f')), 1.0 / 30.0);
        assert_eq!(b.world.get(100, 9, 101), AIR);
        assert!(b.unsaved && b.world.changed.len() == 2);

        // A held key digs on: straight down to the floor, and no further.
        let mut b = on_flat();
        b.pitch = -1.5;
        run_for(&mut b, &held(&[Key::Char('f')]), 6.0);
        assert_eq!((b.pos.y, b.world.get(100, 0, 100)), (1.0, BEDROCK));
        // The sky is out of reach, and so is ground far ahead.
        let mut b = on_flat();
        b.pitch = 1.0;
        assert_eq!(b.aim(), None);
        b.pitch = -0.2;
        assert_eq!(b.aim(), None);
    }

    #[test]
    fn no_block_is_built_where_the_walker_stands() {
        let mut b = on_flat();
        // A wall right in front; building on its near side would fill
        // the walker's own place.
        for y in 10..12 { b.set([100, y, 101], STONE); }
        b.pos.z = 100.69;
        let before = b.world.changed.len();
        b.update(&pressed(Key::Char('e')), 1.0 / 30.0);
        assert_eq!(b.world.changed.len(), before);
        assert!(!b.blocked(b.pos));
    }

    #[test]
    fn a_chunk_shows_only_the_faces_that_can_be_seen() {
        let mut w = flat(1);
        // The floor of one chunk: 256 tops, no sides, no bottoms.
        assert_eq!(w.mesh(3, 3).tris.len(), 256 * 2);
        // One block on it adds five faces and hides one.
        w.put([56, 1, 56], STONE);
        assert_eq!(w.mesh(3, 3).tris.len(), (256 + 5 - 1) * 2);
        // A second beside it: each has four sides and a top to show.
        w.put([57, 1, 56], STONE);
        assert_eq!(w.mesh(3, 3).tris.len(), (256 + 8 - 2) * 2);
        // Glass hides nothing behind it, and glass on glass has no face between.
        let mut w = flat(1);
        w.put([56, 1, 56], GLASS);
        w.put([56, 2, 56], GLASS);
        assert_eq!(w.mesh(3, 3).tris.len(), (256 + 4 + 4 + 1) * 2);
    }

    #[test]
    fn it_is_darker_under_a_roof_and_in_a_corner() {
        let top_light = |w: &World, x: i32, z: i32| {
            let m = w.mesh(x / CHUNK, z / CHUNK);
            let at = |v: &Vert| v.p.y == 1.0 && (x as f32..=x as f32 + 1.0).contains(&v.p.x) && (z as f32..=z as f32 + 1.0).contains(&v.p.z);
            let own: Vec<f32> = m.tris.iter().filter(|(t, _)| t.iter().all(|&i| at(&m.verts[i as usize]))).flat_map(|(t, _)| t.map(|i| m.verts[i as usize].lit[0])).collect();
            own.iter().sum::<f32>() / own.len() as f32
        };
        let mut w = flat(1);
        assert_eq!(top_light(&w, 56, 56), 1.0);
        // A roof three blocks up, five by five.
        for x in 54..59 {
            for z in 54..59 { w.put([x, 4, z], STONE); }
        }
        assert!((top_light(&w, 56, 56) - SHADE).abs() < 0.001);
        // A block beside the place darkens the corners next to it.
        let mut w = flat(1);
        w.put([57, 1, 56], STONE);
        let beside = top_light(&w, 56, 56);
        assert!(beside < 0.95 && beside > 0.8, "{beside}");
    }

    #[test]
    fn the_world_comes_back_from_its_text() {
        let mut w = World::grow(11);
        w.put([20, 40, 30], BRICK);
        w.put([20, 41, 30], GLASS);
        let x = (0..SX).find(|&x| w.get(x, w.roof_at(x, 9), 9) == GRASS).unwrap();
        w.put([x, w.roof_at(x, 9), 9], AIR);
        let back = World::from_text(&w.text()).expect("a world's own text reads back");
        assert!(back.cells == w.cells && back.roof == w.roof && back.changed == w.changed && back.seed == 11);
        // Another generator's world, or junk, gives none.
        assert!(World::from_text(&w.text().replacen("1 ", "2 ", 1)).is_none());
        for junk in ["", "1", "1 x", "1 11 5", "1 11 5 99", "1 11 99999999 3"] { assert!(World::from_text(junk).is_none(), "{junk:?}"); }
    }

    #[test]
    fn the_walker_stands_where_it_was_kept() {
        let mut b = on_flat();
        b.stand("50.5 10 60.5 1.25 -0.5 8");
        assert_eq!((b.pos, b.yaw, b.pitch, b.pick), (V3::new(50.5, 10.0, 60.5), 1.25, -0.5, 8));
        // Not inside the stone, not from junk.
        for bad in ["50.5 5 60.5 0 0 0", "1 2 3", "a b c d e f", "NaN 10 60.5 0 0 0", "-5 10 5 0 0 0"] {
            b.stand(bad);
            assert_eq!(b.pos, V3::new(50.5, 10.0, 60.5), "{bad:?}");
        }
    }

    #[test]
    fn a_new_world_starts_on_open_ground() {
        for seed in [1, 2, 3] {
            let mut b = Blocks::new(World::grow(seed));
            assert!(!b.blocked(b.pos), "seed {seed}: {:?}", b.pos);
            let under = b.world.get(b.pos.x as i32, b.pos.y as i32 - 1, b.pos.z as i32);
            assert!(matches!(under, GRASS | SAND), "seed {seed} stands on {under}");
            // A walk with jumps for a minute: the walker stays in the
            // world, on its feet, and never inside a block.
            b.mode = Mode::Play;
            let mut rng = Rng::new(seed as u64);
            for _ in 0..60 {
                b.yaw = rng.range(0.0, 6.28);
                run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 1.0);
                assert!(!b.blocked(b.pos) && b.pos.y >= 1.0, "seed {seed}: {:?}", b.pos);
            }
        }
    }

    #[test]
    fn every_screen_draws() {
        let mut b = Blocks::new(World::grow(3));
        let mut f = Frame::new(W, H);
        b.draw(&mut f);
        let title = f.px.clone();
        b.mode = Mode::Ask;
        b.draw(&mut f);
        assert!(f.px != title);
        b.mode = Mode::Play;
        b.pitch = -0.6;
        b.draw(&mut f);
        // The cross in the middle, the picked kind's frame, and land
        // that is not one flat colour.
        assert_eq!(f.get(W / 2 - 3, H / 2), WHITE);
        assert_eq!(f.get((W - 9 * 22) / 2 + 2 * 22, H - 26), WHITE);
        let mut seen: Vec<Rgb> = (0..W).map(|x| f.get(x, 300)).collect();
        seen.sort();
        seen.dedup();
        assert!(seen.len() > 30, "{} colours on a row of land", seen.len());
        b.draw_paused(&mut f);
    }
}
