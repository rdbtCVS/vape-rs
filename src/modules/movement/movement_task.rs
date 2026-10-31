// The reference's movement-task manager (`PlayerMovementTaskManager` with
// `TargetPositionMovementTask`) and its key helpers (`MovementInputHelper`).
// A task walks the player toward a horizontal target by pressing the
// movement keys whose step brings the projected position closest to it.
//
// The reference presses key bindings (`KeyBinding.setKeyBindState`), which
// stay pressed until the next key event for that key. `Keys` is that
// binding state over the user's own input: an entry lasts until the user's
// state of its axis changes, and the movement writer turns the entries into
// requests each tick.
use super::super::super::core::movement_input::{Axis, Owner, Request};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The local player as the movement and placement helpers read it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub pos: [f64; 3],
    /// Last tick's position (`prevPosX/Y/Z`).
    pub prev: [f64; 3],
    pub motion: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    /// `isSneaking`.
    pub sneaking: bool,
    pub sprinting: bool,
    /// `capabilities.isFlying`.
    pub flying: bool,
    pub eye_height: f64,
    /// `isOnLadder`: in a climbable block.
    pub on_ladder: bool,
}

impl Body {
    /// `RotationUtil.c()`: the yaw in 0..360.
    pub fn yaw360(&self) -> f32 {
        (self.yaw % 360.0 + 360.0) % 360.0
    }
}

/// World queries for the helpers.
pub trait World {
    /// `isAirBlock` at a block position.
    fn air(&mut self, pos: [i32; 3]) -> bool;
    /// No collision for the player's box placed at the horizontal position
    /// `at`, shrunk horizontally by `shrink`, moved by this tick's
    /// horizontal motion and one block down
    /// (`getCollidingBoundingBoxes(...).isEmpty()`).
    fn clear_below(&mut self, at: [f64; 2], shrink: f64) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bound {
    pressed: bool,
    /// The user's state when the binding was set.
    user: bool,
}

/// The key bindings' pressed state over the user's input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    bound: [Option<Bound>; 7],
    user: [bool; 7],
}

impl Keys {
    /// This tick's user (physical) state; a key event of an axis, the user's
    /// state changing, sets the binding to it.
    pub fn observe(&mut self, user: [bool; 7]) {
        for (bound, (&now, &was)) in self.bound.iter_mut().zip(user.iter().zip(&self.user)) {
            if now != was {
                *bound = None;
            }
        }
        self.user = user;
    }
    /// `ClientSettings.isPhysicalKeyDown`.
    pub fn user(&self, axis: Axis) -> bool {
        self.user[axis as usize]
    }
    /// `KeyBinding.isKeyDown`.
    pub fn down(&self, axis: Axis) -> bool {
        self.bound[axis as usize].map_or(self.user[axis as usize], |b| b.pressed)
    }
    /// `setKeyPressed`.
    pub fn set(&mut self, axis: Axis, pressed: bool) {
        let user = self.user[axis as usize];
        self.bound[axis as usize] = (pressed != user).then_some(Bound { pressed, user });
    }
    /// `releaseMovementKeys`.
    pub fn release_movement(&mut self) {
        for axis in [Axis::Left, Axis::Right, Axis::Forward, Axis::Back] {
            self.set(axis, false);
        }
    }
    /// `releaseAllInput`.
    pub fn release_all(&mut self) {
        self.release_movement();
        for axis in [Axis::Sprint, Axis::Sneak, Axis::Jump] {
            self.set(axis, false);
        }
    }
    /// `restorePhysicalInput`.
    pub fn restore_physical(&mut self) {
        self.bound = [None; 7];
    }
    /// The bindings as this tick's movement requests.
    pub fn requests(&self) -> Vec<Request> {
        Axis::ALL
            .iter()
            .zip(&self.bound)
            .filter_map(|(&axis, bound)| {
                bound.map(|b| Request {
                    axis,
                    owner: Owner::KeyHelper,
                    priority: 3,
                    pressed: b.pressed,
                })
            })
            .collect()
    }
}

/// A task's completion flag, shared with the module that submitted it.
#[derive(Clone, Debug, Default)]
pub struct Done(Arc<AtomicBool>);

impl Done {
    /// A task that is already complete (the modes' placeholder task).
    pub fn completed() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    pub fn set(&self, completed: bool) {
        self.0.store(completed, Ordering::Relaxed);
    }
    pub fn same(&self, other: &Done) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// `TargetPositionMovementTask`.
#[derive(Clone, Debug)]
pub struct Task {
    pub target: [f64; 2],
    pub tolerance: f64,
    pub restore: bool,
    pub wait_for_ground: bool,
    pub sneak_near: bool,
    pub require_support: bool,
    /// Keys the task never presses (forward, right, back or left).
    pub excluded: Vec<Axis>,
    pub ignore_x: bool,
    pub ignore_z: bool,
    remaining: [f64; 2],
    reached: bool,
    done: Done,
}

impl Task {
    pub fn new(target: [f64; 2]) -> Self {
        Self {
            target,
            tolerance: 0.2,
            restore: true,
            wait_for_ground: false,
            sneak_near: false,
            require_support: false,
            excluded: Vec::new(),
            ignore_x: false,
            ignore_z: false,
            remaining: [0.0; 2],
            reached: false,
            done: Done::default(),
        }
    }
    pub fn done(&self) -> Done {
        self.done.clone()
    }
    /// `hasReachedTarget`, which also refreshes the remaining offset.
    fn reached_target(&mut self, body: &Body) -> bool {
        self.remaining = [
            if self.ignore_x {
                0.0
            } else {
                self.target[0] - body.pos[0]
            },
            if self.ignore_z {
                0.0
            } else {
                self.target[1] - body.pos[2]
            },
        ];
        let limit = if self.sneak_near { 0.1 } else { self.tolerance };
        self.remaining[0].abs() <= limit && self.remaining[1].abs() <= limit
    }
    /// `updateCompletion`.
    fn update(&mut self, body: &Body, no_screen: bool) {
        if !no_screen {
            return;
        }
        if self.reached_target(body) {
            self.reached = true;
            self.done.set(true);
        } else if self.reached && (!self.wait_for_ground || body.on_ground) {
            self.done.set(true);
        }
    }
    /// `applyMovementInput`, from the offset of the last completion check.
    fn apply(&self, body: &Body, keys: &mut Keys, world: &mut dyn World) {
        let [rx, rz] = self.remaining;
        if rx == 0.0 && rz == 0.0 {
            return;
        }
        let sneak = (self.sneak_near && rx.abs() < 2.0 && rz.abs() < 2.0) || keys.user(Axis::Sneak);
        keys.set(Axis::Sneak, sneak);
        apply_toward(
            body,
            keys,
            world,
            self.remaining,
            &self.excluded,
            self.require_support,
        );
    }
}

/// `getMovementStep`: how far one key moves the player this tick.
pub fn step(body: &Body, axis: Axis) -> f64 {
    let mut step = 0.2;
    if body.sneaking && body.on_ground {
        step = 0.06;
    } else if body.sprinting && axis == Axis::Forward {
        step = 0.3;
    }
    if !body.on_ground {
        step *= 0.02;
    }
    step
}

/// `applyMovementToward`: each of forward, right, back and left (minus the
/// excluded) is pressed when its step brings the projected position closer
/// to `pos + offset`; with `require_support` nothing is pressed unless the
/// projected position stands on something.
pub fn apply_toward(
    body: &Body,
    keys: &mut Keys,
    world: &mut dyn World,
    offset: [f64; 2],
    excluded: &[Axis],
    require_support: bool,
) {
    let yaw = f64::from(body.yaw360());
    let mut candidates: Vec<(f64, Axis)> = [Axis::Forward, Axis::Right, Axis::Back, Axis::Left]
        .into_iter()
        .enumerate()
        .map(|(i, axis)| ((yaw + 90.0 * i as f64) % 360.0, axis))
        .collect();
    candidates.retain(|(_, axis)| !excluded.contains(axis));
    let target = [body.pos[0] + offset[0], body.pos[2] + offset[1]];
    let distance = |p: [f64; 2]| ((target[0] - p[0]).powi(2) + (target[1] - p[1]).powi(2)).sqrt();
    let mut projected = [body.pos[0] + body.motion[0], body.pos[2] + body.motion[2]];
    let mut best = distance(projected);
    let mut selected = Vec::with_capacity(candidates.len());
    for &(heading, axis) in &candidates {
        let step = step(body, axis);
        let radians = heading.to_radians();
        let moved = [
            projected[0] - step * radians.sin(),
            projected[1] + step * radians.cos(),
        ];
        let d = distance(moved);
        if d < best {
            projected = moved;
            best = d;
            selected.push(true);
        } else {
            selected.push(false);
        }
    }
    if require_support && world.clear_below(projected, 0.15) {
        selected.iter_mut().for_each(|s| *s = false);
    }
    for (&(_, axis), &pressed) in candidates.iter().zip(&selected) {
        keys.set(axis, pressed);
    }
}

/// `PlayerMovementTaskManager`: at most one active task.
#[derive(Debug, Default)]
pub struct Tasks {
    active: Option<Task>,
}

impl Tasks {
    /// `submit`: replaces the active task.
    pub fn submit(&mut self, task: Task) -> Done {
        let done = task.done();
        self.active = Some(task);
        done
    }
    /// `cancel(task)`: completes it if it is the active one.
    pub fn cancel(&mut self, done: &Done) {
        if let Some(task) = &self.active
            && task.done.same(done)
        {
            task.done.set(true);
        }
    }
    /// `cancel(getActiveTask())`.
    pub fn cancel_active(&mut self) {
        if let Some(task) = &self.active {
            task.done.set(true);
        }
    }
    /// A task is running and not complete.
    pub fn busy(&self) -> bool {
        self.active.as_ref().is_some_and(|t| !t.done.get())
    }
    pub fn active(&self) -> bool {
        self.active.is_some()
    }
    /// `onPreLocalPlayerTick`: the completion check; a finished task hands
    /// the keys back (or releases the movement keys) and is dropped.
    pub fn update(&mut self, body: &Body, no_screen: bool, keys: &mut Keys) {
        let Some(task) = &mut self.active else {
            return;
        };
        task.update(body, no_screen);
        if task.done.get() {
            if task.restore {
                keys.restore_physical();
            } else {
                keys.release_movement();
            }
            self.active = None;
        }
    }
    /// `onTick` (pre-tick): the active task presses its keys.
    pub fn apply(&mut self, body: &Body, keys: &mut Keys, world: &mut dyn World) {
        if let Some(task) = &self.active {
            task.apply(body, keys, world);
        }
    }
    pub fn clear(&mut self) {
        self.active = None;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A flat world of solid blocks, air elsewhere.
    #[derive(Default)]
    pub(crate) struct Flat {
        pub solid: HashSet<[i32; 3]>,
        /// The player box is 0.6 wide; its feet are at this height.
        pub feet: f64,
        pub motion: [f64; 2],
    }
    impl World for Flat {
        fn air(&mut self, pos: [i32; 3]) -> bool {
            !self.solid.contains(&pos)
        }
        fn clear_below(&mut self, at: [f64; 2], shrink: f64) -> bool {
            let half = 0.3 - shrink;
            let (x, z) = (at[0] + self.motion[0], at[1] + self.motion[1]);
            let y = (self.feet - 1.0).floor() as i32;
            let xs = (x - half).floor() as i32..=((x + half).floor() as i32);
            !xs.into_iter().any(|bx| {
                ((z - half).floor() as i32..=((z + half).floor() as i32))
                    .any(|bz| self.solid.contains(&[bx, y, bz]))
            })
        }
    }

    fn body(pos: [f64; 3], yaw: f32) -> Body {
        Body {
            pos,
            prev: pos,
            yaw,
            on_ground: true,
            ..Default::default()
        }
    }

    #[test]
    fn keys_hold_until_the_user_changes_that_key() {
        let mut keys = Keys::default();
        keys.observe([false; 7]);
        keys.set(Axis::Back, true);
        keys.set(Axis::Forward, false);
        assert!(keys.down(Axis::Back));
        assert_eq!(keys.requests().len(), 1);
        keys.observe([false; 7]);
        assert!(keys.down(Axis::Back));
        // The user presses and releases back: the binding follows the key.
        let mut held = [false; 7];
        held[Axis::Back as usize] = true;
        keys.observe(held);
        assert!(keys.down(Axis::Back));
        keys.observe([false; 7]);
        assert!(!keys.down(Axis::Back));
        assert!(keys.requests().is_empty());
        keys.observe(held);
        keys.release_all();
        assert!(!keys.down(Axis::Back));
        assert!(!keys.requests()[0].pressed);
        keys.restore_physical();
        assert!(keys.down(Axis::Back));
    }

    #[test]
    fn task_walks_toward_the_target_then_restores_the_keys() {
        let mut world = Flat::default();
        let mut keys = Keys::default();
        let mut tasks = Tasks::default();
        // Facing +Z (yaw 0): a target at +X is to the left (yaw 270 heading).
        let done = tasks.submit(Task::new([2.5, 0.5]));
        let b = body([0.5, 100.0, 0.5], 0.0);
        tasks.apply(&b, &mut keys, &mut world);
        assert!(
            keys.requests().is_empty(),
            "no offset before the first check"
        );
        tasks.update(&b, true, &mut keys);
        tasks.apply(&b, &mut keys, &mut world);
        assert!(keys.down(Axis::Left) && !keys.down(Axis::Right));
        assert!(!keys.down(Axis::Forward) && !keys.down(Axis::Back));
        assert!(tasks.busy());
        tasks.update(&body([2.4, 100.0, 0.5], 0.0), true, &mut keys);
        assert!(done.get());
        assert!(!tasks.active());
        assert!(!keys.down(Axis::Left), "restored to the user's keys");
    }

    #[test]
    fn task_with_a_closed_screen_only_and_support_and_exclusions() {
        let mut world = Flat {
            feet: 100.0,
            ..Default::default()
        };
        let mut keys = Keys::default();
        let mut tasks = Tasks::default();
        let mut task = Task::new([0.5, 3.5]);
        task.require_support = true;
        task.restore = false;
        tasks.submit(task);
        let b = body([0.5, 100.0, 0.5], 0.0);
        tasks.update(&b, false, &mut keys);
        tasks.apply(&b, &mut keys, &mut world);
        assert!(keys.requests().is_empty(), "a screen skips the check");
        tasks.update(&b, true, &mut keys);
        tasks.apply(&b, &mut keys, &mut world);
        assert!(
            !keys.down(Axis::Forward),
            "no floor under the projected step"
        );
        world.solid.insert([0, 99, 0]);
        tasks.apply(&b, &mut keys, &mut world);
        assert!(keys.down(Axis::Forward));
        // Excluding forward leaves no key that gets closer; the excluded key
        // is not touched (the replaced task's binding stays).
        let mut task = Task::new([0.5, 3.5]);
        task.excluded = vec![Axis::Forward];
        let done = tasks.submit(task);
        tasks.update(&b, true, &mut keys);
        tasks.apply(&b, &mut keys, &mut world);
        assert!(keys.down(Axis::Forward));
        assert!(!keys.down(Axis::Back) && !keys.down(Axis::Left) && !keys.down(Axis::Right));
        tasks.cancel(&done);
        tasks.update(&b, true, &mut keys);
        assert!(!tasks.active());
    }

    #[test]
    fn steps_follow_sneak_sprint_and_air() {
        let mut b = body([0.0; 3], 0.0);
        assert_eq!(step(&b, Axis::Back), 0.2);
        b.sprinting = true;
        assert_eq!(step(&b, Axis::Forward), 0.3);
        assert_eq!(step(&b, Axis::Left), 0.2);
        b.sneaking = true;
        assert_eq!(step(&b, Axis::Forward), 0.06);
        b.on_ground = false;
        assert!((step(&b, Axis::Forward) - 0.006).abs() < 1e-12);
    }
}
