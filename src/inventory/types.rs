// Bounded host-owned inventory snapshots and optimistic container requests.
use std::{collections::BTreeMap, time::Instant};

pub const MAX_SLOTS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Feet,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Healing {
    Soup,
    Potion,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ItemClass {
    Sword,
    Bow,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Other,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fingerprint {
    pub id: String,
    pub count: u8,
    pub damage: u16,
    pub tag_hash: u64,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub fingerprint: Fingerprint,
    pub max_count: u8,
    pub max_damage: u16,
    pub armor: Option<ArmorSlot>,
    pub class: ItemClass,
    /// Host-calculated effective strength (base attributes plus applicable enchantments).
    pub score: f32,
    pub healing: Option<Healing>,
    pub harmful_splash: bool,
    /// Host-decoded display and component facts used by rule matching.
    pub meta: ItemMeta,
}
/// One decoded potion effect: unnamespaced effect id (`instant_health`),
/// zero-based amplifier and duration in ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PotionEffect {
    pub id: String,
    pub amplifier: i32,
    pub duration: i32,
}
/// Bounded item facts copied by the host from Horizon's decoded components.
/// Nothing here is guessed from an item-id suffix except where noted by the
/// host bridge.
#[derive(Clone, Debug, Default)]
pub struct ItemMeta {
    /// `custom_name` as plain text, when the stack was renamed.
    pub custom_name: Option<String>,
    /// The hover name (`getDisplayName`): the custom name, else the item's
    /// name, as the host's tooltip name line gives it.
    pub display_name: String,
    /// The item's own name (`getItemStackDisplayName`), ignoring a custom
    /// name: item name component, potion name, or translation.
    pub item_name: String,
    /// `minecraft:lore` lines as plain text.
    pub lore: Vec<String>,
    /// Potion contents (base potion plus custom effects) of potion items.
    pub effects: Vec<PotionEffect>,
    /// `minecraft:splash_potion` or `minecraft:lingering_potion`.
    pub thrown_potion: bool,
    /// `food` component: nutrition and saturation (points, the component's
    /// `saturation`).
    pub food: Option<(u32, f32)>,
    /// The item places a block.
    pub block: bool,
    /// Block destroy-time hardness of the placed block, when known.
    pub hardness: Option<f32>,
    /// Enchantments (namespaced id, level).
    pub enchantments: Vec<(String, u32)>,
    /// Fastest `tool` rule speed (1.0 for items without a tool component).
    pub mining_speed: f32,
    /// The localized hover name (`getHoverName`): the custom name, the item
    /// name component, or the translated name (`Splash Potion of Poison`).
    pub hover_name: String,
    /// `minecraft:armor` and `minecraft:armor_toughness` modifier totals that
    /// apply when worn.
    pub armor_points: f32,
    pub armor_toughness: f32,
    /// Summed amounts of the item's default modifiers in the `mainhand`
    /// slot group: attack damage plus attack speed for vanilla weapons and
    /// tools.
    pub mainhand_modifiers: f32,
    /// Amount of the first of the item's default modifiers (attack damage
    /// for vanilla tools).
    pub first_modifier: f32,
    /// The item has a `tool` component.
    pub tool: bool,
}
impl ItemMeta {
    pub fn enchantment(&self, id: &str) -> u32 {
        self.enchantments
            .iter()
            .find(|(e, _)| e == id || e.strip_prefix("minecraft:") == Some(id))
            .map_or(0, |(_, level)| *level)
    }
    pub fn has_effect(&self, id: &str) -> bool {
        self.effects.iter().any(|e| e.id == id)
    }
}
impl Item {
    pub fn durability_used(&self) -> u16 {
        self.fingerprint.damage
    }
    pub fn same_stack_kind(&self, other: &Self) -> bool {
        self.fingerprint.id == other.fingerprint.id
            && self.fingerprint.tag_hash == other.fingerprint.tag_hash
    }
}
#[derive(Clone, Debug)]
pub struct Slot {
    pub index: u16,
    pub item: Option<Item>,
    pub role: SlotRole,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotRole {
    Container,
    Main,
    Hotbar(u8),
    Armor(ArmorSlot),
    Other,
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub session: u64,
    pub container_id: u32,
    pub revision: u32,
    pub slots: Vec<Slot>,
    pub carried: Option<Item>,
    pub screen_open: bool,
    pub is_player_inventory: bool,
    pub is_chest: bool,
    pub chest_title_standard: bool,
    /// Player facts the healing and throwing planners read.
    pub vitals: Vitals,
    /// Monotonic time the host copied this state; bounds transaction waits.
    pub observed: Instant,
}
impl Snapshot {
    /// Content identity of every slot and the cursor, independent of menu id
    /// and protocol revision. Two snapshots with equal digests show the same
    /// items in the same slots.
    pub fn digest(&self) -> u64 {
        fn mix(h: u64, bytes: &[u8]) -> u64 {
            bytes
                .iter()
                .fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
        }
        fn item(h: u64, item: Option<&Item>) -> u64 {
            let Some(item) = item else {
                return mix(h, &[0]);
            };
            let f = &item.fingerprint;
            let h = mix(mix(h, &[1]), f.id.as_bytes());
            let h = mix(mix(h, &[f.count]), &f.damage.to_le_bytes());
            mix(h, &f.tag_hash.to_le_bytes())
        }
        let h = self.slots.iter().fold(0xcbf29ce484222325, |h, slot| {
            item(mix(h, &slot.index.to_le_bytes()), slot.item.as_ref())
        });
        item(h, self.carried.as_ref())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.slots.len() > MAX_SLOTS {
            return Err("inventory snapshot has too many slots".into());
        }
        let mut seen = BTreeMap::new();
        for slot in &self.slots {
            if seen.insert(slot.index, ()).is_some() {
                return Err("inventory snapshot repeats a slot".into());
            }
            if let Some(item) = &slot.item
                && (item.fingerprint.id.is_empty()
                    || item.fingerprint.id.len() > 256
                    || item.fingerprint.count == 0
                    || item.max_count == 0
                    || item.fingerprint.count > item.max_count
                    || !item.score.is_finite())
            {
                return Err("invalid inventory item metadata".into());
            }
        }
        Ok(())
    }
    pub fn slot(&self, index: u16) -> Option<&Slot> {
        self.slots.iter().find(|s| s.index == index)
    }
    pub fn slots(&self, role: impl Fn(SlotRole) -> bool) -> impl Iterator<Item = &Slot> {
        self.slots.iter().filter(move |s| role(s.role))
    }
    pub fn hotbar(&self, index: u8) -> Option<&Slot> {
        self.slots
            .iter()
            .find(|s| s.role == SlotRole::Hotbar(index))
    }
}
/// Player state copied with the inventory snapshot.
#[derive(Clone, Debug, Default)]
pub struct Vitals {
    pub health: f32,
    pub max_health: f32,
    /// Selected hotbar slot (0..9).
    pub selected: u8,
    pub on_ground: bool,
    /// Active status effects, unnamespaced (`speed`, `regeneration`).
    pub effects: Vec<String>,
    /// The player is using an item (eating, drawing a bow).
    pub using_item: bool,
    /// The camera's yaw and pitch, degrees.
    pub view: (f32, f32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Click {
    Pickup,
    SwapOffhand,
    QuickMove,
    /// Drop one item from the slot (`THROW`, button 0).
    Throw,
    /// Drop the whole stack (`THROW`, button 1; Ctrl+Q in a menu).
    ThrowStack,
    /// Number-key swap with hotbar slot n (`SWAP`, button n).
    SwapHotbar(u8),
    /// Left click outside the menu (slot -999): drops the carried stack.
    Outside,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub session: u64,
    pub container_id: u32,
    pub revision: u32,
    pub slot: u16,
    pub click: Click,
    pub expected: Option<Fingerprint>,
    pub expected_carried: Option<Fingerprint>,
}
impl Request {
    pub fn for_slot(snapshot: &Snapshot, slot: &Slot, click: Click) -> Self {
        Self {
            session: snapshot.session,
            container_id: snapshot.container_id,
            revision: snapshot.revision,
            slot: slot.index,
            click,
            expected: slot.item.as_ref().map(|i| i.fingerprint.clone()),
            expected_carried: snapshot.carried.as_ref().map(|i| i.fingerprint.clone()),
        }
    }
    /// A [`Click::Outside`] request; only the carried stack is expected.
    pub fn outside(snapshot: &Snapshot) -> Self {
        Self {
            session: snapshot.session,
            container_id: snapshot.container_id,
            revision: snapshot.revision,
            slot: OUTSIDE_SLOT,
            click: Click::Outside,
            expected: None,
            expected_carried: snapshot.carried.as_ref().map(|i| i.fingerprint.clone()),
        }
    }
}
/// The slot id of [`Click::Outside`] requests; no snapshot slot uses it.
pub const OUTSIDE_SLOT: u16 = u16::MAX;
