//! Noise for land, clouds and textures: the same value for the same
//! place and seed every time, so a world is built the same way on every
//! run. All values are 0.0 up to 1.0.

/// A value for a grid point.
pub fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

/// An S-curve from 0 to 1: flat at both ends.
pub fn smooth(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

/// Smooth noise that repeats every `period` grid points both ways, so a
/// map built from it has no seam where it wraps.
pub fn noise(x: f32, y: f32, period: i32, seed: u32) -> f32 { noise2(x, y, period, period, seed) }

/// Smooth noise with its own period across and down.
pub fn noise2(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (smooth(x - xi as f32), smooth(y - yi as f32));
    let g = |ix: i32, iy: i32| hash(ix.rem_euclid(px), iy.rem_euclid(py), seed);
    let a = g(xi, yi) + (g(xi + 1, yi) - g(xi, yi)) * fx;
    let b = g(xi, yi + 1) + (g(xi + 1, yi + 1) - g(xi, yi + 1)) * fx;
    a + (b - a) * fy
}

/// Layers of noise, each twice as fine and half as tall: hills, clouds.
/// It repeats every `period` units.
pub fn fbm(x: f32, y: f32, octaves: u32, seed: u32, period: i32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += noise(x * freq, y * freq, (period << o).max(1), seed + o) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Noise with sharp ridges, layered `octaves` times: mountains. It
/// repeats every 6 units.
pub fn ridged(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_the_same_every_time_and_wraps() {
        assert_eq!(hash(3, 4, 5), hash(3, 4, 5));
        assert_ne!(hash(3, 4, 5), hash(3, 4, 6));
        for i in 0..50 {
            let (x, y) = (i as f32 * 0.37, i as f32 * 0.91);
            let v = noise(x, y, 8, 1);
            assert!((0.0..1.0).contains(&v));
            assert!((v - noise(x + 8.0, y, 8, 1)).abs() < 1e-4, "one period across is the same place");
            assert!((0.0..=1.0).contains(&ridged(x, y, 4, 1)));
            assert!((0.0..1.0).contains(&fbm(x, y, 4, 1, 8)));
        }
        // On a grid point noise is that point's own value.
        assert_eq!(noise(2.0, 3.0, 8, 1), hash(2, 3, 1));
    }
}
