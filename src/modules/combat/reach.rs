// Reach (reference `Reach`): extends the entity pick range to a random value
// in Range while the chance roll allows it. Advanced mode rolls once per
// traced target with a ten-tick cooldown after a successful roll and drops
// the extension after each attack; Normal mode rolls on every range query.
// Misplace (moving other players' rendered positions toward the view) is not
// ported; Vertical check only applies to it in the reference.
use super::super::super::core::settings::{Definition, Kind, Settings};
use serde_json::{Value, json};

pub const ID: &str = "combat.reach";
/// Vanilla survival reach that `getReachDistance` falls back to.
pub const BASE: f64 = 3.0;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::new(&key("range"), "Range", Kind::Range { min: 3.0, max: 4.0, step: 0.01 }, json!([3.0, 3.1])),
        Definition::number(&key("chance"), "Chance", 0.0, 50.0, 100.0, 1.0),
        Definition::choice(&key("chance-mode"), "Chance mode", &["Advanced", "Normal"], "Advanced"),
        Definition::boolean(&key("vertical-check"), "Vertical check", false),
        Definition::boolean(&key("only-sprinting"), "Only while sprinting", false),
        Definition::boolean(&key("disable-in-water"), "Disable in water", false),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub range: (f64, f64),
    pub chance: f64,
    pub advanced: bool,
    pub only_sprinting: bool,
    pub disable_in_water: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let boolean = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        let range = get("range").and_then(Value::as_array);
        let at = |i: usize, d: f64| range.and_then(|r| r.get(i)).and_then(Value::as_f64).unwrap_or(d);
        Self {
            range: (at(0, 3.0), at(1, 3.1)),
            chance: get("chance").and_then(Value::as_f64).unwrap_or(50.0),
            advanced: get("chance-mode").and_then(Value::as_str) != Some("Normal"),
            only_sprinting: boolean("only-sprinting"),
            disable_in_water: boolean("disable-in-water"),
        }
    }
    /// `getDetailedSuffix`: the range display ("3-3.1").
    pub fn suffix(&self) -> String {
        let f = |v: f64| {
            let s = format!("{v:.2}");
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        };
        if self.range.0 == self.range.1 { f(self.range.0) } else { format!("{}-{}", f(self.range.0), f(self.range.1)) }
    }
}

/// The local player's state `isReachAllowed` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Player {
    pub sprinting: bool,
    /// In water or lava.
    pub in_liquid: bool,
}

#[derive(Debug)]
pub struct Reach {
    hit_cooldown: i32,
    reach_active: bool,
    last_target: Option<i32>,
    rng: u64,
}

impl Default for Reach {
    fn default() -> Self {
        Self { hit_cooldown: 0, reach_active: false, last_target: None, rng: 0x2545_f491_4f6c_dd1d }
    }
}

impl Reach {
    pub fn reset(&mut self) {
        let rng = self.rng;
        *self = Self { rng, ..Self::default() };
    }
    fn next_int(&mut self, bound: u64) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng % bound.max(1)
    }
    fn unit(&mut self) -> f64 {
        self.next_int(1 << 53) as f64 / (1u64 << 53) as f64
    }
    fn allowed(o: &Options, p: Player) -> bool {
        !(o.disable_in_water && p.in_liquid) && (!o.only_sprinting || p.sprinting)
    }
    fn roll(&mut self, o: &Options) -> bool {
        o.chance > self.next_int(100) as f64
    }
    /// `onPreTick` (Advanced): the entity traced within the maximum range.
    pub fn pre_tick(&mut self, o: &Options, p: Player, traced: Option<i32>) {
        if !o.advanced {
            return;
        }
        if self.hit_cooldown > 0 {
            self.hit_cooldown -= 1;
        }
        let Some(target) = traced else {
            self.last_target = None;
            return;
        };
        if self.last_target.is_some_and(|last| last != target) {
            self.last_target = None;
            return;
        }
        if Self::allowed(o, p) && self.hit_cooldown == 0 {
            self.reach_active = self.roll(o);
            if self.reach_active {
                self.hit_cooldown = 10;
            }
        }
        self.last_target = Some(target);
    }
    /// `onPostAttack`.
    pub fn post_attack(&mut self, o: &Options) {
        if o.advanced {
            self.reach_active = false;
        }
    }
    /// `getReachDistance`.
    pub fn distance(&mut self, o: &Options, p: Player) -> f64 {
        let extend = if o.advanced { self.reach_active } else { self.roll(o) };
        if !Self::allowed(o, p) || !extend {
            return BASE;
        }
        let (lo, hi) = (o.range.0.min(o.range.1), o.range.0.max(o.range.1));
        lo + self.unit() * (hi - lo)
    }
}
