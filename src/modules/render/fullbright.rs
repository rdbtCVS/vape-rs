// Render-only brightness ownership. Real potion state and saved options stay host-owned.
use std::time::Instant;
#[derive(Debug, Default)]
pub struct Gamma {
    current: Option<f32>,
    original: Option<f32>,
    last: Option<Instant>,
}
impl Gamma {
    pub fn update(
        &mut self,
        enabled: bool,
        gamma_mode: bool,
        fade: bool,
        base: f32,
        immediate: bool,
    ) -> Option<f32> {
        if !base.is_finite() {
            return None;
        }
        let active = enabled && gamma_mode;
        if active && self.current.is_none() {
            self.current = Some(base);
            self.original = Some(base);
            self.last = None;
        }
        let current = self.current?;
        let target = if active {
            10.0
        } else {
            self.original.unwrap_or(base)
        };
        let next = if !fade || immediate || !gamma_mode {
            target
        } else if self
            .last
            .is_none_or(|last| last.elapsed().as_millis() >= 10)
        {
            self.last = Some(Instant::now());
            if current < target {
                (current + 0.4).min(target)
            } else {
                (current - 0.4).max(target)
            }
        } else {
            current
        };
        if !active && next == target {
            self.current = None;
            self.original = None;
            self.last = None;
        } else {
            self.current = Some(next);
        }
        Some(next)
    }
}
