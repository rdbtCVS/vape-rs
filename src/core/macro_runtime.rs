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
        let can_restore = self.macro_can_restore();
        let effects = self.macro_engine.replace(macros)?;
        self.macro_pending.extend(
            effects.into_iter().filter(|effect| {
                can_restore || !matches!(effect, MacroEffect::RestoreHotbar { .. })
            }),
        );
        if let Some(lease) = self.macro_lease.take() {
            self.actions.release(lease);
        }
        if let Some(lease) = self.macro_use_lease.take() {
            self.actions.release(lease);
        }
        self.macro_selected = None;
        self.dirty = true;
        Ok(())
    }
    /// `MacroManager` keeps macros in insertion order and by identity, so
    /// names may repeat; an edit removes the old macro and adds the new one
    /// last.
    pub fn upsert_macro(&mut self, index: Option<usize>, definition: Macro) -> Result<(), String> {
        let mut macros = self.macros().to_vec();
        if let Some(index) = index {
            if index >= macros.len() {
                return Err("Unknown macro".into());
            }
            macros.remove(index);
        }
        macros.push(definition);
        self.replace_macros(macros)
    }
    pub fn remove_macro(&mut self, index: usize) -> Result<(), String> {
        let mut macros = self.macros().to_vec();
        if index >= macros.len() {
            return Err("Unknown macro".into());
        }
        macros.remove(index);
        self.replace_macros(macros)
    }
    pub fn macro_context(&mut self, context: MacroContext) {
        // Selection may change between an input cleanup and its frame drain.
        if self.macro_context.selected_slot != context.selected_slot {
            self.macro_pending
                .retain(|effect| !matches!(effect, MacroEffect::RestoreHotbar { .. }));
        }
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
    fn macro_can_restore(&self) -> bool {
        self.macro_lease
            .is_some_and(|lease| self.actions.owns(lease, self.session.epoch()))
            && self.macro_selected.is_some()
            && self.macro_selected == self.macro_context.selected_slot
    }
    pub(super) fn cancel_macros(&mut self, restore: bool) {
        let restore = restore && self.macro_can_restore();
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
    /// Cleanup and queued effects run every frame; `advance` (the client tick,
    /// the reference's `EventPreTick`) also moves the active macro one phase.
    pub fn macro_frame(&mut self, mut context: MacroContext, advance: bool) -> Vec<MacroEffect> {
        self.macro_context(context.clone());
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
            self.macro_context = context.clone();
            self.cancel_macros(!lost_lease && !manual_slot);
        } else if advance {
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
        if let Some(selected) = self.macro_selected {
            context.selected_slot = Some(selected);
        }
        self.macro_context = context;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::input::macros::Delay;
    use super::super::lifecycle::{CleanupReason, Context};
    use super::*;

    fn active_item() -> (Runtime, MacroContext) {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime
            .replace_macros(vec![Macro {
                name: "snowball".into(),
                binds: vec!["KeyM".into()],
                delay: Delay::bounded(100, 100),
                double_click: false,
                double_click_delay: Delay::bounded(100, 100),
            }])
            .unwrap();
        let mut context = MacroContext {
            hotbar: vec![None, None, Some("minecraft:snowball".into())],
            hotbar_names: vec![vec![], vec![], vec!["Snowball".into()]],
            selected_slot: Some(0),
            accepts_input: true,
            ..MacroContext::default()
        };
        runtime.macro_context(context.clone());
        runtime.input("KeyM", KeyAction::Press, true);
        assert!(runtime.macro_frame(context.clone(), false).is_empty());
        let effects = runtime.macro_frame(context.clone(), true);
        assert!(effects.contains(&MacroEffect::SelectHotbar { slot: 2 }));
        assert!(effects.contains(&MacroEffect::UseItem { down: true }));
        context.selected_slot = Some(2);
        (runtime, context)
    }

    #[test]
    fn menu_cleanup_restores_owned_slot_and_releases_use_claim() {
        let (mut runtime, mut context) = active_item();
        runtime.release_input();
        context.accepts_input = false;
        let effects = runtime.macro_frame(context, false);
        assert!(effects.contains(&MacroEffect::RestoreHotbar { slot: 0 }));
        assert!(effects.contains(&MacroEffect::UseItem { down: false }));
        assert!(!runtime.use_claimed());
    }

    #[test]
    fn manual_selection_wins_over_macro_cleanup() {
        for queued_cleanup in [false, true] {
            let (mut runtime, mut context) = active_item();
            if queued_cleanup {
                runtime.release_input();
            }
            context.selected_slot = Some(1);
            let effects = runtime.macro_frame(context, false);
            assert!(
                !effects
                    .iter()
                    .any(|effect| matches!(effect, MacroEffect::RestoreHotbar { .. }))
            );
            assert!(effects.contains(&MacroEffect::UseItem { down: false }));
            assert!(!runtime.use_claimed());
        }
    }

    #[test]
    fn panic_restores_same_world_but_dimension_drops_old_selection() {
        for (reason, restore) in [
            (CleanupReason::Panic, true),
            (CleanupReason::Dimension, false),
        ] {
            let (mut runtime, context) = active_item();
            runtime.session_change(reason);
            let effects = runtime.macro_frame(context, false);
            assert_eq!(
                effects.contains(&MacroEffect::RestoreHotbar { slot: 0 }),
                restore
            );
            assert!(!runtime.use_claimed());
        }
    }

    #[test]
    fn names_may_repeat_and_edits_move_the_macro_last() {
        let named = |name: &str, key: &str| Macro {
            name: name.into(),
            binds: vec![key.into()],
            delay: Delay::bounded(100, 100),
            double_click: false,
            double_click_delay: Delay::bounded(100, 100),
        };
        let mut runtime = Runtime::default();
        runtime
            .upsert_macro(None, named("Snowball", "KeyR"))
            .unwrap();
        runtime
            .upsert_macro(None, named("snowball", "KeyU"))
            .unwrap();
        runtime.upsert_macro(None, named("egg", "KeyV")).unwrap();
        // Editing the first one (removeMacro then addMacro) puts it last.
        runtime
            .upsert_macro(Some(0), named("Snowball", "KeyY"))
            .unwrap();
        let order: Vec<_> = runtime
            .macros()
            .iter()
            .map(|m| (m.name.as_str(), m.binds[0].as_str()))
            .collect();
        assert_eq!(
            order,
            [("snowball", "KeyU"), ("egg", "KeyV"), ("Snowball", "KeyY")]
        );
        runtime.remove_macro(0).unwrap();
        assert_eq!(runtime.macros().len(), 2);
        assert!(runtime.remove_macro(2).is_err());
        assert!(runtime.upsert_macro(Some(5), named("x", "KeyX")).is_err());
        assert_eq!(runtime.macros().len(), 2);
    }
}
