// Runtime state and entry points for the movement/world modules in
// `modules::movement` (Scaffold and its bridge modes, InvWalk, WindCharge),
// the NoFall jump-key detail and the shared module control claims. Kept
// apart from core/mod.rs so the shared runtime only carries one field and
// one toggle call for them.
use super::super::actions::{Lease, Owner, Resource};
use super::super::modules::blatant::no_fall;
use super::super::modules::movement::god_bridge::GodBridge;
use super::super::modules::movement::movement_task::{Body, Keys, Tasks, World};
use super::super::modules::movement::scaffold_bridge::{
    self as bridge, Ctx, EdgeTick, Hit, Inventory, Locks, Rotor,
};
use super::super::modules::movement::telly_bridge::TellyBridge;
use super::super::modules::movement::{inv_walk, scaffold, step, wind_charge};
use super::super::modules::rng::Rng;
use super::movement_input::{self, Axis, Request};
use super::rotation_runtime::Shared;
use super::{ModuleId, Runtime};

const WIND_CHARGE_OWNER: Owner = Owner("utility.wind-charge");

#[derive(Debug, Default)]
pub struct State {
    legit: scaffold::Legit,
    wind_charge: wind_charge::WindCharge,
    wind_lease: Option<Lease>,
    /// A slot to restore once WindCharge was turned off mid-throw.
    wind_restore: Option<usize>,
    /// NoFall released the jump key; it stays released until let go.
    jump_released: bool,
    step: step::Step,
    /// Step's 0.45 timer runs until this time.
    step_timer_until: Option<u64>,
    bridge: Bridge,
    /// `SharedModuleControlClaims.movementInput` and `.rightClickUse`: one
    /// lock each, shared by every module that sets them.
    pub(super) claims: Locks,
}

/// Scaffold's running mode and the pieces the bridge modes drive.
#[derive(Debug)]
struct Bridge {
    /// The mode whose sub-module is enabled.
    running: Option<scaffold::Mode>,
    god: GodBridge,
    telly: TellyBridge,
    rotor: Rotor,
    keys: Keys,
    tasks: Tasks,
    rng: Rng,
    /// This tick's edge sneak, for the movement writer's second pass.
    sneak: Option<scaffold::Sneak>,
}

impl Default for Bridge {
    fn default() -> Self {
        Self {
            running: None,
            god: GodBridge::default(),
            telly: TellyBridge::default(),
            rotor: Rotor::default(),
            keys: Keys::default(),
            tasks: Tasks::default(),
            rng: Rng::new(0x5caf_f01d),
            sneak: None,
        }
    }
}

/// One pre-tick of host state for Scaffold's bridge modes.
#[derive(Clone, Debug)]
pub struct BridgeTick {
    pub no_screen: bool,
    pub body: Body,
    pub inventory: Inventory,
    pub hit: Option<Hit>,
    pub right_mouse: bool,
    /// This tick's mouse movement in counts.
    pub mouse: [i32; 2],
    pub view: [f32; 2],
    /// The user's own state of each movement axis (`Axis` order).
    pub user: [bool; 7],
}

/// What a bridge mode does in the host this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BridgeActions {
    pub select: Option<usize>,
    /// One right click (`use.onTick(1)`).
    pub click: bool,
}

impl Runtime {
    /// Called by `set_enabled` after the module's flag changed.
    pub(super) fn movement_toggled(&mut self, module: ModuleId, enabled: bool) {
        match module {
            ModuleId::Scaffold if enabled => {
                let options = scaffold::Options::read(&self.settings);
                self.scaffold_mode(Some(options.mode), &options);
            }
            ModuleId::Scaffold => {
                let options = scaffold::Options::read(&self.settings);
                self.scaffold_mode(None, &options);
                // `Scaffold.onDisable`.
                self.combat.claim.release(bridge::OWNER);
                self.world_movement.bridge.tasks.clear();
            }
            ModuleId::WindCharge if !enabled => {
                let mut rotation = Shared::of(&mut self.combat, &mut self.rotations);
                let state = &mut self.world_movement;
                state.wind_charge.release(&mut rotation);
                state.wind_restore = state.wind_charge.restore_slot();
                state.wind_charge.reset();
                if let Some(lease) = state.wind_lease.take() {
                    self.actions.release(lease);
                }
            }
            ModuleId::NoFall if !enabled => self.world_movement.jump_released = false,
            ModuleId::Step => self.world_movement.step.reset(),
            _ => {}
        }
    }

    /// The sub-module swap: the running mode's `onDisable`, then the new
    /// one's `onEnable`. Leaving a bridge mode also clears the locks (Telly's
    /// `onDisable` leaves them set).
    fn scaffold_mode(&mut self, mode: Option<scaffold::Mode>, options: &scaffold::Options) {
        let state = &mut self.world_movement;
        let b = &mut state.bridge;
        if b.running == mode {
            return;
        }
        let mut rotation = Shared::of(&mut self.combat, &mut self.rotations);
        match b.running.take() {
            Some(scaffold::Mode::Legit) => state.legit.reset(),
            Some(old) => {
                if old == scaffold::Mode::GodBridge {
                    b.god.disable();
                } else {
                    b.telly.disable();
                }
                bridge::release_controls(&mut b.rotor, &mut rotation, &mut b.tasks, &mut b.keys);
                state.claims = Locks::default();
            }
            None => {}
        }
        b.sneak = None;
        match mode {
            Some(scaffold::Mode::Legit) => state.legit.enable(options),
            Some(scaffold::Mode::GodBridge) => b.god.enable(),
            Some(scaffold::Mode::TellyBridge) => b.telly.enable(),
            None => {}
        }
        b.running = mode;
    }

    /// The options of the running Scaffold mode, after a mode change.
    fn scaffold_options(&mut self) -> Option<scaffold::Options> {
        if !self.enabled(ModuleId::Scaffold) || !self.session.in_world() {
            return None;
        }
        let options = scaffold::Options::read(&self.settings);
        self.scaffold_mode(Some(options.mode), &options);
        Some(options)
    }

    /// Scaffold Legit's sneak for this pre-entity-update.
    pub fn scaffold_sneak(&mut self, tick: &scaffold::Tick) -> Option<scaffold::Sneak> {
        let options = self.scaffold_options()?;
        if options.mode != scaffold::Mode::Legit {
            return None;
        }
        self.world_movement.legit.tick(&options, tick)
    }

    /// The movement writer's Scaffold requests: Legit's sneak, or for a
    /// bridge mode the task's completion check (`onPreLocalPlayerTick`), the
    /// edge sneak (`onPreEntityUpdate`) and the key bindings. `again` (the
    /// writer's second pass this tick) reuses this tick's decisions.
    pub fn scaffold_requests(
        &mut self,
        tick: &scaffold::Tick,
        body: &Body,
        again: bool,
    ) -> Vec<Request> {
        let sneak_request = |sneak: scaffold::Sneak, priority| Request {
            axis: Axis::Sneak,
            owner: movement_input::Owner::Scaffold,
            priority,
            pressed: sneak == scaffold::Sneak::Press,
        };
        let Some(options) = self.scaffold_options() else {
            return Vec::new();
        };
        if options.mode == scaffold::Mode::Legit {
            if again {
                return Vec::new();
            }
            return self
                .world_movement
                .legit
                .tick(&options, tick)
                .map(|s| sneak_request(s, 3))
                .into_iter()
                .collect();
        }
        let b = &mut self.world_movement.bridge;
        if !again {
            b.tasks.update(body, tick.no_screen, &mut b.keys);
            let edge = EdgeTick {
                now_ms: tick.now_ms,
                no_screen: tick.no_screen,
                can_activate: true,
                on_ladder: tick.on_ladder,
                pitch_blocked: options.pitch_check && f64::from(tick.pitch) < options.pitch,
                forward: tick.forward,
                on_ground: tick.on_ground,
                sneaking: tick.sneaking,
                edge: tick.edge,
            };
            let mut rotation = Shared::of(&mut self.combat, &mut self.rotations);
            b.sneak = if options.mode == scaffold::Mode::GodBridge {
                b.god.pre_entity(&edge, &mut rotation)
            } else {
                b.telly.pre_entity(&edge, &mut rotation)
            };
        }
        let mut out = b.keys.requests();
        out.extend(b.sneak.map(|s| sneak_request(s, 4)));
        out
    }

    /// A bridge mode's pre-tick, then the movement task's keys.
    pub fn scaffold_bridge_tick(
        &mut self,
        tick: BridgeTick,
        world: &mut dyn World,
    ) -> BridgeActions {
        let Some(options) = self.scaffold_options() else {
            return BridgeActions::default();
        };
        if options.mode == scaffold::Mode::Legit {
            return BridgeActions::default();
        }
        let now_ms = self.now_ms();
        let mut rotation = Shared::of(&mut self.combat, &mut self.rotations);
        let state = &mut self.world_movement;
        let b = &mut state.bridge;
        b.keys.observe(tick.user);
        let body = tick.body;
        let mut ctx = Ctx {
            now_ms,
            no_screen: tick.no_screen,
            body,
            inventory: tick.inventory,
            hit: tick.hit,
            right_mouse: tick.right_mouse,
            mouse: tick.mouse,
            view: tick.view,
            options: &options,
            world,
            keys: &mut b.keys,
            tasks: &mut b.tasks,
            rotation: &mut rotation,
            rotor: &mut b.rotor,
            locks: &mut state.claims,
            rng: &mut b.rng,
            select: None,
            click: false,
        };
        if options.mode == scaffold::Mode::GodBridge {
            b.god.tick(&mut ctx);
        } else {
            b.telly.tick(&mut ctx);
        }
        let actions = BridgeActions {
            select: ctx.select,
            click: ctx.click,
        };
        b.tasks.apply(&body, &mut b.keys, world);
        actions
    }

    /// Session cleanup: the manager forgot the bridge's controller, so the
    /// mode, its task and its bindings start over (the next tick enables the
    /// mode again).
    pub(super) fn scaffold_session_reset(&mut self) {
        let state = &mut self.world_movement;
        state.legit.reset();
        let rng = std::mem::replace(&mut state.bridge.rng, Rng::new(0));
        state.bridge = Bridge {
            rng,
            ..Bridge::default()
        };
        state.claims = Locks::default();
    }

    /// `SharedModuleControlClaims.movementInput.isLocked()`.
    pub fn movement_input_locked(&self) -> bool {
        self.world_movement.claims.movement
    }

    /// `SharedModuleControlClaims.rightClickUse.isClaimed()`.
    pub fn right_click_use_claimed(&self) -> bool {
        self.world_movement.claims.right_click
    }

    /// InvWalk's options when it hands the physical keys over behind `screen`.
    pub fn inv_walk(&self, screen: inv_walk::Screen) -> Option<inv_walk::Options> {
        if !self.enabled(ModuleId::InvWalk) || !self.session.in_world() {
            return None;
        }
        let options = inv_walk::Options::read(&self.settings);
        inv_walk::active(&options, screen).then_some(options)
    }

    /// WindCharge's pre-tick; a pending slot restore after a manual disable
    /// comes back as a plain `select`.
    pub fn wind_charge_tick(&mut self, tick: &wind_charge::Tick) -> wind_charge::Actions {
        if let Some(slot) = self.world_movement.wind_restore.take() {
            return wind_charge::Actions {
                select: Some(slot),
                ..Default::default()
            };
        }
        if !self.enabled(ModuleId::WindCharge) || !self.session.in_world() {
            return Default::default();
        }
        let epoch = self.session.epoch();
        let lease = match self.world_movement.wind_lease {
            Some(lease) if self.actions.owns(lease, epoch) => Some(lease),
            _ => self
                .actions
                .acquire(Resource::Hotbar, WIND_CHARGE_OWNER, epoch, 90),
        };
        let Some(lease) = lease else {
            // Another module holds the hotbar this tick.
            return Default::default();
        };
        self.world_movement.wind_lease = Some(lease);
        let options = wind_charge::Options::read(&self.settings);
        let mut rotation = Shared::of(&mut self.combat, &mut self.rotations);
        let actions = self
            .world_movement
            .wind_charge
            .tick(&options, tick, &mut rotation);
        if let Some(reason) = actions.refused {
            self.notify(format!("WindCharge: {reason}"));
        }
        if actions.disable {
            // The final stage already restored the slot.
            self.world_movement.wind_charge.reset();
            self.set_enabled(ModuleId::WindCharge, false);
            self.world_movement.wind_restore = None;
        }
        actions
    }

    /// NoFall Normal's claim this motion packet, with the jump key held: the
    /// reference releases the jump binding, which stays up until re-pressed.
    pub fn no_fall_claimed(&mut self, jump_held: bool) {
        if jump_held
            && self.enabled(ModuleId::NoFall)
            && no_fall::Mode::read(&self.settings) == no_fall::Mode::Normal
        {
            self.world_movement.jump_released = true;
        }
    }

    /// Whether NoFall keeps jump released this tick (`user_jump`: the
    /// user's own jump state).
    pub fn no_fall_jump_released(&mut self, user_jump: bool) -> bool {
        if !user_jump || !self.enabled(ModuleId::NoFall) {
            self.world_movement.jump_released = false;
        }
        self.world_movement.jump_released
    }

    /// Step at the position send: after a recognised step, Speed's stage and
    /// the timer; returns the reference's intermediate positions, which the
    /// host does not send (one position per client tick on 26.3).
    pub fn step_motion(&mut self, sample: step::Sample) -> Vec<[f64; 3]> {
        if !self.enabled(ModuleId::Step) || !self.session.in_world() {
            self.world_movement.step.reset();
            return Vec::new();
        }
        let Some(stepped) = self.world_movement.step.observe(sample) else {
            return Vec::new();
        };
        self.speed.set_stage(-4);
        self.world_movement.step_timer_until = Some(self.now_ms() + step::TIMER_MS);
        stepped.packets
    }

    /// Step's timer override, while it runs.
    pub(super) fn step_timer(&self) -> Option<f32> {
        let until = self.world_movement.step_timer_until?;
        (self.now_ms() < until && self.session.in_world()).then_some(step::TIMER)
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::modules::combat::sprint;
    use super::super::super::modules::movement::movement_task::tests::Flat;
    use super::super::super::modules::movement::scaffold_bridge::tests::stone;
    use super::super::lifecycle::Context;
    use super::*;
    use serde_json::json;

    fn in_world(mode: &str) -> Runtime {
        let mut r = Runtime::default();
        r.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        r.set_setting(&format!("{}.mode", scaffold::ID), json!(mode))
            .unwrap();
        r.set_enabled(ModuleId::Scaffold, true);
        r
    }

    /// Back held at the runway's end, over its edge, facing +Z.
    fn edge() -> scaffold::Tick {
        scaffold::Tick {
            now_ms: 0,
            no_screen: true,
            sneak_held: false,
            sneaking: false,
            pitch: 80.0,
            forward: -1.0,
            on_ground: true,
            on_ladder: false,
            edge: true,
            held: Default::default(),
        }
    }

    fn body() -> Body {
        let pos = [0.5, 100.0, 0.3];
        Body {
            pos,
            prev: pos,
            on_ground: true,
            eye_height: 1.62,
            ..Default::default()
        }
    }

    fn bridge_tick(hit: Option<Hit>) -> BridgeTick {
        let mut inventory = Inventory {
            slots: vec![None; 36],
            ..Default::default()
        };
        inventory.slots[0] = stone(64);
        let mut user = [false; 7];
        user[Axis::Back as usize] = true;
        BridgeTick {
            no_screen: true,
            body: body(),
            inventory,
            hit,
            right_mouse: false,
            mouse: [0, 0],
            view: [0.0, 80.0],
            user,
        }
    }

    /// Sprint's tick as the movement writer builds it.
    fn walking(r: &Runtime) -> sprint::Tick {
        sprint::Tick {
            playing: true,
            movement_locked: r.movement_input_locked(),
            ..Default::default()
        }
    }

    /// Two hand-placed blocks behind the runway activate GodBridge; the
    /// tick after, its locks hold the movement input and the right click.
    fn activate(r: &mut Runtime, world: &mut Flat) {
        for z in 0..4 {
            world.solid.insert([0, 99, z]);
        }
        let requests = r.scaffold_requests(&edge(), &body(), false);
        assert!(
            requests.iter().any(|q| q.axis == Axis::Sneak
                && q.owner == movement_input::Owner::Scaffold
                && q.priority == 4
                && q.pressed),
            "the edge sneak until the bridge activates: {requests:?}"
        );
        assert!(r.combat.claim.owned_by(bridge::OWNER));
        for placed in [None, Some(-1), Some(-2), None] {
            if let Some(z) = placed {
                world.solid.insert([0, 99, z]);
            }
            assert!(!r.scaffold_bridge_tick(bridge_tick(None), world).click);
            r.scaffold_requests(&edge(), &body(), false);
        }
        assert!(!r.movement_input_locked());
        let hit = Some(Hit {
            pos: [0, 99, -2],
            side: 2,
        });
        r.scaffold_bridge_tick(bridge_tick(hit), world);
        assert!(r.movement_input_locked() && r.right_click_use_claimed());
        assert!(r.rotation().active(bridge::OWNER));
        r.scaffold_requests(&edge(), &body(), false);
    }

    #[test]
    fn god_bridge_locks_sprint_and_the_use_input_until_the_mode_changes() {
        let mut r = in_world("GodBridge");
        let mut world = Flat {
            feet: 100.0,
            ..Default::default()
        };
        r.set_enabled(ModuleId::Sprint, true);
        activate(&mut r, &mut world);
        // Sprint.java: no new press while the movement input is locked.
        assert!(!r.sprint_press(&walking(&r), false));
        assert!(r.use_claimed());
        // The task walking onto the line presses its keys as bindings (the
        // client tick's head, then the movement writer).
        r.scaffold_bridge_tick(bridge_tick(None), &mut world);
        let requests = r.scaffold_requests(&edge(), &body(), false);
        assert!(
            requests
                .iter()
                .any(|q| q.owner == movement_input::Owner::KeyHelper && q.priority == 3),
            "{requests:?}"
        );
        // The second writer pass this tick repeats the bindings.
        assert_eq!(r.scaffold_requests(&edge(), &body(), true), requests);
        // Switching to Legit swaps the sub-module: the locks and the
        // rotation controller go, and Sprint presses again. The claim stays
        // until `Scaffold.onDisable`.
        r.set_setting(&format!("{}.mode", scaffold::ID), json!("Legit"))
            .unwrap();
        assert_eq!(
            r.scaffold_bridge_tick(bridge_tick(None), &mut world),
            BridgeActions::default()
        );
        assert!(!r.movement_input_locked() && !r.right_click_use_claimed());
        assert!(!r.use_claimed());
        assert!(!r.rotation().active(bridge::OWNER));
        assert!(r.combat.claim.owned_by(bridge::OWNER));
        assert!(r.sprint_press(&walking(&r), false));
    }

    #[test]
    fn disabling_scaffold_or_leaving_the_world_clears_the_locks() {
        let mut world = Flat {
            feet: 100.0,
            ..Default::default()
        };
        let mut r = in_world("GodBridge");
        activate(&mut r, &mut world);
        r.set_enabled(ModuleId::Scaffold, false);
        assert!(!r.movement_input_locked() && !r.right_click_use_claimed());
        assert!(!r.combat.claim.owned_by(bridge::OWNER));
        assert!(r.scaffold_requests(&edge(), &body(), false).is_empty());

        let mut world = Flat {
            feet: 100.0,
            ..Default::default()
        };
        let mut r = in_world("GodBridge");
        activate(&mut r, &mut world);
        r.observe_context(Context::default());
        assert!(!r.movement_input_locked() && !r.right_click_use_claimed());
        assert_eq!(
            r.scaffold_bridge_tick(bridge_tick(None), &mut world),
            BridgeActions::default(),
            "nothing runs outside a world"
        );
    }

    #[test]
    fn telly_bridge_runs_its_own_sub_module() {
        let mut world = Flat {
            feet: 100.0,
            ..Default::default()
        };
        let mut r = in_world("TellyBridge");
        r.scaffold_requests(&edge(), &body(), false);
        r.scaffold_bridge_tick(bridge_tick(None), &mut world);
        assert_eq!(
            r.world_movement.bridge.running,
            Some(scaffold::Mode::TellyBridge)
        );
        r.set_setting(&format!("{}.mode", scaffold::ID), json!("GodBridge"))
            .unwrap();
        r.scaffold_bridge_tick(bridge_tick(None), &mut world);
        assert_eq!(
            r.world_movement.bridge.running,
            Some(scaffold::Mode::GodBridge)
        );
        assert!(!r.movement_input_locked());
    }
}
