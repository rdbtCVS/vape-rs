/// Pure targeting and rotation primitives. Host snapshots and action dispatch are separate.
pub mod entities;
pub mod filter;
use super::core::lifecycle::{EntityHandle, Epoch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode {
    Distance,
    Yaw,
    Armor,
    Threat,
    Health,
}

#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub handle: EntityHandle,
    pub distance: f64,
    /// Absolute yaw difference in degrees.
    pub yaw: f64,
    pub health: f64,
    pub armor: f64,
    pub threat: f64,
    pub player: bool,
    pub friend: bool,
    pub enemy: bool,
    pub same_team: bool,
    pub visible: bool,
    pub alive: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Filter {
    pub epoch: Epoch,
    pub max_distance: f64,
    pub max_yaw: f64,
    pub exclude_friends: bool,
    pub enemies_only: bool,
    pub exclude_team: bool,
    pub require_visible: bool,
}

/// Select from a single live session. Equal scores preserve host enumeration order.
/// Mixed player/non-player equipment modes use distance for the entire selection,
/// avoiding the non-transitive pairwise comparator in the reference.
pub fn select(candidates: &[Candidate], filter: Filter, mode: SortMode) -> Option<EntityHandle> {
    if !filter.max_distance.is_finite()
        || !filter.max_yaw.is_finite()
        || filter.max_distance < 0.0
        || filter.max_yaw < 0.0
    {
        return None;
    }
    let eligible: Vec<_> = candidates
        .iter()
        .filter(|c| {
            c.handle.epoch == filter.epoch
                && c.alive
                && c.distance.is_finite()
                && c.distance >= 0.0
                && c.yaw.is_finite()
                && c.yaw >= 0.0
                && c.health.is_finite()
                && c.distance <= filter.max_distance
                && c.yaw <= filter.max_yaw
                && (!filter.exclude_friends || !c.friend)
                && (!filter.enemies_only || c.enemy)
                && (!filter.exclude_team || !c.same_team)
                && (!filter.require_visible || c.visible)
        })
        .collect();
    let equipment = matches!(mode, SortMode::Armor | SortMode::Threat)
        && eligible
            .iter()
            .all(|c| c.player && c.armor.is_finite() && c.threat.is_finite());
    eligible
        .into_iter()
        .min_by(|a, b| {
            let score = |c: &Candidate| match mode {
                SortMode::Distance => c.distance,
                SortMode::Yaw => c.yaw,
                SortMode::Health => c.health,
                SortMode::Armor if equipment => c.armor,
                SortMode::Threat if equipment => c.threat,
                _ => c.distance,
            };
            score(a).total_cmp(&score(b))
        })
        .map(|c| c.handle)
}

/// Move toward a desired view, using the shortest wrapped yaw and bounded pitch.
/// Invalid inputs are rejected; callers retain the host view on None.
pub fn smooth_rotation(
    current: (f64, f64),
    desired: (f64, f64),
    max_yaw: f64,
    max_pitch: f64,
) -> Option<(f64, f64)> {
    if ![
        current.0, current.1, desired.0, desired.1, max_yaw, max_pitch,
    ]
    .iter()
    .all(|v| v.is_finite())
        || max_yaw < 0.0
        || max_pitch < 0.0
    {
        return None;
    }
    let delta = (desired.0 - current.0 + 180.0).rem_euclid(360.0) - 180.0;
    let yaw = (current.0 + delta.clamp(-max_yaw, max_yaw) + 180.0).rem_euclid(360.0) - 180.0;
    let pitch = current.1.clamp(-90.0, 90.0);
    let target_pitch = desired.1.clamp(-90.0, 90.0);
    Some((
        yaw,
        pitch + (target_pitch - pitch).clamp(-max_pitch, max_pitch),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::core::lifecycle::Session;
    use super::*;
    fn candidate(epoch: Epoch, id: i32) -> Candidate {
        Candidate {
            handle: EntityHandle { epoch, id },
            distance: 2.0,
            yaw: 20.0,
            health: 10.0,
            armor: 4.0,
            threat: 8.0,
            player: true,
            friend: false,
            enemy: false,
            same_team: false,
            visible: true,
            alive: true,
        }
    }
    fn filter(epoch: Epoch) -> Filter {
        Filter {
            epoch,
            max_distance: 5.0,
            max_yaw: 90.0,
            exclude_friends: true,
            enemies_only: false,
            exclude_team: true,
            require_visible: true,
        }
    }
    #[test]
    fn filters_session_people_team_visibility_and_invalid_values() {
        let mut session = Session::default();
        let epoch = session.epoch();
        let mut a = candidate(epoch, 1);
        let mut b = candidate(epoch, 2);
        a.friend = true;
        b.same_team = true;
        assert_eq!(select(&[a, b], filter(epoch), SortMode::Distance), None);
        b.same_team = false;
        b.visible = false;
        assert_eq!(select(&[a, b], filter(epoch), SortMode::Distance), None);
        b.visible = true;
        b.distance = f64::NAN;
        assert_eq!(select(&[a, b], filter(epoch), SortMode::Distance), None);
        b.distance = 2.0;
        assert_eq!(
            select(&[a, b], filter(epoch), SortMode::Distance),
            Some(b.handle)
        );
        session.advance();
        assert_eq!(
            select(&[b], filter(session.epoch()), SortMode::Distance),
            None
        );
    }
    #[test]
    fn scores_and_mixed_entity_fallback_are_deterministic() {
        let e = Session::default().epoch();
        let mut a = candidate(e, 1);
        let mut b = candidate(e, 2);
        a.distance = 1.0;
        a.yaw = 30.0;
        a.health = 15.0;
        a.armor = 8.0;
        a.threat = 9.0;
        b.distance = 3.0;
        b.yaw = 2.0;
        b.health = 2.0;
        b.armor = 1.0;
        b.threat = 1.0;
        for mode in [
            SortMode::Yaw,
            SortMode::Health,
            SortMode::Armor,
            SortMode::Threat,
        ] {
            assert_eq!(select(&[a, b], filter(e), mode), Some(b.handle));
        }
        assert_eq!(
            select(&[a, b], filter(e), SortMode::Distance),
            Some(a.handle)
        );
        b.player = false;
        assert_eq!(select(&[a, b], filter(e), SortMode::Armor), Some(a.handle));
        b.distance = a.distance;
        assert_eq!(
            select(&[a, b], filter(e), SortMode::Distance),
            Some(a.handle)
        );
    }
    #[test]
    fn rotation_wraps_and_limits_axes() {
        assert_eq!(
            smooth_rotation((179.0, 0.0), (-179.0, 100.0), 1.0, 10.0),
            Some((-180.0, 10.0))
        );
        assert_eq!(smooth_rotation((0.0, 0.0), (1.0, 1.0), -1.0, 1.0), None);
        assert_eq!(smooth_rotation((f64::NAN, 0.0), (0.0, 0.0), 1.0, 1.0), None);
    }
}
