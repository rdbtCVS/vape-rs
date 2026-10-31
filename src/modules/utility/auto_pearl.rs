// AutoPearl (reference `AutoPearl`, Category.UTILITY, with
// `AutoPearlAimLock`, `AutoPearlTrackedPearl`, `AutoPearlState`,
// `AutoPearlRotationController`, `AutoPearlPointRotationController` and
// `ProjectilePitchUtil`): follows an enemy's ender pearl by throwing one at
// its predicted landing spot.
//
// "Aggro" reacts to every enemy pearl that appears; "On bind" looks once,
// when enabled, at the pearls already flying and disables itself when none
// is worth following or after its throw. A throw claims the shared rotation
// (priority 7), turns toward the landing yaw at the pitch that lands a pearl
// there, waits 100 ms once the turn completes, selects the hotbar pearl,
// uses it, and restores the slot the next tick. "Silent aim" turns the
// managed rotation instead of the camera and returns it afterwards.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::combat::rotation::{Claim, Claimant, Managed, wrap};
use super::pearl_rotation::{Controller, Frame, Options as Aim, Target};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub const ID: &str = "utility.auto-pearl";
/// `MAX_PLAYER_DISTANCE`: how far from a pearl its owner may be.
const MAX_OWNER_DISTANCE: f64 = 10.0;
/// A stuck state resets after this long.
const STATE_TIMEOUT_MS: u64 = 5000;
/// Between the completed turn and the throw.
const THROW_DELAY_MS: u64 = 100;
/// The landing prediction gives up below this height.
const VOID_Y: f64 = -128.0;
/// Bounds the prediction loop (a pearl reaches the void well before).
const MAX_STEPS: usize = 4096;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::choice(&key("mode"), "Mode", &["on-bind", "aggro"], "on-bind"),
        Definition::number(&key("aim-speed"), "Aim speed", 0.1, 5.0, 15.0, 0.5),
        Definition::number(&key("angle-limit"), "Angle limit", 30.0, 180.0, 360.0, 5.0),
        Definition::number(&key("min-health"), "Min health", 1.0, 5.0, 20.0, 1.0),
        Definition::number(
            &key("distance-limit"),
            "Distance limit",
            0.0,
            6.0,
            10.0,
            0.1,
        ),
        Definition::boolean(&key("vertical-check"), "Vertical check", false),
        Definition::number(
            &key("pearl-cooldown"),
            "Pearl cooldown",
            0.0,
            1.0,
            15.0,
            0.1,
        ),
        Definition::boolean(&key("silent-aim"), "Silent aim", false),
        Definition::boolean(&key("limit-items"), "Limit to items", true),
        Definition::new(
            &key("allowed-items"),
            "Allowed items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["swords", "ender pearl", "hand"]),
        )
        .when(&key("limit-items"), json!(true)),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    OnBind,
    Aggro,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub aim_speed: f32,
    pub angle_limit: f64,
    pub min_health: f32,
    pub distance_limit: f64,
    pub vertical_check: bool,
    /// Seconds.
    pub cooldown: f64,
    pub silent: bool,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        let boolean = |k: &str, d: bool| get(k).and_then(Value::as_bool).unwrap_or(d);
        Self {
            mode: match get("mode").and_then(Value::as_str) {
                Some("aggro") => Mode::Aggro,
                _ => Mode::OnBind,
            },
            aim_speed: number("aim-speed", 5.0) as f32,
            angle_limit: number("angle-limit", 180.0),
            min_health: number("min-health", 5.0) as f32,
            distance_limit: number("distance-limit", 6.0),
            vertical_check: boolean("vertical-check", false),
            cooldown: number("pearl-cooldown", 1.0),
            silent: boolean("silent-aim", false),
            limit_items: boolean("limit-items", true),
            allowed_items: get("allowed-items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        }
    }
    /// `buildRotationController`'s settings.
    fn aim(&self) -> Aim {
        Aim {
            speed: self.aim_speed * 0.2,
            tolerance: 0.5,
            clamp: true,
            proportional: true,
            angle_based: false,
            linear: true,
            cubic: true,
            randomize: true,
            retain: false,
        }
    }
}

/// One ender pearl in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pearl {
    pub id: u32,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub in_water: bool,
    /// Landed (`isPearlActive` is false once it has hit something).
    pub stuck: bool,
}

/// A player who may own a pearl (the local player included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Person {
    pub id: i32,
    pub position: [f64; 3],
    pub local: bool,
}

/// The block one prediction segment hits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub position: [f64; 3],
    pub water: bool,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `AutoPearlTrackedPearl.predictLandingPosition`: steps the pearl's motion
/// (drag, then gravity) until a segment hits a solid block. `ray` casts one
/// segment through blocks, ignoring liquids.
pub fn predict_landing(
    pearl: &Pearl,
    ray: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<Hit>,
) -> Option<[f64; 3]> {
    let mut position = pearl.position;
    let mut motion = pearl.velocity;
    let drag = if pearl.in_water { 0.8 } else { 0.99 };
    for _ in 0..MAX_STEPS {
        let next = [
            position[0] + motion[0],
            position[1] + motion[1],
            position[2] + motion[2],
        ];
        let hit = ray(position, next);
        position = next;
        if let Some(hit) = hit
            && !hit.water
        {
            return Some(hit.position);
        }
        if position[1] < VOID_Y {
            break;
        }
        motion = [motion[0] * drag, motion[1] * drag - 0.03, motion[2] * drag];
    }
    None
}

/// `ProjectilePitchUtil.calculatePitch`: the low-arc pitch that carries a
/// 1.5-speed, 0.03-gravity projectile from the source to the target; NaN
/// when it cannot reach.
pub fn calculate_pitch(source: [f64; 3], target: [f64; 3]) -> f64 {
    const V2: f64 = 1.5 * 1.5;
    const G: f64 = 0.03;
    let h = ((target[0] - source[0]).powi(2) + (target[2] - source[2]).powi(2)).sqrt();
    let dy = target[1] - source[1];
    let disc = V2 * V2 - G * (G * h * h + 2.0 * dy * V2);
    if disc < 0.0 {
        return f64::NAN;
    }
    -((V2 - disc.sqrt()) / (G * h)).atan().to_degrees()
}

/// `computeThrowPitch`: from the hand-offset throw origin, nudged 5 degrees
/// away from level.
pub fn throw_pitch(
    position: [f64; 3],
    eye_height: f64,
    yaw: f32,
    landing: [f64; 3],
) -> Option<f32> {
    let yaw = f64::from(yaw) / 180.0 * std::f64::consts::PI;
    let source = [
        position[0] - yaw.cos() * 0.16,
        position[1] + eye_height - 0.1,
        position[2] - yaw.sin() * 0.16,
    ];
    let pitch = calculate_pitch(source, landing);
    if pitch.is_nan() {
        return None;
    }
    let adjusted = if pitch < 0.0 {
        pitch - 5.0
    } else if pitch > 0.0 {
        pitch + 5.0
    } else {
        pitch
    };
    Some(adjusted.clamp(-90.0, 90.0) as f32)
}

/// `angleToPoint`: the signed yaw from the crosshair to a point.
pub fn angle_to_point(position: [f64; 3], yaw: f32, x: f64, z: f64) -> f32 {
    let dx = x - position[0];
    let dz = z - position[2];
    let target = if dz < 0.0 && dx < 0.0 {
        90.0 + (dz / dx).atan().to_degrees()
    } else if dz < 0.0 && dx > 0.0 {
        -90.0 + (dz / dx).atan().to_degrees()
    } else {
        (-(dx / dz).atan()).to_degrees()
    };
    wrap(-(yaw - target as f32))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    AcquiringPearl,
    AcquiringAimLock,
    PendingAimJob,
    PendingThrow,
    PendingReset,
}

/// `AutoPearlAimLock`: the pearl followed, its owner, the turn and where
/// the pearl lands.
#[derive(Clone, Debug)]
struct AimLock {
    pearl: u32,
    controller: Controller,
    landing: Option<[f64; 3]>,
    /// Set as the manager's active controller.
    active: bool,
}

/// One tick's view of the player and world.
pub struct Tick<'a> {
    pub now_ms: u64,
    /// Feet.
    pub position: [f64; 3],
    pub eye_height: f64,
    /// The camera's rotation.
    pub view: (f32, f32),
    pub alive: bool,
    pub health: f32,
    pub held: HeldItem,
    /// The held stack is an ender pearl.
    pub held_pearl: bool,
    /// The first hotbar slot holding an ender pearl.
    pub pearl_slot: Option<u8>,
    pub selected: u8,
    pub pearls: &'a [Pearl],
    pub people: &'a [Person],
    pub sensitivity: f64,
    pub screen: bool,
    pub freecam: bool,
    /// The hotbar and use-item claims are available for the throw.
    pub hotbar_free: bool,
}

/// The shared rotation claim and managed rotation, the released pearl aim
/// returning it to the view, and how to make SilentAura stop driving the
/// managed rotation when a pearl takes it.
pub struct Shared<'a> {
    pub claim: &'a mut Claim,
    pub managed: &'a mut Managed,
    pub returning: &'a mut Option<Controller>,
    pub yield_aura: &'a mut dyn FnMut(&mut Claim, &mut Managed),
}
impl Shared<'_> {
    /// `RotationManager.setController` for a pearl module's controller: an
    /// adaptive one continues from the managed rotation (or starts at the
    /// view) after SilentAura stands down; a mouse one replaces it.
    pub fn set_controller(&mut self, silent: bool, view: (f32, f32)) {
        (self.yield_aura)(self.claim, self.managed);
        if silent {
            self.managed.start(view);
        } else {
            self.managed.clear();
        }
        *self.returning = None;
    }
    /// `releaseController` for a pearl module's active controller.
    pub fn release_controller(&mut self, mut controller: Controller) {
        if controller.silent && self.managed.active {
            controller.release();
            self.managed.release();
            *self.returning = Some(controller);
        }
    }
}

/// What the host does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    /// The camera after a visible turn.
    pub view: Option<(f32, f32)>,
    /// Select this hotbar slot.
    pub select: Option<u8>,
    /// Use the held item; with silent aim, at this managed rotation.
    pub throw: Option<Option<(f32, f32)>>,
    pub disable: bool,
}

#[derive(Debug, Default)]
pub struct AutoPearl {
    state: Option<State>,
    state_since: u64,
    throw_ready_at: u64,
    aim: Option<AimLock>,
    pending: Option<u32>,
    /// Pearl id -> owner id (`trackedPearls`).
    tracked: HashMap<u32, Person>,
    handled: HashSet<u32>,
    seen: HashSet<u32>,
    primed: bool,
    saved_slot: Option<u8>,
    cooldown_at: Option<u64>,
    pearl_used_at: Option<u64>,
    seed: u64,
}

impl AutoPearl {
    pub fn state(&self) -> State {
        self.state.unwrap_or(State::AcquiringPearl)
    }
    /// The aim lock drives the managed rotation.
    pub fn aiming_silently(&self, claim: &Claim) -> bool {
        self.aim
            .as_ref()
            .is_some_and(|a| a.active && a.controller.silent && claim.owned_by(Claimant::AutoPearl))
    }
    /// `onPlayerUseItem`: the local player used an ender pearl.
    pub fn pearl_used(&mut self, now_ms: u64) {
        self.pearl_used_at = Some(now_ms);
    }
    /// `onEnable`.
    pub fn enabled(&mut self, options: &Options, shared: &mut Shared<'_>, now_ms: u64) {
        self.state_since = now_ms;
        self.primed = false;
        self.seen.clear();
        if options.mode == Mode::OnBind {
            self.reset(shared, now_ms);
        }
    }
    /// `resetState` (also `onDisable` and `onWorldChange`): a silent aim
    /// that was driving the rotation returns to the view; the claim is
    /// released at once.
    pub fn reset(&mut self, shared: &mut Shared<'_>, now_ms: u64) {
        if let Some(aim) = self.aim.take()
            && aim.active
            && shared.claim.owned_by(Claimant::AutoPearl)
        {
            shared.release_controller(aim.controller);
        }
        shared.claim.release(Claimant::AutoPearl);
        self.pending = None;
        self.state = Some(State::AcquiringPearl);
        self.state_since = now_ms;
    }
    /// A world change: nothing tracked survives, the managed rotation has
    /// already been cleared by the combat session reset.
    pub fn session_reset(&mut self) {
        *self = Self {
            seed: self.seed,
            cooldown_at: self.cooldown_at,
            ..Self::default()
        };
    }

    fn next_seed(&mut self) -> u64 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.seed
    }
    fn holding_allowed(o: &Options, t: &Tick<'_>) -> bool {
        !o.limit_items || t.held_pearl || item_limit::matches(&o.allowed_items, &t.held)
    }
    fn enough_health(o: &Options, t: &Tick<'_>) -> bool {
        t.alive && t.health >= o.min_health
    }
    /// `isOnCooldown`: whole seconds, as the reference truncates both.
    fn on_cooldown(&self, o: &Options, now: u64) -> bool {
        let seconds = o.cooldown.max(0.0) as u64;
        seconds > 0
            && self
                .cooldown_at
                .is_some_and(|at| now.saturating_sub(at) / 1000 < seconds)
    }
    /// `isPearlInRange`.
    fn in_range(o: &Options, t: &Tick<'_>, point: [f64; 3]) -> bool {
        let angle = angle_to_point(t.position, t.view.0, point[0], point[2]);
        if f64::from(angle.abs()) > o.angle_limit / 2.0 {
            return false;
        }
        if distance(t.position, point) <= o.distance_limit {
            return false;
        }
        !o.vertical_check || point[1] - t.position[1] < 7.0
    }
    fn landing_valid(o: &Options, t: &Tick<'_>, landing: Option<[f64; 3]>) -> bool {
        landing.is_some_and(|l| {
            Self::in_range(o, t, l) && throw_pitch(t.position, t.eye_height, t.view.0, l).is_some()
        })
    }
    /// The nearest player within 10 blocks of the pearl, cached per pearl.
    fn owner(&mut self, t: &Tick<'_>, pearl: &Pearl) -> Option<Person> {
        if let Some(owner) = self.tracked.get(&pearl.id) {
            // Follow the owner's latest position while they are still here.
            let owner = t
                .people
                .iter()
                .find(|p| p.id == owner.id && p.local == owner.local)
                .copied()
                .unwrap_or(*owner);
            self.tracked.insert(pearl.id, owner);
            return Some(owner);
        }
        let mut best: Option<(f64, Person)> = None;
        for person in t.people {
            let d = distance(person.position, pearl.position);
            if d <= MAX_OWNER_DISTANCE && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, *person));
            }
        }
        let owner = best.map(|(_, p)| p)?;
        self.tracked.insert(pearl.id, owner);
        Some(owner)
    }
    /// `shouldThrowAt`.
    fn should_throw_at(
        &mut self,
        o: &Options,
        t: &Tick<'_>,
        pearl: &Pearl,
        ray: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<Hit>,
    ) -> bool {
        if t.pearl_slot.is_none()
            || !Self::holding_allowed(o, t)
            || !Self::enough_health(o, t)
            || self.on_cooldown(o, t.now_ms)
            || self.aim.as_ref().is_some_and(|a| !a.controller.complete())
            || pearl.stuck
        {
            return false;
        }
        let Some(owner) = self.owner(t, pearl) else {
            return false;
        };
        if owner.local {
            return false;
        }
        let landing = predict_landing(pearl, ray);
        let Some(point) = landing else {
            return false;
        };
        if distance(t.position, point) <= distance(t.position, owner.position) {
            return false;
        }
        Self::landing_valid(o, t, landing) && !self.handled.contains(&pearl.id)
    }
    /// `beginAimLock`.
    fn begin_aim(
        &mut self,
        o: &Options,
        t: &Tick<'_>,
        pearl: &Pearl,
        ray: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<Hit>,
    ) {
        self.state_since = t.now_ms;
        let landing = predict_landing(pearl, ray);
        let pitch = landing
            .and_then(|l| throw_pitch(t.position, t.eye_height, t.view.0, l))
            .unwrap_or(t.view.1);
        let target = Target::Point(landing.unwrap_or(pearl.position), pitch);
        let seed = self.next_seed();
        self.aim = Some(AimLock {
            pearl: pearl.id,
            controller: Controller::new(o.aim(), o.silent, target, seed),
            landing,
            active: false,
        });
        self.state = Some(State::AcquiringAimLock);
    }
    /// `onEntityJoinWorld` for pearls that appeared since the last tick.
    fn track_new(&mut self, o: &Options, t: &Tick<'_>) {
        let present: HashSet<u32> = t.pearls.iter().map(|p| p.id).collect();
        if self.primed {
            let fresh: Vec<Pearl> = t
                .pearls
                .iter()
                .filter(|p| !self.seen.contains(&p.id))
                .copied()
                .collect();
            for pearl in &fresh {
                let Some(owner) = self.owner(t, pearl) else {
                    continue;
                };
                if !owner.local {
                    self.pending = Some(pearl.id);
                } else if o.cooldown.max(0.0) as u64 > 0
                    && self
                        .pearl_used_at
                        .is_some_and(|at| t.now_ms.saturating_sub(at) <= 1000)
                {
                    self.cooldown_at = Some(t.now_ms);
                }
            }
        }
        self.primed = true;
        self.tracked.retain(|id, _| present.contains(id));
        self.handled.retain(|id| present.contains(id));
        self.seen = present;
    }
    /// `isStateStillValid`.
    fn still_valid(&self, o: &Options, t: &Tick<'_>) -> bool {
        Self::holding_allowed(o, t)
            && Self::enough_health(o, t)
            && self
                .aim
                .as_ref()
                .is_none_or(|a| a.landing.is_none() || Self::landing_valid(o, t, a.landing))
    }

    /// The rotation manager's updates for the active turn (its render
    /// frames run before the client tick), then `onTick` on the turned view.
    pub fn tick(
        &mut self,
        o: &Options,
        t: &Tick<'_>,
        shared: &mut Shared<'_>,
        ray: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<Hit>,
    ) -> Effects {
        let mut fx = Effects {
            view: self.rotate(t, shared),
            ..Effects::default()
        };
        let turned;
        let t = match fx.view {
            Some(view) => {
                turned = Tick {
                    view,
                    held: t.held.clone(),
                    ..*t
                };
                &turned
            }
            None => t,
        };
        self.track_new(o, t);
        if self.state() != State::AcquiringPearl
            && t.now_ms.saturating_sub(self.state_since) > STATE_TIMEOUT_MS
        {
            self.reset(shared, t.now_ms);
        }
        let mut stop = false;
        loop {
            let previous = self.state();
            if !matches!(previous, State::AcquiringPearl | State::PendingReset)
                && !self.still_valid(o, t)
            {
                self.reset(shared, t.now_ms);
                break;
            }
            match previous {
                State::AcquiringPearl => {
                    if self.aim.is_some() {
                        self.state = Some(State::AcquiringAimLock);
                    } else if o.mode == Mode::OnBind {
                        let mut found = None;
                        for pearl in t.pearls.iter().rev() {
                            if self.should_throw_at(o, t, pearl, ray) {
                                found = Some(*pearl);
                                break;
                            }
                        }
                        if let Some(pearl) = found {
                            self.begin_aim(o, t, &pearl, ray);
                        } else {
                            fx.disable = true;
                            stop = true;
                        }
                    } else if let Some(id) = self.pending.take()
                        && let Some(pearl) = t.pearls.iter().find(|p| p.id == id).copied()
                        && self.should_throw_at(o, t, &pearl, ray)
                    {
                        self.begin_aim(o, t, &pearl, ray);
                    }
                }
                State::AcquiringAimLock => {
                    let returning = shared.managed.relative;
                    let claimed = shared.claim.owned_by(Claimant::AutoPearl)
                        || shared
                            .claim
                            .acquire(Claimant::AutoPearl, o.silent, returning);
                    if claimed && let Some(aim) = self.aim.as_mut() {
                        shared.set_controller(aim.controller.silent, t.view);
                        aim.active = true;
                        self.handled.insert(aim.pearl);
                        self.state = Some(State::PendingAimJob);
                    }
                }
                State::PendingAimJob => {
                    if self.aim.as_ref().is_some_and(|a| a.controller.complete()) {
                        self.state = Some(State::PendingThrow);
                        self.throw_ready_at = t.now_ms + THROW_DELAY_MS;
                    }
                }
                State::PendingThrow => {
                    // The hotbar and use claims are held (or it waits).
                    if t.now_ms >= self.throw_ready_at && t.hotbar_free {
                        if let Some(slot) = t.pearl_slot {
                            self.saved_slot = Some(t.selected);
                            fx.select = Some(slot);
                            fx.throw = Some(o.silent.then(|| shared.managed.sent()).flatten());
                            self.cooldown_at = Some(t.now_ms);
                            self.state = Some(State::PendingReset);
                        } else {
                            fx.disable = true;
                        }
                        stop = true;
                    }
                }
                State::PendingReset => {
                    if let Some(slot) = self.saved_slot.take() {
                        fx.select = Some(slot);
                    }
                    self.reset(shared, t.now_ms);
                    if o.mode == Mode::OnBind {
                        fx.disable = true;
                        stop = true;
                    }
                }
            }
            if previous == self.state() || stop {
                break;
            }
        }
        fx
    }

    /// The rotation manager's 50 updates for the active turn.
    fn rotate(&mut self, t: &Tick<'_>, shared: &mut Shared<'_>) -> Option<(f32, f32)> {
        let aim = self.aim.as_mut().filter(|a| a.active)?;
        if !shared.claim.owned_by(Claimant::AutoPearl) {
            return None;
        }
        drive(&mut aim.controller, shared.managed, &t.frame())
    }
}

impl Tick<'_> {
    pub fn frame(&self) -> Frame {
        Frame {
            view: self.view,
            eye: [
                self.position[0],
                self.position[1] + self.eye_height,
                self.position[2],
            ],
            sensitivity: self.sensitivity,
            screen: self.screen,
            freecam: self.freecam,
        }
    }
}

/// One tick of the manager's updates for an active pearl controller: a
/// silent one turns the managed rotation, a visible one returns the new
/// camera rotation while it is alive.
pub fn drive(
    controller: &mut Controller,
    managed: &mut Managed,
    frame: &Frame,
) -> Option<(f32, f32)> {
    if controller.silent {
        if managed.active {
            managed.previous_yaw = managed.yaw;
            managed.previous_pitch = managed.pitch;
            let mut rotation = (managed.yaw, managed.pitch);
            controller.tick(&mut rotation, frame);
            (managed.yaw, managed.pitch) = rotation;
        }
        return None;
    }
    if !controller.alive() {
        return None;
    }
    let mut view = frame.view;
    controller.tick(&mut view, frame);
    Some(view)
}

/// Drives a released silent aim back to the view; another owner starting
/// its own managed rotation (which clears `relative`) ends it.
pub fn drive_return(returning: &mut Option<Controller>, managed: &mut Managed, frame: &Frame) {
    let Some(controller) = returning.as_mut() else {
        return;
    };
    if !managed.active || !managed.relative {
        *returning = None;
        return;
    }
    managed.previous_yaw = managed.yaw;
    managed.previous_pitch = managed.pitch;
    let mut rotation = (managed.yaw, managed.pitch);
    let alive = controller.tick(&mut rotation, frame);
    (managed.yaw, managed.pitch) = rotation;
    if !alive {
        managed.clear();
        *returning = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(mode: Mode, silent: bool) -> Options {
        Options {
            mode,
            aim_speed: 15.0,
            angle_limit: 180.0,
            min_health: 5.0,
            distance_limit: 6.0,
            vertical_check: false,
            cooldown: 1.0,
            silent,
            limit_items: true,
            allowed_items: vec!["swords".into(), "ender pearl".into(), "hand".into()],
        }
    }
    /// A flat floor at y = 64.
    fn floor(a: [f64; 3], b: [f64; 3]) -> Option<Hit> {
        if b[1] > 64.0 {
            return None;
        }
        let t = (a[1] - 64.0) / (a[1] - b[1]);
        Some(Hit {
            position: [a[0] + (b[0] - a[0]) * t, 64.0, a[2] + (b[2] - a[2]) * t],
            water: false,
        })
    }
    fn tick<'a>(now: u64, view: (f32, f32), pearls: &'a [Pearl], people: &'a [Person]) -> Tick<'a> {
        Tick {
            now_ms: now,
            position: [0.0, 64.0, 0.0],
            eye_height: 1.62,
            view,
            alive: true,
            health: 20.0,
            held: HeldItem {
                id: Some("diamond_sword".into()),
                ..HeldItem::default()
            },
            held_pearl: false,
            pearl_slot: Some(4),
            selected: 0,
            pearls,
            people,
            sensitivity: 0.5,
            screen: false,
            freecam: false,
            hotbar_free: true,
        }
    }
    /// An enemy 3 blocks ahead (+z) throws a pearl further ahead.
    fn scene() -> (Vec<Pearl>, Vec<Person>) {
        (
            vec![Pearl {
                id: 77,
                position: [0.0, 66.0, 3.5],
                velocity: [0.0, 0.4, 1.2],
                in_water: false,
                stuck: false,
            }],
            vec![
                Person {
                    id: 1,
                    position: [0.0, 64.0, 0.0],
                    local: true,
                },
                Person {
                    id: 9,
                    position: [0.0, 64.0, 3.0],
                    local: false,
                },
            ],
        )
    }

    #[test]
    fn landing_prediction_and_pitch() {
        let (pearls, _) = scene();
        let landing = predict_landing(&pearls[0], &mut floor).unwrap();
        assert!(
            (landing[1] - 64.0).abs() < 1e-9 && landing[2] > 20.0,
            "{landing:?}"
        );
        let pitch = throw_pitch([0.0, 64.0, 0.0], 1.62, 0.0, landing).unwrap();
        assert!(pitch < -5.0 && pitch > -45.0, "{pitch}");
        // Out of reach: no pitch.
        assert!(calculate_pitch([0.0, 0.0, 0.0], [500.0, 0.0, 0.0]).is_nan());
        // Straight ahead (+z) is 0 degrees, to the left (+x) is negative.
        assert!(angle_to_point([0.0; 3], 0.0, 0.0, 10.0).abs() < 1e-4);
        assert!((angle_to_point([0.0; 3], 0.0, 10.0, 0.0) + 90.0).abs() < 1e-3);
        assert!((angle_to_point([0.0; 3], 90.0, 0.0, 10.0) + 90.0).abs() < 1e-3);
    }

    fn run(
        pearl: &mut AutoPearl,
        o: &Options,
        claim: &mut Claim,
        managed: &mut Managed,
        t: &Tick<'_>,
    ) -> Effects {
        thread_local!(static RETURNING: std::cell::RefCell<Option<Controller>> = const { std::cell::RefCell::new(None) });
        RETURNING.with_borrow_mut(|returning| {
            let mut nothing = |_: &mut Claim, _: &mut Managed| {};
            let mut shared = Shared {
                claim,
                managed,
                returning,
                yield_aura: &mut nothing,
            };
            let fx = pearl.tick(o, t, &mut shared, &mut floor);
            if !pearl.aiming_silently(shared.claim) {
                drive_return(shared.returning, shared.managed, &t.frame());
            }
            fx
        })
    }

    #[test]
    fn aggro_follows_an_enemy_pearl_and_restores_the_slot() {
        let o = options(Mode::Aggro, false);
        let (pearls, people) = scene();
        let mut p = AutoPearl::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        // Primed before the pearl appears.
        run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(0, (0.0, 0.0), &[], &people),
        );
        let mut view = (0.0, 0.0);
        let mut threw = None;
        for i in 1..80 {
            let fx = run(
                &mut p,
                &o,
                &mut claim,
                &mut managed,
                &tick(i * 50, view, &pearls, &people),
            );
            if let Some(v) = fx.view {
                view = v;
            }
            if fx.throw.is_some() {
                threw = Some((i, fx));
                break;
            }
        }
        let (i, fx) = threw.expect("threw");
        assert_eq!(fx.select, Some(4));
        assert_eq!(fx.throw, Some(None));
        assert!(view.1 < -5.0, "{view:?}");
        assert!(view.0.abs() < 2.0, "{view:?}");
        let fx = run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(i * 50 + 50, view, &pearls, &people),
        );
        assert_eq!(fx.select, Some(0));
        assert!(!fx.disable);
        assert_eq!(claim.owner(), None);
        // The same pearl is not followed twice.
        let fx = run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(i * 50 + 100, view, &pearls, &people),
        );
        assert_eq!(fx, Effects::default());
    }

    #[test]
    fn own_pearls_and_cooldown_are_ignored() {
        let o = options(Mode::Aggro, false);
        let (mut pearls, mut people) = scene();
        people[1].position = [50.0, 64.0, 50.0];
        pearls[0].position = [0.0, 66.0, 0.5];
        let mut p = AutoPearl::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(0, (0.0, 0.0), &[], &people),
        );
        p.pearl_used(10);
        run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(50, (0.0, 0.0), &pearls, &people),
        );
        assert_eq!(p.state(), State::AcquiringPearl);
        assert!(p.on_cooldown(&o, 500));
        assert!(!p.on_cooldown(&o, 1100));
    }

    #[test]
    fn on_bind_disables_without_a_pearl_and_limits_items() {
        let o = options(Mode::OnBind, false);
        let (pearls, people) = scene();
        let mut p = AutoPearl::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        let fx = run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(0, (0.0, 0.0), &[], &people),
        );
        assert!(fx.disable);
        let mut t = tick(0, (0.0, 0.0), &pearls, &people);
        t.held.id = Some("stone".into());
        let fx = run(&mut p, &o, &mut claim, &mut managed, &t);
        assert!(fx.disable);
        let fx = run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(0, (0.0, 0.0), &pearls, &people),
        );
        assert!(!fx.disable);
        assert_eq!(p.state(), State::PendingAimJob);
        assert!(claim.owned_by(Claimant::AutoPearl));
    }

    #[test]
    fn silent_aim_turns_the_managed_rotation_and_returns_it() {
        let o = options(Mode::OnBind, true);
        let (pearls, people) = scene();
        let mut p = AutoPearl::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        let mut threw = None;
        for i in 0..80 {
            let fx = run(
                &mut p,
                &o,
                &mut claim,
                &mut managed,
                &tick(i * 50, (0.0, 0.0), &pearls, &people),
            );
            assert_eq!(fx.view, None);
            if let Some(t) = fx.throw {
                threw = Some((i, t));
                break;
            }
        }
        let (i, rotation) = threw.expect("threw");
        let (_, pitch) = rotation.expect("silent rotation");
        assert!(pitch < -5.0);
        assert!(p.aiming_silently(&claim));
        let fx = run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(i * 50 + 50, (0.0, 0.0), &pearls, &people),
        );
        assert!(fx.disable);
        assert!(managed.relative && claim.owner().is_none());
        for j in 0..60 {
            run(
                &mut p,
                &o,
                &mut claim,
                &mut managed,
                &tick(i * 50 + 100 + j * 50, (0.0, 0.0), &[], &people),
            );
        }
        assert!(!managed.active && !p.aiming_silently(&claim));
    }

    #[test]
    fn a_stuck_state_times_out() {
        let o = options(Mode::Aggro, true);
        let (pearls, people) = scene();
        let mut p = AutoPearl::default();
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        // A higher-priority owner keeps the claim.
        assert!(claim.try_claim(Claimant::AutoTotem));
        run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(0, (0.0, 0.0), &[], &people),
        );
        run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(50, (0.0, 0.0), &pearls, &people),
        );
        assert_eq!(p.state(), State::AcquiringAimLock);
        run(
            &mut p,
            &o,
            &mut claim,
            &mut managed,
            &tick(5200, (0.0, 0.0), &pearls, &people),
        );
        assert_eq!(p.state(), State::AcquiringPearl);
        assert!(claim.owned_by(Claimant::AutoTotem));
    }
}
