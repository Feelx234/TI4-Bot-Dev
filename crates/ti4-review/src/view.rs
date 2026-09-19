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

use ti4_model::id::{PlanetId, SystemId};

use crate::{PlanetMeta, ReviewFrame, ReviewSession};

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

/// Why a tile is the colour it is, in the order the rules are consulted.
///
/// The order matters more than the colours: a purged system keeps its fill even when it is selected,
/// and an anomaly beats the selection tint, because what the reader has to see first is what the
/// rules of the board forbid there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileFill {
    /// Nothing lives here any more.
    Purged,
    /// The Fracture's own special area.
    Fracture,
    /// The Nexus, drawn outside the ordinary geometry.
    Nexus,
    /// An ordinary system the hyperlane network crosses.
    Hyperlane,
    /// An anomaly tints the tile; its label comes from [`anomaly_style`].
    Anomaly,
    /// The tile the reader selected, on an otherwise ordinary system.
    Selected,
    /// Deep space.
    Plain,
}

/// The board's fill rule, consulted in the order the variants are named.
#[must_use]
pub fn tile_fill(
    purged: bool,
    special_area: Option<&str>,
    hyperlane: bool,
    anomalies: &[String],
    selected: bool,
) -> (TileFill, Color32) {
    if purged {
        (TileFill::Purged, Color32::from_rgb(24, 24, 28))
    } else if special_area == Some("fracture") {
        (TileFill::Fracture, Color32::from_rgb(39, 25, 57))
    } else if special_area == Some("nexus") {
        (TileFill::Nexus, Color32::from_rgb(25, 48, 61))
    } else if hyperlane {
        (TileFill::Hyperlane, Color32::from_rgb(55, 39, 91))
    } else if let Some((color, _)) = anomaly_style(anomalies) {
        (TileFill::Anomaly, color)
    } else if selected {
        (TileFill::Selected, Color32::from_rgb(48, 91, 116))
    } else {
        (TileFill::Plain, Color32::from_rgb(23, 44, 69))
    }
}

/// The six corners of a tile, in the orientation the board and its control rings share.
#[must_use]
pub fn hex_corners(point: Pos2, radius: f32) -> Vec<Pos2> {
    (0..6)
        .map(|corner| {
            let angle = std::f32::consts::FRAC_PI_6 + std::f32::consts::TAU * corner as f32 / 6.0;
            point + Vec2::new(radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

/// Where the centre of a planet sits inside its tile, relative to the tile centre.
///
/// One planet is centred, two move apart, and three or more spread on a tighter pitch and lift
/// slightly so their labels clear the bottom edge.
#[allow(clippy::cast_precision_loss)] // planet counts per tile are tiny; this is presentation maths
#[must_use]
pub fn planet_offset(index: usize, count: usize) -> (f32, f32) {
    let offset_x = match count {
        // A tile with no planets is never drawn, and answers as though it had one centred planet
        // rather than underflowing the count.
        0 | 1 => 0.0,
        2 => (index as f32 * 2.0 - 1.0) * 22.0,
        _ => (index as f32 - (count - 1) as f32 / 2.0) * 19.0,
    };
    (offset_x, if count > 2 { 24.0 } else { 27.0 })
}

/// Units the board draws as one glyph: everything the reader can tell apart at a glance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitStack {
    pub owner: PlayerId,
    pub base: String,
    pub damaged: bool,
    pub galvanized: bool,
    pub count: usize,
}

/// Group units the way the board wants them: by owner, base type, and the two flags it draws.
///
/// The grouping is keyed by the same tuple it is built from, so a stack does not jump between frames
/// because a unit arrived in a different order.
#[must_use]
pub fn unit_stacks(content: &ContentStore, units: &[Unit]) -> Vec<UnitStack> {
    let mut groups: std::collections::BTreeMap<(PlayerId, String, bool, bool), usize> =
        std::collections::BTreeMap::new();
    for unit in units {
        *groups
            .entry((
                unit.owner.clone(),
                unit_base(content, unit),
                unit.sustained_damage,
                unit.galvanized,
            ))
            .or_default() += 1;
    }
    groups
        .into_iter()
        .map(|((owner, base, damaged, galvanized), count)| UnitStack {
            owner,
            base,
            damaged,
            galvanized,
            count,
        })
        .collect()
}

/// A wormhole on a tile, in the order the board draws them, with suppression already resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WormholeView {
    pub kind: String,
    /// Whether this is a placed token (white outer rim) or a printed wormhole.
    pub token: bool,
    /// Suppressed wormholes keep their glyph and gain a red slash.
    pub suppressed: bool,
}

/// A planet as the board draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanetView {
    pub meta: PlanetMeta,
    pub owner: Option<PlayerId>,
    pub purged: bool,
    pub color: Color32,
    /// `I·B` style summary of the traits and the tech specialty, for the line under the disc.
    pub trait_label: String,
    /// The mark before the name: destroyed, a station, legendary, or nothing.
    pub badge: &'static str,
    /// Other players sharing the planet under a coexistence marker.
    pub coexisting: Vec<PlayerId>,
    pub attachments: usize,
    pub ground: Vec<UnitStack>,
}

/// One visible tile at one frame, with every decision the renderer makes already made.
///
/// Geometry is deliberately absent: positions depend on the space the application was given, while
/// everything here depends only on the frame. That is what lets the reviewer and the replayer agree
/// about a board while drawing it at different sizes.
// Four independent facts about a tile, each drawing its own stroke; collapsing them into one enum
// would lose combinations that occur, such as a selected system that is also an ingress portal.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq)]
pub struct TileView {
    pub system: String,
    pub label: String,
    pub q: i32,
    pub r: i32,
    pub special_area: Option<String>,
    pub fill: TileFill,
    pub color: Color32,
    pub anomaly_label: Option<&'static str>,
    /// The single player controlling space, or `None` for empty or contested.
    pub space_owner: Option<PlayerId>,
    /// Sorted, deduplicated controllers of the tile's planets. One draws a ring, several a split.
    pub planet_owners: Vec<PlayerId>,
    pub selected: bool,
    /// Whether the selected system's portal links to this one.
    pub portal_linked: bool,
    pub ingress: bool,
    pub egress: bool,
    pub wormholes: Vec<WormholeView>,
    pub units: Vec<UnitStack>,
    pub command_tokens: Vec<PlayerId>,
    pub token_labels: Vec<String>,
    pub planets: Vec<PlanetView>,
}

/// The board as one frame shows it: visible tiles only, in the order the session lists them.
#[must_use]
pub fn board_view(
    content: &ContentStore,
    session: &ReviewSession,
    frame: &ReviewFrame,
    selected: Option<&str>,
) -> Vec<TileView> {
    let state = &frame.state;
    let fracture_visible = state.fracture_in_play;
    let selected_is_ingress =
        selected.is_some_and(|system| state.ingress_tokens.contains(&SystemId::new(system)));
    let selected_is_egress = selected.is_some_and(|system| {
        session
            .board
            .iter()
            .any(|tile| tile.system == system && tile.egress)
    });
    let alpha_beta_suppressed = ti4_engine::laws::wormholes_suppressed(state);

    let mut tiles = Vec::new();
    for tile in &session.board {
        if tile.special_area.as_deref() == Some("fracture") && !fracture_visible {
            continue;
        }
        if tile.special_area.as_deref() == Some("nexus")
            && !state.board.contains_key(&SystemId::new(&tile.system))
        {
            continue;
        }
        let system_id = SystemId::new(&tile.system);
        let system_purged = state.purged_systems.contains(&system_id);
        let is_selected = selected == Some(tile.system.as_str());
        let (fill, color) = tile_fill(
            system_purged,
            tile.special_area.as_deref(),
            tile.hyperlane,
            &tile.anomalies,
            is_selected,
        );
        let nexus_suppressed = tile.special_area.as_deref() == Some("nexus")
            && ti4_engine::laws::nexus_wormholes_suppressed(state);
        let system_state = state.board.get(&system_id);
        let tile_planets = planets_for_tile(session, frame, tile);

        let mut space_owners: Vec<&PlayerId> = system_state
            .into_iter()
            .flat_map(|system| system.units.iter())
            .filter(|unit| {
                ti4_content::units::unit_type(content, unit.type_id.as_str(), FULL)
                    .is_some_and(|kind| kind.is_ship())
            })
            .map(|unit| &unit.owner)
            .collect();
        space_owners.sort();
        space_owners.dedup();

        let mut planet_owners: Vec<&PlayerId> = system_state
            .into_iter()
            .flat_map(|system| {
                tile_planets.iter().filter_map(|planet| {
                    let planet_id = PlanetId::new(&planet.id);
                    (!system.purged_planets.contains(&planet_id))
                        .then(|| system.planet_control.get(&planet_id))
                        .flatten()
                })
            })
            .collect();
        planet_owners.sort();
        planet_owners.dedup();

        let mut wormholes: Vec<WormholeView> = Vec::new();
        for kind in &tile.wormholes {
            let suppressed = matches!(kind.as_str(), "ALPHA" | "BETA")
                && (alpha_beta_suppressed || nexus_suppressed);
            wormholes.push(WormholeView {
                kind: kind.clone(),
                token: false,
                suppressed,
            });
        }
        for (kind, system) in &state.wormhole_tokens {
            if system != &system_id {
                continue;
            }
            let suppressed = matches!(kind.as_str(), "ALPHA" | "BETA") && alpha_beta_suppressed;
            wormholes.push(WormholeView {
                kind: kind.clone(),
                token: true,
                suppressed,
            });
        }
        if let Some((system, face)) = &state.ion_storm
            && system == &system_id
        {
            let suppressed = matches!(face.as_str(), "ALPHA" | "BETA") && alpha_beta_suppressed;
            wormholes.push(WormholeView {
                kind: face.clone(),
                token: true,
                suppressed,
            });
        }

        let units =
            system_state.map_or_else(Vec::new, |system| unit_stacks(content, &system.units));
        let command_tokens = system_state.map_or_else(Vec::new, |system| {
            system.command_tokens.iter().cloned().collect()
        });

        let mut token_labels = Vec::new();
        if state.frontier_tokens.contains(&system_id) {
            token_labels.push("Frontier".to_owned());
        }
        if state.breach_tokens.contains(&system_id) {
            token_labels.push("Breach".to_owned());
        }
        if state.thunders_edge_system.as_ref() == Some(&system_id) {
            token_labels.push("Thunder's Edge".to_owned());
        }

        let planets = tile_planets
            .iter()
            .map(|planet| {
                let planet_id = PlanetId::new(&planet.id);
                let owner =
                    system_state.and_then(|system| system.planet_control.get(&planet_id).cloned());
                let purged =
                    system_state.is_some_and(|system| system.purged_planets.contains(&planet_id));
                let color = if purged {
                    Color32::from_rgb(38, 38, 42)
                } else {
                    owner
                        .as_ref()
                        .map_or(Color32::from_rgb(91, 96, 105), player_color)
                };
                let trait_label = planet
                    .traits
                    .iter()
                    .map(|value| short_trait(value))
                    .collect::<Vec<_>>()
                    .join("");
                let tech_label = planet
                    .tech_specialties
                    .iter()
                    .map(|value| short_specialty(value))
                    .collect::<Vec<_>>()
                    .join("");
                let trait_label = if trait_label.is_empty() || tech_label.is_empty() {
                    trait_label
                } else {
                    format!("{trait_label}·{tech_label}")
                };
                let ground = system_state
                    .and_then(|system| system.planet_units.get(&planet_id))
                    .map_or_else(Vec::new, |units| unit_stacks(content, units));
                PlanetView {
                    meta: planet.clone(),
                    owner,
                    purged,
                    color,
                    trait_label,
                    badge: if purged {
                        "× "
                    } else if planet.space_station {
                        "S "
                    } else if planet.legendary {
                        "★"
                    } else {
                        ""
                    },
                    coexisting: system_state
                        .and_then(|system| system.coexisting.get(&planet_id))
                        .map_or_else(Vec::new, |set| set.iter().cloned().collect()),
                    attachments: state
                        .planet_attachments
                        .get(&planet_id)
                        .filter(|attachments| !attachments.is_empty())
                        .map_or(0, Vec::len),
                    ground,
                }
            })
            .collect();

        tiles.push(TileView {
            system: tile.system.clone(),
            label: if system_purged {
                format!("{} · PURGED", tile.label)
            } else {
                tile.label.clone()
            },
            q: tile.q,
            r: tile.r,
            special_area: tile.special_area.clone(),
            fill,
            color,
            anomaly_label: anomaly_style(&tile.anomalies).map(|(_, label)| label),
            // Only a single controller paints the thick ring; a contested system paints none.
            space_owner: match space_owners.as_slice() {
                [owner] => Some((*owner).clone()),
                _ => None,
            },
            planet_owners: planet_owners.into_iter().cloned().collect(),
            selected: is_selected,
            portal_linked: (selected_is_ingress && tile.egress)
                || (selected_is_egress && state.ingress_tokens.contains(&system_id)),
            ingress: state.ingress_tokens.contains(&system_id),
            egress: tile.egress,
            wormholes,
            units,
            command_tokens,
            token_labels,
            planets,
        });
    }
    tiles
}
