// Runtime glue for the session-based arrangement planners (InventoryManager,
// InvCleaner, AutoHotbar, ArmorSwitch). They share the inventory scheduler,
// container claim and transaction gate with the retained planners in
// `inventory_runtime`; this file only maps settings to planner options and
// planner steps to scheduler outcomes.
use super::super::{
    inventory::{
        rules::{Profile, find_profile, parse_presets, parse_profiles},
        session::{Effect, Policy, Step},
        types::Snapshot,
    },
    modules::utility::{
        armor_switch::{ArmorSwitch, Material},
        auto_heal::AutoHeal,
        auto_hotbar::{self, AutoHotbar},
        inv_cleaner::{self, InvCleaner},
        inventory_manager::InventoryManager,
    },
};
use super::inventory_runtime::InventoryEffect;
use super::settings::{Definition, Kind};
use super::{ModuleId, Runtime};
use serde_json::{Value, json};
use std::time::Duration;

pub const MODULES: [ModuleId; 5] = [
    ModuleId::InventoryManager,
    ModuleId::InvCleaner,
    ModuleId::AutoHotbar,
    ModuleId::ArmorSwitch,
    // AutoHeal's bowl handling and Replace (its use sequence lives in
    // `throw_runtime`).
    ModuleId::AutoHeal,
];

#[derive(Debug, Default)]
pub struct Planners {
    manager: InventoryManager,
    cleaner: InvCleaner,
    hotbar: AutoHotbar,
    switch: ArmorSwitch,
    heal: AutoHeal,
    /// Whether the active on-key run issued any click (for "No work
    /// available").
    worked: bool,
}
impl Planners {
    pub fn cancel(&mut self) {
        self.manager.cancel();
        self.cleaner.cancel();
        self.hotbar.cancel();
        self.switch.cancel();
        self.heal.cancel();
        self.worked = false;
    }
    pub fn opened(&self) -> bool {
        self.manager.opened()
            || self.cleaner.opened()
            || self.hotbar.opened()
            || self.switch.opened()
            || self.heal.opened()
    }
}

fn activation(prefix: &str) -> Definition {
    Definition::choice(
        &format!("{prefix}.trigger"),
        "Activation",
        &["on-key", "toggle"],
        "on-key",
    )
}
fn range(id: &str, label: &str, min: f64, max: f64, default: [f64; 2]) -> Definition {
    Definition::new(
        id,
        label,
        Kind::Range {
            min,
            max,
            step: 1.0,
        },
        json!(default),
    )
}
fn text(id: &str, label: &str, max_chars: usize) -> Definition {
    Definition::new(id, label, Kind::Text { max_chars }, json!(""))
}
fn material(id: &str, label: &str, default: &str) -> Definition {
    Definition::choice(
        id,
        label,
        &["diamond", "gold", "iron", "leather", "chain"],
        default,
    )
}

pub(super) fn definitions() -> Vec<Definition> {
    let toggle = || json!("toggle");
    let on_key = || json!("on-key");
    const IM: &str = "inventory.inventory-manager";
    const IC: &str = "inventory.inv-cleaner";
    const AH: &str = "inventory.auto-hotbar";
    const AS: &str = "inventory.armor-switch";
    vec![
        activation(IM),
        Definition::boolean(&format!("{IM}.open-inventory"), "Open inventory", true)
            .when(&format!("{IM}.trigger"), toggle()),
        Definition::boolean(&format!("{IM}.combat-check"), "Combat check", false)
            .when(&format!("{IM}.trigger"), toggle()),
        range(
            &format!("{IM}.click-delay"),
            "Click delay",
            50.0,
            300.0,
            [100.0, 150.0],
        ),
        // The "Inventory" profile value: the selected profile name, the
        // profile list and the shared presets (JSON, edited by the editor
        // frames).
        text(&format!("{IM}.profile"), "Inventory", 64),
        text(&format!("{IM}.profiles"), "Profiles", 65_536),
        text(&format!("{IM}.presets"), "Presets", 65_536),
        activation(IC),
        range(&format!("{IC}.delay"), "Delay", 1.0, 200.0, [100.0, 120.0]),
        Definition::boolean(&format!("{IC}.open-inventory"), "Open Inventory", true)
            .when(&format!("{IC}.trigger"), on_key()),
        Definition::boolean(&format!("{IC}.inventory-only"), "Inventory Only", true)
            .when(&format!("{IC}.trigger"), toggle()),
        Definition::boolean(&format!("{IC}.best-items"), "Best Items", true),
        Definition::boolean(
            &format!("{IC}.remove-negative-potions"),
            "Remove Negative Potions",
            true,
        ),
        Definition::boolean(&format!("{IC}.remove-food"), "Remove Food", true),
        Definition::new(
            &format!("{IC}.blacklist"),
            "Blacklisted",
            Kind::StringList {
                max_entries: 256,
                max_chars: 256,
            },
            json!(inv_cleaner::DEFAULT_BLACKLIST),
        ),
        activation(AH),
        Definition::boolean(&format!("{AH}.open-inventory"), "Open Inventory", true)
            .when(&format!("{AH}.trigger"), toggle()),
        Definition::number(&format!("{AH}.delay"), "Delay", 0.0, 110.0, 300.0, 1.0),
        text(&format!("{AH}.hotbar"), "Hotbar", 8_192),
        material(&format!("{AS}.set-1"), "Set 1", "diamond"),
        material(&format!("{AS}.set-2"), "Set 2", "gold"),
        Definition::number(&format!("{AS}.delay"), "Delay", 0.0, 100.0, 200.0, 1.0),
    ]
}

/// Scheduler outcome of one planner step: the effect, the delay range before
/// the next step, and whether the planner keeps the container claim.
pub(super) type Outcome = (Option<InventoryEffect>, (u64, u64), bool);

impl Runtime {
    fn extra_str(&self, key: &str) -> &str {
        self.settings.get(key).and_then(Value::as_str).unwrap_or("")
    }
    fn extra_number(&self, key: &str, default: u64) -> (u64, u64) {
        let n = self
            .settings
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .map_or(default, |n| n.max(0.0).round() as u64);
        (n, n)
    }
    fn on_key(&self, module: ModuleId) -> bool {
        matches!(
            module,
            ModuleId::InventoryManager | ModuleId::InvCleaner | ModuleId::AutoHotbar
        ) && self.extra_str(&format!("{}.trigger", module.id())) != "toggle"
    }
    /// The selected InventoryManager profile: `findByName` matches the saved
    /// selection case-insensitively, and nothing is selected without a match.
    pub fn inventory_profile(&self) -> Result<Option<Profile>, String> {
        const IM: &str = "inventory.inventory-manager";
        let presets = parse_presets(self.extra_str(&format!("{IM}.presets")))?;
        let mut profiles = parse_profiles(self.extra_str(&format!("{IM}.profiles")), &presets)?;
        let selected = self.extra_str(&format!("{IM}.profile"));
        Ok(find_profile(&profiles, selected).map(|index| profiles.swap_remove(index)))
    }
    /// Steps one of [`MODULES`]; `combat_blocked` is the shared combat guard.
    pub(super) fn extra_step(&mut self, module: ModuleId, snapshot: &Snapshot) -> Outcome {
        let on_key = self.on_key(module);
        let prefix = module.id();
        let flag =
            |r: &Self, key: &str, default| r.inventory_bool(&format!("{prefix}.{key}"), default);
        // Open/close waits of the reference InventoryManager (200 + rand(200) ms).
        let jitter = Duration::from_millis(
            200 + u64::from(snapshot.observed.elapsed().subsec_nanos() % 200),
        );
        let (step, delay) = match module {
            ModuleId::InventoryManager => {
                let delay = self.inventory_delay(&format!("{prefix}.click-delay"), (100, 150));
                if !on_key && flag(self, "combat-check", false) && self.combat_blocked {
                    return (None, delay, false);
                }
                let profile = match self.inventory_profile() {
                    Ok(Some(profile)) => profile,
                    // `shouldSkipProcessing`: no selected profile, nothing
                    // to manage; the module stays on and idles.
                    Ok(None) => return (None, delay, false),
                    Err(error) => {
                        self.notify(format!("InventoryManager: {error}"));
                        self.set_enabled(module, false);
                        return (None, delay, false);
                    }
                };
                let policy = Policy {
                    open: on_key || flag(self, "open-inventory", true),
                    close: true,
                    open_wait: jitter,
                    close_wait: jitter,
                };
                (
                    self.inventory_extra
                        .manager
                        .step(snapshot, &profile, policy),
                    delay,
                )
            }
            ModuleId::InvCleaner => {
                let options = inv_cleaner::Options {
                    best_items: flag(self, "best-items", true),
                    remove_negative_potions: flag(self, "remove-negative-potions", true),
                    remove_food: flag(self, "remove-food", true),
                    blacklist: self.inventory_list(&format!("{prefix}.blacklist")),
                };
                let open = if on_key {
                    flag(self, "open-inventory", true)
                } else {
                    !flag(self, "inventory-only", true)
                };
                let policy = Policy {
                    open,
                    ..Policy::default()
                };
                (
                    self.inventory_extra
                        .cleaner
                        .step(snapshot, &options, policy),
                    self.inventory_delay(&format!("{prefix}.delay"), (100, 120)),
                )
            }
            ModuleId::AutoHotbar => {
                let delay = self.extra_number(&format!("{prefix}.delay"), 110);
                let rules =
                    match auto_hotbar::parse_rules(self.extra_str(&format!("{prefix}.hotbar"))) {
                        Ok(rules) => rules,
                        Err(error) => {
                            self.notify(format!("AutoHotbar: {error}"));
                            self.set_enabled(module, false);
                            return (None, delay, false);
                        }
                    };
                let policy = Policy {
                    open: on_key || flag(self, "open-inventory", true),
                    ..Policy::default()
                };
                (
                    self.inventory_extra.hotbar.step(snapshot, &rules, policy),
                    delay,
                )
            }
            ModuleId::ArmorSwitch => {
                let set = |r: &Self, key: &str, default| {
                    Material::find(r.extra_str(&format!("{prefix}.{key}"))).unwrap_or(default)
                };
                let sets = (
                    set(self, "set-1", Material::Diamond),
                    set(self, "set-2", Material::Gold),
                );
                (
                    self.inventory_extra
                        .switch
                        .step(snapshot, sets, Policy::default()),
                    self.extra_number(&format!("{prefix}.delay"), 100),
                )
            }
            ModuleId::AutoHeal => {
                // The use sequence holds the container claim while it runs.
                let options = self.heal_options();
                (
                    self.inventory_extra.heal.step(snapshot, &options),
                    self.inventory_delay("inventory.auto-heal.swap-delay", (75, 125)),
                )
            }
            _ => unreachable!("extra inventory module"),
        };
        match step {
            Step::Effect(effect) => {
                if matches!(effect, Effect::Request(_)) {
                    self.inventory_extra.worked = true;
                }
                (Some(session_effect(effect)), delay, true)
            }
            Step::Wait => (None, delay, true),
            Step::Blocked => (None, delay, false),
            Step::Done => {
                // ArmorSwitch is a one-shot action; on-key modules finish
                // their run.
                if on_key || module == ModuleId::ArmorSwitch {
                    if !self.inventory_extra.worked && module == ModuleId::InventoryManager {
                        self.notify("No work available".into());
                    }
                    self.set_enabled(module, false);
                }
                self.inventory_extra.worked = false;
                (None, delay, false)
            }
        }
    }
}
fn session_effect(effect: Effect) -> InventoryEffect {
    match effect {
        Effect::Request(r) => InventoryEffect::Request(r),
        Effect::OpenInventory => InventoryEffect::OpenInventory,
        Effect::CloseInventory => InventoryEffect::CloseInventory,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::inventory::{session::tests::snapshot, types::Click};
    use super::*;
    use std::time::Instant;

    #[test]
    fn manager_without_a_profile_idles_and_stays_on() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::InventoryManager, true);
        let mut s = snapshot(false, Instant::now());
        s.session = runtime.session().epoch().value();
        assert!(runtime.inventory_step(&s).is_empty());
        assert!(runtime.enabled(ModuleId::InventoryManager));
    }

    #[test]
    fn on_key_cleaner_opens_drops_closes_and_disables() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::InvCleaner, true);
        let t = Instant::now();
        let mut s = snapshot(false, t);
        s.session = runtime.session().epoch().value();
        assert_eq!(
            runtime.inventory_step(&s),
            vec![InventoryEffect::OpenInventory]
        );
        let mut s = snapshot(true, t + Duration::from_millis(300));
        s.session = runtime.session().epoch().value();
        let effects = runtime.inventory_step(&s);
        assert!(matches!(
            effects.as_slice(),
            [InventoryEffect::Request(r)] if r.slot == 20 && r.click == Click::ThrowStack
        ));
        let mut done = s.clone();
        done.slots[20].item = None;
        done.revision += 1;
        let mut effects = Vec::new();
        for ms in [600, 900, 1200] {
            done.observed = t + Duration::from_millis(ms);
            effects.extend(runtime.inventory_step(&done));
        }
        assert_eq!(effects, vec![InventoryEffect::CloseInventory]);
        let mut closed = snapshot(false, t + Duration::from_millis(1500));
        closed.session = done.session;
        closed.slots[20].item = None;
        runtime.inventory_step(&closed);
        assert!(!runtime.enabled(ModuleId::InvCleaner));
    }
}
