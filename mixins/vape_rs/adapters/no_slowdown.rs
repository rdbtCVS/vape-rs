fn no_slowdown(world: &mut crate::motion::MotionWorld<'_, '_, '_>) {
    if world.player.vape_rs_no_slowdown() {
        world.player.item_use_speed = None;
    }
}
