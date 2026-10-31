// The mouse rotation controllers AutoPearl and PearlCatch aim with
// (reference `FixedRotationController`, `PointRotationController`,
// `AdaptiveRotationController`, their `MouseRotationController` base and
// `AutoPearlRotationController`/`AutoPearlPointRotationController`, which pin
// the pitch while the yaw follows a point).
//
// A visible controller turns the camera in whole mouse counts; a silent
// (adaptive) one turns the shared managed rotation the host sends instead,
// and returns it to the view when released (`setRelativeMode(true)`).
// `RotationManager` advances the active controller 50 updates per client
// tick; each update runs `update` then `applyPendingMovement`.
use super::super::combat::rotation::{UPDATES_PER_TICK, look_at, wrap};

/// The reference's `-999` "no target on this axis" marker.
pub const UNSET: f32 = -999.0;

/// The controller's tuning (`setSpeed`, `setTolerance`, `setClampStepToRemaining`,
/// `setScaleAxesProportionally`, the acceleration flags, `setRandomizeMovement`
/// and `setRetainAfterCompletion`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub speed: f32,
    pub tolerance: f32,
    pub clamp: bool,
    pub proportional: bool,
    pub angle_based: bool,
    pub linear: bool,
    pub cubic: bool,
    pub randomize: bool,
    pub retain: bool,
}
impl Default for Options {
    /// `MouseRotationController`'s field defaults.
    fn default() -> Self {
        Self {
            speed: 1.0,
            tolerance: 3.0,
            clamp: false,
            proportional: false,
            angle_based: false,
            linear: false,
            cubic: false,
            randomize: false,
            retain: false,
        }
    }
}

/// What the controller aims at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// `setTargetRotation(yaw, pitch)`.
    Rotation(f32, f32),
    /// The yaw toward a point from the eye, refreshed every update, with a
    /// fixed pitch (AutoPearl's point controllers).
    Point([f64; 3], f32),
}

/// One update's inputs that do not change within a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The camera's rotation.
    pub view: (f32, f32),
    pub eye: [f64; 3],
    pub sensitivity: f64,
    /// A screen is open (`GuiScreen.isNotNull()`).
    pub screen: bool,
    /// Freecam owns the camera, so mouse deltas are not applied.
    pub freecam: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Controller {
    pub options: Options,
    /// Adaptive: drives the managed rotation instead of the camera.
    pub silent: bool,
    target: Target,
    pending_yaw: f32,
    pending_pitch: f32,
    complete: bool,
    /// Adaptive `relativeMode`: returning to the view.
    relative: bool,
    jitter_ticks: i32,
    yaw_direction: i32,
    pitch_direction: i32,
    yaw_jitter: i32,
    pitch_jitter: i32,
    rng: u64,
}

/// `getMouseScale()`: the host's mouse-delta units per count.
pub fn mouse_scale(sensitivity: f64) -> f32 {
    let base = sensitivity as f32 * 0.6 + 0.2;
    base * base * base * 8.0
}

impl Controller {
    pub fn new(options: Options, silent: bool, target: Target, seed: u64) -> Self {
        Self {
            options,
            silent,
            target,
            pending_yaw: 0.0,
            pending_pitch: 0.0,
            complete: false,
            relative: false,
            jitter_ticks: 0,
            yaw_direction: 0,
            pitch_direction: 0,
            yaw_jitter: 0,
            pitch_jitter: 0,
            rng: seed | 1,
        }
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn relative(&self) -> bool {
        self.relative
    }
    pub fn target(&self) -> Target {
        self.target
    }
    /// `setTargetRotation`: a new target is not complete yet.
    pub fn set_target(&mut self, target: Target) {
        self.target = target;
        self.complete = false;
    }
    /// `releaseController` on an adaptive controller: return to the view.
    pub fn release(&mut self) {
        self.relative = true;
        self.complete = false;
    }
    /// `shouldRetainAfterCompletion`: the manager drops a complete
    /// controller that is not retained (an adaptive one only once it has
    /// returned).
    pub fn retained(&self) -> bool {
        if self.silent {
            !self.relative
        } else {
            self.options.retain
        }
    }
    /// Whether the manager still runs this controller.
    pub fn alive(&self) -> bool {
        !(self.complete && !self.retained())
    }

    fn next(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
    /// `Random.nextInt(bound)`.
    fn below(&mut self, bound: i32) -> i32 {
        (self.next() % bound as u64) as i32
    }
    /// `MathUtil.randomExclusiveUpper(random, min, max)`.
    fn between(&mut self, min: i32, max: i32) -> i32 {
        min + self.below(max - min)
    }

    fn target_rotation(&self, frame: &Frame) -> (f32, f32) {
        if self.silent && self.relative {
            return frame.view;
        }
        match self.target {
            Target::Rotation(yaw, pitch) => (yaw, pitch),
            Target::Point(point, pitch) => (look_at(frame.eye, point).0, pitch),
        }
    }

    fn outside(&self, error: f32, degrees: f32) -> bool {
        let tolerance = (self.options.tolerance / degrees).round().max(0.0);
        (error.abs() / degrees).round() > tolerance
    }
    fn yaw_acceleration(&self, step: f32, error: f32) -> f32 {
        let o = &self.options;
        if o.angle_based {
            step * (225.0 + error) / 180.0
        } else if o.linear {
            step + error * 0.05
        } else if o.cubic {
            let s = error / 100.0 + 0.7;
            step * (0.4 + 2.0 * s * s * s + s * s).clamp(1.0, 4.0)
        } else {
            step
        }
    }
    fn pitch_acceleration(&self, step: f32, error: f32) -> f32 {
        let o = &self.options;
        if o.angle_based {
            step * (135.0 + error) / 90.0
        } else if o.linear {
            step + error * 0.05
        } else if o.cubic {
            let s = error / 75.0 + 0.7;
            step * (0.4 + 2.0 * s * s * s + s * s).max(1.0)
        } else {
            step
        }
    }
    fn pending_step(&self, pending: f32, error: f32, mut step: f32, degrees: f32) -> f32 {
        if self.options.clamp {
            step = step.min((error / degrees).abs());
        }
        if error > 0.0 {
            pending + step
        } else {
            pending - step
        }
    }
    fn errors(&self, current: (f32, f32), target: (f32, f32), degrees: f32) -> (f32, f32) {
        let predicted_yaw = current.0 + (self.pending_yaw as i32) as f32 * degrees;
        let predicted_pitch = current.1 - (-self.pending_pitch as i32) as f32 * degrees;
        (
            wrap((target.0 - predicted_yaw) % 360.0),
            wrap((target.1 - predicted_pitch) % 360.0),
        )
    }
    fn update_yaw(&mut self, current: (f32, f32), target: (f32, f32), degrees: f32) -> bool {
        if target.0 == UNSET {
            return true;
        }
        let (yaw_error, pitch_error) = self.errors(current, target, degrees);
        if !self.outside(yaw_error, degrees) {
            return true;
        }
        let mut step = self.options.speed * 0.25;
        let ratio = yaw_error.abs() / pitch_error.abs();
        if self.options.proportional && ratio < 1.0 {
            step *= ratio;
        }
        let step = self.yaw_acceleration(step, yaw_error.abs());
        self.pending_yaw = self.pending_step(self.pending_yaw, yaw_error, step, degrees);
        false
    }
    fn update_pitch(&mut self, current: (f32, f32), target: (f32, f32), degrees: f32) -> bool {
        if target.1 == UNSET {
            return true;
        }
        let current = (
            current.0,
            if current.1 == -90.0 {
                -89.99
            } else {
                current.1
            },
        );
        let (yaw_error, pitch_error) = self.errors(current, target, degrees);
        if !self.outside(pitch_error, degrees) {
            return true;
        }
        let mut step = self.options.speed * 0.25;
        let ratio = pitch_error.abs() / yaw_error.abs();
        if self.options.proportional && ratio < 1.0 {
            step *= ratio;
        }
        let step = self.pitch_acceleration(step, pitch_error.abs());
        self.pending_pitch = self.pending_step(self.pending_pitch, pitch_error, step, degrees);
        false
    }
    /// `update`: a visible controller does nothing behind a screen; a
    /// visible completion sticks, an adaptive one is recomputed every update
    /// (an adaptive controller retains itself until it is returning, so it
    /// keeps holding its aim until released).
    fn update(&mut self, current: (f32, f32), frame: &Frame, degrees: f32) {
        if frame.screen && !self.silent {
            return;
        }
        let target = self.target_rotation(frame);
        let yaw = self.update_yaw(current, target, degrees);
        let pitch = self.update_pitch(current, target, degrees);
        let done = yaw && pitch && self.pending_yaw.abs() < 1.0 && self.pending_pitch.abs() < 1.0;
        if !self.silent {
            if done {
                self.complete = true;
            }
        } else {
            self.complete = done;
        }
    }
    /// `updateJitter`, once per elapsed millisecond.
    fn update_jitter(&mut self) {
        self.jitter_ticks += 1;
        if self.jitter_ticks >= 250 + self.below(50) {
            self.jitter_ticks = self.between(-100, -50);
            self.yaw_direction = self.between(-1, 2);
            self.pitch_direction = -self.between(-1, 2);
        }
        let mut yaw = self.yaw_direction;
        let mut pitch = self.pitch_direction;
        if self.below(10) < 2 {
            yaw = 0;
        }
        if self.below(10) < 2 {
            pitch = 0;
        }
        if self.jitter_ticks < 0 {
            yaw = 0;
            pitch = 0;
        }
        if self.below(20) == 1 {
            self.yaw_jitter += yaw;
            self.pitch_jitter += pitch;
        }
        if (self.pending_yaw > 0.0 && self.yaw_jitter < 0)
            || (self.pending_yaw < 0.0 && self.yaw_jitter > 0)
        {
            self.yaw_jitter = 0;
        }
    }
    /// `applyPendingMovement`: whole counts turn the rotation (not behind a
    /// screen or while Freecam holds the camera); the remainder is kept.
    fn apply(&mut self, current: &mut (f32, f32), frame: &Frame, elapsed_ms: u32) {
        if self.options.randomize {
            for _ in 0..elapsed_ms {
                self.update_jitter();
            }
            self.pending_yaw += self.yaw_jitter as f32;
            self.pending_pitch += self.pitch_jitter as f32;
        }
        let yaw_counts = self.pending_yaw as i32;
        let pitch_counts = self.pending_pitch as i32;
        self.pending_yaw -= yaw_counts as f32;
        self.pending_pitch -= pitch_counts as f32;
        self.yaw_jitter = 0;
        self.pitch_jitter = 0;
        if !frame.freecam && !frame.screen {
            // Entity.turn: yaw += dx * 0.15, pitch -= -dy * 0.15, clamped.
            let scale = mouse_scale(frame.sensitivity);
            current.0 += yaw_counts as f32 * scale * 0.15;
            current.1 = (current.1 + pitch_counts as f32 * scale * 0.15).clamp(-90.0, 90.0);
        }
    }
    /// One client tick of the manager's 50 updates on `current` (the view
    /// for a visible controller, the managed rotation for a silent one).
    /// Returns false once the manager would drop the controller.
    pub fn tick(&mut self, current: &mut (f32, f32), frame: &Frame) -> bool {
        let degrees = (mouse_scale(frame.sensitivity) * 0.15).max(1e-4);
        for _ in 0..UPDATES_PER_TICK {
            if !self.alive() {
                return false;
            }
            self.update(*current, frame, degrees);
            self.apply(current, frame, 1);
        }
        self.alive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(view: (f32, f32)) -> Frame {
        Frame {
            view,
            eye: [0.0, 1.62, 0.0],
            sensitivity: 0.5,
            screen: false,
            freecam: false,
        }
    }
    fn catch_options() -> Options {
        Options {
            speed: 10.0,
            tolerance: 0.35,
            clamp: true,
            proportional: true,
            linear: true,
            retain: true,
            ..Options::default()
        }
    }

    #[test]
    fn visible_controller_reaches_the_target_in_whole_counts() {
        let mut c = Controller::new(catch_options(), false, Target::Rotation(40.0, -90.0), 7);
        let mut view = (0.0, 10.0);
        let degrees = mouse_scale(0.5) * 0.15;
        for _ in 0..40 {
            let f = frame(view);
            assert!(c.tick(&mut view, &f));
        }
        assert!(c.complete());
        assert!((view.0 - 40.0).abs() <= degrees);
        assert_eq!(view.1, -90.0);
        let counts = view.0 / degrees;
        assert!((counts - counts.round()).abs() < 1e-2);
    }

    #[test]
    fn a_screen_stops_a_visible_controller_and_freecam_stops_the_turn() {
        let mut c = Controller::new(catch_options(), false, Target::Rotation(40.0, 0.0), 7);
        let mut view = (0.0, 0.0);
        let mut f = frame(view);
        f.screen = true;
        c.tick(&mut view, &f);
        assert_eq!(view, (0.0, 0.0));
        let mut f = frame(view);
        f.freecam = true;
        c.tick(&mut view, &f);
        assert_eq!(view, (0.0, 0.0));
    }

    #[test]
    fn a_non_retained_visible_controller_is_dropped_when_complete() {
        let o = Options {
            retain: false,
            ..catch_options()
        };
        let mut c = Controller::new(o, false, Target::Rotation(5.0, 0.0), 3);
        let mut view = (0.0, 0.0);
        let mut alive = true;
        for _ in 0..20 {
            let f = frame(view);
            alive = c.tick(&mut view, &f);
        }
        assert!(!alive && c.complete());
    }

    #[test]
    fn point_target_follows_the_yaw_and_pins_the_pitch() {
        let o = Options {
            speed: 1.0,
            tolerance: 0.5,
            clamp: true,
            proportional: true,
            linear: true,
            cubic: true,
            randomize: true,
            ..Options::default()
        };
        let mut c = Controller::new(o, false, Target::Point([-10.0, 0.0, 0.0], -20.0), 9);
        let mut view = (0.0, 0.0);
        for _ in 0..60 {
            let f = frame(view);
            c.tick(&mut view, &f);
        }
        assert!(c.complete());
        assert!((view.0 - 90.0).abs() < 1.5, "{view:?}");
        assert!((view.1 + 20.0).abs() < 1.5, "{view:?}");
    }

    #[test]
    fn silent_controller_returns_to_the_view_after_release() {
        let mut c = Controller::new(catch_options(), true, Target::Rotation(60.0, -90.0), 5);
        let mut managed = (0.0, 0.0);
        let view = (0.0, 0.0);
        for _ in 0..30 {
            assert!(c.tick(&mut managed, &frame(view)));
        }
        assert!(c.complete() && (managed.0 - 60.0).abs() < 1.0);
        c.release();
        let mut alive = true;
        for _ in 0..40 {
            alive = c.tick(&mut managed, &frame(view));
            if !alive {
                break;
            }
        }
        assert!(!alive);
        assert!(
            managed.0.abs() < 1.0 && managed.1.abs() < 1.0,
            "{managed:?}"
        );
    }

    #[test]
    fn silent_aim_holds_until_released() {
        let o = Options {
            retain: false,
            ..catch_options()
        };
        let mut c = Controller::new(o, true, Target::Rotation(20.0, 0.0), 5);
        let mut managed = (0.0, 0.0);
        // An adaptive controller retains itself until it is returning.
        for _ in 0..40 {
            assert!(c.tick(&mut managed, &frame((0.0, 0.0))));
        }
        assert!(c.complete() && !c.relative() && (managed.0 - 20.0).abs() < 1.0);
    }

    #[test]
    fn unset_axes_are_left_alone() {
        let mut c = Controller::new(catch_options(), false, Target::Rotation(UNSET, 30.0), 1);
        let mut view = (12.0, 0.0);
        for _ in 0..20 {
            let f = frame(view);
            c.tick(&mut view, &f);
        }
        assert_eq!(view.0, 12.0);
        assert!((view.1 - 30.0).abs() < 1.0);
    }
}
