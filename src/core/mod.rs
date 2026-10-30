pub mod automation;
pub mod inventory_runtime;
pub mod lifecycle;
pub mod macro_runtime;
pub mod movement_input;
pub mod profiles;
mod registry;
pub mod settings;

pub use registry::ModuleId;

use super::actions::{Coordinator, HotbarSlot, Lease, Owner, Resource, SlotChange};
use super::input::{BindingMode, KeyAction, MultiBinding};
use super::modules::utility::{
    auto_tool,
    inventory_guard::{CombatGuard, CombatSnapshot},
};
use super::ui::gui::{Action, Frame, Gui, ModuleView, PersonView, View};
use super::ui::{CoordinatesLayout, HudLine, HudPanel, Snapshot};
use lifecycle::{CleanupReason, Context, Session};
use profiles::{Document, Profiles};
use serde_json::{Value, json};
use settings::{Definition, Kind, Settings};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
};

#[derive(Debug)]
pub struct ToolAction {
    pub change: SlotChange,
    decision: auto_tool::Decision,
    options: auto_tool::Options,
}

#[derive(Debug, Default)]
struct ModuleState {
    enabled: bool,
    binding: MultiBinding,
}

#[derive(Debug)]
pub struct Runtime {
    modules: BTreeMap<ModuleId, ModuleState>,
    panic_restore: Option<Vec<ModuleId>>,
    panic_transition: bool,
    ticks: u64,
    session: Session,
    actions: Coordinator,
    auto_tool: auto_tool::AutoTool,
    auto_fish: super::modules::utility::auto_fish::AutoFish,
    auto_fish_gate: super::inventory::planner::Gate,
    fish_hooked_entity: Option<(u32, i32)>,
    macro_use_lease: Option<Lease>,
    auto_armor: super::modules::utility::auto_armor::AutoArmor,
    auto_totem: super::modules::utility::auto_totem::AutoTotem,
    refill: super::modules::utility::refill::Refill,
    chest_steal: super::modules::utility::chest_steal::ChestSteal,
    tool_lease: Option<Lease>,
    combat_guard: CombatGuard,
    combat_guard_tick: Option<u64>,
    combat_blocked: bool,
    macro_engine: super::input::macros::MacroEngine,
    macro_context: super::input::macros::MacroContext,
    macro_pending: Vec<super::input::macros::MacroEffect>,
    macro_origin: std::time::Instant,
    macro_lease: Option<Lease>,
    macro_selected: Option<u8>,
    inventory_schedule: inventory_runtime::Scheduler,
    settings: Settings,
    document: Document,
    storage: Option<Profiles>,
    initialized: bool,
    dirty: bool,
    persistence_blocked: bool,
    notes: VecDeque<String>,
    gamma: super::modules::render::fullbright::Gamma,
    brightness_reset: bool,
    /// The single writer of synthetic movement keys for this client.
    movement: movement_input::Writer,
    parkour_tapped: bool,
    aim_assist: super::modules::combat::aim_assist::AimAssist,
    aim_clock: Option<std::time::Instant>,
    trigger_bot: super::modules::combat::trigger_bot::TriggerBot,
    velocity: super::modules::combat::velocity::Velocity,
    left_clicker: super::modules::combat::clicker::Engine,
    right_clicker: super::modules::combat::clicker::Engine,
    reach: super::modules::combat::reach::Reach,
    no_fall: super::modules::blatant::no_fall::NoFall,
    no_slowdown: super::modules::blatant::no_slowdown::NoSlowdown,
    /// A clicker was reset while holding its mapping down: [attack, use].
    clicker_release: [bool; 2],
    motion_override: Option<([f64; 3], [f64; 3])>,
    explosion_reentry: bool,
    hud_state: super::ui::hud_state::HudState,
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
                    json!(module.default_binds()),
                ),
                Definition::choice(
                    &format!("{prefix}.activation"),
                    "Activation",
                    &["toggle", "hold"],
                    "toggle",
                ),
            ] {
                settings
                    .define(definition)
                    .expect("static module definition");
            }
            if !module.is_panel() {
                continue;
            }
            for definition in [
                Definition::boolean(&format!("{prefix}.background"), "Render background", true),
                Definition::number(
                    &format!("{prefix}.x"),
                    "X",
                    0.0,
                    module.position().0,
                    8192.0,
                    1.0,
                ),
                Definition::number(
                    &format!("{prefix}.y"),
                    "Y",
                    0.0,
                    module.position().1,
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
        for definition in [
            Definition::choice(
                "hud.clock.type",
                "Clock Type",
                &["analog", "digital"],
                "analog",
            ),
            Definition::boolean("hud.clock.date", "Show date", true)
                .when("hud.clock.type", json!("digital")),
            Definition::boolean("hud.clock.24-hour", "24 Hour Time", false),
            Definition::choice(
                "hud.keystrokes.key-style",
                "Key Style",
                &["keyboard", "arrow"],
                "keyboard",
            ),
            Definition::choice(
                "hud.keystrokes.mouse-style",
                "Mouse Style",
                &["button", "icon"],
                "button",
            ),
            Definition::boolean("hud.keystrokes.space", "Show Spacebar", true),
            Definition::boolean("hud.keystrokes.cps-only", "Show CPS Only", false),
            Definition::boolean("hud.armor.compact", "Compact", false),
            Definition::boolean("hud.potions.positive", "Show Positive Effects", true),
            Definition::boolean("hud.potions.negative", "Show Negative Effects", true),
        ] {
            settings
                .define(definition)
                .expect("static widget definition");
        }
        for definition in [
            Definition::number(
                "world.auto-tool.swap-to-delay",
                "Swap to delay",
                0.0,
                50.0,
                500.0,
                50.0,
            ),
            Definition::boolean("world.auto-tool.swap-weapon", "Swap weapon", true),
            Definition::boolean("world.auto-tool.instant-swap", "Instant swap", true)
                .when("world.auto-tool.swap-weapon", json!(true)),
            Definition::boolean("world.auto-tool.swap-back", "Swap back", false),
            Definition::number(
                "world.auto-tool.swap-back-delay",
                "Swap back delay",
                50.0,
                350.0,
                1000.0,
                50.0,
            )
            .when("world.auto-tool.swap-back", json!(true)),
            Definition::boolean("world.auto-tool.require-mouse", "Require mouse down", true),
            Definition::boolean(
                "world.auto-tool.only-sneaking",
                "Only while sneaking",
                false,
            ),
            Definition::boolean("world.auto-tool.not-combat", "Not during combat", false),
            Definition::boolean("world.auto-tool.not-use", "Not during use", false),
            Definition::boolean("friends.use", "Use Friends", true),
        ] {
            settings
                .define(definition)
                .expect("static AutoTool definition");
        }
        settings
            .define(Definition::boolean(
                "combat.sprint.cancel-invis",
                "Cancel Invis",
                false,
            ))
            .unwrap();
        settings
            .define(Definition::choice(
                "world.fast-place.held-item",
                "Held Item",
                &["all", "blocks", "projectiles"],
                "all",
            ))
            .unwrap();
        settings
            .define(Definition::number(
                "world.fast-place.delay",
                "Delay",
                0.0,
                1.0,
                4.0,
                1.0,
            ))
            .unwrap();
        settings
            .define(Definition::choice(
                "render.fullbright.mode",
                "Mode",
                &["night-vision", "gamma"],
                "night-vision",
            ))
            .unwrap();
        settings
            .define(
                Definition::boolean("render.fullbright.fade", "Fade", false)
                    .when("render.fullbright.mode", json!("gamma")),
            )
            .unwrap();
        for definition in [
            Definition::boolean("utility.auto-fish.recast-ground", "Recast ground", false),
            Definition::boolean("utility.auto-fish.recast-caught", "Recast caught", false),
            Definition::boolean("utility.auto-fish.refill-rods", "Refill rods", true),
            Definition::new(
                "utility.auto-fish.click-delay",
                "Click delay",
                Kind::Range {
                    min: 50.0,
                    max: 200.0,
                    step: 5.0,
                },
                json!([75.0, 125.0]),
            )
            .when("utility.auto-fish.refill-rods", json!(true)),
        ] {
            settings.define(definition).unwrap();
        }
        for definition in super::modules::combat::velocity::definitions() {
            settings.define(definition).unwrap();
        }
        for definition in super::modules::combat::clicker::left_definitions()
            .into_iter()
            .chain(super::modules::combat::clicker::right_definitions())
            .chain(super::modules::combat::reach::definitions())
            .chain(super::modules::blatant::definitions())
        {
            settings.define(definition).unwrap();
        }
        for definition in super::modules::combat::trigger_bot::definitions() {
            settings.define(definition).unwrap();
        }
        for definition in super::modules::combat::aim_assist::definitions() {
            settings.define(definition).unwrap();
        }
        for definition in super::modules::render::storage_esp::definitions() {
            settings.define(definition).unwrap();
        }
        for definition in inventory_runtime::definitions() {
            settings
                .define(definition)
                .expect("static inventory definition");
        }
        settings
            .define(Definition::boolean(
                "world.safe-walk.direction-check",
                "Direction Check",
                true,
            ))
            .unwrap();
        settings
            .define(Definition::boolean(
                "utility.panic.re-enable",
                "Re-enable",
                false,
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
            modules: ModuleId::ALL
                .into_iter()
                .map(|id| (id, ModuleState::default()))
                .collect(),
            panic_restore: None,
            panic_transition: false,
            ticks: 0,
            session: Session::default(),
            actions: Coordinator::default(),
            auto_tool: auto_tool::AutoTool::default(),
            auto_fish: Default::default(),
            auto_fish_gate: Default::default(),
            fish_hooked_entity: None,
            macro_use_lease: None,
            auto_armor: Default::default(),
            auto_totem: Default::default(),
            refill: Default::default(),
            chest_steal: Default::default(),
            tool_lease: None,
            combat_guard: CombatGuard::default(),
            combat_guard_tick: None,
            combat_blocked: false,
            macro_engine: Default::default(),
            macro_context: Default::default(),
            macro_pending: Vec::new(),
            macro_origin: std::time::Instant::now(),
            macro_lease: None,
            macro_selected: None,
            inventory_schedule: Default::default(),
            settings,
            document: Document::default(),
            storage: None,
            initialized: false,
            dirty: false,
            persistence_blocked: false,
            notes: VecDeque::new(),
            gamma: Default::default(),
            brightness_reset: false,
            movement: movement_input::Writer::default(),
            parkour_tapped: false,
            aim_assist: Default::default(),
            aim_clock: None,
            trigger_bot: Default::default(),
            velocity: Default::default(),
            left_clicker: super::modules::combat::clicker::Engine::new(0x5851_f42d_4c95_7f2d),
            right_clicker: super::modules::combat::clicker::Engine::new(0x2c1b_3c6d_4a1f_9e37),
            reach: Default::default(),
            no_fall: Default::default(),
            no_slowdown: Default::default(),
            clicker_release: [false; 2],
            motion_override: None,
            explosion_reentry: false,
            hud_state: super::ui::hud_state::HudState::default(),
            gui: Gui::default(),
            suppress_game_input: false,
            gui_bind: MultiBinding::default(),
            ui_screen_owned: false,
            profile_names: Vec::new(),
        }
    }
}
impl Runtime {
    /// Parkour taps jump for one tick at a time; returns the previous tick's tap.
    pub fn replace_parkour_tap(&mut self, tapped: bool) -> bool {
        std::mem::replace(&mut self.parkour_tapped, tapped)
    }
    pub fn movement(&mut self) -> &mut movement_input::Writer {
        &mut self.movement
    }
    pub fn enabled(&self, module: ModuleId) -> bool {
        self.modules[&module].enabled
    }
    pub fn set_enabled(&mut self, module: ModuleId, enabled: bool) {
        if module == ModuleId::Panic {
            if enabled {
                self.panic();
            }
            return;
        }
        if self.enabled(module) == enabled {
            return;
        }
        if !self.panic_transition {
            self.panic_restore = None;
        }
        self.modules
            .get_mut(&module)
            .expect("registered module")
            .enabled = enabled;
        if matches!(
            module,
            ModuleId::AutoArmor | ModuleId::AutoTotem | ModuleId::Refill | ModuleId::ChestSteal
        ) && !enabled
        {
            self.cancel_inventory();
        }
        if module == ModuleId::AutoTool && !enabled {
            self.reset_auto_tool();
        }
        if module == ModuleId::AutoFish && !enabled {
            self.cancel_auto_fish();
        }
        if !enabled {
            match module {
                ModuleId::AimAssist => self.reset_aim_assist(),
                ModuleId::TriggerBot => {
                    self.trigger_bot.reset();
                }
                ModuleId::Velocity => self.reset_velocity(),
                ModuleId::AutoClicker => self.clicker_release[0] |= self.left_clicker.reset(),
                ModuleId::RightClicker => self.clicker_release[1] |= self.right_clicker.reset(),
                ModuleId::Reach => self.reach.reset(),
                ModuleId::NoFall => self.no_fall.reset(),
                ModuleId::NoSlowdown => self.no_slowdown.reset(),
                _ => {}
            }
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
        if id.starts_with("utility.auto-fish.") {
            self.cancel_auto_fish();
        }
        if id.starts_with("inventory.") || id.starts_with("world.chest-steal.") {
            self.cancel_inventory();
        }
        if id.starts_with("world.auto-tool.") {
            self.reset_auto_tool();
        }
        if id.starts_with("combat.aim-assist.") {
            self.reset_aim_assist();
        }
        if id.starts_with("combat.trigger-bot.") {
            self.trigger_bot.reset();
        }
        if id.starts_with("combat.velocity.") {
            self.reset_velocity();
        }
        if id.starts_with("combat.auto-clicker.") {
            self.clicker_release[0] |= self.left_clicker.reset();
        }
        if id.starts_with("combat.right-clicker.") {
            self.clicker_release[1] |= self.right_clicker.reset();
        }
        if id.starts_with("combat.reach.") {
            self.reach.reset();
        }
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
        let bind = &mut self
            .modules
            .get_mut(&module)
            .expect("registered module")
            .binding;
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
        if !was_open && accepts_input && action == KeyAction::Press {
            if key == "MouseLeft" {
                self.hud_state.click(0);
            } else if key == "MouseRight" {
                self.hud_state.click(1);
            }
        }
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
        } else if was_open
            && !self.gui.capturing()
            && self
                .settings
                .get("utility.panic.binds")
                .and_then(Value::as_array)
                .is_some_and(|keys| keys.iter().any(|v| v.as_str() == Some(key)))
        {
            self.suppress_game_input = self.key_named(ModuleId::Panic, key, action, true);
            if action == KeyAction::Press {
                self.gui.close();
            }
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
        self.macro_input(
            key,
            action,
            accepts_input && !self.gui.open && !self.suppress_game_input,
        );
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
                Action::Layout(frames) => {
                    self.document.0["ui"]["frames"] = frames;
                    self.dirty = true;
                    Ok(())
                }
                Action::Sync => {
                    self.dirty = true;
                    self.flush()
                        .map(|()| self.notify("Configuration saved".to_string()))
                }
                Action::UpsertMacro(definition) => self.upsert_macro(definition),
                Action::RemoveMacro(name) => self.remove_macro(&name),
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
                    category: m.category().into(),
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
            macros: self.macros().to_vec(),
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
        // Rescaling mid-drag moves the slider under the cursor, so apply on release.
        if !self.gui.dragging_setting("client.gui-scale") {
            self.gui.user_scale = self
                .settings
                .get("client.gui-scale")
                .and_then(Value::as_f64)
                .unwrap_or(1.0) as f32;
        }
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
            self.gui.wheel(delta, cursor);
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
        self.cancel_macros(true);
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
    pub fn logic_tick(&self) -> u64 {
        self.ticks
    }
    fn reset_aim_assist(&mut self) {
        self.aim_assist.reset();
        self.aim_clock = None;
    }
    /// Disable, setting change and session cleanup for the combat modules.
    /// TriggerBot's synthetic press is a click, so releasing it needs no
    /// host action.
    fn reset_combat(&mut self) {
        self.reset_aim_assist();
        self.trigger_bot.reset();
        self.reset_velocity();
        self.clicker_release[0] |= self.left_clicker.reset();
        self.clicker_release[1] |= self.right_clicker.reset();
        self.reach.reset();
        self.no_fall.reset();
        self.no_slowdown.reset();
    }
    fn reset_velocity(&mut self) {
        self.velocity.reset();
        self.motion_override = None;
    }
    /// The replacement for the local player's last motion packet, and the
    /// motion that packet set.
    pub fn set_motion_override(&mut self, replacement: [f64; 3], received: [f64; 3]) {
        self.motion_override = Some((replacement, received));
    }
    pub fn take_motion_override(&mut self) -> Option<([f64; 3], [f64; 3])> {
        self.motion_override.take()
    }
    /// Marks the next explosion as Velocity's own re-dispatch.
    pub fn set_explosion_reentry(&mut self) {
        self.explosion_reentry = true;
    }
    pub fn explosion_reentry_pending(&self) -> bool {
        self.explosion_reentry
    }
    pub fn take_explosion_reentry(&mut self) -> bool {
        std::mem::take(&mut self.explosion_reentry)
    }
    pub fn velocity_active(&mut self) -> bool {
        let active = self.enabled(ModuleId::Velocity) && self.session.in_world();
        if !active {
            self.reset_velocity();
        }
        active
    }
    pub fn velocity_motion(
        &mut self,
        facing: super::modules::combat::velocity::Facing,
        water: bool,
        motion: [f64; 3],
    ) -> super::modules::combat::velocity::Decision {
        let options = super::modules::combat::velocity::Options::read(&self.settings);
        self.velocity.entity_motion(&options, facing, water, motion)
    }
    pub fn velocity_explosion(
        &mut self,
        facing: super::modules::combat::velocity::Facing,
        water: bool,
        push: [f64; 3],
    ) -> super::modules::combat::velocity::Decision {
        let options = super::modules::combat::velocity::Options::read(&self.settings);
        self.velocity.explosion(&options, facing, water, push)
    }
    pub fn velocity_tick(&mut self, water: bool, velocity: [f64; 3]) -> Option<[f64; 3]> {
        if !self.velocity_active() {
            return None;
        }
        let options = super::modules::combat::velocity::Options::read(&self.settings);
        self.velocity.tick(&options, water, velocity)
    }
    /// One frame of a clicker (`right` for RightClicker). Inactive clickers
    /// reset; the actions include any release a reset left pending.
    pub fn clicker_frame(
        &mut self,
        right: bool,
        input: &super::modules::combat::clicker::Input,
    ) -> super::modules::combat::clicker::Actions {
        use super::modules::combat::clicker::{Actions, LEFT, Options, RIGHT};
        let (module, prefix) = if right {
            (ModuleId::RightClicker, RIGHT)
        } else {
            (ModuleId::AutoClicker, LEFT)
        };
        let active = self.enabled(module) && self.session.interactive();
        let slot = usize::from(right);
        let engine = if right {
            &mut self.right_clicker
        } else {
            &mut self.left_clicker
        };
        let mut out = if active {
            engine.frame(&Options::read(&self.settings, prefix), input)
        } else {
            self.clicker_release[slot] |= engine.reset();
            Actions::default()
        };
        out.release |= std::mem::take(&mut self.clicker_release[slot]);
        out
    }
    /// The clickers' pre-tick jitter step.
    pub fn clicker_tick(&mut self) {
        use super::modules::combat::clicker::{LEFT, Options, RIGHT};
        if self.enabled(ModuleId::AutoClicker) {
            self.left_clicker.tick(&Options::read(&self.settings, LEFT));
        }
        if self.enabled(ModuleId::RightClicker) {
            self.right_clicker
                .tick(&Options::read(&self.settings, RIGHT));
        }
    }
    /// Reach's pre-tick target trace (Advanced mode).
    pub fn reach_tick(
        &mut self,
        player: super::modules::combat::reach::Player,
        traced: Option<i32>,
    ) {
        if !self.enabled(ModuleId::Reach) || !self.session.in_world() {
            self.reach.reset();
            return;
        }
        let options = super::modules::combat::reach::Options::read(&self.settings);
        self.reach.pre_tick(&options, player, traced);
    }
    pub fn reach_post_attack(&mut self) {
        if self.enabled(ModuleId::Reach) {
            let options = super::modules::combat::reach::Options::read(&self.settings);
            self.reach.post_attack(&options);
        }
    }
    /// The entity pick range and box expansion, or None for the host's own.
    pub fn pick_range(
        &mut self,
        player: super::modules::combat::reach::Player,
    ) -> Option<(f64, f64)> {
        let reach = self.enabled(ModuleId::Reach);
        let hitboxes = self.enabled(ModuleId::HitBoxes);
        if !reach && !hitboxes {
            return None;
        }
        let range = if reach {
            let options = super::modules::combat::reach::Options::read(&self.settings);
            self.reach.distance(&options, player)
        } else {
            super::modules::combat::reach::BASE
        };
        let expand = if hitboxes {
            super::modules::blatant::hitbox_expansion(&self.settings)
        } else {
            0.0
        };
        Some((range, expand))
    }
    /// Timer's speed for the game clock (1.0 when off or out of a world).
    pub fn timer_speed(&self) -> f32 {
        if self.enabled(ModuleId::Timer) && self.session.in_world() {
            super::modules::blatant::timer_speed(&self.settings)
        } else {
            1.0
        }
    }
    /// Fly's replacement move this tick, when enabled.
    pub fn fly_move(&self, input: &super::modules::blatant::fly::Input) -> Option<[f64; 3]> {
        if !self.enabled(ModuleId::Fly) || !self.session.in_world() {
            return None;
        }
        let options = super::modules::blatant::fly::Options::read(&self.settings);
        Some(super::modules::blatant::fly::movement(&options, input))
    }
    /// NoFall's pre-motion step; `skip` covers dead, creative and flying.
    pub fn no_fall_motion(
        &mut self,
        skip: bool,
        fall_distance: f32,
        motion_y: f64,
    ) -> super::modules::blatant::no_fall::Effects {
        if !self.enabled(ModuleId::NoFall) || !self.session.in_world() {
            self.no_fall.reset();
            return Default::default();
        }
        let mode = super::modules::blatant::no_fall::Mode::read(&self.settings);
        let skip = skip || self.enabled(ModuleId::Fly);
        self.no_fall.pre_motion(mode, skip, fall_distance, motion_y)
    }
    pub fn no_slowdown_knockback(&mut self) {
        if self.enabled(ModuleId::NoSlowdown) {
            self.no_slowdown.knockback();
        }
    }
    pub fn no_slowdown_tick(&mut self, tick: &super::modules::blatant::no_slowdown::Tick) -> bool {
        if !self.enabled(ModuleId::NoSlowdown) || !self.session.in_world() {
            self.no_slowdown.reset();
            return false;
        }
        let options = super::modules::blatant::no_slowdown::Options::read(&self.settings);
        self.no_slowdown.tick(&options, tick)
    }
    /// Monotonic milliseconds for module timers.
    pub fn now_ms(&self) -> u64 {
        self.macro_origin.elapsed().as_millis() as u64
    }
    pub fn trigger_bot_active(&mut self) -> bool {
        let active = self.enabled(ModuleId::TriggerBot) && self.session.interactive();
        if !active {
            self.trigger_bot.reset();
        }
        active
    }
    pub fn trigger_bot_tick(
        &mut self,
        tick: &super::modules::combat::trigger_bot::Tick,
    ) -> super::modules::combat::trigger_bot::Effects {
        let options = super::modules::combat::trigger_bot::Options::read(&self.settings);
        let people = self.target_people();
        self.trigger_bot.tick(&options, tick, &people)
    }
    /// The attack button changed; true swallows the activation click.
    pub fn trigger_bot_input(
        &mut self,
        tick: &super::modules::combat::trigger_bot::Tick,
        pressed: bool,
    ) -> bool {
        if !self.enabled(ModuleId::TriggerBot) {
            return false;
        }
        let options = super::modules::combat::trigger_bot::Options::read(&self.settings);
        let people = self.target_people();
        self.trigger_bot.input(&options, tick, pressed, &people)
    }
    /// The local friend (when "Use Friends" is on) and enemy name lists.
    pub fn target_people(&self) -> super::targeting::filter::NameLists {
        let names = |relation: &str| -> Vec<String> {
            self.document.0[relation]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| p["target"].as_bool().unwrap_or(true))
                .filter_map(|p| p["name"].as_str().map(str::to_owned))
                .collect()
        };
        super::targeting::filter::NameLists {
            friends: self.friend_keys(),
            enemies: names("enemies"),
        }
    }
    /// Whether AimAssist needs a host snapshot this frame.
    pub fn aim_assist_active(&mut self) -> bool {
        let active = self.enabled(ModuleId::AimAssist) && self.session.interactive();
        if !active {
            self.reset_aim_assist();
        }
        active
    }
    /// One render frame of AimAssist; returns the mouse delta for the host's
    /// `Entity.turn`. Elapsed time comes from the runtime's monotonic clock.
    pub fn aim_assist_frame(
        &mut self,
        mut frame: super::modules::combat::aim_assist::Frame,
    ) -> (f64, f64) {
        let now = std::time::Instant::now();
        frame.elapsed_ms = self
            .aim_clock
            .map_or(0, |at| now.duration_since(at).as_millis().min(1000) as u32);
        // Keep the sub-millisecond remainder for the next frame.
        self.aim_clock = Some(match self.aim_clock {
            Some(at) => at + std::time::Duration::from_millis(u64::from(frame.elapsed_ms)),
            None => now,
        });
        let options = super::modules::combat::aim_assist::Options::read(&self.settings);
        let people = self.target_people();
        self.aim_assist.frame(&options, &frame, &people)
    }
    pub fn friend_keys(&self) -> Vec<String> {
        if !self
            .settings
            .get("friends.use")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            return Vec::new();
        }
        self.document.0["friends"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["target"].as_bool().unwrap_or(true))
            .filter_map(|p| p["name"].as_str().map(str::to_owned))
            .collect()
    }
    fn reset_auto_tool(&mut self) {
        self.auto_tool.reset();
        self.tool_lease = None;
        self.actions.release_owner(Owner("world.auto-tool"));
    }
    fn auto_tool_options(&self) -> auto_tool::Options {
        let boolean = |name: &str| {
            self.settings
                .get(&format!("world.auto-tool.{name}"))
                .and_then(Value::as_bool)
                .unwrap()
        };
        let ticks = |name: &str| {
            (self
                .settings
                .get(&format!("world.auto-tool.{name}"))
                .and_then(Value::as_f64)
                .unwrap()
                / 50.0)
                .ceil() as u64
        };
        auto_tool::Options {
            swap_weapon: boolean("swap-weapon"),
            instant_weapon: boolean("instant-swap"),
            swap_to_ticks: ticks("swap-to-delay"),
            swap_back: boolean("swap-back"),
            swap_back_ticks: ticks("swap-back-delay"),
            require_attack: boolean("require-mouse"),
            only_sneaking: boolean("only-sneaking"),
            not_during_use: boolean("not-use"),
            not_during_combat: boolean("not-combat"),
        }
    }
    pub fn update_combat(&mut self, combat: CombatSnapshot) {
        if self.combat_guard_tick != Some(self.ticks) {
            self.combat_blocked = self.combat_guard.update(combat);
            self.combat_guard_tick = Some(self.ticks);
        }
    }
    pub fn auto_tool_propose(
        &mut self,
        mut snapshot: auto_tool::Snapshot,
        combat: CombatSnapshot,
    ) -> Option<ToolAction> {
        if !self.enabled(ModuleId::AutoTool)
            || !self.session.interactive()
            || snapshot.epoch != self.session.epoch()
        {
            self.reset_auto_tool();
            return None;
        }
        if let Some(auto_tool::Target::Entity(handle)) = snapshot.target
            && !self.session.owns(handle)
        {
            self.reset_auto_tool();
            return None;
        }
        let options = self.auto_tool_options();
        if options.not_during_combat {
            if self.combat_guard_tick != Some(snapshot.tick) {
                self.combat_blocked = self.combat_guard.update(combat);
                self.combat_guard_tick = Some(snapshot.tick);
            }
            snapshot.combat_blocked = self.combat_blocked;
        }
        if self
            .tool_lease
            .is_some_and(|lease| !self.actions.owns(lease, self.session.epoch()))
        {
            self.auto_tool.interrupt(snapshot.target);
            self.tool_lease = None;
        }
        let decision = self.auto_tool.step(&snapshot, options);
        if !self.auto_tool.retains_claim()
            && let Some(lease) = self.tool_lease.take()
        {
            self.actions.release(lease);
        }
        let decision = decision?;
        let lease = self.actions.acquire(
            Resource::Hotbar,
            Owner("world.auto-tool"),
            self.session.epoch(),
            100,
        )?;
        self.tool_lease = Some(lease);
        Some(ToolAction {
            change: SlotChange {
                lease,
                expected: decision.expected,
                selected: decision.selected,
                item: decision.item.clone(),
            },
            decision,
            options,
        })
    }
    pub fn auto_tool_authorize(&self, action: &ToolAction, current: HotbarSlot) -> bool {
        self.enabled(ModuleId::AutoTool)
            && self.session.interactive()
            && self
                .actions
                .authorize(&action.change, self.session.epoch(), current)
    }
    pub fn auto_tool_applied(&mut self, action: ToolAction) {
        if !self.actions.owns(action.change.lease, self.session.epoch()) {
            return;
        }
        self.auto_tool.applied(&action.decision, action.options);
        if !self.auto_tool.retains_claim() {
            self.actions.release(action.change.lease);
            self.tool_lease = None;
        }
    }
    pub fn session(&self) -> &Session {
        &self.session
    }
    pub fn observe_context(&mut self, context: Context) {
        if let Some(reason) = self.session.observe(context) {
            self.session_change(reason);
        }
        if !self.session.interactive() {
            self.release_input();
            self.reset_auto_tool();
        }
    }
    pub fn session_change(&mut self, reason: CleanupReason) {
        if !matches!(&reason, CleanupReason::Panic) {
            self.panic_restore = None;
        }
        self.release_input();
        if !matches!(reason, CleanupReason::Panic | CleanupReason::Profile) {
            self.macro_pending.retain(|effect| {
                matches!(
                    effect,
                    super::input::macros::MacroEffect::UseItem { down: false }
                )
            });
        }
        self.reset_auto_tool();
        self.reset_combat();
        self.cancel_inventory();
        self.cancel_auto_fish();
        self.combat_guard.reset();
        self.combat_guard_tick = None;
        self.combat_blocked = false;
        self.fish_hooked_entity = None;
        self.actions.clear();
        self.hud_state.reset_session();
        self.brightness_reset = true;
        if reason.closes_gui() {
            self.gui.close();
        }
        let epoch = self.session.advance();
        if self.notes.len() == 32 {
            self.notes.pop_front();
        }
        self.notes.push_back(format!(
            "session epoch {}: {}",
            epoch.value(),
            reason.label()
        ));
    }
    pub fn panic(&mut self) {
        let re_enable = self
            .settings
            .get("utility.panic.re-enable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let restore = if re_enable {
            self.panic_restore.take()
        } else {
            None
        };
        let active: Vec<_> = ModuleId::ALL
            .into_iter()
            .filter(|&m| m != ModuleId::Panic && self.enabled(m))
            .collect();
        self.session_change(CleanupReason::Panic);
        self.panic_transition = true;
        if let Some(modules) = restore {
            for module in modules {
                self.set_enabled(module, true);
            }
        } else {
            for module in &active {
                self.set_enabled(*module, false);
            }
            if re_enable {
                self.panic_restore = Some(active);
            }
        }
        self.panic_transition = false;
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
        self.panic_restore = None;
        self.session_change(CleanupReason::Profile);
        for warning in self.settings.load(&document.0["settings"]) {
            self.notify(warning);
        }
        for module in ModuleId::ALL {
            let enabled = document.0["modules"][module.id()]["enabled"]
                .as_bool()
                .unwrap_or(false);
            if module != ModuleId::Panic {
                self.set_enabled(module, enabled && self.mode(module) != BindingMode::Hold);
            }
        }
        if let Err(error) = self
            .macro_engine
            .load_json(document.0.get("macros").unwrap_or(&json!([])))
        {
            self.notify(format!("Macros were not loaded: {error}"));
        }
        self.gui.restore_layout(&document.0["ui"]);
        self.document = document;
        self.dirty = true;
    }
    pub fn snapshot(&self) -> Document {
        let mut document = self.document.clone();
        document.0["settings"] = self.settings.snapshot();
        document.0["macros"] = self.macro_engine.to_json();
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
    pub fn fullbright_night_vision(&self) -> bool {
        self.enabled(ModuleId::Fullbright)
            && self
                .settings
                .get("render.fullbright.mode")
                .and_then(Value::as_str)
                == Some("night-vision")
    }
    pub fn fullbright_gamma(&mut self, base: f32, in_world: bool) -> Option<f32> {
        let gamma_mode = self
            .settings
            .get("render.fullbright.mode")
            .and_then(Value::as_str)
            == Some("gamma");
        let fade = self
            .settings
            .get("render.fullbright.fade")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let reset = std::mem::take(&mut self.brightness_reset);
        self.gamma.update(
            self.enabled(ModuleId::Fullbright) && in_world && !reset,
            gamma_mode,
            fade,
            base,
            reset || !in_world,
        )
    }
    pub fn update_hud(&mut self, snapshot: &Snapshot) {
        self.hud_state.update(snapshot);
    }
    pub fn register_synthetic_click(&mut self, button: usize) {
        self.hud_state.click(button);
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
            if !module.is_panel() {
                continue;
            }
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
                ModuleId::Compass if !snapshot.yaw.is_finite() => continue,
                _ => Vec::new(),
            };
            if lines.is_empty() && matches!(module, ModuleId::Coordinates | ModuleId::Fps) {
                continue;
            }
            let value = |suffix: &str| {
                self.settings
                    .get(&format!("{}.{suffix}", module.id()))
                    .unwrap()
            };
            let content = match module {
                ModuleId::Coordinates => super::ui::hud::Content::Coordinates {
                    layout: if self
                        .settings
                        .get("hud.coordinates.display")
                        .and_then(Value::as_str)
                        == Some("vertical")
                    {
                        CoordinatesLayout::Vertical
                    } else {
                        CoordinatesLayout::Horizontal
                    },
                    position: snapshot.position,
                    yaw: snapshot.yaw,
                    biome: snapshot.biome.clone(),
                },
                ModuleId::Compass => super::ui::hud::Content::Compass { yaw: snapshot.yaw },
                ModuleId::Fps => super::ui::hud::Content::Lines,
                ModuleId::Clock => super::ui::hud::Content::Clock {
                    time: snapshot.clock,
                    digital: self.settings.get("hud.clock.type").and_then(Value::as_str)
                        == Some("digital"),
                    show_date: self
                        .settings
                        .get("hud.clock.date")
                        .and_then(Value::as_bool)
                        .unwrap(),
                    twenty_four: self
                        .settings
                        .get("hud.clock.24-hour")
                        .and_then(Value::as_bool)
                        .unwrap(),
                },
                ModuleId::Keystrokes => super::ui::hud::Content::Keystrokes {
                    keys: snapshot.keys,
                    mouse: snapshot.mouse,
                    cps: self.hud_state.cps(),
                    arrows: self
                        .settings
                        .get("hud.keystrokes.key-style")
                        .and_then(Value::as_str)
                        == Some("arrow"),
                    icons: self
                        .settings
                        .get("hud.keystrokes.mouse-style")
                        .and_then(Value::as_str)
                        == Some("icon"),
                    space: self
                        .settings
                        .get("hud.keystrokes.space")
                        .and_then(Value::as_bool)
                        .unwrap(),
                    cps_only: self
                        .settings
                        .get("hud.keystrokes.cps-only")
                        .and_then(Value::as_bool)
                        .unwrap(),
                },
                ModuleId::ArmorStatus => {
                    let pieces = self.hud_state.armor(&snapshot, preview);
                    if pieces.is_empty() {
                        continue;
                    }
                    super::ui::hud::Content::Armor {
                        pieces,
                        compact: self
                            .settings
                            .get("hud.armor.compact")
                            .and_then(Value::as_bool)
                            .unwrap(),
                    }
                }
                ModuleId::PotionEffects => {
                    let effects = self
                        .hud_state
                        .effects(&snapshot, preview)
                        .into_iter()
                        .filter(|e| {
                            self.settings
                                .get(if e.negative {
                                    "hud.potions.negative"
                                } else {
                                    "hud.potions.positive"
                                })
                                .and_then(Value::as_bool)
                                .unwrap()
                        })
                        .collect::<Vec<_>>();
                    if effects.is_empty() {
                        continue;
                    }
                    super::ui::hud::Content::Potions { effects }
                }
                ModuleId::NoHurtCamera
                | ModuleId::AutoTool
                | ModuleId::AutoArmor
                | ModuleId::AutoTotem
                | ModuleId::Refill
                | ModuleId::ChestSteal
                | ModuleId::FastPlace
                | ModuleId::Fullbright
                | ModuleId::ClearWater
                | ModuleId::AutoFish
                | ModuleId::NoJumpDelay
                | ModuleId::NoClickDelay
                | ModuleId::Sprint
                | ModuleId::Panic
                | ModuleId::StorageEsp
                | ModuleId::Parkour
                | ModuleId::AimAssist
                | ModuleId::SafeWalk
                | ModuleId::TriggerBot
                | ModuleId::Velocity
                | ModuleId::AutoClicker
                | ModuleId::RightClicker
                | ModuleId::Reach
                | ModuleId::HitBoxes
                | ModuleId::Timer
                | ModuleId::Fly
                | ModuleId::NoFall
                | ModuleId::NoSlowdown => continue,
            };
            use super::ui::hud::Content;
            let (min_width, min_height) = match &content {
                Content::Clock {
                    digital,
                    show_date,
                    twenty_four,
                    ..
                } => (
                    if *digital {
                        70.0 + if *show_date { 24.0 } else { 0.0 }
                            + if *twenty_four { 0.0 } else { 12.0 }
                    } else {
                        70.0
                    },
                    if *digital { 32.0 } else { 65.0 },
                ),
                Content::Keystrokes {
                    icons,
                    space,
                    cps_only,
                    ..
                } => (
                    if *cps_only {
                        75.0
                    } else if *icons {
                        102.0
                    } else {
                        54.0
                    },
                    if *cps_only {
                        20.0
                    } else {
                        (if *icons { 40.0 } else { 72.0 }) + if *space { 14.0 } else { 0.0 }
                    },
                ),
                Content::Armor { pieces, compact } => (
                    if *compact { 26.0 } else { 54.0 },
                    2.0 + pieces.len() as f32 * if *compact { 24.0 } else { 20.0 },
                ),
                Content::Potions { effects } => (40.0, 2.0 + effects.len() as f32 * 30.0),
                Content::Compass { .. } => (308.0, 30.0),
                Content::Coordinates {
                    layout: CoordinatesLayout::Vertical,
                    ..
                } => (70.0, 90.0),
                Content::Coordinates { .. } => (0.0, 35.0),
                Content::Lines => (50.0, 20.0),
            };
            panels.push(HudPanel {
                content,
                module: module.id(),
                name: module.name(),
                enabled: self.enabled(module),
                min_width,
                min_height,
                x: value("x").as_f64().unwrap() as f32,
                y: value("y").as_f64().unwrap() as f32,
                scale: value("scale").as_f64().unwrap() as f32,
                color: value("color").as_u64().unwrap() as u32,
                background: value("background").as_bool().unwrap()
                    && !(module == ModuleId::Keystrokes
                        && self
                            .settings
                            .get("hud.keystrokes.cps-only")
                            .and_then(Value::as_bool)
                            .unwrap()),
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
/// Owned by one host Game. A temporary handle clone lets adapters release the
/// field borrow before applying host effects; no process/thread singleton exists.
pub type Client = std::sync::Arc<std::sync::Mutex<Runtime>>;
pub fn with_client<T>(client: &Client, action: impl FnOnce(&mut Runtime) -> T) -> T {
    action(&mut client.lock().expect("vape-rs client runtime poisoned"))
}
/// `with_client` for host queries that may run while the runtime is already
/// held on this thread (a pick inside an adapter): None instead of blocking.
pub fn try_with_client<T>(client: &Client, action: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    match client.try_lock() {
        Ok(mut runtime) => Some(action(&mut runtime)),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_owner_is_per_client_and_cancel_invis_defaults_off() {
        let mut first = Runtime::default();
        let mut second = Runtime::default();
        assert!(!first.enabled(ModuleId::Sprint));
        assert_eq!(
            first.settings().get("combat.sprint.cancel-invis"),
            Some(&json!(false))
        );
        use movement_input::{Axis, HostView, Overrides, Owner, Request};
        let mut requests = Overrides::default();
        requests.request(Request {
            axis: Axis::Sprint,
            owner: Owner::Sprint,
            priority: 1,
            pressed: true,
        });
        let view = HostView::default();
        assert_eq!(
            first.movement().resolve(&requests, &view),
            [(Axis::Sprint, true)]
        );
        assert!(first.movement().owns(Axis::Sprint));
        assert!(!second.movement().owns(Axis::Sprint));
    }

    #[test]
    fn client_ownership_is_independent_and_temporary_handles_preserve_it() {
        let a = Client::default();
        let b = Client::default();
        with_client(&a, |runtime| {
            runtime.set_enabled(ModuleId::Coordinates, true)
        });
        assert!(!with_client(&b, |runtime| runtime.enabled(ModuleId::Coordinates)));
        let moved = a.clone();
        drop(a);
        assert!(with_client(&moved, |runtime| runtime.enabled(ModuleId::Coordinates)));
        with_client(&moved, |runtime| runtime.panic());
        assert!(!with_client(&moved, |runtime| runtime.enabled(ModuleId::Coordinates)));
    }

    #[test]
    fn session_transition_releases_held_modules_without_erasing_toggle_preferences() {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime
            .set_setting("hud.fps.activation", json!("hold"))
            .unwrap();
        runtime.set_enabled(ModuleId::Coordinates, true);
        runtime.key(ModuleId::Fps, KeyAction::Press, true);
        assert!(runtime.enabled(ModuleId::Fps));
        runtime.gui.open = true;
        let before = runtime.session().epoch();
        runtime.session_change(CleanupReason::Dimension);
        assert!(!runtime.enabled(ModuleId::Fps));
        assert!(runtime.enabled(ModuleId::Coordinates));
        assert!(!runtime.gui.open);
        assert_ne!(before, runtime.session().epoch());
        runtime.key(ModuleId::Fps, KeyAction::Release, true);
        assert!(!runtime.enabled(ModuleId::Fps));
    }

    #[test]
    fn auto_tool_actions_are_revoked_by_session_change_and_user_selection() {
        let mut runtime = Runtime::default();
        runtime.observe_context(Context {
            in_world: true,
            alive: true,
            interactive: true,
        });
        runtime.set_enabled(ModuleId::AutoTool, true);
        runtime
            .set_setting("world.auto-tool.swap-to-delay", json!(0.0))
            .unwrap();
        let mut snapshot = auto_tool::Snapshot {
            epoch: runtime.session().epoch(),
            tick: 1,
            selected: HotbarSlot::new(0).unwrap(),
            target: Some(auto_tool::Target::Block([1, 64, 1])),
            candidates: std::array::from_fn(|_| auto_tool::Candidate {
                item: None,
                score: 1.0,
            }),
            attack_down: true,
            sneaking: false,
            using_item: false,
            combat_blocked: false,
        };
        snapshot.candidates[2] = auto_tool::Candidate {
            item: Some("minecraft:diamond_pickaxe".into()),
            score: 8.0,
        };
        let action = runtime
            .auto_tool_propose(snapshot.clone(), CombatSnapshot::default())
            .unwrap();
        assert!(runtime.auto_tool_authorize(&action, snapshot.selected));
        assert!(!runtime.auto_tool_authorize(&action, HotbarSlot::new(1).unwrap()));
        runtime.session_change(CleanupReason::Dimension);
        assert!(!runtime.auto_tool_authorize(&action, snapshot.selected));
        assert!(
            runtime
                .auto_tool_propose(snapshot.clone(), CombatSnapshot::default())
                .is_none()
        );
        snapshot.epoch = runtime.session().epoch();
        let action = runtime
            .auto_tool_propose(snapshot, CombatSnapshot::default())
            .unwrap();
        runtime.panic();
        assert!(!runtime.auto_tool_authorize(&action, HotbarSlot::new(0).unwrap()));
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            position: [-1.5, 64.2, 10.9],
            fps: 143.8,
            yaw: 0.0,
            biome: "Forest".into(),
            in_world: true,
            show_hud: true,
            ..Snapshot::default()
        }
    }

    #[test]
    fn panic_reenable_restores_only_until_another_module_changes() {
        let mut runtime = Runtime::default();
        runtime
            .set_setting("utility.panic.re-enable", json!(true))
            .unwrap();
        runtime.set_enabled(ModuleId::Fps, true);
        runtime.panic();
        assert!(!runtime.enabled(ModuleId::Fps));
        runtime.panic();
        assert!(runtime.enabled(ModuleId::Fps));
        runtime.panic();
        runtime.set_enabled(ModuleId::Coordinates, true);
        runtime.panic();
        assert!(!runtime.enabled(ModuleId::Coordinates));
        assert!(!runtime.enabled(ModuleId::Fps));
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
        assert!(runtime.hud(data.clone()).is_empty());
        data.in_world = true;
        data.show_hud = false;
        assert!(runtime.hud(data.clone()).is_empty());
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
    fn combat_module_cleanup_is_per_module() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::Velocity, true);
        runtime.set_motion_override([0.0; 3], [0.4, 0.3, 0.4]);
        // A disabled AimAssist resets itself every frame; Velocity keeps its state.
        assert!(!runtime.aim_assist_active());
        assert!(!runtime.trigger_bot_active());
        runtime.set_enabled(ModuleId::TriggerBot, true);
        runtime.set_enabled(ModuleId::TriggerBot, false);
        assert_eq!(
            runtime.take_motion_override(),
            Some(([0.0; 3], [0.4, 0.3, 0.4]))
        );
        runtime.set_motion_override([0.0; 3], [0.4, 0.3, 0.4]);
        runtime.set_enabled(ModuleId::Velocity, false);
        assert_eq!(runtime.take_motion_override(), None);
        runtime.set_enabled(ModuleId::Velocity, true);
        runtime.set_motion_override([0.0; 3], [0.4, 0.3, 0.4]);
        runtime.session_change(CleanupReason::OpenWorld);
        assert_eq!(runtime.take_motion_override(), None);
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
