// AimAssist Adaptive mode (reference `AimAssistTargetingSubModule` and the
// `AimAssistWorkerThread` that drives it).
//
// The reference worker thread sleeps 1 ms between `runTargetLoop` calls and
// measures the time since the previous call, clamped to [1/120 s, 0.12 s]:
// each call integrates at least 1/120 s. Here `AimAssist::frame` runs one
// step per elapsed millisecond on a step clock that advances 1 ms per step,
// so the clamp gives the same 1/120 s per step, and the noise and drift
// phases read that clock. The render-frame application turns the whole mouse
// counts accumulated by the steps into `Entity.turn` deltas, keeping the
// fractions. Yaw and pitch snapping need a speed above 20, which the 1..10
// speed settings never reach; the code is kept as the reference has it.
use super::super::super::core::lifecycle::EntityHandle;
use super::super::super::targeting::entities::EntityView;
use super::super::super::targeting::filter::People;
use super::super::rng::Rng;
use super::aim_assist::{AimAssist, Frame, Options};

const SNAP_ENTER: f32 = 5.0;
const SNAP_EXIT: f32 = 7.0;
/// The step clock's tick: the worker sleeps 1 ms between calls.
const STEP_NS: u64 = 1_000_000;

/// `MathUtil.wrapAngleTo180` on floats.
fn wrap(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `Math.signum`: zero stays zero.
fn signum(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        v
    }
}

fn lerp(factor: f32, start: f32, end: f32) -> f32 {
    start + factor * (end - start)
}

fn smooth_step(edge_start: f32, edge_end: f32, value: f32) -> f32 {
    let n = ((value - edge_start) / (edge_end - edge_start)).clamp(0.0, 1.0);
    n * n * (3.0 - 2.0 * n)
}

/// `Math.round(float)`.
fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// `shouldSnap`: enter within 5 degrees, leave past 7.
fn should_snap(enabled: bool, snapped: bool, angle: f32) -> bool {
    enabled && angle <= if snapped { SNAP_EXIT } else { SNAP_ENTER }
}

/// The yaw and pitch from the eye toward a point, as the submodule computes
/// them (`atan2(dz, dx) - 90`).
fn angles_to(eye: [f64; 3], point: [f64; 3]) -> (f32, f32) {
    let (dx, dy, dz) = (point[0] - eye[0], point[1] - eye[1], point[2] - eye[2]);
    let flat = dx.hypot(dz);
    let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
    let pitch = (-dy.atan2(flat.max(1.0e-4)).to_degrees()) as f32;
    (yaw, pitch)
}

#[derive(Debug)]
pub struct Adaptive {
    target: Option<EntityHandle>,
    target_switch_ticks: i32,
    random: Rng,
    /// The worker's `System.nanoTime()`, advanced 1 ms per step.
    now_ns: u64,
    pitch_velocity: f32,
    yaw_velocity: f32,
    pitch_accel: f32,
    yaw_accel: f32,
    last_yaw_diff: f32,
    last_pitch_diff: f32,
    pending_yaw: f32,
    pending_pitch: f32,
    yaw_bias: f32,
    pitch_bias: f32,
    overshoot: f32,
    aim_strength: f32,
    yaw_flick_ticks: f32,
    pitch_flick_ticks: f32,
    last_yaw_sign: f32,
    last_pitch_sign: f32,
    last_player_yaw: f32,
    last_player_pitch: f32,
    last_target_yaw: f32,
    last_target_pitch: f32,
    yaw_snapped: bool,
    pitch_snapped: bool,
    predicted: [f64; 3],
    aim: [f64; 3],
    lead: [f64; 3],
    aim_point_initialized: bool,
    prediction_initialized: bool,
    air_factor: f32,
    vertical_velocity: f32,
    last_eye_y: f64,
    last_ground_eye_y: f64,
    drift_pos: f32,
    drift_velocity: f32,
    drift_target: f32,
    drift_noise: f32,
    drift_next_ns: u64,
    noise_start_ns: u64,
    last_frame_ns: u64,
    initialized: bool,
}

impl Default for Adaptive {
    fn default() -> Self {
        Self::new(0x2545_f491_4f6c_dd1d)
    }
}

impl Adaptive {
    pub fn new(seed: u64) -> Self {
        Self {
            target: None,
            target_switch_ticks: 0,
            random: Rng::new(seed),
            now_ns: 0,
            pitch_velocity: 0.0,
            yaw_velocity: 0.0,
            pitch_accel: 0.0,
            yaw_accel: 0.0,
            last_yaw_diff: 0.0,
            last_pitch_diff: 0.0,
            pending_yaw: 0.0,
            pending_pitch: 0.0,
            yaw_bias: 0.0,
            pitch_bias: 0.0,
            overshoot: 0.0,
            aim_strength: 0.0,
            yaw_flick_ticks: 0.0,
            pitch_flick_ticks: 0.0,
            last_yaw_sign: 0.0,
            last_pitch_sign: 0.0,
            last_player_yaw: 0.0,
            last_player_pitch: 0.0,
            last_target_yaw: 0.0,
            last_target_pitch: 0.0,
            yaw_snapped: false,
            pitch_snapped: false,
            predicted: [0.0; 3],
            aim: [0.0; 3],
            lead: [0.0; 3],
            aim_point_initialized: false,
            prediction_initialized: false,
            air_factor: 0.0,
            vertical_velocity: 0.0,
            last_eye_y: 0.0,
            last_ground_eye_y: 0.0,
            drift_pos: 0.0,
            drift_velocity: 0.0,
            drift_target: 0.0,
            drift_noise: 0.0,
            drift_next_ns: 0,
            noise_start_ns: 0,
            last_frame_ns: 0,
            initialized: false,
        }
    }

    /// `getTarget`.
    pub fn target(&self) -> Option<EntityHandle> {
        self.target
    }

    /// `onEnable` / `onDisable`: no target and fresh rotation state.
    pub fn clear(&mut self) {
        self.target = None;
        self.reset();
    }

    /// `reset`.
    fn reset(&mut self) {
        self.pending_yaw = 0.0;
        self.pending_pitch = 0.0;
        self.yaw_snapped = false;
        self.pitch_snapped = false;
        self.yaw_bias = 0.0;
        self.pitch_bias = 0.0;
        self.yaw_velocity = 0.0;
        self.pitch_velocity = 0.0;
        self.last_yaw_diff = 0.0;
        self.last_pitch_diff = 0.0;
        self.last_target_yaw = 0.0;
        self.last_target_pitch = 0.0;
        self.yaw_accel = 0.0;
        self.pitch_accel = 0.0;
        self.last_player_yaw = 0.0;
        self.last_player_pitch = 0.0;
        self.initialized = false;
        self.last_frame_ns = 0;
        self.aim_point_initialized = false;
        self.aim = [0.0; 3];
        self.lead = [0.0; 3];
        self.aim_strength = 0.0;
        self.yaw_flick_ticks = 0.0;
        self.pitch_flick_ticks = 0.0;
        self.last_yaw_sign = 0.0;
        self.last_pitch_sign = 0.0;
        self.overshoot = 0.0;
        self.last_eye_y = 0.0;
        self.last_ground_eye_y = 0.0;
        self.vertical_velocity = 0.0;
        self.air_factor = 0.0;
        self.prediction_initialized = false;
        self.predicted = [0.0; 3];
        self.noise_start_ns = self.now_ns;
        self.drift_pos = 0.0;
        self.drift_velocity = 0.0;
        self.drift_target = 0.0;
        self.drift_noise = 0.0;
        self.drift_next_ns = 0;
    }

    fn entity<'a>(&self, frame: &'a Frame) -> Option<&'a EntityView> {
        let handle = self.target?;
        frame.entities.iter().find(|e| e.handle == handle)
    }

    /// One worker step: `runTargetLoop`. `can_aim` is the parent's `canAim`.
    pub fn step(&mut self, options: &Options, frame: &Frame, people: &impl People, can_aim: bool) {
        self.now_ns += STEP_NS;
        if !can_aim {
            self.reset();
            self.target = None;
            return;
        }
        if self.target.is_some() && self.entity(frame).is_none() {
            self.target = None;
        }
        if options.require_mouse && !frame.attack_down {
            self.target = None;
            self.reset();
            return;
        }
        if let Some(e) = self.entity(frame)
            && (!e.alive() || frame.player.distance(e.tick_position) > options.distance)
        {
            self.reset();
            self.target = None;
        }
        if !options.require_mouse || self.target.is_none() {
            let candidate = AimAssist::best(options, frame, people);
            if !options.require_mouse {
                self.target_switch_ticks += 1;
                if self.target_switch_ticks > 700 || self.target.is_none() {
                    if self.target.is_none() || self.target != candidate {
                        self.reset();
                    }
                    self.target = candidate;
                    self.target_switch_ticks = 0;
                }
            } else {
                if self.target.is_none() || self.target != candidate {
                    self.reset();
                }
                self.target = candidate;
            }
        }
        match self.entity(frame).cloned() {
            Some(e) if frame.playing => self.update_aim(options, frame, &e),
            _ => {
                self.target = None;
                self.reset();
            }
        }
    }

    fn eye(frame: &Frame) -> [f64; 3] {
        let p = frame.player.tick_position;
        [p[0], p[1] + frame.eye_height, p[2]]
    }

    /// `resolveSnapPoint` (`prediction` false) and `resolveAimTarget`
    /// (`prediction` true: the Closest point is smoothed between calls).
    fn resolve(
        &mut self,
        options: &Options,
        frame: &Frame,
        target: &EntityView,
        prediction: bool,
    ) -> [f64; 3] {
        let eye = Self::eye(frame);
        let (min_y, max_y) = (target.bounds.min[1], target.bounds.max[1]);
        let center_y = min_y + (max_y - min_y) * 0.65;
        let airborne = 0.85f64.min(f64::from(self.air_factor));
        let eye_offset = 0.1 + airborne;
        let clamp_y = |y: f64| y.min(max_y - 0.01).max(min_y + 0.01);
        let eye_aligned = clamp_y(eye[1] - eye_offset);
        let center_blend = (airborne / 0.55).clamp(0.0, 1.0);
        let target_y = clamp_y(eye_aligned + (center_y - eye_aligned) * center_blend);
        if !options.closest_area {
            let at = target.tick_position;
            return [at[0], target_y, at[2]];
        }
        // RotationUtil.T: the box point closest to the player's x/z and eye.
        let mut closest = [0.0; 3];
        for (i, from) in [
            frame.player.tick_position[0],
            eye[1],
            frame.player.tick_position[2],
        ]
        .into_iter()
        .enumerate()
        {
            closest[i] = from.clamp(target.bounds.min[i], target.bounds.max[i]);
            if i != 1 && closest[i] == from {
                closest[i] = from + 0.01;
            }
        }
        let mut closest_y = clamp_y(closest[1] - eye_offset);
        closest_y += (center_y - closest_y) * center_blend;
        let mut resolved = [closest[0], clamp_y(closest_y), closest[2]];
        if prediction {
            if self.prediction_initialized {
                for (r, p) in resolved.iter_mut().zip(self.predicted) {
                    *r = p + (*r - p) * 0.35;
                }
            }
            self.predicted = resolved;
            self.prediction_initialized = true;
        }
        resolved
    }

    /// `updateAim`.
    fn update_aim(&mut self, options: &Options, frame: &Frame, target: &EntityView) {
        let yaw_snap_enabled = options.horizontal_speed > 20.0;
        let pitch_snap_enabled = options.aim_vertically && options.vertical_speed > 20.0;
        let mut snap_yaw = yaw_snap_enabled && self.yaw_snapped;
        let mut snap_pitch = pitch_snap_enabled && self.pitch_snapped;
        let now = self.now_ns;
        let delta_time: f32 = if !self.initialized || self.last_frame_ns == 0 {
            1.0 / 60.0
        } else {
            ((now - self.last_frame_ns) as f32 / 1.0e9).clamp(1.0 / 120.0, 0.12)
        };
        self.last_frame_ns = now;
        let dt = f64::from(delta_time);

        let resolved = self.resolve(options, frame, target, true);
        let target_motion = target.motion;
        let player_motion = frame.motion;
        let relative: [f64; 3] = std::array::from_fn(|i| target_motion[i] - player_motion[i]);
        let target_lead = [
            target_motion[0] + relative[0] * 0.08,
            target_motion[1] + relative[1] * 0.1,
            target_motion[2] + relative[2] * 0.08,
        ];
        if !self.aim_point_initialized {
            self.aim = resolved;
            self.lead = target_lead;
            self.aim_point_initialized = true;
        }
        let target_hs = target_motion[0].hypot(target_motion[2]);
        let relative_hs = relative[0].hypot(relative[2]);
        let target_distance = frame.player.distance(target.tick_position);
        let base_smoothing = if options.closest_area { 0.3 } else { 0.28 };
        let aim_smoothing = (base_smoothing + (relative_hs * 0.5).min(0.35)).clamp(0.08, 0.75);
        let lead_motion_scale = target_hs + relative_hs * 0.25;
        let lead_smoothing = (0.18 + (lead_motion_scale * 0.72).min(0.42)).clamp(0.12, 0.68);
        let frame_scale = (dt * 60.0).clamp(0.45, 2.4);
        let aim_blend = 1.0 - (1.0 - aim_smoothing).powf(frame_scale);
        let lead_blend = 1.0 - (1.0 - lead_smoothing).powf(frame_scale);
        for i in 0..3 {
            self.aim[i] += (resolved[i] - self.aim[i]) * aim_blend;
            self.lead[i] += (target_lead[i] - self.lead[i]) * lead_blend;
        }
        let lag = (resolved[0] - self.aim[0]).hypot(resolved[2] - self.aim[2]);
        if lag > 0.3 + target_hs * 1.3 + relative_hs * 0.4 {
            self.aim = resolved;
            self.lead = target_lead;
        }

        let mut lead_amount =
            (0.35 + target_distance * 0.045 + target_hs * 0.95 + (relative_hs * 0.12).min(0.18))
                .clamp(0.1, 1.05);
        let distance_lead_scale = ((target_distance - 0.8) / 2.5).clamp(0.0, 1.0);
        lead_amount *= 0.3 + 0.7 * distance_lead_scale;
        lead_amount = lead_amount.max(0.05);
        let eye = Self::eye(frame);
        let mut measured = 0.0f32;
        if self.initialized && self.last_eye_y != 0.0 {
            measured = ((eye[1] - self.last_eye_y) / dt) as f32;
        }
        self.vertical_velocity += (measured - self.vertical_velocity) * 0.65;
        self.last_eye_y = eye[1];
        if frame.on_ground {
            self.last_ground_eye_y = eye[1];
            self.air_factor *= 0.35;
        } else {
            if self.last_ground_eye_y == 0.0 {
                self.last_ground_eye_y = eye[1];
            }
            let height = (eye[1] - self.last_ground_eye_y).max(0.0);
            let velocity_air = (f64::from(self.vertical_velocity).max(0.0) * 0.34).min(0.7) as f32;
            let height_air = (height * 0.58).min(0.82) as f32;
            self.air_factor = (self.air_factor * 0.92).max(velocity_air.max(height_air));
        }

        let vertical_delta = self.aim[1] - eye[1];
        let offset_scale = (vertical_delta.abs() / 0.7).clamp(0.0, 1.0);
        let vertical_lead_amount = lead_amount.min(0.7 + target_distance * 0.04);
        let vertical_lead_scale = 0.28 + 0.52 * offset_scale;
        let limit = 0.16 + target_distance * 0.055;
        let vertical_lead =
            (self.lead[1] * vertical_lead_amount * vertical_lead_scale).clamp(-limit, limit);
        let mut adjusted = [self.aim[0], self.aim[1] + vertical_lead, self.aim[2]];
        if snap_yaw {
            adjusted[0] = resolved[0];
            adjusted[2] = resolved[2];
        }
        if snap_pitch {
            adjusted[1] = resolved[1];
        }
        let (mut target_yaw, mut target_pitch) = angles_to(eye, adjusted);
        let player_yaw = frame.player.yaw;
        let player_pitch = frame.player.pitch;
        if self.initialized {
            let yaw_change = wrap(target_yaw - self.last_target_yaw).abs();
            let pitch_change = wrap(target_pitch - self.last_target_pitch).abs();
            let threshold = if options.closest_area { 20.0 } else { 12.0 };
            let abrupt = ((yaw_change.max(pitch_change) - threshold) / 50.0).clamp(0.0, 1.0);
            self.overshoot = self.overshoot.max(abrupt);
            if abrupt > 0.3 {
                self.aim_strength *= 0.3;
                self.aim = resolved;
            }
            let yaw_rate_scale = 1.0 + (yaw_change / 60.0).min(2.0);
            let pitch_rate_scale = 1.0 + (pitch_change / 60.0).min(2.0);
            let mut max_yaw_rate = (120.0 + (relative_hs * 700.0) as f32) * yaw_rate_scale;
            let mut max_pitch_rate =
                (95.0 + (self.lead[1].abs() * 550.0) as f32) * pitch_rate_scale;
            let view_yaw_difference = wrap(target_yaw - player_yaw).abs();
            max_yaw_rate *= 1.0 + smooth_step(10.0, 35.0, view_yaw_difference) * 1.5;
            max_yaw_rate = max_yaw_rate.clamp(90.0, 1080.0);
            max_pitch_rate = max_pitch_rate.clamp(70.0, 500.0);
            let mut yaw_step = wrap(target_yaw - self.last_target_yaw);
            let mut pitch_step = wrap(target_pitch - self.last_target_pitch);
            if !snap_yaw {
                yaw_step = yaw_step.clamp(-max_yaw_rate * delta_time, max_yaw_rate * delta_time);
            }
            if !snap_pitch {
                pitch_step =
                    pitch_step.clamp(-max_pitch_rate * delta_time, max_pitch_rate * delta_time);
            }
            target_yaw = self.last_target_yaw + yaw_step;
            target_pitch = self.last_target_pitch + pitch_step;
        }
        self.overshoot *= 0.02f64.powf(dt) as f32;
        if self.overshoot < 0.01 {
            self.overshoot = 0.0;
        }
        let overshoot_multiplier = 1.0 + 3.0 * self.overshoot;
        let yaw_error = wrap(target_yaw - player_yaw);
        let mut pitch_error = wrap(target_pitch - player_pitch);
        if !options.aim_vertically {
            pitch_error = 0.0;
        }
        let abs_yaw_error = yaw_error.abs();
        let abs_pitch_error = pitch_error.abs();
        let combined = abs_yaw_error.hypot(abs_pitch_error);
        self.yaw_snapped = should_snap(yaw_snap_enabled, self.yaw_snapped, abs_yaw_error);
        self.pitch_snapped = should_snap(pitch_snap_enabled, self.pitch_snapped, abs_pitch_error);
        snap_yaw = yaw_snap_enabled && self.yaw_snapped;
        snap_pitch = pitch_snap_enabled && self.pitch_snapped;
        let mut desired_strength = 1.0 - smooth_step(1.5, 8.0, combined);
        if self.initialized {
            let closing = -(abs_yaw_error - self.last_yaw_diff.abs()) / delta_time;
            desired_strength += (closing / 20.0).clamp(-0.3, 0.3);
            desired_strength = desired_strength.clamp(0.0, 1.0);
        }
        let strength_blend = if desired_strength > self.aim_strength {
            (delta_time * 3.0).clamp(0.01, 0.25)
        } else {
            (delta_time * 20.0).clamp(0.05, 0.8)
        };
        self.aim_strength += (desired_strength - self.aim_strength) * strength_blend;
        let strength = self.aim_strength;

        let mut target_yaw_rate = 0.0;
        let mut target_pitch_rate = 0.0;
        let mut player_yaw_rate = 0.0;
        let mut player_pitch_rate = 0.0;
        if self.initialized {
            let yaw_rate = wrap(target_yaw - self.last_target_yaw) / delta_time;
            let pitch_rate = wrap(target_pitch - self.last_target_pitch) / delta_time;
            player_yaw_rate = wrap(player_yaw - self.last_player_yaw) / delta_time;
            player_pitch_rate = wrap(player_pitch - self.last_player_pitch) / delta_time;
            let blend = (delta_time * 12.0).clamp(0.05, 0.45);
            self.yaw_accel += (yaw_rate - self.yaw_accel) * blend;
            self.pitch_accel += (pitch_rate - self.pitch_accel) * blend;
            target_yaw_rate = self.yaw_accel;
            target_pitch_rate = self.pitch_accel;
        }
        let horizontal_speed = options.horizontal_speed * 0.75;
        let vertical_speed = options.vertical_speed * 0.75;
        let horizontal_factor = ((horizontal_speed - 10.0) / 90.0).clamp(0.0, 1.0);
        let vertical_factor = ((vertical_speed - 10.0) / 90.0).clamp(0.0, 1.0);
        if signum(yaw_error) != signum(self.last_yaw_diff)
            && yaw_error.abs() > 0.1
            && self.last_yaw_diff.abs() > 0.1
        {
            self.yaw_bias *= 0.3;
        }
        if signum(pitch_error) != signum(self.last_pitch_diff)
            && pitch_error.abs() > 0.1
            && self.last_pitch_diff.abs() > 0.1
        {
            self.pitch_bias *= 0.3;
        }
        let squared = strength * strength;
        self.yaw_bias += yaw_error * delta_time * squared * (1.0 - horizontal_factor);
        self.pitch_bias += pitch_error * delta_time * squared * (1.0 - vertical_factor);
        let retention = 1.0 - (1.0 - strength) * (delta_time * 5.0).clamp(0.0, 0.5);
        self.yaw_bias *= retention;
        self.pitch_bias *= retention;
        let yaw_bias_limit = 15.0 * (1.0 - horizontal_factor * 0.9);
        let pitch_bias_limit = 10.0 * (1.0 - vertical_factor * 0.9);
        self.yaw_bias = self.yaw_bias.clamp(-yaw_bias_limit, yaw_bias_limit);
        self.pitch_bias = self.pitch_bias.clamp(-pitch_bias_limit, pitch_bias_limit);

        let (yaw_error_velocity, pitch_error_velocity) = if self.initialized {
            (
                (yaw_error - self.last_yaw_diff) / delta_time,
                (pitch_error - self.last_pitch_diff) / delta_time,
            )
        } else {
            (0.0, 0.0)
        };
        let velocity_blend = 0.15;
        self.yaw_velocity =
            self.yaw_velocity * (1.0 - velocity_blend) + yaw_error_velocity * velocity_blend;
        self.pitch_velocity =
            self.pitch_velocity * (1.0 - velocity_blend) + pitch_error_velocity * velocity_blend;

        let flick_threshold = if options.closest_area { 1.5 } else { 0.5 };
        let yaw_sign = signum(yaw_error);
        self.yaw_flick_ticks =
            if yaw_sign != self.last_yaw_sign && yaw_error.abs() > flick_threshold {
                (self.yaw_flick_ticks + 1.0).min(8.0)
            } else {
                (self.yaw_flick_ticks - delta_time * 3.0).max(0.0)
            };
        self.last_yaw_sign = yaw_sign;
        let pitch_sign = signum(pitch_error);
        self.pitch_flick_ticks =
            if pitch_sign != self.last_pitch_sign && pitch_error.abs() > flick_threshold {
                (self.pitch_flick_ticks + 1.0).min(8.0)
            } else {
                (self.pitch_flick_ticks - delta_time * 3.0).max(0.0)
            };
        self.last_pitch_sign = pitch_sign;
        let yaw_flick = (self.yaw_flick_ticks / 5.0).clamp(0.0, 1.0);
        let pitch_flick = (self.pitch_flick_ticks / 5.0).clamp(0.0, 1.0);

        let limited_h = horizontal_speed.min(10.0);
        let limited_v = vertical_speed.min(10.0);
        let mut h_gain_input = (limited_h - 1.0) / 9.0;
        let mut v_gain_input = (limited_v - 1.0) / 9.0;
        h_gain_input *= h_gain_input;
        v_gain_input *= v_gain_input;
        let h_gain = 0.15 + 0.85 * h_gain_input.clamp(0.0, 1.0);
        let v_gain = 0.15 + 0.85 * v_gain_input.clamp(0.0, 1.0);
        let mut yaw_error_gain = lerp(strength, 8.0, 2.5 + limited_h * 0.15) * h_gain;
        let mut pitch_error_gain = lerp(strength, 7.0, 2.2 + limited_v * 0.13) * v_gain;
        let yaw_bias_gain = lerp(strength, 0.15, 0.8 + limited_h * 0.04) * h_gain;
        let pitch_bias_gain = lerp(strength, 0.12, 0.65 + limited_v * 0.035) * v_gain;
        let mut yaw_velocity_gain = lerp(strength, 0.08, 0.25) * h_gain;
        let mut pitch_velocity_gain = lerp(strength, 0.06, 0.2) * v_gain;
        let target_yaw_rate_gain = (0.85 + limited_h * 0.015) * h_gain;
        let target_pitch_rate_gain = (0.82 + limited_v * 0.013) * v_gain;
        let player_yaw_compensation = lerp(strength, 0.1, 0.3);
        let player_pitch_compensation = lerp(strength, 0.08, 0.25);
        yaw_error_gain *= 1.0 - 0.6 * yaw_flick;
        yaw_velocity_gain *= 1.0 + 2.0 * yaw_flick;
        pitch_error_gain *= 1.0 - 0.6 * pitch_flick;
        pitch_velocity_gain *= 1.0 + 2.0 * pitch_flick;

        let mut yaw_rate = yaw_error_gain * yaw_error
            + yaw_bias_gain * self.yaw_bias
            + yaw_velocity_gain * self.yaw_velocity
            + target_yaw_rate_gain * target_yaw_rate
            - player_yaw_compensation * player_yaw_rate;
        let mut pitch_rate = 0.0;
        if options.aim_vertically {
            pitch_rate = pitch_error_gain * pitch_error
                + pitch_bias_gain * self.pitch_bias
                + pitch_velocity_gain * self.pitch_velocity
                + target_pitch_rate_gain * target_pitch_rate
                - player_pitch_compensation * player_pitch_rate;
        } else {
            self.pitch_bias = 0.0;
            self.pitch_velocity = 0.0;
        }
        if !snap_pitch {
            pitch_rate += self.pitch_drift(player_pitch, delta_time, now);
        }
        let strafe = frame.player.strafe;
        if options.strafe_increase && strafe.abs() > 0.01 {
            let right = yaw_error > 0.0;
            if (right && strafe < 0.0) || (!right && strafe > 0.0) {
                yaw_rate *= 1.15;
            }
        }

        let wide_yaw = smooth_step(8.0, 40.0, abs_yaw_error);
        let wide_turn = 1.0 + wide_yaw * 2.5;
        let yaw_base_limit = (22.0 + limited_h * 15.0) * h_gain * overshoot_multiplier * wide_turn;
        let pitch_base_limit = (18.0 + limited_v * 13.0) * v_gain * overshoot_multiplier;
        let yaw_motion_limit =
            (target_yaw_rate.abs() * 0.4 + 18.0) * (0.35 + h_gain * 0.65) * wide_turn;
        let pitch_motion_limit = (target_pitch_rate.abs() * 0.38 + 14.0) * (0.35 + v_gain * 0.65);
        let max_yaw =
            (400.0 * overshoot_multiplier * wide_turn).min(yaw_base_limit.max(yaw_motion_limit));
        let max_pitch =
            (300.0 * overshoot_multiplier).min(pitch_base_limit.max(pitch_motion_limit));
        yaw_rate = yaw_rate.clamp(-max_yaw, max_yaw);
        pitch_rate = pitch_rate.clamp(-max_pitch, max_pitch);

        let distance_scale = smooth_step(0.5, 3.0, target_distance as f32);
        let wide_base = lerp(wide_yaw, 0.15, 0.65);
        yaw_rate *= lerp(
            strength,
            wide_base + (1.0 - wide_base) * distance_scale,
            0.4 + 0.6 * distance_scale,
        );
        pitch_rate *= lerp(
            strength,
            0.2 + 0.8 * distance_scale,
            0.45 + 0.55 * distance_scale,
        );
        let t = f64::from(now.saturating_sub(self.noise_start_ns) as f32 / 1.0e9);
        let high_yaw =
            ((t * 62.83).sin() * 0.4 + (t * 47.12).sin() * 0.25 + (t * 78.54).sin() * 0.15) as f32;
        let high_pitch = ((t * 56.55 + 1.3).sin() * 0.35
            + (t * 43.98 + 0.7).sin() * 0.2
            + (t * 72.26 + 2.1).sin() * 0.12) as f32;
        let medium_yaw = ((t * 12.57).sin() * 0.8 + (t * 7.85).sin() * 0.5) as f32;
        let medium_pitch = ((t * 10.47 + 0.9).sin() * 0.6 + (t * 5.65 + 1.8).sin() * 0.4) as f32;
        let low_yaw = ((t * 1.26).sin() * 0.3) as f32;
        let low_pitch = ((t * 0.94 + 0.5).sin() * 0.2) as f32;
        let noise_strength = 0.15 + 0.85 * strength;
        let speed_noise = 0.5 + 0.5 * (1.0 - h_gain_input);
        let yaw_noise = (high_yaw + medium_yaw + low_yaw) * noise_strength * speed_noise * 1.5;
        let pitch_noise = (high_pitch + medium_pitch + low_pitch) * noise_strength * speed_noise;
        if !snap_yaw {
            yaw_rate += yaw_noise;
        }
        if options.aim_vertically && !snap_pitch {
            pitch_rate += pitch_noise;
        }

        let yaw_frame = yaw_rate * delta_time;
        let pitch_frame = pitch_rate * delta_time;
        let base = frame.sensitivity * 0.6 + 0.2;
        let unit = base * base * base * 8.0 * 0.15;
        if unit > 1.0e-5 {
            let yaw_units = yaw_frame / unit;
            let pitch_units = pitch_frame / unit;
            let pitch_blend = if options.aim_vertically {
                vertical_factor
            } else {
                0.0
            };
            if snap_yaw {
                self.pending_yaw = 0.0;
            }
            if snap_pitch {
                self.pending_pitch = 0.0;
            }
            if !snap_yaw {
                if horizontal_factor > 0.0 {
                    let limit = horizontal_speed * 2.0;
                    let correction = (yaw_error / unit - self.pending_yaw).clamp(-limit, limit);
                    self.pending_yaw +=
                        yaw_units * (1.0 - horizontal_factor) + correction * horizontal_factor;
                } else {
                    self.pending_yaw += yaw_units;
                }
            }
            if !snap_pitch {
                if pitch_blend > 0.0 {
                    let wanted = if options.aim_vertically {
                        pitch_error / unit
                    } else {
                        0.0
                    };
                    let limit = vertical_speed * 2.0;
                    let correction = (wanted - self.pending_pitch).clamp(-limit, limit);
                    self.pending_pitch +=
                        pitch_units * (1.0 - pitch_blend) + correction * pitch_blend;
                } else {
                    self.pending_pitch += pitch_units;
                }
            }
        }
        self.initialized = true;
        self.last_yaw_diff = yaw_error;
        self.last_pitch_diff = pitch_error;
        self.last_target_yaw = target_yaw;
        self.last_target_pitch = target_pitch;
        self.last_player_yaw = player_yaw;
        self.last_player_pitch = player_pitch;
    }

    /// `computePitchDrift`: a slow random spring with a little noise.
    fn pitch_drift(&mut self, player_pitch: f32, delta_time: f32, now: u64) -> f32 {
        let strength = 0.65 + 0.35 * self.aim_strength;
        if self.drift_next_ns == 0 || now >= self.drift_next_ns {
            let amplitude = lerp(strength, 0.05, 0.15);
            let correction = if player_pitch > 22.0 {
                -0.18
            } else if player_pitch < -22.0 {
                0.18
            } else {
                0.0
            };
            let r = self.random.double() as f32;
            self.drift_target = ((r * 2.0 - 1.0) * amplitude + correction).clamp(-0.5, 0.5);
            let interval_ms = 300 + self.random.int(420);
            self.drift_next_ns = now + interval_ms * 1_000_000;
        }
        let spring = lerp(strength, 2.0, 5.0);
        let damping = 0.04f32.powf(delta_time);
        self.drift_velocity += (self.drift_target - self.drift_pos) * spring * delta_time;
        self.drift_velocity *= damping;
        self.drift_pos += self.drift_velocity * delta_time;
        let position_limit = lerp(strength, 0.45, 0.8);
        if self.drift_pos > position_limit {
            self.drift_pos = position_limit;
            self.drift_velocity = self.drift_velocity.min(0.0);
        } else if self.drift_pos < -position_limit {
            self.drift_pos = -position_limit;
            self.drift_velocity = self.drift_velocity.max(0.0);
        }
        let r = self.random.double() as f32;
        self.drift_noise +=
            (r * 2.0 - 1.0 - self.drift_noise) * (delta_time * 5.0).clamp(0.02, 0.18);
        let t = f64::from(now.saturating_sub(self.noise_start_ns) as f32 / 1.0e9);
        let noise = ((t * 8.7 + 0.4).sin() * f64::from(0.35f32)
            + (t * 13.1 + 2.2).sin() * f64::from(0.22f32)
            + (t * 19.6 + 1.1).sin() * f64::from(0.12f32)
            + f64::from(self.drift_noise * 0.3)) as f32;
        let restoring = -self.drift_pos * lerp(strength, 1.4, 3.0);
        let output = self.drift_velocity * 0.25 + restoring + noise * lerp(strength, 0.35, 0.9);
        let pitch_limit_scale = 1.0 - 0.85 * smooth_step(72.0, 88.0, player_pitch.abs());
        let limit = lerp(strength, 0.25, 0.55) * pitch_limit_scale;
        output.clamp(-limit, limit)
    }

    /// `onPreRenderTick`: the whole mouse counts as an `Entity.turn` delta
    /// (+y looks down); the fractions stay pending.
    pub fn render(&mut self, options: &Options, frame: &Frame) -> (f64, f64) {
        let Some(target) = self.entity(frame).cloned() else {
            return (0.0, 0.0);
        };
        let snap_yaw = options.horizontal_speed > 20.0 && self.yaw_snapped;
        let snap_pitch =
            options.aim_vertically && options.vertical_speed > 20.0 && self.pitch_snapped;
        let yaw_steps = if snap_yaw {
            round(self.pending_yaw)
        } else {
            self.pending_yaw as i32
        };
        let pitch_steps = if snap_pitch {
            round(self.pending_pitch)
        } else {
            self.pending_pitch as i32
        };
        let remaining_yaw = if snap_yaw {
            0.0
        } else {
            self.pending_yaw - yaw_steps as f32
        };
        let remaining_pitch = if snap_pitch {
            0.0
        } else {
            self.pending_pitch - pitch_steps as f32
        };
        let base = frame.sensitivity * 0.6 + 0.2;
        let scale = base * base * base * 8.0;
        let mut x = yaw_steps as f32 * scale;
        let mut y = pitch_steps as f32 * scale;
        if snap_yaw || snap_pitch {
            let point = self.resolve(options, frame, &target, false);
            let (yaw, pitch) = angles_to(Self::eye(frame), point);
            if snap_yaw {
                x = wrap(yaw - frame.player.yaw) / 0.15;
            }
            if snap_pitch {
                y = wrap(pitch - frame.player.pitch) / 0.15;
            }
        }
        self.pending_yaw = remaining_yaw;
        self.pending_pitch = remaining_pitch;
        (f64::from(x), f64::from(y))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::settings::Settings;
    use super::super::super::super::targeting::entities::{EntityClass, PlayerView};
    use super::super::super::super::targeting::filter::{NameLists, tests::entity};
    use super::super::aim_assist::{AimAssist, definitions};
    use super::*;
    use serde_json::json;

    fn options(mode: &str) -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        settings.set("combat.aim-assist.mode", json!(mode)).unwrap();
        Options::read(&settings)
    }

    fn frame(targets: &[(i32, [f64; 3])]) -> Frame {
        Frame {
            player: PlayerView::default(),
            entities: targets
                .iter()
                .map(|(id, at)| {
                    let mut e = entity(EntityClass::Player, "Target", *at);
                    e.handle.id = *id;
                    e
                })
                .collect(),
            attack_down: true,
            playing: true,
            sensitivity: 0.5,
            elapsed_ms: 16,
            eye_height: 1.62,
            on_ground: true,
            ..Frame::default()
        }
    }

    /// Turn the view by each frame's delta, as `Entity.turn` does.
    fn run(aim: &mut AimAssist, o: &Options, f: &mut Frame, frames: usize) -> f32 {
        for _ in 0..frames {
            let (x, y) = aim.frame(o, f, &NameLists::default());
            assert!(x.is_finite() && y.is_finite());
            f.player.yaw += (x * 0.15) as f32;
            f.player.pitch = (f.player.pitch + (y * 0.15) as f32).clamp(-90.0, 90.0);
        }
        f.player.yaw
    }

    #[test]
    fn adaptive_is_offered_and_tracks_toward_the_target() {
        let o = options("Adaptive");
        assert!(o.adaptive);
        let mut aim = AimAssist::new(11);
        // Facing +Z; the target sits 30 degrees to the left (-yaw).
        let at = [
            3.0 * 30f64.to_radians().sin(),
            0.0,
            3.0 * 30f64.to_radians().cos(),
        ];
        let mut f = frame(&[(7, at)]);
        let want = -30.0f32;
        let start = (want - f.player.yaw).abs();
        run(&mut aim, &o, &mut f, 30);
        assert_eq!(aim.target().map(|t| t.id), Some(7));
        let mid = (want - f.player.yaw).abs();
        assert!(mid < start - 5.0, "{start} -> {mid}");
        run(&mut aim, &o, &mut f, 150);
        let end = (want - f.player.yaw).abs();
        assert!(end < 4.0, "settles near the target: {end}");
        // Vertical aim is off by default: only the small pitch drift moves
        // the pitch (the reference adds it either way).
        assert!(f.player.pitch.abs() < 1.0, "{}", f.player.pitch);
    }

    #[test]
    fn adaptive_aims_vertically_toward_the_eye_aligned_point() {
        let mut o = options("Adaptive");
        o.aim_vertically = true;
        let mut aim = AimAssist::new(5);
        // A target two blocks below: the eye-aligned point is its top.
        let mut f = frame(&[(7, [0.0, -2.0, 3.0])]);
        run(&mut aim, &o, &mut f, 200);
        assert!(f.player.pitch > 10.0, "looked down: {}", f.player.pitch);
    }

    #[test]
    fn adaptive_drops_the_target_on_release_screen_and_range() {
        let o = options("Adaptive");
        let mut aim = AimAssist::new(3);
        let mut f = frame(&[(7, [1.0, 0.0, 3.0])]);
        run(&mut aim, &o, &mut f, 2);
        assert!(aim.target().is_some());
        f.attack_down = false;
        assert_eq!(aim.frame(&o, &f, &NameLists::default()), (0.0, 0.0));
        assert!(aim.target().is_none());
        f.attack_down = true;
        f.playing = false;
        assert_eq!(aim.frame(&o, &f, &NameLists::default()), (0.0, 0.0));
        assert!(
            aim.target().is_none(),
            "a screen clears the Adaptive target"
        );
        f.playing = true;
        run(&mut aim, &o, &mut f, 1);
        f.entities[0].tick_position = [1.0, 0.0, 5.5];
        f.entities[0].position = [1.0, 0.0, 5.5];
        run(&mut aim, &o, &mut f, 1);
        assert!(aim.target().is_none(), "beyond Distance");
    }

    #[test]
    fn adaptive_keeps_its_target_without_require_mouse_until_the_switch_window() {
        let mut o = options("Adaptive");
        o.require_mouse = false;
        let mut aim = AimAssist::new(9);
        let mut f = frame(&[(7, [1.5, 0.0, 3.0])]);
        run(&mut aim, &o, &mut f, 1);
        assert_eq!(aim.target().map(|t| t.id), Some(7));
        // A better (smaller yaw) candidate appears: kept until 700 steps pass.
        let mut better = entity(EntityClass::Player, "Better", [0.0, 0.0, 3.0]);
        better.handle.id = 8;
        f.entities.push(better);
        f.player.yaw = 0.0;
        f.elapsed_ms = 100;
        aim.frame(&o, &f, &NameLists::default());
        assert_eq!(aim.target().map(|t| t.id), Some(7));
        for _ in 0..7 {
            f.player.yaw = 0.0;
            aim.frame(&o, &f, &NameLists::default());
        }
        assert_eq!(aim.target().map(|t| t.id), Some(8));
    }

    #[test]
    fn closest_area_aims_at_the_near_edge_with_smoothing() {
        let mut o = options("Adaptive");
        o.closest_area = true;
        let mut a = Adaptive::new(1);
        let f = frame(&[(7, [2.0, 0.0, 3.0])]);
        let e = f.entities[0].clone();
        let first = a.resolve(&o, &f, &e, true);
        // The player's x is left of the box: its min x edge.
        assert!((first[0] - e.bounds.min[0]).abs() < 1e-9, "{first:?}");
        assert!((first[2] - e.bounds.min[2]).abs() < 1e-9, "{first:?}");
        let mut moved = e.clone();
        for v in [&mut moved.bounds.min, &mut moved.bounds.max] {
            v[2] += 1.0;
        }
        let second = a.resolve(&o, &f, &moved, true);
        assert!((second[2] - (first[2] + 0.35)).abs() < 1e-9, "{second:?}");
        // The snap point is the raw closest point.
        let raw = a.resolve(&o, &f, &moved, false);
        assert!((raw[2] - moved.bounds.min[2]).abs() < 1e-9);
    }

    #[test]
    fn helpers_follow_java_semantics() {
        assert_eq!(signum(0.0), 0.0);
        assert_eq!(signum(-2.0), -1.0);
        assert_eq!(round(-0.5), 0);
        assert_eq!(round(2.5), 3);
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
        assert!(should_snap(true, false, 5.0) && !should_snap(true, false, 6.0));
        assert!(should_snap(true, true, 7.0) && !should_snap(false, true, 1.0));
        let (yaw, pitch) = angles_to([0.0; 3], [-1.0, 0.0, 1.0]);
        assert!((yaw - 45.0).abs() < 1e-4 && pitch.abs() < 1e-4, "{yaw}");
    }
}
