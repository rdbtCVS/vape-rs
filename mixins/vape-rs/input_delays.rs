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

// The single writer of synthetic movement keys. Host canonical keys already
// include rebinding, controllers and the control socket; sneak/sprint follow
// the host toggle mappings. Runs before local and remote movement ticks.
use crate::game::vape_rs::core::movement_input::{Axis, HostView, Overrides, Owner, Request};
use winit::keyboard::KeyCode;

const KEYS: [(Axis, KeyCode); 7] = [
    (Axis::Forward, KeyCode::KeyW),
    (Axis::Back, KeyCode::KeyS),
    (Axis::Left, KeyCode::KeyA),
    (Axis::Right, KeyCode::KeyD),
    (Axis::Jump, KeyCode::Space),
    (Axis::Sneak, KeyCode::ShiftLeft),
    (Axis::Sprint, KeyCode::ControlLeft),
];

/// The movement axis of a canonical (already rebound) gameplay key.
pub(crate) fn axis(key: KeyCode) -> Option<Axis> {
    KEYS.iter().find(|(_, k)| *k == key).map(|(axis, _)| *axis)
}

fn playing(game: &crate::game::Game<'_>) -> bool {
    game.focused
        && game.screen == crate::game::Screen::Playing
        && game.title.is_none()
        && game.sleeping.is_none()
        && !game.dialog_open()
        && game.server_packs.prompt.is_none()
        && !game.spectator()
        && game.player.as_ref().is_some_and(|player| player.health.health() > 0.0)
}

/// Collects this tick's module requests and applies the resolved keys. With
/// `requests` false (screens, pause, focus loss) it only hands owned axes
/// back to the user's state.
pub(crate) fn movement(game: &mut crate::game::Game<'_>, requests: bool) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let view = HostView {
        keys: KEYS.map(|(_, key)| game.keys.contains(&key)),
        toggles: [game.toggles.is_down(0), game.toggles.is_down(1)],
    };
    let active = requests && playing(game);
    let invisible = game.potions.effects.list.iter().any(|effect| effect.id == "invisibility");
    let (sprint, parkour, forward, jump) = with_client(&client, |runtime| {
        let cancel_invis = runtime.settings().get("combat.sprint.cancel-invis")
            .and_then(serde_json::Value::as_bool).unwrap_or(false);
        let tapped = runtime.replace_parkour_tap(false);
        (
            active && runtime.enabled(ModuleId::Sprint) && !(cancel_invis && invisible),
            active && runtime.enabled(ModuleId::Parkour) && !tapped,
            runtime.movement().user(Axis::Forward, &view),
            runtime.movement().user(Axis::Jump, &view),
        )
    });
    let mut overrides = Overrides::default();
    if sprint {
        overrides.request(Request { axis: Axis::Sprint, owner: Owner::Sprint, priority: 1, pressed: true });
    }
    // Parkour taps jump for one tick when the user moves forward without
    // jumping and the ground ends one block ahead.
    let tap = parkour && forward && !jump && crate::game::vape_parkour::edge_ahead(game);
    if tap {
        overrides.request(Request { axis: Axis::Jump, owner: Owner::Parkour, priority: 1, pressed: true });
    }
    // SafeWalk (reference onPreMove): edge back-off for this tick's move,
    // except when walking straight forward with Direction Check on.
    let safe_walk = with_client(&client, |runtime| {
        active
            && runtime.enabled(ModuleId::SafeWalk)
            && !(runtime.settings().get("world.safe-walk.direction-check")
                .and_then(serde_json::Value::as_bool).unwrap_or(true)
                && !runtime.movement().user(Axis::Sneak, &view)
                && runtime.movement().user(Axis::Forward, &view)
                && !runtime.movement().user(Axis::Back, &view)
                && runtime.movement().user(Axis::Left, &view) == runtime.movement().user(Axis::Right, &view))
    });
    if let Some(player) = game.player.as_ref() {
        player.vape_rs_set_safe_walk(safe_walk);
    }
    let writes = with_client(&client, |runtime| {
        runtime.replace_parkour_tap(tap);
        runtime.movement().resolve(&overrides, &view)
    });
    for (axis, pressed) in writes {
        let key = KEYS[KEYS.iter().position(|(a, _)| *a == axis).unwrap()].1;
        if pressed {
            game.keys.insert(key);
        } else {
            game.keys.remove(&key);
        }
    }
}
