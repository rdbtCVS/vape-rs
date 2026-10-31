// Runtime side of AutoPearl and PearlCatch: their state, the hotbar and
// use claims a throw holds, and the host's per-tick entry.
//
// Both claim the shared rotation (priority 7) and set their controllers on
// the shared driver (`rotation_runtime`), which the host ticks before this
// one, so SilentAura stands down while a pearl aims and a released silent
// aim returns to the view on that driver.
use super::super::actions::{Lease, Owner, Resource};
use super::super::inventory::item_limit::HeldItem;
use super::super::modules::utility::{
    auto_pearl::{self, AutoPearl, Hit, Pearl, Person, State},
    pearl_catch::{self, PearlCatch, Player},
};
use super::rotation_runtime::Shared;
use super::settings::Definition;
use super::{ModuleId, Runtime};
use std::time::Instant;

const OWNERS: [Owner; 2] = [Owner(auto_pearl::ID), Owner(pearl_catch::ID)];
const MODULES: [ModuleId; 2] = [ModuleId::AutoPearl, ModuleId::PearlCatch];
/// The same as the hotbar use sequences: above inventory planners, below
/// macros.
const PRIORITY: u16 = 150;

#[derive(Debug)]
pub struct Pearls {
    auto_pearl: AutoPearl,
    pearl_catch: PearlCatch,
    leases: [Vec<Lease>; 2],
    /// `onEnable` ran for the module's current enable.
    started: [bool; 2],
    /// PearlCatch's starting slot, selected on the next tick.
    restore: Option<u8>,
    origin: Instant,
}
impl Default for Pearls {
    fn default() -> Self {
        Self {
            auto_pearl: AutoPearl::default(),
            pearl_catch: PearlCatch::default(),
            leases: [Vec::new(), Vec::new()],
            started: [false; 2],
            restore: None,
            origin: Instant::now(),
        }
    }
}
impl Pearls {
    fn now(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.origin).as_millis() as u64
    }
}

pub fn definitions() -> Vec<Definition> {
    auto_pearl::definitions()
        .into_iter()
        .chain(pearl_catch::definitions())
        .collect()
}

/// One tick of the host's player and world.
pub struct PearlInput<'a> {
    pub at: Instant,
    /// Feet.
    pub position: [f64; 3],
    pub eye_height: f64,
    pub motion: [f64; 3],
    pub on_ground: bool,
    /// The camera's rotation, after the shared rotation's updates.
    pub view: (f32, f32),
    pub alive: bool,
    pub health: f32,
    pub held: HeldItem,
    pub held_pearl: bool,
    /// The first hotbar slots holding an ender pearl and a wind charge.
    pub pearl_slot: Option<u8>,
    pub charge_slot: Option<u8>,
    pub selected: u8,
    pub pearls: &'a [Pearl],
    pub people: &'a [Person],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PearlAction {
    Select(u8),
    /// Use the held item once; with silent aim, at this rotation.
    Use(Option<(f32, f32)>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PearlOutput {
    /// In order.
    pub actions: Vec<PearlAction>,
}

impl Runtime {
    /// The host should gather pearls and players this tick.
    pub fn pearls_wanted(&self) -> bool {
        self.session().in_world()
            && (MODULES.iter().any(|m| self.enabled(*m)) || self.pearls.restore.is_some())
    }
    /// `onPlayerUseItem` with an ender pearl.
    pub fn pearl_used(&mut self, at: Instant) {
        let now = self.pearls.now(at);
        self.pearls.auto_pearl.pearl_used(now);
    }
    /// Enable and disable: `onEnable` runs on the module's next tick;
    /// `onDisable` hands the rotation back at once.
    pub(super) fn pearls_toggled(&mut self, module: ModuleId, enabled: bool) {
        let Some(index) = MODULES.iter().position(|m| *m == module) else {
            return;
        };
        self.pearls.started[index] = false;
        if enabled {
            return;
        }
        let now = self.pearls.now(Instant::now());
        let p = &mut self.pearls;
        let mut shared = Shared::of(&mut self.combat, &mut self.rotations);
        let restore = if index == 0 {
            p.auto_pearl.reset(&mut shared, now);
            None
        } else {
            p.pearl_catch.disable(&mut shared)
        };
        if restore.is_some() {
            self.pearls.restore = restore;
        }
        self.release_pearl_claims(index);
    }
    /// A world change: nothing tracked survives.
    pub(super) fn pearls_session_reset(&mut self) {
        let p = &mut self.pearls;
        p.auto_pearl.session_reset();
        p.pearl_catch.session_reset();
        p.leases = [Vec::new(), Vec::new()];
        p.restore = None;
    }
    fn release_pearl_claims(&mut self, index: usize) {
        for lease in std::mem::take(&mut self.pearls.leases[index]) {
            self.actions.release(lease);
        }
    }
    /// Holds (or takes) the hotbar, use and container claims for a module.
    fn pearl_claims(&mut self, index: usize) -> bool {
        let epoch = self.session.epoch();
        if !self.pearls.leases[index].is_empty() {
            if self.pearls.leases[index]
                .iter()
                .all(|l| self.actions.owns(*l, epoch))
            {
                return true;
            }
            self.release_pearl_claims(index);
            return false;
        }
        let mut leases = Vec::new();
        for resource in [Resource::Hotbar, Resource::UseItem, Resource::Container] {
            match self
                .actions
                .acquire(resource, OWNERS[index], epoch, PRIORITY)
            {
                Some(lease) => leases.push(lease),
                None => {
                    for lease in leases {
                        self.actions.release(lease);
                    }
                    return false;
                }
            }
        }
        self.pearls.leases[index] = leases;
        true
    }

    /// One client tick, after the shared rotation's: AutoPearl, then
    /// PearlCatch.
    pub fn pearls_tick(
        &mut self,
        input: &PearlInput<'_>,
        ray: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<Hit>,
    ) -> PearlOutput {
        let mut out = PearlOutput::default();
        let in_world = self.session().in_world();
        let on = MODULES.map(|m| self.enabled(m) && in_world);
        let now = self.pearls.now(input.at);
        let auto_options = auto_pearl::Options::read(&self.settings);
        let catch_options = pearl_catch::Options::read(&self.settings);
        let mut disable = [false; 2];
        // AutoPearl takes the claims for its throw and keeps them until the
        // slot is restored; it waits while another owner holds them.
        let hotbar_free =
            on[0] && self.pearls.auto_pearl.state() == State::PendingThrow && self.pearl_claims(0);
        // PearlCatch holds them for its whole run.
        if on[1] && !self.pearl_claims(1) {
            disable[1] = true;
        }
        let p = &mut self.pearls;
        let mut shared = Shared::of(&mut self.combat, &mut self.rotations);
        if on[0] {
            if !p.started[0] {
                p.started[0] = true;
                p.auto_pearl.enabled(&auto_options, &mut shared, now);
            }
            let tick = auto_pearl::Tick {
                now_ms: now,
                position: input.position,
                eye_height: input.eye_height,
                view: input.view,
                alive: input.alive,
                health: input.health,
                held: input.held.clone(),
                held_pearl: input.held_pearl,
                pearl_slot: input.pearl_slot,
                selected: input.selected,
                pearls: input.pearls,
                people: input.people,
                hotbar_free,
            };
            let fx = p.auto_pearl.tick(&auto_options, &tick, &mut shared, ray);
            out.actions.extend(fx.select.map(PearlAction::Select));
            out.actions.extend(fx.throw.map(PearlAction::Use));
            disable[0] = fx.disable;
        }
        if on[1] && !disable[1] {
            let player = Player {
                position: input.position,
                eye_height: input.eye_height,
                motion: input.motion,
                on_ground: input.on_ground,
                view: input.view,
            };
            if !p.started[1] {
                p.started[1] = true;
                disable[1] = !p.pearl_catch.enable(
                    &catch_options,
                    &player,
                    input.pearl_slot,
                    input.charge_slot,
                    input.selected,
                    &mut shared,
                );
            }
            if !disable[1] {
                let tick = pearl_catch::Tick { player };
                let fx = p.pearl_catch.tick(&catch_options, &tick, &mut shared);
                if let Some((slot, rotation)) = fx.throw {
                    out.actions.push(PearlAction::Select(slot));
                    out.actions.push(PearlAction::Use(rotation));
                }
                disable[1] = fx.disable;
            }
        }
        if !matches!(
            self.pearls.auto_pearl.state(),
            State::PendingThrow | State::PendingReset
        ) {
            self.release_pearl_claims(0);
        }
        for (index, module) in MODULES.into_iter().enumerate() {
            if disable[index] {
                self.set_enabled(module, false);
            }
        }
        if let Some(slot) = self.pearls.restore.take() {
            out.actions.push(PearlAction::Select(slot));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::lifecycle::Context;
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn input<'a>(
        at: Instant,
        view: (f32, f32),
        pearls: &'a [Pearl],
        people: &'a [Person],
    ) -> PearlInput<'a> {
        PearlInput {
            at,
            position: [0.0, 64.0, 0.0],
            eye_height: 1.62,
            motion: [0.0, -0.0784, 0.0],
            on_ground: true,
            view,
            alive: true,
            health: 20.0,
            held: HeldItem::default(),
            held_pearl: false,
            pearl_slot: Some(4),
            charge_slot: Some(5),
            selected: 0,
            pearls,
            people,
        }
    }
    /// The host's order: the shared rotation's updates, then the pearls.
    fn step(
        runtime: &mut Runtime,
        view: &mut (f32, f32),
        at: Instant,
        pearls: &[Pearl],
        people: &[Person],
    ) -> PearlOutput {
        runtime.rotation_eye([0.0, 65.62, 0.0]);
        if let Some([yaw, pitch]) = runtime.rotation_tick(*view, 0.5, true) {
            *view = (yaw, pitch);
        }
        runtime.pearls_tick(&input(at, *view, pearls, people), &mut floor)
    }
    fn floor(a: [f64; 3], b: [f64; 3]) -> Option<Hit> {
        if b[1] > 64.0 || a[1] < 64.0 {
            return None;
        }
        let t = (a[1] - 64.0) / (a[1] - b[1]);
        Some(Hit {
            position: [a[0] + (b[0] - a[0]) * t, 64.0, a[2] + (b[2] - a[2]) * t],
            water: false,
        })
    }
    fn in_world() -> Runtime {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime
    }

    #[test]
    fn pearl_catch_throws_both_and_restores_the_slot() {
        let mut runtime = in_world();
        runtime.set_enabled(ModuleId::PearlCatch, true);
        let t = Instant::now();
        let mut view = (0.0, 0.0);
        let mut actions = Vec::new();
        for tick in 0..80 {
            let at = t + Duration::from_millis(50 * tick);
            let out = step(&mut runtime, &mut view, at, &[], &[]);
            actions.extend(out.actions);
            if !runtime.enabled(ModuleId::PearlCatch) {
                break;
            }
        }
        assert!(!runtime.enabled(ModuleId::PearlCatch));
        // Silent by default: both throws carry the managed rotation, the
        // pearl straight up.
        assert_eq!(actions.len(), 5, "{actions:?}");
        assert_eq!(actions[0], PearlAction::Select(4));
        assert!(matches!(actions[1], PearlAction::Use(Some((_, pitch))) if pitch < -89.0));
        assert_eq!(actions[2], PearlAction::Select(5));
        assert!(matches!(actions[3], PearlAction::Use(Some(_))));
        assert_eq!(actions[4], PearlAction::Select(0));
        // The claims and the rotation are handed back.
        let epoch = runtime.session.epoch();
        assert!(!runtime.actions.claimed(Resource::Hotbar, epoch));
        assert!(runtime.combat.claim.owner().is_none());
        // The released silent aim returns to the view on the shared driver.
        assert!(runtime.combat.managed.relative);
        for _ in 0..40 {
            runtime.rotation_tick(view, 0.5, true);
        }
        assert!(!runtime.combat.managed.active);
        assert_eq!(runtime.rotations.silent_owner(), None);
    }

    #[test]
    fn a_held_hotbar_claim_stops_pearl_catch() {
        let mut runtime = in_world();
        let epoch = runtime.session.epoch();
        runtime
            .actions
            .acquire(Resource::Hotbar, Owner("macro"), epoch, 200)
            .unwrap();
        runtime.set_enabled(ModuleId::PearlCatch, true);
        let out = runtime.pearls_tick(&input(Instant::now(), (0.0, 0.0), &[], &[]), &mut floor);
        assert!(out.actions.is_empty());
        assert!(!runtime.enabled(ModuleId::PearlCatch));
    }

    #[test]
    fn auto_pearl_waits_for_the_hotbar_and_throws_once() {
        let mut runtime = in_world();
        runtime
            .set_setting("utility.auto-pearl.mode", json!("aggro"))
            .unwrap();
        runtime.set_enabled(ModuleId::AutoPearl, true);
        let people = [
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
        ];
        let pearl = Pearl {
            id: 77,
            position: [0.0, 66.0, 3.5],
            velocity: [0.0, 0.4, 1.2],
            in_water: false,
            stuck: false,
        };
        let t = Instant::now();
        let mut view = (0.0, 0.0);
        // Primed without the pearl, then it appears.
        step(&mut runtime, &mut view, t, &[], &people);
        let epoch = runtime.session.epoch();
        let blocker = runtime
            .actions
            .acquire(Resource::UseItem, Owner("macro"), epoch, 200)
            .unwrap();
        let pearls = [pearl];
        let mut actions = Vec::new();
        for tick in 1..40 {
            if tick == 20 {
                runtime.actions.release(blocker);
            }
            let at = t + Duration::from_millis(50 * tick);
            let out = step(&mut runtime, &mut view, at, &pearls, &people);
            if tick < 20 {
                assert!(out.actions.is_empty(), "tick {tick}: {:?}", out.actions);
            }
            actions.extend(out.actions);
        }
        assert_eq!(
            actions,
            vec![
                PearlAction::Select(4),
                PearlAction::Use(None),
                PearlAction::Select(0)
            ]
        );
        assert!(runtime.enabled(ModuleId::AutoPearl));
        assert!(!runtime.actions.claimed(Resource::Hotbar, epoch));
    }
}
