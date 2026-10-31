// The shared rotation owner (reference `SharedModuleControlClaims.rotation`,
// `RotationControlClaim` and the `AdaptiveRotationController` driven by
// `RotationManager`). One claim arbitrates every rotating module by priority;
// the owner drives a silent managed rotation that the host sends in its
// movement packets instead of the camera's view.
//
// The managed rotation moves in whole mouse counts like the reference's
// controller (`addPendingStep`/`applyPendingMovement`), so every step is a
// multiple of the sensitivity's degrees-per-count, and is advanced the
// reference's 50 controller updates per client tick.
//
// The reference's `RotationManager` runs one controller at a time: the
// silent `Managed` rotation (`AdaptiveRotationController`) or a visible
// `Fixed` one (`FixedRotationController`, `PointRotationController`) that
// turns the view itself. The movement and world modules drive either through
// `core::rotation_runtime` under the same claim.

/// Every module that may own the rotation, with the reference priorities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claimant {
    SilentAura,
    AutoMace,
    LagBlockHit,
    KnockbackDelay,
    HitFlick,
    CrystalAura,
    Scaffold,
    WindCharge,
    BowAimbot,
    AutoPearl,
    PearlCatch,
    AutoAnchor,
    BlockIn,
    Mlg,
    AutoLadder,
    Clutch,
    AutoTotem,
    Freecam,
}
impl Claimant {
    pub fn priority(self) -> u32 {
        match self {
            Self::SilentAura | Self::AutoMace | Self::LagBlockHit | Self::KnockbackDelay => 5,
            Self::HitFlick | Self::CrystalAura | Self::Scaffold | Self::WindCharge => 6,
            Self::BowAimbot | Self::AutoPearl | Self::PearlCatch => 7,
            Self::AutoAnchor => 8,
            Self::BlockIn => 9,
            Self::Mlg => 10,
            Self::AutoLadder => 50,
            Self::Clutch => 60,
            Self::AutoTotem => 99,
            Self::Freecam => 100,
        }
    }
}

/// `RotationControlClaim`: an owner and the higher-priority requester
/// waiting for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Claim {
    owner: Option<Claimant>,
    pending: Option<Claimant>,
}
impl Claim {
    pub fn owner(&self) -> Option<Claimant> {
        self.owner
    }
    pub fn owned_by(&self, who: Claimant) -> bool {
        self.owner == Some(who)
    }
    /// `tryClaim`: free (and nobody else waiting) claims it; otherwise a
    /// higher-priority requester queues as the pending owner.
    pub fn try_claim(&mut self, who: Claimant) -> bool {
        if self.owner == Some(who) {
            return true;
        }
        if self.owner.is_none() && self.pending.is_none_or(|p| p == who) {
            self.owner = Some(who);
            self.pending = None;
            return true;
        }
        if self
            .owner
            .is_some_and(|owner| who.priority() > owner.priority())
            && self
                .pending
                .is_none_or(|pending| who.priority() >= pending.priority())
        {
            self.pending = Some(who);
        }
        false
    }
    /// `acquire(module, allowPreemption)`: a higher-priority requester (or
    /// any requester while the owner's rotation is returning to the view)
    /// takes the claim from the current owner.
    pub fn acquire(&mut self, who: Claimant, preempt: bool, returning: bool) -> bool {
        if self.try_claim(who) {
            return true;
        }
        let Some(owner) = self.owner else {
            return false;
        };
        if preempt && (who.priority() > owner.priority() || returning) {
            self.owner = Some(who);
            if self.pending == Some(who) {
                self.pending = None;
            }
            return true;
        }
        false
    }
    /// `isBlockedFor`: someone else owns it, or waits for it with a higher
    /// priority.
    pub fn blocked_for(&self, who: Claimant) -> bool {
        self.owner.is_some_and(|o| o != who)
            || self
                .pending
                .is_some_and(|p| p != who && p.priority() > who.priority())
    }
    pub fn release(&mut self, who: Claimant) {
        if self.owner == Some(who) {
            self.owner = self.pending.take();
        } else if self.pending == Some(who) {
            self.pending = None;
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Degrees one mouse count turns the view (`getMouseScale() * 0.15`).
pub fn degrees_per_count(sensitivity: f64) -> f32 {
    let base = sensitivity as f32 * 0.6 + 0.2;
    base * base * base * 8.0 * 0.15
}

pub fn wrap(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// The rotation from `eye` to `target`, in Minecraft's yaw/pitch.
pub fn look_at(eye: [f64; 3], target: [f64; 3]) -> (f32, f32) {
    let dx = target[0] - eye[0];
    let dy = target[1] - eye[1];
    let dz = target[2] - eye[2];
    let horizontal = (dx * dx + dz * dz).sqrt();
    let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
    let pitch = (-dy.atan2(horizontal).to_degrees()) as f32;
    (wrap(yaw), pitch.clamp(-90.0, 90.0))
}

/// The unit view vector for a rotation.
pub fn direction(yaw: f32, pitch: f32) -> [f64; 3] {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ]
}

/// Ray/box distance along `dir` from `eye`, within `reach`.
pub fn ray_box(
    eye: [f64; 3],
    dir: [f64; 3],
    reach: f64,
    min: [f64; 3],
    max: [f64; 3],
) -> Option<f64> {
    if (0..3).all(|i| eye[i] >= min[i] && eye[i] <= max[i]) {
        return Some(0.0);
    }
    let (mut near, mut far) = (0.0f64, reach);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if eye[i] < min[i] || eye[i] > max[i] {
                return None;
            }
            continue;
        }
        let a = (min[i] - eye[i]) / dir[i];
        let b = (max[i] - eye[i]) / dir[i];
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// How the managed rotation moves this update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Mouse counts per controller update (`getSpeed() * 0.25`).
    pub counts: f64,
    /// `setClampStepToRemaining`.
    pub clamp: bool,
    /// `setScaleAxesProportionally`.
    pub proportional: bool,
    /// `setLinearAcceleration` (`step + error * 0.05`).
    pub linear: bool,
}

/// The adaptive (silent) managed rotation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Managed {
    pub active: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub previous_yaw: f32,
    pub previous_pitch: f32,
    /// Returning to the view after release (`setRelativeMode(true)`).
    pub relative: bool,
    pending_yaw: f32,
    pending_pitch: f32,
    target: Option<(f32, f32)>,
    /// `setTolerance` (0 for SilentAura).
    tolerance: f32,
    /// `isComplete` of the last update.
    complete: bool,
}
impl Managed {
    /// `setController`: starts from the current view.
    pub fn start(&mut self, view: (f32, f32)) {
        if !self.active {
            *self = Self {
                active: true,
                yaw: view.0,
                pitch: view.1,
                previous_yaw: view.0,
                previous_pitch: view.1,
                ..Self::default()
            };
        }
        self.relative = false;
        self.tolerance = 0.0;
    }
    /// `setTargetRotation`; an `UNSET` axis is left where it is.
    pub fn set_target(&mut self, yaw: f32, pitch: f32) {
        self.relative = false;
        self.complete = false;
        let pitch = if pitch == UNSET {
            UNSET
        } else {
            pitch.clamp(-90.0, 90.0)
        };
        self.target = Some((yaw, pitch));
    }
    pub fn set_tolerance(&mut self, tolerance: f32) {
        self.tolerance = tolerance;
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    /// `releaseController`: return to the view, then hand rotation back.
    pub fn release(&mut self) {
        if self.active {
            self.relative = true;
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// The rotation sent this tick, if any.
    pub fn sent(&self) -> Option<(f32, f32)> {
        self.active.then_some((self.yaw, self.pitch))
    }
    fn axis(pending: f32, error: f64, mut step: f64, other: f64, degrees: f32, s: Step) -> f32 {
        let ratio = error.abs() / other.abs();
        if s.proportional && ratio < 1.0 {
            step *= ratio;
        }
        if s.linear {
            step += error.abs() * 0.05;
        }
        if s.clamp {
            step = step.min((error / f64::from(degrees)).abs());
        }
        (if error > 0.0 {
            f64::from(pending) + step
        } else {
            f64::from(pending) - step
        }) as f32
    }
    fn update(&mut self, target: (f32, f32), degrees: f32, step: Step) -> bool {
        let predicted_yaw = self.yaw + (self.pending_yaw as i32) as f32 * degrees;
        let predicted_pitch = self.pitch - (-self.pending_pitch as i32) as f32 * degrees;
        let yaw_error = f64::from(wrap((target.0 - predicted_yaw) % 360.0));
        let pitch_error = f64::from(wrap((target.1 - predicted_pitch) % 360.0));
        let limit = (f64::from(self.tolerance) / f64::from(degrees))
            .round()
            .max(0.0);
        let outside = |e: f64| (e.abs() / f64::from(degrees)).round() > limit;
        let yaw_done = target.0 == UNSET || !outside(yaw_error);
        let pitch_done = target.1 == UNSET || !outside(pitch_error);
        if !yaw_done {
            self.pending_yaw = Self::axis(
                self.pending_yaw,
                yaw_error,
                step.counts,
                pitch_error,
                degrees,
                step,
            );
        }
        if !pitch_done {
            self.pending_pitch = Self::axis(
                self.pending_pitch,
                pitch_error,
                step.counts,
                yaw_error,
                degrees,
                step,
            );
        }
        let complete = yaw_done
            && pitch_done
            && self.pending_yaw.abs() < 1.0
            && self.pending_pitch.abs() < 1.0;
        // applyPendingMovement: whole counts move the managed rotation.
        let yaw_counts = self.pending_yaw as i32;
        let pitch_counts = self.pending_pitch as i32;
        self.pending_yaw -= yaw_counts as f32;
        self.pending_pitch -= pitch_counts as f32;
        self.yaw += yaw_counts as f32 * degrees;
        self.pitch = (self.pitch + pitch_counts as f32 * degrees).clamp(-90.0, 90.0);
        complete
    }
    /// One client tick: `updates` controller updates toward the target (or
    /// back to `view` when returning). `speed` gives each update's counts.
    /// Returns true when the managed rotation ended this tick.
    pub fn tick(
        &mut self,
        view: (f32, f32),
        sensitivity: f64,
        updates: u32,
        mut speed: impl FnMut() -> Step,
    ) -> bool {
        if !self.active {
            return false;
        }
        self.previous_yaw = self.yaw;
        self.previous_pitch = self.pitch;
        let degrees = degrees_per_count(sensitivity).max(1e-4);
        let target = if self.relative {
            Some(view)
        } else {
            self.target
        };
        let Some(target) = target else {
            return false;
        };
        let mut complete = false;
        for _ in 0..updates {
            complete = self.update(target, degrees, speed());
        }
        self.complete = complete;
        if self.relative
            && (complete
                || (wrap(view.0 - self.yaw).abs() <= degrees
                    && (view.1 - self.pitch).abs() <= degrees))
        {
            self.clear();
            return true;
        }
        false
    }
}

/// `FixedRotationController.UNSET_ROTATION`: that axis is left alone.
pub const UNSET: f32 = -999.0;

/// A visible controller (`FixedRotationController`): queues mouse counts that
/// turn the view itself, `speed * 0.25` counts per update (plus 5% of the
/// remaining error with linear acceleration).
#[derive(Clone, Debug, PartialEq)]
pub struct Fixed {
    pub target: [f32; 2],
    pub speed: f32,
    /// `MouseRotationController.tolerance` (3 by default).
    pub tolerance: f32,
    pub linear_acceleration: bool,
    pub scale_axes: bool,
    /// `setClampStepToRemaining`.
    pub clamp: bool,
    /// `setRetainAfterCompletion`: stays active once complete.
    pub retain: bool,
    pending: [f32; 2],
    complete: bool,
}

impl Fixed {
    pub fn new(target: [f32; 2], speed: f32) -> Self {
        Self {
            target,
            speed,
            tolerance: 3.0,
            linear_acceleration: false,
            scale_axes: false,
            clamp: false,
            retain: false,
            pending: [0.0; 2],
            complete: false,
        }
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn set_complete(&mut self, complete: bool) {
        self.complete = complete;
    }
    /// `setTargetRotation`.
    pub fn set_target(&mut self, target: [f32; 2]) {
        self.target = target;
        self.complete = false;
    }
    /// `isComplete() && !shouldRetainAfterCompletion()`: the manager drops it.
    pub fn finished(&self) -> bool {
        self.complete && !self.retain
    }
    fn outside(&self, error: f64, per_count: f32) -> bool {
        let per_count = f64::from(per_count);
        (error / per_count).round() > (f64::from(self.tolerance) / per_count).round().max(0.0)
    }
    /// `updateYaw` (axis 0) or `updatePitch` (axis 1) from `rotation`;
    /// true when that axis is done.
    pub fn update_axis(&mut self, axis: usize, rotation: [f32; 2], per_count: f32) -> bool {
        if self.target[axis] == UNSET {
            return true;
        }
        let pitch = if axis == 1 && rotation[1] == -90.0 {
            -89.99
        } else {
            rotation[1]
        };
        let predicted = [
            rotation[0] + self.pending[0].trunc() * per_count,
            pitch + self.pending[1].trunc() * per_count,
        ];
        let error = [
            f64::from(wrap((self.target[0] - predicted[0]) % 360.0)),
            f64::from(wrap((self.target[1] - predicted[1]) % 360.0)),
        ];
        let absolute = error[axis].abs();
        if !self.outside(absolute, per_count) {
            return true;
        }
        let mut step = f64::from(self.speed) * 0.25;
        let ratio = absolute / error[1 - axis].abs();
        if self.scale_axes && ratio < 1.0 {
            step *= ratio;
        }
        if self.linear_acceleration {
            step += absolute * 0.05;
        }
        if self.clamp {
            step = step.min((error[axis] / f64::from(per_count)).abs());
        }
        let step = step as f32;
        self.pending[axis] += if error[axis] > 0.0 { step } else { -step };
        false
    }
    /// `applyPendingMovement`: whole counts turn `rotation`, the remainder
    /// is kept.
    pub fn apply(&mut self, rotation: [f32; 2], per_count: f32) -> [f32; 2] {
        let counts = [self.pending[0].trunc(), self.pending[1].trunc()];
        self.pending = [self.pending[0] - counts[0], self.pending[1] - counts[1]];
        [
            rotation[0] + counts[0] * per_count,
            (rotation[1] + counts[1] * per_count).clamp(-90.0, 90.0),
        ]
    }
    /// `MouseRotationController.update` then `applyPendingMovement`.
    pub fn update(&mut self, rotation: [f32; 2], per_count: f32) -> [f32; 2] {
        let yaw = self.update_axis(0, rotation, per_count);
        let pitch = self.update_axis(1, rotation, per_count);
        if yaw && pitch && self.pending[0].abs() < 1.0 && self.pending[1].abs() < 1.0 {
            self.complete = true;
        }
        self.apply(rotation, per_count)
    }
    /// One client tick's updates until it finishes.
    pub fn tick(&mut self, rotation: [f32; 2], sensitivity: f64, updates: u32) -> [f32; 2] {
        let per_count = degrees_per_count(sensitivity).max(1e-4);
        let mut rotation = rotation;
        for _ in 0..updates {
            if self.finished() {
                break;
            }
            rotation = self.update(rotation, per_count);
        }
        rotation
    }
}

/// The reference's 50 controller updates per tick
/// (`Math.round(50 * getSensitivityTimeScale())` with a unit scale).
pub const UPDATES_PER_TICK: u32 = 50;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_priority_preempts_and_lower_waits() {
        let mut claim = Claim::default();
        assert!(claim.try_claim(Claimant::SilentAura));
        assert!(!claim.try_claim(Claimant::BowAimbot));
        assert!(claim.blocked_for(Claimant::SilentAura));
        assert!(claim.acquire(Claimant::BowAimbot, true, false));
        assert!(claim.owned_by(Claimant::BowAimbot));
        assert!(!claim.acquire(Claimant::SilentAura, true, false));
        assert!(claim.acquire(Claimant::SilentAura, true, true));
        claim.release(Claimant::SilentAura);
        assert_eq!(claim.owner(), None);
    }

    #[test]
    fn pending_owner_takes_over_on_release() {
        let mut claim = Claim::default();
        assert!(claim.try_claim(Claimant::SilentAura));
        assert!(!claim.try_claim(Claimant::HitFlick));
        claim.release(Claimant::SilentAura);
        assert!(claim.owned_by(Claimant::HitFlick));
    }

    #[test]
    fn look_at_matches_minecraft_axes() {
        let (yaw, pitch) = look_at([0.0, 0.0, 0.0], [0.0, 0.0, 5.0]);
        assert!(yaw.abs() < 1e-4 && pitch.abs() < 1e-4);
        let (yaw, _) = look_at([0.0, 0.0, 0.0], [-5.0, 0.0, 0.0]);
        assert!((yaw - 90.0).abs() < 1e-4);
        let (_, pitch) = look_at([0.0, 0.0, 0.0], [0.0, -5.0, 0.001]);
        assert!(pitch > 89.0);
        let d = direction(90.0, 0.0);
        assert!((d[0] + 1.0).abs() < 1e-9);
    }

    #[test]
    fn managed_rotation_moves_in_counts_reaches_target_and_returns() {
        let mut m = Managed::default();
        m.start((0.0, 0.0));
        m.set_target(60.0, 20.0);
        let step = || Step {
            counts: 7.0 * 2.0 * 0.25,
            clamp: true,
            proportional: false,
            linear: false,
        };
        let degrees = degrees_per_count(0.5);
        m.tick((0.0, 0.0), 0.5, UPDATES_PER_TICK, step);
        assert!(m.yaw > 0.0 && m.yaw < 60.0);
        let counts = m.yaw / degrees;
        assert!((counts - counts.round()).abs() < 1e-3);
        for _ in 0..20 {
            m.tick((0.0, 0.0), 0.5, UPDATES_PER_TICK, step);
        }
        assert!((m.yaw - 60.0).abs() <= degrees && (m.pitch - 20.0).abs() <= degrees);
        m.release();
        let mut ended = false;
        for _ in 0..40 {
            ended |= m.tick((0.0, 0.0), 0.5, UPDATES_PER_TICK, step);
        }
        assert!(ended && !m.active && m.sent().is_none());
    }

    #[test]
    fn fixed_pitch_only_reaches_the_ground_and_keeps_yaw() {
        let mut c = Fixed::new([UNSET, 90.0], 7.0);
        c.tolerance = 5.0;
        c.linear_acceleration = true;
        let mut r = [33.0, 0.0];
        let mut ticks = 0;
        while !c.complete() && ticks < 20 {
            r = c.tick(r, 0.5, UPDATES_PER_TICK);
            ticks += 1;
        }
        assert!(c.complete(), "not complete after {ticks} ticks");
        assert_eq!(r[0], 33.0);
        assert!(r[1] > 84.0 && r[1] <= 90.0, "pitch {}", r[1]);
        assert!(ticks <= 5, "{ticks}");
    }

    #[test]
    fn fixed_yaw_turns_the_short_way_round() {
        let mut c = Fixed::new([-170.0, UNSET], 12.0);
        let mut r = [170.0, 10.0];
        for _ in 0..10 {
            r = c.tick(r, 0.5, UPDATES_PER_TICK);
        }
        assert!(c.complete());
        assert!(wrap(r[0] + 170.0).abs() <= 3.5, "yaw {}", r[0]);
        assert!(r[0] > 170.0 || r[0] < -160.0);
    }

    #[test]
    fn fixed_clamped_and_retained_settles_without_overshoot() {
        let mut c = Fixed::new([40.0, 30.0], 12.0);
        c.tolerance = 0.5;
        c.clamp = true;
        c.linear_acceleration = true;
        c.retain = true;
        let mut r = [0.0, 0.0];
        for _ in 0..20 {
            r = c.tick(r, 0.5, UPDATES_PER_TICK);
        }
        assert!(c.complete() && !c.finished());
        // Within the tolerance, never past the target.
        let near = c.tolerance + degrees_per_count(0.5);
        assert!(r[0] <= 40.0 && r[1] <= 30.0, "{r:?}");
        assert!(40.0 - r[0] <= near && 30.0 - r[1] <= near, "{r:?}");
    }

    #[test]
    fn managed_unset_axis_stays_and_tolerance_completes() {
        let mut m = Managed::default();
        m.start((25.0, 0.0));
        m.set_target(UNSET, 90.0);
        m.set_tolerance(5.0);
        let step = || Step {
            counts: 7.0 * 0.25,
            clamp: false,
            proportional: false,
            linear: true,
        };
        for _ in 0..10 {
            m.tick((25.0, 0.0), 0.5, UPDATES_PER_TICK, step);
        }
        assert!(m.complete());
        assert_eq!(m.yaw, 25.0);
        assert!(m.pitch > 84.0, "{}", m.pitch);
    }

    #[test]
    fn wrap_matches_minecraft() {
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
        assert_eq!(wrap(180.0), -180.0);
    }

    #[test]
    fn ray_box_hits_and_misses() {
        let min = [-0.3, 0.0, 2.0];
        let max = [0.3, 1.8, 2.6];
        assert!(ray_box([0.0, 1.0, 0.0], direction(0.0, 0.0), 3.0, min, max).is_some());
        assert!(ray_box([0.0, 1.0, 0.0], direction(90.0, 0.0), 3.0, min, max).is_none());
        assert!(ray_box([0.0, 1.0, 0.0], direction(0.0, 0.0), 1.5, min, max).is_none());
    }
}
