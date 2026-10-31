fn pearl_use_item(game: &crate::game::Game<'_>, hand: horizon_client::first_person::Hand) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    let stack = match hand {
        horizon_client::first_person::Hand::Main => game.inventory.get(game.selected).and_then(|s| s.as_ref()),
        horizon_client::first_person::Hand::Off => game.offhand.as_ref(),
    };
    if let Some(stack) = stack {
        crate::game::remote::vape_pearls::used(game, &stack.item);
    }
}
