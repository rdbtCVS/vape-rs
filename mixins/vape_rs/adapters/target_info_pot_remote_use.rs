fn target_info_pot_remote_use(game: &mut crate::game::Game<'_>, hand: horizon_client::first_person::Hand) {
    let held = game.with_hand(hand, |g| g.held().cloned());
    crate::game::vape_hud::target_heal_used(game, held.as_ref());
}
