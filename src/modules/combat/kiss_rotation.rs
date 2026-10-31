// SilentAura's Kiss rotation mode (reference `KissRotationEngine`, KissClient's
// Linear engine): a per-tick rotation step at the Kiss speed, full speed past
// 45 degrees and slowing to `|delta| * 0.5 + 4` inside it, never past the
// target, quantised to whole mouse counts. Two human touches: the speed
// "breathes" (a double cosine whose phase advances 0.45..1.15 rad per tick,
// with white noise) and a low-frequency triangular jitter is added to the
// aim, wider the further the target. The engine keeps no absolute angles;
// the managed rotation applies the step (`applyAiRotationDelta`).
use super::rotation::{degrees_per_count, wrap};

/// The base speed without a Kiss speed override (Kiss `b.java:857`).
pub const BASE_SPEED: f32 = 30.0;
/// The angle past which the step is the full speed.
pub const FAST_ANGLE: f32 = 45.0;
const MIN_STEP: f32 = 4.0;
const STEP_SCALE: f32 = 0.5;
/// `mc45()`'s amplitude, `0.35 * 0.75`.
const SPEED_AMPLITUDE: f32 = 0.35 * 0.75;
/// The white noise mixed into `mc45()`.
const NOISE_MIX: f32 = 0.225;
const TWO_PI: f32 = 6.283_185_5;
/// `updateJitter()`'s base amplitude in degrees.
pub const JITTER_BASE: f32 = 0.15;
/// The range the jitter's distance is normalised by (SilentAura passes 6).
pub const JITTER_RANGE: f64 = 6.0;

#[derive(Clone, Debug)]
pub struct Kiss {
    rng: u64,
    phase: f32,
    jitter_yaw: f32,
    jitter_pitch: f32,
    jitter_scale: f32,
    target_jitter_yaw: f32,
    target_jitter_pitch: f32,
    jitter_ticks: i32,
    owner: Option<i32>,
}

impl Default for Kiss {
    fn default() -> Self {
        Self::new(0x006b_6973_7321)
    }
}

impl Kiss {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: seed | 1,
            phase: 0.0,
            jitter_yaw: 0.0,
            jitter_pitch: 0.0,
            jitter_scale: 1.0,
            target_jitter_yaw: 0.0,
            target_jitter_pitch: 0.0,
            jitter_ticks: 0,
            owner: None,
        }
    }
    /// `Random.nextFloat`: uniform in 0..1.
    fn next_float(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
    /// "Kiss jitter": scales the jitter (0 turns it off).
    pub fn set_jitter_scale(&mut self, scale: f32) {
        self.jitter_scale = scale.max(0.0);
    }
    pub fn reset(&mut self) {
        let rng = self.rng;
        *self = Self {
            rng,
            jitter_scale: self.jitter_scale,
            ..Self::new(rng)
        };
    }
    /// A new target clears the jitter (Kiss `b.java:436-460`).
    pub fn set_target(&mut self, target: Option<i32>) {
        if self.owner != target {
            self.owner = target;
            self.jitter_yaw = 0.0;
            self.jitter_pitch = 0.0;
            self.target_jitter_yaw = 0.0;
            self.target_jitter_pitch = 0.0;
            self.jitter_ticks = 0;
        }
    }
    /// This tick's (yaw, pitch) step toward the absolute aim from the
    /// current rotation, quantised to the sensitivity's mouse counts.
    /// `speed <= 0` falls back to the breathing base speed.
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        &mut self,
        aim: (f32, f32),
        current: (f32, f32),
        speed: f32,
        jitter: bool,
        distance: f64,
        range: f64,
        sensitivity: f64,
    ) -> (f32, f32) {
        let (mut aim_yaw, mut aim_pitch) = aim;
        self.advance_phase();
        if jitter {
            self.advance_jitter(distance, range);
            aim_yaw += self.jitter_yaw;
            aim_pitch = (aim_pitch + self.jitter_pitch).clamp(-90.0, 90.0);
        }
        let delta_yaw = wrap(aim_yaw - current.0);
        let delta_pitch = aim_pitch - current.1;
        // Linear: both axes share the base step.
        let base = if speed > 0.0 {
            speed
        } else {
            BASE_SPEED * self.smooth_multiplier(0.0)
        };
        let step = |delta: f32| {
            if delta.abs() > FAST_ANGLE {
                base
            } else {
                base.min(delta.abs() * STEP_SCALE + MIN_STEP)
            }
        };
        let mut yaw = delta_yaw.abs().min(step(delta_yaw)).copysign(delta_yaw);
        let mut pitch = delta_pitch
            .abs()
            .min(step(delta_pitch))
            .copysign(delta_pitch);
        let gcd = degrees_per_count(sensitivity);
        if gcd > 0.0 {
            yaw = quantise(yaw, gcd);
            pitch = quantise(pitch, gcd);
        }
        (yaw, pitch)
    }
    /// `mx337()`: the breathing phase moves 0.45..1.15 rad (mod 2 pi).
    fn advance_phase(&mut self) {
        self.phase = (self.phase + 0.45 + self.next_float() * 0.7) % TWO_PI;
    }
    /// `mc45(offset)`: `1 + 0.2625 * (2v - 1)` with
    /// `v = 0.5 + 0.5 * (0.62 cos x + 0.38 cos(2.3x + 0.71))` and 22.5% noise.
    fn smooth_multiplier(&mut self, offset: f32) -> f32 {
        let x = self.phase + offset;
        let wave = 0.5 + 0.5 * (0.62 * x.cos() + 0.38 * (2.3 * x + 0.71).cos());
        let wave = wave * (1.0 - NOISE_MIX) + NOISE_MIX * self.next_float();
        1.0 + SPEED_AMPLITUDE * (2.0 * wave - 1.0)
    }
    /// `updateJitter`: a triangular sample every 3..7 ticks, low-passed at
    /// 0.25, `0.15 * (1 + 0.75 t^2)` wide (t the distance over the range),
    /// the pitch at 0.65 of the yaw.
    fn advance_jitter(&mut self, distance: f64, range: f64) {
        let t = ((distance / range.max(0.001)) as f32).clamp(0.0, 1.0);
        let amplitude = JITTER_BASE * self.jitter_scale * (1.0 + 0.75 * t * t);
        self.jitter_ticks -= 1;
        if self.jitter_ticks <= 0 {
            self.target_jitter_yaw = self.triangle(amplitude);
            self.target_jitter_pitch = self.triangle(amplitude * 0.65);
            self.jitter_ticks = 3 + (self.next_float() * 5.0) as i32;
        }
        self.target_jitter_yaw = self.target_jitter_yaw.clamp(-amplitude, amplitude);
        let pitch_amplitude = amplitude * 0.65;
        self.target_jitter_pitch = self
            .target_jitter_pitch
            .clamp(-pitch_amplitude, pitch_amplitude);
        self.jitter_yaw += (self.target_jitter_yaw - self.jitter_yaw) * 0.25;
        self.jitter_pitch += (self.target_jitter_pitch - self.jitter_pitch) * 0.25;
    }
    /// `mc763`: two uniforms subtracted, in `[-amplitude, amplitude]`.
    fn triangle(&mut self, amplitude: f32) -> f32 {
        (self.next_float() + self.next_float() - 1.0) * amplitude
    }
}

fn quantise(delta: f32, gcd: f32) -> f32 {
    if delta == 0.0 {
        0.0
    } else {
        (delta / gcd).round() * gcd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_at_full_speed_far_away_and_slow_down_near_the_target() {
        let mut kiss = Kiss::new(3);
        kiss.set_jitter_scale(0.0);
        let gcd = degrees_per_count(0.5);
        // 90 degrees off: the full Kiss speed (30), quantised.
        let (yaw, pitch) = kiss.compute((90.0, 0.0), (0.0, 0.0), 30.0, true, 3.0, 6.0, 0.5);
        assert!((yaw - 30.0).abs() <= gcd / 2.0 + 1e-4, "{yaw}");
        assert_eq!(pitch, 0.0);
        // 20 degrees off: 20 * 0.5 + 4 = 14.
        let (yaw, _) = kiss.compute((-20.0, 0.0), (0.0, 0.0), 30.0, true, 3.0, 6.0, 0.5);
        assert!((yaw + 14.0).abs() <= gcd / 2.0 + 1e-4, "{yaw}");
        // 2 degrees off: never past the target.
        let (yaw, _) = kiss.compute((2.0, 0.0), (0.0, 0.0), 30.0, true, 3.0, 6.0, 0.5);
        assert!((yaw - 2.0).abs() <= gcd / 2.0 + 1e-4, "{yaw}");
        // Every step is whole mouse counts.
        let counts = yaw / gcd;
        assert!((counts - counts.round()).abs() < 1e-3);
        // Yaw wraps the short way.
        let (yaw, _) = kiss.compute((-170.0, 0.0), (170.0, 0.0), 30.0, true, 3.0, 6.0, 0.5);
        assert!(yaw > 0.0 && (yaw - 14.0).abs() <= gcd / 2.0 + 1e-4, "{yaw}");
    }

    #[test]
    fn the_breathing_speed_and_the_jitter_stay_in_their_bands() {
        let mut kiss = Kiss::new(9);
        kiss.set_target(Some(4));
        let mut seen = (f32::MAX, f32::MIN);
        for _ in 0..400 {
            let m = kiss.smooth_multiplier(0.0);
            seen = (seen.0.min(m), seen.1.max(m));
            assert!((1.0 - SPEED_AMPLITUDE - 1e-5..=1.0 + SPEED_AMPLITUDE + 1e-5).contains(&m));
            kiss.advance_phase();
            assert!((0.0..TWO_PI).contains(&kiss.phase));
            kiss.advance_jitter(6.0, 6.0);
            // At the full range the jitter is 0.15 * 1.75 wide.
            assert!(kiss.jitter_yaw.abs() <= JITTER_BASE * 1.75 + 1e-6);
            assert!(kiss.jitter_pitch.abs() <= JITTER_BASE * 1.75 * 0.65 + 1e-6);
        }
        assert!(seen.1 - seen.0 > 0.2, "the speed breathes: {seen:?}");
        // A new target clears the jitter; no override uses the base speed.
        kiss.set_target(Some(5));
        assert_eq!(
            (kiss.jitter_yaw, kiss.jitter_pitch, kiss.jitter_ticks),
            (0.0, 0.0, 0)
        );
        kiss.set_jitter_scale(0.0);
        let (yaw, _) = kiss.compute((120.0, 0.0), (0.0, 0.0), 0.0, true, 3.0, 6.0, 0.5);
        assert!(
            (yaw - BASE_SPEED).abs() <= BASE_SPEED * SPEED_AMPLITUDE + 0.2,
            "{yaw}"
        );
    }
}
