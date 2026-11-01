// FastPlace's host adapter; install inside game.rs and invoke from a HEAD hook
// on Game::tick before the host decrements and reassigns use_delay.
use crate::game::vape_rs::{
    core::{with_client, ModuleId},
    modules::world::fast_place::{applies, clamp_delay, HeldItem, Options},
};
pub(super) fn fast_place(game: &mut crate::game::Game<'_>) {
    // `FastPlace.onTick` checks only the player and the right-click claim; the
    // host reads use_delay only when it processes the use key.
    if game.serving.is_some() || game.options.dedicated.is_some() || game.player.is_none() {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let options = with_client(&client, |runtime| {
        if !runtime.enabled(ModuleId::FastPlace) || runtime.use_claimed() {
            return None;
        }
        let held_item = match runtime
            .settings()
            .get("world.fast-place.held-item")
            .and_then(serde_json::Value::as_str)
        {
            Some("blocks") => HeldItem::Blocks,
            Some("projectiles") => HeldItem::Projectiles,
            _ => HeldItem::All,
        };
        let delay = runtime
            .settings()
            .get("world.fast-place.delay")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value.round().clamp(0.0, 4.0) as u8)
            .unwrap_or(1);
        Some(Options { held_item, delay })
    });
    let Some(options) = options else {
        return;
    };
    let main = game
        .inventory
        .get(game.selected)
        .and_then(Option::as_ref)
        .map(|s| &*s.item);
    let off = game.offhand.as_ref().map(|s| &*s.item);
    if applies(options, main, off, |id| {
        game.res
            .items()
            .get(id)
            .is_some_and(|item| item.block.is_some())
    }) {
        game.use_delay = clamp_delay(game.use_delay, options);
    }
}
