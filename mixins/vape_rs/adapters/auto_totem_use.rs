fn auto_totem_use(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    // AutoTotem's onRightClickMouse: no use while a silent open holds input.
    if crate::game::remote::vape_transactions::input_held(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
