// WTap (reference `WTap`). On an attack against a player (a vulnerable one
// with Select hits), with the configured chance, forward is released after
// the release delay and handed back after the re-press delay. Forward is
// held released through the movement writer; any open screen hands it back.
use super::super::super::core::settings::{Definition, Settings};
use super::super::rng::Rng;
use serde_json::Value;

pub const ID: &str = "combat.wtap";

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::number(&format!("{ID}.chance"), "Chance", 0.0, 90.0, 100.0, 1.0),
        Definition::number(
            &format!("{ID}.release-delay"),
            "Release delay",
            0.0,
            0.0,
            500.0,
            50.0,
        ),
        Definition::number(
            &format!("{ID}.re-press-delay"),
            "Re-press delay",
            0.0,
            0.0,
            500.0,
            50.0,
        ),
        Definition::boolean(&format!("{ID}.select-hits"), "Select hits", true),
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub chance: f64,
    pub release_delay: u64,
    pub re_press_delay: u64,
    pub select_hits: bool,
}

impl Options {
    pub fn read(settings: &Settings) -> Self {
        let number = |key: &str, default: f64| {
            settings
                .get(&format!("{ID}.{key}"))
                .and_then(Value::as_f64)
                .unwrap_or(default)
        };
        Self {
            chance: number("chance", 90.0),
            release_delay: number("release-delay", 0.0) as u64,
            re_press_delay: number("re-press-delay", 0.0) as u64,
            select_hits: settings
                .get(&format!("{ID}.select-hits"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    /// Waiting the release delay since this time.
    Release(u64),
    /// Forward released since this time; waiting the re-press delay.
    RePress(u64),
}

#[derive(Debug)]
pub struct WTap {
    phase: Phase,
    random: Rng,
}

impl Default for WTap {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            random: Rng::new(0x5bd1_e995_9e37_79b9),
        }
    }
}

impl WTap {
    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
    }
    /// `onPreAttack`: `player` when the target is a player; `hurt_time` the
    /// target's remaining hurt ticks.
    pub fn pre_attack(&mut self, options: &Options, now: u64, player: bool, hurt_time: i32) {
        if !player || self.phase != Phase::Idle {
            return;
        }
        if options.select_hits && hurt_time > 0 {
            return;
        }
        if options.chance >= self.random.double() * 100.0 {
            self.phase = Phase::Release(now);
            self.advance(options, now);
        }
    }
    fn advance(&mut self, options: &Options, now: u64) {
        if let Phase::Release(at) = self.phase
            && now.saturating_sub(at) >= options.release_delay
        {
            self.phase = Phase::RePress(now);
        }
    }
    /// `onTick`: whether forward is held released this tick. `screen` resets
    /// to the user's key.
    pub fn tick(&mut self, options: &Options, now: u64, screen: bool) -> bool {
        if screen {
            self.phase = Phase::Idle;
            return false;
        }
        match self.phase {
            Phase::Idle => false,
            Phase::Release(_) => {
                self.advance(options, now);
                matches!(self.phase, Phase::RePress(_))
            }
            Phase::RePress(at) => {
                if now.saturating_sub(at) >= options.re_press_delay {
                    self.phase = Phase::Idle;
                    false
                } else {
                    true
                }
            }
        }
    }
}
