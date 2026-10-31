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
    /// The released controller's own return speed (AutoTotem's), instead
    /// of the ticking module's.
    return_step: Option<Step>,
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
        self.return_step = None;
    }
    /// A speed-0 controller (AutoTotem's silent open): the sent rotation
    /// stays where it is until released.
    pub fn hold(&mut self, view: (f32, f32)) {
        self.start(view);
        self.target = None;
    }
    pub fn set_target(&mut self, yaw: f32, pitch: f32) {
        self.relative = false;
        self.target = Some((yaw, pitch.clamp(-90.0, 90.0)));
    }
    /// `releaseController`: return to the view, then hand rotation back.
    pub fn release(&mut self) {
        if self.active {
            self.relative = true;
        }
        self.return_step = None;
    }
    /// `releaseController` after the controller's own speed was set: the
    /// return to the view moves at `step`.
    pub fn release_with(&mut self, step: Step) {
        self.release();
        if self.active {
            self.return_step = Some(step);
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
        let outside = |e: f64| (e.abs() / f64::from(degrees)).round() > 0.0;
        let yaw_done = !outside(yaw_error);
        let pitch_done = !outside(pitch_error);
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
            let step = match (self.relative, self.return_step) {
                (true, Some(step)) => step,
                _ => speed(),
            };
            complete = self.update(target, degrees, step);
        }
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
    fn held_rotation_stays_then_returns_at_its_own_speed() {
        let mut managed = Managed::default();
        managed.hold((10.0, 5.0));
        let fast = || Step {
            counts: 400.0,
            clamp: true,
            proportional: false,
            linear: false,
        };
        // The view turns; the held rotation does not follow.
        assert!(!managed.tick((90.0, 5.0), 0.5, UPDATES_PER_TICK, fast));
        assert_eq!(managed.sent(), Some((10.0, 5.0)));
        managed.release_with(Step {
            counts: 0.75,
            clamp: false,
            proportional: false,
            linear: true,
        });
        let mut ticks = 0;
        while !managed.tick((90.0, 5.0), 0.5, UPDATES_PER_TICK, fast) {
            ticks += 1;
            assert!(ticks < 200, "returns");
        }
        // The 400-count speed would have arrived on the first tick.
        assert!(ticks > 1, "{ticks}");
        assert!(!managed.active);
        // A plain release uses the ticking module's speed again.
        managed.hold((10.0, 5.0));
        managed.release_with(Step {
            counts: 0.75,
            clamp: false,
            proportional: false,
            linear: true,
        });
        managed.release();
        assert!(managed.tick((90.0, 5.0), 0.5, UPDATES_PER_TICK, fast));
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
    fn ray_box_hits_and_misses() {
        let min = [-0.3, 0.0, 2.0];
        let max = [0.3, 1.8, 2.6];
        assert!(ray_box([0.0, 1.0, 0.0], direction(0.0, 0.0), 3.0, min, max).is_some());
        assert!(ray_box([0.0, 1.0, 0.0], direction(90.0, 0.0), 3.0, min, max).is_none());
        assert!(ray_box([0.0, 1.0, 0.0], direction(0.0, 0.0), 1.5, min, max).is_none());
    }
}
