// Runs inside game::remote so use presses join the host's normal input queue.
use super::super::{Game, Screen};
use crate::game::vape_rs::{core::with_client, input::macros::{MacroContext, MacroEffect}};

pub fn context(game: &Game<'_>) -> MacroContext {
    MacroContext {
        hotbar: game.inventory.iter().take(9).map(|stack| stack.as_ref().map(|s| {
            s.item.rsplit(':').next().unwrap_or(&s.item).replace('_', " ")
        })).collect(),
        selected_slot: u8::try_from(game.selected).ok().filter(|s| *s < 9),
        fishing_rod_ready: false,
        accepts_input: game.focused && game.player.as_ref().is_some_and(|p| p.health.health() > 0.0)
            && game.screen == Screen::Playing && game.title.is_none()
            && game.sleeping.is_none() && !game.dialog_open() && game.server_packs.prompt.is_none(),
    }
}
pub fn frame(game: &mut Game<'_>) {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let context = context(game);
    let allowed = context.accepts_input;
    let effects = with_client(&client, |r| r.macro_frame(context));
    for effect in effects {
        match effect {
            MacroEffect::Chat { text } if allowed => game.handle_chat_input(&text),
            MacroEffect::Command { command } if allowed => game.handle_chat_input(&format!("/{command}")),
            MacroEffect::SelectHotbar { slot } if allowed && slot < 9 => game.selected = usize::from(slot),
            MacroEffect::UseItem { down: true } if allowed => {
                game.right = true;
                if let Some(remote) = game.remote.as_mut() {
                    remote.input.press(horizon_client::interaction_input::Button::Use);
                } else if game.using.is_none() {
                    if let Err(error) = game.use_with_hand() {
                        with_client(&client, |r| r.notify(format!("Macro use failed: {error}")));
                    }
                    game.use_delay = 4;
                }
            }
            MacroEffect::UseItem { down: false } => game.right = game.focused && game.toggles.is_down(3),
            _ => {}
        }
    }
}
