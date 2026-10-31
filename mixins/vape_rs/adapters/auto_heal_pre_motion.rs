fn auto_heal_pre_motion(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::remote::vape_transactions::auto_heal_pre_motion(game);
}
