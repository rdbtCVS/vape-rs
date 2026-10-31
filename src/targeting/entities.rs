// Owned per-frame entity snapshots built by the host adapter. Positions are
// interpolated at the render partial tick, like the reference's
// `Entity.c()/A()/Z()`; `tick_position` is the uninterpolated tick value.
use super::super::core::lifecycle::EntityHandle;

/// The reference target-filter classes, mapped onto the pinned host's
/// entity categories (`EntityTargetFilterValue.isValidTarget`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityClass {
    /// Another player (`EntityOtherPlayerMP`).
    Player,
    /// `Monster`, slimes, ghasts and the ender dragon.
    Monster,
    /// Animals, villagers and snow golems.
    Peaceful,
    /// Ambient creatures (bats): only with Peaceful.
    Ambient,
    /// Any other living entity: only with Peaceful.
    Other,
}

impl EntityClass {
    /// Classify a host entity type id and spawn category.
    pub fn classify(id: &str, category: &str) -> Self {
        let id = id.trim_start_matches("minecraft:");
        match id {
            "player" => Self::Player,
            "slime" | "magma_cube" | "ghast" | "ender_dragon" => Self::Monster,
            "villager" | "snow_golem" => Self::Peaceful,
            _ => match category {
                "monster" => Self::Monster,
                "creature" | "axolotls" => Self::Peaceful,
                "ambient" => Self::Ambient,
                _ => Self::Other,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Bounds {
    pub fn center(&self) -> [f64; 3] {
        [0, 1, 2].map(|i| (self.min[i] + self.max[i]) / 2.0)
    }
    /// The point of the box closest to `from` (`RotationUtil.T`): an axis
    /// already inside the box is nudged by 0.01 like the reference.
    pub fn closest(&self, from: [f64; 3]) -> [f64; 3] {
        let mut out = [0, 1, 2].map(|i| from[i].clamp(self.min[i], self.max[i]));
        for i in [0, 2] {
            if out[i] == from[i] {
                out[i] = from[i] + 0.01;
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct EntityView {
    pub handle: EntityHandle,
    pub class: EntityClass,
    /// Player profile name or custom name; empty if none.
    pub name: String,
    pub position: [f64; 3],
    pub tick_position: [f64; 3],
    /// The tick (uninterpolated) bounding box.
    pub bounds: Bounds,
    pub health: f32,
    pub invisible: bool,
    /// Something is held in the main hand.
    pub holding: bool,
    /// Any armor piece is worn.
    pub armored: bool,
    /// Summed worn-armor score (`EntityEquipmentValueComparator`, the
    /// reference "Armor" ordering, weakest first).
    pub armor: f64,
    /// Held-weapon score with the Strength multiplier
    /// (`EntityArmorValueComparator`, the reference "Threat" ordering).
    pub threat: f64,
    /// Eye-to-eye line of sight from the local player.
    pub visible: bool,
    /// The local player rides this entity.
    pub vehicle: bool,
    /// Raising a shield (`RotationUtil.n`).
    pub blocking: bool,
    /// Head yaw and pitch, when the host tracks them.
    pub look: Option<(f32, f32)>,
    /// Motion per tick (`motionX/Y/Z`); zero where the host keeps none.
    pub motion: [f64; 3],
}

impl EntityView {
    pub fn alive(&self) -> bool {
        self.health > 0.0
    }
    /// `RotationUtil.k`: invisible and showing nothing.
    pub fn fully_invisible(&self) -> bool {
        self.invisible && !self.holding && !self.armored
    }
    /// `RotationUtil.b`: a player with no held item and no armor.
    pub fn naked(&self) -> bool {
        self.class == EntityClass::Player && !self.holding && !self.armored
    }
}

/// The local player at the same partial tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerView {
    pub position: [f64; 3],
    pub tick_position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// `MovementInput.moveStrafe`: +1 left, -1 right.
    pub strafe: f32,
}

impl PlayerView {
    pub fn distance(&self, to: [f64; 3]) -> f64 {
        distance(self.tick_position, to)
    }
}

pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// The reference's quadrant atan yaw from `(x, z)` toward `(tx, tz)`. An
/// axis-aligned delta (one component zero) yields 0, as the reference does.
fn yaw_to(x: f64, z: f64, tx: f64, tz: f64) -> f64 {
    let dx = tx - x;
    let dz = tz - z;
    if dz > 0.0 && dx != 0.0 {
        (-(dx / dz).atan()).to_degrees()
    } else if dz < 0.0 && dx > 0.0 {
        -90.0 + (dz / dx).atan().to_degrees()
    } else if dz < 0.0 && dx < 0.0 {
        90.0 + (dz / dx).atan().to_degrees()
    } else {
        0.0
    }
}

pub fn wrap_degrees(angle: f64) -> f64 {
    let a = angle % 360.0;
    if a >= 180.0 {
        a - 360.0
    } else if a < -180.0 {
        a + 360.0
    } else {
        a
    }
}

/// `RotationUtil.C(x, z, yaw, tx, tz)`: absolute yaw difference, 0..=180.
pub fn yaw_difference(x: f64, z: f64, yaw: f32, tx: f64, tz: f64) -> f64 {
    let d = (yaw_to(x, z, tx, tz) - f64::from(yaw)).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

/// `RotationUtil.a(player, target)`: the integer yaw difference from tick
/// positions that the target validity check uses.
pub fn int_yaw_difference(player: &PlayerView, target: [f64; 3]) -> i32 {
    let n = ((yaw_to(
        player.tick_position[0],
        player.tick_position[2],
        target[0],
        target[2],
    ) - f64::from(player.yaw))
    .abs()
        % 360.0) as i32;
    if n > 180 { 360 - n } else { n }
}

/// `RotationUtil.p`: the target is on the left of the view.
pub fn on_left(x: f64, z: f64, yaw: f32, tx: f64, tz: f64) -> bool {
    (wrap_degrees((yaw_to(x, z, tx, tz) - f64::from(yaw)) % 360.0) as i32) < 0
}

/// `RotationUtil.H`: integer pitch difference from the player's feet to a point.
pub fn pitch_difference(player: &PlayerView, target: [f64; 3]) -> i32 {
    let dx = target[0] - player.tick_position[0];
    let dy = target[1] - player.tick_position[1];
    let dz = target[2] - player.tick_position[2];
    let horizontal = (dx * dx + dz * dz).sqrt();
    let pitch = -(dy.atan2(horizontal).to_degrees()) as f32;
    wrap_degrees(f64::from(player.pitch - pitch)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_reference_groups() {
        assert_eq!(
            EntityClass::classify("minecraft:zombie", "monster"),
            EntityClass::Monster
        );
        assert_eq!(
            EntityClass::classify("minecraft:slime", "monster"),
            EntityClass::Monster
        );
        assert_eq!(
            EntityClass::classify("minecraft:cow", "creature"),
            EntityClass::Peaceful
        );
        assert_eq!(
            EntityClass::classify("minecraft:villager", "misc"),
            EntityClass::Peaceful
        );
        assert_eq!(
            EntityClass::classify("minecraft:bat", "ambient"),
            EntityClass::Ambient
        );
        assert_eq!(
            EntityClass::classify("minecraft:iron_golem", "misc"),
            EntityClass::Other
        );
        assert_eq!(
            EntityClass::classify("minecraft:player", "misc"),
            EntityClass::Player
        );
    }
    #[test]
    fn angles_follow_minecraft_yaw() {
        // Yaw 0 faces +Z; a target at +Z is straight ahead.
        assert!(yaw_difference(0.0, 0.0, 0.0, 1.0, 5.0) < 12.0);
        // Facing +Z, +X is on the left (yaw -90 looks toward +X).
        assert!(on_left(0.0, 0.0, 0.0, 3.0, 3.0));
        assert!(!on_left(0.0, 0.0, 0.0, -3.0, 3.0));
        assert!((yaw_difference(0.0, 0.0, 0.0, 1.0, -1.0) - 135.0).abs() < 1e-9);
        let player = PlayerView {
            yaw: 0.0,
            ..PlayerView::default()
        };
        assert_eq!(int_yaw_difference(&player, [-1.0, 0.0, -1.0]), 135);
    }
    #[test]
    fn closest_point_nudges_inside_axes() {
        let b = Bounds {
            min: [-0.3, 0.0, 4.7],
            max: [0.3, 1.8, 5.3],
        };
        assert_eq!(b.closest([0.0, 1.6, 0.0]), [0.01, 1.6, 4.7]);
    }
}
