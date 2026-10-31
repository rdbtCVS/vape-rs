fn auto_totem_break(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    // AutoTotem's onSendClickBlock: no block breaking (or its reset) while a
    // silent open holds input.
    if crate::game::remote::vape_transactions::input_held(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
