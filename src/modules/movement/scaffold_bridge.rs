// The pieces GodBridge and TellyBridge share: the reference `Scaffold`
// helpers (block counts and slots, placement points, rotation speeds,
// `applyFixedRotation`/`applyPointRotation`/`releaseRotation`), the edge
// sneak that runs until a bridge activates (`ScaffoldEdgeSneakHelper`), the
// point controller TellyBridge aims with (`ScaffoldPointRotationController`)
// and one pre-tick of host state (`Ctx`).
use super::super::super::core::movement_input::Axis;
use super::super::super::core::rotation_runtime::Shared;
use super::super::super::inventory::item_limit::HeldItem;
use super::super::combat::rotation::{Claimant, Fixed, degrees_per_count};
use super::super::rng::Rng;
use super::movement_task::{Body, Keys, Tasks, World};
use super::scaffold::{self, Options, Sneak};

pub const OWNER: Claimant = Claimant::Scaffold;

/// An inventory stack.
#[derive(Clone, Debug, Default)]
pub struct Stack {
    pub item: HeldItem,
    pub count: u32,
}

/// The player's inventory as the modes read it.
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    /// 0..9 the hotbar (container slots 36..45), 9..36 the main inventory.
    pub slots: Vec<Option<Stack>>,
    pub offhand: Option<Stack>,
    pub selected: usize,
    pub creative: bool,
}

impl Inventory {
    pub fn slot(&self, slot: usize) -> Option<&Stack> {
        self.slots.get(slot).and_then(Option::as_ref)
    }
    /// `getHeldItem`.
    pub fn main_hand(&self) -> Option<&Stack> {
        self.slot(self.selected)
    }
    /// The main hand as the item limits read it.
    pub fn held(&self) -> HeldItem {
        self.main_hand().map_or_else(
            || HeldItem {
                slot: self.selected,
                ..Default::default()
            },
            |s| s.item.clone(),
        )
    }
    /// The main hand's identity (`ItemStack.equals`): the same slot still
    /// holding the same item.
    pub fn held_stack(&self) -> Option<HeldStack> {
        let id = self.main_hand()?.item.id.clone()?;
        Some(HeldStack {
            slot: self.selected,
            id,
        })
    }
}

/// A remembered main-hand stack (`heldBlock`, `activationItemStack`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldStack {
    pub slot: usize,
    pub id: String,
}

/// `isUsableBlockStack`/`isValidBlockStack`.
pub fn valid(o: &Options, stack: Option<&Stack>) -> bool {
    stack.is_some_and(|s| scaffold::valid_block(o, &s.item))
}

fn amount(inv: &Inventory, stack: &Stack, exact: bool) -> u32 {
    if inv.creative && !exact {
        64
    } else {
        stack.count
    }
}

/// `countHeldBlocks`: the offhand's blocks (26.3 has an offhand).
pub fn count_held(o: &Options, inv: &Inventory, exact: bool) -> u32 {
    inv.offhand
        .as_ref()
        .filter(|s| valid(o, Some(s)))
        .map_or(0, |s| amount(inv, s, exact))
}

/// `countBlocks(section, exact)`: the offhand plus the main hand (section
/// 0), the hotbar (1) or the whole inventory (2).
pub fn count_blocks(o: &Options, inv: &Inventory, section: u8, exact: bool) -> u32 {
    let slots = match section {
        0 => inv.selected..inv.selected + 1,
        1 => 0..9,
        _ => 0..36,
    };
    count_held(o, inv, exact)
        + slots
            .filter_map(|i| inv.slot(i).filter(|s| valid(o, Some(s))))
            .map(|s| amount(inv, s, exact))
            .sum::<u32>()
}

/// `findBlockHotbarSlot`.
pub fn find_block_slot(o: &Options, inv: &Inventory) -> Option<usize> {
    (0..9).find(|&i| valid(o, inv.slot(i)))
}

/// `findMatchingHotbarSlot`: the first hotbar slot with the same item.
pub fn find_matching_slot(inv: &Inventory, id: &str) -> Option<usize> {
    (0..9).find(|&i| {
        inv.slot(i)
            .is_some_and(|s| s.item.id.as_deref() == Some(id))
    })
}

/// `getPlacementY`: the block under the feet (the feet's own block on a
/// half-block surface).
pub fn placement_y(y: f64) -> f64 {
    if (y - y.trunc()).abs() == 0.5 {
        y.floor()
    } else {
        (y - 1.0).floor()
    }
}

/// `getCardinalDirection`: the horizontal direction behind the player (5
/// is -Z, 6 +X, 7 +Z, 8 -X).
pub fn cardinal(body: &Body) -> u8 {
    let yaw = f64::from((body.yaw360() + 180.0) % 360.0);
    if yaw > 315.0 || yaw <= 45.0 {
        7
    } else if yaw <= 135.0 {
        8
    } else if yaw <= 225.0 {
        5
    } else {
        6
    }
}

/// `offsetPosition`: `distance` blocks along `direction` (1..4 are the
/// diagonals).
pub fn offset(pos: [f64; 3], distance: i32, direction: u8) -> [f64; 3] {
    let d = f64::from(distance);
    let [x, y, z] = pos;
    match direction {
        1 => [x + d, y, z + d],
        2 => [x - d, y, z + d],
        3 => [x - d, y, z - d],
        4 => [x + d, y, z - d],
        6 => [x + d, y, z],
        8 => [x - d, y, z],
        7 => [x, y, z + d],
        5 => [x, y, z - d],
        _ => pos,
    }
}

/// `isAirBlockAt` at a floored position.
pub fn air(world: &mut dyn World, pos: [f64; 3]) -> bool {
    world.air(pos.map(|v| v.floor() as i32))
}

/// `computeElevatedPlacementPoint`: a point on the top face of the block
/// at `pos`, `lateral` across the path and randomly 0.45..0.65 along it.
pub fn elevated_point(rng: &mut Rng, pos: [f64; 3], lateral: f64, direction: u8) -> [f64; 3] {
    let [mut x, y, mut z] = pos;
    let r = 0.45 + rng.double() * 0.2;
    match direction {
        6 => (x, z) = (x + r, z + 0.5 - lateral),
        8 => (x, z) = (x + 1.0 - r, z + 0.5 + lateral),
        7 => (x, z) = (x + 0.5 + lateral, z + r),
        5 => (x, z) = (x + 0.5 - lateral, z + 1.0 - r),
        _ => {}
    }
    [x, y + 1.0, z]
}

/// `computePlacementAimPoint`: a point on the far side face of the block
/// at `pos`.
pub fn aim_point(pos: [f64; 3], lateral: f64, vertical: f64, direction: u8) -> [f64; 3] {
    let [mut x, y, mut z] = pos;
    match direction {
        6 => (x, z) = (x + 1.0, z + 0.5 - lateral),
        8 => z += 0.5 + lateral,
        7 => (x, z) = (x + 0.5 + lateral, z + 1.0),
        5 => x += 0.5 - lateral,
        _ => {}
    }
    [x, y + 0.5 + vertical, z]
}

/// `angularDistance` (not symmetric for angles past 360).
pub fn angular_distance(a: f64, b: f64) -> f64 {
    let direct = (a - b).abs();
    let wrapped = (360.0 - a + b).abs();
    if direct <= wrapped { direct } else { wrapped }
}

/// `updatePlacementHistory`: this tick's mouse movement in front; the
/// reference's removal loop keeps seven entries.
pub fn update_history(history: &mut Vec<i32>, mouse: [i32; 2]) {
    history.insert(0, mouse[0].abs());
    history.insert(0, mouse[1].abs());
    let mut i = 6;
    while i < history.len() {
        history.remove(i);
        i += 1;
    }
}

/// `sumPlacementHistory`.
pub fn history_sum(history: &[i32]) -> i32 {
    history.iter().sum()
}

/// `hasPlacementDrifted`, with the reference's operator precedence: a
/// 4-block Z drift counts for every direction.
pub fn drifted(
    place: [f64; 3],
    player: [f64; 3],
    direction: u8,
    activation: f64,
    placed: i32,
) -> bool {
    let perpendicular = if direction.is_multiple_of(2) { 2 } else { 0 };
    if direction > 4 && place[perpendicular] != player[perpendicular] {
        return true;
    }
    if (direction < 5 && (place[0] - player[0]).abs() >= 4.0) || (place[2] - player[2]).abs() >= 4.0
    {
        return true;
    }
    if place[1] != player[1] {
        return true;
    }
    let origin = offset(place, -placed, direction);
    let d = [
        origin[0] - player[0],
        origin[1] - player[1],
        origin[2] - player[2],
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > activation + 2.0
}

/// `computeRotationSpeed`: faster for a larger yaw change.
pub fn rotation_speed(body: &Body, rotation: [f32; 2]) -> f32 {
    speed(body.yaw360(), rotation[0], 8.0)
}

/// `min(2 + angularDistance(yaw, target) / divisor, 12)`.
pub fn speed(yaw360: f32, target: f32, divisor: f64) -> f32 {
    (2.0 + angular_distance(f64::from(yaw360), f64::from(target)) / divisor).min(12.0) as f32
}

/// `getDirectionRotationSpeed` from the yaw `yaw360`.
pub fn direction_rotation_speed(yaw360: f32, direction: u8) -> f32 {
    let target = match direction {
        6 => 90.0,
        8 => 270.0,
        7 => 0.0,
        5 => 180.0,
        _ => yaw360,
    };
    speed(yaw360, target, 8.0)
}

/// `RotationUtil.c()` of a raw yaw.
fn yaw360(yaw: f32) -> f32 {
    (yaw % 360.0 + 360.0) % 360.0
}

fn wrap180(angle: f64) -> f64 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// `RotationVectorMath.H`: the rotation from `eye` to `target`. With
/// `normalize`, a target more than 90 degrees off `yaw` gets the
/// reference's pitch of 90 radians, which the controller wraps.
pub fn point_rotation(eye: [f64; 3], target: [f64; 3], yaw: f32, normalize: bool) -> [f32; 2] {
    const DEGREES: f64 = 57.29577951308232;
    let d = [eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]];
    let yaw_radians = d[0].atan2(-d[2]);
    let mut pitch = d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt());
    let off = wrap180(((yaw_radians * DEGREES - f64::from(yaw)) % 360.0).trunc()).abs();
    if normalize && off > 90.0 {
        pitch = 90.0;
    }
    [(yaw_radians * DEGREES) as f32, (pitch * DEGREES) as f32]
}

/// `RotationUtil.p(facingForDirection(direction), placement)`: the block
/// at last tick's position (`pos - motion`) is past the placement block
/// along the bridge.
pub fn passed(direction: u8, placement: [f64; 3], body: &Body) -> bool {
    let x = (body.pos[0] - body.motion[0]).floor();
    let z = (body.pos[2] - body.motion[2]).floor();
    let (cx, cz) = (placement[0].floor(), placement[2].floor());
    match direction {
        5 => z < cz,
        6 => x > cx,
        7 => z > cz,
        8 => x < cx,
        _ => false,
    }
}

fn eye(body: &Body) -> [f64; 3] {
    [body.pos[0], body.pos[1] + body.eye_height, body.pos[2]]
}

/// `ScaffoldPointRotationController`: aims at a world point; pitch always
/// moves, yaw only once the player is past the placement block (with the
/// direction's speed from then on). It never completes.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaffoldPoint {
    pub fixed: Fixed,
    pub point: [f64; 3],
    direction: u8,
    placement: [f64; 3],
    speed_initialized: bool,
}

impl ScaffoldPoint {
    /// `new ScaffoldPointRotationController(point, direction, placement)`.
    pub fn new(
        view: [f32; 2],
        body: &Body,
        point: [f64; 3],
        direction: u8,
        placement: [f64; 3],
    ) -> Self {
        Self {
            fixed: Fixed::new(point_rotation(eye(body), point, view[0], true), 0.0),
            point,
            direction,
            placement,
            speed_initialized: false,
        }
    }
    /// `update` then `applyPendingMovement`.
    pub fn update(&mut self, rotation: [f32; 2], per_count: f32, body: &Body) -> [f32; 2] {
        self.fixed
            .set_target(point_rotation(eye(body), self.point, rotation[0], true));
        self.fixed.update_axis(1, rotation, per_count);
        if passed(self.direction, self.placement, body) {
            if !self.speed_initialized {
                self.fixed.speed = direction_rotation_speed(yaw360(rotation[0]), self.direction);
                self.speed_initialized = true;
            }
            self.fixed.update_axis(0, rotation, per_count);
        }
        self.fixed.apply(rotation, per_count)
    }
    /// One client tick's updates.
    pub fn tick(
        &mut self,
        rotation: [f32; 2],
        sensitivity: f64,
        updates: u32,
        body: &Body,
    ) -> [f32; 2] {
        let per_count = degrees_per_count(sensitivity).max(1e-4);
        (0..updates).fold(rotation, |r, _| self.update(r, per_count, body))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Fixed,
    Point,
}

/// Scaffold's `rotationController` field: the controller it last set. It
/// is written through the shared rotation only while the manager still
/// runs it (a replaced controller object takes no effect).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rotor {
    kind: Option<Kind>,
    target: [f32; 2],
}

impl Rotor {
    pub fn active(&self) -> bool {
        self.kind.is_some()
    }
    /// `getTargetYaw/Pitch`.
    pub fn target(&self) -> [f32; 2] {
        self.target
    }
    /// `applyFixedRotation`.
    pub fn apply_fixed(&mut self, s: &mut Shared<'_>, rotation: [f32; 2], speed: f32) {
        if self.kind == Some(Kind::Point) {
            self.release(s);
        }
        self.set_fixed(s, rotation, speed, |f| f.tolerance = 0.5);
    }
    /// BlatantScaffoldMode's own `applyRotation`: no tolerance, scaled axes.
    pub fn apply_mode(&mut self, s: &mut Shared<'_>, rotation: [f32; 2], speed: f32) {
        self.set_fixed(s, rotation, speed, |f| {
            f.tolerance = 0.0;
            f.scale_axes = true;
        });
    }
    fn set_fixed(
        &mut self,
        s: &mut Shared<'_>,
        rotation: [f32; 2],
        speed: f32,
        tune: impl FnOnce(&mut Fixed),
    ) {
        let speed = speed.clamp(2.0, 12.0);
        self.target = rotation;
        if self.kind.is_none() {
            let mut fixed = Fixed::new(rotation, speed);
            fixed.clamp = true;
            fixed.linear_acceleration = true;
            fixed.retain = true;
            tune(&mut fixed);
            s.visible(OWNER, fixed);
            self.kind = Some(Kind::Fixed);
        } else if let Some(fixed) = s.visible_mut(OWNER) {
            fixed.speed = speed;
            fixed.set_target(rotation);
        }
    }
    /// `applyPointRotation`: the direction and placement stay from the
    /// controller's construction.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_point(
        &mut self,
        s: &mut Shared<'_>,
        view: [f32; 2],
        body: &Body,
        point: [f64; 3],
        speed: f32,
        direction: u8,
        placement: [f64; 3],
    ) {
        if self.kind == Some(Kind::Fixed) {
            self.release(s);
        }
        let speed = speed.clamp(2.0, 12.0);
        if self.kind.is_none() {
            let mut p = ScaffoldPoint::new(view, body, point, direction, placement);
            p.fixed.speed = speed;
            p.fixed.clamp = true;
            p.fixed.tolerance = 0.0;
            p.fixed.retain = true;
            p.fixed.linear_acceleration = true;
            self.target = p.fixed.target;
            s.point(OWNER, p);
            self.kind = Some(Kind::Point);
        } else if let Some(p) = s.point_mut(OWNER) {
            p.fixed.speed = speed;
            p.point = point;
        }
    }
    /// `releaseRotation`.
    pub fn release(&mut self, s: &mut Shared<'_>) {
        if self.kind.take().is_some() {
            s.release_controller(OWNER);
        }
    }
}

/// `TimerUtil` as a reset time; never reset counts as long elapsed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timer(Option<u64>);

impl Timer {
    pub fn reset(&mut self, now_ms: u64) {
        self.0 = Some(now_ms);
    }
    /// `hasTimeElapsed`.
    pub fn elapsed(&self, now_ms: u64, ms: u64) -> bool {
        self.0.is_none_or(|at| now_ms.saturating_sub(at) >= ms)
    }
    /// `getLastMS` against `ms`: more than `ms` since the reset.
    pub fn over(&self, now_ms: u64, ms: u64) -> bool {
        self.0.is_none_or(|at| now_ms.saturating_sub(at) > ms)
    }
}

/// `ScaffoldEdgeSneakHelper`: Legit's edge sneak while a bridge waits to
/// activate; it also takes the rotation claim at the edge. Its "Sneak
/// delay" belongs to an unlisted sub-module and stays at 100..200 ms.
#[derive(Debug)]
pub struct EdgeSneak {
    rng: Rng,
    timer: Timer,
    delay_ms: u64,
}

impl Default for EdgeSneak {
    fn default() -> Self {
        Self {
            rng: Rng::new(0xed9e_5ea4),
            timer: Timer::default(),
            delay_ms: 150,
        }
    }
}

/// One pre-entity-update for the edge sneak.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeTick {
    pub now_ms: u64,
    pub no_screen: bool,
    /// `Scaffold.canActivate`.
    pub can_activate: bool,
    pub on_ladder: bool,
    /// Pitch check on and the pitch under the threshold.
    pub pitch_blocked: bool,
    /// Physical forward input: 1, 0 or -1.
    pub forward: f32,
    pub on_ground: bool,
    pub sneaking: bool,
    /// No collision under the box shrunk by 0.2, moved by the motion.
    pub edge: bool,
}

impl EdgeSneak {
    const DELAY: (f64, f64) = (100.0, 200.0);
    /// `onEnable`.
    pub fn enable(&mut self, now_ms: u64) {
        self.timer.reset(now_ms);
        self.delay_ms = self.rng.range(Self::DELAY) as u64;
    }
    /// `onPreEntityUpdate`; the press lasts this tick (`onPostTick` puts
    /// the key back).
    pub fn pre(&mut self, t: &EdgeTick, s: &mut Shared<'_>) -> Option<Sneak> {
        if !t.no_screen || !t.can_activate {
            return None;
        }
        if t.on_ladder || t.pitch_blocked {
            s.claim.release(OWNER);
            return None;
        }
        let mut sneak = false;
        if t.forward <= 0.0 && t.on_ground && t.edge {
            sneak = true;
            s.claim(OWNER, false);
        }
        if s.owned_by(OWNER) && (t.forward > 0.0 || (!sneak && self.timer.elapsed(t.now_ms, 500))) {
            s.claim.release(OWNER);
        }
        let mut keep_timer = false;
        if !sneak && !self.timer.elapsed(t.now_ms, self.delay_ms) && self.delay_ms > 30 {
            sneak = true;
            keep_timer = true;
        }
        if !(sneak && t.on_ground) {
            // Releasing a key that is not down leaves the user's state.
            return None;
        }
        if !t.sneaking {
            self.delay_ms = self.rng.range(Self::DELAY) as u64;
        }
        if !keep_timer {
            self.timer.reset(t.now_ms);
        }
        Some(Sneak::Press)
    }
}

/// The block the crosshair's block-reach ray hit (`normalReachRayTrace`);
/// `side` is `EnumFacing` order (down, up, north, south, west, east).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub pos: [i32; 3],
    pub side: u8,
}

/// `MovementInputLock` and `RightClickUseControlClaim` while bridging.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Locks {
    pub movement: bool,
    pub right_click: bool,
}

/// `releaseControls`: the rotation, the active task and the key bindings.
pub fn release_controls(rotor: &mut Rotor, s: &mut Shared<'_>, tasks: &mut Tasks, keys: &mut Keys) {
    rotor.release(s);
    tasks.cancel_active();
    keys.release_all();
    keys.restore_physical();
}

/// One pre-tick of host state with the shared pieces a bridge mode drives.
pub struct Ctx<'a, 's> {
    pub now_ms: u64,
    pub no_screen: bool,
    pub body: Body,
    pub inventory: Inventory,
    pub hit: Option<Hit>,
    pub right_mouse: bool,
    /// This tick's mouse movement in counts (`Mouse.getDX/DY`).
    pub mouse: [i32; 2],
    /// The view, for a new point controller.
    pub view: [f32; 2],
    pub options: &'a Options,
    pub world: &'a mut dyn World,
    pub keys: &'a mut Keys,
    pub tasks: &'a mut Tasks,
    pub rotation: &'a mut Shared<'s>,
    pub rotor: &'a mut Rotor,
    pub locks: &'a mut Locks,
    pub rng: &'a mut Rng,
    /// The hotbar slot selected this tick.
    pub select: Option<usize>,
    /// `use.onTick(1)`: one right click this tick.
    pub click: bool,
}

impl Ctx<'_, '_> {
    pub fn air(&mut self, pos: [f64; 3]) -> bool {
        air(self.world, pos)
    }
    /// The player's box shrunk by `shrink` and moved by the motion and one
    /// block down touches nothing.
    pub fn clear(&mut self, shrink: f64) -> bool {
        self.world
            .clear_below([self.body.pos[0], self.body.pos[2]], shrink)
    }
    pub fn placement_y(&self) -> f64 {
        placement_y(self.body.pos[1])
    }
    /// `selectHotbarSlot`.
    pub fn select(&mut self, slot: usize) {
        self.inventory.selected = slot;
        self.select = Some(slot);
    }
    pub fn owned(&self) -> bool {
        self.rotation.owned_by(OWNER)
    }
    pub fn count_blocks(&self, section: u8, exact: bool) -> u32 {
        count_blocks(self.options, &self.inventory, section, exact)
    }
    pub fn find_block_slot(&self) -> Option<usize> {
        find_block_slot(self.options, &self.inventory)
    }
    pub fn main_hand_valid(&self) -> bool {
        valid(self.options, self.inventory.main_hand())
    }
    pub fn physical(&self, axis: Axis) -> bool {
        self.keys.user(axis)
    }
    /// `releaseControls`.
    pub fn release_controls(&mut self) {
        release_controls(self.rotor, self.rotation, self.tasks, self.keys);
    }
    pub fn release_rotation(&mut self) {
        self.rotor.release(self.rotation);
    }
    pub fn apply_fixed(&mut self, rotation: [f32; 2], speed: f32) {
        self.rotor.apply_fixed(self.rotation, rotation, speed);
    }
    pub fn lock(&mut self, on: bool) {
        self.locks.movement = on;
        self.locks.right_click = on;
    }
    /// Select the remembered stack's item again if the hand changed, else
    /// the first block slot (the shared tail of `ensureBlockSelected` and
    /// `ensureBlockSlot`).
    pub fn reselect(&mut self, remembered: Option<&HeldStack>, slot: usize) {
        let Some(held) = remembered else { return };
        if self.inventory.held_stack().as_ref() == Some(held) {
            return;
        }
        match find_matching_slot(&self.inventory, &held.id) {
            Some(m) => self.select(m),
            None if self.inventory.selected != slot => self.select(slot),
            None => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::super::super::core::Runtime;
    use super::*;

    pub(crate) fn stone(count: u32) -> Option<Stack> {
        Some(Stack {
            item: HeldItem {
                id: Some("stone".into()),
                display_name: "Stone".into(),
                block: true,
                ..Default::default()
            },
            count,
        })
    }

    pub(crate) fn options() -> Options {
        let mut s = super::super::super::super::core::settings::Settings::default();
        for d in scaffold::definitions() {
            s.define(d).unwrap();
        }
        Options::read(&s)
    }

    #[test]
    fn counts_slots_and_placement_height() {
        let o = options();
        let mut inv = Inventory {
            slots: vec![None; 36],
            ..Default::default()
        };
        inv.slots[3] = stone(10);
        inv.slots[20] = stone(30);
        inv.offhand = stone(5);
        inv.selected = 3;
        assert_eq!(count_blocks(&o, &inv, 0, true), 15);
        assert_eq!(count_blocks(&o, &inv, 1, true), 15);
        assert_eq!(count_blocks(&o, &inv, 2, true), 45);
        inv.creative = true;
        assert_eq!(count_blocks(&o, &inv, 1, false), 128);
        assert_eq!(find_block_slot(&o, &inv), Some(3));
        assert_eq!(find_matching_slot(&inv, "stone"), Some(3));
        assert_eq!(placement_y(100.0), 99.0);
        assert_eq!(placement_y(100.5), 100.0);
        assert_eq!(placement_y(100.42), 99.0);
    }

    #[test]
    fn directions_offsets_and_history() {
        let mut b = Body::default();
        assert_eq!(cardinal(&b), 5, "facing +Z the back is -Z");
        b.yaw = 90.0;
        assert_eq!(cardinal(&b), 6);
        b.yaw = -90.0;
        assert_eq!(cardinal(&b), 8);
        b.yaw = 180.0;
        assert_eq!(cardinal(&b), 7);
        assert_eq!(offset([0.0; 3], 2, 3), [-2.0, 0.0, -2.0]);
        assert_eq!(offset([0.0; 3], -1, 6), [-1.0, 0.0, 0.0]);
        let mut h = Vec::new();
        for _ in 0..6 {
            update_history(&mut h, [3, -1]);
        }
        assert_eq!(h.len(), 7);
        assert_eq!(history_sum(&h), 4 * 3 + 3);
        assert_eq!(angular_distance(350.0, 10.0), 20.0);
        assert_eq!(angular_distance(10.0, 350.0), 340.0);
        assert!(direction_rotation_speed(0.0, 6) > 12.0 - 1e-6);
        assert!((direction_rotation_speed(80.0, 6) - 3.25).abs() < 1e-6);
    }

    #[test]
    fn drift_checks_follow_the_reference_precedence() {
        let p = [0.0, 99.0, 0.0];
        assert!(drifted(p, [0.0, 99.0, 1.0], 6, 2.0, 0), "off the path");
        assert!(
            drifted(p, [0.0, 99.0, 4.0], 7, 9.0, 0),
            "4 blocks of z for any direction"
        );
        assert!(!drifted(p, [3.0, 99.0, 0.0], 6, 2.0, 0));
        let ahead = [5.0, 99.0, 0.0];
        assert!(drifted(ahead, [0.0, 99.0, 0.0], 6, 2.0, 0));
        assert!(
            !drifted(ahead, [0.0, 99.0, 0.0], 6, 2.0, 2),
            "measured from the first placement"
        );
        assert!(drifted(p, [0.0, 100.0, 0.0], 6, 2.0, 0));
    }

    #[test]
    fn point_rotation_matches_minecraft_axes_and_the_pitch_quirk() {
        let [yaw, pitch] = point_rotation([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.0, true);
        assert!(yaw.abs() < 1e-4 && (pitch - 45.0).abs() < 1e-4);
        let [yaw, _] = point_rotation([0.0; 3], [-1.0, 0.0, 0.0], 90.0, true);
        assert!((yaw - 90.0).abs() < 1e-4);
        let [_, pitch] = point_rotation([0.0; 3], [0.0, 0.0, -1.0], 0.0, true);
        assert!((pitch - 5156.62).abs() < 0.01, "{pitch}");
    }

    #[test]
    fn point_controller_turns_yaw_only_past_the_placement() {
        let body = Body {
            pos: [0.5, 100.0, 0.5],
            eye_height: 1.62,
            ..Default::default()
        };
        // Bridging -Z (5) facing +Z; the aim point is the block below.
        let point = [0.5, 99.5, 0.0];
        let mut p = ScaffoldPoint::new([0.0, 0.0], &body, point, 5, [0.0, 99.0, 0.0]);
        p.fixed.speed = 12.0;
        p.fixed.clamp = true;
        p.fixed.linear_acceleration = true;
        let r = p.tick([20.0, 0.0], 0.5, 50, &body);
        assert_eq!(r[0], 20.0, "the yaw waits");
        assert!(r[1] > 20.0, "{r:?}");
        let past = Body {
            pos: [0.5, 100.0, -0.5],
            ..body
        };
        let r = p.tick(r, 0.5, 50, &past);
        assert!(r[0] < 20.0, "{r:?}");
        assert!(!p.fixed.complete());
    }

    #[test]
    fn edge_sneak_presses_at_the_edge_and_takes_the_claim() {
        let mut r = Runtime::default();
        let mut s = r.rotation();
        let mut e = EdgeSneak::default();
        e.enable(0);
        let mut t = EdgeTick {
            now_ms: 1000,
            no_screen: true,
            can_activate: true,
            forward: -1.0,
            on_ground: true,
            edge: true,
            ..Default::default()
        };
        assert_eq!(e.pre(&t, &mut s), Some(Sneak::Press));
        assert!(s.owned_by(OWNER));
        t.edge = false;
        t.now_ms = 1010;
        assert_eq!(e.pre(&t, &mut s), Some(Sneak::Press), "held for the delay");
        t.now_ms = 1700;
        assert_eq!(e.pre(&t, &mut s), None);
        assert!(!s.owned_by(OWNER), "released 500 ms after the edge");
        t.on_ladder = true;
        t.edge = true;
        assert_eq!(e.pre(&t, &mut s), None);
    }
}
