// Time Changer and Weather (reference onPreRenderTick): the client world's
// time and weather strength are rewritten every frame while enabled. The
// server's next time packet and the weather tick take over again once off;
// Weather restores the strengths it saved on enable, like the reference.
use crate::game::vape_rs::core::with_client;

thread_local! {
    /// The weather strengths saved when Weather took over: (rain, thunder).
    static SAVED: core::cell::Cell<Option<((f32, f32), (f32, f32))>> = const { core::cell::Cell::new(None) };
}

pub(crate) fn frame(game: &mut crate::game::Game<'_>) {
    if game.player.is_none() {
        SAVED.with(|s| s.set(None));
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (time, weather) = with_client(&client, |runtime| (runtime.time_override(), runtime.weather_override()));
    if let Some(time) = time {
        // The day count stays, so moon phases and saved time are untouched.
        game.world.day_time = game.world.day_time.div_euclid(24000) * 24000 + time;
    }
    let w = &mut game.world.weather;
    match weather {
        Some((rain, thunder)) => {
            SAVED.with(|s| {
                if s.get().is_none() {
                    s.set(Some((w.rain_level, w.thunder_level)));
                }
            });
            w.rain_level = (rain, rain);
            if let Some(thunder) = thunder {
                w.thunder_level = (thunder, thunder);
            }
        }
        None => {
            if let Some((rain, thunder)) = SAVED.with(|s| s.take()) {
                w.rain_level = rain;
                w.thunder_level = thunder;
            }
        }
    }
}

/// AntiDebuff (reference `onTick`): the chosen effects leave the client's
/// effect list, and slowness its movement-speed modifier.
pub(crate) fn anti_debuff(game: &mut crate::game::Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let removed = with_client(&client, |runtime| runtime.anti_debuff());
    for id in removed {
        if game.potions.effects.get(id).is_none() {
            continue;
        }
        game.potions.effects.list.retain(|e| e.id != id);
        if let Some((attribute, modifier, _, operation)) = horizon_client::effects::effect_modifier(id)
            && let Some(player) = game.player.as_mut()
        {
            let _ = player.set_modifier(attribute, modifier, 0.0, operation, false);
        }
    }
}
