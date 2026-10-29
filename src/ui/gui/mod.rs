use super::super::core::settings::{Definition, Kind};
use super::super::input::KeyAction;
use super::super::input::macros::{Delay, Macro};
use serde_json::{Value, json};
use std::collections::BTreeSet;

mod input;
mod render;

const BG: u32 = 0xff1a191a;
const HEADER: u32 = 0xff141414;
const CARD: u32 = 0xff252426;
const FG: u32 = 0xffd1d1d1;
const MUTED: u32 = 0xff737173;
const ACCENT: u32 = 0xff06a17e;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect(pub f32, pub f32, pub f32, pub f32);
impl Rect {
    pub fn contains(self, p: (f32, f32)) -> bool {
        p.0 >= self.0 && p.1 >= self.1 && p.0 < self.0 + self.2 && p.1 < self.1 + self.3
    }
}
#[derive(Clone, Debug)]
pub enum Paint {
    Fill(Rect, u32),
    Text(f32, f32, String, u32),
    Edit(Rect, String, usize, usize),
}
#[derive(Clone, Debug)]
pub struct Draw {
    pub paint: Paint,
    pub clip: Rect,
}
#[derive(Clone, Debug)]
pub struct Frame {
    pub draw: Vec<Draw>,
    pub origin: (f32, f32),
    pub scale: f32,
    pub hud_editor: bool,
}
#[derive(Clone, Debug)]
pub struct HudRegion {
    pub id: String,
    pub rect: Rect,
    pub enabled: bool,
}
#[derive(Clone, Debug)]
pub struct ModuleView {
    pub id: String,
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub favorite: bool,
}
#[derive(Clone, Debug)]
pub struct PersonView {
    pub relation: String,
    pub name: String,
    pub alias: String,
    pub targeted: bool,
}
#[derive(Clone, Debug, Default)]
pub struct View {
    pub modules: Vec<ModuleView>,
    pub definitions: Vec<Definition>,
    pub values: Value,
    pub profiles: Vec<String>,
    pub people: Vec<PersonView>,
    pub macros: Vec<Macro>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Toggle(String),
    Set(String, Value),
    Favorite(String),
    Save(String),
    Load(String),
    Rename(String, String),
    Delete(String),
    Import(String),
    Export(String),
    UpsertPerson(String, String, String),
    RemovePerson(String, String),
    ToggleTarget(String, String),
    UpsertMacro(Macro),
    RemoveMacro(String),
    Window(f32, f32),
    Close,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    Modules,
    Friends,
    Profiles,
    Macros,
}
#[derive(Clone, Debug)]
enum Hit {
    Header,
    Close,
    Hud,
    Page(Page),
    Category(String),
    Toggle(String),
    Settings(String),
    Favorite(String),
    Focus(String, Rect),
    Edit(String, Value, Rect),
    Choice(String, Vec<String>, Rect),
    ChoiceValue(String, Value),
    Slider(String, f64, f64, f64, Rect),
    RangeSlider(String, f64, f64, f64, f64, f64, Rect),
    Color(String, u32, Rect),
    ColorChannel(String, usize, Rect),
    Cycle(String, Vec<Value>),
    Capture(String),
    SelectProfile(String),
    Save,
    Load,
    Rename,
    Delete,
    ConfirmDelete,
    Cancel,
    Import,
    Export,
    AddPerson(String),
    RemovePerson(String, String),
    Target(String, String),
    SelectMacro(String),
    SaveMacro,
    RemoveMacro(String),
    CaptureMacro,
    ToggleMacroDouble,
}
#[derive(Clone, Debug)]
enum SettingDrag {
    Number {
        id: String,
        min: f64,
        max: f64,
        step: f64,
        rect: Rect,
    },
    Range {
        id: String,
        min: f64,
        max: f64,
        step: f64,
        upper: bool,
        rect: Rect,
        other: f64,
    },
    Color {
        id: String,
        channel: usize,
        rect: Rect,
        color: u32,
    },
}
#[derive(Clone, Debug)]
struct TextDrag {
    rect: Rect,
    anchor: usize,
}
#[derive(Clone, Debug)]
struct Capture {
    id: String,
    keys: Vec<String>,
    first: Option<String>,
}
#[derive(Clone, Debug)]
struct Editor {
    id: String,
    text: String,
    kind: Kind,
}
/// Character offsets keep navigation and replacement on UTF-8 boundaries.
#[derive(Debug)]
struct TextCursor {
    caret: usize,
    anchor: usize,
}
impl Default for TextCursor {
    fn default() -> Self {
        Self {
            caret: usize::MAX,
            anchor: usize::MAX,
        }
    }
}
impl TextCursor {
    fn bounds(&mut self, text: &str) -> (usize, usize) {
        let len = text.chars().count();
        self.caret = self.caret.min(len);
        self.anchor = self.anchor.min(len);
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }
    fn select_all(&mut self, text: &str) {
        self.anchor = 0;
        self.caret = text.chars().count();
    }
    fn replace(&mut self, text: &mut String, inserted: &str) {
        let (start, end) = self.bounds(text);
        let mut chars: Vec<char> = text.chars().collect();
        let available = 256usize.saturating_sub(chars.len() - (end - start));
        let inserted: Vec<char> = inserted
            .chars()
            .filter(|c| !c.is_control())
            .take(available)
            .collect();
        self.caret = start + inserted.len();
        self.anchor = self.caret;
        chars.splice(start..end, inserted);
        *text = chars.into_iter().collect();
    }
    fn key(&mut self, text: &mut String, key: &str, shift: bool, control: bool) {
        let (start, end) = self.bounds(text);
        let chars: Vec<char> = text.chars().collect();
        if key == "KeyA" && control {
            self.select_all(text);
            return;
        }
        if key == "Backspace" || key == "Delete" {
            if start == end {
                if key == "Backspace" {
                    self.anchor = self.caret.saturating_sub(1);
                } else {
                    self.anchor = (self.caret + 1).min(chars.len());
                }
            }
            self.replace(text, "");
            return;
        }
        let next = match key {
            "Home" => 0,
            "End" => chars.len(),
            "ArrowLeft" if !shift && start != end => start,
            "ArrowRight" if !shift && start != end => end,
            "ArrowLeft" if control => {
                let mut i = self.caret;
                while i > 0 && chars[i - 1].is_whitespace() {
                    i -= 1;
                }
                while i > 0 && !chars[i - 1].is_whitespace() {
                    i -= 1;
                }
                i
            }
            "ArrowRight" if control => {
                let mut i = self.caret;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                i
            }
            "ArrowLeft" => self.caret.saturating_sub(1),
            "ArrowRight" => (self.caret + 1).min(chars.len()),
            _ => return,
        };
        self.caret = next;
        if !shift {
            self.anchor = next;
        }
    }
}
#[derive(Debug)]
pub struct Gui {
    pub open: bool,
    pub hud_editor: bool,
    pub hud_regions: Vec<HudRegion>,
    dragging_hud: Option<(String, f32, f32)>,
    pub page: Page,
    pub category: String,
    pub selected: Option<String>,
    pub status: String,
    pub multi_bind: bool,
    pub user_scale: f32,
    pub search: String,
    pub profile_name: String,
    pub person_name: String,
    pub alias: String,
    pub macro_name: String,
    pub macro_delay: String,
    pub macro_double_delay: String,
    pub macro_double_click: bool,
    pub macro_binds: Vec<String>,
    selected_profile: Option<String>,
    selected_macro: Option<String>,
    focus: Option<String>,
    cursor: TextCursor,
    editor: Option<Editor>,
    capture: Option<Capture>,
    confirm_delete: bool,
    held: BTreeSet<String>,
    raw_text_seen: bool,
    hits: Vec<(Rect, Hit)>,
    frame_origin: (f32, f32),
    scale: f32,
    window: Option<(f32, f32)>,
    dragging: Option<(f32, f32)>,
    setting_drag: Option<SettingDrag>,
    text_drag: Option<TextDrag>,
    open_choice: Option<(String, Vec<String>, Rect)>,
    open_color: Option<(String, u32, Rect)>,
    viewport: (f32, f32),
    pub scroll: f32,
}
impl Default for Gui {
    fn default() -> Self {
        Self {
            open: false,
            hud_editor: false,
            hud_regions: Vec::new(),
            dragging_hud: None,
            page: Page::Modules,
            category: "Favorites".into(),
            selected: None,
            status: String::new(),
            multi_bind: false,
            user_scale: 1.0,
            search: String::new(),
            profile_name: String::new(),
            person_name: String::new(),
            alias: String::new(),
            macro_name: String::new(),
            macro_delay: "120,120".into(),
            macro_double_delay: "200,200".into(),
            macro_double_click: false,
            macro_binds: Vec::new(),
            selected_profile: None,
            selected_macro: None,
            focus: None,
            cursor: TextCursor::default(),
            editor: None,
            capture: None,
            confirm_delete: false,
            held: BTreeSet::new(),
            raw_text_seen: false,
            hits: Vec::new(),
            frame_origin: (0.0, 0.0),
            scale: 1.0,
            window: None,
            dragging: None,
            setting_drag: None,
            text_drag: None,
            open_choice: None,
            open_color: None,
            viewport: (400.0, 300.0),
            scroll: 0.0,
        }
    }
}
impl Gui {
    pub fn restore_window(&mut self, value: &Value) {
        self.window = value
            .get("window")
            .and_then(Value::as_array)
            .filter(|a| a.len() == 2)
            .and_then(|a| Some((a[0].as_f64()? as f32, a[1].as_f64()? as f32)))
            .filter(|p| p.0.is_finite() && p.1.is_finite());
    }
    pub fn window(&self) -> Option<(f32, f32)> {
        self.window
    }
    pub fn close(&mut self) {
        self.open = false;
        self.confirm_delete = false;
        self.hud_editor = false;
        self.dragging_hud = None;
        self.focus = None;
        self.editor = None;
        self.capture = None;
        self.dragging = None;
        self.setting_drag = None;
        self.text_drag = None;
        self.open_choice = None;
        self.open_color = None;
        self.held.clear();
        self.raw_text_seen = false;
    }
    pub fn select_profile(&mut self, name: &str) {
        self.selected_profile = Some(name.into());
        self.profile_name = name.into();
        self.cursor = TextCursor::default();
    }
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }
    fn edit_text(&mut self, operation: impl FnOnce(&mut TextCursor, &mut String)) {
        if self.confirm_delete {
            return;
        }
        let target = if let Some(editor) = self.editor.as_mut() {
            Some(&mut editor.text)
        } else {
            match self.focus.as_deref() {
                Some("search") => Some(&mut self.search),
                Some("profile") => Some(&mut self.profile_name),
                Some("person") => Some(&mut self.person_name),
                Some("alias") => Some(&mut self.alias),
                Some("macro-name") => Some(&mut self.macro_name),
                Some("macro-delay") => Some(&mut self.macro_delay),
                Some("macro-double-delay") => Some(&mut self.macro_double_delay),
                _ => None,
            }
        };
        if let Some(target) = target {
            operation(&mut self.cursor, target);
        }
    }
    fn text(&mut self, text: &str) {
        self.edit_text(|cursor, target| cursor.replace(target, text));
    }
    pub fn commit_text(&mut self, text: &str, from_key: bool) {
        if self.open && self.capture.is_none() {
            self.text(text);
            self.raw_text_seen = from_key;
        }
    }
    pub fn clipboard_key(&self, key: &str, action: KeyAction) -> bool {
        let modifier = if cfg!(target_os = "macos") {
            ["SuperLeft", "SuperRight"]
        } else {
            ["ControlLeft", "ControlRight"]
        };
        self.open
            && !self.confirm_delete
            && (self.editor.is_some() || self.focus.is_some())
            && action != KeyAction::Release
            && modifier.iter().any(|k| self.held.contains(*k))
            && !["ShiftLeft", "ShiftRight", "AltLeft", "AltRight"]
                .iter()
                .any(|k| self.held.contains(*k))
            && matches!(key, "KeyC" | "KeyX" | "KeyV")
    }
    pub fn copy_selection(&mut self, cut: bool) -> Option<String> {
        let mut copied = None;
        self.edit_text(|cursor, text| {
            let (start, end) = cursor.bounds(text);
            if start != end {
                copied = Some(text.chars().skip(start).take(end - start).collect());
                if cut {
                    cursor.replace(text, "");
                }
            }
        });
        copied
    }
    fn edit_value(editor: &Editor) -> Result<Action, String> {
        let value = match editor.kind {
            Kind::Number { .. } => json!(
                editor
                    .text
                    .parse::<f64>()
                    .map_err(|_| "Enter a finite number")?
            ),
            Kind::Color => json!(
                u32::from_str_radix(editor.text.trim_start_matches('#'), 16)
                    .map_err(|_| "Use #AARRGGBB")?
            ),
            Kind::Range { .. } => {
                let values = editor
                    .text
                    .split(',')
                    .map(|s| s.trim().parse::<f64>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "Enter minimum, maximum")?;
                json!(values)
            }
            Kind::StringList { .. } => json!(
                editor
                    .text
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            ),
            _ => json!(editor.text),
        };
        Ok(Action::Set(editor.id.clone(), value))
    }
    // The retained GUI's text painter owns proportional-font measurement.  Input
    // stays painter-independent, so pointer placement uses the same compact
    // six-pixel cell used by the source-sized controls; keyboard navigation
    // remains character-indexed and precise.
    fn cursor_at(&mut self, rect: Rect, local_x: f32) {
        let offset = ((local_x - rect.0 - 4.0) / 6.0).round().max(0.0) as usize;
        self.edit_text(|cursor, text| {
            let at = offset.min(text.chars().count());
            cursor.caret = at;
            cursor.anchor = at;
        });
    }
    fn quantized_slider(x: f32, rect: Rect, min: f64, max: f64, step: f64) -> f64 {
        let t = ((x - rect.0) / rect.2.max(1.0)).clamp(0.0, 1.0) as f64;
        let value = min + (max - min) * t;
        ((value / step).round() * step).clamp(min, max)
    }
    fn drag_action(drag: &SettingDrag, local_x: f32) -> Action {
        match drag {
            SettingDrag::Number {
                id,
                min,
                max,
                step,
                rect,
            } => Action::Set(
                id.clone(),
                json!(Self::quantized_slider(local_x, *rect, *min, *max, *step)),
            ),
            SettingDrag::Range {
                id,
                min,
                max,
                step,
                upper,
                rect,
                other,
            } => {
                let value = Self::quantized_slider(local_x, *rect, *min, *max, *step);
                Action::Set(
                    id.clone(),
                    if *upper {
                        json!([*other, value.max(*other)])
                    } else {
                        json!([value.min(*other), *other])
                    },
                )
            }
            SettingDrag::Color {
                id,
                channel,
                rect,
                color,
            } => {
                let value = ((local_x - rect.0) / rect.2.max(1.0) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u32;
                let shift = 24 - (*channel as u32 * 8);
                let next = (*color & !(0xff << shift)) | (value << shift);
                Action::Set(id.clone(), json!(next))
            }
        }
    }
    fn macro_action(&self) -> Result<Action, String> {
        let delay = |value: &str| -> Result<Delay, String> {
            let numbers = value
                .split(',')
                .map(str::trim)
                .map(|s| s.parse::<u16>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "Macro delay uses min,max milliseconds")?;
            if numbers.len() != 2 {
                return Err("Macro delay uses min,max milliseconds".into());
            }
            let delay = Delay::bounded(numbers[0], numbers[1]);
            if delay.min_ms != numbers[0] || delay.max_ms != numbers[1] {
                return Err("Macro delays must be 1–1000 ms and ordered".into());
            }
            Ok(delay)
        };
        let macro_ = Macro {
            name: self.macro_name.clone(),
            binds: self.macro_binds.clone(),
            delay: delay(&self.macro_delay)?,
            double_click: self.macro_double_click,
            double_click_delay: delay(&self.macro_double_delay)?,
        };
        macro_.validate()?;
        Ok(Action::UpsertMacro(macro_))
    }
}
fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.0.max(b.0);
    let y = a.1.max(b.1);
    Rect(
        x,
        y,
        ((a.0 + a.2).min(b.0 + b.2) - x).max(0.0),
        ((a.1 + a.3).min(b.1 + b.3) - y).max(0.0),
    )
}
struct Layout {
    draw: Vec<Draw>,
    hits: Vec<(Rect, Hit)>,
    clip: Rect,
}
impl Layout {
    fn fill(&mut self, rect: Rect, color: u32) {
        self.draw.push(Draw {
            paint: Paint::Fill(rect, color),
            clip: self.clip,
        });
    }
    fn text(&mut self, x: f32, y: f32, text: &str, color: u32) {
        self.draw.push(Draw {
            paint: Paint::Text(x, y, text.into(), color),
            clip: self.clip,
        });
    }
    fn hit(&mut self, rect: Rect, hit: Hit) {
        let x = rect.0.max(self.clip.0);
        let y = rect.1.max(self.clip.1);
        let w = (rect.0 + rect.2).min(self.clip.0 + self.clip.2) - x;
        let h = (rect.1 + rect.3).min(self.clip.1 + self.clip.3) - y;
        if w > 0.0 && h > 0.0 {
            self.hits.push((Rect(x, y, w, h), hit));
        }
    }
    fn button(&mut self, rect: Rect, label: &str, hit: Hit) {
        self.fill(rect, CARD);
        self.text(rect.0 + 7.0, rect.1 + 7.0, label, FG);
        self.hit(rect, hit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_hit_testing_and_search_use_the_rendered_geometry() {
        let mut gui = Gui {
            open: true,
            category: "Legit".into(),
            ..Gui::default()
        };
        let view = View {
            modules: vec![ModuleView {
                id: "hud.fps".into(),
                name: "FPS".into(),
                category: "Legit".into(),
                enabled: false,
                favorite: false,
            }],
            ..View::default()
        };
        let frame = gui.frame(&view, (320.0, 240.0));
        let point = (
            frame.origin.0 + 125.0 * frame.scale,
            frame.origin.1 + 85.0 * frame.scale,
        );
        assert_eq!(
            gui.mouse("MouseLeft", true, point, &view),
            vec![Action::Toggle("hud.fps".into())]
        );
        gui.search = "nothing".into();
        gui.frame(&view, (320.0, 240.0));
        assert!(gui.mouse("MouseLeft", true, point, &view).is_empty());
    }
    #[test]
    fn unicode_caret_selection_deletion_and_clipboard() {
        let mut gui = Gui {
            open: true,
            focus: Some("person".into()),
            ..Gui::default()
        };
        gui.commit_text("a好🙂 z", false);
        gui.key("Home", KeyAction::Press);
        gui.key("ArrowRight", KeyAction::Press);
        gui.key("Delete", KeyAction::Press);
        assert_eq!(gui.person_name, "a🙂 z");
        gui.key("ShiftLeft", KeyAction::Press);
        gui.key("ArrowRight", KeyAction::Press);
        gui.key("ShiftLeft", KeyAction::Release);
        assert_eq!(gui.copy_selection(false).as_deref(), Some("🙂"));
        gui.commit_text("é", false);
        assert_eq!(gui.person_name, "aé z");
        gui.key("End", KeyAction::Press);
        gui.key("Backspace", KeyAction::Press);
        assert_eq!(gui.person_name, "aé ");
        gui.cursor.select_all(&gui.person_name);
        assert_eq!(gui.copy_selection(true).as_deref(), Some("aé "));
        assert!(gui.person_name.is_empty());
        gui.commit_text("paste\ntext", false);
        assert_eq!(gui.person_name, "pastetext");
    }
    #[test]
    fn delete_confirmation_blocks_background_input() {
        let mut gui = Gui {
            open: true,
            page: Page::Profiles,
            profile_name: "safe".into(),
            focus: Some("profile".into()),
            confirm_delete: true,
            ..Gui::default()
        };
        gui.commit_text("x", false);
        assert_eq!(gui.profile_name, "safe");
        gui.frame(&View::default(), (400.0, 300.0));
        assert!(
            gui.hits
                .iter()
                .all(|(_, h)| matches!(h, Hit::ConfirmDelete | Hit::Cancel))
        );
        gui.key("Escape", KeyAction::Press);
        assert!(!gui.confirm_delete);
        assert!(gui.open);
    }
    #[test]
    fn hud_drag_is_bounded_and_snaps_to_screen_edges() {
        let mut gui = Gui {
            open: true,
            hud_editor: true,
            ..Gui::default()
        };
        gui.update_viewport((400.0, 300.0));
        gui.hud_regions = vec![HudRegion {
            id: "hud.coordinates".into(),
            rect: Rect(8.0, 8.0, 100.0, 35.0),
            enabled: true,
        }];
        assert!(
            gui.mouse("MouseLeft", true, (20.0, 20.0), &View::default())
                .is_empty()
        );
        assert_eq!(
            gui.pointer((309.0, 277.0)),
            vec![
                Action::Set("hud.coordinates.x".into(), json!(300.0)),
                Action::Set("hud.coordinates.y".into(), json!(265.0))
            ]
        );
        gui.mouse("MouseLeft", false, (309.0, 277.0), &View::default());
        assert!(gui.pointer((0.0, 0.0)).is_empty());
    }
    #[test]
    fn editing_unicode_uses_raw_text_once_and_capture_matches_reference_chords() {
        let mut gui = Gui {
            open: true,
            focus: Some("person".into()),
            ..Gui::default()
        };
        gui.commit_text("好", true);
        gui.key("KeyH", KeyAction::Press);
        assert_eq!(gui.person_name, "好");
        gui.capture = Some(Capture {
            id: "x.binds".into(),
            keys: Vec::new(),
            first: None,
        });
        gui.multi_bind = true;
        assert!(gui.key("ShiftLeft", KeyAction::Press).is_empty());
        assert!(gui.key("KeyK", KeyAction::Press).is_empty());
        assert_eq!(
            gui.key("ShiftLeft", KeyAction::Release),
            vec![Action::Set("x.binds".into(), json!(["ShiftLeft", "KeyK"]))]
        );
    }
}
