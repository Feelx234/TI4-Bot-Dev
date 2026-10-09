//! Map generation and static board tile extraction.

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_content::galaxy::{self, Galaxy};
use ti4_engine::seating::{self, SeatingError};
use ti4_engine::setup::start_game_seeded_with_card_set;
use ti4_model::content_types::{FULL, POK, SourceSet};
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::protocol::view::{BoardTileView, PlanetMetaView};

/// Stable, reconstructible galaxy geometry for stateless consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalaxyLayout {
    pub version: u16,
    pub active_sources: Vec<String>,
    pub placements: Vec<GalaxyPlacement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off_map_system_ids: Vec<String>,
}

/// One main-map system placement in axial coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalaxyPlacement {
    pub system_id: String,
    pub q: i32,
    pub r: i32,
}

impl GalaxyLayout {
    #[must_use]
    pub fn from_galaxy(galaxy: &Galaxy, active_sources: SourceSet) -> Self {
        let mut placements: Vec<_> = galaxy
            .system_ids()
            .into_iter()
            .filter_map(|system_id| {
                galaxy.coord_of(system_id).map(|hex| GalaxyPlacement {
                    system_id: system_id.to_owned(),
                    q: hex.q,
                    r: hex.r,
                })
            })
            .collect();
        placements.sort_by_key(|placement| (placement.q, placement.r, placement.system_id.clone()));
        Self {
            version: 1,
            active_sources: active_source_names(active_sources),
            placements,
            off_map_system_ids: galaxy
                .off_map_system_ids()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }
}

fn active_source_names(sources: SourceSet) -> Vec<String> {
    [
        (ti4_model::content_types::Source::Base, "base"),
        (ti4_model::content_types::Source::Pok, "pok"),
        (ti4_model::content_types::Source::Codex1, "codex1"),
        (ti4_model::content_types::Source::Codex2, "codex2"),
        (ti4_model::content_types::Source::Codex3, "codex3"),
        (ti4_model::content_types::Source::Codex4, "codex4"),
        (
            ti4_model::content_types::Source::ThundersEdge,
            "thunders_edge",
        ),
    ]
    .into_iter()
    .filter(|(source, _)| sources.contains(*source))
    .map(|(_, name)| name.to_owned())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::hex::Hex;

    #[test]
    fn galaxy_layout_round_trip_preserves_static_topology() {
        let content = ContentStore::embedded();
        let mut galaxy = Galaxy::placed(
            content,
            &[
                ("18", Hex::new(0, 0)),
                ("39", Hex::new(2, 0)),
                ("26", Hex::new(-2, 0)),
            ],
            POK,
        )
        .expect("source systems");
        galaxy
            .place_off_map(content, "82b", POK)
            .expect("nexus system");

        let layout = GalaxyLayout::from_galaxy(&galaxy, POK);
        let placements: Vec<_> = layout
            .placements
            .iter()
            .map(|placement| {
                (
                    placement.system_id.as_str(),
                    Hex::new(placement.q, placement.r),
                )
            })
            .collect();
        let mut rebuilt = Galaxy::placed(content, &placements, POK).expect("layout placements");
        for system_id in &layout.off_map_system_ids {
            rebuilt
                .place_off_map(content, system_id, POK)
                .expect("layout off-map system");
        }

        assert_eq!(layout.version, 1);
        assert_eq!(layout.active_sources, active_source_names(POK));
        assert_eq!(rebuilt.coord_of("39"), galaxy.coord_of("39"));
        assert_eq!(rebuilt.adjacent("39"), galaxy.adjacent("39"));
        assert!(rebuilt.are_adjacent("82b", "39"));
    }
}

/// Extracts static geometry and metadata for all tiles in a galaxy.
#[must_use]
pub fn build_board_tiles(content: &ContentStore, galaxy: &Galaxy) -> Vec<BoardTileView> {
    let mut board: Vec<BoardTileView> = galaxy
        .system_ids()
        .into_iter()
        .filter_map(|id| {
            let coord = galaxy.coord_of(id)?;
            let special_area = matches!(id, "82a" | "82b").then(|| "nexus".to_owned());
            system_tile_metadata(content, id, coord.q, coord.r, special_area)
        })
        .collect();

    board.sort_by_key(|tile| (tile.special_area.is_some(), tile.q, tile.r));

    for id in ["82a", "82b"] {
        if !board.iter().any(|tile| tile.system_id == id)
            && let Some(tile) = system_tile_metadata(content, id, 0, 0, Some("nexus".to_owned()))
        {
            board.push(tile);
        }
    }

    board.extend(
        ti4_engine::fracture::systems(content, FULL)
            .into_iter()
            .enumerate()
            .filter_map(|(index, id)| {
                system_tile_metadata(
                    content,
                    id.as_str(),
                    i32::try_from(index).unwrap_or(0),
                    0,
                    Some("fracture".to_owned()),
                )
            }),
    );

    board
}

fn system_tile_metadata(
    content: &ContentStore,
    id: &str,
    q: i32,
    r: i32,
    special_area: Option<String>,
) -> Option<BoardTileView> {
    let system = galaxy::system(content, id, FULL)?;
    let planets = system
        .planets()
        .into_iter()
        .filter_map(|planet_id| galaxy::planet(content, planet_id, FULL))
        .map(|planet| PlanetMetaView {
            id: planet.id().to_owned(),
            label: planet.name().unwrap_or(planet.id()).to_owned(),
            resources: i32::try_from(planet.resources()).unwrap_or(0),
            influence: i32::try_from(planet.influence()).unwrap_or(0),
            traits: planet.traits().into_iter().map(str::to_owned).collect(),
            tech_specialties: planet
                .tech_specialties()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            legendary: planet.is_legendary(),
            space_station: planet.is_space_station(),
        })
        .collect();

    let anomalies = [
        (system.is_nebula(), "nebula"),
        (system.is_supernova(), "supernova"),
        (system.is_asteroid_field(), "asteroid field"),
        (system.is_gravity_rift(), "gravity rift"),
        (system.is_scar(), "entropic scar"),
    ]
    .into_iter()
    .filter(|(present, _)| *present)
    .map(|(_, kind)| kind.to_owned())
    .collect();

    let egress = special_area.as_deref() == Some("fracture")
        && system
            .name()
            .is_some_and(|name| name.to_ascii_lowercase().contains("egress"));

    Some(BoardTileView {
        system_id: id.to_owned(),
        label: system.name().unwrap_or(id).to_owned(),
        q,
        r,
        hyperlane: system.is_hyperlane(),
        special_area,
        anomalies,
        wormholes: system.wormholes().into_iter().map(str::to_owned).collect(),
        egress,
        planets,
    })
}

/// Initializes a fresh game with seated factions, deployed starting fleets, and a constructed galaxy.
///
/// # Errors
///
/// Returns [`SeatingError`] if seating, deployment, or galaxy board construction fails.
pub fn create_game_with_map(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
) -> Result<(GameState, Galaxy), SeatingError> {
    create_game_with_map_rotated(content, player_ids, seed, 0)
}

/// [`create_game_with_map`] with the factions rotated (see `seating::seat_in_scope_rotated`).
///
/// # Errors
///
/// As [`create_game_with_map`].
pub fn create_game_with_map_rotated(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    rotation: usize,
) -> Result<(GameState, Galaxy), SeatingError> {
    create_game_with_map_rotated_and_card_set(content, player_ids, seed, rotation, None)
}

/// As [`create_game_with_map`], with the strategy-card set named (`None`: the PoK-scope set).
///
/// # Errors
///
/// Returns [`SeatingError`] if setup (including an unknown card set), seating, deployment, or
/// galaxy board construction fails.
pub fn create_game_with_map_and_card_set(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), SeatingError> {
    create_game_with_map_rotated_and_card_set(content, player_ids, seed, 0, card_set)
}

/// [`create_game_with_map`] with the factions rotated and the strategy-card set named.
///
/// # Errors
///
/// As [`create_game_with_map_and_card_set`].
pub fn create_game_with_map_rotated_and_card_set(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    rotation: usize,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), SeatingError> {
    let assignments = seating::seat_in_scope_rotated(player_ids, rotation);
    create_game_with_map_assigned(content, player_ids, seed, assignments, card_set)
}

/// Deals every seat its faction's promissory notes (69.1).
///
/// Setup deals while the seats still hold the placeholder faction `generic`, which leaves four
/// `<alias>:generic` notes in the last seat's hand and no faction note anywhere. Dealing again once
/// the factions are seated gives each seat its real hand. The opening state is stored with the
/// game, so games created before this call keep the hand they were created with.
fn deal_notes_after_seating(state: &mut GameState, content: &ContentStore) {
    ti4_engine::promissory::deal(state, content, POK);
}

/// [`create_game_with_map`] with explicit faction assignments (smoke presets that seat factions
/// outside the in-scope six) and the strategy-card set named.
///
/// # Errors
///
/// As [`create_game_with_map_and_card_set`].
pub fn create_game_with_map_assigned(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    assignments: std::collections::BTreeMap<PlayerId, ti4_model::id::FactionId>,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), SeatingError> {
    let mut state = start_game_seeded_with_card_set(content, player_ids, POK, None, seed, card_set)
        .map_err(|e| SeatingError::UnknownPlayer(e.to_string()))?;

    for (player, faction) in &assignments {
        seating::deploy(&mut state, content, player, faction, POK)?;
    }
    deal_notes_after_seating(&mut state, content);

    let filler = seating::map_filler(content, 36, POK, seed);
    let filler_refs: Vec<&str> = filler.iter().map(SystemId::as_str).collect();
    // `build_board` places homes in key order. Real player ids sort arbitrarily, so key the
    // table by seat number instead: seat order, not id order, decides who sits where, which is
    // what a lobby's map preview (made before anyone has an id) can show.
    let by_seat: std::collections::BTreeMap<PlayerId, _> = player_ids
        .iter()
        .enumerate()
        .map(|(seat, player)| {
            (
                PlayerId::new(format!("seat_{seat:02}")),
                assignments[player].clone(),
            )
        })
        .collect();
    let galaxy = seating::build_board(content, &by_seat, &filler_refs, POK)?;

    Ok((state, galaxy))
}

/// Like [`create_game_with_map`], but lays the board out from the predefined map template
/// `template` when one is named; `None` keeps the seeded random-filler board.
///
/// # Errors
///
/// A message for an unknown template, a seat-count mismatch, or any seating/galaxy failure.
pub fn create_game_with_template(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
) -> Result<(GameState, Galaxy), String> {
    create_game_with_template_rotated(content, player_ids, seed, template, 0)
}

/// [`create_game_with_template`] with the factions rotated (see `seating::seat_in_scope_rotated`).
///
/// # Errors
///
/// As [`create_game_with_template`].
pub fn create_game_with_template_rotated(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    rotation: usize,
) -> Result<(GameState, Galaxy), String> {
    create_game_with_template_rotated_and_card_set(content, player_ids, seed, template, rotation, None)
}

/// As [`create_game_with_template`], with the strategy-card set named (`None`: the PoK-scope set).
///
/// # Errors
///
/// A message for an unknown template or card set, a seat-count mismatch, or any seating/galaxy
/// failure.
pub fn create_game_with_template_and_card_set(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), String> {
    create_game_with_template_rotated_and_card_set(content, player_ids, seed, template, 0, card_set)
}

/// [`create_game_with_template`] with the factions rotated and the strategy-card set named.
///
/// # Errors
///
/// As [`create_game_with_template_and_card_set`].
pub fn create_game_with_template_rotated_and_card_set(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    rotation: usize,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), String> {
    let assignments = seating::seat_in_scope_rotated(player_ids, rotation);
    create_game_with_template_assigned(content, player_ids, seed, template, assignments, card_set)
}

/// [`create_game_with_template`] with explicit faction assignments and the card set named.
///
/// # Errors
///
/// As [`create_game_with_template_and_card_set`].
pub fn create_game_with_template_assigned(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    assignments: std::collections::BTreeMap<PlayerId, ti4_model::id::FactionId>,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), String> {
    let Some(alias) = template else {
        return create_game_with_map_assigned(content, player_ids, seed, assignments, card_set)
            .map_err(|e| e.to_string());
    };
    let loader = crate::maps::TemplateLoader::load()?;
    let template = loader
        .get(alias)
        .ok_or_else(|| crate::maps::TemplateError::Unknown(alias.to_owned()).to_string())?;

    let mut state = start_game_seeded_with_card_set(content, player_ids, POK, None, seed, card_set)
        .map_err(|e| e.to_string())?;
    for (player, faction) in &assignments {
        seating::deploy(&mut state, content, player, faction, POK).map_err(|e| e.to_string())?;
    }
    deal_notes_after_seating(&mut state, content);
    let homes: Vec<SystemId> = player_ids
        .iter()
        .map(|player| {
            let one =
                std::collections::BTreeMap::from([(player.clone(), assignments[player].clone())]);
            seating::home_systems(content, &one).map(|mut homes| homes.remove(0))
        })
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let galaxy = crate::maps::build_template_galaxy(content, template, &homes, seed, POK)
        .map_err(|e| e.to_string())?;
    Ok((state, galaxy))
}

/// As [`create_game_with_template`], then applies the named start preset (if any) to the opening
/// state. Without a preset the result is exactly [`create_game_with_template`]'s.
///
/// # Errors
/// Anything [`create_game_with_template`] reports, or an unknown or unplaceable preset.
pub fn create_game_with_preset(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    preset: Option<&str>,
) -> Result<(GameState, Galaxy), String> {
    create_game_with_options(content, player_ids, seed, template, preset, None)
}

/// As [`create_game_with_preset`], with the strategy-card set named (`None`: the PoK-scope set).
///
/// # Errors
/// Anything [`create_game_with_preset`] reports, or an unknown card set.
pub fn create_game_with_options(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    preset: Option<&str>,
    card_set: Option<&str>,
) -> Result<(GameState, Galaxy), String> {
    let rotation = preset.map_or(0, |name| crate::preset::rotation(name, seed));
    // A preset that seats factions outside the in-scope six names them itself.
    let assignments = preset
        .and_then(|name| crate::preset::roster(name, seed, player_ids))
        .unwrap_or_else(|| seating::seat_in_scope_rotated(player_ids, rotation));
    let (mut state, galaxy) = create_game_with_template_assigned(
        content, player_ids, seed, template, assignments, card_set,
    )?;
    if let Some(preset) = preset {
        crate::preset::apply(content, &mut state, &galaxy, player_ids, seed, preset)?;
    }
    Ok((state, galaxy))
}

/// The galaxy a game was built with, rebuilt for recovery from the factions its saved opening state
/// seats. Rebuilding with the in-scope order instead (as recovery did) lays the board out for the
/// wrong home systems whenever a start preset rotated the factions or seated others (`+rot`,
/// `+fac`, `leaders`, `world`), and the replay then diverges at the first move.
///
/// # Errors
///
/// As [`create_game_with_template_assigned`].
pub fn rebuild_galaxy(
    content: &ContentStore,
    player_ids: &[PlayerId],
    seed: u64,
    template: Option<&str>,
    opening: &GameState,
) -> Result<Galaxy, String> {
    let seated: std::collections::BTreeMap<PlayerId, ti4_model::id::FactionId> = player_ids
        .iter()
        .filter_map(|player| {
            opening
                .player(player)
                .map(|seat| (player.clone(), seat.faction.clone()))
        })
        .collect();
    // A state whose seats have no faction yet (the "generic" placeholder of a bare test session)
    // keeps the old behaviour.
    let real = seated.len() == player_ids.len()
        && seated
            .values()
            .all(|faction| ti4_content::factions::get(content, faction.as_str()).is_some());
    let assignments = if real {
        seated
    } else {
        seating::seat_in_scope(player_ids)
    };
    create_game_with_template_assigned(content, player_ids, seed, template, assignments, None)
        .map(|(_, galaxy)| galaxy)
}

#[cfg(test)]
mod template_tests {
    use super::*;

    fn players(n: usize) -> Vec<PlayerId> {
        (1..=n).map(|i| PlayerId::new(format!("p{i}"))).collect()
    }

    #[test]
    fn a_rebuilt_galaxy_follows_the_seated_factions_not_the_in_scope_order() {
        let content = ContentStore::embedded();
        let list = players(4);
        for preset in ["world", "combat+rot", "leaders", "agenda+fac:naalu:mentak", "combat"] {
            let (state, galaxy) =
                create_game_with_preset(content, &list, 7, Some("4pHyperlanes"), Some(preset)).unwrap();
            let rebuilt = rebuild_galaxy(content, &list, 7, Some("4pHyperlanes"), &state).unwrap();
            assert_eq!(
                build_board_tiles(content, &galaxy),
                build_board_tiles(content, &rebuilt),
                "{preset}"
            );
        }
    }

    #[test]
    fn an_unknown_template_is_an_error() {
        let err = create_game_with_template(ContentStore::embedded(), &players(6), 1, Some("nope"))
            .unwrap_err();
        assert!(err.contains("unknown map template"), "{err}");
    }

    #[test]
    fn no_template_keeps_the_random_board() {
        let content = ContentStore::embedded();
        let (_, a) = create_game_with_template(content, &players(6), 9, None).unwrap();
        let (_, b) = create_game_with_map(content, &players(6), 9).unwrap();
        assert_eq!(
            build_board_tiles(content, &a),
            build_board_tiles(content, &b)
        );
    }

    #[test]
    fn a_template_game_seats_every_player_on_their_home() {
        let content = ContentStore::embedded();
        let (state, galaxy) =
            create_game_with_template(content, &players(6), 4, Some("6pStandard")).unwrap();
        assert_eq!(state.players.len(), 6);
        assert!(galaxy.coord_of("18").is_some());
        let (_, random) = create_game_with_map(content, &players(6), 4).unwrap();
        assert_eq!(
            build_board_tiles(content, &galaxy).len(),
            build_board_tiles(content, &random).len()
        );
    }

    #[test]
    fn every_roster_faction_is_dealt_its_own_notes_at_every_table_size() {
        use crate::preset::ROSTER;
        let content = ContentStore::embedded();
        let mut failures = Vec::new();
        for alias in ROSTER {
            for (n, seed) in [(3, 1_u64), (4, 2), (6, 3)] {
                let list = players(n);
                let name = format!("world+fac:{alias}");
                // Both the plain map path and a preset (which seats named factions).
                let (state, _) = create_game_with_map(content, &list, seed).unwrap();
                assert!(
                    state.promissory_notes.keys().all(|k| !k.ends_with(":generic")),
                    "generic notes left in a {n}p default game"
                );
                let _ = &name;
                let Ok((state, _)) = create_game_with_preset(content, &list, seed, None, Some(&name))
                else {
                    failures.push(format!("{alias} {n}p"));
                    continue;
                };
                for seat in &state.players {
                    let faction = seat.faction.as_str();
                    for generic in ["cf", "ps", "ta"] {
                        let id = ti4_engine::promissory::note_id(generic, faction);
                        if !state.promissory_notes.contains_key(&id) {
                            failures.push(format!("{alias} {n}p: {faction} lacks {id}"));
                        }
                    }
                    let own = ti4_content::factions::get(content, faction)
                        .map(|f| f.promissory_notes().len())
                        .unwrap_or(0);
                    let held = state
                        .promissory_notes
                        .keys()
                        .filter(|k| ti4_engine::promissory::owner_of(k).as_deref() == Some(faction))
                        .count();
                    if own == 0 || held < 3 + own {
                        failures.push(format!("{alias} {n}p: {faction} holds {held}, own {own}"));
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
