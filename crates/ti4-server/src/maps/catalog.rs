//! What the host can choose, and what each choice looks like: the buildable template list, a
//! pictured preview of any choice, and the public description of a lobby's current choice.
//!
//! Previews are built with the very functions a started game uses
//! ([`crate::map::create_game_with_template`] and `create_game_with_map`), so a lobby's preview
//! is the board its game gets. No seed ever leaves this module.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_engine::seating;
use ti4_model::content_types::POK;
use ti4_model::id::PlayerId;

use super::template_loader::{MapTemplateSummary, TemplateLoader};
use crate::protocol::view::BoardTileView;

/// The host's choice of map for a lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MapChoice {
    /// A predefined layout; open slots are filled from the lobby's seeded filler.
    Template { alias: String },
    /// The seeded random board.
    Random,
}

impl MapChoice {
    /// The choice a persisted lobby's optional template alias stands for; `None` is the random board.
    #[must_use]
    pub fn from_stored(template: Option<&str>) -> Self {
        template.map_or(Self::Random, |alias| Self::Template {
            alias: alias.to_owned(),
        })
    }

    /// The optional alias a lobby record stores for this choice.
    #[must_use]
    pub fn stored(&self) -> Option<String> {
        match self {
            Self::Template { alias } => Some(alias.clone()),
            Self::Random => None,
        }
    }
}

/// Public description of a lobby's map choice. Deliberately has no seed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapChoiceView {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub systems: usize,
    pub hyperlanes: bool,
    pub recommended: bool,
}

/// One seat on the previewed map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeatPreview {
    /// 1-based seat number, the lobby slot's position.
    pub seat: usize,
    pub faction: String,
    pub faction_name: String,
    pub home_system_id: String,
}

/// A pictured map: the tiles a game with this choice has, and where each seat sits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapPreview {
    pub choice: MapChoice,
    pub player_count: usize,
    pub tiles: Vec<BoardTileView>,
    pub seats: Vec<SeatPreview>,
}

/// Seat order stand-ins; the map depends on seat order only, never on who sits where.
fn placeholder_players(count: usize) -> Vec<PlayerId> {
    (1..=count)
        .map(|i| PlayerId::new(format!("p{i}")))
        .collect()
}

/// The map a game of `player_count` seated in slot order gets for `choice` and `seed`.
///
/// # Errors
/// A message when the template is unknown, seats another number of players, or does not build.
pub fn preview(choice: &MapChoice, player_count: usize, seed: u64) -> Result<MapPreview, String> {
    let content = ContentStore::embedded();
    let players = placeholder_players(player_count);
    let (_, galaxy) = crate::map::create_game_with_template(
        content,
        &players,
        seed,
        match choice {
            MapChoice::Template { alias } => Some(alias.as_str()),
            MapChoice::Random => None,
        },
    )?;
    let tiles = crate::map::build_board_tiles(content, &galaxy);
    let seats = seating::IN_SCOPE_FACTIONS
        .iter()
        .cycle()
        .take(player_count)
        .zip(super::placeholder_homes(content, player_count))
        .enumerate()
        .map(|(index, (faction, home))| SeatPreview {
            seat: index + 1,
            faction: (*faction).to_owned(),
            faction_name: ti4_content::factions::get(content, faction)
                .and_then(|f| f.name().map(str::to_owned))
                .unwrap_or_else(|| (*faction).to_owned()),
            home_system_id: home.as_str().to_owned(),
        })
        .collect();
    Ok(MapPreview {
        choice: choice.clone(),
        player_count,
        tiles,
        seats,
    })
}

/// Checks a choice for a lobby of `player_count` seats by building it once.
///
/// # Errors
/// The same message [`preview`] reports.
pub fn validate(choice: &MapChoice, player_count: usize) -> Result<(), String> {
    preview(choice, player_count, 0).map(|_| ())
}

/// A repeatable seed for the picture of variant `variant` of a card. Mixed with a per-process
/// secret so the numbers cannot be walked to learn anything about a real game's seed.
#[must_use]
pub fn variant_seed(choice: &MapChoice, player_count: usize, variant: u32) -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    let salt = *SALT.get_or_init(rand::random);
    let mut x = salt ^ u64::from(variant).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= player_count as u64;
    if let MapChoice::Template { alias } = choice {
        for b in alias.bytes() {
            x = (x ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
    }
    // splitmix64 finaliser
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

type ShapeKey = (Option<String>, usize);

/// Cached `(systems, hyperlanes)` of a template or random board at a size, or `None` if it does
/// not build. The shape does not depend on the seed.
fn shape(choice: &MapChoice, player_count: usize) -> Option<(usize, bool)> {
    static CACHE: OnceLock<Mutex<BTreeMap<ShapeKey, Option<(usize, bool)>>>> = OnceLock::new();
    let key = (choice.stored(), player_count);
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(hit) = cache.lock().expect("shape cache").get(&key) {
        return *hit;
    }
    let computed = preview(choice, player_count, 0).ok().map(|p| {
        let main = p.tiles.iter().filter(|t| t.special_area.is_none());
        let systems = main.clone().filter(|t| !t.hyperlane).count();
        (systems, main.clone().any(|t| t.hyperlane))
    });
    cache.lock().expect("shape cache").insert(key, computed);
    computed
}

/// The template a lobby of `player_count` seats starts with when nobody chooses (cached).
#[must_use]
pub fn recommended_for(player_count: usize) -> Option<String> {
    static CACHE: OnceLock<Mutex<BTreeMap<usize, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(hit) = cache.lock().expect("default cache").get(&player_count) {
        return hit.clone();
    }
    let found = TemplateLoader::load().ok().and_then(|loader| {
        super::default_template_for(ContentStore::embedded(), &loader, player_count, POK)
    });
    cache
        .lock()
        .expect("default cache")
        .insert(player_count, found.clone());
    found
}

/// The templates for `player_count` that build, or every template (with `buildable` telling
/// them apart) when no count is given. The recommended one is flagged.
///
/// # Errors
/// A message when the template data cannot be loaded.
pub fn catalog(player_count: Option<usize>) -> Result<Vec<MapTemplateSummary>, String> {
    let loader = TemplateLoader::load()?;
    let mut out = Vec::new();
    for mut summary in loader.list_summaries() {
        if player_count.is_some_and(|n| n != summary.player_count) {
            continue;
        }
        let choice = MapChoice::Template {
            alias: summary.alias.clone(),
        };
        if let Some((systems, hyperlanes)) = shape(&choice, summary.player_count) {
            summary.buildable = true;
            summary.systems = systems;
            summary.hyperlanes = hyperlanes;
        }
        if player_count.is_some() && !summary.buildable {
            continue;
        }
        out.push(summary);
    }
    if let Some(n) = player_count
        && let Some(default) = recommended_for(n)
        && let Some(entry) = out.iter_mut().find(|s| s.alias == default)
    {
        entry.recommended = true;
    }
    Ok(out)
}

/// The public face of `choice` in a lobby of `player_count` seats.
#[must_use]
pub fn describe(choice: &MapChoice, player_count: usize) -> MapChoiceView {
    let (systems, hyperlanes) = shape(choice, player_count).unwrap_or((0, false));
    match choice {
        MapChoice::Random => MapChoiceView {
            kind: "random",
            alias: None,
            author: None,
            systems,
            hyperlanes,
            recommended: false,
        },
        MapChoice::Template { alias } => {
            let author = TemplateLoader::load()
                .ok()
                .and_then(|l| l.get(alias).map(|t| t.author.clone()))
                .filter(|a| !a.is_empty());
            let recommended = recommended_for(player_count).as_deref() == Some(alias.as_str());
            MapChoiceView {
                kind: "template",
                alias: Some(alias.clone()),
                author,
                systems,
                hyperlanes,
                recommended,
            }
        }
    }
}
