// Speed (reference `Speed` with its `BhopSpeed` and `MineplexSpeed`
// sub-modules): each pre-move's horizontal motion is replaced along the
// movement direction, and the module jumps itself on the ground. The
// reference registers it as Category.HIDDEN.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const ID: &str = "blatant.speed";
pub const MODES: [&str; 2] = ["AntiCheat B", "Bhop"];

pub fn definitions() -> Vec<Definition> {
    vec![Definition::choice(
        &format!("{ID}.mode"),
        "Mode",
        &MODES,
        "Bhop",
    )]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    AntiCheatB,
    Bhop,
}

impl Mode {
    pub fn read(settings: &Settings) -> Self {
        match settings.get(&format!("{ID}.mode")).and_then(Value::as_str) {
            Some("AntiCheat B") => Self::AntiCheatB,
            _ => Self::Bhop,
        }
    }
}

/// One pre-move of host state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    /// `moveForward` / `moveStrafing` (left positive).
    pub forward: f64,
    pub strafe: f64,
    pub yaw: f32,
    pub on_ground: bool,
    pub collided_vertically: bool,
    /// The player box moved by this tick's vertical motion hits a block.
    pub blocked_vertically: bool,
    pub in_liquid: bool,
    pub sneaking: bool,
    pub food: i32,
    pub speed_amplifier: Option<i32>,
    pub jump_amplifier: Option<i32>,
    /// Horizontal position at this tick's start.
    pub position: [f64; 2],
    /// LongJump's surface factors: in a vine (`isOnLadder` on a vine block)
    /// and on a slippery (0.98, ice-like) block 0.1 below the feet.
    pub on_vine: bool,
    pub on_ice: bool,
}

impl Tick {
    pub fn moving(&self) -> bool {
        self.forward != 0.0 || self.strafe != 0.0
    }
}

/// The pre-move replacement: horizontal motion and, for a jump, the motion
/// and move Y.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Move {
    pub horizontal: Option<[f64; 2]>,
    pub y: Option<f64>,
}

/// `defaultSpeed` (0.15 per Speed level).
pub fn default_speed(amplifier: Option<i32>) -> f64 {
    let base = 0.287_300_006_915_628_96;
    amplifier.map_or(base, |a| base * (1.0 + 0.15 * f64::from(a + 1)))
}

/// The jump motion with Jump Boost.
pub fn jump_motion(amplifier: Option<i32>) -> f64 {
    f64::from(0.42f32) + amplifier.map_or(0.0, |a| f64::from((a + 1) as f32 * 0.1))
}

/// `Speed.strafe`: `speed` along the input direction (zero without input),
/// with the reference's 45-degree diagonal turn.
pub fn strafe(forward: f64, strafe: f64, yaw: f32, speed: f64) -> [f64; 2] {
    let (mut forward, mut strafe, mut yaw) = (forward, strafe, yaw);
    if forward == 0.0 && strafe == 0.0 {
        return [0.0, 0.0];
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
        forward * speed * angle.sin() - strafe * speed * angle.cos(),
    ]
}

#[derive(Clone, Debug, Default)]
pub struct State {
    started: bool,
    move_speed: f64,
    stage: i32,
    last: Option<[f64; 2]>,
    last_distance: f64,
}

impl State {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// `setStage` (Step sets -4 after a step).
    pub fn set_stage(&mut self, stage: i32) {
        self.stage = stage;
    }

    /// `onMotionUpdate` of the previous tick: the distance its move covered.
    fn track(&mut self, tick: &Tick) {
        if !self.started {
            // onEnable
            self.started = true;
            self.move_speed = default_speed(tick.speed_amplifier);
            self.last_distance = 0.0;
            self.stage = 2;
        } else if let Some([x, z]) = self.last {
            self.last_distance = (tick.position[0] - x).hypot(tick.position[1] - z);
        }
        self.last = Some(tick.position);
    }

    pub fn pre_move(&mut self, mode: Mode, tick: &Tick) -> Move {
        self.track(tick);
        if tick.in_liquid {
            return Move::default();
        }
        match mode {
            Mode::AntiCheatB => self.mineplex(tick),
            Mode::Bhop => self.bhop(tick),
        }
    }

    fn mineplex(&mut self, t: &Tick) -> Move {
        let y = (t.moving() && t.collided_vertically).then(|| jump_motion(t.jump_amplifier));
        Move {
            horizontal: Some(strafe(t.forward, t.strafe, t.yaw, f64::from(0.42f32))),
            y,
        }
    }

    fn bhop(&mut self, t: &Tick) -> Move {
        let base = default_speed(t.speed_amplifier);
        let moving = t.moving();
        let mut out = Move::default();
        if !moving {
            self.move_speed = base;
        }
        if self.stage == 1 && t.collided_vertically && moving {
            self.move_speed = 0.25 + base - 0.01;
        }
        if self.stage == 2 && moving && t.on_ground {
            out.y = Some(jump_motion(t.jump_amplifier));
            self.move_speed *= 2.149;
        } else if self.stage == 3 {
            let reduction = 0.66 * (self.last_distance - base);
            self.move_speed = self.last_distance - reduction;
        } else {
            if (t.blocked_vertically || t.collided_vertically) && self.stage > 0 {
                self.stage = if 1.35 * base - 0.01 > self.move_speed {
                    0
                } else {
                    i32::from(moving)
                };
            }
            self.move_speed = self.last_distance - self.last_distance / 159.0;
        }
        if self.stage > 0 {
            self.move_speed = self.move_speed.max(base);
            out.horizontal = Some(strafe(t.forward, t.strafe, t.yaw, self.move_speed));
        }
        if moving {
            self.stage += 1;
        }
        out
    }
}
