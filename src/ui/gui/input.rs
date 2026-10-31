// GUI keyboard, pointer, mouse and wheel handling.
use super::*;

impl Gui {
    pub fn key(&mut self, key: &str, action: KeyAction) -> Vec<Action> {
        if !self.open {
            return Vec::new();
        }
        let raw = std::mem::take(&mut self.raw_text_seen);
        if action == KeyAction::FocusLost {
            self.held.clear();
            self.capture = None;
            self.dragging = None;
            self.setting_drag = None;
            self.text_drag = None;
            self.raw_text_seen = false;
            return Vec::new();
        }
        if action == KeyAction::Release {
            self.held.remove(key);
        } else {
            self.held.insert(key.into());
        }
        if let Some(capture) = &mut self.capture {
            if action == KeyAction::Press {
                if key == "MouseLeft" {
                    let id = capture.id.clone();
                    self.capture = None;
                    if id == "macro.binds" {
                        self.macro_binds.clear();
                        self.status = "Macro binding cleared".into();
                        return Vec::new();
                    }
                    return vec![Action::Set(id, json!([]))];
                }
                if !capture.keys.iter().any(|k| k == key) {
                    capture.keys.push(key.into());
                    if capture.first.is_none() {
                        capture.first = Some(key.into());
                    }
                }
            }
            let complete = (!self.multi_bind && !capture.keys.is_empty())
                || capture.keys.len() >= 3
                || (action == KeyAction::Release && capture.first.as_deref() == Some(key));
            if complete {
                let c = self.capture.take().unwrap();
                if c.id == "macro.binds" {
                    self.macro_binds = c.keys;
                    self.status = "Macro binding updated".into();
                    return Vec::new();
                }
                self.status = "Binding updated".into();
                return vec![Action::Set(c.id, json!(c.keys))];
            }
            return Vec::new();
        }
        if action == KeyAction::Release {
            return Vec::new();
        }
        if self.hud_editor {
            if key == "Escape" {
                self.hud_editor = false;
                self.dragging_hud = None;
            }
            return Vec::new();
        }
        if key == "Escape" {
            if self.confirm_delete.take().is_some()
                || self.editor.take().is_some()
                || self.focus.take().is_some()
                || self.open_choice.take().is_some()
                || self.open_color.take().is_some()
                || self.profile_input.take().is_some()
            {
                return Vec::new();
            }
            if self.selected.take().is_some() || self.profile_menu.take().is_some() {
                return Vec::new();
            }
            if self.searching {
                self.searching = false;
                self.search.clear();
                return Vec::new();
            }
            return vec![Action::Close];
        }
        if key == "Enter" && self.editor.is_none() && self.confirm_delete.is_none() {
            match self.focus.as_deref() {
                Some("profile") if self.profile_input.is_some() => {
                    return self.submit_profile();
                }
                Some("macro-name") => {
                    self.new_macro();
                    return Vec::new();
                }
                _ => {}
            }
        }
        if key == "Enter"
            && let Some(editor) = self.editor.take()
        {
            return match Self::edit_value(&editor) {
                Ok(action) => vec![action],
                Err(e) => {
                    self.status = e;
                    self.editor = Some(editor);
                    Vec::new()
                }
            };
        }
        let control = self.held.contains("ControlLeft")
            || self.held.contains("ControlRight")
            || self.held.contains("SuperLeft")
            || self.held.contains("SuperRight");
        if self.confirm_delete.is_some() {
            return Vec::new();
        }
        if control && key == "KeyF" {
            self.search_shortcut();
            return Vec::new();
        }
        if matches!(
            key,
            "Backspace" | "Delete" | "ArrowLeft" | "ArrowRight" | "Home" | "End"
        ) || (control && key == "KeyA")
        {
            let shift = self.held.contains("ShiftLeft") || self.held.contains("ShiftRight");
            self.edit_text(|cursor, text| cursor.key(text, key, shift, control));
            self.raw_text_seen = false;
            return Vec::new();
        }
        // Raw window text handles Unicode/keyboard layout. The control socket
        // sends physical key events without text, so provide their ASCII form.
        if !raw && !control {
            let shift = self.held.contains("ShiftLeft") || self.held.contains("ShiftRight");
            let text = if key.len() == 4 && key.starts_with("Key") {
                let c = key.chars().last().unwrap();
                Some(if shift {
                    c.to_string()
                } else {
                    c.to_lowercase().to_string()
                })
            } else if key.starts_with("Digit") && key.len() == 6 {
                Some(key[5..].into())
            } else {
                match key {
                    "Space" => Some(" ".into()),
                    "Minus" => Some("-".into()),
                    "Period" => Some(".".into()),
                    "Comma" => Some(",".into()),
                    _ => None,
                }
            };
            if let Some(text) = text {
                self.text(&text);
            }
        }
        Vec::new()
    }
    pub fn pointer(&mut self, p: (f32, f32)) -> Vec<Action> {
        if let Some((id, dx, dy)) = &self.dragging_hud
            && let Some(region) = self.hud_regions.iter().find(|r| &r.id == id)
        {
            let width = region.rect.2;
            let height = region.rect.3;
            let mut x = (p.0 - dx).clamp(0.0, (self.viewport.0 - width).max(0.0));
            let mut y = (p.1 - dy).clamp(0.0, (self.viewport.1 - height).max(0.0));
            if !self.held.contains("ShiftLeft") && !self.held.contains("ShiftRight") {
                let xs = std::iter::once(0.0)
                    .chain(std::iter::once(self.viewport.0 - width))
                    .chain(
                        self.hud_regions
                            .iter()
                            .filter(|r| r.id != *id)
                            .flat_map(|r| {
                                [
                                    r.rect.0,
                                    r.rect.0 + r.rect.2,
                                    r.rect.0 - width,
                                    r.rect.0 + r.rect.2 - width,
                                ]
                            }),
                    );
                let ys = std::iter::once(0.0)
                    .chain(std::iter::once(self.viewport.1 - height))
                    .chain(
                        self.hud_regions
                            .iter()
                            .filter(|r| r.id != *id)
                            .flat_map(|r| {
                                [
                                    r.rect.1,
                                    r.rect.1 + r.rect.3,
                                    r.rect.1 - height,
                                    r.rect.1 + r.rect.3 - height,
                                ]
                            }),
                    );
                if let Some(snap) = xs
                    .filter(|v| v.is_finite() && (x - v).abs() <= 3.0)
                    .min_by(|a, b| (x - a).abs().total_cmp(&(x - b).abs()))
                {
                    x = snap;
                }
                if let Some(snap) = ys
                    .filter(|v| v.is_finite() && (y - v).abs() <= 3.0)
                    .min_by(|a, b| (y - a).abs().total_cmp(&(y - b).abs()))
                {
                    y = snap;
                }
            }
            x = x.clamp(0.0, (self.viewport.0 - width).max(0.0));
            y = y.clamp(0.0, (self.viewport.1 - height).max(0.0));
            return vec![
                Action::Set(format!("{id}.x"), json!(x)),
                Action::Set(format!("{id}.y"), json!(y)),
            ];
        }
        let local = (p.0 / self.scale, p.1 / self.scale);
        self.hover = local;
        if let Some((name, dx, dy)) = self.dragging.clone() {
            let (width, height) = self.logical();
            let x = (local.0 - dx).clamp(0.0, (width - FRAME_WIDTH).max(0.0));
            let y = (local.1 - dy).clamp(0.0, (height - HEADER_HEIGHT).max(0.0));
            self.panels.entry(name).or_default().pos = Some((x, y));
            return Vec::new();
        }
        if let Some(drag) = &self.setting_drag {
            return vec![Self::drag_action(drag, local.0)];
        }
        if let Some(drag) = self.text_drag.clone() {
            self.edit_text(|cursor, text| {
                cursor.caret = caret_at(text, local.0 - drag.rect.0 - 4.0);
                cursor.anchor = drag.anchor;
            });
        }
        Vec::new()
    }
    pub fn mouse(
        &mut self,
        button: &str,
        pressed: bool,
        p: (f32, f32),
        view: &View,
    ) -> Vec<Action> {
        if self.capture.is_some() {
            return self.key(
                button,
                if pressed {
                    KeyAction::Press
                } else {
                    KeyAction::Release
                },
            );
        }
        if !pressed {
            self.dragging_hud = None;
            self.setting_drag = None;
            self.text_drag = None;
            if let Some((name, ..)) = self.dragging.take() {
                if name != render::HUD {
                    self.drop_panel(&name);
                }
                return vec![Action::Layout(self.layout_value())];
            }
            return Vec::new();
        }
        let local = (p.0 / self.scale, p.1 / self.scale);
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(local))
            .map(|(_, h)| h.clone());
        if self.hud_editor && hit.is_none() {
            return self.hud_mouse(button, p);
        }
        let Some(hit) = hit else {
            self.focus = None;
            self.open_choice = None;
            self.open_color = None;
            return Vec::new();
        };
        if self.confirm_delete.is_some() && !matches!(hit, Hit::ConfirmDelete | Hit::Cancel) {
            return Vec::new();
        }
        if let Some((name, _)) = self
            .panel_rects
            .iter()
            .rev()
            .find(|(_, r)| r.contains(local))
        {
            let name = name.clone();
            self.raise(&name);
        }
        if !matches!(
            hit,
            Hit::Choice(..)
                | Hit::ChoiceValue(..)
                | Hit::Color(..)
                | Hit::ColorChannel(..)
                | Hit::Popup
        ) {
            self.open_choice = None;
            self.open_color = None;
        }
        if !matches!(hit, Hit::Focus(..) | Hit::Edit(..)) {
            self.focus = None;
            self.editor = None;
        }
        match hit {
            Hit::Drag(name) if button == "MouseLeft" => {
                let (x, y) = self
                    .rect_of(&name)
                    .map_or_else(|| self.pos(&name), |r| (r.0, r.1));
                self.panels.entry(name.clone()).or_default().pos = Some((x, y));
                self.dragging = Some((name, local.0 - x, local.1 - y));
            }
            Hit::Collapse(name) => {
                let panel = self.panels.entry(name).or_default();
                panel.collapsed = !panel.collapsed;
                return vec![Action::Layout(self.layout_value())];
            }
            Hit::OpenPanel(name) => {
                let panel = self.panels.entry(name.clone()).or_default();
                panel.open = !panel.open;
                if panel.open {
                    self.raise(&name);
                }
                return vec![Action::Layout(self.layout_value())];
            }
            Hit::Search if self.search_style == SearchStyle::Floating => self.focus_search(),
            Hit::Search => {
                self.searching = !self.searching;
                self.search.clear();
                if self.searching {
                    self.focus = Some("search".into());
                    self.cursor = TextCursor::default();
                }
            }
            Hit::ClearSearch => {
                self.search.clear();
                self.focus_search();
            }
            Hit::Sync => return vec![Action::Sync],
            Hit::Hud => {
                self.hud_editor = true;
                self.dragging = None;
            }
            Hit::HudGroup(group) => {
                self.hud_group = group;
                self.scrolls.remove(render::HUD);
            }
            Hit::HudBack => {
                self.hud_editor = false;
                self.dragging_hud = None;
            }
            Hit::Toggle(id) => {
                if button == "MouseRight" {
                    self.expand(id);
                } else if self.held.contains("ShiftLeft") || self.held.contains("ShiftRight") {
                    self.capture = Some(Capture {
                        id: format!("{id}.binds"),
                        keys: Vec::new(),
                        first: None,
                    });
                } else {
                    return vec![Action::Toggle(id)];
                }
            }
            Hit::Expand(id) => self.expand(id),
            Hit::Favorite(id) => return vec![Action::Favorite(id)],
            Hit::Focus(id, rect) => {
                if self.focus.as_deref() != Some(&id) {
                    self.cursor = TextCursor::default();
                }
                self.focus = Some(id);
                self.editor = None;
                self.cursor_at(rect, local.0);
                if button == "MouseLeft" {
                    self.text_drag = Some(TextDrag {
                        rect,
                        anchor: self.cursor.caret,
                    });
                }
            }
            Hit::Edit(id, value, rect) => {
                if let Some(d) = view.definitions.iter().find(|d| d.id == id) {
                    let text = match d.kind {
                        Kind::Text { .. } => value.as_str().unwrap_or("").into(),
                        Kind::StringList { .. } => value
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .unwrap_or_default(),
                        _ => value.to_string(),
                    };
                    if self.editor.as_ref().is_none_or(|e| e.id != id) {
                        self.cursor = TextCursor::default();
                    }
                    self.editor = Some(Editor {
                        id,
                        text,
                        kind: d.kind.clone(),
                    });
                    self.focus = None;
                    self.cursor_at(rect, local.0);
                    if button == "MouseLeft" {
                        self.text_drag = Some(TextDrag {
                            rect,
                            anchor: self.cursor.caret,
                        });
                    }
                }
            }
            Hit::Choice(id, _, _) => {
                self.open_color = None;
                self.open_choice = if self.open_choice.as_ref() == Some(&id) {
                    None
                } else {
                    Some(id)
                };
            }
            Hit::ChoiceValue(id, value) => {
                self.open_choice = None;
                return vec![Action::Set(id, value)];
            }
            Hit::Slider(id, min, max, step, rect) => {
                let drag = SettingDrag::Number {
                    id,
                    min,
                    max,
                    step,
                    rect,
                };
                let action = Self::drag_action(&drag, local.0);
                self.setting_drag = Some(drag);
                return vec![action];
            }
            Hit::RangeSlider(id, min, max, step, lo, hi, rect) => {
                let clicked = Self::quantized_slider(local.0, rect, min, max, step);
                let upper = (clicked - hi).abs() < (clicked - lo).abs();
                let current = if upper { lo } else { hi };
                let drag = SettingDrag::Range {
                    id,
                    min,
                    max,
                    step,
                    upper,
                    rect,
                    other: current,
                };
                let action = Self::drag_action(&drag, local.0);
                self.setting_drag = Some(drag);
                return vec![action];
            }
            Hit::Color(id, _) => {
                self.open_choice = None;
                self.open_color = if self.open_color.as_ref() == Some(&id) {
                    None
                } else {
                    Some(id)
                };
            }
            Hit::ColorChannel(id, channel, rect) => {
                let color = view.values[&id].as_u64().unwrap_or(0) as u32;
                let drag = SettingDrag::Color {
                    id,
                    channel,
                    rect,
                    color,
                };
                let action = Self::drag_action(&drag, local.0);
                self.setting_drag = Some(drag);
                return vec![action];
            }
            Hit::Cycle(id, values) => {
                let current = &view.values[&id];
                let i = values.iter().position(|v| v == current).unwrap_or(0);
                let next = if button == "MouseRight" {
                    (i + values.len() - 1) % values.len()
                } else {
                    (i + 1) % values.len()
                };
                return vec![Action::Set(id, values[next].clone())];
            }
            Hit::Capture(id) => {
                self.capture = Some(Capture {
                    id,
                    keys: Vec::new(),
                    first: None,
                });
            }
            Hit::Profile(name) => {
                self.selected_profile = Some(name.clone());
                self.profile_menu = None;
                return vec![Action::Load(name)];
            }
            Hit::ProfileMenu(name) => {
                self.profile_menu = if self.profile_menu.as_ref() == Some(&name) {
                    None
                } else {
                    Some(name)
                };
            }
            Hit::CreateProfile => self.profile_entry(ProfileInput::Create, ""),
            Hit::ImportProfile => self.profile_entry(ProfileInput::Import, ""),
            Hit::RenameProfile(name) => {
                self.profile_entry(ProfileInput::Rename(name.clone()), &name);
                self.cursor.select_all(&self.profile_name);
            }
            Hit::SubmitProfile => return self.submit_profile(),
            Hit::SaveProfile(name) => return vec![Action::Save(name)],
            Hit::ExportProfile(name) => return vec![Action::Export(name)],
            Hit::DeleteProfile(name) => self.confirm_delete = Some(name),
            Hit::ConfirmDelete => {
                if let Some(name) = self.confirm_delete.take() {
                    if self.selected_profile.as_ref() == Some(&name) {
                        self.selected_profile = None;
                    }
                    self.profile_menu = None;
                    return vec![Action::Delete(name)];
                }
            }
            Hit::Cancel => self.confirm_delete = None,
            Hit::AddPerson(relation) => {
                return vec![Action::UpsertPerson(
                    relation,
                    self.person_name.clone(),
                    self.alias.clone(),
                )];
            }
            Hit::RemovePerson(relation, name) => return vec![Action::RemovePerson(relation, name)],
            Hit::Target(relation, name) => return vec![Action::ToggleTarget(relation, name)],
            Hit::NewMacro => self.new_macro(),
            Hit::SelectMacro(name) => {
                if self.macro_open && self.selected_macro.as_ref() == Some(&name) {
                    self.macro_open = false;
                    self.selected_macro = None;
                } else if let Some(macro_) = view.macros.iter().find(|m| m.name == name) {
                    self.macro_open = true;
                    self.selected_macro = Some(macro_.name.clone());
                    self.macro_name = macro_.name.clone();
                    self.macro_binds = macro_.binds.clone();
                    self.macro_delay = format!("{},{}", macro_.delay.min_ms, macro_.delay.max_ms);
                    self.macro_double_click = macro_.double_click;
                    self.macro_double_delay = format!(
                        "{},{}",
                        macro_.double_click_delay.min_ms, macro_.double_click_delay.max_ms
                    );
                }
            }
            Hit::SaveMacro => match self.macro_action() {
                Ok(action) => {
                    self.selected_macro = Some(self.macro_name.clone());
                    return vec![action];
                }
                Err(error) => self.status = error,
            },
            Hit::RemoveMacro(name) => {
                self.macro_open = false;
                self.selected_macro = None;
                return vec![Action::RemoveMacro(name)];
            }
            Hit::CaptureMacro => {
                self.capture = Some(Capture {
                    id: "macro.binds".into(),
                    keys: Vec::new(),
                    first: None,
                })
            }
            Hit::ToggleMacroDouble => self.macro_double_click = !self.macro_double_click,
            Hit::Panel | Hit::Popup | Hit::Drag(_) => {}
        }
        Vec::new()
    }
    /// HUD editor presses that miss the HUD frame: drag, toggle or open a
    /// HUD's settings in the frame.
    fn hud_mouse(&mut self, button: &str, p: (f32, f32)) -> Vec<Action> {
        for region in self.hud_regions.iter().rev() {
            let Rect(x, y, w, h) = region.rect;
            let hy = if y >= 8.0 { (y - 10.0).max(0.0) } else { y + h };
            if Rect(x + w - 11.0, hy, 11.0, 10.0).contains(p) {
                return vec![Action::Toggle(region.id.clone())];
            }
            if Rect(x + w - 24.0, hy, 12.0, 10.0).contains(p)
                || (button == "MouseRight" && region.rect.contains(p))
            {
                self.selected = Some(region.id.clone());
                self.scrolls.remove(super::render::HUD);
                return Vec::new();
            }
            if button == "MouseLeft"
                && Rect(x, y.min(hy), w, (y + h).max(hy + 10.0) - y.min(hy)).contains(p)
            {
                self.dragging_hud = Some((region.id.clone(), p.0 - x, p.1 - y));
                return Vec::new();
            }
        }
        Vec::new()
    }
    fn expand(&mut self, id: String) {
        self.selected = if self.selected.as_ref() == Some(&id) {
            None
        } else {
            Some(id)
        };
        self.editor = None;
    }
    fn profile_entry(&mut self, input: ProfileInput, text: &str) {
        if self.profile_input.as_ref() == Some(&input) {
            self.profile_input = None;
            self.focus = None;
            return;
        }
        self.profile_input = Some(input);
        self.profile_name = text.into();
        self.focus = Some("profile".into());
        self.cursor = TextCursor::default();
    }
    fn submit_profile(&mut self) -> Vec<Action> {
        let name = self.profile_name.clone();
        self.focus = None;
        match self.profile_input.take() {
            Some(ProfileInput::Create) => vec![Action::Save(name)],
            Some(ProfileInput::Import) => vec![Action::Import(name)],
            Some(ProfileInput::Rename(old)) => {
                self.profile_menu = None;
                vec![Action::Rename(old, name)]
            }
            None => Vec::new(),
        }
    }
    /// The macro input's arrow: starts a new macro draft with that name.
    fn new_macro(&mut self) {
        if self.selected_macro.take().is_some() {
            self.macro_binds.clear();
            self.macro_delay = "120,120".into();
            self.macro_double_delay = "200,200".into();
            self.macro_double_click = false;
        }
        self.macro_open = true;
        self.focus = None;
    }
    /// Scrolls the topmost frame under the pointer.
    pub fn wheel(&mut self, delta: f32, p: (f32, f32)) {
        let local = (p.0 / self.scale, p.1 / self.scale);
        if let Some((name, _)) = self
            .panel_rects
            .iter()
            .rev()
            .find(|(_, r)| r.contains(local))
        {
            let scroll = self.scrolls.entry(name.clone()).or_default();
            *scroll = (*scroll - delta * 24.0).max(0.0);
        }
    }
    fn focus_search(&mut self) {
        self.focus = Some("search".into());
        self.editor = None;
        self.cursor = TextCursor::default();
    }
    /// `ClientSettings.handleSearchShortcut` (Ctrl+F): an active query is
    /// cleared; otherwise the style's search bar takes focus.
    fn search_shortcut(&mut self) {
        if !self.search.is_empty() {
            self.search.clear();
            self.focus_search();
            return;
        }
        match self.search_style {
            SearchStyle::Floating => self.focus_search(),
            SearchStyle::Integrated => {
                self.searching = true;
                self.focus_search();
            }
            SearchStyle::None => {}
        }
    }
    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }
    pub fn update_viewport(&mut self, viewport: (f32, f32)) {
        self.viewport = viewport;
    }
}
