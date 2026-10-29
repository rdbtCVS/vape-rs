// Client-only input-delay removals. Install at Game::tick HEAD, before either
// local or remote player simulation decrements/uses these host-owned timers.
use crate::game::vape_rs::core::{with_client, ModuleId};

pub(crate) fn input_delays(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some()
        || game.options.dedicated.is_some()
        || !game.focused
        || game.screen != crate::game::Screen::Playing
        || game.title.is_some()
        || game.sleeping.is_some()
        || game.dialog_open()
        || game.server_packs.prompt.is_some()
        || game.spectator()
        || !game
            .player
            .as_ref()
            .is_some_and(|player| player.health.health() > 0.0)
    {
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (no_jump_delay, no_click_delay) = with_client(&client, |runtime| {
        (
            runtime.enabled(ModuleId::NoJumpDelay),
            runtime.enabled(ModuleId::NoClickDelay),
        )
    });
    if no_jump_delay {
        if let Some(player) = game.player.as_mut() {
            // PlayerMotion::no_jump_delay is the actual ten-tick post-jump
            // gate checked by MotionWorld::jump_phase.
            player.no_jump_delay = 0;
        }
    }
    if no_click_delay && game.left {
        // Clicks::miss_time is the real missed-attack cooldown. Do not clear
        // it after attack is released: the host's normal release path owns
        // that transition, matching the source's attack-button condition.
        game.clicks.miss_time = 0;
    }
}

// Feed the same key state consumed by both local and remote movement ticks.
// Remember ownership so disabling does not release a physically held key.
pub(crate) fn sprint(game: &mut crate::game::Game<'_>) {
    use winit::keyboard::KeyCode;
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (enabled, cancel_invis, previous) = with_client(&client, |runtime| {
        let enabled = runtime.enabled(ModuleId::Sprint);
        let cancel = runtime.settings().get("combat.sprint.cancel-invis")
            .and_then(serde_json::Value::as_bool).unwrap_or(false);
        (enabled, cancel, runtime.replace_sprint_input_owner(false))
    });
    let physical = game.physical_keys.contains(&KeyCode::ControlLeft);
    if previous && !physical { game.keys.remove(&KeyCode::ControlLeft); }
    let invisible = game.potions.effects.list.iter().any(|effect| effect.id == "invisibility");
    let active = enabled && game.focused && game.screen == crate::game::Screen::Playing
        && game.title.is_none() && game.sleeping.is_none() && !game.dialog_open()
        && game.server_packs.prompt.is_none() && !game.spectator()
        && game.player.as_ref().is_some_and(|player| player.health.health() > 0.0)
        && !(cancel_invis && invisible);
    if active && !game.keys.contains(&KeyCode::ControlLeft) {
        game.keys.insert(KeyCode::ControlLeft);
        with_client(&client, |runtime| { runtime.replace_sprint_input_owner(true); });
    }
}
