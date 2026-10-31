fn inbound_packet(game: &mut crate::game::Game<'_>, packet: &horizon_protocol::packets::play::Clientbound) -> core::ops::ControlFlow<Result<()>> {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    if let core::ops::ControlFlow::Break(done) = crate::game::remote::vape_packets::inbound(game, packet) {
        return core::ops::ControlFlow::Break(done);
    }
    crate::game::vape_combat::inbound(game, packet);
    crate::game::vape_block_hit::inbound(game, packet);
    if let horizon_protocol::packets::play::Clientbound::BlockUpdate(update) = packet {
        let client = game.__horizon_mixins_vape_rs_client().clone();
        crate::game::vape_rs::core::with_client(&client, |runtime| runtime.prop_hunt_block_update(update.pos));
    }
    core::ops::ControlFlow::Continue(())
}
