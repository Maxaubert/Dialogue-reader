//! Small fixed-algorithm RNG (SplitMix64). Its output never changes between crate versions,
//! so a seed always gives the same suite.

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

/// One SplitMix64 step. Also used to derive child seeds.
pub fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: mix(seed) }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn f32(&mut self) -> f32 {
        self.f64() as f32
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    pub fn rangef(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform integer in [lo, hi] (inclusive).
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i64
    }

    pub fn index(&mut self, len: usize) -> usize {
        if len == 0 { 0 } else { (self.next_u64() % len as u64) as usize }
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.index(xs.len())]
    }

    /// A child RNG whose stream does not depend on how much of this one is used later.
    pub fn fork(&mut self, salt: u64) -> Rng {
        Rng::new(self.next_u64() ^ mix(salt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = Rng::new(43);
        assert_ne!(Rng::new(42).next_u64(), c.next_u64());
    }

    #[test]
    fn ranges_hold() {
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let x = r.range(2.0, 3.0);
            assert!((2.0..3.0).contains(&x));
            let i = r.int(-2, 2);
            assert!((-2..=2).contains(&i));
        }
    }
}
