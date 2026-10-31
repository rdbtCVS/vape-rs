// PearlCatch (reference `PearlCatch`, Category.UTILITY): throws an ender
// pearl, then a wind charge that meets it in the air.
//
// On enable it needs a pearl and a wind charge in the hotbar and searches
// for an intercept plan (the pearl's and the charge's rotations and the
// tick to throw the charge); without one it disables. "Upward" throws the
// pearl almost straight up and the charge after "Charge delay" ticks;
// "Current aim" keeps the charge on the crosshair and searches the pearl's
// rotation and the delay. Each rotation claims the shared rotation
// (priority 7) and turns there before the throw; the module disables after
// the charge, restoring the slot it started on. A stage that is not reached
// within 40 ticks aborts.
use super::super::super::core::settings::{Definition, Settings};
use super::super::combat::rotation::{Claim, Claimant, direction, wrap};
use super::auto_pearl::{Shared, drive};
use super::pearl_rotation::{Controller, Frame, Options as Aim, Target};
use serde_json::Value;

pub const ID: &str = "utility.pearl-catch";
const PROJECTILE_SPEED: f64 = 1.5;
const PEARL_DRAG: f64 = 0.99;
const PEARL_GRAVITY: f64 = 0.03;
const MAX_INTERCEPT_DISTANCE_SQUARED: f64 = 0.36;
const MAX_FLIGHT_TICKS: i32 = 30;
const MAX_AIM_TICKS: i32 = 40;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    vec![
        Definition::choice(
            &key("aim-mode"),
            "Aim mode",
            &["upward", "current-aim"],
            "upward",
        ),
        Definition::number(&key("aim-speed"), "Aim speed", 1.0, 10.0, 10.0, 0.1),
        Definition::boolean(&key("silent-aim"), "Silent aim", true),
        Definition::number(&key("charge-delay"), "Charge delay", 0.0, 0.0, 10.0, 1.0),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub current_aim: bool,
    pub aim_speed: f32,
    pub silent: bool,
    pub charge_delay: i32,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        Self {
            current_aim: get("aim-mode").and_then(Value::as_str) == Some("current-aim"),
            aim_speed: number("aim-speed", 10.0) as f32,
            silent: get("silent-aim").and_then(Value::as_bool).unwrap_or(true),
            charge_delay: number("charge-delay", 0.0) as i32,
        }
    }
    /// `setRotation`'s controller settings.
    fn aim(&self) -> Aim {
        Aim {
            speed: self.aim_speed,
            tolerance: 0.35,
            clamp: true,
            proportional: true,
            linear: true,
            retain: true,
            ..Aim::default()
        }
    }
}

/// The local player as the plan sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    /// Feet.
    pub position: [f64; 3],
    pub eye_height: f64,
    pub motion: [f64; 3],
    pub on_ground: bool,
    pub view: (f32, f32),
}

/// `InterceptPlan`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub pearl: (f32, f32),
    pub charge: (f32, f32),
    pub charge_tick: i32,
    pub miss_squared: f64,
    /// The charge tick counts from the pearl throw, not from the aim.
    pub absolute: bool,
}

/// `evaluatePlan`: both projectiles' flights, the pearl from below the eye
/// now and the charge from the eye `charge_tick` ticks later, and their
/// closest approach.
pub fn evaluate(
    player: &Player,
    pearl: (f32, f32),
    charge: (f32, f32),
    charge_tick: i32,
    minimum_delay: i32,
    absolute: bool,
) -> Option<Plan> {
    let pd = direction(pearl.0, pearl.1);
    let cd = direction(charge.0, charge.1);
    let [mx, _, mz] = player.motion;
    let my = if player.on_ground {
        0.0
    } else {
        player.motion[1]
    };
    let [x, y, z] = player.position;
    let eye = y + player.eye_height;
    let mut p = [x, eye - 0.1, z];
    let mut pm = [
        pd[0] * PROJECTILE_SPEED + mx,
        pd[1] * PROJECTILE_SPEED + my,
        pd[2] * PROJECTILE_SPEED + mz,
    ];
    let t = f64::from(charge_tick);
    let mut c = [x + mx * t, eye + my * t, z + mz * t];
    let cm = [
        cd[0] * PROJECTILE_SPEED + mx,
        cd[1] * PROJECTILE_SPEED + my,
        cd[2] * PROJECTILE_SPEED + mz,
    ];
    let mut closest = f64::MAX;
    for tick in 1..=charge_tick + MAX_FLIGHT_TICKS {
        pm[0] *= PEARL_DRAG;
        pm[1] = (pm[1] - PEARL_GRAVITY) * PEARL_DRAG;
        pm[2] *= PEARL_DRAG;
        for i in 0..3 {
            p[i] += pm[i];
        }
        if tick <= charge_tick {
            continue;
        }
        for i in 0..3 {
            c[i] += cm[i];
        }
        let d = (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2);
        closest = closest.min(d);
    }
    if charge_tick < minimum_delay || closest > MAX_INTERCEPT_DISTANCE_SQUARED {
        return None;
    }
    Some(Plan {
        pearl,
        charge,
        charge_tick,
        miss_squared: closest,
        absolute,
    })
}

/// `findUpwardPlan`: the pearl near straight up along the view (or the
/// movement direction when it differs), the charge straight up after the
/// delay.
pub fn upward_plan(player: &Player, minimum_delay: i32) -> Option<Plan> {
    let reference = player.view.0;
    let [mx, _, mz] = player.motion;
    let mut yaws = vec![reference];
    if mx * mx + mz * mz >= 0.003 {
        let movement = (-mx).atan2(mz).to_degrees() as f32;
        if wrap(movement - reference).abs() >= 20.0 {
            yaws.push(movement);
        }
    }
    let mut best = None;
    let mut best_score = f64::MAX;
    let mut pearl_pitch = -90.0f32;
    while pearl_pitch <= -76.0 {
        for &base in &yaws {
            let mut pearl_offset = -20.0f32;
            while pearl_offset <= 20.0 {
                let pearl_yaw = base + pearl_offset;
                let mut charge_pitch = -90.0f32;
                while charge_pitch <= -86.0 {
                    let mut charge_offset = -20.0f32;
                    while charge_offset <= 20.0 {
                        let charge_yaw = reference + charge_offset;
                        if let Some(plan) = evaluate(
                            player,
                            (pearl_yaw, pearl_pitch),
                            (charge_yaw, charge_pitch),
                            minimum_delay,
                            minimum_delay,
                            false,
                        ) {
                            let score = plan.miss_squared
                                + f64::from(wrap(charge_yaw - reference).abs()) * 0.01
                                + f64::from(wrap(pearl_yaw - reference).abs()) * 0.003
                                + f64::from((charge_pitch + 90.0).abs()) * 0.03;
                            if score < best_score {
                                best = Some(plan);
                                best_score = score;
                            }
                        }
                        charge_offset += 5.0;
                    }
                    charge_pitch += 1.0;
                }
                pearl_offset += 2.5;
            }
        }
        pearl_pitch += 0.5;
    }
    best
}

/// `findCurrentAimPlan`: the charge along the view; the pearl's rotation
/// and the delay are searched.
pub fn current_aim_plan(player: &Player, minimum_delay: i32) -> Option<Plan> {
    let charge = player.view;
    let first = minimum_delay.max(3);
    let mut best = None;
    let mut best_score = f64::MAX;
    for delay in first..=first + 12 {
        let mut pitch = -90.0f32;
        while pitch <= 60.0 {
            let mut offset = -180.0f32;
            while offset < 180.0 {
                let pearl_yaw = charge.0 + offset;
                if let Some(plan) = evaluate(player, (pearl_yaw, pitch), charge, delay, delay, true)
                {
                    let score = plan.miss_squared
                        + f64::from(wrap(pearl_yaw - charge.0).abs()) * 0.0005
                        + f64::from(delay) * 0.0001;
                    if score < best_score {
                        best = Some(plan);
                        best_score = score;
                    }
                }
                offset += 5.0;
            }
            pitch += 2.5;
        }
    }
    best
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    AimingPearl,
    ThrowingPearl,
    AimingCharge,
    WaitingForCharge,
    Finished,
}

/// One tick's inputs.
pub struct Tick {
    pub player: Player,
    pub sensitivity: f64,
    pub screen: bool,
    pub freecam: bool,
}
impl Tick {
    fn frame(&self) -> Frame {
        let p = &self.player;
        Frame {
            view: p.view,
            eye: [p.position[0], p.position[1] + p.eye_height, p.position[2]],
            sensitivity: self.sensitivity,
            screen: self.screen,
            freecam: self.freecam,
        }
    }
}

/// What the host does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    /// The camera after a visible turn.
    pub view: Option<(f32, f32)>,
    /// Select this slot and use it once; with silent aim, at this managed
    /// rotation.
    pub throw: Option<(u8, Option<(f32, f32)>)>,
    pub disable: bool,
}

#[derive(Debug, Default)]
pub struct PearlCatch {
    state: State,
    controller: Option<Controller>,
    /// The controller is the manager's active one.
    active: bool,
    plan: Option<Plan>,
    pearl_slot: Option<u8>,
    charge_slot: Option<u8>,
    saved_slot: Option<u8>,
    state_ticks: i32,
    ticks_since_pearl: i32,
    seed: u64,
}

impl PearlCatch {
    pub fn state(&self) -> State {
        self.state
    }
    pub fn aiming_silently(&self, claim: &Claim) -> bool {
        self.active
            && claim.owned_by(Claimant::PearlCatch)
            && self.controller.as_ref().is_some_and(|c| c.silent)
    }
    /// `onEnable`: false when it cannot start (it then disables at once).
    pub fn enable(
        &mut self,
        o: &Options,
        player: &Player,
        pearl_slot: Option<u8>,
        charge_slot: Option<u8>,
        selected: u8,
        shared: &mut Shared<'_>,
    ) -> bool {
        let (Some(pearl), Some(charge)) = (pearl_slot, charge_slot) else {
            return false;
        };
        let plan = if o.current_aim {
            current_aim_plan(player, o.charge_delay.max(2))
        } else {
            upward_plan(player, o.charge_delay)
        };
        let Some(plan) = plan else {
            return false;
        };
        self.pearl_slot = Some(pearl);
        self.charge_slot = Some(charge);
        self.saved_slot = Some(selected);
        self.plan = Some(plan);
        self.state = State::AimingPearl;
        self.state_ticks = 0;
        self.ticks_since_pearl = 0;
        self.set_rotation(o, plan.pearl, player.view, shared);
        true
    }
    /// `onDisable`: the rotation is handed back and the slot the module
    /// started on is returned for the host to select.
    pub fn disable(&mut self, shared: &mut Shared<'_>) -> Option<u8> {
        self.release_rotation(shared);
        let saved = self.saved_slot.take();
        *self = Self {
            seed: self.seed,
            ..Self::default()
        };
        saved
    }
    /// A world change: the session reset has already cleared the shared
    /// rotation.
    pub fn session_reset(&mut self) {
        *self = Self {
            seed: self.seed,
            ..Self::default()
        };
    }

    fn set_rotation(
        &mut self,
        o: &Options,
        (yaw, pitch): (f32, f32),
        view: (f32, f32),
        shared: &mut Shared<'_>,
    ) {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.controller = Some(Controller::new(
            o.aim(),
            o.silent,
            Target::Rotation(yaw, pitch),
            self.seed,
        ));
        self.active = false;
        if shared.claim.owned_by(Claimant::PearlCatch) {
            shared.set_controller(o.silent, view);
            self.active = true;
        }
    }
    fn release_rotation(&mut self, shared: &mut Shared<'_>) {
        if self.active
            && shared.claim.owned_by(Claimant::PearlCatch)
            && let Some(controller) = self.controller.take()
        {
            shared.release_controller(controller);
        }
        self.active = false;
        shared.claim.release(Claimant::PearlCatch);
    }
    /// `hasRotationControl`.
    fn has_control(&self, o: &Options, shared: &mut Shared<'_>) -> bool {
        shared.claim.owned_by(Claimant::PearlCatch)
            || shared
                .claim
                .acquire(Claimant::PearlCatch, o.silent, shared.managed.relative)
    }
    /// `isRotationReady`.
    fn ready(&mut self, view: (f32, f32), shared: &mut Shared<'_>) -> bool {
        let Some(controller) = self.controller.as_ref() else {
            return false;
        };
        if !shared.claim.owned_by(Claimant::PearlCatch) {
            return false;
        }
        if !self.active {
            shared.set_controller(controller.silent, view);
            self.active = true;
        }
        controller.complete()
    }

    /// The manager's updates for the active turn (its render frames run
    /// before the client tick), then `onTick` on the turned view.
    pub fn tick(&mut self, o: &Options, t: &Tick, shared: &mut Shared<'_>) -> Effects {
        let mut fx = Effects::default();
        if self.active
            && shared.claim.owned_by(Claimant::PearlCatch)
            && let Some(controller) = self.controller.as_mut()
        {
            fx.view = drive(controller, shared.managed, &t.frame());
        }
        let Some(plan) = self.plan else {
            fx.disable = true;
            return fx;
        };
        self.state_ticks += 1;
        if matches!(self.state, State::AimingCharge | State::WaitingForCharge) {
            self.ticks_since_pearl += 1;
        }
        if self.state_ticks > MAX_AIM_TICKS && self.state != State::WaitingForCharge {
            fx.disable = true;
            return fx;
        }
        let view = fx.view.unwrap_or(t.player.view);
        match self.state {
            State::AimingPearl => {
                if self.has_control(o, shared) && self.ready(view, shared) {
                    self.state = State::ThrowingPearl;
                }
            }
            State::ThrowingPearl => {
                fx.throw = self
                    .pearl_slot
                    .map(|slot| (slot, o.silent.then(|| shared.managed.sent()).flatten()));
                self.state = State::AimingCharge;
                self.state_ticks = 0;
                self.ticks_since_pearl = 0;
                self.set_rotation(o, plan.charge, view, shared);
            }
            State::AimingCharge => {
                if self.has_control(o, shared) {
                    if self.ready(view, shared) {
                        self.state = State::WaitingForCharge;
                        self.state_ticks = 0;
                    } else if plan.absolute && self.ticks_since_pearl >= plan.charge_tick {
                        fx.disable = true;
                        return fx;
                    }
                }
            }
            State::WaitingForCharge => {
                let ready = if plan.absolute {
                    self.ticks_since_pearl >= plan.charge_tick
                } else {
                    self.state_ticks >= plan.charge_tick
                };
                if ready {
                    fx.throw = self
                        .charge_slot
                        .map(|slot| (slot, o.silent.then(|| shared.managed.sent()).flatten()));
                    self.state = State::Finished;
                }
            }
            State::Finished => fx.disable = true,
            State::Idle => {}
        }
        fx
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::combat::rotation::Managed;
    use super::super::auto_pearl::drive_return;
    use super::*;

    fn player(view: (f32, f32)) -> Player {
        Player {
            position: [0.0, 64.0, 0.0],
            eye_height: 1.62,
            motion: [0.0, -0.0784, 0.0],
            on_ground: true,
            view,
        }
    }
    fn options(current_aim: bool, silent: bool) -> Options {
        Options {
            current_aim,
            aim_speed: 10.0,
            silent,
            charge_delay: 0,
        }
    }

    #[test]
    fn upward_plan_meets_the_pearl() {
        let p = player((30.0, 0.0));
        let plan = upward_plan(&p, 0).expect("plan");
        assert!(plan.pearl.1 <= -76.0 && plan.charge.1 <= -86.0, "{plan:?}");
        assert!(plan.miss_squared <= MAX_INTERCEPT_DISTANCE_SQUARED);
        assert!(!plan.absolute && plan.charge_tick == 0);
        let again = evaluate(&p, plan.pearl, plan.charge, plan.charge_tick, 0, false).unwrap();
        assert_eq!(again.miss_squared, plan.miss_squared);
        // A level charge never meets an upward pearl, and a plan before
        // the minimum delay is rejected.
        assert!(evaluate(&p, (30.0, -90.0), (30.0, 0.0), 0, 0, false).is_none());
        assert!(evaluate(&p, plan.pearl, plan.charge, 0, 1, false).is_none());
    }

    #[test]
    fn current_aim_plan_keeps_the_charge_on_the_crosshair() {
        let p = player((0.0, -30.0));
        let plan = current_aim_plan(&p, 2).expect("plan");
        assert_eq!(plan.charge, (0.0, -30.0));
        assert!(plan.absolute && plan.charge_tick >= 3 && plan.charge_tick <= 15);
        assert!(plan.miss_squared <= MAX_INTERCEPT_DISTANCE_SQUARED);
    }

    struct World {
        claim: Claim,
        managed: Managed,
        returning: Option<Controller>,
    }
    impl World {
        fn new() -> Self {
            Self {
                claim: Claim::default(),
                managed: Managed::default(),
                returning: None,
            }
        }
        fn with<R>(&mut self, f: impl FnOnce(&mut Shared<'_>) -> R) -> R {
            let mut nothing = |_: &mut Claim, _: &mut Managed| {};
            let mut shared = Shared {
                claim: &mut self.claim,
                managed: &mut self.managed,
                returning: &mut self.returning,
                yield_aura: &mut nothing,
            };
            f(&mut shared)
        }
    }
    fn tick(view: (f32, f32)) -> Tick {
        Tick {
            player: player(view),
            sensitivity: 0.5,
            screen: false,
            freecam: false,
        }
    }

    #[test]
    fn visible_catch_throws_the_pearl_then_the_charge() {
        let o = options(false, false);
        let mut w = World::new();
        let mut c = PearlCatch::default();
        let start = (30.0, 0.0);
        assert!(w.with(|s| c.enable(&o, &player(start), Some(2), Some(3), 0, s)));
        let mut view = start;
        let mut throws = Vec::new();
        for _ in 0..60 {
            let fx = w.with(|s| c.tick(&o, &tick(view), s));
            if let Some(v) = fx.view {
                view = v;
            }
            if let Some((slot, rotation)) = fx.throw {
                assert_eq!(rotation, None);
                throws.push((slot, view));
            }
            if fx.disable {
                break;
            }
        }
        assert_eq!(throws.len(), 2, "{throws:?}");
        assert_eq!(throws[0].0, 2);
        assert!(throws[0].1.1 <= -75.0, "{throws:?}");
        assert_eq!(throws[1].0, 3);
        assert!(throws[1].1.1 <= -85.0, "{throws:?}");
        assert_eq!(c.state(), State::Finished);
        assert_eq!(w.with(|s| c.disable(s)), Some(0));
        assert_eq!(w.claim.owner(), None);
    }

    #[test]
    fn silent_catch_uses_the_managed_rotation_and_returns_it() {
        let o = options(true, true);
        let mut w = World::new();
        let mut c = PearlCatch::default();
        let view = (0.0, -30.0);
        assert!(w.with(|s| c.enable(&o, &player(view), Some(2), Some(3), 5, s)));
        let mut throws = Vec::new();
        for _ in 0..60 {
            let fx = w.with(|s| c.tick(&o, &tick(view), s));
            assert_eq!(fx.view, None);
            if let Some((slot, rotation)) = fx.throw {
                throws.push((slot, rotation.expect("managed")));
            }
            if fx.disable {
                break;
            }
        }
        assert_eq!(throws.len(), 2, "{throws:?}");
        let charge = throws[1].1;
        assert!(
            (charge.0 - 0.0).abs() < 1.0 && (charge.1 + 30.0).abs() < 1.0,
            "{throws:?}"
        );
        assert_eq!(w.with(|s| c.disable(s)), Some(5));
        assert!(w.managed.relative && w.returning.is_some());
        for _ in 0..40 {
            let frame = tick(view).frame();
            drive_return(&mut w.returning, &mut w.managed, &frame);
        }
        assert!(!w.managed.active && w.returning.is_none());
    }

    #[test]
    fn missing_items_or_a_held_claim_abort() {
        let o = options(false, false);
        let mut w = World::new();
        let mut c = PearlCatch::default();
        assert!(!w.with(|s| c.enable(&o, &player((0.0, 0.0)), Some(2), None, 0, s)));
        assert!(w.claim.try_claim(Claimant::AutoTotem));
        assert!(w.with(|s| c.enable(&o, &player((0.0, 0.0)), Some(2), Some(3), 0, s)));
        let mut disabled = false;
        for _ in 0..41 {
            disabled |= w.with(|s| c.tick(&o, &tick((0.0, 0.0)), s)).disable;
        }
        assert!(disabled && c.state() == State::AimingPearl);
        assert!(w.claim.owned_by(Claimant::AutoTotem));
    }
}
