// GUI painting: windows, pages, editors and module rows.
use super::*;

impl Gui {
    pub fn frame(&mut self, view: &View, viewport: (f32, f32)) -> Frame {
        self.viewport = viewport;
        if self.hud_editor {
            self.frame_origin = (0.0, 0.0);
            self.scale = 1.0;
            let mut l = Layout {
                draw: Vec::new(),
                hits: Vec::new(),
                clip: Rect(0.0, 0.0, viewport.0, viewport.1),
            };
            l.text(
                8.0,
                viewport.1 - 47.0,
                "Drag HUDs; scroll to scale; right click settings",
                FG,
            );
            l.text(
                8.0,
                viewport.1 - 36.0,
                "Disabled HUDs appear as editor previews",
                MUTED,
            );
            l.fill(Rect(8.0, viewport.1 - 26.0, 58.0, 18.0), HEADER);
            l.text(17.0, viewport.1 - 21.0, "Back", FG);
            return Frame {
                draw: l.draw,
                origin: (0.0, 0.0),
                scale: 1.0,
                hud_editor: true,
            };
        }
        self.scale = (viewport.0 / 400.0)
            .min(viewport.1 / 240.0)
            .min(self.user_scale)
            .clamp(0.01, 4.0);
        let maximum = (
            (viewport.0 - 400.0 * self.scale).max(0.0),
            (viewport.1 - 240.0 * self.scale).max(0.0),
        );
        self.frame_origin = self.window.unwrap_or((maximum.0 / 2.0, maximum.1 / 2.0));
        self.frame_origin.0 = self.frame_origin.0.clamp(0.0, maximum.0);
        self.frame_origin.1 = self.frame_origin.1.clamp(0.0, maximum.1);
        let mut layout = Layout {
            draw: Vec::new(),
            hits: Vec::new(),
            clip: Rect(0.0, 0.0, 400.0, 240.0),
        };
        layout.fill(Rect(0.0, 0.0, 400.0, 240.0), BG);
        layout.fill(Rect(0.0, 0.0, 400.0, 40.0), HEADER);
        layout.hit(Rect(0.0, 0.0, 400.0, 40.0), Hit::Header);
        layout.text(12.0, 16.0, "VAPE", ACCENT);
        for (x, page, label) in [
            (64.0, Page::Modules, "Modules"),
            (122.0, Page::Friends, "Friends"),
            (180.0, Page::Profiles, "Profiles"),
            (235.0, Page::Macros, "Macros"),
        ] {
            layout.text(x, 16.0, label, if page == self.page { FG } else { MUTED });
            layout.hit(Rect(x - 4.0, 10.0, 54.0, 24.0), Hit::Page(page));
            if page == self.page {
                layout.fill(Rect(x, 32.0, 49.0, 1.0), ACCENT);
            }
        }
        layout.text(365.0, 16.0, "X", MUTED);
        layout.hit(Rect(357.0, 8.0, 37.0, 27.0), Hit::Close);
        layout.text(292.0, 16.0, "HUD", MUTED);
        layout.hit(Rect(286.0, 8.0, 29.0, 27.0), Hit::Hud);
        layout.text(320.0, 16.0, "Setup", MUTED);
        layout.hit(Rect(314.0, 8.0, 42.0, 27.0), Hit::Settings("client".into()));
        match self.page {
            Page::Modules => self.modules(view, &mut layout),
            Page::Friends => self.people(view, &mut layout),
            Page::Profiles => self.profiles_page(view, &mut layout),
            Page::Macros => self.macros_page(view, &mut layout),
        }
        layout.clip = Rect(0.0, 0.0, 400.0, 240.0);
        let status = if self.capture.is_some() {
            "Press a key/chord; left click clears binding".into()
        } else if self.editor.is_some() {
            "Enter applies edit; Escape cancels".into()
        } else {
            self.status.clone()
        };
        layout.fill(Rect(0.0, 224.0, 400.0, 16.0), HEADER);
        layout.text(
            8.0,
            228.0,
            &status.chars().take(64).collect::<String>(),
            MUTED,
        );
        self.hits = layout.hits;
        Frame {
            draw: layout.draw,
            origin: self.frame_origin,
            scale: self.scale,
            hud_editor: false,
        }
    }
    pub(super) fn edit_paint(&self, l: &mut Layout, rect: Rect, value: &str) {
        let len = value.chars().count();
        l.draw.push(Draw {
            paint: Paint::Edit(
                rect,
                value.into(),
                self.cursor.caret.min(len),
                self.cursor.anchor.min(len),
            ),
            clip: intersect(l.clip, rect),
        });
    }
    pub(super) fn field(
        &self,
        l: &mut Layout,
        rect: Rect,
        id: &str,
        placeholder: &str,
        value: &str,
    ) {
        l.fill(rect, HEADER);
        if self.focus.as_deref() == Some(id) {
            self.edit_paint(l, rect, value);
        } else {
            let clip = l.clip;
            l.clip = intersect(clip, rect);
            l.text(
                rect.0 + 4.0,
                rect.1 + 5.0,
                if value.is_empty() { placeholder } else { value },
                if value.is_empty() { MUTED } else { FG },
            );
            l.clip = clip;
        }
        l.hit(rect, Hit::Focus(id.into(), rect));
        if self.focus.as_deref() == Some(id) {
            l.fill(Rect(rect.0, rect.1 + rect.3 - 1.0, rect.2, 1.0), ACCENT);
        }
    }
    pub(super) fn modules(&mut self, view: &View, l: &mut Layout) {
        for (i, name) in [
            "Favorites",
            "Combat",
            "Render",
            "Utility",
            "World",
            "Inventory",
            "Minigames",
            "Other",
            "Legit",
        ]
        .iter()
        .enumerate()
        {
            let y = 46.0 + i as f32 * 18.0;
            if self.category == *name {
                l.fill(Rect(5.0, y, 94.0, 17.0), CARD);
                l.fill(Rect(5.0, y, 2.0, 17.0), ACCENT);
            }
            l.text(
                13.0,
                y + 5.0,
                name,
                if self.category == *name { FG } else { MUTED },
            );
            l.hit(Rect(5.0, y, 94.0, 17.0), Hit::Category((*name).into()));
        }
        self.field(
            l,
            Rect(112.0, 47.0, 280.0, 18.0),
            "search",
            "Search modules...",
            &self.search,
        );
        let right = if self.selected.is_some() {
            274.0
        } else {
            394.0
        };
        let matches: Vec<_> = view
            .modules
            .iter()
            .filter(|m| {
                (self.search.is_empty()
                    && (self.category == m.category
                        || (self.category == "Favorites" && m.favorite)))
                    || (!self.search.is_empty()
                        && m.name.to_lowercase().contains(&self.search.to_lowercase()))
            })
            .collect();
        let max_scroll = (matches.len() as f32 * 31.0 - 146.0).max(0.0);
        if self.selected.is_none() {
            self.scroll = self.scroll.min(max_scroll);
        }
        l.clip = Rect(108.0, 70.0, right - 108.0, 151.0);
        for (i, module) in matches.iter().enumerate() {
            let y = 71.0 + i as f32 * 31.0
                - if self.selected.is_none() {
                    self.scroll
                } else {
                    0.0
                };
            let rect = Rect(112.0, y, right - 112.0, 28.0);
            l.fill(rect, if module.enabled { 0xff173c32 } else { CARD });
            l.text(119.0, y + 10.0, &module.name, FG);
            l.hit(rect, Hit::Toggle(module.id.clone()));
            l.text(
                right - 69.0,
                y + 10.0,
                if module.favorite { "*" } else { "+" },
                if module.favorite { ACCENT } else { MUTED },
            );
            l.hit(
                Rect(right - 73.0, y + 2.0, 17.0, 24.0),
                Hit::Favorite(module.id.clone()),
            );
            l.text(right - 51.0, y + 10.0, "...", MUTED);
            l.hit(
                Rect(right - 56.0, y + 2.0, 24.0, 24.0),
                Hit::Settings(module.id.clone()),
            );
            l.fill(
                Rect(right - 27.0, y + 9.0, 20.0, 10.0),
                if module.enabled { ACCENT } else { 0xff494849 },
            );
        }
        if matches.is_empty() {
            l.text(
                120.0,
                88.0,
                if self.category == "Favorites" {
                    "No favorites"
                } else {
                    "No matches"
                },
                MUTED,
            );
        }
        if let Some(id) = self.selected.clone() {
            l.clip = Rect(278.0, 70.0, 116.0, 151.0);
            l.fill(Rect(278.0, 70.0, 116.0, 151.0), HEADER);
            let definitions: Vec<_> = view
                .definitions
                .iter()
                .filter(|d| d.id.starts_with(&(id.clone() + ".")))
                .collect();
            self.scroll = self
                .scroll
                .min((definitions.len() as f32 * 31.0 - 143.0).max(0.0));
            for (i, d) in definitions.into_iter().enumerate() {
                let y = 74.0 + i as f32 * 31.0 - self.scroll;
                l.text(283.0, y, &d.label, MUTED);
                let value = &view.values[&d.id];
                let rect = Rect(282.0, y + 10.0, 108.0, 17.0);
                let editor = self.editor.as_ref().filter(|e| e.id == d.id);
                let text = if let Some(editor) = editor {
                    editor.text.clone()
                } else {
                    match &d.kind {
                        Kind::Boolean => {
                            if value.as_bool().unwrap_or(false) {
                                "ON".into()
                            } else {
                                "OFF".into()
                            }
                        }
                        Kind::Choice(_) | Kind::Text { .. } => value.as_str().unwrap_or("").into(),
                        Kind::Color => format!("#{:08X}", value.as_u64().unwrap_or(0)),
                        Kind::StringList { .. } => value
                            .as_array()
                            .map(|v| {
                                v.iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(" + ")
                            })
                            .unwrap_or_default(),
                        _ => value.to_string(),
                    }
                };
                l.fill(rect, CARD);
                match &d.kind {
                    Kind::Number { min, max, .. } => {
                        let t = ((value.as_f64().unwrap_or(*min) - min)
                            / (max - min).max(f64::EPSILON)) as f32;
                        l.fill(
                            Rect(
                                rect.0 + 2.0,
                                rect.1 + 13.0,
                                (rect.2 - 4.0) * t.clamp(0.0, 1.0),
                                2.0,
                            ),
                            ACCENT,
                        );
                    }
                    Kind::Range { min, max, .. } => {
                        let endpoints = value.as_array().cloned().unwrap_or_default();
                        let lo = endpoints.first().and_then(Value::as_f64).unwrap_or(*min);
                        let hi = endpoints.get(1).and_then(Value::as_f64).unwrap_or(*max);
                        let span = (max - min).max(f64::EPSILON);
                        let a = ((lo - min) / span) as f32;
                        let b = ((hi - min) / span) as f32;
                        l.fill(
                            Rect(
                                rect.0 + 2.0 + (rect.2 - 4.0) * a,
                                rect.1 + 13.0,
                                (rect.2 - 4.0) * (b - a).max(0.0),
                                2.0,
                            ),
                            ACCENT,
                        );
                    }
                    Kind::Color => l.fill(
                        Rect(rect.0 + rect.2 - 13.0, rect.1 + 3.0, 9.0, 9.0),
                        value.as_u64().unwrap_or(0) as u32,
                    ),
                    _ => {}
                }
                if editor.is_some() {
                    self.edit_paint(l, rect, &text);
                } else {
                    let clip = l.clip;
                    l.clip = intersect(clip, rect);
                    l.text(rect.0 + 3.0, rect.1 + 5.0, &text, FG);
                    l.clip = clip;
                }
                let hit = match &d.kind {
                    Kind::Boolean => Hit::Cycle(d.id.clone(), vec![json!(false), json!(true)]),
                    Kind::Choice(options) => Hit::Choice(d.id.clone(), options.clone(), rect),
                    Kind::StringList { .. } if d.id.ends_with(".binds") => {
                        Hit::Capture(d.id.clone())
                    }
                    Kind::Number { min, max, step } => {
                        Hit::Slider(d.id.clone(), *min, *max, *step, rect)
                    }
                    Kind::Range { min, max, step } => {
                        let endpoints = value.as_array().cloned().unwrap_or_default();
                        let lo = endpoints.first().and_then(Value::as_f64).unwrap_or(*min);
                        let hi = endpoints.get(1).and_then(Value::as_f64).unwrap_or(*max);
                        Hit::RangeSlider(d.id.clone(), *min, *max, *step, lo, hi, rect)
                    }
                    Kind::Color => {
                        Hit::Color(d.id.clone(), value.as_u64().unwrap_or(0) as u32, rect)
                    }
                    _ => Hit::Edit(d.id.clone(), value.clone(), rect),
                };
                l.hit(rect, hit);
            }
            if let Some((choice_id, options, rect)) = self.open_choice.clone() {
                let menu = Rect(
                    rect.0,
                    rect.1 + rect.3,
                    rect.2,
                    (options.len().min(5) as f32) * 17.0,
                );
                l.fill(menu, BG);
                for (index, option) in options.iter().take(5).enumerate() {
                    let row = Rect(menu.0, menu.1 + index as f32 * 17.0, menu.2, 17.0);
                    l.text(row.0 + 4.0, row.1 + 5.0, option, FG);
                    l.hit(row, Hit::ChoiceValue(choice_id.clone(), json!(option)));
                }
            }
            if let Some((color_id, _opened_color, rect)) = self.open_color.clone() {
                let color = view.values[&color_id].as_u64().unwrap_or(0) as u32;
                let popup = Rect(rect.0, rect.1 + rect.3, rect.2, 68.0);
                l.fill(popup, BG);
                for channel in 0..4 {
                    let row = Rect(
                        popup.0 + 4.0,
                        popup.1 + 3.0 + channel as f32 * 16.0,
                        popup.2 - 8.0,
                        11.0,
                    );
                    let byte = ((color >> (24 - channel * 8)) & 0xff) as f32 / 255.0;
                    l.text(row.0, row.1, ["A", "R", "G", "B"][channel], MUTED);
                    l.fill(
                        Rect(row.0 + 10.0, row.1 + 5.0, (row.2 - 13.0) * byte, 2.0),
                        [0xffd1d1d1, 0xffe35c5c, 0xff5cd87a, 0xff5c8de3][channel],
                    );
                    let slider = Rect(row.0 + 10.0, row.1, row.2 - 13.0, row.3);
                    l.hit(slider, Hit::ColorChannel(color_id.clone(), channel, slider));
                }
            }
        }
    }
    pub(super) fn profiles_page(&self, view: &View, l: &mut Layout) {
        l.text(10.0, 49.0, "Local profiles", FG);
        l.clip = Rect(6.0, 64.0, 178.0, 155.0);
        for (i, name) in view.profiles.iter().enumerate() {
            let y = 68.0 + i as f32 * 19.0 - self.scroll;
            if self.selected_profile.as_ref() == Some(name) {
                l.fill(Rect(7.0, y, 174.0, 18.0), CARD);
            }
            l.text(12.0, y + 5.0, name, FG);
            l.hit(Rect(7.0, y, 174.0, 18.0), Hit::SelectProfile(name.clone()));
        }
        l.clip = Rect(190.0, 44.0, 204.0, 176.0);
        self.field(
            l,
            Rect(196.0, 49.0, 194.0, 20.0),
            "profile",
            "Profile name",
            &self.profile_name,
        );
        for (x, y, label, hit) in [
            (196.0, 78.0, "Save", Hit::Save),
            (296.0, 78.0, "Load", Hit::Load),
            (196.0, 104.0, "Rename", Hit::Rename),
            (296.0, 104.0, "Delete", Hit::Delete),
            (196.0, 130.0, "Import", Hit::Import),
            (296.0, 130.0, "Export", Hit::Export),
        ] {
            l.button(Rect(x, y, 94.0, 21.0), label, hit);
        }
        l.text(196.0, 162.0, "Import: imports/<name>.json", MUTED);
        l.text(196.0, 177.0, "Export: exports/<name>.json", MUTED);
        if self.confirm_delete {
            l.hits.clear();
            l.fill(Rect(192.0, 74.0, 202.0, 127.0), BG);
            l.text(201.0, 88.0, "Delete selected profile?", FG);
            l.button(Rect(200.0, 115.0, 87.0, 22.0), "Delete", Hit::ConfirmDelete);
            l.button(Rect(296.0, 115.0, 87.0, 22.0), "Cancel", Hit::Cancel);
        }
    }
    pub(super) fn macros_page(&self, view: &View, l: &mut Layout) {
        l.text(10.0, 49.0, "Local macros", FG);
        l.clip = Rect(6.0, 64.0, 178.0, 155.0);
        for (i, macro_) in view.macros.iter().enumerate() {
            let y = 68.0 + i as f32 * 22.0 - self.scroll;
            if self.selected_macro.as_deref() == Some(&macro_.name) {
                l.fill(Rect(7.0, y, 174.0, 20.0), CARD);
            }
            l.text(12.0, y + 4.0, &macro_.name, FG);
            l.text(12.0, y + 13.0, &macro_.binds.join(" + "), MUTED);
            l.hit(
                Rect(7.0, y, 174.0, 20.0),
                Hit::SelectMacro(macro_.name.clone()),
            );
        }
        l.clip = Rect(190.0, 44.0, 204.0, 176.0);
        self.field(
            l,
            Rect(196.0, 49.0, 194.0, 20.0),
            "macro-name",
            "Item or /command",
            &self.macro_name,
        );
        self.field(
            l,
            Rect(196.0, 76.0, 94.0, 20.0),
            "macro-delay",
            "Delay min,max",
            &self.macro_delay,
        );
        l.button(
            Rect(296.0, 76.0, 94.0, 20.0),
            &if self.macro_binds.is_empty() {
                "Bind...".into()
            } else {
                self.macro_binds.join("+")
            },
            Hit::CaptureMacro,
        );
        l.button(
            Rect(196.0, 103.0, 194.0, 20.0),
            if self.macro_double_click {
                "Double click: ON"
            } else {
                "Double click: OFF"
            },
            Hit::ToggleMacroDouble,
        );
        if self.macro_double_click {
            self.field(
                l,
                Rect(196.0, 130.0, 194.0, 20.0),
                "macro-double-delay",
                "Double delay min,max",
                &self.macro_double_delay,
            );
        }
        l.button(Rect(196.0, 159.0, 94.0, 21.0), "Save", Hit::SaveMacro);
        if let Some(name) = &self.selected_macro {
            l.button(
                Rect(296.0, 159.0, 94.0, 21.0),
                "Remove",
                Hit::RemoveMacro(name.clone()),
            );
        }
        l.text(196.0, 190.0, "Physical keys only; 1–3 keys", MUTED);
    }
    pub(super) fn people(&self, view: &View, l: &mut Layout) {
        self.field(
            l,
            Rect(9.0, 49.0, 92.0, 20.0),
            "person",
            "Name / UUID",
            &self.person_name,
        );
        self.field(
            l,
            Rect(9.0, 76.0, 92.0, 20.0),
            "alias",
            "Alias",
            &self.alias,
        );
        l.button(
            Rect(9.0, 105.0, 92.0, 21.0),
            "Add friend",
            Hit::AddPerson("friends".into()),
        );
        l.button(
            Rect(9.0, 133.0, 92.0, 21.0),
            "Add enemy",
            Hit::AddPerson("enemies".into()),
        );
        l.button(
            Rect(9.0, 168.0, 92.0, 21.0),
            if view.values["friends.use"].as_bool().unwrap_or(true) {
                "Friends: ON"
            } else {
                "Friends: OFF"
            },
            Hit::Cycle("friends.use".into(), vec![json!(false), json!(true)]),
        );
        l.clip = Rect(110.0, 45.0, 283.0, 174.0);
        for (i, person) in view.people.iter().enumerate() {
            let y = 49.0 + i as f32 * 29.0 - self.scroll;
            l.fill(Rect(113.0, y, 278.0, 26.0), CARD);
            l.text(
                119.0,
                y + 4.0,
                &person.name,
                if person.relation == "friends" {
                    ACCENT
                } else {
                    0xfff44242
                },
            );
            l.text(119.0, y + 15.0, &person.alias, MUTED);
            l.text(
                329.0,
                y + 9.0,
                if person.targeted { "ON" } else { "OFF" },
                FG,
            );
            l.hit(
                Rect(325.0, y, 33.0, 26.0),
                Hit::Target(person.relation.clone(), person.name.clone()),
            );
            l.text(371.0, y + 9.0, "x", MUTED);
            l.hit(
                Rect(363.0, y, 28.0, 26.0),
                Hit::RemovePerson(person.relation.clone(), person.name.clone()),
            );
        }
    }
}
