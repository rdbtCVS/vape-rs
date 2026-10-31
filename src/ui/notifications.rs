// In-world notification toasts (`NotificationManager` + `Notification`):
// a bottom-right stack of up to 20 boxes that slide in from the right edge,
// count down with a progress bar and slide back out. Pure: the core advances
// the animation, the host painter measures text and draws the `Draw` list.

use super::text_gui::{Draw, spans};
use std::collections::VecDeque;

const CAPACITY: usize = 20;
/// Reference width with the 18-unit icon margin, used to turn the pixel
/// slide steps into a fraction of the box.
const NOMINAL_WIDTH: f32 = 136.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Info,
    /// Only the unported hidden-module and online-account paths raise it.
    #[allow(dead_code)]
    Warning,
    Alert,
}

impl Kind {
    pub fn color(self) -> u32 {
        match self {
            Kind::Info => 0xffffffff,
            Kind::Warning => 0xffec812c,
            Kind::Alert => 0xfffa3238,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// Reused notifications (module toggle alerts) share a key.
    pub key: String,
    pub title: String,
    pub message: String,
    pub kind: Kind,
    duration_ms: f32,
    /// `None` until first drawn: the countdown starts when it is shown.
    remaining_ms: Option<f32>,
    /// Slide position: 0 off-screen right, 1 fully shown.
    shown: f32,
    target_shown: f32,
    /// Offset of the box's top edge from the screen bottom (negative).
    y: f32,
    target_y: f32,
}

impl Toast {
    pub fn height(&self) -> f32 {
        // 25 plus the wrapped message (one 0.8-scale line plus 4).
        if self.message.is_empty() { 25.0 } else { 37.0 }
    }
    pub fn progress(&self) -> f32 {
        match self.remaining_ms {
            Some(r) if self.duration_ms > 0.0 => (r / self.duration_ms).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }
    pub fn shown(&self) -> f32 {
        self.shown
    }
    pub fn y(&self) -> f32 {
        self.y
    }
}

#[derive(Debug, Default)]
pub struct Toasts {
    list: VecDeque<Toast>,
}

/// `interpolateToward`: step `|target - current| * 0.3` per 60 Hz frame,
/// truncated to whole units, never below 0.1.
fn toward(target: f32, current: f32, elapsed_ms: f32, unit: f32) -> f32 {
    if target == current {
        return target;
    }
    let step = ((target - current).abs() * unit * 0.3).trunc();
    let step = (step * elapsed_ms.max(1.0) / (1000.0 / 60.0)).max(0.1) / unit;
    current + (target - current).clamp(-step, step)
}

impl Toasts {
    pub fn push(&mut self, key: &str, title: &str, message: &str, kind: Kind, duration_ms: f32) {
        if let Some(toast) = self
            .list
            .iter_mut()
            .find(|t| !key.is_empty() && t.key == key)
        {
            // ReusableTextNotification.reset: new text, full time, slide in.
            toast.title = title.into();
            toast.message = message.into();
            toast.duration_ms = duration_ms;
            toast.remaining_ms = Some(duration_ms);
            toast.target_shown = 1.0;
            return;
        }
        let mut toast = Toast {
            key: key.into(),
            title: title.into(),
            message: message.into(),
            kind,
            duration_ms,
            remaining_ms: None,
            shown: 0.0,
            target_shown: 1.0,
            y: 0.0,
            target_y: 0.0,
        };
        let start = toast.height() + 16.0 + self.list.iter().map(|t| t.height() + 2.0).sum::<f32>();
        toast.y = -start;
        toast.target_y = -start;
        if self.list.len() == CAPACITY {
            self.list.pop_front();
        }
        self.list.push_back(toast);
    }
    pub fn clear(&mut self) {
        self.list.clear();
    }
    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.list.iter()
    }
    /// One render pass: restack, slide, count down and drop finished boxes.
    pub fn update(&mut self, elapsed_ms: f32) {
        let mut next = -14.0;
        for toast in &mut self.list {
            next -= toast.height() + 2.0;
            toast.target_y = next;
            toast.shown = toward(toast.target_shown, toast.shown, elapsed_ms, NOMINAL_WIDTH);
            toast.y = toward(toast.target_y, toast.y, elapsed_ms, 1.0);
            let remaining = toast
                .remaining_ms
                .map_or(toast.duration_ms, |r| r - elapsed_ms);
            toast.remaining_ms = Some(remaining);
            if remaining <= 0.0 {
                // dismiss(): slide 5 units past the right edge.
                toast.target_shown = -5.0 / NOMINAL_WIDTH;
            }
        }
        // shouldRemove: currentX >= 1 unit past the edge.
        self.list
            .retain(|t| !(t.target_shown < 0.0 && t.shown <= -1.0 / NOMINAL_WIDTH));
    }
}

/// Lay out every toast against a `screen` of HUD units. `measure(text,
/// size)` is the width of `text` at `size` times the 8-unit font line.
pub fn layout(
    toasts: &Toasts,
    screen: (f32, f32),
    measure: impl Fn(&str, f32) -> f32,
) -> Vec<Draw> {
    let plain = |text: &str| {
        spans(text, 0)
            .into_iter()
            .map(|s| s.text)
            .collect::<String>()
    };
    let mut draws = Vec::new();
    for toast in toasts.iter() {
        // getWidth: max(118, title + 2, message + 4) + 18.
        let width = (measure(&plain(&toast.title), 1.0) + 2.0)
            .max(measure(&plain(&toast.message), 0.8) + 4.0)
            .max(118.0)
            + 18.0;
        let height = toast.height();
        let x = screen.0 + 3.0 - width * toast.shown();
        let y = screen.1 + toast.y();
        let fill = |draws: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, color: u32| {
            draws.push(Draw::Fill { x, y, w, h, color });
        };
        // A 1-unit border around a translucent body with clipped corners.
        let border = 0xff303030;
        fill(&mut draws, x + 1.0, y, width - 2.0, 1.0, border);
        fill(
            &mut draws,
            x + 1.0,
            y + height - 1.0,
            width - 2.0,
            1.0,
            border,
        );
        fill(&mut draws, x, y + 1.0, 1.0, height - 2.0, border);
        fill(
            &mut draws,
            x + width - 1.0,
            y + 1.0,
            1.0,
            height - 2.0,
            border,
        );
        fill(
            &mut draws,
            x + 1.0,
            y + 1.0,
            width - 2.0,
            height - 2.0,
            0xad000000,
        );
        // Authored "i" badge: a ring with a dot and stem in the type colour.
        let (cx, cy) = (x + 6.0, y + 6.0);
        let icon = toast.kind.color();
        for (dx, dy, w, h) in [
            (3.0, 0.0, 6.0, 1.0),
            (3.0, 11.0, 6.0, 1.0),
            (0.0, 3.0, 1.0, 6.0),
            (11.0, 3.0, 1.0, 6.0),
            (1.0, 1.0, 2.0, 2.0),
            (9.0, 1.0, 2.0, 2.0),
            (1.0, 9.0, 2.0, 2.0),
            (9.0, 9.0, 2.0, 2.0),
            (5.5, 3.0, 1.0, 1.0),
            (5.5, 5.0, 1.0, 4.0),
        ] {
            fill(&mut draws, cx + dx, cy + dy, w, h, icon);
        }
        let title_color = if toast.kind == Kind::Alert {
            toast.kind.color()
        } else {
            0xffe6e6e6
        };
        let text = |draws: &mut Vec<Draw>, x: f32, y: f32, size: f32, raw: &str, base: u32| {
            let mut x = x;
            for span in spans(raw, base) {
                let w = measure(&span.text, size);
                draws.push(Draw::Text {
                    x,
                    y,
                    size,
                    text: span.text,
                    color: span.color,
                    shadow: None,
                });
                x += w;
            }
        };
        text(
            &mut draws,
            x + 23.0,
            y + 6.0,
            1.0,
            &toast.title,
            title_color,
        );
        if !toast.message.is_empty() {
            text(
                &mut draws,
                x + 23.0,
                y + 21.0,
                0.8,
                &toast.message,
                0xffaaaaaa,
            );
        }
        let bar = (width * toast.progress() - 1.0).max(0.0);
        fill(
            &mut draws,
            x + 1.0,
            y + height - 1.5,
            bar,
            0.5,
            toast.kind.color(),
        );
    }
    draws
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(toasts: &mut Toasts, ms: f32) {
        for _ in 0..(ms / 16.0) as usize {
            toasts.update(16.0);
        }
    }

    #[test]
    fn toasts_slide_in_stack_upward_and_expire() {
        let mut t = Toasts::default();
        t.push("a", "Sprint", "\u{a7}2Enabled", Kind::Info, 1500.0);
        t.push(
            "",
            "Loaded",
            "Press RSHIFT to open the GUI",
            Kind::Info,
            5000.0,
        );
        settle(&mut t, 1000.0);
        let all: Vec<_> = t.iter().collect();
        assert!((all[0].shown() - 1.0).abs() < 1e-3);
        assert_eq!(all[0].y(), -14.0 - 39.0);
        assert_eq!(all[1].y(), -14.0 - 39.0 - 39.0);
        settle(&mut t, 2500.0);
        // The toggle alert expired, slid out and the other one moved down.
        let all: Vec<_> = t.iter().collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "Loaded");
        assert_eq!(all[0].y(), -14.0 - 39.0);
    }

    #[test]
    fn reused_keys_update_in_place_and_capacity_drops_the_oldest() {
        let mut t = Toasts::default();
        t.push("m", "Fly", "\u{a7}2Enabled", Kind::Info, 1500.0);
        settle(&mut t, 1000.0);
        t.push("m", "Fly", "\u{a7}cDisabled", Kind::Info, 1500.0);
        assert_eq!(t.iter().count(), 1);
        assert_eq!(t.iter().next().unwrap().progress(), 1.0);
        for n in 0..25 {
            t.push("", &n.to_string(), "", Kind::Info, 100.0);
        }
        assert_eq!(t.iter().count(), CAPACITY);
        assert_eq!(t.iter().next().unwrap().title, "5");
    }

    #[test]
    fn layout_anchors_boxes_to_the_bottom_right() {
        let mut t = Toasts::default();
        t.push("", "Hi", "", Kind::Alert, 5000.0);
        settle(&mut t, 1500.0);
        let draws = layout(&t, (400.0, 300.0), |s, size| s.len() as f32 * 6.0 * size);
        let body = draws
            .iter()
            .find_map(|d| match d {
                Draw::Fill {
                    x,
                    y,
                    w,
                    h,
                    color: 0xad000000,
                } => Some((*x, *y, *w, *h)),
                _ => None,
            })
            .unwrap();
        // width 136: x = 400 + 3 - 136; y = 300 - 14 - 27.
        assert_eq!(body, (268.0, 260.0, 134.0, 23.0));
        assert!(draws.iter().any(|d| matches!(
            d,
            Draw::Text { text, color: 0xfffa3238, .. } if text == "Hi"
        )));
    }
}
