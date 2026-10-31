// AntiFireball (reference `BowAimbot`, display "AntiFireball", priority 7 on
// the shared rotation claim): a large fireball predicted to pass within four
// blocks is aimed at with a mouse-count rotation controller (the view, or a
// silent adaptive rotation with Silent aim), then clicked to deflect it.
// Owners are recorded when the fireball first appears: the nearest player
// within two blocks of it, or none.
use super::super::super::core::settings::{Definition, Settings};
use super::super::rng::Rng;
use super::rotation::{Claim, Claimant, wrap};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const ID: &str = "combat.anti-fireball";
const ME: Claimant = Claimant::BowAimbot;
/// The reference's `drag` for a fireball (`0.95f`).
const DRAG: f64 = 0.949_999_988_079_071;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let describe = |mut d: Definition, text: &str| {
        d.description = text.into();
        d
    };
    vec![
        Definition::number(&key("angle-limit"), "Angle limit", 1.0, 45.0, 180.0, 5.0),
        Definition::number(&key("aim-speed"), "Aim speed", 1.0, 9.0, 10.0, 0.1),
        describe(
            Definition::boolean(&key("stop-movement"), "Stop movement", false),
            "Forces you to stand when attacking fireball",
        ),
        describe(
            Definition::boolean(&key("move-on-finish"), "Move on finish", false),
            "Will repress your movement keys after attacking",
        )
        .when(&key("stop-movement"), json!(true)),
        describe(
            Definition::boolean(&key("silent-aim"), "Silent aim", false),
            "Uses Silent Aim system",
        ),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub angle_limit: f64,
    pub aim_speed: f64,
    pub stop_movement: bool,
    pub move_on_finish: bool,
    pub silent_aim: bool,
}
impl Options {
    pub fn read(settings: &Settings) -> Self {
        let id = |k: &str| format!("{ID}.{k}");
        let boolean = |k: &str| {
            settings
                .effective(&id(k))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        };
        let number = |k: &str, d: f64| settings.get(&id(k)).and_then(Value::as_f64).unwrap_or(d);
        Self {
            angle_limit: number("angle-limit", 45.0),
            aim_speed: number("aim-speed", 9.0),
            stop_movement: boolean("stop-movement"),
            move_on_finish: boolean("move-on-finish"),
            silent_aim: boolean("silent-aim"),
        }
    }
}

/// A box, `min` to `max`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
impl Aabb {
    fn grow(self, d: f64) -> Self {
        Self {
            min: self.min.map(|v| v - d),
            max: self.max.map(|v| v + d),
        }
    }
    /// `addCoord`: stretched along `v`.
    fn stretch(self, v: [f64; 3]) -> Self {
        let mut out = self;
        for (i, d) in v.into_iter().enumerate() {
            if d < 0.0 {
                out.min[i] += d;
            } else {
                out.max[i] += d;
            }
        }
        out
    }
    fn intersects(&self, o: &Aabb) -> bool {
        (0..3).all(|i| o.max[i] > self.min[i] && o.min[i] < self.max[i])
    }
}

/// Another living entity: a mob, a LAN guest or a remote player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Body {
    pub id: i32,
    pub position: [f64; 3],
    pub bounds: Aabb,
    pub player: bool,
    /// `isValidArrowOwner` for a player: not a teammate and not a friend.
    pub valid_owner: bool,
}

/// The local player.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Me {
    pub id: i32,
    /// Feet position and the previous tick's.
    pub position: [f64; 3],
    pub previous: [f64; 3],
    pub eye_height: f64,
    pub bounds: Aabb,
    /// The view rotation.
    pub yaw: f32,
    pub pitch: f32,
}

/// A large fireball the host tracks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fireball {
    pub id: i32,
    pub position: [f64; 3],
    pub previous: [f64; 3],
    pub velocity: [f64; 3],
    /// `accelerationPower`.
    pub acceleration: f64,
    pub age: u32,
    pub width: f64,
    pub height: f64,
}
impl Fireball {
    fn bounds(&self) -> Aabb {
        let h = self.width / 2.0;
        let p = self.position;
        Aabb {
            min: [p[0] - h, p[1], p[2] - h],
            max: [p[0] + h, p[1] + self.height, p[2] + h],
        }
    }
    /// The acceleration added each tick: the unit motion times the power.
    fn accel(&self) -> [f64; 3] {
        let v = self.velocity;
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if n < 1e-7 {
            return [0.0; 3];
        }
        v.map(|c| c / n * self.acceleration)
    }
}

/// The host's view for one tick.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub me: Me,
    pub bodies: Vec<Body>,
    pub fireballs: Vec<Fireball>,
    /// The window has focus.
    pub focused: bool,
    /// A screen is open and InvWalk does not handle it.
    pub blocked_by_screen: bool,
    /// Any screen is open (controllers stop applying to the view).
    pub screen_open: bool,
    /// The attack key is held.
    pub attack_down: bool,
    /// The redirectable projectile under the crosshair, at the rotation the
    /// server currently has.
    pub crosshair: Option<i32>,
    pub sensitivity: f64,
    /// SilentAura's managed rotation is active, and what it last sent.
    pub managed: Option<(f32, f32)>,
    pub managed_returning: bool,
}

/// What the host does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    /// `sendLeftButtonUp`.
    pub release_attack: bool,
    /// `sendLeftButtonDown`: the click at the target.
    pub press_attack: bool,
    /// The view after the fixed controller turned it.
    pub view: Option<(f32, f32)>,
    /// The adaptive rotation's turn-to-view normalization (whole turns).
    pub view_yaw_offset: f32,
    /// The rotation for this tick's motion packet while a controller is
    /// installed: the adaptive rotation, or the view (a fixed controller).
    pub sent: Option<Option<(f32, f32)>>,
}

/// `getMouseScale()`: the sensitivity curve.
fn mouse_scale(sensitivity: f64) -> f32 {
    let b = sensitivity as f32 * 0.6 + 0.2;
    b * b * b * 8.0
}

/// Java's `Math.round(double)`.
fn round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

const UNSET: f32 = -999.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// `FixedRotationController`: turns the view.
    Fixed,
    /// `AdaptiveRotationController`: its own rotation, absolute or an
    /// offset from the view (`relativeMode`).
    Adaptive {
        yaw: f32,
        pitch: f32,
        relative: bool,
        saved: (f32, f32),
    },
}

/// `MouseRotationController` with the fixed and adaptive subclasses.
#[derive(Clone, Debug, PartialEq)]
pub struct Controller {
    kind: Kind,
    target: (f32, f32),
    tolerance: f64,
    speed: f64,
    pending_yaw: f32,
    pending_pitch: f32,
    yaw_jitter: i32,
    pitch_jitter: i32,
    yaw_dir: i32,
    pitch_dir: i32,
    jitter_ticks: i32,
    complete: bool,
    retain: bool,
    randomize: bool,
    clamp: bool,
    proportional: bool,
    cubic: bool,
    linear: bool,
    angle_based: bool,
}
impl Controller {
    fn new(kind: Kind, target: (f32, f32)) -> Self {
        Self {
            kind,
            target,
            tolerance: 3.0,
            speed: 1.0,
            pending_yaw: 0.0,
            pending_pitch: 0.0,
            yaw_jitter: 0,
            pitch_jitter: 0,
            yaw_dir: 0,
            pitch_dir: 0,
            jitter_ticks: 0,
            complete: false,
            retain: false,
            randomize: false,
            clamp: false,
            proportional: false,
            cubic: false,
            linear: false,
            angle_based: false,
        }
    }
    pub fn fixed(target: (f32, f32)) -> Self {
        Self::new(Kind::Fixed, target)
    }
    /// Starts at the managed rotation (or the view when there is none).
    pub fn adaptive(start: (f32, f32), view: (f32, f32)) -> Self {
        Self::new(
            Kind::Adaptive {
                yaw: start.0,
                pitch: start.1,
                relative: false,
                saved: view,
            },
            start,
        )
    }
    pub fn adaptive_kind(&self) -> bool {
        matches!(self.kind, Kind::Adaptive { .. })
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
    pub fn should_retain(&self) -> bool {
        match self.kind {
            Kind::Fixed => self.retain,
            Kind::Adaptive { relative, .. } => !relative,
        }
    }
    fn set_target(&mut self, target: (f32, f32)) {
        self.target = target;
        self.complete = false;
    }
    /// `setRelativeMode`; the view yaw's normalization offset when turning
    /// relative.
    fn set_relative(&mut self, on: bool, view: (f32, f32)) -> f32 {
        let Kind::Adaptive {
            yaw,
            pitch,
            relative,
            saved,
        } = &mut self.kind
        else {
            return 0.0;
        };
        if *relative == on {
            return 0.0;
        }
        let mut offset = 0.0f32;
        if on {
            let yaw_diff = view.0 - *yaw;
            let pitch_diff = view.1 - *pitch;
            while yaw_diff + offset > 180.0 {
                offset -= 360.0;
            }
            while yaw_diff + offset < -180.0 {
                offset += 360.0;
            }
            *yaw = wrap(-yaw_diff);
            *pitch = (-pitch_diff).clamp(-90.0, 90.0);
            *saved = (view.0 + offset, view.1);
        } else {
            *yaw += view.0;
            *pitch = (*pitch + view.1).clamp(-90.0, 90.0);
        }
        *relative = on;
        self.complete = false;
        offset
    }
    /// The rotation the adaptive controller renders (none for fixed).
    pub fn rendered(&self, view: (f32, f32)) -> Option<(f32, f32)> {
        match self.kind {
            Kind::Fixed => None,
            Kind::Adaptive {
                yaw,
                pitch,
                relative: true,
                ..
            } => Some((view.0 + yaw, (view.1 + pitch).clamp(-90.0, 90.0))),
            Kind::Adaptive { yaw, pitch, .. } => Some((yaw, pitch.clamp(-90.0, 90.0))),
        }
    }
    /// `onPostRenderTick` while relative: view turns since the last frame
    /// eat into the offset when they shrink it.
    fn follow_view(&mut self, view: (f32, f32)) {
        if let Kind::Adaptive {
            yaw,
            pitch,
            relative,
            saved,
        } = &mut self.kind
        {
            if *relative {
                let dy = view.0 - saved.0;
                let dp = view.1 - saved.1;
                if (*yaw - dy).abs() < yaw.abs() {
                    *yaw -= dy;
                }
                if (*pitch - dp).abs() < pitch.abs() {
                    *pitch -= dp;
                }
            }
            *saved = view;
        }
    }
    fn current(&self, view: (f32, f32)) -> (f32, f32) {
        match self.kind {
            Kind::Fixed => view,
            Kind::Adaptive { yaw, pitch, .. } => (yaw, pitch),
        }
    }
    fn outside(&self, error: f64, scale: f32) -> bool {
        let unit = f64::from(scale * 0.15);
        round(error / unit) > round(self.tolerance / unit).max(0)
    }
    fn predicted(&self, current: (f32, f32), scale: f32, pitch_floor: bool) -> (f32, f32) {
        let yaw = current.0 + (self.pending_yaw as i32) as f32 * scale * 0.15;
        let mut pitch = current.1;
        if pitch_floor && pitch == -90.0 {
            pitch = -89.99;
        }
        let pitch = pitch - ((-self.pending_pitch) as i32) as f32 * scale * 0.15;
        (yaw, pitch)
    }
    fn add_step(&self, pending: f32, error: f64, mut step: f64, scale: f32) -> f32 {
        if self.clamp {
            step = step.min((error / f64::from(scale * 0.15)).abs());
        }
        (if error > 0.0 {
            f64::from(pending) + step
        } else {
            f64::from(pending) - step
        }) as f32
    }
    fn yaw_accel(&self, step: f64, e: f64) -> f64 {
        if self.angle_based {
            step * (225.0 + e) / 180.0
        } else if self.linear {
            step + e * 0.05
        } else if !self.cubic {
            step
        } else {
            let x = e / 100.0 + 0.7;
            step * (0.4 + 2.0 * x * x * x + x * x).clamp(1.0, 4.0)
        }
    }
    fn pitch_accel(&self, step: f64, e: f64) -> f64 {
        if self.angle_based {
            step * (135.0 + e) / 90.0
        } else if self.linear {
            step + e * 0.05
        } else if !self.cubic {
            step
        } else {
            let x = e / 75.0 + 0.7;
            step * (0.4 + 2.0 * x * x * x + x * x).max(1.0)
        }
    }
    fn errors(&self, current: (f32, f32), scale: f32, floor: bool) -> (f64, f64) {
        let (py, pp) = self.predicted(current, scale, floor);
        (
            f64::from(wrap((self.target.0 - py) % 360.0)),
            f64::from(wrap((self.target.1 - pp) % 360.0)),
        )
    }
    /// `updateYaw`: true when the yaw is inside the tolerance.
    fn update_yaw(&mut self, current: (f32, f32), scale: f32) -> bool {
        if self.target.0 == UNSET {
            return true;
        }
        let (ye, pe) = self.errors(current, scale, false);
        if !self.outside(ye.abs(), scale) {
            return true;
        }
        let mut step = self.speed * 0.25;
        let ratio = ye.abs() / pe.abs();
        if self.proportional && ratio < 1.0 {
            step *= ratio;
        }
        let step = self.yaw_accel(step, ye.abs());
        self.pending_yaw = self.add_step(self.pending_yaw, ye, step, scale);
        false
    }
    fn update_pitch(&mut self, current: (f32, f32), scale: f32) -> bool {
        if self.target.1 == UNSET {
            return true;
        }
        let (ye, pe) = self.errors(current, scale, true);
        if !self.outside(pe.abs(), scale) {
            return true;
        }
        let mut step = self.speed * 0.25;
        let ratio = pe.abs() / ye.abs();
        if self.proportional && ratio < 1.0 {
            step *= ratio;
        }
        let step = self.pitch_accel(step, pe.abs());
        self.pending_pitch = self.add_step(self.pending_pitch, pe, step, scale);
        false
    }
    /// `update(player, screen)`.
    fn update(&mut self, view: (f32, f32), screen_open: bool, scale: f32) -> f32 {
        let mut offset = 0.0;
        match self.kind {
            Kind::Fixed => {
                if screen_open {
                    return 0.0;
                }
                let yc = self.update_yaw(view, scale);
                let pc = self.update_pitch(view, scale);
                if yc && pc && self.pending_yaw.abs() < 1.0 && self.pending_pitch.abs() < 1.0 {
                    self.complete = true;
                }
            }
            Kind::Adaptive { relative, .. } => {
                if relative {
                    self.target = (0.0, 0.0);
                }
                let current = self.current(view);
                let yc = self.update_yaw(current, scale);
                let pc = self.update_pitch(current, scale);
                let done =
                    yc && pc && self.pending_yaw.abs() < 1.0 && self.pending_pitch.abs() < 1.0;
                if !relative && done && !self.should_retain() {
                    offset = self.set_relative(true, view);
                } else {
                    self.complete = done;
                }
            }
        }
        offset
    }
    /// `updateJitter`, once per controller update.
    fn update_jitter(&mut self, rng: &mut Rng) {
        let mut int = |b: u64| rng.int(b) as i32;
        self.jitter_ticks += 1;
        if self.jitter_ticks >= 250 + int(50) {
            self.jitter_ticks = -100 + int(50);
            self.yaw_dir = -1 + int(3);
            self.pitch_dir = -(-1 + int(3));
        }
        let mut yaw_step = self.yaw_dir;
        let mut pitch_step = self.pitch_dir;
        if int(10) < 2 {
            yaw_step = 0;
        }
        if int(10) < 2 {
            pitch_step = 0;
        }
        if self.jitter_ticks < 0 {
            yaw_step = 0;
            pitch_step = 0;
        }
        if int(20) == 1 {
            self.yaw_jitter += yaw_step;
            self.pitch_jitter += pitch_step;
        }
        if (self.pending_yaw > 0.0 && self.yaw_jitter < 0)
            || (self.pending_yaw < 0.0 && self.yaw_jitter > 0)
        {
            self.yaw_jitter = 0;
        }
    }
    /// `applyPendingMovement`: whole counts turn the view (fixed) or the
    /// controller's own rotation (adaptive).
    fn apply_pending(
        &mut self,
        view: &mut (f32, f32),
        screen_open: bool,
        scale: f32,
        rng: &mut Rng,
    ) {
        if self.randomize {
            self.update_jitter(rng);
            self.pending_yaw += self.yaw_jitter as f32;
            self.pending_pitch += self.pitch_jitter as f32;
        }
        let yaw_steps = self.pending_yaw as i32;
        let pitch_steps = self.pending_pitch as i32;
        let yaw_rest = self.pending_yaw - yaw_steps as f32;
        let pitch_rest = self.pending_pitch - pitch_steps as f32;
        let dx = yaw_steps as f32 * scale;
        let dy = -(pitch_steps as f32 * scale);
        if !screen_open {
            match &mut self.kind {
                Kind::Fixed => {
                    // Entity.turn.
                    view.0 += dx * 0.15;
                    view.1 = (view.1 - dy * 0.15).clamp(-90.0, 90.0);
                }
                Kind::Adaptive { yaw, pitch, .. } => {
                    *yaw = (f64::from(*yaw) + f64::from(dx) * 0.15) as f32;
                    *pitch = ((f64::from(*pitch) - f64::from(dy) * 0.15) as f32).clamp(-90.0, 90.0);
                }
            }
        }
        self.pending_yaw = yaw_rest;
        self.pending_pitch = pitch_rest;
        self.yaw_jitter = 0;
        self.pitch_jitter = 0;
    }
    /// `releaseController`.
    fn release(&mut self, view: (f32, f32)) -> f32 {
        match self.kind {
            Kind::Fixed => {
                self.retain = false;
                self.complete = true;
                0.0
            }
            Kind::Adaptive { .. } => self.set_relative(true, view),
        }
    }
}

/// `RotationManager.onTick` for the installed controller: the updates the
/// sensitivity's time scale asks for this tick.
pub fn updates_per_tick(sensitivity: f64) -> u32 {
    let scale = f64::from(mouse_scale(sensitivity)).max(1e-4);
    round(50.0 / scale).clamp(1, 10_000) as u32
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `RotationUtil.T(player, box)`: the box point nearest the player's x/z
/// and eye height, nudged off the player's own column.
fn closest_point(me: &Me, b: Aabb) -> [f64; 3] {
    let eye = me.previous[1] + me.eye_height;
    let mut x = me.position[0].clamp(b.min[0], b.max[0]);
    let y = eye.clamp(b.min[1], b.max[1]);
    let mut z = me.position[2].clamp(b.min[2], b.max[2]);
    if x == me.position[0] {
        x = me.position[0] + 0.01;
    }
    if z == me.position[2] {
        z = me.position[2] + 0.01;
    }
    [x, y, z]
}

/// `computeRotationTo`: `RotationUtil.k` for yaw, `RotationUtil.h` (from
/// the feet) for pitch.
pub fn rotation_to(me: &Me, t: [f64; 3]) -> (f32, f32) {
    let (px, py, pz) = (me.position[0], me.position[1], me.position[2]);
    let d5 = t[0] - px;
    let d6 = t[2] - pz;
    let mut yaw = (-(d5 / d6).atan()).to_degrees() as f32;
    if d6 < 0.0 && d5 < 0.0 {
        yaw = (90.0 + (d6 / d5).atan().to_degrees()) as f32;
    } else if d6 < 0.0 && d5 > 0.0 {
        yaw = (-90.0 + (d6 / d5).atan().to_degrees()) as f32;
    }
    let dx = px - t[0];
    let dz = pz - t[2];
    let horizontal = f64::from(((dx * dx + dz * dz) as f32).sqrt());
    let pitch = ((py - t[1]).atan2(horizontal) * 180.0 / std::f64::consts::PI) as f32;
    (yaw, pitch)
}

/// `RotationUtil.a(player, entity)`: the whole yaw degrees between the view
/// and the entity.
fn yaw_off(me: &Me, at: [f64; 3]) -> i32 {
    let d2 = at[0] - me.position[0];
    let d3 = at[2] - me.position[2];
    let d = if d3 > 0.0 && d2 != 0.0 {
        (-(d2 / d3).atan()).to_degrees()
    } else if d3 < 0.0 && d2 > 0.0 {
        -90.0 + (d3 / d2).atan().to_degrees()
    } else if d3 < 0.0 && d2 < 0.0 {
        90.0 + (d3 / d2).atan().to_degrees()
    } else {
        0.0
    };
    let n = ((d - f64::from(me.yaw)).abs() % 360.0) as i32;
    if n > 180 { 360 - n } else { n }
}

/// `isArrowIncoming`: its next position and its last step both bring it
/// closer, unless it was just launched beside and behind the player.
pub fn incoming(me: &Me, f: &Fireball) -> bool {
    let a = f.accel();
    let motion = [
        f.position[0] - f.previous[0],
        f.position[1] - f.previous[1],
        f.position[2] - f.previous[2],
    ];
    let predicted = add(f.position, add(f.velocity, a).map(|c| c * DRAG));
    let current = f64::from(distance(me.position, f.position) as f32);
    let incoming = distance(me.position, predicted) < current
        && distance(me.position, add(f.position, motion)) < current;
    if incoming && f.age <= 4 && current <= 2.5 && yaw_off(me, f.position).abs() > 90 {
        return false;
    }
    incoming
}

/// `predictArrowDistance`: the closest approach within four blocks over the
/// next ten ticks (999 if none), or the distance to the block it hits.
pub fn predict(
    me: &Me,
    bodies: &[Body],
    f: &Fireball,
    clip: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
) -> f64 {
    let mut closest = 999.0f64;
    let mut p = f.position;
    let mut v = f.velocity;
    let accel = f.accel();
    let (w, h) = (f.width, f.height);
    let me_box = me.bounds;
    for _ in 0..10 {
        let start = p;
        let end = add(p, v);
        let block = clip(start, end);
        let mut arrow = Aabb {
            min: [p[0] - w, p[1], p[2] - w],
            max: [p[0] + w, p[1] + h, p[2] + w],
        };
        let search = arrow.stretch(v).grow(1.0);
        // The modern branch keeps only a MISS intercept, so no entity is
        // ever hit; the box is still replaced by the last candidate's.
        let candidates = std::iter::once(me_box).chain(bodies.iter().map(|b| b.bounds));
        for b in candidates {
            if search.intersects(&b) {
                arrow = b.grow(0.3);
            }
        }
        if let Some(hit) = block {
            return distance(me.position, hit);
        }
        p = end;
        let point = closest_point(me, arrow.grow(1.0));
        let d = distance(me.position, point);
        if d <= 4.0 && d < closest {
            closest = d;
        }
        v = add(v, accel).map(|c| c * DRAG);
    }
    closest
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Movement {
    /// `movementLock`: Sprint stands down while it is held.
    locked: bool,
    /// W/A/S/D were set up this tick.
    forced: bool,
    /// Keys set up earlier and never re-pressed: they stay up while the
    /// user keeps holding them (vanilla key state only changes on events).
    released: [bool; 4],
}

#[derive(Clone, Debug)]
pub struct AntiFireball {
    /// `arrowOwners`: fireball id to owner (None for no player nearby).
    owners: BTreeMap<i32, Option<i32>>,
    seen: BTreeSet<i32>,
    target: Option<i32>,
    swing_ticks: i32,
    triggered: bool,
    attack_pending: bool,
    controller: Option<Controller>,
    /// The controller is the manager's active one.
    installed: bool,
    /// `toggledOff`: a disable deferred while the adaptive controller runs.
    toggled_off: bool,
    movement: Movement,
    rng: Rng,
}
impl Default for AntiFireball {
    fn default() -> Self {
        Self {
            owners: BTreeMap::new(),
            seen: BTreeSet::new(),
            target: None,
            swing_ticks: 0,
            triggered: false,
            attack_pending: false,
            controller: None,
            installed: false,
            toggled_off: false,
            movement: Movement::default(),
            rng: Rng::new(0x5bd1_e995_7f4a_7c15),
        }
    }
}

impl AntiFireball {
    pub fn target(&self) -> Option<i32> {
        self.target
    }
    pub fn controller(&self) -> Option<&Controller> {
        self.controller.as_ref().filter(|_| self.installed)
    }
    pub fn toggled_off(&self) -> bool {
        self.toggled_off
    }
    /// The movement lock is held (Sprint's `movementLocked`).
    pub fn movement_locked(&self) -> bool {
        self.movement.locked
    }
    /// The forward, back, left and right keys this module holds up, given
    /// the user's physical keys.
    pub fn held_up(&mut self, user: [bool; 4]) -> [bool; 4] {
        if self.movement.forced {
            self.movement.released = user;
            return [true; 4];
        }
        for (r, u) in self.movement.released.iter_mut().zip(user) {
            *r &= u;
        }
        self.movement.released
    }

    /// `setEnabled(false)` with the adaptive controller running only flips
    /// `toggledOff`; true when the toggle was taken here.
    pub fn defer_disable(&mut self, enabled: bool) -> bool {
        if !enabled
            && self
                .controller
                .as_ref()
                .is_some_and(Controller::adaptive_kind)
        {
            self.toggled_off = !self.toggled_off;
            return true;
        }
        self.toggled_off = false;
        false
    }

    /// A real disable or a session change: the target, movement, rotation
    /// and claim are all released (the reference has no `onDisable`).
    pub fn reset(&mut self, claim: &mut Claim) -> Effects {
        let fx = Effects {
            release_attack: self.attack_pending,
            ..Effects::default()
        };
        claim.release(ME);
        *self = Self {
            rng: self.rng.clone(),
            ..Self::default()
        };
        fx
    }

    /// `isOwnedBy`.
    fn owned(claim: &Claim) -> bool {
        claim.owner() == Some(ME) && claim.pending().is_none()
    }
    /// `isBlockedFor`.
    fn blocked(claim: &Claim) -> bool {
        let owner = claim.owner();
        owner.is_some_and(|o| o != ME && ME.priority() <= o.priority())
            || claim
                .pending()
                .is_some_and(|p| p != ME && p.priority() > owner.map_or(0, Claimant::priority))
    }
    fn acquire(&self, claim: &mut Claim, t: &Tick, silent: bool) -> bool {
        if claim.owner() == Some(ME) {
            return false;
        }
        let adaptive =
            t.managed.is_some() || self.controller().is_some_and(Controller::adaptive_kind);
        claim.acquire(ME, silent && adaptive, t.managed_returning)
    }

    /// `suppressMovement`.
    fn suppress(&mut self, stop: bool) {
        if stop {
            self.movement.locked = true;
            self.movement.forced = true;
        }
    }
    /// `restoreMovement`: Move on finish re-reads the physical keys (unless
    /// a screen blocks input); otherwise keys set up stay up.
    fn restore_movement(&mut self, o: &Options, blocked_by_screen: bool) {
        if !o.stop_movement {
            return;
        }
        self.movement.locked = false;
        if o.move_on_finish && !blocked_by_screen {
            self.movement.forced = false;
            self.movement.released = [false; 4];
        }
    }

    /// `releaseRotation`.
    fn release_rotation(&mut self, aim_speed: f64, view: (f32, f32), fx: &mut Effects) {
        if !self.installed {
            return;
        }
        if let Some(c) = self.controller.as_mut() {
            c.speed = (aim_speed * 0.6).min(6.0);
            c.randomize = false;
            c.cubic = false;
            c.angle_based = true;
            fx.view_yaw_offset += c.release(view);
        }
    }

    fn clear_target(&mut self, o: &Options, t: &Tick, view: (f32, f32), fx: &mut Effects) {
        self.target = None;
        self.swing_ticks = 0;
        self.triggered = false;
        self.restore_movement(o, t.blocked_by_screen);
        self.release_rotation(o.aim_speed, view, fx);
    }

    /// `onEntityJoinWorld` for fireballs not seen before; `cleanOwnerMap`.
    fn track(&mut self, t: &Tick) {
        for f in &t.fireballs {
            if !self.seen.insert(f.id) {
                continue;
            }
            let search = f.bounds().grow(2.0);
            let me = Body {
                id: t.me.id,
                position: t.me.position,
                bounds: t.me.bounds,
                player: true,
                valid_owner: true,
            };
            let owner = std::iter::once(&me)
                .chain(&t.bodies)
                .filter(|b| b.bounds.intersects(&search) && b.player)
                .map(|b| (b, f64::from(distance(b.position, f.position) as f32)))
                .fold(None::<(&Body, f64)>, |best, (b, d)| match best {
                    Some((_, bd)) if d >= bd => best,
                    _ => Some((b, d)),
                })
                .map(|(b, _)| b);
            if owner.is_none_or(|b| b.valid_owner) {
                self.owners.insert(f.id, owner.map(|b| b.id));
            }
        }
        let live: BTreeSet<i32> = t.fireballs.iter().map(|f| f.id).collect();
        self.seen.retain(|id| live.contains(id));
        let alive = |id: i32| id == t.me.id || t.bodies.iter().any(|b| b.id == id);
        self.owners
            .retain(|id, owner| live.contains(id) && owner.is_none_or(alive));
    }

    /// `selectTargetArrow`.
    fn select(
        &mut self,
        o: &Options,
        t: &Tick,
        claim: &mut Claim,
        clip: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
    ) {
        if self.target.is_some() && self.swing_ticks <= 0 {
            return;
        }
        let mut best: Option<(i32, f64)> = None;
        for id in self.owners.keys() {
            let Some(f) = t.fireballs.iter().find(|f| f.id == *id) else {
                continue;
            };
            let d = predict(&t.me, &t.bodies, f, clip);
            if !incoming(&t.me, f) || d > 6.0 || best.is_some_and(|(_, b)| d >= b) {
                continue;
            }
            let rot = rotation_to(&t.me, f.position);
            let yaw_diff = wrap(-(t.me.yaw - rot.0)).abs();
            let pitch_diff = t.me.pitch - rot.1;
            if f64::from(yaw_diff) > o.angle_limit || f64::from(pitch_diff) > o.angle_limit / 2.0 {
                continue;
            }
            best = Some((*id, d));
        }
        if let Some((id, _)) = best
            && self.target != Some(id)
            && (Self::owned(claim) || self.acquire(claim, t, o.silent_aim))
        {
            self.swing_ticks = 0;
            self.triggered = false;
            self.target = Some(id);
        }
    }

    /// `onTick`, then the rotation manager's updates for the installed
    /// controller.
    pub fn tick(
        &mut self,
        o: &Options,
        t: &Tick,
        claim: &mut Claim,
        clip: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
    ) -> Effects {
        let mut fx = Effects::default();
        let mut view = (t.me.yaw, t.me.pitch);
        if let Some(c) = self.controller.as_mut().filter(|_| self.installed) {
            c.follow_view(view);
        }
        self.movement.forced = false;
        self.module_tick(o, t, claim, clip, view, &mut fx);
        // RotationManager.onTick (LOWEST).
        if self.installed
            && let Some(c) = self.controller.as_mut()
        {
            if c.complete && !c.should_retain() {
                self.installed = false;
            } else {
                let scale = mouse_scale(t.sensitivity);
                view.0 += fx.view_yaw_offset;
                for _ in 0..updates_per_tick(t.sensitivity) {
                    let offset = c.update(view, t.screen_open, scale);
                    view.0 += offset;
                    fx.view_yaw_offset += offset;
                    c.apply_pending(&mut view, t.screen_open, scale, &mut self.rng);
                }
                if !c.adaptive_kind() {
                    fx.view = Some(view);
                }
            }
        }
        if self.installed
            && let Some(c) = self.controller.as_ref()
        {
            fx.sent = Some(c.rendered(view));
        }
        fx
    }

    fn module_tick(
        &mut self,
        o: &Options,
        t: &Tick,
        claim: &mut Claim,
        clip: &mut dyn FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]>,
        view: (f32, f32),
        fx: &mut Effects,
    ) {
        self.track(t);
        if self.attack_pending {
            fx.release_attack = true;
            self.attack_pending = false;
        }
        let adaptive_active = self.controller().is_some_and(Controller::adaptive_kind);
        // releaseCompletedRotationClaim.
        if self
            .controller
            .as_ref()
            .is_some_and(|c| !c.should_retain() && c.complete)
        {
            self.controller = None;
            self.installed = false;
        }
        if self.controller.is_none() {
            claim.release(ME);
        }
        let can_acquire =
            t.focused && !t.blocked_by_screen && !Self::blocked(claim) && !self.toggled_off;
        if can_acquire {
            self.select(o, t, claim, clip);
        }
        if o.silent_aim && adaptive_active && o.stop_movement && !o.move_on_finish {
            self.suppress(o.stop_movement);
        }
        let Some(id) = self.target else {
            self.release_rotation(o.aim_speed, view, fx);
            return;
        };
        let fireball = t.fireballs.iter().find(|f| f.id == id);
        if let Some(f) = fireball {
            let aim_box = Aabb {
                min: [
                    f.position[0] - f.width,
                    f.position[1],
                    f.position[2] - f.width,
                ],
                max: [
                    f.position[0] + f.width,
                    f.position[1] + f.height,
                    f.position[2] + f.width,
                ],
            };
            let aim = rotation_to(&t.me, closest_point(&t.me, aim_box));
            if self.controller.is_none() {
                let mut c = if o.silent_aim {
                    Controller::adaptive(t.managed.unwrap_or(view), view)
                } else {
                    Controller::fixed(aim)
                };
                c.clamp = true;
                c.tolerance = 0.5;
                c.proportional = true;
                self.controller = Some(c);
                self.installed = true;
            }
            if let Some(c) = self.controller.as_mut() {
                if c.adaptive_kind() {
                    c.set_relative(false, view);
                }
                c.retain = true;
                c.cubic = true;
                c.speed = o.aim_speed * 1.5;
                c.set_target(aim);
                c.randomize = true;
            }
        }
        let threat = self.triggered
            || fireball
                .is_some_and(|f| incoming(&t.me, f) && predict(&t.me, &t.bodies, f, clip) <= 6.0);
        let can_acquire =
            t.focused && !t.blocked_by_screen && !Self::blocked(claim) && !self.toggled_off;
        if fireball.is_none() || !Self::owned(claim) || !can_acquire || !threat {
            self.clear_target(o, t, view, fx);
            return;
        }
        self.suppress(o.stop_movement);
        if !self.triggered && t.crosshair == Some(id) {
            if t.attack_down {
                fx.release_attack = true;
                return;
            }
            fx.press_attack = true;
            self.attack_pending = true;
            self.triggered = true;
            self.swing_ticks = 0;
        }
        if self.triggered {
            if let Some(c) = self.controller.as_mut() {
                c.speed = 2.0;
            }
            let ticks = self.swing_ticks;
            self.swing_ticks += 1;
            if ticks > 5 {
                self.clear_target(o, t, view, fx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.validate_conditions().unwrap();
        Options::read(&s)
    }
    fn me() -> Me {
        Me {
            id: 1,
            position: [0.0, 64.0, 0.0],
            previous: [0.0, 64.0, 0.0],
            eye_height: 1.62,
            bounds: Aabb {
                min: [-0.3, 64.0, -0.3],
                max: [0.3, 65.8, 0.3],
            },
            yaw: 0.0,
            pitch: 0.0,
        }
    }
    /// A fireball `z` blocks ahead (yaw 0 looks along +z), flying back at
    /// the player.
    fn fireball(id: i32, z: f64, speed: f64) -> Fireball {
        Fireball {
            id,
            position: [0.0, 64.6, z],
            previous: [0.0, 64.6, z + speed],
            velocity: [0.0, 0.0, -speed],
            acceleration: 0.1,
            age: 10,
            width: 1.0,
            height: 1.0,
        }
    }
    fn tick(fireballs: Vec<Fireball>) -> Tick {
        Tick {
            me: me(),
            fireballs,
            focused: true,
            sensitivity: 0.5,
            ..Tick::default()
        }
    }
    fn no_clip() -> impl FnMut([f64; 3], [f64; 3]) -> Option<[f64; 3]> {
        |_, _| None
    }

    #[test]
    fn defaults_match_the_reference() {
        let o = options();
        assert_eq!((o.angle_limit, o.aim_speed), (45.0, 9.0));
        assert!(!o.stop_movement && !o.move_on_finish && !o.silent_aim);
        assert_eq!(updates_per_tick(0.5), 50);
    }

    #[test]
    fn incoming_and_prediction_follow_the_reference() {
        let me = me();
        let f = fireball(5, 6.0, 0.5);
        assert!(incoming(&me, &f));
        let away = Fireball {
            velocity: [0.0, 0.0, 0.5],
            previous: [0.0, 64.6, 5.5],
            ..f
        };
        assert!(!incoming(&me, &away));
        let d = predict(&me, &[], &f, &mut no_clip());
        assert!(d <= 4.0, "{d}");
        // A wall in the way: the distance to where it hits.
        let wall = predict(&me, &[], &f, &mut |_, _| Some([0.0, 64.6, 5.0]));
        assert!((wall - 5.0).abs() < 0.1);
        // Just launched beside and behind the player (`RotationUtil.a`
        // reads 0 straight behind, so it is off to one side).
        let behind = Fireball {
            position: [0.5, 64.6, -2.0],
            previous: [0.5, 64.6, -2.4],
            velocity: [0.0, 0.0, 0.4],
            age: 2,
            ..f
        };
        assert!(!incoming(&me, &behind));
    }

    #[test]
    fn aims_clicks_and_clears_after_the_swing() {
        let o = options();
        let mut af = AntiFireball::default();
        let mut claim = Claim::default();
        let mut t = tick(vec![fireball(5, 6.0, 0.3)]);
        t.me.yaw = 10.0;
        let fx = af.tick(&o, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), Some(5));
        assert!(claim.owned_by(Claimant::BowAimbot));
        // The fixed controller turned the view toward it in whole counts.
        let (yaw, _) = fx.view.unwrap();
        assert!(yaw < 10.0, "{yaw}");
        assert_eq!(fx.sent, Some(None));
        t.me.yaw = yaw;
        t.crosshair = Some(5);
        let fx = af.tick(&o, &t, &mut claim, &mut no_clip());
        assert!(fx.press_attack && !fx.release_attack);
        // Deflected: it flies back out, but the swing still runs its ticks.
        t.fireballs = vec![Fireball {
            velocity: [0.0, 0.0, 0.4],
            previous: [0.0, 64.6, 5.6],
            ..fireball(5, 6.0, 0.3)
        }];
        let fx = af.tick(&o, &t, &mut claim, &mut no_clip());
        assert!(fx.release_attack && !fx.press_attack);
        for _ in 0..4 {
            af.tick(&o, &t, &mut claim, &mut no_clip());
            assert_eq!(af.target(), Some(5));
        }
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), None);
        // Released: the controller completes, then the claim goes.
        af.tick(&o, &tick(vec![]), &mut claim, &mut no_clip());
        af.tick(&o, &tick(vec![]), &mut claim, &mut no_clip());
        assert_eq!(claim.owner(), None);
        assert!(af.controller().is_none());
    }

    #[test]
    fn angle_limit_owner_filter_and_screens_skip_fireballs() {
        let o = Options {
            angle_limit: 10.0,
            ..options()
        };
        let mut af = AntiFireball::default();
        let mut claim = Claim::default();
        let mut t = tick(vec![fireball(5, 6.0, 0.3)]);
        t.me.yaw = 40.0;
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), None);
        // A friend's fireball is never recorded.
        let mut af = AntiFireball::default();
        let mut t = tick(vec![fireball(6, 1.5, 0.3)]);
        t.bodies.push(Body {
            id: 9,
            position: [0.0, 64.0, 2.5],
            bounds: Aabb {
                min: [-0.3, 64.0, 2.2],
                max: [0.3, 65.8, 2.8],
            },
            player: true,
            valid_owner: false,
        });
        t.me.position = [0.0, 64.0, -3.0];
        t.me.previous = t.me.position;
        t.me.bounds = Aabb {
            min: [-0.3, 64.0, -3.3],
            max: [0.3, 65.8, -2.7],
        };
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert!(af.owners.is_empty() && af.target().is_none());
        // A screen InvWalk does not handle blocks acquiring.
        let mut af = AntiFireball::default();
        let mut t = tick(vec![fireball(5, 6.0, 0.3)]);
        t.blocked_by_screen = true;
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), None);
    }

    #[test]
    fn silent_aim_sends_an_adaptive_rotation_and_defers_disable() {
        let o = Options {
            silent_aim: true,
            stop_movement: true,
            ..options()
        };
        let mut af = AntiFireball::default();
        let mut claim = Claim::default();
        let mut t = tick(vec![fireball(5, 6.0, 0.3)]);
        t.me.yaw = 20.0;
        let fx = af.tick(&o, &t, &mut claim, &mut no_clip());
        assert!(fx.view.is_none());
        let (yaw, _) = fx.sent.unwrap().unwrap();
        assert!(yaw < 20.0, "{yaw}");
        assert!(af.movement_locked());
        assert_eq!(af.held_up([true, false, false, false]), [true; 4]);
        // Disabling flips toggledOff instead, and acquiring stops.
        assert!(af.defer_disable(false));
        assert!(af.toggled_off());
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), None);
        assert!(!af.movement_locked());
        // Keys set up this tick; W stays up while it is still held.
        assert_eq!(af.held_up([true, false, false, false]), [true; 4]);
        // The silent rotation returns to the view (Stop movement keeps
        // suppressing meanwhile), then drops.
        for _ in 0..40 {
            af.tick(&o, &t, &mut claim, &mut no_clip());
        }
        assert!(af.controller().is_none());
        assert_eq!(claim.owner(), None);
        // The reference never unlocks after that return: no clearTarget
        // runs, so the lock waits for the next cleared target.
        assert!(af.movement_locked());
        assert_eq!(
            af.held_up([true, false, false, false]),
            [true, false, false, false]
        );
        assert_eq!(af.held_up([false; 4]), [false; 4]);
        assert_eq!(af.held_up([true, false, false, false]), [false; 4]);
        // With the controller gone the next disable is real.
        assert!(!af.defer_disable(false));
        assert!(!af.toggled_off());
    }

    #[test]
    fn move_on_finish_hands_the_keys_back_at_once() {
        let o = Options {
            stop_movement: true,
            move_on_finish: true,
            ..options()
        };
        let mut af = AntiFireball::default();
        let mut claim = Claim::default();
        let mut t = tick(vec![fireball(5, 6.0, 0.3)]);
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert!(af.movement_locked());
        assert_eq!(af.held_up([true, false, true, false]), [true; 4]);
        // The fireball turns away: the target clears and the keys return.
        t.fireballs = vec![Fireball {
            velocity: [0.0, 0.0, 0.4],
            previous: [0.0, 64.6, 5.6],
            ..fireball(5, 6.0, 0.3)
        }];
        af.tick(&o, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), None);
        assert!(!af.movement_locked());
        assert_eq!(af.held_up([true, false, true, false]), [false; 4]);
        // A real disable releases the claim.
        af.reset(&mut claim);
        assert_eq!(claim.owner(), None);
    }

    #[test]
    fn higher_priority_owner_blocks_and_silent_preempts_silent_aura() {
        let o = options();
        let mut claim = Claim::default();
        assert!(claim.try_claim(Claimant::AutoAnchor));
        assert!(AntiFireball::blocked(&claim));
        let mut claim = Claim::default();
        assert!(claim.try_claim(Claimant::SilentAura));
        assert!(!AntiFireball::blocked(&claim));
        let mut af = AntiFireball::default();
        let mut t = tick(vec![fireball(5, 6.0, 0.3)]);
        t.managed = Some((0.0, 0.0));
        af.tick(&o, &t, &mut claim, &mut no_clip());
        // Without Silent aim it only queues behind SilentAura.
        assert_eq!(af.target(), None);
        assert_eq!(claim.pending(), Some(Claimant::BowAimbot));
        let silent = Options {
            silent_aim: true,
            ..o
        };
        let mut claim = Claim::default();
        assert!(claim.try_claim(Claimant::SilentAura));
        af.tick(&silent, &t, &mut claim, &mut no_clip());
        assert_eq!(af.target(), Some(5));
        assert!(claim.owned_by(Claimant::BowAimbot));
    }

    #[test]
    fn fixed_controller_steps_in_mouse_counts_with_the_reference_math() {
        let mut c = Controller::fixed((30.0, 0.0));
        c.clamp = true;
        c.tolerance = 0.5;
        c.speed = 9.0 * 1.5;
        c.cubic = true;
        let scale = mouse_scale(0.5);
        let mut rng = Rng::new(1);
        let mut view = (0.0f32, 0.0f32);
        for _ in 0..50 {
            c.update(view, false, scale);
            c.apply_pending(&mut view, false, scale, &mut rng);
        }
        let counts = view.0 / (scale * 0.15);
        assert!((counts - counts.round()).abs() < 1e-3);
        assert!(view.0 > 0.0 && view.0 <= 30.0 + 0.2);
        for _ in 0..500 {
            c.update(view, false, scale);
            c.apply_pending(&mut view, false, scale, &mut rng);
        }
        assert!(c.complete());
        assert!((view.0 - 30.0).abs() <= 0.5);
        // A screen stops the fixed controller.
        let mut c = Controller::fixed((90.0, 0.0));
        let mut v = (0.0, 0.0);
        c.update(v, true, scale);
        c.apply_pending(&mut v, true, scale, &mut rng);
        assert_eq!(v, (0.0, 0.0));
    }

    #[test]
    fn rotation_helpers_match_the_reference_quadrants() {
        let me = me();
        let (yaw, pitch) = rotation_to(&me, [0.0, 64.0, 5.0]);
        assert!(yaw.abs() < 1e-4 && pitch.abs() < 1e-4);
        let (yaw, _) = rotation_to(&me, [-5.0, 64.0, -0.0001]);
        assert!((yaw - 90.0).abs() < 0.1, "{yaw}");
        let (yaw, _) = rotation_to(&me, [5.0, 64.0, -5.0]);
        assert!((yaw + 135.0).abs() < 0.1, "{yaw}");
        // Pitch is measured from the feet.
        let (_, pitch) = rotation_to(&me, [0.0, 65.0, 1.0]);
        assert!((pitch + 45.0).abs() < 0.1, "{pitch}");
        assert_eq!(yaw_off(&me, [0.0, 64.0, -5.0]), 0);
        assert_eq!(yaw_off(&me, [5.0, 64.0, -5.0]), 135);
        let p = closest_point(
            &me,
            Aabb {
                min: [-1.0, 64.0, 4.0],
                max: [1.0, 65.0, 6.0],
            },
        );
        assert_eq!(p, [0.01, 65.0, 4.0]);
    }
}
