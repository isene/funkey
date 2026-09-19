//! Doom's WAD files: the directory of lumps, the palette and colour
//! maps, wall textures built from patches, flats, sprites, and a level's
//! geometry with the BSP tree the level was built with.

use std::collections::HashMap;
use std::path::Path;

fn u16le(b: &[u8], at: usize) -> u16 { u16::from_le_bytes([b[at], b[at + 1]]) }
fn i16le(b: &[u8], at: usize) -> i16 { i16::from_le_bytes([b[at], b[at + 1]]) }
fn u32le(b: &[u8], at: usize) -> u32 { u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) }
fn i32le(b: &[u8], at: usize) -> i32 { i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) }

/// A lump name: up to eight bytes, upper case, zero padded.
pub fn name8(b: &[u8]) -> String {
    b.iter().take(8).take_while(|&&c| c != 0).map(|&c| (c as char).to_ascii_uppercase()).collect()
}

pub struct Lump { pub name: String, pub data: Vec<u8> }

pub struct Wad {
    pub lumps: Vec<Lump>,
}

impl Wad {
    pub fn open(path: &Path) -> Result<Wad, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        if bytes.len() < 12 || (&bytes[0..4] != b"IWAD" && &bytes[0..4] != b"PWAD") {
            return Err(format!("{}: not a WAD file", path.display()));
        }
        let count = u32le(&bytes, 4) as usize;
        let dir = u32le(&bytes, 8) as usize;
        let mut lumps = Vec::with_capacity(count);
        for i in 0..count {
            let e = dir + i * 16;
            if e + 16 > bytes.len() { break; }
            let at = u32le(&bytes, e) as usize;
            let size = u32le(&bytes, e + 4) as usize;
            let name = name8(&bytes[e + 8..e + 16]);
            let data = if at + size <= bytes.len() { bytes[at..at + size].to_vec() } else { Vec::new() };
            lumps.push(Lump { name, data });
        }
        Ok(Wad { lumps })
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.lumps.iter().rposition(|l| l.name == name)
    }

    pub fn lump(&self, name: &str) -> Option<&[u8]> {
        self.index(name).map(|i| self.lumps[i].data.as_slice())
    }

    /// The lumps between two markers, such as F_START and F_END.
    pub fn between(&self, start: &str, end: &str) -> Vec<usize> {
        let Some(s) = self.lumps.iter().position(|l| l.name == start) else { return Vec::new() };
        let e = self.lumps.iter().skip(s).position(|l| l.name == end).map(|p| s + p).unwrap_or(self.lumps.len());
        (s + 1..e).filter(|&i| !self.lumps[i].data.is_empty()).collect()
    }

    /// The 256-colour palette as 0xRRGGBB.
    pub fn palette(&self) -> Vec<u32> {
        self.palettes().into_iter().next().unwrap_or_else(|| vec![0; 256])
    }

    /// All the palettes in PLAYPAL: the plain one, then the red pain
    /// shades, the yellow pickup shades and the green suit tint.
    pub fn palettes(&self) -> Vec<Vec<u32>> {
        let p = self.lump("PLAYPAL").unwrap_or(&[]);
        (0..p.len() / 768).map(|n| (0..256).map(|i| {
            let a = n * 768 + i * 3;
            ((p[a] as u32) << 16) | ((p[a + 1] as u32) << 8) | p[a + 2] as u32
        }).collect()).collect()
    }

    /// A picture lump by name, such as STBAR or TITLEPIC.
    pub fn picture(&self, name: &str) -> Option<Picture> {
        self.lump(name).and_then(Picture::from_patch)
    }

    /// 34 colour maps of 256 entries: light levels from bright to dark.
    pub fn colormaps(&self) -> Vec<[u8; 256]> {
        let c = self.lump("COLORMAP").unwrap_or(&[]);
        (0..c.len() / 256).map(|i| { let mut m = [0u8; 256]; m.copy_from_slice(&c[i * 256..i * 256 + 256]); m }).collect()
    }
}

/// A picture in Doom's patch format, unpacked: width by height indices
/// into the palette, 0xff where nothing is drawn.
#[derive(Clone)]
pub struct Picture {
    pub w: i32,
    pub h: i32,
    pub left: i32,
    pub top: i32,
    /// Column-major: `px[x * h + y]`.
    pub px: Vec<u8>,
}

pub const NO_PIXEL: u8 = 0xff;

impl Picture {
    pub fn from_patch(d: &[u8]) -> Option<Picture> {
        if d.len() < 8 { return None; }
        let w = u16le(d, 0) as i32;
        let h = u16le(d, 2) as i32;
        let left = i16le(d, 4) as i32;
        let top = i16le(d, 6) as i32;
        if w <= 0 || h <= 0 || w > 4096 || h > 4096 || d.len() < 8 + w as usize * 4 { return None; }
        let mut px = vec![NO_PIXEL; (w * h) as usize];
        for x in 0..w as usize {
            let mut at = u32le(d, 8 + x * 4) as usize;
            let mut last_top = 0i32;
            loop {
                if at >= d.len() { break; }
                let mut row = d[at] as i32;
                if row == 255 { break; }
                if at + 1 >= d.len() { break; }
                let len = d[at + 1] as usize;
                // Tall patches: a start row not above the last one is an offset.
                if row <= last_top { row += last_top; }
                last_top = row;
                let src = at + 3;
                for i in 0..len {
                    let y = row + i as i32;
                    if src + i < d.len() && y >= 0 && y < h { px[x * h as usize + y as usize] = d[src + i]; }
                }
                at = src + len + 1;
            }
        }
        Some(Picture { w, h, left, top, px })
    }

    /// A flat: 64 by 64 raw indices.
    pub fn from_flat(d: &[u8]) -> Option<Picture> {
        if d.len() < 4096 { return None; }
        let mut px = vec![0u8; 4096];
        for y in 0..64 { for x in 0..64 { px[x * 64 + y] = d[y * 64 + x]; } }
        Some(Picture { w: 64, h: 64, left: 0, top: 0, px })
    }

    #[inline]
    pub fn at(&self, x: i32, y: i32) -> u8 {
        self.px[(x.rem_euclid(self.w) * self.h + y.rem_euclid(self.h)) as usize]
    }
}

/// Everything a level's walls, floors and things are drawn with.
pub struct Art {
    /// The palette in use; one of `palettes`.
    pub palette: Vec<u32>,
    pub palettes: Vec<Vec<u32>>,
    pub colormaps: Vec<[u8; 256]>,
    pub textures: HashMap<String, Picture>,
    pub flats: HashMap<String, Picture>,
    /// Sprite pictures, found through `frames`.
    pub sprite_pics: Vec<Picture>,
    /// Sprite name and frame letter to the picture for each of the
    /// eight rotations, with whether it is drawn mirrored.
    pub frames: HashMap<[u8; 5], [Option<(u32, bool)>; 8]>,
}

impl Art {
    pub fn load(wad: &Wad) -> Art {
        let palettes = wad.palettes();
        let palette = palettes.first().cloned().unwrap_or_else(|| vec![0; 256]);
        let colormaps = wad.colormaps();
        // Patches by name, through PNAMES.
        let pnames: Vec<String> = wad.lump("PNAMES").map(|p| {
            let n = u32le(p, 0) as usize;
            (0..n).filter(|i| 4 + (i + 1) * 8 <= p.len()).map(|i| name8(&p[4 + i * 8..4 + i * 8 + 8])).collect()
        }).unwrap_or_default();
        let mut patches: HashMap<usize, Picture> = HashMap::new();
        let mut textures = HashMap::new();
        for tl in ["TEXTURE1", "TEXTURE2"] {
            let Some(t) = wad.lump(tl) else { continue };
            if t.len() < 4 { continue; }
            let n = u32le(t, 0) as usize;
            for i in 0..n {
                if 4 + (i + 1) * 4 > t.len() { break; }
                let at = u32le(t, 4 + i * 4) as usize;
                if at + 22 > t.len() { continue; }
                let name = name8(&t[at..at + 8]);
                let w = u16le(t, at + 12) as i32;
                let h = u16le(t, at + 14) as i32;
                let np = u16le(t, at + 20) as usize;
                if w <= 0 || h <= 0 { continue; }
                let mut pic = Picture { w, h, left: 0, top: 0, px: vec![NO_PIXEL; (w * h) as usize] };
                for p in 0..np {
                    let e = at + 22 + p * 10;
                    if e + 10 > t.len() { break; }
                    let ox = i16le(t, e) as i32;
                    let oy = i16le(t, e + 2) as i32;
                    let pi = u16le(t, e + 4) as usize;
                    let patch = match patches.get(&pi) {
                        Some(p) => p,
                        None => {
                            let Some(pn) = pnames.get(pi) else { continue };
                            let Some(pd) = wad.lump(pn) else { continue };
                            let Some(pp) = Picture::from_patch(pd) else { continue };
                            patches.insert(pi, pp);
                            &patches[&pi]
                        }
                    };
                    for x in 0..patch.w {
                        let tx = ox + x;
                        if tx < 0 || tx >= w { continue; }
                        for y in 0..patch.h {
                            let ty = oy + y;
                            if ty < 0 || ty >= h { continue; }
                            let c = patch.px[(x * patch.h + y) as usize];
                            if c != NO_PIXEL { pic.px[(tx * h + ty) as usize] = c; }
                        }
                    }
                }
                textures.insert(name, pic);
            }
        }
        let mut flats = HashMap::new();
        for i in wad.between("F_START", "F_END").into_iter().chain(wad.between("FF_START", "FF_END")) {
            if let Some(p) = Picture::from_flat(&wad.lumps[i].data) { flats.insert(wad.lumps[i].name.clone(), p); }
        }
        let mut sprite_pics = Vec::new();
        let mut frames: HashMap<[u8; 5], [Option<(u32, bool)>; 8]> = HashMap::new();
        for i in wad.between("S_START", "S_END").into_iter().chain(wad.between("SS_START", "SS_END")) {
            let n = wad.lumps[i].name.as_bytes();
            if n.len() != 6 && n.len() != 8 { continue; }
            let Some(p) = Picture::from_patch(&wad.lumps[i].data) else { continue };
            let id = sprite_pics.len() as u32;
            sprite_pics.push(p);
            let mut set = |frame: u8, rot: u8, mirror: bool| {
                let mut key = [0u8; 5];
                key[..4].copy_from_slice(&n[..4]);
                key[4] = frame;
                let slots = frames.entry(key).or_insert([None; 8]);
                if rot == b'0' { for s in slots.iter_mut() { if s.is_none() { *s = Some((id, mirror)); } } }
                else if (b'1'..=b'8').contains(&rot) { slots[(rot - b'1') as usize] = Some((id, mirror)); }
            };
            set(n[4], n[5], false);
            if n.len() == 8 { set(n[6], n[7], true); }
        }
        Art { palette, palettes, colormaps, textures, flats, sprite_pics, frames }
    }

    /// The picture for a sprite name, frame letter and rotation (1..8),
    /// and whether to draw it mirrored. Lumps such as `TROOA2A8` serve
    /// two rotations, one of them mirrored.
    pub fn sprite(&self, name: &[u8; 4], frame: u8, rotation: u8) -> Option<(&Picture, bool)> {
        let key = [name[0], name[1], name[2], name[3], frame];
        let (id, mirror) = self.frames.get(&key)?[(rotation.clamp(1, 8) - 1) as usize]?;
        Some((&self.sprite_pics[id as usize], mirror))
    }

    /// Switch to palette `n`: 0 plain, 1-8 red, 9-12 yellow, 13 green.
    pub fn use_palette(&mut self, n: usize) {
        if let Some(p) = self.palettes.get(n.min(self.palettes.len().saturating_sub(1))) {
            if self.palette != *p { self.palette = p.clone(); }
        }
    }
}

// ---------------------------------------------------------------- level

#[derive(Clone, Debug)]
pub struct Thing { pub x: f32, pub y: f32, pub angle: f32, pub kind: u16, pub flags: u16 }

#[derive(Clone, Debug)]
pub struct Linedef { pub v1: usize, pub v2: usize, pub flags: u16, pub special: u16, pub tag: u16, pub front: Option<usize>, pub back: Option<usize> }

pub const ML_BLOCKING: u16 = 1;
pub const ML_TWOSIDED: u16 = 4;
pub const ML_DONTPEGTOP: u16 = 8;
pub const ML_DONTPEGBOTTOM: u16 = 16;

#[derive(Clone, Debug)]
pub struct Sidedef { pub x_off: f32, pub y_off: f32, pub upper: String, pub lower: String, pub middle: String, pub sector: usize }

#[derive(Clone, Debug)]
pub struct Sector { pub floor: f32, pub ceiling: f32, pub floor_flat: String, pub ceiling_flat: String, pub light: i32, pub special: u16, pub tag: u16 }

#[derive(Clone, Debug)]
pub struct Seg { pub v1: usize, pub v2: usize, pub angle: f32, pub linedef: usize, pub back_side: bool, pub offset: f32 }

#[derive(Clone, Debug)]
pub struct Subsector { pub first: usize, pub count: usize }

#[derive(Clone, Debug)]
pub struct Node {
    pub x: f32, pub y: f32, pub dx: f32, pub dy: f32,
    /// Children: a subsector when the high bit is set.
    pub child: [u16; 2],
}

pub const NF_SUBSECTOR: u16 = 0x8000;

pub struct Level {
    pub name: String,
    pub vertexes: Vec<(f32, f32)>,
    pub linedefs: Vec<Linedef>,
    pub sidedefs: Vec<Sidedef>,
    pub sectors: Vec<Sector>,
    pub segs: Vec<Seg>,
    pub subsectors: Vec<Subsector>,
    pub nodes: Vec<Node>,
    pub things: Vec<Thing>,
}

impl Level {
    /// Load the level whose marker lump is `name` (E1M1, MAP01).
    pub fn load(wad: &Wad, name: &str) -> Result<Level, String> {
        let base = wad.lumps.iter().position(|l| l.name == name).ok_or(format!("no level {}", name))?;
        let get = |n: &str| -> &[u8] {
            wad.lumps[base + 1..(base + 11).min(wad.lumps.len())].iter().find(|l| l.name == n).map(|l| l.data.as_slice()).unwrap_or(&[])
        };
        let v = get("VERTEXES");
        let vertexes: Vec<(f32, f32)> = (0..v.len() / 4).map(|i| (i16le(v, i * 4) as f32, i16le(v, i * 4 + 2) as f32)).collect();
        let l = get("LINEDEFS");
        let side = |s: u16| if s == 0xffff { None } else { Some(s as usize) };
        let linedefs: Vec<Linedef> = (0..l.len() / 14).map(|i| { let a = i * 14; Linedef {
            v1: u16le(l, a) as usize, v2: u16le(l, a + 2) as usize, flags: u16le(l, a + 4), special: u16le(l, a + 6),
            tag: u16le(l, a + 8), front: side(u16le(l, a + 10)), back: side(u16le(l, a + 12)) } }).collect();
        let s = get("SIDEDEFS");
        let sidedefs: Vec<Sidedef> = (0..s.len() / 30).map(|i| { let a = i * 30; Sidedef {
            x_off: i16le(s, a) as f32, y_off: i16le(s, a + 2) as f32,
            upper: name8(&s[a + 4..a + 12]), lower: name8(&s[a + 12..a + 20]), middle: name8(&s[a + 20..a + 28]),
            sector: u16le(s, a + 28) as usize } }).collect();
        let sc = get("SECTORS");
        let sectors: Vec<Sector> = (0..sc.len() / 26).map(|i| { let a = i * 26; Sector {
            floor: i16le(sc, a) as f32, ceiling: i16le(sc, a + 2) as f32,
            floor_flat: name8(&sc[a + 4..a + 12]), ceiling_flat: name8(&sc[a + 12..a + 20]),
            light: i16le(sc, a + 20) as i32, special: u16le(sc, a + 22), tag: u16le(sc, a + 24) } }).collect();
        let sg = get("SEGS");
        let segs: Vec<Seg> = (0..sg.len() / 12).map(|i| { let a = i * 12; Seg {
            v1: u16le(sg, a) as usize, v2: u16le(sg, a + 2) as usize,
            angle: (u16le(sg, a + 4) as f32) * std::f32::consts::TAU / 65536.0,
            linedef: u16le(sg, a + 6) as usize, back_side: u16le(sg, a + 8) != 0, offset: i16le(sg, a + 10) as f32 } }).collect();
        let ss = get("SSECTORS");
        let subsectors: Vec<Subsector> = (0..ss.len() / 4).map(|i| Subsector { count: u16le(ss, i * 4) as usize, first: u16le(ss, i * 4 + 2) as usize }).collect();
        let nd = get("NODES");
        let nodes: Vec<Node> = (0..nd.len() / 28).map(|i| { let a = i * 28; Node {
            x: i16le(nd, a) as f32, y: i16le(nd, a + 2) as f32, dx: i16le(nd, a + 4) as f32, dy: i16le(nd, a + 6) as f32,
            child: [u16le(nd, a + 24), u16le(nd, a + 26)] } }).collect();
        let th = get("THINGS");
        let things: Vec<Thing> = (0..th.len() / 10).map(|i| { let a = i * 10; Thing {
            x: i16le(th, a) as f32, y: i16le(th, a + 2) as f32, angle: (i16le(th, a + 4) as f32).to_radians(),
            kind: u16le(th, a + 6), flags: u16le(th, a + 8) } }).collect();
        if vertexes.is_empty() || nodes.is_empty() || subsectors.is_empty() {
            return Err(format!("{}: level lumps missing or empty", name));
        }
        let _ = i32le;
        Ok(Level { name: name.to_string(), vertexes, linedefs, sidedefs, sectors, segs, subsectors, nodes, things })
    }

    /// Which side of a node's dividing line a point is on: 0 front
    /// (the right of the line's direction), 1 back.
    pub fn node_side(&self, n: &Node, x: f32, y: f32) -> usize {
        if (x - n.x) * n.dy - (y - n.y) * n.dx > 0.0 { 0 } else { 1 }
    }

    /// The subsector under a point, by walking the tree.
    pub fn subsector_at(&self, x: f32, y: f32) -> usize {
        let mut id = (self.nodes.len() - 1) as u16;
        loop {
            if id & NF_SUBSECTOR != 0 { return (id & !NF_SUBSECTOR) as usize; }
            let n = &self.nodes[id as usize];
            id = n.child[self.node_side(n, x, y)];
        }
    }

    pub fn sector_at(&self, x: f32, y: f32) -> usize {
        let ss = &self.subsectors[self.subsector_at(x, y)];
        let seg = &self.segs[ss.first];
        let ld = &self.linedefs[seg.linedef];
        let side = if seg.back_side { ld.back } else { ld.front };
        side.map(|s| self.sidedefs[s].sector).unwrap_or(0)
    }

    pub fn player_start(&self) -> Option<&Thing> {
        self.things.iter().find(|t| t.kind == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_unpacks_its_posts_into_columns() {
        // 2 wide, 3 tall; column 0 has one post of two pixels at row 1,
        // column 1 is empty.
        let mut d = vec![2, 0, 3, 0, 0, 0, 0, 0];
        d.extend_from_slice(&[16, 0, 0, 0, 23, 0, 0, 0]);
        d.extend_from_slice(&[1, 2, 0, 7, 9, 0, 255]);
        d.extend_from_slice(&[255]);
        let p = Picture::from_patch(&d).unwrap();
        assert_eq!((p.w, p.h), (2, 3));
        assert_eq!(p.px, vec![NO_PIXEL, 7, 9, NO_PIXEL, NO_PIXEL, NO_PIXEL]);
        assert_eq!(p.at(-2, 4), 7, "lookups wrap");
    }

    #[test]
    fn the_front_of_a_node_is_to_the_right_of_its_line() {
        let l = Level { name: "T".into(), vertexes: vec![], linedefs: vec![], sidedefs: vec![], sectors: vec![], segs: vec![],
            subsectors: vec![], nodes: vec![], things: vec![] };
        // A line pointing north (+y): east is its right, the front.
        let n = Node { x: 0.0, y: 0.0, dx: 0.0, dy: 100.0, child: [0, 0] };
        assert_eq!(l.node_side(&n, 10.0, 50.0), 0);
        assert_eq!(l.node_side(&n, -10.0, 50.0), 1);
    }

    #[test]
    fn names_are_upper_case_and_stop_at_the_first_zero() {
        assert_eq!(name8(b"e1m1\0\0\0\0"), "E1M1");
        assert_eq!(name8(b"STARTAN3"), "STARTAN3");
    }
}
