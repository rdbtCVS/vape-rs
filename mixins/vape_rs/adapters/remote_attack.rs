fn remote_attack(game: &mut crate::game::Game<'_>) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    // A click AutoTotem's silent open cancels is no attack.
    if crate::game::remote::vape_transactions::input_held(game) {
        return;
    }
    crate::game::vape_hud::target_attack(game);
    crate::game::vape_combat::post_attack(game);
    crate::game::vape_modern_combat::post_attack(game);
}
