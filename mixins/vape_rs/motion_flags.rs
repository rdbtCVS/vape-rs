// NoSlowdown and Fly marks on the one local player the client tick is about
// to move. Like SafeWalk's mark they live on the client thread and name that
// player's address, so the integrated server, guests and mobs never match;
// the client tick rewrites them before every move.
thread_local! {
    static NO_SLOWDOWN: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static FLY: core::cell::Cell<(usize, [f64; 3])> = const { core::cell::Cell::new((0, [0.0; 3])) };
}

impl crate::motion::PlayerMotion {
    /// Mark (or unmark) this player to move at full speed while using an item.
    pub fn vape_rs_set_no_slowdown(&self, on: bool) {
        let me = self as *const Self as usize;
        NO_SLOWDOWN.with(|cell| {
            if on {
                cell.set(me);
            } else if cell.get() == me {
                cell.set(0);
            }
        });
    }
    pub(crate) fn vape_rs_no_slowdown(&self) -> bool {
        NO_SLOWDOWN.with(|cell| cell.get() == self as *const Self as usize)
    }
    /// Fly's (or Speed's / LongJump's) replacement for this player's next
    /// self-move (`EventPreMove`); a NaN component keeps the host's.
    pub fn vape_rs_set_fly_move(&self, movement: Option<[f64; 3]>) {
        let me = self as *const Self as usize;
        FLY.with(|cell| match movement {
            Some(m) => cell.set((me, m)),
            None if cell.get().0 == me => cell.set((0, [0.0; 3])),
            None => {}
        });
    }
    /// Takes the replacement once, so the re-dispatched move runs unchanged.
    pub(crate) fn vape_rs_take_fly_move(&self, host: horizon_core::geometry::Vec3) -> Option<horizon_core::geometry::Vec3> {
        FLY.with(|cell| {
            let (who, m) = cell.get();
            (who == self as *const Self as usize).then(|| {
                cell.set((0, [0.0; 3]));
                let pick = |v: f64, h: f64| if v.is_nan() { h } else { v };
                horizon_core::geometry::Vec3::new(pick(m[0], host.x), pick(m[1], host.y), pick(m[2], host.z))
            })
        })
    }
}
