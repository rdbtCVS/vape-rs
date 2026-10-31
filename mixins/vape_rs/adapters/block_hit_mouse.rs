fn block_hit_mouse(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
    pressed: bool,
) -> core::ops::ControlFlow<Result<()>> {
    match crate::game::vape_block_hit::mouse(game, button, pressed) {
        core::ops::ControlFlow::Break(()) => core::ops::ControlFlow::Break(Ok(())),
        core::ops::ControlFlow::Continue(()) => core::ops::ControlFlow::Continue(()),
    }
}
