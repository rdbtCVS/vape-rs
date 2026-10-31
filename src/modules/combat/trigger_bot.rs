// TriggerBot (reference `Triggerbot`): attacks the entity under the crosshair
// when the attack cooldown is ready, through a one-tick synthetic attack
// press like `AttackKeyController`. Timers use the caller's monotonic
// milliseconds; attack readiness uses the host's attack-strength ticker.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::super::targeting::entities::{EntityClass, EntityView};
use super::super::super::targeting::filter::{self, People, TargetFilter};
use serde_json::{Value, json};

pub const ID: &str = "combat.trigger-bot";

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mut out = filter::definitions(ID);
    out.extend([
        Definition::new(
            &key("extra-delay"),
            "Extra delay",
            Kind::Range {
                min: -20.0,
                max: 20.0,
                step: 0.1,
            },
            json!([0.0, 0.0]),
        ),
        Definition::new(
            &key("mouse-over-delay"),
            "Mouse over delay",
            Kind::Range {
                min: 0.0,
                max: 200.0,
                step: 10.0,
            },
            json!([0.0, 0.0]),
        ),
        Definition::boolean(&key("require-mouse"), "Require mouse down", false),
        Definition::boolean(&key("ignore-activation"), "Ignore activation click", false)
            .when(&key("require-mouse"), json!(true)),
        Definition::boolean(&key("air-crits"), "Air crits", false),
        Definition::boolean(&key("shield-check"), "Shield check", false),
        Definition::boolean(&key("select-first-hit"), "Select first hit", false),
        Definition::number(
            &key("miss-chance"),
            "Target miss chance",
            0.0,
            0.0,
            100.0,
            1.0,
        ),
        Definition::number(
            &key("early-chance"),
            "Early hit chance",
            0.0,
            0.0,
            100.0,
            1.0,
        ),
        Definition::boolean(&key("limit-items"), "Limit to items", false),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!([]),
        )
        .when(&key("limit-items"), json!(true)),
    ]);
    out
}

#[derive(Clone, Debug)]
pub struct Options {
    pub filter: TargetFilter,
    pub extra_delay: (f64, f64),
    pub mouse_over_delay: (f64, f64),
    pub require_mouse: bool,
    pub ignore_activation: bool,
    pub air_crits: bool,
    pub shield_check: bool,
    pub select_first_hit: bool,
    pub miss_chance: i32,
    pub early_chance: i32,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let boolean = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        let number = |k: &str| get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let range = |k: &str| {
            let v = get(k).and_then(Value::as_array);
            let at = |i: usize| {
                v.and_then(|v| v.get(i))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
            };
            (at(0), at(1))
        };
        Self {
            filter: TargetFilter::read(settings, ID),
            extra_delay: range("extra-delay"),
            mouse_over_delay: range("mouse-over-delay"),
            require_mouse: boolean("require-mouse"),
            ignore_activation: boolean("ignore-activation"),
            air_crits: boolean("air-crits"),
            shield_check: boolean("shield-check"),
            select_first_hit: boolean("select-first-hit"),
            miss_chance: number("miss-chance") as i32,
            early_chance: number("early-chance") as i32,
            limit_items: boolean("limit-items"),
            allowed_items: get("allowed-items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        }
    }
}

/// The host's attack-strength ticker (`getCooledAttackStrength`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Cooldown {
    pub ticks: f32,
    pub delay: f32,
}
impl Cooldown {
    /// `AttackCooldownUtil.isAttackReady(offset)`: strength exactly 1.
    pub fn ready(self, offset: f32) -> bool {
        self.delay > 0.0 && ((self.ticks + offset) / self.delay).clamp(0.0, 1.0) == 1.0
    }
}

/// One client tick of host state.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub now_ms: u64,
    pub hurt_time: i32,
    /// The attack key mapping is down.
    pub attack_down: bool,
    /// No screen is open and the window has focus.
    pub playing: bool,
    pub held: HeldItem,
    pub on_ground: bool,
    /// `posY - prevPosY` this tick.
    pub vertical_motion: f64,
    /// The entity under the crosshair before any block.
    pub crosshair: Option<EntityView>,
    /// The crosshair hits nothing at all (`RayTraceResult` MISS).
    pub crosshair_miss: bool,
    pub cooldown: Cooldown,
    /// `canBypassCooldown`: smashing with a ready AutoMace mace, HitSwap's
    /// mace swap, or (HitSwap off) a held mace.
    pub cooldown_bypass: bool,
    /// Shield check gives way: ShieldBreaker or HitSwap has an axe, or
    /// AutoMace's Stun slam can take the player.
    pub shield_exempt: bool,
}

/// What the adapter must do this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Release the synthetic attack press made last time.
    pub release: bool,
    /// Press attack once (a `KeyMapping.click`).
    pub attack: bool,
}

#[derive(Debug)]
struct Rng(u64);
impl Rng {
    fn int(&mut self, bound: i32) -> i32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound.max(1) as u64) as i32
    }
    fn range(&mut self, (lo, hi): (f64, f64)) -> f64 {
        if hi <= lo {
            return lo;
        }
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        lo + (self.0 >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
}

#[derive(Debug)]
pub struct TriggerBot {
    attack_timer: u64,
    release_pending: bool,
    mouse_down: bool,
    miss_pending: bool,
    early_ticks: i32,
    last_hurt_time: i32,
    mouse_over: Option<i32>,
    mouse_over_delay: u64,
    mouse_over_at: u64,
    select_target: Option<i32>,
    waiting_first_hit: bool,
    grace: bool,
    select_at: u64,
    random: Rng,
}

impl Default for TriggerBot {
    fn default() -> Self {
        Self::new(0x2545_f491_4f6c_dd1d)
    }
}

impl TriggerBot {
    pub fn new(seed: u64) -> Self {
        Self {
            attack_timer: 0,
            release_pending: false,
            mouse_down: false,
            miss_pending: false,
            early_ticks: 0,
            last_hurt_time: 0,
            mouse_over: None,
            mouse_over_delay: 0,
            mouse_over_at: 0,
            select_target: None,
            waiting_first_hit: false,
            grace: false,
            select_at: 0,
            random: Rng(seed.max(1)),
        }
    }

    /// A swap module cancelled the synthetic press
    /// (`releasePending = requestSyntheticAttack(this)`).
    pub fn attack_cancelled(&mut self) {
        self.release_pending = false;
    }
    /// `onDisable`: returns whether a held synthetic press must be released.
    pub fn reset(&mut self) -> bool {
        self.mouse_down = false;
        self.miss_pending = false;
        self.early_ticks = 0;
        self.reset_mouse_over(0);
        self.reset_select(0);
        std::mem::take(&mut self.release_pending)
    }

    fn reset_mouse_over(&mut self, now: u64) {
        self.mouse_over = None;
        self.mouse_over_delay = 0;
        self.mouse_over_at = now;
    }
    fn reset_select(&mut self, now: u64) {
        self.select_target = None;
        self.waiting_first_hit = false;
        self.grace = false;
        self.select_at = now;
    }

    fn valid(options: &Options, tick: &Tick, entity: &EntityView, people: &impl People) -> bool {
        if options.shield_check
            && !tick.shield_exempt
            && entity.class == EntityClass::Player
            && entity.blocking
        {
            return false;
        }
        options.filter.valid(entity, people)
    }

    fn ready(options: &Options, tick: &Tick, offset: f32) -> bool {
        let _ = options;
        tick.cooldown_bypass || tick.cooldown.ready(offset)
    }

    fn can_attack(
        options: &Options,
        tick: &Tick,
        require_activation: bool,
        mouse_down: bool,
    ) -> bool {
        if !tick.playing {
            return false;
        }
        if require_activation && options.require_mouse && !mouse_down {
            return false;
        }
        if options.limit_items && !item_limit::matches(&options.allowed_items, &tick.held) {
            return false;
        }
        !(options.air_crits && !tick.on_ground && tick.vertical_motion >= 0.0)
    }

    /// The attack key went down or up (`handleInput`). Returns true when the
    /// activation click must be swallowed ("Ignore activation click").
    pub fn input(
        &mut self,
        options: &Options,
        tick: &Tick,
        pressed: bool,
        people: &impl People,
    ) -> bool {
        if options.require_mouse && pressed && !self.mouse_down {
            self.mouse_down = true;
            if options.ignore_activation && Self::can_attack(options, tick, false, true) {
                let not_ready = !Self::ready(options, tick, -(options.extra_delay.0 as i32) as f32);
                return not_ready || tick.crosshair_miss;
            }
            return false;
        }
        if pressed {
            self.attack_timer = tick.now_ms;
            self.early_ticks =
                if options.early_chance > 0 && self.random.int(100) < options.early_chance {
                    2 + self.random.int(2)
                } else {
                    0
                };
            let hovering = tick
                .crosshair
                .as_ref()
                .is_some_and(|e| Self::valid(options, tick, e, people));
            self.miss_pending =
                hovering && options.miss_chance > 0 && self.random.int(100) < options.miss_chance;
        }
        false
    }

    fn mouse_over_elapsed(&mut self, options: &Options, id: i32, now: u64) -> bool {
        if self.mouse_over != Some(id) {
            self.mouse_over = Some(id);
            self.mouse_over_delay = self.random.range(options.mouse_over_delay).max(0.0) as u64;
            self.mouse_over_at = now;
        }
        now.saturating_sub(self.mouse_over_at) >= self.mouse_over_delay
    }

    fn select_first_hit(&mut self, options: &Options, id: i32, just_hurt: bool, now: u64) -> bool {
        if !options.select_first_hit {
            self.reset_select(now);
            return true;
        }
        if self.select_target != Some(id) {
            self.select_target = Some(id);
            self.waiting_first_hit = true;
        }
        self.grace = false;
        self.select_at = now;
        if self.waiting_first_hit && just_hurt {
            self.waiting_first_hit = false;
        }
        !self.waiting_first_hit
    }

    fn select_satisfied(&self, options: &Options) -> bool {
        !options.select_first_hit || self.select_target.is_none() || !self.waiting_first_hit
    }

    fn select_grace(&mut self, options: &Options, now: u64) {
        if !options.select_first_hit || self.select_target.is_none() {
            return;
        }
        if !self.grace {
            self.grace = true;
            self.select_at = now;
            return;
        }
        if now.saturating_sub(self.select_at) >= 1000 {
            self.reset_select(now);
        }
    }

    /// `onTick` (pre tick).
    pub fn tick(&mut self, options: &Options, tick: &Tick, people: &impl People) -> Effects {
        let mut out = Effects::default();
        let now = tick.now_ms;
        let just_hurt = self.last_hurt_time <= 0 && tick.hurt_time > 0;
        self.last_hurt_time = tick.hurt_time;
        if options.require_mouse && self.mouse_down && !tick.attack_down {
            self.mouse_down = false;
            self.reset_select(now);
        }
        if options.select_first_hit
            && self.waiting_first_hit
            && self.select_target.is_some()
            && just_hurt
        {
            self.waiting_first_hit = false;
        }
        if now.saturating_sub(self.attack_timer) < 50 {
            return out;
        }
        if self.release_pending {
            self.release_pending = false;
            out.release = true;
            return out;
        }
        if !Self::can_attack(options, tick, true, self.mouse_down) {
            return out;
        }
        let offset = -(self.random.range(options.extra_delay) as f32) + self.early_ticks as f32;
        match &tick.crosshair {
            Some(entity) if Self::valid(options, tick, entity, people) => {
                let id = entity.handle.id;
                if self.select_first_hit(options, id, just_hurt, now)
                    && self.mouse_over_elapsed(options, id, now)
                    && Self::ready(options, tick, offset)
                {
                    out.release = true;
                    out.attack = true;
                    self.release_pending = true;
                }
            }
            Some(_) => {
                self.reset_mouse_over(now);
                self.select_grace(options, now);
            }
            None => {
                self.reset_mouse_over(now);
                self.select_grace(options, now);
                if self.miss_pending
                    && self.select_satisfied(options)
                    && Self::ready(options, tick, offset)
                {
                    out.release = true;
                    out.attack = true;
                    self.release_pending = true;
                    self.miss_pending = false;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::settings::Settings;
    use super::super::super::super::targeting::filter::{NameLists, tests::entity};
    use super::*;

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        Options::read(&settings)
    }
    fn tick(now: u64, target: bool, ticks: f32) -> Tick {
        Tick {
            now_ms: now,
            playing: true,
            on_ground: true,
            crosshair: target.then(|| entity(EntityClass::Player, "Target", [0.0, 0.0, 2.0])),
            crosshair_miss: !target,
            cooldown: Cooldown { ticks, delay: 12.5 },
            ..Tick::default()
        }
    }

    #[test]
    fn attacks_only_a_valid_target_with_a_ready_cooldown_then_releases() {
        let o = options();
        let people = NameLists::default();
        let mut t = TriggerBot::new(1);
        assert_eq!(
            t.tick(&o, &tick(100, true, 5.0), &people),
            Effects::default()
        );
        assert_eq!(
            t.tick(&o, &tick(150, false, 13.0), &people),
            Effects::default()
        );
        let hit = t.tick(&o, &tick(200, true, 13.0), &people);
        assert!(hit.attack && hit.release);
        let release = t.tick(&o, &tick(250, true, 0.0), &people);
        assert_eq!(
            release,
            Effects {
                release: true,
                attack: false
            }
        );
        let friends = NameLists {
            friends: vec!["target".into()],
            enemies: vec![],
        };
        assert!(!t.tick(&o, &tick(300, true, 13.0), &friends).attack);
    }

    #[test]
    fn options_gate_attacks() {
        let people = NameLists::default();
        let mut o = options();
        o.require_mouse = true;
        let mut t = TriggerBot::new(2);
        assert!(!t.tick(&o, &tick(100, true, 13.0), &people).attack);
        let mut down = tick(120, true, 13.0);
        down.attack_down = true;
        t.input(&o, &down, true, &people);
        down.now_ms = 200;
        assert!(t.tick(&o, &down, &people).attack);
        let mut air = options();
        air.air_crits = true;
        let mut t = TriggerBot::new(3);
        let mut rising = tick(100, true, 13.0);
        rising.on_ground = false;
        rising.vertical_motion = 0.2;
        assert!(!t.tick(&air, &rising, &people).attack);
        rising.vertical_motion = -0.1;
        assert!(t.tick(&air, &rising, &people).attack);
        let mut shield = options();
        shield.shield_check = true;
        let mut blocking = tick(100, true, 13.0);
        blocking.crosshair.as_mut().unwrap().blocking = true;
        assert!(!TriggerBot::new(4).tick(&shield, &blocking, &people).attack);
        // ShieldBreaker, HitSwap or AutoMace can take the shield.
        blocking.shield_exempt = true;
        assert!(TriggerBot::new(4).tick(&shield, &blocking, &people).attack);
        // The 50 ms gate after a physical press.
        let o = options();
        let mut t = TriggerBot::new(5);
        t.input(&o, &tick(1000, true, 13.0), true, &people);
        assert!(!t.tick(&o, &tick(1020, true, 13.0), &people).attack);
        assert!(t.tick(&o, &tick(1060, true, 13.0), &people).attack);
    }

    #[test]
    fn mouse_over_delay_and_select_first_hit() {
        let people = NameLists::default();
        let mut o = options();
        o.mouse_over_delay = (100.0, 100.0);
        let mut t = TriggerBot::new(6);
        assert!(!t.tick(&o, &tick(100, true, 13.0), &people).attack);
        assert!(!t.tick(&o, &tick(150, true, 13.0), &people).attack);
        assert!(t.tick(&o, &tick(200, true, 13.0), &people).attack);
        let mut o = options();
        o.select_first_hit = true;
        let mut t = TriggerBot::new(7);
        assert!(!t.tick(&o, &tick(100, true, 13.0), &people).attack);
        let mut hurt = tick(150, true, 13.0);
        hurt.hurt_time = 9;
        assert!(t.tick(&o, &hurt, &people).attack);
    }
}
