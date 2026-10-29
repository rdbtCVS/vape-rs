// Host adapter installed as game::fishing::vape_auto_fish so it can read the
// private local/remote Hook state without heuristic motion or packet guesses.
use super::State;
use crate::game::{Game, Screen};
use crate::game::vape_rs::{
    core::{with_client, ModuleId},
    modules::utility::auto_fish::{Context, Effect, Hook, Options, Rod},
};
use std::time::Instant;

fn rod(game: &Game<'_>) -> Rod {
    if game
        .held()
        .is_some_and(|stack| &*stack.item == "minecraft:fishing_rod")
    {
        return Rod::Held;
    }
    if let Some(slot) = game.inventory.iter().take(9).position(|stack| {
        stack
            .as_ref()
            .is_some_and(|stack| &*stack.item == "minecraft:fishing_rod")
    }) {
        return Rod::Hotbar(slot as u8);
    }
    game.inventory
        .iter()
        .enumerate()
        .skip(9)
        .find_map(|(slot, stack)| {
            stack
                .as_ref()
                .is_some_and(|stack| &*stack.item == "minecraft:fishing_rod")
                .then_some(Rod::Inventory(slot as u16))
        })
        .unwrap_or(Rod::Missing)
}

fn hook(game: &Game<'_>, remote_target: Option<i32>) -> Hook {
    let Some(hook) = game.hook.as_ref() else {
        return Hook::Missing;
    };
    if remote_target.is_some() {
        return Hook::Caught;
    }
    // Remote owned hooks report DATA_BITING through `nibble` while their
    // mirrored state remains Flying, so this must take priority over state.
    if hook.nibble > 0 {
        return Hook::Water { biting: true };
    }
    match hook.state {
        State::Hooked(_) => Hook::Caught,
        State::Bobbing => Hook::Water {
            biting: hook.nibble > 0,
        },
        State::Flying if hook.on_ground => Hook::Ground,
        State::Flying => Hook::Flying,
    }
}

// Macro double-click actions can finish their fishing wait when the bobber is
// grounded, has locally attached to a real player target, or received a
// non-sentinel remote DATA_HOOKED_ENTITY value retained by the adapter.
pub(crate) fn macro_rod_ready(game: &Game<'_>) -> bool {
    let Some(hook) = game.hook.as_ref() else {
        return false;
    };
    if hook.on_ground {
        return true;
    }
    match hook.state {
        State::Hooked(id) => game
            .creatures
            .hook_targets
            .iter()
            .any(|target| target.id == id),
        _ if game.remote.is_some() => {
            let client = game.__horizon_mixins_vape_rs_client().clone();
            crate::game::vape_rs::core::with_client(&client, |runtime| {
                runtime.fish_hook_target(hook.id).is_some_and(|target| {
                    game.mobs
                        .iter()
                        .any(|mob| mob.id as i32 == target && &*mob.kind.id == "minecraft:player")
                })
            })
        }
        _ => false,
    }
}

fn settings(runtime: &crate::game::vape_rs::core::Runtime) -> Options {
    let bool = |key, default| {
        runtime
            .settings()
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(default)
    };
    let delay = runtime
        .settings()
        .get("utility.auto-fish.click-delay")
        .and_then(serde_json::Value::as_array)
        .filter(|range| range.len() == 2)
        .and_then(|range| {
            let lo = range[0].as_f64()?;
            let hi = range[1].as_f64()?;
            (lo.is_finite() && hi.is_finite()).then_some((
                lo.clamp(50.0, 200.0).round() as u64,
                hi.clamp(50.0, 200.0).round() as u64,
            ))
        })
        .unwrap_or((75, 125));
    Options {
        recast_ground: bool("utility.auto-fish.recast-ground", false),
        recast_caught: bool("utility.auto-fish.recast-caught", false),
        refill_rods: bool("utility.auto-fish.refill-rods", true),
        click_delay_ms: delay,
    }
}

pub(crate) fn auto_fish(game: &mut Game<'_>) {
    let inventory_open = game.screen == Screen::Inventory;
    let playable = game.focused
        && game
            .player
            .as_ref()
            .is_some_and(|player| player.health.health() > 0.0)
        && game.title.is_none()
        && game.sleeping.is_none()
        && !game.dialog_open()
        && game.server_packs.prompt.is_none()
        && !game.spectator();
    let client = game.__horizon_mixins_vape_rs_client().clone();
    // Take the host snapshot before locking the runtime. The runtime gate
    // waits, with a deadline, for the queued click's visible effect.
    let snapshot = crate::game::remote::vape_transactions::snapshot(game);
    let remote_target = game
        .hook
        .as_ref()
        .and_then(|hook| {
            game.remote
                .as_ref()
                .is_some()
                .then(|| with_client(&client, |runtime| runtime.fish_hook_target(hook.id)))
        })
        .flatten();
    let context = Context {
        now: Instant::now(),
        rod: rod(game),
        hook: hook(game, remote_target),
        inventory_open,
    };
    let effect = with_client(&client, |runtime| {
        if !playable
            || !runtime.enabled(ModuleId::AutoFish)
            || runtime.macros_active_item()
            || (game.screen != Screen::Playing
                && !(inventory_open && runtime.auto_fish_refilling()))
        {
            runtime.cancel_auto_fish();
            return None;
        }
        if matches!(context.rod, Rod::Inventory(_))
            && runtime.inventory_busy()
            && !runtime.auto_fish_refilling()
        {
            return None;
        }
        let options = settings(runtime);
        runtime.auto_fish_step(context, options, snapshot.as_ref())
    });
    match effect {
        Some(Effect::Use) => crate::game::remote::vape_macros::use_once(game),
        Some(Effect::SelectHotbar(slot)) if slot < 9 => game.selected = usize::from(slot),
        // AutoFish opens the ordinary inventory and moves a real inventory
        // slot through Horizon's menu engine; a later frame observes the
        // resulting authoritative slot contents before selecting it.
        Some(Effect::OpenInventory) => {
            game.screen = Screen::Inventory;
            game.capture(false);
        }
        Some(Effect::QuickMove(slot)) => {
            let Some(snapshot) = snapshot.as_ref() else {
                with_client(&client, |runtime| runtime.cancel_auto_fish());
                return;
            };
            let Some(source) = snapshot.slot(slot) else {
                with_client(&client, |runtime| runtime.cancel_auto_fish());
                return;
            };
            let request = crate::game::vape_rs::inventory::types::Request::for_slot(
                snapshot,
                source,
                crate::game::vape_rs::inventory::types::Click::QuickMove,
            );
            if !crate::game::remote::vape_transactions::submit(game, &request) {
                with_client(&client, |runtime| runtime.cancel_auto_fish());
            } else {
                with_client(&client, |runtime| runtime.auto_fish_queued(snapshot, request));
            }
        }
        Some(Effect::CloseInventory) => {
            game.close_container();
            game.screen = Screen::Playing;
            game.capture(true);
        }
        Some(Effect::Disable) => with_client(&client, |runtime| {
            runtime.set_enabled(ModuleId::AutoFish, false)
        }),
        _ => {}
    }
}
