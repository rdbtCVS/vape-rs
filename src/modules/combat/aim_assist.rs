// AimAssist (reference `AimAssist` + `AimAssistRotationSubModule`, Simple mode).
//
// The reference runs its worker `tick()` on a thread roughly every
// millisecond and applies the accumulated mouse steps at each render frame.
// Here the host calls `frame` once per frame, before rendering, with the
// elapsed monotonic time: the worker step runs once per elapsed millisecond
// (capped), then the render-frame application returns mouse deltas for the
// host's own `Entity.turn`. Adaptive mode is not ported yet.
use super::super::super::core::lifecycle::EntityHandle;
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::super::targeting::entities::{
    self, EntityView, PlayerView, int_yaw_difference, on_left, pitch_difference, yaw_difference,
};
use super::super::super::targeting::filter::{self, People, TargetFilter};
use super::super::super::targeting::{self, Candidate, SortMode};
use serde_json::{Value, json};

pub const ID: &str = "combat.aim-assist";
/// Worker steps run per frame at most; a long hitch must not spin the view.
const MAX_STEPS: u32 = 100;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let list = |k: &str, label: &str, default: Value| {
        Definition::new(
            &key(k),
            label,
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            default,
        )
    };
    let mut out = vec![Definition::choice(
        &key("mode"),
        "Mode",
        &["Simple"],
        "Simple",
    )];
    out.extend(filter::definitions(ID));
    out.extend([
        Definition::boolean(&key("require-mouse"), "Require mouse down", true),
        Definition::boolean(&key("strafe-increase"), "Strafe increase", false),
        Definition::boolean(&key("check-block-break"), "Check block break", false),
        Definition::boolean(&key("break-whitelist"), "Break blocks whitelist", false)
            .when(&key("check-block-break"), json!(true)),
        list("break-items", "Items", json!(["pickaxes", "shovels"]))
            .when(&key("check-block-break"), json!(true))
            .when(&key("break-whitelist"), json!(true)),
        Definition::boolean(&key("aim-vertically"), "Aim vertically", false),
        Definition::number(
            &key("vertical-speed"),
            "Vertical speed",
            1.0,
            5.0,
            10.0,
            0.1,
        )
        .when(&key("aim-vertically"), json!(true)),
        Definition::number(
            &key("horizontal-speed"),
            "Horizontal speed",
            1.0,
            5.0,
            10.0,
            0.1,
        ),
        Definition::number(&key("max-angle"), "Max angle", 1.0, 180.0, 360.0, 1.0),
        Definition::number(&key("distance"), "Distance", 1.0, 5.0, 8.0, 0.1),
        Definition::boolean(&key("limit-items"), "Limit to items", false),
        list("allowed-items", "Allowed Items", json!(["swords"]))
            .when(&key("limit-items"), json!(true)),
        Definition::choice(
            &key("target-area"),
            "Target area",
            &["Center", "Closest"],
            "Center",
        ),
        Definition::choice(
            &key("target-mode"),
            "Target mode",
            &["Yaw", "Distance", "Armor", "Threat", "Health"],
            "Yaw",
        ),
    ]);
    out
}

#[derive(Clone, Debug)]
pub struct Options {
    pub filter: TargetFilter,
    pub require_mouse: bool,
    pub strafe_increase: bool,
    pub check_block_break: bool,
    pub break_whitelist: bool,
    pub break_items: Vec<String>,
    pub aim_vertically: bool,
    pub vertical_speed: f32,
    pub horizontal_speed: f32,
    pub max_angle: f64,
    pub distance: f64,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
    pub closest_area: bool,
    pub target_mode: SortMode,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let boolean = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        let number = |k: &str| get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let text = |k: &str| get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let list = |k: &str| {
            get(k)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        };
        Self {
            filter: TargetFilter::read(settings, ID),
            require_mouse: boolean("require-mouse"),
            strafe_increase: boolean("strafe-increase"),
            check_block_break: boolean("check-block-break"),
            break_whitelist: boolean("break-whitelist"),
            break_items: list("break-items"),
            aim_vertically: boolean("aim-vertically"),
            vertical_speed: number("vertical-speed") as f32,
            horizontal_speed: number("horizontal-speed") as f32,
            max_angle: number("max-angle"),
            distance: number("distance"),
            limit_items: boolean("limit-items"),
            allowed_items: list("allowed-items"),
            closest_area: text("target-area") == "Closest",
            target_mode: match text("target-mode").as_str() {
                "Distance" => SortMode::Distance,
                "Armor" => SortMode::Armor,
                "Threat" => SortMode::Threat,
                "Health" => SortMode::Health,
                _ => SortMode::Yaw,
            },
        }
    }
    /// The module-list suffix (`getDetailedSuffix`): horizontal speed, no fraction.
    pub fn suffix(&self) -> String {
        format!("{:.0}", self.horizontal_speed)
    }
}

/// One frame of host state.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub player: PlayerView,
    pub entities: Vec<EntityView>,
    pub held: HeldItem,
    pub attack_down: bool,
    /// The crosshair targets a block (`RayTraceResult` type BLOCK).
    pub aiming_block: bool,
    /// No screen is open and gameplay input is enabled.
    pub playing: bool,
    /// Another module holds the shared movement-input claim.
    pub movement_locked: bool,
    /// `Options.mouseSensitivity`, 0..=1.
    pub sensitivity: f32,
    pub elapsed_ms: u32,
}

#[derive(Debug)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// `Random.nextInt(bound)`.
    fn int(&mut self, bound: i32) -> i32 {
        (self.next() % bound.max(1) as u64) as i32
    }
    /// `MathUtil.randomExclusiveUpper(r, lo, hi)`.
    fn between(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.int(hi - lo)
    }
    /// `MathUtil.randomRange(r, lo, hi)`.
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() >> 11) as f64 / (1u64 << 53) as f64 * (hi - lo)
    }
}

#[derive(Debug)]
pub struct AimAssist {
    target: Option<EntityHandle>,
    block_break_cooldown: i32,
    retarget_counter: i32,
    swap_tick_counter: i32,
    sample_counter: i32,
    random_offset: (i32, i32),
    drift: (i32, i32),
    drift_timer: f64,
    accumulator: (f32, f32),
    horizontal_velocity: f32,
    horizontal_velocity_buffer: f32,
    vertical_velocity: f32,
    vertical_velocity_buffer: f32,
    yaw_boost: f32,
    pitch_boost: f32,
    prev_target: (f64, f64),
    prev_on_left: bool,
    prev_above: bool,
    last_angle_diff: f64,
    random: Rng,
    shared_random: Rng,
}

impl Default for AimAssist {
    fn default() -> Self {
        Self::new(0x9e37_79b9_7f4a_7c15)
    }
}

impl AimAssist {
    pub fn new(seed: u64) -> Self {
        Self {
            target: None,
            block_break_cooldown: 0,
            retarget_counter: 0,
            swap_tick_counter: 0,
            sample_counter: 0,
            random_offset: (0, 0),
            drift: (0, 0),
            drift_timer: 0.0,
            accumulator: (0.0, 0.0),
            horizontal_velocity: 0.0,
            horizontal_velocity_buffer: 0.0,
            vertical_velocity: 0.0,
            vertical_velocity_buffer: 0.0,
            yaw_boost: 0.0,
            pitch_boost: 0.0,
            prev_target: (0.0, 0.0),
            prev_on_left: false,
            prev_above: false,
            last_angle_diff: 0.0,
            random: Rng(seed.max(1)),
            shared_random: Rng(seed.rotate_left(17).max(1)),
        }
    }
    pub fn target(&self) -> Option<EntityHandle> {
        self.target
    }
    /// `onDisable` and session changes.
    pub fn reset(&mut self) {
        self.target = None;
        self.reset_rotation_state();
    }
    fn reset_rotation_state(&mut self) {
        self.accumulator = (0.0, 0.0);
        self.random_offset = (0, 0);
        self.drift = (0, 0);
    }

    /// Run this frame's worker steps and return the `(x, y)` mouse delta for
    /// `Entity.turn` (0.15 degrees per unit; +y looks down).
    pub fn frame(&mut self, options: &Options, frame: &Frame, people: &impl People) -> (f64, f64) {
        for _ in 0..frame.elapsed_ms.min(MAX_STEPS) {
            self.tick(options, frame, people);
        }
        self.render(frame)
    }

    /// `onPreRenderTick`.
    fn render(&mut self, frame: &Frame) -> (f64, f64) {
        if self.target.is_none() {
            return (0.0, 0.0);
        }
        self.accumulator.0 += self.drift.0 as f32;
        self.accumulator.1 += self.drift.1 as f32;
        let steps = (self.accumulator.0 as i32, self.accumulator.1 as i32);
        let rest = (
            self.accumulator.0 - steps.0 as f32,
            self.accumulator.1 - steps.1 as f32,
        );
        let base = frame.sensitivity * 0.6 + 0.2;
        let scale = base * base * base * 8.0;
        let delta = (steps.0 as f32 * scale, steps.1 as f32 * scale);
        self.accumulator = rest;
        self.drift = (0, 0);
        (f64::from(delta.0), f64::from(delta.1))
    }

    fn entity<'a>(&self, frame: &'a Frame) -> Option<&'a EntityView> {
        let handle = self.target?;
        frame.entities.iter().find(|e| e.handle == handle)
    }

    fn has_required_item(options: &Options, frame: &Frame) -> bool {
        !options.limit_items || item_limit::matches(&options.allowed_items, &frame.held)
    }

    fn can_aim(&mut self, options: &Options, frame: &Frame) -> bool {
        if frame.movement_locked {
            return false;
        }
        let mut check = options.check_block_break;
        if check && options.break_whitelist {
            check = item_limit::matches(&options.break_items, &frame.held);
        }
        if check {
            if frame.aiming_block {
                self.block_break_cooldown = 250;
                return false;
            }
            if self.block_break_cooldown > 0 {
                self.block_break_cooldown -= 1;
            }
            if self.block_break_cooldown > 0 {
                return false;
            }
        }
        Self::has_required_item(options, frame)
    }

    /// `AimAssist.isValidTarget`.
    pub fn valid(options: &Options, frame: &Frame, e: &EntityView, people: &impl People) -> bool {
        if !e.alive() {
            return false;
        }
        if frame.player.distance(e.tick_position) >= f64::from(options.distance as i32) {
            return false;
        }
        if int_yaw_difference(&frame.player, e.tick_position) > options.max_angle as i32 / 2 {
            return false;
        }
        if people.friend(&e.name) || e.vehicle {
            return false;
        }
        Self::has_required_item(options, frame) && options.filter.valid(e, people)
    }

    /// `AimAssist.findBestTarget`.
    pub fn best(options: &Options, frame: &Frame, people: &impl People) -> Option<EntityHandle> {
        let candidates: Vec<Candidate> = frame
            .entities
            .iter()
            .filter(|e| Self::valid(options, frame, e, people))
            .map(|e| Candidate {
                handle: e.handle,
                distance: frame.player.distance(e.tick_position),
                yaw: f64::from(int_yaw_difference(&frame.player, e.tick_position)),
                health: f64::from(e.health),
                armor: e.armor,
                threat: e.threat,
                player: e.class == entities::EntityClass::Player,
                friend: false,
                enemy: false,
                same_team: false,
                visible: true,
                alive: true,
            })
            .collect();
        let epoch = candidates.first()?.handle.epoch;
        targeting::select(
            &candidates,
            targeting::Filter {
                epoch,
                max_distance: f64::MAX,
                max_yaw: f64::MAX,
                exclude_friends: false,
                enemies_only: false,
                exclude_team: false,
                require_visible: false,
            },
            options.target_mode,
        )
    }

    /// The worker thread's `tick()`.
    fn tick(&mut self, options: &Options, frame: &Frame, people: &impl People) {
        if !self.can_aim(options, frame) {
            self.reset_rotation_state();
            return;
        }
        if self.target.is_some() && self.entity(frame).is_none() {
            self.target = None;
        }
        if options.require_mouse && !frame.attack_down {
            self.target = None;
            self.reset_rotation_state();
            return;
        }
        if let Some(e) = self.entity(frame)
            && (!e.alive() || frame.player.distance(e.tick_position) > options.distance)
        {
            self.reset_rotation_state();
            self.target = None;
        }
        if !options.require_mouse || self.target.is_none() {
            let candidate = Self::best(options, frame, people);
            if !options.require_mouse {
                self.retarget_counter += 1;
                let invalid = self
                    .entity(frame)
                    .is_none_or(|e| !Self::valid(options, frame, e, people));
                if self.retarget_counter > 700 || invalid {
                    self.target = candidate;
                    self.retarget_counter = 0;
                }
            } else {
                self.target = candidate;
            }
        }
        match self.entity(frame).cloned() {
            Some(e) if frame.playing => {
                self.update_velocity_buffers();
                self.apply_rotation(options, frame, &e);
            }
            _ => self.reset_rotation_state(),
        }
    }

    fn update_velocity_buffers(&mut self) {
        self.swap_tick_counter += 1;
        if self.swap_tick_counter > 10 {
            self.vertical_velocity_buffer = self.vertical_velocity;
            self.horizontal_velocity = self.horizontal_velocity_buffer;
            self.horizontal_velocity_buffer = 0.0;
            self.vertical_velocity = 0.0;
            self.swap_tick_counter = 0;
        }
    }

    fn update_drift(&mut self) {
        self.drift_timer += 1.0;
        if self.drift_timer >= f64::from(250 + self.random.int(50)) {
            self.drift_timer = f64::from(self.random.between(-100, -50));
            self.random_offset = (self.random.between(-1, 2), self.random.between(-1, 2));
        }
        let (mut h, mut v) = self.random_offset;
        // The reference draws two unused samples first.
        self.random.int(10);
        self.random.int(10);
        if self.random.int(10) < 2 {
            h = 0;
        }
        if self.random.int(10) < 2 {
            v = 0;
        }
        if self.drift_timer < 0.0 {
            h = 0;
            v = 0;
        }
        if self.random.int(20) == 1 {
            self.drift.0 += h;
            self.drift.1 += v;
        }
        if (self.accumulator.0 > 0.0 && self.drift.0 < 0)
            || (self.accumulator.0 < 0.0 && self.drift.0 > 0)
        {
            self.drift.0 = 0;
        }
    }

    fn apply_rotation(&mut self, options: &Options, frame: &Frame, target: &EntityView) {
        self.update_drift();
        let player = &frame.player;
        let mut aim = target.position;
        if options.closest_area {
            // computeTargetOffset: the closest point of the tick box, moved
            // back by the motion still ahead of the partial tick
            // (`prev + (closest - prev) * partial`).
            let closest = target.bounds.closest(player.tick_position);
            for i in [0, 2] {
                aim[i] = closest[i] - (target.tick_position[i] - target.position[i]);
            }
        }
        let motion = (aim[0] - self.prev_target.0, aim[2] - self.prev_target.1);
        self.prev_target = (aim[0], aim[2]);
        let predicted = (aim[0] + motion.0 * 1.7, aim[2] + motion.1 * 1.7);
        let (px, pz) = (player.position[0], player.position[2]);
        let angle = yaw_difference(px, pz, player.yaw, predicted.0, predicted.1);
        let left = on_left(px, pz, player.yaw, predicted.0, predicted.1);
        let vertical = pitch_difference(player, aim);
        let above = vertical < 0;
        let dead_zone = vertical.abs() - 10;
        let distance = player.distance(target.tick_position) as f32;

        let mut h_force = 1.0f32;
        let mut v_force = 1.0f32;
        h_force = (f64::from(h_force) + self.shared_random.range(0.0, 2.0)) as f32;
        h_force = (f64::from(h_force) + angle / 50.0) as f32;
        v_force = (f64::from(v_force) + self.shared_random.range(0.0, 2.0)) as f32;
        v_force += dead_zone.abs() as f32 / 50.0;
        if (angle - self.last_angle_diff).abs() > 6.0 {
            h_force = (f64::from(h_force) + angle / 35.0) as f32;
        }
        let proximity = f64::from(((9.0 - distance) / 2.5 - 2.0).max(0.0));
        h_force = (f64::from(h_force) + proximity) as f32;
        let away = if left {
            player.strafe < 0.0
        } else {
            player.strafe > 0.0
        };
        if options.strafe_increase && away {
            h_force = (f64::from(h_force) * 1.6) as f32;
        }
        if distance < 0.5 {
            h_force /= 5.0;
        }
        let mut h_acc = h_force / 90.0 * if left { -1.0 } else { 1.0 };
        let mut v_acc = v_force / 90.0 * if above { 1.0 } else { -1.0 };
        if angle < 5.0 {
            h_acc = 0.0;
            self.horizontal_velocity *= 0.7;
            let toward = if left {
                player.strafe > 0.0
            } else {
                player.strafe < 0.0
            };
            if toward {
                self.horizontal_velocity *= 0.5;
            }
        }
        if left != self.prev_on_left {
            self.horizontal_velocity = -self.horizontal_velocity;
            self.horizontal_velocity_buffer = -self.horizontal_velocity_buffer;
            self.accumulator.0 = 0.0;
        }
        if above != self.prev_above {
            self.vertical_velocity_buffer = -self.vertical_velocity_buffer;
            self.vertical_velocity = -self.vertical_velocity;
            self.accumulator.1 = 0.0;
        }
        if dead_zone < 5 {
            v_acc = 0.0;
            self.vertical_velocity_buffer *= 0.7;
        }
        self.horizontal_velocity_buffer += h_acc;
        self.vertical_velocity += v_acc;
        let smoothed_h = self.horizontal_velocity;
        let smoothed_v = self.vertical_velocity_buffer;
        if smoothed_h.abs() > 10.0 {
            self.horizontal_velocity_buffer = 0.0;
            self.horizontal_velocity = 0.0;
            return;
        }
        let mut h_adjust = smoothed_h * 0.15;
        if angle <= 9.0 {
            h_adjust = (f64::from(h_adjust) / (10.0 - angle)) as f32;
        }
        if h_adjust.is_nan() {
            self.horizontal_velocity_buffer = 0.0;
            self.horizontal_velocity = 0.0;
            return;
        }
        self.queue_horizontal(options, frame, target, h_adjust);
        if options.aim_vertically {
            let v_adjust = (f64::from(smoothed_v) * 0.15) as f32;
            if v_adjust.is_nan() {
                self.vertical_velocity = 0.0;
                self.vertical_velocity_buffer = 0.0;
                return;
            }
            self.queue_vertical(options, v_adjust, vertical as f32);
        }
        self.prev_above = above;
        self.prev_on_left = left;
        self.sample_counter += 1;
        if self.sample_counter > 10 {
            self.last_angle_diff = angle;
            self.sample_counter = 0;
        }
    }

    fn queue_horizontal(
        &mut self,
        options: &Options,
        frame: &Frame,
        target: &EntityView,
        adjust: f32,
    ) {
        if adjust == 0.0 {
            self.accumulator.0 = 0.0;
            return;
        }
        let adjust = adjust * 5.0;
        let mut speed = options.horizontal_speed;
        let angle = int_yaw_difference(&frame.player, target.tick_position) as f32;
        if angle <= 10.0 {
            self.pitch_boost = speed;
        }
        if self.pitch_boost > 0.0 {
            speed -= self.pitch_boost / 3.0;
            self.pitch_boost -= angle / 200.0;
        }
        self.accumulator.0 += speed * adjust;
    }

    fn queue_vertical(&mut self, options: &Options, adjust: f32, angle: f32) {
        if adjust == 0.0 {
            self.accumulator.1 = 0.0;
            return;
        }
        let adjust = adjust * 5.0;
        let mut speed = options.vertical_speed;
        let angle = angle.abs();
        if angle <= 10.0 {
            self.yaw_boost = speed;
        }
        if self.yaw_boost > 0.0 {
            speed -= self.yaw_boost / 3.0;
            self.yaw_boost -= angle / 200.0;
        }
        self.accumulator.1 += speed * adjust;
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::core::settings::Settings;
    use super::super::super::super::targeting::entities::EntityClass;
    use super::super::super::super::targeting::filter::{NameLists, tests::entity};
    use super::*;

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        Options::read(&settings)
    }
    fn frame(target: [f64; 3]) -> Frame {
        Frame {
            player: PlayerView::default(),
            entities: vec![entity(EntityClass::Player, "Target", target)],
            attack_down: true,
            playing: true,
            sensitivity: 0.5,
            elapsed_ms: 16,
            ..Frame::default()
        }
    }

    #[test]
    fn defaults_follow_the_reference() {
        let o = options();
        assert!(o.require_mouse && o.filter.players && !o.filter.mobs);
        assert_eq!(
            (o.horizontal_speed, o.max_angle, o.distance),
            (5.0, 180.0, 5.0)
        );
        assert_eq!(o.target_mode, SortMode::Yaw);
        assert_eq!(o.allowed_items, vec!["swords".to_owned()]);
        assert_eq!(o.suffix(), "5");
    }

    #[test]
    fn turns_toward_a_target_on_the_left_and_stays_bounded() {
        let o = options();
        let people = NameLists::default();
        let mut aim = AimAssist::new(42);
        // Facing +Z; the target is at +X+Z: on the left, so yaw decreases.
        let f = frame([2.0, 0.0, 3.0]);
        let mut total = 0.0;
        for _ in 0..60 {
            let (x, y) = aim.frame(&o, &f, &people);
            assert!(x.is_finite() && x.abs() < 400.0);
            // Only the reference's +-1 drift reaches the vertical axis.
            assert!(y.abs() <= 3.0, "vertical aim is off by default: {y}");
            total += x;
        }
        assert!(aim.target().is_some());
        assert!(total < 0.0, "turned the wrong way: {total}");
    }

    #[test]
    fn mouse_release_friends_range_and_angle_drop_the_target() {
        let o = options();
        let mut aim = AimAssist::new(7);
        let mut f = frame([0.0, 0.0, 3.0]);
        aim.frame(&o, &f, &NameLists::default());
        assert!(aim.target().is_some());
        f.attack_down = false;
        assert_eq!(aim.frame(&o, &f, &NameLists::default()), (0.0, 0.0));
        assert!(aim.target().is_none());
        f.attack_down = true;
        let friends = NameLists {
            friends: vec!["target".into()],
            enemies: vec![],
        };
        aim.frame(&o, &f, &friends);
        assert!(aim.target().is_none());
        // Distance is truncated to whole blocks: 5.0 excludes 5.2.
        let far = frame([0.0, 0.0, 5.2]);
        aim.frame(&o, &far, &NameLists::default());
        assert!(aim.target().is_none());
        // Max angle 180 allows 90 degrees either side; behind is rejected.
        let behind = frame([0.5, 0.0, -3.0]);
        aim.frame(&o, &behind, &NameLists::default());
        assert!(aim.target().is_none());
        // Screens stop the turn without dropping the target.
        let mut screen = frame([1.0, 0.0, 3.0]);
        screen.playing = false;
        assert_eq!(aim.frame(&o, &screen, &NameLists::default()), (0.0, 0.0));
    }

    #[test]
    fn block_break_check_and_item_limit() {
        let mut o = options();
        o.check_block_break = true;
        let mut aim = AimAssist::new(3);
        let mut f = frame([1.0, 0.0, 3.0]);
        f.aiming_block = true;
        aim.frame(&o, &f, &NameLists::default());
        assert!(aim.target().is_none());
        // The 250-step cooldown outlasts a few frames after looking away.
        f.aiming_block = false;
        f.elapsed_ms = 100;
        aim.frame(&o, &f, &NameLists::default());
        assert!(aim.target().is_none());
        aim.frame(&o, &f, &NameLists::default());
        aim.frame(&o, &f, &NameLists::default());
        assert!(aim.target().is_some());
        o.limit_items = true;
        let mut aim = AimAssist::new(3);
        aim.frame(&o, &f, &NameLists::default());
        assert!(aim.target().is_none());
        f.held.id = Some("iron_sword".into());
        aim.frame(&o, &f, &NameLists::default());
        assert!(aim.target().is_some());
    }

    #[test]
    fn yaw_mode_prefers_the_smallest_angle() {
        let o = options();
        let mut f = frame([0.0, 0.0, 3.0]);
        let mut wide = entity(EntityClass::Player, "Wide", [2.0, 0.0, 1.0]);
        wide.handle.id = 8;
        f.entities.insert(0, wide);
        let best = AimAssist::best(&o, &f, &NameLists::default()).unwrap();
        assert_eq!(best.id, 7);
    }
}
