use super::core::lifecycle::Epoch;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotbarSlot(u8);
impl HotbarSlot {
    pub fn new(slot: usize) -> Option<Self> {
        (slot < 9).then_some(Self(slot as u8))
    }
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Owner(pub &'static str);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Resource {
    Hotbar,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Lease {
    resource: Resource,
    owner: Owner,
    epoch: Epoch,
    priority: u16,
    serial: u64,
}
#[derive(Debug, Default)]
pub struct Coordinator {
    claims: BTreeMap<Resource, Lease>,
    serial: u64,
}
impl Coordinator {
    pub fn acquire(
        &mut self,
        resource: Resource,
        owner: Owner,
        epoch: Epoch,
        priority: u16,
    ) -> Option<Lease> {
        if let Some(existing) = self.claims.get(&resource)
            && existing.epoch == epoch
        {
            if existing.owner == owner {
                return Some(*existing);
            }
            if existing.priority >= priority {
                return None;
            }
        }
        self.serial = self
            .serial
            .checked_add(1)
            .expect("action lease serial exhausted");
        let lease = Lease {
            resource,
            owner,
            epoch,
            priority,
            serial: self.serial,
        };
        self.claims.insert(resource, lease);
        Some(lease)
    }
    pub fn owns(&self, lease: Lease, epoch: Epoch) -> bool {
        lease.epoch == epoch && self.claims.get(&lease.resource) == Some(&lease)
    }
    pub fn release(&mut self, lease: Lease) {
        if self.claims.get(&lease.resource) == Some(&lease) {
            self.claims.remove(&lease.resource);
        }
    }
    pub fn release_owner(&mut self, owner: Owner) {
        self.claims.retain(|_, lease| lease.owner != owner);
    }
    pub fn clear(&mut self) {
        self.claims.clear();
    }
    pub fn authorize(&self, action: &SlotChange, epoch: Epoch, current: HotbarSlot) -> bool {
        action.expected == current && self.owns(action.lease, epoch)
    }
}
#[derive(Clone, Debug)]
pub struct SlotChange {
    pub lease: Lease,
    pub expected: HotbarSlot,
    pub selected: HotbarSlot,
    pub item: Option<String>,
}
#[cfg(test)]
mod tests {
    use super::super::core::lifecycle::Session;
    use super::*;
    #[test]
    fn competing_owners_preemption_and_stale_release_do_not_steal_claims() {
        let mut c = Coordinator::default();
        let epoch = Session::default().epoch();
        let first = c
            .acquire(Resource::Hotbar, Owner("first"), epoch, 10)
            .unwrap();
        assert!(
            c.acquire(Resource::Hotbar, Owner("second"), epoch, 10)
                .is_none()
        );
        let second = c
            .acquire(Resource::Hotbar, Owner("second"), epoch, 20)
            .unwrap();
        c.release(first);
        assert!(c.owns(second, epoch));
        assert!(!c.owns(first, epoch));
        c.clear();
        assert!(!c.owns(second, epoch));
    }
    #[test]
    fn selection_requires_current_generation_and_unchanged_user_selection() {
        let mut c = Coordinator::default();
        let mut session = Session::default();
        let lease = c
            .acquire(Resource::Hotbar, Owner("tool"), session.epoch(), 10)
            .unwrap();
        let action = SlotChange {
            lease,
            expected: HotbarSlot::new(0).unwrap(),
            selected: HotbarSlot::new(8).unwrap(),
            item: Some("minecraft:diamond_pickaxe".into()),
        };
        assert!(c.authorize(&action, session.epoch(), HotbarSlot::new(0).unwrap()));
        assert!(!c.authorize(&action, session.epoch(), HotbarSlot::new(1).unwrap()));
        session.advance();
        assert!(!c.authorize(&action, session.epoch(), action.expected));
        assert!(HotbarSlot::new(9).is_none());
        assert!(HotbarSlot::new(usize::MAX).is_none());
    }
}
