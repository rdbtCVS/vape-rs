// The shared rotation for the movement and world modules (reference
// `RotationManager.setController`/`releaseController` under
// `SharedModuleControlClaims.rotation`). The claim and the silent managed
// rotation are the combat ones (`combat_runtime::Combat`), so SilentAura,
// WindCharge, Scaffold, Block-In, MLG, AutoLadder and Clutch arbitrate one
// rotation by the claim's priorities. The manager holds one controller:
// the silent managed rotation, stepped with its owner's settings, or a
// visible `Fixed` controller that turns the view. SilentAura steps the
// managed rotation itself; this driver steps it for the other owners.
use super::super::modules::combat::rotation::{
    Claim, Claimant, Fixed, Managed, Step, UPDATES_PER_TICK,
};
use super::super::modules::movement::movement_task::Body;
use super::super::modules::movement::scaffold_bridge::ScaffoldPoint;
use super::Runtime;
use super::combat_runtime::Combat;

#[derive(Clone, Debug, PartialEq)]
enum Controller {
    /// `AdaptiveRotationController`: the combat managed rotation.
    Silent(Step),
    /// `FixedRotationController` and subclasses: the view.
    Visible(Fixed),
    /// `ScaffoldPointRotationController`: the view toward a world point; it
    /// needs the player each update and never completes.
    Point(ScaffoldPoint),
}

#[derive(Clone, Debug, PartialEq)]
struct Drive {
    owner: Claimant,
    controller: Controller,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rotations {
    drive: Option<Drive>,
    /// The owner whose visible controller completed and was dropped: the
    /// module's reference still reads `isComplete()`.
    done: Option<Claimant>,
    /// The player's eye this tick, for a controller aimed at a point.
    eye: Option<[f64; 3]>,
}
impl Rotations {
    /// The owner of a silent controller this driver steps.
    pub(super) fn silent_owner(&self) -> Option<Claimant> {
        match &self.drive {
            Some(Drive {
                owner,
                controller: Controller::Silent(_),
            }) => Some(*owner),
            _ => None,
        }
    }
}

/// A silent controller's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Silent {
    pub target: (f32, f32),
    pub step: Step,
    pub tolerance: f32,
}

/// The claim, the managed rotation and the driver, borrowed together so a
/// module's tick can drive them while the runtime holds the module.
pub struct Shared<'a> {
    pub claim: &'a mut Claim,
    pub managed: &'a mut Managed,
    pub rotations: &'a mut Rotations,
}

impl<'a> Shared<'a> {
    /// The disjoint runtime fields, for a runtime method that also borrows
    /// its module state.
    pub(super) fn of(combat: &'a mut Combat, rotations: &'a mut Rotations) -> Self {
        Shared {
            claim: &mut combat.claim,
            managed: &mut combat.managed,
            rotations,
        }
    }
    /// `rotationClaim.isOwnedBy(this) || rotationClaim.acquire(this, preempt)`.
    pub fn claim(&mut self, who: Claimant, preempt: bool) -> bool {
        self.claim.owned_by(who) || self.claim.acquire(who, preempt, self.managed.relative)
    }
    pub fn owned_by(&self, who: Claimant) -> bool {
        self.claim.owned_by(who)
    }
    /// `rotationClaim.isBlockedFor(this)`.
    pub fn blocked_for(&self, who: Claimant) -> bool {
        self.claim.blocked_for(who)
    }
    /// `setController(new AdaptiveRotationController(target))`, or a new
    /// target and step for the owner's current silent controller.
    pub fn silent(&mut self, who: Claimant, view: (f32, f32), silent: Silent) {
        self.rotations.done = None;
        self.managed.start(view);
        self.managed.set_target(silent.target.0, silent.target.1);
        self.managed.set_tolerance(silent.tolerance);
        self.rotations.drive = Some(Drive {
            owner: who,
            controller: Controller::Silent(silent.step),
        });
    }
    /// `setController(fixed)`: a visible controller replaces whatever the
    /// manager ran (a silent rotation ends where it is).
    pub fn visible(&mut self, who: Claimant, fixed: Fixed) {
        if self.managed.active {
            self.managed.clear();
        }
        self.rotations.done = None;
        self.rotations.drive = Some(Drive {
            owner: who,
            controller: Controller::Visible(fixed),
        });
    }
    /// The owner's visible controller, to retarget it.
    pub fn visible_mut(&mut self, who: Claimant) -> Option<&mut Fixed> {
        match &mut self.rotations.drive {
            Some(Drive {
                owner,
                controller: Controller::Visible(fixed),
            }) if *owner == who => Some(fixed),
            _ => None,
        }
    }
    /// `setController(point)`, like `visible`.
    pub fn point(&mut self, who: Claimant, point: ScaffoldPoint) {
        if self.managed.active {
            self.managed.clear();
        }
        self.rotations.done = None;
        self.rotations.drive = Some(Drive {
            owner: who,
            controller: Controller::Point(point),
        });
    }
    /// The owner's point controller, to retarget it.
    pub fn point_mut(&mut self, who: Claimant) -> Option<&mut ScaffoldPoint> {
        match &mut self.rotations.drive {
            Some(Drive {
                owner,
                controller: Controller::Point(point),
            }) if *owner == who => Some(point),
            _ => None,
        }
    }
    /// Whether `who` has a controller in the manager.
    pub fn active(&self, who: Claimant) -> bool {
        self.rotations
            .drive
            .as_ref()
            .is_some_and(|d| d.owner == who)
    }
    /// `controller.isComplete()`.
    pub fn complete(&self, who: Claimant) -> bool {
        match &self.rotations.drive {
            Some(Drive {
                owner,
                controller: Controller::Visible(fixed),
            }) if *owner == who => fixed.complete(),
            Some(Drive {
                owner,
                controller: Controller::Silent(_),
            }) if *owner == who => self.managed.complete(),
            _ => self.rotations.done == Some(who),
        }
    }
    /// `RotationManager.getManagedYaw/Pitch`: the silent rotation, else the
    /// view.
    pub fn managed(&self, view: (f32, f32)) -> (f32, f32) {
        self.managed.sent().unwrap_or(view)
    }
    /// `releaseController(controller)` alone: a silent rotation returns to
    /// the view, a visible one stops; the claim is kept.
    pub fn release_controller(&mut self, who: Claimant) {
        if self.rotations.done == Some(who) {
            self.rotations.done = None;
        }
        if let Some(drive) = &self.rotations.drive
            && drive.owner == who
        {
            match drive.controller {
                Controller::Silent(_) => self.managed.release(),
                Controller::Visible(_) | Controller::Point(_) => self.rotations.drive = None,
            }
        }
    }
    /// `releaseController(controller)` then `rotationClaim.release(this)`.
    pub fn release(&mut self, who: Claimant) {
        self.release_controller(who);
        self.claim.release(who);
    }
    /// `RotationManager.onTick` for the movement and world owners: 50
    /// updates of the active controller. A visible one does not move with a
    /// screen open (`MouseRotationController.update`).
    pub fn tick(
        &mut self,
        view: (f32, f32),
        sensitivity: f64,
        no_screen: bool,
    ) -> Option<[f32; 2]> {
        self.tick_with(view, sensitivity, no_screen, None)
    }
    /// `tick` with the local player, which a point controller needs (it
    /// holds still without one).
    pub fn tick_with(
        &mut self,
        view: (f32, f32),
        sensitivity: f64,
        no_screen: bool,
        body: Option<&Body>,
    ) -> Option<[f32; 2]> {
        let eye = self.rotations.eye;
        let drive = self.rotations.drive.as_mut()?;
        let owner = drive.owner;
        match &mut drive.controller {
            Controller::Visible(fixed) => {
                if fixed.finished() {
                    self.rotations.drive = None;
                    self.rotations.done = Some(owner);
                    return None;
                }
                if let Some(eye) = eye {
                    fixed.set_eye(eye);
                }
                no_screen.then(|| fixed.tick([view.0, view.1], sensitivity, UPDATES_PER_TICK))
            }
            Controller::Point(point) => {
                let body = body?;
                no_screen.then(|| point.tick([view.0, view.1], sensitivity, UPDATES_PER_TICK, body))
            }
            Controller::Silent(step) => {
                let step = *step;
                let m = &mut *self.managed;
                // Another owner (SilentAura) took the managed rotation over.
                if !m.active || (!m.relative && !self.claim.owned_by(owner)) {
                    self.rotations.drive = None;
                    return None;
                }
                if let Some(eye) = eye {
                    m.aim(eye);
                }
                if m.tick(view, sensitivity, UPDATES_PER_TICK, || step) {
                    self.rotations.drive = None;
                }
                None
            }
        }
    }
}

impl Runtime {
    pub fn rotation(&mut self) -> Shared<'_> {
        Shared::of(&mut self.combat, &mut self.rotations)
    }
    /// The player's eye for the next `rotation_tick` (point targets).
    pub fn rotation_eye(&mut self, eye: [f64; 3]) {
        self.rotations.eye = Some(eye);
    }
    /// One client tick of the shared rotation; the new view for a visible
    /// controller.
    pub fn rotation_tick(
        &mut self,
        view: (f32, f32),
        sensitivity: f64,
        no_screen: bool,
    ) -> Option<[f32; 2]> {
        self.rotation().tick(view, sensitivity, no_screen)
    }
    /// `rotation_tick` with the local player for a point controller.
    pub fn rotation_tick_at(
        &mut self,
        view: (f32, f32),
        sensitivity: f64,
        no_screen: bool,
        body: &Body,
    ) -> Option<[f32; 2]> {
        self.rotation()
            .tick_with(view, sensitivity, no_screen, Some(body))
    }
    /// Session cleanup: the manager forgets every controller.
    pub(super) fn rotation_session_reset(&mut self) {
        self.rotations = Rotations::default();
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::modules::combat::rotation::UNSET;
    use super::*;

    fn step() -> Step {
        Step {
            counts: 7.0 * 0.25,
            clamp: false,
            proportional: false,
            linear: true,
        }
    }

    #[test]
    fn silent_owner_drives_managed_and_returns_on_release() {
        let mut r = Runtime::default();
        let mut s = r.rotation();
        assert!(s.claim(Claimant::WindCharge, true));
        s.silent(
            Claimant::WindCharge,
            (10.0, 0.0),
            Silent {
                target: (UNSET, 90.0),
                step: step(),
                tolerance: 5.0,
            },
        );
        for _ in 0..6 {
            s.tick((10.0, 0.0), 0.5, true);
        }
        assert!(s.complete(Claimant::WindCharge));
        assert!(s.managed((10.0, 0.0)).1 > 80.0);
        assert_eq!(s.managed((10.0, 0.0)).0, 10.0);
        assert_eq!(s.rotations.silent_owner(), Some(Claimant::WindCharge));
        s.release(Claimant::WindCharge);
        assert!(!s.owned_by(Claimant::WindCharge));
        for _ in 0..20 {
            s.tick((10.0, 0.0), 0.5, true);
        }
        assert_eq!(s.rotations.silent_owner(), None);
        assert_eq!(s.managed((10.0, 0.0)), (10.0, 0.0));
    }

    #[test]
    fn higher_priority_takes_the_rotation_and_lower_waits() {
        let mut r = Runtime::default();
        let mut s = r.rotation();
        assert!(s.claim(Claimant::Scaffold, false));
        s.visible(Claimant::Scaffold, Fixed::new([90.0, 80.0], 12.0));
        assert!(!s.claim(Claimant::SilentAura, true));
        assert!(s.claim(Claimant::Clutch, true));
        assert!(s.blocked_for(Claimant::Scaffold));
        s.visible(Claimant::Clutch, Fixed::new([0.0, 90.0], 12.0));
        assert!(s.visible_mut(Claimant::Scaffold).is_none());
        let view = s.tick((0.0, 0.0), 0.5, true);
        assert!(view.unwrap()[1] > 0.0);
        s.release(Claimant::Clutch);
        assert_eq!(s.claim.owner(), None);
        assert!(s.tick((0.0, 0.0), 0.5, true).is_none());
    }

    #[test]
    fn visible_controller_waits_for_a_closed_screen_and_drops_when_finished() {
        let mut r = Runtime::default();
        let mut s = r.rotation();
        assert!(s.claim(Claimant::AutoLadder, true));
        let mut fixed = Fixed::new([0.0, 45.0], 12.0);
        fixed.linear_acceleration = true;
        s.visible(Claimant::AutoLadder, fixed);
        assert!(s.tick((0.0, 0.0), 0.5, false).is_none());
        let mut view = (0.0, 0.0);
        for _ in 0..20 {
            if let Some([y, p]) = s.tick(view, 0.5, true) {
                view = (y, p);
            }
        }
        assert!((view.1 - 45.0).abs() < 3.5, "{view:?}");
        assert!(!s.active(Claimant::AutoLadder));
        assert!(s.complete(Claimant::AutoLadder));
        s.release(Claimant::AutoLadder);
        assert!(!s.complete(Claimant::AutoLadder));
    }

    #[test]
    fn silent_aura_takes_over_a_returning_rotation() {
        let mut r = Runtime::default();
        let mut s = r.rotation();
        assert!(s.claim(Claimant::Mlg, true));
        s.silent(
            Claimant::Mlg,
            (0.0, 0.0),
            Silent {
                target: (0.0, 90.0),
                step: step(),
                tolerance: 0.0,
            },
        );
        s.tick((0.0, 0.0), 0.5, true);
        s.release(Claimant::Mlg);
        // SilentAura acquires and restarts the managed rotation from there.
        assert!(s.claim(Claimant::SilentAura, true));
        s.managed.start((0.0, 0.0));
        s.tick((0.0, 0.0), 0.5, true);
        assert_eq!(s.rotations.silent_owner(), None);
        assert!(s.managed.active);
    }
}
