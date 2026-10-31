// Per-module visibility (`Mod.visible`): hidden modules leave their category
// frame, refuse bind toggles with a warning toast and are switched off when a
// profile loads. The GUI's edit mode flips it; a GUI click on a hidden module
// (from search) enables and unhides it. Stored per profile as
// `modules.<id>.visible`, written only when it differs from the default.
use super::super::ui::notifications::Kind;
use super::super::ui::text_gui;
use super::{ModuleId, Runtime};
use serde_json::{Value, json};

/// Modules the reference constructs with `setDefaultVisibility(false)`.
/// Panic is left out: it ships with an F10 bind the reference lacks, and a
/// hidden default would turn that bind into a refusal toast.
const DEFAULT_HIDDEN: [ModuleId; 8] = [
    ModuleId::Speed,
    ModuleId::Fly,
    ModuleId::Timer,
    ModuleId::InvCleaner,
    ModuleId::ThrowDebuff,
    ModuleId::AntiDebuff,
    ModuleId::InvWalk,
    ModuleId::AutoHotbar,
];

pub(super) fn default_visible(module: ModuleId) -> bool {
    !DEFAULT_HIDDEN.contains(&module)
}

/// `Mod.load`: the stored flag, else the module's default.
pub(super) fn visible_in(document: &Value, module: ModuleId) -> bool {
    document["modules"][module.id()]["visible"]
        .as_bool()
        .unwrap_or_else(|| default_visible(module))
}

impl Runtime {
    pub fn visible(&self, module: ModuleId) -> bool {
        visible_in(&self.document.0, module)
    }
    /// `Mod.setVisibility` plus `Mod.save`'s "only when not the default".
    pub(super) fn set_visible(&mut self, module: ModuleId, visible: bool) {
        let modules = &mut self.document.0["modules"];
        if !modules.is_object() {
            *modules = json!({});
        }
        let state = modules
            .as_object_mut()
            .unwrap()
            .entry(module.id())
            .or_insert_with(|| json!({}));
        if visible == default_visible(module) {
            if let Some(state) = state.as_object_mut() {
                state.remove("visible");
            }
        } else {
            state["visible"] = json!(visible);
        }
        self.dirty = true;
    }
    fn warn(&mut self, key: &str, title: &str, message: &str) {
        if self.hud_flag("client.notifications") {
            self.toasts.push(key, title, message, Kind::Warning, 2500.0);
        }
    }
    /// `Mod.setEnabled(true, false)` on a hidden module: refused with a
    /// warning. Returns whether the enable was refused.
    pub(super) fn refuse_hidden(&mut self, module: ModuleId, enabling: bool) -> bool {
        if !enabling || self.visible(module) {
            return false;
        }
        let message = format!("Attempted to toggle {}!", module.name());
        self.warn(
            &format!("hidden:{}", module.id()),
            "Hidden Module",
            &message,
        );
        true
    }
    /// `ModManager.disableHiddenModules` after a profile applies; modules
    /// without a Text GUI colour (`getGuiColor() == 0`) are left alone.
    pub(super) fn disable_hidden_modules(&mut self) {
        let hidden: Vec<_> = ModuleId::ALL
            .into_iter()
            .filter(|&m| {
                !self.visible(m) && self.enabled(m) && text_gui::module_color(m.id()).is_some()
            })
            .collect();
        for &module in &hidden {
            self.set_enabled(module, false);
        }
        if !hidden.is_empty() {
            let message = format!("{} module(s) have been disabled!", hidden.len());
            self.warn("hidden-disabled", "Hidden Disabled", &message);
        }
    }
    /// `isRequiresBind`: `UtilityMod` subclasses, and InvCleaner and
    /// AutoHotbar while their activation is "On Key".
    fn requires_bind(&self, module: ModuleId) -> bool {
        match module {
            ModuleId::Panic
            | ModuleId::ThrowDebuff
            | ModuleId::ArmorSwitch
            | ModuleId::ThrowPot
            | ModuleId::Refill
            | ModuleId::WindCharge => true,
            ModuleId::InvCleaner | ModuleId::AutoHotbar => self
                .settings
                .effective(&format!("{}.trigger", module.id()))
                .is_some_and(|v| v == "on-key"),
            _ => false,
        }
    }
    /// `ModuleComponentToggleClickHandler.onPrimaryClick` outside the edit
    /// mode: bind-only modules refuse, others toggle past the visibility
    /// check and become visible.
    pub(super) fn gui_toggle(&mut self, module: ModuleId) {
        if self.requires_bind(module) {
            let bound = self
                .settings
                .get(&format!("{}.binds", module.id()))
                .and_then(Value::as_array)
                .is_some_and(|binds| !binds.is_empty());
            self.gui.refuse_toggle(module.id(), bound);
            return;
        }
        self.set_enabled(module, !self.enabled(module));
        if !self.visible(module) {
            self.set_visible(module, true);
        }
    }
    /// The edit mode's checkbox: hiding an enabled module disables it.
    pub(super) fn flip_visibility(&mut self, module: ModuleId) {
        let visible = !self.visible(module);
        self.set_visible(module, visible);
        if !visible && self.enabled(module) {
            self.set_enabled(module, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Action, Document, KeyAction};
    use super::*;

    fn warnings(runtime: &Runtime) -> Vec<(String, String)> {
        runtime
            .toasts
            .iter()
            .filter(|t| t.kind == Kind::Warning)
            .map(|t| (t.title.clone(), t.message.clone()))
            .collect()
    }

    #[test]
    fn hidden_modules_refuse_binds_and_store_only_non_default_visibility() {
        let mut r = Runtime::default();
        r.settings
            .set("render.fullbright.binds", json!(["KeyG"]))
            .unwrap();
        r.flip_visibility(ModuleId::Fullbright);
        assert!(!r.visible(ModuleId::Fullbright));
        assert_eq!(
            r.snapshot().0["modules"]["render.fullbright"]["visible"],
            false
        );
        r.input("KeyG", KeyAction::Press, true);
        assert!(!r.enabled(ModuleId::Fullbright));
        assert_eq!(
            warnings(&r),
            [(
                "Hidden Module".into(),
                "Attempted to toggle Fullbright!".into()
            )]
        );
        // A GUI click (from search) bypasses the check and unhides it.
        r.perform(vec![Action::Toggle("render.fullbright".into())]);
        assert!(r.enabled(ModuleId::Fullbright) && r.visible(ModuleId::Fullbright));
        assert!(
            r.snapshot().0["modules"]["render.fullbright"]
                .get("visible")
                .is_none()
        );
        // Hiding an enabled module disables it.
        r.perform(vec![Action::Visibility("render.fullbright".into())]);
        assert!(!r.enabled(ModuleId::Fullbright));
        // Default-hidden modules store `true` once shown.
        assert!(r.visible(ModuleId::Panic) && !r.visible(ModuleId::InvCleaner));
        r.flip_visibility(ModuleId::InvCleaner);
        assert_eq!(
            r.snapshot().0["modules"]["inventory.inv-cleaner"]["visible"],
            true
        );
    }

    #[test]
    fn hidden_panic_refuses_f10_and_keeps_the_gui_open() {
        let mut r = Runtime::default();
        r.flip_visibility(ModuleId::Panic);
        r.set_enabled(ModuleId::Fps, true);
        r.input("F10", KeyAction::Press, true);
        assert!(r.enabled(ModuleId::Fps));
        r.input("ShiftRight", KeyAction::Press, true);
        assert!(r.gui.open);
        r.input("F10", KeyAction::Press, true);
        assert!(r.gui.open && r.enabled(ModuleId::Fps));
        assert_eq!(warnings(&r).len(), 1); // the reused "hidden:" key
    }

    #[test]
    fn bind_only_modules_refuse_gui_clicks_with_a_row_status() {
        let mut r = Runtime::default();
        r.perform(vec![Action::Toggle("inventory.refill".into())]);
        assert!(!r.enabled(ModuleId::Refill));
        assert_eq!(
            r.gui.row_status_text("inventory.refill"),
            Some("MUST BE BOUND")
        );
        r.settings
            .set("inventory.refill.binds", json!(["KeyR"]))
            .unwrap();
        r.perform(vec![Action::Toggle("inventory.refill".into())]);
        assert_eq!(
            r.gui.row_status_text("inventory.refill"),
            Some("USE VIA BIND")
        );
        // InvCleaner only while its activation is "On Key".
        r.settings
            .set("inventory.inv-cleaner.trigger", json!("toggle"))
            .unwrap();
        r.perform(vec![Action::Toggle("inventory.inv-cleaner".into())]);
        assert!(r.enabled(ModuleId::InvCleaner) && r.visible(ModuleId::InvCleaner));
    }

    #[test]
    fn profile_loads_skip_and_then_disable_hidden_modules() {
        let mut r = Runtime::default();
        r.set_enabled(ModuleId::Fullbright, true);
        r.set_enabled(ModuleId::Sprint, true);
        r.apply_document(
            Document::parse(
                br#"{"schema":1,"modules":{"render.fullbright":{"enabled":false,"visible":false},"combat.sprint":{"enabled":true,"visible":false},"hud.fps":{"enabled":true}}}"#,
                false,
            )
            .unwrap(),
        );
        // Fullbright kept its state through the loop, then both went off.
        assert!(!r.enabled(ModuleId::Fullbright) && !r.enabled(ModuleId::Sprint));
        assert!(r.enabled(ModuleId::Fps));
        assert_eq!(
            warnings(&r),
            [(
                "Hidden Disabled".into(),
                "2 module(s) have been disabled!".into()
            )]
        );
    }
}
