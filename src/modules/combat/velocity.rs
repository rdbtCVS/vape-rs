// Velocity (reference `Velocity`): reduces (or, in Kite mode, increases)
// knockback from the local player's entity-motion packets and explosions.
// With Ticks > 0 the packet is kept and the player's motion is scaled that
// many pre-player ticks later; otherwise the packet's motion is replaced.
use super::super::super::core::settings::{Definition, Settings};
use serde_json::{Value, json};

pub const ID: &str = "combat.velocity";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::number(&key("chance"), "Chance", 0.0, 40.0, 100.0, 1.0),
        Definition::number(&key("horizontal"), "Horizontal", 0.0, 90.0, 100.0, 1.0),
        Definition::number(&key("vertical"), "Vertical", 0.0, 100.0, 100.0, 1.0),
        Definition::number(&key("ticks"), "Ticks", 0.0, 1.0, 10.0, 1.0),
        Definition::boolean(&key("kite"), "Kite mode", false),
        Definition::number(
            &key("kite-horizontal"),
            "Kite horizontal",
            100.0,
            120.0,
            300.0,
            1.0,
        )
        .when(&key("kite"), json!(true)),
        Definition::number(
            &key("kite-vertical"),
            "Kite vertical",
            100.0,
            120.0,
            300.0,
            1.0,
        )
        .when(&key("kite"), json!(true)),
        Definition::boolean(&key("always-kite"), "Always Kite", false)
            .when(&key("kite"), json!(true)),
        Definition::boolean(&key("only-targeting"), "Only when targeting", false),
        Definition::boolean(&key("water-check"), "Water check", false),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub chance: f64,
    pub horizontal: f64,
    pub vertical: f64,
    pub ticks: i32,
    pub kite: bool,
    pub kite_horizontal: f64,
    pub kite_vertical: f64,
    pub always_kite: bool,
    pub only_targeting: bool,
    pub water_check: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let number = |k: &str| get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let boolean = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        Self {
            chance: number("chance"),
            horizontal: number("horizontal"),
            vertical: number("vertical"),
            ticks: number("ticks") as i32,
            kite: boolean("kite"),
            kite_horizontal: number("kite-horizontal"),
            kite_vertical: number("kite-vertical"),
            always_kite: boolean("always-kite"),
            only_targeting: boolean("only-targeting"),
            water_check: boolean("water-check"),
        }
    }
    /// `getDetailedSuffix`: "90h 100v".
    pub fn suffix(&self) -> String {
        format!("{:.0}h {:.0}v", self.horizontal, self.vertical)
    }
}

/// `RotationUtil.H/F` around the local player.
#[derive(Clone, Copy, Debug, Default)]
pub struct Facing {
    /// `H(player)`: another non-friend player within 6 blocks looks at us.
    pub player_facing_target: bool,
    /// `F(player)`: we look at another non-friend player within 6 blocks.
    pub target_facing_player: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decision {
    /// Leave the packet as it is.
    Keep,
    /// Apply this motion instead.
    Replace([f64; 3]),
}

/// Java `Double.toString` decimal places as `scaleMotion` counts them.
fn java_decimals(value: f64) -> usize {
    let a = value.abs();
    if a != 0.0 && !(1.0e-3..1.0e7).contains(&a) {
        let s = format!("{a:e}");
        let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
        let fraction = mantissa.split_once('.').map_or(1, |(_, f)| f.len());
        return fraction + 1 + exponent.len();
    }
    let s = format!("{a}");
    s.split_once('.').map_or(1, |(_, f)| f.len())
}

/// `NumberFormat.truncate`: cut the plain decimal string to `precision`.
fn truncate(value: f64, precision: usize) -> f64 {
    let a = value.abs();
    if a != 0.0 && !(1.0e-3..1.0e7).contains(&a) {
        return value;
    }
    let s = format!("{value:?}");
    let Some(dot) = s.find('.') else {
        return value;
    };
    let end = (dot + precision + 1).min(s.len());
    s[..end].parse().unwrap_or(value)
}

/// `scaleMotion`: scale and truncate to the original's decimal places.
pub fn scale(motion: f64, factor: f64) -> f64 {
    let mut scaled = motion * factor.abs();
    if factor < 0.0 {
        scaled = -scaled;
    }
    truncate(scaled, java_decimals(motion))
}

#[derive(Debug)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[derive(Debug)]
pub struct Velocity {
    pending: Option<[f64; 3]>,
    ticks_remaining: i32,
    random: Rng,
}

impl Default for Velocity {
    fn default() -> Self {
        Self::new(0x51_7cc1_b727_220a)
    }
}

impl Velocity {
    pub fn new(seed: u64) -> Self {
        Self {
            pending: None,
            ticks_remaining: 0,
            random: Rng(seed.max(1)),
        }
    }
    pub fn reset(&mut self) {
        self.pending = None;
        self.ticks_remaining = 0;
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// `computeReducedPercents` with its shared 0..5% jitter.
    fn reduced(&mut self, o: &Options) -> (f64, f64) {
        let jitter = self.random.unit();
        let mut h = o.horizontal;
        let mut v = o.vertical;
        if h > 0.0 {
            h = (h + 5.0 * jitter).min(100.0);
        }
        if v > 0.0 {
            v += 5.0 * jitter;
            if v >= 90.0 {
                v = 100.0;
            }
        }
        (h / 100.0, v / 100.0)
    }

    /// `rollChance`: `randomExclusiveUpper(0, 100) >= 100 - chance`.
    fn roll(&mut self, o: &Options) -> bool {
        (self.random.next() % 100) as f64 >= 100.0 - o.chance
    }

    fn facing(o: &Options, mut facing: Facing) -> Facing {
        if o.kite && o.always_kite {
            facing.target_facing_player = false;
        }
        facing
    }

    /// The local player's entity-motion packet (`applyEntityVelocity`).
    pub fn entity_motion(
        &mut self,
        o: &Options,
        facing: Facing,
        in_water: bool,
        motion: [f64; 3],
    ) -> Decision {
        if o.water_check && in_water {
            return Decision::Keep;
        }
        let f = Self::facing(o, facing);
        if !f.player_facing_target && !f.target_facing_player && !o.kite && o.only_targeting {
            return Decision::Keep;
        }
        if !self.roll(o) {
            return Decision::Keep;
        }
        let [x, y, z] = motion;
        if f.player_facing_target && !f.target_facing_player && o.kite {
            let (h, v) = (o.kite_horizontal / 100.0, o.kite_vertical / 100.0);
            return Decision::Replace([x * h, y * v, z * h]);
        }
        if o.ticks > 0 {
            self.ticks_remaining = o.ticks;
            self.pending = Some(motion);
            return Decision::Keep;
        }
        let (h, v) = self.reduced(o);
        if h == 0.0 && v == 0.0 {
            return Decision::Replace([0.0; 3]);
        }
        Decision::Replace([x * h, y * v, z * h])
    }

    /// An explosion's knockback push (`applyBridgeVelocity`).
    pub fn explosion(
        &mut self,
        o: &Options,
        facing: Facing,
        in_water: bool,
        push: [f64; 3],
    ) -> Decision {
        if o.water_check && in_water {
            return Decision::Keep;
        }
        let f = Self::facing(o, facing);
        if f.player_facing_target && !f.target_facing_player && !o.kite && o.only_targeting {
            return Decision::Keep;
        }
        if !self.roll(o) {
            return Decision::Keep;
        }
        let [x, y, z] = push;
        if f.player_facing_target && !f.target_facing_player && o.kite {
            let (h, v) = (o.kite_horizontal / 100.0, o.kite_vertical / 100.0);
            return Decision::Replace([scale(x, h), scale(y, v), scale(z, h)]);
        }
        if o.ticks > 0 {
            if push.iter().any(|m| m.abs() >= 0.005) {
                self.ticks_remaining = o.ticks;
                self.pending = Some(push);
            }
            return Decision::Keep;
        }
        let (h, v) = self.reduced(o);
        if h == 0.0 && v == 0.0 {
            return Decision::Replace([0.0; 3]);
        }
        Decision::Replace([scale(x, h), scale(y, v), scale(z, h)])
    }

    /// `onTick` (pre player tick): the delayed scaling of the current motion.
    pub fn tick(&mut self, o: &Options, in_water: bool, velocity: [f64; 3]) -> Option<[f64; 3]> {
        if o.water_check && in_water {
            self.reset();
            return None;
        }
        let pending = self.pending?;
        let mut out = None;
        if self.ticks_remaining <= 0 {
            let (h, v) = self.reduced(o);
            let mut next = velocity;
            if pending[1] != 0.0 && velocity[1] > 0.0 {
                next[1] = scale(velocity[1], v);
            }
            next[0] = scale(velocity[0], h);
            next[2] = scale(velocity[2], h);
            self.pending = None;
            out = Some(next);
        }
        self.ticks_remaining -= 1;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::settings::Settings;
    use super::*;

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        Options::read(&settings)
    }

    #[test]
    fn java_truncation() {
        assert_eq!(java_decimals(0.4), 1);
        assert_eq!(java_decimals(0.3612), 4);
        assert_eq!(java_decimals(2.0), 1);
        assert_eq!(java_decimals(5.0e-4), 4);
        assert_eq!(scale(0.4, 0.9), 0.3);
        assert_eq!(scale(-0.3612, 0.5), -0.1806);
        assert_eq!(scale(0.36, -1.0), -0.36);
    }

    #[test]
    fn defaults_and_delayed_scaling() {
        let mut o = options();
        assert_eq!(
            (o.chance, o.horizontal, o.vertical, o.ticks),
            (40.0, 90.0, 100.0, 1)
        );
        assert_eq!(o.suffix(), "90h 100v");
        o.chance = 100.0;
        let mut v = Velocity::new(9);
        let motion = [0.4, 0.36, -0.4];
        assert_eq!(
            v.entity_motion(&o, Facing::default(), false, motion),
            Decision::Keep
        );
        assert!(v.pending());
        // Ticks = 1: the first pre-tick only counts down.
        assert_eq!(v.tick(&o, false, motion), None);
        let scaled = v.tick(&o, false, motion).unwrap();
        // scaleMotion keeps the original's one decimal place: 0.4 * 0.9.. -> 0.3.
        assert_eq!(scaled[0], 0.3, "{scaled:?}");
        assert_eq!(scaled[1], 0.36, "vertical 100% stays");
        assert!(!v.pending());
        assert_eq!(v.tick(&o, false, motion), None);
    }

    #[test]
    fn chance_water_targeting_and_kite() {
        let mut o = options();
        o.chance = 0.0;
        let mut v = Velocity::new(1);
        for _ in 0..50 {
            assert_eq!(
                v.entity_motion(&o, Facing::default(), false, [0.4, 0.4, 0.4]),
                Decision::Keep
            );
        }
        assert!(!v.pending());
        o.chance = 100.0;
        o.water_check = true;
        assert_eq!(
            v.entity_motion(&o, Facing::default(), true, [0.4, 0.4, 0.4]),
            Decision::Keep
        );
        assert!(!v.pending());
        o.water_check = false;
        o.only_targeting = true;
        assert_eq!(
            v.entity_motion(&o, Facing::default(), false, [0.4, 0.4, 0.4]),
            Decision::Keep
        );
        assert!(!v.pending());
        o.ticks = 0;
        o.horizontal = 0.0;
        o.vertical = 0.0;
        let facing = Facing {
            player_facing_target: false,
            target_facing_player: true,
        };
        assert_eq!(
            v.entity_motion(&o, facing, false, [0.4, 0.4, 0.4]),
            Decision::Replace([0.0; 3])
        );
        o.kite = true;
        let kite = Facing {
            player_facing_target: true,
            target_facing_player: false,
        };
        match v.entity_motion(&o, kite, false, [0.5, 0.25, 0.5]) {
            Decision::Replace([x, y, z]) => {
                assert!((x - 0.6).abs() < 1e-9 && (y - 0.3).abs() < 1e-9 && (z - 0.6).abs() < 1e-9)
            }
            other => panic!("{other:?}"),
        }
    }
}
