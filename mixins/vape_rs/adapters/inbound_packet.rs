fn inbound_packet(game: &mut crate::game::Game<'_>, packet: &horizon_protocol::packets::play::Clientbound) {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return;
    }
    crate::game::vape_combat::inbound(game, packet);
}
