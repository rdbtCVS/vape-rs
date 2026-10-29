// SafeWalk's move-only edge back-off. The binary's movement writer marks the
// one local player it is about to tick; the mark lives on that thread and
// names that player's address, so a server Game, guests or mobs ticking on
// the same thread never match. It is rewritten before every client tick.
thread_local! {
    static ACTIVE: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

impl crate::motion::PlayerMotion {
    /// Mark (or unmark) this player for edge back-off on its next moves.
    pub fn vape_rs_set_safe_walk(&self, on: bool) {
        let me = self as *const Self as usize;
        ACTIVE.with(|active| {
            if on {
                active.set(me);
            } else if active.get() == me {
                active.set(0);
            }
        });
    }
    pub(crate) fn vape_rs_safe_walk(&self) -> bool {
        ACTIVE.with(|active| active.get() == self as *const Self as usize)
    }
}
