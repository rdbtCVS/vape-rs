// The reference click engine (`ClickEngine` + `ClickerMod.runClickCycle` +
// `AutoClickerTimingState`), AutoClicker (`LeftClicker`) and RightClicker. The reference
// runs each click cycle on a worker thread that sleeps through the hold and
// release phases; here `Engine::frame` advances the same cycle from the
// caller's monotonic milliseconds and returns the host actions to perform.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::click_timing::{self, Timing};
use serde_json::{Value, json};

pub const LEFT: &str = "combat.auto-clicker";
pub const RIGHT: &str = "combat.right-clicker";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Randomization {
    Normal,
    Extra,
    ExtraPlus,
}

/// `AutoClickerTimingState` ("Extra+"), with the caller's clock.
#[derive(Debug)]
pub struct ExtraPlus {
    current_cps: f64,
    previous_delay: u64,
    salt: u64,
    target_cps: f64,
    initialized: bool,
    target_offset: f64,
    pause_offset: f64,
    repeat_chance: f64,
    random: u64,
    minimum: f64,
    maximum: f64,
    next_drift: u64,
    fatigue: f64,
    clicks_since_variation: i32,
    burst_remaining: i32,
    last_click: u64,
    next_target: u64,
}

fn mix(mut seed: u64) -> u64 {
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    seed ^ (seed >> 31)
}

impl ExtraPlus {
    pub fn new(seed: u64) -> Self {
        let random = mix(seed ^ 0x9e37_79b9_7f4a_7c15);
        Self {
            current_cps: 0.0,
            previous_delay: 0,
            salt: mix(random.wrapping_add(0xd1b5_4a32_d192_ed03)),
            target_cps: 0.0,
            initialized: false,
            target_offset: 0.0,
            pause_offset: 0.0,
            repeat_chance: 0.06,
            random,
            minimum: 0.0,
            maximum: 0.0,
            next_drift: 0,
            fatigue: 0.0,
            clicks_since_variation: 0,
            burst_remaining: 0,
            last_click: 0,
            next_target: 0,
        }
    }
    /// `SplittableRandom.nextLong` (the golden-gamma Weyl sequence).
    fn next(&mut self) -> u64 {
        self.random = self.random.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.random)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn int(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }
    fn normal(&mut self) -> f64 {
        let sum: f64 = (0..6).map(|_| self.unit()).sum();
        (sum - 3.0) * 1.224_744_871_39
    }
    fn duration(&mut self, now: u64, min: u64, max: u64) -> u64 {
        let time = (now ^ self.salt) & 0xffff;
        let range = max - min + 1;
        min + (time + self.int(range)) % range
    }
    fn noise(&mut self) -> f64 {
        let normal = self.normal();
        let roll = self.unit();
        if roll < 0.12 {
            (self.unit() - 0.5) * 3.0
        } else if roll < 0.17 {
            normal * (1.4 + self.unit() * 0.6)
        } else {
            normal
        }
    }
    fn retarget(&mut self, now: u64) {
        self.target_offset = (self.unit() * 2.0 - 1.0) * 0.75;
        self.target_cps = ((self.minimum + self.maximum) * 0.5 + self.target_offset)
            .clamp(self.minimum, self.maximum);
        self.next_target = now + self.duration(now, 30_000, 90_000);
    }
    pub fn configure(&mut self, minimum: u32, maximum: u32) {
        let lo = f64::from(minimum) * 1.5;
        let hi = f64::from(maximum.max(minimum)) * 1.5;
        let lo = lo.max(0.5);
        let hi = hi.max(0.5);
        self.minimum = lo;
        self.maximum = (lo + 0.1).max(hi);
        if !self.initialized {
            self.target_cps = (self.minimum + self.maximum) * 0.5;
            let n = self.normal();
            self.current_cps = (self.target_cps + n * 0.25).clamp(self.minimum, self.maximum);
        }
    }
    pub fn delay(&mut self, now: u64) -> u64 {
        if !self.initialized {
            self.initialized = true;
            self.next_drift = now + self.duration(now, 1200, 3200);
            self.retarget(now);
            self.last_click = now;
        }
        let idle = if self.last_click == 0 {
            0
        } else {
            now.saturating_sub(self.last_click)
        };
        if idle >= 1200 {
            let recovery = idle.min(5000) as f64 / 10.0;
            self.fatigue = (self.fatigue - 0.004 * recovery).max(0.0);
            self.burst_remaining = if self.unit() < 0.7 {
                2 + self.int(4) as i32
            } else {
                0
            };
            self.repeat_chance = 0.03 + self.unit() * 0.06;
            self.pause_offset = self.unit() * 0.04;
        }
        self.clicks_since_variation += 1;
        if self.clicks_since_variation > 80 + self.int(120) as i32 {
            self.repeat_chance = 0.03 + self.unit() * 0.06;
            self.pause_offset = self.unit() * 0.04;
            self.clicks_since_variation = 0;
        }
        if now >= self.next_drift {
            let drift = self.normal();
            self.current_cps += 0.25 * (self.target_cps - self.current_cps) + 0.45 * drift;
            if self.unit() < 0.03 {
                self.current_cps += (self.unit() - 0.5) * 1.2;
            }
            self.current_cps = self.current_cps.clamp(self.minimum, self.maximum);
            self.next_drift = now + self.duration(now, 1200, 3200);
        }
        if now >= self.next_target {
            self.retarget(now);
        }
        let burst = self.burst_remaining > 0;
        let mut cps = self.current_cps;
        if burst {
            cps *= 1.0 + 0.05 * (0.4 + 0.8 * self.unit());
            self.burst_remaining -= 1;
        }
        cps *= 1.0 - self.fatigue.min(0.4);
        cps = cps.clamp(self.minimum, self.maximum);
        let multiplier = (0.24 * self.noise()).exp();
        let micro = (35 + self.int(11) as i64 - 5).max(1) as u64;
        let mut delay = 1000.0 / cps * multiplier + self.int(micro + 1) as f64;
        if burst {
            delay += self.int(15) as f64;
        }
        if self.previous_delay > 0 && self.unit() < self.repeat_chance {
            delay = self.previous_delay as f64 + self.int(7) as f64 - 3.0;
        }
        if self.unit() < 0.07 {
            delay *= 0.7 + self.unit() * 0.2;
        }
        let pause = (0.04 + 0.12 * self.fatigue + self.pause_offset).min(0.18);
        if self.unit() < pause {
            delay += 50.0 + self.int(101) as f64;
        }
        self.fatigue = (self.fatigue + 0.015).min(0.4);
        self.last_click = now;
        self.previous_delay = delay.clamp(1.0, f64::from(i32::MAX)) as u64;
        self.previous_delay
    }
}

pub fn left_definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{LEFT}.{k}");
    let range = |k: &str, label: &str, min: f64, max: f64, step: f64, default: [f64; 2]| {
        Definition::new(
            &key(k),
            label,
            Kind::Range { min, max, step },
            json!(default),
        )
    };
    let list = |k: &str, label: &str, default: Value| {
        Definition::new(
            &key(k),
            label,
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            default,
        )
    };
    vec![
        Definition::boolean(&key("hold"), "Hold to click", true),
        Definition::boolean(&key("trigger"), "Trigger mode", false),
        Definition::boolean(&key("break-blocks"), "Break blocks", false),
        range(
            "break-delay",
            "Break blocks delay",
            0.0,
            2000.0,
            10.0,
            [0.0, 10.0],
        )
        .when(&key("break-blocks"), json!(true)),
        Definition::boolean(&key("break-whitelist"), "Break blocks whitelist", false)
            .when(&key("break-blocks"), json!(true)),
        list("break-items", "Items", json!(["pickaxes", "shovels"]))
            .when(&key("break-blocks"), json!(true))
            .when(&key("break-whitelist"), json!(true)),
        range("cps", "CPS", 1.0, 20.0, 1.0, [6.0, 13.0]),
        Definition::choice(
            &key("randomization"),
            "Randomization",
            &["Normal", "Extra", "Extra+"],
            "Extra",
        ),
        Definition::boolean(&key("jitter"), "Jitter", false),
        Definition::boolean(&key("limit-items"), "Limit items", false),
        list("allowed-items", "Item whitelist", json!(["swords"]))
            .when(&key("limit-items"), json!(true)),
    ]
}

/// RightClicker: CPS, Start Delay, Randomization, Jitter, the item
/// whitelist and Sneak Only. Its "Hold to Click" is never added to the GUI
/// and stays on.
pub fn right_definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{RIGHT}.{k}");
    vec![
        Definition::new(
            &key("cps"),
            "CPS",
            Kind::Range {
                min: 1.0,
                max: 20.0,
                step: 1.0,
            },
            json!([7.0, 13.0]),
        ),
        Definition::number(&key("start-delay"), "Start Delay", 0.0, 0.0, 1000.0, 1.0),
        Definition::choice(
            &key("randomization"),
            "Randomization",
            &["Normal", "Extra", "Extra+"],
            "Extra+",
        ),
        Definition::boolean(&key("jitter"), "Jitter", false),
        Definition::boolean(&key("limit-items"), "Use item whitelist", false),
        Definition::new(
            &key("allowed-items"),
            "Item whitelist",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["blocks"]),
        )
        .when(&key("limit-items"), json!(true)),
        Definition::boolean(&key("sneak-only"), "Sneak Only", false),
    ]
}

#[derive(Clone, Debug)]
pub struct Options {
    pub hold: bool,
    pub trigger: bool,
    pub break_blocks: bool,
    pub break_delay: (f64, f64),
    pub break_whitelist: bool,
    pub break_items: Vec<String>,
    pub cps: (u32, u32),
    pub randomization: Randomization,
    pub jitter: bool,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
    /// Extra activation delay on top of the reference's 50 ms.
    pub start_delay: u64,
    pub sneak_only: bool,
}

impl Options {
    pub fn read(settings: &Settings, module: &str) -> Self {
        let get = |k: &str| settings.get(&format!("{module}.{k}"));
        let boolean = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        let range = |k: &str| {
            let v = get(k).and_then(Value::as_array);
            let at = |i: usize| {
                v.and_then(|v| v.get(i))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
            };
            (at(0), at(1))
        };
        let list = |k: &str| {
            get(k)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        };
        let cps = range("cps");
        let right = module == RIGHT;
        Self {
            hold: right || boolean("hold"),
            trigger: boolean("trigger"),
            break_blocks: boolean("break-blocks"),
            break_delay: range("break-delay"),
            break_whitelist: boolean("break-whitelist"),
            break_items: list("break-items"),
            cps: (cps.0 as u32, cps.1 as u32),
            randomization: match get("randomization").and_then(Value::as_str) {
                Some("Normal") => Randomization::Normal,
                Some("Extra+") => Randomization::ExtraPlus,
                _ => Randomization::Extra,
            },
            jitter: boolean("jitter"),
            limit_items: boolean("limit-items"),
            allowed_items: list("allowed-items"),
            start_delay: get("start-delay")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .max(0.0) as u64,
            sneak_only: boolean("sneak-only"),
        }
    }
    /// `getDetailedSuffix`: "6-13cps".
    pub fn suffix(&self) -> String {
        if self.cps.0 == self.cps.1 {
            format!("{}cps", self.cps.0)
        } else {
            format!("{}-{}cps", self.cps.0, self.cps.1)
        }
    }
}

/// One frame of host state for a left clicker.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub now_ms: u64,
    /// No screen, window focused and input enabled.
    pub playing: bool,
    /// The physical attack button is held (`isKeyBindingDown(key.attack)`).
    pub attack_held: bool,
    /// The attack key mapping as the host currently has it.
    pub attack_mapped: bool,
    pub held: HeldItem,
    /// The crosshair targets an entity / a block.
    pub entity_hit: bool,
    pub block_hit: bool,
    pub sneaking: bool,
    /// `SharedModuleControlClaims.rightClickUse` is claimed (RightClicker's
    /// `shouldBlockClick`); the runtime sets it.
    pub use_claimed: bool,
}

/// What the adapter does this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actions {
    /// One attack click with the attack mapping pressed.
    pub press: bool,
    /// Release the attack mapping.
    pub release: bool,
    /// Mouse units for `Entity.turn` from the jitter.
    pub turn: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    /// Pressed; release at the deadline.
    Hold(u64),
    /// Released; next cycle at the deadline.
    Wait(u64),
}

#[derive(Debug)]
pub struct Engine {
    phase: Phase,
    release_after: u64,
    activation: Option<u64>,
    timing: Timing,
    extra_plus: ExtraPlus,
    rng: u64,
    breaking: bool,
    break_attack_held: bool,
    break_timer: u64,
    break_delay: u64,
    jitter_target: (f64, f64),
    jitter_steps: f64,
    jitter_remaining: f64,
    pending_jitter: (f32, f32),
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(0x5851_f42d_4c95_7f2d)
    }
}

impl Engine {
    pub fn new(seed: u64) -> Self {
        Self {
            phase: Phase::Idle,
            release_after: 0,
            activation: None,
            timing: Timing::new(seed),
            extra_plus: ExtraPlus::new(seed),
            rng: seed.rotate_left(29).max(1),
            breaking: false,
            break_attack_held: false,
            break_timer: 0,
            break_delay: 0,
            jitter_target: (0.0, 0.0),
            jitter_steps: 0.0,
            jitter_remaining: 0.0,
            pending_jitter: (0.0, 0.0),
        }
    }
    fn next(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Java `Random.nextInt()` reduced `% 10`: -9..=9.
    fn signed_ten(&mut self) -> i64 {
        (self.next() as u32 as i32 % 10) as i64
    }

    /// `onDisable`: returns whether the adapter must release the mapping.
    pub fn reset(&mut self) -> bool {
        let release = matches!(self.phase, Phase::Hold(_)) || self.break_attack_held;
        self.phase = Phase::Idle;
        self.activation = None;
        self.breaking = false;
        self.break_attack_held = false;
        self.pending_jitter = (0.0, 0.0);
        self.jitter_remaining = 0.0;
        release
    }

    fn delay(&mut self, o: &Options, now: u64) -> u64 {
        let (lo, hi) = (o.cps.0.max(1), o.cps.1.max(o.cps.0.max(1)));
        match o.randomization {
            Randomization::Normal => self.timing.delay(lo, hi, click_timing::Mode::Normal),
            Randomization::Extra => self.timing.delay(lo, hi, click_timing::Mode::Extra),
            Randomization::ExtraPlus => {
                self.extra_plus.configure(lo, hi);
                self.extra_plus.delay(now)
            }
        }
    }

    fn activation_held(o: &Options, input: &Input) -> bool {
        // Without Hold to click the reference uses window focus alone.
        if o.hold {
            input.attack_held
        } else {
            input.playing
        }
    }

    /// `LeftClicker.shouldAllowClick`: false while mining a block.
    fn allow_click(&mut self, o: &Options, input: &Input) -> bool {
        if !input.attack_held {
            self.break_timer = input.now_ms;
            return true;
        }
        if o.break_blocks && input.now_ms.saturating_sub(self.break_timer) >= self.break_delay {
            if o.break_whitelist && !item_limit::matches(&o.break_items, &input.held) {
                return true;
            }
            if input.block_hit {
                return false;
            }
            self.break_timer = input.now_ms;
            let (lo, hi) = o.break_delay;
            self.break_delay = (lo + self.unit() * (hi - lo).max(0.0)) as u64;
        }
        true
    }

    /// `shouldDeferAttack` on the pinned modern host (attack cooldown aside,
    /// handled by the host itself): Trigger mode only clicks on entities.
    fn defer(o: &Options, input: &Input) -> bool {
        o.trigger && !input.entity_hit
    }

    /// `stopBreakingBlock`.
    fn stop_breaking(&mut self, out: &mut Actions) {
        self.breaking = false;
        if std::mem::take(&mut self.break_attack_held) {
            out.release = true;
        }
    }

    fn generate_jitter(&mut self, o: &Options) {
        if !o.jitter {
            return;
        }
        let h = self.unit() * 14.0 - 7.0;
        let v = self.unit() * 14.0 - 7.0;
        self.jitter_target = (h, v);
        self.jitter_steps = (h.abs() + v.abs()) * 0.45;
        self.jitter_remaining = self.jitter_steps;
    }

    /// `ClickEngine.updateJitter` (pre tick).
    pub fn tick(&mut self, o: &Options) {
        if !o.jitter {
            return;
        }
        if self.jitter_remaining > 0.0 {
            self.pending_jitter.0 += (self.jitter_target.0 / self.jitter_steps) as f32;
            self.pending_jitter.1 += (self.jitter_target.1 / self.jitter_steps) as f32;
            self.jitter_remaining -= 1.0;
        } else {
            self.pending_jitter = (self.pending_jitter.0.trunc(), self.pending_jitter.1.trunc());
        }
    }

    /// One frame: the worker cycle plus the pre-render jitter application.
    pub fn frame(&mut self, o: &Options, input: &Input) -> Actions {
        let mut out = Actions::default();
        let now = input.now_ms;
        // updateBlockedState / computeBlocked.
        let blocked = if input.playing && !self.allow_click(o, input) {
            self.breaking = true;
            true
        } else {
            self.stop_breaking(&mut out);
            !input.playing
        };
        if self.breaking && input.playing && Self::activation_held(o, input) && !input.attack_mapped
        {
            out.press = true;
            self.generate_jitter(o);
            self.break_attack_held = true;
        }
        match self.phase {
            Phase::Hold(until) if now >= until => {
                // shouldSuppressClickRelease: keep mining a targeted block.
                if self.breaking && input.attack_mapped {
                    self.break_attack_held = true;
                } else {
                    out.release = true;
                }
                let release = self.release_after;
                self.phase = Phase::Wait(now + release);
            }
            Phase::Wait(until) if now >= until => self.phase = Phase::Idle,
            _ => {}
        }
        if self.phase == Phase::Idle && !blocked {
            self.cycle(o, input, &mut out);
        }
        if out.press && out.release && self.break_attack_held {
            // A block-break press supersedes this frame's release.
            out.release = false;
        }
        if o.jitter && input.playing && self.pending_jitter != (0.0, 0.0) {
            out.turn = (
                self.pending_jitter.0 as i32,
                -(self.pending_jitter.1 as i32),
            );
            self.pending_jitter = (
                (self.pending_jitter.0 % 1.0).trunc(),
                (self.pending_jitter.1 % 1.0).trunc(),
            );
        }
        out
    }

    fn cycle(&mut self, o: &Options, input: &Input, out: &mut Actions) {
        let now = input.now_ms;
        if !input.playing {
            return;
        }
        if !Self::activation_held(o, input) {
            self.activation = None;
            return;
        }
        let started = *self.activation.get_or_insert(now);
        if now.saturating_sub(started) < 50 + o.start_delay {
            return;
        }
        if input.use_claimed || (o.sneak_only && !input.sneaking) {
            return;
        }
        if o.limit_items && !item_limit::matches(&o.allowed_items, &input.held) {
            return;
        }
        let mut delay = self.delay(o, now).saturating_sub(5);
        if delay <= 50 {
            delay = 45;
        }
        let scale = (100.0 - delay.min(99) as f64 + 45.0) / 100.0;
        let bias = 40.0 * scale;
        let release = ((30 + self.signed_ten()) as f64 + bias) / 100.0;
        let hold = (delay as f64 * (1.0 - release)) as u64;
        let wait = (delay as f64 * release) as u64;
        if Self::defer(o, input) {
            return;
        }
        out.press = true;
        self.generate_jitter(o);
        self.release_after = wait;
        self.phase = Phase::Hold(now + hold);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::settings::Settings;
    use super::*;

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in left_definitions() {
            settings.define(d).unwrap();
        }
        Options::read(&settings, LEFT)
    }
    fn input(now: u64) -> Input {
        Input {
            now_ms: now,
            playing: true,
            attack_held: true,
            ..Input::default()
        }
    }
    /// Clicks per second over `ms` of 1 ms frames.
    fn run(e: &mut Engine, o: &Options, ms: u64, f: impl Fn(u64) -> Input) -> (u32, u32) {
        let (mut presses, mut releases) = (0, 0);
        for now in 0..ms {
            let a = e.frame(o, &f(now));
            presses += u32::from(a.press);
            releases += u32::from(a.release);
        }
        (presses, releases)
    }

    #[test]
    fn defaults_and_cps_bounds_per_mode() {
        let o = options();
        assert!(o.hold && !o.trigger && o.randomization == Randomization::Extra);
        assert_eq!(o.cps, (6, 13));
        assert_eq!(o.suffix(), "6-13cps");
        for mode in [
            Randomization::Normal,
            Randomization::Extra,
            Randomization::ExtraPlus,
        ] {
            let mut o = options();
            o.randomization = mode;
            let mut e = Engine::new(11);
            let (presses, releases) = run(&mut e, &o, 10_000, input);
            // 6..13 CPS with the reference's phase pauses: 3.5..14 per second.
            assert!(
                (35..=140).contains(&presses),
                "{mode:?}: {presses} clicks in 10 s"
            );
            assert!(
                presses.abs_diff(releases) <= 1,
                "{mode:?}: {presses}/{releases}"
            );
        }
    }

    #[test]
    fn no_clicks_without_activation_screen_or_item() {
        let o = options();
        let mut e = Engine::new(3);
        assert_eq!(
            run(&mut e, &o, 2000, |n| Input {
                attack_held: false,
                ..input(n)
            })
            .0,
            0
        );
        assert_eq!(
            run(&mut e, &o, 2000, |n| Input {
                playing: false,
                ..input(n)
            })
            .0,
            0
        );
        let mut limited = options();
        limited.limit_items = true;
        assert_eq!(run(&mut Engine::new(3), &limited, 2000, input).0, 0);
        let sword = |n| {
            let mut i = input(n);
            i.held.id = Some("iron_sword".into());
            i
        };
        assert!(run(&mut Engine::new(3), &limited, 2000, sword).0 > 0);
        // The first 50 ms after activation never click.
        assert_eq!(run(&mut Engine::new(3), &o, 50, input).0, 0);
    }

    #[test]
    fn trigger_mode_and_break_blocks() {
        let mut o = options();
        o.trigger = true;
        assert_eq!(run(&mut Engine::new(5), &o, 2000, input).0, 0);
        assert!(
            run(&mut Engine::new(5), &o, 2000, |n| Input {
                entity_hit: true,
                ..input(n)
            })
            .0 > 0
        );
        let mut o = options();
        o.break_blocks = true;
        // Aiming at a block holds attack down to mine instead of clicking.
        let mut e = Engine::new(5);
        let first = e.frame(
            &o,
            &Input {
                block_hit: true,
                ..input(0)
            },
        );
        assert!(first.press);
        let (presses, _) = run(&mut e, &o, 1000, |n| Input {
            block_hit: true,
            attack_mapped: true,
            ..input(n + 1)
        });
        assert_eq!(presses, 0);
        assert!(e.reset());
    }

    #[test]
    fn jitter_turns_only_when_enabled() {
        let mut o = options();
        let mut e = Engine::new(9);
        let mut turned = false;
        for n in 0..2000 {
            if n % 50 == 0 {
                e.tick(&o);
            }
            turned |= e.frame(&o, &input(n)).turn != (0, 0);
        }
        assert!(!turned);
        o.jitter = true;
        for n in 2000..6000 {
            if n % 50 == 0 {
                e.tick(&o);
            }
            turned |= e.frame(&o, &input(n)).turn != (0, 0);
        }
        assert!(turned);
    }
}
