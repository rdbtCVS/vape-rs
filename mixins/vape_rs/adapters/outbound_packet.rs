fn outbound_packet(
    game: &crate::game::Game<'_>,
    packet: &horizon_protocol::packets::play::Serverbound,
) -> core::ops::ControlFlow<()> {
    crate::game::remote::vape_packets::send(game, packet)
}
