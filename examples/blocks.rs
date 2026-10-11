//! blocks: a small world of blocks to dig and build in. Minecraft in
//! spirit, on funkey's textured rasterizer: hills, mountains with snow
//! on them, a sea, tunnels and groves grown from a seed, clouds over it
//! all, and cats that come to see who is there. What is dug goes in the
//! pockets, and a few things are made from what is in them. There is
//! nothing to win.
//!
//!     cargo run --release --example blocks
//!
//! W and S walk, A and D sidestep, the arrows look round, Space jumps
//! and swims. F digs the block in the frame, E builds on it, 1 to 9 and
//! 0 pick the kind, C opens the list of things to make. P pauses, Q
//! quits. The world is kept in ~/.funkey/blocks/world: its seed and
//! every block that was changed. `BLOCKS_BENCH=100` draws that many
//! frames from the top of a tower, with no terminal, and prints the
//! time a frame takes.

use funkey::noise::{fbm, hash, noise, ridged, smooth};
use funkey::*;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

const W: i32 = 640;
const H: i32 = 400;
const GAME: &str = "blocks";
/// The game's own version; the engine has its own.
const VERSION: &str = "0.2";

/// The world's size in blocks: across, up and deep.
const SX: i32 = 256;
const SY: i32 = 128;
const SZ: i32 = 256;
/// The world is meshed in columns this wide, so a change redraws little.
const CHUNK: i32 = 16;
const ACROSS: i32 = SX / CHUNK;
/// The land generator's number, kept with the world: changed blocks from
/// an older generator are not laid over this one's land.
const LAND: u32 = 2;
/// Land lower than this lies under water.
const SEA: i32 = 36;
/// No tree grows above the first line, and snow lies above the second.
const TREE_LINE: i32 = 70;
const SNOW_LINE: i32 = 86;
/// The great mountain is this high, and this far from its top to its
/// foot.
const PEAK: f32 = 106.0;
const PEAK_FOOT: f32 = 72.0;

const ZENITH: Rgb = 0x3d7fd8;
const HAZE: Rgb = 0xc4dcf0;
const DEEP: Rgb = 0x1c4f8f;
const SUN: V3 = V3 { x: 0.36, y: 0.80, z: 0.48 };
/// The haze thickens over this many blocks, and a chunk farther off than
/// `FAR` is lost in it and not drawn.
const FOG: f32 = 85.0;
const FAR: f32 = 210.0;
const FOCAL: f32 = W as f32 * 0.78;
const NEAR: f32 = 0.1;
/// The clouds lie flat at this height, in squares this wide, and drift
/// this many blocks a second.
const CLOUDS: f32 = 150.0;
const PUFF: f32 = 12.0;
const DRIFT: f32 = 1.5;

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
/// The speed Space gives in water: enough to climb out onto a bank that
/// is level with the sea.
const SWIM: f32 = 5.5;

/// A cat is a box this wide from its middle and this tall.
const CAT_HALF: f32 = 0.2;
const CAT_TALL: f32 = 0.5;
const CATS: usize = 8;

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
const WATER: u8 = 11;
const SNOW: u8 = 12;

/// The kinds the keys 1 to 9 and 0 pick, and the pockets hold.
const KINDS: [(u8, &str); 10] = [
    (GRASS, "GRASS"), (DIRT, "DIRT"), (STONE, "STONE"), (SAND, "SAND"), (LOG, "LOG"),
    (LEAVES, "LEAVES"), (PLANKS, "PLANKS"), (BRICK, "BRICK"), (GLASS, "GLASS"), (SNOW, "SNOW"),
];

/// Which pocket a kind of block goes in.
fn slot(b: u8) -> Option<usize> { KINDS.iter().position(|k| k.0 == b) }

/// A thing to make: what it takes, and what comes of it. With nothing
/// named it is the pickaxe, which is made once.
struct Recipe { name: &'static str, needs: &'static [(u8, u32)], gives: Option<(u8, u32)> }

const RECIPES: [Recipe; 5] = [
    Recipe { name: "PLANKS", needs: &[(LOG, 1)], gives: Some((PLANKS, 4)) },
    Recipe { name: "PICKAXE", needs: &[(PLANKS, 3), (LOG, 1)], gives: None },
    Recipe { name: "GLASS", needs: &[(SAND, 1)], gives: Some((GLASS, 1)) },
    Recipe { name: "BRICK", needs: &[(STONE, 1), (SAND, 1)], gives: Some((BRICK, 2)) },
    Recipe { name: "GRASS", needs: &[(DIRT, 1), (LEAVES, 1)], gives: Some((GRASS, 1)) },
];

/// A walker cannot pass through these.
fn solid(b: u8) -> bool { !matches!(b, AIR | WATER) }

/// The faces behind these show through them.
fn clear(b: u8) -> bool { matches!(b, AIR | LEAVES | GLASS | WATER) }

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
        (WATER, _) => WAVES,
        (SNOW, _) => 13,
        _ => 11,
    }
}

/// The picture the water wears. It is changed for the next of sixteen
/// a few times a second, each a texel along, so the water moves.
const WAVES: usize = 12;

fn wave(n: u32) -> Texture {
    Texture::from_fn(16, 16, move |x, y| tint(0x3a78c8, 0.86 + noise((x + n) as f32 / 4.0, (y + n) as f32 / 4.0, 4, 17) * 0.28))
}

/// The fourteen pictures the blocks wear, 16 by 16, in `skin`'s order.
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
        wave(0),
        pic(|x, y| grain(0xf1f4f8, x, y, 18, 0.07)),
    ]
}

/// A picture as a sprite `k` times as light, for the row of kinds at
/// the bottom.
fn icon(t: &Texture, k: f32) -> Sprite {
    let texel = |i: u32| t.texel(0, (i % 16) as f32 / 16.0 + 0.03, (i / 16) as f32 / 16.0 + 0.03);
    Sprite { w: 16, h: 16, px: (0..256).map(texel).map(|c| if c == CUTOUT { 0 } else { 0xff00_0000 | tint(c, k) }).collect() }
}

fn pickaxe() -> Sprite {
    Sprite::from_rows(
        &[
            "................", "....HHHHHHH.....", "..HHHHHHHHHHH...", ".HHH.....SHHHH..", ".HH.....SS..HHH.", ".H.....SS....HH.", "......SS......H.", ".....SS.........",
            "....SS..........", "...SS...........", "..SS............", ".SS.............", ".S..............", "................", "................", "................",
        ],
        &[('H', 0xb8bcc4), ('S', 0x9a6b3a)],
    )
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
/// The light on water over a shore and over the deep, as red, green and
/// blue: shallow water is pale and green, deep water dark.
const SHALLOW: [f32; 3] = [1.3, 1.5, 1.02];
const ABYSS: [f32; 3] = [0.5, 0.6, 0.8];
/// Water runs these ways, and comes into a hole from these.
const RUNS: [[i32; 3]; 5] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]];
const POURS: [[i32; 3]; 5] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]];

fn at(x: i32, y: i32, z: i32) -> usize { ((x * SZ + z) * SY + y) as usize }
fn inside(x: i32, y: i32, z: i32) -> bool { (0..SX).contains(&x) && (0..SY).contains(&y) && (0..SZ).contains(&z) }

/// Where the world's one great mountain stands: off to one side of the
/// middle, a way the seed picks.
fn summit(seed: u32) -> (f32, f32) {
    let a = hash(0, 0, seed + 53) * TAU;
    (SX as f32 / 2.0 + a.cos() * 78.0, SZ as f32 / 2.0 + a.sin() * 78.0)
}

/// How high the land stands at a place: low hills everywhere, rocky
/// heights in the regions a coarse noise marks, and the great mountain,
/// which rises a block for each block toward its top, with ridges and
/// gullies down its sides.
fn ground(x: f32, z: f32, seed: u32) -> i32 {
    let wide = noise(x / 110.0, z / 110.0, 4096, seed + 20);
    let hills = fbm(x / 56.0, z / 56.0, 4, seed, 4096);
    let low = 10.0 + wide * 24.0 + hills * 24.0;
    let range = smooth(((noise(x / 110.0, z / 110.0, 4096, seed + 50) - 0.5) * 2.0).clamp(0.0, 1.0));
    let lesser = range * (14.0 + ridged(x / 80.0, z / 80.0, 4, seed + 51) * 36.0);
    let (mx, mz) = summit(seed);
    let cone = (1.0 - ((x - mx).powi(2) + (z - mz).powi(2)).sqrt() / PEAK_FOOT).max(0.0);
    let great = cone * (PEAK - low) + cone * (1.0 - cone) * (ridged(x / 40.0, z / 40.0, 3, seed + 54) - 0.5) * 60.0;
    ((low + lesser.max(great)) as i32).clamp(2, SY - 10)
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

    /// The block a point is in.
    fn around(&self, p: V3) -> u8 { self.get(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) }

    fn roof_at(&self, x: i32, z: i32) -> i32 { self.roof[(x * SZ + z) as usize] }

    /// Does the sky's light reach a place straight down?
    fn lit(&self, x: i32, y: i32, z: i32) -> bool {
        !(0..SX).contains(&x) || !(0..SZ).contains(&z) || y > self.roof_at(x, z)
    }

    fn find_roof(&self, x: i32, z: i32) -> i32 {
        (0..SY).rev().find(|&y| !matches!(self.cells[at(x, y, z)], AIR | GLASS | WATER)).unwrap_or(0)
    }

    fn roofs(&mut self) {
        for x in 0..SX {
            for z in 0..SZ { self.roof[(x * SZ + z) as usize] = self.find_roof(x, z); }
        }
    }

    /// Grow the land from a seed: stone under dirt and grass, sand by the
    /// sea and under it, bare stone and snow high up, tunnels through the
    /// stone, groves of trees, and water over all that lies low.
    fn grow(seed: u32) -> World {
        let mut w = World { seed, cells: vec![AIR; (SX * SY * SZ) as usize], roof: vec![0; (SX * SZ) as usize], changed: HashMap::new() };
        let mut sea = Vec::new();
        for x in 0..SX {
            for z in 0..SZ {
                let (fx, fz) = (x as f32, z as f32);
                let h = ground(fx, fz, seed);
                // A tunnel runs where one noise crosses its middle value,
                // at the height a second one gives, as wide as a third.
                let vein = (noise(fx / 44.0, fz / 44.0, 4096, seed + 31) - 0.5).abs() / 0.03;
                let mid = 6.0 + noise(fx / 60.0, fz / 60.0, 4096, seed + 32) * 34.0;
                let bore = if vein < 1.0 { (1.6 + noise(fx / 9.0, fz / 9.0, 4096, seed + 33) * 2.2) * (1.0 - vein * vein).sqrt() } else { 0.0 };
                let snowy = h + (hash(x, z, seed + 52) * 4.0) as i32 > SNOW_LINE;
                for y in 0..h {
                    w.cells[at(x, y, z)] = if y == 0 {
                        BEDROCK
                    // No tunnel opens in the sea's floor.
                    } else if (y as f32 + 0.5 - mid).abs() < bore && (h > SEA + 1 || y < h - 4) {
                        AIR
                    } else if y == h - 1 && snowy {
                        SNOW
                    } else if y < h - 4 || h > TREE_LINE {
                        STONE
                    } else if h <= SEA + 1 {
                        SAND
                    } else if y < h - 1 {
                        DIRT
                    } else {
                        GRASS
                    };
                }
                for y in h..SEA {
                    w.cells[at(x, y, z)] = WATER;
                    sea.push([x, y, z]);
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
        // The sea finds the tunnels that open beside it.
        w.flow(sea, false);
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

    /// Water runs from these cells sideways and down into open air, and
    /// on from there; never up. Gives the cells it filled.
    fn flow(&mut self, mut from: Vec<[i32; 3]>, record: bool) -> Vec<[i32; 3]> {
        let mut filled = Vec::new();
        while let Some(c) = from.pop() {
            for d in RUNS {
                let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                if !inside(n[0], n[1], n[2]) || self.cells[at(n[0], n[1], n[2])] != AIR { continue; }
                self.cells[at(n[0], n[1], n[2])] = WATER;
                if record { self.changed.insert(at(n[0], n[1], n[2]) as u32, WATER); }
                from.push(n);
                filled.push(n);
            }
        }
        filled
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
            if i as usize >= w.cells.len() || b > SNOW as u32 { return None; }
            w.cells[i as usize] = b as u8;
            w.changed.insert(i, b as u8);
        }
        w.roofs();
        Some(w)
    }

    /// Would a box with its feet at `p`, `half` wide from its middle and
    /// `tall` high, be in a block? Past the sides of the world it is.
    fn stuck(&self, p: V3, half: f32, tall: f32) -> bool {
        let span = |lo: f32, hi: f32| lo.floor() as i32..=hi.floor() as i32;
        span(p.x - half, p.x + half).any(|x| span(p.y, p.y + tall).any(|y| span(p.z - half, p.z + half).any(|z| solid(self.get(x, y, z)))))
    }

    /// How deep the water is under a place on its surface, eight blocks
    /// at most.
    fn depth(&self, x: i32, y: i32, z: i32) -> f32 { (0..8).take_while(|d| self.get(x, y - d, z) == WATER).count() as f32 }

    /// The faces that show in one chunk, lit corner by corner: a corner
    /// is darker where blocks crowd round it, and darker under a roof.
    fn mesh(&self, cx: i32, cz: i32) -> Model {
        let mut m = Model::new();
        for x in cx * CHUNK..(cx + 1) * CHUNK {
            for z in cz * CHUNK..(cz + 1) * CHUNK {
                let top = (0..SY).rev().find(|&y| self.cells[at(x, y, z)] != AIR).unwrap_or(0);
                for y in 0..=top {
                    let b = self.cells[at(x, y, z)];
                    if b == AIR { continue; }
                    // The top of water lies a little under its block's.
                    let dip = if b == WATER && self.get(x, y + 1, z) != WATER { 0.125 } else { 0.0 };
                    for (f, (n, corners, shade)) in FACES.iter().enumerate() {
                        let into = [x + n[0], y + n[1], z + n[2]];
                        let next = self.get(into[0], into[1], into[2]);
                        // Hidden by a block, or by more of the same water, leaves or glass.
                        if !clear(next) || next == b { continue; }
                        // The two directions that run along the face.
                        let (t1, t2) = if n[0] != 0 { (1, 2) } else if n[1] != 0 { (0, 2) } else { (0, 1) };
                        let mut v = [Vert::new(V3::default(), 0.0, 0.0, 0.0); 4];
                        for (i, c) in corners.iter().enumerate() {
                            let p = V3::new((x + c[0]) as f32, (y + c[1]) as f32 - dip * c[1] as f32, (z + c[2]) as f32);
                            if b == WATER {
                                // Water is lit by how deep it is round this corner.
                                let (px, pz) = (x + c[0], z + c[2]);
                                let deep = if f == 0 { [(-1, -1), (0, -1), (-1, 0), (0, 0)].iter().map(|d| self.depth(px + d.0, y, pz + d.1)).sum::<f32>() / 32.0 } else { 0.5 };
                                let day = if self.lit(x, y + 1, z) { 1.0 } else { 0.6 };
                                v[i] = Vert { p, u: UV[i].0, v: UV[i].1, lit: [0, 1, 2].map(|k: usize| (SHALLOW[k] + (ABYSS[k] - SHALLOW[k]) * deep) * day) };
                                continue;
                            }
                            // The place the face looks into, and the three
                            // beside it round this corner.
                            let mut near = [into; 4];
                            near[1][t1] += 2 * c[t1] - 1;
                            near[2][t2] += 2 * c[t2] - 1;
                            near[3][t1] += 2 * c[t1] - 1;
                            near[3][t2] += 2 * c[t2] - 1;
                            let open = near.map(|q| clear(self.get(q[0], q[1], q[2])));
                            let crowd = if !open[1] && !open[2] { 3 } else { open[1..].iter().filter(|o| !**o).count() };
                            let (mut day, mut count) = (0.0, 0.0);
                            for (q, o) in near.iter().zip(open) {
                                if !o { continue; }
                                count += 1.0;
                                if self.lit(q[0], q[1], q[2]) { day += 1.0; }
                            }
                            v[i] = Vert::new(p, UV[i].0, UV[i].1, shade * CROWD[crowd] * (SHADE + (1.0 - SHADE) * day / count));
                        }
                        // Cut the face along the diagonal that keeps one
                        // dark corner in a triangle of its own.
                        let mat = Mat::Tex(skin(b, f));
                        if v[0].lit[0] + v[2].lit[0] < v[1].lit[0] + v[3].lit[0] { m.quad(v[1], v[2], v[3], v[0], mat); } else { m.quad(v[0], v[1], v[2], v[3], mat); }
                        // The top of the water is seen from below as well.
                        if b == WATER && f == 0 { m.quad(v[3], v[2], v[1], v[0], mat); }
                    }
                }
            }
        }
        m.bound();
        m
    }
}

/// Would a block at `c` be in a box with its feet at `p`?
fn boxed(c: [i32; 3], p: V3, half: f32, tall: f32) -> bool {
    let c = c.map(|v| v as f32);
    c[0] < p.x + half && c[0] + 1.0 > p.x - half && c[1] < p.y + tall && c[1] + 1.0 > p.y && c[2] < p.z + half && c[2] + 1.0 > p.z - half
}

/// A box in the world: where its feet are, how fast it rises, and
/// whether it stands on something.
#[derive(Clone, Copy, Debug)]
struct Body { pos: V3, rise: f32, grounded: bool }

impl Body {
    /// Move for `dt` seconds: sideways by `step`, one direction at a
    /// time so that it slides along a wall, then up or down at its own
    /// speed. A landing puts its feet on top of the block it fell into.
    /// True when a wall stopped it.
    fn slide(&mut self, w: &World, step: V3, dt: f32, half: f32, tall: f32) -> bool {
        let mut bumped = false;
        for s in [V3::new(step.x, 0.0, 0.0), V3::new(0.0, 0.0, step.z)] {
            if s.x == 0.0 && s.z == 0.0 { continue; }
            if w.stuck(self.pos.add(s), half, tall) { bumped = true; } else { self.pos = self.pos.add(s); }
        }
        let up = self.pos.add(V3::new(0.0, self.rise * dt, 0.0));
        self.grounded = false;
        if !w.stuck(up, half, tall) {
            self.pos = up;
        } else {
            if self.rise < 0.0 {
                let top = V3::new(self.pos.x, up.y.floor() + 1.0, self.pos.z);
                if !w.stuck(top, half, tall) { self.pos = top; }
                self.grounded = true;
            }
            self.rise = 0.0;
        }
        bumped
    }
}

/// An angle turned toward another by `most` at most, the short way round.
fn turned(from: f32, to: f32, most: f32) -> f32 { from + ((to - from + PI).rem_euclid(TAU) - PI).clamp(-most, most) }

/// A cat's coat: its fur, its stripes if it has them, its muzzle and
/// its eyes.
const COATS: [(Rgb, Option<Rgb>, Rgb, Rgb); 5] = [
    (0xd9893a, Some(0xb2681f), 0xf4ead8, 0x8fd65e),
    (0x2c2c33, None, 0x2c2c33, 0xf2d24a),
    (0x8d929b, Some(0x696e77), 0xe8e8e8, 0x8fd65e),
    (0xf0efea, None, 0xf0efea, 0x6ab4e8),
    (0x7b5b3c, Some(0x553c26), 0xe6d8c0, 0xf2d24a),
];
/// Where a cat's legs join its body, the front pair first, and where its
/// head and its tail do: a cat stands at its own middle and looks along z.
const HIPS: [V3; 4] = [V3::new(-0.09, 0.2, 0.15), V3::new(0.09, 0.2, 0.15), V3::new(-0.09, 0.2, -0.23), V3::new(0.09, 0.2, -0.23)];
const NECK: V3 = V3::new(0.0, 0.4, 0.2);
const TAIL: V3 = V3::new(0.0, 0.38, -0.29);
/// How far a sitting cat's body leans back.
const SIT: f32 = 0.75;
/// A cat's hop clears one block, as the walker's jump does.
const HOP: f32 = 8.6;

/// A cat in four parts: the body, the head, one leg, and the tail. The
/// head sits on its own neck, the leg hangs from its own top and the
/// tail stands on its own root, so each turns about the place it is
/// joined at.
fn cat(coat: (Rgb, Option<Rgb>, Rgb, Rgb)) -> [Model; 4] {
    let (fur, stripes, muzzle, eyes) = coat;
    fn piece(m: &mut Model, lo: [f32; 3], hi: [f32; 3], c: Rgb) {
        m.extend(&Model::block(V3::new(lo[0], lo[1], lo[2]), V3::new(hi[0], hi[1], hi[2]), Mat::Flat(c), 1.0, 9.0), &M4::identity());
    }
    let mut body = Model::new();
    piece(&mut body, [-0.13, 0.2, -0.3], [0.13, 0.42, 0.22], fur);
    let mut head = Model::new();
    piece(&mut head, [-0.14, -0.04, -0.02], [0.14, 0.2, 0.24], fur);
    piece(&mut head, [-0.07, -0.02, 0.24], [0.07, 0.06, 0.27], muzzle);
    piece(&mut head, [-0.02, 0.04, 0.27], [0.02, 0.07, 0.28], 0xe89aa8);
    for side in [-1.0, 1.0] {
        let x = |a: f32, b: f32| if side < 0.0 { (-b, -a) } else { (a, b) };
        let (ear, eye, pupil) = (x(0.06, 0.14), x(0.04, 0.11), x(0.065, 0.085));
        piece(&mut head, [ear.0, 0.2, 0.06], [ear.1, 0.28, 0.14], fur);
        piece(&mut head, [eye.0, 0.09, 0.24], [eye.1, 0.15, 0.252], eyes);
        piece(&mut head, [pupil.0, 0.09, 0.252], [pupil.1, 0.15, 0.26], 0x101014);
    }
    if let Some(dark) = stripes {
        for z in [-0.22, -0.1, 0.02] { piece(&mut body, [-0.135, 0.3, z], [0.135, 0.425, z + 0.04], dark); }
    }
    let mut leg = Model::new();
    piece(&mut leg, [-0.04, -0.2, -0.045], [0.04, 0.0, 0.045], fur);
    let mut tail = Model::new();
    piece(&mut tail, [-0.03, 0.0, -0.03], [0.03, 0.32, 0.03], fur);
    let mut parts = [body, head, leg, tail];
    for m in &mut parts {
        // Light from above and a little from the front.
        m.relight(|_, n| [0.7 + 0.3 * n.y.max(0.0) - 0.18 * (-n.y).max(0.0) + 0.08 * n.z; 3]);
        m.bound();
    }
    parts
}

#[derive(Clone, Copy, Debug)]
struct Cat {
    body: Body,
    yaw: f32,
    /// The way it wanders when nobody is near.
    head: f32,
    coat: usize,
    sits: bool,
    moving: bool,
    /// Seconds left of this sit or this walk.
    left: f32,
    /// How far through a stride its legs are.
    stride: f32,
    /// Seconds until it may meow again.
    mew: f32,
}

impl Cat {
    fn at(pos: V3, coat: usize) -> Cat {
        Cat { body: Body { pos, rise: 0.0, grounded: false }, yaw: 0.0, head: 0.0, coat, sits: true, moving: false, left: 3.0, stride: 0.0, mew: 0.0 }
    }
}

fn draw_cat(scene: &mut Scene, parts: &[Model; 4], c: &Cat) {
    let place = M4::rotate_y(c.yaw).then(&M4::translate(c.body.pos));
    let [body, head, leg, tail] = parts;
    if c.sits {
        // Haunches on the ground and chest up: the body turns about its
        // hind end and sinks, and the head looks up a little. The front
        // legs stand straight under the chest, each two legs long, and
        // the tail lies round to one side.
        let pose = M4::translate(V3::new(0.0, -0.2, 0.25)).then(&M4::rotate_x(-SIT)).then(&M4::translate(V3::new(0.0, 0.06, -0.25)));
        scene.push(body, &pose.then(&place));
        scene.push(head, &M4::rotate_x(-0.15).then(&M4::translate(pose.apply(NECK))).then(&place));
        for hip in &HIPS[..2] {
            let top = pose.apply(*hip);
            for down in [0.0, 0.2] { scene.push(leg, &M4::translate(V3::new(top.x, top.y - down, top.z)).then(&place)); }
        }
        scene.push(tail, &M4::rotate_x(-1.45).then(&M4::rotate_y(0.9)).then(&M4::translate(V3::new(0.0, 0.07, -0.26))).then(&place));
        return;
    }
    scene.push(body, &place);
    scene.push(head, &M4::translate(NECK).then(&place));
    let swing = if c.moving { c.stride.sin() * 0.7 } else { 0.0 };
    for (i, hip) in HIPS.iter().enumerate() {
        // The two legs on a diagonal swing together.
        let a = if i == 0 || i == 3 { swing } else { -swing };
        scene.push(leg, &M4::rotate_x(a).then(&M4::translate(*hip)).then(&place));
    }
    scene.push(tail, &M4::rotate_x(-0.5 + (c.stride * 0.5).sin() * 0.15).then(&M4::translate(TAIL)).then(&place));
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Ask, Play, Craft }

struct Sounds { dig: Sample, build: Sample, meow: Sample, splash: Sample, made: Sample }

struct Blocks {
    world: World,
    scene: Scene,
    /// A mesh for each chunk, row by row.
    chunks: Vec<Model>,
    /// The walker, and where it looks.
    me: Body,
    yaw: f32,
    pitch: f32,
    was_wet: bool,
    /// Which of the `KINDS` is built, and how many of each are in the
    /// pockets.
    pick: usize,
    have: [u32; 10],
    pickaxe: bool,
    cats: Vec<Cat>,
    coats: Vec<[Model; 4]>,
    /// Seconds a look key has been down.
    looking: f32,
    /// Seconds until a held dig or build key acts again.
    again: f32,
    mode: Mode,
    /// The row of the list of things to make.
    row: usize,
    /// A line said to the player, and the seconds it stays.
    note: (String, f32),
    /// Is the world read from the store and written back? Not in a test
    /// or a bench.
    stored: bool,
    unsaved: bool,
    /// Seconds since the last change.
    since: f32,
    time: f32,
    /// The water's pictures, and which one it wears now.
    waves: Vec<Texture>,
    wave: usize,
    icons: Vec<Sprite>,
    faded: Vec<Sprite>,
    tool: Sprite,
    rng: Rng,
    audio: Audio,
    snd: Sounds,
}

impl Blocks {
    fn new(world: World) -> Blocks {
        let mut scene = Scene::new(W, H);
        scene.near = NEAR;
        let pics = pictures();
        let icons = KINDS.iter().map(|&(kind, _)| icon(&pics[skin(kind, 2)], 1.0)).collect();
        let faded = KINDS.iter().map(|&(kind, _)| icon(&pics[skin(kind, 2)], 0.45)).collect();
        for p in pics { scene.texture(p); }
        let snd = Sounds {
            dig: Sample::noise(0.07, 0.5),
            build: Sample::sweep(Wave::Triangle, 230.0, 90.0, 0.07, 0.6),
            meow: Sample::sweep(Wave::Triangle, 560.0, 920.0, 0.1, 0.5).then(&Sample::sweep(Wave::Triangle, 920.0, 600.0, 0.24, 0.5)),
            splash: Sample::noise(0.18, 0.4),
            made: Sample::sweep(Wave::Triangle, 520.0, 780.0, 0.09, 0.5),
        };
        let mut b = Blocks {
            world, scene, chunks: Vec::new(), me: Body { pos: V3::default(), rise: 0.0, grounded: false }, yaw: 0.0, pitch: 0.0, was_wet: false, pick: 0,
            have: [0; 10], pickaxe: false, cats: Vec::new(), coats: COATS.iter().map(|&c| cat(c)).collect(), looking: 0.0, again: 0.0, mode: Mode::Title,
            row: 0, note: (String::new(), 0.0), stored: false, unsaved: false, since: 0.0, time: 0.0, waves: (0..16).map(wave).collect(), wave: 0, icons,
            faded, tool: pickaxe(), rng: Rng::new(1), audio: Audio::off(), snd,
        };
        b.mesh_all();
        b.spawn();
        b.herd();
        b
    }

    fn mesh_all(&mut self) {
        self.chunks = (0..ACROSS * (SZ / CHUNK)).map(|i| self.world.mesh(i % ACROSS, i / ACROSS)).collect();
    }

    /// Stand on open ground as near the middle of the world as there is
    /// some: on a level lawn low down, or on any grass or beach, and not
    /// under a tree. Look the way the land lies most open.
    fn spawn(&mut self) {
        let w = &self.world;
        let top = |x: i32, z: i32| (w.get(x, w.roof_at(x, z), z), w.roof_at(x, z));
        let level = |x: i32, z: i32| [(2, 0), (-2, 0), (0, 2), (0, -2)].iter().all(|d| (w.roof_at(x + d.0, z + d.1) - w.roof_at(x, z)).abs() <= 1);
        let nearest = |fits: &dyn Fn(i32, i32) -> bool| {
            let places = (2..SX - 2).flat_map(|x| (2..SZ - 2).map(move |z| (x, z)));
            places.filter(|&(x, z)| fits(x, z)).min_by_key(|&(x, z)| (x - SX / 2).pow(2) + (z - SZ / 2).pow(2))
        };
        let lawn = nearest(&|x, z| top(x, z).0 == GRASS && top(x, z).1 < SEA + 12 && level(x, z));
        let (x, z) = lawn.or_else(|| nearest(&|x, z| matches!(top(x, z).0, GRASS | SAND) && top(x, z).1 >= SEA)).unwrap_or((SX / 2, SZ / 2));
        self.me = Body { pos: V3::new(x as f32 + 0.5, w.roof_at(x, z) as f32 + 1.0, z as f32 + 0.5), rise: 0.0, grounded: false };
        // The highest thing in the way over the next blocks, each way round.
        let rise = |a: f32| (2..24).map(|d| w.roof_at(((x as f32 + 0.5 + a.sin() * d as f32) as i32).clamp(0, SX - 1), ((z as f32 + 0.5 + a.cos() * d as f32) as i32).clamp(0, SZ - 1))).max();
        let yaw = (0..16).map(|i| i as f32 * TAU / 16.0).min_by_key(|&a| rise(a)).unwrap_or(0.0);
        (self.yaw, self.pitch) = (yaw, -0.1);
    }

    /// Cats on the grass round the walker: the same ones in the same
    /// places for the same world.
    fn herd(&mut self) {
        let mut rng = Rng::new(self.world.seed as u64 + 9);
        self.cats.clear();
        for _ in 0..600 {
            if self.cats.len() == CATS { break; }
            let (x, z) = ((self.me.pos.x + rng.range(-26.0, 26.0)) as i32, (self.me.pos.z + rng.range(-26.0, 26.0)) as i32);
            if !(1..SX - 1).contains(&x) || !(1..SZ - 1).contains(&z) { continue; }
            let y = self.world.roof_at(x, z);
            if self.world.get(x, y, z) != GRASS { continue; }
            let mut c = Cat::at(V3::new(x as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5), self.cats.len() % COATS.len());
            (c.yaw, c.left) = (rng.range(0.0, TAU), rng.range(1.0, 6.0));
            self.cats.push(c);
        }
    }

    /// A new world from a seed, in place of this one, with empty pockets.
    fn fresh(&mut self, seed: u32) {
        self.world = World::grow(seed);
        (self.have, self.pickaxe, self.pick) = ([0; 10], false, 0);
        self.mesh_all();
        self.spawn();
        self.herd();
        self.keep();
    }

    /// The walker as text: where it is, where it looks, and its pockets.
    fn walker_text(&self) -> String {
        let p = self.me.pos;
        let mut t = format!("{} {} {} {} {} {} {}", p.x, p.y, p.z, self.yaw, self.pitch, self.pick, self.pickaxe as u8);
        for n in self.have { t.push_str(&format!(" {n}")); }
        t
    }

    fn cats_text(&self) -> String { self.cats.iter().map(|c| format!("{} {} {} ", c.body.pos.x, c.body.pos.y, c.body.pos.z)).collect() }

    /// Write the world, the walker and the cats to the store.
    fn keep(&mut self) {
        self.unsaved = false;
        if !self.stored { return; }
        store::save(GAME, "world", &self.world.text());
        store::save(GAME, "walker", &self.walker_text());
        store::save(GAME, "cats", &self.cats_text());
    }

    /// Put the walker where `walker_text` wrote it was, if it fits there.
    fn stand(&mut self, t: &str) {
        let n: Vec<f32> = t.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        if n.len() != 17 || n.iter().any(|v| !v.is_finite() || *v < -1e6 || *v > 1e9) { return; }
        let p = V3::new(n[0], n[1], n[2]);
        if self.blocked(p) { return; }
        (self.me.pos, self.yaw, self.pitch, self.pick) = (p, n[3], n[4].clamp(-1.5, 1.5), (n[5].max(0.0) as usize).min(KINDS.len() - 1));
        self.pickaxe = n[6] > 0.0;
        for (have, v) in self.have.iter_mut().zip(&n[7..]) { *have = v.max(0.0) as u32; }
    }

    /// Put the cats where `cats_text` wrote they were. A cat whose place
    /// is in a block now stays where it grew.
    fn settle(&mut self, t: &str) {
        let n: Vec<f32> = t.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        if n.len() != self.cats.len() * 3 || n.iter().any(|v| !v.is_finite()) { return; }
        for (i, p) in n.chunks(3).map(|p| V3::new(p[0], p[1], p[2])).enumerate() {
            if !self.world.stuck(p, CAT_HALF, CAT_TALL) { self.cats[i].body.pos = p; }
        }
    }

    fn eye(&self) -> V3 { self.me.pos.add(V3::new(0.0, EYE, 0.0)) }

    /// The way the walker looks, a step long.
    fn gaze(&self) -> V3 { M4::rotate_x(-self.pitch).then(&M4::rotate_y(self.yaw)).apply_dir(V3::new(0.0, 0.0, 1.0)) }

    /// Would the walker, its feet at `p`, be in a block?
    fn blocked(&self, p: V3) -> bool { self.world.stuck(p, HALF, TALL) }

    /// Is the walker in water to the waist?
    fn wet(&self) -> bool { self.world.around(self.me.pos.add(V3::new(0.0, 0.4, 0.0))) == WATER }

    fn say(&mut self, line: String) { self.note = (line, 2.5); }

    /// Move the walker for `dt` seconds: `ahead` and `side` are -1 to 1.
    fn walk(&mut self, ahead: f32, side: f32, jump: bool, dt: f32) {
        let wet = self.wet();
        let (s, c) = self.yaw.sin_cos();
        let (mut dx, mut dz) = (s * ahead + c * side, c * ahead - s * side);
        let len = (dx * dx + dz * dz).sqrt();
        if len > 1.0 { dx /= len; dz /= len; }
        let speed = if wet { WALK * 0.6 } else { WALK };
        if wet {
            // Water carries: a slow sink, and Space swims up.
            self.me.rise = if jump { SWIM } else { (self.me.rise - PULL * 0.25 * dt).max(-2.5) };
        } else {
            if jump && self.me.grounded { self.me.rise = JUMP; }
            self.me.rise = (self.me.rise - PULL * dt).max(-24.0);
        }
        self.me.slide(&self.world, V3::new(dx * speed * dt, 0.0, dz * speed * dt), dt, HALF, TALL);
    }

    /// The block in the middle of the picture, if an arm reaches it, and
    /// which way the side that is looked at faces. Water is looked
    /// through.
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
            if !solid(self.world.get(c[0], c[1], c[2])) { continue; }
            let mut face = [0; 3];
            face[i] = -step[i];
            return inside(c[0], c[1], c[2]).then_some((c, face));
        }
    }

    /// Change a block. Water beside or over a hole runs into it and on.
    /// Then mesh again every chunk that was touched, and the next one
    /// where a change is on the edge, for the light it gives or takes.
    fn set(&mut self, c: [i32; 3], b: u8) {
        self.world.put(c, b);
        let mut touched = vec![c];
        if b == AIR && POURS.iter().any(|d| self.world.get(c[0] + d[0], c[1] + d[1], c[2] + d[2]) == WATER) {
            self.world.put(c, WATER);
            touched.extend(self.world.flow(vec![c], true));
        }
        let mut again = vec![false; self.chunks.len()];
        let span = |v: i32, most: i32| (v - 1).max(0) / CHUNK..=((v + 1) / CHUNK).min(most - 1);
        for t in &touched {
            for cz in span(t[2], SZ / CHUNK) {
                for cx in span(t[0], ACROSS) { again[(cz * ACROSS + cx) as usize] = true; }
            }
        }
        for i in (0..again.len()).filter(|&i| again[i]) { self.chunks[i] = self.world.mesh(i as i32 % ACROSS, i as i32 / ACROSS); }
        (self.unsaved, self.since) = (true, 0.0);
    }

    /// Dig the block in the frame into the pockets, or build on it out
    /// of them.
    fn act(&mut self, digs: bool) {
        let Some((c, face)) = self.aim() else { return };
        let b = self.world.get(c[0], c[1], c[2]);
        if digs {
            if b == BEDROCK { return; }
            if matches!(b, STONE | BRICK) && !self.pickaxe { return self.say("STONE NEEDS A PICKAXE: C MAKES ONE".into()); }
            self.set(c, AIR);
            if let Some(s) = slot(b) { self.have[s] += 1; }
            self.audio.play(&self.snd.dig, 0.5);
            return;
        }
        let to = [c[0] + face[0], c[1] + face[1], c[2] + face[2]];
        // Not past the world, not into a block, and not where the walker
        // or a cat is.
        if !inside(to[0], to[1], to[2]) || solid(self.world.get(to[0], to[1], to[2])) || boxed(to, self.me.pos, HALF, TALL) { return; }
        if self.cats.iter().any(|cat| boxed(to, cat.body.pos, CAT_HALF, CAT_TALL)) { return; }
        if self.have[self.pick] == 0 { return self.say(format!("NO {} LEFT: DIG SOME, OR C MAKES THINGS", KINDS[self.pick].1)); }
        self.have[self.pick] -= 1;
        self.set(to, KINDS[self.pick].0);
        self.audio.play(&self.snd.build, 0.6);
    }

    /// Is everything a recipe takes in the pockets? The pickaxe is made
    /// once.
    fn can(&self, i: usize) -> bool {
        let r = &RECIPES[i];
        (r.gives.is_some() || !self.pickaxe) && r.needs.iter().all(|&(kind, n)| slot(kind).is_some_and(|s| self.have[s] >= n))
    }

    /// Make a thing from what is in the pockets.
    fn make(&mut self, i: usize) -> bool {
        if !self.can(i) { return false; }
        for &(kind, n) in RECIPES[i].needs {
            if let Some(s) = slot(kind) { self.have[s] -= n; }
        }
        match RECIPES[i].gives.and_then(|(kind, n)| Some((slot(kind)?, n))) {
            Some((s, n)) => self.have[s] += n,
            None => self.pickaxe = true,
        }
        (self.unsaved, self.since) = (true, 0.0);
        self.audio.play(&self.snd.made, 0.6);
        true
    }

    /// The cats' lives: a cat comes to a walker who is near and sits by
    /// it, and wanders or sits when nobody is. It keeps out of water and
    /// away from long drops, hops up a step, and swims ashore if the
    /// water comes to it.
    fn cats(&mut self, dt: f32) {
        for i in 0..self.cats.len() {
            let mut c = self.cats[i];
            let to_me = V3::new(self.me.pos.x - c.body.pos.x, 0.0, self.me.pos.z - c.body.pos.z);
            let far = to_me.len();
            let near = far < 9.0 && (self.me.pos.y - c.body.pos.y).abs() < 4.0;
            c.mew -= dt;
            c.left -= dt;
            let (mut speed, mut want) = (0.0, c.yaw);
            if near {
                want = to_me.x.atan2(to_me.z);
                if far > 2.2 {
                    (speed, c.sits) = (2.6, false);
                } else {
                    if !c.sits && c.mew <= 0.0 {
                        self.audio.play(&self.snd.meow, 0.7);
                        c.mew = self.rng.range(6.0, 14.0);
                    }
                    c.sits = true;
                }
            } else {
                if c.left <= 0.0 {
                    c.sits = !c.sits;
                    c.left = if c.sits { self.rng.range(2.0, 7.0) } else { self.rng.range(1.0, 4.0) };
                    c.head = self.rng.range(0.0, TAU);
                }
                if !c.sits { (speed, want) = (1.5, c.head); }
            }
            // It turns on the spot until it looks where it is going.
            c.yaw = turned(c.yaw, want, 6.0 * dt);
            if (turned(c.yaw, want, 9.0) - c.yaw).abs() > 0.5 { speed = 0.0; }
            let dir = V3::new(c.yaw.sin(), 0.0, c.yaw.cos());
            // What lies a step ahead, at the height of its feet.
            let ahead = c.body.pos.add(dir.mul(0.7));
            let (x, y, z) = (ahead.x.floor() as i32, c.body.pos.y.floor() as i32, ahead.z.floor() as i32);
            let (low, high) = (solid(self.world.get(x, y, z)), solid(self.world.get(x, y + 1, z)));
            let wet = |up: f32| self.world.around(c.body.pos.add(V3::new(0.0, up, 0.0))) == WATER;
            let (afloat, swims) = (wet(0.2), wet(0.2) || wet(-0.3));
            // In water it floats, and climbs the bank it swims into.
            c.body.rise = if !afloat { (c.body.rise - PULL * dt).max(-24.0) } else if speed > 0.0 && low { SWIM } else { 2.0 };
            if !swims {
                // A wall two blocks high, water or a long drop turns a
                // wandering cat, and stops one that was coming.
                let fall = (1..5).take_while(|d| !solid(self.world.get(x, y - d, z))).count();
                let water = (0..4).any(|d| self.world.get(x, y - d, z) == WATER);
                if speed > 0.0 && ((low && high) || water || fall >= 4) {
                    speed = 0.0;
                    if near { c.sits = true; } else { c.head = c.yaw + PI + self.rng.range(-1.0, 1.0); }
                }
            }
            let bumped = c.body.slide(&self.world, dir.mul(speed * dt), dt, CAT_HALF, CAT_TALL);
            if bumped && c.body.grounded && low && !high { c.body.rise = HOP; }
            c.moving = speed > 0.0;
            if c.moving { c.stride += dt * speed * 5.0; }
            self.cats[i] = c;
        }
    }

    fn play(&mut self, input: &Input, dt: f32) {
        let held = |k: Key| input.held(k) as i32 as f32;
        for i in 0..KINDS.len() {
            if input.pressed(Key::Char((b'0' + ((i + 1) % 10) as u8) as char)) { self.pick = i; }
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
        let wet = self.wet();
        if wet && !self.was_wet { self.audio.play(&self.snd.splash, 0.6); }
        self.was_wet = wet;

        // A held key digs or builds again after a moment.
        self.again -= dt;
        let acts = |k: Key| input.pressed(k) || (input.held(k) && self.again <= 0.0);
        let (digs, builds) = (acts(Key::Char('f')), acts(Key::Char('e')));
        if digs || builds {
            self.again = 0.22;
            self.act(digs);
        }
        self.cats(dt);
        self.note.1 -= dt;
        self.since += dt;
        if self.unsaved && self.since > 5.0 { self.keep(); }
    }

    /// The list of things to make: Up and Down choose, Enter makes one.
    fn craft(&mut self, input: &Input) {
        let key = |c: char| input.pressed(Key::Char(c));
        let n = RECIPES.len();
        if input.pressed(Key::Up) || key('w') { self.row = (self.row + n - 1) % n; }
        if input.pressed(Key::Down) || key('s') { self.row = (self.row + 1) % n; }
        if input.pressed(Key::Enter) || input.pressed(Key::Space) || key('e') { self.make(self.row); }
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

    /// The ten kinds along the bottom with their keys above and how many
    /// are in the pockets below, the picked one framed in white, and the
    /// pickaxe beside them once it is made.
    fn bar(&self, f: &mut Frame) {
        let x0 = (W - KINDS.len() as i32 * 22) / 2;
        let y = H - 34;
        for i in 0..KINDS.len() {
            let x = x0 + i as i32 * 22;
            let (picked, n) = (i == self.pick, self.have[i]);
            f.rect(x, y, 20, 20, if picked { WHITE } else { 0x20242c });
            f.rect(x + 1, y + 1, 18, 18, 0x101218);
            f.blit(if n > 0 { &self.icons[i] } else { &self.faded[i] }, x + 2, y + 2);
            f.text(x + 8, y - 7, &((i + 1) % 10).to_string(), if picked { WHITE } else { 0x9098a8 });
            let count = n.min(999).to_string();
            f.text(x + 10 - count.len() as i32 * 2, y + 22, &count, if n > 0 { WHITE } else { 0x596070 });
        }
        if self.pickaxe {
            f.rect(x0 - 26, y, 20, 20, 0x20242c);
            f.rect(x0 - 25, y + 1, 18, 18, 0x101218);
            f.blit(&self.tool, x0 - 24, y + 2);
        }
        f.text_centered(W / 2, y - 17, KINDS[self.pick].1, WHITE, false, 1);
        if self.note.1 > 0.0 { f.text_centered(W / 2, y - 40, &self.note.0, 0xffe08a, true, 1); }
    }

    /// The list of things to make, over the darkened picture.
    fn list(&self, f: &mut Frame) {
        f.dim(0.4);
        let (x, y, w) = (150, 60, 340);
        let rows = RECIPES.len() as i32 * 40;
        f.rect(x, y, w, 62 + rows, 0x3a4252);
        f.rect(x + 1, y + 1, w - 2, 60 + rows, 0x12151c);
        f.text_centered(W / 2, y + 10, "THINGS TO MAKE", WHITE, true, 2);
        for (i, r) in RECIPES.iter().enumerate() {
            let ry = y + 36 + i as i32 * 40;
            let can = self.can(i);
            if i == self.row { f.rect(x + 6, ry - 3, w - 12, 36, 0x26303f); }
            let ink = if can { WHITE } else { 0x6a7080 };
            match r.gives.and_then(|(kind, n)| Some((slot(kind)?, n))) {
                Some((s, n)) => {
                    f.blit_scaled(if can { &self.icons[s] } else { &self.faded[s] }, x + 14, ry - 1, 2, false);
                    f.text_centered(W / 2 - 50, ry + 2, &format!("{n} {}", r.name), ink, true, 1);
                    f.text(x + w - 70, ry + 4, &format!("YOU HAVE {}", self.have[s]), 0x9098a8);
                }
                None => {
                    f.blit_scaled(&self.tool, x + 14, ry - 1, 2, false);
                    f.text_centered(W / 2 - 50, ry + 2, r.name, ink, true, 1);
                    f.text(x + w - 70, ry + 4, if self.pickaxe { "YOU HAVE IT" } else { "DIGS STONE" }, 0x9098a8);
                }
            }
            let needs: Vec<String> = r.needs.iter().map(|&(kind, n)| format!("{n} {}", slot(kind).map_or("", |s| KINDS[s].1))).collect();
            f.text_centered(W / 2 - 50, ry + 18, &format!("TAKES {}", needs.join(" AND ")), if can { 0x70e0d0 } else { 0x596070 }, false, 1);
        }
        f.text_centered(W / 2, y + 44 + rows, "UP AND DOWN CHOOSE   ENTER MAKES ONE   C CLOSES", 0xd0d4e0, false, 1);
    }
}

/// The sky along a line of sight: blue overhead and pale at the horizon,
/// the sun, and flat clouds that drift and thin out far away.
fn sky(d: V3, eye: V3, time: f32) -> Rgb {
    let back = if d.dot(SUN) > 0.9985 { 0xfff6d8 } else { mix(HAZE, ZENITH, (d.y.max(0.0) * 1.7).min(1.0)) };
    if d.y < 0.03 { return back; }
    // Where this line of sight meets the layer the clouds lie in.
    let far = (CLOUDS - eye.y) / d.y;
    let (x, z) = (eye.x + d.x * far + time * DRIFT, eye.z + d.z * far);
    let (cx, cz) = ((x / PUFF).floor() as i32, (z / PUFF).floor() as i32);
    if noise(cx as f32 / 5.0, cz as f32 / 5.0, 4096, 60) < 0.56 { return back; }
    mix(tint(WHITE, 0.93 + hash(cx, cz, 61) * 0.07), back, (far / 1400.0).min(1.0))
}

impl Game for Blocks {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        self.time += dt;
        let key = |c: char| input.pressed(Key::Char(c));
        // In the list, Q and Escape close the list and do not quit.
        if self.mode == Mode::Craft {
            if key('c') || key('q') || input.pressed(Key::Escape) { self.mode = Mode::Play; } else { self.craft(input); }
            return Flow::Continue;
        }
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
            Mode::Play if key('p') => {
                self.keep();
                return Flow::Pause;
            }
            Mode::Play if key('c') => self.mode = Mode::Craft,
            Mode::Play => self.play(input, dt),
            Mode::Craft => {}
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        let wave = (self.time * 6.0) as usize % self.waves.len();
        if wave != self.wave {
            self.scene.textures[WAVES] = self.waves[wave].clone();
            self.wave = wave;
        }
        // The title looks slowly round from where the walker stands.
        let yaw = if self.mode == Mode::Title || self.mode == Mode::Ask { self.yaw + self.time * 0.12 } else { self.yaw };
        let cam = Cam3 { pos: self.eye(), yaw, pitch: self.pitch, focal: FOCAL };
        // Under water the world is near and blue.
        let under = self.world.around(cam.pos.add(V3::new(0.0, 0.125, 0.0))) == WATER;
        self.scene.fog = Some(if under { (DEEP, 9.0) } else { (HAZE, FOG) });
        self.scene.begin(&cam);
        let still = M4::identity();
        for (i, chunk) in self.chunks.iter().enumerate() {
            let (cx, cz) = (i as i32 % ACROSS, i as i32 / ACROSS);
            let (dx, dz) = ((cx * CHUNK + CHUNK / 2) as f32 - cam.pos.x, (cz * CHUNK + CHUNK / 2) as f32 - cam.pos.z);
            if dx * dx + dz * dz < (FAR + CHUNK as f32) * (FAR + CHUNK as f32) { self.scene.push(chunk, &still); }
        }
        for c in &self.cats {
            if c.body.pos.sub(cam.pos).len() < 70.0 { draw_cat(&mut self.scene, &self.coats[c.coat], c); }
        }
        let (eye, time) = (cam.pos, self.time);
        // No sky is seen from under water, past the world's edge either.
        self.scene.render(f, &move |d| if under { DEEP } else { sky(d, eye, time) });
        if under {
            for p in f.px.iter_mut() { *p = mix(*p, DEEP, 0.4); }
        }

        if self.mode == Mode::Play || self.mode == Mode::Craft {
            self.frame_aim(f, &cam);
            f.rect(W / 2 - 4, H / 2, 9, 1, WHITE);
            f.rect(W / 2, H / 2 - 4, 1, 9, WHITE);
            self.bar(f);
            if self.mode == Mode::Craft { self.list(f); }
            return;
        }
        f.dim(0.55);
        let cx = W / 2;
        if self.mode == Mode::Ask {
            f.text_centered(cx, 170, "A NEW WORLD?", WHITE, true, 2);
            f.text_centered(cx, 200, "THE ONE YOU HAVE IS LOST, AND ALL IN YOUR POCKETS.", 0xd0d4e0, true, 1);
            f.text_centered(cx, 226, "Y MAKES IT   ANY OTHER KEY KEEPS THE OLD", 0x70e0d0, true, 1);
            return;
        }
        f.fancy_text(cx, 70, "BLOCKS", 8, self.time, (0x9be06a, 0x4a9a3c));
        f.text_centered(cx, 168, "W A S D WALK   ARROWS LOOK   SPACE JUMPS AND SWIMS", 0xd0d4e0, true, 1);
        f.text_centered(cx, 184, "F DIGS   E BUILDS   1-9 AND 0 PICK A BLOCK", 0xd0d4e0, true, 1);
        f.text_centered(cx, 200, "C MAKES THINGS   P PAUSES   Q QUITS", 0xd0d4e0, true, 1);
        f.text_centered(cx, 250, "SPACE: PLAY", 0x70e0d0, true, 2);
        f.text_centered(cx, 286, "N: A NEW WORLD", 0x9098a8, true, 1);
        f.text(W - 18, H - 9, VERSION, 0x9098a8);
    }
}

/// The game with the world from the store, or a new one.
fn blocks() -> Blocks {
    let text = store::load(GAME, "world");
    let world = text.as_deref().and_then(World::from_text);
    // A world that cannot be read here, one from an older version, is
    // not thrown away: it stays beside the new one.
    if let (None, Some(old)) = (&world, &text) {
        if !old.trim().is_empty() { store::save(GAME, "world-old", old); }
    }
    let new = world.is_none();
    let mut b = Blocks::new(world.unwrap_or_else(|| World::grow(Rng::from_time().next_u32())));
    b.stored = true;
    b.unsaved = new;
    if !new {
        if let Some(t) = store::load(GAME, "walker") { b.stand(&t); }
        if let Some(t) = store::load(GAME, "cats") { b.settle(&t); }
    }
    b.rng = Rng::from_time();
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
        if let Some(n) = std::env::var("BLOCKS_THREADS").ok().and_then(|n| n.parse().ok()) { game.scene.threads = n; }
        let (x, z, top) = (game.me.pos.x as i32, game.me.pos.z as i32, (game.me.pos.y as i32 + 30).min(SY));
        for y in game.me.pos.y as i32..top { game.set([x, y, z], STONE); }
        game.me.pos.y = top as f32;
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
    /// middle, looking along z, with full pockets and a pickaxe.
    fn on_flat() -> Blocks {
        let mut b = Blocks::new(flat(10));
        b.mode = Mode::Play;
        b.me.pos = V3::new(100.5, 10.0, 100.5);
        (b.yaw, b.pitch) = (0.0, 0.0);
        (b.have, b.pickaxe) = ([99; 10], true);
        b
    }

    /// The same with a pool in the stone: four blocks deep, from 98 to
    /// 102 both ways, its water level with the ground.
    fn with_pool() -> Blocks {
        let mut b = on_flat();
        for x in 98..103 {
            for z in 98..103 {
                for y in 6..10 { b.world.cells[at(x, y, z)] = WATER; }
            }
        }
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

    fn tick(b: &mut Blocks, key: Key) -> Flow { b.update(&pressed(key), 1.0 / 30.0) }

    fn run_for(b: &mut Blocks, input: &Input, secs: f32) {
        for _ in 0..(secs * 30.0) as usize { b.update(input, 1.0 / 30.0); }
    }

    fn count(w: &World, kind: u8) -> usize { w.cells.iter().filter(|&&k| k == kind).count() }

    fn columns() -> impl Iterator<Item = (i32, i32)> { (0..SX).flat_map(|x| (0..SZ).map(move |z| (x, z))) }

    /// The highest block of a column that is not air.
    fn top(w: &World, x: i32, z: i32) -> i32 { (0..SY).rev().find(|&y| w.get(x, y, z) != AIR).unwrap() }

    /// `BLOCKS_MAP=<file> cargo test --release --example blocks -- --ignored map`
    /// writes the land of `BLOCKS_SEED` as seen from above, and counts it.
    #[test]
    #[ignore]
    fn a_map_of_the_land() {
        let Ok(path) = std::env::var("BLOCKS_MAP") else { return };
        let seed = std::env::var("BLOCKS_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
        let w = World::grow(seed);
        let mut f = Frame::new(SX, SZ);
        let (mut sea, mut high) = (0, 0);
        for (x, z) in columns() {
            let y = top(&w, x, z);
            let k = 0.5 + y as f32 / SY as f32;
            if w.get(x, y, z) == WATER { sea += 1; }
            high = high.max(y);
            f.rect(x, z, 1, 1, match w.get(x, y, z) {
                WATER => tint(0x3a78c8, 1.2 - w.depth(x, y, z) / 12.0),
                SAND => 0xdccb8c,
                GRASS => tint(0x5fa845, k),
                SNOW => 0xf1f4f8,
                LEAVES | LOG => 0x2f6f2a,
                _ => tint(0x868a8e, k),
            });
        }
        let b = Blocks::new(w);
        f.rect(b.me.pos.x as i32 - 1, b.me.pos.z as i32 - 1, 3, 3, 0xff2020);
        for c in &b.cats { f.rect(c.body.pos.x as i32, c.body.pos.z as i32, 2, 2, 0xff9020); }
        eprintln!("seed {seed}: sea {} %, highest {high}, snow {}, flooded {}, cats {}", sea * 100 / (SX * SZ), count(&b.world, SNOW), count(&b.world, WATER), b.cats.len());
        std::fs::write(&path, f.to_ppm()).unwrap();

        // Views from inside it, beside the map: `<file>-<name>.ppm`.
        let mut b = b;
        b.mode = Mode::Play;
        let mut f = Frame::new(W, H);
        let mut shot = |b: &mut Blocks, name: &str| {
            b.draw(&mut f);
            std::fs::write(format!("{path}-{name}.ppm"), f.to_ppm()).unwrap();
        };
        let home = b.me.pos;
        // Two cats before the walker: one sitting and looking at it, one
        // in mid stride and seen from the side.
        for (dx, dz) in (-4..5).flat_map(|dx| (-1..7).map(move |dz| (dx, dz))) {
            let (x, y, z) = (home.x as i32 + dx, home.y as i32, home.z as i32 + dz);
            b.world.put([x, y - 1, z], GRASS);
            for up in 0..8 { b.world.put([x, y + up, z], AIR); }
        }
        b.mesh_all();
        let stand = |dx: f32, dz: f32| V3::new(home.x + dx, home.y, home.z + dz);
        let (mut sitter, mut strider) = (Cat::at(stand(-0.6, 2.2), 0), Cat::at(stand(0.8, 2.6), 2));
        sitter.yaw = PI;
        (strider.sits, strider.moving, strider.yaw, strider.stride) = (false, true, -1.2, 1.1);
        b.cats = vec![sitter, strider];
        b.pitch = -0.42;
        shot(&mut b, "cats");
        for (i, c) in b.cats.iter_mut().enumerate() { (c.coat, c.yaw) = (i * 2 + 1, c.yaw + 0.9); }
        shot(&mut b, "cats2");
        // The list of things to make, with something in the pockets.
        (b.have, b.mode, b.row) = ([0, 4, 0, 2, 3, 12, 0, 0, 0, 0], Mode::Craft, 1);
        shot(&mut b, "list");
        (b.mode, b.pickaxe, b.pick) = (Mode::Play, true, 4);
        b.say("STONE NEEDS A PICKAXE: C MAKES ONE".into());
        // On a shore, looking out over the water, and then under it.
        let sand = columns().filter(|&(x, z)| (2..SX - 2).contains(&x) && (2..SZ - 2).contains(&z)).find(|&(x, z)| top(&b.world, x, z) == SEA && (3..9).all(|d| b.world.get(x + d, SEA - 1, z) == WATER));
        if let Some((x, z)) = sand {
            (b.me.pos, b.yaw, b.pitch) = (V3::new(x as f32 + 0.5, SEA as f32 + 1.0, z as f32 + 0.5), PI / 2.0, -0.25);
            shot(&mut b, "shore");
        }
        if let Some((x, z)) = columns().find(|&(x, z)| b.world.depth(x, SEA - 1, z) > 5.0) {
            (b.me.pos, b.pitch) = (V3::new(x as f32 + 0.5, b.world.roof_at(x, z) as f32 + 1.0, z as f32 + 0.5), 0.2);
            for (i, yaw) in [0.0, PI / 2.0, PI, -PI / 2.0].into_iter().enumerate() {
                b.yaw = yaw;
                shot(&mut b, &format!("under{i}"));
            }
        }
        // Round from the start, two cats before the walker each time.
        // docs/img/blocks.png is `round0` of seed 5.
        for i in 0..8 {
            let yaw = i as f32 * TAU / 8.0;
            let before = |side: f32, out: f32| {
                let (x, z) = (home.x + yaw.sin() * out + yaw.cos() * side, home.z + yaw.cos() * out - yaw.sin() * side);
                V3::new(x, top(&b.world, x as i32, z as i32) as f32 + 1.0, z)
            };
            let (mut one, mut two) = (Cat::at(before(-0.9, 4.2), 0), Cat::at(before(1.1, 5.2), 2));
            (one.yaw, two.yaw, two.sits, two.moving, two.stride) = (yaw + PI - 0.3, yaw - 1.9, false, true, 1.1);
            b.cats = vec![one, two];
            (b.me.pos, b.yaw, b.pitch, b.pickaxe, b.note.1) = (home, yaw, -0.17, false, 0.0);
            shot(&mut b, &format!("round{i}"));
        }
        // The great mountain from the middle of the world, and the view
        // from its top.
        let (mx, mz) = summit(seed);
        (b.me.pos, b.yaw, b.pitch) = (V3::new(128.5, top(&b.world, 128, 128) as f32 + 14.0, 128.5), (mx - 128.0).atan2(mz - 128.0), 0.1);
        shot(&mut b, "peak");
        (b.me.pos, b.yaw, b.pitch) = (V3::new(mx, top(&b.world, mx as i32, mz as i32) as f32 + 1.0, mz), (128.0 - mx).atan2(128.0 - mz), -0.35);
        shot(&mut b, "top");
    }

    #[test]
    fn a_seed_grows_the_same_world_every_time() {
        let (a, b, c) = (World::grow(5), World::grow(5), World::grow(6));
        assert!(a.cells == b.cells && a.cells != c.cells);
        // Land, tunnels in it and trees on it.
        assert!(count(&a, GRASS) > 15_000 && count(&a, SAND) > 1000 && count(&a, LOG) > 100 && count(&a, LEAVES) > 1000, "{} {} {} {}", count(&a, GRASS), count(&a, SAND), count(&a, LOG), count(&a, LEAVES));
        let holes = columns().filter(|&(x, z)| (1..a.roof_at(x, z)).any(|y| a.get(x, y, z) == AIR && a.get(x, y + 1, z) == STONE)).count();
        assert!(holes > 500, "{holes} columns with a tunnel");
        // The floor is whole, and the top layer is free to build in.
        assert!(columns().all(|(x, z)| a.get(x, 0, z) == BEDROCK && a.get(x, SY - 1, z) == AIR));
    }

    #[test]
    fn the_sea_lies_level_and_has_run_where_it_can() {
        for seed in [5, 6] {
            let w = World::grow(seed);
            let sea = columns().filter(|&(x, z)| w.get(x, SEA - 1, z) == WATER).count();
            assert!(sea > 2000 && sea < 40_000, "seed {seed}: {sea} columns of sea");
            for (x, z) in columns() {
                for y in 1..SY {
                    if w.get(x, y, z) != WATER { continue; }
                    assert!(y < SEA, "water above the sea at {x} {y} {z}");
                    // No air beside water or under it: it would have run there.
                    for d in RUNS { assert!(w.get(x + d[0], y + d[1], z + d[2]) != AIR, "seed {seed}: water has not run on from {x} {y} {z}"); }
                }
            }
        }
    }

    #[test]
    fn mountains_stand_bare_with_snow_on_top() {
        for seed in [5, 6, 7] {
            let w = World::grow(seed);
            let high = columns().map(|(x, z)| top(&w, x, z)).max().unwrap();
            assert!(high > SNOW_LINE + 6 && high < SY - 8, "seed {seed}: the highest land is at {high}");
            // Snow lies on the heights and nowhere low; trees stay below.
            assert!(count(&w, SNOW) > 300, "seed {seed}: {} blocks of snow", count(&w, SNOW));
            for (x, z) in columns() {
                let y = top(&w, x, z);
                if y > SNOW_LINE + 4 { assert_eq!(w.get(x, y, z), SNOW, "bare top at {x} {y} {z}"); }
                if w.get(x, y, z) == SNOW { assert!(y >= SNOW_LINE - 4 && w.get(x, y - 1, z) == STONE); }
                assert!((TREE_LINE + 8..SY).all(|y| !matches!(w.get(x, y, z), LOG | LEAVES)), "a tree high up at {x} {z}");
            }
        }
    }

    #[test]
    fn the_walker_falls_to_the_ground_and_stands_on_it() {
        let mut b = on_flat();
        b.me.pos.y = 14.3;
        run_for(&mut b, &Input::new(), 2.0);
        assert_eq!((b.me.pos.y, b.me.grounded), (10.0, true));
        // A long fall does not pass through the floor either.
        b.me.pos.y = 60.0;
        run_for(&mut b, &Input::new(), 6.0);
        assert_eq!((b.me.pos.y, b.me.grounded), (10.0, true));
    }

    #[test]
    fn a_wall_stops_the_walker_and_it_slides_along() {
        let mut b = on_flat();
        for x in 90..110 {
            for y in 10..13 { b.set([x, y, 104], STONE); }
        }
        run_for(&mut b, &held(&[Key::Char('w')]), 3.0);
        assert!(b.me.pos.z > 103.5 && b.me.pos.z <= 104.0 - HALF, "z {}", b.me.pos.z);
        // Walking into it at a slant still moves the walker along it.
        b.yaw = 0.6;
        let x = b.me.pos.x;
        run_for(&mut b, &held(&[Key::Char('w')]), 1.0);
        assert!(b.me.pos.x > x + 1.0 && b.me.pos.z <= 104.0 - HALF);
    }

    #[test]
    fn a_jump_clears_one_block_and_not_two() {
        let mut b = on_flat();
        b.set([100, 10, 103], STONE);
        run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 1.5);
        assert!(b.me.pos.z > 103.0 && b.me.pos.y >= 11.0, "over one block: {:?}", b.me.pos);

        let mut b = on_flat();
        for y in 10..12 { b.set([100, y, 103], STONE); }
        run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 1.5);
        assert!(b.me.pos.z < 103.0 - HALF + 0.01, "not over two: {:?}", b.me.pos);
    }

    #[test]
    fn nobody_walks_out_of_the_world() {
        let mut b = on_flat();
        b.me.pos = V3::new(2.5, 10.0, 2.5);
        b.yaw = -2.4;
        run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 4.0);
        assert!(b.me.pos.x >= HALF && b.me.pos.z >= HALF && b.me.pos.y >= 10.0, "{:?}", b.me.pos);
        // Nor over the top of it, from a tower at the edge.
        for y in 10..SY { b.world.put([1, y, 1], STONE); }
        b.me.pos = V3::new(1.5, SY as f32, 1.5);
        run_for(&mut b, &held(&[Key::Char('w')]), 2.0);
        assert!(b.me.pos.x >= HALF && b.me.pos.z >= HALF, "{:?}", b.me.pos);
    }

    #[test]
    fn f_digs_the_block_in_the_frame_and_e_builds_on_it() {
        let mut b = on_flat();
        // Looking down at the ground ahead.
        b.pitch = -0.9;
        let (c, face) = b.aim().expect("the ground is within reach");
        assert_eq!((c, face), ([100, 9, 101], [0, 1, 0]));
        tick(&mut b, Key::Char('8'));
        tick(&mut b, Key::Char('e'));
        assert_eq!((b.world.get(100, 10, 101), b.have[7]), (BRICK, 98));
        // Now the brick is in the frame; F takes it, and then the stone.
        assert_eq!(b.aim().unwrap().0, [100, 10, 101]);
        tick(&mut b, Key::Char('f'));
        assert_eq!((b.world.get(100, 10, 101), b.have[7]), (AIR, 99));
        tick(&mut b, Key::Char('f'));
        assert_eq!((b.world.get(100, 9, 101), b.have[2]), (AIR, 100));
        assert!(b.unsaved && b.world.changed.len() == 2);
        // 0 picks the tenth kind.
        tick(&mut b, Key::Char('0'));
        assert_eq!(KINDS[b.pick].0, SNOW);

        // A held key digs on: straight down to the floor, and no further.
        let mut b = on_flat();
        b.pitch = -1.5;
        run_for(&mut b, &held(&[Key::Char('f')]), 6.0);
        assert_eq!((b.me.pos.y, b.world.get(100, 0, 100), b.have[2]), (1.0, BEDROCK, 108));
        // The sky is out of reach, and so is ground far ahead.
        let mut b = on_flat();
        b.pitch = 1.0;
        assert_eq!(b.aim(), None);
        b.pitch = -0.2;
        assert_eq!(b.aim(), None);
    }

    #[test]
    fn no_block_is_built_where_the_walker_or_a_cat_is() {
        let mut b = on_flat();
        // A wall right in front; building on its near side would fill
        // the walker's own place.
        for y in 10..12 { b.set([100, y, 101], STONE); }
        b.me.pos.z = 100.69;
        let before = (b.world.changed.len(), b.have);
        tick(&mut b, Key::Char('e'));
        assert_eq!((b.world.changed.len(), b.have), before);
        assert!(!b.blocked(b.me.pos));

        // A cat sits on the block that is looked at: nothing is built on it.
        let mut b = on_flat();
        b.pitch = -0.9;
        b.cats.push(Cat::at(V3::new(100.5, 10.0, 101.5), 0));
        tick(&mut b, Key::Char('e'));
        assert_eq!(b.world.get(100, 10, 101), AIR);
    }

    #[test]
    fn what_is_dug_goes_in_the_pockets_and_building_takes_it_out() {
        let mut b = on_flat();
        (b.have, b.pickaxe) = ([0; 10], false);
        b.pitch = -0.9;
        // Nothing to build with, and stone wants a pickaxe.
        tick(&mut b, Key::Char('e'));
        assert!(b.world.changed.is_empty() && b.note.0.starts_with("NO GRASS LEFT") && b.note.1 > 0.0);
        tick(&mut b, Key::Char('f'));
        assert!(b.world.changed.is_empty() && b.note.0.starts_with("STONE NEEDS A PICKAXE"));
        // Dirt comes up by hand, and one block of it can be put back.
        b.world.put([100, 9, 101], DIRT);
        tick(&mut b, Key::Char('f'));
        assert_eq!((b.world.get(100, 9, 101), b.have[1]), (AIR, 1));
        tick(&mut b, Key::Char('2'));
        tick(&mut b, Key::Char('e'));
        assert_eq!((b.world.get(100, 9, 101), b.have[1]), (DIRT, 0));
        tick(&mut b, Key::Char('e'));
        assert_eq!(b.world.get(100, 10, 101), AIR);
    }

    #[test]
    fn things_are_made_from_what_is_in_the_pockets() {
        let mut b = on_flat();
        (b.have, b.pickaxe) = ([0; 10], false);
        let n = |kind: u8| slot(kind).unwrap();
        assert!(RECIPES.iter().all(|r| r.needs.iter().all(|k| slot(k.0).is_some()) && r.gives.is_none_or(|g| slot(g.0).is_some())));
        for i in 0..RECIPES.len() { assert!(!b.can(i) && !b.make(i)); }
        // One log is four planks; three of them and a second log are a pickaxe.
        b.have[n(LOG)] = 2;
        assert!(b.make(0) && b.have[n(PLANKS)] == 4 && b.have[n(LOG)] == 1);
        assert!(b.make(1) && b.pickaxe && b.have[n(PLANKS)] == 1 && b.have[n(LOG)] == 0);
        // It is made once, and it digs stone.
        (b.have[n(LOG)], b.have[n(PLANKS)]) = (5, 5);
        assert!(!b.can(1) && !b.make(1));
        b.pitch = -0.9;
        tick(&mut b, Key::Char('f'));
        assert_eq!((b.world.get(100, 9, 101), b.have[n(STONE)]), (AIR, 1));
        // Stone and sand are two bricks; sand alone is glass.
        b.have[n(SAND)] = 2;
        assert!(b.make(3) && b.have[n(BRICK)] == 2 && b.have[n(STONE)] == 0);
        assert!(!b.make(3) && b.make(2) && b.have[n(GLASS)] == 1 && b.have[n(SAND)] == 0);
        // Dirt and leaves are grass.
        (b.have[n(DIRT)], b.have[n(LEAVES)]) = (1, 1);
        assert!(b.make(4) && b.have[n(GRASS)] == 1 && b.have[n(DIRT)] == 0 && b.have[n(LEAVES)] == 0);
    }

    #[test]
    fn c_opens_the_list_and_its_keys_stay_in_it() {
        let mut b = on_flat();
        b.have = [0; 10];
        b.have[slot(LOG).unwrap()] = 1;
        assert!(tick(&mut b, Key::Char('c')) == Flow::Continue && b.mode == Mode::Craft);
        // Enter makes the row that is chosen; nothing moves meanwhile.
        let at = b.me.pos;
        tick(&mut b, Key::Enter);
        assert_eq!((b.have[slot(PLANKS).unwrap()], b.me.pos), (4, at));
        tick(&mut b, Key::Down);
        tick(&mut b, Key::Down);
        assert_eq!(b.row, 2);
        for _ in 0..3 { tick(&mut b, Key::Up); }
        assert_eq!(b.row, RECIPES.len() - 1);
        // Escape and Q close the list; they do not leave the game.
        assert!(tick(&mut b, Key::Escape) == Flow::Continue && b.mode == Mode::Play);
        tick(&mut b, Key::Char('c'));
        assert!(tick(&mut b, Key::Char('q')) == Flow::Continue && b.mode == Mode::Play);
        assert_eq!(tick(&mut b, Key::Char('q')), Flow::Quit);
    }

    #[test]
    fn water_runs_into_a_hole_beside_it_and_never_up() {
        let mut b = with_pool();
        // A trench from the pool's edge: each block dug fills.
        for z in 103..107 { b.set([100, 9, z], AIR); }
        assert!((103..107).all(|z| b.world.get(100, 9, z) == WATER));
        // A shaft under its far end fills too; a hole beside it that only
        // touches a corner stays dry, and so does the air above.
        b.set([100, 8, 106], AIR);
        b.set([100, 7, 106], AIR);
        b.set([101, 9, 107], AIR);
        assert_eq!((b.world.get(100, 7, 106), b.world.get(101, 9, 107), b.world.get(100, 10, 105)), (WATER, AIR, AIR));
        // A dry cave under a dry tunnel: opening the tunnel to the trench
        // floods all of both.
        for x in 110..114 { b.world.cells[at(x, 7, 105)] = AIR; }
        for x in 101..111 { b.set([x, 8, 105], AIR); }
        assert!((101..111).all(|x| b.world.get(x, 8, 105) == AIR));
        b.set([101, 9, 105], AIR);
        assert!((110..114).all(|x| b.world.get(x, 7, 105) == WATER) && (101..111).all(|x| b.world.get(x, 8, 105) == WATER));
        // All of it is remembered, and a block built in water takes its place.
        assert!((110..114).all(|x| b.world.changed.get(&(at(x, 7, 105) as u32)) == Some(&WATER)));
        b.set([100, 9, 104], BRICK);
        assert_eq!(b.world.get(100, 9, 104), BRICK);
    }

    #[test]
    fn the_walker_sinks_slowly_swims_up_and_climbs_out() {
        let mut b = with_pool();
        // Half a second in, it has sunk a little; in time it stands on the floor.
        run_for(&mut b, &Input::new(), 0.5);
        assert!(b.wet() && b.me.pos.y < 10.0 && b.me.pos.y > 8.0, "{:?}", b.me.pos);
        run_for(&mut b, &Input::new(), 4.0);
        assert_eq!((b.me.pos.y, b.me.grounded), (6.0, true));
        // Space swims up to the surface and keeps it there.
        run_for(&mut b, &held(&[Key::Space]), 3.0);
        assert!(b.me.pos.y > 9.0 && b.me.pos.y < 10.4, "{:?}", b.me.pos);
        // Swimming at the bank climbs out onto it.
        run_for(&mut b, &held(&[Key::Space, Key::Char('w')]), 3.0);
        run_for(&mut b, &Input::new(), 1.0);
        assert!(b.me.pos.z > 103.0 && b.me.pos.y == 10.0 && !b.wet(), "{:?}", b.me.pos);
        // Water is looked through: the pool's floor is in the frame.
        b.me.pos = V3::new(100.5, 6.0, 100.5);
        b.pitch = -1.5;
        assert_eq!(b.aim().unwrap().0, [100, 5, 100]);
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
        // Two blocks of water show the floor under them, six sides, and
        // their tops from above and from below.
        let mut w = flat(1);
        w.put([56, 1, 56], WATER);
        w.put([57, 1, 56], WATER);
        let m = w.mesh(3, 3);
        assert_eq!(m.tris.len(), (256 + 6 + 2 * 2) * 2);
        // Those tops lie an eighth under the blocks'.
        assert!(m.verts.iter().any(|v| v.p.y == 1.875) && m.verts.iter().all(|v| v.p.y <= 1.875));
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
    fn water_is_pale_at_the_shore_and_dark_over_the_deep() {
        // A basin eight deep with a shelf one deep beside it.
        let mut w = flat(12);
        for x in 40..56 {
            for z in 40..56 {
                for y in (if x < 48 { 4 } else { 11 })..12 { w.cells[at(x, y, z)] = WATER; }
            }
        }
        w.roofs();
        let mut m = w.mesh(2, 2).verts;
        m.extend(w.mesh(3, 2).verts);
        let green = |x: f32, z: f32| m.iter().find(|v| v.p.y == 11.875 && v.p.x == x && v.p.z == z).map(|v| v.lit[1]).expect("a corner of the water's top");
        let (deep, shelf, shore) = (green(44.0, 48.0), green(52.0, 48.0), green(56.0, 48.0));
        assert!((deep - ABYSS[1]).abs() < 0.001, "{deep}");
        assert!(shelf > deep + 0.5 && shore > shelf && shore < SHALLOW[1], "{deep} {shelf} {shore}");
    }

    #[test]
    fn the_world_comes_back_from_its_text() {
        let mut w = World::grow(11);
        w.put([20, 122, 30], BRICK);
        w.put([20, 123, 30], GLASS);
        w.put([21, 122, 30], SNOW);
        let x = (0..SX).find(|&x| w.get(x, w.roof_at(x, 9), 9) == GRASS).unwrap();
        w.put([x, w.roof_at(x, 9), 9], AIR);
        // A hole in the sea's floor, and the water that ran into it.
        let (x, z) = columns().find(|&(x, z)| w.get(x, SEA - 1, z) == WATER).expect("some sea");
        let hole = [x, w.roof_at(x, z), z];
        w.put(hole, WATER);
        w.flow(vec![hole], true);
        let back = World::from_text(&w.text()).expect("a world's own text reads back");
        assert!(back.cells == w.cells && back.roof == w.roof && back.changed == w.changed && back.seed == 11);
        // An older generator's world, or junk, gives none.
        assert!(World::from_text(&w.text().replacen("2 ", "1 ", 1)).is_none());
        for junk in ["", "2", "2 x", "2 11 5", "2 11 5 99", "2 11 99999999 3"] { assert!(World::from_text(junk).is_none(), "{junk:?}"); }
    }

    #[test]
    fn the_walker_and_its_pockets_come_back_from_their_text() {
        let mut b = on_flat();
        (b.me.pos, b.yaw, b.pitch, b.pick, b.pickaxe) = (V3::new(50.5, 10.0, 60.5), 1.25, -0.5, 8, true);
        b.have = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let text = b.walker_text();
        let mut c = on_flat();
        (c.have, c.pickaxe) = ([0; 10], false);
        c.stand(&text);
        assert_eq!((c.me.pos, c.yaw, c.pitch, c.pick, c.pickaxe, c.have), (b.me.pos, 1.25, -0.5, 8, true, b.have));
        // Not inside the stone, not from junk, not from an older version's line.
        for bad in ["50.5 5 60.5 0 0 0 0 1 1 1 1 1 1 1 1 1 1", "1 2 3", "a b c d e f", "NaN 10 60.5 0 0 0 0 1 1 1 1 1 1 1 1 1 1", "70.5 10 60.5 1.25 -0.5 8"] {
            c.stand(bad);
            assert_eq!((c.me.pos, c.have), (b.me.pos, b.have), "{bad:?}");
        }
    }

    #[test]
    fn a_new_world_starts_on_open_ground_with_cats_near() {
        for seed in [1, 2, 3] {
            let mut b = Blocks::new(World::grow(seed));
            assert!(!b.blocked(b.me.pos), "seed {seed}: {:?}", b.me.pos);
            let under = b.world.get(b.me.pos.x as i32, b.me.pos.y as i32 - 1, b.me.pos.z as i32);
            assert!(matches!(under, GRASS | SAND), "seed {seed} stands on {under}");
            assert!(b.have == [0; 10] && !b.pickaxe);
            // Eight cats on grass, close by, each in the open.
            assert_eq!(b.cats.len(), CATS, "seed {seed}");
            for c in &b.cats {
                let p = c.body.pos;
                assert_eq!(b.world.get(p.x as i32, p.y as i32 - 1, p.z as i32), GRASS);
                assert!(!b.world.stuck(p, CAT_HALF, CAT_TALL) && p.sub(b.me.pos).len() < 60.0);
            }
            let cats = b.cats_text();
            // A walk with jumps for a minute: the walker stays in the
            // world and never inside a block, and neither does a cat.
            b.mode = Mode::Play;
            let mut rng = Rng::new(seed as u64);
            for _ in 0..60 {
                b.yaw = rng.range(0.0, 6.28);
                run_for(&mut b, &held(&[Key::Char('w'), Key::Space]), 1.0);
                assert!(!b.blocked(b.me.pos) && b.me.pos.y >= 1.0, "seed {seed}: {:?}", b.me.pos);
                for c in &b.cats { assert!(!b.world.stuck(c.body.pos, CAT_HALF, CAT_TALL) && c.body.pos.y >= 1.0, "seed {seed}: a cat at {:?}", c.body.pos); }
            }
            // The cats go back to where their text says they were.
            assert!(b.cats_text() != cats);
            b.settle(&cats);
            assert_eq!(b.cats_text(), cats);
            b.settle("1 2 3");
            assert_eq!(b.cats_text(), cats);
        }
    }

    #[test]
    fn a_cat_comes_to_the_walker_and_sits_by_it() {
        let mut b = on_flat();
        b.cats.push(Cat::at(V3::new(100.5, 10.0, 107.5), 0));
        run_for(&mut b, &Input::new(), 4.0);
        let c = b.cats[0];
        let gap = c.body.pos.sub(b.me.pos).len();
        assert!(gap < 2.3 && gap > 1.5 && c.sits && !c.moving, "{gap} {c:?}");
        // It looks at the walker, who is toward smaller z from it.
        assert!((turned(c.yaw, PI, 9.0) - c.yaw).abs() < 0.2, "yaw {}", c.yaw);
        // The walker goes up onto a ledge; the cat gets up, follows, and
        // hops the step.
        for x in 96..105 {
            for z in 106..113 { b.set([x, 10, z], STONE); }
        }
        b.me.pos = V3::new(100.5, 11.0, 109.5);
        run_for(&mut b, &Input::new(), 5.0);
        let c = b.cats[0];
        assert!(c.body.pos.y == 11.0 && c.body.pos.sub(b.me.pos).len() < 2.3 && c.sits, "{c:?}");
        // A walker far away is nothing to a cat: it wanders and sits by turns.
        b.me.pos = V3::new(30.5, 10.0, 30.5);
        let (mut sat, mut walked, mut roamed) = (0, 0, 0.0f32);
        for _ in 0..900 {
            b.update(&Input::new(), 1.0 / 30.0);
            if b.cats[0].sits { sat += 1; } else { walked += 1; }
            roamed = roamed.max(b.cats[0].body.pos.sub(c.body.pos).len());
        }
        assert!(sat > 100 && walked > 100 && roamed > 1.0, "{sat} {walked} {roamed}");
    }

    #[test]
    fn a_cat_keeps_out_of_the_water_and_off_the_cliffs() {
        // An island eight blocks wide: water on two sides, a pit on the others.
        let mut b = on_flat();
        for x in 80..120 {
            for z in 80..120 {
                if (96..104).contains(&x) && (96..104).contains(&z) { continue; }
                for y in 4..10 { b.world.cells[at(x, y, z)] = if x < 96 || x >= 104 { WATER } else { AIR }; }
            }
        }
        b.me.pos = V3::new(30.5, 10.0, 30.5);
        for i in 0..4 { b.cats.push(Cat::at(V3::new(98.5 + i as f32, 10.0, 99.5), i)); }
        let mut roamed = 0.0f32;
        for _ in 0..3600 {
            b.update(&Input::new(), 1.0 / 30.0);
            for c in &b.cats {
                let p = c.body.pos;
                assert!(p.y == 10.0 && (96.0..104.0).contains(&p.x) && (96.0..104.0).contains(&p.z), "{c:?}");
                roamed = roamed.max(p.sub(V3::new(100.0, 10.0, 100.0)).len());
            }
        }
        assert!(roamed > 2.5, "the cats went {roamed} from the middle at most");
        // A walker in the water calls a cat to the edge, and no further.
        b.me.pos = V3::new(92.5, 9.0, 100.5);
        run_for(&mut b, &held(&[Key::Space]), 6.0);
        for c in &b.cats { assert!(c.body.pos.y == 10.0 && c.body.pos.x >= 96.0, "{c:?}"); }
        assert!(b.cats.iter().any(|c| c.body.pos.x < 97.5 && c.sits));

        // The water comes to a cat: it floats, and swims ashore.
        let mut b = with_pool();
        b.me.pos = V3::new(30.5, 10.0, 30.5);
        b.cats.push(Cat::at(V3::new(100.5, 6.0, 100.5), 0));
        run_for(&mut b, &Input::new(), 3.0);
        assert!(b.cats[0].body.pos.y > 9.0 && b.cats[0].body.pos.y < 10.6, "{:?}", b.cats[0].body.pos);
        run_for(&mut b, &Input::new(), 60.0);
        let p = b.cats[0].body.pos;
        assert!(p.y == 10.0 && b.world.get(p.x as i32, 9, p.z as i32) == STONE, "{p:?}");
    }

    #[test]
    fn clouds_stand_in_the_sky_and_drift() {
        let eye = V3::new(128.0, 40.0, 128.0);
        // Lines of sight to a grid of places in the layer the clouds lie in.
        let look = |i: i32| V3::new((i % 20 - 10) as f32 * 30.0 + 7.0, CLOUDS - eye.y, (i / 20 - 10) as f32 * 30.0 + 7.0).norm();
        let white = |c: Rgb| parts(c).0 > 170;
        let cover = |time: f32| (0..400).map(|i| white(sky(look(i), eye, time))).collect::<Vec<bool>>();
        let now = cover(0.0);
        let clouds = now.iter().filter(|w| **w).count();
        assert!(clouds > 40 && clouds < 300, "{clouds} of 400 lines of sight meet a cloud");
        // Half a minute on the clouds have moved; the same moment looks the same.
        assert!(now != cover(30.0) && now == cover(0.0));
        // None under the horizon, and the sun shines where no cloud hides it.
        assert!((0..400).all(|i| { let d = look(i); sky(V3::new(d.x, -d.y, d.z), eye, 0.0) == HAZE }));
        assert!((0..50).any(|i| sky(SUN, V3::new(i as f32 * 40.0, 40.0, 0.0), 0.0) == 0xfff6d8));
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
        b.pick = 2;
        b.cats.clear();
        b.draw(&mut f);
        // The cross in the middle, the picked kind's frame, and land
        // that is not one flat colour.
        assert_eq!(f.get(W / 2 - 3, H / 2), WHITE);
        assert_eq!(f.get((W - 10 * 22) / 2 + 2 * 22, H - 34), WHITE);
        let mut seen: Vec<Rgb> = (0..W).map(|x| f.get(x, 300)).collect();
        seen.sort();
        seen.dedup();
        assert!(seen.len() > 8, "{} colours on a row of land", seen.len());
        let play = f.px.clone();
        // The list lies over the picture, and the pickaxe shows once made.
        b.mode = Mode::Craft;
        b.draw(&mut f);
        assert!(f.px != play);
        (b.mode, b.pickaxe) = (Mode::Play, true);
        b.draw(&mut f);
        assert!(f.px != play);
        // A cat right in front is in the picture, standing or sitting.
        b.pickaxe = false;
        let (x, z) = (b.me.pos.x + b.yaw.sin() * 2.0, b.me.pos.z + b.yaw.cos() * 2.0);
        let mut cat = Cat::at(V3::new(x, b.world.roof_at(x as i32, z as i32) as f32 + 1.0, z), 0);
        b.cats = vec![cat];
        b.draw(&mut f);
        let sitting = f.px.clone();
        assert!(sitting != play);
        cat.sits = false;
        b.cats = vec![cat];
        b.draw(&mut f);
        assert!(f.px != play && f.px != sitting);
        // Under water the picture is blue all over.
        let mut b = with_pool();
        b.me.pos = V3::new(100.5, 6.0, 100.5);
        b.draw(&mut f);
        let (r, _, blue) = parts(f.get(100, 100));
        assert!(blue as i32 > r as i32 + 40, "{r} {blue}");
        b.draw_paused(&mut f);
    }
}
