use super::{Game, NameTag};
use crate::game::vape_rs::core::with_client;
use crate::game::vape_rs::inventory::item_limit::HeldItem;
use crate::game::vape_rs::modules::render::{item_esp::Dropped, name_tags::Subject};
use crate::game::vape_rs::targeting::entities::EntityClass;
use core::ops::ControlFlow;
use horizon_client::{block_entity_render::vertex_light, mesher::Vertex, text::Text};

/// One tag to draw through walls: where it hangs, its lines bottom first
/// (text, lift), its scale relative to the host's and its opacity.
struct Extra {
    at: glam::DVec3,
    lines: Vec<(Text, f64)>,
    scale: f64,
    alpha: f64,
}

/// A host line spacing (`9 * 1.15` pixels at the host's 0.025 scale).
const LINE: f64 = 9.0 * 1.15 * 0.025;

fn text(legacy: &str, color: u32) -> Text {
    let mut text = Text::legacy(legacy);
    for span in &mut text.spans {
        if span.style.color.is_none() {
            span.style.color = Some(color & 0xff_ffff);
        }
    }
    text
}

fn entity_name(id: &str) -> String {
    let path = id.trim_start_matches("minecraft:");
    let title = path
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ");
    horizon_client::lang::get_or(&format!("entity.minecraft.{path}"), &title)
}

fn extras(game: &Game<'_>, alpha: f64, tags_on: bool, items_on: bool) -> Vec<Extra> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let mut out = Vec::new();
    let Some(player) = game.player.as_ref() else {
        return out;
    };
    let feet = glam::DVec3::new(player.position.x, player.position.y, player.position.z);
    let lerp = |a: horizon_core::geometry::Vec3, b: horizon_core::geometry::Vec3| {
        glam::DVec3::new(a.x + (b.x - a.x) * alpha, a.y + (b.y - a.y) * alpha, a.z + (b.z - a.z) * alpha)
    };
    if tags_on {
        let score = |item: Option<&str>, armor: bool| crate::game::vape_combat::item_score(game, item, armor);
        let held = game.inventory.get(game.selected).and_then(Option::as_ref).map(|s| &*s.item);
        let own_threat = score(held, false)
            + game.armor.iter().map(|a| score(a.as_ref().map(|s| &*s.item), true)).sum::<f64>();
        let world = game.mob_world_base(&[]);
        let eye = horizon_core::geometry::Vec3::new(feet.x, feet.y + f64::from(player.dimensions.eye_height), feet.z);
        let players = game.other_players();
        for m in game.mobs.iter().filter(|m| m.alive()) {
            let name = players.iter().find(|(p, _)| p.id == m.id).map(|(_, n)| n.clone());
            let class = if name.is_some() {
                EntityClass::Player
            } else {
                EntityClass::classify(&m.kind.id, &m.kind.category)
            };
            if class == EntityClass::Other || (class == EntityClass::Player && name.is_none()) {
                continue;
            }
            let at = glam::DVec3::new(m.position.x, m.position.y, m.position.z);
            let e = &m.extra;
            let worn = [e.head.as_deref(), e.chest.as_deref(), e.legs.as_deref(), e.feet.as_deref()];
            let head = horizon_core::geometry::Vec3::new(at.x, at.y + f64::from(m.kind.eye_height), at.z);
            let subject = Subject {
                class,
                name: name.clone().unwrap_or_else(|| m.custom_name.clone().unwrap_or_else(|| entity_name(&m.kind.id))),
                distance: feet.distance(at),
                health: m.health,
                max_health: horizon_client::mobs::finalize::max_health(m),
                absorption: 0.0,
                invisible: m.invisible || m.has_effect("minecraft:invisibility"),
                visible: world.line_of_sight(eye, head),
                bot: false,
                creative: false,
                threat: score(e.main_hand.as_deref(), false) + worn.iter().map(|w| score(*w, true)).sum::<f64>(),
            };
            let Some(tag) = with_client(&client, |runtime| runtime.name_tag(&subject, own_threat)) else {
                continue;
            };
            let height = if class == EntityClass::Player && e.crouching { 1.5 } else { f64::from(m.height()) };
            out.push(Extra {
                at: lerp(m.previous, m.position) + glam::DVec3::new(0.0, height + 0.4, 0.0),
                lines: vec![(text(&tag.text, tag.color), 0.0)],
                scale: tag.scale,
                alpha: tag.alpha,
            });
        }
    }
    if items_on {
        let items: Vec<Dropped> = game
            .dropped
            .iter()
            .take(512)
            .map(|d| {
                let info = game.res.items().get(&d.stack.item);
                let position = lerp(d.previous, d.position);
                Dropped {
                    item: HeldItem {
                        id: Some(d.stack.item.trim_start_matches("minecraft:").to_owned()),
                        display_name: horizon_client::stations::hover_name(&|id| game.pack.names.display_item(id), &d.stack),
                        block: info.is_some_and(|i| i.block.is_some()),
                        food: info.is_some_and(|i| i.food.is_some()),
                        potion: d.stack.item.ends_with("potion"),
                        slot: 0,
                    },
                    count: d.stack.count,
                    max_stack: info.map_or(64, |i| i.max_stack),
                    position: position.to_array(),
                    distance: feet.distance(glam::DVec3::new(d.position.x, d.position.y, d.position.z)),
                }
            })
            .collect();
        for label in with_client(&client, |runtime| runtime.item_labels(&items)) {
            let n = label.lines.len();
            out.push(Extra {
                at: glam::DVec3::from_array(label.at),
                lines: label.lines.iter().enumerate().map(|(i, l)| (text(l, 0xff_ffff), (n - 1 - i) as f64 * LINE)).collect(),
                scale: label.scale,
                alpha: 1.0,
            });
        }
    }
    out
}

/// `name_tag_geometry` HEAD: NameTags' and ItemESP's tags over everything,
/// and, while NameTags renders players, the host's other tags without the
/// players' (the rest of the host body, unchanged).
pub(crate) fn geometry(
    game: &mut Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<Vertex>,
    see_through: &mut Vec<Vertex>,
) -> ControlFlow<()> {
    let client = game.__horizon_mixins_vape_rs_client().clone();
    let (tags_on, items_on, replace) = with_client(&client, |runtime| {
        let (t, i) = runtime.world_tag_sources();
        (t, i, runtime.name_tags_replace_players())
    });
    if !tags_on && !items_on {
        return ControlFlow::Continue(());
    }
    let mut patches = Vec::new();
    for extra in extras(game, alpha, tags_on, items_on) {
        let scale = extra.scale.max(0.01);
        let start = see_through.len();
        let text_alpha = ((255.0 * extra.alpha).round() as u32).clamp(4, 255);
        patches.extend(game.name_tag_quads((extra.at - camera) / scale, &extra.lines, text_alpha, true, 2, 0, see_through));
        for v in &mut see_through[start..] {
            v.pos = (glam::Vec3::from_array(v.pos) * scale as f32).to_array();
        }
    }
    if !replace {
        game.map_patches.extend(patches);
        return ControlFlow::Continue(());
    }
    // The host body with other players' tags left out.
    let hidden: Vec<glam::DVec2> = game
        .other_players()
        .iter()
        .map(|(m, _)| {
            glam::DVec2::new(
                m.previous.x + (m.position.x - m.previous.x) * alpha,
                m.previous.z + (m.position.z - m.previous.z) * alpha,
            )
        })
        .collect();
    const FADED: u32 = 0x7F;
    let tags: Vec<NameTag> = game.name_tags(camera, alpha);
    for tag in tags {
        if hidden.iter().any(|p| (p.x - tag.at.x).abs() < 1e-6 && (p.y - tag.at.z).abs() < 1e-6) {
            continue;
        }
        let o = tag.at - camera;
        if tag.discrete {
            game.mark_translucent_entity(out.len(), "minecraft:name_tag");
            let light = vertex_light(tag.light);
            patches.extend(game.name_tag_quads(o, &tag.lines, FADED, true, 0, light, out));
            continue;
        }
        let lit = (tag.light >> 4).max(2) << 4 | (tag.light & 15).max(2);
        game.mark_entity(out.len(), "minecraft:name_tag");
        let light = vertex_light(lit);
        patches.extend(game.name_tag_quads(o, &tag.lines, 0xFF, false, 0, light, out));
        patches.extend(game.name_tag_quads(o, &tag.lines, FADED, true, 2, 0, see_through));
    }
    game.map_patches.extend(patches);
    ControlFlow::Break(())
}
