// A tiny, dependency-free pseudo-random generator.
//
// The core crate stays free of external crates so it compiles anywhere with no
// fuss. We only need randomness to shuffle the word pool, so a small xorshift
// generator is plenty. Seeding it deterministically also makes the trainer's
// behaviour reproducible in tests; the app seeds it from the clock at startup.

/// A minimal xorshift64* pseudo-random generator.
///
/// Not cryptographically secure — do not use for anything security-sensitive.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a generator from a seed. Any non-zero seed works; zero is nudged
    /// to a fixed constant so the generator never gets stuck at all-zeroes.
    pub fn new(seed: u64) -> Self {
        Rng {
            state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed },
        }
    }

    /// Next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A uniformly random index in `0..len`. Returns 0 when `len` is 0.
    pub fn below(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (self.next_u64() % len as u64) as usize
        }
    }

    /// Return `items` in a new random order (Fisher–Yates).
    pub fn shuffled<T: Clone>(&mut self, items: &[T]) -> Vec<T> {
        let mut out = items.to_vec();
        for i in (1..out.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            out.swap(i, j);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut rng = Rng::new(7);
        let input: Vec<u32> = (0..50).collect();
        let mut shuffled = rng.shuffled(&input);
        shuffled.sort_unstable();
        assert_eq!(shuffled, input);
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::new(1);
        for _ in 0..1000 {
            assert!(rng.below(10) < 10);
        }
        assert_eq!(rng.below(0), 0);
    }
}
