// Reference click timing. The caller supplies a monotonic millisecond clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Extra,
}

#[derive(Debug)]
pub struct Timing {
    rng: u64,
    last_delay: u64,
    burst: u32,
    fast: bool,
    phase_count: u32,
    phase_length: u32,
}
impl Timing {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed.max(1),
            last_delay: 0,
            burst: 0,
            fast: true,
            phase_count: 0,
            phase_length: 0,
        }
    }
    fn random(&mut self, upper: u32) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % u64::from(upper)) as u32
    }
    pub fn delay(&mut self, minimum: u32, maximum: u32, mode: Mode) -> u64 {
        let minimum = minimum.max(1);
        let maximum = maximum.max(minimum);
        let cps = if maximum == minimum {
            minimum
        } else {
            minimum + 1 + self.random(maximum - minimum)
        };
        if mode == Mode::Normal {
            return 1000 / u64::from(cps);
        }
        if self.burst == 0 {
            self.last_delay = 1000 / u64::from(cps);
            if self.random(4) == 1 {
                self.burst = 1 + self.random(5);
            } else if self.random(10) != 1 && self.random(10) == 1 {
                self.burst = 5 + self.random(10);
            }
        }
        if self.burst > 0 {
            self.burst -= 1;
        }
        if self
            .random(48)
            .is_multiple_of(if self.fast { 6 } else { 10 })
            && self.burst == 0
        {
            self.last_delay += u64::from(self.random(45) + 40);
        }
        self.phase_count += 1;
        if self.fast {
            if self.phase_count >= self.phase_length {
                self.phase_length = 75 + self.random(125);
                self.fast = false;
                self.phase_count = 0;
            }
            self.last_delay + if self.random(5) == 3 { 50 } else { 25 }
        } else {
            if self.phase_count >= self.phase_length {
                self.fast = true;
                self.phase_length = 7 + self.random(8);
                self.phase_count = 0;
            }
            self.last_delay
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal_range_and_fixed_cps() {
        let mut timing = Timing::new(3);
        assert_eq!(timing.delay(10, 10, Mode::Normal), 100);
        for _ in 0..1000 {
            assert!((50..=142).contains(&timing.delay(6, 20, Mode::Normal)));
        }
    }
    #[test]
    fn extra_retains_bursts_and_phase_state() {
        let mut timing = Timing::new(7);
        let delays: Vec<_> = (0..300).map(|_| timing.delay(6, 13, Mode::Extra)).collect();
        assert!(delays.windows(2).any(|w| w[0] == w[1]));
        assert!(delays.iter().any(|d| *d > 160));
    }
}
