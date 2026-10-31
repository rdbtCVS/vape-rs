// AutoHeal: Legit mode uses a hotbar healing item (soup, or the custom
// "Healing Items") below the health threshold through the shared use
// sequencer; Silent uses it with a server-only held-item change. Silent's
// "Potions" throws a splash potion from "Slot" at PreMotion/PostMotion. The
// inventory side handles empty bowls ("Bowl Mode" Throw or Stack into slot
// 17) and "Replace" (moves healing items from the main inventory into empty
// hotbar slots) one gated click at a time.
use super::super::super::inventory::{
    arrange::place_carried,
    session::{Policy, Session, Step},
    types::{Click, Item, SlotRole, Snapshot, Vitals},
};
use super::throw_items::{is_bowl, is_soup};
use std::collections::BTreeSet;

/// The reference's bowl stack slot (container slot 17, a main row slot).
pub const BOWL_SLOT: u16 = 17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BowlMode {
    Stack,
    Throw,
    None,
}
#[derive(Clone, Debug)]
pub struct Options {
    pub soup: bool,
    pub bowl_mode: BowlMode,
    pub custom_items: bool,
    pub healing_items: BTreeSet<String>,
    pub replace: bool,
    /// Health threshold in HP (of 20).
    pub health: f32,
}
pub fn is_healing(item: &Item, options: &Options) -> bool {
    (options.soup && is_soup(item))
        || (options.custom_items && options.healing_items.contains(&item.fingerprint.id))
}
/// The first hotbar healing item when health is at or under the threshold
/// (the reference compares health / max with threshold / 20).
pub fn heal_slot(snapshot: &Snapshot, options: &Options) -> Option<u8> {
    let v = &snapshot.vitals;
    if v.health <= 0.0 || v.health / v.max_health.max(1.0) > options.health / 20.0 {
        return None;
    }
    (0..9).find(|slot| {
        snapshot
            .hotbar(*slot)
            .and_then(|s| s.item.as_ref())
            .is_some_and(|i| is_healing(i, options))
    })
}

/// The pitch the reference's PreMotion sends while a potion is pending.
pub const POTION_PITCH: f32 = 88.99;

/// "Potions" (shown, so effective, only in Silent): "Regen", "Speed",
/// "Resistance", the hotbar "Slot" a potion is swapped into, and "Replace".
#[derive(Clone, Copy, Debug)]
pub struct Potions {
    pub regen: bool,
    pub speed: bool,
    pub resistance: bool,
    /// Zero-based hotbar slot ("Slot" - 1).
    pub slot: u8,
    pub replace: bool,
    /// Health threshold in HP; potions compare raw health with it.
    pub health: f32,
}
fn has(vitals: &Vitals, effect: &str) -> bool {
    vitals.effects.iter().any(|e| e == effect)
}
/// `isHealingPotion`: a splash potion holding instant health (health at or
/// under the threshold), speed ("Speed" with no speed effect) or
/// regeneration ("Regen", at or under the threshold, no regeneration
/// effect). A resistance potion never matches, as in the reference.
pub fn is_healing_potion(item: &Item, vitals: &Vitals, potions: &Potions) -> bool {
    if item.fingerprint.id != "minecraft:splash_potion" {
        return false;
    }
    let low = vitals.health <= potions.health;
    item.meta.effects.iter().any(|e| match e.id.as_str() {
        "instant_health" => low,
        "speed" => potions.speed && !has(vitals, "speed"),
        "regeneration" => potions.regen && low && !has(vitals, "regeneration"),
        _ => false,
    })
}
/// PreMotion's `needsPotion`: low health (unless "Regen" and already
/// regenerating), or a missing speed/resistance effect that is asked for;
/// only on the ground (`onGround && isCollidedVertically`, which one
/// `Entity.move` sets together).
pub fn needs_potion(vitals: &Vitals, potions: &Potions) -> bool {
    let wanted = vitals.health <= potions.health
        && (!potions.regen || !has(vitals, "regeneration"))
        || potions.speed && !has(vitals, "speed")
        || potions.resistance && !has(vitals, "resistance");
    wanted && vitals.on_ground
}
/// `findHealingPotionSlot`'s result: the hotbar slot to use, and the slot
/// a window-0 SWAP click (button `hotbar`) brings the potion from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PotionPick {
    pub hotbar: u8,
    pub swap: Option<u16>,
}
/// `findHealingPotionSlot`: the first healing potion in player-menu order
/// (the main rows, then the hotbar). The reference stops at it: without
/// "Replace" there is none, with it the potion is swapped into "Slot" (a
/// potion already there needs no click; the reference's same-slot SWAP
/// changes nothing). The 2x2 crafting grid (the reference's `slot < 8`
/// branch) is not scanned.
pub fn potion_slot(snapshot: &Snapshot, potions: &Potions) -> Option<PotionPick> {
    let found = (9..36).chain(0..9).find(|i| {
        snapshot
            .slot(*i)
            .and_then(|s| s.item.as_ref())
            .is_some_and(|item| is_healing_potion(item, &snapshot.vitals, potions))
    })?;
    potions.replace.then_some(PotionPick {
        hotbar: potions.slot,
        swap: (found != u16::from(potions.slot)).then_some(found),
    })
}

/// Next inventory click and whether it may open the inventory (only
/// Replace does, as in the reference).
pub fn next_click(
    snapshot: &Snapshot,
    options: &Options,
    origin: &mut Option<u16>,
) -> Option<((u16, Click), bool)> {
    if snapshot.carried.is_some() {
        let bowl = snapshot.carried.as_ref().is_some_and(is_bowl);
        if bowl
            && *origin != Some(BOWL_SLOT)
            && snapshot.slot(BOWL_SLOT).is_some_and(|s| {
                // Empty, a non-full bowl stack, or another item (the
                // pickup swaps it out; it then goes back to the origin).
                s.item
                    .as_ref()
                    .is_none_or(|i| !is_bowl(i) || i.fingerprint.count < i.max_count)
            })
        {
            return Some(((BOWL_SLOT, Click::Pickup), false));
        }
        return place_carried(snapshot, *origin).map(|c| (c, false));
    }
    if options.soup && options.bowl_mode != BowlMode::None {
        let bowl_slot = snapshot.slot(BOWL_SLOT);
        let stacked = bowl_slot.and_then(|s| s.item.as_ref()).is_some_and(is_bowl);
        for slot in snapshot.slots(|r| matches!(r, SlotRole::Main | SlotRole::Hotbar(_))) {
            let Some(item) = slot.item.as_ref().filter(|i| is_bowl(i)) else {
                continue;
            };
            if options.bowl_mode == BowlMode::Throw {
                return Some(((slot.index, Click::ThrowStack), false));
            }
            if slot.index == BOWL_SLOT {
                continue;
            }
            let full = bowl_slot.and_then(|s| s.item.as_ref()).is_some_and(|i| {
                u16::from(i.fingerprint.count) + u16::from(item.fingerprint.count)
                    > u16::from(i.max_count)
            });
            if stacked && full {
                continue;
            }
            return Some(match (slot.role, stacked) {
                // Hotbar bowl onto slot 17: a number-key swap when 17 holds
                // something else, a shift-click merge when it holds bowls.
                (SlotRole::Hotbar(n), false) => ((BOWL_SLOT, Click::SwapHotbar(n)), false),
                (SlotRole::Hotbar(_), true) => ((slot.index, Click::QuickMove), false),
                _ => {
                    *origin = Some(slot.index);
                    ((slot.index, Click::Pickup), false)
                }
            });
        }
    }
    if options.replace {
        let empty = snapshot
            .slots(|r| matches!(r, SlotRole::Hotbar(_)))
            .find(|s| s.item.is_none());
        let source = snapshot
            .slots(|r| r == SlotRole::Main)
            .find(|s| s.item.as_ref().is_some_and(|i| is_healing(i, options)));
        if let (Some(empty), Some(source)) = (empty, source)
            && let SlotRole::Hotbar(n) = empty.role
        {
            return Some(((source.index, Click::SwapHotbar(n)), true));
        }
    }
    None
}

#[derive(Debug, Default)]
pub struct AutoHeal {
    session: Session,
    origin: Option<u16>,
}
impl AutoHeal {
    pub fn cancel(&mut self) {
        self.session.cancel();
        self.origin = None;
    }
    pub fn opened(&self) -> bool {
        self.session.opened()
    }
    pub fn active(&self) -> bool {
        self.session.active()
    }
    pub fn step(&mut self, snapshot: &Snapshot, options: &Options) -> Step {
        let next = next_click(snapshot, options, &mut self.origin);
        let policy = Policy {
            open: next.is_some_and(|(_, open)| open),
            ..Policy::default()
        };
        let step = self.session.step(snapshot, policy, next.map(|(c, _)| c));
        if step == Step::Done {
            self.origin = None;
        }
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{rules::tests::item, session::tests::snapshot, types::ItemClass};

    fn options() -> Options {
        Options {
            soup: true,
            bowl_mode: BowlMode::Stack,
            custom_items: false,
            healing_items: BTreeSet::new(),
            replace: true,
            health: 17.0,
        }
    }
    fn bowl(count: u8) -> Item {
        let mut i = item("bowl", ItemClass::Other, 0.0);
        i.max_count = 64;
        i.fingerprint.count = count;
        i
    }
    #[test]
    fn heals_below_threshold_and_stacks_bowls() {
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[20].item = None;
        s.vitals.health = 16.0;
        s.vitals.max_health = 20.0;
        s.slots[3].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        assert_eq!(heal_slot(&s, &options()), Some(3));
        s.vitals.health = 18.0;
        assert_eq!(heal_slot(&s, &options()), None);
        s.slots[2].item = Some(bowl(1));
        assert_eq!(
            next_click(&s, &options(), &mut None),
            Some(((BOWL_SLOT, Click::SwapHotbar(2)), false))
        );
        s.slots[17].item = s.slots[2].item.take();
        s.slots[25].item = Some(bowl(1));
        let mut origin = None;
        assert_eq!(
            next_click(&s, &options(), &mut origin),
            Some(((25, Click::Pickup), false))
        );
        s.carried = s.slots[25].item.take();
        assert_eq!(
            next_click(&s, &options(), &mut origin),
            Some(((BOWL_SLOT, Click::Pickup), false))
        );
    }
    fn potions() -> Potions {
        Potions {
            regen: true,
            speed: false,
            resistance: false,
            slot: 5,
            replace: true,
            health: 17.0,
        }
    }
    fn splash(effect: &str) -> Item {
        let mut i = item("splash_potion", ItemClass::Other, 0.0);
        i.meta.effects.push(crate::inventory::types::PotionEffect {
            id: effect.into(),
            amplifier: 0,
            duration: 1,
        });
        i
    }
    #[test]
    fn healing_potions_follow_the_reference_conditions() {
        let mut v = Vitals {
            health: 10.0,
            max_health: 20.0,
            ..Vitals::default()
        };
        let p = potions();
        assert!(is_healing_potion(&splash("instant_health"), &v, &p));
        assert!(is_healing_potion(&splash("regeneration"), &v, &p));
        assert!(!is_healing_potion(&splash("speed"), &v, &p));
        assert!(!is_healing_potion(
            &splash("resistance"),
            &v,
            &Potions {
                resistance: true,
                ..p
            }
        ));
        assert!(is_healing_potion(
            &splash("speed"),
            &v,
            &Potions { speed: true, ..p }
        ));
        let mut drink = splash("instant_health");
        drink.fingerprint.id = "minecraft:potion".into();
        assert!(!is_healing_potion(&drink, &v, &p));
        v.effects.push("regeneration".into());
        assert!(!is_healing_potion(&splash("regeneration"), &v, &p));
        v.health = 18.0;
        assert!(!is_healing_potion(&splash("instant_health"), &v, &p));
    }
    #[test]
    fn a_potion_is_needed_only_on_the_ground() {
        let mut v = Vitals {
            health: 10.0,
            max_health: 20.0,
            on_ground: true,
            ..Vitals::default()
        };
        assert!(needs_potion(&v, &potions()));
        v.effects.push("regeneration".into());
        assert!(!needs_potion(&v, &potions()));
        assert!(needs_potion(
            &v,
            &Potions {
                regen: false,
                ..potions()
            }
        ));
        assert!(needs_potion(
            &v,
            &Potions {
                resistance: true,
                ..potions()
            }
        ));
        v.on_ground = false;
        assert!(!needs_potion(
            &v,
            &Potions {
                regen: false,
                ..potions()
            }
        ));
    }
    #[test]
    fn the_first_potion_in_menu_order_is_swapped_into_slot() {
        let mut s = snapshot(false, std::time::Instant::now());
        s.vitals.health = 10.0;
        s.slots[2].item = Some(splash("instant_health"));
        s.slots[30].item = Some(splash("instant_health"));
        // The main rows come before the hotbar.
        assert_eq!(
            potion_slot(&s, &potions()),
            Some(PotionPick {
                hotbar: 5,
                swap: Some(30)
            })
        );
        s.slots[30].item = None;
        assert_eq!(
            potion_slot(&s, &potions()),
            Some(PotionPick {
                hotbar: 5,
                swap: Some(2)
            })
        );
        s.slots[5].item = s.slots[2].item.take();
        assert_eq!(
            potion_slot(&s, &potions()),
            Some(PotionPick {
                hotbar: 5,
                swap: None
            })
        );
        // Without Replace the reference finds none, even in the hotbar.
        assert_eq!(
            potion_slot(
                &s,
                &Potions {
                    replace: false,
                    ..potions()
                }
            ),
            None
        );
        s.vitals.health = 18.0;
        assert_eq!(potion_slot(&s, &potions()), None);
    }
    #[test]
    fn replace_moves_soup_to_an_empty_hotbar_slot() {
        let mut s = snapshot(false, std::time::Instant::now());
        for i in 0..9 {
            s.slots[i].item = Some(item("stone", ItemClass::Other, 0.0));
        }
        s.slots[4].item = None;
        s.slots[30].item = Some(item("mushroom_stew", ItemClass::Other, 0.0));
        assert_eq!(
            next_click(&s, &options(), &mut None),
            Some(((30, Click::SwapHotbar(4)), true))
        );
    }
}
