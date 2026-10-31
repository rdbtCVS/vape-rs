// HUD-worker panels and module display info, kept out of the shared core
// file: the Text GUI's view, per-module suffixes (`getSimpleSuffix` /
// `getDetailedSuffix`) and its show/hide animation.

use super::super::ui::Snapshot;
use super::super::ui::client_settings::{DEFAULT_ACCENT, bind_text};
use super::super::ui::hud::Content;
use super::super::ui::notifications::{self, Kind};
use super::super::ui::target_info::{self, Seen, Shown};
use super::super::ui::text_gui::{self, Row, View, decimal};
use super::rainbow::{readable_hsb, rgb_to_hsb};
use super::{ModuleId, Runtime};
use serde_json::{Value, json};

/// `Mod.getSuffixForMode`: 0 Basic, 1 Extended (falls back to Basic), 2 None.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuffixMode {
    Basic,
    Extended,
    None,
}

impl Runtime {
    fn toast(&mut self, key: &str, title: &str, message: &str, duration_ms: f32) {
        if self.hud_flag("client.notifications") {
            self.toasts
                .push(key, title, message, Kind::Info, duration_ms);
        }
    }
    /// `Mod.showToggleNotification` when "Toggle alert" is on; profile
    /// loads suppress it like `ModManager.suppressStateNotifications`.
    pub(super) fn toggle_alert(&mut self, module: ModuleId, enabled: bool) {
        if self.suppress_toggle_alerts || !self.hud_flag("client.toggle-alerts") {
            return;
        }
        let message = if enabled {
            "\u{a7}2Enabled"
        } else {
            "\u{a7}cDisabled"
        };
        let title = format!("\u{a7}f{}", module.name());
        self.toast(module.id(), &title, message, 1500.0);
    }
    /// `ModManager.applyProfileModuleStates`: `count` is how many modules the
    /// profile turned on.
    pub(super) fn profile_alert(&mut self, name: &str, count: usize) {
        if !self.hud_flag("client.profile-alerts") {
            return;
        }
        self.toast(
            "client.profile",
            &format!("Profile swap to \u{a7}6{name}"),
            &format!("{count} modules enabled"),
            1500.0,
        );
    }
    /// `Vape` inject message, shown once on first entering a world.
    pub(super) fn bind_indicator(&mut self, snapshot: &Snapshot) {
        if self.bind_indicator_shown || !snapshot.in_world {
            return;
        }
        self.bind_indicator_shown = true;
        if !self.hud_flag("client.gui-bind-indicator") {
            return;
        }
        let keys: Vec<String> = self
            .settings
            .get("client.gui.binds")
            .and_then(Value::as_array)
            .map(|v| {
                v.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let message = format!("Press {} to open the GUI", bind_text(&keys));
        self.toast("", "Loaded", &message, 5000.0);
    }
    /// Advance and lay out the notification stack for one HUD frame.
    pub fn notification_frame(
        &mut self,
        screen: (f32, f32),
        measure: impl Fn(&str, f32) -> f32,
    ) -> Vec<text_gui::Draw> {
        let now = std::time::Instant::now();
        let elapsed = self
            .toast_clock
            .replace(now)
            .map_or(0.0, |then| now.duration_since(then).as_secs_f32() * 1000.0)
            .min(1000.0);
        if !self.hud_flag("client.notifications") {
            self.toasts.clear();
        }
        self.toasts.update(elapsed);
        notifications::layout(&self.toasts, screen, measure)
    }
    /// The tracked target's entity id, for the host to look up each frame.
    pub fn target_info_id(&self) -> Option<i32> {
        self.target_info.id()
    }
    /// `onPostAttack`: a living, non-armor-stand entity becomes the target.
    pub fn target_info_attack(&mut self, seen: Seen) {
        if self.enabled(ModuleId::TargetInfo) {
            self.target_info.show(seen);
        } else {
            self.target_info.clear();
        }
    }
    pub(super) fn tick_target_info(&mut self, snapshot: &Snapshot) {
        let now = std::time::Instant::now();
        let elapsed = self
            .target_info_clock
            .replace(now)
            .map_or(0.0, |then| now.duration_since(then).as_secs_f32() * 1000.0)
            .min(1000.0);
        if !self.enabled(ModuleId::TargetInfo) || !snapshot.in_world {
            self.target_info.clear();
            return;
        }
        let hovered = self.hud_flag(&format!("{}.show-hovered", target_info::ID));
        self.target_info.update(&snapshot.target, elapsed, hovered);
    }
    pub(super) fn target_info_content(
        &self,
        snapshot: &Snapshot,
        preview: bool,
    ) -> Option<Content> {
        let flag = |k: &str| self.hud_flag(&format!("{}.{k}", target_info::ID));
        let shown = Shown {
            hits: flag("hits-comparator"),
            pots: flag("pots-used-comparator"),
            combo: flag("combo-counter"),
            damage: flag("damage-comparator"),
        };
        self.target_info
            .view(snapshot.target.own.as_ref(), preview, shown)
            .map(Content::TargetInfo)
    }
    /// `ReachDisplayHudModule.onPostAttack`: the eye-to-hitbox distance of
    /// an attack on the crosshair entity.
    pub fn reach_display_attack(&mut self, distance: f32) {
        if self.enabled(ModuleId::ReachDisplay) {
            self.reach_last = Some((distance, std::time::Instant::now()));
        }
    }
    /// `ReachDisplayHudFrame`: two decimals, back to 0.00 after 5 s.
    pub(super) fn reach_display_text(&self) -> String {
        let reach = match self.reach_last {
            Some((d, at)) if at.elapsed().as_millis() < 5000 => d,
            _ => 0.0,
        };
        format!("{reach:.2} blocks")
    }
    fn hud_number(&self, id: &str) -> f64 {
        self.settings
            .get(id)
            .and_then(Value::as_f64)
            .unwrap_or_default()
    }
    fn hud_text(&self, id: &str) -> String {
        self.settings
            .get(id)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
    /// Scoreboard: whether the moved sidebar shows its score numbers.
    pub fn scoreboard_numbers(&self) -> bool {
        self.hud_flag(super::super::ui::scoreboard::SHOW_NUMBERS)
    }
    pub(super) fn hud_flag(&self, id: &str) -> bool {
        self.settings
            .effective(id)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// `RandomValue.getDisplayValue`: `min-max` plus its trimmed suffix.
    fn hud_range(&self, id: &str, places: usize, suffix: &str) -> String {
        let values = self
            .settings
            .get(id)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let at = |i: usize| values.get(i).and_then(Value::as_f64).unwrap_or_default();
        format!(
            "{}-{}{suffix}",
            decimal(at(0), places),
            decimal(at(1), places)
        )
    }
    fn simple_suffix(&self, module: ModuleId) -> String {
        match module {
            ModuleId::NoFall | ModuleId::Speed => self.hud_text(&format!("{}.mode", module.id())),
            ModuleId::FakeLag => self.hud_text("world.fake-lag.mode"),
            ModuleId::HitSelect => self.hud_text(&format!("{}.mode", module.id())),
            ModuleId::BlockHit => self.hud_text(&format!("{}.mode", module.id())),
            ModuleId::Blink => match self.outbound.chokes() {
                0 => String::new(),
                n => n.to_string(),
            },
            ModuleId::AutoMace => self.auto_mace_suffix().into(),
            _ => String::new(),
        }
    }
    fn detailed_suffix(&self, module: ModuleId) -> String {
        let id = module.id();
        match module {
            ModuleId::Backtrack => self.hud_range(&format!("{id}.latency"), 0, "ms"),
            ModuleId::JumpReset => {
                format!("{}%", decimal(self.hud_number(&format!("{id}.chance")), 0))
            }
            ModuleId::AutoClicker | ModuleId::RightClicker => {
                format!("{}cps", self.hud_range(&format!("{id}.cps"), 1, ""))
            }
            ModuleId::KnockbackDelay => self.hud_range(&format!("{id}.ground-delay"), 0, ""),
            ModuleId::Velocity => format!(
                "{}h {}v",
                decimal(self.hud_number(&format!("{id}.horizontal")), 0),
                decimal(self.hud_number(&format!("{id}.vertical")), 0)
            ),
            ModuleId::Reach => self.hud_range(&format!("{id}.range"), 2, ""),
            ModuleId::FakeLag => format!(
                "{} {}ms",
                self.hud_text("world.fake-lag.mode"),
                decimal(self.hud_number("world.fake-lag.delay"), 0)
            ),
            ModuleId::AimAssist => decimal(self.hud_number(&format!("{id}.horizontal-speed")), 1),
            ModuleId::WTap => decimal(self.hud_number(&format!("{id}.release-delay")), 0),
            ModuleId::HitSelect => self.hit_select_options().suffix(),
            ModuleId::BlockHit => self.block_hit_suffix(),
            ModuleId::HitFlick => self.hit_flick_suffix(),
            ModuleId::AutoArmor
                if self.inventory_bool("inventory.auto-armor.combat-check", false)
                    && self.combat_blocked =>
            {
                "\u{a7}c[In Combat]".into()
            }
            _ => String::new(),
        }
    }
    pub fn module_suffix(&self, module: ModuleId, mode: SuffixMode) -> String {
        match mode {
            SuffixMode::Basic => self.simple_suffix(module),
            SuffixMode::Extended => {
                let detailed = self.detailed_suffix(module);
                if detailed.is_empty() {
                    self.simple_suffix(module)
                } else {
                    detailed
                }
            }
            SuffixMode::None => String::new(),
        }
    }
    /// Modules the list shows now: enabled, coloured and not hidden.
    fn text_gui_listed(&self) -> Vec<ModuleId> {
        let hidden: Vec<String> = if self.hud_flag("hud.text-gui.hide-modules") {
            self.settings
                .get("hud.text-gui.hidden")
                .and_then(Value::as_array)
                .map(|v| {
                    v.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_lowercase)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        ModuleId::ALL
            .into_iter()
            .filter(|m| {
                self.enabled(*m)
                    && text_gui::module_color(m.id()).is_some()
                    && !hidden.contains(&m.name().to_lowercase())
            })
            .collect()
    }
    /// A Text GUI dropped against the right screen edge stays anchored there
    /// as rows come and go (the painter clamps the maximum X to the edge).
    pub(super) fn anchor_hud_drag(&self, id: &str, value: Value) -> Value {
        let x = text_gui::ID.to_string() + ".x";
        if id != x {
            return value;
        }
        let Some(region) = self.gui.hud_regions.iter().find(|r| r.id == text_gui::ID) else {
            return value;
        };
        match value.as_f64() {
            Some(v) if v as f32 + region.rect.2 >= self.gui.viewport().0 - 0.5 => json!(8192.0),
            _ => value,
        }
    }
    /// `ClientSettings.getAccentColor`: Gui Color, or under its rainbow the
    /// readable colour 0.03 behind its (already cycling) hue at 0.9/1.0.
    pub fn accent(&self) -> u32 {
        let color = self
            .settings
            .get("client.gui-color")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_ACCENT, |v| v as u32);
        if !self.hud_flag("client.gui-color.rainbow") {
            return color;
        }
        let [hue, ..] = rgb_to_hsb(color);
        let mut hue = hue - 0.03;
        if hue <= 0.0 {
            hue += 1.0;
        }
        readable_hsb(hue, 0.9, 1.0)
    }
    /// Whether the click GUI (not the HUD editor) is open: the HUD then
    /// stays drawn under it, as under the reference's GUI screen.
    pub fn gui_shows_hud(&self) -> bool {
        self.gui.open && !self.gui.hud_editor
    }
    /// Text GUI "Click disable" applies while the click GUI is open.
    fn text_gui_clicking(&self) -> bool {
        self.gui_shows_hud() && self.hud_flag(text_gui::CLICK_DISABLE)
    }
    /// The painter's module row boxes for this frame, in HUD pixels.
    pub fn set_text_gui_boxes(&mut self, boxes: Vec<(&'static str, [f32; 4])>) {
        self.text_gui_boxes = boxes;
    }
    fn text_gui_hit(&self, (x, y): (f32, f32)) -> Option<&'static str> {
        self.text_gui_boxes
            .iter()
            .find(|(_, [bx, by, w, h])| x >= *bx && x < bx + w && y >= *by && y < by + h)
            .map(|(id, _)| *id)
    }
    /// `TextGuiOverlayComponent.e`: with Click disable on, a press on a row
    /// toggles its module. A module switched off stays listed as Disabled;
    /// one switched back on keeps its row at full, without a fade-in. The
    /// click is not consumed: the GUI has already handled it.
    pub(super) fn text_gui_click(&mut self, cursor: (f32, f32)) {
        if !self.text_gui_clicking() {
            return;
        }
        let Some(module) = self
            .text_gui_hit(cursor)
            .and_then(|id| ModuleId::ALL.into_iter().find(|m| m.id() == id))
        else {
            return;
        };
        if self.enabled(module) {
            self.set_enabled(module, false);
            self.text_gui_disabled.insert(module.id());
        } else {
            self.set_enabled(module, true);
            self.text_gui_disabled.remove(module.id());
            self.text_gui_rows.show(module.id());
        }
    }
    /// Advance the Text GUI's row animations once per HUD frame.
    pub(super) fn tick_text_gui(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = self
            .text_gui_clock
            .replace(now)
            .map_or(0.0, |then| now.duration_since(then).as_secs_f32() * 1000.0)
            .min(1000.0);
        // `u()`: closing the GUI or turning Click disable off drops the
        // held rows at once; a module enabled elsewhere lists normally.
        if !self.text_gui_clicking() {
            for id in std::mem::take(&mut self.text_gui_disabled) {
                self.text_gui_rows.forget(id);
            }
        }
        let back: Vec<&'static str> = self
            .text_gui_disabled
            .iter()
            .copied()
            .filter(|id| {
                ModuleId::ALL
                    .iter()
                    .any(|m| m.id() == *id && self.enabled(*m))
            })
            .collect();
        for id in back {
            self.text_gui_disabled.remove(id);
        }
        let mut shown: Vec<&'static str> = self.text_gui_listed().iter().map(|m| m.id()).collect();
        shown.extend(self.text_gui_disabled.iter().copied());
        let animate = self.hud_flag("hud.text-gui.animations");
        self.text_gui_rows.update(&shown, elapsed, animate);
        for id in self.text_gui_disabled.iter().copied() {
            self.text_gui_rows.show(id);
        }
    }
    pub(super) fn text_gui_content(&self, preview: bool) -> Option<Content> {
        let mode = match self.hud_text("hud.text-gui.suffix").as_str() {
            "Extended" => SuffixMode::Extended,
            "None" => SuffixMode::None,
            _ => SuffixMode::Basic,
        };
        let accent = self.accent();
        let rainbow = self.hud_flag("client.gui-color.rainbow");
        let color_mode = self.hud_text("hud.text-gui.color-mode");
        let custom_color = self.hud_number("hud.text-gui.color") as u32;
        let color_of = |module: ModuleId| match color_mode.as_str() {
            "Match GUI color" => accent,
            "Custom color" => custom_color,
            _ => text_gui::module_color(module.id()).unwrap_or(accent),
        };
        let mut rows: Vec<Row> = self
            .text_gui_rows
            .fading()
            .filter_map(|id| ModuleId::ALL.into_iter().find(|m| m.id() == id))
            .map(|module| Row {
                id: module.id(),
                name: module.name().into(),
                suffix: if self.text_gui_disabled.contains(module.id()) {
                    "\u{a7}7| Disabled".into()
                } else {
                    self.module_suffix(module, mode)
                },
                color: color_of(module),
                progress: self.text_gui_rows.progress(module.id()),
            })
            .collect();
        if rows.is_empty() && preview {
            rows = (1..=3)
                .map(|n| Row {
                    id: "",
                    name: format!("Example {n}"),
                    suffix: String::new(),
                    color: accent,
                    progress: 1.0,
                })
                .collect();
        }
        let text = self.hud_text("hud.text-gui.text");
        let custom = (self.hud_flag("hud.text-gui.custom-text") && !text.is_empty()).then(|| {
            let color = if self.hud_flag("hud.text-gui.custom-text-color") && !rainbow {
                self.hud_number("hud.text-gui.text-color") as u32
            } else {
                accent
            };
            (text.replace('&', "\u{a7}"), color)
        });
        if rows.is_empty() && custom.is_none() && !self.hud_flag("hud.text-gui.watermark") {
            return None;
        }
        let hovered = if self.text_gui_clicking() {
            self.text_gui_hit(self.gui_cursor)
        } else {
            None
        };
        Some(Content::TextGui(View {
            rows,
            alphabetical: self.hud_text("hud.text-gui.sort") == "Alphabetical",
            shadow: self.hud_flag("hud.text-gui.shadow"),
            background: self.hud_flag("hud.text-gui.background"),
            smooth: self.hud_flag("hud.text-gui.smooth-font"),
            watermark: self.hud_flag("hud.text-gui.watermark"),
            custom,
            accent,
            rainbow,
            hovered,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(runtime: &Runtime) -> View {
        match runtime.text_gui_content(false) {
            Some(Content::TextGui(view)) => view,
            _ => View::default(),
        }
    }

    #[test]
    fn toggle_profile_and_bind_alerts_respect_their_settings() {
        let mut r = Runtime::default();
        r.set_enabled(ModuleId::Fly, true);
        assert_eq!(r.toasts.iter().count(), 0); // Toggle alert defaults off
        r.settings.set("client.toggle-alerts", json!(true)).unwrap();
        r.set_enabled(ModuleId::Fly, false);
        r.set_enabled(ModuleId::Fly, true);
        let all: Vec<_> = r.toasts.iter().collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].message, "\u{a7}2Enabled");
        let snapshot = Snapshot {
            in_world: true,
            ..Snapshot::default()
        };
        r.bind_indicator(&snapshot);
        r.bind_indicator(&snapshot);
        let messages: Vec<_> = r.toasts.iter().map(|t| t.message.clone()).collect();
        assert_eq!(messages[1], "Press RSHIFT to open the GUI");
        assert_eq!(messages.len(), 2);
        r.settings
            .set("client.notifications", json!(false))
            .unwrap();
        r.set_enabled(ModuleId::Fly, false);
        assert_eq!(r.toasts.iter().count(), 2);
        // Disabling notifications clears the stack on the next frame.
        assert!(r.notification_frame((400.0, 300.0), |_, _| 0.0).is_empty());
    }

    #[test]
    fn suffixes_follow_the_reference_display_values() {
        let mut r = Runtime::default();
        assert_eq!(r.module_suffix(ModuleId::Speed, SuffixMode::Basic), "Bhop");
        assert_eq!(
            r.module_suffix(ModuleId::Velocity, SuffixMode::Extended),
            "90h 100v"
        );
        assert_eq!(r.module_suffix(ModuleId::Velocity, SuffixMode::Basic), "");
        assert_eq!(
            r.module_suffix(ModuleId::AutoClicker, SuffixMode::Extended),
            "6-13cps"
        );
        assert_eq!(
            r.module_suffix(ModuleId::Backtrack, SuffixMode::Extended),
            "50-100ms"
        );
        assert_eq!(
            r.module_suffix(ModuleId::Reach, SuffixMode::Extended),
            "3-3.1"
        );
        // Extended falls back to the simple suffix.
        assert_eq!(
            r.module_suffix(ModuleId::Speed, SuffixMode::Extended),
            "Bhop"
        );
        assert_eq!(
            r.module_suffix(ModuleId::FakeLag, SuffixMode::Extended),
            "Latency 100ms"
        );
        assert_eq!(r.module_suffix(ModuleId::Speed, SuffixMode::None), "");
        r.settings
            .set("combat.wtap.release-delay", json!(150.0))
            .unwrap();
        assert_eq!(r.module_suffix(ModuleId::WTap, SuffixMode::Extended), "150");
    }

    #[test]
    fn listed_rows_exclude_uncoloured_and_hidden_modules() {
        let mut r = Runtime::default();
        for m in [ModuleId::Panic, ModuleId::Fps, ModuleId::Esp, ModuleId::Fly] {
            r.modules.get_mut(&m).unwrap().enabled = true;
        }
        r.settings
            .set("hud.text-gui.animations", json!(false))
            .unwrap();
        r.tick_text_gui();
        let names: Vec<_> = view(&r).rows.into_iter().map(|row| row.name).collect();
        assert_eq!(names, ["Fly", "ESP"]); // id order; layout sorts
        r.settings
            .set("hud.text-gui.hide-modules", json!(true))
            .unwrap();
        r.tick_text_gui();
        let names: Vec<_> = view(&r).rows.into_iter().map(|row| row.name).collect();
        assert_eq!(names, ["Fly"]);
        let row = &view(&r).rows[0];
        assert_eq!(row.color, 0xff00c1de);
        r.settings
            .set("hud.text-gui.color-mode", json!("Custom color"))
            .unwrap();
        assert_eq!(view(&r).rows[0].color, 0xffce0707);
    }

    #[test]
    fn editor_preview_shows_examples_and_custom_text_codes() {
        let mut r = Runtime::default();
        assert!(r.text_gui_content(false).is_none());
        let Some(Content::TextGui(preview)) = r.text_gui_content(true) else {
            panic!("preview")
        };
        assert_eq!(preview.rows[2].name, "Example 3");
        r.settings
            .set("hud.text-gui.custom-text", json!(true))
            .unwrap();
        r.settings.set("hud.text-gui.text", json!("&cHi")).unwrap();
        assert_eq!(view(&r).custom, Some(("\u{a7}cHi".into(), DEFAULT_ACCENT)));
    }

    #[test]
    fn match_gui_color_and_the_watermark_follow_gui_color() {
        let mut r = Runtime::default();
        r.modules.get_mut(&ModuleId::Fly).unwrap().enabled = true;
        r.settings
            .set("hud.text-gui.color-mode", json!("Match GUI color"))
            .unwrap();
        r.tick_text_gui();
        assert_eq!(view(&r).rows[0].color, DEFAULT_ACCENT);
        r.settings
            .set("client.gui-color", json!(0xff12_3456u32))
            .unwrap();
        assert_eq!(view(&r).rows[0].color, 0xff12_3456);
        assert_eq!(view(&r).accent, 0xff12_3456);
        assert!(!view(&r).rainbow);
        // Rainbow: 0.03 behind the colour's hue, readable at 0.9/1.0.
        r.settings
            .set("client.gui-color.rainbow", json!(true))
            .unwrap();
        let [hue, ..] = rgb_to_hsb(0xff12_3456);
        assert_eq!(r.accent(), readable_hsb(hue - 0.03, 0.9, 1.0));
        assert!(view(&r).rainbow);
        // The custom text colour gives way to the accent under rainbow.
        for (id, value) in [
            ("hud.text-gui.custom-text", json!(true)),
            ("hud.text-gui.custom-text-color", json!(true)),
            ("hud.text-gui.text", json!("Hi")),
        ] {
            r.settings.set(id, value).unwrap();
        }
        assert_eq!(view(&r).custom, Some(("Hi".into(), r.accent())));
    }

    #[test]
    fn click_disable_toggles_rows_in_the_open_click_gui() {
        let mut r = Runtime::default();
        let fly = ModuleId::Fly.id();
        r.set_enabled(ModuleId::Fly, true);
        r.tick_text_gui();
        r.set_text_gui_boxes(vec![(fly, [100.0, 10.0, 40.0, 11.0])]);
        let rows = |r: &Runtime| -> Vec<(String, String, f32)> {
            view(r)
                .rows
                .into_iter()
                .map(|row| (row.name, row.suffix, row.progress))
                .collect()
        };
        // Off by default, and only while the click GUI is open.
        r.gui.open = true;
        r.text_gui_click((110.0, 15.0));
        assert!(r.enabled(ModuleId::Fly));
        r.settings
            .set(text_gui::CLICK_DISABLE, json!(true))
            .unwrap();
        r.gui.hud_editor = true;
        r.text_gui_click((110.0, 15.0));
        assert!(r.enabled(ModuleId::Fly));
        r.gui.hud_editor = false;
        // Hover follows the pointer over a row.
        r.gui_cursor = (110.0, 15.0);
        assert_eq!(view(&r).hovered, Some(fly));
        r.gui_cursor = (90.0, 15.0);
        assert_eq!(view(&r).hovered, None);
        r.text_gui_click((99.0, 15.0));
        assert!(r.enabled(ModuleId::Fly), "outside the box");
        // A click disables it; the row stays, marked, at full.
        r.text_gui_click((110.0, 15.0));
        assert!(!r.enabled(ModuleId::Fly));
        r.tick_text_gui();
        r.tick_text_gui();
        let held = vec![("Fly".into(), "\u{a7}7| Disabled".into(), 1.0)];
        assert_eq!(rows(&r), held);
        // A second click enables it again at full, without a fade-in.
        r.text_gui_click((110.0, 15.0));
        assert!(r.enabled(ModuleId::Fly));
        r.tick_text_gui();
        assert_eq!(rows(&r), [("Fly".into(), String::new(), 1.0)]);
        // Disabled by click, then enabled elsewhere: listed normally.
        r.text_gui_click((110.0, 15.0));
        r.tick_text_gui();
        r.set_enabled(ModuleId::Fly, true);
        r.tick_text_gui();
        assert!(r.text_gui_disabled.is_empty());
        // Closing the GUI drops a held row at once instead of fading it.
        r.text_gui_click((110.0, 15.0));
        r.tick_text_gui();
        assert_eq!(rows(&r), held);
        r.gui.open = false;
        r.tick_text_gui();
        assert!(rows(&r).is_empty());
        assert!(r.text_gui_disabled.is_empty());
    }

    #[test]
    fn reach_display_and_health_follow_their_modules() {
        let mut r = Runtime::default();
        r.reach_display_attack(2.5);
        assert_eq!(r.reach_display_text(), "0.00 blocks"); // disabled: not recorded
        r.set_enabled(ModuleId::ReachDisplay, true);
        r.reach_display_attack(2.871);
        assert_eq!(r.reach_display_text(), "2.87 blocks");
        r.reach_last = Some((
            3.0,
            std::time::Instant::now() - std::time::Duration::from_secs(6),
        ));
        assert_eq!(r.reach_display_text(), "0.00 blocks");
    }
}
