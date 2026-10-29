fn explosion_push(game: &mut crate::game::Game<'_>, packet: &horizon_protocol::packets::play::Explode) -> core::ops::ControlFlow<()> {
    if game.serving.is_some() || game.options.dedicated.is_some() {
        return core::ops::ControlFlow::Continue(());
    }
    crate::game::vape_combat::explosion(game, packet)
}
