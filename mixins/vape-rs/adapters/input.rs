fn input(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
    pressed: bool,
    repeat: bool,
) {
    use crate::game::vape_rs::{
        core::{ModuleId, with_client},
        input::KeyAction,
    };
    use winit::keyboard::KeyCode;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let accepts_input = game.focused
        && game.player.is_some()
        && game.screen == crate::game::Screen::Playing
        && game.title.is_none();
    let action = if !pressed {
        KeyAction::Release
    } else if repeat {
        KeyAction::Repeat
    } else {
        KeyAction::Press
    };
    with_client(|runtime| match key {
        KeyCode::F8 => runtime.key(ModuleId::Coordinates, action, accepts_input),
        KeyCode::F9 => runtime.key(ModuleId::Fps, action, accepts_input),
        KeyCode::F10 if accepts_input && action == KeyAction::Press => runtime.panic(),
        _ => {}
    });
}
