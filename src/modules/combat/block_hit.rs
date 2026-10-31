// BlockHit (reference `BlockHit` with its modes `ManualBlockHitMode`,
// `PredictBlockHitMode`, `AutoBlockHitMode` and `LagBlockHitMode`). Every mode
// drives the use key while a sword is in the main hand; on 26.3 that raises an
// off-hand shield, or blocks with a sword that carries the `blocks_attacks`
// component, as the reference's modern path does. Manual blocks on a share of
// the attack clicks, Predict blocks ahead of the next hit it expects, Auto
// lets the clickers block every third targeted click, and Lag holds the
// release-use packet back so the server sees the block for longer.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::targeting::entities::{
    EntityClass, EntityView, PlayerView, distance, int_yaw_difference,
};
use super::super::super::targeting::filter::People;
use super::super::rng::Rng;
use super::kill_aura::range;
use serde_json::{Value, json};

pub const ID: &str = "combat.block-hit";
pub const MANUAL: &str = "Manual";
pub const PREDICT: &str = "Predict";
pub const AUTO: &str = "Auto";
pub const LAG: &str = "Lag";

/// `PredictBlockHitMode` constants.
const DAMAGEABLE_HURT_RESISTANT_TIME: i32 = 10;
const DAMAGE_INTERVAL_CAPACITY: usize = 8;
const PREDICTION_SAMPLE_COUNT: usize = 3;
const MIN_DAMAGE_INTERVAL_MILLIS: u64 = 250;
const MAX_DAMAGE_INTERVAL_MILLIS: u64 = 1500;
const TICK_MILLIS: u64 = 50;
/// `AutoBlockHitMode`'s `maxTargetStreak`.
const MAX_TARGET_STREAK: i32 = 3;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mode_is = |m: &str| (key("mode"), json!(m));
    let mut mode = Definition::choice(&key("mode"), "Mode", &[MANUAL, PREDICT, AUTO, LAG], MANUAL);
    mode.description = "Manual: Blockhit based on your CPS\nPredict: Predicts when a player can hit you and blocks ahead of time\nAuto: Legacy auto mode from AutoClicker\nLag: Lags you after blocking to maximize server side block time\n".into();
    let described = |d: Definition, text: &str| {
        let mut d = d;
        d.description = text.into();
        d
    };
    let (manual_key, manual) = mode_is(MANUAL);
    let (predict_key, predict) = mode_is(PREDICT);
    let (lag_key, lag) = mode_is(LAG);
    vec![
        mode,
        described(
            Definition::new(
                &key("chance"),
                "Chance",
                Kind::Range {
                    min: 0.0,
                    max: 100.0,
                    step: 1.0,
                },
                json!([70.0, 90.0]),
            )
            .when(&manual_key, manual),
            "Chance that a click will blockhit\n(Blocks per second = Your CPS * Chance)",
        ),
        described(
            Definition::number(
                &key("maximum-hurt-time"),
                "Maximum hurt time",
                0.0,
                50.0,
                500.0,
                10.0,
            )
            .when(&predict_key, predict.clone()),
            "Milliseconds before you can take damage again to start blocking",
        ),
        described(
            Definition::boolean(&key("include-ping"), "Include ping", true)
                .when(&predict_key, predict.clone()),
            "Adds ping to maximum hurt time timing",
        ),
        described(
            Definition::number(&key("hold-after"), "Hold after", 0.0, 2.0, 10.0, 1.0)
                .when(&predict_key, predict),
            "Ticks to keep blocking after hurt time ends if no hit arrives",
        ),
        Definition::new(
            &key("delay"),
            "Delay",
            Kind::Range {
                min: 0.0,
                max: 500.0,
                step: 1.0,
            },
            json!([50.0, 100.0]),
        )
        .when(&lag_key, lag.clone()),
        described(
            Definition::boolean(&key("require-mouse-down"), "Require mouse down", true),
            "Require block to be pressed to blockhit",
        ),
        described(
            Definition::boolean(&key("ignore-manual-block"), "Ignore manual block", true)
                .when(&key("require-mouse-down"), json!(true))
                .when(&lag_key, lag.clone()),
            "Prevents manually blocking, useful for holding right click to activate",
        ),
        described(
            Definition::number(&key("angle"), "Angle", 0.0, 90.0, 360.0, 1.0)
                .when(&lag_key, lag.clone()),
            "Max target angle to blockhit",
        ),
        described(
            Definition::number(&key("distance"), "Distance", 0.0, 5.0, 6.0, 0.1)
                .when(&lag_key, lag),
            "Max target distance to blockhit",
        ),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Manual,
    Predict,
    Auto,
    Lag,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Manual => MANUAL,
            Mode::Predict => PREDICT,
            Mode::Auto => AUTO,
            Mode::Lag => LAG,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub chance: (f64, f64),
    pub maximum_hurt_time: f64,
    pub include_ping: bool,
    pub hold_after: i32,
    pub delay: (f64, f64),
    pub require_mouse_down: bool,
    pub ignore_manual_block: bool,
    pub angle: f64,
    pub distance: f64,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let key = |k: &str| format!("{ID}.{k}");
        let number = |k: &str, d: f64| settings.get(&key(k)).and_then(Value::as_f64).unwrap_or(d);
        let flag = |k: &str| {
            settings
                .effective(&key(k))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let mode = match settings.get(&key("mode")).and_then(Value::as_str) {
            Some(PREDICT) => Mode::Predict,
            Some(AUTO) => Mode::Auto,
            Some(LAG) => Mode::Lag,
            _ => Mode::Manual,
        };
        Self {
            mode,
            chance: range(settings, &key("chance"), (70.0, 90.0)),
            maximum_hurt_time: number("maximum-hurt-time", 50.0),
            include_ping: flag("include-ping"),
            hold_after: number("hold-after", 2.0) as i32,
            delay: range(settings, &key("delay"), (50.0, 100.0)),
            require_mouse_down: flag("require-mouse-down"),
            ignore_manual_block: flag("ignore-manual-block"),
            angle: number("angle", 90.0),
            distance: number("distance", 5.0),
        }
    }
    /// `getDetailedSuffix`: Manual's chance, Lag's delay (red while it holds
    /// packets), else the mode.
    pub fn suffix(&self, buffering: bool) -> String {
        let whole = |(lo, hi): (f64, f64)| format!("{}-{}", lo.round() as i64, hi.round() as i64);
        match self.mode {
            Mode::Manual => format!("{}%", whole(self.chance)),
            Mode::Lag => {
                let text = format!("Lag {}ms", whole(self.delay));
                if buffering {
                    format!("\u{a7}c{text}")
                } else {
                    text
                }
            }
            mode => mode.name().into(),
        }
    }
}

/// `BlockHit.findTarget(angle, distance)`: the first player (`RotationUtil.u`)
/// closer than `distance`, a valid target within half the angle of our yaw,
/// that also looks at us within half the angle (`RotationUtil.o`, friends
/// excluded).
pub fn find_target(
    player: &PlayerView,
    entities: &[EntityView],
    people: &impl People,
    angle: f64,
    max_distance: f64,
) -> bool {
    let half = angle / 2.0;
    let Some(target) = entities.iter().find(|e| {
        e.class == EntityClass::Player
            && e.alive()
            && distance(e.position, player.position) < max_distance
            && !people.friend(&e.name)
            && !people.teammate(e)
            && !people.bot(e)
            && f64::from(int_yaw_difference(player, e.position)) < half
    }) else {
        return false;
    };
    // `RotationUtil.a(target, player)`: from the target's yaw toward us.
    let Some((yaw, pitch)) = target.look else {
        return false;
    };
    let them = PlayerView {
        position: target.position,
        tick_position: target.position,
        yaw,
        pitch,
        strafe: 0.0,
    };
    !people.friend(&target.name)
        && distance(player.position, target.position) < max_distance
        && f64::from(int_yaw_difference(&them, player.position)) < half
}

/// What BlockHit sees at a tick or a click.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub player: bool,
    /// `BlockHit.isHoldingSword`: no screen and a sword in the main hand.
    pub holding_sword: bool,
    /// A sword in the main hand, screen or not (Auto's own check).
    pub sword_in_hand: bool,
    /// `ClientSettings.isUseItemButtonDown`/`isAttackButtonDown`: physical.
    pub use_down: bool,
    pub attack_down: bool,
    /// `isUsingItem`/`isBlocking`.
    pub using: bool,
    pub hurt_time: i32,
    /// `getHurtResistantTime`.
    pub hurt_resistant_time: i32,
    pub focused: bool,
    /// The extended-reach ray trace hits an entity (Auto).
    pub targeting_entity: bool,
    /// `findTarget(angle, distance)` (Lag).
    pub target: bool,
    /// `findDefaultTarget` (Predict): 90 degrees, 5 blocks.
    pub default_target: bool,
    /// LeftClicker or SilentAura is enabled: Manual leaves it to them.
    pub clicker: bool,
    /// `AttackPacketTimingTracker`.
    pub expected_hurt_ticks: i32,
    pub average_hit_delay: u64,
    pub now_ms: u64,
}

impl Input {
    fn hurt_expired(&self) -> bool {
        self.hurt_time > self.expected_hurt_ticks + 1
    }
}

/// A change of the use key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// `KeyBinding.setPressed(true)`.
    Press,
    /// `KeyBindingInputState.sendRightButtonDown`: pressed with a click.
    Click,
    /// `setPressed(false)`/`sendRightButtonUp`.
    Release,
}

/// What a mouse press becomes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mouse {
    /// The press is cancelled (`EventMouseButton.setCancelled`).
    pub cancel: bool,
    pub key: Option<Key>,
}

/// Lag's verdict on a serverbound packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Send {
    Pass,
    /// Held back at the end of the queue.
    Queue,
    /// Every queued packet goes, in order, then this one.
    Flush,
}

#[derive(Debug, Default)]
struct Manual {
    blocking: bool,
    release_ms: u64,
}

#[derive(Debug, Default)]
struct Predict {
    blocking: bool,
    damage_observed: bool,
    hold_started: bool,
    hold_until: u64,
    last_damage: u64,
    intervals: [u64; DAMAGE_INTERVAL_CAPACITY],
    count: usize,
    next: usize,
    /// `PRE_TICK_EXECUTOR.execute(() -> setBlocking(false))` after a hit.
    release_pending: bool,
}

#[derive(Debug, Default)]
struct Lag {
    blocking: bool,
    cycle_completed: bool,
    buffering: bool,
    delay_ms: i64,
    buffer_start: u64,
    queued: usize,
}

#[derive(Debug)]
pub struct BlockHit {
    manual: Manual,
    predict: Predict,
    target_streak: i32,
    lag: Lag,
    /// `SharedModuleControlClaims.primaryAction`: FakeLag yields while set.
    claimed: bool,
    random: Rng,
}

impl Default for BlockHit {
    fn default() -> Self {
        Self {
            manual: Manual::default(),
            predict: Predict::default(),
            target_streak: 1,
            lag: Lag::default(),
            claimed: false,
            random: Rng::new(0x9e37_79b9_7f4a_7c15),
        }
    }
}

/// `setBlocking` of Manual and Predict: the use key follows a state change.
fn set(blocking: &mut bool, to: bool) -> Option<Key> {
    if *blocking == to {
        return None;
    }
    *blocking = to;
    Some(if to { Key::Press } else { Key::Release })
}

/// The last of two key changes in one handler wins.
fn then(first: Option<Key>, second: Option<Key>) -> Option<Key> {
    second.or(first)
}

impl BlockHit {
    pub fn claimed(&self) -> bool {
        self.claimed
    }
    pub fn buffering(&self) -> bool {
        self.lag.buffering
    }
    /// The active mode's `isBlocking` (Auto's has the side effect of its
    /// `shouldBlock`, so it is not offered here).
    pub fn blocking(&self, o: &Options) -> bool {
        match o.mode {
            Mode::Manual => self.manual.blocking,
            Mode::Predict => self.predict.blocking,
            Mode::Lag => self.lag.blocking,
            Mode::Auto => false,
        }
    }

    /// `BlockHit.shouldBlock`, which the clickers ask at each click
    /// (`shouldSimulateBlockHit`).
    pub fn should_block(&mut self, o: &Options, input: &Input) -> bool {
        match o.mode {
            Mode::Manual => self.manual_should_block(o, input),
            Mode::Auto => self.auto_should_block(o, input),
            Mode::Predict | Mode::Lag => false,
        }
    }

    fn manual_should_block(&mut self, o: &Options, input: &Input) -> bool {
        if !input.holding_sword || (o.require_mouse_down && !input.use_down) {
            return false;
        }
        let chance = self.random.range(o.chance);
        chance >= self.random.double() * 100.0
    }

    /// `AutoBlockHitMode.shouldBlockSwordUse`: every third click in a row on
    /// an entity.
    fn auto_should_block(&mut self, o: &Options, input: &Input) -> bool {
        if (o.require_mouse_down && !input.use_down) || !input.sword_in_hand {
            return false;
        }
        let targeting = input.targeting_entity;
        let mut block = true;
        if self.target_streak != 1 || !targeting {
            block = false;
            if self.target_streak >= MAX_TARGET_STREAK {
                self.target_streak = 0;
            }
        }
        self.target_streak = if targeting { self.target_streak + 1 } else { 1 };
        block
    }

    /// `EventMouseButton` for a press of the attack (left) or use (right)
    /// key.
    pub fn press(&mut self, o: &Options, input: &Input, attack: bool) -> Mouse {
        match o.mode {
            Mode::Manual => {
                if !attack || input.clicker || !self.manual_should_block(o, input) {
                    return Mouse::default();
                }
                if self.manual.blocking || input.using {
                    return Mouse::default();
                }
                let key = set(&mut self.manual.blocking, true);
                self.manual.release_ms = input.now_ms + 50;
                Mouse { cancel: false, key }
            }
            Mode::Auto | Mode::Predict => Mouse {
                cancel: !attack && input.holding_sword && o.require_mouse_down && input.attack_down,
                key: None,
            },
            Mode::Lag => Mouse::default(),
        }
    }

    /// Predict's `onDamaged`: the local player took a hit.
    pub fn damaged(&mut self, o: &Options, now_ms: u64) {
        if o.mode != Mode::Predict {
            return;
        }
        let p = &mut self.predict;
        p.damage_observed = true;
        p.hold_started = false;
        p.hold_until = 0;
        if p.last_damage > 0 {
            let interval = now_ms.saturating_sub(p.last_damage);
            if (MIN_DAMAGE_INTERVAL_MILLIS..=MAX_DAMAGE_INTERVAL_MILLIS).contains(&interval) {
                p.intervals[p.next] = interval;
                p.next = (p.next + 1) % DAMAGE_INTERVAL_CAPACITY;
                p.count = (p.count + 1).min(DAMAGE_INTERVAL_CAPACITY);
            } else {
                p.count = 0;
                p.next = 0;
            }
        }
        p.last_damage = now_ms;
        p.release_pending = true;
    }

    /// `EventPreTick` of the active mode.
    pub fn tick(&mut self, o: &Options, input: &Input) -> Option<Key> {
        match o.mode {
            Mode::Manual => self.manual_tick(input),
            Mode::Predict => self.predict_tick(o, input),
            Mode::Auto => None,
            Mode::Lag => self.lag_tick(o, input),
        }
    }

    fn manual_tick(&mut self, input: &Input) -> Option<Key> {
        if input.clicker || !input.player {
            return None;
        }
        let release = self.manual.release_ms > 0 && input.now_ms >= self.manual.release_ms;
        if input.hurt_expired() || release {
            self.manual.release_ms = 0;
            return set(&mut self.manual.blocking, false);
        }
        None
    }

    fn early_window_ms(o: &Options, input: &Input) -> u64 {
        let ping = if o.include_ping {
            input.average_hit_delay
        } else {
            0
        };
        o.maximum_hurt_time.round().max(0.0) as u64 + ping + TICK_MILLIS
    }

    fn predict_tick(&mut self, o: &Options, input: &Input) -> Option<Key> {
        let mut key = None;
        if std::mem::take(&mut self.predict.release_pending) {
            key = set(&mut self.predict.blocking, false);
        }
        let can_predict = input.player
            && input.holding_sword
            && (!o.require_mouse_down || input.use_down)
            && input.default_target;
        if !can_predict {
            let blocking = self.predict.blocking;
            self.predict = Predict {
                blocking,
                ..Predict::default()
            };
            return then(key, set(&mut self.predict.blocking, false));
        }
        let hrt = input.hurt_resistant_time;
        let early_ms = Self::early_window_ms(o, input);
        let early_ticks = early_ms.div_ceil(TICK_MILLIS) as i32;
        let can_take_damage_soon = hrt <= DAMAGEABLE_HURT_RESISTANT_TIME + early_ticks;
        let hold = o.hold_after.max(0) as u64;
        let p = &mut self.predict;
        let stable = p.count >= PREDICTION_SAMPLE_COUNT && p.last_damage > 0;
        if p.damage_observed && stable {
            let samples = p.count.min(PREDICTION_SAMPLE_COUNT);
            let total: u64 = (0..samples)
                .map(|offset| {
                    p.intervals[(p.next + DAMAGE_INTERVAL_CAPACITY - 1 - offset)
                        % DAMAGE_INTERVAL_CAPACITY]
                })
                .sum();
            let expected = p.last_damage + total / samples as u64;
            let inside = input.now_ms + early_ms >= expected
                && input.now_ms <= expected + hold * TICK_MILLIS;
            return then(key, set(&mut p.blocking, inside && can_take_damage_soon));
        }
        if hrt > DAMAGEABLE_HURT_RESISTANT_TIME {
            p.hold_started = false;
            p.hold_until = 0;
            return then(key, set(&mut p.blocking, can_take_damage_soon));
        }
        if p.blocking {
            // `releaseAfterConfiguredHold`
            if hold == 0 {
                return then(key, set(&mut p.blocking, false));
            }
            if !p.hold_started {
                p.hold_started = true;
                p.hold_until = input.now_ms + hold * TICK_MILLIS;
            }
            if input.now_ms >= p.hold_until {
                return then(key, set(&mut p.blocking, false));
            }
        }
        key
    }

    fn lag_set(&mut self, to: bool) -> Option<Key> {
        if self.lag.blocking == to {
            return None;
        }
        self.lag.blocking = to;
        Some(if to { Key::Click } else { Key::Release })
    }

    fn lag_elapsed(&self, now_ms: u64, ms: i64) -> bool {
        now_ms as i64 - self.lag.buffer_start as i64 >= ms
    }

    fn lag_tick(&mut self, o: &Options, input: &Input) -> Option<Key> {
        if !input.player || !input.focused {
            return None;
        }
        if input.target {
            let should_block = input.holding_sword && !(o.require_mouse_down && !input.use_down);
            if input.hurt_expired() {
                self.lag.cycle_completed = false;
                return self.lag_set(false);
            }
            if should_block {
                if self.lag.buffering && self.lag_elapsed(input.now_ms, self.lag.delay_ms - 50) {
                    return self.lag_set(true);
                }
                if !self.lag.cycle_completed {
                    if !self.lag.blocking {
                        return self.lag_set(true);
                    }
                    self.lag.cycle_completed = true;
                    return self.lag_set(false);
                }
                if !self.lag.blocking && !input.using {
                    self.lag.cycle_completed = false;
                }
            } else if self.lag.blocking {
                self.lag.cycle_completed = false;
                return self.lag_set(false);
            }
            return None;
        }
        if input.holding_sword
            && o.require_mouse_down
            && o.ignore_manual_block
            && !self.lag.buffering
            && input.use_down
            && input.using
        {
            // A manual block without a target is let go.
            self.lag.blocking = false;
            self.lag.cycle_completed = false;
            return Some(Key::Release);
        }
        if input.hurt_expired() {
            self.lag.cycle_completed = false;
            return self.lag_set(false);
        }
        if self.lag.blocking {
            self.lag.cycle_completed = false;
            return self.lag_set(false);
        }
        None
    }

    /// Lag's `onPacketSend` (lowest priority, so packets other modules took
    /// never get here). `target` and `sword` are only read while buffering.
    pub fn send(
        &mut self,
        o: &Options,
        release_use: bool,
        target: bool,
        sword: bool,
        now_ms: u64,
    ) -> Send {
        if o.mode != Mode::Lag {
            return Send::Pass;
        }
        if self.lag.buffering {
            if !target || !sword || self.lag_elapsed(now_ms, self.lag.delay_ms) {
                self.lag.queued = 0;
                self.lag.buffering = false;
                self.lag.cycle_completed = false;
                return Send::Flush;
            }
            self.lag.queued += 1;
            return Send::Queue;
        }
        let mut verdict = Send::Pass;
        if release_use {
            self.claimed = true;
            self.lag.queued += 1;
            self.lag.buffering = true;
            self.lag.delay_ms = self.random.range(o.delay) as i64;
            self.lag.buffer_start = now_ms;
            verdict = Send::Queue;
        }
        if !self.lag.buffering && self.lag.queued == 0 {
            self.claimed = false;
        }
        verdict
    }

    /// The queued packets went out (disable, disconnect, no player): the
    /// claim goes with them.
    pub fn flushed(&mut self) {
        self.lag.queued = 0;
        self.lag.buffering = false;
        self.lag.cycle_completed = false;
        self.claimed = false;
    }

    pub fn queued(&self) -> usize {
        self.lag.queued
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::targeting::filter::NameLists;
    use super::super::super::super::targeting::filter::tests::entity;
    use super::*;

    fn options(mode: Mode) -> Options {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.validate_conditions().unwrap();
        s.set(&format!("{ID}.mode"), json!(mode.name())).unwrap();
        Options::read(&s)
    }
    fn input() -> Input {
        Input {
            player: true,
            holding_sword: true,
            sword_in_hand: true,
            use_down: true,
            focused: true,
            target: true,
            default_target: true,
            expected_hurt_ticks: 2,
            now_ms: 10_000,
            ..Input::default()
        }
    }

    #[test]
    fn options_defaults_visibility_and_suffix() {
        let manual = options(Mode::Manual);
        assert_eq!(manual.chance, (70.0, 90.0));
        assert!(manual.require_mouse_down);
        // Hidden outside Lag, so the effective value is false.
        assert!(!manual.ignore_manual_block);
        assert_eq!(manual.suffix(false), "70-90%");
        let lag = options(Mode::Lag);
        assert!(lag.ignore_manual_block);
        assert_eq!(
            (lag.angle, lag.distance, lag.delay),
            (90.0, 5.0, (50.0, 100.0))
        );
        assert_eq!(lag.suffix(false), "Lag 50-100ms");
        assert_eq!(lag.suffix(true), "\u{a7}cLag 50-100ms");
        let predict = options(Mode::Predict);
        assert!(predict.include_ping);
        assert_eq!((predict.maximum_hurt_time, predict.hold_after), (50.0, 2));
        assert_eq!(predict.suffix(false), "Predict");
        assert_eq!(options(Mode::Auto).suffix(true), "Auto");
    }

    #[test]
    fn targets_must_be_close_ahead_and_facing_us() {
        let me = PlayerView {
            position: [0.0, 0.0, 0.0],
            tick_position: [0.0, 0.0, 0.0],
            yaw: 0.0,
            ..PlayerView::default()
        };
        // Yaw 0 looks toward +z; a player 3 blocks ahead looking back (180).
        let mut foe = entity(EntityClass::Player, "Foe", [0.2, 0.0, 3.0]);
        foe.look = Some((180.0, 0.0));
        let people = NameLists::default();
        assert!(find_target(&me, &[foe.clone()], &people, 90.0, 5.0));
        // Too far, or looking away.
        assert!(!find_target(&me, &[foe.clone()], &people, 90.0, 2.5));
        let mut away = foe.clone();
        away.look = Some((0.0, 0.0));
        assert!(!find_target(&me, &[away], &people, 90.0, 5.0));
        // Behind us.
        let mut behind = foe.clone();
        behind.position = [0.2, 0.0, -3.0];
        behind.look = Some((0.0, 0.0));
        assert!(!find_target(&me, &[behind], &people, 90.0, 5.0));
        // Friends and mobs are not targets.
        let friends = NameLists {
            friends: vec!["foe".into()],
            ..NameLists::default()
        };
        assert!(!find_target(&me, &[foe.clone()], &friends, 90.0, 5.0));
        let mut cow = foe;
        cow.class = EntityClass::Peaceful;
        assert!(!find_target(&me, &[cow], &people, 90.0, 5.0));
    }

    #[test]
    fn manual_blocks_a_click_then_lets_go_after_50_ms() {
        let mut o = options(Mode::Manual);
        o.chance = (100.0, 100.0);
        let mut b = BlockHit::default();
        let i = input();
        let m = b.press(&o, &i, true);
        assert_eq!(
            m,
            Mouse {
                cancel: false,
                key: Some(Key::Press)
            }
        );
        assert!(b.blocking(&o));
        // A second click while blocking does nothing; nor does a use click.
        assert_eq!(b.press(&o, &i, true).key, None);
        assert_eq!(b.press(&o, &i, false).key, None);
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 10_049,
                    ..i
                }
            ),
            None
        );
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 10_050,
                    ..i
                }
            ),
            Some(Key::Release)
        );
        // A hurt player stops blocking at once.
        b.press(
            &o,
            &Input {
                now_ms: 20_000,
                ..i
            },
            true,
        );
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 20_001,
                    hurt_time: 9,
                    ..i
                }
            ),
            Some(Key::Release)
        );
        // The clickers own it while enabled, and the mouse must be down.
        assert_eq!(b.press(&o, &Input { clicker: true, ..i }, true).key, None);
        assert_eq!(
            b.press(
                &o,
                &Input {
                    use_down: false,
                    ..i
                },
                true
            )
            .key,
            None
        );
        assert!(b.should_block(&o, &i));
        o.chance = (0.0, 0.0);
        assert!(!(0..20).any(|_| b.should_block(&o, &i)));
    }

    #[test]
    fn auto_blocks_every_third_targeted_click_and_swallows_use() {
        let o = options(Mode::Auto);
        let mut b = BlockHit::default();
        let on = Input {
            targeting_entity: true,
            ..input()
        };
        let got: Vec<bool> = (0..7).map(|_| b.should_block(&o, &on)).collect();
        assert_eq!(got, [true, false, false, true, false, false, true]);
        // A miss restarts the streak.
        assert!(!b.should_block(&o, &input()));
        assert!(b.should_block(&o, &on));
        assert!(!b.should_block(
            &o,
            &Input {
                use_down: false,
                ..on
            }
        ));
        // A use press while attacking is cancelled; an attack press is not.
        let attacking = Input {
            attack_down: true,
            ..on
        };
        assert!(b.press(&o, &attacking, false).cancel);
        assert!(!b.press(&o, &attacking, true).cancel);
        assert!(!b.press(&o, &on, false).cancel);
    }

    #[test]
    fn predict_reacts_then_predicts_the_next_hit() {
        let o = options(Mode::Predict);
        let mut b = BlockHit::default();
        let i = input();
        // Not hurt, not blocking: nothing.
        assert_eq!(b.tick(&o, &i), None);
        // Early window: 50 + 0 ping + 50 = 100 ms, 2 ticks. Hurt resistant
        // time 12 (hurt time 2) blocks; 20 does not.
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    hurt_resistant_time: 20,
                    ..i
                }
            ),
            None
        );
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    hurt_resistant_time: 12,
                    ..i
                }
            ),
            Some(Key::Press)
        );
        // Hurt time over: hold for 2 ticks (100 ms), then let go.
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 10_000,
                    ..i
                }
            ),
            None
        );
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 10_099,
                    ..i
                }
            ),
            None
        );
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 10_100,
                    ..i
                }
            ),
            Some(Key::Release)
        );
        // Four hits 500 ms apart make a stable pattern: block from 100 ms
        // before the next expected hit to 100 ms after it.
        let mut b = BlockHit::default();
        for n in 0..4 {
            b.damaged(&o, 1_000 + n * 500);
        }
        let at = |ms| Input { now_ms: ms, ..i };
        assert_eq!(b.tick(&o, &at(2_600)), None);
        assert_eq!(b.tick(&o, &at(2_900)), Some(Key::Press));
        assert_eq!(b.tick(&o, &at(3_100)), None);
        assert_eq!(b.tick(&o, &at(3_101)), Some(Key::Release));
        // A hit queues a release for the next tick; out of range resets.
        b.tick(&o, &at(2_950));
        b.damaged(&o, 3_000);
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    hurt_resistant_time: 20,
                    ..at(3_010)
                }
            ),
            Some(Key::Release)
        );
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    default_target: false,
                    ..at(3_020)
                }
            ),
            None
        );
        assert_eq!(b.predict.count, 0);
        // Use presses while attacking are swallowed.
        assert!(
            b.press(
                &o,
                &Input {
                    attack_down: true,
                    ..i
                },
                false
            )
            .cancel
        );
    }

    #[test]
    fn lag_cycles_the_block_and_holds_the_release_packet() {
        let mut o = options(Mode::Lag);
        o.delay = (200.0, 200.0);
        let mut b = BlockHit::default();
        let i = input();
        assert_eq!(b.tick(&o, &i), Some(Key::Click));
        assert_eq!(b.tick(&o, &Input { using: true, ..i }), Some(Key::Release));
        // The release-use packet is held and FakeLag yields.
        assert_eq!(b.send(&o, true, true, true, 10_000), Send::Queue);
        assert!(b.claimed() && b.buffering());
        assert_eq!(b.send(&o, false, true, true, 10_100), Send::Queue);
        // 150 ms in, the next tick blocks again ahead of the flush.
        assert_eq!(
            b.tick(
                &o,
                &Input {
                    now_ms: 10_150,
                    using: true,
                    ..i
                }
            ),
            Some(Key::Click)
        );
        assert_eq!(b.send(&o, false, true, true, 10_199), Send::Queue);
        assert_eq!(b.queued(), 3);
        assert_eq!(b.send(&o, false, true, true, 10_200), Send::Flush);
        assert!(b.claimed() && !b.buffering());
        // The next packet passes and the claim goes.
        assert_eq!(b.send(&o, false, true, true, 10_250), Send::Pass);
        assert!(!b.claimed());
        // A lost target flushes at once.
        b.send(&o, true, true, true, 11_000);
        assert_eq!(b.send(&o, false, false, true, 11_001), Send::Flush);
        // Without a target a manual block is let go.
        let none = Input {
            target: false,
            using: true,
            ..i
        };
        assert_eq!(b.tick(&o, &none), Some(Key::Release));
        // Other modes never hold packets.
        assert_eq!(
            b.send(&options(Mode::Manual), true, true, true, 0),
            Send::Pass
        );
    }
}
