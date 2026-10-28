fn mouse_bind(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    use crate::game::vape_rs::{core::with_client, input::KeyAction};
    use winit::event::MouseButton;
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    let name = match button {
        MouseButton::Left => "MouseLeft".into(),
        MouseButton::Right => "MouseRight".into(),
        MouseButton::Middle => "MouseMiddle".into(),
        MouseButton::Back => "MouseBack".into(),
        MouseButton::Forward => "MouseForward".into(),
        MouseButton::Other(n) => format!("Mouse{n}"),
    };
    let suppressed = with_client(|runtime| {
        runtime.input(
            &name,
            if pressed {
                KeyAction::Press
            } else {
                KeyAction::Release
            },
            game.focused && game.screen == crate::game::Screen::Playing,
        );
        runtime.suppress_game_input()
    });
    if suppressed {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
