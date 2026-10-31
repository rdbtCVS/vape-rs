// Target Info (`TargetInfoSettings` + `TargetInfoPreviewComponent`): the
// entity last attacked or hovered, with its name, a smoothed health bar and
// optional hit/pot/combo/damage comparison strips. Pure: the host feeds
// `Seen` entities each HUD frame and on attack, the painter measures text
// and draws the `Draw` list `layout` returns.

use super::super::core::settings::Definition;
use super::text_gui::{Draw, Layout, spans};

pub const ID: &str = "hud.target-info";
const GREEN: u32 = 0xff058669;
const RED: u32 = 0xfffa3238;
const GRAY: u32 = 0xffd1d1d1;
const STRIP: u32 = 0xff141414;
const DAMAGE_UP: u32 = 0xff1f7c55;
const NAME_LIMIT: f32 = 80.0;
const NAME_SIZE: f32 = 1.3;
const STAT_SIZE: f32 = 0.9;

fn key(k: &str) -> String {
    format!("{ID}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    let flag = |k: &str, label: &str, default: bool, text: &str| {
        let mut d = Definition::boolean(&key(k), label, default);
        d.description = text.into();
        d
    };
    vec![
        flag(
            "show-hovered",
            "Show Hovered",
            true,
            "Show information on a hovered entity if not attacking.",
        ),
        flag(
            "damage-comparator",
            "Damage Comparator",
            true,
            "Measures strength of target compared to yourself\nConsiders armor and weapon damage",
        ),
        flag(
            "combo-counter",
            "Combo Counter",
            true,
            "Shows how many hits in a direct row you've landed, or taken, from target",
        ),
        flag(
            "hits-comparator",
            "Hits Comparator",
            false,
            "Measures how many hits you've landed compared to target",
        ),
        flag(
            "pots-used-comparator",
            "Pots Used Comparator",
            false,
            "Measures how many heal pots you've used compared to target",
        ),
    ]
}

/// One living entity as the host sees it this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    pub id: i32,
    pub name: String,
    pub health: f32,
    pub max_health: f32,
    pub player: bool,
    /// Distance from the local player.
    pub distance: f32,
    pub hurt_time: i32,
    /// `RotationUtil.y(me, target)`: `max(me→target, 1) - max(target→me, 1)`
    /// ([`damage_comparison`]).
    pub damage: i32,
    /// A player's skin texture as the host draws it (players only).
    pub skin: Option<String>,
}

/// What the host saw this HUD frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feed {
    /// The crosshair entity (living, not an armor stand).
    pub hovered: Option<Seen>,
    /// The current target's fresh state, `None` once it left the world.
    pub tracked: Option<Seen>,
    /// The local player's hurt animation time.
    pub own_hurt_time: i32,
    /// The local player, shown while the GUI is open (preview mode).
    pub own: Option<Seen>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub name: String,
    pub player: bool,
    /// Displayed health over maximum health.
    pub fraction: f32,
    pub hits: Option<i32>,
    pub pots: Option<i32>,
    pub combo: Option<i32>,
    pub damage: Option<i32>,
    /// The player skin whose face fills the 12x12 entity preview.
    pub skin: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub hits: bool,
    pub pots: bool,
    pub combo: bool,
    pub damage: bool,
}

#[derive(Debug, Default)]
pub struct Tracker {
    target: Option<Seen>,
    last_id: Option<i32>,
    since_refresh_ms: f32,
    hits: i32,
    combo: i32,
    /// `potsUsedStrip`: heal pots thrown near the target minus our own.
    pots: i32,
    /// `pendingLocalHealingPotions`: our own heal pots used but not yet
    /// seen joining the world.
    pending_local: i32,
    displayed: f32,
    snap: bool,
    animation_ms: f32,
    target_hurt: i32,
    own_hurt: i32,
}

impl Tracker {
    pub fn id(&self) -> Option<i32> {
        self.target.as_ref().map(|t| t.id)
    }
    #[cfg(test)]
    pub fn target(&self) -> Option<&Seen> {
        self.target.as_ref()
    }
    #[cfg(test)]
    pub fn hits(&self) -> i32 {
        self.hits
    }
    #[cfg(test)]
    pub fn combo(&self) -> i32 {
        self.combo
    }
    #[cfg(test)]
    pub fn pots(&self) -> i32 {
        self.pots
    }
    /// `onPlayerUseItem`: the local player used a heal potion while a
    /// target is shown (`preview`: the GUI shows the local player).
    pub fn local_heal_used(&mut self, preview: bool) {
        if preview || self.target.is_some() {
            self.pending_local += 1;
        }
    }
    /// `onEntityJoinWorld`: a thrown heal potion joined the world. One of
    /// our own pending potions counts against us, any other for the target.
    pub fn heal_joined(&mut self, preview: bool) {
        if !preview && self.target.is_none() {
            return;
        }
        if self.pending_local > 0 {
            self.pots -= 1;
            self.pending_local -= 1;
        } else {
            self.pots += 1;
        }
    }
    pub fn clear(&mut self) {
        self.target = None;
    }
    /// `showTarget`: refresh the timeout; a new entity snaps the health bar
    /// and, unless it is the last one tracked, resets the counters.
    pub fn show(&mut self, seen: Seen) {
        self.since_refresh_ms = 0.0;
        if let Some(target) = &mut self.target
            && target.id == seen.id
        {
            *target = Seen {
                hurt_time: target.hurt_time,
                ..seen
            };
            return;
        }
        if self.last_id != Some(seen.id) {
            self.hits = 0;
            self.pots = 0;
            self.combo = 0;
        }
        self.last_id = Some(seen.id);
        self.target_hurt = seen.hurt_time;
        self.snap = true;
        self.target = Some(seen);
    }
    fn increment_combo(&mut self) {
        self.combo = if self.combo >= 0 { self.combo + 1 } else { 0 };
    }
    fn decrement_combo(&mut self) {
        self.combo = if self.combo <= 0 { self.combo - 1 } else { 0 };
    }
    /// One HUD frame: hurt events, timeouts, hover and health smoothing.
    pub fn update(&mut self, feed: &Feed, elapsed_ms: f32, show_hovered: bool) {
        self.since_refresh_ms += elapsed_ms;
        if let (Some(target), Some(fresh)) = (&mut self.target, &feed.tracked)
            && target.id == fresh.id
        {
            *target = fresh.clone();
        }
        // `onUpdate(EventLivingUpdate)`: hurt starts within 6 blocks.
        let own_hurt = feed.own_hurt_time > self.own_hurt;
        self.own_hurt = feed.own_hurt_time;
        if let Some(target) = self.target.clone() {
            let target_hurt = target.hurt_time > self.target_hurt;
            self.target_hurt = target.hurt_time;
            if target.distance <= 6.0 {
                if own_hurt {
                    self.hits -= 1;
                    self.decrement_combo();
                }
                if target_hurt {
                    self.hits += 1;
                    self.increment_combo();
                }
            }
            if (self.since_refresh_ms >= 1000.0 && target.health <= 0.0)
                || self.since_refresh_ms >= 3000.0
            {
                self.target = None;
            }
        }
        if show_hovered && let Some(hovered) = &feed.hovered {
            self.show(hovered.clone());
        }
        self.smooth(elapsed_ms);
    }
    /// `TargetInfoHealthBarComponent.getSmoothedHealth`, stepped every 10 ms.
    fn smooth(&mut self, elapsed_ms: f32) {
        let Some(target) = &self.target else {
            return;
        };
        let goal = target.health;
        if self.snap {
            self.snap = false;
            self.animation_ms = 0.0;
            self.displayed = goal;
            return;
        }
        self.animation_ms += elapsed_ms;
        if self.animation_ms < 10.0 {
            return;
        }
        let l = self.animation_ms;
        let diff = (self.displayed - goal).abs();
        let step = if diff < 0.5 && self.displayed < 0.5 {
            0.05
        } else {
            0.08
        } * (l / 10.0);
        let factor = diff.sqrt().max(1.0);
        if self.displayed < goal {
            self.displayed += step * factor;
        }
        if self.displayed > goal {
            self.displayed -= step * factor;
        }
        self.displayed = self.displayed.clamp(0.0, 20.0);
        if (self.displayed - goal).abs() < 0.001 || !self.displayed.is_finite() {
            self.displayed = goal;
        }
        self.animation_ms = 0.0;
    }
    /// The panel contents, or `None` with nothing to show. `preview` (the
    /// GUI is open) shows the local player at 60% health.
    pub fn view(&self, own: Option<&Seen>, preview: bool, shown: Shown) -> Option<View> {
        let (target, fraction) = if preview {
            (own?, 0.6)
        } else {
            let target = self.target.as_ref()?;
            let max = if target.max_health > 0.0 {
                target.max_health
            } else {
                20.0
            };
            (target, self.displayed.max(0.0) / max)
        };
        Some(View {
            name: target.name.clone(),
            player: target.player,
            fraction,
            hits: shown.hits.then_some(self.hits),
            pots: shown.pots.then_some(self.pots),
            combo: shown.combo.then_some(self.combo),
            damage: shown.damage.then_some(target.damage),
            skin: target.skin.clone().filter(|_| target.player),
        })
    }
}

/// What `RotationUtil.j` reads from one side of a fight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fighter {
    /// The held item's Sharpness level.
    pub sharpness: u32,
    /// `ItemStackScoreUtil.a`: a held sword's or tool's default
    /// attack-damage modifiers, 0 for anything else.
    pub weapon_bonus: f32,
    /// `RotationUtil.g`: armor points worn, counted for players only.
    pub armor: f32,
    /// The active Resistance amplifier.
    pub resistance: Option<i32>,
}

/// `RotationUtil.b`: the local player's attack-damage attribute (`None`
/// without one), plus the attacker's Sharpness (`0.5 + 0.5 * level` from
/// 1.21) and weapon bonus. The local attribute stands in for both sides.
fn base_damage(local_attack: Option<f32>, attacker: &Fighter) -> f32 {
    let Some(mut damage) = local_attack else {
        return 0.0;
    };
    let mut bonus = 0.0;
    if attacker.sharpness > 0 {
        bonus += 0.5 + 0.5 * attacker.sharpness as f32;
    }
    bonus += attacker.weapon_bonus;
    if damage > 0.0 || bonus > 0.0 {
        damage += bonus;
    }
    damage
}

/// `RotationUtil.j(attacker, defender)`: armor (`m`), then Resistance
/// (`u`). Protection (`EnchantmentHelper.B`) is always 0 from 1.21.
fn hit_damage(local_attack: Option<f32>, attacker: &Fighter, defender: &Fighter) -> f32 {
    let mut damage = base_damage(local_attack, attacker);
    damage = damage * (25.0 - defender.armor) as i32 as f32 / 25.0;
    if let Some(amplifier) = defender.resistance {
        damage = damage * (25 - (amplifier + 1) * 5) as f32 / 25.0;
    }
    if damage <= 0.0 { 0.0 } else { damage }
}

/// `RotationUtil.y(me, target)` as the damage strip shows it.
pub fn damage_comparison(local_attack: Option<f32>, me: &Fighter, target: &Fighter) -> i32 {
    let dealt = hit_damage(local_attack, me, target).max(1.0);
    let taken = hit_damage(local_attack, target, me).max(1.0);
    (dealt - taken) as i32
}

/// A rounded rectangle as rows of fills (`GuiRenderPrimitives` rounded rect).
fn rounded(draws: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, radius: f32, color: u32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let rows = r.ceil() as usize;
    for i in 0..rows {
        let row = (i as f32).min(r);
        let height = (r - row).min(1.0);
        let dy = r - row - height / 2.0;
        let inset = r - (r * r - dy * dy).max(0.0).sqrt();
        for top in [y + row, y + h - row - height] {
            draws.push(Draw::Fill {
                x: x + inset,
                y: top,
                w: w - inset * 2.0,
                h: height,
                color,
            });
        }
    }
    draws.push(Draw::Fill {
        x,
        y: y + r,
        w,
        h: h - r * 2.0,
        color,
    });
}

/// Authored 6x6 pixel icons for the stat strips.
const SWORD: [&str; 6] = [".....#", "....#.", ".#.#..", "..#...", ".#.#..", "#....."];
const POTION: [&str; 6] = ["..##..", "..##..", ".#..#.", "#....#", "#....#", ".####."];
const COMBO: [&str; 6] = ["...##.", "..##..", ".####.", "..##..", ".##...", ".#...."];

fn icon(draws: &mut Vec<Draw>, pixels: &[&str; 6], x: f32, y: f32, color: u32) {
    for (dy, row) in pixels.iter().enumerate() {
        for (dx, c) in row.bytes().enumerate() {
            if c == b'#' {
                draws.push(Draw::Fill {
                    x: x + dx as f32,
                    y: y + dy as f32,
                    w: 1.0,
                    h: 1.0,
                    color,
                });
            }
        }
    }
}

fn sign_color(value: i32) -> u32 {
    match value.signum() {
        1 => GREEN,
        -1 => RED,
        _ => GRAY,
    }
}

fn signed(value: i32) -> String {
    if value >= 9 {
        "9+".into()
    } else if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

fn plain(text: &str) -> String {
    spans(text, 0).into_iter().map(|s| s.text).collect()
}

/// `TruncatedTextComponent`: cut to fit `limit` with a trailing "...".
fn truncate(text: &str, limit: f32, measure: &impl Fn(&str, f32) -> f32) -> String {
    if measure(&plain(text), NAME_SIZE) <= limit {
        return text.into();
    }
    let chars: Vec<char> = text.chars().collect();
    for end in (0..chars.len()).rev() {
        let cut: String = chars[..end].iter().collect();
        let candidate = format!("{cut}...");
        if measure(&plain(&candidate), NAME_SIZE) <= limit {
            return candidate;
        }
    }
    "...".into()
}

fn has_stats(view: &View) -> bool {
    view.hits.is_some() || view.pots.is_some() || view.combo.is_some() || view.damage.is_some()
}

fn padding(view: &View) -> f32 {
    if has_stats(view) { 5.0 } else { 10.0 }
}

/// Top-left of the 12x12 entity preview (`TargetInfoEntityPreviewComponent`)
/// in layout units: the header's start, shown for players only.
pub fn face_origin(view: &View) -> Option<(f32, f32)> {
    let pad = padding(view);
    let y = pad + 2.0 + if has_stats(view) { 14.0 } else { 0.0 };
    view.player.then_some((pad, y))
}

/// Lay out the panel in HUD units. `measure(text, size)` is the width of
/// `text` at `size` times the 8-unit font line.
pub fn layout(view: &View, measure: impl Fn(&str, f32) -> f32) -> Layout {
    let mut draws = Vec::new();
    // (width, value, icon and its x, combo: shows the magnitude only)
    type Strip = (f32, i32, Option<(&'static [&'static str; 6], f32)>, bool);
    let strips: Vec<Strip> = [
        view.hits.map(|v| (24.0, v, Some((&SWORD, 5.0)), false)),
        view.pots.map(|v| (24.0, v, Some((&POTION, 4.0)), false)),
        view.combo.map(|v| (24.0, v, Some((&COMBO, 4.0)), true)),
        view.damage.map(|v| (16.0, v, None, false)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let stats = !strips.is_empty();
    let pad = padding(view);
    let content = 2.0 + if stats { 14.0 } else { 0.0 } + 10.0 + 8.0 + 4.0;
    let (width, height) = (100.0 + pad * 2.0, content + pad * 2.0);
    rounded(&mut draws, 0.0, 2.0, width, height, 3.0, 0xad121212);
    let mut y = pad + 2.0;
    if stats {
        let total: f32 = strips.iter().map(|s| s.0).sum();
        let mut x = pad + 100.0 - total;
        for (w, value, pixels, combo) in strips {
            let (background, text, color) = match pixels {
                Some(_) if combo => (STRIP, value.abs().to_string(), sign_color(value)),
                Some(_) => (STRIP, signed(value), sign_color(value)),
                None => (
                    match value.signum() {
                        1 => DAMAGE_UP,
                        -1 => RED,
                        _ => STRIP,
                    },
                    if value <= -9 {
                        "-9".into()
                    } else {
                        signed(value)
                    },
                    0xb4ffffff,
                ),
            };
            rounded(&mut draws, x + 1.0, y, w - 2.0, 10.0, 1.0, background);
            if let Some((pixels, icon_x)) = pixels {
                icon(&mut draws, pixels, x + icon_x, y + 2.0, color);
            }
            draws.push(Draw::Text {
                x: x + w - measure(&text, STAT_SIZE) - 5.0,
                y: y + 1.0,
                size: STAT_SIZE,
                color,
                text,
                shadow: None,
            });
            x += w;
        }
        y += 14.0;
    }
    // Header: the 12x12 entity preview (players only, drawn by the host
    // painter at `face_origin`) then the truncated name, centred on the
    // preview or the header.
    let name = if plain(&view.name).trim().is_empty() {
        "\u{a7}7(Empty Name)".to_string()
    } else {
        truncate(&view.name, NAME_LIMIT, &measure)
    };
    let text_height = 8.0 * NAME_SIZE;
    let (mut x, name_y) = if view.player {
        (pad + 12.0 + 5.0, y + 6.0 - text_height / 2.0)
    } else {
        (pad + 2.0, y + 5.0 - text_height / 2.0)
    };
    for span in spans(&name, 0xffffffff) {
        let w = measure(&span.text, NAME_SIZE);
        draws.push(Draw::Text {
            x,
            y: name_y,
            size: NAME_SIZE,
            text: span.text,
            color: span.color,
            shadow: None,
        });
        x += w;
    }
    y += 10.0 + 8.0;
    rounded(&mut draws, pad, y, 100.0, 4.0, 1.5, 0x64000000);
    let fill = (100.0 * view.fraction).clamp(0.0, 100.0);
    rounded(&mut draws, pad, y, fill, 4.0, 1.5, GREEN);
    Layout {
        width,
        height: height + 2.0,
        draws,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zombie(health: f32, hurt_time: i32) -> Seen {
        Seen {
            id: 7,
            name: "Zombie".into(),
            health,
            max_health: 20.0,
            distance: 2.0,
            hurt_time,
            damage: 0,
            ..Seen::default()
        }
    }

    fn feed(tracked: Option<Seen>, own_hurt_time: i32) -> Feed {
        Feed {
            tracked,
            own_hurt_time,
            ..Feed::default()
        }
    }

    #[test]
    fn hits_and_combo_follow_hurt_edges_and_reset_on_a_new_target() {
        let mut t = Tracker::default();
        t.show(zombie(20.0, 0));
        t.update(&feed(Some(zombie(17.0, 10)), 0), 16.0, true);
        t.update(&feed(Some(zombie(17.0, 9)), 0), 16.0, true);
        t.update(&feed(Some(zombie(14.0, 10)), 0), 16.0, true);
        assert_eq!((t.hits(), t.combo()), (2, 2));
        // Taking a hit resets a positive combo and costs a hit.
        t.update(&feed(Some(zombie(14.0, 8)), 10), 16.0, true);
        assert_eq!((t.hits(), t.combo()), (1, 0));
        t.update(&feed(Some(zombie(14.0, 7)), 9), 16.0, true);
        t.update(&feed(Some(zombie(14.0, 6)), 10), 16.0, true);
        assert_eq!((t.hits(), t.combo()), (0, -1));
        t.update(&feed(Some(zombie(14.0, 5)), 4), 16.0, true);
        t.update(&feed(Some(zombie(14.0, 4)), 10), 16.0, true);
        assert_eq!((t.hits(), t.combo()), (-1, -2));
        // Landing a hit ends a negative streak.
        t.update(&feed(Some(zombie(14.0, 10)), 9), 16.0, true);
        assert_eq!((t.hits(), t.combo()), (0, 0));
        // The same entity again keeps the counters; another resets them.
        t.clear();
        t.show(zombie(14.0, 0));
        assert_eq!(t.hits(), 0);
        t.update(&feed(Some(zombie(14.0, 10)), 0), 16.0, true);
        assert_eq!((t.hits(), t.combo()), (1, 1));
        t.show(Seen {
            id: 8,
            ..zombie(20.0, 0)
        });
        assert_eq!((t.hits(), t.combo()), (0, 0));
    }

    #[test]
    fn targets_time_out_and_hover_refreshes_them() {
        let mut t = Tracker::default();
        t.show(zombie(20.0, 0));
        for _ in 0..180 {
            t.update(&feed(Some(zombie(20.0, 0)), 0), 16.0, true);
        }
        assert!(t.id().is_some());
        t.update(&feed(Some(zombie(20.0, 0)), 0), 200.0, true);
        assert!(t.id().is_none());
        let hovered = Feed {
            hovered: Some(zombie(20.0, 0)),
            ..Feed::default()
        };
        t.update(&hovered, 16.0, false);
        assert!(t.id().is_none());
        t.update(&hovered, 16.0, true);
        assert_eq!(t.id(), Some(7));
        // Dead targets go after one second.
        t.update(&feed(Some(zombie(0.0, 10)), 0), 500.0, true);
        assert!(t.id().is_some());
        t.update(&feed(Some(zombie(0.0, 10)), 0), 600.0, true);
        assert!(t.id().is_none());
    }

    #[test]
    fn health_bar_snaps_then_eases_toward_the_target() {
        let mut t = Tracker::default();
        t.show(zombie(20.0, 0));
        t.update(&feed(Some(zombie(20.0, 0)), 0), 0.0, true);
        let fraction = |t: &Tracker| t.view(None, false, Shown::default()).unwrap().fraction;
        assert_eq!(fraction(&t), 1.0);
        t.update(&feed(Some(zombie(10.0, 10)), 0), 10.0, true);
        // 0.08 * sqrt(10) per 10 ms step.
        assert!((fraction(&t) - (20.0 - 0.08 * 10f32.sqrt()) / 20.0).abs() < 1e-5);
        for _ in 0..200 {
            t.update(&feed(Some(zombie(10.0, 0)), 0), 10.0, true);
        }
        assert!((fraction(&t) - 0.5).abs() < 0.01);
        let own = Seen {
            name: "Steve".into(),
            player: true,
            ..Seen::default()
        };
        let preview = t.view(Some(&own), true, Shown::default()).unwrap();
        assert_eq!((preview.name.as_str(), preview.fraction), ("Steve", 0.6));
    }

    #[test]
    fn heal_pots_count_for_the_target_and_against_our_own() {
        let mut t = Tracker::default();
        // Nothing shown: neither event counts.
        t.local_heal_used(false);
        t.heal_joined(false);
        assert_eq!(t.pots(), 0);
        t.show(zombie(20.0, 0));
        t.heal_joined(false);
        t.heal_joined(false);
        assert_eq!(t.pots(), 2);
        // Our own potion joins after its use: one down, not one up.
        t.local_heal_used(false);
        t.local_heal_used(false);
        t.heal_joined(false);
        assert_eq!(t.pots(), 1);
        t.heal_joined(false);
        assert_eq!(t.pots(), 0);
        t.heal_joined(false);
        assert_eq!(t.pots(), 1);
        let shown = Shown {
            pots: true,
            ..Shown::default()
        };
        assert_eq!(t.view(None, false, shown).unwrap().pots, Some(1));
        // A new target resets the strip; the GUI preview counts without one.
        t.show(Seen {
            id: 8,
            ..zombie(20.0, 0)
        });
        assert_eq!(t.pots(), 0);
        t.clear();
        t.local_heal_used(true);
        t.heal_joined(true);
        assert_eq!(t.pots(), -1);
    }

    #[test]
    fn damage_comparison_follows_rotation_util() {
        // Bare fists on both sides: max(1, 1) - max(1, 1).
        let fist = Fighter::default();
        assert_eq!(damage_comparison(Some(1.0), &fist, &fist), 0);
        assert_eq!(damage_comparison(None, &fist, &fist), 0);
        // A diamond sword (attribute 7 with the sword held, +6 bonus counted
        // again) against an unarmored zombie: 13 - 1.
        let sword = Fighter {
            weapon_bonus: 6.0,
            ..Fighter::default()
        };
        assert_eq!(damage_comparison(Some(7.0), &sword, &fist), 13 - 7);
        // The zombie's side uses our attribute too: 7 - 13.
        assert_eq!(damage_comparison(Some(7.0), &fist, &sword), 7 - 13);
        // Sharpness V adds 3; full diamond (20 armor) on a player target
        // keeps (int)(25 - 20) / 25 of it.
        let sharp = Fighter {
            sharpness: 5,
            ..sword
        };
        let armored = Fighter {
            armor: 20.0,
            ..Fighter::default()
        };
        // dealt 16 * 5 / 25 = 3.2; taken 7 * 25 / 25 = 7.
        assert_eq!(damage_comparison(Some(7.0), &sharp, &armored), -3);
        // Fractional armor truncates: (int)(25 - 2.5) = 22.
        let light = Fighter {
            armor: 2.5,
            ..Fighter::default()
        };
        // dealt 13 * 22 / 25 = 11.44; taken 7.
        assert_eq!(damage_comparison(Some(7.0), &sword, &light), 4);
        // Resistance II keeps 15 of 25; level V (amplifier 4) none.
        let resistant = Fighter {
            resistance: Some(1),
            ..Fighter::default()
        };
        // dealt 13 * 15 / 25 = 7.8; taken 7: 0.8 truncates to 0.
        assert_eq!(damage_comparison(Some(7.0), &sword, &resistant), 0);
        assert!((hit_damage(Some(7.0), &sword, &resistant) - 7.8).abs() < 1e-5);
        let immune = Fighter {
            resistance: Some(4),
            ..Fighter::default()
        };
        assert!((hit_damage(Some(7.0), &sword, &immune)).abs() < 1e-6);
        assert_eq!(damage_comparison(Some(7.0), &sword, &immune), 1 - 7);
        // No attribute: the attacker's side is 0 before the max(1).
        assert_eq!(base_damage(None, &sharp), 0.0);
        // Without attribute damage, the bonus alone still applies.
        assert_eq!(base_damage(Some(0.0), &sword), 6.0);
        assert_eq!(base_damage(Some(0.0), &fist), 0.0);
    }

    #[test]
    fn the_face_sits_at_the_header_start_for_players() {
        let view = View {
            name: "Steve".into(),
            player: true,
            skin: Some("entity/player/wide/steve".into()),
            ..View::default()
        };
        assert_eq!(face_origin(&view), Some((10.0, 12.0)));
        let stats = View {
            hits: Some(0),
            ..view.clone()
        };
        assert_eq!(face_origin(&stats), Some((5.0, 21.0)));
        assert_eq!(
            face_origin(&View {
                player: false,
                ..view.clone()
            }),
            None
        );
        // Only players carry their skin into the view.
        let mut t = Tracker::default();
        let seen = Seen {
            player: false,
            skin: Some("x".into()),
            ..zombie(20.0, 0)
        };
        t.show(seen);
        assert_eq!(t.view(None, false, Shown::default()).unwrap().skin, None);
        let own = Seen {
            player: true,
            skin: Some("entity/player/wide/steve".into()),
            ..Seen::default()
        };
        let preview = t.view(Some(&own), true, Shown::default()).unwrap();
        assert_eq!(preview.skin.as_deref(), Some("entity/player/wide/steve"));
    }

    #[test]
    fn layout_matches_the_reference_geometry() {
        let measure = |s: &str, size: f32| s.chars().count() as f32 * 5.0 * size;
        let view = View {
            name: "Zombie".into(),
            fraction: 0.5,
            ..View::default()
        };
        let plain = layout(&view, measure);
        assert_eq!((plain.width, plain.height), (120.0, 46.0));
        let stats = layout(
            &View {
                combo: Some(3),
                damage: Some(-12),
                ..view.clone()
            },
            measure,
        );
        assert_eq!((stats.width, stats.height), (110.0, 50.0));
        let texts: Vec<_> = stats
            .draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text { text, color, x, .. } => Some((text.clone(), *color, *x)),
                _ => None,
            })
            .collect();
        // Strips right-aligned: combo at 65..89, damage at 89..105.
        assert_eq!(texts[0], ("3".into(), GREEN, 65.0 + 24.0 - 4.5 - 5.0));
        assert_eq!(texts[1].0, "-9");
        assert_eq!(texts[2].0, "Zombie");
        // The health fill is half the bar.
        assert!(stats.draws.iter().any(|d| matches!(
            d,
            Draw::Fill { w, color: GREEN, .. } if (*w - 50.0).abs() < 1e-4
        )));
        let long = layout(
            &View {
                name: "A very long entity name".into(),
                ..view.clone()
            },
            measure,
        );
        let name = long.draws.iter().find_map(|d| match d {
            Draw::Text { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("A very lo..."));
        let empty = layout(&View::default(), measure);
        assert!(
            empty
                .draws
                .iter()
                .any(|d| matches!(d, Draw::Text { text, .. } if text == "(Empty Name)"))
        );
    }
}
