//! Features that are switched off in the product (owner decision 2026-10-29,
//! `docs/DISABLE_XF123.md`): structured diplomacy, the turn-redo deck reserve, and the factions
//! Last Bastion and the Crimson Rebellion.
//!
//! The string engine still plays all three (selfplay, training and the unit tests build such
//! tables directly), and the core port will panic if it is ever given one. This module is the
//! single list the **product edges** consult: the server's turn-redo route and game recovery, and
//! game creation ([`crate::setup::create`], which the server lobby and the wasm shell both use).
//! Nothing here deletes data: a refused save stays on disk.

use ti4_model::state::GameState;

use crate::rng_force::RngMarks;

/// Turn redo on the server rewinds a seat's turn and replays the round under a deck reserve
/// (`RngForce::attach`), which the core does not carry. The offline shell's undo replays answers
/// and has no reserve, so it is not affected.
pub const TURN_REDO_ENABLED: bool = false;

/// What a refused request or save says (HTTP 403 body of the turn-redo route).
pub const TURN_REDO_OFF: &str = "Turn redo is switched off in this version.";

/// Faction ids that can no longer be seated: Last Bastion and the Crimson Rebellion.
pub const DISABLED_FACTIONS: [&str; 2] = ["bastion", "crimson"];

/// Why a table or a save is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Disabled {
    #[error(
        "the faction '{0}' is switched off in this version (Last Bastion and the Crimson Rebellion are not playable)"
    )]
    Faction(String),
    #[error("structured diplomacy is switched off in this version")]
    Diplomacy,
    #[error("this game used turn redo, which is switched off in this version")]
    TurnRedo,
}

/// Whether `faction` is one of [`DISABLED_FACTIONS`].
#[must_use]
pub fn is_disabled_faction(faction: &str) -> bool {
    DISABLED_FACTIONS.contains(&faction)
}

/// Refuses a game state that seats a switched-off faction or has structured diplomacy on.
///
/// # Errors
/// [`Disabled::Faction`] or [`Disabled::Diplomacy`].
pub fn check_state(state: &GameState) -> Result<(), Disabled> {
    if let Some(player) = state
        .players
        .iter()
        .find(|player| is_disabled_faction(player.faction.as_str()))
    {
        return Err(Disabled::Faction(player.faction.as_str().to_owned()));
    }
    if state.diplomacy.enabled {
        return Err(Disabled::Diplomacy);
    }
    Ok(())
}

/// Refuses a history with `rng_marks`: only a turn redo writes them, and replaying them attaches
/// the deck reserve.
///
/// # Errors
/// [`Disabled::TurnRedo`].
pub fn check_marks(marks: &RngMarks) -> Result<(), Disabled> {
    if marks.is_empty() {
        Ok(())
    } else {
        Err(Disabled::TurnRedo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::deck_reserve::DeckSegment;
    use ti4_model::id::{FactionId, PlayerId};

    fn state(factions: &[&str]) -> GameState {
        let ids: Vec<PlayerId> = (0..factions.len())
            .map(|i| PlayerId::new(format!("p{i}")))
            .collect();
        let mut state = GameState::new(&ids, &[], std::collections::BTreeMap::new(), None, 0);
        for (player, faction) in state.players.iter_mut().zip(factions) {
            player.faction = FactionId::new(*faction);
        }
        state
    }

    #[test]
    fn a_table_with_ordinary_factions_passes() {
        assert_eq!(check_state(&state(&["sol", "hacan", "mentak"])), Ok(()));
    }

    #[test]
    fn last_bastion_and_crimson_are_refused_by_id() {
        for faction in DISABLED_FACTIONS {
            assert_eq!(
                check_state(&state(&["sol", faction])),
                Err(Disabled::Faction(faction.to_owned()))
            );
        }
        let message = Disabled::Faction("crimson".to_owned()).to_string();
        assert!(message.contains("crimson") && message.contains("switched off"));
    }

    #[test]
    fn structured_diplomacy_is_refused() {
        let mut state = state(&["sol", "hacan", "mentak"]);
        state.diplomacy.enabled = true;
        assert_eq!(check_state(&state), Err(Disabled::Diplomacy));
    }

    #[test]
    fn marks_of_a_redo_are_refused_and_no_marks_pass() {
        assert_eq!(check_marks(&RngMarks::new()), Ok(()));
        let mut marks = RngMarks::new();
        marks.deck_plan.push(DeckSegment {
            from: 1,
            until: 2,
            reserved: Vec::new(),
        });
        assert_eq!(check_marks(&marks), Err(Disabled::TurnRedo));
    }
}
