// Client tick HEAD for KillAura and SilentAura: snapshots for the runtime,
// then the attacks by id and the managed rotation for this tick's motion
// packet (game::remote::vape_combat_remote).
use crate::game::remote::vape_combat_remote as remote;
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::modules::combat::{kill_aura, silent_aura, trigger_bot::Cooldown};
use crate::game::vape_rs::targeting::entities::PlayerView;
use crate::game::{Game, Screen};

fn no_screen(game: &Game<'_>) -> bool {
    game.screen == Screen::Playing && game.title.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none()
}

fn cooldown_ready(game: &Game<'_>) -> bool {
    Cooldown {
        ticks: game.hand_strength.ticker as f32,
        delay: horizon_client::attack_strength::delay(game.main_hand_attack_speed()),
    }
    .ready(0.0)
}

/// Every snapshot entity's previous tick position.
fn previous(game: &Game<'_>) -> Vec<(i32, [f64; 3])> {
    let mut out: Vec<_> = game.mobs.iter().map(|m| (m.id as i32, [m.previous.x, m.previous.y, m.previous.z])).collect();
    if let Some(host) = &game.lan {
        out.extend(host.hub.guests.iter().map(|g| (g.entity_id, g.previous_position)));
    }
    out
}

pub(super) fn tick(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (kill, silent, epoch, now, walls) = with_client(&client, |runtime| {
        let walls = |id: &str| runtime.settings().get(&format!("{id}.targets.ignore-walls")).and_then(serde_json::Value::as_bool).unwrap_or(false);
        (
            runtime.kill_aura_active(),
            runtime.silent_aura_wanted(),
            runtime.session().epoch(),
            runtime.now_ms(),
            walls(kill_aura::ID) || walls(silent_aura::ID),
        )
    });
    remote::set_rotation(None);
    let Some(p) = game.player.as_ref() else { return; };
    if (!kill && !silent) || game.title.is_some() {
        return;
    }
    let position = [p.position.x, p.position.y, p.position.z];
    let player = PlayerView { position, tick_position: position, yaw: game.yaw, pitch: game.pitch, strafe: 0.0 };
    let alive = p.health.health() > 0.0;
    let eye_height = f64::from(p.dimensions.eye_height);
    let motion = [
        p.position.x - game.previous_position.x,
        p.position.y - game.previous_position.y,
        p.position.z - game.previous_position.z,
    ];
    let entities = super::vape_combat::snapshot(game, epoch, walls.then_some(8.0));
    let held = super::vape_combat::held(game);
    if kill {
        let tick = kill_aura::Tick {
            now_ms: now,
            player,
            entities: entities.clone(),
            attack_down: game.left,
            use_down: game.right,
            using: game.using.is_some(),
            no_screen: no_screen(game),
            focused: game.focused,
            alive,
            held: held.clone(),
            cooldown_ready: cooldown_ready(game),
        };
        let effects = with_client(&client, |runtime| runtime.kill_aura_tick(&tick));
        if effects.release_use {
            game.right = false;
        }
        for (id, attack) in effects.swings {
            if !(attack && remote::attack(game, id)) {
                remote::swing(game);
            }
        }
        if effects.reuse {
            crate::game::remote::vape_macros::use_once(game);
        }
    }
    if silent {
        let tick = silent_aura::Tick {
            now_ms: now,
            player,
            eye_height,
            motion,
            entities,
            previous: previous(game),
            attack_down: game.left,
            focused: game.focused,
            no_screen: no_screen(game),
            alive,
            held,
            sensitivity: f64::from(game.settings.sensitivity),
            cooldown_ready: cooldown_ready(game),
            shield_bypass: false,
        };
        // HitFlick's start paths (the attack intercepts) are not hooked yet,
        // so it never leaves Idle and needs no host view here.
        let flick = crate::game::vape_rs::core::combat_runtime::Flick::default();
        let result = with_client(&client, |runtime| runtime.silent_aura_tick(&tick, &flick));
        remote::set_rotation(result.sent);
        match result.click {
            silent_aura::Click::Attack(id) => {
                if !remote::attack(game, id) {
                    remote::miss(game);
                }
                crate::game::vape_block_hit::silent_click(game);
            }
            silent_aura::Click::Miss => {
                remote::miss(game);
                crate::game::vape_block_hit::silent_click(game);
            }
            silent_aura::Click::None => {}
        }
    }
}
