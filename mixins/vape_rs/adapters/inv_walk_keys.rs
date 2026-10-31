fn inv_walk_keys(world: &mut crate::motion::MotionWorld<'_, '_, '_>, keys: crate::sprint::Keys, sprint_window: i32) -> core::ops::ControlFlow<Result<()>> {
    // InvWalk: the movement keys physically held behind a screen join the
    // (screen-gated, so released) keys of this controlled tick.
    match world.player.vape_rs_take_inv_walk() {
        Some(held) => {
            let merged = crate::sprint::Keys {
                forward: keys.forward || held.forward,
                backward: keys.backward || held.backward,
                left: keys.left || held.left,
                right: keys.right || held.right,
                jump: keys.jump || held.jump,
                shift: keys.shift || held.shift,
                ..keys
            };
            core::ops::ControlFlow::Break(world.controlled_simulation_tick_with_sprint_window(merged, sprint_window))
        }
        None => core::ops::ControlFlow::Continue(()),
    }
}
