// Anti-AFK (reference `AntiAFK`). After a random start delay without the user
// moving, it periodically turns the view a little and walks briefly towards
// a random point within a block (Keep close: back to where it started).
// The user's own movement input pauses it and restarts the delay.
//
// Rotations step towards the target at a random speed per turn (the
// reference's `FixedRotationController`); Silent aim has no silent rotation
// system here yet and turns the view like normal aim.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::rng::Rng;
use serde_json::{Value, json};

pub const ID: &str = "world.anti-afk";

pub fn definitions() -> Vec<Definition> {
    let rotation = |d: Definition| d.when(&format!("{ID}.rotation"), json!(true));
    vec![
        Definition::new(
            &format!("{ID}.start-delay"),
            "Start delay",
            Kind::Range {
                min: 10.0,
                max: 200.0,
                step: 1.0,
            },
            json!([30.0, 40.0]),
        ),
        Definition::number(&format!("{ID}.frequency"), "Frequency", 0.1, 0.2, 20.0, 0.1),
        Definition::boolean(&format!("{ID}.keep-close"), "Keep close", false),
        Definition::boolean(&format!("{ID}.rotation"), "Rotation", true),
        rotation(Definition::boolean(
            &format!("{ID}.silent-aim"),
            "Silent aim",
            false,
        )),
        rotation(Definition::number(
            &format!("{ID}.max-yaw"),
            "Max yaw change",
            1.0,
            10.0,
            180.0,
            1.0,
        )),
        rotation(Definition::number(
            &format!("{ID}.max-pitch"),
            "Max pitch change",
            1.0,
            5.0,
            90.0,
            1.0,
        )),
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub start_delay: (f64, f64),
    pub frequency: f64,
    pub keep_close: bool,
    pub rotation: bool,
    pub max_yaw: f64,
    pub max_pitch: f64,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |key: &str| settings.get(&format!("{ID}.{key}"));
        let number = |key: &str, default: f64| get(key).and_then(Value::as_f64).unwrap_or(default);
        let flag = |key: &str, default: bool| get(key).and_then(Value::as_bool).unwrap_or(default);
        let start_delay = get("start-delay")
            .and_then(Value::as_array)
            .and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)))
            .unwrap_or((30.0, 40.0));
        Self {
            start_delay,
            frequency: number("frequency", 0.2),
            keep_close: flag("keep-close", false),
            rotation: flag("rotation", true),
            max_yaw: number("max-yaw", 10.0),
            max_pitch: number("max-pitch", 5.0),
        }
    }
}

/// The local player as Anti-AFK sees it at the start of a tick.
#[derive(Clone, Copy, Debug)]
pub struct Tick {
    pub now_ms: u64,
    /// The user is holding a movement key in a focused window.
    pub user_moving: bool,
    /// A screen is open.
    pub screen: bool,
    pub position: [f64; 2],
    pub yaw: f32,
    pub pitch: f32,
}

/// What Anti-AFK wants this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Actions {
    /// Movement keys to hold: forward, back, left, right.
    pub keys: [bool; 4],
    /// The view to turn by (yaw, pitch degrees).
    pub turn: (f32, f32),
}

fn wrap(degrees: f32) -> f32 {
    let d = degrees % 360.0;
    if d >= 180.0 {
        d - 360.0
    } else if d < -180.0 {
        d + 360.0
    } else {
        d
    }
}

#[derive(Debug)]
pub struct AntiAfk {
    inactive_since: Option<u64>,
    start_delay: u64,
    next_movement: u64,
    next_rotation: u64,
    movement_at: u64,
    rotation_at: u64,
    reference: Option<([f64; 2], f32, f32)>,
    /// The point being walked to and when the walk started.
    walk: Option<([f64; 2], u64, u64)>,
    /// The view target and the step speed in degrees per tick.
    turn: Option<(f32, f32, f32)>,
    random: Rng,
}

impl Default for AntiAfk {
    fn default() -> Self {
        Self {
            inactive_since: None,
            start_delay: 0,
            next_movement: 0,
            next_rotation: 0,
            movement_at: 0,
            rotation_at: 0,
            reference: None,
            walk: None,
            turn: None,
            random: Rng::new(0x9e37_79b9_7f4a_7c15),
        }
    }
}

impl AntiAfk {
    pub fn reset(&mut self) {
        let random = self.random.clone();
        *self = Self {
            random,
            ..Self::default()
        };
    }
    /// `resetActionState`: restarts the inactivity delay from `now`.
    fn restart(&mut self, options: &Options, now: u64) {
        self.inactive_since = Some(now);
        self.start_delay = (self.random.range(options.start_delay) * 1000.0) as u64;
        self.next_movement = self.start_delay;
        self.next_rotation = self.start_delay;
        self.movement_at = now;
        self.rotation_at = now;
        self.reference = None;
        self.walk = None;
        self.turn = None;
    }
    fn capture(&mut self, tick: &Tick) {
        self.reference
            .get_or_insert((tick.position, tick.yaw, tick.pitch));
    }
    pub fn tick(&mut self, options: &Options, tick: &Tick) -> Actions {
        let now = tick.now_ms;
        let mut out = Actions::default();
        if tick.user_moving || self.inactive_since.is_none() {
            self.restart(options, now);
            return out;
        }
        if now.saturating_sub(self.inactive_since.unwrap_or(now)) < self.start_delay {
            return out;
        }
        let interval = (1000.0 / options.frequency.max(0.1)) as u64;
        if options.rotation
            && !tick.screen
            && now.saturating_sub(self.rotation_at) >= self.next_rotation
        {
            self.capture(tick);
            self.next_rotation = self.random.int(1000 + interval);
            self.choose_rotation(options, tick);
            self.rotation_at = now;
        }
        if let Some((yaw, pitch, speed)) = self.turn {
            let dy = wrap(yaw - tick.yaw);
            let dp = pitch - tick.pitch;
            let step = |d: f32| d.clamp(-speed, speed);
            out.turn = (step(dy), step(dp));
            if dy.abs() <= speed && dp.abs() <= speed {
                self.turn = None;
            }
        }
        match self.walk {
            None => {
                if now.saturating_sub(self.movement_at) >= self.next_movement {
                    self.capture(tick);
                    self.next_movement = self.random.int(1000 + interval);
                    let mut target = [
                        tick.position[0] + self.random.double() * 2.0 - 1.0,
                        tick.position[1] + self.random.double() * 2.0 - 1.0,
                    ];
                    if options.keep_close
                        && let Some((origin, _, _)) = self.reference
                        && (origin[0] - tick.position[0]).hypot(origin[1] - tick.position[1])
                            >= 0.75
                    {
                        target = origin;
                    }
                    let duration = 100 + self.random.int(100);
                    self.walk = Some((target, now, duration));
                    self.movement_at = now;
                }
            }
            Some((_, since, duration)) if now.saturating_sub(since) >= duration => self.walk = None,
            Some(_) => {}
        }
        if let Some((target, _, _)) = self.walk {
            out.keys = keys_towards(tick.position, tick.yaw, target);
        }
        out
    }
    /// `updateRandomRotation`.
    fn choose_rotation(&mut self, options: &Options, tick: &Tick) {
        let mut from_reference = 0.0f32;
        if options.keep_close
            && let Some((_, yaw, pitch)) = self.reference
        {
            from_reference = wrap(tick.yaw - yaw).hypot(wrap(tick.pitch - pitch));
        }
        let limit = options.max_yaw.hypot(options.max_pitch);
        let mut yaw_change = self.random.double() * options.max_yaw;
        let mut pitch_change = self.random.double() * options.max_pitch;
        if self.random.double() > 0.6 {
            yaw_change = -yaw_change;
        }
        if self.random.double() > 0.5 {
            pitch_change = -pitch_change;
        }
        let (yaw, pitch) = match self.reference {
            Some((_, yaw, pitch)) if f64::from(from_reference) >= limit => (yaw, pitch),
            _ => (
                (f64::from(tick.yaw) + yaw_change) as f32 % 360.0,
                ((f64::from(tick.pitch) + pitch_change) as f32 % 90.0).clamp(-90.0, 90.0),
            ),
        };
        let speed = (self.random.double() * 10.0).max(1.0) as f32;
        self.turn = Some((yaw, pitch, speed));
    }
}

/// The movement keys that walk from `position` towards `target` with this
/// view yaw (`TargetPositionMovementTask`).
fn keys_towards(position: [f64; 2], yaw: f32, target: [f64; 2]) -> [bool; 4] {
    let (dx, dz) = (target[0] - position[0], target[1] - position[1]);
    if dx.hypot(dz) < 0.1 {
        return [false; 4];
    }
    let rad = f64::from(yaw).to_radians();
    // Minecraft's forward vector for yaw is (-sin, cos); strafe left is (cos, sin).
    let forward = -rad.sin() * dx + rad.cos() * dz;
    let left = rad.cos() * dx + rad.sin() * dz;
    let threshold = 0.3 * dx.hypot(dz);
    [
        forward > threshold,
        forward < -threshold,
        left > threshold,
        left < -threshold,
    ]
}
