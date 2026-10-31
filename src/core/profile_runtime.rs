// The reference's local profile model (`ProfilesManager`, `Profile`,
// `BuiltinProfile`): one active profile whose file follows the live state,
// switching that captures the active profile, resets every per-profile
// value and applies the target, the "Modern PVP" / "Classic PVP" built-ins,
// profile keybinds, and the global values that survive a switch.
//
// `current.json` stays the live state and carries the profile index:
// `profile` (the active name), `profile_order` (newest first, as
// `addProfile(profile, true)` prepends) and `profile_binds`.

use super::super::input::BindingMode;
use super::profiles::Document;
use super::{ModuleId, Runtime};
use serde_json::{Value, json};

/// `Profile.toJson` keeps names to 48 characters.
pub const MAX_NAME: usize = 48;
const INDEX: [&str; 3] = ["profile", "profile_order", "profile_binds"];
/// `PublicProfileSettings` values: global, so a switch keeps them.
const GLOBAL_SETTINGS: [&str; 6] = [
    "client.gui.binds",
    "client.notifications",
    "client.toggle-alerts",
    "client.profile-alerts",
    "client.profile-auto-load",
    "client.profile-frames",
];
/// `initializeBuiltinProfiles`: `Minecraft121BuiltinProfile` then
/// `BuiltinProfileState`, each the default state plus its favorites; the
/// first applicable one (Modern PVP on 1.21+) is selected. Favorites the
/// port has no module for are skipped, as `selectModule` skips them.
const BUILTINS: [(&str, &[&str]); 2] = [
    (
        "Modern PVP",
        &[
            "combat.trigger-bot",
            "combat.aim-assist",
            "combat.sprint",
            "inventory.auto-totem",
            "utility.wind-charge",
        ],
    ),
    (
        "Classic PVP",
        &[
            "combat.aim-assist",
            "combat.auto-clicker",
            "combat.jump-reset",
            "combat.sprint",
            "blatant.backtrack",
        ],
    ),
];
/// Name given to a pre-profile `current.json` live state on migration.
const MIGRATED: &str = "Default";

/// `HudModule`s keep their own state list (`legit_enabled`), restored even
/// when "Auto-load module states" is off; the port's HUD and Legit modules.
fn hud_module(module: ModuleId) -> bool {
    matches!(module.category(), "HUD" | "Legit")
}

/// The name a create or rename would store: trimmed, at most 48
/// characters, no control characters. `None` means a blank name.
pub fn clean_name(name: &str) -> Result<Option<String>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!("Profile names are at most {MAX_NAME} characters"));
    }
    super::profiles::file_stem(name)?;
    Ok(Some(name.into()))
}

impl Runtime {
    pub fn active_profile(&self) -> Option<String> {
        self.document.0["profile"].as_str().map(Into::into)
    }
    /// Profiles in `sortOrder`: the stored order, then any other file
    /// (an import or one copied in by hand) alphabetically.
    pub(super) fn ordered_profiles(&self) -> Result<Vec<String>, String> {
        let mut names = self.profiles()?;
        let order: Vec<&str> = self.document.0["profile_order"]
            .as_array()
            .map(|v| v.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        names.sort_by_key(|n| order.iter().position(|o| o == n).unwrap_or(usize::MAX));
        Ok(names)
    }
    pub(super) fn refresh_profiles(&mut self) {
        self.profile_names = self.ordered_profiles().unwrap_or_default();
        self.document.0["profile_order"] = json!(self.profile_names);
        if let Some(active) = self.active_profile() {
            self.gui.select_profile(&active);
        }
    }
    fn find_profile(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self
            .profiles()?
            .into_iter()
            .find(|n| n.eq_ignore_ascii_case(name) || n.to_lowercase() == name.to_lowercase()))
    }
    pub(super) fn profile_binds(&self) -> Vec<(String, Vec<String>)> {
        self.document.0["profile_binds"]
            .as_object()
            .map(|binds| {
                binds
                    .iter()
                    .map(|(name, keys)| {
                        let keys = keys
                            .as_array()
                            .map(|k| k.iter().filter_map(Value::as_str).map(Into::into).collect())
                            .unwrap_or_default();
                        (name.clone(), keys)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// `addProfile(profile, true)`: a new profile goes first.
    fn prepend_profile(&mut self, name: &str) {
        let mut order = vec![json!(name)];
        order.extend(self.profile_names.iter().map(|n| json!(n)));
        self.document.0["profile_order"] = Value::Array(order);
    }
    /// The live state as a profile file: without the index and the global
    /// friend and enemy lists.
    pub(super) fn profile_snapshot(&self) -> Document {
        let mut document = self.snapshot();
        let object = document.0.as_object_mut().unwrap();
        for key in INDEX.into_iter().chain(["friends", "enemies"]) {
            object.remove(key);
        }
        document
    }
    /// `Profile.captureCurrentState` for the active profile.
    pub(super) fn capture_active(&self) -> Result<(), String> {
        if let (Some(storage), Some(active)) = (&self.storage, self.active_profile()) {
            storage.save(&active, &self.profile_snapshot())?;
        }
        Ok(())
    }
    /// Startup (`Vape` init plus `ProfilesManager.loadJson`): apply the live
    /// state, seed the built-ins when there are no profiles, and give a
    /// pre-profile live state its own profile.
    pub(super) fn start_profiles(&mut self, current: Option<Document>) -> Result<(), String> {
        let storage = self.storage.clone().ok_or("Local storage is unavailable")?;
        let existing = storage.list()?;
        match current {
            Some(document) => {
                let active = document.0["profile"].as_str().map(String::from);
                self.apply_profile(document, false);
                if active.is_none() {
                    let name = if existing.is_empty() {
                        self.seed_builtins(true)?;
                        BUILTINS[0].0.to_string()
                    } else {
                        let mut name = MIGRATED.to_string();
                        let mut n = 2;
                        while existing.iter().any(|e| e.eq_ignore_ascii_case(&name)) {
                            name = format!("{MIGRATED} {n}");
                            n += 1;
                        }
                        let mut order = vec![name.clone()];
                        order.extend(existing);
                        self.document.0["profile_order"] = json!(order);
                        name
                    };
                    self.document.0["profile"] = json!(name);
                    self.capture_active()?;
                }
            }
            None if existing.is_empty() => {
                let mut first = self.seed_builtins(false)?;
                first.0["profile"] = json!(BUILTINS[0].0);
                first.0["profile_order"] = json!(BUILTINS.map(|b| b.0));
                self.apply_profile(first, false);
            }
            None => {
                // No live state but saved profiles: `getActiveProfile`
                // falls back to the first profile.
                let first = existing[0].clone();
                let mut document = storage.load(&first)?;
                document.0["profile"] = json!(first);
                document.0["profile_order"] = json!(existing);
                self.apply_profile(document, false);
            }
        }
        self.dirty = true;
        self.refresh_profiles();
        Ok(())
    }
    /// Writes the built-ins. With `keep_live`, the first one takes the live
    /// state of a pre-profile install, keeping its favorites if it had any.
    fn seed_builtins(&mut self, keep_live: bool) -> Result<Document, String> {
        let storage = self.storage.clone().ok_or("Local storage is unavailable")?;
        let mut first = None;
        for (i, (name, favorites)) in BUILTINS.into_iter().enumerate() {
            let mut document = if keep_live && i == 0 {
                self.profile_snapshot()
            } else {
                Document::default()
            };
            let modules = document.0["modules"].as_object_mut().unwrap();
            let has_favorites = modules
                .values()
                .any(|m| m["favorite"].as_bool() == Some(true));
            if !(keep_live && i == 0 && has_favorites) {
                for module in modules.values_mut() {
                    module.as_object_mut().unwrap().remove("favorite");
                }
                for id in favorites {
                    modules.entry(id.to_string()).or_insert_with(|| json!({}))["favorite"] =
                        json!(true);
                }
            }
            if keep_live && i == 0 {
                self.document.0["modules"] = document.0["modules"].clone();
            }
            storage.save(name, &document)?;
            first.get_or_insert(document);
        }
        self.document.0["profile_order"] = json!(BUILTINS.map(|b| b.0));
        Ok(first.unwrap())
    }
    /// `ProfilesManager.switchProfile`: capture the active profile, then
    /// `setActiveProfile`, which does nothing for the active one and
    /// otherwise resets every per-profile value and applies the target.
    pub(super) fn switch_profile(&mut self, name: &str) -> Result<(), String> {
        let name = self.find_profile(name)?.ok_or("Unknown profile")?;
        if self.active_profile().as_deref() == Some(name.as_str()) {
            return Ok(());
        }
        self.capture_active()?;
        let mut target = self
            .storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .load(&name)?;
        self.keep_globals(&mut target);
        target.0["profile"] = json!(name);
        self.apply_profile(target, true);
        self.refresh_profiles();
        self.flush()?;
        Ok(())
    }
    /// Carries what a switch keeps into the target: the profile index,
    /// friends and enemies, the global values, and frame positions unless
    /// "Frame positions per profile" is on.
    fn keep_globals(&self, target: &mut Document) {
        for key in INDEX.into_iter().chain(["friends", "enemies"]) {
            target.0[key] = self.document.0[key].clone();
        }
        if !target.0["settings"].is_object() {
            target.0["settings"] = json!({});
        }
        for id in GLOBAL_SETTINGS {
            if let Some(value) = self.settings.get(id) {
                target.0["settings"][id] = value.clone();
            }
        }
        if !self.hud_flag("client.profile-frames") {
            target.0["ui"] = self.document.0["ui"].clone();
        }
    }
    /// `Profile.apply` / `loadData(true)`: values, macros and favorites, then
    /// with "Auto-load module states" the saved module states.
    /// Without it a switch leaves every module as it was, and startup
    /// restores only the HUD modules (`applyLegitEnabledModuleStates`).
    pub(super) fn apply_profile(&mut self, document: Document, switching: bool) {
        let states = document.0["modules"].clone();
        let suppress = std::mem::replace(&mut self.suppress_toggle_alerts, true);
        self.apply_values(document);
        let auto_load = self.hud_flag("client.profile-auto-load");
        let mut enabled = 0;
        for module in ModuleId::ALL {
            if module == ModuleId::Panic {
                continue;
            }
            let saved = states[module.id()]["enabled"].as_bool().unwrap_or(false)
                && self.mode(module) != BindingMode::Hold;
            if auto_load || (!switching && hud_module(module)) {
                self.set_enabled(module, saved);
                enabled += usize::from(auto_load && saved && !hud_module(module));
            }
        }
        self.suppress_toggle_alerts = suppress;
        if auto_load && let Some(name) = self.active_profile() {
            self.profile_alert(&name, enabled);
        }
    }
    /// `ClickGuiProfilesPage` create: a blank name does nothing, an existing
    /// name (ignoring case) is ignored, otherwise the active profile is
    /// captured, cloned under the new name, prepended and made active.
    pub(super) fn create_profile(&mut self, name: &str) -> Result<(), String> {
        let Some(name) = clean_name(name)? else {
            return Ok(());
        };
        if self.find_profile(&name)?.is_some() {
            return Ok(());
        }
        self.capture_active()?;
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .save(&name, &self.profile_snapshot())?;
        self.prepend_profile(&name);
        self.document.0["profile"] = json!(name);
        self.dirty = true;
        self.refresh_profiles();
        self.notify(format!("Created profile {name}"));
        Ok(())
    }
    /// The profile menu's "Save current settings": the live state into the
    /// named profile (the active profile autosaves anyway).
    pub(super) fn save_profile_as(&mut self, name: &str) -> Result<(), String> {
        let name = self.find_profile(name)?.ok_or("Unknown profile")?;
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .save(&name, &self.profile_snapshot())?;
        self.notify(format!("Saved profile {name}"));
        Ok(())
    }
    pub(super) fn rename_profile(&mut self, old: &str, new: &str) -> Result<(), String> {
        let Some(new) = clean_name(new)? else {
            return Ok(());
        };
        if let Some(existing) = self.find_profile(&new)?
            && existing != old
        {
            return Err("profile already exists".into());
        }
        if old == new {
            return Ok(());
        }
        let active = self.active_profile().as_deref() == Some(old);
        if active {
            self.capture_active()?;
        }
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .rename(old, &new)?;
        if active {
            self.document.0["profile"] = json!(new);
        }
        if let Some(order) = self.document.0["profile_order"].as_array_mut() {
            for entry in order.iter_mut().filter(|e| e.as_str() == Some(old)) {
                *entry = json!(new);
            }
        }
        if let Some(binds) = self.document.0["profile_binds"].as_object_mut()
            && let Some(keys) = binds.remove(old)
        {
            binds.insert(new.clone(), keys);
        }
        self.dirty = true;
        self.refresh_profiles();
        self.gui
            .select_profile(&self.active_profile().unwrap_or_default());
        self.notify(format!("Renamed profile to {new}"));
        Ok(())
    }
    /// The GUI keeps the selected profile: "You cannot delete your selected
    /// profile".
    pub(super) fn delete_profile(&mut self, name: &str) -> Result<(), String> {
        if self.active_profile().as_deref() == Some(name) {
            return Err("You cannot delete your selected profile".into());
        }
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .delete(name)?;
        if let Some(binds) = self.document.0["profile_binds"].as_object_mut() {
            binds.remove(name);
        }
        self.dirty = true;
        self.refresh_profiles();
        Ok(())
    }
    /// A local extra: `imports/<name>.json` becomes a profile, added first
    /// like `addProfile(profile, true)`, its module states left off.
    pub(super) fn import_profile(&mut self, name: &str) -> Result<(), String> {
        let Some(name) = clean_name(name)? else {
            return Ok(());
        };
        if self.find_profile(&name)?.is_some() {
            return Err("profile already exists; choose a new name".into());
        }
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .import_file(&name)?;
        self.prepend_profile(&name);
        self.dirty = true;
        self.refresh_profiles();
        self.notify(format!("Imported profile {name}"));
        Ok(())
    }
    pub(super) fn export_profile(&mut self, name: &str) -> Result<(), String> {
        if self.active_profile().as_deref() == Some(name) {
            self.capture_active()?;
        }
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .export_file(name)?;
        self.notify(format!("Exported profile {name}"));
        Ok(())
    }
    /// A profile entry's right click: `Profile.applyEnabledModuleStates`
    /// enables the profile's saved non-HUD modules and turns off the rest.
    pub(super) fn apply_profile_states(&mut self, name: &str) -> Result<(), String> {
        let name = self.find_profile(name)?.ok_or("Unknown profile")?;
        let document = if self.active_profile().as_deref() == Some(name.as_str()) {
            self.capture_active()?;
            self.profile_snapshot()
        } else {
            self.storage
                .as_ref()
                .ok_or("Local storage is unavailable")?
                .load(&name)?
        };
        let suppress = std::mem::replace(&mut self.suppress_toggle_alerts, true);
        let mut enabled = 0;
        for module in ModuleId::ALL {
            if module == ModuleId::Panic || hud_module(module) {
                continue;
            }
            let next = document.0["modules"][module.id()]["enabled"]
                .as_bool()
                .unwrap_or(false)
                && self.mode(module) != BindingMode::Hold;
            self.set_enabled(module, next);
            enabled += usize::from(next);
        }
        self.suppress_toggle_alerts = suppress;
        self.profile_alert(&name, enabled);
        Ok(())
    }
    /// A profile's keybind (`Profile` is a `Bendable`); the reference's
    /// profile bind input refuses the left and right mouse buttons.
    pub(super) fn bind_profile(&mut self, name: &str, keys: Vec<String>) -> Result<(), String> {
        let name = self.find_profile(name)?.ok_or("Unknown profile")?;
        let keys: Vec<String> = keys
            .into_iter()
            .filter(|k| k != "MouseLeft" && k != "MouseRight")
            .collect();
        if !self.document.0["profile_binds"].is_object() {
            self.document.0["profile_binds"] = json!({});
        }
        let binds = self.document.0["profile_binds"].as_object_mut().unwrap();
        if keys.is_empty() {
            binds.remove(&name);
        } else {
            binds.insert(name, json!(keys));
        }
        self.dirty = true;
        Ok(())
    }
    /// `Profile.onBindActivated` on the press that completes its chord.
    pub(super) fn profile_bind_pressed(&self, key: &str) -> Option<String> {
        self.profile_binds().into_iter().find_map(|(name, keys)| {
            (keys.iter().any(|k| k == key) && keys.iter().all(|k| self.held_keys.contains(k)))
                .then_some(name)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::input::KeyAction;
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "vape-profile-runtime-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn seed(&self, current: Value) -> &Self {
            std::fs::write(self.0.join("current.json"), current.to_string()).unwrap();
            self
        }
        fn runtime(&self) -> Runtime {
            let mut runtime = Runtime::default();
            runtime.attach_storage(self.0.clone());
            runtime
        }
        fn file(&self, name: &str) -> Value {
            serde_json::from_slice(&std::fs::read(self.0.join("profiles").join(name)).unwrap())
                .unwrap()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn on(runtime: &Runtime, module: ModuleId) -> bool {
        runtime.enabled(module)
    }

    #[test]
    fn a_fresh_install_seeds_the_builtins_and_selects_modern_pvp() {
        let scratch = Scratch::new();
        let mut r = scratch.runtime();
        assert_eq!(r.active_profile().as_deref(), Some("Modern PVP"));
        assert_eq!(r.ordered_profiles().unwrap(), ["Modern PVP", "Classic PVP"]);
        let modern = scratch.file("Modern%20PVP.json");
        assert_eq!(
            modern["modules"]["combat.trigger-bot"]["favorite"],
            json!(true)
        );
        assert_eq!(
            modern["modules"]["combat.auto-clicker"]["favorite"],
            Value::Null
        );
        let classic = scratch.file("Classic%20PVP.json");
        assert_eq!(
            classic["modules"]["combat.auto-clicker"]["favorite"],
            json!(true)
        );
        r.flush().unwrap();
        let current: Value =
            serde_json::from_slice(&std::fs::read(scratch.0.join("current.json")).unwrap())
                .unwrap();
        assert_eq!(current["profile"], json!("Modern PVP"));
        assert_eq!(
            current["profile_order"],
            json!(["Modern PVP", "Classic PVP"])
        );
        // Profile files carry neither the index nor the friend lists.
        assert!(scratch.file("Modern%20PVP.json").get("profile").is_none());
        assert!(scratch.file("Modern%20PVP.json").get("friends").is_none());
    }

    #[test]
    fn a_pre_profile_live_state_keeps_its_modules_under_a_profile() {
        let scratch = Scratch::new();
        scratch.seed(
            json!({"schema": 1, "modules": {"hud.coordinates": {"enabled": true},
            "blatant.fly": {"enabled": true, "favorite": true}}}),
        );
        let r = scratch.runtime();
        // No profiles yet: the live state becomes Modern PVP with its own
        // favorites, and the second built-in is seeded beside it.
        assert_eq!(r.active_profile().as_deref(), Some("Modern PVP"));
        assert!(on(&r, ModuleId::Coordinates) && on(&r, ModuleId::Fly));
        let modern = scratch.file("Modern%20PVP.json");
        assert_eq!(modern["modules"]["blatant.fly"]["favorite"], json!(true));
        assert_eq!(
            modern["modules"]["combat.trigger-bot"]["favorite"],
            Value::Null
        );

        // With profiles already on disk, it migrates to "Default".
        let scratch = Scratch::new();
        scratch.seed(json!({"schema": 1, "modules": {"blatant.fly": {"enabled": true}}}));
        std::fs::create_dir(scratch.0.join("profiles")).unwrap();
        std::fs::write(scratch.0.join("profiles/one.json"), br#"{"schema":1}"#).unwrap();
        std::fs::write(scratch.0.join("profiles/Default.json"), br#"{"schema":1}"#).unwrap();
        let r = scratch.runtime();
        assert_eq!(r.active_profile().as_deref(), Some("Default 2"));
        assert_eq!(
            r.ordered_profiles().unwrap(),
            ["Default 2", "Default", "one"]
        );
        assert!(on(&r, ModuleId::Fly));
        assert_eq!(
            scratch.file("Default%202.json")["modules"]["blatant.fly"]["enabled"],
            json!(true)
        );
    }

    #[test]
    fn switching_captures_the_active_profile_and_keeps_global_values() {
        let scratch = Scratch::new();
        let mut r = scratch.runtime();
        r.set_enabled(ModuleId::Fly, true);
        r.set_setting("blatant.fly.speed", json!(3.0)).ok();
        r.set_setting("client.toggle-alerts", json!(true)).unwrap();
        r.set_setting("client.profile-alerts", json!(true)).unwrap();
        r.switch_profile("classic pvp").unwrap();
        assert_eq!(r.active_profile().as_deref(), Some("Classic PVP"));
        assert!(!on(&r, ModuleId::Fly));
        // Global values survive; per-profile ones reset to the target's.
        assert_eq!(r.settings().get("client.toggle-alerts"), Some(&json!(true)));
        let modern = scratch.file("Modern%20PVP.json");
        assert_eq!(modern["modules"]["blatant.fly"]["enabled"], json!(true));
        // Toggle alerts stay quiet; the switch shows one profile alert.
        let toasts: Vec<_> = r.toasts.iter().map(|t| t.title.clone()).collect();
        assert_eq!(toasts, ["Profile swap to \u{a7}6Classic PVP"]);
        r.switch_profile("Modern PVP").unwrap();
        assert!(on(&r, ModuleId::Fly));
        assert_eq!(r.toasts.iter().last().unwrap().message, "1 modules enabled");
        // Switching to the active profile does nothing.
        let before = r.toasts.iter().count();
        r.switch_profile("Modern PVP").unwrap();
        assert_eq!(r.toasts.iter().count(), before);
        assert!(r.switch_profile("missing").is_err());
    }

    #[test]
    fn without_auto_load_a_switch_keeps_states_and_startup_restores_only_hud() {
        let scratch = Scratch::new();
        let mut r = scratch.runtime();
        r.set_setting("client.profile-auto-load", json!(false))
            .unwrap();
        r.set_enabled(ModuleId::Fly, true);
        r.set_enabled(ModuleId::Coordinates, true);
        r.switch_profile("Classic PVP").unwrap();
        assert!(on(&r, ModuleId::Fly) && on(&r, ModuleId::Coordinates));
        r.flush().unwrap();
        drop(r);
        let r = scratch.runtime();
        assert!(!on(&r, ModuleId::Fly));
        assert!(on(&r, ModuleId::Coordinates));
    }

    #[test]
    fn create_rename_delete_follow_the_profiles_page() {
        let scratch = Scratch::new();
        let mut r = scratch.runtime();
        r.set_enabled(ModuleId::Fly, true);
        r.create_profile("  Mine  ").unwrap();
        assert_eq!(r.active_profile().as_deref(), Some("Mine"));
        assert_eq!(
            r.ordered_profiles().unwrap(),
            ["Mine", "Modern PVP", "Classic PVP"]
        );
        assert_eq!(
            scratch.file("Mine.json")["modules"]["blatant.fly"]["enabled"],
            json!(true)
        );
        r.create_profile("mine").unwrap();
        r.create_profile("   ").unwrap();
        assert_eq!(r.ordered_profiles().unwrap().len(), 3);
        assert!(r.create_profile(&"x".repeat(MAX_NAME + 1)).is_err());
        r.create_profile(&"x".repeat(MAX_NAME)).unwrap();
        r.switch_profile("Mine").unwrap();

        assert_eq!(
            r.delete_profile("Mine").unwrap_err(),
            "You cannot delete your selected profile"
        );
        r.bind_profile("Mine", vec!["KeyK".into(), "MouseLeft".into()])
            .unwrap();
        assert_eq!(
            r.rename_profile("Mine", "classic pvp").unwrap_err(),
            "profile already exists"
        );
        r.rename_profile("Mine", "Ours").unwrap();
        assert_eq!(r.active_profile().as_deref(), Some("Ours"));
        assert_eq!(r.ordered_profiles().unwrap()[1], "Ours");
        assert_eq!(
            r.profile_binds(),
            [("Ours".to_string(), vec!["KeyK".to_string()])]
        );
        r.delete_profile("Classic PVP").unwrap();
        assert!(!scratch.0.join("profiles/Classic%20PVP.json").exists());
    }

    #[test]
    fn a_profile_bind_switches_on_its_chord_and_right_click_applies_states() {
        let scratch = Scratch::new();
        let mut r = scratch.runtime();
        r.bind_profile("Classic PVP", vec!["ControlLeft".into(), "KeyK".into()])
            .unwrap();
        r.input("KeyK", KeyAction::Press, true);
        r.input("KeyK", KeyAction::Release, true);
        assert_eq!(r.active_profile().as_deref(), Some("Modern PVP"));
        r.input("ControlLeft", KeyAction::Press, true);
        r.input("KeyK", KeyAction::Press, false);
        assert_eq!(r.active_profile().as_deref(), Some("Modern PVP"));
        r.input("KeyK", KeyAction::Release, true);
        r.input("KeyK", KeyAction::Press, true);
        assert_eq!(r.active_profile().as_deref(), Some("Classic PVP"));
        r.bind_profile("Classic PVP", vec![]).unwrap();
        assert!(r.profile_binds().is_empty());

        // Right click: the saved states of another profile, values kept.
        // Classic PVP is active, so it follows the live state too.
        r.set_enabled(ModuleId::Fly, true);
        r.create_profile("Flying").unwrap();
        r.switch_profile("Modern PVP").unwrap();
        assert!(!on(&r, ModuleId::Fly));
        r.set_enabled(ModuleId::AimAssist, true);
        r.apply_profile_states("Flying").unwrap();
        assert_eq!(r.active_profile().as_deref(), Some("Modern PVP"));
        assert!(on(&r, ModuleId::Fly) && !on(&r, ModuleId::AimAssist));
    }

    #[test]
    fn names_are_trimmed_bounded_and_need_a_file_name() {
        assert_eq!(clean_name("  a b ").unwrap().as_deref(), Some("a b"));
        assert_eq!(clean_name("  ").unwrap(), None);
        assert!(clean_name(&"y".repeat(MAX_NAME + 1)).is_err());
        assert!(clean_name("bad\nname").is_err());
    }
}
