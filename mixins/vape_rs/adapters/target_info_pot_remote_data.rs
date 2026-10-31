fn target_info_pot_remote_data(
    game: &crate::game::Game<'_>,
    id: i32,
    values: &[(u8, i32, horizon_protocol::entity_data::Value)],
) {
    for (index, _, value) in values {
        if let (8, horizon_protocol::entity_data::Value::Item(stack)) = (index, value) {
            crate::game::vape_hud::target_remote_potion(game, id, game.to_stack(stack));
        }
    }
}
