//! Victory points and the public objectives turned up while a position is built (legacy
//! `objectives::{adjust_victory_points, reveal_top, reveal_stage, stage_of, points_for}`, the string
//! bodies).

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;
use ti4_model::id::{ObjectiveId, PlayerId};
use ti4_model::state::GameState;

/// Victory points that win the game (`objectives::VICTORY_TARGET`).
pub const VICTORY_TARGET: i32 = 10;

/// What a revealed objective is worth, or `None` if the corpus does not know it.
#[must_use]
pub fn points_for(content: &ContentStore, alias: &ObjectiveId) -> Option<i32> {
    // Both decks: Classified Document Leaks moves a secret objective into the public area.
    [ContentType::PublicObjectives, ContentType::SecretObjectives]
        .into_iter()
        .find_map(|category| content.get(category, alias.as_str()))
        .and_then(|record| record.int("points"))
        .and_then(|points| i32::try_from(points).ok())
}

/// The stage of a public objective, read from its points: stage I is worth one, stage II two.
#[must_use]
pub fn stage_of(content: &ContentStore, alias: &ObjectiveId) -> Option<u8> {
    match points_for(content, alias)? {
        1 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

/// Reveal the first facedown objective of a given stage (61.13, 61.14a). Not simply the top card:
/// the deck is stage I then stage II.
pub fn reveal_stage(
    state: &mut GameState,
    content: &ContentStore,
    stage: u8,
) -> Option<ObjectiveId> {
    let alias = ti4_model::deck_reserve::take_where(
        &mut state.deck_reserve,
        "objective",
        &mut state.objective_deck,
        None,
        &|alias| stage_of(content, alias) == Some(stage),
    )?;
    state.revealed_objectives.push(alias.clone());
    Some(alias)
}

/// Turn the top facedown public objective faceup (LRR 81.2).
pub fn reveal_top(state: &mut GameState) -> Option<ObjectiveId> {
    state.reveal_objective()
}

/// Move a seat's victory points by `delta` (capped at the target above and at zero below) and
/// ledger the change that actually happened. Returns that change; 0 for a seat that does not exist.
pub fn adjust_victory_points(
    state: &mut GameState,
    player: &PlayerId,
    delta: i32,
    reason: &str,
) -> i32 {
    let Some(seat) = state.player_mut(player) else {
        return 0;
    };
    let before = seat.victory_points;
    seat.victory_points = if delta >= 0 {
        (before + delta).min(VICTORY_TARGET)
    } else {
        (before + delta).max(0)
    };
    let applied = seat.victory_points - before;
    state.note_vp(player, applied, reason);
    applied
}
