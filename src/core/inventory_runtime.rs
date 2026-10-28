// Runtime-side arbitration for the three retained inventory planners.
use super::super::{
    inventory::types::{Request, Snapshot},
    modules::utility::{auto_armor, chest_steal, refill},
};
use super::settings::{Definition, Kind};
use super::{ModuleId, Runtime};
use serde_json::json;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryEffect {
    Request(Request),
    OpenInventory,
    CloseInventory,
}
#[derive(Debug)]
pub struct Scheduler {
    due: Option<Instant>,
    seed: u64,
    active: Option<ModuleId>,
    cursor: u8,
}
impl Default for Scheduler {
    fn default() -> Self {
        Self {
            due: None,
            seed: 0x56a9,
            active: None,
            cursor: 0,
        }
    }
}
impl Scheduler {
    pub fn clear(&mut self) {
        self.due = None;
        self.active = None;
    }
    fn ready(&self) -> bool {
        self.due.is_none_or(|due| Instant::now() >= due)
    }
    fn active(&self) -> Option<ModuleId> {
        self.active
    }
    fn select(&mut self, module: ModuleId) {
        self.active = Some(module);
    }
    fn release(&mut self) {
        self.due = None;
        self.active = None;
    }
    fn choose(&mut self, enabled: [bool; 3]) -> Option<ModuleId> {
        let modules = [ModuleId::ChestSteal, ModuleId::AutoArmor, ModuleId::Refill];
        for offset in 0..modules.len() {
            let index = (usize::from(self.cursor) + offset) % modules.len();
            if enabled[index] {
                self.cursor = ((index + 1) % modules.len()) as u8;
                return Some(modules[index]);
            }
        }
        None
    }
    fn wait(&mut self, range: (u64, u64)) {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let (min, max) = (range.0.min(range.1), range.0.max(range.1));
        let ms = min + self.seed % (max - min + 1);
        self.due = Some(Instant::now() + Duration::from_millis(ms));
    }
}

pub(super) fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(
            "inventory.auto-armor.inventory-only",
            "Inventory only",
            true,
        ),
        Definition::boolean(
            "inventory.auto-armor.open-inventory",
            "Open inventory",
            true,
        ),
        Definition::boolean(
            "inventory.auto-armor.check-durability",
            "Check durability",
            true,
        ),
        Definition::boolean("inventory.auto-armor.drop-equipped", "Drop equipped", false),
        Definition::boolean("inventory.auto-armor.combat-check", "Combat check", false),
        Definition::new(
            "inventory.auto-armor.delay",
            "Delay",
            Kind::Range {
                min: 100.0,
                max: 200.0,
                step: 1.0,
            },
            json!([120.0, 120.0]),
        ),
        Definition::choice(
            "inventory.refill.type",
            "Type",
            &["both", "pots", "soup"],
            "both",
        ),
        Definition::boolean("inventory.refill.vertical", "Vertical", false),
        Definition::boolean("inventory.refill.scatter", "Scatter", false),
        Definition::boolean("inventory.refill.hotbar-clear", "Hotbar clear", false),
        Definition::new(
            "inventory.refill.allowed",
            "Non junk items",
            Kind::StringList {
                max_entries: 256,
                max_chars: 256,
            },
            json!([]),
        ),
        Definition::new(
            "inventory.refill.delay",
            "Delay",
            Kind::Range {
                min: 50.0,
                max: 200.0,
                step: 5.0,
            },
            json!([75.0, 125.0]),
        ),
        Definition::boolean("world.chest-steal.check-menu", "Check menu", true),
        Definition::boolean("world.chest-steal.best-only", "Best only", false),
        Definition::boolean("world.chest-steal.keep-open", "Keep open", false),
        Definition::boolean("world.chest-steal.shuffle", "Shuffle", false),
        Definition::new(
            "world.chest-steal.blacklist",
            "Blacklisted",
            Kind::StringList {
                max_entries: 256,
                max_chars: 256,
            },
            json!([]),
        ),
        Definition::new(
            "world.chest-steal.click-delay",
            "Click delay",
            Kind::Range {
                min: 50.0,
                max: 300.0,
                step: 5.0,
            },
            json!([150.0, 200.0]),
        ),
    ]
}

impl Runtime {
    /// Advances at most one planner request. A returned request must be sent
    /// to the host once; the next call only advances when a newer snapshot
    /// acknowledges it through the planner gate.
    pub fn inventory_step(&mut self, snapshot: &Snapshot) -> Vec<InventoryEffect> {
        let combat_blocked = self.combat_blocked;
        let module = self.inventory_schedule.active().or_else(|| {
            self.inventory_schedule.choose([
                self.enabled(ModuleId::ChestSteal),
                self.enabled(ModuleId::AutoArmor),
                self.enabled(ModuleId::Refill),
            ])
        });
        let Some(module) = module else {
            return Vec::new();
        };
        if !self.enabled(module) {
            self.cancel_inventory();
            return Vec::new();
        }
        self.inventory_schedule.select(module);
        // A pending request remains owned by this module. Its next planner
        // observation is also delayed: no same-frame follow-up can mistake a
        // local prediction for server acceptance.
        if !self.inventory_schedule.ready() {
            return Vec::new();
        }
        let (effect, delay, active) = match module {
            ModuleId::ChestSteal => {
                let effect = chest_effect(self.chest_steal.step(snapshot, &self.chest_options()));
                (
                    effect,
                    self.inventory_delay("world.chest-steal.click-delay", (150, 200)),
                    self.chest_steal.active(),
                )
            }
            ModuleId::AutoArmor => {
                let effect = self
                    .auto_armor
                    .step(snapshot, self.armor_options(), combat_blocked)
                    .into_iter()
                    .find_map(armor_effect);
                (
                    effect,
                    self.inventory_delay("inventory.auto-armor.delay", (120, 120)),
                    self.auto_armor.active(),
                )
            }
            ModuleId::Refill => {
                let mut effect = None;
                for next in self.refill.step(snapshot, &self.refill_options()) {
                    match next {
                        refill::Effect::Request(request) => {
                            effect.get_or_insert(InventoryEffect::Request(request));
                        }
                        refill::Effect::OpenInventory => {
                            effect.get_or_insert(InventoryEffect::OpenInventory);
                        }
                        refill::Effect::CloseInventory => {
                            effect.get_or_insert(InventoryEffect::CloseInventory);
                        }
                        refill::Effect::Disable => {
                            self.set_enabled(ModuleId::Refill, false);
                        }
                    }
                }
                (
                    effect,
                    self.inventory_delay("inventory.refill.delay", (75, 125)),
                    self.refill.active(),
                )
            }
            _ => unreachable!("inventory module registry"),
        };
        if let Some(effect) = effect {
            self.inventory_schedule.wait(delay);
            return vec![effect];
        }
        if active {
            self.inventory_schedule.wait(delay);
        } else {
            self.inventory_schedule.release();
        }
        Vec::new()
    }
    pub fn cancel_inventory(&mut self) {
        self.auto_armor.cancel();
        self.refill.cancel();
        self.chest_steal.cancel();
        self.inventory_schedule.clear();
    }
    /// The backend calls this when `submit` rejects a stale request; no
    /// planner retries it against a different menu or revision.
    pub fn inventory_rejected(&mut self) {
        self.cancel_inventory();
    }

    fn inventory_bool(&self, key: &str, default: bool) -> bool {
        self.settings
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(default)
    }
    fn inventory_list(&self, key: &str) -> BTreeSet<String> {
        self.settings
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .map(str::to_owned)
            .collect()
    }
    fn inventory_delay(&self, key: &str, default: (u64, u64)) -> (u64, u64) {
        self.settings
            .get(key)
            .and_then(serde_json::Value::as_array)
            .filter(|range| range.len() == 2)
            .and_then(|range| Some((range[0].as_u64()?, range[1].as_u64()?)))
            .unwrap_or(default)
    }
    fn armor_options(&self) -> auto_armor::Options {
        auto_armor::Options {
            inventory_only: self.inventory_bool("inventory.auto-armor.inventory-only", true),
            open_inventory: self.inventory_bool("inventory.auto-armor.open-inventory", true),
            check_durability: self.inventory_bool("inventory.auto-armor.check-durability", true),
            drop_equipped: self.inventory_bool("inventory.auto-armor.drop-equipped", false),
            combat_check: self.inventory_bool("inventory.auto-armor.combat-check", false),
        }
    }
    fn refill_options(&self) -> refill::Options {
        refill::Options {
            kind: match self
                .settings
                .get("inventory.refill.type")
                .and_then(serde_json::Value::as_str)
            {
                Some("pots") => refill::Kind::Pots,
                Some("soup") => refill::Kind::Soup,
                _ => refill::Kind::Both,
            },
            vertical: self.inventory_bool("inventory.refill.vertical", false),
            scatter: self.inventory_bool("inventory.refill.scatter", false),
            hotbar_clear: self.inventory_bool("inventory.refill.hotbar-clear", false),
            allowed: self.inventory_list("inventory.refill.allowed"),
        }
    }
    fn chest_options(&self) -> chest_steal::Options {
        chest_steal::Options {
            check_menu: self.inventory_bool("world.chest-steal.check-menu", true),
            best_only: self.inventory_bool("world.chest-steal.best-only", false),
            keep_open: self.inventory_bool("world.chest-steal.keep-open", false),
            shuffle: self.inventory_bool("world.chest-steal.shuffle", false),
            blacklist: self.inventory_list("world.chest-steal.blacklist"),
        }
    }
}
fn armor_effect(effect: auto_armor::Effect) -> Option<InventoryEffect> {
    match effect {
        auto_armor::Effect::Request(r) => Some(InventoryEffect::Request(r)),
        auto_armor::Effect::OpenInventory => Some(InventoryEffect::OpenInventory),
        auto_armor::Effect::CloseInventory => Some(InventoryEffect::CloseInventory),
    }
}
fn chest_effect(effect: Vec<chest_steal::Effect>) -> Option<InventoryEffect> {
    effect.into_iter().next().map(|effect| match effect {
        chest_steal::Effect::Request(r) => InventoryEffect::Request(r),
        chest_steal::Effect::Close => InventoryEffect::CloseInventory,
    })
}
