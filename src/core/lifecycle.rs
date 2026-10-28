/// A generation is local to one client instance and never serialized in profiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Epoch(u64);
impl Epoch {
    pub fn value(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupReason {
    OpenWorld,
    CloseWorld,
    Dimension,
    Respawn,
    Connect,
    Disconnect,
    WorldReady,
    Death,
    Profile,
    Panic,
}
impl CleanupReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenWorld => "world-open",
            Self::CloseWorld => "world-close",
            Self::Dimension => "dimension",
            Self::Respawn => "respawn",
            Self::Connect => "connect",
            Self::Disconnect => "disconnect",
            Self::WorldReady => "world-ready",
            Self::Death => "death",
            Self::Profile => "profile",
            Self::Panic => "panic",
        }
    }
    pub fn closes_gui(self) -> bool {
        self != Self::Profile
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityHandle {
    pub epoch: Epoch,
    pub id: i32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Context {
    pub in_world: bool,
    pub alive: bool,
    pub interactive: bool,
}
#[derive(Debug)]
pub struct Session {
    epoch: Epoch,
    context: Context,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            epoch: Epoch(0),
            context: Context::default(),
        }
    }
}
impl Session {
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }
    pub fn interactive(&self) -> bool {
        self.context.in_world && self.context.alive && self.context.interactive
    }
    pub fn advance(&mut self) -> Epoch {
        self.epoch.0 = self
            .epoch
            .0
            .checked_add(1)
            .expect("session generation exhausted");
        self.epoch
    }
    pub fn observe(&mut self, next: Context) -> Option<CleanupReason> {
        let previous = self.context;
        self.context = next;
        if previous.in_world && !next.in_world {
            Some(CleanupReason::CloseWorld)
        } else if !previous.in_world && next.in_world {
            Some(CleanupReason::WorldReady)
        } else if previous.alive && !next.alive && next.in_world {
            Some(CleanupReason::Death)
        } else {
            None
        }
    }
    pub fn owns(&self, handle: EntityHandle) -> bool {
        self.context.in_world && self.context.alive && self.epoch == handle.epoch
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generations_invalidate_reused_entity_ids_and_context_does_not_churn() {
        let mut session = Session::default();
        let playing = Context {
            in_world: true,
            alive: true,
            interactive: true,
        };
        assert_eq!(session.observe(playing), Some(CleanupReason::WorldReady));
        session.advance();
        let old = EntityHandle {
            epoch: session.epoch(),
            id: 7,
        };
        assert!(session.owns(old));
        assert_eq!(session.observe(playing), None);
        assert_eq!(
            session.observe(Context {
                interactive: false,
                ..playing
            }),
            None
        );
        assert!(!session.interactive());
        session.advance();
        assert!(!session.owns(old));
        let next = EntityHandle {
            epoch: session.epoch(),
            id: 7,
        };
        assert!(session.owns(next));
        assert_eq!(
            session.observe(Context {
                alive: false,
                ..playing
            }),
            Some(CleanupReason::Death)
        );
        assert!(!session.owns(next));
        assert_eq!(
            session.observe(Context::default()),
            Some(CleanupReason::CloseWorld)
        );
    }
}
