// Retained AutoFish timing and rod-replacement policy. The host owns every
// actual input/menu operation; this model only selects the next safe action.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Options {
    pub recast_ground: bool,
    pub recast_caught: bool,
    pub refill_rods: bool,
    pub click_delay_ms: (u64, u64),
}

impl Default for Options {
    fn default() -> Self {
        Self {
            recast_ground: false,
            recast_caught: false,
            refill_rods: true,
            click_delay_ms: (75, 125),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rod {
    Held,
    Hotbar(u8),
    Inventory(u16),
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hook {
    Missing,
    Flying,
    Water { biting: bool },
    Ground,
    Caught,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    pub now: Instant,
    pub rod: Rod,
    pub hook: Hook,
    pub inventory_open: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// A single ordinary use activation: cast when no hook exists, otherwise reel.
    Use,
    SelectHotbar(u8),
    OpenInventory,
    QuickMove(u16),
    CloseInventory,
    Disable,
}

#[derive(Debug)]
pub struct AutoFish {
    cast_due: Option<Instant>,
    ground_since: Option<Instant>,
    refill_due: Option<Instant>,
    refill_issued: bool,
    reel_issued: bool,
    seed: u64,
}

impl Default for AutoFish {
    fn default() -> Self {
        Self {
            cast_due: None,
            ground_since: None,
            refill_due: None,
            refill_issued: false,
            reel_issued: false,
            seed: 0x7a4d_0f51,
        }
    }
}

impl AutoFish {
    pub fn refilling(&self) -> bool {
        self.refill_due.is_some() || self.refill_issued
    }

    pub fn cancel(&mut self) {
        self.cast_due = None;
        self.ground_since = None;
        self.refill_due = None;
        self.refill_issued = false;
        self.reel_issued = false;
    }

    pub fn step(&mut self, context: Context, options: Options) -> Option<Effect> {
        match context.rod {
            Rod::Held => {
                if context.inventory_open {
                    return Some(Effect::CloseInventory);
                }
                self.refill_due = None;
                self.refill_issued = false;
            }
            Rod::Hotbar(slot) => {
                self.cancel();
                if context.inventory_open {
                    return Some(Effect::CloseInventory);
                }
                return Some(Effect::SelectHotbar(slot));
            }
            Rod::Inventory(slot) if options.refill_rods => {
                self.cast_due = None;
                self.ground_since = None;
                if !context.inventory_open {
                    if !self.refill_issued {
                        if self.refill_due.is_none() {
                            let delay = self.delay(options.click_delay_ms);
                            self.refill_due = Some(context.now + Duration::from_millis(delay));
                        }
                    }
                    return Some(Effect::OpenInventory);
                }
                if !self.refill_issued {
                    if self.refill_due.is_none() {
                        let delay = self.delay(options.click_delay_ms);
                        self.refill_due = Some(context.now + Duration::from_millis(delay));
                    }
                    let due = self.refill_due.expect("refill due set above");
                    if context.now >= due {
                        self.refill_issued = true;
                        self.refill_due = None;
                        return Some(Effect::QuickMove(slot));
                    }
                }
                return None;
            }
            Rod::Inventory(_) | Rod::Missing => {
                self.cancel();
                return Some(Effect::Disable);
            }
        }

        match context.hook {
            Hook::Missing => {
                self.ground_since = None;
                self.reel_issued = false;
                let due = self.cast_due.get_or_insert(context.now);
                if context.now >= *due {
                    self.cast_due = Some(context.now + Duration::from_secs(1));
                    return Some(Effect::Use);
                }
            }
            Hook::Flying => self.ground_since = None,
            Hook::Water { biting: true } if !self.reel_issued => {
                return Some(self.reel(context.now));
            }
            Hook::Water { .. } => self.ground_since = None,
            Hook::Caught if options.recast_caught && !self.reel_issued => {
                return Some(self.reel(context.now));
            }
            Hook::Caught if options.recast_caught => {}
            Hook::Caught => {
                self.cancel();
                return Some(Effect::Disable);
            }
            Hook::Ground if options.recast_ground => {
                let since = self.ground_since.get_or_insert(context.now);
                if !self.reel_issued && context.now.duration_since(*since) >= Duration::from_secs(3)
                {
                    return Some(self.reel(context.now));
                }
            }
            Hook::Ground => self.ground_since = None,
        }
        None
    }

    fn reel(&mut self, now: Instant) -> Effect {
        // Match the reference's cast timer: after reeling, wait one second
        // before the missing-hook branch is allowed to cast again.
        self.cast_due = Some(now + Duration::from_secs(1));
        self.ground_since = None;
        self.reel_issued = true;
        Effect::Use
    }

    fn delay(&mut self, range: (u64, u64)) -> u64 {
        let (lo, hi) = if range.0 <= range.1 {
            range
        } else {
            (range.1, range.0)
        };
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        lo.saturating_add(self.seed % hi.saturating_sub(lo).saturating_add(1))
    }
}
