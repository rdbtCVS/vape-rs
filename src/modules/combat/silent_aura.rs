// SilentAura (reference `SilentAura`, `SilentAuraRotationController`,
// `SilentAuraAimJitter`, `SilentAuraEntityIdComparator` and the hidden
// `SilentAuraClicker`): claims the shared rotation, steers the silent managed
// rotation onto the target with the reference's PID mode, and clicks at the
// aura's rate once the managed rotation is on target. The AI rotation mode
// and its models are excluded from the port.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::super::targeting::SortMode;
use super::super::super::targeting::entities::{
    EntityClass, EntityView, PlayerView, int_yaw_difference,
};
use super::super::super::targeting::filter::{self, People, TargetFilter};
use super::click_timing::Timing;
use super::kill_aura::{self, KillAura, SORT_MODES};
use super::rotation::{self, Claim, Claimant, Managed, Step};
use serde_json::{Value, json};

pub const ID: &str = "combat.silent-aura";
/// Vanilla's base entity interaction range, which the extra swing distance
/// extends for targeting.
pub const BASE_RANGE: f64 = 3.0;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mut out = filter::definitions(ID);
    out.extend([
        Definition::choice(&key("rotation-mode"), "Rotation Mode", &["PID"], "PID"),
        Definition::number(&key("aim-speed"), "Aim speed", 1.0, 7.0, 10.0, 0.1),
        kill_aura::aps_definition(&key("aps")),
        Definition::number(
            &key("extra-swing"),
            "Extra swing distance",
            0.0,
            1.0,
            3.0,
            0.1,
        ),
        Definition::number(&key("max-angle"), "Max angle", 1.0, 120.0, 360.0, 5.0),
        Definition::choice(&key("target-mode"), "Target mode", &SORT_MODES, "Distance"),
        Definition::choice(
            &key("target-area"),
            "Target area",
            &["Center", "Closest"],
            "Center",
        ),
        Definition::boolean(&key("shield-check"), "Shield check", false),
        Definition::boolean(&key("disable-on-death"), "Disable on death", false),
        Definition::boolean(&key("require-mouse"), "Require mouse down", false),
        Definition::boolean(&key("limit-items"), "Limit to items", false),
        Definition::new(
            &key("allowed-items"),
            "Allowed Items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!([]),
        )
        .when(&key("limit-items"), json!(true)),
        Definition::boolean(&key("perfect-swing"), "Perfect swing", false),
        Definition::boolean(&key("switch"), "Switch", false),
        Definition::boolean(&key("random-aim"), "Random aim", false),
        Definition::number(&key("random-size"), "Random size", 0.0, 30.0, 100.0, 5.0)
            .when(&key("random-aim"), json!(true)),
        Definition::number(&key("random-speed"), "Random aim speed", 0.1, 2.0, 5.0, 0.1)
            .when(&key("random-aim"), json!(true)),
    ]);
    out
}

#[derive(Clone, Debug)]
pub struct Options {
    pub filter: TargetFilter,
    pub aim_speed: f64,
    pub aps: (f64, f64),
    pub range: f64,
    pub max_angle: f64,
    pub mode: SortMode,
    pub closest: bool,
    pub shield_check: bool,
    pub disable_on_death: bool,
    pub require_mouse: bool,
    pub limit_items: bool,
    pub allowed_items: Vec<String>,
    pub perfect_swing: bool,
    pub switch: bool,
    pub random_aim: bool,
    pub random_size: f64,
    pub random_speed: f64,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let get = |k: &str| settings.get(&format!("{ID}.{k}"));
        let boolean = |k: &str| get(k).and_then(Value::as_bool).unwrap_or(false);
        let number = |k: &str, d: f64| get(k).and_then(Value::as_f64).unwrap_or(d);
        Self {
            filter: TargetFilter::read(settings, ID),
            aim_speed: number("aim-speed", 7.0),
            aps: kill_aura::range(settings, &format!("{ID}.aps"), (6.0, 13.0)),
            range: BASE_RANGE + number("extra-swing", 1.0),
            max_angle: number("max-angle", 120.0),
            mode: kill_aura::sort_mode(
                get("target-mode")
                    .and_then(Value::as_str)
                    .unwrap_or("Distance"),
            ),
            closest: get("target-area").and_then(Value::as_str) == Some("Closest"),
            shield_check: boolean("shield-check"),
            disable_on_death: boolean("disable-on-death"),
            require_mouse: boolean("require-mouse"),
            limit_items: boolean("limit-items"),
            allowed_items: kill_aura::strings(settings, &format!("{ID}.allowed-items")),
            perfect_swing: boolean("perfect-swing"),
            switch: boolean("switch"),
            random_aim: boolean("random-aim"),
            random_size: number("random-size", 30.0),
            random_speed: number("random-speed", 2.0),
        }
    }
}

/// One client tick of host state.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub now_ms: u64,
    pub player: PlayerView,
    pub eye_height: f64,
    /// The local player's tick motion (`posX - prevPosX`, ...).
    pub motion: [f64; 3],
    pub entities: Vec<EntityView>,
    /// Each entity's previous tick position, by id.
    pub previous: Vec<(i32, [f64; 3])>,
    pub attack_down: bool,
    pub focused: bool,
    pub no_screen: bool,
    pub alive: bool,
    pub held: HeldItem,
    pub sensitivity: f64,
    pub cooldown_ready: bool,
    /// The shield check is bypassed (ShieldBreaker axe, HitSwap axe or
    /// AutoMace mace available).
    pub shield_bypass: bool,
}

/// What the host does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Click {
    #[default]
    None,
    /// Attack this entity: the managed rotation's ray hits it.
    Attack(i32),
    /// A click whose managed-rotation ray hits nothing (a miss swing).
    Miss,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    pub click: Click,
    pub disable: bool,
}

#[derive(Clone, Copy, Debug)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next() * (hi - lo)
    }
    fn gaussian(&mut self) -> f64 {
        let u = self.next().max(1e-12);
        let v = self.next();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// `SilentAuraAimJitter`: small offsets that drift toward re-rolled targets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Jitter {
    current: [f64; 3],
    target: [f64; 3],
    next_ms: u64,
}
impl Jitter {
    const RANGES: [(f64, f64); 3] = [(-0.15, 0.15), (-0.3, 0.25), (-0.15, 0.15)];
    fn update(&mut self, now: u64, rng: &mut Rng) {
        if now >= self.next_ms {
            for (t, (lo, hi)) in self.target.iter_mut().zip(Self::RANGES) {
                *t = rng.range(lo, hi);
            }
            self.next_ms = now + 100 + (rng.next() * 900.0) as u64;
        }
        for i in 0..3 {
            let step = 0.01 + rng.next() * 0.05;
            let d = self.target[i] - self.current[i];
            self.current[i] += d.clamp(-step, step);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RandomAim {
    current: [f64; 3],
    target: [f64; 3],
    next_ms: u64,
}

#[derive(Debug)]
pub struct SilentAura {
    rng: Rng,
    timing: Timing,
    last_click_ms: Option<u64>,
    target: Option<i32>,
    ready: bool,
    gains: [f64; 4],
    integrals: [f64; 2],
    jitter: Jitter,
    random_aim: RandomAim,
    controlling: bool,
}
impl Default for SilentAura {
    fn default() -> Self {
        Self::new(0x5a_5a17)
    }
}
impl SilentAura {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng(seed.max(1)),
            timing: Timing::new(seed ^ 0x9e37),
            last_click_ms: None,
            target: None,
            ready: false,
            gains: [1.0; 4],
            integrals: [0.0; 2],
            jitter: Jitter::default(),
            random_aim: RandomAim::default(),
            controlling: false,
        }
    }
    pub fn target(&self) -> Option<i32> {
        self.target
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
    /// `resetTargeting`: drop the target and hand the rotation back
    /// (returning to the view; the claim is released when it arrives).
    pub fn reset(&mut self, claim: &mut Claim, managed: &mut Managed) {
        self.integrals = [0.0; 2];
        self.target = None;
        self.ready = false;
        if self.controlling {
            managed.release();
            self.controlling = false;
        }
        if !managed.active {
            claim.release(Claimant::SilentAura);
        }
    }
    /// The managed rotation finished returning to the view.
    pub fn returned(&mut self, claim: &mut Claim) {
        if !self.controlling {
            claim.release(Claimant::SilentAura);
        }
    }

    /// `computeAimCoords`: the current and previous aim point.
    fn aim_coords(
        o: &Options,
        t: &Tick,
        e: &EntityView,
        previous: [f64; 3],
    ) -> ([f64; 3], [f64; 3]) {
        if !o.closest {
            return (e.tick_position, previous);
        }
        let eye = [
            t.player.tick_position[0],
            t.player.tick_position[1] + t.eye_height,
            t.player.tick_position[2],
        ];
        let c = e.bounds.closest(eye);
        let motion = [
            e.tick_position[0] - previous[0],
            e.tick_position[2] - previous[2],
        ];
        let now = [c[0], e.tick_position[1], c[2]];
        let before = [c[0] - motion[0], previous[1], c[2] - motion[1]];
        (now, before)
    }

    fn previous(t: &Tick, e: &EntityView) -> [f64; 3] {
        t.previous
            .iter()
            .find(|(id, _)| *id == e.handle.id)
            .map_or(e.tick_position, |(_, p)| *p)
    }

    fn valid(o: &Options, t: &Tick, e: &EntityView, people: &impl People) -> bool {
        let (aim, _) = Self::aim_coords(o, t, e, Self::previous(t, e));
        e.alive()
            && !e.vehicle
            && t.player.distance(aim) <= o.range
            && f64::from(int_yaw_difference(&t.player, e.tick_position)) <= o.max_angle / 2.0
            && !(e.class == EntityClass::Player && people.friend(&e.name))
            && !(o.shield_check && e.blocking && !t.shield_bypass)
            && o.filter.valid(e, people)
    }

    fn randomize_gains(&mut self) {
        self.gains = [
            0.85 + self.rng.next() * 0.3,
            0.85 + self.rng.next() * 0.3,
            0.8 + self.rng.next() * 0.4,
            0.85 + self.rng.next() * 0.3,
        ];
    }

    fn update_target(
        &mut self,
        o: &Options,
        t: &Tick,
        people: &impl People,
        claim: &mut Claim,
        managed: &mut Managed,
    ) {
        if claim.blocked_for(Claimant::SilentAura)
            && !claim.acquire(Claimant::SilentAura, false, managed.relative)
        {
            return;
        }
        let mut candidates: Vec<EntityView> = t
            .entities
            .iter()
            .filter(|e| Self::valid(o, t, e, people))
            .cloned()
            .collect();
        if candidates.is_empty() {
            self.reset(claim, managed);
            return;
        }
        if o.switch {
            candidates.sort_by_key(|e| e.handle.id);
        } else {
            kill_aura::sort(&mut candidates, &t.player, o.mode);
        }
        let chosen = &candidates[0];
        if self.target != Some(chosen.handle.id) {
            self.randomize_gains();
            self.integrals = [0.0; 2];
        }
        self.target = Some(chosen.handle.id);
        if !claim.acquire(Claimant::SilentAura, true, managed.relative) {
            return;
        }
        if !self.controlling {
            managed.start((t.player.yaw, t.player.pitch));
            self.controlling = true;
        }
        let (aim, _) = Self::aim_coords(o, t, chosen, Self::previous(t, chosen));
        let eye = [
            t.player.tick_position[0],
            t.player.tick_position[1] + t.eye_height,
            t.player.tick_position[2],
        ];
        let desired = rotation::look_at(eye, aim_point(chosen, aim, eye));
        let yaw = rotation::wrap(desired.0 - managed.yaw);
        let pitch = desired.1 - managed.pitch;
        self.ready = (yaw * yaw + pitch * pitch).sqrt() < 3.0 && t.player.distance(aim) <= o.range;
    }

    /// `updateAim` (PID mode): the next managed-rotation target.
    fn update_aim(&mut self, o: &Options, t: &Tick, claim: &Claim, managed: &mut Managed) {
        let Some(target) = self
            .target
            .and_then(|id| t.entities.iter().find(|e| e.handle.id == id))
        else {
            return;
        };
        if !claim.owned_by(Claimant::SilentAura) || !self.controlling {
            return;
        }
        self.jitter.update(t.now_ms, &mut self.rng);
        let previous = Self::previous(t, target);
        let (aim, _) = Self::aim_coords(o, t, target, previous);
        let h_motion = ((target.tick_position[0] - previous[0]).powi(2)
            + (target.tick_position[2] - previous[2]).powi(2))
        .sqrt();
        let j = self.jitter.current;
        let mut jx = aim[0] + j[0] * (1.0 + h_motion);
        let mut jz = aim[2] + j[2] * (1.0 + h_motion);
        let eye_y = t.player.tick_position[1] + t.eye_height;
        let height = target.bounds.max[1] - target.bounds.min[1];
        let mut jy = if eye_y < aim[1] {
            aim[1] + j[1] * 0.5
        } else {
            eye_y.min(aim[1] + height) - 0.275 + j[1]
        };
        if o.random_aim {
            let r = &mut self.random_aim;
            if t.now_ms >= r.next_ms {
                let half = o.random_size / 100.0 / 2.0;
                let w = target.bounds.max[0] - target.bounds.min[0];
                r.target = [
                    self.rng.range(-half, half) * w,
                    self.rng.range(-half, half) * height,
                    self.rng.range(-half, half) * w,
                ];
                r.next_ms = t.now_ms + 100 + (self.rng.next() * 200.0) as u64;
            }
            let k = (o.random_speed * 0.08).min(1.0);
            for i in 0..3 {
                r.current[i] += (r.target[i] - r.current[i]) * k;
            }
            jx += r.current[0];
            jy += r.current[1];
            jz += r.current[2];
        }
        let eye = [t.player.tick_position[0], eye_y, t.player.tick_position[2]];
        let desired = rotation::look_at(eye, [jx, jy, jz]);
        let target_pitch = rotation::look_at(eye, [aim[0], jy, aim[2]]).1;
        let mut yaw_err = f64::from(rotation::wrap(desired.0 - managed.yaw));
        let mut pitch_err = f64::from(rotation::wrap(target_pitch - managed.pitch));
        let prev_pitch_step = f64::from(managed.pitch - managed.previous_pitch);
        let mut prev_yaw_step = f64::from(rotation::wrap(managed.yaw - managed.previous_yaw));
        let step = 0.05;
        let same_dir = yaw_err.signum() == prev_yaw_step.signum();
        let [gp, gip, gy, giy] = self.gains;
        let pitch_p = 0.45 * gp;
        let pitch_i = 0.91 * gip;
        let mut yaw_p = if self.ready { 0.05 } else { 0.1 } * gy;
        let yaw_i = 0.33 * giy;
        if t.motion[1].abs() > 0.1 {
            pitch_err *= 1.0 + self.rng.next() * 0.32;
        }
        if same_dir && yaw_err.abs() < 20.0 {
            yaw_p *= 2.5;
            let speed = (t.motion[0].powi(2) + t.motion[2].powi(2)).sqrt();
            prev_yaw_step *= 1.0 + (h_motion + speed).min(0.25);
        }
        let distance = t.player.distance(aim);
        if distance < 0.8 {
            let f = distance / 0.8;
            pitch_err *= f * f;
            yaw_err *= f;
        }
        let sign = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
        let pitch_ctl = pitch_err - prev_pitch_step + prev_yaw_step * 0.05 * sign;
        let yaw_ctl = yaw_err - prev_yaw_step;
        self.integrals[0] += pitch_ctl * step;
        self.integrals[1] += yaw_ctl * step;
        let pitch_adj = pitch_p * pitch_ctl + pitch_i * self.integrals[0];
        let mut yaw_adj = yaw_p * yaw_ctl + yaw_i * self.integrals[1];
        if yaw_err.abs() > 120.0 {
            self.integrals[1] = 0.0;
            yaw_adj = 0.0;
        }
        if !t.no_screen {
            self.integrals = [0.0; 2];
        }
        managed.set_target(
            (f64::from(managed.yaw) + yaw_err + yaw_adj / 3.0) as f32,
            (f64::from(managed.pitch) + pitch_adj) as f32,
        );
    }

    /// `SilentAuraRotationController.getSpeed`, as counts per update.
    pub fn step(&mut self, o: &Options, distance: f64) -> Step {
        let mut speed = o.aim_speed * (0.65 + 0.25 * self.rng.gaussian()).exp().clamp(1.4, 3.0);
        if distance < 0.8 {
            speed *= distance / 0.8;
        }
        Step {
            counts: speed * 0.25,
            clamp: true,
            proportional: false,
            linear: false,
        }
    }

    pub fn distance_to_target(&self, t: &Tick) -> f64 {
        self.target
            .and_then(|id| t.entities.iter().find(|e| e.handle.id == id))
            .map_or(f64::MAX, |e| t.player.distance(e.tick_position))
    }

    /// One tick before the rotation advances: targeting and aim.
    pub fn tick(
        &mut self,
        o: &Options,
        t: &Tick,
        people: &impl People,
        claim: &mut Claim,
        managed: &mut Managed,
    ) -> Effects {
        let mut effects = Effects::default();
        if !t.alive {
            self.reset(claim, managed);
            effects.disable = o.disable_on_death;
            return effects;
        }
        if !t.focused || (o.require_mouse && !t.attack_down) {
            self.reset(claim, managed);
            return effects;
        }
        self.update_target(o, t, people, claim, managed);
        self.update_aim(o, t, claim, managed);
        effects
    }

    /// `SilentAuraClicker` after the rotation advanced: a click when the
    /// aura can attack, hitting what the managed rotation's ray hits.
    pub fn click(&mut self, o: &Options, t: &Tick, managed: &Managed, reach: f64) -> Click {
        if !self.ready || !t.no_screen || !self.controlling {
            return Click::None;
        }
        if o.limit_items && !item_limit::matches(&o.allowed_items, &t.held) {
            return Click::None;
        }
        if o.require_mouse && !t.attack_down {
            return Click::None;
        }
        let ready = if o.perfect_swing {
            t.cooldown_ready
        } else {
            KillAura::click_ready(&mut self.timing, self.last_click_ms, t.now_ms, o.aps)
        };
        if !ready {
            return Click::None;
        }
        self.last_click_ms = Some(t.now_ms);
        let Some((yaw, pitch)) = managed.sent() else {
            return Click::None;
        };
        let eye = [
            t.player.tick_position[0],
            t.player.tick_position[1] + t.eye_height,
            t.player.tick_position[2],
        ];
        let dir = rotation::direction(yaw, pitch);
        let hit = t
            .entities
            .iter()
            .filter(|e| e.alive() && !e.vehicle)
            .filter_map(|e| {
                rotation::ray_box(eye, dir, reach, e.bounds.min, e.bounds.max)
                    .map(|d| (e.handle.id, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match hit {
            Some((id, _)) => Click::Attack(id),
            None => Click::Miss,
        }
    }
}

/// The reference aim height before jitter: the target's base when the eye
/// is below it, else level with the eye within the box.
fn aim_point(e: &EntityView, aim: [f64; 3], eye: [f64; 3]) -> [f64; 3] {
    let height = e.bounds.max[1] - e.bounds.min[1];
    let y = if eye[1] < aim[1] {
        aim[1]
    } else {
        eye[1].min(aim[1] + height) - 0.275
    };
    [aim[0], y, aim[2]]
}

#[cfg(test)]
mod tests {
    use super::super::super::super::targeting::filter::NameLists;
    use super::super::super::super::targeting::filter::tests::entity as base;
    use super::super::rotation::UPDATES_PER_TICK;
    use super::*;

    fn zombie(id: i32, at: [f64; 3]) -> EntityView {
        let mut e = base(EntityClass::Monster, "", at);
        e.handle.id = id;
        e
    }

    fn options() -> Options {
        let mut settings = Settings::default();
        for d in definitions() {
            settings.define(d).unwrap();
        }
        settings
            .set(&format!("{ID}.targets.mobs"), json!(true))
            .unwrap();
        Options::read(&settings)
    }

    fn tick(now: u64, yaw: f32, entities: Vec<EntityView>) -> Tick {
        Tick {
            now_ms: now,
            player: PlayerView {
                yaw,
                ..PlayerView::default()
            },
            eye_height: 1.62,
            entities,
            focused: true,
            no_screen: true,
            alive: true,
            sensitivity: 0.5,
            ..Tick::default()
        }
    }

    #[test]
    fn claims_aims_silently_and_attacks_once_on_target() {
        let o = options();
        let mut aura = SilentAura::new(11);
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        // Target 40 degrees to the left of the view (yaw +40 turns left).
        let at = [
            -2.5 * 40f64.to_radians().sin(),
            0.0,
            2.5 * 40f64.to_radians().cos(),
        ];
        let mut attacked = None;
        for i in 0..40u64 {
            let t = tick(1000 + i * 50, 0.0, vec![zombie(5, at)]);
            aura.tick(&o, &t, &NameLists::default(), &mut claim, &mut managed);
            let d = aura.distance_to_target(&t);
            managed.tick((0.0, 0.0), t.sensitivity, UPDATES_PER_TICK, || {
                aura.step(&o, d)
            });
            if let Click::Attack(id) = aura.click(&o, &t, &managed, 3.0) {
                attacked = Some((i, id));
                break;
            }
        }
        let (ticks, id) = attacked.expect("an attack");
        assert_eq!(id, 5);
        assert!(ticks > 0, "the managed rotation turns before the first hit");
        assert!(claim.owned_by(Claimant::SilentAura));
        assert!(
            (managed.yaw - 40.0).abs() < 10.0,
            "managed yaw {}",
            managed.yaw
        );
        // Losing the target returns the rotation to the view, then releases.
        let t = tick(5000, 0.0, vec![]);
        aura.tick(&o, &t, &NameLists::default(), &mut claim, &mut managed);
        assert!(managed.relative);
        for _ in 0..40 {
            if managed.tick((0.0, 0.0), 0.5, UPDATES_PER_TICK, || aura.step(&o, 3.0)) {
                aura.returned(&mut claim);
            }
        }
        assert!(!managed.active && claim.owner().is_none());
    }

    #[test]
    fn a_higher_claim_blocks_the_aura() {
        let o = options();
        let mut aura = SilentAura::new(3);
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        assert!(claim.try_claim(Claimant::BowAimbot));
        let t = tick(1000, 0.0, vec![zombie(5, [0.0, 0.0, 2.0])]);
        aura.tick(&o, &t, &NameLists::default(), &mut claim, &mut managed);
        assert!(!managed.active && aura.target().is_none());
    }

    #[test]
    fn switch_sorts_by_id_and_shield_check_skips_blockers() {
        let mut o = options();
        o.switch = true;
        let mut aura = SilentAura::new(5);
        let (mut claim, mut managed) = (Claim::default(), Managed::default());
        let t = tick(
            1000,
            0.0,
            vec![zombie(9, [0.0, 0.0, 1.5]), zombie(4, [0.0, 0.0, 3.0])],
        );
        aura.tick(&o, &t, &NameLists::default(), &mut claim, &mut managed);
        assert_eq!(aura.target(), Some(4));
        o.shield_check = true;
        let mut blocker = zombie(4, [0.0, 0.0, 3.0]);
        blocker.blocking = true;
        let t = tick(1050, 0.0, vec![zombie(9, [0.0, 0.0, 1.5]), blocker]);
        aura.tick(&o, &t, &NameLists::default(), &mut claim, &mut managed);
        assert_eq!(aura.target(), Some(9));
    }
}
