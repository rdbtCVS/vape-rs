// Movement/world marks on the one local player the client tick is about to
// move (InvWalk's physical keys, NoSlowdown's sprint), and LongJump's surface
// query. Like the other marks they live on the client thread and name that
// player's address, so the integrated server's players never match.
thread_local! {
    static INV_WALK: core::cell::Cell<(usize, Option<crate::sprint::Keys>)> = const { core::cell::Cell::new((0, None)) };
    static SPRINT: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

impl crate::motion::PlayerMotion {
    /// InvWalk: the keys physically held behind a screen for this player's
    /// next controlled tick (None clears it).
    pub fn vape_rs_set_inv_walk(&self, keys: Option<crate::sprint::Keys>) {
        let me = self as *const Self as usize;
        INV_WALK.with(|cell| match keys {
            Some(_) => cell.set((me, keys)),
            None if cell.get().0 == me => cell.set((0, None)),
            None => {}
        });
    }
    /// Taken once, so the re-dispatched tick runs with the merged keys.
    pub(crate) fn vape_rs_take_inv_walk(&self) -> Option<crate::sprint::Keys> {
        INV_WALK.with(|cell| {
            let (who, keys) = cell.get();
            (who == self as *const Self as usize).then(|| {
                cell.set((0, None));
                keys
            })?
        })
    }
    /// NoSlowdown with Sprint on: the used item does not stop the sprint.
    pub fn vape_rs_set_no_slowdown_sprint(&self, on: bool) {
        let me = self as *const Self as usize;
        SPRINT.with(|cell| {
            if on {
                cell.set(me);
            } else if cell.get() == me {
                cell.set(0);
            }
        });
    }
    pub(crate) fn vape_rs_no_slowdown_sprint(&self) -> bool {
        SPRINT.with(|cell| cell.get() == self as *const Self as usize)
    }
}

impl crate::motion::MotionWorld<'_, '_, '_> {
    /// LongJump's `getBaseMoveSpeed` surfaces: (climbing a vine, standing on
    /// a block at least as slippery as ice 0.1 below the feet).
    pub fn vape_rs_surface(&mut self) -> (bool, bool) {
        let p = self.player.position;
        let feet = horizon_core::position::BlockPos::containing(p);
        let name = |world: &Self, at| {
            world
                .assets
                .registry
                .state(world.block(at))
                .map(|s| s.name.clone())
                .unwrap_or_default()
        };
        let vine = self.climbable().unwrap_or(false) && name(self, feet).trim_start_matches("minecraft:") == "vine";
        let below = horizon_core::position::BlockPos::containing(horizon_core::geometry::Vec3::new(p.x, p.y - 0.1, p.z));
        let ice = self
            .resources
            .blocks
            .get(self.block(below))
            .is_ok_and(|b| b.coefficients.friction >= 0.98);
        (vine, ice)
    }
}
