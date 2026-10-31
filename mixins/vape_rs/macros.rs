// Runs inside game::remote so use presses join the host's normal input queue.
use super::super::{Game, Screen};
use crate::game::vape_rs::{core::with_client, input::macros::{MacroContext, MacroEffect}};

pub fn context(game: &Game<'_>) -> MacroContext {
    let hotbar = game.inventory.iter().take(9);
    MacroContext {
        hotbar: hotbar.clone().map(|stack| stack.as_ref().map(|s| s.item.to_string())).collect(),
        // `findHotbarSlot`'s display names: the hover name (`getHoverName`) and
        // the item's own name, formatting codes removed.
        hotbar_names: hotbar.map(|stack| stack.as_ref().map_or_else(Vec::new, |s| {
            let item = game.pack.names.display_item(&s.item);
            let hover = s.custom_name().or_else(|| s.item_name()).unwrap_or_else(|| item.clone());
            vec![plain(&hover), plain(&item)]
        })).collect(),
        selected_slot: u8::try_from(game.selected).ok().filter(|s| *s < 9),
        fishing_rod_ready: crate::game::fishing::vape_auto_fish::macro_rod_ready(game),
        use_held: game.focused && game.toggles.is_down(3),
        accepts_input: game.focused && game.player.as_ref().is_some_and(|p| p.health.health() > 0.0)
            && game.screen == Screen::Playing && game.title.is_none()
            && game.sleeping.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none(),
    }
}
// `StringUtils.l`: drop `§x` formatting pairs.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}
// `advance` is the client tick (the reference's pre-tick); frames only clean up.
pub fn frame(game: &mut Game<'_>, advance: bool) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let context = context(game);
    let allowed = context.accepts_input;
    let effects = with_client(&client, |r| r.macro_frame(context, advance));
    for effect in effects {
        match effect {
            MacroEffect::Chat { text } if allowed => game.handle_chat_input(&text),
            MacroEffect::Command { command } if allowed => game.handle_chat_input(&format!("/{command}")),
            MacroEffect::SelectHotbar { slot } if allowed && slot < 9 => game.selected = usize::from(slot),
            MacroEffect::RestoreHotbar { slot } if slot < 9 && game.title.is_none() && game.player.as_ref().is_some_and(|p| p.health.health() > 0.0) => game.selected = usize::from(slot),
            MacroEffect::UseItem { down: true } if allowed => {
                game.right = true;
                use_once(game);
            }
            MacroEffect::UseItem { down: false } => game.right = game.focused && game.toggles.is_down(3),
            _ => {}
        }
    }
}

// Ordinary use activation shared by macros and fishing automation. Caller owns
// a use-input claim; this never re-enters the physical binding hook.
pub fn use_once(game: &mut Game<'_>) {
    if let Some(remote) = game.remote.as_mut() {
        remote.input.press(horizon_client::interaction_input::Button::Use);
    } else if game.using.is_none() {
        if let Err(error) = game.use_with_hand() {
            with_client(&game.__horizon_mixins_vape_rs_client(), |r| r.notify(format!("Automatic item use failed: {error}")));
        }
        game.use_delay = 4;
    }
}

// One attack click (`KeyMapping.click` on key.attack) through the host's
// normal input path: the remote interaction queue, or the integrated tick's
// attack clicks. Never re-enters the physical binding hook.
pub fn attack_once(game: &mut Game<'_>) {
    if let Some(remote) = game.remote.as_mut() {
        remote.input.press(horizon_client::interaction_input::Button::Attack);
    } else {
        game.attack_clicks += 1;
    }
}
