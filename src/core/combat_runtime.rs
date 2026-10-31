// Runtime side of the combat-worker modules: the shared rotation claim and
// managed rotation, KillAura, SilentAura (with its clicker) and KeepSprint.
use super::super::modules::combat::{
    keep_sprint::{self, KeepSprint},
    kill_aura::{self, KillAura},
    rotation::{Claim, Claimant, Managed, UPDATES_PER_TICK},
    silent_aura::{self, Click, SilentAura},
};
use super::settings::Definition;
use super::{ModuleId, Runtime};

#[derive(Debug, Default)]
pub struct Combat {
    pub(super) claim: Claim,
    pub(super) managed: Managed,
    kill_aura: KillAura,
    silent_aura: SilentAura,
    keep_sprint: KeepSprint,
}

pub fn definitions() -> Vec<Definition> {
    kill_aura::definitions()
        .into_iter()
        .chain(silent_aura::definitions())
        .chain(keep_sprint::definitions())
        .collect()
}

/// One SilentAura tick's outcome for the host.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    pub click: Click,
    /// The managed rotation to send this tick instead of the view.
    pub sent: Option<(f32, f32)>,
}

impl Runtime {
    /// The module was disabled.
    pub(super) fn combat_disabled(&mut self, module: ModuleId) {
        let c = &mut self.combat;
        match module {
            ModuleId::KillAura => c.kill_aura.reset(),
            ModuleId::KeepSprint => c.keep_sprint.reset(),
            // The managed rotation returns to the view before the claim is
            // released (the reference's deferred toggle-off).
            ModuleId::SilentAura => c.silent_aura.reset(&mut c.claim, &mut c.managed),
            _ => {}
        }
    }
    pub(super) fn combat_setting_changed(&mut self, id: &str) {
        let c = &mut self.combat;
        if id.starts_with("combat.kill-aura.") {
            c.kill_aura.reset();
        }
        if id.starts_with("combat.keep-sprint.") {
            c.keep_sprint.reset();
        }
        if id.starts_with("combat.silent-aura.") {
            c.silent_aura.reset(&mut c.claim, &mut c.managed);
        }
    }
    /// Session cleanup: nothing survives a world change, including the
    /// managed rotation.
    pub(super) fn combat_session_reset(&mut self) {
        let c = &mut self.combat;
        c.kill_aura.reset();
        c.keep_sprint.reset();
        c.silent_aura.reset(&mut c.claim, &mut c.managed);
        c.managed.clear();
        c.claim.clear();
    }

    pub fn kill_aura_active(&self) -> bool {
        self.enabled(ModuleId::KillAura) && self.session().in_world()
    }
    pub fn kill_aura_tick(&mut self, tick: &kill_aura::Tick) -> kill_aura::Effects {
        let options = kill_aura::Options::read(&self.settings);
        let people = self.target_people();
        let effects = self.combat.kill_aura.tick(&options, tick, &people);
        if effects.disable {
            self.set_enabled(ModuleId::KillAura, false);
        }
        effects
    }

    pub fn silent_aura_wanted(&self) -> bool {
        (self.enabled(ModuleId::SilentAura) && self.session().in_world())
            || self.combat.managed.active
    }
    /// One tick: SilentAura's targeting and aim (while enabled), the managed
    /// rotation's 50 updates, then the clicker.
    pub fn silent_aura_tick(&mut self, tick: &silent_aura::Tick) -> Rotation {
        let options = silent_aura::Options::read(&self.settings);
        let people = self.target_people();
        let enabled = self.enabled(ModuleId::SilentAura) && self.session().in_world();
        let c = &mut self.combat;
        let mut disable = false;
        if enabled {
            disable = c
                .silent_aura
                .tick(&options, tick, &people, &mut c.claim, &mut c.managed)
                .disable;
        }
        let distance = c.silent_aura.distance_to_target(tick);
        let aura = &mut c.silent_aura;
        let view = (tick.player.yaw, tick.player.pitch);
        // Another owner's silent rotation (still its own, or returning) is
        // stepped by `rotation_runtime`.
        let driven = c.managed.active
            && self.rotations.silent_owner().is_some_and(|o| {
                o != Claimant::SilentAura && (c.claim.owned_by(o) || c.managed.relative)
            });
        if !driven
            && c.managed
                .tick(view, tick.sensitivity, UPDATES_PER_TICK, || {
                    aura.step(&options, distance)
                })
        {
            c.silent_aura.returned(&mut c.claim);
        }
        let click = if enabled && !disable {
            // Vanilla's entity interaction range for the click's ray.
            c.silent_aura
                .click(&options, tick, &c.managed, silent_aura::BASE_RANGE)
        } else {
            Click::None
        };
        let sent = c.managed.sent();
        if disable {
            self.set_enabled(ModuleId::SilentAura, false);
        }
        Rotation { click, sent }
    }

    pub fn keep_sprint_hit(&mut self, hit: keep_sprint::Hit) -> Option<keep_sprint::Keep> {
        if !self.enabled(ModuleId::KeepSprint) {
            return None;
        }
        let options = keep_sprint::Options::read(&self.settings);
        self.combat.keep_sprint.post_attack(&options, hit)
    }
}
