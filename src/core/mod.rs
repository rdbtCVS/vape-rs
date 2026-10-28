pub mod profiles;
pub mod settings;

use super::input::{BindingMode, KeyAction, MultiBinding};
use super::ui::{CoordinatesLayout, HudLine, HudPanel, Snapshot};
use profiles::{Document, Profiles};
use serde_json::{Value, json};
use settings::{Definition, Kind, Settings};
use std::{collections::VecDeque, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleId {
    Coordinates,
    Fps,
}
impl ModuleId {
    pub fn id(self) -> &'static str {
        match self {
            Self::Coordinates => "hud.coordinates",
            Self::Fps => "hud.fps",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Coordinates => "Coords",
            Self::Fps => "FPS",
        }
    }
    pub const ALL: [Self; 2] = [Self::Coordinates, Self::Fps];
}

#[derive(Debug)]
pub struct Runtime {
    coordinates: bool,
    fps: bool,
    coordinates_bind: MultiBinding,
    fps_bind: MultiBinding,
    ticks: u64,
    settings: Settings,
    document: Document,
    storage: Option<Profiles>,
    initialized: bool,
    dirty: bool,
    persistence_blocked: bool,
    notes: VecDeque<String>,
}
impl Default for Runtime {
    fn default() -> Self {
        let mut settings = Settings::default();
        for module in ModuleId::ALL {
            let prefix = module.id();
            for definition in [
                Definition::new(
                    &format!("{prefix}.binds"),
                    "Bindings",
                    Kind::StringList {
                        max_entries: 8,
                        max_chars: 64,
                    },
                    json!([if module == ModuleId::Coordinates {
                        "F8"
                    } else {
                        "F9"
                    }]),
                ),
                Definition::choice(
                    &format!("{prefix}.activation"),
                    "Activation",
                    &["toggle", "hold"],
                    "toggle",
                ),
                Definition::boolean(&format!("{prefix}.background"), "Render background", true),
                Definition::number(&format!("{prefix}.x"), "X", 0.0, 8.0, 8192.0, 1.0),
                Definition::number(
                    &format!("{prefix}.y"),
                    "Y",
                    0.0,
                    if module == ModuleId::Coordinates {
                        8.0
                    } else {
                        36.0
                    },
                    8192.0,
                    1.0,
                ),
                Definition::number(&format!("{prefix}.scale"), "Scale", 0.5, 1.0, 4.0, 0.05),
                Definition::new(
                    &format!("{prefix}.color"),
                    "Color",
                    Kind::Color,
                    json!(0xffffffffu32),
                ),
            ] {
                settings.define(definition).expect("static HUD definition");
            }
        }
        settings
            .define(Definition::choice(
                "hud.coordinates.display",
                "Display Type",
                &["horizontal", "vertical"],
                "horizontal",
            ))
            .unwrap();
        settings
            .validate_conditions()
            .expect("static visibility schema");
        Self {
            coordinates: false,
            fps: false,
            coordinates_bind: MultiBinding::default(),
            fps_bind: MultiBinding::default(),
            ticks: 0,
            settings,
            document: Document::default(),
            storage: None,
            initialized: false,
            dirty: false,
            persistence_blocked: false,
            notes: VecDeque::new(),
        }
    }
}
impl Runtime {
    pub fn enabled(&self, module: ModuleId) -> bool {
        match module {
            ModuleId::Coordinates => self.coordinates,
            ModuleId::Fps => self.fps,
        }
    }
    pub fn set_enabled(&mut self, module: ModuleId, enabled: bool) {
        if self.enabled(module) == enabled {
            return;
        }
        match module {
            ModuleId::Coordinates => self.coordinates = enabled,
            ModuleId::Fps => self.fps = enabled,
        }
        self.dirty = true;
        self.notify(format!(
            "{} {}",
            module.name(),
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    pub fn initialized(&self) -> bool {
        self.initialized
    }
    pub fn set_setting(&mut self, id: &str, value: Value) -> Result<(), String> {
        let next = self.settings.validate_value(id, &value)?;
        if self.settings.get(id) == Some(&next) {
            return Ok(());
        }
        if id.ends_with(".binds") || id.ends_with(".activation") {
            self.release_input();
        }
        self.settings.set(id, next)?;
        self.dirty = true;
        Ok(())
    }
    pub fn set_layout(&mut self, layout: CoordinatesLayout) {
        let _ = self.set_setting(
            "hud.coordinates.display",
            json!(if layout == CoordinatesLayout::Horizontal {
                "horizontal"
            } else {
                "vertical"
            }),
        );
    }
    pub fn key(&mut self, module: ModuleId, action: KeyAction, accepts_input: bool) {
        self.key_named(module, "integration-key", action, accepts_input);
    }
    fn mode(&self, module: ModuleId) -> BindingMode {
        match self
            .settings
            .get(&format!("{}.activation", module.id()))
            .and_then(Value::as_str)
        {
            Some("hold") => BindingMode::Hold,
            Some("release") => BindingMode::Release,
            _ => BindingMode::Toggle,
        }
    }
    fn key_named(&mut self, module: ModuleId, key: &str, action: KeyAction, accepts_input: bool) {
        let enabled = self.enabled(module);
        let mode = self.mode(module);
        let required = self
            .settings
            .get(&format!("{}.binds", module.id()))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let bind = match module {
            ModuleId::Coordinates => &mut self.coordinates_bind,
            ModuleId::Fps => &mut self.fps_bind,
        };
        if let Some(next) = bind.apply(key, action, mode, enabled, accepts_input, required) {
            self.set_enabled(module, next);
        }
    }
    pub fn input(&mut self, key: &str, action: KeyAction, accepts_input: bool) {
        if key == "F10" && action == KeyAction::Press && accepts_input {
            self.panic();
            return;
        }
        for module in ModuleId::ALL {
            if self
                .settings
                .get(&format!("{}.binds", module.id()))
                .and_then(Value::as_array)
                .is_some_and(|keys| keys.iter().any(|k| k.as_str() == Some(key)))
            {
                self.key_named(module, key, action, accepts_input);
            }
        }
    }
    pub fn release_input(&mut self) {
        for module in ModuleId::ALL {
            self.key_named(module, "", KeyAction::FocusLost, false);
        }
    }
    pub fn panic(&mut self) {
        self.release_input();
        for module in ModuleId::ALL {
            self.set_enabled(module, false);
        }
    }
    pub fn notify(&mut self, message: String) {
        if self.notes.len() == 32 {
            self.notes.pop_front();
        }
        self.notes.push_back(message);
    }
    pub fn drain_notes(&mut self) -> Vec<String> {
        self.notes.drain(..).collect()
    }
    pub fn attach_storage(&mut self, root: PathBuf) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        match Profiles::new(root) {
            Ok(storage) => {
                match storage.current() {
                    Ok(Some(document)) => self.apply_document(document),
                    Ok(None) => self.dirty = true,
                    Err(error) => {
                        self.persistence_blocked = true;
                        self.notify(format!(
                            "Current profile was not loaded; preserving its file: {error}"
                        ));
                    }
                }
                self.storage = Some(storage);
            }
            Err(error) => self.notify(format!("Local profile storage unavailable: {error}")),
        }
    }
    pub fn apply_document(&mut self, document: Document) {
        self.release_input();
        for warning in self.settings.load(&document.0["settings"]) {
            self.notify(warning);
        }
        for module in ModuleId::ALL {
            let enabled = document.0["modules"][module.id()]["enabled"]
                .as_bool()
                .unwrap_or(false);
            self.set_enabled(module, enabled && self.mode(module) != BindingMode::Hold);
        }
        self.document = document;
        self.dirty = true;
    }
    pub fn snapshot(&self) -> Document {
        let mut document = self.document.clone();
        document.0["settings"] = self.settings.snapshot();
        for module in ModuleId::ALL {
            let object = document.0["modules"].as_object_mut().unwrap();
            let state = object.entry(module.id()).or_insert_with(|| json!({}));
            state["enabled"] = json!(self.enabled(module));
        }
        document
    }
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if self.persistence_blocked {
            return Err("Current profile is protected after a load/save error; repair the file before autosaving".into());
        }
        if let Some(storage) = &self.storage {
            storage.save_current(&self.snapshot())?;
            self.dirty = false;
        }
        Ok(())
    }
    pub fn save_profile(&mut self, name: &str) -> Result<(), String> {
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .save(name, &self.snapshot())?;
        self.notify(format!("Saved profile {name}"));
        Ok(())
    }
    pub fn load_profile(&mut self, name: &str) -> Result<(), String> {
        let document = self
            .storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .load(name)?;
        self.apply_document(document);
        self.notify(format!("Loaded profile {name}"));
        Ok(())
    }
    pub fn profiles(&self) -> Result<Vec<String>, String> {
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .list()
    }
    pub fn tick(&mut self) -> bool {
        let first = self.ticks == 0;
        self.ticks = self.ticks.saturating_add(1);
        if self.dirty
            && !self.persistence_blocked
            && let Err(error) = self.flush()
        {
            self.persistence_blocked = true;
            self.notify(format!("Profile save failed: {error}"));
        }
        first
    }
    pub fn panels(&self, snapshot: Snapshot) -> Vec<HudPanel> {
        if !snapshot.in_world || !snapshot.show_hud {
            return Vec::new();
        }
        let mut panels = Vec::new();
        for module in ModuleId::ALL {
            if !self.enabled(module) {
                continue;
            }
            let lines = match module {
                ModuleId::Coordinates => {
                    let layout = if self
                        .settings
                        .get("hud.coordinates.display")
                        .and_then(Value::as_str)
                        == Some("vertical")
                    {
                        CoordinatesLayout::Vertical
                    } else {
                        CoordinatesLayout::Horizontal
                    };
                    layout.lines(snapshot.position)
                }
                ModuleId::Fps if snapshot.fps.is_finite() => vec![HudLine {
                    label: String::new(),
                    value: format!("{:.0} FPS", snapshot.fps.max(0.0)),
                }],
                _ => continue,
            };
            if lines.is_empty() {
                continue;
            }
            let value = |suffix: &str| {
                self.settings
                    .get(&format!("{}.{suffix}", module.id()))
                    .unwrap()
            };
            panels.push(HudPanel {
                module: module.id(),
                x: value("x").as_f64().unwrap() as f32,
                y: value("y").as_f64().unwrap() as f32,
                scale: value("scale").as_f64().unwrap() as f32,
                color: value("color").as_u64().unwrap() as u32,
                background: value("background").as_bool().unwrap(),
                lines,
            });
        }
        panels
    }
    pub fn hud(&self, snapshot: Snapshot) -> Vec<HudLine> {
        self.panels(snapshot)
            .into_iter()
            .flat_map(|p| p.lines)
            .collect()
    }
}
std::thread_local! {
    // Adapters reject integrated/dedicated server Game instances before access.
    static CLIENT:std::cell::RefCell<Runtime>=std::cell::RefCell::new(Runtime::default());
}
pub fn with_client<T>(action: impl FnOnce(&mut Runtime) -> T) -> T {
    CLIENT.with_borrow_mut(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            position: [-1.5, 64.2, 10.9],
            fps: 143.8,
            in_world: true,
            show_hud: true,
        }
    }

    #[test]
    fn disabled_by_default_and_panic_clears_all_state() {
        let mut runtime = Runtime::default();
        assert!(runtime.hud(snapshot()).is_empty());
        runtime.key(ModuleId::Coordinates, KeyAction::Press, true);
        runtime.set_enabled(ModuleId::Fps, true);
        assert_eq!(runtime.hud(snapshot()).len(), 2);
        runtime.panic();
        assert!(runtime.hud(snapshot()).is_empty());
        runtime.key(ModuleId::Coordinates, KeyAction::Press, true);
        assert!(runtime.enabled(ModuleId::Coordinates));
    }

    #[test]
    fn screens_and_f1_hide_draws_without_changing_preferences() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::Coordinates, true);
        let mut data = snapshot();
        data.in_world = false;
        assert!(runtime.hud(data).is_empty());
        data.in_world = true;
        data.show_hud = false;
        assert!(runtime.hud(data).is_empty());
        assert!(runtime.enabled(ModuleId::Coordinates));
    }

    #[test]
    fn profile_applies_real_hud_layout_and_preserves_unknown_data() {
        let mut runtime = Runtime::default();
        let document=Document::parse(br#"{"schema":1,"modules":{"hud.coordinates":{"enabled":true},"future.module":{"enabled":true,"x":9}},"settings":{"hud.coordinates.display":"vertical","hud.coordinates.scale":2.0,"future.setting":{"x":4}},"future":{"version":7}}"#,false).unwrap();
        runtime.apply_document(document);
        let panels = runtime.panels(snapshot());
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].lines.len(), 3);
        assert_eq!(panels[0].scale, 2.0);
        let saved = runtime.snapshot();
        assert_eq!(saved.0["future"]["version"], json!(7));
        assert_eq!(saved.0["modules"]["future.module"]["x"], json!(9));
        assert_eq!(saved.0["settings"]["future.setting"]["x"], json!(4));
    }
    #[test]
    fn changing_a_held_bind_cleans_up_using_the_old_activation_mode() {
        let mut runtime = Runtime::default();
        runtime
            .set_setting("hud.coordinates.activation", json!("hold"))
            .unwrap();
        runtime.input("F8", KeyAction::Press, true);
        assert!(runtime.enabled(ModuleId::Coordinates));
        runtime
            .set_setting("hud.coordinates.activation", json!("toggle"))
            .unwrap();
        assert!(!runtime.enabled(ModuleId::Coordinates));
        runtime.input("F8", KeyAction::Press, true);
        assert!(runtime.enabled(ModuleId::Coordinates));
    }
    #[test]
    fn held_profiles_start_inactive_without_a_physical_press() {
        let mut runtime = Runtime::default();
        runtime.apply_document(Document::parse(br#"{"schema":1,"modules":{"hud.fps":{"enabled":true}},"settings":{"hud.fps.activation":"hold","hud.fps.binds":["KeyG","F9"]}}"#,false).unwrap());
        assert!(!runtime.enabled(ModuleId::Fps));
        runtime.input("KeyG", KeyAction::Press, true);
        assert!(!runtime.enabled(ModuleId::Fps));
        runtime.input("F9", KeyAction::Press, true);
        assert!(runtime.enabled(ModuleId::Fps));
        runtime.input("KeyG", KeyAction::Release, true);
        assert!(!runtime.enabled(ModuleId::Fps));
        runtime.input("F9", KeyAction::Release, true);
        assert!(!runtime.enabled(ModuleId::Fps));
    }

    #[test]
    fn tick_diagnostic_only_fires_once() {
        let mut runtime = Runtime::default();
        assert!(runtime.tick());
        assert!(!runtime.tick());
        runtime.panic();
        assert!(!runtime.tick());
    }

    #[test]
    fn independent_runtime_instances_do_not_share_module_state() {
        let mut first = Runtime::default();
        let second = Runtime::default();
        first.set_enabled(ModuleId::Fps, true);
        assert!(!second.enabled(ModuleId::Fps));
    }
}
