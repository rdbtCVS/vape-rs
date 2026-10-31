fn pearl_throw_item(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    // A LAN guest's turn throws as the guest, not as this client's player.
    if game.serving.is_some() || game.options.dedicated.is_some() || game.lan_acting.is_some() {
        return;
    }
    crate::game::remote::vape_pearls::used(game, &stack.item);
}
