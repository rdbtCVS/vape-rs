// NoFall (reference `NoFall`). Normal claims ground in the motion packet once
// the fall passes 2.224 blocks while descending; AntiCheat sends an extra
// on-ground/airborne status pair every further three blocks of falling.
// Skipped while dead, in creative, flying, or with Fly on.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "blatant.no-fall";

pub fn definitions() -> Vec<Definition> {
    vec![Definition::choice(
        &format!("{ID}.mode"),
        "Mode",
        &["Normal", "AntiCheat"],
        "Normal",
    )]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    AntiCheat,
}

impl Mode {
    pub fn read(settings: &Settings) -> Self {
        match settings.get(&format!("{ID}.mode")).and_then(Value::as_str) {
            Some("AntiCheat") => Self::AntiCheat,
            _ => Self::Normal,
        }
    }
    /// `getSimpleSuffix`.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::AntiCheat => "AntiCheat",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Replace the motion packet's on-ground flag with true.
    pub claim_ground: bool,
    /// Send on-ground true, then false, status packets first.
    pub status_pair: bool,
}

#[derive(Debug, Default)]
pub struct NoFall {
    last_fall_distance: f32,
}

impl NoFall {
    pub fn reset(&mut self) {
        self.last_fall_distance = 0.0;
    }
    /// `onMotionUpdate(EventPreMotion)`; `skip` covers the dead, creative,
    /// flying and Fly checks.
    pub fn pre_motion(
        &mut self,
        mode: Mode,
        skip: bool,
        fall_distance: f32,
        motion_y: f64,
    ) -> Effects {
        let mut out = Effects::default();
        if skip {
            return out;
        }
        match mode {
            Mode::Normal => out.claim_ground = f64::from(fall_distance) > 2.224 && motion_y < 0.0,
            Mode::AntiCheat => {
                if self.last_fall_distance > fall_distance {
                    self.last_fall_distance = 0.0;
                }
                if f64::from(fall_distance) > 2.124
                    && motion_y < 0.0
                    && fall_distance >= 3.0
                    && fall_distance - self.last_fall_distance > 3.0
                {
                    self.last_fall_distance = fall_distance;
                    out.status_pair = true;
                }
            }
        }
        out
    }
}
