fn inbound_packet(game: &mut crate::game::Game<'_>, packet: &horizon_protocol::packets::play::Clientbound) -> core::ops::ControlFlow<Result<()>> {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    crate::game::vape_combat::inbound(game, packet);
    core::ops::ControlFlow::Continue(())
}
