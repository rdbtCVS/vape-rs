// DESIGN DRAFT ONLY — excluded from modules, native pack and compilation.
// Proposed support module for game/fishing.rs. A cancellable HEAD on
// Game::fishing_line would call `fishing_line`: disabled returns Continue(())
// so the pristine host body runs; enabled copies the pinned body and appends
// selected-block outline geometry.
use super::Game;
use crate::game::vape_rs::{
    core::{ModuleId, with_client},
    modules::render::block_overlay::{Options, expand},
};
use core::ops::ControlFlow;
use horizon_client::gizmos::{Gizmos, Line};

fn options(game: &Game<'_>) -> Option<Options> {
    if game.serving.is_some()
        || game.options.dedicated.is_some()
        || !game.focused
        || game.screen != super::Screen::Playing
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
        return None;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    with_client(&client, |runtime| {
        if !runtime.enabled(ModuleId::BlockOverlay) {
            return None;
        }
        let color = |key, default| {
            runtime
                .settings()
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .filter(|value| *value <= u64::from(u32::MAX))
                .map(|value| value as u32)
                .unwrap_or(default)
        };
        Some(Options {
            overlay_color: color("render.block-overlay.overlay-color", 0x5fff_0000),
            outline_color: color("render.block-overlay.outline-color", 0xc8ff_0000),
            ..Options::default()
        })
    })
}

pub(crate) fn fishing_line(
    game: &Game<'_>,
    alpha: f64,
    size: [u32; 2],
) -> ControlFlow<Vec<Line>> {
    let Some(options) = options(game) else {
        return ControlFlow::Continue(());
    };
    let own = game
        .hook_position(alpha)
        .zip(game.rod_hand_position(alpha, size));
    let width = horizon_client::gfx::outline_width(size[0]);
    let mut lines = Vec::new();
    for (hook, hand) in own.into_iter().chain(game.remote_hook_ends(alpha)) {
        let offset = (hand - (hook + glam::DVec3::new(0.0, 0.25, 0.0))).as_vec3();
        let point = |a: f32| {
            let y = offset.y * (a * a + a) * 0.5 + 0.25;
            hook + glam::Vec3::new(offset.x * a, y, offset.z * a).as_dvec3()
        };
        lines.extend((0..16).map(|i| Line {
            start: point(i as f32 / 16.0).to_array(),
            end: point((i + 1) as f32 / 16.0).to_array(),
            color: 0xFF00_0000,
            width,
        }));
    }
    if options.outline_color >> 24 != 0 {
        if let Some(hit) = game.pick() {
            let mut overlay = Gizmos::default();
            for bounds in game.outline_boxes(hit.pos) {
                let (min, max) = expand(bounds);
                overlay.cuboid(
                    min,
                    max,
                    options.outline_color,
                    f32::from(options.line_width),
                );
            }
            lines.extend(overlay.lines);
        }
    }
    ControlFlow::Break(lines)
}
