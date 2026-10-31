fn block_hit_send(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Serverbound,
) -> core::ops::ControlFlow<()> {
    crate::game::vape_block_hit::send(game, packet)
}
