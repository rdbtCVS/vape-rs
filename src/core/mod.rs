pub mod profiles;
pub mod settings;

use super::input::{BindingMode, KeyAction, MultiBinding};
use super::ui::gui::{Action, Frame, Gui, ModuleView, PersonView, View};
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
    pub gui: Gui,
    suppress_game_input: bool,
    gui_bind: MultiBinding,
    pub ui_screen_owned: bool,
    profile_names: Vec<String>,
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
        settings
            .define(Definition::boolean(
                "client.multi-bind",
                "Enable Multi-Keybinding",
                false,
            ))
            .unwrap();
        settings
            .define(Definition::new(
                "client.gui.binds",
                "GUI binding",
                Kind::StringList {
                    max_entries: 3,
                    max_chars: 64,
                },
                json!(["ShiftRight"]),
            ))
            .unwrap();
        settings
            .define(Definition::number(
                "client.gui-scale",
                "GUI scale",
                0.5,
                1.0,
                2.0,
                0.05,
            ))
            .unwrap();
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
            gui: Gui::default(),
            suppress_game_input: false,
            gui_bind: MultiBinding::default(),
            ui_screen_owned: false,
            profile_names: Vec::new(),
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
    fn key_named(
        &mut self,
        module: ModuleId,
        key: &str,
        action: KeyAction,
        accepts_input: bool,
    ) -> bool {
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
            return true;
        }
        accepts_input && action != KeyAction::Release && bind.complete(required)
    }
    pub fn suppress_game_input(&self) -> bool {
        self.suppress_game_input
    }
    pub fn input(&mut self, key: &str, action: KeyAction, accepts_input: bool) {
        let was_open = self.gui.open;
        self.suppress_game_input = false;
        let gui_keys = self
            .settings
            .get("client.gui.binds")
            .and_then(Value::as_array)
            .unwrap();
        let gui_toggle =
            if !self.gui.capturing() && gui_keys.iter().any(|v| v.as_str() == Some(key)) {
                self.gui_bind.apply(
                    key,
                    action,
                    BindingMode::Toggle,
                    was_open,
                    accepts_input || was_open,
                    gui_keys.len(),
                )
            } else {
                None
            };
        if gui_toggle.is_some() {
            if was_open {
                self.gui.close();
            } else if accepts_input {
                self.release_input();
                self.gui.open = true;
                self.profile_names = self.profiles().unwrap_or_default();
            }
        } else if key == "F10"
            && action == KeyAction::Press
            && (accepts_input || was_open)
            && !self.gui.capturing()
        {
            self.panic();
            self.gui.close();
            self.suppress_game_input = true;
        } else if was_open {
            let actions = self.gui.key(key, action);
            self.perform(actions);
        } else {
            for module in ModuleId::ALL {
                if self
                    .settings
                    .get(&format!("{}.binds", module.id()))
                    .and_then(Value::as_array)
                    .is_some_and(|keys| keys.iter().any(|k| k.as_str() == Some(key)))
                {
                    self.suppress_game_input |= self.key_named(module, key, action, accepts_input);
                }
            }
        }
        if action != KeyAction::Release && (was_open || self.gui.open) {
            self.suppress_game_input = true;
        }
        // Releases must still reach the host's key mapping cleanup.
        if action == KeyAction::Release {
            self.suppress_game_input = false;
        }
    }
    fn perform(&mut self, actions: Vec<Action>) {
        for action in actions {
            let refresh_profiles = matches!(
                &action,
                Action::Rename(..) | Action::Delete(..) | Action::Import(..)
            );
            let result: Result<(), String> = match action {
                Action::Toggle(id) => {
                    if let Some(module) = ModuleId::ALL.into_iter().find(|m| m.id() == id) {
                        self.set_enabled(module, !self.enabled(module));
                        Ok(())
                    } else {
                        Err("Unknown module".into())
                    }
                }
                Action::Set(id, value) => self.set_setting(&id, value),
                Action::Favorite(id) => {
                    let mut document = self.snapshot();
                    let state = document.0["modules"]
                        .as_object_mut()
                        .unwrap()
                        .entry(id)
                        .or_insert_with(|| json!({}));
                    state["favorite"] = json!(!state["favorite"].as_bool().unwrap_or(false));
                    self.document = document;
                    self.dirty = true;
                    Ok(())
                }
                Action::Save(name) => self.save_profile(&name),
                Action::Load(name) => self.load_profile(&name),
                Action::Rename(old, new) => {
                    let result = self
                        .storage
                        .as_ref()
                        .ok_or_else(|| "Local storage unavailable".to_string())
                        .and_then(|s| s.rename(&old, &new));
                    if result.is_ok() {
                        self.gui.select_profile(&new);
                        self.notify(format!("Renamed profile to {new}"));
                    }
                    result
                }
                Action::Delete(name) => self
                    .storage
                    .as_ref()
                    .ok_or_else(|| "Local storage unavailable".to_string())
                    .and_then(|s| s.delete(&name)),
                Action::Import(name) => self
                    .storage
                    .as_ref()
                    .ok_or_else(|| "Local storage unavailable".to_string())
                    .and_then(|s| s.import_file(&name)),
                Action::Export(name) => self
                    .storage
                    .as_ref()
                    .ok_or_else(|| "Local storage unavailable".to_string())
                    .and_then(|s| s.export_file(&name)),
                Action::UpsertPerson(relation, name, alias) => {
                    self.upsert_person(&relation, &name, &alias)
                }
                Action::RemovePerson(relation, name) => self.remove_person(&relation, &name),
                Action::ToggleTarget(relation, name) => self.toggle_person_target(&relation, &name),
                Action::Window(x, y) => {
                    self.document.0["ui"]["window"] = json!([x, y]);
                    self.dirty = true;
                    Ok(())
                }
                Action::Close => {
                    self.gui.close();
                    Ok(())
                }
            };
            if result.is_ok() && refresh_profiles {
                self.profile_names = self.profiles().unwrap_or_default();
            }
            if let Err(error) = result {
                self.notify(error);
            }
        }
    }
    fn people_list_mut(&mut self, relation: &str) -> Result<&mut Vec<Value>, String> {
        if !matches!(relation, "friends" | "enemies") {
            return Err("Unknown local relation".into());
        }
        self.document.0[relation]
            .as_array_mut()
            .ok_or_else(|| "Invalid local people list".into())
    }
    pub fn upsert_person(&mut self, relation: &str, name: &str, alias: &str) -> Result<(), String> {
        let name = name.trim();
        let alias = if alias.is_empty() { name } else { alias };
        if name.is_empty()
            || name.chars().count() > 64
            || name.chars().any(char::is_control)
            || alias.chars().count() > 64
            || alias.chars().any(char::is_control)
        {
            return Err("Name and alias must be 1–64 printable characters".into());
        }
        let entries = self.people_list_mut(relation)?;
        if let Some(entry) = entries.iter_mut().find(|v| {
            v["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) {
            entry["name"] = json!(name);
            entry["alias"] = json!(alias);
        } else {
            if entries.len() >= 4096 {
                return Err("Local people list is full".into());
            }
            entries.push(if relation == "friends" {
                json!({"name":name,"alias":alias,"target":true})
            } else {
                json!({"name":name,"alias":alias,"exclusive":false})
            });
        }
        self.dirty = true;
        Ok(())
    }
    pub fn remove_person(&mut self, relation: &str, name: &str) -> Result<(), String> {
        self.people_list_mut(relation)?.retain(|v| {
            !v["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
        self.dirty = true;
        Ok(())
    }
    pub fn toggle_person_target(&mut self, relation: &str, name: &str) -> Result<(), String> {
        if let Some(entry) = self.people_list_mut(relation)?.iter_mut().find(|v| {
            v["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) {
            let key = if relation == "friends" {
                "target"
            } else {
                "exclusive"
            };
            entry[key] = json!(!entry[key].as_bool().unwrap_or(relation == "friends"));
        }
        self.dirty = true;
        Ok(())
    }
    pub fn people(&self) -> Vec<PersonView> {
        let mut people = Vec::new();
        for relation in ["friends", "enemies"] {
            if let Some(entries) = self.document.0[relation].as_array() {
                for entry in entries {
                    if let Some(name) = entry["name"].as_str() {
                        people.push(PersonView {
                            relation: relation.into(),
                            name: name.into(),
                            alias: entry["alias"].as_str().unwrap_or(name).into(),
                            targeted: entry[if relation == "friends" {
                                "target"
                            } else {
                                "exclusive"
                            }]
                            .as_bool()
                            .unwrap_or(relation == "friends"),
                        });
                    }
                }
            }
        }
        people.sort_by_key(|p| (p.relation.clone(), p.name.to_ascii_lowercase()));
        people
    }
    pub fn gui_view(&self) -> View {
        View {
            modules: ModuleId::ALL
                .into_iter()
                .map(|m| ModuleView {
                    id: m.id().into(),
                    name: m.name().into(),
                    category: "Legit".into(),
                    enabled: self.enabled(m),
                    favorite: self.document.0["modules"][m.id()]["favorite"]
                        .as_bool()
                        .unwrap_or(false),
                })
                .collect(),
            definitions: self
                .settings
                .definitions()
                .filter(|d| self.settings.visible(&d.id))
                .cloned()
                .collect(),
            values: self.settings.known_values(),
            profiles: self.profile_names.clone(),
            people: self.people(),
        }
    }
    pub fn gui_frame(&mut self, viewport: (f32, f32), cursor: (f32, f32)) -> Option<Frame> {
        if !self.gui.open {
            return None;
        }
        self.gui.update_viewport(viewport);
        let actions = self.gui.pointer(cursor);
        self.perform(actions);
        self.gui.multi_bind = self
            .settings
            .get("client.multi-bind")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.gui.user_scale = self
            .settings
            .get("client.gui-scale")
            .and_then(Value::as_f64)
            .unwrap_or(1.0) as f32;
        Some(self.gui.frame(&self.gui_view(), viewport))
    }
    pub fn gui_pointer(&mut self, cursor: (f32, f32)) {
        let actions = self.gui.pointer(cursor);
        self.perform(actions);
    }
    pub fn gui_wheel(&mut self, delta: f32, cursor: (f32, f32)) {
        if self.gui.hud_editor {
            if let Some(region) = self
                .gui
                .hud_regions
                .iter()
                .find(|r| r.rect.contains(cursor))
            {
                let id = format!("{}.scale", region.id);
                let current = self
                    .settings
                    .get(&id)
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0);
                if let Err(error) = self.set_setting(&id, json!(current + f64::from(delta) * 0.05))
                {
                    self.notify(error);
                }
            }
        } else {
            self.gui.wheel(delta);
        }
    }
    pub fn gui_mouse(&mut self, button: &str, pressed: bool, cursor: (f32, f32)) {
        if !self.gui.open {
            return;
        }
        let actions = self.gui.mouse(button, pressed, cursor, &self.gui_view());
        self.perform(actions);
    }
    pub fn release_input(&mut self) {
        self.gui_bind.apply(
            "",
            KeyAction::FocusLost,
            BindingMode::Toggle,
            self.gui.open,
            false,
            0,
        );
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
        self.gui.status = message.clone();
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
                self.profile_names = self.profiles().unwrap_or_default();
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
        self.gui.restore_window(&document.0["ui"]);
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
        self.profile_names = self.profiles().unwrap_or_default();
        self.gui.select_profile(name);
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
        self.gui.select_profile(name);
        self.notify(format!("Loaded profile {name}"));
        Ok(())
    }
    pub fn profiles(&self) -> Result<Vec<String>, String> {
        self.storage
            .as_ref()
            .ok_or("Local storage is unavailable")?
            .list()
    }
    pub fn frame(&mut self) {
        if self.dirty
            && !self.persistence_blocked
            && let Err(error) = self.flush()
        {
            self.persistence_blocked = true;
            self.notify(format!("Profile save failed: {error}"));
        }
    }
    pub fn tick(&mut self) -> bool {
        let first = self.ticks == 0;
        self.ticks = self.ticks.saturating_add(1);
        self.frame();
        first
    }
    pub fn panels(&self, snapshot: Snapshot) -> Vec<HudPanel> {
        self.panels_for(snapshot, false)
    }
    pub fn preview_panels(&self, snapshot: Snapshot) -> Vec<HudPanel> {
        self.panels_for(snapshot, true)
    }
    fn panels_for(&self, snapshot: Snapshot, preview: bool) -> Vec<HudPanel> {
        if !snapshot.in_world || !snapshot.show_hud {
            return Vec::new();
        }
        let mut panels = Vec::new();
        for module in ModuleId::ALL {
            if !preview && !self.enabled(module) {
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
                name: module.name(),
                enabled: self.enabled(module),
                min_width: if module == ModuleId::Fps {
                    50.0
                } else if self
                    .settings
                    .get("hud.coordinates.display")
                    .and_then(Value::as_str)
                    == Some("vertical")
                {
                    70.0
                } else {
                    0.0
                },
                min_height: if module == ModuleId::Fps {
                    20.0
                } else if self
                    .settings
                    .get("hud.coordinates.display")
                    .and_then(Value::as_str)
                    == Some("vertical")
                {
                    90.0
                } else {
                    35.0
                },
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
    fn gui_captures_presses_allows_host_release_cleanup_and_panic_unwinds_it() {
        let mut runtime = Runtime::default();
        runtime.input("ShiftRight", KeyAction::Press, true);
        assert!(runtime.gui.open);
        assert!(runtime.suppress_game_input());
        runtime.input("KeyW", KeyAction::Press, false);
        assert!(runtime.suppress_game_input());
        runtime.input("KeyW", KeyAction::Release, false);
        assert!(!runtime.suppress_game_input());
        runtime.set_enabled(ModuleId::Fps, true);
        runtime.input("F10", KeyAction::Press, false);
        assert!(!runtime.gui.open);
        assert!(!runtime.enabled(ModuleId::Fps));
        assert!(runtime.suppress_game_input());
    }
    #[test]
    fn local_people_edits_deduplicate_by_name_and_preserve_extensions() {
        let mut runtime = Runtime::default();
        runtime.apply_document(Document::parse(br#"{"schema":1,"friends":[{"name":"Player","alias":"Old","target":true,"future":4}]}"#,false).unwrap());
        runtime.upsert_person("friends", "PLAYER", "New").unwrap();
        assert_eq!(runtime.people().len(), 1);
        assert_eq!(runtime.snapshot().0["friends"][0]["future"], json!(4));
        runtime.toggle_person_target("friends", "player").unwrap();
        assert!(!runtime.people()[0].targeted);
        runtime.upsert_person("enemies", "Enemy", "Alias").unwrap();
        assert_eq!(runtime.people().len(), 2);
        runtime.remove_person("friends", "player").unwrap();
        assert_eq!(runtime.people().len(), 1);
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
