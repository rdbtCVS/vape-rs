// Shared action claims for native automation producers.
use super::super::{
    actions::{Owner, Resource},
    inventory::types::{Request, Snapshot},
    modules::utility::auto_fish::{Context, Effect, Options, Rod},
};
use super::{ModuleId, Runtime};

impl Runtime {
    pub fn use_claimed(&self) -> bool {
        self.actions
            .claimed(Resource::UseItem, self.session.epoch())
    }
    pub fn inventory_busy(&self) -> bool {
        self.actions
            .claimed(Resource::Container, self.session.epoch())
    }
    pub fn auto_fish_refilling(&self) -> bool {
        self.auto_fish.refilling()
    }
    pub fn cancel_auto_fish(&mut self) {
        self.auto_fish.cancel();
        self.auto_fish_pending = None;
        self.actions.release_owner(Owner("utility.auto-fish"));
    }
    pub fn auto_fish_queued(&mut self, request: Request) {
        self.auto_fish_pending = Some(request);
    }
    pub fn auto_fish_step(
        &mut self,
        context: Context,
        options: Options,
        snapshot: Option<&Snapshot>,
    ) -> Option<Effect> {
        if self.auto_fish_pending.is_some() && !context.inventory_open {
            self.cancel_auto_fish();
            self.set_enabled(ModuleId::AutoFish, false);
            return None;
        }
        if let Some(request) = &self.auto_fish_pending {
            let Some(snapshot) = snapshot else {
                return None;
            };
            if request.session != snapshot.session || request.container_id != snapshot.container_id
            {
                self.cancel_auto_fish();
                return None;
            }
            if request.revision == snapshot.revision {
                return None;
            }
            self.auto_fish_pending = None;
        }
        let epoch = self.session.epoch();
        for resource in [Resource::Hotbar, Resource::UseItem] {
            if self
                .actions
                .acquire(resource, Owner("utility.auto-fish"), epoch, 80)
                .is_none()
            {
                self.cancel_auto_fish();
                return None;
            }
        }
        if matches!(context.rod, Rod::Inventory(_)) || self.auto_fish.refilling() {
            if self
                .actions
                .acquire(Resource::Container, Owner("utility.auto-fish"), epoch, 100)
                .is_none()
            {
                self.cancel_auto_fish();
                return None;
            }
        }
        let effect = self.auto_fish.step(context, options);
        if !self.auto_fish.refilling() {
            self.actions
                .release_resource(Owner("utility.auto-fish"), Resource::Container);
        }
        effect
    }
    pub fn observe_fish_hook(&mut self, id: u32, target: Option<i32>) {
        if let Some(target) = target {
            self.fish_hooked_entity = Some((id, target));
        } else if self.fish_hooked_entity.is_some_and(|(hook, _)| hook == id) {
            self.fish_hooked_entity = None;
        }
    }
    pub fn fish_hook_target(&self, id: u32) -> Option<i32> {
        self.fish_hooked_entity
            .filter(|(hook, _)| *hook == id)
            .map(|(_, target)| target)
    }
}
