// Time Changer and Weather (reference onPreRenderTick): the client world's
// time and weather strength are rewritten every frame while enabled. Weather
// restores the strengths it saved on enable, like the reference. The modern
// server only sends the world clock when it jumps, so the reference's "next
// time packet takes over" never comes: Time Changer keeps the real clock
// running in the background and puts it back on disable.
use crate::game::vape_rs::core::with_client;

thread_local! {
    /// The weather strengths saved when Weather took over: (rain, thunder).
    static SAVED: core::cell::Cell<Option<((f32, f32), (f32, f32))>> = const { core::cell::Cell::new(None) };
    /// Time Changer's (real day time, day time it last wrote).
    static CLOCK: core::cell::Cell<Option<(i64, i64)>> = const { core::cell::Cell::new(None) };
}

/// The real clock after the client moved `written` on to `now`: its own
/// advance (at most a few ticks per frame) carries on, anything larger is a
/// server clock update that replaces it.
fn real_clock(real: i64, written: i64, now: i64) -> i64 {
    match now - written {
        d @ 0..=40 => real + d,
        _ => now,
    }
}

pub(crate) fn frame(game: &mut crate::game::Game<'_>) {
    if game.player.is_none() {
        SAVED.with(|s| s.set(None));
        CLOCK.with(|c| c.set(None));
        return;
    }
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (time, weather) = with_client(&client, |runtime| (runtime.time_override(), runtime.weather_override()));
    let now = game.world.day_time;
    let last = CLOCK.with(|c| c.get());
    match time {
        Some(time) => {
            let real = last.map_or(now, |(real, written)| real_clock(real, written, now));
            // The day count stays, so moon phases and saved time are untouched.
            let written = real.div_euclid(24000) * 24000 + time;
            game.world.day_time = written;
            CLOCK.with(|c| c.set(Some((real, written))));
        }
        None => {
            if let Some((real, written)) = last {
                game.world.day_time = real_clock(real, written, now);
                CLOCK.with(|c| c.set(None));
            }
        }
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
        if game.potions.effects.get(id).is_some() {
            game.potions.effects.list.retain(|e| e.id != id);
        }
        // Every tick, not only with the effect: the server's attribute sync
        // follows its effect packet and puts the modifier back after the
        // effect has left the list.
        if let Some((attribute, modifier, _, operation)) = horizon_client::effects::effect_modifier(id)
            && let Some(player) = game.player.as_mut()
        {
            let _ = player.set_modifier(attribute, modifier, 0.0, operation, false);
        }
    }
}
