fn remote_attack(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::vape_combat::post_attack(game);
}
