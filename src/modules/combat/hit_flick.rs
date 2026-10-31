// HitFlick (reference `HitFlick` and `HitFlickAdaptiveRotationController`):
// an attack on a target first turns the managed rotation away from it by the
// configured angle, then back onto it for the attack, so the hit's knockback
// leaves at the flicked angle. It owns the shared rotation claim at priority
// 6 (above SilentAura) while flicking, cancels the attack that started the
// flick and sends its own synthetic attack once back on target. Blink holds
// every outgoing packet during the flick and flushes them with the attack.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::super::targeting::entities::{EntityView, PlayerView};
use super::super::super::targeting::filter::{self, People, TargetFilter};
use super::super::rng::Rng;
use super::rotation::{self, Claim, Claimant, Managed, Step};
use serde_json::{Value, json};

pub const ID: &str = "combat.hit-flick";
/// `isValidTarget`: feet to feet.
pub const RANGE: f64 = 5.0;
/// The controller's speed while it is idle or returning to the view.
const IDLE_SPEED: f64 = 48.0;
/// `blinkTicks` before the held packets go without an attack.
const BLINK_TICKS: u32 = 7;
/// `hasReachedFlickRotation`'s tolerance in degrees.
const FLICK_TOLERANCE: f32 = 6.0;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let described = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    let mut out = filter::definitions(ID);
    out.extend([
        described(
            Definition::number(&key("angle"), "Angle", 0.0, 90.0, 360.0, 1.0),
            "0 = none, 90 = right, 180 = pull toward, 270 = left",
        ),
        described(
            Definition::number(&key("chance"), "Chance", 0.0, 100.0, 100.0, 1.0),
            "Chance of starting a hit flick for a given attack",
        ),
        described(
            Definition::number(&key("flick-delay"), "Flick delay", 0.0, 250.0, 2000.0, 25.0),
            "Minimum delay between hit flick attempts",
        ),
        described(
            Definition::boolean(&key("randomize-offset"), "Randomize offset", false),
            "Randomizes the configured angle by a per-flick range",
        ),
        described(
            Definition::number(&key("offset-range"), "Randomize offset", 0.0, 0.0, 180.0, 1.0)
                .when(&key("randomize-offset"), json!(true)),
            "Applies a random offset range around Angle\nExample: 10 means Angle +/- 5 degrees per flick",
        ),
        described(
            Definition::boolean(&key("strafe-invert"), "Strafe invert", false),
            "Flips the flick side when you strafe toward the current push direction",
        ),
        described(
            Definition::boolean(&key("select-hits"), "Select hits", true),
            "Only start a hit flick when the target is vulnerable",
        ),
        described(
            Definition::boolean(&key("blink"), "Blink", false),
            "Chokes outgoing packets during the flick and flushes once the attack is sent",
        ),
        described(
            Definition::boolean(&key("limit-items"), "Limit to items", false),
            "HitFlick functions only while holding selected items",
        ),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["swords"]),
        )
        .when(&key("limit-items"), json!(true)),
    ]);
    out
}

#[derive(Clone, Debug)]
pub struct Options {
    pub filter: TargetFilter,
    pub angle: f64,
    pub chance: f64,
    pub flick_delay: f64,
    pub randomize: bool,
    pub offset_range: f64,
    pub strafe_invert: bool,
    pub select_hits: bool,
    pub blink: bool,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let boolean = |k: &str, d: bool| get(k).and_then(Value::as_bool).unwrap_or(d);
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        Self {
            filter: TargetFilter::read(settings, ID),
            angle: number("angle", 90.0),
            chance: number("chance", 100.0),
            flick_delay: number("flick-delay", 250.0),
            randomize: boolean("randomize-offset", false),
            offset_range: number("offset-range", 0.0),
            strafe_invert: boolean("strafe-invert", false),
            select_hits: boolean("select-hits", true),
            blink: boolean("blink", false),
            limit_items: boolean("limit-items", false),
            allowed_items: get("allowed-items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
        }
    }
    /// `getConfiguredFlickAngle`.
    fn configured(&self) -> f32 {
        normalize(self.angle as f32)
    }
    /// `hasConfiguredAngle`.
    fn has_angle(&self) -> bool {
        rotation::wrap(self.configured()).abs() > 0.001
            || (self.randomize && self.offset_range as f32 > 0.001)
    }
    /// `getDetailedSuffix`: the angle, red while flicking.
    pub fn suffix(&self, flicking: bool) -> String {
        let angle = format!("{}deg", self.angle.round() as i64);
        if flicking {
            format!("\u{a7}c{angle}")
        } else {
            angle
        }
    }
}

/// `normalizeAngle`: into `[0, 360)`.
fn normalize(angle: f32) -> f32 {
    let n = angle % 360.0;
    if n < 0.0 { n + 360.0 } else { n }
}

/// `SilentAuraRotationMode`, which HitFlick shares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Idle,
    FlickingAway,
    Attacking,
}

/// The host state a flick reads.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    pub now_ms: u64,
    pub player: &'a PlayerView,
    pub eye_height: f64,
    pub entities: &'a [EntityView],
    /// No screen is open.
    pub no_screen: bool,
    pub held: &'a HeldItem,
    /// The physical strafe keys (left, right) for Strafe invert.
    pub strafe: (bool, bool),
    /// Entity hurt times by id (`EntityLivingBase.hurtTime`).
    pub hurt: &'a [(i32, i32)],
}

/// The entity the attack would hit (`objectMouseOver` under the managed
/// rotation): the nearest living, non-vehicle box along the ray, expanded
/// by HitBoxes.
pub fn ray(v: &View<'_>, rotation: (f32, f32), reach: f64, expand: f64) -> Option<i32> {
    let eye = HitFlick::eye(v);
    let dir = rotation::direction(rotation.0, rotation.1);
    v.entities
        .iter()
        .filter(|e| e.alive() && !e.vehicle)
        .filter_map(|e| {
            let min = e.bounds.min.map(|c| c - expand);
            let max = e.bounds.max.map(|c| c + expand);
            rotation::ray_box(eye, dir, reach, min, max).map(|d| (e.handle.id, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// What a flick start needs beyond the view.
#[derive(Clone, Copy, Debug, Default)]
pub struct Start {
    /// The candidate's hurt time (`EntityLivingBase.hurtTime`).
    pub hurt_time: i32,
    /// `AttackPacketTimingTracker.getExpectedHurtTimeTicks`.
    pub expected_hurt_ticks: i32,
    /// Someone holds the primary action claim already (BlockHit's Lag).
    pub primary_claimed: bool,
}

/// What the host does after a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Send the synthetic attack now (`requestSyntheticAttack(this)`).
    pub attack: bool,
    /// Send the held packets now.
    pub flush: bool,
}

#[derive(Debug)]
pub struct HitFlick {
    rng: Rng,
    mode: Mode,
    target: Option<i32>,
    flick_angle: f32,
    state_ticks: u32,
    attack_queued: bool,
    attacked: bool,
    /// The reference's flick-delay timer, starting 10 s in the past.
    last_flick_ms: Option<u64>,
    /// `setSprinting(false)` at the next player tick.
    cancel_sprint: bool,
    blinking: bool,
    blink_ticks: u32,
    /// HitFlick marked the primary action claim.
    claim_held: bool,
    /// HitFlick's controller is the active one.
    controlling: bool,
    /// The controller's settings once released (`resetFlickState`).
    returning: bool,
}
impl Default for HitFlick {
    fn default() -> Self {
        Self::new(0x041f_11c4)
    }
}
impl HitFlick {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            mode: Mode::Idle,
            target: None,
            flick_angle: 0.0,
            state_ticks: 0,
            attack_queued: false,
            attacked: false,
            last_flick_ms: None,
            cancel_sprint: false,
            blinking: false,
            blink_ticks: 0,
            claim_held: false,
            controlling: false,
            returning: false,
        }
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn target(&self) -> Option<i32> {
        self.target
    }
    pub fn flick_angle(&self) -> f32 {
        self.flick_angle
    }
    /// Outgoing packets are held (Blink).
    pub fn blinking(&self) -> bool {
        self.blinking
    }
    /// HitFlick holds the primary action claim.
    pub fn claim_held(&self) -> bool {
        self.claim_held
    }
    /// HitFlick's controller drives the managed rotation (also while it
    /// returns to the view after the flick).
    pub fn controlling(&self) -> bool {
        self.controlling
    }
    /// Another controller replaced HitFlick's (`setController`), or the
    /// managed rotation ended.
    pub fn yield_controller(&mut self) {
        self.controlling = false;
        self.returning = false;
    }
    /// The forward and sprint keys are forced down (`updateMovementKeys`).
    pub fn forcing_keys(&self) -> bool {
        self.mode == Mode::Attacking
    }
    /// `EventPrePlayerTick`: drop sprinting once, then cancel the next
    /// sprint start (`sprintCancelPending`).
    pub fn take_cancel_sprint(&mut self) -> bool {
        std::mem::take(&mut self.cancel_sprint)
    }

    fn entity<'a>(&self, v: &View<'a>) -> Option<&'a EntityView> {
        let id = self.target?;
        v.entities.iter().find(|e| e.handle.id == id)
    }

    /// `isValidTarget`.
    fn valid(o: &Options, v: &View<'_>, e: &EntityView, people: &impl People) -> bool {
        e.alive() && o.filter.valid(e, people) && v.player.distance(e.tick_position) <= RANGE
    }

    /// `canStartFlick`.
    pub fn can_start(&self, o: &Options, v: &View<'_>) -> bool {
        o.has_angle()
            && v.no_screen
            && (!o.limit_items || item_limit::matches(&o.allowed_items, v.held))
            && self
                .last_flick_ms
                .is_none_or(|t| v.now_ms.saturating_sub(t) as f64 >= o.flick_delay)
    }

    /// `getRandomizedFlickAngle` then `resolveStrafeAdjustedAngle`.
    fn resolve_angle(&mut self, o: &Options, v: &View<'_>) -> f32 {
        let angle = if o.randomize && o.offset_range > 0.0 {
            let offset = (self.rng.double() as f32 - 0.5) * o.offset_range as f32;
            normalize(o.angle as f32 + offset)
        } else {
            o.configured()
        };
        let (left, right) = v.strafe;
        if !o.strafe_invert || left == right {
            return angle;
        }
        let signed = rotation::wrap(angle);
        if signed.abs() <= 0.001 || (signed.abs() - 180.0).abs() <= 0.001 {
            return angle;
        }
        if (left && signed < 0.0) || (right && signed > 0.0) {
            normalize(-signed)
        } else {
            angle
        }
    }

    fn eye(v: &View<'_>) -> [f64; 3] {
        let p = v.player.tick_position;
        [p[0], p[1] + v.eye_height, p[2]]
    }

    /// `calculateTargetRotation`: toward the closest point of the target's
    /// box.
    fn target_rotation(v: &View<'_>, e: &EntityView) -> (f32, f32) {
        let eye = Self::eye(v);
        rotation::look_at(eye, e.bounds.closest(eye))
    }

    /// `startFlick`: true when the flick started (and the attack that asked
    /// is cancelled).
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        o: &Options,
        v: &View<'_>,
        target: i32,
        s: Start,
        people: &impl People,
        claim: &mut Claim,
        managed: &mut Managed,
    ) -> bool {
        if self.mode != Mode::Idle || !self.can_start(o, v) {
            return false;
        }
        let Some(e) = v.entities.iter().find(|e| e.handle.id == target) else {
            return false;
        };
        if !Self::valid(o, v, e, people) {
            return false;
        }
        if o.select_hits && s.hurt_time > s.expected_hurt_ticks + 1 {
            return false;
        }
        if o.chance < self.rng.double() * 100.0 {
            return false;
        }
        if !claim.owned_by(Claimant::HitFlick)
            && !claim.acquire(Claimant::HitFlick, true, managed.relative)
        {
            return false;
        }
        self.target = Some(target);
        self.attack_queued = true;
        self.flick_angle = self.resolve_angle(o, v);
        self.mode = Mode::FlickingAway;
        self.state_ticks = 0;
        self.cancel_sprint = true;
        self.last_flick_ms = Some(v.now_ms);
        if o.blink && !s.primary_claimed && !self.claim_held {
            self.blinking = true;
            self.claim_held = true;
            self.blink_ticks = 0;
        }
        self.update_controller(o, v, people, claim, managed);
        true
    }

    /// `updateRotationController`: the target's closest point plus the
    /// flick angle while flicking away.
    fn update_controller(
        &mut self,
        o: &Options,
        v: &View<'_>,
        people: &impl People,
        claim: &mut Claim,
        managed: &mut Managed,
    ) -> bool {
        let Some(e) = self
            .entity(v)
            .filter(|e| Self::valid(o, v, e, people))
            .cloned()
        else {
            self.reset(claim, managed);
            return false;
        };
        let (yaw, pitch) = Self::target_rotation(v, &e);
        let offset = if self.mode == Mode::FlickingAway {
            self.flick_angle
        } else {
            0.0
        };
        // setController: keeps a running managed rotation (SilentAura's).
        managed.start((v.player.yaw, v.player.pitch));
        self.controlling = true;
        self.returning = false;
        managed.set_target(yaw + offset, pitch);
        true
    }

    /// `hasReachedFlickRotation`.
    fn reached_flick(&self, v: &View<'_>, e: &EntityView, managed: &Managed) -> bool {
        let (yaw, pitch) = Self::target_rotation(v, e);
        let yaw_error = rotation::wrap(managed.yaw - (yaw + self.flick_angle)).abs();
        let pitch_error = rotation::wrap(managed.pitch - pitch).abs();
        yaw_error.max(pitch_error) <= FLICK_TOLERANCE
    }

    /// `resetFlickState`: everything back to idle, the claim released and
    /// the controller returning to the view. True when held packets must go.
    pub fn reset(&mut self, claim: &mut Claim, managed: &mut Managed) -> bool {
        let flush = self.blinking || self.claim_held;
        self.blinking = false;
        self.blink_ticks = 0;
        self.claim_held = false;
        self.attack_queued = false;
        self.attacked = false;
        self.cancel_sprint = false;
        self.mode = Mode::Idle;
        self.state_ticks = 0;
        self.flick_angle = 0.0;
        self.target = None;
        claim.release(Claimant::HitFlick);
        if self.controlling && managed.active && !self.returning {
            // Proportional axes and linear acceleration for the way back.
            self.returning = true;
            managed.release();
        }
        flush
    }

    /// `onTick` (`EventPreTick`, lowest): the flick's phases.
    pub fn tick(
        &mut self,
        o: &Options,
        v: &View<'_>,
        people: &impl People,
        claim: &mut Claim,
        managed: &mut Managed,
    ) -> Effects {
        let mut effects = Effects::default();
        if !v.no_screen {
            effects.flush = self.reset(claim, managed);
            return effects;
        }
        if self.mode == Mode::Idle {
            return effects;
        }
        let Some(e) = self
            .entity(v)
            .filter(|e| Self::valid(o, v, e, people))
            .cloned()
        else {
            effects.flush = self.reset(claim, managed);
            return effects;
        };
        if !claim.owned_by(Claimant::HitFlick)
            && !claim.acquire(Claimant::HitFlick, true, managed.relative)
        {
            effects.flush = self.reset(claim, managed);
            return effects;
        }
        if self.blinking {
            self.blink_ticks += 1;
            if self.blink_ticks >= BLINK_TICKS {
                effects.flush = true;
                self.flushed();
            }
        }
        self.state_ticks += 1;
        match self.mode {
            Mode::FlickingAway => {
                if self.state_ticks >= 5 || self.reached_flick(v, &e, managed) {
                    self.mode = Mode::Attacking;
                    self.state_ticks = 0;
                }
            }
            Mode::Attacking => {
                if !self.attacked && self.attack_queued {
                    effects.attack = true;
                    self.attacked = true;
                    self.attack_queued = false;
                }
                if self.state_ticks >= 2 {
                    effects.flush |= self.reset(claim, managed);
                }
            }
            Mode::Idle => {}
        }
        if self.mode != Mode::Idle {
            self.update_controller(o, v, people, claim, managed);
        }
        effects
    }

    /// The held packets went (an attack packet, the blink limit or a
    /// reset): the primary claim goes with them (`releaseBlinkClaim`).
    pub fn flushed(&mut self) {
        self.blinking = false;
        self.blink_ticks = 0;
        self.claim_held = false;
    }

    /// `HitFlickAdaptiveRotationController.getSpeed` as counts for one
    /// controller update, from the managed rotation before that update.
    pub fn step(&self, v: &View<'_>, managed: &Managed) -> Step {
        let speed = match (self.mode, self.entity(v)) {
            (Mode::FlickingAway, _) => {
                IDLE_SPEED.max(f64::from(rotation::wrap(self.flick_angle).abs()) / 1.875)
            }
            (Mode::Attacking, Some(e)) => {
                let (yaw, pitch) = Self::target_rotation(v, e);
                let error = rotation::wrap(managed.yaw - yaw)
                    .abs()
                    .max(rotation::wrap(managed.pitch - pitch).abs());
                f64::from(error + 2.0) / 1.875
            }
            (Mode::Attacking, None) => 2.0 / 1.875,
            (Mode::Idle, _) => IDLE_SPEED,
        };
        Step {
            counts: speed * 0.25,
            clamp: true,
            proportional: self.returning,
            linear: self.returning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::targeting::entities::EntityClass;
    use super::super::super::super::targeting::filter::NameLists;
    use super::super::super::super::targeting::filter::tests::entity as base;
    use super::super::rotation::UPDATES_PER_TICK;
    use super::*;

    fn settings() -> Settings {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        settings
            .set(&format!("{ID}.targets.mobs"), json!(true))
            .unwrap();
        settings
    }

    fn zombie(id: i32, at: [f64; 3]) -> EntityView {
        let mut e = base(EntityClass::Monster, "", at);
        e.handle.id = id;
        e
    }

    struct World {
        player: PlayerView,
        entities: Vec<EntityView>,
        held: HeldItem,
    }
    impl World {
        fn new() -> Self {
            Self {
                player: PlayerView::default(),
                entities: vec![zombie(7, [0.0, 0.0, 3.0])],
                held: HeldItem::default(),
            }
        }
        fn view(&self, now_ms: u64) -> View<'_> {
            View {
                now_ms,
                player: &self.player,
                eye_height: 1.62,
                entities: &self.entities,
                no_screen: true,
                held: &self.held,
                strafe: (false, false),
                hurt: &[],
            }
        }
    }

    fn run(
        flick: &mut HitFlick,
        o: &Options,
        v: &View<'_>,
        claim: &mut Claim,
        managed: &mut Managed,
    ) -> Effects {
        let people = NameLists::default();
        let effects = flick.tick(o, v, &people, claim, managed);
        if managed.tick_by((v.player.yaw, v.player.pitch), 0.5, UPDATES_PER_TICK, |m| {
            flick.step(v, m)
        }) {
            flick.yield_controller();
        }
        effects
    }

    #[test]
    fn the_attack_ray_follows_the_managed_rotation_and_hitboxes() {
        let mut world = World::new();
        world.entities.push(zombie(8, [0.0, 0.0, 2.0]));
        let v = world.view(0);
        assert_eq!(ray(&v, (0.0, 0.0), 3.0, 0.0), Some(8));
        assert_eq!(ray(&v, (60.0, 0.0), 3.0, 0.0), None);
        // A box 0.6 wide at z=2: 15 degrees off misses, HitBoxes reaches it.
        assert_eq!(ray(&v, (-15.0, 0.0), 3.0, 0.0), None);
        assert_eq!(ray(&v, (-15.0, 0.0), 3.0, 0.2), Some(8));
    }

    #[test]
    fn defaults_match_the_reference() {
        let o = Options::read(&settings());
        assert_eq!(o.angle, 90.0);
        assert_eq!(o.chance, 100.0);
        assert_eq!(o.flick_delay, 250.0);
        assert!(!o.randomize && o.offset_range == 0.0);
        assert!(o.select_hits && !o.blink && !o.strafe_invert);
        assert_eq!(o.allowed_items, vec!["swords".to_owned()]);
        assert_eq!(o.suffix(false), "90deg");
        assert_eq!(o.suffix(true), "\u{a7}c90deg");
    }

    #[test]
    fn flick_turns_away_then_attacks_on_target_and_returns() {
        let o = Options::read(&settings());
        let w = World::new();
        let v = w.view(1_000);
        let people = NameLists::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        let mut flick = HitFlick::default();
        let start = Start {
            hurt_time: 0,
            expected_hurt_ticks: 0,
            primary_claimed: false,
        };
        assert!(flick.start(&o, &v, 7, start, &people, &mut claim, &mut managed));
        assert!(claim.owned_by(Claimant::HitFlick));
        assert_eq!(flick.mode(), Mode::FlickingAway);
        assert!(flick.take_cancel_sprint() && !flick.take_cancel_sprint());
        // A second start is refused while flicking.
        assert!(!flick.start(&o, &v, 7, start, &people, &mut claim, &mut managed));
        // The first tick turns most of the way toward +90 degrees.
        let e = run(&mut flick, &o, &v, &mut claim, &mut managed);
        assert!(!e.attack);
        assert!(managed.yaw > 45.0, "{}", managed.yaw);
        let mut attacked = 0;
        for _ in 0..12 {
            let e = run(&mut flick, &o, &v, &mut claim, &mut managed);
            if e.attack {
                attacked += 1;
                // The attack goes on the second tick back toward the
                // target, whatever the remaining error.
                assert!(managed.yaw.abs() < 20.0, "{}", managed.yaw);
                assert!(flick.forcing_keys());
            }
        }
        assert_eq!(attacked, 1);
        assert_eq!(flick.mode(), Mode::Idle);
        assert!(!claim.owned_by(Claimant::HitFlick));
        assert!(!managed.active && !flick.controlling());
    }

    #[test]
    fn delay_chance_select_hits_and_items_gate_the_start() {
        let mut s = settings();
        let w = World::new();
        let people = NameLists::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        let o = Options::read(&s);
        let mut flick = HitFlick::default();
        let hurt = Start {
            hurt_time: 5,
            expected_hurt_ticks: 2,
            primary_claimed: false,
        };
        // Select hits: hurt time above the expected ticks + 1.
        assert!(!flick.start(&o, &w.view(0), 7, hurt, &people, &mut claim, &mut managed));
        let ready = Start {
            hurt_time: 3,
            ..hurt
        };
        assert!(flick.start(&o, &w.view(0), 7, ready, &people, &mut claim, &mut managed));
        flick.reset(&mut claim, &mut managed);
        // The 250 ms flick delay.
        assert!(!flick.can_start(&o, &w.view(200)));
        assert!(flick.can_start(&o, &w.view(250)));
        s.set(&format!("{ID}.chance"), json!(0.0)).unwrap();
        let o = Options::read(&s);
        assert!(!flick.start(
            &o,
            &w.view(1_000),
            7,
            ready,
            &people,
            &mut claim,
            &mut managed
        ));
        s.set(&format!("{ID}.chance"), json!(100.0)).unwrap();
        s.set(&format!("{ID}.limit-items"), json!(true)).unwrap();
        let o = Options::read(&s);
        assert!(!flick.can_start(&o, &w.view(2_000)));
        // Too far: more than 5 blocks feet to feet.
        let mut far = World::new();
        far.entities = vec![zombie(7, [0.0, 0.0, 5.5])];
        let o = Options::read(&settings());
        assert!(!flick.start(
            &o,
            &far.view(3_000),
            7,
            ready,
            &people,
            &mut claim,
            &mut managed
        ));
        // No angle configured.
        let mut s = settings();
        s.set(&format!("{ID}.angle"), json!(0.0)).unwrap();
        assert!(!flick.can_start(&Options::read(&s), &w.view(4_000)));
        s.set(&format!("{ID}.angle"), json!(360.0)).unwrap();
        assert!(!flick.can_start(&Options::read(&s), &w.view(4_000)));
    }

    #[test]
    fn strafe_invert_flips_toward_the_push() {
        let mut s = settings();
        s.set(&format!("{ID}.strafe-invert"), json!(true)).unwrap();
        let o = Options::read(&s);
        let w = World::new();
        let mut flick = HitFlick::default();
        let mut v = w.view(0);
        assert_eq!(flick.resolve_angle(&o, &v), 90.0);
        v.strafe = (false, true);
        // Right held with a right (positive) flick: inverted to 270.
        assert_eq!(flick.resolve_angle(&o, &v), 270.0);
        v.strafe = (true, false);
        assert_eq!(flick.resolve_angle(&o, &v), 90.0);
        v.strafe = (true, true);
        assert_eq!(flick.resolve_angle(&o, &v), 90.0);
        s.set(&format!("{ID}.angle"), json!(180.0)).unwrap();
        let o = Options::read(&s);
        v.strafe = (false, true);
        assert_eq!(flick.resolve_angle(&o, &v), 180.0);
    }

    #[test]
    fn randomized_offset_stays_in_range() {
        let mut s = settings();
        s.set(&format!("{ID}.randomize-offset"), json!(true))
            .unwrap();
        s.set(&format!("{ID}.offset-range"), json!(10.0)).unwrap();
        s.set(&format!("{ID}.angle"), json!(0.0)).unwrap();
        let o = Options::read(&s);
        let w = World::new();
        let v = w.view(0);
        // Range alone counts as a configured angle.
        let mut flick = HitFlick::default();
        assert!(flick.can_start(&o, &v));
        for _ in 0..100 {
            let a = rotation::wrap(flick.resolve_angle(&o, &v));
            assert!(a.abs() <= 5.0, "{a}");
        }
    }

    #[test]
    fn blink_holds_until_the_attack_or_seven_ticks() {
        let mut s = settings();
        s.set(&format!("{ID}.blink"), json!(true)).unwrap();
        let o = Options::read(&s);
        let w = World::new();
        let v = w.view(0);
        let people = NameLists::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        let mut flick = HitFlick::default();
        let start = Start::default();
        // Lag's claim keeps Blink off.
        let lag = Start {
            primary_claimed: true,
            ..start
        };
        assert!(flick.start(&o, &v, 7, lag, &people, &mut claim, &mut managed));
        assert!(!flick.blinking());
        flick.reset(&mut claim, &mut managed);
        managed.clear();
        flick.yield_controller();
        let v = w.view(1_000);
        assert!(flick.start(&o, &v, 7, start, &people, &mut claim, &mut managed));
        assert!(flick.blinking() && flick.claim_held());
        // The attack packet flushes (host side), which ends the blink.
        flick.flushed();
        assert!(!flick.claim_held());
        // A held flick that never attacks flushes after seven ticks.
        flick.reset(&mut claim, &mut managed);
        managed.clear();
        flick.yield_controller();
        let v = w.view(2_000);
        assert!(flick.start(&o, &v, 7, start, &people, &mut claim, &mut managed));
        // A target lost mid-flick resets and flushes.
        let mut gone = World::new();
        gone.entities.clear();
        let e = flick.tick(&o, &gone.view(2_050), &people, &mut claim, &mut managed);
        assert!(e.flush && !flick.blinking() && flick.mode() == Mode::Idle);
    }

    #[test]
    fn preempts_silent_aura_and_speeds_follow_the_mode() {
        let o = Options::read(&settings());
        let w = World::new();
        let v = w.view(0);
        let people = NameLists::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        assert!(claim.try_claim(Claimant::SilentAura));
        managed.start((0.0, 0.0));
        let mut flick = HitFlick::default();
        assert_eq!(flick.step(&v, &managed).counts, IDLE_SPEED * 0.25);
        assert!(flick.start(
            &o,
            &v,
            7,
            Start::default(),
            &people,
            &mut claim,
            &mut managed
        ));
        assert!(claim.owned_by(Claimant::HitFlick));
        // A 90 degree flick: max(48, 90 / 1.875) = 48.
        assert_eq!(flick.step(&v, &managed).counts, 48.0 * 0.25);
        let s = flick.step(&v, &managed);
        assert!(s.clamp && !s.proportional && !s.linear);
        flick.reset(&mut claim, &mut managed);
        let s = flick.step(&v, &managed);
        assert!(s.proportional && s.linear && managed.relative);
        assert_eq!(claim.owner(), None);
    }
}
