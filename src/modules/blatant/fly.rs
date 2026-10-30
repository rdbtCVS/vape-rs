// Fly (reference `Fly`): each move is replaced by the vertical speed from the
// jump/sneak keys and a horizontal move of max(Speed, base speed) along the
// view, with the reference's 45-degree diagonal turn; motion packets claim
// the player is airborne.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "blatant.fly";

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::number(&format!("{ID}.speed"), "Speed", 0.1, 0.5, 5.0, 0.1),
        Definition::number(
            &format!("{ID}.vertical-speed"),
            "Vertical Speed",
            0.1,
            0.2,
            5.0,
            0.1,
        ),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub speed: f64,
    pub vertical_speed: f64,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str, d: f64| {
            settings
                .get(&format!("{ID}.{k}"))
                .and_then(Value::as_f64)
                .unwrap_or(d)
        };
        Self {
            speed: get("speed", 0.5),
            vertical_speed: get("vertical-speed", 0.2),
        }
    }
}

/// One pre-move of host input.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// `movementInput.moveForward` / `moveStrafe` (left positive).
    pub forward: f64,
    pub strafe: f64,
    pub yaw: f32,
    pub jump: bool,
    pub sneak: bool,
    /// The Speed effect's amplifier, if any.
    pub speed_amplifier: Option<i32>,
}

/// `getBaseMoveSpeed`.
pub fn base_speed(amplifier: Option<i32>) -> f64 {
    let base = 0.2873;
    amplifier.map_or(base, |a| base * (1.0 + 0.2 * f64::from(a + 1)))
}

/// `onPreMove`: the move (and the vertical motion it also sets).
pub fn movement(o: &Options, input: &Input) -> [f64; 3] {
    let y = if input.jump {
        o.vertical_speed
    } else if input.sneak {
        -o.vertical_speed
    } else {
        0.0
    };
    let speed = o.speed.max(base_speed(input.speed_amplifier));
    let (mut forward, mut strafe, mut yaw) = (input.forward, input.strafe, input.yaw);
    if forward == 0.0 && strafe == 0.0 {
        return [0.0, y, 0.0];
    }
    if forward != 0.0 {
        if strafe > 0.0 {
            yaw += if forward > 0.0 { -45.0 } else { 45.0 };
        } else if strafe < 0.0 {
            yaw += if forward > 0.0 { 45.0 } else { -45.0 };
        }
        strafe = 0.0;
        forward = forward.signum();
    }
    let angle = f64::from(yaw + 90.0).to_radians();
    [
        forward * speed * angle.cos() + strafe * speed * angle.sin(),
        y,
        forward * speed * angle.sin() - strafe * speed * angle.cos(),
    ]
}
