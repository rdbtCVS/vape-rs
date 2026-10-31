// The reference's blatant modules (`gg.vape.module.blatant`). HitBoxes,
// Timer, Fly and NoFall are Category.OTHER (listed only on an exact search);
// NoSlowdown is Category.HIDDEN.
pub mod anti_fall;
pub mod backtrack;
pub mod fly;
pub mod long_jump;
pub mod no_fall;
pub mod no_slowdown;
pub mod outbound;
pub mod speed;

use super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const HITBOXES: &str = "blatant.hitboxes";
pub const TIMER: &str = "blatant.timer";
/// Step (reference `Step`): no settings; see `Runtime::step_active`.
pub const STEP: &str = "blatant.step";

pub fn definitions() -> Vec<Definition> {
    let mut out = vec![
        Definition::number(
            &format!("{HITBOXES}.expand"),
            "Expand amount",
            0.0,
            0.35,
            1.0,
            0.01,
        ),
        Definition::number(&format!("{TIMER}.speed"), "Speed", 0.1, 1.07, 2.0, 0.01),
    ];
    out.extend(fly::definitions());
    out.extend(no_fall::definitions());
    out.extend(no_slowdown::definitions());
    out.extend(speed::definitions());
    out.extend(long_jump::definitions());
    out.extend(anti_fall::definitions());
    out
}

fn number(settings: &Settings, id: &str, default: f64) -> f64 {
    settings.get(id).and_then(Value::as_f64).unwrap_or(default)
}

/// HitBoxes: added to every entity's pick border.
pub fn hitbox_expansion(settings: &Settings) -> f64 {
    number(settings, &format!("{HITBOXES}.expand"), 0.35)
}

/// Timer: the game timer speed while enabled (`setTimerSpeed`).
pub fn timer_speed(settings: &Settings) -> f32 {
    number(settings, &format!("{TIMER}.speed"), 1.07) as f32
}
