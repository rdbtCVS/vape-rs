// Profile-owned macros share the hotbar coordinator with gameplay modules.
use super::super::{
    actions::{Owner, Resource},
    input::{
        KeyAction,
        macros::{InputAction, InputOrigin, Macro, MacroContext, MacroEffect},
    },
};
use super::Runtime;

impl Runtime {
    pub fn macros_active_item(&self) -> bool {
        self.macro_engine.uses_hotbar()
    }
    pub fn macros(&self) -> &[Macro] {
        self.macro_engine.macros()
    }
    pub fn replace_macros(&mut self, macros: Vec<Macro>) -> Result<(), String> {
        let effects = self.macro_engine.replace(macros)?;
        self.macro_pending.extend(effects);
        self.dirty = true;
        Ok(())
    }
    pub fn upsert_macro(&mut self, definition: Macro) -> Result<(), String> {
        let mut macros = self.macros().to_vec();
        if let Some(existing) = macros
            .iter_mut()
            .find(|m| m.name.eq_ignore_ascii_case(&definition.name))
        {
            *existing = definition;
        } else {
            macros.push(definition);
        }
        self.replace_macros(macros)
    }
    pub fn remove_macro(&mut self, name: &str) -> Result<(), String> {
        let macros = self
            .macros()
            .iter()
            .filter(|m| m.name != name)
            .cloned()
            .collect();
        self.replace_macros(macros)
    }
    pub fn macro_context(&mut self, context: MacroContext) {
        self.macro_context = context;
    }
    fn macro_now(&self) -> u64 {
        self.macro_origin
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64
    }
    pub(super) fn macro_input(&mut self, key: &str, action: KeyAction, accepts_input: bool) {
        let action = match action {
            KeyAction::Press => InputAction::Press,
            KeyAction::Release => InputAction::Release,
            KeyAction::Repeat => InputAction::Repeat,
            KeyAction::FocusLost => InputAction::FocusLost,
        };
        let mut context = self.macro_context.clone();
        context.accepts_input &= accepts_input;
        let effects = self.macro_engine.input(
            key,
            action,
            InputOrigin::Physical,
            self.macro_now(),
            &context,
        );
        self.macro_pending.extend(effects);
        if self.macro_engine.uses_hotbar() && self.macro_lease.is_none() {
            self.macro_lease = self.actions.acquire(
                Resource::Hotbar,
                Owner("client.macros"),
                self.session.epoch(),
                200,
            );
            self.macro_use_lease = self.actions.acquire(
                Resource::UseItem,
                Owner("client.macros"),
                self.session.epoch(),
                200,
            );
            if self.macro_lease.is_none() || self.macro_use_lease.is_none() {
                self.cancel_macros(false);
            } else {
                self.macro_selected = context.selected_slot;
            }
        }
    }
    pub(super) fn cancel_macros(&mut self, restore: bool) {
        let effects = self.macro_engine.cancel();
        self.macro_pending.extend(
            effects
                .into_iter()
                .filter(|e| restore || matches!(e, MacroEffect::UseItem { down: false })),
        );
        if let Some(lease) = self.macro_lease.take() {
            self.actions.release(lease);
        }
        if let Some(lease) = self.macro_use_lease.take() {
            self.actions.release(lease);
        }
        self.macro_selected = None;
    }
    pub fn macro_frame(&mut self, mut context: MacroContext) -> Vec<MacroEffect> {
        context.accepts_input &= !self.gui.open && self.session.interactive();
        let lost_lease = self
            .macro_lease
            .is_some_and(|l| !self.actions.owns(l, self.session.epoch()))
            || self
                .macro_use_lease
                .is_some_and(|l| !self.actions.owns(l, self.session.epoch()));
        let manual_slot = self.macro_engine.uses_hotbar()
            && self.macro_selected.is_some()
            && self.macro_selected != context.selected_slot;
        if !context.accepts_input || lost_lease || manual_slot {
            self.cancel_macros(false);
        } else {
            let effects = self.macro_engine.tick(self.macro_now(), &context);
            self.macro_pending.extend(effects);
        }
        let effects = std::mem::take(&mut self.macro_pending);
        for effect in &effects {
            if let MacroEffect::SelectHotbar { slot } = effect {
                self.macro_selected = Some(*slot);
            }
        }
        if !self.macro_engine.active() {
            if let Some(lease) = self.macro_use_lease.take() {
                self.actions.release(lease);
            }
            if let Some(lease) = self.macro_lease.take() {
                self.actions.release(lease);
            }
            self.macro_selected = None;
        }
        self.macro_context = context;
        effects
    }
}
