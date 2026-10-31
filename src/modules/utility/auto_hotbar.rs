// AutoHotbar: arranges hotbar slots from per-slot rules (item, any
// material, only best). Each move is one gated number-key swap
// (`SwapHotbar`); topping up a partial stack is a pickup merge.
use super::super::super::inventory::{
    arrange::{java_sort, place_carried},
    rules::Category,
    session::{Effect, Policy, Session, Step},
    types::{Click, Item, ItemClass, Slot, SlotRole, Snapshot},
};
use serde_json::Value;
use std::{cmp::Ordering, collections::BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct SlotRule {
    pub slot: u8,
    /// Namespaced item id.
    pub item: String,
    pub any_material: bool,
    pub only_best: bool,
}
/// Parses the `hotbar` rules setting:
/// `[{"slot":0,"item":"minecraft:diamond_sword","anyMaterial":true,"onlyBest":true}]`.
/// Legacy `item-id`/`meta`/`anyMeta` numeric fields have no modern meaning
/// and are ignored.
pub fn parse_rules(text: &str) -> Result<Vec<SlotRule>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut rules = Vec::new();
    for rule in value
        .as_array()
        .ok_or("hotbar rules must be a JSON array")?
    {
        let slot = rule
            .get("slot")
            .and_then(Value::as_u64)
            .filter(|s| *s < 9)
            .ok_or("hotbar rule needs slot 0..8")? as u8;
        let Some(item) = rule
            .get("item")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let item = if item.contains(':') {
            item.to_owned()
        } else {
            format!("minecraft:{item}")
        };
        if rules.iter().any(|r: &SlotRule| r.slot == slot) {
            continue;
        }
        rules.push(SlotRule {
            slot,
            item,
            any_material: rule
                .get("anyMaterial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            only_best: rule
                .get("onlyBest")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    rules.sort_by_key(|r| r.slot);
    Ok(rules)
}
/// Item kind standing in for the reference's item class equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Class(ItemClass),
    Armor,
    Food,
    Potion,
    Block,
    Other,
}
fn kind(item: &Item) -> Kind {
    if item.class != ItemClass::Other {
        Kind::Class(item.class)
    } else if item.armor.is_some() {
        Kind::Armor
    } else if item.meta.food.is_some() {
        Kind::Food
    } else if item.fingerprint.id.ends_with("potion") {
        Kind::Potion
    } else if item.meta.block {
        Kind::Block
    } else {
        Kind::Other
    }
}
/// `AutoHotbar.registerComparators`, keyed by the rule item's class. Hoes,
/// armor, blocks and other items have none: "Only best" takes the first
/// match there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Comparator {
    /// `InventoryManagerPrimaryItemScoreComparator` (swords, axes):
    /// `getWeaponDamageScore`.
    Primary,
    /// `BowSlotComparator` (pickaxes, shovels): `getToolDamageScore`.
    Tool,
    /// `InventoryManagerSecondaryItemScoreComparator` (bows):
    /// `getHiddenItemScore`.
    Secondary,
    /// `ArmorSlotComparator` (potions): amplifier comparisons summed over
    /// the effects both potions share.
    Potion,
    /// `FoodSlotComparator`: nutrition × saturation.
    Food,
}
fn comparator(kind: Kind) -> Option<Comparator> {
    Some(match kind {
        Kind::Class(ItemClass::Sword | ItemClass::Axe) => Comparator::Primary,
        Kind::Class(ItemClass::Pickaxe | ItemClass::Shovel) => Comparator::Tool,
        Kind::Class(ItemClass::Bow) => Comparator::Secondary,
        Kind::Potion => Comparator::Potion,
        Kind::Food => Comparator::Food,
        _ => return None,
    })
}
/// Sharpness's modern damage bonus (`EnchantmentHelper` in both scores).
fn sharpness_bonus(item: &Item) -> f64 {
    let level = item.meta.enchantment("sharpness");
    if level > 0 {
        0.5 * f64::from(level) + 0.5
    } else {
        0.0
    }
}
impl Comparator {
    fn compare(self, a: &Item, b: &Item) -> Ordering {
        let score = |item: &Item| match self {
            // The summed main-hand modifiers of a tool or sword, the
            // sharpness bonus and a hundredth per Fire Aspect level.
            Self::Primary => {
                f64::from(item.meta.mainhand_modifiers)
                    + sharpness_bonus(item)
                    + 0.01 * f64::from(item.meta.enchantment("fire_aspect"))
            }
            // The first attribute modifier's amount and the sharpness bonus.
            Self::Tool => f64::from(item.meta.first_modifier) + sharpness_bonus(item),
            // `ItemStackScoreUtil`'s bow score, InventoryManager's
            // "Best overall".
            Self::Secondary => Category::BowBest.score(item).unwrap_or(0.0),
            Self::Potion | Self::Food => 0.0,
        };
        match self {
            Self::Potion => {
                let sum: i32 = a
                    .meta
                    .effects
                    .iter()
                    .flat_map(|x| {
                        b.meta
                            .effects
                            .iter()
                            .filter(move |y| y.id == x.id)
                            .map(move |y| x.amplifier.cmp(&y.amplifier) as i32)
                    })
                    .sum();
                sum.cmp(&0)
            }
            // `Float.compare` of float products.
            Self::Food => {
                let product = |item: &Item| {
                    item.meta
                        .food
                        .map_or(0.0, |(nutrition, saturation)| nutrition as f32 * saturation)
                };
                product(a).total_cmp(&product(b))
            }
            _ => score(a).total_cmp(&score(b)),
        }
    }
}
/// The kind a rule's own item id names without an instance: the tool and
/// weapon classes and armor pieces, from the same id suffixes the host
/// bridge classifies stacks by. Other kinds (food, potions, blocks) need
/// the item's components, so they come from a matching stack.
fn kind_of_id(id: &str) -> Option<Kind> {
    let class = [
        ("_sword", ItemClass::Sword),
        ("_bow", ItemClass::Bow),
        (":bow", ItemClass::Bow),
        ("_pickaxe", ItemClass::Pickaxe),
        ("_axe", ItemClass::Axe),
        ("_shovel", ItemClass::Shovel),
        ("_hoe", ItemClass::Hoe),
    ]
    .into_iter()
    .find(|(suffix, _)| id.ends_with(suffix));
    if let Some((_, class)) = class {
        return Some(Kind::Class(class));
    }
    ["_helmet", "_chestplate", "_leggings", "_boots"]
        .iter()
        .any(|suffix| id.ends_with(suffix))
        .then_some(Kind::Armor)
}
impl SlotRule {
    /// The reference compares the candidate's item class with the rule
    /// item's class, so the rule needs no copy of its item in the inventory.
    fn exemplar_kind(&self, snapshot: &Snapshot) -> Option<Kind> {
        kind_of_id(&self.item).or_else(|| {
            snapshot
                .slots(|_| true)
                .filter_map(|s| s.item.as_ref())
                .find(|i| i.fingerprint.id == self.item)
                .map(kind)
        })
    }
    fn matches(&self, item: &Item, exemplar: Option<Kind>) -> bool {
        item.fingerprint.id == self.item
            || (self.any_material && exemplar.is_some_and(|k| kind(item) == k && k != Kind::Other))
    }
}

#[derive(Debug, Default)]
pub struct AutoHotbar {
    session: Session,
    origin: Option<u16>,
    /// `touchedSlots`: settled rule targets and both ends of every issued
    /// move, kept for the rest of the run.
    touched: BTreeSet<u16>,
}
impl AutoHotbar {
    pub fn cancel(&mut self) {
        self.session.cancel();
        self.origin = None;
        self.touched.clear();
    }
    pub fn opened(&self) -> bool {
        self.session.opened()
    }
    pub fn active(&self) -> bool {
        self.session.active()
    }
    pub fn step(&mut self, snapshot: &Snapshot, rules: &[SlotRule], policy: Policy) -> Step {
        let mut touched = self.touched.clone();
        let next = next_click(snapshot, rules, &mut touched, &mut self.origin);
        let step = self.session.step(snapshot, policy, next);
        match step {
            Step::Done => {
                self.origin = None;
                self.touched.clear();
            }
            // As in the reference, slots count as touched once their click
            // is queued, not while the inventory is still being opened.
            Step::Effect(Effect::Request(_)) => self.touched = touched,
            _ => {}
        }
        step
    }
}

/// `container.getInventorySlots()` of the player menu in menu order: armor,
/// main inventory, hotbar, offhand (the crafting grid is not copied).
fn menu_order(snapshot: &Snapshot) -> impl Iterator<Item = &Slot> {
    let role = |pick: fn(SlotRole) -> bool| snapshot.slots(pick);
    role(|r| matches!(r, SlotRole::Armor(_)))
        .chain(role(|r| r == SlotRole::Main))
        .chain(role(|r| matches!(r, SlotRole::Hotbar(_))))
        .chain(role(|r| r == SlotRole::Other))
}
/// `findBestMatchingSlot`: the target first when it matches, then every
/// untouched matching menu slot. "Only best" with a comparator for the
/// rule's class sorts ascending, reverses, and keeps the target (`None`)
/// when the best compares equal to it; otherwise the first match wins.
fn best_matching<'a>(
    snapshot: &'a Snapshot,
    target: &'a Slot,
    rule: &SlotRule,
    exemplar: Option<Kind>,
    touched: &BTreeSet<u16>,
) -> Option<&'a Slot> {
    let matches = |slot: &Slot| {
        slot.item
            .as_ref()
            .is_some_and(|i| rule.matches(i, exemplar))
    };
    let current = matches(target).then_some(target);
    let mut list: Vec<&Slot> = current
        .into_iter()
        .chain(
            menu_order(snapshot)
                .filter(|s| s.index != target.index && !touched.contains(&s.index) && matches(s)),
        )
        .collect();
    if rule.only_best
        && let Some(comparator) = exemplar.and_then(comparator)
    {
        let compare = |a: &&Slot, b: &&Slot| {
            comparator.compare(a.item.as_ref().unwrap(), b.item.as_ref().unwrap())
        };
        java_sort(&mut list, compare);
        list.reverse();
        if let (Some(current), Some(best)) = (current, list.first())
            && compare(best, &current) == Ordering::Equal
        {
            return None;
        }
    }
    list.first().copied()
}

pub fn next_click(
    snapshot: &Snapshot,
    rules: &[SlotRule],
    touched: &mut BTreeSet<u16>,
    origin: &mut Option<u16>,
) -> Option<(u16, Click)> {
    if let Some(carried) = snapshot.carried.as_ref() {
        // Finishing a top-up: merge into the rule slot holding this kind.
        for rule in rules {
            if let Some(target) = snapshot.hotbar(rule.slot)
                && target.item.as_ref().is_some_and(|i| {
                    i.same_stack_kind(carried) && i.fingerprint.count < i.max_count
                })
                && Some(target.index) != *origin
            {
                return Some((target.index, Click::Pickup));
            }
        }
        return place_carried(snapshot, *origin);
    }
    for rule in rules {
        let Some(target) = snapshot.hotbar(rule.slot) else {
            continue;
        };
        let exemplar = rule.exemplar_kind(snapshot);
        let Some(source) = best_matching(snapshot, target, rule, exemplar, touched) else {
            continue;
        };
        touched.insert(target.index);
        if source.index == target.index {
            // Already placed: top up a partial stack from the untouched
            // slots after the armor (`findOverflowSlots`), with the same
            // kind of stack.
            let item = target.item.as_ref().unwrap();
            if item.fingerprint.count < item.max_count
                && let Some(extra) = menu_order(snapshot).find(|s| {
                    !matches!(s.role, SlotRole::Armor(_))
                        && !touched.contains(&s.index)
                        && s.item
                            .as_ref()
                            .is_some_and(|i| i.same_stack_kind(item) && rule.matches(i, exemplar))
                })
            {
                touched.insert(extra.index);
                *origin = Some(extra.index);
                return Some((extra.index, Click::Pickup));
            }
            continue;
        }
        touched.insert(source.index);
        return Some((source.index, Click::SwapHotbar(rule.slot)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        rules::tests::item,
        session::tests::snapshot,
        types::{ArmorSlot, PotionEffect},
    };

    /// A weapon with vanilla-like main-hand modifiers (attack damage and
    /// attack speed).
    fn weapon(id: &str, class: ItemClass, damage: f32, speed: f32) -> Item {
        let mut item = item(id, class, damage + 1.0);
        item.meta.mainhand_modifiers = damage + speed;
        item.meta.first_modifier = damage;
        item
    }
    fn next(s: &Snapshot, rules: &[SlotRule]) -> Option<(u16, Click)> {
        next_click(s, rules, &mut BTreeSet::new(), &mut None)
    }

    #[test]
    fn swaps_best_matching_item_and_tops_up_stacks() {
        let rules = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_sword","anyMaterial":true,"onlyBest":true},
                {"slot":1,"item":"cobblestone"}]"#,
        )
        .unwrap();
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[10].item = Some(weapon("iron_sword", ItemClass::Sword, 5.0, -2.4));
        s.slots[11].item = Some(weapon("diamond_sword", ItemClass::Sword, 6.0, -2.4));
        assert_eq!(next(&s, &rules), Some((11, Click::SwapHotbar(0))));
        s.slots[0].item = s.slots[11].item.take();
        assert_eq!(next(&s, &rules[..1]), None);
        let mut cobble = item("cobblestone", ItemClass::Other, 0.0);
        cobble.max_count = 64;
        cobble.fingerprint.count = 20;
        s.slots[1].item = Some(cobble.clone());
        s.slots[12].item = Some(cobble);
        let mut origin = None;
        let mut touched = BTreeSet::new();
        assert_eq!(
            next_click(&s, &rules, &mut touched, &mut origin),
            Some((12, Click::Pickup))
        );
        s.carried = s.slots[12].item.take();
        assert_eq!(
            next_click(&s, &rules, &mut touched, &mut origin),
            Some((1, Click::Pickup))
        );
    }

    #[test]
    fn any_material_needs_no_copy_of_the_rule_item() {
        // A diamond-sword rule with no diamond sword anywhere: the best
        // sword of any material replaces the worse one in the slot.
        let rules = parse_rules(
            r#"[{"slot":0,"item":"minecraft:diamond_sword","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[0].item = Some(weapon("wooden_sword", ItemClass::Sword, 3.0, -2.4));
        s.slots[10].item = Some(weapon("iron_sword", ItemClass::Sword, 5.0, -2.4));
        s.slots[11].item = Some(weapon("stone_sword", ItemClass::Sword, 4.0, -2.4));
        assert_eq!(next(&s, &rules), Some((10, Click::SwapHotbar(0))));
        // Without "Any material" only the exact item counts.
        let exact = parse_rules(r#"[{"slot":0,"item":"minecraft:diamond_sword"}]"#).unwrap();
        assert_eq!(next(&s, &exact), None);
    }

    #[test]
    fn only_best_takes_the_last_equal_best_and_keeps_an_equal_target() {
        let rules = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_sword","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[0].item = Some(weapon("stone_sword", ItemClass::Sword, 4.0, -2.4));
        // Equal bests in main slot 10 and hotbar slot 5: the stable
        // ascending sort keeps menu order, the reverse puts the hotbar one
        // (later in the menu) first.
        s.slots[10].item = Some(weapon("iron_sword", ItemClass::Sword, 5.0, -2.4));
        s.slots[5].item = Some(weapon("iron_sword", ItemClass::Sword, 5.0, -2.4));
        assert_eq!(next(&s, &rules), Some((5, Click::SwapHotbar(0))));
        // Sharpness I (+1) only ties them (main slot 12 is still ahead of
        // hotbar slot 5 in the menu); Fire Aspect adds a hundredth.
        let mut sharp = weapon("stone_sword", ItemClass::Sword, 4.0, -2.4);
        sharp.meta.enchantments = vec![("minecraft:sharpness".into(), 1)];
        s.slots[12].item = Some(sharp.clone());
        assert_eq!(next(&s, &rules), Some((5, Click::SwapHotbar(0))));
        sharp.meta.enchantments = vec![("minecraft:fire_aspect".into(), 1)];
        s.slots[12].item = Some(sharp.clone());
        assert_eq!(next(&s, &rules), Some((5, Click::SwapHotbar(0))));
        sharp.meta.enchantments = vec![
            ("minecraft:sharpness".into(), 1),
            ("minecraft:fire_aspect".into(), 1),
        ];
        s.slots[12].item = Some(sharp);
        assert_eq!(next(&s, &rules), Some((12, Click::SwapHotbar(0))));
        // A target equal to the best stays, and is not marked touched.
        s.slots[12].item = None;
        s.slots[0].item = Some(weapon("iron_sword", ItemClass::Sword, 5.0, -2.4));
        let mut touched = BTreeSet::new();
        assert_eq!(next_click(&s, &rules, &mut touched, &mut None), None);
        assert!(touched.is_empty());
        // The score sums every main-hand modifier, attack speed included.
        s.slots[10].item = Some(weapon("iron_axe", ItemClass::Axe, 8.0, -3.1));
        s.slots[5].item = None;
        let axes = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_axe","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        s.slots[0].item = Some(weapon("stone_axe", ItemClass::Axe, 8.0, -3.2));
        assert_eq!(next(&s, &axes), Some((10, Click::SwapHotbar(0))));
    }

    #[test]
    fn tools_compare_their_first_modifier_and_hoes_take_the_first_match() {
        let pickaxes = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_pickaxe","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        let mut s = snapshot(true, std::time::Instant::now());
        let mut golden = weapon("golden_pickaxe", ItemClass::Pickaxe, 1.0, -2.8);
        golden.meta.mining_speed = 12.0;
        let mut iron = weapon("iron_pickaxe", ItemClass::Pickaxe, 3.0, -2.8);
        iron.meta.mining_speed = 6.0;
        s.slots[10].item = Some(golden);
        s.slots[11].item = Some(iron);
        assert_eq!(next(&s, &pickaxes), Some((11, Click::SwapHotbar(0))));
        let hoes = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_hoe","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        s.slots[10].item = Some(weapon("wooden_hoe", ItemClass::Hoe, 0.0, -3.0));
        s.slots[11].item = Some(weapon("diamond_hoe", ItemClass::Hoe, 0.0, 0.0));
        assert_eq!(next(&s, &hoes), Some((10, Click::SwapHotbar(0))));
    }

    #[test]
    fn food_compares_nutrition_times_saturation_and_potions_their_amplifiers() {
        let mut s = snapshot(true, std::time::Instant::now());
        let food = |id: &str, nutrition, saturation| {
            let mut i = item(id, ItemClass::Other, 0.0);
            i.max_count = 64;
            i.meta.food = Some((nutrition, saturation));
            i
        };
        s.slots[10].item = Some(food("golden_apple", 4, 9.6));
        s.slots[11].item = Some(food("cooked_beef", 8, 12.8));
        s.slots[12].item = Some(food("bread", 5, 6.0));
        let rules = parse_rules(
            r#"[{"slot":0,"item":"minecraft:golden_apple","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        assert_eq!(next(&s, &rules), Some((11, Click::SwapHotbar(0))));
        let potion = |id: &str, effects: &[(&str, i32)]| {
            let mut i = item(id, ItemClass::Other, 0.0);
            i.meta.effects = effects
                .iter()
                .map(|&(id, amplifier)| PotionEffect {
                    id: id.into(),
                    amplifier,
                    duration: 1800,
                })
                .collect();
            i
        };
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots[10].item = Some(potion("splash_potion", &[("speed", 0)]));
        s.slots[11].item = Some(potion("splash_potion", &[("speed", 1)]));
        s.slots[12].item = Some(potion("splash_potion", &[("strength", 1)]));
        let rules = parse_rules(
            r#"[{"slot":0,"item":"minecraft:splash_potion","anyMaterial":true,"onlyBest":true}]"#,
        )
        .unwrap();
        // Strength shares no effect with the speed potions, so it compares
        // equal to both: the sort keeps menu order and the reverse puts
        // Strength first, ahead of Speed II.
        let best = next(&s, &rules);
        let mut list = vec![10u16, 11, 12];
        let cmp = |a: &u16, b: &u16| {
            Comparator::Potion.compare(
                s.slot(*a).unwrap().item.as_ref().unwrap(),
                s.slot(*b).unwrap().item.as_ref().unwrap(),
            )
        };
        java_sort(&mut list, cmp);
        assert_eq!(list, [10, 11, 12]);
        assert_eq!(best, Some((12, Click::SwapHotbar(0))));
    }

    #[test]
    fn candidates_follow_menu_order_and_touched_slots_persist() {
        let mut s = snapshot(true, std::time::Instant::now());
        s.slots.push(Slot {
            index: 500,
            role: SlotRole::Armor(ArmorSlot::Head),
            item: None,
        });
        s.slots.push(Slot {
            index: 504,
            role: SlotRole::Other,
            item: Some(weapon("iron_sword", ItemClass::Sword, 5.0, -2.4)),
        });
        let mut helmet = item("iron_helmet", ItemClass::Other, 0.0);
        helmet.armor = Some(ArmorSlot::Head);
        s.slots[14].item = Some(helmet.clone());
        s.slots[36].item = Some(helmet);
        // Armor slots come first in the menu, then main, hotbar, offhand.
        let helmets = parse_rules(r#"[{"slot":2,"item":"minecraft:iron_helmet"}]"#).unwrap();
        assert_eq!(next(&s, &helmets), Some((500, Click::SwapHotbar(2))));
        // The offhand sword is the only sword.
        let swords = parse_rules(
            r#"[{"slot":0,"item":"minecraft:iron_sword","anyMaterial":true,"onlyBest":true},
                {"slot":1,"item":"minecraft:iron_sword"}]"#,
        )
        .unwrap();
        let mut touched = BTreeSet::new();
        assert_eq!(
            next_click(&s, &swords, &mut touched, &mut None),
            Some((504, Click::SwapHotbar(0)))
        );
        assert_eq!(touched, BTreeSet::from([0, 504]));
        // After the swap the first rule keeps its equal best without
        // touching it again, but slot 0 stays touched for the run, so the
        // second rule cannot take its sword.
        s.slots[0].item = s.slots[37].item.take();
        assert_eq!(next_click(&s, &swords, &mut touched, &mut None), None);
        assert_eq!(next(&s, &swords), Some((0, Click::SwapHotbar(1))));
    }
}
