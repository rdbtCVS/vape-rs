use super::types::{Request, Snapshot};

/// One-at-a-time optimistic transaction gate. A host must acknowledge every
/// request with a new snapshot/revision before another request may be emitted.
#[derive(Debug, Default)]
pub struct Gate {
    pending: Option<Request>,
}
impl Gate {
    pub fn pending(&self) -> Option<&Request> {
        self.pending.as_ref()
    }
    pub fn clear(&mut self) {
        self.pending = None;
    }
    pub fn issue(&mut self, snapshot: &Snapshot, request: Request) -> Option<Request> {
        if self.pending.is_none()
            && request.session == snapshot.session
            && request.container_id == snapshot.container_id
            && request.revision == snapshot.revision
        {
            self.pending = Some(request.clone());
            Some(request)
        } else {
            None
        }
    }
    pub fn acknowledge(&mut self, snapshot: &Snapshot) -> bool {
        let Some(request) = &self.pending else {
            return true;
        };
        if request.session != snapshot.session || request.container_id != snapshot.container_id {
            self.clear();
            return false;
        }
        if snapshot.revision == request.revision {
            return false;
        }
        self.clear();
        true
    }
    pub fn interrupted(&mut self, snapshot: &Snapshot) -> bool {
        self.pending.as_ref().is_some_and(|r| {
            r.session != snapshot.session || r.container_id != snapshot.container_id
        }) && {
            self.clear();
            true
        }
    }
}
