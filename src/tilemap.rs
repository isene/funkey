//! A grid of tiles and the boxes that move through it: what a platformer
//! needs. Tiles are characters, so a level is a few lines of text.

/// A level as rows of characters. `#` is solid by default; a game can
/// say which other characters are solid, ladders, or anything else.
pub struct Tilemap {
    pub w: i32,
    pub h: i32,
    pub tile: i32,
    tiles: Vec<char>,
    pub solid: Vec<char>,
    /// Tiles a body can stand on but pass through from below or the side:
    /// the top of a ladder, a thin platform.
    pub one_way: Vec<char>,
}

impl Tilemap {
    pub fn from_rows(rows: &[&str], tile: i32) -> Tilemap {
        let h = rows.len() as i32;
        let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut tiles = vec![' '; (w * h) as usize];
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() { tiles[y * w as usize + x] = c; }
        }
        Tilemap { w, h, tile, tiles, solid: vec!['#'], one_way: Vec::new() }
    }

    pub fn at(&self, tx: i32, ty: i32) -> char {
        if tx < 0 || ty < 0 || tx >= self.w || ty >= self.h { '#' } else { self.tiles[(ty * self.w + tx) as usize] }
    }

    pub fn set(&mut self, tx: i32, ty: i32, c: char) {
        if tx >= 0 && ty >= 0 && tx < self.w && ty < self.h { self.tiles[(ty * self.w + tx) as usize] = c; }
    }

    pub fn is_solid(&self, tx: i32, ty: i32) -> bool { self.solid.contains(&self.at(tx, ty)) }
    pub fn is_one_way(&self, tx: i32, ty: i32) -> bool { self.one_way.contains(&self.at(tx, ty)) }

    /// Would a box whose bottom edge moves from `bottom_before` down to
    /// `bottom_after` land on a one-way tile?
    pub fn lands_on_one_way(&self, x: f32, w: f32, bottom_before: f32, bottom_after: f32) -> Option<f32> {
        let t = self.tile as f32;
        let (x0, x1) = ((x / t).floor() as i32, ((x + w - 0.001) / t).floor() as i32);
        let (r0, r1) = ((bottom_before / t).floor() as i32, ((bottom_after - 0.001) / t).floor() as i32);
        for ty in r0..=r1 {
            let top = ty as f32 * t;
            if top < bottom_before - 0.001 { continue; }
            for tx in x0..=x1 { if self.is_one_way(tx, ty) { return Some(top); } }
        }
        None
    }

    /// Every tile position holding `c`.
    pub fn find(&self, c: char) -> Vec<(i32, i32)> {
        (0..self.h).flat_map(|y| (0..self.w).map(move |x| (x, y))).filter(|&(x, y)| self.at(x, y) == c).collect()
    }

    /// Pixel size of the whole map.
    pub fn pixel_w(&self) -> i32 { self.w * self.tile }
    pub fn pixel_h(&self) -> i32 { self.h * self.tile }

    /// Does the box from (x, y) with size (w, h), in pixels, overlap a
    /// solid tile?
    pub fn blocked(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let t = self.tile as f32;
        let (x0, y0) = ((x / t).floor() as i32, (y / t).floor() as i32);
        let (x1, y1) = (((x + w - 0.001) / t).floor() as i32, ((y + h - 0.001) / t).floor() as i32);
        for ty in y0..=y1 { for tx in x0..=x1 { if self.is_solid(tx, ty) { return true; } } }
        false
    }

    /// The characters under a box, for pickups, ladders, hazards.
    pub fn under(&self, x: f32, y: f32, w: f32, h: f32) -> Vec<(i32, i32, char)> {
        let t = self.tile as f32;
        let (x0, y0) = ((x / t).floor() as i32, (y / t).floor() as i32);
        let (x1, y1) = (((x + w - 0.001) / t).floor() as i32, ((y + h - 0.001) / t).floor() as i32);
        let mut out = Vec::new();
        for ty in y0..=y1 { for tx in x0..=x1 { out.push((tx, ty, self.at(tx, ty))); } }
        out
    }
}

/// A moving box: position and size in pixels, speed in pixels a second.
#[derive(Clone, Debug, Default)]
pub struct Body {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub vx: f32,
    pub vy: f32,
    pub on_ground: bool,
    pub hit_wall: bool,
}

impl Body {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Body {
        Body { x, y, w, h, ..Default::default() }
    }

    /// Move by the speed for `dt` seconds, stopping at solid tiles, one
    /// axis at a time so a corner never catches. One-way tiles stop a
    /// fall from above unless `through` is set (climbing down a ladder).
    pub fn step(&mut self, map: &Tilemap, dt: f32) { self.step_through(map, dt, false); }

    pub fn step_through(&mut self, map: &Tilemap, dt: f32, through: bool) {
        self.hit_wall = false;
        let dx = self.vx * dt;
        if dx != 0.0 {
            let nx = self.x + dx;
            if map.blocked(nx, self.y, self.w, self.h) {
                let t = map.tile as f32;
                self.x = if dx > 0.0 { ((nx + self.w) / t).floor() * t - self.w - 0.001 } else { (nx / t).ceil() * t };
                self.vx = 0.0;
                self.hit_wall = true;
            } else {
                self.x = nx;
            }
        }
        let dy = self.vy * dt;
        self.on_ground = false;
        if dy != 0.0 {
            let ny = self.y + dy;
            if dy > 0.0 && !through {
                if let Some(top) = map.lands_on_one_way(self.x, self.w, self.y + self.h, ny + self.h) {
                    self.y = top - self.h - 0.001;
                    self.vy = 0.0;
                    self.on_ground = true;
                    return;
                }
            }
            if map.blocked(self.x, ny, self.w, self.h) {
                let t = map.tile as f32;
                if dy > 0.0 {
                    self.y = ((ny + self.h) / t).floor() * t - self.h - 0.001;
                    self.on_ground = true;
                } else {
                    self.y = (ny / t).ceil() * t;
                }
                self.vy = 0.0;
            } else {
                self.y = ny;
            }
        } else if map.blocked(self.x, self.y + 0.01, self.w, self.h)
            || (!through && map.lands_on_one_way(self.x, self.w, self.y + self.h, self.y + self.h + 0.01).is_some()) {
            self.on_ground = true;
        }
    }

    pub fn overlaps(&self, o: &Body) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Tilemap {
        Tilemap::from_rows(&[
            "#      #",
            "#      #",
            "#   #  #",
            "########",
        ], 8)
    }

    #[test]
    fn a_body_falls_onto_the_floor_and_stops_at_walls() {
        let m = map();
        let mut b = Body::new(12.0, 4.0, 6.0, 6.0);
        b.vy = 100.0;
        for _ in 0..30 { b.step(&m, 0.05); }
        assert!(b.on_ground);
        assert!((b.y + b.h - 24.0).abs() < 0.01, "rests on the floor at y=24, got {}", b.y + b.h);
        b.vx = 100.0;
        for _ in 0..30 { b.vy = 100.0; b.step(&m, 0.05); }
        assert!(b.hit_wall || b.vx == 0.0);
        assert!(b.x + b.w <= 32.0, "stopped by the block at column 4, got {}", b.x + b.w);
    }

    #[test]
    fn a_one_way_tile_holds_from_above_and_lets_through_when_asked() {
        let mut m = Tilemap::from_rows(&["    ", "    ", " HH ", "####"], 8);
        m.one_way = vec!['H'];
        let mut b = Body::new(9.0, 0.0, 6.0, 6.0);
        b.vy = 60.0;
        for _ in 0..20 { b.step(&m, 0.05); }
        assert!(b.on_ground);
        assert!((b.y + b.h - 16.0).abs() < 0.01, "stands on the ladder top, got {}", b.y + b.h);
        for _ in 0..20 { b.vy = 60.0; b.step_through(&m, 0.05, true); }
        assert!((b.y + b.h - 24.0).abs() < 0.01, "climbed down to the floor, got {}", b.y + b.h);
        let mut up = Body::new(9.0, 17.0, 6.0, 6.0);
        up.vy = -60.0;
        up.step(&m, 0.05);
        assert!(up.y < 17.0, "moving up through it is free");
    }

    #[test]
    fn tiles_under_a_box_and_finds() {
        let m = map();
        assert_eq!(m.find('#').len(), 15);
        assert!(m.under(30.0, 14.0, 4.0, 4.0).iter().any(|&(_, _, c)| c == '#'));
        assert!(!m.blocked(8.0, 0.0, 8.0, 8.0));
        assert!(m.blocked(0.0, 0.0, 4.0, 4.0));
    }
}
