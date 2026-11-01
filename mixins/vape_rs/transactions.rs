// Native bridge for the pure inventory planners. This module is installed
// inside game::remote, so it can use Remote's authoritative menu/state id.
use super::super::{Game, Screen};
use crate::game::vape_rs::{
    core::with_client,
    inventory::types::{
        ArmorSlot, Click, Fingerprint, Healing, Item, ItemClass, ItemMeta, PotionEffect, Request,
        Slot, SlotRole, Snapshot, Vitals,
    },
};
use horizon_client::{brewing, items::ItemStack, menu::Input};

fn hash(bytes: &[u8]) -> u64 {
    hash_from(0xcbf29ce484222325, bytes)
}
fn hash_from(h: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(h, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
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
/// Worn `minecraft:armor` and `minecraft:armor_toughness` modifier totals.
fn armor_values(info: &horizon_client::items::ItemInfo) -> (f32, f32) {
    info.modifiers
        .iter()
        // Vanilla armor modifiers name the piece's own slot group (`head`,
        // `chest`...), not `armor`; count every group that applies when worn.
        .filter(|(_, _, _, _, group)| {
            group == "armor" || group == "any" || info.equip_slot.as_deref() == Some(group)
        })
        .fold((0.0, 0.0), |(armor, toughness), (attribute, _, amount, _, _)| {
            match attribute.as_str() {
                "minecraft:armor" | "armor" => (armor + *amount as f32, toughness),
                "minecraft:armor_toughness" | "armor_toughness" => {
                    (armor, toughness + *amount as f32)
                }
                _ => (armor, toughness),
            }
        })
}
fn score(stack: &ItemStack, info: Option<&horizon_client::items::ItemInfo>) -> f32 {
    let Some(info) = info else {
        return 0.0;
    };
    let (points, toughness) = armor_values(info);
    let armor = points + toughness * 0.25;
    if info.equip_slot.is_some() {
        armor + stack.enchantment_level("minecraft:protection") as f32 * 0.25
    } else {
        info.attack_damage
    }
}

fn potion_metadata(id: &str, stack: &ItemStack) -> (Option<Healing>, bool) {
    // `ItemStackScoreUtil.v`: the item's default food is the mushroom stew's
    // (6 nutrition, 0.6 saturation), which beetroot soup shares and rabbit
    // stew does not.
    if matches!(id, "minecraft:mushroom_stew" | "minecraft:beetroot_soup") {
        return (Some(Healing::Soup), false);
    }
    // `ItemStackScoreUtil.o`: any potion item (drinkable, splash or
    // lingering) whose contents, base or custom, include instant health or
    // regeneration. ChestSteal separately rejects the five harmful splash
    // effects named by the reference; both decisions come from Horizon's
    // decoded PotionContents component, never from an item-id suffix.
    if !matches!(
        id,
        "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion"
    ) {
        return (None, false);
    }
    let definition = stack.potion().and_then(brewing::potion);
    let healing = definition
        .iter()
        .flat_map(|d| d.effects.iter().map(|(effect, _, _)| (*effect).to_owned()))
        .chain(
            stack
                .custom_potion_effects()
                .into_iter()
                .map(|(effect, _, _)| unprefixed(&effect)),
        )
        .any(|effect| matches!(effect.as_str(), "instant_health" | "regeneration"))
        .then_some(Healing::Potion);
    let harmful = id == "minecraft:splash_potion"
        && definition.is_some_and(|d| {
            d.effects.iter().any(|(effect, _, _)| {
                matches!(
                    *effect,
                    "poison" | "instant_damage" | "slowness" | "weakness" | "blindness"
                )
            })
        });
    (healing, harmful)
}
fn stack(game: &Game<'_>, stack: &ItemStack) -> Item {
    let info = game.res.items().get(&stack.item);
    let id = stack.item.to_string();
    let tag_hash = hash(format!("{:?}", stack.extra).as_bytes());
    let (healing, harmful_splash) = potion_metadata(&id, stack);
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
        healing,
        harmful_splash,
        meta: meta(game, &id, stack, info),
    }
}
fn unprefixed(id: &str) -> String {
    id.strip_prefix("minecraft:").unwrap_or(id).to_string()
}
/// Copies the decoded components rule matching reads. Potion effects come
/// from PotionContents (base potion definition plus custom effects).
fn meta(
    game: &Game<'_>,
    id: &str,
    stack: &ItemStack,
    info: Option<&horizon_client::items::ItemInfo>,
) -> ItemMeta {
    let potion_item = matches!(
        id,
        "minecraft:potion" | "minecraft:splash_potion" | "minecraft:lingering_potion"
    );
    let mut effects = Vec::new();
    if potion_item {
        if let Some(definition) = stack.potion().and_then(brewing::potion) {
            effects.extend(definition.effects.iter().map(|(e, duration, amplifier)| {
                PotionEffect {
                    id: unprefixed(e),
                    amplifier: *amplifier,
                    duration: *duration,
                }
            }));
        }
        effects.extend(
            stack
                .custom_potion_effects()
                .into_iter()
                .map(|(e, duration, amplifier)| PotionEffect {
                    id: unprefixed(&e),
                    amplifier,
                    duration,
                }),
        );
    }
    let block = info.and_then(|i| i.block);
    let items = game.res.items();
    // Tool score: the best speed against one representative block of each
    // vanilla tool family (pickaxe/axe/shovel/hoe).
    let mining_speed = [
        "minecraft:stone",
        "minecraft:oak_log",
        "minecraft:dirt",
        "minecraft:hay_block",
    ]
    .iter()
    .map(|b| items.mining_speed(Some(stack), b))
    .fold(1.0_f32, f32::max);
    // The tooltip's name line in the client language: custom name first,
    // then the item's own name; the item name is the same line without a
    // custom name.
    let translate = |key: &str| game.pack.names.display_get(key);
    let display_name = horizon_client::item_tooltip::name_line(stack, &translate);
    let item_name = if stack.custom_name().is_some() {
        let mut plain = stack.clone();
        plain.data_mut().custom_name = None;
        horizon_client::item_tooltip::name_line(&plain, &translate)
    } else {
        display_name.clone()
    };
    ItemMeta {
        custom_name: stack.custom_name(),
        display_name,
        item_name,
        lore: horizon_client::item_tooltip::lore_texts(stack)
            .iter()
            .map(|line| line.plain())
            .collect(),
        effects,
        thrown_potion: matches!(
            id,
            "minecraft:splash_potion" | "minecraft:lingering_potion"
        ),
        food: info.and_then(|i| i.food),
        block: block.is_some(),
        hardness: block.map(|state| game.res.facts().get(state).hardness),
        enchantments: stack
            .enchantments()
            .iter()
            .map(|(e, level)| (e.to_string(), *level))
            .collect(),
        mining_speed,
        hover_name: horizon_client::item_tooltip::name_line(stack, &|k| {
            game.pack.names.display_get(k)
        }),
        armor_points: info.map_or(0.0, |i| armor_values(i).0),
        armor_toughness: info.map_or(0.0, |i| armor_values(i).1),
        mainhand_modifiers: info.map_or(0.0, |i| {
            i.modifiers
                .iter()
                .filter(|(.., slot)| slot == "mainhand")
                .map(|(_, _, amount, ..)| *amount as f32)
                .sum()
        }),
        first_modifier: info
            .and_then(|i| i.modifiers.first())
            .map_or(0.0, |(_, _, amount, ..)| *amount as f32),
        tool: info.is_some_and(|i| i.tool_damage_per_block.is_some()),
    }
}
fn slot(game: &Game<'_>, index: usize, role: SlotRole, value: Option<&ItemStack>) -> Slot {
    Slot {
        index: index as u16,
        item: value.map(|s| stack(game, s)),
        role,
    }
}
fn revision(game: &Game<'_>, slots: &[Slot], carried: Option<&Item>) -> u32 {
    if let Some(remote) = &game.remote {
        return remote.state_id.max(0) as u32;
    }
    // Single player has no protocol state-id. A content fingerprint changes
    // only when the ordinary local menu engine changed its authoritative copy.
    fn fold_item(h: u64, item: Option<&Item>) -> u64 {
        let Some(item) = item else {
            return hash_from(h, &[0]);
        };
        let h = hash_from(h, &[1]);
        let h = hash_from(h, item.fingerprint.id.as_bytes());
        let h = hash_from(h, &item.fingerprint.count.to_le_bytes());
        let h = hash_from(h, &item.fingerprint.damage.to_le_bytes());
        hash_from(h, &item.fingerprint.tag_hash.to_le_bytes())
    }
    let h = slots.iter().fold(0xcbf29ce484222325, |h, slot| {
        fold_item(hash_from(h, &slot.index.to_le_bytes()), slot.item.as_ref())
    });
    let h = fold_item(h, carried);
    (h ^ (h >> 32)) as u32
}

fn vitals(game: &Game<'_>) -> Vitals {
    let Some(player) = game.player.as_ref() else {
        return Vitals::default();
    };
    let max_health = player
        .attributes
        .lookup(player.attributes.key("minecraft:max_health").as_ref())
        .and_then(|h| player.attributes.instance(&h).ok())
        .map_or(20.0, |i| i.cached_value() as f32);
    Vitals {
        health: player.health.health(),
        max_health,
        selected: game.selected.min(8) as u8,
        on_ground: player.ground.on_ground,
        effects: game
            .potions
            .effects
            .list
            .iter()
            .map(|e| unprefixed(&e.id))
            .collect(),
        using_item: game.using.is_some(),
        view: (game.yaw, game.pitch),
    }
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
    slots.push(slot(game, 504, SlotRole::Other, game.offhand.as_ref()));
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
    let carried = game.carried.as_ref().map(|s| stack(game, s));
    let out = Snapshot {
        session,
        container_id,
        revision: revision(game, &slots, carried.as_ref()),
        slots,
        carried,
        screen_open,
        is_player_inventory: screen_open && !is_chest,
        is_chest,
        chest_title_standard,
        vitals: vitals(game),
        observed: std::time::Instant::now(),
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
    if request.click == Click::Outside {
        // Slot -999: only the carried stack the planner saw may be dropped.
        return request.expected_carried.is_some()
            && current.carried.as_ref().map(|i| &i.fingerprint) == request.expected_carried.as_ref()
            && game.menu_ops(&[(None, 0, Input::Pickup)]);
    }
    if !current_matches(&current, request) {
        return false;
    }
    let Some((button, input)) = click_input(request.click) else {
        return false;
    };
    // Game slot ids are intentionally preserved: menu_ops translates them to
    // each concrete local/remote menu's authoritative menu index, predicts
    // only through Horizon's verified engine, and serializes state/changed
    // slots/carried stack for remote ContainerClick packets.
    game.menu_ops(&[(Some(usize::from(request.slot)), button, input)])
}
/// The button and click type of a planned click on a slot.
fn click_input(click: Click) -> Option<(i32, Input)> {
    Some(match click {
        Click::Pickup | Click::Outside => (0, Input::Pickup),
        Click::SwapOffhand => (40, Input::Swap),
        Click::QuickMove => (0, Input::QuickMove),
        Click::Throw => (0, Input::Throw),
        Click::ThrowStack => (1, Input::Throw),
        Click::SwapHotbar(n) if n < 9 => (i32::from(n), Input::Swap),
        Click::SwapHotbar(_) => return None,
    })
}
/// The request still names this session, menu, revision and slot contents.
fn current_matches(current: &Snapshot, request: &Request) -> bool {
    current.session == request.session
        && current.container_id == request.container_id
        && current.revision == request.revision
        && current
            .slot(request.slot)
            .is_some_and(|slot| slot.item.as_ref().map(|i| &i.fingerprint) == request.expected.as_ref())
        && current.carried.as_ref().map(|i| &i.fingerprint) == request.expected_carried.as_ref()
}
/// A player-menu click with no screen shown: AutoTotem's silent swap,
/// AutoArmor's clicks with both of its screen settings off and AutoHeal's
/// "Bowl Mode" throws and stacking. The click goes
/// to the player menu (window 0) through the remote menu engine, which
/// predicts it and sends the ordinary `ContainerClick`; the server's player
/// menu needs no open screen. A shown inventory screen takes the ordinary
/// path.
fn submit_silent(game: &mut Game<'_>, request: &Request) -> bool {
    if game.screen == Screen::Inventory {
        return submit(game, request);
    }
    let Some(current) = snapshot(game) else {
        return false;
    };
    if current.screen_open || current.container_id != 0 || game.remote.is_none() {
        return false;
    }
    let op = match request.click {
        // Slot -999: only the carried stack the planner saw may be dropped.
        Click::Outside => {
            let carried = current.carried.as_ref().map(|i| &i.fingerprint);
            if current.session != request.session
                || current.revision != request.revision
                || request.expected_carried.is_none()
                || carried != request.expected_carried.as_ref()
            {
                return false;
            }
            (None, 0, Input::Pickup)
        }
        _ if !current_matches(&current, request) => return false,
        click => match click_input(click) {
            Some((button, input)) => (Some(usize::from(request.slot)), button, input),
            None => return false,
        },
    };
    game.remote_menu_ops(&[op])
}
/// `closeInventory` of a silent session (`closeScreen`): the player menu's
/// `ContainerClose`, or the player's own inventory screen if one was shown
/// (`close_screen`). A container's menu replaced the player menu: nothing to
/// close.
fn silent_close(game: &mut Game<'_>, close_screen: bool) {
    if game.remote.as_ref().is_none_or(|r| r.menu.is_some()) {
        return;
    }
    match game.screen {
        Screen::Inventory if close_screen => {
            game.close_container();
            game.screen = Screen::Playing;
            game.capture(true);
        }
        Screen::Inventory => {}
        _ => game.remote_close_container(),
    }
}

/// AutoTotem's silent open holds attack, use and block breaking (only on
/// the window's own game, not the integrated server's).
pub fn input_held(game: &Game<'_>) -> bool {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return false;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    with_client(&client, |r| r.auto_totem_input_held())
}

// Dispatch on frames so an open inventory menu can advance while the
// world simulation is paused. Scheduling uses monotonic time in the runtime.
pub fn dispatch(game: &mut Game<'_>) {
    use crate::game::vape_rs::core::inventory_runtime::InventoryEffect;
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let allowed = matches!(game.screen, Screen::Playing | Screen::Inventory)
        && game.focused
        && game.title.is_none()
        && game
            .player
            .as_ref()
            .is_some_and(|p| p.health.health() > 0.0)
        && !game.dialog_open()
        && game.server_packs.prompt.is_none()
        && !game.creative
        && game.sleeping.is_none()
        && with_client(&client, |r| !r.gui.open);
    let snapshot = allowed.then(|| snapshot(game)).flatten();
    let Some(snapshot) = snapshot else {
        let owed = with_client(&client, |r| {
            r.cancel_inventory();
            r.cancel_throws();
            r.take_silent_close()
        });
        // A cancelled silent session's server-side close; a screen the user
        // opened stays.
        if owed {
            silent_close(game, false);
        }
        return;
    };
    if with_client(&client, |r| r.take_silent_close()) {
        silent_close(game, false);
    }
    // Hotbar use sequences (Throwpot, ThrowDebuff, AutoHeal) through the
    // ordinary selected-slot, use and drop paths; only while playing.
    let playing = game.screen == Screen::Playing;
    let throws = with_client(&client, |r| r.throw_frame(&snapshot, playing));
    for effect in throws {
        use crate::game::vape_rs::core::throw_runtime::ThrowEffect;
        match effect {
            ThrowEffect::Select(slot) if slot < 9 => game.selected = usize::from(slot),
            ThrowEffect::Use => super::vape_macros::use_once(game),
            ThrowEffect::Drop if game.remote.is_some() => game.remote_drop(false),
            ThrowEffect::Drop => game.drop_held(false),
            ThrowEffect::SilentUse(slot) => {
                silent_use(game, usize::from(slot), None);
            }
            _ => {}
        }
    }
    // InventoryFill: the hovered slot of an open container screen.
    if game.screen == Screen::Inventory
        && let Some(size) = game.renderer.as_ref().map(|r| [r.config.width, r.config.height])
    {
        use crate::game::vape_rs::core::inventory_fill::FillInput;
        use winit::{event::MouseButton, keyboard::KeyCode};
        let batch = horizon_client::hud::HudBatch::new(size);
        let input = FillInput {
            now: snapshot.observed,
            container_screen: true,
            shift: game.physical_keys.contains(&KeyCode::ShiftLeft)
                || game.physical_keys.contains(&KeyCode::ShiftRight),
            left: game.physical_buttons.contains(&MouseButton::Left),
            hovered: game
                .slot_at(&batch, game.cursor.0, game.cursor.1)
                .and_then(|s| u16::try_from(s).ok()),
            carrying: snapshot.carried.is_some(),
        };
        if let Some(slot) = with_client(&client, |r| r.inventory_fill(input)) {
            game.menu_ops(&[(Some(usize::from(slot)), 0, Input::QuickMove)]);
        }
    }
    let friends = with_client(&client, |r| r.friend_keys());
    let combat = crate::game::vape_inventory::combat(game, &friends, None);
    let effects = with_client(&client, |r| {
        r.update_combat(combat);
        r.inventory_step(&snapshot)
    });
    for effect in effects {
        match effect {
            InventoryEffect::Request(request) => {
                if !submit(game, &request) {
                    with_client(&client, |r| r.inventory_rejected());
                }
            }
            InventoryEffect::SilentRequest(request) => {
                if !submit_silent(game, &request) {
                    with_client(&client, |r| r.inventory_rejected());
                }
            }
            InventoryEffect::SilentClose => silent_close(game, true),
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

/// AutoHeal Silent's use: `CPacketHeldItemChange(slot)`, `sendUseItem`,
/// `CPacketHeldItemChange(current)`. The server holds `slot` for this one
/// `UseItem` (Horizon's own `remote_use_item`, which also starts the local
/// use it predicts); the client's selection and shown hotbar never change.
/// `look` is the yaw/pitch the use carries (Potions' looking down). Only a
/// server keeps its carried slot apart from the shown one: the deprecated
/// in-frame world gets nothing.
fn silent_use(game: &mut Game<'_>, slot: usize, look: Option<(f32, f32)>) -> bool {
    use horizon_client::first_person::Hand;
    use horizon_protocol::packets::play::{self, Serverbound as S};
    let held = game.inventory.get(slot).is_some_and(Option::is_some);
    if slot >= 9 || !held || game.remote.is_none() {
        return false;
    }
    let (selected, view) = (game.selected, (game.yaw, game.pitch));
    game.send(S::SetCarriedItem(play::SetCarriedItem { slot: slot as i16 }));
    game.selected = slot;
    if let Some((yaw, pitch)) = look {
        (game.yaw, game.pitch) = (yaw, pitch);
    }
    game.remote_use_item(Hand::Main);
    game.selected = selected;
    (game.yaw, game.pitch) = view;
    game.send(S::SetCarriedItem(play::SetCarriedItem {
        slot: selected as i16,
    }));
    if let Some(r) = game.remote.as_mut() {
        r.carried = Some(selected);
    }
    true
}

thread_local! {
    /// Potions' look for this motion packet, and the managed rotation it
    /// replaced (restored at PostMotion).
    static POTION_LOOK: std::cell::Cell<Option<((f32, f32), Option<(f32, f32)>)>> =
        const { std::cell::Cell::new(None) };
}
fn motion_allowed(game: &Game<'_>) -> bool {
    game.remote.is_some()
        && game.title.is_none()
        && game.player.as_ref().is_some_and(|p| p.health.health() > 0.0)
}
/// `auto-heal-pre-motion` (HEAD of `remote_send_state`): AutoHeal's
/// `EventPreMotion`. Replace's window-0 SWAP goes out first (the server's
/// player menu needs no screen; with a container open the server would
/// ignore it); an armed potion makes this motion packet look down through
/// the managed rotation, keeping its yaw.
pub fn auto_heal_pre_motion(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if !motion_allowed(game) || !with_client(&client, |r| r.auto_heal_potions()) {
        return;
    }
    let Some(snapshot) = snapshot(game) else {
        return;
    };
    let motion = with_client(&client, |r| r.auto_heal_pre_motion(&snapshot));
    if let Some((from, button)) = motion.swap
        && game.remote.as_ref().is_some_and(|r| r.menu.is_none())
    {
        game.remote_menu_ops(&[(Some(usize::from(from)), i32::from(button), Input::Swap)]);
    }
    if let Some(pitch) = motion.pitch {
        use super::vape_combat_remote as remote;
        let managed = remote::rotation((f32::INFINITY, 0.0));
        let previous = managed.0.is_finite().then_some(managed);
        let yaw = previous.map_or(game.yaw, |(yaw, _)| yaw);
        remote::set_rotation(Some((yaw, pitch)));
        POTION_LOOK.with(|l| l.set(Some(((yaw, pitch), previous))));
    }
}
/// `auto-heal-post-motion` (RETURN of `remote_send_state`): AutoHeal's
/// `EventPostMotion`. The armed potion is used silently with the look the
/// motion packet sent, and the managed rotation goes back.
pub fn auto_heal_post_motion(game: &mut Game<'_>) {
    let look = POTION_LOOK.with(std::cell::Cell::take);
    if let Some((_, previous)) = look {
        super::vape_combat_remote::set_rotation(previous);
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    if !motion_allowed(game) {
        return;
    }
    let now = std::time::Instant::now();
    if let Some(slot) = with_client(&client, |r| r.auto_heal_post_motion(now)) {
        silent_use(game, usize::from(slot), look.map(|(look, _)| look));
    }
}
