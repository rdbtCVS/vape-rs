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
            if self.editor.take().is_some() || self.focus.take().is_some() || self.confirm_delete {
                self.confirm_delete = false;
                return Vec::new();
            }
            if self.selected.take().is_some() {
                return Vec::new();
            }
            return vec![Action::Close];
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
        if self.confirm_delete {
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
        if let Some(offset) = self.dragging {
            let x = (p.0 - offset.0).clamp(0.0, (self.viewport.0 - 400.0 * self.scale).max(0.0));
            let y = (p.1 - offset.1).clamp(0.0, (self.viewport.1 - 240.0 * self.scale).max(0.0));
            if self.window != Some((x, y)) {
                self.window = Some((x, y));
                return vec![Action::Window(x, y)];
            }
        }
        let local = (
            (p.0 - self.frame_origin.0) / self.scale,
            (p.1 - self.frame_origin.1) / self.scale,
        );
        if let Some(drag) = &self.setting_drag {
            return vec![Self::drag_action(drag, local.0)];
        }
        if let Some(drag) = self.text_drag.clone() {
            let offset = ((local.0 - drag.rect.0 - 4.0) / 6.0).round().max(0.0) as usize;
            self.edit_text(|cursor, text| {
                cursor.caret = offset.min(text.chars().count());
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
        if self.hud_editor {
            if !pressed {
                self.dragging_hud = None;
                return Vec::new();
            }
            if Rect(8.0, self.viewport.1 - 26.0, 58.0, 18.0).contains(p) {
                self.hud_editor = false;
                return Vec::new();
            }
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
                    self.category = "Legit".into();
                    self.page = Page::Modules;
                    self.hud_editor = false;
                    self.scroll = 0.0;
                    return Vec::new();
                }
                if button == "MouseLeft"
                    && Rect(x, y.min(hy), w, (y + h).max(hy + 10.0) - y.min(hy)).contains(p)
                {
                    self.dragging_hud = Some((region.id.clone(), p.0 - x, p.1 - y));
                    return Vec::new();
                }
            }
            return Vec::new();
        }
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
            self.dragging = None;
            self.setting_drag = None;
            self.text_drag = None;
            return Vec::new();
        }
        let local = (
            (p.0 - self.frame_origin.0) / self.scale,
            (p.1 - self.frame_origin.1) / self.scale,
        );
        let Some(hit) = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(local))
            .map(|(_, h)| h.clone())
        else {
            self.focus = None;
            return Vec::new();
        };
        if self.confirm_delete && !matches!(hit, Hit::ConfirmDelete | Hit::Cancel) {
            return Vec::new();
        }
        match hit {
            Hit::Header if button == "MouseLeft" => {
                self.dragging = Some((p.0 - self.frame_origin.0, p.1 - self.frame_origin.1));
            }
            Hit::Page(page) => {
                self.page = page;
                self.selected = None;
                self.focus = None;
                self.editor = None;
                self.scroll = 0.0;
                self.confirm_delete = false;
            }
            Hit::Category(category) => {
                self.category = category;
                self.selected = None;
                self.scroll = 0.0;
            }
            Hit::Toggle(id) => {
                if button == "MouseRight" {
                    self.selected = Some(id);
                    self.scroll = 0.0;
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
            Hit::Settings(id) => {
                self.page = Page::Modules;
                self.selected = Some(id);
                self.scroll = 0.0;
            }
            Hit::Favorite(id) => return vec![Action::Favorite(id)],
            Hit::Focus(id, rect) => {
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
                        Kind::Color => format!("#{:08X}", value.as_u64().unwrap_or(0)),
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
                        Kind::Range { .. } => value
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .unwrap_or_default(),
                        _ => value.to_string(),
                    };
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
            Hit::Choice(id, options, rect) => {
                self.open_choice = Some((id, options, rect));
                self.open_color = None;
                self.editor = None;
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
            Hit::Color(id, color, rect) => {
                self.open_color = Some((id, color, rect));
                self.open_choice = None;
                self.editor = None;
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
                self.focus = None;
            }
            Hit::SelectProfile(name) => {
                self.selected_profile = Some(name.clone());
                self.profile_name = name;
                self.cursor = TextCursor::default();
                self.confirm_delete = false;
            }
            Hit::Save => return vec![Action::Save(self.profile_name.clone())],
            Hit::Load => {
                if let Some(name) = &self.selected_profile {
                    return vec![Action::Load(name.clone())];
                }
            }
            Hit::Rename => {
                if let Some(name) = &self.selected_profile {
                    return vec![Action::Rename(name.clone(), self.profile_name.clone())];
                }
            }
            Hit::Delete => {
                self.focus = None;
                self.editor = None;
                self.confirm_delete = true;
            }
            Hit::ConfirmDelete => {
                if let Some(name) = self.selected_profile.take() {
                    self.confirm_delete = false;
                    return vec![Action::Delete(name)];
                }
            }
            Hit::Cancel => {
                self.confirm_delete = false;
            }
            Hit::Close => return vec![Action::Close],
            Hit::Hud => {
                self.hud_editor = true;
                self.dragging = None;
                self.focus = None;
                self.editor = None;
            }
            Hit::Import => return vec![Action::Import(self.profile_name.clone())],
            Hit::Export => {
                if let Some(name) = &self.selected_profile {
                    return vec![Action::Export(name.clone())];
                }
            }
            Hit::AddPerson(relation) => {
                return vec![Action::UpsertPerson(
                    relation,
                    self.person_name.clone(),
                    self.alias.clone(),
                )];
            }
            Hit::RemovePerson(relation, name) => return vec![Action::RemovePerson(relation, name)],
            Hit::Target(relation, name) => return vec![Action::ToggleTarget(relation, name)],
            Hit::SelectMacro(name) => {
                if let Some(macro_) = view.macros.iter().find(|macro_| macro_.name == name) {
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
                Ok(action) => return vec![action],
                Err(error) => self.status = error,
            },
            Hit::RemoveMacro(name) => return vec![Action::RemoveMacro(name)],
            Hit::CaptureMacro => {
                self.capture = Some(Capture {
                    id: "macro.binds".into(),
                    keys: Vec::new(),
                    first: None,
                })
            }
            Hit::ToggleMacroDouble => self.macro_double_click = !self.macro_double_click,
            _ => {}
        }
        Vec::new()
    }
    pub fn wheel(&mut self, delta: f32) {
        self.scroll = (self.scroll - delta * 24.0).max(0.0);
    }
    pub fn update_viewport(&mut self, viewport: (f32, f32)) {
        self.viewport = viewport;
    }
}
