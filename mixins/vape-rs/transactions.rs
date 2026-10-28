// Native bridge for the pure inventory planners. This module is installed
// inside game::remote, so it can use Remote's authoritative menu/state id.
use super::super::{Game, Screen};
use crate::game::vape_rs::{
    core::with_client,
    inventory::types::{
        ArmorSlot, Click, Fingerprint, Healing, Item, ItemClass, Request, Slot, SlotRole, Snapshot,
    },
};
use horizon_client::{items::ItemStack, menu::Input};

fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
fn armor(slot: Option<&str>) -> Option<ArmorSlot> {
    match slot {
        Some("head") => Some(ArmorSlot::Head),
        Some("chest") => Some(ArmorSlot::Chest),
        Some("legs") => Some(ArmorSlot::Legs),
        Some("feet") => Some(ArmorSlot::Feet),
        _ => None,
    }
}
fn class(id: &str) -> ItemClass {
    if id.ends_with("_sword") {
        ItemClass::Sword
    } else if id.ends_with("_bow") || id.ends_with(":bow") {
        ItemClass::Bow
    } else if id.ends_with("_pickaxe") {
        ItemClass::Pickaxe
    } else if id.ends_with("_axe") {
        ItemClass::Axe
    } else if id.ends_with("_shovel") {
        ItemClass::Shovel
    } else if id.ends_with("_hoe") {
        ItemClass::Hoe
    } else {
        ItemClass::Other
    }
}
fn score(stack: &ItemStack, info: Option<&horizon_client::items::ItemInfo>) -> f32 {
    let Some(info) = info else {
        return 0.0;
    };
    let armor = info
        .modifiers
        .iter()
        .filter_map(|(attribute, _, amount, _, group)| {
            (group == "armor" || group == "any").then_some((attribute.as_str(), *amount))
        })
        .fold(0.0, |total, (attribute, amount)| {
            total
                + match attribute {
                    "minecraft:armor" | "armor" => amount as f32,
                    "minecraft:armor_toughness" | "armor_toughness" => amount as f32 * 0.25,
                    _ => 0.0,
                }
        });
    if info.equip_slot.is_some() {
        armor + stack.enchantment_level("minecraft:protection") as f32 * 0.25
    } else {
        info.attack_damage
    }
}
fn stack(game: &Game<'_>, stack: &ItemStack) -> Item {
    let info = game.res.items().get(&stack.item);
    let id = stack.item.to_string();
    let tag_hash = hash(format!("{:?}", stack.extra).as_bytes());
    Item {
        fingerprint: Fingerprint {
            id: id.clone(),
            count: stack.count.min(u32::from(u8::MAX)) as u8,
            damage: stack.damage.min(u32::from(u16::MAX)) as u16,
            tag_hash,
        },
        max_count: info.map_or(64, |i| i.max_stack.min(u32::from(u8::MAX)) as u8),
        max_damage: info.map_or(0, |i| i.max_damage.min(u32::from(u16::MAX)) as u16),
        armor: info.and_then(|i| armor(i.equip_slot.as_deref())),
        class: class(&id),
        score: score(stack, info),
        healing: if id.contains("mushroom_stew") || id.contains("rabbit_stew") {
            Some(Healing::Soup)
        } else if id.contains("potion") {
            Some(Healing::Potion)
        } else {
            None
        },
        // Host packet/item components do not expose a lossy heuristic here;
        // adapters must populate a precise potion classification before a
        // harmful-potion policy is enabled.
        harmful_splash: false,
    }
}
fn slot(game: &Game<'_>, index: usize, role: SlotRole, value: Option<&ItemStack>) -> Slot {
    Slot {
        index: index as u16,
        item: value.map(|s| stack(game, s)),
        role,
    }
}
fn revision(game: &Game<'_>, slots: &[Slot]) -> u32 {
    if let Some(remote) = &game.remote {
        return remote.state_id.max(0) as u32;
    }
    // Single player has no protocol state-id. A content fingerprint changes
    // only when the ordinary local menu engine changed its authoritative copy.
    slots.iter().fold(0u32, |h, s| {
        h.rotate_left(5)
            ^ u32::from(s.index)
            ^ s.item.as_ref().map_or(0, |i| {
                (i.fingerprint.tag_hash as u32)
                    ^ u32::from(i.fingerprint.count)
                    ^ u32::from(i.fingerprint.damage)
            })
    })
}

/// Copies the host's actual player/menu slots; it never predicts acceptance.
pub fn snapshot(game: &Game<'_>) -> Option<Snapshot> {
    if game.player.is_none() {
        return None;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let session = with_client(&client, |runtime| runtime.session().epoch().value());
    let mut slots = Vec::with_capacity(64);
    for (index, value) in game.inventory.iter().enumerate() {
        let role = if index < 9 {
            SlotRole::Hotbar(index as u8)
        } else {
            SlotRole::Main
        };
        slots.push(slot(game, index, role, value.as_ref()));
    }
    for (index, value) in game.armor.iter().enumerate() {
        let role = [
            ArmorSlot::Head,
            ArmorSlot::Chest,
            ArmorSlot::Legs,
            ArmorSlot::Feet,
        ][index];
        slots.push(slot(
            game,
            500 + index,
            SlotRole::Armor(role),
            value.as_ref(),
        ));
    }
    let screen_open = game.screen == Screen::Inventory;
    let is_chest = screen_open
        && game
            .container_kind()
            .is_some_and(|e| matches!(e, horizon_client::containers::BlockEntity::Chest(_)));
    if is_chest {
        for (pos, _) in game.container_parts() {
            if let Some(entity) = game.world.block_entities.get(&pos) {
                let base = slots
                    .iter()
                    .filter(|s| s.role == SlotRole::Container)
                    .count();
                slots.extend(entity.slots().iter().enumerate().map(|(i, value)| {
                    slot(game, 300 + base + i, SlotRole::Container, value.as_ref())
                }));
            }
        }
    }
    let (container_id, chest_title_standard) =
        match game.remote.as_ref().and_then(|r| r.menu.as_ref()) {
            Some((id, name)) => (
                *id as u32,
                name.starts_with("minecraft:generic_9x") || name == "minecraft:shulker_box",
            ),
            None => (0, true),
        };
    let out = Snapshot {
        session,
        container_id,
        revision: revision(game, &slots),
        slots,
        carried: game.carried.as_ref().map(|s| stack(game, s)),
        screen_open,
        is_player_inventory: screen_open && !is_chest,
        is_chest,
        chest_title_standard,
    };
    out.validate().ok()?;
    Some(out)
}

/// Routes a validated request through the host's normal menu engine and its
/// server `ContainerClick` bookkeeping. `true` means queued/sent, never that
/// a server accepted it; callers must wait for a changed snapshot/revision.
pub fn submit(game: &mut Game<'_>, request: &Request) -> bool {
    let Some(current) = snapshot(game) else {
        return false;
    };
    if !current.screen_open
        || current.session != request.session
        || current.container_id != request.container_id
        || current.revision != request.revision
    {
        return false;
    }
    let Some(slot) = current.slot(request.slot) else {
        return false;
    };
    if slot.item.as_ref().map(|i| &i.fingerprint) != request.expected.as_ref() {
        return false;
    }
    let input = match request.click {
        Click::Pickup => Input::Pickup,
        Click::QuickMove => Input::QuickMove,
        Click::Throw => Input::Throw,
    };
    // Game slot ids are intentionally preserved: menu_ops translates them to
    // each concrete local/remote menu's authoritative menu index, predicts
    // only through Horizon's verified engine, and serializes state/changed
    // slots/carried stack for remote ContainerClick packets.
    game.menu_ops(&[(Some(usize::from(request.slot)), 0, input)])
}

// Dispatch on frames so an in-frame inventory menu can advance while the
// world simulation is paused. Scheduling uses monotonic time in the runtime.
pub fn dispatch(game: &mut Game<'_>) {
    use crate::game::vape_rs::core::inventory_runtime::InventoryEffect;
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let allowed = matches!(game.screen, Screen::Playing | Screen::Inventory) && game.focused && game.title.is_none()
        && game.player.as_ref().is_some_and(|p| p.health.health() > 0.0)
        && !game.dialog_open() && game.server_packs.prompt.is_none()
        && !game.creative && game.sleeping.is_none()
        && with_client(&client, |r| !r.gui.open);
    let snapshot = allowed.then(|| snapshot(game)).flatten();
    let Some(snapshot) = snapshot else {
        with_client(&client, |r| r.cancel_inventory());
        return;
    };
    let friends = with_client(&client, |r| r.friend_keys());
    let combat = crate::game::vape_inventory::combat(game, &friends, None);
    let effects = with_client(&client, |r| { r.update_combat(combat); r.inventory_step(&snapshot) });
    for effect in effects {
        match effect {
            InventoryEffect::Request(request) => if !submit(game, &request) { with_client(&client, |r| r.inventory_rejected()); },
            InventoryEffect::OpenInventory if game.screen == Screen::Playing => {
                game.screen = Screen::Inventory;
                game.capture(false);
            }
            InventoryEffect::CloseInventory if game.screen == Screen::Inventory => {
                game.close_container();
                game.screen = Screen::Playing;
                game.capture(true);
            }
            _ => {}
        }
    }
}
