// HUD-worker panels and module display info, kept out of the shared core
// file: the Text GUI's view, per-module suffixes (`getSimpleSuffix` /
// `getDetailedSuffix`) and its show/hide animation.

use super::super::ui::hud::Content;
use super::super::ui::text_gui::{self, Row, View, decimal};
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
    fn hud_flag(&self, id: &str) -> bool {
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
            ModuleId::Blink => match self.outbound.chokes() {
                0 => String::new(),
                n => n.to_string(),
            },
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
    /// Advance the Text GUI's row animations once per HUD frame.
    pub(super) fn tick_text_gui(&mut self) {
        let now = std::time::Instant::now();
        let elapsed = self
            .text_gui_clock
            .replace(now)
            .map_or(0.0, |then| now.duration_since(then).as_secs_f32() * 1000.0)
            .min(1000.0);
        let shown: Vec<&'static str> = self.text_gui_listed().iter().map(|m| m.id()).collect();
        let animate = self.hud_flag("hud.text-gui.animations");
        self.text_gui_rows.update(&shown, elapsed, animate);
    }
    pub(super) fn text_gui_content(&self, preview: bool) -> Option<Content> {
        let mode = match self.hud_text("hud.text-gui.suffix").as_str() {
            "Extended" => SuffixMode::Extended,
            "None" => SuffixMode::None,
            _ => SuffixMode::Basic,
        };
        let color_mode = self.hud_text("hud.text-gui.color-mode");
        let custom_color = self.hud_number("hud.text-gui.color") as u32;
        let color_of = |module: ModuleId| match color_mode.as_str() {
            "Match GUI color" => text_gui::GUI_COLOR,
            "Custom color" => custom_color,
            _ => text_gui::module_color(module.id()).unwrap_or(text_gui::GUI_COLOR),
        };
        let mut rows: Vec<Row> = self
            .text_gui_rows
            .fading()
            .filter_map(|id| ModuleId::ALL.into_iter().find(|m| m.id() == id))
            .map(|module| Row {
                name: module.name().into(),
                suffix: self.module_suffix(module, mode),
                color: color_of(module),
                progress: self.text_gui_rows.progress(module.id()),
            })
            .collect();
        let accent = if color_mode == "Custom color" {
            custom_color
        } else {
            text_gui::GUI_COLOR
        };
        if rows.is_empty() && preview {
            rows = (1..=3)
                .map(|n| Row {
                    name: format!("Example {n}"),
                    suffix: String::new(),
                    color: accent,
                    progress: 1.0,
                })
                .collect();
        }
        let text = self.hud_text("hud.text-gui.text");
        let custom = (self.hud_flag("hud.text-gui.custom-text") && !text.is_empty()).then(|| {
            let color = if self.hud_flag("hud.text-gui.custom-text-color") {
                self.hud_number("hud.text-gui.text-color") as u32
            } else {
                accent
            };
            (text.replace('&', "\u{a7}"), color)
        });
        if rows.is_empty() && custom.is_none() && !self.hud_flag("hud.text-gui.watermark") {
            return None;
        }
        Some(Content::TextGui(View {
            rows,
            alphabetical: self.hud_text("hud.text-gui.sort") == "Alphabetical",
            shadow: self.hud_flag("hud.text-gui.shadow"),
            background: self.hud_flag("hud.text-gui.background"),
            smooth: self.hud_flag("hud.text-gui.smooth-font"),
            watermark: self.hud_flag("hud.text-gui.watermark"),
            custom,
            accent,
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
        assert_eq!(
            view(&r).custom,
            Some(("\u{a7}cHi".into(), text_gui::GUI_COLOR))
        );
    }
}
