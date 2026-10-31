fn auto_tool_attack(game: &mut crate::game::Game<'_>) {
    crate::game::vape_inventory::auto_tool(game, true);
    if game.serving.is_none() && game.options.dedicated.is_none() {
        crate::game::vape_hud::target_attack(game);
        crate::game::vape_combat::post_attack(game);
        crate::game::vape_modern_combat::post_attack(game);
    }
}
