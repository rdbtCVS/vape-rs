// KeepSprint (reference `KeepSprint`): after a hit that applied the
// attacker's extra-knockback cut (motion x/z * 0.6 and the sprint stop),
// replace the cut with the configured slowdown and keep sprinting. Buffer
// abuse only lets a run of hits through before it makes one hit take the
// vanilla cut again.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::{Value, json};

pub const ID: &str = "combat.keep-sprint";
/// `Player.attack`'s horizontal cut after extra knockback.
pub const VANILLA_SLOWDOWN: f64 = 0.6;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::number(
            &key("velocity-slowdown"),
            "Hit Slow Down During Velocity",
            0.0,
            0.6,
            1.0,
            0.05,
        ),
        Definition::number(
            &key("normal-slowdown"),
            "Hit Slow Down Normal",
            0.0,
            0.6,
            1.0,
            0.05,
        ),
        Definition::boolean(&key("velocity-sprint"), "Velocity Hit Sprint", true),
        Definition::boolean(&key("normal-sprint"), "Normal Hit Sprint", true),
        Definition::boolean(&key("buffer-abuse"), "Buffer Abuse", false),
        Definition::number(
            &key("buffer-decrease"),
            "Buffer Decrease",
            0.1,
            1.0,
            10.0,
            0.1,
        )
        .when(&key("buffer-abuse"), json!(true)),
        Definition::number(&key("max-buffer"), "Max Buffer", 1.0, 5.0, 10.0, 1.0)
            .when(&key("buffer-abuse"), json!(true)),
        Definition::boolean(&key("only-in-air"), "Only In Air", false),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub velocity_slowdown: f64,
    pub normal_slowdown: f64,
    pub velocity_sprint: bool,
    pub normal_sprint: bool,
    pub buffer_abuse: bool,
    pub buffer_decrease: f64,
    pub max_buffer: f64,
    pub only_in_air: bool,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        let boolean = |k: &str, d: bool| get(k).and_then(Value::as_bool).unwrap_or(d);
        Self {
            velocity_slowdown: number("velocity-slowdown", 0.6),
            normal_slowdown: number("normal-slowdown", 0.6),
            velocity_sprint: boolean("velocity-sprint", true),
            normal_sprint: boolean("normal-sprint", true),
            buffer_abuse: boolean("buffer-abuse", false),
            buffer_decrease: number("buffer-decrease", 1.0),
            max_buffer: number("max-buffer", 5.0),
            only_in_air: boolean("only-in-air", false),
        }
    }
    /// The array-list suffix: the normal slowdown.
    pub fn suffix(&self) -> String {
        format!("{}", self.normal_slowdown)
    }
}

/// The attacker around one hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Horizontal motion before the hit.
    pub before: [f64; 2],
    /// Horizontal motion after the hit.
    pub after: [f64; 2],
    pub on_ground: bool,
    pub health: f32,
    /// The local player's own hurt time (taking velocity).
    pub hurt_time: i32,
}

/// What to apply after the hit, when it applies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keep {
    pub motion: [f64; 2],
    pub sprinting: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeepSprint {
    combo: f64,
    resetting: bool,
}
impl KeepSprint {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    fn buffer(&mut self, o: &Options) -> bool {
        if !o.buffer_abuse {
            self.combo = 0.0;
            return true;
        }
        if self.combo < o.max_buffer && !self.resetting {
            self.combo += 1.0;
            true
        } else if self.combo > 0.0 {
            self.combo = (self.combo - o.buffer_decrease).max(0.0);
            self.resetting = true;
            false
        } else {
            self.resetting = false;
            true
        }
    }
    pub fn post_attack(&mut self, o: &Options, hit: Hit) -> Option<Keep> {
        let cut = (0..2).all(|i| hit.after[i] == hit.before[i] * VANILLA_SLOWDOWN);
        if !cut || (o.only_in_air && hit.on_ground) || hit.health <= 0.0 {
            return None;
        }
        if !self.buffer(o) {
            return None;
        }
        let velocity = hit.hurt_time > 0;
        let factor = if velocity {
            o.velocity_slowdown
        } else {
            o.normal_slowdown
        };
        Some(Keep {
            motion: [hit.before[0] * factor, hit.before[1] * factor],
            sprinting: if velocity {
                o.velocity_sprint
            } else {
                o.normal_sprint
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        Options::read(&settings)
    }

    fn hit(hurt_time: i32) -> Hit {
        Hit {
            before: [0.2, -0.1],
            after: [0.2 * 0.6, -0.1 * 0.6],
            on_ground: true,
            health: 20.0,
            hurt_time,
        }
    }

    #[test]
    fn replaces_the_cut_with_the_configured_slowdown() {
        let mut o = options();
        o.normal_slowdown = 1.0;
        o.velocity_slowdown = 0.8;
        o.velocity_sprint = false;
        let mut k = KeepSprint::default();
        assert_eq!(
            k.post_attack(&o, hit(0)),
            Some(Keep {
                motion: [0.2, -0.1],
                sprinting: true
            })
        );
        let keep = k.post_attack(&o, hit(5)).unwrap();
        assert!((keep.motion[0] - 0.16).abs() < 1e-12 && !keep.sprinting);
        // No cut happened: nothing to restore.
        assert_eq!(
            k.post_attack(
                &o,
                Hit {
                    after: [0.2, -0.1],
                    ..hit(0)
                }
            ),
            None
        );
        o.only_in_air = true;
        assert_eq!(k.post_attack(&o, hit(0)), None);
    }

    #[test]
    fn buffer_abuse_lets_a_run_through_then_resets() {
        let mut o = options();
        o.buffer_abuse = true;
        o.max_buffer = 2.0;
        o.buffer_decrease = 1.0;
        let mut k = KeepSprint::default();
        let run: Vec<bool> = (0..6)
            .map(|_| k.post_attack(&o, hit(0)).is_some())
            .collect();
        assert_eq!(run, vec![true, true, false, false, true, true]);
    }
}
