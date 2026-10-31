fn auto_totem_attack(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    // AutoTotem's onClickMouse: no attack while a silent open holds input.
    if crate::game::remote::vape_transactions::input_held(game) {
        return core::ops::ControlFlow::Break(false);
    }
    core::ops::ControlFlow::Continue(())
}
