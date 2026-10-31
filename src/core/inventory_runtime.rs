// Runtime-side arbitration for the three retained inventory planners.
use super::super::{
    inventory::types::{Request, Snapshot},
    modules::utility::{auto_armor, auto_totem, chest_steal, refill},
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
/// Longest time a selected planner may hold the container claim without
/// emitting an effect. Planner clicks resolve within the gate deadline and
/// configured delays are far shorter; exceeding this is a planner defect,
/// which the scheduler recovers from instead of starving other automation.
pub const STALL_LIMIT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct Scheduler {
    due: Option<Instant>,
    seed: u64,
    active: Option<ModuleId>,
    progress: Option<Instant>,
    cursor: u8,
}
impl Default for Scheduler {
    fn default() -> Self {
        Self {
            due: None,
            seed: 0x56a9,
            active: None,
            progress: None,
            cursor: 0,
        }
    }
}
impl Scheduler {
    pub fn clear(&mut self) {
        self.due = None;
        self.active = None;
        self.progress = None;
    }
    fn ready(&self, now: Instant) -> bool {
        self.due.is_none_or(|due| now >= due)
    }
    fn active(&self) -> Option<ModuleId> {
        self.active
    }
    fn select(&mut self, module: ModuleId, now: Instant) {
        if self.active != Some(module) {
            self.progress = Some(now);
        }
        self.active = Some(module);
    }
    fn progressed(&mut self, now: Instant) {
        self.progress = Some(now);
    }
    fn stalled(&self, now: Instant) -> bool {
        self.progress
            .is_some_and(|since| now.saturating_duration_since(since) > STALL_LIMIT)
    }
    fn release(&mut self) {
        self.clear();
    }
    fn choose(&mut self, modules: &[(ModuleId, bool)]) -> Option<ModuleId> {
        for offset in 0..modules.len() {
            let index = (usize::from(self.cursor) + offset) % modules.len();
            if modules[index].1 {
                self.cursor = ((index + 1) % modules.len()) as u8;
                return Some(modules[index].0);
            }
        }
        None
    }
    fn wait(&mut self, now: Instant, range: (u64, u64)) {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let (min, max) = (range.0.min(range.1), range.0.max(range.1));
        let ms = min + self.seed % (max - min + 1);
        self.due = Some(now + Duration::from_millis(ms));
    }
}

pub(super) fn definitions() -> Vec<Definition> {
    vec![
        Definition::boolean(
            "inventory.auto-totem.open-inventory",
            "Open inventory",
            true,
        ),
        Definition::boolean(
            "inventory.auto-totem.close-inventory",
            "Close inventory",
            true,
        )
        .when("inventory.auto-totem.open-inventory", json!(true)),
        Definition::boolean(
            "inventory.auto-totem.inventory-only",
            "Inventory only",
            false,
        ),
        Definition::boolean("inventory.auto-totem.random-slot", "Random slot", true),
        Definition::new(
            "inventory.auto-totem.delay",
            "Delay",
            Kind::Range {
                min: 50.0,
                max: 200.0,
                step: 1.0,
            },
            json!([100.0, 120.0]),
        ),
        Definition::boolean(
            "inventory.auto-totem.extra-randomization",
            "Extra randomization",
            true,
        ),
        Definition::boolean(
            "inventory.auto-totem.show-totem-count",
            "Show totem count",
            false,
        ),
        Definition::boolean(
            "inventory.auto-armor.open-inventory",
            "Open inventory",
            true,
        ),
        Definition::boolean(
            "inventory.auto-armor.inventory-only",
            "Inventory only",
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
                min: 1.0,
                max: 200.0,
                step: 1.0,
            },
            json!([100.0, 120.0]),
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
        use super::super::actions::{Owner, Resource};
        if self
            .actions
            .acquire(
                Resource::Container,
                Owner("inventory"),
                self.session.epoch(),
                100,
            )
            .is_none()
        {
            return Vec::new();
        }
        let effects = self.inventory_step_owned(snapshot);
        if self.inventory_schedule.active().is_none() {
            self.actions.release_owner(Owner("inventory"));
        }
        effects
    }
    fn inventory_step_owned(&mut self, snapshot: &Snapshot) -> Vec<InventoryEffect> {
        let combat_blocked = self.combat_blocked;
        let module = self.inventory_schedule.active().or_else(|| {
            let modules: Vec<_> = [
                ModuleId::ChestSteal,
                ModuleId::AutoArmor,
                ModuleId::Refill,
                ModuleId::AutoTotem,
            ]
            .into_iter()
            .chain(super::inventory_extra::MODULES)
            .map(|module| (module, self.enabled(module)))
            .collect();
            self.inventory_schedule.choose(&modules)
        });
        let Some(module) = module else {
            return Vec::new();
        };
        if !self.enabled(module) {
            self.cancel_inventory();
            return Vec::new();
        }
        let now = snapshot.observed;
        self.inventory_schedule.select(module, now);
        if self.inventory_schedule.stalled(now) {
            let owned_screen = self.auto_armor.opened()
                || self.auto_totem.opened()
                || self.refill.opened()
                || self.inventory_extra.opened();
            self.cancel_inventory();
            return if owned_screen && snapshot.screen_open {
                vec![InventoryEffect::CloseInventory]
            } else {
                Vec::new()
            };
        }
        // A pending request remains owned by this module. Its next planner
        // observation is also delayed: no same-frame follow-up can mistake a
        // local prediction for server acceptance.
        if !self.inventory_schedule.ready(now) {
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
                // The planner owns its open and click timers.
                (effect, (0, 0), self.auto_armor.active())
            }
            ModuleId::AutoTotem => {
                let inventory_only =
                    self.inventory_bool("inventory.auto-totem.inventory-only", false);
                let open = self.inventory_bool("inventory.auto-totem.open-inventory", true);
                let close = self.inventory_bool("inventory.auto-totem.close-inventory", true);
                self.auto_totem.random_slot =
                    self.inventory_bool("inventory.auto-totem.random-slot", true);
                let effect = self
                    .auto_totem
                    .step(snapshot, inventory_only, open, close)
                    .map(|effect| match effect {
                        auto_totem::Effect::Request(r) => InventoryEffect::Request(r),
                        auto_totem::Effect::OpenInventory => InventoryEffect::OpenInventory,
                        auto_totem::Effect::CloseInventory => InventoryEffect::CloseInventory,
                    });
                let range = self.inventory_delay("inventory.auto-totem.delay", (100, 120));
                let extra = self.inventory_bool("inventory.auto-totem.extra-randomization", true);
                let ms = self.auto_totem.delay(range, extra);
                (effect, (ms, ms), self.auto_totem.active())
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
            module if super::inventory_extra::MODULES.contains(&module) => {
                self.extra_step(module, snapshot)
            }
            _ => unreachable!("inventory module registry"),
        };
        if let Some(effect) = effect {
            self.inventory_schedule.wait(now, delay);
            self.inventory_schedule.progressed(now);
            return vec![effect];
        }
        if active {
            self.inventory_schedule.wait(now, delay);
        } else {
            self.inventory_schedule.release();
        }
        Vec::new()
    }
    /// AutoTotem's "Show totem count" label and colour. Nothing draws it
    /// yet: the reference shows it in the active-module stack frame, which
    /// has no port (documented in docs/decisions.md).
    pub fn totem_count_display(&self, snapshot: &Snapshot) -> Option<(String, [u8; 3])> {
        (self.enabled(ModuleId::AutoTotem)
            && self.inventory_bool("inventory.auto-totem.show-totem-count", false))
        .then(|| {
            let count = auto_totem::totem_count(snapshot);
            (count.to_string(), auto_totem::count_color(count))
        })
    }
    pub fn cancel_inventory(&mut self) {
        self.actions
            .release_owner(super::super::actions::Owner("inventory"));
        self.auto_armor.cancel();
        self.auto_totem.cancel();
        self.refill.cancel();
        self.chest_steal.cancel();
        self.inventory_extra.cancel();
        self.inventory_schedule.clear();
    }
    /// The backend calls this when `submit` rejects a stale request; no
    /// planner retries it against a different menu or revision.
    pub fn inventory_rejected(&mut self) {
        self.cancel_inventory();
    }

    pub(super) fn inventory_bool(&self, key: &str, default: bool) -> bool {
        self.settings
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(default)
    }
    pub(super) fn inventory_list(&self, key: &str) -> BTreeSet<String> {
        self.settings
            .get(key)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .map(str::to_owned)
            .collect()
    }
    pub(super) fn inventory_delay(&self, key: &str, default: (u64, u64)) -> (u64, u64) {
        self.settings
            .get(key)
            .and_then(serde_json::Value::as_array)
            .filter(|range| range.len() == 2)
            .and_then(|range| {
                let min = range[0].as_f64()?;
                let max = range[1].as_f64()?;
                (min.is_finite() && max.is_finite())
                    .then_some((min.max(0.0).round() as u64, max.max(0.0).round() as u64))
            })
            .unwrap_or(default)
    }
    fn armor_options(&self) -> auto_armor::Options {
        auto_armor::Options {
            inventory_only: self.inventory_bool("inventory.auto-armor.inventory-only", true),
            open_inventory: self.inventory_bool("inventory.auto-armor.open-inventory", true),
            check_durability: self.inventory_bool("inventory.auto-armor.check-durability", true),
            drop_equipped: self.inventory_bool("inventory.auto-armor.drop-equipped", false),
            combat_check: self.inventory_bool("inventory.auto-armor.combat-check", false),
            delay: self.inventory_delay("inventory.auto-armor.delay", (100, 120)),
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

#[cfg(test)]
mod tests {
    use super::super::super::inventory::{
        planner::{DEADLINE, tests::item},
        types::{ArmorSlot, Click, Item, Slot, SlotRole},
    };
    use super::*;

    fn helmet(score: f32) -> Item {
        let mut helmet = Item {
            armor: Some(ArmorSlot::Head),
            score,
            max_count: 1,
            ..item("minecraft:iron_helmet", 1)
        };
        helmet.meta.armor_points = score;
        helmet
    }
    /// Survival inventory with every main/hotbar slot full, a worse helmet
    /// equipped and a better one in the inventory: removing the equipped
    /// helmet by quick-move can never succeed.
    fn full_inventory(runtime: &Runtime, at: Instant, open: bool) -> Snapshot {
        let mut slots: Vec<Slot> = (0..36u16)
            .map(|index| Slot {
                index,
                item: Some(item("minecraft:stone", 64)),
                role: if index < 9 {
                    SlotRole::Hotbar(index as u8)
                } else {
                    SlotRole::Main
                },
            })
            .collect();
        slots[20].item = Some(helmet(3.0));
        slots.push(Slot {
            index: 500,
            item: Some(helmet(1.0)),
            role: SlotRole::Armor(ArmorSlot::Head),
        });
        slots.push(Slot {
            index: 504,
            item: None,
            role: SlotRole::Other,
        });
        Snapshot {
            session: runtime.session().epoch().value(),
            container_id: 0,
            revision: 11,
            slots,
            carried: None,
            screen_open: open,
            is_player_inventory: open,
            is_chest: false,
            chest_title_standard: true,
            vitals: Default::default(),
            observed: at,
        }
    }

    #[test]
    fn no_op_armor_click_is_abandoned_and_releases_the_container() {
        let mut runtime = Runtime::default();
        runtime
            .set_setting("inventory.auto-armor.inventory-only", json!(false))
            .unwrap();
        runtime.set_enabled(ModuleId::AutoArmor, true);
        let t = Instant::now();
        assert_eq!(
            runtime.inventory_step(&full_inventory(&runtime, t, false)),
            vec![InventoryEffect::OpenInventory]
        );
        // The opened screen resets AutoArmor's open timer (100 + Delay).
        let opened_at = t + Duration::from_millis(50);
        let mut t = opened_at;
        let effects = loop {
            assert!(t < opened_at + Duration::from_millis(400), "no click");
            let effects = runtime.inventory_step(&full_inventory(&runtime, t, true));
            if !effects.is_empty() {
                break effects;
            }
            assert!(runtime.inventory_busy(), "the opened inventory stays owned");
            t += Duration::from_millis(50);
        };
        assert!(matches!(
            effects.as_slice(),
            [InventoryEffect::Request(Request {
                slot: 500,
                click: Click::QuickMove,
                ..
            })]
        ));
        // The host performs the click; nothing changes because there is no room.
        let mut now = t;
        let mut closed = false;
        while now < t + DEADLINE + Duration::from_millis(400) {
            now += Duration::from_millis(50);
            let effects = runtime.inventory_step(&full_inventory(&runtime, now, true));
            assert!(
                !effects
                    .iter()
                    .any(|e| matches!(e, InventoryEffect::Request(_))),
                "the no-op must not be retried"
            );
            if effects == [InventoryEffect::CloseInventory] {
                closed = true;
                break;
            }
            assert!(
                now < t + DEADLINE + Duration::from_millis(350),
                "still waiting"
            );
        }
        assert!(closed, "AutoArmor closes the inventory it opened");
        // Back in game with unchanged content: no reopen, no retained claim.
        for _ in 0..5 {
            now += Duration::from_millis(250);
            assert!(
                runtime
                    .inventory_step(&full_inventory(&runtime, now, false))
                    .is_empty()
            );
            assert!(!runtime.inventory_busy());
        }
    }

    #[test]
    fn stalled_planner_does_not_starve_other_inventory_modules() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::AutoArmor, true);
        runtime.set_enabled(ModuleId::AutoTotem, true);
        let t = Instant::now();
        let mut snapshot = full_inventory(&runtime, t, true);
        snapshot.slots[30].item = Some(item("minecraft:totem_of_undying", 1));
        // Round-robin starts with AutoArmor; its no-op click times out.
        let mut now = t;
        let mut seen_totem = false;
        for _ in 0..80 {
            snapshot.observed = now;
            for effect in runtime.inventory_step(&snapshot) {
                if let InventoryEffect::Request(r) = effect
                    && r.click == Click::SwapOffhand
                {
                    assert_eq!(r.slot, 30);
                    seen_totem = true;
                }
            }
            if seen_totem {
                break;
            }
            now += Duration::from_millis(50);
        }
        assert!(
            seen_totem,
            "AutoTotem gets the container after AutoArmor fails"
        );
        assert!(now <= t + DEADLINE + Duration::from_millis(600));
    }

    #[test]
    fn watchdog_recovers_a_planner_that_never_resolves() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::ChestSteal, true);
        let t = Instant::now();
        let mut snapshot = full_inventory(&runtime, t, true);
        // A selected module that keeps reporting itself active without ever
        // emitting an effect is cancelled after STALL_LIMIT.
        runtime.inventory_schedule.select(ModuleId::ChestSteal, t);
        runtime.chest_steal.cancel();
        snapshot.observed = t + STALL_LIMIT + Duration::from_millis(1);
        runtime.inventory_schedule.due = Some(snapshot.observed + Duration::from_secs(60));
        assert!(runtime.inventory_step(&snapshot).is_empty());
        assert!(runtime.inventory_schedule.active().is_none());
        assert!(!runtime.inventory_busy());
    }
}
