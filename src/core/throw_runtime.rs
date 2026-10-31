// Runtime side of the hotbar use sequences (Throwpot, ThrowDebuff, AutoHeal
// Legit). A sequence holds the hotbar, use and container claims while it
// runs, so inventory planners and macros cannot interleave with it; it is
// abandoned (without restoring) when the player changes slot themselves,
// a screen opens, or a claim is lost.
use super::super::{
    actions::{Lease, Owner, Resource},
    inventory::types::Snapshot,
    modules::utility::{
        auto_heal,
        throw_items::{self, DebuffMode, Effect, PotKind, Sequencer},
    },
};
use super::settings::{Definition, Kind};
use super::{ModuleId, Runtime};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const OWNER: Owner = Owner("inventory.throw");
/// Above inventory planners (100) and AutoFish (100), below macros (200).
const PRIORITY: u16 = 150;

#[derive(Debug)]
pub struct Throws {
    module: Option<ModuleId>,
    sequencer: Sequencer,
    leases: Vec<Lease>,
    origin: Instant,
    seed: u64,
    /// AutoHeal's "Delay" between heals (the reference's one `useTimer`).
    heal_due: Option<Instant>,
    /// A potion's hotbar slot between PreMotion and PostMotion.
    potion: Option<u8>,
}
impl Default for Throws {
    fn default() -> Self {
        Self {
            module: None,
            sequencer: Sequencer::default(),
            leases: Vec::new(),
            origin: Instant::now(),
            seed: 0x7417,
            heal_due: None,
            potion: None,
        }
    }
}
impl Throws {
    fn random(&mut self, (min, max): (u64, u64)) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let (min, max) = (min.min(max), min.max(max));
        min + self.seed % (max - min + 1)
    }
    fn now(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.origin).as_millis() as u64
    }
}

/// Host effects for one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThrowEffect {
    Select(u8),
    Use,
    Drop,
    /// AutoHeal Silent: the server's held slot moves to this hotbar slot for
    /// one item use and back; the shown hotbar never changes.
    SilentUse(u8),
}

/// AutoHeal's PreMotion: a SWAP click (game slot, hotbar button) bringing a
/// potion into "Slot", and the pitch this motion packet sends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HealMotion {
    pub swap: Option<(u16, u8)>,
    pub pitch: Option<f32>,
}

pub(super) fn definitions() -> Vec<Definition> {
    const TP: &str = "inventory.throw-pot";
    const TD: &str = "inventory.throw-debuff";
    const AH: &str = "inventory.auto-heal";
    let range = |id: String, label: &str, max: f64, default: [f64; 2]| {
        Definition::new(
            &id,
            label,
            Kind::Range {
                min: 0.0,
                max,
                step: 1.0,
            },
            json!(default),
        )
    };
    let mut out = vec![
        Definition::choice(
            &format!("{TP}.type"),
            "Type",
            &["both", "pots", "soup"],
            "both",
        ),
        Definition::choice(
            &format!("{TP}.mode"),
            "Mode",
            &["dynamic", "single"],
            "dynamic",
        ),
        range(format!("{TP}.delay"), "Delay", 200.0, [80.0, 115.0]),
        Definition::boolean(&format!("{TP}.scroll"), "Scroll", false),
        Definition::number(
            &format!("{TP}.scroll-delay"),
            "Scroll delay",
            0.0,
            100.0,
            200.0,
            1.0,
        )
        .when(&format!("{TP}.scroll"), json!(true)),
        Definition::boolean(&format!("{TP}.random"), "Random", false),
        Definition::boolean(&format!("{TP}.throw-bowls"), "Throw bowls", true),
        Definition::choice(
            &format!("{TD}.mode"),
            "Mode",
            &["all", "one-of-each", "first"],
            "one-of-each",
        ),
    ];
    for (label, _) in throw_items::DEBUFFS {
        out.push(Definition::boolean(
            &format!("{TD}.{}", label.to_lowercase()),
            label,
            true,
        ));
    }
    out.extend([
        range(format!("{TD}.delay"), "Delay", 200.0, [70.0, 120.0]),
        Definition::boolean(&format!("{TD}.scroll"), "Scroll", false),
        Definition::number(
            &format!("{TD}.scroll-delay"),
            "Scroll delay",
            0.0,
            100.0,
            200.0,
            1.0,
        )
        .when(&format!("{TD}.scroll"), json!(true)),
        Definition::choice(&format!("{AH}.mode"), "Mode", &["silent", "legit"], "legit"),
        Definition::boolean(&format!("{AH}.soup"), "Soup", false),
        Definition::choice(
            &format!("{AH}.bowl-mode"),
            "Bowl Mode",
            &["stack", "throw", "none"],
            "stack",
        )
        .when(&format!("{AH}.mode"), json!("silent"))
        .when(&format!("{AH}.soup"), json!(true)),
        Definition::boolean(&format!("{AH}.potions"), "Potions", false)
            .when(&format!("{AH}.mode"), json!("silent")),
        Definition::boolean(&format!("{AH}.regen"), "Regen", true)
            .when(&format!("{AH}.potions"), json!(true)),
        Definition::boolean(&format!("{AH}.speed"), "Speed", false)
            .when(&format!("{AH}.potions"), json!(true)),
        Definition::boolean(&format!("{AH}.resistance"), "Resistance", false)
            .when(&format!("{AH}.potions"), json!(true)),
        Definition::number(&format!("{AH}.slot"), "Slot", 1.0, 6.0, 9.0, 1.0)
            .when(&format!("{AH}.potions"), json!(true)),
        Definition::boolean(&format!("{AH}.use-custom-items"), "Use Custom Items", false),
        Definition::new(
            &format!("{AH}.healing-items"),
            "Healing Items",
            Kind::StringList {
                max_entries: 256,
                max_chars: 256,
            },
            json!(["minecraft:player_head"]),
        )
        .when(&format!("{AH}.use-custom-items"), json!(true)),
        Definition::boolean(&format!("{AH}.replace"), "Replace", false),
        Definition::number(&format!("{AH}.health"), "Health", 1.0, 17.0, 20.0, 1.0),
        Definition::number(&format!("{AH}.delay"), "Delay", 50.0, 500.0, 1000.0, 50.0),
        Definition::new(
            &format!("{AH}.swap-delay"),
            "Swap Delay",
            Kind::Range {
                min: 0.0,
                max: 200.0,
                step: 5.0,
            },
            json!([75.0, 125.0]),
        )
        .when(&format!("{AH}.mode"), json!("legit")),
    ]);
    out
}

impl Runtime {
    fn throw_str(&self, key: &str) -> &str {
        self.settings.get(key).and_then(Value::as_str).unwrap_or("")
    }
    fn throw_number(&self, key: &str, default: f64) -> f64 {
        self.settings
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .unwrap_or(default)
    }
    pub(super) fn heal_options(&self) -> auto_heal::Options {
        const AH: &str = "inventory.auto-heal";
        auto_heal::Options {
            soup: self.inventory_bool(&format!("{AH}.soup"), false),
            bowl_mode: match self.throw_str(&format!("{AH}.bowl-mode")) {
                "throw" => auto_heal::BowlMode::Throw,
                "none" => auto_heal::BowlMode::None,
                _ => auto_heal::BowlMode::Stack,
            },
            custom_items: self.inventory_bool(&format!("{AH}.use-custom-items"), false),
            healing_items: self.inventory_list(&format!("{AH}.healing-items")),
            replace: self.inventory_bool(&format!("{AH}.replace"), false),
            health: self.throw_number(&format!("{AH}.health"), 17.0) as f32,
        }
    }
    /// Potions' options when they take effect (enabled, Silent, "Potions").
    fn heal_potions(&self) -> Option<auto_heal::Potions> {
        const AH: &str = "inventory.auto-heal";
        let on = self.enabled(ModuleId::AutoHeal)
            && self.throw_str(&format!("{AH}.mode")) == "silent"
            && self.inventory_bool(&format!("{AH}.potions"), false);
        on.then(|| auto_heal::Potions {
            regen: self.inventory_bool(&format!("{AH}.regen"), true),
            speed: self.inventory_bool(&format!("{AH}.speed"), false),
            resistance: self.inventory_bool(&format!("{AH}.resistance"), false),
            slot: self
                .throw_number(&format!("{AH}.slot"), 6.0)
                .clamp(1.0, 9.0) as u8
                - 1,
            replace: self.inventory_bool(&format!("{AH}.replace"), false),
            health: self.throw_number(&format!("{AH}.health"), 17.0) as f32,
        })
    }
    /// Whether AutoHeal's motion hooks have anything to do.
    pub fn auto_heal_potions(&self) -> bool {
        self.heal_potions().is_some()
    }
    fn heal_ready(&self, at: Instant) -> bool {
        self.throws.heal_due.is_none_or(|due| at >= due)
    }
    fn heal_used(&mut self, at: Instant) {
        let delay = self.throw_number("inventory.auto-heal.delay", 500.0);
        self.throws.heal_due = Some(at + Duration::from_millis(delay.max(0.0) as u64));
    }
    /// AutoHeal's PreMotion (`EventPreMotion`, Potions only). As in the
    /// reference the potion search, and so Replace's swap, runs on every
    /// motion update; a use is armed only when one is needed on the ground
    /// and the delay has passed, and then this packet looks down.
    pub fn auto_heal_pre_motion(&mut self, snapshot: &Snapshot) -> HealMotion {
        let Some(potions) = self.heal_potions() else {
            self.throws.potion = None;
            return HealMotion::default();
        };
        let pick = auto_heal::potion_slot(snapshot, &potions);
        let mut out = HealMotion {
            swap: pick.and_then(|p| p.swap).map(|slot| (slot, potions.slot)),
            pitch: None,
        };
        match pick {
            Some(pick)
                if auto_heal::needs_potion(&snapshot.vitals, &potions)
                    && self.heal_ready(snapshot.observed) =>
            {
                if self.throws.potion.is_none() {
                    self.throws.potion = Some(pick.hotbar);
                    out.pitch = Some(auto_heal::POTION_PITCH);
                }
            }
            _ => self.throws.potion = None,
        }
        out
    }
    /// AutoHeal's PostMotion: the armed potion's hotbar slot to use
    /// silently. The delay restarts whether or not the slot still holds it.
    pub fn auto_heal_post_motion(&mut self, at: Instant) -> Option<u8> {
        self.heal_potions()?;
        if !self.heal_ready(at) {
            return None;
        }
        let slot = self.throws.potion.take()?;
        self.heal_used(at);
        Some(slot)
    }
    pub fn throws_module(&self) -> Option<ModuleId> {
        self.throws.module
    }
    pub fn cancel_throws(&mut self) {
        self.throws.sequencer.cancel();
        self.throws.module = None;
        for lease in std::mem::take(&mut self.throws.leases) {
            self.actions.release(lease);
        }
    }
    fn throw_claims(&mut self) -> bool {
        let epoch = self.session.epoch();
        let mut leases = Vec::new();
        for resource in [Resource::Hotbar, Resource::UseItem, Resource::Container] {
            match self.actions.acquire(resource, OWNER, epoch, PRIORITY) {
                Some(lease) => leases.push(lease),
                None => {
                    for lease in leases {
                        self.actions.release(lease);
                    }
                    return false;
                }
            }
        }
        self.throws.leases = leases;
        true
    }
    /// Advances the active use sequence or starts one. `playing` is true
    /// when no screen is shown and the player accepts input.
    pub fn throw_frame(&mut self, snapshot: &Snapshot, playing: bool) -> Vec<ThrowEffect> {
        let at = snapshot.observed;
        let now = self.throws.now(at);
        if let Some(module) = self.throws.module {
            let epoch = self.session.epoch();
            let lost = self
                .throws
                .leases
                .iter()
                .any(|l| !self.actions.owns(*l, epoch));
            let moved = self
                .throws
                .sequencer
                .expected
                .is_some_and(|slot| slot != snapshot.vitals.selected);
            if !playing || lost || moved || !self.enabled(module) {
                self.cancel_throws();
                if module != ModuleId::AutoHeal {
                    self.set_enabled(module, false);
                }
                return Vec::new();
            }
            let effects = self.throws.sequencer.tick(now);
            if !self.throws.sequencer.active() {
                self.cancel_throws();
                if module == ModuleId::AutoHeal {
                    self.heal_used(at);
                } else {
                    self.set_enabled(module, false);
                }
            }
            // "Throw bowls" drops only an actual bowl: on servers where soup
            // is eaten instantly the held stew has become one, on vanilla
            // (where stew must be held to eat) the stew is kept.
            let held_bowl = snapshot
                .hotbar(snapshot.vitals.selected)
                .and_then(|s| s.item.as_ref())
                .is_some_and(throw_items::is_bowl);
            return effects
                .into_iter()
                .filter(|e| *e != Effect::Drop || held_bowl)
                .map(host_effect)
                .collect();
        }
        for module in [ModuleId::ThrowPot, ModuleId::ThrowDebuff] {
            if !self.enabled(module) {
                continue;
            }
            // The reference refuses to start with a screen open.
            let uses = if playing {
                self.throw_plan(module, snapshot)
            } else {
                Vec::new()
            };
            if uses.is_empty() || !self.throw_claims() {
                self.set_enabled(module, false);
                continue;
            }
            let prefix = module.id();
            let delay = self.inventory_delay(
                &format!("{prefix}.delay"),
                if module == ModuleId::ThrowPot {
                    (80, 115)
                } else {
                    (70, 120)
                },
            );
            let scroll = self
                .inventory_bool(&format!("{prefix}.scroll"), false)
                .then(|| {
                    self.throw_number(&format!("{prefix}.scroll-delay"), 100.0)
                        .max(0.0) as u64
                });
            let throws = &mut self.throws;
            let queue = throw_items::sequence(
                uses,
                snapshot.vitals.selected,
                || throws.random(delay),
                scroll,
            );
            throws.sequencer.start(queue, now);
            throws.module = Some(module);
            return self.throw_frame(snapshot, playing);
        }
        if self.enabled(ModuleId::AutoHeal) && playing {
            let ready = self.heal_ready(at);
            let options = self.heal_options();
            // Silent (onTick): `CPacketHeldItemChange(slot)`, `sendUseItem`,
            // `CPacketHeldItemChange(current)`, then the delay restarts.
            if self.throw_str("inventory.auto-heal.mode") == "silent" {
                let Some(slot) = auto_heal::heal_slot(snapshot, &options).filter(|_| ready) else {
                    return Vec::new();
                };
                self.heal_used(at);
                return vec![ThrowEffect::SilentUse(slot)];
            }
            if let Some(slot) = auto_heal::heal_slot(snapshot, &options).filter(|_| ready)
                && self.throw_claims()
            {
                let saved = snapshot.vitals.selected;
                let swap = self.inventory_delay("inventory.auto-heal.swap-delay", (75, 125));
                let swap = self.throws.random(swap);
                let queue = [
                    throw_items::Action::Select(slot),
                    throw_items::Action::Use,
                    throw_items::Action::Wait(throw_items::PRESS_MS),
                    throw_items::Action::Wait(swap),
                    throw_items::Action::Select(saved),
                ];
                self.throws
                    .sequencer
                    .start(queue.into_iter().collect(), now);
                self.throws.module = Some(ModuleId::AutoHeal);
                return self.throw_frame(snapshot, playing);
            }
        }
        Vec::new()
    }
    fn throw_plan(&mut self, module: ModuleId, snapshot: &Snapshot) -> Vec<throw_items::Use> {
        let prefix = module.id();
        if module == ModuleId::ThrowDebuff {
            let mode = match self.throw_str(&format!("{prefix}.mode")) {
                "all" => DebuffMode::All,
                "first" => DebuffMode::First,
                _ => DebuffMode::OneOfEach,
            };
            let enabled = throw_items::DEBUFFS.map(|(label, _)| {
                self.inventory_bool(&format!("{prefix}.{}", label.to_lowercase()), true)
            });
            return throw_items::debuff_plan(snapshot, mode, enabled);
        }
        let kind = match self.throw_str(&format!("{prefix}.type")) {
            "pots" => PotKind::Pots,
            "soup" => PotKind::Soup,
            _ => PotKind::Both,
        };
        let mut order: Vec<u8> = (0..9).collect();
        if self.inventory_bool(&format!("{prefix}.random"), false) {
            for i in (1..order.len()).rev() {
                let j = self.throws.random((0, i as u64)) as usize;
                order.swap(i, j);
            }
        }
        throw_items::throwpot_plan(
            snapshot,
            kind,
            self.throw_str(&format!("{prefix}.mode")) != "single",
            self.inventory_bool(&format!("{prefix}.throw-bowls"), true),
            &order,
        )
    }
}
fn host_effect(effect: Effect) -> ThrowEffect {
    match effect {
        Effect::Select(slot) => ThrowEffect::Select(slot),
        Effect::Use => ThrowEffect::Use,
        Effect::Drop => ThrowEffect::Drop,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::inventory::{
        rules::tests::item, session::tests::snapshot, types::ItemClass,
    };
    use super::*;

    #[test]
    fn throwpot_runs_its_sequence_and_disables() {
        let mut runtime = Runtime::default();
        let t = Instant::now();
        let mut s = snapshot(false, t);
        s.vitals.health = 10.0;
        s.vitals.max_health = 20.0;
        s.slots[3].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        runtime.set_enabled(ModuleId::ThrowPot, true);
        assert_eq!(
            runtime.throw_frame(&s, true),
            vec![ThrowEffect::Select(3), ThrowEffect::Use]
        );
        s.vitals.selected = 3;
        s.slots[3].item = Some(item("bowl", ItemClass::Other, 0.0));
        let mut effects = Vec::new();
        for ms in 1..400 {
            s.observed = t + Duration::from_millis(ms);
            if effects.contains(&ThrowEffect::Select(0)) {
                s.vitals.selected = 0;
            }
            effects.extend(runtime.throw_frame(&s, true));
        }
        assert_eq!(effects, vec![ThrowEffect::Drop, ThrowEffect::Select(0)]);
        assert!(!runtime.enabled(ModuleId::ThrowPot));
        // Stew that was not eaten (vanilla) is not thrown away.
        s.vitals.selected = 0;
        s.slots[3].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        runtime.set_enabled(ModuleId::ThrowPot, true);
        s.observed = t + Duration::from_millis(1000);
        runtime.throw_frame(&s, true);
        s.vitals.selected = 3;
        let mut effects = Vec::new();
        for ms in 1001..1400 {
            s.observed = t + Duration::from_millis(ms);
            if effects.contains(&ThrowEffect::Select(0)) {
                s.vitals.selected = 0;
            }
            effects.extend(runtime.throw_frame(&s, true));
        }
        assert_eq!(effects, vec![ThrowEffect::Select(0)]);
    }
    #[test]
    fn manual_slot_change_abandons_the_sequence() {
        let mut runtime = Runtime::default();
        let t = Instant::now();
        let mut s = snapshot(false, t);
        s.slots[5].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        s.vitals.health = 5.0;
        s.vitals.max_health = 20.0;
        runtime
            .set_setting("inventory.auto-heal.soup", json!(true))
            .unwrap();
        runtime.set_enabled(ModuleId::AutoHeal, true);
        assert_eq!(
            runtime.throw_frame(&s, true),
            vec![ThrowEffect::Select(5), ThrowEffect::Use]
        );
        s.vitals.selected = 7;
        s.observed = t + Duration::from_millis(10);
        assert!(runtime.throw_frame(&s, true).is_empty());
        assert!(runtime.throws.module.is_none());
        assert!(runtime.enabled(ModuleId::AutoHeal));
    }
    fn silent_heal(runtime: &mut Runtime) {
        for (key, value) in [
            ("mode", json!("silent")),
            ("soup", json!(true)),
            ("delay", json!(500.0)),
        ] {
            runtime
                .set_setting(&format!("inventory.auto-heal.{key}"), value)
                .unwrap();
        }
        runtime.set_enabled(ModuleId::AutoHeal, true);
    }
    #[test]
    fn silent_uses_the_item_once_per_delay_without_selecting_it() {
        let mut runtime = Runtime::default();
        silent_heal(&mut runtime);
        let t = Instant::now();
        let mut s = snapshot(false, t);
        s.slots[4].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        s.vitals.health = 8.0;
        s.vitals.max_health = 20.0;
        // A screen stops it, as in the reference's onTick.
        assert!(runtime.throw_frame(&s, false).is_empty());
        assert_eq!(
            runtime.throw_frame(&s, true),
            vec![ThrowEffect::SilentUse(4)]
        );
        assert!(runtime.throws.module.is_none());
        let mut effects = Vec::new();
        for ms in 1..=500 {
            s.observed = t + Duration::from_millis(ms);
            effects.extend(runtime.throw_frame(&s, true));
        }
        assert_eq!(effects, vec![ThrowEffect::SilentUse(4)]);
        s.vitals.health = 18.0;
        s.observed = t + Duration::from_millis(2000);
        assert!(runtime.throw_frame(&s, true).is_empty());
    }
    #[test]
    fn potions_arm_at_pre_motion_and_fire_at_post_motion() {
        let mut runtime = Runtime::default();
        silent_heal(&mut runtime);
        for (key, value) in [("potions", json!(true)), ("replace", json!(true))] {
            runtime
                .set_setting(&format!("inventory.auto-heal.{key}"), value)
                .unwrap();
        }
        let t = Instant::now();
        let mut s = snapshot(false, t);
        let mut potion = item("splash_potion", ItemClass::Other, 0.0);
        potion
            .meta
            .effects
            .push(super::super::super::inventory::types::PotionEffect {
                id: "instant_health".into(),
                amplifier: 1,
                duration: 1,
            });
        s.slots[30].item = Some(potion);
        s.vitals.health = 8.0;
        s.vitals.max_health = 20.0;
        // In the air: Replace still swaps, nothing is armed.
        assert_eq!(
            runtime.auto_heal_pre_motion(&s),
            HealMotion {
                swap: Some((30, 5)),
                pitch: None
            }
        );
        assert_eq!(runtime.auto_heal_post_motion(t), None);
        s.slots[5].item = s.slots[30].item.take();
        s.vitals.on_ground = true;
        assert_eq!(
            runtime.auto_heal_pre_motion(&s),
            HealMotion {
                swap: None,
                pitch: Some(auto_heal::POTION_PITCH)
            }
        );
        assert_eq!(runtime.auto_heal_post_motion(t), Some(5));
        // The shared delay holds both the next potion and silent soup.
        s.observed = t + Duration::from_millis(100);
        assert_eq!(runtime.auto_heal_pre_motion(&s), HealMotion::default());
        assert_eq!(runtime.auto_heal_post_motion(s.observed), None);
        s.slots[4].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        assert!(runtime.throw_frame(&s, true).is_empty());
        s.observed = t + Duration::from_millis(500);
        assert_eq!(
            runtime.auto_heal_pre_motion(&s).pitch,
            Some(auto_heal::POTION_PITCH)
        );
        // Legit hides Potions: nothing arms or fires.
        runtime
            .set_setting("inventory.auto-heal.mode", json!("legit"))
            .unwrap();
        assert_eq!(runtime.auto_heal_post_motion(s.observed), None);
        assert_eq!(runtime.auto_heal_pre_motion(&s), HealMotion::default());
    }
}
