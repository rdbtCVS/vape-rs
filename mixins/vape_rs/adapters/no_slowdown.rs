fn no_slowdown(world: &mut crate::motion::MotionWorld<'_, '_, '_>) {
    if world.player.vape_rs_no_slowdown() {
        world.player.item_use_speed = None;
        // NoSlowdown with Sprint on keeps sprinting while using the item.
        if world.player.vape_rs_no_slowdown_sprint() {
            world.player.item_use_slows_sprint = false;
        }
    }
}
