use super::super::core::settings::{Definition, Settings};
/// The reference `EntityTargetFilterValue`: seven toggles shared by combat
/// modules, stored under `<module>.targets.*`.
use super::entities::{EntityClass, EntityView};
use serde_json::Value;

const KEYS: [(&str, &str, bool); 7] = [
    ("players", "Players", true),
    ("mobs", "Mobs", false),
    ("peaceful", "Peaceful", false),
    ("neutral", "Neutral", false),
    ("ignore-naked", "Ignore naked", false),
    ("ignore-invisible", "Ignore invisible", false),
    ("ignore-walls", "Ignore behind walls", false),
];

pub fn definitions(module: &str) -> Vec<Definition> {
    KEYS.iter()
        .map(|(key, label, default)| {
            Definition::boolean(&format!("{module}.targets.{key}"), label, *default)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetFilter {
    pub players: bool,
    pub mobs: bool,
    pub peaceful: bool,
    /// Shown in the reference summary only; it has no filtering effect there.
    pub neutral: bool,
    pub ignore_naked: bool,
    pub ignore_invisible: bool,
    pub ignore_walls: bool,
}

impl Default for TargetFilter {
    fn default() -> Self {
        Self {
            players: true,
            mobs: false,
            peaceful: false,
            neutral: false,
            ignore_naked: false,
            ignore_invisible: false,
            ignore_walls: false,
        }
    }
}

/// The local friend/enemy lists and the AntiBot/team services the filter
/// consults. Team and bot checks are only active with AntiBot, as in the
/// reference `ClientSettings.isTeammate/isBot`.
pub trait People {
    fn friend(&self, name: &str) -> bool;
    fn enemy(&self, name: &str) -> bool;
    fn teammate(&self, _entity: &EntityView) -> bool {
        false
    }
    fn bot(&self, _entity: &EntityView) -> bool {
        false
    }
    /// TargetFilter's "Recolor visuals" team colour (0xRRGGBB).
    fn team_color(&self, _entity: &EntityView) -> Option<u32> {
        None
    }
}

/// Lowercased name lists from the local friend/enemy records.
#[derive(Clone, Debug, Default)]
pub struct NameLists {
    pub friends: Vec<String>,
    pub enemies: Vec<String>,
}

impl People for NameLists {
    fn friend(&self, name: &str) -> bool {
        !name.is_empty() && self.friends.iter().any(|f| f.eq_ignore_ascii_case(name))
    }
    fn enemy(&self, name: &str) -> bool {
        !name.is_empty() && self.enemies.iter().any(|f| f.eq_ignore_ascii_case(name))
    }
}

impl TargetFilter {
    pub fn read(settings: &Settings, module: &str) -> Self {
        let get = |key: &str| {
            settings
                .get(&format!("{module}.targets.{key}"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        Self {
            players: get("players"),
            mobs: get("mobs"),
            peaceful: get("peaceful"),
            neutral: get("neutral"),
            ignore_naked: get("ignore-naked"),
            ignore_invisible: get("ignore-invisible"),
            ignore_walls: get("ignore-walls"),
        }
    }

    /// The reference summary shown beside the setting ("Players Mobs | NoWalls").
    pub fn summary(&self) -> String {
        let mut out = String::new();
        for (on, name) in [
            (self.players, "Players "),
            (self.mobs, "Mobs "),
            (self.peaceful, "Peaceful "),
            (self.neutral, "Neutral "),
        ] {
            if on {
                out.push_str(name);
            }
        }
        let mut value = out.trim().to_owned();
        if value.is_empty() {
            value = "None".into();
        }
        if self.ignore_naked || self.ignore_invisible || self.ignore_walls {
            out.push_str(" | ");
            for (on, name) in [
                (self.ignore_naked, "NoNaked "),
                (self.ignore_invisible, "NoInvis "),
                (self.ignore_walls, "NoWalls"),
            ] {
                if on {
                    out.push_str(name);
                }
            }
            value = out.trim().to_owned();
        }
        value
    }

    /// `EntityTargetFilterValue.isValidTarget`. The local player, armor
    /// stands and non-living entities never reach the snapshot.
    pub fn valid(&self, entity: &EntityView, people: &impl People) -> bool {
        if entity.class == EntityClass::Ambient && !self.peaceful {
            return false;
        }
        if !entity.alive() {
            return false;
        }
        if self.ignore_invisible && entity.fully_invisible() {
            return false;
        }
        if self.ignore_walls && !entity.visible {
            return false;
        }
        if people.friend(&entity.name) {
            return false;
        }
        match entity.class {
            EntityClass::Player => {
                if !self.players {
                    return false;
                }
                if people.enemy(&entity.name) {
                    return true;
                }
                if self.ignore_naked && entity.naked() {
                    return false;
                }
                !people.teammate(entity) && !people.bot(entity)
            }
            EntityClass::Monster => self.mobs,
            EntityClass::Peaceful => self.peaceful,
            EntityClass::Ambient | EntityClass::Other => self.peaceful,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::super::core::lifecycle::{EntityHandle, Session};
    use super::super::entities::Bounds;
    use super::*;

    pub(crate) fn entity(class: EntityClass, name: &str, at: [f64; 3]) -> EntityView {
        EntityView {
            handle: EntityHandle {
                epoch: Session::default().epoch(),
                id: 7,
            },
            class,
            name: name.into(),
            position: at,
            tick_position: at,
            bounds: Bounds {
                min: [at[0] - 0.3, at[1], at[2] - 0.3],
                max: [at[0] + 0.3, at[1] + 1.8, at[2] + 0.3],
            },
            health: 20.0,
            invisible: false,
            holding: true,
            armored: false,
            armor: 0.0,
            threat: 0.0,
            visible: true,
            vehicle: false,
            blocking: false,
            look: None,
        }
    }

    #[test]
    fn reference_filter_rules() {
        let people = NameLists {
            friends: vec!["Alex".into()],
            enemies: vec!["Steve".into()],
        };
        let f = TargetFilter::default();
        let player = entity(EntityClass::Player, "Herobrine", [0.0; 3]);
        assert!(f.valid(&player, &people));
        assert!(!f.valid(&entity(EntityClass::Player, "alex", [0.0; 3]), &people));
        assert!(!f.valid(&entity(EntityClass::Monster, "", [0.0; 3]), &people));
        assert!(!f.valid(&entity(EntityClass::Other, "", [0.0; 3]), &people));
        let mut naked = player.clone();
        naked.holding = false;
        let strict = TargetFilter {
            ignore_naked: true,
            ..f
        };
        assert!(!strict.valid(&naked, &people));
        // An enemy bypasses the naked check.
        naked.name = "Steve".into();
        assert!(strict.valid(&naked, &people));
        let mut hidden = player.clone();
        hidden.visible = false;
        assert!(
            !TargetFilter {
                ignore_walls: true,
                ..f
            }
            .valid(&hidden, &people)
        );
        let all = TargetFilter {
            mobs: true,
            peaceful: true,
            ..f
        };
        assert!(all.valid(&entity(EntityClass::Ambient, "", [0.0; 3]), &people));
        assert!(all.valid(&entity(EntityClass::Other, "", [0.0; 3]), &people));
        let mut dead = player;
        dead.health = 0.0;
        assert!(!all.valid(&dead, &people));
        assert_eq!(f.summary(), "Players");
        // The reference keeps the trailing separator space.
        assert_eq!(strict.summary(), "Players  | NoNaked");
    }
}
