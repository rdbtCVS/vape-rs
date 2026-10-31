use super::super::core::settings::{Definition, Kind};
use super::super::input::KeyAction;
use super::super::input::macros::{Delay, Macro};
use super::client_settings::SearchStyle;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

mod input;
mod render;

// Reference ThemeColors: frame body, inset, pressed row, outline and text.
const BG: u32 = 0xff1a191a;
const INSET: u32 = 0xff141414;
const PRESSED: u32 = 0xff1f1e1f;
const BUTTON: u32 = 0xff2e2d2f;
const TRACK: u32 = 0xff363536;
const OUTLINE: u32 = 0x80363536;
const FG: u32 = 0xffd1d1d1;
const TEXT: u32 = 0xffa3a3a3;
const MUTED: u32 = 0xff7a7a7a;
const FAINT: u32 = 0xff595859;
const ACCENT: u32 = 0xff06a17e;
const FAVORITE: u32 = 0xffec812c;
const DANGER: u32 = 0xfffa3238;

/// Width of every click-GUI frame, as `ClientSettingsSearchFrame.MENU_WIDTH`.
pub const FRAME_WIDTH: f32 = 110.0;
pub const HEADER_HEIGHT: f32 = 20.0;
pub const NAV_HEADER_HEIGHT: f32 = 22.0;
pub const ROW_HEIGHT: f32 = 20.0;
/// Text scales relative to the painter's 8-pixel line (the interface font is
/// drawn at a 9-pixel em per unit of scale).
const TITLE_SCALE: f32 = 0.8;
const ROW_SCALE: f32 = 0.75;
const FIELD_SCALE: f32 = 0.7;
const SMALL_SCALE: f32 = 0.55;
/// Estimated advance of `c` in the painter's interface font at scale 1, in
/// units of its 9-pixel em. Layout stays painter-independent, so truncation
/// and pointer placement use this table; the painter measures exactly.
fn advance(c: char) -> f32 {
    9.0 * match c {
        'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' | 'I' => 0.26,
        ' ' | 'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '-' => 0.36,
        'm' | 'w' | 'M' | 'W' => 0.8,
        'A'..='Z' => 0.66,
        '0'..='9' => 0.58,
        _ => 0.54,
    }
}
fn text_advance(text: &str, scale: f32) -> f32 {
    text.chars().map(advance).sum::<f32>() * scale
}
/// The caret index nearest `x` pixels into `text` at the field scale.
fn caret_at(text: &str, x: f32) -> usize {
    let mut pen = 0.0;
    for (i, c) in text.chars().enumerate() {
        let w = advance(c) * FIELD_SCALE;
        if x < pen + w / 2.0 {
            return i;
        }
        pen += w;
    }
    text.chars().count()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect(pub f32, pub f32, pub f32, pub f32);
impl Rect {
    pub fn contains(self, p: (f32, f32)) -> bool {
        p.0 >= self.0 && p.1 >= self.1 && p.0 < self.0 + self.2 && p.1 < self.1 + self.3
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug)]
pub enum Paint {
    Fill(Rect, u32),
    /// Host-font text; `x` is the left, centre or right edge per `align`.
    Text {
        x: f32,
        y: f32,
        text: String,
        color: u32,
        scale: f32,
        align: Align,
    },
    Edit {
        rect: Rect,
        text: String,
        caret: usize,
        anchor: usize,
        scale: f32,
    },
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
    /// Persist frame positions, visibility and collapse state (`ui.frames`).
    Layout(Value),
    /// Save the current configuration now (the header's sync button).
    Sync,
    Close,
}
/// The pending profile name entry under the Profiles frame's buttons.
#[derive(Clone, Debug, PartialEq)]
enum ProfileInput {
    Create,
    Import,
    Rename(String),
}
#[derive(Clone, Debug)]
enum Hit {
    /// A frame's body: raises it and keeps clicks off frames behind it.
    Panel,
    Drag(String),
    Collapse(String),
    OpenPanel(String),
    Search,
    /// The floating search frame's close icon.
    ClearSearch,
    Sync,
    Hud,
    HudBack,
    HudGroup(&'static str),
    Popup,
    Toggle(String),
    Expand(String),
    Favorite(String),
    Focus(String, Rect),
    Edit(String, Value, Rect),
    Choice(String, Vec<String>, Rect),
    ChoiceValue(String, Value),
    Slider(String, f64, f64, f64, Rect),
    RangeSlider(String, f64, f64, f64, f64, f64, Rect),
    Color(String, Rect),
    ColorChannel(String, usize, Rect),
    Cycle(String, Vec<Value>),
    Capture(String),
    Profile(String),
    ProfileMenu(String),
    CreateProfile,
    ImportProfile,
    SubmitProfile,
    SaveProfile(String),
    RenameProfile(String),
    ExportProfile(String),
    DeleteProfile(String),
    ConfirmDelete,
    Cancel,
    AddPerson(String),
    RemovePerson(String, String),
    Target(String, String),
    NewMacro,
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
/// One frame. Click-GUI frames tile in columns (`column`, stacked by `row`)
/// so expanding one pushes the frames below it down instead of overlapping
/// them; `pos` is the free position of the HUD editor frame and of a frame
/// while it is being dragged to another slot.
#[derive(Clone, Debug, Default, PartialEq)]
struct Panel {
    pos: Option<(f32, f32)>,
    column: Option<usize>,
    row: usize,
    open: bool,
    collapsed: bool,
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
    /// The HUD editor frame's group tab: Favorite, All, HUD or Game.
    hud_group: &'static str,
    dragging_hud: Option<(String, f32, f32)>,
    /// The module whose settings are expanded under its row.
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
    macro_open: bool,
    profile_input: Option<ProfileInput>,
    profile_menu: Option<String>,
    searching: bool,
    /// `ClientSettings` "Search bar style", read from the view each frame.
    search_style: super::client_settings::SearchStyle,
    focus: Option<String>,
    cursor: TextCursor,
    editor: Option<Editor>,
    capture: Option<Capture>,
    confirm_delete: Option<String>,
    held: BTreeSet<String>,
    raw_text_seen: bool,
    hits: Vec<(Rect, Hit)>,
    panels: BTreeMap<String, Panel>,
    /// Raise order; the last frame is drawn on top.
    order: Vec<String>,
    scrolls: BTreeMap<String, f32>,
    panel_rects: Vec<(String, Rect)>,
    hover: (f32, f32),
    scale: f32,
    dragging: Option<(String, f32, f32)>,
    setting_drag: Option<SettingDrag>,
    text_drag: Option<TextDrag>,
    open_choice: Option<String>,
    open_color: Option<String>,
    viewport: (f32, f32),
}
impl Default for Gui {
    fn default() -> Self {
        Self {
            open: false,
            hud_editor: false,
            hud_regions: Vec::new(),
            hud_group: "All",
            dragging_hud: None,
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
            macro_open: false,
            profile_input: None,
            profile_menu: None,
            searching: false,
            search_style: Default::default(),
            focus: None,
            cursor: TextCursor::default(),
            editor: None,
            capture: None,
            confirm_delete: None,
            held: BTreeSet::new(),
            raw_text_seen: false,
            hits: Vec::new(),
            panels: BTreeMap::new(),
            order: Vec::new(),
            scrolls: BTreeMap::new(),
            panel_rects: Vec::new(),
            hover: (-1.0, -1.0),
            scale: 1.0,
            dragging: None,
            setting_drag: None,
            text_drag: None,
            open_choice: None,
            open_color: None,
            viewport: (400.0, 300.0),
        }
    }
}
impl Gui {
    /// Restores `ui.frames`; an older single-window `ui.window` is ignored,
    /// and a document without frames (an imported profile) keeps the
    /// current layout rather than closing the frame it was loaded from.
    pub fn restore_layout(&mut self, value: &Value) {
        let Some(frames) = value.get("frames").and_then(Value::as_object) else {
            return;
        };
        self.panels.clear();
        for (name, state) in frames {
            let pos = state
                .get("x")
                .and_then(Value::as_f64)
                .zip(state.get("y").and_then(Value::as_f64))
                .map(|(x, y)| (x as f32, y as f32))
                .filter(|p| p.0.is_finite() && p.1.is_finite());
            let column = state
                .get("column")
                .and_then(Value::as_u64)
                .map(|c| c.min(64) as usize);
            let row = state.get("row").and_then(Value::as_u64).unwrap_or(0) as usize;
            self.panels.insert(
                name.clone(),
                Panel {
                    pos: pos.filter(|_| name == render::HUD),
                    column,
                    row,
                    open: state.get("open").and_then(Value::as_bool).unwrap_or(false),
                    collapsed: state
                        .get("collapsed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
            );
        }
    }
    fn layout_value(&self) -> Value {
        Value::Object(
            self.panels
                .iter()
                .map(|(name, panel)| {
                    let mut state = json!({"open": panel.open, "collapsed": panel.collapsed});
                    if let Some(column) = panel.column {
                        state["column"] = json!(column);
                        state["row"] = json!(panel.row);
                    }
                    if let Some((x, y)) = panel.pos.filter(|_| name == render::HUD) {
                        state["x"] = json!(x);
                        state["y"] = json!(y);
                    }
                    (name.clone(), state)
                })
                .collect(),
        )
    }
    pub fn close(&mut self) {
        self.open = false;
        self.confirm_delete = None;
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
        if self.confirm_delete.is_some() {
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
            && self.confirm_delete.is_none()
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
    // The retained GUI's text painter owns exact measurement. Input stays
    // painter-independent, so pointer placement uses the advance estimate at
    // the field text scale; keyboard navigation remains character-indexed.
    fn cursor_at(&mut self, rect: Rect, local_x: f32) {
        self.edit_text(|cursor, text| {
            let at = caret_at(text, local_x - rect.0 - 4.0);
            cursor.caret = at;
            cursor.anchor = at;
        });
    }
    pub fn dragging_setting(&self, setting: &str) -> bool {
        matches!(&self.setting_drag, Some(SettingDrag::Number { id, .. }) if id == setting)
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
    fn new(clip: Rect) -> Self {
        Self {
            draw: Vec::new(),
            hits: Vec::new(),
            clip,
        }
    }
    fn fill(&mut self, rect: Rect, color: u32) {
        self.draw.push(Draw {
            paint: Paint::Fill(rect, color),
            clip: self.clip,
        });
    }
    /// A fill with its four corner units cut, for the reference's 2 px radius.
    fn rounded(&mut self, r: Rect, color: u32) {
        let c = 1.0f32.min(r.2 / 2.0).min(r.3 / 2.0);
        self.fill(Rect(r.0 + c, r.1, r.2 - 2.0 * c, r.3), color);
        self.fill(Rect(r.0, r.1 + c, c, r.3 - 2.0 * c), color);
        self.fill(Rect(r.0 + r.2 - c, r.1 + c, c, r.3 - 2.0 * c), color);
    }
    fn label(&mut self, x: f32, y: f32, text: &str, color: u32, scale: f32, align: Align) {
        self.draw.push(Draw {
            paint: Paint::Text {
                x,
                y,
                text: text.into(),
                color,
                scale,
                align,
            },
            clip: self.clip,
        });
    }
    /// A label vertically centred on a row; host glyphs are 7 units tall.
    #[allow(clippy::too_many_arguments)]
    fn row_label(
        &mut self,
        x: f32,
        y: f32,
        height: f32,
        text: &str,
        color: u32,
        scale: f32,
        align: Align,
    ) {
        self.label(
            x,
            y + (height - 7.0 * scale) / 2.0,
            text,
            color,
            scale,
            align,
        );
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
    /// Authored pixel icons stand in for the reference's bundled images.
    fn icon(&mut self, x: f32, y: f32, cell: f32, rows: &[&str], color: u32) {
        for (row, line) in rows.iter().enumerate() {
            let mut start = None;
            for (column, filled) in line.chars().map(|c| c == '#').chain([false]).enumerate() {
                match (filled, start) {
                    (true, None) => start = Some(column),
                    (false, Some(first)) => {
                        self.fill(
                            Rect(
                                x + first as f32 * cell,
                                y + row as f32 * cell,
                                (column - first) as f32 * cell,
                                cell,
                            ),
                            color,
                        );
                        start = None;
                    }
                    _ => {}
                }
            }
        }
    }
    fn button(&mut self, rect: Rect, label: &str, hit: Hit) {
        self.rounded(rect, BUTTON);
        self.row_label(
            rect.0 + rect.2 / 2.0,
            rect.1,
            rect.3,
            label,
            FG,
            SMALL_SCALE,
            Align::Center,
        );
        self.hit(rect, hit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View {
        View {
            modules: vec![
                ModuleView {
                    id: "render.fullbright".into(),
                    name: "Fullbright".into(),
                    category: "Render".into(),
                    enabled: false,
                    favorite: false,
                },
                ModuleView {
                    id: "combat.sprint".into(),
                    name: "Sprint".into(),
                    category: "Combat".into(),
                    enabled: false,
                    favorite: false,
                },
            ],
            values: json!({}),
            ..View::default()
        }
    }
    fn click(gui: &mut Gui, view: &View, point: (f32, f32)) -> Vec<Action> {
        gui.frame(view, (400.0, 300.0));
        let actions = gui.mouse("MouseLeft", true, point, view);
        gui.mouse("MouseLeft", false, point, view);
        actions
    }
    #[test]
    fn navigation_opens_category_frames_whose_rows_toggle_modules() {
        let mut gui = Gui {
            open: true,
            ..Gui::default()
        };
        let mut view = view();
        // Keep the top-centre floating search bar off the Render column.
        view.values = json!({"client.search-bar": "Integrated"});
        // The second navigation row opens Render in the first default column.
        let opened = click(&mut gui, &view, (30.0, 6.0 + NAV_HEADER_HEIGHT + 30.0));
        assert!(
            matches!(&opened[..], [Action::Layout(layout)] if layout["Render"]["open"] == json!(true))
        );
        let row = (150.0, 6.0 + HEADER_HEIGHT + 10.0);
        assert_eq!(
            click(&mut gui, &view, row),
            vec![Action::Toggle("render.fullbright".into())]
        );
        gui.frame(&view, (400.0, 300.0));
        assert!(
            gui.mouse("MouseRight", true, row, &view).is_empty()
                && gui.selected.as_deref() == Some("render.fullbright")
        );
        // Collapsing hides the rows.
        click(&mut gui, &view, (122.0 + FRAME_WIDTH - 6.0, 16.0));
        assert!(click(&mut gui, &view, row).is_empty());
    }
    #[test]
    fn header_search_lists_matching_modules_in_the_navigation_frame() {
        let mut gui = Gui {
            open: true,
            ..Gui::default()
        };
        let mut view = view();
        view.values = json!({"client.search-bar": "Integrated"});
        click(&mut gui, &view, (6.0 + FRAME_WIDTH - 34.0, 16.0));
        assert_eq!(gui.focus.as_deref(), Some("search"));
        gui.commit_text("spr", false);
        assert_eq!(
            click(&mut gui, &view, (30.0, 6.0 + NAV_HEADER_HEIGHT + 10.0)),
            vec![Action::Toggle("combat.sprint".into())]
        );
    }
    #[test]
    fn floating_search_frame_lists_results_and_ctrl_f_follows_the_style() {
        let mut gui = Gui {
            open: true,
            ..Gui::default()
        };
        let mut view = view();
        // Floating (default): a top-centre bar; the header has no magnifier.
        let bar = ((400.0 - FRAME_WIDTH) / 2.0 + 20.0, 15.0);
        click(&mut gui, &view, bar);
        assert_eq!(gui.focus.as_deref(), Some("search"));
        gui.commit_text("ful", false);
        assert_eq!(
            click(&mut gui, &view, (bar.0, 7.0 + 20.0 + 10.0)),
            vec![Action::Toggle("render.fullbright".into())]
        );
        // Ctrl+F with a query clears it and keeps the bar focused.
        gui.key("ControlLeft", KeyAction::Press);
        gui.key("KeyF", KeyAction::Press);
        assert!(gui.search.is_empty());
        assert_eq!(gui.focus.as_deref(), Some("search"));
        gui.focus = None;
        gui.key("KeyF", KeyAction::Press);
        assert_eq!(gui.focus.as_deref(), Some("search"));
        // Integrated opens the header field; None ignores the shortcut.
        view.values = json!({"client.search-bar": "Integrated"});
        gui.focus = None;
        gui.frame(&view, (400.0, 300.0));
        gui.key("KeyF", KeyAction::Press);
        assert!(gui.searching && gui.focus.as_deref() == Some("search"));
        gui.searching = false;
        gui.focus = None;
        view.values = json!({"client.search-bar": "None"});
        gui.frame(&view, (400.0, 300.0));
        gui.key("KeyF", KeyAction::Press);
        assert!(gui.focus.is_none() && !gui.searching);
    }
    #[test]
    fn dragged_frames_persist_and_restore_their_layout() {
        let mut gui = Gui {
            open: true,
            ..Gui::default()
        };
        let view = view();
        gui.frame(&view, (400.0, 300.0));
        assert!(gui.mouse("MouseLeft", true, (20.0, 10.0), &view).is_empty());
        gui.pointer((150.0, 50.0));
        let released = gui.mouse("MouseLeft", false, (150.0, 50.0), &view);
        let [Action::Layout(layout)] = &released[..] else {
            panic!("{released:?}");
        };
        assert_eq!(layout["gui"]["column"], json!(1));
        assert_eq!(layout["gui"]["row"], json!(0));
        let mut restored = Gui::default();
        restored.restore_layout(&json!({ "frames": layout }));
        assert_eq!(restored.slot("gui"), (1, 0));
    }
    #[test]
    fn frames_past_the_screen_bottom_move_to_a_column_with_room() {
        let mut gui = Gui {
            open: true,
            ..Gui::default()
        };
        let mut view = view();
        view.modules = ["Render", "Combat"]
            .into_iter()
            .flat_map(|category| {
                (0..6).map(move |i| ModuleView {
                    id: format!("{category}.m{i}"),
                    name: format!("M{i}"),
                    category: category.into(),
                    enabled: false,
                    favorite: false,
                })
            })
            .collect();
        // Two 142-unit frames stacked in one column overrun a 300-unit screen.
        for (row, name) in ["Render", "Combat"].into_iter().enumerate() {
            gui.panels.insert(
                name.into(),
                Panel {
                    open: true,
                    column: Some(1),
                    row,
                    ..Panel::default()
                },
            );
        }
        gui.frame(&view, (400.0, 300.0));
        assert_eq!(gui.slot("Render"), (1, 0));
        assert_eq!(gui.slot("Combat"), (2, 0));
        assert!(gui.panel_rects.iter().all(|(_, r)| r.1 + r.3 <= 294.0));
        // Collapsed, both fit again and nothing moves.
        gui.panels.get_mut("Combat").unwrap().collapsed = true;
        gui.panels.get_mut("Combat").unwrap().column = Some(1);
        gui.panels.get_mut("Combat").unwrap().row = 1;
        gui.frame(&view, (400.0, 300.0));
        assert_eq!(gui.slot("Combat"), (1, 1));
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
            profile_name: "safe".into(),
            focus: Some("profile".into()),
            confirm_delete: Some("safe".into()),
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
        assert!(gui.confirm_delete.is_none());
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
