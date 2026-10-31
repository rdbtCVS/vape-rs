// Scaffold TellyBridge (reference `TellyBridgeScaffoldMode`). Like
// GodBridge it waits with the edge sneak until the user has placed
// "Activation blocks" by hand behind them, then walks onto the bridge
// line. From there it sprint-jumps along the bridge: on the ground it faces
// ahead and a movement task runs two or three blocks on; in the air a point
// rotation looks back at the side face past the last block and a right
// click places it. Every "Y increase" jumps (randomly one more or one
// fewer) it places one block up instead, towering the bridge. Back held
// (and right click with "Require right click"), the rotation claim, five
// blocks in the hotbar and a still mouse keep it running.
use super::super::super::core::movement_input::Axis;
use super::super::super::core::rotation_runtime::Shared;
use super::movement_task::{Body, Done, Task};
use super::scaffold::Sneak;
use super::scaffold_bridge::{
    self as bridge, Ctx, EdgeSneak, EdgeTick, HeldStack, aim_point, direction_rotation_speed,
    elevated_point, offset, rotation_speed,
};

/// `computeTargetPosition`: the point on the block at `pos` the movement
/// task walks to.
pub fn target_position(pos: [f64; 2], direction: u8) -> [f64; 2] {
    let (a, b) = match direction {
        6 => (0.3, 0.6),
        8 => (0.7, 0.4),
        7 => (0.4, 0.3),
        5 => (0.6, 0.7),
        _ => return pos,
    };
    [pos[0].floor() + a, pos[1].floor() + b]
}

/// `hasReachedPathEdge`: 0.8 along the block at `pos` (the jump point).
pub fn reached_path_edge(body: &Body, pos: [f64; 3], direction: u8) -> bool {
    let [x, _, z] = body.pos;
    match direction {
        6 => x - (pos[0] + 0.8) >= -0.05,
        8 => x - (pos[0] - 0.2) <= 0.05,
        7 => z - (pos[2] + 0.8) >= -0.05,
        5 => z - (pos[2] - 0.2) <= 0.05,
        _ => false,
    }
}

/// `hasPassedPathEdge` against the block at `pos`: still on it and on the
/// line.
pub fn passed_path_edge(body: &Body, pos: [f64; 3], direction: u8) -> bool {
    let [x, _, z] = body.pos;
    let [tx, _, tz] = pos;
    match direction {
        6 => x - (tx + 1.0) < -0.1 && (z - (tz + 0.6)).abs() <= 0.15,
        8 => x - tx > 0.1 && (z - (tz + 0.4)).abs() <= 0.15,
        7 => z - (tz + 1.0) < -0.1 && (x - (tx + 0.4)).abs() <= 0.15,
        5 => z - tz > 0.1 && (x - (tx + 0.6)).abs() <= 0.15,
        _ => false,
    }
}

/// The motion along the bridge's axis.
fn axis_motion(body: &Body, direction: u8) -> f64 {
    if direction.is_multiple_of(2) {
        body.motion[0]
    } else {
        body.motion[2]
    }
    .abs()
}

/// `hasAxisMotion`: moving along the bridge.
fn has_axis_motion(body: &Body, direction: u8) -> bool {
    axis_motion(body, direction) >= 0.1
}

/// `isAxisMotionBelowThreshold`: slower than a sprint jump along it.
fn axis_motion_below(body: &Body, direction: u8) -> bool {
    axis_motion(body, direction) < 0.6
}

#[derive(Debug, Default)]
pub struct TellyBridge {
    /// `bridgePath`: the placed blocks, the last one the next to build on.
    path: Vec<[f64; 3]>,
    bridging: bool,
    vertical_target: Option<[f64; 3]>,
    /// `bridgeLevel`: 1 while the jump places one block up.
    level: i32,
    edge: EdgeSneak,
    reset_pending: bool,
    activation_stack: Option<HeldStack>,
    was_airborne: bool,
    task: Option<Done>,
    manual_blocks: i32,
    height_increases: i32,
    initial: Option<[f64; 3]>,
    last_aim: Option<[f64; 3]>,
    manual_complete: bool,
    direction: u8,
    /// `recentPlacements`: recent mouse movement.
    history: Vec<i32>,
}

impl TellyBridge {
    /// `onEnable`.
    pub fn enable(&mut self) {
        self.reset_pending = true;
    }
    /// `onDisable` without `releaseControls` (the caller releases them).
    pub fn disable(&mut self) {
        self.reset_bridge_state();
    }
    pub fn reset_pending(&self) -> bool {
        self.reset_pending
    }
    /// `onPreEntityUpdate`: the edge sneak until the bridge activates.
    pub fn pre_entity(&mut self, t: &EdgeTick, s: &mut Shared<'_>) -> Option<Sneak> {
        if self.reset_pending {
            self.edge.pre(t, s)
        } else {
            None
        }
    }

    /// `onTick` (pre-tick).
    pub fn tick(&mut self, c: &mut Ctx<'_, '_>) {
        c.lock(self.bridging);
        if !self.reset_pending && self.bridging && self.looking_at_placement(c) {
            c.click = true;
        }
        if self.reset_pending {
            if self.bridging {
                self.edge.enable(c.now_ms);
                c.release_controls();
            }
            self.reset_bridge_state();
            self.reset_pending = self.evaluate_manual_activation(c);
            return;
        }
        if self.should_reset(c) {
            self.edge.enable(c.now_ms);
            return;
        }
        if !self.bridging && self.task.as_ref().is_some_and(Done::get) {
            self.bridging = true;
        }
        self.update_bridge(c);
    }

    fn last(&self) -> Option<[f64; 3]> {
        self.path.last().copied()
    }

    /// `evaluateManualActivation`: counts the user's placements; false
    /// once the walk onto the bridge line starts.
    fn evaluate_manual_activation(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        let body = c.body;
        let (bx, bz) = (body.pos[0].floor(), body.pos[2].floor());
        let y = c.placement_y();
        if !c.main_hand_valid() {
            self.initial = None;
            self.manual_blocks = 0;
            return true;
        }
        let current = bridge::cardinal(&body);
        if self.direction != 0 && current != self.direction {
            self.initial = None;
            self.manual_blocks = 0;
        }
        self.direction = current;
        let player_block = [bx, y, bz];
        let one_ahead = offset(player_block, 1, self.direction);
        let two_ahead = offset(player_block, 2, self.direction);
        let activation = c.options.telly_activation;
        if self.initial.is_none() && body.on_ground {
            if c.air(player_block) {
                self.initial = Some(player_block);
            } else if c.air(one_ahead) {
                self.initial = Some(one_ahead);
            } else if c.air(two_ahead) {
                self.initial = Some(two_ahead);
            }
        } else if let Some(initial) = self.initial {
            if f64::from(self.manual_blocks) >= activation {
                self.path.push(initial);
                self.manual_blocks = 0;
                let anchor = elevated_point(
                    c.rng,
                    offset(initial, -1, self.direction),
                    0.0,
                    self.direction,
                );
                let target = target_position([anchor[0], anchor[2]], self.direction);
                self.submit(c, target, false);
                self.activation_stack = c.inventory.held_stack();
                self.initial = None;
                self.manual_complete = true;
                return false;
            }
            if !c.air(initial) {
                self.manual_blocks += 1;
                let next = offset(initial, 1, self.direction);
                let next_empty = c.air(next);
                if next_empty && f64::from(self.manual_blocks) < activation {
                    self.initial = Some(next);
                } else if !next_empty {
                    self.initial = None;
                    self.manual_blocks = 0;
                }
            } else if bridge::drifted(
                initial,
                player_block,
                self.direction,
                activation,
                self.manual_blocks,
            ) {
                self.initial = None;
                self.manual_blocks = 0;
            }
        }
        true
    }

    /// `repeatLastPathPosition`.
    fn repeat_last(&mut self, size: usize) {
        if let Some(last) = self.last() {
            self.path = vec![last; size];
        }
    }

    /// `computeNextPlacementPosition`: the side face past the last block,
    /// or its top face for the block up.
    fn next_placement(&self, c: &mut Ctx<'_, '_>) -> Option<[f64; 3]> {
        let last = self.last()?;
        Some(if self.level != 0 && self.path.len() == 4 {
            elevated_point(c.rng, last, 0.2, self.direction)
        } else {
            aim_point(last, 0.3, 0.2, self.direction)
        })
    }

    /// `updateMovementKeys`: jump at the edge of the last block, sprint
    /// otherwise.
    fn update_movement_keys(&self, c: &mut Ctx<'_, '_>) {
        let Some(last) = self.last() else {
            return;
        };
        let body = &c.body;
        if body.on_ground
            && (self.level != 0 || body.sprinting)
            && reached_path_edge(body, last, self.direction)
        {
            c.keys.set(Axis::Jump, true);
            return;
        }
        if c.keys.down(Axis::Jump) {
            c.keys.set(Axis::Jump, false);
        }
        if !c.keys.down(Axis::Sprint) {
            c.keys.set(Axis::Sprint, true);
        }
    }

    /// `updateBridge`. The reference's legacy key-repeat switch is a no-op
    /// on 26.3. With every path block gone the reference throws out of the
    /// handler at the first path read; the port skips the rest of the tick.
    fn update_bridge(&mut self, c: &mut Ctx<'_, '_>) {
        if !self.bridging {
            return;
        }
        let on_ground = c.body.on_ground;
        if !on_ground {
            self.was_airborne = true;
        }
        self.prune_and_extend(c);
        let Some(last) = self.last() else {
            return;
        };
        let mut landed = false;
        if on_ground && (self.manual_complete || self.was_airborne) {
            landed = true;
            self.level = if self.manual_complete
                || (!has_axis_motion(&c.body, self.direction)
                    && passed_path_edge(&c.body, last, self.direction))
            {
                0
            } else {
                1
            };
            self.manual_complete = false;
            self.last_aim = None;
            self.vertical_target = None;
            let threshold = self.random_height_threshold(c);
            if self.height_increases >= threshold && self.level == 1 {
                if c.options.y_increase == 0.0 && !axis_motion_below(&c.body, self.direction) {
                    let target = self.movement_target(false);
                    self.submit(c, target, false);
                    self.repeat_last(5);
                    self.was_airborne = false;
                    landed = false;
                } else {
                    self.level = 0;
                    if self.vertical_target.is_none() {
                        let ahead = offset(last, 1, self.direction);
                        let vertical = elevated_point(c.rng, ahead, 0.0, self.direction);
                        self.vertical_target = Some(vertical);
                        let target = target_position([vertical[0], vertical[2]], self.direction);
                        self.submit(c, target, false);
                    }
                    self.repeat_last(1);
                    self.update_rotation_target(c);
                    return;
                }
            }
        }
        if landed {
            if self.level == 0 {
                let rotation = self.bridge_rotation(c);
                c.apply_fixed(rotation, rotation_speed(&c.body, rotation));
                c.keys.set(Axis::Sprint, true);
                let target = self.movement_target(true);
                self.submit(c, target, false);
                self.repeat_last(1);
                self.was_airborne = false;
                self.height_increases = 0;
            } else {
                let target = self.movement_target(false);
                self.submit(c, target, false);
                self.repeat_last(4);
                self.height_increases += 1;
                self.was_airborne = false;
            }
        }
        if !c.body.on_ground {
            self.update_rotation_target(c);
        }
        self.update_movement_keys(c);
    }

    /// `isGroundPositionClear`: true on the ground over the bridge's last
    /// block.
    fn ground_held(&self, c: &mut Ctx<'_, '_>) -> bool {
        let [mut x, _, mut z] = c.body.pos;
        match self.direction {
            6 => x += 0.15,
            8 => x -= 0.15,
            7 => z += 0.15,
            5 => z -= 0.15,
            _ => return true,
        }
        let y = c.placement_y();
        !c.air([x.floor(), y, z.floor()]) && c.body.on_ground
    }

    /// `shouldReset`: back to waiting for the activation.
    fn should_reset(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        if !c.owned() {
            self.reset_pending = true;
            c.keys.restore_physical();
            return true;
        }
        if !self.ensure_block_slot(c) {
            self.reset_pending = true;
            c.keys.release_all();
            return true;
        }
        let back = c.physical(Axis::Back);
        let activation = !c.options.require_right_click || c.right_mouse;
        if !back || !activation {
            if self.bridging {
                if !c.body.flying && !self.ground_held(c) {
                    return false;
                }
                c.keys.release_all();
            }
            self.reset_pending = true;
            return true;
        }
        // FreeLook is not ported, so the mouse history always counts.
        bridge::update_history(&mut self.history, c.mouse);
        if self.bridging && bridge::history_sum(&self.history) >= 10 {
            c.keys.release_all();
            self.reset_pending = true;
            return true;
        }
        false
    }

    /// `updateRotationTarget`: a point rotation at the next placement,
    /// renewed when it moves to another block.
    fn update_rotation_target(&mut self, c: &mut Ctx<'_, '_>) {
        let (Some(aim), Some(placement)) = (self.next_placement(c), self.last()) else {
            return;
        };
        let moved = self
            .last_aim
            .is_none_or(|last| (0..3).any(|i| last[i].floor() != aim[i].floor()));
        if moved {
            self.last_aim = Some(aim);
            let speed = direction_rotation_speed(c.body.yaw360(), self.direction);
            c.rotor.apply_point(
                c.rotation,
                c.view,
                &c.body,
                aim,
                speed,
                self.direction,
                placement,
            );
        }
    }

    /// `randomHeightIncreaseThreshold`: "Y increase", 15% one more and 10%
    /// one fewer.
    fn random_height_threshold(&self, c: &mut Ctx<'_, '_>) -> i32 {
        let random = c.rng.double();
        let configured = c.options.y_increase as i32;
        if configured == 0 {
            return 0;
        }
        if random < 0.15 {
            configured + 1
        } else if random < 0.25 {
            configured - 1
        } else {
            configured
        }
    }

    /// `computeBridgeRotation`: the view while running along the bridge.
    fn bridge_rotation(&self, c: &mut Ctx<'_, '_>) -> [f32; 2] {
        let mut yaw = match self.direction {
            6 => 230.0,
            8 => 50.0,
            7 => 320.0,
            5 => 140.0,
            _ => f64::from(c.body.yaw360()),
        };
        yaw += if c.rng.double() < 0.5 {
            c.rng.double() * -4.0
        } else {
            c.rng.double() * 4.0
        };
        let pitch = 90.0 + c.rng.double() * -5.0;
        [yaw as f32, pitch as f32]
    }

    /// `isLookingAtPlacement`: the crosshair on the last block, on a side
    /// face (the top face for the block up).
    fn looking_at_placement(&self, c: &Ctx<'_, '_>) -> bool {
        let (Some(hit), Some(last)) = (c.hit, self.last()) else {
            return false;
        };
        let matches = (0..3).all(|i| last[i] == f64::from(hit.pos[i]));
        let enough = if self.level != 0 && self.path.len() == 4 {
            hit.side == 1
        } else {
            hit.side > 1
        };
        matches && enough
    }

    /// `pruneAndExtendPath`: placed blocks drop off the end while air (the
    /// reference's bound shrinks with the path, so every trailing air entry
    /// goes); a placed block past the last one extends the path.
    fn prune_and_extend(&mut self, c: &mut Ctx<'_, '_>) {
        let mut index = self.path.len() as i64 - 1;
        while index >= (self.path.len() as i64 - 3).max(0) && c.air(self.path[index as usize]) {
            self.last_aim = None;
            self.path.remove(index as usize);
            index -= 1;
        }
        let Some(last) = self.last() else {
            return;
        };
        let size = self.path.len();
        let next = self.advance(last, size);
        if !c.air(next) {
            if size == 6 {
                self.path.clear();
            }
            self.path.push(next);
            c.release_rotation();
        }
    }

    /// `computeMovementTarget`: three blocks on from a level jump (the
    /// reference's four for the first one is read after it is cleared),
    /// two from one up.
    fn movement_target(&self, initial: bool) -> [f64; 2] {
        let last = self.last().unwrap_or_default();
        let distance = if initial {
            if self.manual_complete { 4 } else { 3 }
        } else {
            2
        };
        let p = offset(last, distance, self.direction);
        target_position([p[0], p[2]], self.direction)
    }

    /// `resetBridgeState`.
    fn reset_bridge_state(&mut self) {
        self.reset_pending = true;
        self.height_increases = 0;
        self.task = None;
        self.bridging = false;
        self.direction = 0;
        self.path.clear();
        self.activation_stack = None;
    }

    /// `submitMovementTask`: it waits for the ground after arriving.
    fn submit(&mut self, c: &mut Ctx<'_, '_>, target: [f64; 2], restore: bool) {
        let mut task = Task::new(target);
        task.restore = restore;
        task.wait_for_ground = true;
        self.task = Some(c.tasks.submit(task));
    }

    /// `ensureBlockSlot`: a block slot and at least five blocks.
    fn ensure_block_slot(&self, c: &mut Ctx<'_, '_>) -> bool {
        let Some(slot) = c.find_block_slot() else {
            return false;
        };
        if c.count_blocks(1, false) < 5 {
            return false;
        }
        c.reselect(self.activation_stack.as_ref(), slot);
        true
    }

    /// `advancePathPosition`: one block up for the block up, else one on.
    fn advance(&self, pos: [f64; 3], size: usize) -> [f64; 3] {
        let [mut x, mut y, mut z] = pos;
        if self.level != 0 && size == 4 {
            y += 1.0;
        } else {
            match self.direction {
                6 => x += 1.0,
                8 => x -= 1.0,
                7 => z += 1.0,
                5 => z -= 1.0,
                _ => {}
            }
        }
        [x, y, z]
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::Runtime;
    use super::super::super::rng::Rng;
    use super::super::movement_task::tests::Flat;
    use super::super::movement_task::{Keys, Tasks};
    use super::super::scaffold::Options;
    use super::super::scaffold_bridge::tests::{options, stone};
    use super::super::scaffold_bridge::{Hit, Inventory, Locks, OWNER, Rotor};
    use super::*;

    #[test]
    fn edge_and_target_tables() {
        assert_eq!(target_position([2.4, -3.1], 5), [2.6, -4.0 + 0.7]);
        assert_eq!(target_position([2.4, -3.1], 6), [2.3, -4.0 + 0.6]);
        assert_eq!(target_position([2.4, -3.1], 1), [2.4, -3.1]);
        let mut b = Body {
            pos: [0.6, 100.0, -1.1],
            ..Default::default()
        };
        // Bridging -Z on the block at z -2: the jump point is z -2.2.
        assert!(!reached_path_edge(&b, [0.0, 99.0, -2.0], 5));
        b.pos[2] = -2.16;
        assert!(reached_path_edge(&b, [0.0, 99.0, -2.0], 5));
        b.pos = [0.6, 100.0, -1.8];
        assert!(passed_path_edge(&b, [0.0, 99.0, -2.0], 5));
        b.pos[0] = 0.9;
        assert!(!passed_path_edge(&b, [0.0, 99.0, -2.0], 5), "off the line");
        b.pos = [2.95, 100.0, 0.5];
        assert!(reached_path_edge(&b, [2.0, 99.0, 0.0], 6));
    }

    struct Rig {
        runtime: Runtime,
        world: Flat,
        keys: Keys,
        tasks: Tasks,
        rotor: Rotor,
        locks: Locks,
        rng: Rng,
        options: Options,
    }

    impl Rig {
        fn new() -> Self {
            let mut world = Flat {
                feet: 100.0,
                ..Default::default()
            };
            for z in 0..4 {
                world.solid.insert([0, 99, z]);
            }
            let mut options = options();
            options.require_right_click = false;
            Self {
                runtime: Runtime::default(),
                world,
                keys: Keys::default(),
                tasks: Tasks::default(),
                rotor: Rotor::default(),
                locks: Locks::default(),
                rng: Rng::new(1),
                options,
            }
        }
        /// One tick facing +Z (the bridge goes -Z, direction 5).
        fn tick(
            &mut self,
            telly: &mut TellyBridge,
            now_ms: u64,
            body: Body,
            user: [bool; 7],
            hit: Option<Hit>,
        ) -> bool {
            self.keys.observe(user);
            let mut inventory = Inventory {
                slots: vec![None; 36],
                ..Default::default()
            };
            inventory.slots[0] = stone(64);
            let mut rotation = self.runtime.rotation();
            let mut c = Ctx {
                now_ms,
                no_screen: true,
                body,
                inventory,
                hit,
                right_mouse: false,
                mouse: [0, 0],
                view: [body.yaw, body.pitch],
                options: &self.options,
                world: &mut self.world,
                keys: &mut self.keys,
                tasks: &mut self.tasks,
                rotation: &mut rotation,
                rotor: &mut self.rotor,
                locks: &mut self.locks,
                rng: &mut self.rng,
                select: None,
                click: false,
            };
            telly.tick(&mut c);
            c.click
        }
    }

    fn standing(pos: [f64; 3]) -> Body {
        Body {
            pos,
            prev: pos,
            on_ground: true,
            eye_height: 1.62,
            ..Default::default()
        }
    }

    fn back() -> [bool; 7] {
        let mut keys = [false; 7];
        keys[Axis::Back as usize] = true;
        keys
    }

    #[test]
    fn hand_placed_blocks_start_the_walk_then_the_bridge() {
        let mut rig = Rig::new();
        let mut telly = TellyBridge::default();
        telly.enable();
        assert!(rig.runtime.rotation().claim(OWNER, false));
        let pos = standing([0.5, 100.0, 0.3]);
        rig.tick(&mut telly, 0, pos, back(), None);
        assert_eq!(telly.initial, Some([0.0, 99.0, -1.0]));
        rig.world.solid.insert([0, 99, -1]);
        rig.tick(&mut telly, 50, pos, back(), None);
        rig.world.solid.insert([0, 99, -2]);
        rig.tick(&mut telly, 100, pos, back(), None);
        assert_eq!(telly.manual_blocks, 2);
        rig.tick(&mut telly, 150, pos, back(), None);
        assert!(!telly.reset_pending(), "two blocks start the walk");
        assert_eq!(telly.path, vec![[0.0, 99.0, -2.0]]);
        assert!(rig.tasks.busy());
        assert!(!rig.locks.movement, "not bridging until the walk ends");
        // The walk to the block behind the last one ends: bridging starts on
        // the next tick, facing ahead and running three blocks on.
        rig.tasks.cancel_active();
        rig.tick(&mut telly, 200, pos, back(), None);
        assert!(telly.bridging);
        assert!(rig.rotor.active(), "the run rotation");
        assert!(rig.keys.down(Axis::Sprint));
        assert_eq!(telly.level, 0);
        rig.tick(&mut telly, 250, pos, back(), None);
        assert!(rig.locks.movement && rig.locks.right_click);
        // In the air past the last block, it aims back at its side face and
        // clicks there.
        let mut air = standing([0.6, 101.0, -2.3]);
        air.on_ground = false;
        rig.tick(&mut telly, 300, air, back(), None);
        assert!(telly.last_aim.is_some());
        let face = Some(Hit {
            pos: [0, 99, -2],
            side: 2,
        });
        assert!(rig.tick(&mut telly, 350, air, back(), face));
        // The placed block extends the path.
        rig.world.solid.insert([0, 99, -3]);
        rig.tick(&mut telly, 400, air, back(), None);
        assert_eq!(telly.last(), Some([0.0, 99.0, -3.0]));
    }

    #[test]
    fn letting_go_of_back_resets_once_on_the_ground() {
        let mut rig = Rig::new();
        let mut telly = TellyBridge::default();
        telly.enable();
        telly.reset_pending = false;
        telly.bridging = true;
        telly.direction = 5;
        telly.path = vec![[0.0, 99.0, 0.0]];
        assert!(rig.runtime.rotation().claim(OWNER, false));
        let mut air = standing([0.5, 100.5, -0.6]);
        air.on_ground = false;
        rig.tick(&mut telly, 0, air, [false; 7], None);
        assert!(!telly.reset_pending(), "mid-jump it keeps going");
        rig.tick(
            &mut telly,
            50,
            standing([0.5, 100.0, 0.5]),
            [false; 7],
            None,
        );
        assert!(telly.reset_pending());
        // Require right click: back alone does not run the bridge.
        rig.options.require_right_click = true;
        telly.reset_pending = false;
        rig.tick(&mut telly, 100, standing([0.5, 100.0, 0.5]), back(), None);
        assert!(telly.reset_pending());
    }

    /// Runs `f` with a bare context over `world`.
    fn with_ctx<R>(world: &mut Flat, hit: Option<Hit>, f: impl FnOnce(&mut Ctx<'_, '_>) -> R) -> R {
        let mut runtime = Runtime::default();
        let mut rotation = runtime.rotation();
        let options = options();
        let (mut keys, mut tasks, mut rotor, mut locks, mut rng) = (
            Keys::default(),
            Tasks::default(),
            Rotor::default(),
            Locks::default(),
            Rng::new(1),
        );
        let mut c = Ctx {
            now_ms: 0,
            no_screen: true,
            body: Body::default(),
            inventory: Inventory::default(),
            hit,
            right_mouse: false,
            mouse: [0, 0],
            view: [0.0, 0.0],
            options: &options,
            world,
            keys: &mut keys,
            tasks: &mut tasks,
            rotation: &mut rotation,
            rotor: &mut rotor,
            locks: &mut locks,
            rng: &mut rng,
            select: None,
            click: false,
        };
        f(&mut c)
    }

    #[test]
    fn the_path_prunes_air_and_extends_by_placed_blocks() {
        let mut world = Flat::default();
        world.solid.insert([0, 99, 0]);
        let mut telly = TellyBridge {
            direction: 5,
            path: vec![[0.0, 99.0, 0.0], [0.0, 99.0, -1.0], [0.0, 99.0, -2.0]],
            ..Default::default()
        };
        with_ctx(&mut world, None, |c| telly.prune_and_extend(c));
        assert_eq!(telly.path, vec![[0.0, 99.0, 0.0]]);
        world.solid.insert([0, 99, -1]);
        with_ctx(&mut world, None, |c| telly.prune_and_extend(c));
        assert_eq!(telly.path.len(), 2);
        // A block up on the fourth entry while towering.
        telly.level = 1;
        telly.path = vec![[0.0, 99.0, -1.0]; 4];
        world.solid.insert([0, 100, -1]);
        with_ctx(&mut world, None, |c| telly.prune_and_extend(c));
        assert_eq!(telly.last(), Some([0.0, 100.0, -1.0]));
        assert!(!with_ctx(&mut world, None, |c| telly.looking_at_placement(c)));
        let side = Some(Hit {
            pos: [0, 100, -1],
            side: 2,
        });
        assert!(with_ctx(&mut world, side, |c| telly.looking_at_placement(c)));
    }
}
