// Scaffold (reference `Scaffold`, Category.UTILITY) with its Legit mode
// (`LegitScaffoldMode`): sneaks at the edge of the block below while not
// walking forward (fastbridge/ninja/eagle), and holds the sneak for a
// random "Sneak delay" after leaving the edge. GodBridge and TellyBridge
// (`BlatantScaffoldMode`, `TellyBridgeScaffoldMode`) drive the movement-task
// manager and the shared rotation owner and are not ported: selecting them
// is refused with a notification rather than doing nothing silently.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::rng::Rng;
use serde_json::{Value, json};

pub const ID: &str = "blatant.scaffold";
pub const MODES: [&str; 3] = ["Legit", "GodBridge", "TellyBridge"];

/// The reference's default "Block Blacklist".
pub const BLACKLIST: [&str; 51] = [
    "Dispenser",
    "Note Block",
    "Cobweb",
    "TNT",
    "Monster Spawner",
    "Enchantment Table",
    "Oak Fence",
    "Jukebox",
    "Melon",
    "Command Block",
    "Anvil",
    "Glass Pane",
    "White Stained Glass Pane",
    "Iron Bars",
    "Ice",
    "Packed Ice",
    "Anvil",
    "Block of Redstone",
    "Gold Ore",
    "Iron Ore",
    "Coal Ore",
    "Lapis Lazuli Ore",
    "Redstone Ore",
    "Acacia Wood Stairs",
    "Wooden Pressure Plate",
    "Stone Pressure Plate",
    "Beacon",
    "Oak Sapling",
    "Powered Rail",
    "Detector Rail",
    "Shrub",
    "Dead Bush",
    "Dandelion",
    "Poppy",
    "Mushroom",
    "Ladder",
    "Rail",
    "Wooden Trapdoor",
    "Lily Pad",
    "Tripwire Hook",
    "Carpet",
    "Snow",
    "Trapped Chest",
    "Daylight Sensor",
    "Hopper",
    "Chest",
    "Torch",
    "Lever",
    "Redstone Torch",
    "Button",
    "Cactus",
];

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    let list = |default: Value| {
        (
            Kind::StringList {
                max_entries: 128,
                max_chars: 64,
            },
            default,
        )
    };
    let (black_kind, black_default) = list(json!(BLACKLIST.as_slice()));
    let (white_kind, white_default) = list(json!(["blocks"]));
    vec![
        Definition::choice(&key("mode"), "Mode", &MODES, "Legit"),
        Definition::new(
            &key("sneak-delay"),
            "Sneak delay",
            Kind::Range {
                min: 0.0,
                max: 500.0,
                step: 1.0,
            },
            json!([100.0, 200.0]),
        )
        .when(&key("mode"), json!("Legit")),
        Definition::boolean(&key("require-sneak"), "Require sneak", false)
            .when(&key("mode"), json!("Legit")),
        Definition::boolean(&key("block-count"), "Block count", false),
        Definition::boolean(&key("pitch-check"), "Pitch check", false),
        Definition::number(&key("pitch"), "Pitch", 0.0, 45.0, 90.0, 1.0)
            .when(&key("pitch-check"), json!(true)),
        Definition::boolean(&key("blacklist"), "Blacklist", true),
        Definition::new(
            &key("blacklist-blocks"),
            "Block Blacklist",
            black_kind,
            black_default,
        )
        .when(&key("blacklist"), json!(true)),
        Definition::boolean(&key("whitelist"), "Whitelist", false),
        Definition::new(
            &key("whitelist-blocks"),
            "Block Whitelist",
            white_kind,
            white_default,
        )
        .when(&key("whitelist"), json!(true)),
    ]
}

fn strings(settings: &Settings, id: &str) -> Vec<String> {
    settings
        .get(id)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Legit,
    GodBridge,
    TellyBridge,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub mode: Mode,
    pub sneak_delay: (f64, f64),
    pub require_sneak: bool,
    pub block_count: bool,
    pub pitch_check: bool,
    pub pitch: f64,
    pub blacklist: Option<Vec<String>>,
    pub whitelist: Option<Vec<String>>,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let flag = |k: &str, d: bool| settings.get(&key(k)).and_then(Value::as_bool).unwrap_or(d);
        let sneak_delay = settings
            .get(&key("sneak-delay"))
            .and_then(Value::as_array)
            .and_then(|r| Some((r.first()?.as_f64()?, r.get(1)?.as_f64()?)))
            .unwrap_or((100.0, 200.0));
        Self {
            mode: match settings.get(&key("mode")).and_then(Value::as_str) {
                Some("GodBridge") => Mode::GodBridge,
                Some("TellyBridge") => Mode::TellyBridge,
                _ => Mode::Legit,
            },
            sneak_delay,
            require_sneak: flag("require-sneak", false),
            block_count: flag("block-count", false),
            pitch_check: flag("pitch-check", false),
            pitch: settings
                .get(&key("pitch"))
                .and_then(Value::as_f64)
                .unwrap_or(45.0),
            blacklist: flag("blacklist", true).then(|| strings(settings, &key("blacklist-blocks"))),
            whitelist: flag("whitelist", false)
                .then(|| strings(settings, &key("whitelist-blocks"))),
        }
    }
}

/// `isValidBlockStack`: a block item not blacklisted and, with Whitelist
/// on, whitelisted (an empty whitelist allows everything).
pub fn valid_block(o: &Options, held: &HeldItem) -> bool {
    if !held.block || held.id.is_none() {
        return false;
    }
    if o.blacklist
        .as_ref()
        .is_some_and(|list| item_limit::matches(list, held))
    {
        return false;
    }
    o.whitelist
        .as_ref()
        .is_none_or(|list| list.is_empty() || item_limit::matches(list, held))
}

/// `canActivate`: Legit only scaffolds while a whitelisted item is held.
pub fn can_activate(o: &Options, held: &HeldItem) -> bool {
    o.mode != Mode::Legit
        || o.whitelist
            .as_ref()
            .is_none_or(|list| list.is_empty() || item_limit::matches(list, held))
}

/// `getModuleDisplayInfo`'s colour for a block count.
pub fn count_color(count: u32) -> [u8; 3] {
    if count >= 32 {
        [2, 190, 58]
    } else if count >= 16 {
        [255, 249, 18]
    } else {
        [255, 20, 20]
    }
}

/// One pre-entity-update of host state for Legit.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    pub now_ms: u64,
    /// No screen is open (the click GUI counts as one).
    pub no_screen: bool,
    /// The sneak key is physically held.
    pub sneak_held: bool,
    /// The player is already sneaking (`isSneaking`).
    pub sneaking: bool,
    pub pitch: f32,
    /// Forward input from the physical keys: 1, 0 or -1.
    pub forward: f32,
    pub on_ground: bool,
    /// No collision under the deflated box moved by this tick's motion.
    pub edge: bool,
    pub held: HeldItem,
}

/// What Legit does to the sneak key this tick (the post-tick restores it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sneak {
    Press,
    Release,
}

#[derive(Debug)]
pub struct Legit {
    rng: Rng,
    delay_ms: u64,
    /// `edgeSneakTimer` and `standDelayTimer` resets.
    edge_at: Option<u64>,
    stood_at: Option<u64>,
}

impl Default for Legit {
    fn default() -> Self {
        Self {
            rng: Rng::new(0x5ca1_f01d),
            delay_ms: 150,
            edge_at: None,
            stood_at: None,
        }
    }
}

impl Legit {
    /// `onEnable`.
    pub fn enable(&mut self, o: &Options) {
        self.delay_ms = self.rng.range(o.sneak_delay) as u64;
        self.edge_at = None;
        self.stood_at = None;
    }
    pub fn reset(&mut self) {
        self.edge_at = None;
        self.stood_at = None;
    }
    /// `shouldScaffold` (its one obfuscated player flag is not identified
    /// and not checked).
    pub fn should_scaffold(o: &Options, t: &Tick) -> bool {
        t.no_screen
            && (!o.require_sneak || t.sneak_held)
            && !(o.pitch_check && f64::from(t.pitch) < o.pitch)
            && can_activate(o, &t.held)
    }
    /// `onPreEntityUpdate`.
    pub fn tick(&mut self, o: &Options, t: &Tick) -> Option<Sneak> {
        if !Self::should_scaffold(o, t) {
            return None;
        }
        let elapsed =
            |at: Option<u64>, ms: u64| at.is_none_or(|at| t.now_ms.saturating_sub(at) >= ms);
        let mut sneak = t.forward <= 0.0 && t.on_ground && t.edge;
        let mut keep_timer = false;
        if !sneak && !elapsed(self.edge_at, self.delay_ms) && self.delay_ms > 30 {
            sneak = true;
            keep_timer = true;
        }
        if !t.on_ground {
            return None;
        }
        if sneak {
            if !t.sneaking {
                self.delay_ms = self.rng.range(o.sneak_delay) as u64;
            }
            self.stood_at = Some(t.now_ms);
            if !keep_timer {
                self.edge_at = Some(t.now_ms);
            }
            Some(Sneak::Press)
        } else if o.require_sneak {
            (!elapsed(self.stood_at, 1000) && t.forward < 0.0).then_some(Sneak::Release)
        } else {
            // `!sneakKeyWasDown`: releasing an unheld key is the user's state.
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        Options::read(&{
            let mut s = Settings::default();
            for d in definitions() {
                s.define(d).unwrap();
            }
            s
        })
    }

    fn tick(now_ms: u64, edge: bool, forward: f32) -> Tick {
        Tick {
            now_ms,
            no_screen: true,
            on_ground: true,
            edge,
            forward,
            held: HeldItem {
                id: Some("stone".into()),
                block: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn sneaks_at_the_edge_walking_backwards_then_holds_for_the_delay() {
        let o = options();
        let mut legit = Legit::default();
        legit.enable(&o);
        assert_eq!(legit.tick(&o, &tick(0, false, -1.0)), None);
        assert_eq!(legit.tick(&o, &tick(50, true, -1.0)), Some(Sneak::Press));
        // Off the edge again: still held within the 100..200 ms delay.
        assert_eq!(legit.tick(&o, &tick(100, false, -1.0)), Some(Sneak::Press));
        assert_eq!(legit.tick(&o, &tick(400, false, -1.0)), None);
        // Walking forward never sneaks.
        assert_eq!(legit.tick(&o, &tick(1000, true, 1.0)), None);
    }

    #[test]
    fn pitch_check_and_screens_disable_it() {
        let mut o = options();
        o.pitch_check = true;
        let mut legit = Legit::default();
        let mut t = tick(0, true, -1.0);
        t.pitch = 30.0;
        assert_eq!(legit.tick(&o, &t), None);
        t.pitch = 70.0;
        assert_eq!(legit.tick(&o, &t), Some(Sneak::Press));
        t.no_screen = false;
        assert_eq!(legit.tick(&o, &t), None);
    }

    #[test]
    fn default_blacklist_rejects_chests_and_count_colours() {
        let o = options();
        let chest = HeldItem {
            id: Some("chest".into()),
            display_name: "Chest".into(),
            block: true,
            ..Default::default()
        };
        assert!(!valid_block(&o, &chest));
        let stone = HeldItem {
            id: Some("stone".into()),
            display_name: "Stone".into(),
            block: true,
            ..Default::default()
        };
        assert!(valid_block(&o, &stone));
        assert_eq!(count_color(40), [2, 190, 58]);
        assert_eq!(count_color(20), [255, 249, 18]);
        assert_eq!(count_color(3), [255, 20, 20]);
    }
}
