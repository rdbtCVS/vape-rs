// Scaffold GodBridge (reference `BlatantScaffoldMode`). Until it activates,
// the edge sneak runs and the mode counts the blocks the user places by
// hand along the direction behind them; after "Activation blocks" it takes
// the walk over: a movement task to the bridge line, a fixed rotation at
// the placement face, the back key (with a strafe key on straight bridges)
// while the block under the player stands, and a right click whenever the
// crosshair is on a side face. The strafe keys turn the bridge by 45
// degrees. The physical back key, the rotation claim, a block in the
// hotbar and a still mouse keep it running.
use super::super::super::core::movement_input::Axis;
use super::super::super::core::rotation_runtime::Shared;
use super::movement_task::{Body, Done, Task};
use super::scaffold::Sneak;
use super::scaffold_bridge::{
    self as bridge, Ctx, EdgeSneak, EdgeTick, HeldStack, Timer, angular_distance, offset, speed,
};

/// `directionCycle`: the eight directions, 45 degrees apart.
const CYCLE: [u8; 8] = [5, 4, 6, 1, 7, 2, 8, 3];

/// `rotateDirection`: `offset` steps of 45 degrees along the cycle.
pub fn rotate_direction(direction: u8, offset: i32) -> u8 {
    let index = CYCLE
        .iter()
        .position(|&d| d == direction)
        .map_or(-1, |i| i as i32)
        + offset;
    CYCLE[index.rem_euclid(CYCLE.len() as i32) as usize]
}

/// `computePlacementPoint`: where the player stands on the block at `pos`
/// to bridge along `direction`.
pub fn placement_point(pos: [f64; 2], direction: u8, reversed: bool) -> [f64; 2] {
    let (a, b) = match (direction, reversed) {
        (1, true) => (0.65, 0.35),
        (1, false) => (0.35, 0.65),
        (2, true) => (0.65, 0.65),
        (2, false) => (0.35, 0.35),
        (3, true) => (0.35, 0.65),
        (3, false) => (0.65, 0.35),
        (4, true) => (0.35, 0.35),
        (4, false) => (0.65, 0.65),
        (6, true) => (0.8, 0.8),
        (6, false) => (0.8, 0.2),
        (8, true) => (0.2, 0.2),
        (8, false) => (0.2, 0.8),
        (7, true) => (0.2, 0.8),
        (7, false) => (0.8, 0.8),
        (5, true) => (0.8, 0.2),
        (5, false) => (0.2, 0.2),
        _ => return pos,
    };
    [pos[0].floor() + a, pos[1].floor() + b]
}

/// `isPastCorner`: the player stands past the middle of their block along
/// `direction`. Measured from the floored corner, so the -X and -Z checks
/// (8, 5) never pass, as in the reference.
pub fn past_corner(body: &Body, direction: u8) -> bool {
    let [x, _, z] = body.pos;
    let (bx, bz) = (x.floor(), z.floor());
    match direction {
        1 => x - bx + (z - bz) > 1.0,
        2 => bx - x + (z - bz) > 1.0,
        3 => bx - x + (bz - z) > 1.0,
        4 => x - bx + (bz - z) > 1.0,
        6 => x - bx > 0.5,
        8 => bx - x > 0.5,
        7 => z - bz > 0.5,
        5 => bz - z > 0.5,
        _ => false,
    }
}

/// `getDirectionYawBounds`.
fn yaw_bounds(direction: u8) -> [f64; 2] {
    match direction {
        6 => [135.0, 45.0],
        8 => [315.0, 225.0],
        7 => [225.0, 135.0],
        _ => [45.0, 315.0],
    }
}

#[derive(Debug)]
pub struct GodBridge {
    move_timer: Timer,
    switch_timer: Timer,
    /// `targetPos`: the bridge line point the player walks along.
    target_pos: Option<[f64; 2]>,
    /// `slotHistory`: recent mouse movement.
    history: Vec<i32>,
    rotation_pending: bool,
    at_edge: bool,
    task_blocks_update: bool,
    task_tick_limit: u32,
    prev_left: bool,
    prev_right: bool,
    blocks_placed: i32,
    pending_direction: u8,
    direction: u8,
    activation_pending: bool,
    task_ticks: u32,
    reversed: bool,
    task: Option<Done>,
    key_idle: bool,
    held_block: Option<HeldStack>,
    pending_reversed: bool,
    pending_pos: Option<[f64; 2]>,
    switching: bool,
    target_rotation: Option<[f32; 2]>,
    edge: EdgeSneak,
    /// The next block the user places by hand before the activation.
    place_pos: Option<[f64; 3]>,
}

impl Default for GodBridge {
    fn default() -> Self {
        Self {
            move_timer: Timer::default(),
            switch_timer: Timer::default(),
            target_pos: None,
            history: Vec::new(),
            rotation_pending: false,
            at_edge: false,
            task_blocks_update: false,
            task_tick_limit: 0,
            prev_left: false,
            prev_right: false,
            blocks_placed: 0,
            pending_direction: 0,
            direction: 0,
            activation_pending: false,
            task_ticks: 0,
            reversed: false,
            task: None,
            key_idle: true,
            held_block: None,
            pending_reversed: false,
            pending_pos: None,
            switching: false,
            target_rotation: None,
            edge: EdgeSneak::default(),
            place_pos: None,
        }
    }
}

impl GodBridge {
    /// `onEnable`.
    pub fn enable(&mut self) {
        self.activation_pending = true;
    }
    /// `onDisable` without `releaseControls` (the caller releases them):
    /// `resetState`; the caller clears the locks.
    pub fn disable(&mut self) {
        self.reset_fields();
    }
    pub fn activation_pending(&self) -> bool {
        self.activation_pending
    }
    /// `onPreEntityUpdate`: the edge sneak until the bridge activates.
    pub fn pre_entity(&mut self, t: &EdgeTick, s: &mut Shared<'_>) -> Option<Sneak> {
        if self.activation_pending {
            self.edge.pre(t, s)
        } else {
            None
        }
    }

    /// `onTick` (pre-tick).
    pub fn tick(&mut self, c: &mut Ctx<'_, '_>) {
        if !self.activation_pending && self.looking_at_placement(c) {
            c.click = true;
        }
        if !(self.activation_pending || self.rotation_pending || self.task_blocks_update) {
            self.update_bridge_input(c);
        }
        if self.activation_pending {
            if self.task.is_some() || self.target_rotation.is_some() {
                c.release_controls();
                self.edge.enable(c.now_ms);
            }
            self.reset_state(c);
            self.activation_pending = self.update_placement(c);
            c.lock(false);
            return;
        }
        c.lock(true);
        if self.should_release_control(c) {
            c.release_rotation();
            self.edge.enable(c.now_ms);
            c.lock(false);
            return;
        }
        if self.handle_rotation_and_task(c) {
            return;
        }
        if self.handle_direction_switch(c) {
            return;
        }
        if self.recover_if_stuck(c) {
            return;
        }
        self.update_rotation(c);
        self.update_bridge_input(c);
    }

    /// `targetPos`; it is set whenever the bridge runs (the player's
    /// position stands in for the reference's null).
    fn target(&self, body: &Body) -> [f64; 2] {
        self.target_pos.unwrap_or([body.pos[0], body.pos[2]])
    }

    /// `isNextBlockEmpty`, which is true when the sampled block ahead is
    /// not air.
    fn next_block_filled(&self, c: &mut Ctx<'_, '_>) -> bool {
        let [x, _, z] = c.body.pos;
        let tp = self.target(&c.body);
        let (x, z) = match self.direction {
            1 => (x + 0.2, z + 0.2),
            2 => (x - 0.2, z + 0.2),
            3 => (x - 0.2, z - 0.2),
            4 => (x + 0.2, z - 0.2),
            6 => (x + 0.25, tp[1]),
            8 => (x - 0.25, tp[1]),
            7 => (tp[0], z + 0.25),
            5 => (tp[0], z - 0.25),
            _ => return true,
        };
        let y = c.placement_y();
        !c.air([x.floor(), y, z.floor()])
    }

    /// `advanceTask`: true while the task holds the bridge back, or when it
    /// ran out of ticks and was cancelled.
    fn advance_task(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        let Some(task) = &self.task else {
            return false;
        };
        if !task.get() && self.task_ticks < self.task_tick_limit {
            self.task_ticks += 1;
            if self.task_blocks_update {
                return true;
            }
        } else if task.get() {
            self.task_ticks = 0;
            self.task_blocks_update = false;
        } else {
            task.set(true);
            c.tasks.cancel_active();
            self.task = None;
            self.task_ticks = 0;
            self.task_blocks_update = false;
            return true;
        }
        false
    }

    /// `shouldReverse`: on a straight bridge, the side whose yaw bound is
    /// nearer.
    fn should_reverse(&self, body: &Body, direction: u8) -> bool {
        if direction > 4 {
            let [a, b] = yaw_bounds(direction);
            let yaw = f64::from(body.yaw360());
            angular_distance(yaw, a) <= angular_distance(yaw, b)
        } else {
            !self.reversed
        }
    }

    /// The mode's own `applyRotation`.
    fn apply_rotation(c: &mut Ctx<'_, '_>, rotation: [f32; 2], speed: f32) {
        c.rotor.apply_mode(c.rotation, rotation, speed);
    }

    /// `isFrontClear`.
    fn front_clear(c: &mut Ctx<'_, '_>) -> bool {
        c.clear(0.2)
    }

    /// `updateBridgeInput`: back (and the strafe key on a straight bridge)
    /// until the player reaches the edge.
    fn update_bridge_input(&mut self, c: &mut Ctx<'_, '_>) {
        let at_edge = self.is_at_edge(c, self.direction);
        let diagonal = self.direction < 5;
        if !at_edge {
            self.move_timer.reset(c.now_ms);
        }
        self.at_edge = at_edge;
        if at_edge {
            c.keys.set(Axis::Back, false);
            c.keys.set(Axis::Left, false);
            c.keys.set(Axis::Right, false);
        } else {
            c.keys.set(Axis::Back, true);
            if !diagonal {
                c.keys.set(
                    if self.reversed {
                        Axis::Right
                    } else {
                        Axis::Left
                    },
                    true,
                );
            } else {
                c.keys.set(Axis::Left, false);
                c.keys.set(Axis::Right, false);
            }
        }
    }

    /// `computeAnchorTarget`: the block to bridge on again after a stall.
    fn anchor_target(c: &mut Ctx<'_, '_>, direction: u8) -> [f64; 3] {
        let y = c.placement_y() + if c.body.motion[1] > 0.0 { 1.0 } else { 0.0 };
        let mut a = [c.body.pos[0].floor(), y, c.body.pos[2].floor()];
        if c.air(a) {
            a = offset(a, -1, direction);
            if c.air(a) {
                a = offset(a, 1, rotate_direction(direction, 2));
                if c.air(a) {
                    a = offset(a, -2, rotate_direction(direction, 2));
                }
            }
        }
        a
    }

    /// `shouldReleaseControl`: back to waiting for the activation.
    fn should_release_control(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        if !c.owned() {
            self.activation_pending = true;
            c.keys.restore_physical();
            return true;
        }
        if !self.ensure_block_selected(c) {
            self.activation_pending = true;
            c.keys.release_movement();
            return true;
        }
        if !c.physical(Axis::Back) {
            if !c.body.flying && !self.next_block_filled(c) {
                return false;
            }
            c.keys.release_movement();
            self.activation_pending = true;
            return true;
        }
        // FreeLook is not ported, so the mouse history always counts.
        bridge::update_history(&mut self.history, c.mouse);
        if bridge::history_sum(&self.history) >= 10 {
            c.keys.release_movement();
            self.activation_pending = true;
            return true;
        }
        false
    }

    /// `ensureBlockSelected`.
    fn ensure_block_selected(&self, c: &mut Ctx<'_, '_>) -> bool {
        let Some(slot) = c.find_block_slot() else {
            return false;
        };
        c.reselect(self.held_block.as_ref(), slot);
        true
    }

    /// `isRotationOffTarget`: a pending rotation holds the bridge back
    /// until the yaw is within 4 degrees, then it is released.
    fn rotation_off_target(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        if c.rotor.active() {
            let yaw = f64::from(c.body.yaw360());
            if angular_distance(yaw, f64::from(c.rotor.target()[0])) > 4.0 {
                if self.rotation_pending {
                    return true;
                }
            } else if self.rotation_pending {
                c.release_rotation();
                self.rotation_pending = false;
            }
        }
        false
    }

    /// `handleRotationAndTask`: both run every tick.
    fn handle_rotation_and_task(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        let off_target = self.rotation_off_target(c);
        let task_blocking = self.advance_task(c);
        off_target || task_blocking
    }

    /// `isAtEdge`: on a straight bridge the block just in front along the
    /// line is air; on a diagonal one nothing is under the shrunk box.
    fn is_at_edge(&self, c: &mut Ctx<'_, '_>, direction: u8) -> bool {
        if direction > 4 {
            let [x, _, z] = c.body.pos;
            let tp = self.target(&c.body);
            let (x, z) = match direction {
                6 => (x - 0.15, tp[1]),
                8 => (x + 0.15, tp[1]),
                7 => (tp[0], z - 0.15),
                5 => (tp[0], z + 0.15),
                _ => (x, z),
            };
            let y = c.placement_y();
            return c.air([x.floor(), y, z.floor()]);
        }
        c.clear(0.16)
    }

    /// `updatePlacement`: counts the user's placements; false once the
    /// bridge activates.
    fn update_placement(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        let body = c.body;
        let (bx, bz) = (body.pos[0].floor(), body.pos[2].floor());
        let y = c.placement_y();
        if !c.main_hand_valid() {
            self.place_pos = None;
            self.blocks_placed = 0;
            return true;
        }
        let current = bridge::cardinal(&body);
        if self.direction != 0 && current != self.direction {
            self.place_pos = None;
            self.blocks_placed = 0;
        }
        self.direction = current;
        let player_block = [bx, y, bz];
        let one_ahead = offset(player_block, 1, self.direction);
        let two_ahead = offset(player_block, 2, self.direction);
        let activation = c.options.god_activation;
        if self.place_pos.is_none() && body.on_ground {
            if c.air(player_block) {
                self.place_pos = Some(player_block);
            } else if c.air(one_ahead) {
                self.place_pos = Some(one_ahead);
            } else if c.air(two_ahead) {
                self.place_pos = Some(two_ahead);
            }
        } else if let Some(place) = self.place_pos {
            if f64::from(self.blocks_placed) >= activation {
                self.reversed = self.should_reverse(&body, self.direction);
                let target = placement_point([place[0], place[2]], self.direction, self.reversed);
                self.target_pos = Some(target);
                self.held_block = c.inventory.held_stack();
                if !c.physical(Axis::Jump) && !body.flying {
                    self.start_task(c, target, true, false, 40);
                    let rotation = self.diagonal_rotation(c, self.reversed);
                    self.target_rotation = Some(rotation);
                    c.apply_fixed(rotation, speed(body.yaw360(), rotation[0], 15.0));
                }
                self.blocks_placed = 0;
                self.place_pos = None;
                self.move_timer.reset(c.now_ms);
                return false;
            }
            if !c.air(place) {
                self.blocks_placed += 1;
                let next = offset(place, 1, self.direction);
                let next_empty = c.air(next);
                if next_empty && f64::from(self.blocks_placed) < activation {
                    self.place_pos = Some(next);
                } else if !next_empty {
                    self.place_pos = None;
                    self.blocks_placed = 0;
                }
            } else if bridge::drifted(
                place,
                player_block,
                self.direction,
                activation,
                self.blocks_placed,
            ) {
                self.place_pos = None;
                self.blocks_placed = 0;
            }
        }
        true
    }

    /// `handleDirectionSwitch`: a strafe key turns the bridge; the player
    /// first walks to the new line, then the rotation turns.
    fn handle_direction_switch(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        if self.switching && self.task.as_ref().is_some_and(Done::get) {
            self.at_edge = true;
            self.task = None;
            self.target_pos = self.pending_pos;
            self.direction = self.pending_direction;
            self.reversed = self.pending_reversed;
            self.switching = false;
            c.keys.release_movement();
            self.move_timer.reset(c.now_ms);
            self.switch_timer.reset(c.now_ms);
            let yaw = c.body.yaw360();
            if self.direction < 5 {
                let point = placement_point(self.target(&c.body), self.direction, self.reversed);
                let rotation = self.placement_rotation(c, point, self.direction);
                Self::apply_rotation(c, rotation, speed(yaw, rotation[0], 15.0));
            } else {
                let rotation = self.diagonal_rotation(c, self.reversed);
                Self::apply_rotation(c, rotation, speed(yaw, rotation[0], 12.0));
            }
            self.rotation_pending = true;
            return true;
        }

        let left = c.physical(Axis::Left);
        let right = c.physical(Axis::Right);
        if !self.switching {
            let (left_edge, right_edge) = if self.key_idle {
                (left && !self.prev_left, right && !self.prev_right)
            } else {
                (!left && self.prev_left, !right && self.prev_right)
            };
            self.prev_left = left;
            self.prev_right = right;
            if self.key_idle && self.switch_timer.elapsed(c.now_ms, 0) {
                self.key_idle = !left_edge && !right_edge;
                if left_edge {
                    self.pending_direction = rotate_direction(self.direction, 1);
                } else if right_edge {
                    self.pending_direction = rotate_direction(self.direction, -1);
                }
            } else if !self.key_idle {
                self.key_idle = left_edge || right_edge;
                if left_edge {
                    self.pending_direction = rotate_direction(self.direction, -1);
                } else if right_edge {
                    self.pending_direction = rotate_direction(self.direction, 1);
                }
            } else {
                if left {
                    c.keys.set(Axis::Left, false);
                } else if right {
                    c.keys.set(Axis::Right, false);
                }
                return false;
            }
            self.switching = left_edge || right_edge;
        }

        if self.switching {
            self.task = None;
            let [x, _, z] = c.body.pos;
            let player_block = [x.floor(), c.placement_y(), z.floor()];
            if !(c.air(player_block)
                || Self::front_clear(c)
                || past_corner(&c.body, self.direction))
            {
                self.pending_reversed = self.should_reverse(&c.body, self.pending_direction);
                let pending = placement_point(
                    [player_block[0], player_block[2]],
                    self.pending_direction,
                    self.pending_reversed,
                );
                self.pending_pos = Some(pending);
                c.release_controls();
                let d = self.direction;
                if (d > 4 && !self.reversed && self.pending_direction == rotate_direction(d, -1))
                    || (d > 4 && self.reversed && self.pending_direction == rotate_direction(d, 1))
                {
                    self.task = Some(Done::completed());
                } else if (x - pending[0]).abs() > 0.15 || (z - pending[1]).abs() > 0.15 {
                    self.start_task(c, pending, true, false, 40);
                } else {
                    self.task = Some(Done::completed());
                }
            } else if !self.reversed {
                if left && self.is_at_edge(c, self.direction) {
                    c.keys.set(Axis::Left, false);
                } else if right {
                    c.keys.set(Axis::Right, false);
                }
            } else if right && self.is_at_edge(c, self.direction) {
                c.keys.set(Axis::Right, false);
            } else if left {
                c.keys.set(Axis::Left, false);
            }
        }
        false
    }

    /// `recoverIfStuck`: 800 ms without leaving the edge walks back onto
    /// the line.
    fn recover_if_stuck(&mut self, c: &mut Ctx<'_, '_>) -> bool {
        if !self.move_timer.elapsed(c.now_ms, 800) {
            return false;
        }
        let anchor = Self::anchor_target(c, self.direction);
        let target = placement_point([anchor[0], anchor[2]], self.direction, self.reversed);
        self.target_pos = Some(target);
        let rotation = self.diagonal_rotation(c, self.reversed);
        self.target_rotation = Some(rotation);
        c.apply_fixed(rotation, speed(c.body.yaw360(), rotation[0], 15.0));
        self.start_task(c, target, true, false, 40);
        self.at_edge = true;
        self.move_timer.reset(c.now_ms);
        true
    }

    /// `computeDiagonalRotation`: the yaw that walks a straight bridge with
    /// back and one strafe key, corrected toward the line.
    fn diagonal_rotation(&self, c: &Ctx<'_, '_>, reversed: bool) -> [f32; 2] {
        let body = &c.body;
        let yaw = f64::from(body.yaw);
        let [px, _, pz] = body.prev;
        let tp = self.target(body);
        let (reversed_yaw, forward_yaw) = match self.direction {
            6 => (135.0 + 20.0 * (tp[1] - pz), 45.0 + 20.0 * (tp[1] - pz)),
            8 => (-45.0 - 20.0 * (tp[1] - pz), -135.0 - 20.0 * (tp[1] - pz)),
            7 => (-135.0 - 20.0 * (tp[0] - px), 135.0 + 20.0 * (px - tp[0])),
            5 => (45.0 + 20.0 * (tp[0] - px), -45.0 - 20.0 * (px - tp[0])),
            _ => (yaw, yaw),
        };
        let yaw = if reversed { reversed_yaw } else { forward_yaw };
        let pitch = if self.move_timer.elapsed(c.now_ms, 300) {
            80.0
        } else {
            78.0
        };
        [yaw as f32, pitch]
    }

    /// `startMoveTaskNoTurn`.
    fn start_task(
        &mut self,
        c: &mut Ctx<'_, '_>,
        target: [f64; 2],
        block_updates: bool,
        restore: bool,
        limit: u32,
    ) {
        let mut task = Task::new(target);
        self.task_blocks_update = block_updates;
        task.restore = restore;
        self.task = Some(c.tasks.submit(task));
        self.task_ticks = 0;
        self.task_tick_limit = limit;
    }

    /// `updateRotation`.
    fn update_rotation(&mut self, c: &mut Ctx<'_, '_>) {
        if self.direction == 0 {
            self.target_rotation = Some([c.body.yaw, 90.0]);
            return;
        }
        let previous = self.target_rotation;
        let rotation = if self.direction < 5 {
            let point = placement_point(self.target(&c.body), self.direction, self.reversed);
            self.placement_rotation(c, point, self.direction)
        } else {
            self.diagonal_rotation(c, self.reversed)
        };
        self.target_rotation = Some(rotation);
        if !c.rotor.active() || previous != Some(rotation) {
            c.apply_fixed(rotation, speed(c.body.yaw360(), rotation[0], 15.0));
        }
    }

    /// `computePlacementRotation`: a diagonal bridge's view, nudged by the
    /// side of the path the player is on.
    fn placement_rotation(&self, c: &Ctx<'_, '_>, point: [f64; 2], direction: u8) -> [f32; 2] {
        let player = [c.body.pos[0], c.body.pos[2]];
        let offset = 0.1 * self.side_of_path(c, point, player, direction);
        let yaw = match direction {
            1 => 135.0 - offset,
            2 => -135.0 - offset,
            3 => -45.0 - offset,
            _ => 45.0 - offset,
        };
        [
            yaw,
            if self.move_timer.over(c.now_ms, 500) {
                83.0
            } else {
                81.0
            },
        ]
    }

    /// `isLookingAtPlacement`: a side face (a top face too while moving
    /// vertically or switching).
    fn looking_at_placement(&self, c: &Ctx<'_, '_>) -> bool {
        let Some(hit) = c.hit else {
            return false;
        };
        let vertical = c.body.motion[1];
        if !(-0.1..=0.1).contains(&vertical) || self.switching {
            return hit.side != 0;
        }
        hit.side > 1
    }

    /// `computeSideOfPath`: the cross product of the path and the player's
    /// offset. The reference adds the hotbar's block count to the distance
    /// before stepping along the path.
    fn side_of_path(
        &self,
        c: &Ctx<'_, '_>,
        point: [f64; 2],
        player: [f64; 2],
        direction: u8,
    ) -> f32 {
        let distance = ((point[0] - player[0]).powi(2) + (point[1] - player[1]).powi(2)).sqrt();
        let relative = (distance + f64::from(c.count_blocks(1, false))) as i32;
        let adjacent = offset([point[0], 0.0, point[1]], relative, direction);
        ((adjacent[0] - point[0]) * (player[1] - point[1])
            - (adjacent[2] - point[1]) * (player[0] - point[0])) as f32
    }

    /// `resetState` without the claims.
    fn reset_fields(&mut self) {
        self.activation_pending = true;
        self.target_pos = None;
        self.task = None;
        self.task_ticks = 0;
        self.target_rotation = None;
        self.prev_left = false;
        self.prev_right = false;
        self.switching = false;
        self.at_edge = true;
        self.direction = 0;
        self.pending_direction = 0;
        self.history.clear();
        self.held_block = None;
        self.pending_reversed = false;
        self.key_idle = true;
    }

    /// `resetState`.
    fn reset_state(&mut self, c: &mut Ctx<'_, '_>) {
        self.reset_fields();
        c.lock(false);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::Runtime;
    use super::super::super::rng::Rng;
    use super::super::movement_task::tests::Flat;
    use super::super::movement_task::{Keys, Tasks};
    use super::super::scaffold_bridge::tests::{options, stone};
    use super::super::scaffold_bridge::{Hit, Inventory, Locks, OWNER, Rotor};
    use super::*;

    #[test]
    fn directions_rotate_along_the_cycle_and_place_on_the_line() {
        assert_eq!(rotate_direction(5, 1), 4);
        assert_eq!(rotate_direction(5, -1), 3);
        assert_eq!(rotate_direction(3, 1), 5);
        assert_eq!(rotate_direction(6, 2), 7);
        assert_eq!(
            rotate_direction(0, -2),
            2,
            "an unknown direction starts before the cycle"
        );
        assert_eq!(placement_point([2.4, -3.1], 6, false), [2.8, -4.0 + 0.2]);
        assert_eq!(placement_point([2.4, -3.1], 1, true), [2.65, -4.0 + 0.35]);
        assert_eq!(placement_point([2.4, -3.1], 0, true), [2.4, -3.1]);
        let mut b = Body {
            pos: [0.6, 99.0, 0.4],
            ..Default::default()
        };
        assert!(past_corner(&b, 6) && !past_corner(&b, 7));
        // The block corner is the floor, so -X and -Z never pass it.
        assert!(!past_corner(&b, 8) && !past_corner(&b, 5));
        b.pos = [0.7, 99.0, 0.7];
        assert!(past_corner(&b, 1) && !past_corner(&b, 3));
    }

    struct Rig {
        runtime: Runtime,
        world: Flat,
        keys: Keys,
        tasks: Tasks,
        rotor: Rotor,
        locks: Locks,
        rng: Rng,
        options: super::super::scaffold::Options,
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
            Self {
                runtime: Runtime::default(),
                world,
                keys: Keys::default(),
                tasks: Tasks::default(),
                rotor: Rotor::default(),
                locks: Locks::default(),
                rng: Rng::new(1),
                options: options(),
            }
        }
        /// One tick facing +Z (the bridge goes -Z, direction 5).
        fn tick(
            &mut self,
            god: &mut GodBridge,
            now_ms: u64,
            pos: [f64; 3],
            user: [bool; 7],
            hit: Option<Hit>,
        ) {
            self.keys.observe(user);
            let mut inventory = Inventory {
                slots: vec![None; 36],
                ..Default::default()
            };
            inventory.slots[0] = stone(64);
            let body = Body {
                pos,
                prev: pos,
                on_ground: true,
                eye_height: 1.62,
                ..Default::default()
            };
            let mut rotation = self.runtime.rotation();
            let mut c = Ctx {
                now_ms,
                no_screen: true,
                body,
                inventory,
                hit,
                right_mouse: false,
                mouse: [0, 0],
                view: [0.0, 0.0],
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
            god.tick(&mut c);
            assert!(!c.click || hit.is_some());
        }
    }

    #[test]
    fn hand_placed_blocks_activate_the_bridge() {
        let mut rig = Rig::new();
        let mut god = GodBridge::default();
        god.enable();
        assert!(rig.runtime.rotation().claim(OWNER, false));
        let mut back = [false; 7];
        back[Axis::Back as usize] = true;
        let pos = [0.5, 100.0, 0.3];
        // The block behind the runway's end is air: the first placement.
        rig.tick(&mut god, 0, pos, back, None);
        assert_eq!(god.place_pos, Some([0.0, 99.0, -1.0]));
        assert!(god.activation_pending());
        // The user places it, then the next one.
        rig.world.solid.insert([0, 99, -1]);
        rig.tick(&mut god, 50, pos, back, None);
        assert_eq!(
            (god.blocks_placed, god.place_pos),
            (1, Some([0.0, 99.0, -2.0]))
        );
        rig.world.solid.insert([0, 99, -2]);
        rig.tick(&mut god, 100, pos, back, None);
        assert_eq!(god.blocks_placed, 2);
        rig.tick(&mut god, 150, pos, back, None);
        assert!(!god.activation_pending(), "two blocks activate the bridge");
        assert_eq!(
            god.target_pos,
            Some(placement_point([0.0, -2.0], 5, god.reversed))
        );
        assert!(rig.tasks.busy(), "a task walks onto the line");
        assert!(rig.rotor.active());
        assert!(rig.runtime.rotation().active(OWNER));
        assert!(!rig.locks.movement, "locks follow on the next tick");
        rig.tick(
            &mut god,
            200,
            pos,
            back,
            Some(Hit {
                pos: [0, 99, -2],
                side: 2,
            }),
        );
        assert!(rig.locks.movement && rig.locks.right_click);
    }

    #[test]
    fn releasing_back_or_losing_the_claim_returns_to_waiting() {
        let mut rig = Rig::new();
        let mut god = GodBridge::default();
        god.enable();
        god.activation_pending = false;
        god.direction = 5;
        god.target_pos = Some([0.2, -1.8]);
        // No claim: back to the edge sneak with the physical keys.
        rig.tick(&mut god, 0, [0.5, 100.0, 0.3], [false; 7], None);
        assert!(god.activation_pending());
        assert!(!rig.locks.movement);
        // With the claim and back released over a filled block, it stops.
        god.activation_pending = false;
        assert!(rig.runtime.rotation().claim(OWNER, false));
        rig.tick(&mut god, 50, [0.5, 100.0, 1.5], [false; 7], None);
        assert!(god.activation_pending());
        // Over air ahead it keeps bridging without the back key.
        god.activation_pending = false;
        rig.tick(&mut god, 100, [0.5, 100.0, 0.1], [false; 7], None);
        assert!(!god.activation_pending());
        assert!(rig.locks.movement);
    }

    #[test]
    fn looking_at_a_side_face_clicks_and_the_top_face_only_while_moving_vertically() {
        let rig = Rig::new();
        drop(rig);
        let mut god = GodBridge::default();
        let body = Body::default();
        let side = |side| Some(Hit { pos: [0; 3], side });
        let mut runtime = Runtime::default();
        let mut rotation = runtime.rotation();
        let options = options();
        let mut world = Flat::default();
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
            body,
            inventory: Inventory::default(),
            hit: side(1),
            right_mouse: false,
            mouse: [0, 0],
            view: [0.0, 0.0],
            options: &options,
            world: &mut world,
            keys: &mut keys,
            tasks: &mut tasks,
            rotation: &mut rotation,
            rotor: &mut rotor,
            locks: &mut locks,
            rng: &mut rng,
            select: None,
            click: false,
        };
        assert!(!god.looking_at_placement(&c), "top face on flat ground");
        c.hit = side(3);
        assert!(god.looking_at_placement(&c));
        c.hit = side(1);
        c.body.motion[1] = 0.2;
        assert!(god.looking_at_placement(&c));
        c.hit = side(0);
        assert!(!god.looking_at_placement(&c));
        c.body.motion[1] = 0.0;
        god.switching = true;
        c.hit = side(1);
        assert!(god.looking_at_placement(&c));
        c.hit = None;
        assert!(!god.looking_at_placement(&c));
    }
}
