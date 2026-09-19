//! Random numbers for games: fast, seedable, and the same sequence for
//! the same seed, so a level plays the same way in a test.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng { Rng(seed ^ 0x9e37_79b9_7f4a_7c15 | 1) }

    /// Seeded from the clock: a different game every time.
    pub fn from_time() -> Rng {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Rng::new(t)
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    /// 0.0 up to, not including, 1.0.
    pub fn float(&mut self) -> f32 { (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32 }

    /// A float from `lo` up to `hi`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 { lo + self.float() * (hi - lo) }

    /// An integer from `lo` up to, not including, `hi`.
    pub fn below(&mut self, n: u32) -> u32 { if n == 0 { 0 } else { self.next_u32() % n } }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f32) -> bool { self.float() < p }

    /// One of a slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() { None } else { Some(&items[self.below(items.len() as u32) as usize]) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence_and_floats_stay_in_range() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        assert_eq!(a.next_u32(), b.next_u32());
        let mut r = Rng::new(3);
        for _ in 0..1000 { let f = r.float(); assert!((0.0..1.0).contains(&f)); assert!(r.below(5) < 5); }
    }
}
