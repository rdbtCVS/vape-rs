// TargetFilter (reference `AntiBot`, shown as "TargetFilter"). On 26.3
// (>= 1.21.11) its team checks read the player list: "Teams by color" treats
// another player as a teammate when both names resolve to the same colour
// (`AntiBotEntityCache.getTeamColor`), "Teams by server" when both scoreboard
// teams share a prefix or a name, and "Recolor visuals" paints ESP and Tracers
// with that colour. "Auto-Detect color" and the 16 "Your team color" options
// are legacy-only; 26.3 shows the detected colour instead. `isBot` returns
// false outside 1.7, so the AntiBot toggle never removes a target here.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::targeting::entities::EntityView;
use super::super::super::targeting::filter::{NameLists, People};
use serde_json::{Value, json};
use std::collections::HashMap;

pub const ID: &str = "utility.target-filter";

/// `AntiBotEntityCache.NO_TEAM_COLOR`: a white team colour counts as none.
const NO_TEAM_COLOR: u32 = 0xff_ffff;

pub fn definitions() -> Vec<Definition> {
    let key = |k: &str| format!("{ID}.{k}");
    let mut detected = Definition::new(
        &key("detected-color"),
        "Your team color",
        Kind::Text { max_chars: 7 },
        json!("None"),
    )
    .when(&key("teams-by-color"), json!(true));
    detected.description = "Shows your detected team color".into();
    vec![
        Definition::boolean(&key("teams-by-server"), "Teams by server", false),
        Definition::boolean(&key("teams-by-color"), "Teams by color", false),
        Definition::boolean(&key("recolor-visuals"), "Recolor visuals", false)
            .when(&key("teams-by-color"), json!(true)),
        detected,
        Definition::boolean(&key("antibot"), "AntiBot", false),
    ]
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub teams_by_server: bool,
    pub teams_by_color: bool,
    pub recolor_visuals: bool,
    pub antibot: bool,
}

impl Options {
    /// The effective options of an enabled module (all off when disabled).
    pub fn read(settings: &Settings, enabled: bool) -> Self {
        let get = |k: &str| {
            enabled
                && settings
                    .effective(&format!("{ID}.{k}"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
        };
        Self {
            teams_by_server: get("teams-by-server"),
            teams_by_color: get("teams-by-color"),
            recolor_visuals: get("teams-by-color") && get("recolor-visuals"),
            antibot: get("antibot"),
        }
    }
    fn filtering(&self) -> bool {
        self.teams_by_server || self.teams_by_color
    }
}

/// A scoreboard team as `isTeammate` compares it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Team {
    pub name: String,
    /// `ScorePlayerTeam.getPrefix`, as plain text.
    pub prefix: String,
    /// The team's formatting colour (`ScorePlayerTeam.getColor().getColor()`).
    pub color: Option<u32>,
}

/// One player-list entry (`PlayerInfo`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub name: String,
    /// The display-name component, as JSON.
    pub display: Option<Value>,
    /// `getDisplayName().getString()`.
    pub display_text: Option<String>,
    /// `PlayerInfo.getTeam`.
    pub team: Option<Team>,
}

/// The player list and teams one client tick sees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub local: String,
    /// `getTeam(localPlayer)`.
    pub local_team: Option<Team>,
    pub entries: Vec<Entry>,
    /// `getTeam` of other players by profile name (scoreboard, else list).
    pub teams: HashMap<String, Team>,
}

/// `AntiBotEntityCache.sanitizeName`: letters, digits and underscores only.
fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// `TextFormatting.fromName(name).getColor()` for the 16 colours.
pub fn named_color(name: &str) -> Option<u32> {
    Some(match name.to_ascii_lowercase().as_str() {
        "black" => 0x00_0000,
        "dark_blue" => 0x00_00aa,
        "dark_green" => 0x00_aa00,
        "dark_aqua" => 0x00_aaaa,
        "dark_red" => 0xaa_0000,
        "dark_purple" => 0xaa_00aa,
        "gold" => 0xff_aa00,
        "gray" => 0xaa_aaaa,
        "dark_gray" => 0x55_5555,
        "blue" => 0x55_55ff,
        "green" => 0x55_ff55,
        "aqua" => 0x55_ffff,
        "red" => 0xff_5555,
        "light_purple" => 0xff_55ff,
        "yellow" => 0xff_ff55,
        "white" => 0xff_ffff,
        _ => return None,
    })
}

/// `AntiBotEntityCache.parseColor`: `#RRGGBB` or a formatting name.
fn parse_color(name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    match name.strip_prefix('#') {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => named_color(name),
    }
}

/// `Component.getString` of a JSON component: its text and its siblings'.
fn plain(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Array(list) => list.iter().map(plain).collect(),
        Value::Object(o) => {
            let own = match (o.get("text"), o.get("translate")) {
                (Some(t), _) => t.as_str().map(str::to_owned).unwrap_or_default(),
                (None, Some(key)) => o
                    .get("fallback")
                    .or(Some(key))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                _ => String::new(),
            };
            let extra: String = o
                .get("extra")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(plain)
                .collect();
            own + &extra
        }
        _ => String::new(),
    }
}

/// `extractText`: a literal's own text, else the sanitized full string.
fn own_text(c: &Value) -> Option<String> {
    match c {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => match o.get("text") {
            Some(t) => t.as_str().map(str::to_owned),
            None => Some(sanitize(&plain(c))),
        },
        _ => None,
    }
}

/// `namesMatch`: equal, or equal once sanitized.
fn names_match(text: &str, name: &str) -> bool {
    text == name || sanitize(text) == name
}

/// `findColorInComponent`: the own style colour of the first component (depth
/// first) whose text is the player name.
fn color_in(c: &Value, name: &str) -> Option<u32> {
    // A JSON list is its first element with the rest as siblings.
    if let Value::Array(list) = c {
        let (first, rest) = list.split_first()?;
        return color_in(first, name).or_else(|| rest.iter().find_map(|s| color_in(s, name)));
    }
    if own_text(c).is_some_and(|t| names_match(&t, name)) {
        let style = c.get("color").and_then(Value::as_str).and_then(parse_color);
        if style.is_some() {
            return style;
        }
    }
    c.get("extra")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|s| color_in(s, name))
}

/// `AntiBotEntityCache.getTeamColor`: the colour of the player's name in a
/// player-list display name containing it, else the first own entry's team
/// colour other than white.
pub fn team_color(snapshot: &Snapshot, name: &str) -> Option<u32> {
    let mut team = None;
    for entry in &snapshot.entries {
        if let Some(display) = &entry.display {
            if !entry
                .display_text
                .as_deref()
                .is_some_and(|t| t.contains(name))
            {
                continue;
            }
            if let Some(color) = color_in(display, name) {
                return Some(color);
            }
        }
        if team.is_some() || entry.name != name {
            continue;
        }
        team = entry
            .team
            .as_ref()
            .and_then(|t| t.color)
            .filter(|c| *c != NO_TEAM_COLOR);
    }
    team
}

/// `AntiBot.isTeammate` for another player, by profile name.
pub fn teammate(options: &Options, snapshot: &Snapshot, name: &str) -> bool {
    if !options.filtering() || name.is_empty() || name == snapshot.local {
        return false;
    }
    if options.teams_by_color {
        // `hasSameTeamColor`
        let own = team_color(snapshot, &snapshot.local);
        if own.is_some() && own == team_color(snapshot, name) {
            return true;
        }
    }
    if options.teams_by_server {
        let (Some(own), Some(other)) = (&snapshot.local_team, snapshot.teams.get(name)) else {
            return false;
        };
        // Equal prefixes (two empty ones too) or the same team.
        return own.prefix == other.prefix || own.name == other.name;
    }
    false
}

/// The detected colour as `AntiBotBooleanValue.getDisplayValue` shows it.
pub fn detected_text(color: u32) -> String {
    format!("#{:06X}", color & 0xff_ffff)
}

/// The targeting people for one tick: name lists plus the team verdicts.
#[derive(Clone, Debug, Default)]
pub struct Targets {
    pub names: NameLists,
    /// Profile names judged teammates.
    pub teammates: Vec<String>,
    /// Recolour-visuals colours by profile name (0xRRGGBB).
    pub colors: HashMap<String, u32>,
}

impl Targets {
    pub fn new(names: NameLists, options: &Options, snapshot: &Snapshot) -> Self {
        let mut players: Vec<&str> = snapshot.entries.iter().map(|e| e.name.as_str()).collect();
        players.extend(snapshot.teams.keys().map(String::as_str));
        players.sort_unstable();
        players.dedup();
        let teammates = players
            .iter()
            .filter(|p| teammate(options, snapshot, p))
            .map(|p| (*p).to_owned())
            .collect();
        let colors = if options.recolor_visuals {
            players
                .iter()
                .filter(|p| **p != snapshot.local)
                .filter_map(|p| Some(((*p).to_owned(), team_color(snapshot, p)?)))
                .collect()
        } else {
            HashMap::new()
        };
        Self {
            names,
            teammates,
            colors,
        }
    }
}

impl People for Targets {
    fn friend(&self, name: &str) -> bool {
        self.names.friend(name)
    }
    fn enemy(&self, name: &str) -> bool {
        self.names.enemy(name)
    }
    fn teammate(&self, entity: &EntityView) -> bool {
        !entity.name.is_empty() && self.teammates.contains(&entity.name)
    }
    fn team_color(&self, entity: &EntityView) -> Option<u32> {
        self.colors.get(&entity.name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::targeting::entities::EntityClass;
    use super::super::super::super::targeting::filter::tests::entity;
    use super::*;

    fn team(name: &str, prefix: &str, color: Option<u32>) -> Team {
        Team {
            name: name.into(),
            prefix: prefix.into(),
            color,
        }
    }
    fn entry(name: &str, display: Option<Value>, team: Option<Team>) -> Entry {
        Entry {
            name: name.into(),
            display_text: display.as_ref().map(plain),
            display,
            team,
        }
    }
    fn on() -> Options {
        Options {
            teams_by_server: true,
            teams_by_color: true,
            recolor_visuals: true,
            antibot: false,
        }
    }

    #[test]
    fn display_name_colour_wins_over_the_team_colour() {
        let display = json!({"text":"","extra":[{"text":"[RED] ","color":"gold"},{"text":"Alex","color":"red"}]});
        let s = Snapshot {
            local: "Steve".into(),
            entries: vec![entry(
                "Alex",
                Some(display),
                Some(team("t", "", Some(0x55_5555))),
            )],
            ..Snapshot::default()
        };
        assert_eq!(team_color(&s, "Alex"), Some(0xff_5555));
        // A hex colour and a sanitized match.
        let hex = json!({"text":"Alex!","color":"#123456"});
        let s = Snapshot {
            entries: vec![entry("Alex", Some(hex), None)],
            ..Snapshot::default()
        };
        assert_eq!(team_color(&s, "Alex"), Some(0x12_3456));
    }

    #[test]
    fn team_colour_falls_back_and_ignores_white() {
        let s = Snapshot {
            entries: vec![
                entry("Alex", None, Some(team("t", "", Some(NO_TEAM_COLOR)))),
                entry("Steve", None, Some(team("u", "", Some(0x55_ff55)))),
            ],
            ..Snapshot::default()
        };
        assert_eq!(team_color(&s, "Alex"), None);
        assert_eq!(team_color(&s, "Steve"), Some(0x55_ff55));
        // A display name without the player's name skips that entry's team.
        let s = Snapshot {
            entries: vec![entry(
                "Steve",
                Some(json!("Someone")),
                Some(team("u", "", Some(0x55_ff55))),
            )],
            ..Snapshot::default()
        };
        assert_eq!(team_color(&s, "Steve"), None);
        // An uncoloured name component searches its siblings.
        let nested = json!(["", {"text":"Steve","extra":[{"text":"Steve","color":"aqua"}]}]);
        let s = Snapshot {
            entries: vec![entry("Steve", Some(nested), None)],
            ..Snapshot::default()
        };
        assert_eq!(team_color(&s, "Steve"), Some(0x55_ffff));
    }

    #[test]
    fn teammates_by_colour_and_by_server_team() {
        let green = Some(team("green", "[G] ", Some(0x55_ff55)));
        let red = Some(team("red", "[R] ", Some(0xff_5555)));
        let mut s = Snapshot {
            local: "Me".into(),
            local_team: green.clone(),
            entries: vec![
                entry("Me", None, green.clone()),
                entry("Ally", None, green.clone()),
                entry("Foe", None, red.clone()),
            ],
            teams: HashMap::from([
                ("Ally".to_owned(), green.clone().unwrap()),
                ("Foe".to_owned(), red.clone().unwrap()),
            ]),
        };
        let colour = Options {
            teams_by_color: true,
            ..Options::default()
        };
        assert!(teammate(&colour, &s, "Ally"));
        assert!(!teammate(&colour, &s, "Foe"));
        assert!(!teammate(&colour, &s, "Me"));
        assert!(!teammate(&Options::default(), &s, "Ally"));
        let server = Options {
            teams_by_server: true,
            ..Options::default()
        };
        assert!(teammate(&server, &s, "Ally"));
        assert!(!teammate(&server, &s, "Foe"));
        // Same prefix on another team counts; so do two empty prefixes.
        s.teams.insert("Foe".into(), team("red", "[G] ", None));
        assert!(teammate(&server, &s, "Foe"));
        s.local_team = Some(team("a", "", None));
        s.teams.insert("Foe".into(), team("b", "", None));
        assert!(teammate(&server, &s, "Foe"));
        // No own team: nobody.
        s.local_team = None;
        assert!(!teammate(&server, &s, "Foe"));
    }

    #[test]
    fn targets_filter_teammates_and_recolour() {
        let green = Some(team("green", "", Some(0x55_ff55)));
        let s = Snapshot {
            local: "Me".into(),
            local_team: green.clone(),
            entries: vec![
                entry("Me", None, green.clone()),
                entry("Ally", None, green.clone()),
                entry("Foe", None, Some(team("red", "x", Some(0xff_5555)))),
            ],
            teams: HashMap::new(),
        };
        let t = Targets::new(NameLists::default(), &on(), &s);
        let ally = entity(EntityClass::Player, "Ally", [0.0; 3]);
        let foe = entity(EntityClass::Player, "Foe", [0.0; 3]);
        assert!(t.teammate(&ally));
        assert!(!t.teammate(&foe));
        assert_eq!(t.team_color(&foe), Some(0xff_5555));
        let plain = Targets::new(NameLists::default(), &Options::default(), &s);
        assert!(!plain.teammate(&ally));
        assert_eq!(plain.team_color(&foe), None);
    }

    #[test]
    fn options_follow_visibility_and_enablement() {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s.validate_conditions().unwrap();
        s.set(&format!("{ID}.recolor-visuals"), json!(true))
            .unwrap();
        assert!(!Options::read(&s, true).recolor_visuals);
        s.set(&format!("{ID}.teams-by-color"), json!(true)).unwrap();
        assert!(Options::read(&s, true).recolor_visuals);
        assert_eq!(Options::read(&s, false), Options::default());
        assert_eq!(detected_text(0xff55_ff55), "#55FF55");
    }
}
