// The reference's GAME-group legit modules (`HudModuleGroup.GAME`) that have
// no runtime state of their own: Time Changer and Weather.
use super::super::core::settings::{Definition, Settings};
use serde_json::Value;

pub const TIME_CHANGER: &str = "legit.time-changer";
pub const WEATHER: &str = "legit.weather";

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::number(
            &format!("{TIME_CHANGER}.time"),
            "Time",
            0.0,
            12.0,
            24.0,
            1.0,
        ),
        Definition::choice(
            &format!("{WEATHER}.weather"),
            "Weather",
            &["Clear", "Raining"],
            "Clear",
        ),
    ]
}

/// `TimeChangerHudModule.onTick`: the hour setting as a time of day in
/// ticks (hour 6 is tick 0).
pub fn time_of_day(settings: &Settings) -> i64 {
    let mut hours = settings
        .get(&format!("{TIME_CHANGER}.time"))
        .and_then(Value::as_f64)
        .unwrap_or(12.0)
        - 6.0;
    if hours < 0.0 {
        hours += 24.0;
    }
    (hours * 1000.0).round() as i64
}

/// `WeatherChangerHudModule.onPreRenderTick`: the rain and thunder strength
/// written each frame (Clear: 0/0; Raining: rain 1, thunder untouched).
pub fn weather(settings: &Settings) -> (f32, Option<f32>) {
    match settings
        .get(&format!("{WEATHER}.weather"))
        .and_then(Value::as_str)
    {
        Some("Raining") => (1.0, None),
        _ => (0.0, Some(0.0)),
    }
}
