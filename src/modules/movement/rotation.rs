// The movement/world modules' aim (reference `FixedRotationController` on
// `MouseRotationController`, driven by `RotationManager`). The reference
// queues mouse counts: each update adds `speed * 0.25` counts (plus 5% of
// the remaining error with linear acceleration) towards the target, applied
// in whole counts of `mouseScale * 0.15` degrees, about 50 updates per tick.
// A silent controller (the reference's `AdaptiveRotationController`) moves a
// managed rotation that only the server sees; a visible one turns the view.
// This is a small isolated owner for these modules until a shared rotation
// owner exists; only one movement/world module aims at a time.

/// `FixedRotationController.UNSET_ROTATION`: that axis is left alone.
pub const UNSET: f32 = -999.0;

/// `MathHelper.wrapAngleTo180_double`.
pub fn wrap(angle: f64) -> f64 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `getMouseScale` for a mouse sensitivity in [0, 1] (0.5 by default).
pub fn mouse_scale(sensitivity: f32) -> f32 {
    let base = sensitivity * 0.6 + 0.2;
    base * base * base * 8.0
}

#[derive(Clone, Debug, PartialEq)]
pub struct Controller {
    pub target: [f32; 2],
    pub speed: f32,
    pub tolerance: f32,
    pub linear_acceleration: bool,
    pub scale_axes: bool,
    /// Server-only rotation (AdaptiveRotationController) rather than the view.
    pub silent: bool,
    pending: [f32; 2],
    complete: bool,
}

impl Controller {
    pub fn new(target: [f32; 2], speed: f32, silent: bool) -> Self {
        Self {
            target,
            speed,
            tolerance: 3.0,
            linear_acceleration: false,
            scale_axes: false,
            silent,
            pending: [0.0; 2],
            complete: false,
        }
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn set_target(&mut self, target: [f32; 2]) {
        self.target = target;
        self.complete = false;
    }
    fn outside(&self, error: f64, per_count: f32) -> bool {
        let per_count = f64::from(per_count);
        (error / per_count).round() > (f64::from(self.tolerance) / per_count).round().max(0.0)
    }
    fn accelerate(&self, step: f64, error: f64) -> f64 {
        if self.linear_acceleration {
            step + error * 0.05
        } else {
            step
        }
    }
    /// One `update` and `applyPendingMovement` from `[yaw, pitch]`.
    fn update(&mut self, rotation: [f32; 2], scale: f32) -> [f32; 2] {
        let per_count = scale * 0.15;
        let predicted = [
            rotation[0] + self.pending[0].trunc() * per_count,
            rotation[1] + self.pending[1].trunc() * per_count,
        ];
        let error = [
            wrap(f64::from((self.target[0] - predicted[0]) % 360.0)),
            wrap(f64::from((self.target[1] - predicted[1]) % 360.0)),
        ];
        let mut done = [true; 2];
        for axis in 0..2 {
            if self.target[axis] == UNSET {
                continue;
            }
            let absolute = error[axis].abs();
            if !self.outside(absolute, per_count) {
                continue;
            }
            done[axis] = false;
            let mut step = f64::from(self.speed) * 0.25;
            let ratio = absolute / error[1 - axis].abs();
            if self.scale_axes && ratio < 1.0 {
                step *= ratio;
            }
            let step = self.accelerate(step, absolute) as f32;
            self.pending[axis] += if error[axis] > 0.0 { step } else { -step };
        }
        if done[0] && done[1] && self.pending[0].abs() < 1.0 && self.pending[1].abs() < 1.0 {
            self.complete = true;
        }
        // applyPendingMovement: whole counts, the remainder kept.
        let counts = [self.pending[0].trunc(), self.pending[1].trunc()];
        self.pending = [self.pending[0] - counts[0], self.pending[1] - counts[1]];
        [
            rotation[0] + counts[0] * per_count,
            (rotation[1] + counts[1] * per_count).clamp(-90.0, 90.0),
        ]
    }
    /// One client tick's 50 updates (`Math.round(50 * timeScale)`).
    pub fn tick(&mut self, rotation: [f32; 2], sensitivity: f32) -> [f32; 2] {
        let scale = mouse_scale(sensitivity);
        let mut rotation = rotation;
        for _ in 0..50 {
            if self.complete {
                break;
            }
            rotation = self.update(rotation, scale);
        }
        rotation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_only_controller_reaches_the_ground_and_keeps_yaw() {
        let mut c = Controller::new([UNSET, 90.0], 7.0, false);
        c.tolerance = 5.0;
        c.linear_acceleration = true;
        let mut r = [33.0, 0.0];
        let mut ticks = 0;
        while !c.complete() && ticks < 20 {
            r = c.tick(r, 0.5);
            ticks += 1;
        }
        assert!(c.complete(), "not complete after {ticks} ticks");
        assert_eq!(r[0], 33.0);
        assert!(r[1] > 84.0 && r[1] <= 90.0, "pitch {}", r[1]);
        assert!(ticks <= 5, "{ticks}");
    }

    #[test]
    fn yaw_turns_the_short_way_round() {
        let mut c = Controller::new([-170.0, UNSET], 12.0, false);
        let mut r = [170.0, 10.0];
        for _ in 0..10 {
            r = c.tick(r, 0.5);
        }
        assert!(c.complete());
        assert!(wrap(f64::from(r[0] + 170.0)).abs() <= 3.5, "yaw {}", r[0]);
        // Turned through 180, not back through 0.
        assert!(r[0] > 170.0 || r[0] < -160.0);
    }

    #[test]
    fn wrap_matches_minecraft() {
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
        assert_eq!(wrap(180.0), -180.0);
    }
}
