// Sprint (reference `Sprint.onPreEntityUpdate`): presses the sprint key. The
// press is a latch like `KeyBinding.setKeyBindState(sprint, true)`: it is made
// only while the key is up, the player is not sprinting and not against a
// wall, and then stays down until the module is disabled, a screen releases
// every key, or the user's own sprint key goes up. Cancel Invis (and a held
// movement-input claim) only skips new presses; a latched key stays down.

/// One client tick of host state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    /// In the world with no screen (the reference's `currentScreen` check).
    pub playing: bool,
    /// The user's own sprint key or toggle, without our write.
    pub user_down: bool,
    pub sprinting: bool,
    /// `collidedHorizontally` this tick (minor collisions included).
    pub collided_horizontally: bool,
    /// The player has the Invisibility effect.
    pub invisible: bool,
    /// `capabilities.allowFlying` (creative or spectator abilities).
    pub may_fly: bool,
    /// `SharedModuleControlClaims.movementInput` is claimed.
    pub movement_locked: bool,
}

#[derive(Debug, Default)]
pub struct Sprint {
    latched: bool,
    user_down: bool,
}

impl Sprint {
    /// `onDisable`: the key goes up.
    pub fn reset(&mut self) {
        self.latched = false;
    }

    pub fn latched(&self) -> bool {
        self.latched
    }

    /// Whether the synthetic sprint press is held this tick.
    pub fn tick(&mut self, cancel_invis: bool, tick: &Tick) -> bool {
        // A key event on the sprint key sets its state; its release ends the
        // latch like the host's own key-up does.
        if self.user_down && !tick.user_down {
            self.latched = false;
        }
        self.user_down = tick.user_down;
        if !tick.playing {
            // Screens release every key (`KeyMapping.releaseAll`).
            self.latched = false;
            return false;
        }
        if tick.movement_locked || (cancel_invis && tick.invisible && !tick.may_fly) {
            return self.latched;
        }
        if !self.latched && !tick.user_down && !tick.sprinting && !tick.collided_horizontally {
            self.latched = true;
        }
        self.latched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walking() -> Tick {
        Tick {
            playing: true,
            ..Tick::default()
        }
    }

    #[test]
    fn presses_once_not_sprinting_and_keeps_the_latch() {
        let mut s = Sprint::default();
        assert!(s.tick(false, &walking()));
        // Sprinting, then into a wall: the latched key stays down.
        let mut t = walking();
        t.sprinting = true;
        assert!(s.tick(false, &t));
        t.collided_horizontally = true;
        assert!(s.tick(false, &t));
        s.reset();
        assert!(!s.tick(false, &t));
    }

    #[test]
    fn no_new_press_while_sprinting_or_against_a_wall() {
        let mut s = Sprint::default();
        let mut t = walking();
        t.sprinting = true;
        assert!(!s.tick(false, &t));
        t.sprinting = false;
        t.collided_horizontally = true;
        assert!(!s.tick(false, &t));
        t.collided_horizontally = false;
        assert!(s.tick(false, &t));
    }

    #[test]
    fn cancel_invis_skips_new_presses_unless_the_player_may_fly() {
        let mut s = Sprint::default();
        let mut t = walking();
        t.invisible = true;
        assert!(!s.tick(true, &t));
        assert!(s.tick(false, &t));
        // Already latched: invisibility does not release it.
        assert!(s.tick(true, &t));
        let mut s = Sprint::default();
        t.may_fly = true;
        assert!(s.tick(true, &t));
        let mut s = Sprint::default();
        t.may_fly = false;
        t.movement_locked = true;
        assert!(!s.tick(false, &t));
    }

    #[test]
    fn screens_and_the_users_key_release_end_the_latch() {
        let mut s = Sprint::default();
        assert!(s.tick(false, &walking()));
        assert!(!s.tick(false, &Tick::default()));
        let mut t = walking();
        t.sprinting = true;
        // Back from the screen while still sprinting: no new press.
        assert!(!s.tick(false, &t));
        // The user holds their own key: no synthetic press over it.
        t.sprinting = false;
        t.user_down = true;
        assert!(!s.tick(false, &t));
        // Their key goes up while sprinting continues: no press yet.
        t.user_down = false;
        t.sprinting = true;
        assert!(!s.tick(false, &t));
        t.sprinting = false;
        assert!(s.tick(false, &t));
        // A press and release of the user's key over the latch ends it.
        t.user_down = true;
        assert!(s.tick(false, &t));
        t.user_down = false;
        t.sprinting = true;
        assert!(!s.tick(false, &t));
    }
}
