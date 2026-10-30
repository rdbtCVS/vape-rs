// JumpReset (`JumpReset`: jump right after our own knockback lands so the
// ground friction takes part of it) and KnockbackDelay (`KnockbackDelay`:
// hold our own velocity packet, and every packet behind it, for a random
// time, so the push arrives late). Both read the local player's
// `SetEntityMotion`; the host keeps KnockbackDelay's packets in order and
// this state only decides.
//
// Differences: JumpReset skips the reference's unidentified `PotionRegistry.Z`
// effect check, and its "landed" check compares the rounded current motion
// with the packet's. KnockbackDelay treats the reference's hit counter as
// consecutive grounded ticks (fewer than 3 picks the air delay) and takes its
// target from nearby players the host found in front of us.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::rng::Rng;
use serde_json::{Value, json};

pub const JUMP_RESET: &str = "combat.jump-reset";
pub const KNOCKBACK_DELAY: &str = "combat.knockback-delay";

pub fn definitions() -> Vec<Definition> {
    let jump = |k: &str| format!("{JUMP_RESET}.{k}");
    let delay = |k: &str| format!("{KNOCKBACK_DELAY}.{k}");
    let range = |max| Kind::Range {
        min: 0.0,
        max,
        step: 1.0,
    };
    vec![
        Definition::number(&jump("chance"), "Chance", 0.0, 40.0, 100.0, 1.0),
        Definition::new(
            &jump("accuracy"),
            "Accuracy",
            range(100.0),
            json!([40.0, 60.0]),
        ),
        Definition::boolean(&jump("only-targeting"), "Only when targeting", false),
        Definition::boolean(&jump("water-check"), "Water check", false),
        Definition::number(&delay("chance"), "Chance", 0.0, 40.0, 100.0, 1.0),
        Definition::new(
            &delay("air-delay"),
            "Air delay",
            range(500.0),
            json!([50.0, 100.0]),
        ),
        Definition::new(
            &delay("ground-delay"),
            "Ground delay",
            range(500.0),
            json!([200.0, 250.0]),
        ),
        Definition::boolean(&delay("water-check"), "Water check", false),
    ]
}

fn number(settings: &Settings, id: &str, default: f64) -> f64 {
    settings.get(id).and_then(Value::as_f64).unwrap_or(default)
}

fn flag(settings: &Settings, id: &str) -> bool {
    settings.get(id).and_then(Value::as_bool).unwrap_or(false)
}

fn range(settings: &Settings, id: &str, default: (f64, f64)) -> (f64, f64) {
    settings
        .get(id)
        .and_then(Value::as_array)
        .and_then(|r| Some((r.first()?.as_f64()?, r.get(1)?.as_f64()?)))
        .unwrap_or(default)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Active {
    pub jump_reset: bool,
    pub knockback_delay: bool,
}

/// The local player when its own velocity packet arrives.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub now_ms: u64,
    /// Touching water (counts only with a module's "Water check").
    pub in_water: bool,
    /// Face to face with an opponent, yaw and pitch (JumpReset's targeting).
    pub facing: bool,
    /// A player is in front of us within 5 blocks (KnockbackDelay).
    pub target: bool,
    /// Packets KnockbackDelay is already holding.
    pub queued: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Hold this packet and everything behind it until the time passes.
    Hold {
        release_at: u64,
    },
}

#[derive(Debug)]
pub struct Knockback {
    random: Rng,
    jumping: bool,
    should_jump: bool,
    /// The packet's motion JumpReset waits to see applied.
    waiting: Option<[f64; 3]>,
    last_hit_ms: Option<u64>,
    grounded_ticks: u32,
}

impl Default for Knockback {
    fn default() -> Self {
        Self::new(0x2f_9be6_5c1d_a403)
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

impl Knockback {
    pub fn new(seed: u64) -> Self {
        Self {
            random: Rng::new(seed),
            jumping: false,
            should_jump: false,
            waiting: None,
            last_hit_ms: None,
            grounded_ticks: 0,
        }
    }

    pub fn reset_jump(&mut self) {
        self.jumping = false;
        self.should_jump = false;
        self.waiting = None;
    }

    pub fn reset_delay(&mut self) {
        self.last_hit_ms = None;
        self.grounded_ticks = 0;
    }

    fn roll(&mut self, chance: f64) -> bool {
        self.random.int(100) as f64 >= 100.0 - chance
    }

    /// The local player's `SetEntityMotion` (`motion` in blocks per tick).
    pub fn own_velocity(
        &mut self,
        settings: &Settings,
        active: Active,
        ctx: &Context,
        motion: [f64; 3],
    ) -> Verdict {
        if active.jump_reset {
            self.observe_jump_reset(settings, ctx, motion);
        } else {
            self.reset_jump();
        }
        if !active.knockback_delay {
            return Verdict::Pass;
        }
        let id = |k: &str| format!("{KNOCKBACK_DELAY}.{k}");
        let recent = self
            .last_hit_ms
            .is_some_and(|at| ctx.now_ms.saturating_sub(at) <= 475);
        if recent || !ctx.target {
            return Verdict::Pass;
        }
        if flag(settings, &id("water-check")) && ctx.in_water {
            return Verdict::Pass;
        }
        let chance = number(settings, &id("chance"), 40.0);
        if !self.roll(chance) {
            return Verdict::Pass;
        }
        self.last_hit_ms = Some(ctx.now_ms);
        if ctx.queued > 0 {
            return Verdict::Pass;
        }
        let delay = if self.grounded_ticks < 3 {
            self.random
                .range(range(settings, &id("air-delay"), (50.0, 100.0)))
        } else {
            self.random
                .range(range(settings, &id("ground-delay"), (200.0, 250.0)))
        } as u64;
        if delay == 0 {
            Verdict::Pass
        } else {
            Verdict::Hold {
                release_at: ctx.now_ms + delay,
            }
        }
    }

    fn observe_jump_reset(&mut self, settings: &Settings, ctx: &Context, motion: [f64; 3]) {
        let id = |k: &str| format!("{JUMP_RESET}.{k}");
        let unsuitable = (motion[0] == 0.0 && motion[2] == 0.0) || motion[1] < 0.0;
        if unsuitable || self.jumping {
            return;
        }
        if flag(settings, &id("water-check")) && ctx.in_water {
            return;
        }
        if flag(settings, &id("only-targeting")) && !ctx.facing {
            return;
        }
        let chance = number(settings, &id("chance"), 40.0);
        if self.roll(chance) {
            self.waiting = Some(motion);
        }
    }

    /// Client tick, before the player's tick: the grounded counter, and
    /// whether the waited-for motion has landed (then a jump is due unless
    /// the jump key is already down).
    pub fn tick(&mut self, on_ground: bool, motion: [f64; 3], jump_down: bool) {
        self.grounded_ticks = if on_ground {
            self.grounded_ticks + 1
        } else {
            0
        };
        if let Some(expected) = self.waiting
            && motion.map(round3) == expected.map(round3)
        {
            self.waiting = None;
            self.should_jump = !jump_down;
        }
    }

    /// Whether JumpReset holds the jump key this tick. A new press passes the
    /// accuracy roll (a miss tries again next tick); the key goes up on the
    /// tick after.
    pub fn jump_key(&mut self, settings: &Settings, again: bool) -> bool {
        if again {
            return self.jumping;
        }
        self.jumping = false;
        if self.should_jump {
            let accuracy = self.random.range(range(
                settings,
                &format!("{JUMP_RESET}.accuracy"),
                (40.0, 60.0),
            ));
            if self.random.int(100) as f64 >= 100.0 - accuracy {
                self.jumping = true;
                self.should_jump = false;
            }
        }
        self.jumping
    }

    pub fn jump_pending(&self) -> bool {
        self.jumping || self.should_jump || self.waiting.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(chance: f64) -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        for m in [JUMP_RESET, KNOCKBACK_DELAY] {
            s.set(&format!("{m}.chance"), json!(chance)).unwrap();
        }
        s
    }

    fn ctx(queued: usize) -> Context {
        Context {
            now_ms: 1000,
            in_water: false,
            facing: true,
            target: true,
            queued,
        }
    }

    #[test]
    fn delay_holds_with_a_target_and_not_twice_within_475ms() {
        let s = settings(100.0);
        let on = Active {
            jump_reset: false,
            knockback_delay: true,
        };
        let mut k = Knockback::default();
        let Verdict::Hold { release_at } = k.own_velocity(&s, on, &ctx(0), [0.3, 0.4, 0.0]) else {
            panic!()
        };
        assert!((1050..=1100).contains(&release_at), "{release_at}");
        let mut later = ctx(0);
        later.now_ms = 1200;
        assert_eq!(
            k.own_velocity(&s, on, &later, [0.3, 0.4, 0.0]),
            Verdict::Pass
        );
        let mut none = ctx(0);
        none.now_ms = 5000;
        none.target = false;
        assert_eq!(
            k.own_velocity(&s, on, &none, [0.3, 0.4, 0.0]),
            Verdict::Pass
        );
    }

    #[test]
    fn ground_delay_after_three_grounded_ticks() {
        let s = settings(100.0);
        let on = Active {
            jump_reset: false,
            knockback_delay: true,
        };
        let mut k = Knockback::default();
        for _ in 0..3 {
            k.tick(true, [0.0; 3], false);
        }
        let Verdict::Hold { release_at } = k.own_velocity(&s, on, &ctx(0), [0.3, 0.4, 0.0]) else {
            panic!()
        };
        assert!((1200..=1250).contains(&release_at), "{release_at}");
    }

    #[test]
    fn jump_waits_for_the_motion_then_presses_once() {
        let mut s = settings(100.0);
        s.set(&format!("{JUMP_RESET}.accuracy"), json!([100.0, 100.0]))
            .unwrap();
        let on = Active {
            jump_reset: true,
            knockback_delay: false,
        };
        let mut k = Knockback::default();
        let m = [0.3, 0.4, 0.1];
        assert_eq!(k.own_velocity(&s, on, &ctx(0), m), Verdict::Pass);
        k.tick(false, [0.0; 3], false);
        assert!(!k.jump_key(&s, false));
        k.tick(false, m, false);
        assert!(k.jump_key(&s, false));
        assert!(k.jump_key(&s, true));
        assert!(!k.jump_key(&s, false));
        assert!(!k.jump_pending());
    }

    #[test]
    fn downward_or_horizontal_zero_velocity_is_ignored() {
        let s = settings(100.0);
        let on = Active {
            jump_reset: true,
            knockback_delay: false,
        };
        let mut k = Knockback::default();
        k.own_velocity(&s, on, &ctx(0), [0.3, -0.1, 0.1]);
        k.own_velocity(&s, on, &ctx(0), [0.0, 0.4, 0.0]);
        assert!(!k.jump_pending());
    }
}
