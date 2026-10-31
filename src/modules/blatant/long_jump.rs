// LongJump (reference `LongJump`): a boosted jump off the ground, then the
// reference's air decay, along the movement direction; Toggle turns the
// module off on landing.
use super::super::super::core::settings::{Definition, Settings};
use super::speed::{Move, Tick};
use serde_json::Value;

pub const ID: &str = "blatant.long-jump";

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::number(&format!("{ID}.boost"), "Boost", 3.0, 4.1, 5.0, 0.1),
        Definition::boolean(&format!("{ID}.toggle"), "Toggle", true),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub boost: f64,
    pub toggle: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        Self {
            boost: settings
                .get(&format!("{ID}.boost"))
                .and_then(Value::as_f64)
                .unwrap_or(4.1),
            toggle: settings
                .get(&format!("{ID}.toggle"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

/// `getBaseMoveSpeed` (0.15 per Speed level).
fn base_speed(amplifier: Option<i32>, base: f64) -> f64 {
    amplifier.map_or(base, |a| base * (1.0 + 0.15 * f64::from(a + 1)))
}

/// `getMovementYaw`, in radians.
fn movement_yaw(t: &Tick) -> f64 {
    let mut yaw = t.yaw;
    if t.forward < 0.0 {
        yaw += 180.0;
    }
    let factor = if t.forward < 0.0 {
        -0.5
    } else if t.forward > 0.0 {
        0.5
    } else {
        1.0
    };
    if t.strafe > 0.0 {
        yaw -= 90.0 * factor;
    }
    if t.strafe < 0.0 {
        yaw += 90.0 * factor;
    }
    f64::from(yaw * std::f32::consts::PI / 180.0)
}

#[derive(Clone, Debug)]
pub struct State {
    jump_started: bool,
    move_speed: f64,
    was_on_ground: bool,
    last: Option<[f64; 2]>,
    last_distance: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            jump_started: false,
            move_speed: 0.28,
            was_on_ground: false,
            last: None,
            last_distance: 0.0,
        }
    }
}

impl State {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `onMotionUpdate` (after the previous move) then `onPreMove`. Returns
    /// the replacement and whether Toggle turns the module off.
    pub fn tick(&mut self, o: &Options, t: &Tick) -> (Move, bool) {
        if let Some([x, z]) = self.last {
            self.last_distance = (t.position[0] - x).hypot(t.position[1] - z);
        }
        self.last = Some(t.position);
        if o.toggle {
            if !self.was_on_ground && t.on_ground && self.jump_started {
                return (Move::default(), true);
            }
            if !self.jump_started && t.moving() {
                self.jump_started = true;
            }
        }
        (self.pre_move(o, t), false)
    }

    fn pre_move(&mut self, o: &Options, t: &Tick) -> Move {
        let walk = base_speed(t.speed_amplifier, 0.287_300_006_915_628_96);
        if !t.moving() || t.in_liquid {
            self.move_speed = walk;
            return Move::default();
        }
        // `getBaseMoveSpeed`'s surface factors, in the reference's order.
        let factor = if t.on_vine {
            0.5
        } else if t.sneaking {
            0.8
        } else if t.on_ice {
            2.4
        } else if t.food >= 6 {
            1.0
        } else {
            0.765
        };
        let base = walk * factor;
        self.move_speed = base;
        let mut y = None;
        if t.on_ground {
            if self.was_on_ground {
                self.move_speed = base * o.boost;
                y = Some(f64::from(0.42f32));
            }
        } else if self.was_on_ground {
            let air = base_speed(t.speed_amplifier, 0.330_395_007_952_973_3);
            self.move_speed = self.last_distance - 0.666 * (self.last_distance - air);
            self.jump_started = true;
        } else {
            self.move_speed = self.last_distance - self.last_distance / 50.0;
        }
        self.was_on_ground = t.on_ground;
        self.move_speed = self.move_speed.max(base);
        let yaw = movement_yaw(t);
        Move {
            horizontal: Some([-yaw.sin() * self.move_speed, yaw.cos() * self.move_speed]),
            y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_move(t: Tick) -> f64 {
        let o = Options {
            boost: 4.1,
            toggle: false,
        };
        let mut state = State::default();
        let [x, z] = state.tick(&o, &t).0.horizontal.unwrap();
        x.hypot(z)
    }

    #[test]
    fn vine_and_ice_scale_the_base_speed() {
        let t = Tick {
            forward: 1.0,
            food: 20,
            ..Default::default()
        };
        let normal = first_move(t);
        let vine = first_move(Tick {
            on_vine: true,
            sneaking: true,
            ..t
        });
        let ice = first_move(Tick { on_ice: true, ..t });
        let sneak_ice = first_move(Tick {
            on_ice: true,
            sneaking: true,
            ..t
        });
        assert!((vine / normal - 0.5).abs() < 1e-9);
        assert!((ice / normal - 2.4).abs() < 1e-9);
        assert!((sneak_ice / normal - 0.8).abs() < 1e-9);
    }
}
