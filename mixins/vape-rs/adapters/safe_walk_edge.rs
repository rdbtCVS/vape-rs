fn safe_walk_edge(world: &crate::motion::MotionWorld<'_, '_, '_>) -> core::ops::ControlFlow<Result<bool>> {
    if world.player.vape_rs_safe_walk() {
        core::ops::ControlFlow::Break(Ok(true))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
