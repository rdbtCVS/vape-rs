// InventoryFill: while Shift and the left mouse button are held over an open
// container screen, shift-clicks the hovered slot at the "CPS" rate (the
// reference's clicker timing, CPS +-1; 20 CPS clicks every frame). A slot is
// not clicked twice in a row unless the 80 % "remember" roll failed, and
// nothing is clicked while a stack is carried. Each click takes the container
// claim for its duration, so it never interleaves with a planner's click.
use super::super::actions::{Owner, Resource};
use super::settings::Definition;
use super::{ModuleId, Runtime};
use serde_json::Value;
use std::time::{Duration, Instant};

const OWNER: Owner = Owner("inventory.fill");
const PRIORITY: u16 = 90;

#[derive(Debug)]
pub struct Fill {
    last_clicked: Option<u16>,
    last_click: Option<Instant>,
    next_delay: Duration,
    seed: u64,
}
impl Default for Fill {
    fn default() -> Self {
        Self {
            last_clicked: None,
            last_click: None,
            next_delay: Duration::ZERO,
            seed: 0x1f11,
        }
    }
}
impl Fill {
    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// What the host observed this frame.
#[derive(Clone, Copy, Debug)]
pub struct FillInput {
    pub now: Instant,
    pub container_screen: bool,
    pub shift: bool,
    pub left: bool,
    /// The game slot id under the cursor, inside the container's bounds.
    pub hovered: Option<u16>,
    pub carrying: bool,
}

pub(super) fn definitions() -> Vec<Definition> {
    vec![Definition::number(
        "inventory.inventory-fill.cps",
        "CPS",
        1.0,
        15.0,
        20.0,
        0.1,
    )]
}

impl Runtime {
    /// The slot to shift-click now, if any.
    pub fn inventory_fill(&mut self, input: FillInput) -> Option<u16> {
        if !self.enabled(ModuleId::InventoryFill) || !input.left {
            return None;
        }
        let cps = self
            .settings
            .get("inventory.inventory-fill.cps")
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .unwrap_or(15.0)
            .clamp(1.0, 20.0);
        let fill = &mut self.inventory_fill;
        if cps < 20.0
            && fill
                .last_click
                .is_some_and(|at| input.now.saturating_duration_since(at) < fill.next_delay)
        {
            return None;
        }
        if !input.container_screen || !input.shift || input.carrying {
            return None;
        }
        let slot = input.hovered?;
        if fill.last_clicked == Some(slot) {
            return None;
        }
        let lease =
            self.actions
                .acquire(Resource::Container, OWNER, self.session.epoch(), PRIORITY)?;
        self.actions.release(lease);
        let fill = &mut self.inventory_fill;
        if fill.random() <= 0.8 {
            fill.last_clicked = Some(slot);
        }
        let rate = (cps - 1.0) + fill.random() * 2.0;
        fill.next_delay = Duration::from_secs_f64(1.0 / rate.max(0.5));
        fill.last_click = Some(input.now);
        Some(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_hovered_slots_at_the_cps_rate() {
        let mut runtime = Runtime::default();
        runtime.set_enabled(ModuleId::InventoryFill, true);
        let t = Instant::now();
        let mut input = FillInput {
            now: t,
            container_screen: true,
            shift: true,
            left: true,
            hovered: Some(300),
            carrying: false,
        };
        assert_eq!(runtime.inventory_fill(input), Some(300));
        input.hovered = Some(301);
        assert_eq!(runtime.inventory_fill(input), None, "CPS delay");
        input.now = t + Duration::from_millis(200);
        assert_eq!(runtime.inventory_fill(input), Some(301));
        input.now = t + Duration::from_millis(400);
        input.shift = false;
        input.hovered = Some(302);
        assert_eq!(runtime.inventory_fill(input), None);
        input.shift = true;
        input.carrying = true;
        assert_eq!(runtime.inventory_fill(input), None);
    }
}
