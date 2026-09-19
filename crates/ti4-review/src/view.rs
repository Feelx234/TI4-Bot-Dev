//! The presentation layer both reviewers draw from.
//!
//! R01's viewer and R02's replayer are meant to be indistinguishable wherever they show the same
//! frame, so the palette, the seat colours, the abbreviations, the section chrome and the board
//! glyphs live here once. Everything in this module is a pure function of the data handed to it: no
//! app state, no `LiveReview`, no clock, no filesystem. A re-tone is a change to this file and
//! nowhere else, and the snapshot tests pin the values the two apps promise each other.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Shape, Stroke, Vec2};
use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, FULL};
use ti4_model::id::PlayerId;
use ti4_model::units::Unit;

use crate::{ReviewFrame, ReviewSession};

pub const SEAT_COLORS: [Color32; 6] = [
    Color32::from_rgb(224, 66, 66),
    Color32::from_rgb(66, 142, 235),
    Color32::from_rgb(242, 198, 56),
    Color32::from_rgb(54, 184, 116),
    Color32::from_rgb(173, 103, 224),
    Color32::from_rgb(238, 126, 49),
];

pub const NEUTRAL_COLOR: Color32 = Color32::from_rgb(166, 174, 184);

/// Text on the light player panel. Colour marks a seat only as a swatch, never as the text itself.
pub const PANEL_TEXT: Color32 = Color32::BLACK;

pub const PANEL_FILL: Color32 = Color32::from_rgb(246, 247, 250);

pub fn seat_index(player: &PlayerId) -> Option<usize> {
    player
        .to_string()
        .strip_prefix("seat")
        .and_then(|value| value.parse::<usize>().ok())
        .map(|index| index % SEAT_COLORS.len())
}

pub fn player_color(player: &PlayerId) -> Color32 {
    seat_index(player).map_or(NEUTRAL_COLOR, |index| SEAT_COLORS[index])
}

pub fn short_trait(value: &str) -> &'static str {
    if value.eq_ignore_ascii_case("cultural") {
        "C"
    } else if value.eq_ignore_ascii_case("hazardous") {
        "H"
    } else if value.eq_ignore_ascii_case("industrial") {
        "I"
    } else {
        "·"
    }
}

pub fn short_specialty(value: &str) -> &'static str {
    let lower = value.to_ascii_lowercase();
    if lower.contains("biotic") || lower.contains("green") {
        "G"
    } else if lower.contains("cybernetic") || lower.contains("yellow") {
        "Y"
    } else if lower.contains("propulsion") || lower.contains("blue") {
        "B"
    } else if lower.contains("warfare") || lower.contains("red") {
        "R"
    } else {
        "T"
    }
}

/// A panel section under a heading-sized header the reader can fold away; open by default.
pub fn section<R>(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui) -> R) {
    section_with_id(ui, title, title, add_contents);
}

/// [`section`] whose fold state survives a changing title.
pub fn section_with_id<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) {
    egui::CollapsingHeader::new(egui::RichText::new(title).heading())
        .id_salt(("panel-section", id))
        .default_open(true)
        .show(ui, add_contents);
}

pub fn item_section(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    items: Vec<String>,
    color: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(icon);
        ui.strong(format!("{title} · {}", items.len()));
    });
    if items.is_empty() {
        ui.weak("None");
    } else {
        ui.horizontal_wrapped(|ui| {
            for item in items {
                ui.label(
                    egui::RichText::new(item)
                        .background_color(color.gamma_multiply(0.35))
                        .color(PANEL_TEXT),
                );
            }
        });
    }
}

/// A seat name in black, preceded by its colour swatch.
pub fn seat_label(ui: &mut egui::Ui, player: &PlayerId, text: impl Into<String>) {
    ui.horizontal(|ui| {
        ui.colored_label(player_color(player), "●");
        ui.label(egui::RichText::new(text.into()).color(PANEL_TEXT));
    });
}

pub fn stat_badge(ui: &mut egui::Ui, icon: &str, label: &str, value: impl std::fmt::Display) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.strong(icon);
            ui.label(format!("{label} {value}"));
        });
    });
}

pub fn content_label(
    content: &ContentStore,
    category: ContentType,
    id: impl std::fmt::Display,
) -> String {
    let id = id.to_string();
    let Some(record) = content.get(category, &id) else {
        return id;
    };
    let Some(name) = record
        .text("name")
        .or_else(|| record.text("shortName"))
        .or_else(|| record.text("title"))
        .filter(|name| !name.eq_ignore_ascii_case(&id))
    else {
        return id;
    };
    format!("{name} [{id}]")
}

pub fn unit_base(content: &ContentStore, unit: &Unit) -> String {
    ti4_content::units::unit_type(content, unit.type_id.as_str(), FULL).map_or_else(
        || unit.type_id.to_string(),
        |kind| kind.base_type().to_owned(),
    )
}

pub fn planets_for_tile(
    session: &ReviewSession,
    frame: &ReviewFrame,
    tile: &crate::BoardTile,
) -> Vec<crate::PlanetMeta> {
    let mut planets = tile.planets.clone();
    for (planet, system) in &frame.state.placed_planets {
        if system.as_str() != tile.system || planets.iter().any(|known| known.id == planet.as_str())
        {
            continue;
        }
        if let Some(meta) = session
            .planet_catalog
            .iter()
            .find(|candidate| candidate.id == planet.as_str())
        {
            planets.push(meta.clone());
        }
    }
    planets
}

pub fn polygon(center: Pos2, radius: f32, sides: usize, offset: f32) -> Vec<Pos2> {
    (0..sides)
        .map(|index| {
            let angle = offset + std::f32::consts::TAU * index as f32 / sides as f32;
            center + Vec2::angled(angle) * radius
        })
        .collect()
}

pub fn anomaly_style(kinds: &[String]) -> Option<(Color32, &'static str)> {
    if kinds.iter().any(|kind| kind == "entropic scar") {
        Some((Color32::from_rgb(48, 24, 64), "╳ SCAR"))
    } else if kinds.iter().any(|kind| kind == "supernova") {
        Some((Color32::from_rgb(105, 38, 20), "✹ NOVA"))
    } else if kinds.iter().any(|kind| kind == "gravity rift") {
        Some((Color32::from_rgb(50, 28, 82), "◉ RIFT"))
    } else if kinds.iter().any(|kind| kind == "nebula") {
        Some((Color32::from_rgb(55, 45, 91), "☁ NEBULA"))
    } else if kinds.iter().any(|kind| kind == "asteroid field") {
        Some((Color32::from_rgb(67, 61, 52), "✦ ASTEROIDS"))
    } else {
        None
    }
}

pub fn wormhole_style(kind: &str) -> (Color32, &str) {
    match kind.to_ascii_uppercase().as_str() {
        "ALPHA" => (Color32::from_rgb(225, 82, 82), "α"),
        "BETA" => (Color32::from_rgb(72, 190, 111), "β"),
        "GAMMA" => (Color32::from_rgb(230, 184, 68), "γ"),
        "DELTA" => (Color32::from_rgb(100, 154, 238), "δ"),
        _ => (Color32::LIGHT_GRAY, "?"),
    }
}

pub fn draw_wormhole(
    painter: &egui::Painter,
    center: Pos2,
    kind: &str,
    token: bool,
    suppressed: bool,
    scale: f32,
) {
    let (mut color, symbol) = wormhole_style(kind);
    if suppressed {
        color = color.gamma_multiply(0.35);
    }
    let radius = 7.2 * scale.max(0.75);
    if token {
        painter.circle_filled(center, radius + 2.2 * scale, Color32::from_rgb(13, 20, 30));
        painter.circle_stroke(
            center,
            radius + 2.2 * scale,
            Stroke::new(1.3 * scale, Color32::WHITE),
        );
    }
    painter.circle_stroke(center, radius, Stroke::new(2.1 * scale, color));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        symbol,
        FontId::proportional(10.0 * scale.max(0.8)),
        color,
    );
    if suppressed {
        painter.line_segment(
            [
                center + Vec2::new(-radius, radius),
                center + Vec2::new(radius, -radius),
            ],
            Stroke::new(1.8 * scale, Color32::LIGHT_RED),
        );
    }
}

pub fn draw_fracture_portal(painter: &egui::Painter, center: Pos2, ingress: bool, scale: f32) {
    let color = if ingress {
        Color32::from_rgb(71, 220, 225)
    } else {
        Color32::from_rgb(190, 105, 235)
    };
    let radius = 8.5 * scale.max(0.75);
    painter.circle_stroke(center, radius, Stroke::new(2.4 * scale, color));
    painter.circle_stroke(center, radius * 0.55, Stroke::new(1.3 * scale, color));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        if ingress { "IN" } else { "OUT" },
        FontId::monospace(5.2 * scale.max(0.9)),
        Color32::WHITE,
    );
}

pub fn draw_unit_symbol(
    painter: &egui::Painter,
    center: Pos2,
    base: &str,
    color: Color32,
    count: usize,
    damaged: bool,
    galvanized: bool,
    scale: f32,
) {
    let size = 5.5 * scale.max(0.75);
    let dark = Color32::from_rgb(12, 18, 27);
    let stroke = Stroke::new(1.1 * scale.max(0.8), dark);
    match base {
        "fighter" => {
            painter.add(Shape::convex_polygon(
                polygon(center, size, 3, -std::f32::consts::FRAC_PI_2),
                color,
                stroke,
            ));
        }
        "destroyer" => {
            painter.add(Shape::convex_polygon(
                polygon(center, size, 4, std::f32::consts::FRAC_PI_4),
                color,
                stroke,
            ));
        }
        "cruiser" => {
            painter.add(Shape::convex_polygon(
                polygon(center, size, 4, 0.0),
                color,
                stroke,
            ));
        }
        "carrier" => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-size * 1.3, -size * 0.6),
                    center + Vec2::new(size * 1.3, -size * 0.6),
                    center + Vec2::new(size, size * 0.6),
                    center + Vec2::new(-size, size * 0.6),
                ],
                color,
                stroke,
            ));
        }
        "dreadnought" => {
            painter.add(Shape::convex_polygon(
                polygon(center, size, 6, 0.0),
                color,
                stroke,
            ));
        }
        "flagship" => {
            painter.circle_filled(center, size, color);
            painter.circle_stroke(center, size, stroke);
            painter.line_segment(
                [
                    center - Vec2::splat(size * 0.7),
                    center + Vec2::splat(size * 0.7),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-size * 0.7, size * 0.7),
                    center + Vec2::new(size * 0.7, -size * 0.7),
                ],
                stroke,
            );
        }
        "war_sun" => {
            painter.circle_filled(center, size * 1.15, color);
            painter.circle_stroke(center, size * 1.15, Stroke::new(1.5, Color32::WHITE));
        }
        "infantry" => {
            painter.circle_filled(center, size * 0.75, color);
            painter.circle_stroke(center, size * 0.75, stroke);
        }
        "mech" => {
            painter.add(Shape::convex_polygon(
                polygon(center, size, 5, -std::f32::consts::FRAC_PI_2),
                color,
                stroke,
            ));
        }
        "pds" => {
            painter.rect_filled(
                egui::Rect::from_center_size(center, Vec2::splat(size * 1.4)),
                1.0,
                color,
            );
            painter.line_segment(
                [
                    center + Vec2::new(0.0, -size),
                    center + Vec2::new(0.0, size),
                ],
                stroke,
            );
        }
        "space_dock" => {
            painter.add(Shape::convex_polygon(
                vec![
                    center + Vec2::new(-size, size),
                    center + Vec2::new(size, size),
                    center + Vec2::new(size * 0.65, -size),
                    center + Vec2::new(-size * 0.65, -size),
                ],
                color,
                stroke,
            ));
        }
        _ => {
            painter.circle_filled(center, size, color);
            painter.circle_stroke(center, size, stroke);
        }
    }
    if damaged {
        painter.line_segment(
            [
                center + Vec2::new(-size, size),
                center + Vec2::new(size, -size),
            ],
            Stroke::new(1.6, Color32::RED),
        );
    }
    if galvanized {
        painter.circle_stroke(center, size * 1.45, Stroke::new(1.2, Color32::YELLOW));
    }
    let abbreviation: String = base.chars().take(2).collect();
    painter.text(
        center + Vec2::new(0.0, size + 4.0 * scale),
        Align2::CENTER_TOP,
        format!("{abbreviation}×{count}"),
        FontId::monospace(5.8 * scale.max(0.9)),
        Color32::WHITE,
    );
}
