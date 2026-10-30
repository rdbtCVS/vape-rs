fn timer_advance(clock: &mut crate::clock::FixedStep, elapsed: std::time::Duration) -> core::ops::ControlFlow<u32> {
    match clock.vape_rs_timer_speed() {
        Some(speed) => core::ops::ControlFlow::Break(clock.vape_rs_advance_scaled(elapsed, speed)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
