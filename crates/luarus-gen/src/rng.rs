/// A small deterministic generator, so a failing case can always be reproduced
/// from its seed alone.
///
/// xorshift64*: not for anything that needs to be unpredictable, entirely
/// adequate for choosing between grammar alternatives.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // The state must never be zero, or the sequence collapses.
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`. Returns 0 when `n` is 0.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next() % n as u64) as usize
    }

    /// A number in `lo..=hi`.
    pub fn between(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den as usize) < num as usize
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_reproduces_its_sequence() {
        let take = |seed| {
            let mut r = Rng::new(seed);
            (0..32).map(|_| r.below(1000)).collect::<Vec<_>>()
        };
        assert_eq!(take(7), take(7));
        assert_ne!(take(7), take(8));
    }

    #[test]
    fn below_stays_in_range() {
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            assert!(r.below(10) < 10);
        }
    }

    #[test]
    fn between_stays_in_range() {
        let mut r = Rng::new(2);
        for _ in 0..1000 {
            let v = r.between(-5, 5);
            assert!((-5..=5).contains(&v));
        }
    }

    #[test]
    fn a_zero_seed_still_produces_a_sequence() {
        let mut r = Rng::new(0);
        let a: Vec<_> = (0..8).map(|_| r.below(100)).collect();
        assert!(a.iter().any(|&x| x != a[0]), "the sequence should not be constant");
    }
}
