fn target_info_pot_use(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    crate::game::vape_hud::target_heal_used(game, Some(stack));
}
