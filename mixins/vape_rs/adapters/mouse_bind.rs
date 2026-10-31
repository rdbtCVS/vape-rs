fn mouse_bind(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
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
    let macro_context = crate::game::remote::vape_macros::context(game);
    let suppressed = with_client(&client, |runtime| {
        runtime.macro_context(macro_context);
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
    // TriggerBot's "Ignore activation click" swallows the first manual press.
    let swallowed = button == MouseButton::Left
        && !suppressed
        && crate::game::vape_combat::trigger_bot_input(game, pressed)
        && pressed;
    // HitSwap, AutoMace and ShieldBreaker see the press, swallowed or not,
    // and may cancel it.
    let cancelled = button == MouseButton::Left
        && pressed
        && !suppressed
        && crate::game::vape_modern_combat::press(game, swallowed);
    if suppressed || swallowed || cancelled {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
