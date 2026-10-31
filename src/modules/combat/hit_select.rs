// HitSelect (reference `HitSelect` with its submodules
// `WTapRightClickUseCancelMode` and `WTapSprintResetMode`). Mode "Right-click
// use cancel" drops attack clicks while the crosshair target is still
// invulnerable (and LeftClicker defers its clicks); with the chance failed it
// rests for `COOLDOWN_TICKS`. Mode "Normal" lets a click through only when it
// resets sprint at the right time (the target's hurt time against the measured
// attack-to-damage delay); every other click is replaced by the swing and its
// particles, and use, right click and block hitting are held back until the
// next tick. Its Preference picks knockback reduction (no select after a
// velocity) or critical hits (airborne clicks after a velocity pass).
use super::super::super::core::settings::{Definition, Settings};
use super::super::rng::Rng;
use serde_json::{Value, json};

pub const ID: &str = "combat.hit-select";
pub const RIGHT_CLICK_USE_CANCEL: &str = "Right-click use cancel";
pub const NORMAL: &str = "Normal";
pub const KB_REDUCTION: &str = "KB reduction";
pub const CRITICAL_HITS: &str = "Critical hits";

/// `WTapRightClickUseCancelMode.COOLDOWN_TICKS` (decoded).
const COOLDOWN_TICKS: i32 = 8;
/// `WTapSprintResetMode.VELOCITY_RESET_TICKS` (decoded).
const VELOCITY_RESET_TICKS: i32 = 7;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mut mode = Definition::choice(
        &key("mode"),
        "Mode",
        &[RIGHT_CLICK_USE_CANCEL, NORMAL],
        RIGHT_CLICK_USE_CANCEL,
    );
    mode.description = "Mode".into();
    let mut preference = Definition::choice(
        &key("preference"),
        "Preference",
        &[KB_REDUCTION, CRITICAL_HITS],
        KB_REDUCTION,
    )
    .when(&key("mode"), json!(NORMAL));
    preference.description =
        "KB reduction: Favors knockback reduction\nCritical hits: Favors critical hit frequency"
            .into();
    vec![
        mode,
        Definition::number(&key("chance"), "Chance", 0.0, 90.0, 100.0, 1.0),
        preference,
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// Mode "Normal" (`WTapSprintResetMode`), else "Right-click use cancel".
    pub normal: bool,
    pub chance: f64,
    pub kb_reduction: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let text = |k: &str| {
            settings
                .get(&format!("{ID}.{k}"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        Self {
            normal: text("mode").as_deref() == Some(NORMAL),
            chance: settings
                .get(&format!("{ID}.chance"))
                .and_then(Value::as_f64)
                .unwrap_or(90.0),
            kb_reduction: text("preference").as_deref() != Some(CRITICAL_HITS),
        }
    }
    /// `getDetailedSuffix`: the mode and the chance.
    pub fn suffix(&self) -> String {
        let mode = if self.normal {
            NORMAL
        } else {
            RIGHT_CLICK_USE_CANCEL
        };
        format!("{mode} {}%", self.chance.round() as i64)
    }
}

/// The crosshair target of the extended-reach ray trace (a living entity).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    pub position: [f64; 3],
    pub hurt_time: i32,
}

impl Target {
    /// `hurtResistantTime > 12`. 26.3's damage event sets the invulnerable
    /// time to 20 with the hurt time to 10, so this is `hurtTime > 2`.
    fn invulnerable(&self) -> bool {
        self.hurt_time > 2
    }
}

/// The local player and its target at a click or a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    pub player: bool,
    pub on_ground: bool,
    pub position: [f64; 3],
    pub motion: [f64; 3],
    pub target: Option<Target>,
}

/// `AttackPacketTimingTracker` at a click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub expected_hurt_ticks: i32,
    pub average_hit_delay: u64,
    pub now_ms: u64,
}

/// What a click becomes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Click {
    /// The click is cancelled.
    pub cancel: bool,
    /// `ClientSettings.applyAttackEffects` on the target.
    pub effects: bool,
}

#[derive(Debug)]
pub struct HitSelect {
    cooldown_ticks: i32,
    cancel_active: bool,
    velocity_ticks: i32,
    pending_timestamp_update: bool,
    last_reset_ms: u64,
    cancel_use: bool,
    random: Rng,
}

impl Default for HitSelect {
    fn default() -> Self {
        Self {
            cooldown_ticks: -1,
            cancel_active: false,
            velocity_ticks: 0,
            pending_timestamp_update: false,
            last_reset_ms: 0,
            cancel_use: true,
            random: Rng::new(0x2545_f491_4f6c_dd1d),
        }
    }
}

impl HitSelect {
    /// `shouldTrigger`.
    fn roll(&mut self, o: &Options) -> bool {
        o.chance >= self.random.double() * 100.0
    }
    /// `isRightClickCancelActive` (LeftClicker's `isHitSelectActive`).
    pub fn right_click_cancel_active(&self, o: &Options) -> bool {
        !o.normal && self.cancel_active
    }
    /// Use, right clicks and block hitting are held back this tick.
    pub fn cancel_use(&self, o: &Options) -> bool {
        o.normal && self.cancel_use
    }
    /// `onPacketReceive`: an explosion or a velocity for the local player.
    pub fn velocity(&mut self, o: &Options) {
        if o.normal {
            self.velocity_ticks = VELOCITY_RESET_TICKS;
        }
    }
    /// `onTick(EventPreTick)` of the selected mode.
    pub fn pre_tick(&mut self, o: &Options, view: &View, now_ms: u64) {
        if o.normal {
            if std::mem::take(&mut self.pending_timestamp_update) {
                self.last_reset_ms = now_ms;
            }
            self.cancel_use = false;
            return;
        }
        if !view.player {
            self.cancel_active = false;
            return;
        }
        if self.cooldown_ticks > 0 {
            self.cooldown_ticks -= 1;
            self.cancel_active = false;
            return;
        }
        if let Some(target) = view.target
            && view.on_ground
            && target.invulnerable()
        {
            if !self.roll(o) {
                self.cooldown_ticks = COOLDOWN_TICKS;
            }
            self.cancel_active = true;
            return;
        }
        self.cancel_active = false;
    }
    /// `onClickMouse(EventClickMouse)` of the selected mode.
    pub fn click(&mut self, o: &Options, view: &View, timing: &Timing) -> Click {
        if !o.normal {
            return Click {
                cancel: self.cancel_active,
                effects: false,
            };
        }
        let pass = Click::default();
        if !self.roll(o) {
            return pass;
        }
        let mut airborne_after_velocity = false;
        if self.velocity_ticks > 0 {
            self.velocity_ticks -= 1;
            if view.on_ground {
                self.velocity_ticks = 0;
            }
            if o.kb_reduction {
                return pass;
            }
            if view.motion[1] > 0.0 {
                airborne_after_velocity = true;
            } else if !view.on_ground {
                return pass;
            }
        }
        let Some(target) = view.target else {
            return pass;
        };
        // `isMovingTowardTarget`: the motion points at the target per axis.
        let toward = |axis: usize| {
            (target.position[axis] - view.position[axis] < 0.0) == (view.motion[axis] < 0.0)
        };
        if !(toward(0) && toward(2)) {
            return pass;
        }
        if !airborne_after_velocity {
            let expected = timing.expected_hurt_ticks;
            if target.hurt_time <= expected
                && timing.now_ms.saturating_sub(self.last_reset_ms) >= timing.average_hit_delay * 2
            {
                self.pending_timestamp_update = true;
                return pass;
            }
            if target.hurt_time > expected && target.hurt_time <= expected + 1 {
                return pass;
            }
        }
        self.cancel_use = true;
        Click {
            cancel: true,
            effects: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(normal: bool, chance: f64, kb_reduction: bool) -> Options {
        Options {
            normal,
            chance,
            kb_reduction,
        }
    }
    fn view(on_ground: bool, hurt_time: i32) -> View {
        View {
            player: true,
            on_ground,
            position: [0.0; 3],
            motion: [0.1, 0.0, 0.1],
            target: Some(Target {
                position: [2.0, 0.0, 2.0],
                hurt_time,
            }),
        }
    }

    #[test]
    fn right_click_use_cancel_holds_clicks_while_the_target_is_invulnerable() {
        let o = options(false, 100.0, true);
        let mut h = HitSelect::default();
        h.pre_tick(&o, &view(true, 9), 0);
        assert!(h.right_click_cancel_active(&o));
        assert!(h.click(&o, &view(true, 9), &Timing::default()).cancel);
        // Airborne, vulnerable again or no target: clicks pass.
        for v in [
            view(false, 9),
            view(true, 2),
            View {
                target: None,
                ..view(true, 9)
            },
        ] {
            h.pre_tick(&o, &v, 0);
            assert!(!h.click(&o, &v, &Timing::default()).cancel);
        }
        // The other mode never reports it.
        h.pre_tick(&o, &view(true, 9), 0);
        assert!(!h.right_click_cancel_active(&options(true, 100.0, true)));
    }

    #[test]
    fn a_failed_chance_rests_eight_ticks_after_the_cancelling_tick() {
        let o = options(false, 0.0, true);
        let mut h = HitSelect::default();
        // The failed roll still cancels this tick, then rests.
        h.pre_tick(&o, &view(true, 9), 0);
        assert!(h.right_click_cancel_active(&o));
        for _ in 0..COOLDOWN_TICKS {
            h.pre_tick(&o, &view(true, 9), 0);
            assert!(!h.right_click_cancel_active(&o));
        }
        h.pre_tick(&o, &view(true, 9), 0);
        assert!(h.right_click_cancel_active(&o));
    }

    #[test]
    fn normal_passes_resets_and_cancels_the_rest_with_effects() {
        let o = options(true, 100.0, true);
        let t = |now_ms| Timing {
            expected_hurt_ticks: 3,
            average_hit_delay: 100,
            now_ms,
        };
        let mut h = HitSelect::default();
        h.pre_tick(&o, &view(true, 0), 0);
        assert!(!h.cancel_use(&o));
        // A vulnerable target long after the last reset: the sprint reset hit.
        assert_eq!(h.click(&o, &view(true, 0), &t(1000)), Click::default());
        h.pre_tick(&o, &view(true, 0), 1000);
        // Too soon for another reset: cancelled with the swing effects.
        let c = h.click(&o, &view(true, 0), &t(1100));
        assert_eq!(
            c,
            Click {
                cancel: true,
                effects: true
            }
        );
        assert!(h.cancel_use(&o));
        // The tick just past the expected delay passes.
        assert!(!h.click(&o, &view(true, 4), &t(1100)).cancel);
        h.pre_tick(&o, &view(true, 0), 1150);
        assert!(!h.cancel_use(&o));
        assert!(h.click(&o, &view(true, 7), &t(1150)).cancel);
        // Moving away from the target: no say.
        let mut away = view(true, 7);
        away.motion = [-0.1, 0.0, 0.1];
        assert!(!h.click(&o, &away, &t(1150)).cancel);
    }

    #[test]
    fn velocity_hands_the_click_to_the_preference() {
        let t = Timing {
            expected_hurt_ticks: 3,
            average_hit_delay: 100,
            now_ms: 0,
        };
        // KB reduction: clicks after a velocity pass while it counts down.
        let kb = options(true, 100.0, true);
        let mut h = HitSelect::default();
        h.velocity(&kb);
        assert!(!h.click(&kb, &view(false, 7), &t).cancel);
        // Critical hits: rising clicks skip the timing and are cancelled;
        // falling airborne clicks pass.
        let crit = options(true, 100.0, false);
        let mut h = HitSelect::default();
        h.velocity(&crit);
        let mut rising = view(false, 0);
        rising.motion[1] = 0.3;
        assert!(h.click(&crit, &rising, &t).cancel);
        let mut falling = view(false, 0);
        falling.motion[1] = -0.1;
        assert!(!h.click(&crit, &falling, &t).cancel);
        // The velocity is only counted in mode Normal.
        let mut h = HitSelect::default();
        h.velocity(&options(false, 100.0, true));
        assert_eq!(h.velocity_ticks, 0);
    }

    #[test]
    fn definitions_and_suffix() {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.validate_conditions().unwrap();
        let o = Options::read(&s);
        assert!(!o.normal && o.kb_reduction && o.chance == 90.0);
        assert_eq!(o.suffix(), "Right-click use cancel 90%");
        s.set(&format!("{ID}.mode"), json!(NORMAL)).unwrap();
        s.set(&format!("{ID}.preference"), json!(CRITICAL_HITS))
            .unwrap();
        let o = Options::read(&s);
        assert!(o.normal && !o.kb_reduction);
        assert_eq!(o.suffix(), "Normal 90%");
    }
}
