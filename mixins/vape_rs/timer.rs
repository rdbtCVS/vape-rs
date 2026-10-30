// Timer's game-timer speed for the one client clock that set it. The frame
// adapter rewrites the speed every frame (1.0 when Timer is off); the clock's
// own advance then runs once on the scaled elapsed time.
thread_local! {
    static SPEED: core::cell::Cell<(usize, f32)> = const { core::cell::Cell::new((0, 1.0)) };
    static REENTRY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

impl crate::clock::FixedStep {
    pub fn vape_rs_set_timer_speed(&self, speed: f32) {
        let me = self as *const Self as usize;
        SPEED.with(|cell| {
            if speed != 1.0 {
                cell.set((me, speed));
            } else if cell.get().0 == me {
                cell.set((0, 1.0));
            }
        });
    }
    /// The speed to scale this advance by, unless this is the scaled re-run.
    pub(crate) fn vape_rs_timer_speed(&self) -> Option<f32> {
        if REENTRY.with(core::cell::Cell::get) {
            return None;
        }
        SPEED.with(|cell| {
            let (who, speed) = cell.get();
            (who == self as *const Self as usize && speed != 1.0).then_some(speed)
        })
    }
    pub(crate) fn vape_rs_advance_scaled(&mut self, elapsed: std::time::Duration, speed: f32) -> u32 {
        REENTRY.with(|r| r.set(true));
        let ticks = self.advance(elapsed.mul_f32(speed.max(0.0)));
        REENTRY.with(|r| r.set(false));
        ticks
    }
}
