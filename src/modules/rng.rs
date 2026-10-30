// A small xorshift generator for module randomness (chances, delays, jitter).

#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// `Random.nextDouble`: uniform in [0, 1).
    pub fn double(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// `Random.nextInt(bound)`: uniform in [0, bound).
    pub fn int(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }
    /// `RandomValue.getRandomValue`: uniform between the range's ends.
    pub fn range(&mut self, (lo, hi): (f64, f64)) -> f64 {
        if hi <= lo {
            lo
        } else {
            lo + self.double() * (hi - lo)
        }
    }
}
