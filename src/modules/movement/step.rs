// Step's `onStepHeightOverride`: after a real step of more than half a
// block, the two intermediate position packets (for steps of 0.87 or more),
// Speed's stage reset and a 100 ms 0.45 game timer. The one-block step
// height itself is `Runtime::step_active`.
//
// The pinned host has no step event, so the step is recognised after the
// move, at the next position send: on the ground both times, the feet rose
// by more than 0.5 and at most the step height, with a short horizontal move.
// The intermediate packets are computed but not sent: a 26.3 server takes one
// position per client tick and disconnects on a second.

/// The step height Step's snapshot sets.
pub const HEIGHT: f64 = 1.0;
/// `setTimerSpeed(0.45f)` for the ResetTimerThread's 100 ms.
pub const TIMER: f32 = 0.45;
pub const TIMER_MS: u64 = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub position: [f64; 3],
    pub on_ground: bool,
    pub sneaking: bool,
}

/// What a recognised step does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stepped {
    /// Position packets to send before this tick's own, `[x, y, z]`.
    pub packets: Vec<[f64; 3]>,
}

#[derive(Clone, Debug, Default)]
pub struct Step {
    last: Option<Sample>,
}

impl Step {
    pub fn reset(&mut self) {
        self.last = None;
    }
    pub fn observe(&mut self, now: Sample) -> Option<Stepped> {
        let last = self.last.replace(now)?;
        let rise = now.position[1] - last.position[1];
        let across = (now.position[0] - last.position[0]).hypot(now.position[2] - last.position[2]);
        if !(last.on_ground
            && now.on_ground
            && !now.sneaking
            && rise > 0.5
            && rise <= HEIGHT + 1e-6
            && across < 1.0)
        {
            return None;
        }
        let [x, y, z] = last.position;
        let packets = if rise >= 0.87 {
            vec![[x, y + rise * 0.42, z], [x, y + rise * 0.75, z]]
        } else {
            Vec::new()
        };
        Some(Stepped { packets })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: f64, z: f64) -> Sample {
        Sample {
            position: [0.5, y, z],
            on_ground: true,
            sneaking: false,
        }
    }

    #[test]
    fn a_full_block_step_sends_the_two_offsets() {
        let mut step = Step::default();
        assert_eq!(step.observe(at(100.0, 0.5)), None);
        let stepped = step.observe(at(101.0, 0.7)).unwrap();
        assert_eq!(
            stepped.packets,
            vec![[0.5, 100.42, 0.5], [0.5, 100.75, 0.5]]
        );
    }

    #[test]
    fn slabs_jumps_and_teleports_are_not_steps() {
        let mut step = Step::default();
        step.observe(at(100.0, 0.5));
        // A half slab.
        assert_eq!(step.observe(at(100.5, 0.7)), None);
        // A low step (0.6 of a block) only slows the timer.
        assert_eq!(
            step.observe(at(101.1, 0.9)).unwrap().packets,
            Vec::<[f64; 3]>::new()
        );
        // Airborne.
        step.observe(Sample {
            on_ground: false,
            ..at(102.0, 1.0)
        });
        assert_eq!(step.observe(at(102.9, 1.1)), None);
        // Teleported upwards.
        assert_eq!(step.observe(at(110.0, 1.1)), None);
        // Sneaking.
        assert_eq!(
            step.observe(Sample {
                sneaking: true,
                ..at(111.0, 1.2)
            }),
            None
        );
    }
}
