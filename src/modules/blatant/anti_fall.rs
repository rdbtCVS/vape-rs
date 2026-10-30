// AntiFall (reference `AntiFall`): over the void, past Fall Dist, the player
// is set back up by the distance fallen one pre-motion after the first
// detection; a 250 ms pending window or landing clears it.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "blatant.anti-fall";

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(&format!("{ID}.speed-check"), "Speed Check", false),
        Definition::number(&format!("{ID}.fall-dist"), "Fall Dist", 0.1, 2.0, 5.0, 0.1),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub speed_check: bool,
    pub fall_distance: f64,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        Self {
            speed_check: settings
                .get(&format!("{ID}.speed-check"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            fall_distance: settings
                .get(&format!("{ID}.fall-dist"))
                .and_then(Value::as_f64)
                .unwrap_or(2.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    pub now_ms: u64,
    /// Dead, creative, flying, or Fly (or Speed with Speed Check) enabled.
    pub skip: bool,
    /// Any non-air block in the column below the player.
    pub block_below: bool,
    /// The block directly below the feet is air.
    pub over_air: bool,
    pub on_ground: bool,
    pub fall_distance: f64,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pending: bool,
    since: u64,
}

impl State {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// A server position packet (`S08PacketPlayerPosLook`) clears the lagback.
    pub fn teleported(&mut self, now_ms: u64) {
        self.pending = false;
        self.since = now_ms;
    }

    /// `onMotionUpdate`: the height to set the player back up by, if any.
    pub fn pre_motion(&mut self, o: &Options, t: &Tick) -> Option<f64> {
        if t.skip || (!self.pending && t.block_below) {
            return None;
        }
        if (self.pending && t.now_ms.saturating_sub(self.since) >= 250) || t.on_ground {
            self.pending = false;
            self.since = t.now_ms;
            return None;
        }
        if t.fall_distance >= o.fall_distance && t.over_air {
            if !self.pending {
                self.pending = true;
                self.since = t.now_ms;
            } else {
                return Some(t.fall_distance);
            }
        }
        None
    }
}
