// Arrows (`Arrows`): a marker on an ellipse around the screen centre
// pointing at each player that is off screen or behind you, optionally
// with its distance and fading with distance. Bots, teammates and friends
// are skipped. The reference's `exo` arrow image is a Vape asset; the
// marker here is a filled triangle of the same footprint.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::targeting::entities::{EntityClass, EntityView};
use super::super::super::targeting::filter::People;
use super::overlay::{Align, Draw, Frame};
use serde_json::{Value, json};

pub const ARROWS: &str = "render.arrows";

fn key(k: &str) -> String {
    format!("{ARROWS}.{k}")
}

pub fn definitions() -> Vec<Definition> {
    vec![
        Definition::new(&key("color"), "Color", Kind::Color, json!(0xffff_0000_u32)),
        Definition::number(&key("radius-scale"), "Radius Scale", 0.0, 0.5, 1.0, 0.05),
        Definition::boolean(&key("show-distance"), "Show Distance", false),
        Definition::boolean(&key("scale-opacity"), "Scale Opacity", false),
    ]
}

fn tracked(e: &EntityView, people: &impl People) -> bool {
    e.class == EntityClass::Player
        && e.health > 0.0
        && !people.bot(e)
        && !people.teammate(e)
        && !people.friend(&e.name)
}

pub fn draws(settings: &Settings, frame: &Frame, people: &impl People) -> Vec<Draw> {
    let boolean = |k: &str| {
        settings
            .get(&key(k))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let color = settings
        .get(&key("color"))
        .and_then(Value::as_u64)
        .map_or(0xffff_0000, |c| c as u32);
    let scale = settings
        .get(&key("radius-scale"))
        .and_then(Value::as_f64)
        .unwrap_or(0.5) as f32;
    let p = &frame.projection;
    let (cx, cy) = (p.width / 2.0, p.height / 2.0);
    let (rx, ry) = (cx * scale, cy * scale);
    let mut out = Vec::new();
    for living in &frame.entities {
        let e = &living.view;
        if !tracked(e, people) {
            continue;
        }
        let on_screen = p
            .project(e.position)
            .is_some_and(|[x, y]| x > 0.0 && y > 0.0 && x < p.width && y < p.height);
        if on_screen {
            continue;
        }
        let Some([dx, dy]) = p.direction(e.position) else {
            continue;
        };
        // The ellipse point along the direction.
        let r = rx * ry / ((ry * dx).hypot(rx * dy)).max(1e-6);
        let (ax, ay) = (cx + dx * r, cy + dy * r);
        let distance = (0..3)
            .map(|i| (e.position[i] - frame.own.position[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        let fade = ((distance * 1.5) as u32).min(255);
        let alpha = if boolean("scale-opacity") {
            255 - fade
        } else {
            color >> 24
        };
        let tint = (color & 0x00ff_ffff) | (alpha << 24);
        // Tip 6 out from the point, base 2 in, 4.5 to each side.
        let (nx, ny) = (-dy, dx);
        let tip = [ax + dx * 6.0, ay + dy * 6.0];
        let base = [ax - dx * 2.0, ay - dy * 2.0];
        let left = [base[0] + nx * 4.5, base[1] + ny * 4.5];
        let right = [base[0] - nx * 4.5, base[1] - ny * 4.5];
        out.push(Draw::Quad {
            corners: [tip, tip, right, left],
            color: tint,
        });
        if boolean("show-distance") && fade < 255 {
            let text_alpha = if boolean("scale-opacity") {
                255 - fade
            } else {
                255
            };
            out.push(Draw::Text {
                x: ax,
                y: ay - 2.0,
                scale: 0.5,
                text: format!("{}m", distance as i64),
                color: 0x00ff_ffff | (text_alpha << 24),
                shadow: true,
                align: Align::Center,
                backdrop: None,
            });
        }
    }
    out
}
