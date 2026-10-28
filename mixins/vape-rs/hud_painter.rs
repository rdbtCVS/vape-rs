use crate::game::vape_rs::ui::{
    HudPanel,
    gui::Rect,
    hud::{Command, biome_title},
};
use horizon_client::hud::HudBatch;

pub(super) fn biome(game: &crate::game::Game<'_>, position: [f64; 3]) -> String {
    let [x, y, z] = position.map(|v| v.floor() as i32);
    game.world
        .chunks
        .get(&(x >> 4, z >> 4))
        .map(|chunk| biome_title(&game.res.biomes().get(chunk.biome(x, y, z)).name))
        .unwrap_or_default()
}
fn color(argb: u32, enabled: bool) -> u32 {
    let alpha = if enabled {
        argb & 0xff000000
    } else {
        ((argb >> 24) / 3) << 24
    };
    alpha | (argb & 0x0000ff00) | ((argb & 0x00ff0000) >> 16) | ((argb & 0x000000ff) << 16)
}
pub(super) fn panel(game: &crate::game::Game<'_>, batch: &mut HudBatch, panel: &HudPanel) -> Rect {
    let atlas = &game.gui.atlas;
    let layout = panel.layout(|text| HudBatch::text_width(atlas, text));
    let w = layout.width * panel.scale;
    let h = layout.height * panel.scale;
    let x = panel.x.clamp(0.0, (batch.width - w).max(0.0));
    let y = panel.y.clamp(0.0, (batch.height - h).max(0.0));
    let start = batch.vertices.len();
    if panel.background {
        batch.fill(
            atlas,
            x,
            y,
            layout.width,
            layout.height,
            color(0x99000000, panel.enabled),
        );
    }
    for command in layout.commands {
        match command {
            Command::Text {
                x: tx,
                y: ty,
                scale,
                text,
                color: argb,
            } => {
                let first = batch.vertices.len();
                batch.text(
                    atlas,
                    &text,
                    x + tx,
                    y + ty,
                    color(argb, panel.enabled),
                    !panel.background,
                );
                batch.scale_since(first, x + tx, y + ty, scale, scale);
            }
            Command::Fill {
                x: tx,
                y: ty,
                width,
                height,
                color: argb,
            } => batch.fill(
                atlas,
                x + tx,
                y + ty,
                width,
                height,
                color(argb, panel.enabled),
            ),
            Command::Triangle {
                points,
                color: argb,
            } => {
                let points = points.map(|[px, py]| [x + px, y + py]);
                batch.fill_quad(
                    atlas,
                    [points[0], points[1], points[2], points[2]],
                    color(argb, panel.enabled),
                );
            }
        }
    }
    batch.scale_since(start, x, y, panel.scale, panel.scale);
    // Compass headings extend above the strip, as specified by its frame.
    if panel.module == "hud.compass" {
        batch.clip_since(start, x, y - 10.0 * panel.scale, x + w, y + h);
    }
    batch.clip_since(start, 0.0, 0.0, batch.width, batch.height);
    Rect(x, y, w, h)
}
