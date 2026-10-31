// GUI painting: the reference click GUI's draggable frames. The navigation
// frame opens category, Favorites, Friends, Profiles, Macros and client
// settings frames; module rows expand their settings in place.
use super::*;

pub(super) const CATEGORIES: [&str; 6] = [
    "Combat",
    "Render",
    "Utility",
    "World",
    "Inventory",
    "Network",
];
const MISC: [&str; 3] = ["Friends", "Profiles", "Macros"];
/// Default columns, in the reference layout's left-to-right order.
const COLUMNS: [&str; 11] = [
    "Render",
    "Combat",
    "Utility",
    "Network",
    "World",
    "Inventory",
    "Favorites",
    "Profiles",
    "Macros",
    "Friends",
    "Settings",
];
const NAV: &str = "gui";
const COLUMN_PITCH: f32 = 116.0;
pub(super) const HUD: &str = "HUD";
/// The reference's `HudModuleGroup` tabs, in order. The GAME group's modules
/// are listed under the Render category instead (user request 2026-10-30).
const HUD_GROUPS: [&str; 3] = ["Favorite", "All", "HUD"];

const COMBAT: [&str; 7] = [
    "     ##", "    ###", "   ### ", "# ###  ", " ###   ", " ##    ", "#  #   ",
];
const RENDER: [&str; 7] = [
    "       ", "  ###  ", " #   # ", "#  #  #", " #   # ", "  ###  ", "       ",
];
const UTILITY: [&str; 7] = [
    "##   ##", "###  # ", " ### # ", "  ###  ", " # ### ", " #  ###", "##   ##",
];
const WORLD: [&str; 7] = [
    "  ###  ", " # # # ", "#######", "#  #  #", "#######", " # # # ", "  ###  ",
];
const INVENTORY: [&str; 7] = [
    "  ###  ", "  # #  ", "#######", "#######", "#     #", "#######", "#######",
];
const NETWORK: [&str; 7] = [
    "      #", "      #", "    # #", "    # #", "  # # #", "  # # #", "# # # #",
];
const STAR: [&str; 7] = [
    "   #   ", "   #   ", "#######", " ##### ", "  ###  ", " ## ## ", " #   # ",
];
const PERSON: [&str; 7] = [
    "  ###  ", "  ###  ", "  ###  ", "       ", " ##### ", "#######", "#######",
];
const PROFILES: [&str; 7] = [
    "###    ", "       ", "#######", "       ", "#####  ", "       ", "#######",
];
const MACROS: [&str; 7] = [
    "#######", "#     #", "# # # #", "#     #", "# ### #", "#     #", "#######",
];
const SEARCH: [&str; 7] = [
    " ###   ", "#   #  ", "#   #  ", "#   #  ", " ####  ", "     # ", "      #",
];
const GEAR: [&str; 7] = [
    "   #   ", " ##### ", " #   # ", "## # ##", " #   # ", " ##### ", "   #   ",
];
const SYNC: [&str; 7] = [
    "  #### ", " #   ##", "#   ###", "#     #", "###   #", "##   # ", " ####  ",
];
const LAYOUT: [&str; 7] = [
    "#      ", "###### ", "#######", "###### ", "#      ", "#      ", "#      ",
];
const UP: [&str; 3] = ["  #  ", " # # ", "#   #"];
const DOWN: [&str; 3] = ["#   #", " # # ", "  #  "];
const RIGHT: [&str; 5] = ["#  ", " # ", "  #", " # ", "#  "];
const DOTS: [&str; 5] = ["#", " ", "#", " ", "#"];
const PLUS: [&str; 5] = ["  #  ", "  #  ", "#####", "  #  ", "  #  "];
const ARROW: [&str; 5] = ["  #  ", "   # ", "#####", "   # ", "  #  "];
const IMPORT: [&str; 5] = ["  #  ", "  #  ", "# # #", " ### ", "  #  "];
const CROSS: [&str; 5] = ["#   #", " # # ", "  #  ", " # # ", "#   #"];
/// `newedit` and `newhide`: the category header's hidden-module edit icon,
/// crossed out once the frame hides something.
const PENCIL: [&str; 7] = [
    "     # ", "    ###", "   ### ", "  ###  ", " ###   ", "##     ", "#      ",
];
const HIDE: [&str; 7] = [
    "#      ", " ####  ", " ##  # ", "#  #  #", " #  ## ", "  #### ", "      #",
];
/// `ThemeColors` bind highlight: label, fill and empty-bind icon.
const HIGHLIGHT_TEXT: u32 = 0xffffa054;
const HIGHLIGHT_FILL: u32 = 0x33ec812c;
const STATUS_SHADOW: u32 = 0xff101010;

fn icon_for(name: &str) -> &'static [&'static str] {
    match name {
        "Combat" => &COMBAT,
        "Render" => &RENDER,
        "Utility" => &UTILITY,
        "World" => &WORLD,
        "Inventory" => &INVENTORY,
        "Network" => &NETWORK,
        "Favorites" => &STAR,
        "Friends" => &PERSON,
        "Profiles" => &PROFILES,
        "Macros" => &MACROS,
        HUD => &LAYOUT,
        _ => &GEAR,
    }
}
/// `ColorUtil.getContrastingGray(accent, 45, 240)` for enabled-row text.
/// `ModuleComponent.renderStatusTooltip`: a dark box over the name with a
/// pointer leaning right. The pointer's triangles (tips 4 and 24 above the
/// row, clipped to it) are painted as 2-pixel slices; `Paint` has no
/// triangle primitive.
fn status_box(l: &mut Layout, x: f32, y: f32, text: &str, color: u32) {
    let width = text_advance(text, ROW_SCALE) + 10.0;
    l.fill(Rect(x, y, width, ROW_HEIGHT), INSET);
    let pointer = x + width - 1.0;
    let bottom = y + ROW_HEIGHT;
    for slice in 0..(ROW_HEIGHT / 2.0) as usize {
        let rise = slice as f32 * 2.0 + 1.0;
        let top = bottom - rise - 1.0;
        l.fill(
            Rect(pointer + 1.0, top, 7.0 * (1.0 - rise / 44.0), 2.0),
            STATUS_SHADOW,
        );
        l.fill(Rect(pointer, top, 7.0 * (1.0 - rise / 24.0), 2.0), INSET);
    }
    l.row_label(x + 6.0, y, ROW_HEIGHT, text, color, ROW_SCALE, Align::Left);
}
fn contrast(color: u32) -> u32 {
    let [r, g, b] = [16, 8, 0].map(|s| ((color >> s) & 0xff) as f32);
    if r * 0.299 + g * 0.587 + b * 0.114 > 150.0 {
        0xff2d2d2d
    } else {
        0xfff0f0f0
    }
}
/// Truncates to an estimated width; the painter's clip bounds the rest.
/// Approximate host-font advance of a field-scale label.
fn text_width(text: &str) -> f32 {
    text_advance(text, FIELD_SCALE)
}
fn fit(text: &str, width: f32, scale: f32) -> String {
    if text_advance(text, scale) <= width {
        return text.into();
    }
    let room = width - text_advance("..", scale);
    let mut pen = 0.0;
    let mut out: String = text
        .chars()
        .take_while(|&c| {
            pen += advance(c) * scale;
            pen <= room
        })
        .collect();
    if out.is_empty() {
        out.extend(text.chars().next());
    }
    out + ".."
}
fn short_key(key: &str) -> &str {
    key.strip_prefix("Key")
        .or_else(|| key.strip_prefix("Digit"))
        .unwrap_or(key)
}
fn decimals(step: f64) -> usize {
    if step >= 1.0 {
        0
    } else {
        (-step.log10()).ceil().clamp(0.0, 4.0) as usize
    }
}

impl Gui {
    pub(super) fn logical(&self) -> (f32, f32) {
        (self.viewport.0 / self.scale, self.viewport.1 / self.scale)
    }
    pub(super) fn is_open(&self, name: &str) -> bool {
        name == NAV || self.panels.get(name).is_some_and(|p| p.open)
    }
    pub(super) fn raise(&mut self, name: &str) {
        self.order.retain(|n| n != name);
        self.order.push(name.into());
    }
    fn default_pos(&self, name: &str) -> (f32, f32) {
        let (width, _) = self.logical();
        match name {
            HUD => ((width - FRAME_WIDTH) / 2.0, 6.0),
            _ => (6.0, 6.0),
        }
    }
    /// Grid columns that fit the screen; frames tile 116 units apart.
    pub(super) fn columns(&self) -> usize {
        ((self.logical().0 - 6.0) / COLUMN_PITCH).floor().max(1.0) as usize
    }
    fn column_x(column: usize) -> f32 {
        6.0 + column as f32 * COLUMN_PITCH
    }
    /// A frame's grid column (kept on screen) and row; the navigation frame
    /// defaults to the top of the first column.
    pub(super) fn slot(&self, name: &str) -> (usize, usize) {
        let panel = self.panels.get(name);
        let column = panel.and_then(|p| p.column).unwrap_or(0);
        (column.min(self.columns() - 1), panel.map_or(0, |p| p.row))
    }
    /// The open frames tiled in `column`, top to bottom, except `skip`.
    pub(super) fn column_names(&self, column: usize, skip: Option<&str>) -> Vec<String> {
        let mut names: Vec<(usize, usize, String)> = std::iter::once(NAV)
            .chain(COLUMNS)
            .enumerate()
            .filter(|(_, n)| self.is_open(n) && Some(*n) != skip && self.slot(n).0 == column)
            .map(|(i, n)| (self.slot(n).1, i, n.to_string()))
            .collect();
        names.sort();
        names.into_iter().map(|(_, _, n)| n).collect()
    }
    fn placed(&self, name: &str) -> bool {
        name == NAV || self.panels.get(name).is_some_and(|p| p.column.is_some())
    }
    /// Gives newly opened frames the bottom of the shortest column, as the
    /// reference stacks its frames.
    fn assign_columns(&mut self, view: &View) {
        let columns = self.columns();
        let mut heights: Vec<f32> = (0..columns)
            .map(|c| {
                self.column_names(c, None)
                    .iter()
                    .filter(|n| self.placed(n))
                    .map(|n| self.natural_height(view, n) + 6.0)
                    .sum()
            })
            .collect();
        for name in COLUMNS {
            if !self.is_open(name) || self.placed(name) {
                continue;
            }
            let column = (0..columns).fold(
                0,
                |best, c| if heights[c] < heights[best] { c } else { best },
            );
            let row = self
                .column_names(column, None)
                .iter()
                .map(|n| self.slot(n).1 + 1)
                .max()
                .unwrap_or(0);
            heights[column] += self.natural_height(view, name) + 6.0;
            let panel = self.panels.entry(name.into()).or_default();
            panel.column = Some(column);
            panel.row = row;
        }
    }
    /// Moves frames that would run past the bottom of the screen to the
    /// bottom of the shortest other column they fit in whole; frames that
    /// fit nowhere stay and scroll. Except `skip`, the frame being dragged.
    fn reflow(&mut self, view: &View, skip: Option<&str>) {
        let limit = self.logical().1 - 6.0;
        let columns = self.columns();
        let mut stacks: Vec<Vec<(String, f32)>> = (0..columns)
            .map(|c| {
                self.column_names(c, skip)
                    .into_iter()
                    .map(|n| {
                        let h = self.natural_height(view, &n);
                        (n, h)
                    })
                    .collect()
            })
            .collect();
        // Where the next frame in a stack would start.
        let bottom = |stack: &[(String, f32)]| stack.iter().fold(6.0, |y, (_, h)| y + h + 6.0);
        for column in 0..columns {
            let (mut i, mut y) = (0, 6.0);
            while i < stacks[column].len() {
                let height = stacks[column][i].1;
                if y + height > limit {
                    let target = (0..columns)
                        .filter(|&c| c != column && bottom(&stacks[c]) + height <= limit)
                        .min_by(|&a, &b| bottom(&stacks[a]).total_cmp(&bottom(&stacks[b])));
                    if let Some(target) = target {
                        let row = stacks[target]
                            .iter()
                            .map(|(n, _)| self.slot(n).1 + 1)
                            .max()
                            .unwrap_or(0);
                        let frame = stacks[column].remove(i);
                        let panel = self.panels.entry(frame.0.clone()).or_default();
                        panel.column = Some(target);
                        panel.row = row;
                        stacks[target].push(frame);
                        continue;
                    }
                }
                y += height + 6.0;
                i += 1;
            }
        }
    }
    /// Drops a dragged frame into the grid slot nearest its free position.
    pub(super) fn drop_panel(&mut self, name: &str) {
        let Some((x, y)) = self.panels.get(name).and_then(|p| p.pos) else {
            return;
        };
        let column = (((x - 6.0) / COLUMN_PITCH).round().max(0.0) as usize).min(self.columns() - 1);
        let mut names = self.column_names(column, Some(name));
        let index = names
            .iter()
            .filter(|n| {
                self.panel_rects
                    .iter()
                    .find(|(m, _)| m == *n)
                    .is_some_and(|(_, r)| r.1 + r.3 / 2.0 < y)
            })
            .count();
        names.insert(index, name.into());
        for (row, n) in names.into_iter().enumerate() {
            let panel = self.panels.entry(n).or_default();
            panel.column = Some(column);
            panel.row = row;
        }
        if let Some(panel) = self.panels.get_mut(name) {
            panel.pos = None;
        }
    }
    pub(super) fn rect_of(&self, name: &str) -> Option<Rect> {
        self.panel_rects
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| *r)
    }
    /// A frame's position, kept on screen with at least its header visible.
    pub(super) fn pos(&self, name: &str) -> (f32, f32) {
        let (width, height) = self.logical();
        let (x, y) = self
            .panels
            .get(name)
            .and_then(|p| p.pos)
            .unwrap_or_else(|| self.default_pos(name));
        (
            x.clamp(0.0, (width - FRAME_WIDTH).max(0.0)),
            y.clamp(0.0, (height - HEADER_HEIGHT).max(0.0)),
        )
    }
    pub fn frame(&mut self, view: &View, viewport: (f32, f32)) -> Frame {
        self.viewport = viewport;
        self.age_row_status();
        self.scale = self
            .user_scale
            .clamp(0.5, 2.0)
            .min(viewport.1 / 250.0)
            .max(0.25);
        let (width, height) = self.logical();
        let mut l = Layout::new(Rect(0.0, 0.0, width, height));
        self.panel_rects.clear();
        self.search_style = SearchStyle::parse(view.values["client.search-bar"].as_str());
        if self.hud_editor {
            l.label(
                6.0,
                height - 38.0,
                "Drag HUDs; scroll to scale; right click settings",
                FG,
                FIELD_SCALE,
                Align::Left,
            );
            l.label(
                6.0,
                height - 29.0,
                "Disabled HUDs appear as editor previews",
                MUTED,
                FIELD_SCALE,
                Align::Left,
            );
            l.button(Rect(6.0, height - 20.0, 40.0, 14.0), "BACK", Hit::HudBack);
            self.panel(view, &mut l, HUD, None);
        } else {
            self.assign_columns(view);
            let dragged = self.dragging.as_ref().map(|d| d.0.clone());
            self.reflow(view, dragged.as_deref());
            for column in 0..self.columns() {
                let mut y = 6.0;
                for name in self.column_names(column, dragged.as_deref()) {
                    let rect = self.panel(view, &mut l, &name, Some((Self::column_x(column), y)));
                    y = rect.1 + rect.3 + 6.0;
                }
            }
            // The dragged frame follows the pointer above the grid.
            if let Some(name) = dragged.filter(|n| self.is_open(n)) {
                self.panel(view, &mut l, &name, None);
            }
            if self.search_style == SearchStyle::Floating {
                self.floating_search(view, &mut l, width);
            }
        }
        self.popups(view, &mut l);
        let status = if self.capture.is_some() {
            "Press a key or chord; left click clears the binding".into()
        } else if self.editor.is_some() {
            "Enter applies the edit; Escape cancels".into()
        } else {
            self.status.clone()
        };
        if !status.is_empty() {
            let text = fit(&status, width - 20.0, FIELD_SCALE);
            let w = text_width(&text) + 12.0;
            l.rounded(Rect((width - w) / 2.0, height - 18.0, w, 13.0), INSET);
            l.row_label(
                width / 2.0,
                height - 18.0,
                13.0,
                &text,
                FG,
                FIELD_SCALE,
                Align::Center,
            );
        }
        if let Some(name) = self.confirm_delete.clone() {
            l.hits.clear();
            l.fill(Rect(0.0, 0.0, width, height), 0x88000000);
            let modal = Rect(width / 2.0 - 70.0, height / 2.0 - 28.0, 140.0, 56.0);
            l.rounded(
                Rect(modal.0 - 0.5, modal.1 - 0.5, modal.2 + 1.0, modal.3 + 1.0),
                OUTLINE,
            );
            l.rounded(modal, BG);
            l.row_label(
                width / 2.0,
                modal.1 + 6.0,
                14.0,
                &fit(&format!("Delete profile {name}?"), 130.0, ROW_SCALE),
                FG,
                ROW_SCALE,
                Align::Center,
            );
            l.button(
                Rect(modal.0 + 8.0, modal.1 + 32.0, 58.0, 15.0),
                "DELETE",
                Hit::ConfirmDelete,
            );
            l.button(
                Rect(modal.0 + 74.0, modal.1 + 32.0, 58.0, 15.0),
                "CANCEL",
                Hit::Cancel,
            );
        }
        self.hits = l.hits;
        Frame {
            draw: l.draw,
            origin: (0.0, 0.0),
            scale: self.scale,
            hud_editor: self.hud_editor,
        }
    }
    /// One frame at its grid slot `at` (or its free position): outline,
    /// header and a body clipped to the screen, which scrolls when it does
    /// not fit. Returns the painted rectangle.
    fn panel(&mut self, view: &View, l: &mut Layout, name: &str, at: Option<(f32, f32)>) -> Rect {
        let (x, y) = at.unwrap_or_else(|| self.pos(name));
        let (_, height) = self.logical();
        let header = if name == NAV {
            NAV_HEADER_HEIGHT
        } else {
            HEADER_HEIGHT
        };
        let collapsed = name != NAV && self.panels.get(name).is_some_and(|p| p.collapsed);
        let room = (height - y - header - 4.0).max(ROW_HEIGHT * 2.0);
        let scroll = self.scrolls.get(name).copied().unwrap_or(0.0);
        let mut body = Layout::new(intersect(l.clip, Rect(x, y + header, FRAME_WIDTH, room)));
        let top = y + header - scroll;
        let content = if collapsed {
            0.0
        } else {
            self.body(view, &mut body, name, x, top)
        };
        let overflow = (content - room).max(0.0);
        self.scrolls.insert(name.into(), scroll.min(overflow));
        let shown = content.min(room);
        let rect = Rect(
            x,
            y,
            FRAME_WIDTH,
            header + shown + if shown > 0.0 { 2.0 } else { 0.0 },
        );
        l.rounded(
            Rect(rect.0 - 0.5, rect.1 - 0.5, rect.2 + 1.0, rect.3 + 1.0),
            OUTLINE,
        );
        l.rounded(rect, BG);
        l.hit(rect, Hit::Panel);
        if name == NAV {
            self.nav_header(l, x, y);
        } else {
            l.hit(Rect(x, y, FRAME_WIDTH, header), Hit::Drag(name.into()));
            l.icon(x + 6.0, y + 6.5, 1.0, icon_for(name), FG);
            l.row_label(x + 17.0, y, header, name, FG, TITLE_SCALE, Align::Left);
            if CATEGORIES.contains(&name) {
                self.edit_header(view, l, name, rect, header);
            }
            if name != HUD {
                l.icon(
                    x + FRAME_WIDTH - 12.0,
                    y + 8.5,
                    1.0,
                    if collapsed { &DOWN } else { &UP },
                    MUTED,
                );
                l.hit(
                    Rect(x + FRAME_WIDTH - 18.0, y, 18.0, header),
                    Hit::Collapse(name.into()),
                );
            }
        }
        l.draw.extend(body.draw);
        l.hits.extend(body.hits);
        if overflow > 0.0 {
            let track = shown;
            let thumb = (track * room / content).max(8.0);
            let offset = (track - thumb) * self.scrolls[name] / overflow;
            l.fill(
                Rect(x + FRAME_WIDTH - 2.0, y + header + offset, 1.0, thumb),
                TRACK,
            );
        }
        self.panel_rects.push((name.into(), rect));
        rect
    }
    /// `ModuleCategoryFrameHeader`: on a hovered frame the pencil (or, with
    /// hidden modules, the crossed-out eye and their count) enters the
    /// hidden-module edit mode, whose "Done" label leaves it.
    fn edit_header(&self, view: &View, l: &mut Layout, name: &str, rect: Rect, header: f32) {
        let (x, y) = (rect.0, rect.1);
        // The reference's 16-wide icon button would overlap our collapse
        // target, so the icon sits in the 8 units left of it.
        let button = Rect(x + FRAME_WIDTH - 26.0, y, 8.0, header);
        if self.edit_hidden {
            let half = text_advance("Done", TITLE_SCALE) / 2.0;
            l.row_label(
                button.0 - half,
                y,
                header,
                "Done",
                FG,
                ROW_SCALE,
                Align::Left,
            );
            l.hit(
                Rect(button.0 - half - 2.0, y, half * 2.0 + 4.0, header),
                Hit::EditHidden,
            );
            return;
        }
        // Registered unhovered too: the hits come from the last frame, and a
        // click at the button implies the header is under the pointer.
        l.hit(button, Hit::EditHidden);
        if !rect.contains(self.hover) {
            return;
        }
        let hidden = view
            .modules
            .iter()
            .filter(|m| m.category == name && !m.visible)
            .count();
        if hidden > 0 {
            let count = hidden.to_string();
            l.row_label(
                button.0 - 1.0,
                y,
                header,
                &count,
                TEXT,
                TITLE_SCALE,
                Align::Right,
            );
        }
        let icon = if hidden > 0 { &HIDE } else { &PENCIL };
        l.icon(button.0 + 0.5, y + 7.5, 1.0, icon, FG);
    }
    /// A frame's content, drawn from `top`; returns its full height.
    fn body(&self, view: &View, l: &mut Layout, name: &str, x: f32, top: f32) -> f32 {
        match name {
            NAV => self.nav_body(view, l, x, top),
            "Favorites" => self.module_list(view, l, x, top, false, |m| m.favorite),
            "Friends" => self.friends_body(view, l, x, top),
            "Profiles" => self.profiles_body(view, l, x, top),
            "Macros" => self.macros_body(view, l, x, top),
            "Settings" => self.settings(view, l, x, top, |d| d.id.starts_with("client.")),
            HUD => self.hud_body(view, l, x, top),
            category => self.module_list(view, l, x, top, self.edit_hidden, |m| {
                m.category == category && (m.visible || self.edit_hidden)
            }),
        }
    }
    /// A frame's unscrolled height with every expanded row shown.
    fn natural_height(&self, view: &View, name: &str) -> f32 {
        let header = if name == NAV {
            NAV_HEADER_HEIGHT
        } else {
            HEADER_HEIGHT
        };
        if self.panels.get(name).is_some_and(|p| p.collapsed) && name != NAV {
            return header;
        }
        let content = self.body(
            view,
            &mut Layout::new(Rect(0.0, 0.0, 0.0, 0.0)),
            name,
            0.0,
            0.0,
        );
        header + content + if content > 0.0 { 2.0 } else { 0.0 }
    }
    fn nav_header(&self, l: &mut Layout, x: f32, y: f32) {
        let h = NAV_HEADER_HEIGHT;
        l.hit(Rect(x, y, FRAME_WIDTH, h), Hit::Drag(NAV.into()));
        if self.searching {
            self.field(
                l,
                Rect(x + 4.0, y + 4.0, FRAME_WIDTH - 46.0, 14.0),
                "search",
                "Search modules",
                &self.search,
            );
        } else {
            for dx in [0.0, 0.4] {
                l.row_label(x + 6.0 + dx, y, h, "VAPE", 0xffffffff, 1.0, Align::Left);
            }
            let badge = Rect(x + 32.0, y + 6.0, 13.0, 10.0);
            l.fill(badge, ACCENT);
            l.fill(
                Rect(badge.0 + 0.5, badge.1 + 0.5, badge.2 - 1.0, badge.3 - 1.0),
                BG,
            );
            l.row_label(
                badge.0 + badge.2 / 2.0,
                badge.1,
                badge.3,
                "V4",
                ACCENT,
                SMALL_SCALE,
                Align::Center,
            );
        }
        for (dx, icon, hit, active) in [
            (38.0, &SEARCH, Hit::Search, self.searching),
            (
                25.0,
                &GEAR,
                Hit::OpenPanel("Settings".into()),
                self.is_open("Settings"),
            ),
            (12.0, &SYNC, Hit::Sync, false),
        ] {
            if matches!(hit, Hit::Search) && self.search_style != SearchStyle::Integrated {
                continue;
            }
            l.icon(
                x + FRAME_WIDTH - dx,
                y + 7.5,
                1.0,
                icon,
                if active { FG } else { MUTED },
            );
            l.hit(Rect(x + FRAME_WIDTH - dx - 3.0, y + 3.0, 13.0, 16.0), hit);
        }
    }
    /// `HudModuleSelectorFrame`: a Favorite/All/HUD tab bar over the
    /// legit (HUD-editor) modules, filtered by the selected group.
    fn hud_body(&self, view: &View, l: &mut Layout, x: f32, top: f32) -> f32 {
        let mut tx = x + 6.0;
        for group in HUD_GROUPS {
            let width = text_advance(group, SMALL_SCALE);
            let selected = self.hud_group == group;
            l.row_label(
                tx,
                top,
                14.0,
                group,
                if selected { FG } else { MUTED },
                SMALL_SCALE,
                Align::Left,
            );
            if selected {
                l.fill(Rect(tx, top + 11.5, width, 0.75), ACCENT);
            }
            l.hit(Rect(tx - 2.0, top, width + 4.0, 14.0), Hit::HudGroup(group));
            tx += width + 9.0;
        }
        l.fill(Rect(x, top + 14.0, FRAME_WIDTH, 0.5), OUTLINE);
        let group = self.hud_group;
        16.0 + self.module_list(view, l, x, top + 16.0, false, |m| match group {
            "Favorite" => m.favorite && m.category == "HUD",
            "All" => m.category == "HUD",
            group => m.category == group,
        })
    }
    fn nav_body(&self, view: &View, l: &mut Layout, x: f32, top: f32) -> f32 {
        if self.search_style == SearchStyle::Integrated && self.searching && !self.search.is_empty()
        {
            return self.search_results(view, l, x, top);
        }
        let mut y = top;
        for name in CATEGORIES {
            self.nav_row(l, x, y, name, true, None);
            y += ROW_HEIGHT;
        }
        l.fill(Rect(x, y, FRAME_WIDTH, 0.5), OUTLINE);
        l.row_label(x + 8.0, y, 14.0, "MISC", FAINT, SMALL_SCALE, Align::Left);
        y += 14.0;
        for name in MISC {
            let badge = (name == "Profiles")
                .then_some(self.selected_profile.as_deref())
                .flatten();
            self.nav_row(l, x, y, name, false, badge);
            y += ROW_HEIGHT;
        }
        l.fill(Rect(x, y, FRAME_WIDTH, 0.5), OUTLINE);
        for (dx, icon, hit, color) in [
            (24.0, &STAR, Hit::OpenPanel("Favorites".into()), FAVORITE),
            (12.0, &LAYOUT, Hit::Hud, MUTED),
        ] {
            l.icon(x + FRAME_WIDTH - dx, y + 6.5, 1.0, icon, color);
            l.hit(Rect(x + FRAME_WIDTH - dx - 3.0, y, 13.0, ROW_HEIGHT), hit);
        }
        y + ROW_HEIGHT - top
    }
    /// Modules matching the search. The reference lists Category.OTHER
    /// (blatant) modules only on an exact name match; Category.HIDDEN never.
    fn search_results(&self, view: &View, l: &mut Layout, x: f32, top: f32) -> f32 {
        let query = self.search.to_lowercase();
        self.module_list(view, l, x, top, false, |m| {
            let name = m.name.to_lowercase();
            match m.category.as_str() {
                "Other" => name == query,
                "Hidden" => false,
                _ => name.contains(&query),
            }
        })
    }
    /// `ModuleSearchFrame`: a 110-wide search bar at the top centre whose
    /// results list below it; the right icon clears a non-empty query.
    fn floating_search(&self, view: &View, l: &mut Layout, width: f32) {
        let (_, height) = self.logical();
        let (x, y) = ((width - FRAME_WIDTH) / 2.0, 7.0);
        let header = 20.0;
        let room = (height - y - header - 4.0).max(ROW_HEIGHT);
        let mut body = Layout::new(intersect(l.clip, Rect(x, y + header, FRAME_WIDTH, room)));
        let content = if self.search.is_empty() {
            0.0
        } else {
            self.search_results(view, &mut body, x, y + header)
        };
        let shown = content.min(room);
        let rect = Rect(
            x,
            y,
            FRAME_WIDTH,
            header + shown + if shown > 0.0 { 2.0 } else { 0.0 },
        );
        l.rounded(
            Rect(rect.0 - 0.5, rect.1 - 0.5, rect.2 + 1.0, rect.3 + 1.0),
            OUTLINE,
        );
        l.rounded(rect, BG);
        l.hit(rect, Hit::Popup);
        self.field(
            l,
            Rect(x + 4.0, y + 3.0, FRAME_WIDTH - 21.0, header - 6.0),
            "search",
            "Search modules",
            &self.search,
        );
        let (icon, hit) = if self.search.is_empty() {
            (&SEARCH[..], Hit::Search)
        } else {
            (&CROSS[..], Hit::ClearSearch)
        };
        let color = if self.focus.as_deref() == Some("search") {
            FG
        } else {
            MUTED
        };
        l.icon(x + FRAME_WIDTH - 13.0, y + 6.5, 1.0, icon, color);
        l.hit(Rect(x + FRAME_WIDTH - 16.0, y + 2.0, 14.0, 16.0), hit);
        l.draw.extend(body.draw);
        l.hits.extend(body.hits);
    }
    fn nav_row(&self, l: &mut Layout, x: f32, y: f32, name: &str, icon: bool, badge: Option<&str>) {
        let row = Rect(x, y, FRAME_WIDTH, ROW_HEIGHT);
        let open = self.is_open(name);
        if row.contains(self.hover) && l.clip.contains(self.hover) {
            l.fill(row, PRESSED);
        }
        let color = if open { FG } else { TEXT };
        let text_x = if icon {
            l.icon(x + 8.0, y + 6.5, 1.0, icon_for(name), color);
            x + 20.0
        } else {
            x + 8.0
        };
        l.row_label(text_x, y, ROW_HEIGHT, name, color, ROW_SCALE, Align::Left);
        if let Some(badge) = badge {
            let rect = Rect(x + 58.0, y + 4.0, 38.0, 12.0);
            l.rounded(rect, BUTTON);
            l.row_label(
                rect.0 + 3.0,
                rect.1,
                rect.3,
                &fit(badge, 32.0, SMALL_SCALE),
                TEXT,
                SMALL_SCALE,
                Align::Left,
            );
        }
        l.icon(
            x + FRAME_WIDTH - 11.0,
            y + 7.5,
            1.0,
            &RIGHT,
            if open { FG } else { FAINT },
        );
        l.hit(row, Hit::OpenPanel(name.into()));
    }
    fn module_list(
        &self,
        view: &View,
        l: &mut Layout,
        x: f32,
        top: f32,
        edit: bool,
        filter: impl Fn(&ModuleView) -> bool,
    ) -> f32 {
        let mut modules: Vec<_> = view.modules.iter().filter(|m| filter(m)).collect();
        modules.sort_by_key(|m| m.name.to_lowercase());
        let mut y = top;
        for module in &modules {
            y += if edit {
                self.edit_row(l, x, y, module)
            } else {
                self.module_row(view, l, x, y, module)
            };
        }
        if modules.is_empty() {
            l.row_label(
                x + 6.0,
                y,
                ROW_HEIGHT,
                "No modules",
                FAINT,
                ROW_SCALE,
                Align::Left,
            );
            y += ROW_HEIGHT;
        }
        y - top
    }
    /// `ModuleComponent`: accent fill when enabled, settings dots on the
    /// right, the binding beside them and expanded settings below.
    fn module_row(&self, view: &View, l: &mut Layout, x: f32, y: f32, m: &ModuleView) -> f32 {
        let row = Rect(x, y, FRAME_WIDTH, ROW_HEIGHT);
        let expanded = self.selected.as_deref() == Some(&m.id);
        let hovered = row.contains(self.hover) && l.clip.contains(self.hover);
        let (background, color) = if m.enabled {
            (Some(ACCENT), contrast(ACCENT))
        } else if expanded || hovered {
            (Some(PRESSED), FG)
        } else {
            (None, TEXT)
        };
        if let Some(background) = background {
            l.fill(row, background);
        }
        l.hit(row, Hit::Toggle(m.id.clone()));
        let status = self
            .row_status
            .as_ref()
            .filter(|s| s.id == m.id)
            .map(|s| s.text);
        if let Some(status) = status {
            status_box(l, x, y, status, color);
        } else {
            l.row_label(
                x + 6.0,
                y,
                ROW_HEIGHT,
                &m.name,
                if m.visible { color } else { FAINT },
                ROW_SCALE,
                Align::Left,
            );
        }
        let accent = if m.enabled { color } else { MUTED };
        if status.is_none() {
            l.icon(x + FRAME_WIDTH - 9.0, y + 7.5, 1.0, &DOTS, accent);
            l.hit(
                Rect(x + FRAME_WIDTH - 16.0, y, 16.0, ROW_HEIGHT),
                Hit::Expand(m.id.clone()),
            );
        }
        let mut right = x + FRAME_WIDTH - 16.0;
        if expanded {
            l.icon(
                right - 8.0,
                y + 6.5,
                1.0,
                &STAR,
                if m.favorite { FAVORITE } else { accent },
            );
            l.hit(
                Rect(right - 11.0, y, 12.0, ROW_HEIGHT),
                Hit::Favorite(m.id.clone()),
            );
            right -= 12.0;
        }
        let binds: Vec<&str> = view.values[format!("{}.binds", m.id)]
            .as_array()
            .map(|keys| {
                keys.iter()
                    .filter_map(Value::as_str)
                    .map(short_key)
                    .collect()
            })
            .unwrap_or_default();
        if status.is_some() {
            // `BindableInputComponent.setHighlighted`: the orange bind box.
            let label = binds.join("+");
            let width = if label.is_empty() {
                10.0
            } else {
                text_advance(&label, SMALL_SCALE) + 5.0
            };
            l.fill(Rect(right - width, y + 5.0, width, 10.0), HIGHLIGHT_FILL);
            if label.is_empty() {
                l.icon(right - 7.5, y + 7.5, 1.0, &PLUS, FAVORITE);
            } else {
                l.row_label(
                    right - 2.5,
                    y,
                    ROW_HEIGHT,
                    &label,
                    HIGHLIGHT_TEXT,
                    SMALL_SCALE,
                    Align::Right,
                );
            }
        } else if !binds.is_empty() {
            l.row_label(
                right - 1.0,
                y,
                ROW_HEIGHT,
                &binds.join("+"),
                accent,
                SMALL_SCALE,
                Align::Right,
            );
        }
        if m.enabled {
            l.fill(Rect(x, y + ROW_HEIGHT - 0.5, FRAME_WIDTH, 0.5), OUTLINE);
        }
        let prefix = format!("{}.", m.id);
        ROW_HEIGHT
            + if expanded {
                self.settings(view, l, x, y + ROW_HEIGHT, |d| d.id.starts_with(&prefix))
            } else {
                0.0
            }
    }
    /// A module row in the hidden-module edit mode: the visibility checkbox
    /// strip replaces toggling, settings, favourite and bind.
    fn edit_row(&self, l: &mut Layout, x: f32, y: f32, m: &ModuleView) -> f32 {
        let row = Rect(x, y, FRAME_WIDTH, ROW_HEIGHT);
        let hovered = row.contains(self.hover) && l.clip.contains(self.hover);
        let (background, color) = if m.enabled {
            (Some(ACCENT), contrast(ACCENT))
        } else if hovered {
            (Some(PRESSED), FG)
        } else {
            (None, TEXT)
        };
        if let Some(background) = background {
            l.fill(row, background);
        }
        let strip = Rect(x, y, 20.0, ROW_HEIGHT);
        l.fill(strip, INSET);
        let inset = |by: f32| {
            Rect(
                x + 7.0 + by,
                y + 7.0 + by,
                20.0 - (7.0 + by) * 2.0,
                ROW_HEIGHT - (7.0 + by) * 2.0,
            )
        };
        if m.visible {
            l.fill(inset(0.0), ACCENT);
            l.fill(inset(0.5), INSET);
            l.fill(inset(1.0), ACCENT);
        } else {
            l.fill(inset(0.0), OUTLINE);
            l.fill(inset(0.5), INSET);
        }
        l.hit(strip, Hit::Visibility(m.id.clone()));
        l.row_label(
            x + 26.0,
            y,
            ROW_HEIGHT,
            &m.name,
            if m.visible { color } else { FAINT },
            ROW_SCALE,
            Align::Left,
        );
        if m.enabled {
            l.fill(
                Rect(x + 20.0, y + ROW_HEIGHT - 0.5, FRAME_WIDTH - 20.0, 0.5),
                OUTLINE,
            );
        }
        ROW_HEIGHT
    }
    /// Inline value components on the darker inset, as the reference's
    /// expanded module.
    fn settings(
        &self,
        view: &View,
        l: &mut Layout,
        x: f32,
        top: f32,
        filter: impl Fn(&Definition) -> bool,
    ) -> f32 {
        let w = FRAME_WIDTH;
        let mut y = top;
        for d in view.definitions.iter().filter(|d| filter(d)) {
            let value = &view.values[&d.id];
            let text_field = matches!(d.kind, Kind::Text { .. })
                || matches!(d.kind, Kind::StringList { .. } if !d.id.ends_with(".binds"));
            let h = match d.kind {
                Kind::Number { .. } | Kind::Range { .. } => 24.0,
                _ if text_field => 30.0,
                _ => 18.0,
            };
            let row = Rect(x, y, w, h);
            l.fill(row, INSET);
            // Width taken at the right of the label line by the value.
            let reserved = match &d.kind {
                Kind::Boolean => {
                    let on = value.as_bool().unwrap_or(false);
                    let track = Rect(x + w - 22.0, y + 5.0, 14.0, 8.0);
                    l.rounded(track, if on { ACCENT } else { TRACK });
                    l.fill(
                        Rect(
                            if on { track.0 + 7.5 } else { track.0 + 1.5 },
                            track.1 + 1.5,
                            5.0,
                            5.0,
                        ),
                        FG,
                    );
                    l.hit(
                        row,
                        Hit::Cycle(d.id.clone(), vec![json!(false), json!(true)]),
                    );
                    26.0
                }
                Kind::Number { min, max, step } => {
                    let current = value.as_f64().unwrap_or(*min);
                    let text = format!("{:.*}", decimals(*step), current);
                    l.row_label(
                        x + w - 8.0,
                        y + 2.0,
                        14.0,
                        &text,
                        FG,
                        FIELD_SCALE,
                        Align::Right,
                    );
                    let track = Rect(x + 8.0, y + 17.0, w - 16.0, 2.0);
                    let t =
                        ((current - min) / (max - min).max(f64::EPSILON)).clamp(0.0, 1.0) as f32;
                    l.fill(track, TRACK);
                    l.fill(Rect(track.0, track.1, track.2 * t, track.3), ACCENT);
                    l.fill(
                        Rect(track.0 + track.2 * t - 2.0, track.1 - 1.5, 4.0, 5.0),
                        FG,
                    );
                    l.hit(row, Hit::Slider(d.id.clone(), *min, *max, *step, track));
                    text_width(&text) + 4.0
                }
                Kind::Range { min, max, step } => {
                    let endpoints = value.as_array().cloned().unwrap_or_default();
                    let lo = endpoints.first().and_then(Value::as_f64).unwrap_or(*min);
                    let hi = endpoints.get(1).and_then(Value::as_f64).unwrap_or(*max);
                    let places = decimals(*step);
                    let text = format!("{lo:.places$} - {hi:.places$}");
                    l.row_label(
                        x + w - 8.0,
                        y + 2.0,
                        14.0,
                        &text,
                        FG,
                        FIELD_SCALE,
                        Align::Right,
                    );
                    let track = Rect(x + 8.0, y + 17.0, w - 16.0, 2.0);
                    let span = (max - min).max(f64::EPSILON);
                    let a = ((lo - min) / span).clamp(0.0, 1.0) as f32;
                    let b = ((hi - min) / span).clamp(0.0, 1.0) as f32;
                    l.fill(track, TRACK);
                    l.fill(
                        Rect(
                            track.0 + track.2 * a,
                            track.1,
                            track.2 * (b - a).max(0.0),
                            track.3,
                        ),
                        ACCENT,
                    );
                    for t in [a, b] {
                        l.fill(
                            Rect(track.0 + track.2 * t - 2.0, track.1 - 1.5, 4.0, 5.0),
                            FG,
                        );
                    }
                    l.hit(
                        row,
                        Hit::RangeSlider(d.id.clone(), *min, *max, *step, lo, hi, track),
                    );
                    text_width(&text) + 4.0
                }
                Kind::Choice(options) => {
                    l.row_label(
                        x + w - 16.0,
                        y,
                        h,
                        value.as_str().unwrap_or(""),
                        FG,
                        FIELD_SCALE,
                        Align::Right,
                    );
                    l.icon(x + w - 12.0, y + 7.5, 0.8, &DOWN, MUTED);
                    l.hit(
                        row,
                        Hit::Choice(d.id.clone(), options.clone(), Rect(x + 4.0, y, w - 8.0, h)),
                    );
                    text_width(value.as_str().unwrap_or("")) + 12.0
                }
                Kind::Color => {
                    l.rounded(
                        Rect(x + w - 18.0, y + 4.5, 10.0, 9.0),
                        value.as_u64().unwrap_or(0) as u32,
                    );
                    l.hit(row, Hit::Color(d.id.clone(), Rect(x + 4.0, y, w - 8.0, h)));
                    14.0
                }
                Kind::StringList { .. } if d.id.ends_with(".binds") => {
                    let keys: Vec<&str> = value
                        .as_array()
                        .map(|k| k.iter().filter_map(Value::as_str).map(short_key).collect())
                        .unwrap_or_default();
                    let capturing = self.capture.as_ref().is_some_and(|c| c.id == d.id);
                    let text = if capturing {
                        "...".into()
                    } else if keys.is_empty() {
                        "None".into()
                    } else {
                        keys.join(" + ")
                    };
                    l.row_label(
                        x + w - 8.0,
                        y,
                        h,
                        &text,
                        if capturing { ACCENT } else { FG },
                        FIELD_SCALE,
                        Align::Right,
                    );
                    l.hit(row, Hit::Capture(d.id.clone()));
                    text_width(&text) + 4.0
                }
                _ => {
                    let rect = Rect(x + 6.0, y + 14.0, w - 12.0, 13.0);
                    l.rounded(rect, BG);
                    if let Some(editor) = self.editor.as_ref().filter(|e| e.id == d.id) {
                        self.edit_paint(l, rect, &editor.text);
                    } else {
                        let text = match &d.kind {
                            Kind::StringList { .. } => value
                                .as_array()
                                .map(|v| {
                                    v.iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default(),
                            _ => value.as_str().unwrap_or("").into(),
                        };
                        let clip = l.clip;
                        l.clip = intersect(clip, rect);
                        l.row_label(
                            rect.0 + 4.0,
                            rect.1,
                            rect.3,
                            &text,
                            FG,
                            FIELD_SCALE,
                            Align::Left,
                        );
                        l.clip = clip;
                    }
                    l.hit(rect, Hit::Edit(d.id.clone(), value.clone(), rect));
                    0.0
                }
            };
            l.row_label(
                x + 8.0,
                y + if h == 18.0 { 0.0 } else { 2.0 },
                if h == 18.0 { h } else { 14.0 },
                &fit(&d.label, w - 16.0 - reserved, FIELD_SCALE),
                TEXT,
                FIELD_SCALE,
                Align::Left,
            );
            y += h;
        }
        y - top
    }
    /// Choice and colour popups draw last, over every frame.
    fn popups(&self, view: &View, l: &mut Layout) {
        let anchor = |wanted: &Option<String>, color: bool| {
            let wanted = wanted.as_deref()?;
            l.hits.iter().find_map(|(_, hit)| match hit {
                Hit::Choice(id, options, rect) if !color && id == wanted => {
                    Some((options.clone(), *rect))
                }
                Hit::Color(id, rect) if color && id == wanted => Some((Vec::new(), *rect)),
                _ => None,
            })
        };
        let choice = anchor(&self.open_choice, false);
        let color = anchor(&self.open_color, true);
        if let (Some(id), Some((options, rect))) = (&self.open_choice, choice) {
            let current = &view.values[id];
            let menu = Rect(
                rect.0,
                rect.1 + rect.3,
                rect.2,
                options.len().min(10) as f32 * 14.0,
            );
            l.rounded(
                Rect(menu.0 - 0.5, menu.1 - 0.5, menu.2 + 1.0, menu.3 + 1.0),
                OUTLINE,
            );
            l.rounded(menu, BG);
            l.hit(menu, Hit::Popup);
            for (index, option) in options.iter().take(10).enumerate() {
                let row = Rect(menu.0, menu.1 + index as f32 * 14.0, menu.2, 14.0);
                let selected = current.as_str() == Some(option.as_str());
                if row.contains(self.hover) {
                    l.fill(row, PRESSED);
                }
                l.row_label(
                    row.0 + 5.0,
                    row.1,
                    row.3,
                    option,
                    if selected { ACCENT } else { FG },
                    FIELD_SCALE,
                    Align::Left,
                );
                l.hit(row, Hit::ChoiceValue(id.clone(), json!(option)));
            }
        }
        if let (Some(id), Some((_, rect))) = (&self.open_color, color) {
            let value = view.values[id].as_u64().unwrap_or(0) as u32;
            let popup = Rect(rect.0, rect.1 + rect.3, rect.2, 72.0);
            l.rounded(
                Rect(popup.0 - 0.5, popup.1 - 0.5, popup.2 + 1.0, popup.3 + 1.0),
                OUTLINE,
            );
            l.rounded(popup, BG);
            l.hit(popup, Hit::Popup);
            for channel in 0..4 {
                let row = Rect(
                    popup.0 + 5.0,
                    popup.1 + 3.0 + channel as f32 * 14.0,
                    popup.2 - 10.0,
                    10.0,
                );
                let byte = ((value >> (24 - channel * 8)) & 0xff) as f32 / 255.0;
                l.row_label(
                    row.0,
                    row.1,
                    row.3,
                    ["A", "R", "G", "B"][channel],
                    MUTED,
                    FIELD_SCALE,
                    Align::Left,
                );
                let track = Rect(row.0 + 9.0, row.1 + 4.0, row.2 - 9.0, 2.0);
                l.fill(track, TRACK);
                l.fill(
                    Rect(track.0, track.1, track.2 * byte, track.3),
                    [0xffd1d1d1, 0xffe35c5c, 0xff5cd87a, 0xff5c8de3][channel],
                );
                l.hit(
                    Rect(track.0, row.1, track.2, row.3),
                    Hit::ColorChannel(id.clone(), channel, track),
                );
            }
            // The colour's "<name> rainbow" switch.
            let rainbow = format!("{id}.rainbow");
            let on = view.values.get(&rainbow).and_then(Value::as_bool);
            if let Some(on) = on {
                let row = Rect(popup.0 + 5.0, popup.1 + 59.0, popup.2 - 10.0, 10.0);
                l.row_label(
                    row.0,
                    row.1,
                    row.3,
                    "Rainbow",
                    MUTED,
                    FIELD_SCALE,
                    Align::Left,
                );
                let track = Rect(row.0 + row.2 - 14.0, row.1 + 1.0, 14.0, 8.0);
                l.rounded(track, if on { ACCENT } else { TRACK });
                l.fill(
                    Rect(
                        if on { track.0 + 7.5 } else { track.0 + 1.5 },
                        track.1 + 1.5,
                        5.0,
                        5.0,
                    ),
                    FG,
                );
                l.hit(row, Hit::Cycle(rainbow, vec![json!(false), json!(true)]));
            }
        }
    }
    pub(super) fn edit_paint(&self, l: &mut Layout, rect: Rect, value: &str) {
        let len = value.chars().count();
        l.draw.push(Draw {
            paint: Paint::Edit {
                rect,
                text: value.into(),
                caret: self.cursor.caret.min(len),
                anchor: self.cursor.anchor.min(len),
                scale: FIELD_SCALE,
            },
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
        l.rounded(rect, INSET);
        if self.focus.as_deref() == Some(id) {
            self.edit_paint(l, rect, value);
            l.fill(
                Rect(rect.0 + 1.0, rect.1 + rect.3 - 0.5, rect.2 - 2.0, 0.5),
                ACCENT,
            );
        } else {
            let clip = l.clip;
            l.clip = intersect(clip, rect);
            l.row_label(
                rect.0 + 4.0,
                rect.1,
                rect.3,
                if value.is_empty() { placeholder } else { value },
                if value.is_empty() { FAINT } else { FG },
                FIELD_SCALE,
                Align::Left,
            );
            l.clip = clip;
        }
        l.hit(rect, Hit::Focus(id.into(), rect));
    }
    fn icon_button(
        &self,
        l: &mut Layout,
        rect: Rect,
        label: &str,
        icon: &[&str],
        hit: Hit,
        active: bool,
    ) {
        l.rounded(rect, if active { PRESSED } else { BUTTON });
        if active {
            l.fill(
                Rect(rect.0 + 1.0, rect.1 + rect.3 - 0.5, rect.2 - 2.0, 0.5),
                ACCENT,
            );
        }
        l.icon(rect.0 + 5.0, rect.1 + (rect.3 - 4.0) / 2.0, 0.8, icon, FG);
        l.row_label(rect.0 + 12.0, rect.1, rect.3, label, FG, 0.5, Align::Left);
        l.hit(rect, hit);
    }
    fn profiles_body(&self, view: &View, l: &mut Layout, x: f32, top: f32) -> f32 {
        let mut y = top + 3.0;
        let half = (FRAME_WIDTH - 15.0) / 2.0;
        self.icon_button(
            l,
            Rect(x + 5.0, y, half, 14.0),
            "CREATE NEW",
            &PLUS,
            Hit::CreateProfile,
            self.profile_input == Some(ProfileInput::Create),
        );
        self.icon_button(
            l,
            Rect(x + 10.0 + half, y, half, 14.0),
            "IMPORT",
            &IMPORT,
            Hit::ImportProfile,
            self.profile_input == Some(ProfileInput::Import),
        );
        y += 18.0;
        if let Some(input) = &self.profile_input {
            let placeholder = match input {
                ProfileInput::Create => "Profile name",
                ProfileInput::Import => "imports/<name>.json",
                ProfileInput::Rename(_) => "New name",
            };
            self.field(
                l,
                Rect(x + 5.0, y, FRAME_WIDTH - 24.0, 14.0),
                "profile",
                placeholder,
                &self.profile_name,
            );
            let submit = Rect(x + FRAME_WIDTH - 17.0, y + 1.0, 12.0, 12.0);
            l.rounded(submit, FG);
            l.icon(submit.0 + 3.5, submit.1 + 3.5, 1.0, &ARROW, INSET);
            l.hit(submit, Hit::SubmitProfile);
            y += 18.0;
        }
        for name in &view.profiles {
            let row = Rect(x + 5.0, y, FRAME_WIDTH - 10.0, 16.0);
            let active = self.selected_profile.as_ref() == Some(name);
            let color = if active {
                l.rounded(row, 0xffffffff);
                INSET
            } else {
                l.rounded(
                    row,
                    if row.contains(self.hover) {
                        PRESSED
                    } else {
                        0xff1e1d1e
                    },
                );
                MUTED
            };
            l.row_label(
                row.0 + 5.0,
                y,
                row.3,
                &fit(name, 80.0, FIELD_SCALE),
                color,
                FIELD_SCALE,
                Align::Left,
            );
            l.hit(row, Hit::Profile(name.clone()));
            l.icon(x + FRAME_WIDTH - 11.0, y + 5.5, 1.0, &DOTS, color);
            l.hit(
                Rect(x + FRAME_WIDTH - 18.0, y, 13.0, 16.0),
                Hit::ProfileMenu(name.clone()),
            );
            y += 18.0;
            if self.profile_menu.as_ref() == Some(name) {
                for (label, hit) in [
                    ("Save current settings", Hit::SaveProfile(name.clone())),
                    ("Rename", Hit::RenameProfile(name.clone())),
                    ("Export to exports/", Hit::ExportProfile(name.clone())),
                    ("Delete", Hit::DeleteProfile(name.clone())),
                ] {
                    let item = Rect(x + 5.0, y - 1.0, FRAME_WIDTH - 10.0, 13.0);
                    l.fill(item, INSET);
                    let danger = matches!(hit, Hit::DeleteProfile(_));
                    l.row_label(
                        item.0 + 8.0,
                        item.1,
                        item.3,
                        label,
                        if danger { DANGER } else { TEXT },
                        FIELD_SCALE,
                        Align::Left,
                    );
                    l.hit(item, hit);
                    y += 13.0;
                }
                y += 3.0;
            }
        }
        if view.profiles.is_empty() {
            l.row_label(
                x + 6.0,
                y,
                14.0,
                "No local profiles",
                FAINT,
                FIELD_SCALE,
                Align::Left,
            );
            y += 14.0;
        }
        y + 2.0 - top
    }
    fn macros_body(&self, view: &View, l: &mut Layout, x: f32, top: f32) -> f32 {
        let mut y = top + 3.0;
        self.field(
            l,
            Rect(x + 5.0, y, FRAME_WIDTH - 24.0, 14.0),
            "macro-name",
            "Type item name",
            &self.macro_name,
        );
        let submit = Rect(x + FRAME_WIDTH - 17.0, y + 1.0, 12.0, 12.0);
        l.rounded(submit, FG);
        l.icon(submit.0 + 3.5, submit.1 + 3.5, 1.0, &ARROW, INSET);
        l.hit(submit, Hit::NewMacro);
        y += 18.0;
        if self.macro_open {
            let field_x = x + 44.0;
            let field_w = FRAME_WIDTH - 49.0;
            l.row_label(x + 8.0, y, 14.0, "Delay", TEXT, FIELD_SCALE, Align::Left);
            self.field(
                l,
                Rect(field_x, y, field_w, 14.0),
                "macro-delay",
                "min,max ms",
                &self.macro_delay,
            );
            y += 17.0;
            l.row_label(x + 8.0, y, 14.0, "Bind", TEXT, FIELD_SCALE, Align::Left);
            let capturing = self.capture.as_ref().is_some_and(|c| c.id == "macro.binds");
            let bind = Rect(field_x, y, field_w, 14.0);
            l.rounded(bind, INSET);
            l.row_label(
                bind.0 + 4.0,
                bind.1,
                bind.3,
                &if capturing {
                    "...".into()
                } else if self.macro_binds.is_empty() {
                    "Click to bind".into()
                } else {
                    self.macro_binds
                        .iter()
                        .map(|k| short_key(k))
                        .collect::<Vec<_>>()
                        .join(" + ")
                },
                if capturing {
                    ACCENT
                } else if self.macro_binds.is_empty() {
                    FAINT
                } else {
                    FG
                },
                FIELD_SCALE,
                Align::Left,
            );
            l.hit(bind, Hit::CaptureMacro);
            y += 17.0;
            let double = Rect(x, y, FRAME_WIDTH, 14.0);
            l.row_label(
                x + 8.0,
                y,
                14.0,
                "Double click",
                TEXT,
                FIELD_SCALE,
                Align::Left,
            );
            let track = Rect(x + FRAME_WIDTH - 22.0, y + 3.0, 14.0, 8.0);
            l.rounded(
                track,
                if self.macro_double_click {
                    ACCENT
                } else {
                    TRACK
                },
            );
            l.fill(
                Rect(
                    if self.macro_double_click {
                        track.0 + 7.5
                    } else {
                        track.0 + 1.5
                    },
                    track.1 + 1.5,
                    5.0,
                    5.0,
                ),
                FG,
            );
            l.hit(double, Hit::ToggleMacroDouble);
            y += 17.0;
            if self.macro_double_click {
                l.row_label(x + 8.0, y, 14.0, "Delay 2", TEXT, FIELD_SCALE, Align::Left);
                self.field(
                    l,
                    Rect(field_x, y, field_w, 14.0),
                    "macro-double-delay",
                    "min,max ms",
                    &self.macro_double_delay,
                );
                y += 17.0;
            }
            let half = (FRAME_WIDTH - 15.0) / 2.0;
            l.button(Rect(x + 5.0, y, half, 14.0), "SAVE", Hit::SaveMacro);
            if let Some(name) = &self.selected_macro {
                l.button(
                    Rect(x + 10.0 + half, y, half, 14.0),
                    "REMOVE",
                    Hit::RemoveMacro(name.clone()),
                );
            }
            y += 19.0;
        }
        for macro_ in &view.macros {
            let row = Rect(x + 5.0, y, FRAME_WIDTH - 10.0, 16.0);
            let selected = self.macro_open && self.selected_macro.as_deref() == Some(&macro_.name);
            l.rounded(
                row,
                if selected || row.contains(self.hover) {
                    PRESSED
                } else {
                    0xff1e1d1e
                },
            );
            l.row_label(
                row.0 + 5.0,
                y,
                row.3,
                &fit(&macro_.name, 60.0, FIELD_SCALE),
                if selected { FG } else { TEXT },
                FIELD_SCALE,
                Align::Left,
            );
            l.row_label(
                row.0 + row.2 - 4.0,
                y,
                row.3,
                &macro_
                    .binds
                    .iter()
                    .map(|k| short_key(k))
                    .collect::<Vec<_>>()
                    .join("+"),
                MUTED,
                SMALL_SCALE,
                Align::Right,
            );
            l.hit(row, Hit::SelectMacro(macro_.name.clone()));
            y += 18.0;
        }
        y + 2.0 - top
    }
    fn friends_body(&self, view: &View, l: &mut Layout, x: f32, top: f32) -> f32 {
        let mut y = top + 3.0;
        self.field(
            l,
            Rect(x + 5.0, y, FRAME_WIDTH - 10.0, 14.0),
            "person",
            "Name / UUID",
            &self.person_name,
        );
        y += 17.0;
        self.field(
            l,
            Rect(x + 5.0, y, FRAME_WIDTH - 10.0, 14.0),
            "alias",
            "Alias",
            &self.alias,
        );
        y += 17.0;
        let half = (FRAME_WIDTH - 15.0) / 2.0;
        self.icon_button(
            l,
            Rect(x + 5.0, y, half, 14.0),
            "FRIEND",
            &PLUS,
            Hit::AddPerson("friends".into()),
            false,
        );
        self.icon_button(
            l,
            Rect(x + 10.0 + half, y, half, 14.0),
            "ENEMY",
            &PLUS,
            Hit::AddPerson("enemies".into()),
            false,
        );
        y += 18.0;
        let enabled = view.values["friends.use"].as_bool().unwrap_or(true);
        l.row_label(
            x + 8.0,
            y,
            14.0,
            "Use friends",
            TEXT,
            FIELD_SCALE,
            Align::Left,
        );
        let track = Rect(x + FRAME_WIDTH - 22.0, y + 3.0, 14.0, 8.0);
        l.rounded(track, if enabled { ACCENT } else { TRACK });
        l.fill(
            Rect(
                if enabled {
                    track.0 + 7.5
                } else {
                    track.0 + 1.5
                },
                track.1 + 1.5,
                5.0,
                5.0,
            ),
            FG,
        );
        l.hit(
            Rect(x, y, FRAME_WIDTH, 14.0),
            Hit::Cycle("friends.use".into(), vec![json!(false), json!(true)]),
        );
        y += 17.0;
        y = self.settings(view, l, x, y, |d| {
            matches!(
                d.id.as_str(),
                "friends.recolor" | "friends.color" | "enemies.use-color" | "enemies.color"
            )
        });
        for person in &view.people {
            let row = Rect(x + 5.0, y, FRAME_WIDTH - 10.0, 20.0);
            l.rounded(row, 0xff1e1d1e);
            l.label(
                row.0 + 5.0,
                y + 3.5,
                &fit(&person.name, 62.0, FIELD_SCALE),
                if person.relation == "friends" {
                    ACCENT
                } else {
                    DANGER
                },
                FIELD_SCALE,
                Align::Left,
            );
            if person.alias != person.name {
                l.label(
                    row.0 + 5.0,
                    y + 11.5,
                    &fit(&person.alias, 62.0, SMALL_SCALE),
                    MUTED,
                    SMALL_SCALE,
                    Align::Left,
                );
            }
            l.row_label(
                x + FRAME_WIDTH - 20.0,
                y,
                20.0,
                if person.targeted { "ON" } else { "OFF" },
                if person.targeted { FG } else { FAINT },
                SMALL_SCALE,
                Align::Right,
            );
            l.hit(
                Rect(x + FRAME_WIDTH - 38.0, y, 20.0, 20.0),
                Hit::Target(person.relation.clone(), person.name.clone()),
            );
            l.icon(x + FRAME_WIDTH - 14.0, y + 8.0, 0.8, &CROSS, MUTED);
            l.hit(
                Rect(x + FRAME_WIDTH - 18.0, y, 13.0, 20.0),
                Hit::RemovePerson(person.relation.clone(), person.name.clone()),
            );
            y += 22.0;
        }
        y + 2.0 - top
    }
}
