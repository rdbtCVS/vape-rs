// InventoryManager rule model: item matchers, priority categories, custom
// rule presets (condition groups) and profiles, with the reference's JSON
// shape. Everything here is pure; slots are judged from host-copied items.
use super::types::{ArmorSlot, Item, ItemClass};
use serde_json::{Value, json};
use std::cmp::Ordering;

/// Picker group a matcher is listed under.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    Hidden,
    Weapons,
    Tools,
    Food,
    Blocks,
    Armor,
}
impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hidden => "Hidden",
            Self::Weapons => "Weapons",
            Self::Tools => "Tools",
            Self::Food => "Food",
            Self::Blocks => "Blocks",
            Self::Armor => "Armor",
        }
    }
}

/// The reference's registered item matchers, in registry order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Matcher {
    Hand,
    AnyItem,
    AnyBow,
    AnyPotion,
    Weapons,
    Swords,
    AxeWeapon,
    AnyTool,
    Pickaxe,
    AxeTool,
    Shovel,
    Hoe,
    AnyFood,
    CookedFood,
    AnyBlock,
    BuildingBlocks,
    AnyArmor,
}
impl Matcher {
    pub const ALL: [Self; 17] = [
        Self::Hand,
        Self::AnyItem,
        Self::AnyBow,
        Self::AnyPotion,
        Self::Weapons,
        Self::Swords,
        Self::AxeWeapon,
        Self::AnyTool,
        Self::Pickaxe,
        Self::AxeTool,
        Self::Shovel,
        Self::Hoe,
        Self::AnyFood,
        Self::CookedFood,
        Self::AnyBlock,
        Self::BuildingBlocks,
        Self::AnyArmor,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Hand => "hand",
            Self::AnyItem => "any-item",
            Self::AnyBow => "any-bow",
            Self::AnyPotion => "any-potion",
            Self::Weapons => "weapons",
            Self::Swords => "swords",
            Self::AxeWeapon => "axe-weapon",
            Self::AnyTool => "any-tool",
            Self::Pickaxe => "pickaxe",
            Self::AxeTool => "axe-tool",
            Self::Shovel => "shovel",
            Self::Hoe => "hoe",
            Self::AnyFood => "any-food",
            Self::CookedFood => "cooked-food",
            Self::AnyBlock => "any-block",
            Self::BuildingBlocks => "building-blocks",
            Self::AnyArmor => "any-armor",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Hand => "Hand",
            Self::AnyItem => "Any Item",
            Self::AnyBow => "Any bow",
            Self::AnyPotion => "Any potion",
            Self::Weapons => "Any Weapon",
            Self::Swords => "Sword",
            Self::AxeWeapon | Self::AxeTool => "Axe",
            Self::AnyTool => "Any tool",
            Self::Pickaxe => "Pickaxe",
            Self::Shovel => "Shovel",
            Self::Hoe => "Hoe",
            Self::AnyFood => "Any food",
            Self::CookedFood => "Cooked food",
            Self::AnyBlock => "Any block",
            Self::BuildingBlocks => "Building blocks",
            Self::AnyArmor => "Any armor",
        }
    }
    pub fn group(self) -> Group {
        match self {
            Self::Hand | Self::AnyItem | Self::AnyBow | Self::AnyPotion | Self::AnyTool => {
                Group::Hidden
            }
            Self::Weapons | Self::Swords | Self::AxeWeapon => Group::Weapons,
            Self::Pickaxe | Self::AxeTool | Self::Shovel | Self::Hoe => Group::Tools,
            Self::AnyFood | Self::CookedFood => Group::Food,
            Self::AnyBlock | Self::BuildingBlocks => Group::Blocks,
            Self::AnyArmor => Group::Armor,
        }
    }
    /// Resolves a saved `itemFilter` matcher reference (id or display name).
    pub fn find(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|m| m.id() == name || m.name().to_ascii_lowercase() == name)
    }
    pub fn matches(self, item: Option<&Item>) -> bool {
        let Some(item) = item else {
            return self == Self::Hand;
        };
        let id = item.fingerprint.id.as_str();
        let path = id.strip_prefix("minecraft:").unwrap_or(id);
        match self {
            Self::Hand => false,
            Self::AnyItem => true,
            Self::AnyBow => item.class == ItemClass::Bow || path == "crossbow",
            Self::AnyPotion => matches!(path, "potion" | "splash_potion" | "lingering_potion"),
            Self::Weapons => matches!(item.class, ItemClass::Sword | ItemClass::Axe),
            Self::Swords => item.class == ItemClass::Sword,
            Self::AxeWeapon | Self::AxeTool => item.class == ItemClass::Axe,
            Self::AnyTool => matches!(
                item.class,
                ItemClass::Pickaxe | ItemClass::Axe | ItemClass::Shovel | ItemClass::Hoe
            ),
            Self::Pickaxe => item.class == ItemClass::Pickaxe,
            Self::Shovel => item.class == ItemClass::Shovel,
            Self::Hoe => item.class == ItemClass::Hoe,
            Self::AnyFood => item.meta.food.is_some(),
            Self::CookedFood => {
                item.meta.food.is_some()
                    && (path.starts_with("cooked_") || path.starts_with("baked_"))
            }
            Self::AnyBlock => item.meta.block,
            Self::BuildingBlocks => item.meta.block && building_block(path),
            Self::AnyArmor => item.armor.is_some(),
        }
    }
    /// The comparator the matcher sorts candidates with when a rule has no
    /// priority override.
    pub fn default_category(self) -> Category {
        match self {
            Self::AnyBow => Category::BowDamage,
            Self::AnyPotion => Category::PotionAmplifier,
            Self::Weapons | Self::Swords | Self::AxeWeapon => Category::WeaponDamage,
            Self::Pickaxe | Self::AxeTool | Self::Shovel => Category::ToolSpeed,
            Self::AnyFood | Self::CookedFood => Category::Nutrition,
            Self::BuildingBlocks => Category::MaxStackSize,
            Self::AnyArmor => Category::Armor,
            Self::Hand | Self::AnyItem | Self::AnyTool | Self::Hoe | Self::AnyBlock => {
                Category::AnyType
            }
        }
    }
}
fn building_block(path: &str) -> bool {
    path.ends_with("_wool")
        || path == "stone"
        || path.ends_with("_planks")
        || path == "red_sandstone"
        || path.ends_with("terracotta")
        || path == "end_stone"
        || path == "obsidian"
}

/// Priority categories (`priority` key). The last three are the implicit
/// comparators of matchers without a selectable category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    AnyType,
    WeaponDamage,
    WeaponBest,
    ToolSpeed,
    BlockHardness,
    BlockSoftness,
    MaxStackSize,
    BowDamage,
    BowBest,
    PotionAmplifier,
    Nutrition,
    Armor,
}
impl Category {
    /// Selectable categories, in the reference's registry order.
    pub const SELECTABLE: [Self; 9] = [
        Self::AnyType,
        Self::WeaponDamage,
        Self::WeaponBest,
        Self::ToolSpeed,
        Self::BlockHardness,
        Self::BlockSoftness,
        Self::MaxStackSize,
        Self::BowDamage,
        Self::BowBest,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::AnyType => "any_type",
            Self::WeaponDamage => "weapon_damage",
            Self::WeaponBest => "weapon_best",
            Self::ToolSpeed => "tool_speed",
            Self::BlockHardness => "block_hardness",
            Self::BlockSoftness => "block_softness",
            Self::MaxStackSize => "max_stack_size",
            Self::BowDamage => "bow_damage",
            Self::BowBest => "bow_best",
            Self::PotionAmplifier => "potion_amplifier",
            Self::Nutrition => "nutrition",
            Self::Armor => "armor",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::AnyType => "First available",
            Self::WeaponDamage | Self::BowDamage => "Highest attack damage",
            Self::WeaponBest | Self::BowBest => "Best overall",
            Self::ToolSpeed => "Fastest mining speed",
            Self::BlockHardness => "Hardest block",
            Self::BlockSoftness => "Softest block",
            Self::MaxStackSize => "Highest stack size",
            Self::PotionAmplifier => "Strongest effect",
            Self::Nutrition => "Most nutrition",
            Self::Armor => "Best armor",
        }
    }
    pub fn find(id: &str) -> Option<Self> {
        Self::SELECTABLE.into_iter().find(|c| c.id() == id)
    }
    /// Higher is better. `None` means the category does not order items.
    pub fn score(self, item: &Item) -> Option<f64> {
        let ench = |id: &str| f64::from(item.meta.enchantment(id));
        Some(match self {
            Self::AnyType => return None,
            Self::WeaponDamage => weapon_damage(item),
            Self::WeaponBest => {
                weapon_damage(item)
                    + 0.01
                        * (ench("smite")
                            + ench("bane_of_arthropods")
                            + ench("fire_aspect")
                            + ench("looting")
                            + ench("mending")
                            + ench("unbreaking"))
                    + 0.05 * (ench("knockback") + ench("sweeping_edge"))
            }
            Self::ToolSpeed => f64::from(item.meta.mining_speed),
            Self::BlockHardness => f64::from(item.meta.hardness?),
            Self::BlockSoftness => -f64::from(item.meta.hardness?),
            Self::MaxStackSize => {
                if item.max_count < 2 {
                    return None;
                }
                f64::from(item.fingerprint.count)
            }
            Self::BowDamage => bow_damage(item),
            Self::BowBest => {
                bow_damage(item)
                    + 0.1 * (ench("punch") + ench("flame") + ench("mending") + ench("unbreaking"))
                    + ench("infinity")
            }
            Self::PotionAmplifier => item
                .meta
                .effects
                .iter()
                .map(|e| f64::from(e.amplifier))
                .fold(-1.0, f64::max),
            Self::Nutrition => f64::from(item.meta.food?.0),
            Self::Armor => {
                item.armor?;
                f64::from(item.score)
            }
        })
    }
    /// Orders two candidates best-first; equal or unscored pairs keep order.
    pub fn compare(self, a: &Item, b: &Item) -> Ordering {
        match (self.score(a), self.score(b)) {
            (Some(a), Some(b)) => b.partial_cmp(&a).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
}
/// Attack damage plus the modern sharpness bonus (0.5·level + 0.5).
pub fn weapon_damage(item: &Item) -> f64 {
    let sharpness = item.meta.enchantment("sharpness");
    let bonus = if sharpness > 0 {
        0.5 * f64::from(sharpness) + 0.5
    } else {
        0.0
    };
    f64::from(item.score) + bonus
}
/// Arrow base damage 2 plus the power bonus (0.5·level + 0.5).
pub fn bow_damage(item: &Item) -> f64 {
    let power = item.meta.enchantment("power");
    2.0 + if power > 0 {
        0.5 * f64::from(power) + 0.5
    } else {
        0.0
    }
}

/// `ComparisonOperator`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compare {
    Equals,
    NotEquals,
    Greater,
    GreaterEq,
    Less,
    LessEq,
}
impl Compare {
    pub const ALL: [Self; 6] = [
        Self::Equals,
        Self::NotEquals,
        Self::Greater,
        Self::GreaterEq,
        Self::Less,
        Self::LessEq,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::NotEquals => "does not equal",
            Self::Greater => "greater than",
            Self::GreaterEq => "greater or equal to",
            Self::Less => "less than",
            Self::LessEq => "less or equal to",
        }
    }
    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
    pub fn test(self, left: f64, right: f64) -> bool {
        match self {
            Self::Equals => left == right,
            Self::NotEquals => left != right,
            Self::Greater => left > right,
            Self::GreaterEq => left >= right,
            Self::Less => left < right,
            Self::LessEq => left <= right,
        }
    }
}
/// `TextMatchMode`. Both sides are lowercased; a condition matches when any
/// of its texts does (`matchesAny`), so negative modes with several texts
/// pass when any one text is absent, as in the reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextMode {
    Equals,
    NotEquals,
    MatchRegex,
    NotMatchRegex,
    Contains,
    NotContains,
    IsIn,
    IsNotIn,
}
impl TextMode {
    pub const ALL: [Self; 8] = [
        Self::Equals,
        Self::NotEquals,
        Self::MatchRegex,
        Self::NotMatchRegex,
        Self::Contains,
        Self::NotContains,
        Self::IsIn,
        Self::IsNotIn,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Equals => "equals",
            Self::NotEquals => "does not equal",
            Self::MatchRegex => "matches regex",
            Self::NotMatchRegex => "does not match regex",
            Self::Contains => "contains",
            Self::NotContains => "does not contain",
            Self::IsIn => "is in",
            Self::IsNotIn => "is not in",
        }
    }
    /// `supportsMultipleValues`: the editor lists several texts.
    pub fn multi_value(self) -> bool {
        matches!(self, Self::IsIn | Self::IsNotIn)
    }
    /// `TextMatchMode.fromName`: case-insensitive, unknown names are Equals.
    pub fn parse(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
            .unwrap_or(Self::Equals)
    }
    pub fn matches(self, value: &str, candidate: &str) -> bool {
        let value = value.to_lowercase();
        let candidate = candidate.to_lowercase();
        // `String.matches`; an invalid pattern never matches.
        let regex = || super::text_regex::Regex::new(&candidate).map(|r| r.matches(&value));
        match self {
            Self::Equals => value == candidate,
            Self::NotEquals => value != candidate,
            Self::MatchRegex => regex() == Some(true),
            Self::NotMatchRegex => regex() == Some(false),
            Self::Contains | Self::IsIn => value.contains(&candidate),
            Self::NotContains | Self::IsNotIn => !value.contains(&candidate),
        }
    }
    pub fn test(self, value: &str, texts: &[String]) -> bool {
        texts.iter().any(|t| self.matches(value, t))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectMode {
    Has,
    Level,
    Duration,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    /// `ITEM_DURABILITY`: percentage or value of remaining durability.
    Durability {
        percentage: bool,
        operator: Compare,
        value: i32,
    },
    AttackDamage {
        operator: Compare,
        value: f64,
    },
    /// `ENCHANTMENT` with mode Has (operator ignored) or Level.
    Enchantment {
        level_mode: bool,
        enchantment: String,
        operator: Compare,
        level: u32,
    },
    PotionEffect {
        mode: EffectMode,
        effect: String,
        operator: Compare,
        value: i32,
    },
    Quantity {
        operator: Compare,
        amount: i32,
    },
    DisplayName {
        mode: TextMode,
        texts: Vec<String>,
    },
    ItemName {
        mode: TextMode,
        texts: Vec<String>,
    },
    /// `LORE`: any lore line matches.
    Lore {
        mode: TextMode,
        texts: Vec<String>,
    },
    Material {
        include: bool,
        items: Vec<String>,
    },
}
fn number(value: &Value, key: &str) -> Result<f64, String> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing number {key:?}"))
}
fn text(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("missing string {key:?}"))
}
fn compare(value: &Value) -> Result<Compare, String> {
    let name = text(value, "operator")?;
    Compare::parse(&name).ok_or_else(|| format!("unknown operator {name:?}"))
}
fn unprefixed(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    id.strip_prefix("minecraft:").unwrap_or(&id).to_string()
}
/// Legacy numeric `MobEffect` ids saved by the reference's potion picker.
pub fn legacy_effect(id: i64) -> Option<&'static str> {
    const IDS: [&str; 33] = [
        "speed",
        "slowness",
        "haste",
        "mining_fatigue",
        "strength",
        "instant_health",
        "instant_damage",
        "jump_boost",
        "nausea",
        "regeneration",
        "resistance",
        "fire_resistance",
        "water_breathing",
        "invisibility",
        "blindness",
        "night_vision",
        "hunger",
        "weakness",
        "poison",
        "wither",
        "health_boost",
        "absorption",
        "saturation",
        "glowing",
        "levitation",
        "luck",
        "unluck",
        "slow_falling",
        "conduit_power",
        "dolphins_grace",
        "bad_omen",
        "hero_of_the_village",
        "darkness",
    ];
    usize::try_from(id - 1)
        .ok()
        .and_then(|i| IDS.get(i))
        .copied()
}
impl Condition {
    pub fn parse(value: &Value) -> Result<Self, String> {
        let kind = text(value, "type")?;
        let texts = || -> Vec<String> {
            value
                .get("text")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(base64_decode)
                .collect()
        };
        Ok(match kind.as_str() {
            "ITEM_DURABILITY" => Self::Durability {
                percentage: value.get("durabilityMode").and_then(Value::as_str) != Some("Value"),
                operator: compare(value)?,
                value: number(value, "damage")? as i32,
            },
            "ATTACK_DAMAGE" => Self::AttackDamage {
                operator: compare(value)?,
                value: number(value, "damage")?,
            },
            "ENCHANTMENT" => Self::Enchantment {
                level_mode: value.get("mode").and_then(Value::as_str) == Some("Level"),
                enchantment: unprefixed(&text(value, "enchantment")?),
                operator: compare(value).unwrap_or(Compare::GreaterEq),
                level: number(value, "level").unwrap_or(1.0).max(0.0) as u32,
            },
            "POTION_EFFECT" => {
                let effect = match value.get("potionId") {
                    Some(Value::Number(n)) => n
                        .as_i64()
                        .and_then(legacy_effect)
                        .map(str::to_owned)
                        .ok_or("unknown potion id")?,
                    Some(Value::String(s)) => unprefixed(s),
                    _ => return Err("missing potionId".into()),
                };
                Self::PotionEffect {
                    mode: match value.get("mode").and_then(Value::as_str) {
                        Some("Level") => EffectMode::Level,
                        Some("Duration") => EffectMode::Duration,
                        _ => EffectMode::Has,
                    },
                    effect,
                    operator: compare(value).unwrap_or(Compare::GreaterEq),
                    value: number(value, "level").unwrap_or(0.0) as i32,
                }
            }
            "QUANTITY" => Self::Quantity {
                operator: compare(value)?,
                amount: number(value, "amount")? as i32,
            },
            "DISPLAY_NAME" => Self::DisplayName {
                mode: TextMode::parse(&text(value, "operator")?),
                texts: texts(),
            },
            "ITEM_NAME" => Self::ItemName {
                mode: TextMode::parse(&text(value, "operator")?),
                texts: texts(),
            },
            "MATERIAL" => Self::Material {
                include: value.get("operator").and_then(Value::as_str) != Some("is not in"),
                items: value
                    .get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(unprefixed)
                    .collect(),
            },
            "LORE" => Self::Lore {
                mode: TextMode::parse(&text(value, "operator")?),
                texts: texts(),
            },
            other => return Err(format!("unknown condition type {other:?}")),
        })
    }
    pub fn to_json(&self) -> Value {
        let texts = |t: &[String]| t.iter().map(|s| base64_encode(s)).collect::<Vec<_>>();
        match self {
            Self::Durability {
                percentage,
                operator,
                value,
            } => json!({"type": "ITEM_DURABILITY", "operator": operator.name(),
                "durabilityMode": if *percentage {"Percentage"} else {"Value"}, "damage": value}),
            Self::AttackDamage { operator, value } => {
                json!({"type": "ATTACK_DAMAGE", "operator": operator.name(), "damage": value})
            }
            Self::Enchantment {
                level_mode,
                enchantment,
                operator,
                level,
            } => json!({"type": "ENCHANTMENT", "operator": operator.name(),
                "mode": if *level_mode {"Level"} else {"Has"}, "enchantment": enchantment, "level": level}),
            Self::PotionEffect {
                mode,
                effect,
                operator,
                value,
            } => json!({"type": "POTION_EFFECT", "operator": operator.name(),
                "mode": match mode {EffectMode::Has => "Has", EffectMode::Level => "Level", EffectMode::Duration => "Duration"},
                "potionId": effect, "level": value}),
            Self::Quantity { operator, amount } => {
                json!({"type": "QUANTITY", "operator": operator.name(), "amount": amount})
            }
            Self::DisplayName { mode, texts: t } => {
                json!({"type": "DISPLAY_NAME", "operator": mode.name(), "text": texts(t)})
            }
            Self::ItemName { mode, texts: t } => {
                json!({"type": "ITEM_NAME", "operator": mode.name(), "text": texts(t)})
            }
            Self::Lore { mode, texts: t } => {
                json!({"type": "LORE", "operator": mode.name(), "text": texts(t)})
            }
            Self::Material { include, items } => json!({"type": "MATERIAL",
                "operator": if *include {"is in"} else {"is not in"}, "items": items}),
        }
    }
    pub fn matches(&self, item: &Item) -> bool {
        let id = unprefixed(&item.fingerprint.id);
        match self {
            Self::Durability {
                percentage,
                operator,
                value,
            } => {
                if item.max_damage == 0 {
                    return false;
                }
                let left = i32::from(item.max_damage) - i32::from(item.durability_used());
                let left = if *percentage {
                    f64::from(left) * 100.0 / f64::from(item.max_damage)
                } else {
                    f64::from(left)
                };
                operator.test(left, f64::from(*value))
            }
            Self::AttackDamage { operator, value } => {
                item.armor.is_none() && operator.test(weapon_damage(item), *value)
            }
            Self::Enchantment {
                level_mode,
                enchantment,
                operator,
                level,
            } => {
                let have = item.meta.enchantment(enchantment);
                if *level_mode {
                    have > 0 && operator.test(f64::from(have), f64::from(*level))
                } else {
                    have > 0
                }
            }
            Self::PotionEffect {
                mode,
                effect,
                operator,
                value,
            } => item
                .meta
                .effects
                .iter()
                .filter(|e| &e.id == effect)
                .any(|e| match mode {
                    EffectMode::Has => true,
                    // Levels are shown one-based (Strength II = 2).
                    EffectMode::Level => {
                        operator.test(f64::from(e.amplifier + 1), f64::from(*value))
                    }
                    // Durations are compared in seconds.
                    EffectMode::Duration => {
                        operator.test(f64::from(e.duration) / 20.0, f64::from(*value))
                    }
                }),
            Self::Quantity { operator, amount } => {
                operator.test(f64::from(item.fingerprint.count), f64::from(*amount))
            }
            Self::DisplayName { mode, texts } => mode.test(&item.meta.display_name, texts),
            Self::ItemName { mode, texts } => mode.test(&item.meta.item_name, texts),
            Self::Lore { mode, texts } => item.meta.lore.iter().any(|l| mode.test(l, texts)),
            Self::Material { include, items } => items.contains(&id) == *include,
        }
    }
}
/// Conditions combined with And; a preset matches when any group matches.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConditionGroup {
    pub conditions: Vec<Condition>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preset {
    pub uuid: String,
    pub name: String,
    pub groups: Vec<ConditionGroup>,
}
impl Preset {
    pub fn matches(&self, item: &Item) -> bool {
        self.groups.is_empty()
            || self
                .groups
                .iter()
                .any(|g| g.conditions.iter().all(|c| c.matches(item)))
    }
    pub fn parse(value: &Value) -> Result<Self, String> {
        let groups = value
            .get("conditions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|group| {
                Ok(ConditionGroup {
                    conditions: group
                        .get("conditions")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(Condition::parse)
                        .collect::<Result<_, String>>()?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            uuid: value
                .get("uuid")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            groups,
        })
    }
    pub fn to_json(&self) -> Value {
        json!({
            "uuid": self.uuid,
            "name": self.name,
            "conditions": self.groups.iter().map(|g| json!({
                "conditions": g.conditions.iter().map(Condition::to_json).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
        })
    }
}

/// `ItemFilterSelection`: a matcher or one concrete item id.
#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    None,
    Matcher(Matcher),
    Item(String),
}
impl Filter {
    pub fn parse(value: Option<&Value>) -> Self {
        let name = match value {
            Some(Value::String(s)) => Some(s.as_str()),
            Some(Value::Object(o)) => o
                .get("items")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str),
            _ => None,
        };
        match name.map(str::trim).filter(|n| !n.is_empty()) {
            None => Self::None,
            Some(n) => Matcher::find(n).map_or_else(|| Self::Item(unprefixed(n)), Self::Matcher),
        }
    }
    pub fn to_json(&self) -> Value {
        match self {
            Self::None => Value::Null,
            Self::Matcher(m) => json!(m.id()),
            Self::Item(id) => json!(format!("minecraft:{id}")),
        }
    }
    pub fn matches(&self, item: Option<&Item>) -> bool {
        match self {
            Self::None => false,
            Self::Matcher(m) => m.matches(item),
            Self::Item(id) => item.is_some_and(|i| unprefixed(&i.fingerprint.id) == *id),
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::None => "None".into(),
            Self::Matcher(m) => m.name().into(),
            Self::Item(id) => id.clone(),
        }
    }
}
/// `InventoryFilterAction`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Remove,
    Move,
    Condense,
}
impl Action {
    pub const ALL: [Self; 3] = [Self::Remove, Self::Move, Self::Condense];
    pub fn name(self) -> &'static str {
        match self {
            Self::Remove => "Drop item",
            Self::Move => "Move into inventory",
            Self::Condense => "Condense stacks",
        }
    }
}
/// A slot rule (hotbar slot 0..9) or an inventory item rule (with action).
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub filter: Filter,
    /// Inline custom rule; shared preset references resolve into this.
    pub preset: Option<Preset>,
    /// `customRule` was a shared preset uuid.
    pub shared: Option<String>,
    pub priority: Option<Category>,
    pub slot: Option<u8>,
    pub action: Action,
}
impl Rule {
    pub fn category(&self) -> Category {
        self.priority.unwrap_or(match &self.filter {
            Filter::Matcher(m) => m.default_category(),
            _ => Category::AnyType,
        })
    }
    /// The item fits the filter and the custom rule (if any).
    pub fn accepts(&self, item: Option<&Item>) -> bool {
        self.filter.matches(item)
            && item.is_none_or(|i| self.preset.as_ref().is_none_or(|p| p.matches(i)))
    }
    fn parse(value: &Value, presets: &[Preset]) -> Result<Self, String> {
        let (preset, shared) = match value.get("customRule") {
            Some(Value::String(uuid)) => (
                presets.iter().find(|p| &p.uuid == uuid).cloned(),
                Some(uuid.clone()),
            ),
            Some(v @ Value::Object(_)) => (Some(Preset::parse(v)?), None),
            _ => (None, None),
        };
        Ok(Self {
            filter: Filter::parse(value.get("itemFilter")),
            preset,
            shared,
            priority: value
                .get("priority")
                .and_then(Value::as_str)
                .and_then(Category::find),
            slot: value
                .get("slot")
                .and_then(Value::as_u64)
                .filter(|s| *s < 9)
                .map(|s| s as u8),
            action: match value.get("filterAction").and_then(Value::as_str) {
                Some("Move into inventory") => Action::Move,
                Some("Condense stacks") => Action::Condense,
                _ => Action::Remove,
            },
        })
    }
    fn to_json(&self, item_rule: bool) -> Value {
        let mut out = json!({ "itemFilter": self.filter.to_json() });
        match (&self.shared, &self.preset) {
            (Some(uuid), _) => out["customRule"] = json!(uuid),
            (None, Some(p)) => out["customRule"] = p.to_json(),
            _ => {}
        }
        if let Some(priority) = self.priority {
            out["priority"] = json!(priority.id());
        }
        if let Some(slot) = self.slot {
            out["slot"] = json!(slot);
        }
        if item_rule {
            out["filterAction"] = json!(self.action.name());
        }
        out
    }
}
/// `InventoryCleanerProfile`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub name: String,
    pub slots: Vec<Rule>,
    pub items: Vec<Rule>,
    pub best_armor: bool,
}
impl Profile {
    pub fn parse(value: &Value, presets: &[Preset]) -> Result<Self, String> {
        let rules = |key: &str| -> Result<Vec<Rule>, String> {
            value
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|r| Rule::parse(r, presets))
                .collect()
        };
        let mut slots = rules("slots")?;
        slots.retain(|r| r.slot.is_some());
        slots.sort_by_key(|r| r.slot);
        slots.dedup_by_key(|r| r.slot);
        Ok(Self {
            name: value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Inventory #1")
                .into(),
            slots,
            items: rules("inventoryFilters")?,
            best_armor: value.get("armor_mode").and_then(Value::as_str) == Some("Best armor"),
        })
    }
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "slots": self.slots.iter().map(|r| r.to_json(false)).collect::<Vec<_>>(),
            "inventoryFilters": self.items.iter().map(|r| r.to_json(true)).collect::<Vec<_>>(),
            "armor_mode": if self.best_armor {"Best armor"} else {"No armor management"},
        })
    }
    pub fn slot_rule(&self, slot: u8) -> Option<&Rule> {
        self.slots.iter().find(|r| r.slot == Some(slot))
    }
    pub fn slot_rule_mut(&mut self, slot: u8) -> &mut Rule {
        if let Some(i) = self.slots.iter().position(|r| r.slot == Some(slot)) {
            return &mut self.slots[i];
        }
        self.slots.push(Rule {
            filter: Filter::None,
            preset: None,
            shared: None,
            priority: None,
            slot: Some(slot),
            action: Action::Remove,
        });
        self.slots.sort_by_key(|r| r.slot);
        let i = self
            .slots
            .iter()
            .position(|r| r.slot == Some(slot))
            .unwrap();
        &mut self.slots[i]
    }
    /// `findMatchingItemRule`: the first item rule whose filter matches.
    pub fn item_rule(&self, item: &Item) -> Option<&Rule> {
        self.items.iter().find(|r| r.filter.matches(Some(item)))
    }
}
/// Parses the `profiles` text setting: an array of profiles or one profile
/// (none when empty).
pub fn parse_profiles(text: &str, presets: &[Preset]) -> Result<Vec<Profile>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    match &value {
        Value::Array(list) => list.iter().map(|p| Profile::parse(p, presets)).collect(),
        Value::Object(_) => Ok(vec![Profile::parse(&value, presets)?]),
        _ => Err("profiles must be a JSON object or array".into()),
    }
}
pub fn parse_presets(text: &str) -> Result<Vec<Preset>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    value
        .as_array()
        .ok_or("presets must be a JSON array")?
        .iter()
        .map(Preset::parse)
        .collect()
}
pub fn armor_slot_index(slot: ArmorSlot) -> u16 {
    500 + match slot {
        ArmorSlot::Head => 0,
        ArmorSlot::Chest => 1,
        ArmorSlot::Legs => 2,
        ArmorSlot::Feet => 3,
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub fn base64_encode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
pub fn base64_decode(text: &str) -> Option<String> {
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for c in text.bytes().filter(|c| *c != b'=') {
        let v = B64.iter().position(|b| *b == c)? as u32;
        bits = bits << 6 | v;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count & 0xff) as u8);
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::inventory::types::{Fingerprint, ItemMeta, PotionEffect};

    pub(crate) fn item(id: &str, class: ItemClass, score: f32) -> Item {
        Item {
            fingerprint: Fingerprint {
                id: format!("minecraft:{id}"),
                count: 1,
                damage: 0,
                tag_hash: 0,
            },
            max_count: 1,
            max_damage: 0,
            armor: None,
            class,
            score,
            healing: None,
            harmful_splash: false,
            meta: ItemMeta {
                mining_speed: 1.0,
                ..Default::default()
            },
        }
    }

    #[test]
    fn base64_round_trips() {
        for s in ["", "a", "ab", "abc", "Strength II", "héllo"] {
            assert_eq!(base64_decode(&base64_encode(s)).as_deref(), Some(s));
        }
        assert_eq!(base64_encode("abc"), "YWJj");
    }
    #[test]
    fn matchers_and_scores_follow_reference() {
        let mut sword = item("diamond_sword", ItemClass::Sword, 7.0);
        sword.meta.enchantments = vec![("minecraft:sharpness".into(), 2)];
        assert!(Matcher::Swords.matches(Some(&sword)));
        assert!(Matcher::Weapons.matches(Some(&sword)));
        assert!(!Matcher::AnyTool.matches(Some(&sword)));
        assert!(Matcher::Hand.matches(None));
        assert_eq!(Category::WeaponDamage.score(&sword), Some(8.5));
        let mut bow = item("bow", ItemClass::Bow, 0.0);
        bow.meta.enchantments = vec![
            ("minecraft:power".into(), 1),
            ("minecraft:infinity".into(), 1),
        ];
        assert_eq!(Category::BowDamage.score(&bow), Some(3.0));
        assert_eq!(Category::BowBest.score(&bow), Some(4.0));
        let mut wool = item("white_wool", ItemClass::Other, 0.0);
        wool.meta.block = true;
        assert!(Matcher::BuildingBlocks.matches(Some(&wool)));
        assert_eq!(Matcher::find("Any Weapon"), Some(Matcher::Weapons));
    }
    #[test]
    fn profile_json_round_trips_and_rejects_unsupported() {
        let text = r#"[{"name":"PvP","armor_mode":"Best armor",
            "slots":[{"slot":0,"itemFilter":"swords","priority":"weapon_best"},
                     {"slot":8,"itemFilter":"minecraft:golden_apple",
                      "customRule":{"uuid":"u","name":"r","conditions":[{"conditions":[
                        {"type":"QUANTITY","operator":"greater than","amount":2}]}]}}],
            "inventoryFilters":[{"itemFilter":"any-food","filterAction":"Condense stacks"}]}]"#;
        let profiles = parse_profiles(text, &[]).unwrap();
        let p = &profiles[0];
        assert!(p.best_armor);
        assert_eq!(p.slot_rule(0).unwrap().category(), Category::WeaponBest);
        assert_eq!(p.items[0].action, Action::Condense);
        let mut apple = item("golden_apple", ItemClass::Other, 0.0);
        assert!(!p.slot_rule(8).unwrap().accepts(Some(&apple)));
        apple.fingerprint.count = 3;
        assert!(p.slot_rule(8).unwrap().accepts(Some(&apple)));
        let again = Profile::parse(&p.to_json(), &[]).unwrap();
        assert_eq!(&again, p);
        let lore = r#"{"type":"LORE","operator":"Contains","text":["c291bA=="]}"#;
        let lore = Condition::parse(&serde_json::from_str(lore).unwrap()).unwrap();
        assert_eq!(
            lore,
            Condition::Lore {
                mode: TextMode::Contains,
                texts: vec!["soul".into()]
            }
        );
        assert_eq!(Condition::parse(&lore.to_json()).unwrap(), lore);
        let regex = r#"{"type":"ITEM_NAME","operator":"matches regex","text":[]}"#;
        assert!(matches!(
            Condition::parse(&serde_json::from_str(regex).unwrap()),
            Ok(Condition::ItemName {
                mode: TextMode::MatchRegex,
                ..
            })
        ));
    }
    #[test]
    fn potion_and_text_conditions() {
        let mut pot = item("splash_potion", ItemClass::Other, 0.0);
        pot.meta.effects = vec![PotionEffect {
            id: "instant_health".into(),
            amplifier: 1,
            duration: 1,
        }];
        pot.meta.custom_name = Some("Heal Pot".into());
        pot.meta.display_name = "Heal Pot".into();
        pot.meta.item_name = "Splash Potion of Healing".into();
        pot.meta.lore = vec!["Soulbound".into(), "Tier II".into()];
        let c = Condition::PotionEffect {
            mode: EffectMode::Level,
            effect: legacy_effect(6).unwrap().into(),
            operator: Compare::GreaterEq,
            value: 2,
        };
        assert!(c.matches(&pot));
        let d = Condition::DisplayName {
            mode: TextMode::Contains,
            texts: vec!["heal".into()],
        };
        assert!(d.matches(&pot));
        assert!(Condition::parse(&d.to_json()).unwrap() == d);
        let text = |mode, texts: &[&str]| (mode, texts.iter().map(|t| (*t).to_owned()).collect());
        type Case = ((TextMode, Vec<String>), bool, bool, bool);
        let cases: [Case; 8] = [
            // (condition, display name, item name, lore)
            (text(TextMode::Equals, &["heal pot"]), true, false, false),
            (text(TextMode::NotEquals, &["heal pot"]), false, true, true),
            (
                text(TextMode::MatchRegex, &[".*potion of \\w+"]),
                false,
                true,
                false,
            ),
            (
                text(TextMode::NotMatchRegex, &["tier i+"]),
                true,
                true,
                true,
            ),
            (text(TextMode::Contains, &["healing"]), false, true, false),
            (text(TextMode::NotContains, &["pot"]), false, false, true),
            (text(TextMode::IsIn, &["sword", "soul"]), false, false, true),
            // Any absent text passes, like the reference's matchesAny.
            (text(TextMode::IsNotIn, &["heal", "x"]), true, true, true),
        ];
        for ((mode, texts), display, name, lore) in cases {
            let d = Condition::DisplayName {
                mode,
                texts: texts.clone(),
            };
            let n = Condition::ItemName {
                mode,
                texts: texts.clone(),
            };
            let l = Condition::Lore { mode, texts };
            assert_eq!(
                (d.matches(&pot), n.matches(&pot), l.matches(&pot)),
                (display, name, lore),
                "{mode:?}"
            );
        }
        let empty = Condition::Lore {
            mode: TextMode::NotContains,
            texts: Vec::new(),
        };
        assert!(!empty.matches(&pot));
    }
}
