//! Map edits (Creuss hero swap, Muaat hero replace): recorded in the position and replayed onto the
//! galaxy (legacy `movement::{MapEdit, MapEditError, NOVA_SEED, recorded_map_edits, replay_map_edits,
//! apply_map_edit}`).

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::SystemId;
use ti4_model::state::GameState;

/// The Nova Seed tile, the Muaat supernova.
pub const NOVA_SEED: &str = "81";

/// A change to which tile sits where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapEdit {
    /// Creuss hero: "Swap the positions of any 2 non-Fracture systems that contain wormholes or
    /// your units." The two systems exchange hexes; each keeps its contents.
    Swap { a: String, b: String },
    /// Muaat hero: "replace that system tile with the Muaat supernova tile".
    Replace { old: String, new: String },
}

impl MapEdit {
    fn encode(&self) -> String {
        match self {
            Self::Swap { a, b } => format!("swap|{a}|{b}"),
            Self::Replace { old, new } => format!("replace|{old}|{new}"),
        }
    }

    fn decode(text: &str) -> Option<Self> {
        let mut parts = text.split('|');
        match (parts.next()?, parts.next()?, parts.next()?, parts.next()) {
            ("swap", a, b, None) => Some(Self::Swap {
                a: a.to_owned(),
                b: b.to_owned(),
            }),
            ("replace", old, new, None) => Some(Self::Replace {
                old: old.to_owned(),
                new: new.to_owned(),
            }),
            _ => None,
        }
    }
}

const MAP_EDIT_PREFIX: &str = "map_edit|";

/// The map edits this state records, oldest first: what a viewer applies to the setup board to
/// show the tiles where they stand in this frame.
#[must_use]
pub fn recorded_map_edits(state: &GameState) -> Vec<MapEdit> {
    state
        .faction_marks
        .range(MAP_EDIT_PREFIX.to_owned()..)
        .take_while(|(key, _)| key.starts_with(MAP_EDIT_PREFIX))
        .filter_map(|(_, value)| MapEdit::decode(value))
        .collect()
}

/// Why a map edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MapEditError {
    #[error(transparent)]
    Galaxy(#[from] ti4_content::galaxy::GalaxyError),
}

fn apply_to_galaxy_only(
    galaxy: &mut Galaxy,
    content: &ContentStore,
    sources: SourceSet,
    edit: &MapEdit,
) -> Result<(), ti4_content::galaxy::GalaxyError> {
    match edit {
        MapEdit::Swap { a, b } => galaxy.swap_systems(a, b),
        MapEdit::Replace { old, new } => galaxy.replace_system(content, old, new, sources),
    }
}

/// Bring `galaxy` up to the edits recorded in `state`. Idempotent: the galaxy counts the edits it
/// carries.
///
/// # Errors
/// The first edit that no longer fits the map; edits before it stay applied.
pub fn replay_map_edits(
    state: &GameState,
    galaxy: &mut Galaxy,
    content: &ContentStore,
    sources: SourceSet,
) -> Result<(), MapEditError> {
    for edit in recorded_map_edits(state).iter().skip(galaxy.edits_applied()) {
        apply_to_galaxy_only(galaxy, content, sources, edit)?;
    }
    Ok(())
}

/// Make a map edit: change the galaxy, record it in the state so a re-derived map follows, and
/// carry the system's contents with a replaced tile. Atomic: on error neither the galaxy nor the
/// state has changed.
///
/// # Errors
/// [`MapEditError::Galaxy`] for a system not on the grid, a replacement already on the map or not
/// in the corpus, or a swap of a system with itself.
pub fn apply_map_edit(
    state: &mut GameState,
    galaxy: &mut Galaxy,
    content: &ContentStore,
    sources: SourceSet,
    edit: &MapEdit,
) -> Result<(), MapEditError> {
    // Catch the galaxy up first, so the index recorded below is the galaxy's own.
    replay_map_edits(state, galaxy, content, sources)?;
    let mut trial = galaxy.clone();
    apply_to_galaxy_only(&mut trial, content, sources, edit)?;
    *galaxy = trial;

    if let MapEdit::Replace { old, new } = edit {
        let (old, new) = (SystemId::new(old.as_str()), SystemId::new(new.as_str()));
        if let Some(gone) = state.board.remove(&old) {
            let moved = state.system_mut(&new);
            moved.units = gone.units;
            moved.command_tokens = gone.command_tokens;
        }
        if state.frontier_tokens.remove(&old) {
            state.frontier_tokens.insert(new);
        }
        state.wormhole_tokens.retain(|_, system| system != &old);
        if state
            .ion_storm
            .as_ref()
            .is_some_and(|(system, _)| system == &old)
        {
            state.ion_storm = None;
        }
    }
    let index = state
        .faction_marks
        .keys()
        .filter(|key| key.starts_with(MAP_EDIT_PREFIX))
        .count();
    state
        .faction_marks
        .insert(format!("{MAP_EDIT_PREFIX}{index:04}"), edit.encode());
    Ok(())
}
