fn fly_move(world: &mut crate::motion::MotionWorld<'_, '_, '_>) -> core::ops::ControlFlow<Result<()>> {
    match world.player.vape_rs_take_fly_move() {
        Some(movement) => core::ops::ControlFlow::Break(world.move_self(movement)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
