//! What the product refuses to play, and the one place that says so (owner decisions 2026-10-29,
//! `docs/DISABLE_XF123.md`, and 2026-11-03, `docs/D28E_RESUME.md`):
//!
//! * a game of a **rules version below [`MIN_SUPPORTED_RULES_VERSION`]** (only the newest is
//!   played; nothing converts or opens an older save);
//! * **Thunder's Edge content** (the table switch [`GameState::thunders_edge`]: the expedition, the
//!   breakthroughs, the printings; the strategy card *set* `te` is not content and stays allowed);
//! * **structured diplomacy**, the **turn-redo deck reserve**, and the factions Last Bastion, the
//!   Crimson Rebellion, the Council Keleres, the Deepwrought Scholarate, the Firmament, the
//!   Obsidian and the Ral Nel Consortium.
//!
//! The legacy string engine still plays the dropped features (selfplay, training and the unit tests
//! build such tables directly), and the typed core panics if it is ever given one
//! (`ti4_engine::core_sync::unsupported`). A *known* refusal must therefore never reach that panic
//! in the product: [`check_playable`] answers it with an `Err` at the product edges, which are
//! game creation ([`crate::setup::create`], used by the server lobby and the wasm shell), server
//! recovery, the host's replay ([`crate::replay`]) and the wasm resume. Nothing here deletes
//! data: a refused save stays on disk as it was.

use ti4_model::state::{GameState, MIN_SUPPORTED_RULES_VERSION};

use crate::rng_force::RngMarks;

/// Turn redo on the server rewinds a seat's turn and replays the round under a deck reserve
/// (`RngForce::attach`), which the core does not carry. The offline shell's undo replays answers
/// and has no reserve, so it is not affected.
pub const TURN_REDO_ENABLED: bool = false;

/// What a refused request or save says (HTTP 403 body of the turn-redo route).
pub const TURN_REDO_OFF: &str = "Turn redo is switched off in this version.";

/// The faction ids that are switched off, for the tests and the messages. The decision is the
/// engine's ([`disabled_faction`]); a test keeps this list equal to what the engine refuses.
pub const DISABLED_FACTIONS: [&str; 9] = [
    "bastion",
    "crimson",
    "keleresa",
    "keleresm",
    "keleresx",
    "deepwrought",
    "firmament",
    "obsidian",
    "ralnel",
];

/// Why a table or a save is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Disabled {
    /// A seat plays a faction that is switched off (the id as the state has it).
    #[error(
        "the faction '{0}' is switched off in this version (Last Bastion, the Crimson Rebellion, the Council Keleres, the Deepwrought Scholarate, the Firmament, the Obsidian and the Ral Nel Consortium are not playable)"
    )]
    Faction(String),
    #[error("structured diplomacy is switched off in this version")]
    Diplomacy,
    #[error("this game used turn redo, which is switched off in this version")]
    TurnRedo,
    /// The table switch for Thunder's Edge content is on (or a save carries its effects).
    #[error(
        "Thunder's Edge content (the expedition and the breakthroughs) is switched off in this version; the Thunder's Edge strategy cards can still be played"
    )]
    ThundersEdge,
    /// The game was made with an older rules version than the product plays.
    #[error(
        "this game was saved with an older version of the rules (version {found}); only games of version {min} or newer can be played, so it cannot be opened"
    )]
    OlderVersion { found: u32, min: u32 },
}

impl Disabled {
    /// Whether the refusal is the age of the save (the files are fine, the version is gone) as
    /// opposed to a switched-off feature.
    #[must_use]
    pub const fn is_older_version(&self) -> bool {
        matches!(self, Self::OlderVersion { .. })
    }
}

/// The switched-off feature that `faction` belongs to, if any. The list is the typed core's
/// (`ti4_engine::core_sync::unsupported::find`), asked through a one-seat state rather than copied.
#[must_use]
pub fn disabled_faction(faction: &str) -> Option<Disabled> {
    let player = ti4_model::id::PlayerId::new("seat");
    let mut state = GameState::new(
        std::slice::from_ref(&player),
        &[],
        std::collections::BTreeMap::new(),
        None,
        0,
    );
    state.players.first_mut()?.faction = ti4_model::id::FactionId::new(faction);
    ti4_engine::core_sync::unsupported::find(&state)
        .map(|_| Disabled::Faction(faction.to_owned()))
}

/// Whether `faction` is one of the factions that can no longer be seated.
#[must_use]
pub fn is_disabled_faction(faction: &str) -> bool {
    disabled_faction(faction).is_some()
}

/// Refuses the lobby options that switch Thunder's Edge content on: the table switch, and the
/// start preset `expedition` (with any suffix), which sets it itself.
///
/// # Errors
/// [`Disabled::ThundersEdge`].
pub fn check_table_options(thunders_edge: bool, preset: Option<&str>) -> Result<(), Disabled> {
    let preset_plays_it = preset
        .is_some_and(|name| name.split('+').next() == Some(crate::preset::EXPEDITION));
    if thunders_edge || preset_plays_it {
        Err(Disabled::ThundersEdge)
    } else {
        Ok(())
    }
}

/// Refuses a game state the product does not play: a rules version below
/// [`MIN_SUPPORTED_RULES_VERSION`], Thunder's Edge content, structured diplomacy, a deck reserve
/// (turn redo) or a seat of a switched-off faction. The first one found, in that order.
///
/// # Errors
/// The [`Disabled`] reason.
pub fn check_playable(state: &GameState) -> Result<(), Disabled> {
    if state.rules_version < MIN_SUPPORTED_RULES_VERSION {
        return Err(Disabled::OlderVersion {
            found: state.rules_version,
            min: MIN_SUPPORTED_RULES_VERSION,
        });
    }
    if state.thunders_edge || state.thunders_edge_system.is_some() {
        return Err(Disabled::ThundersEdge);
    }
    // Faction first so that the message names the seat; the engine's list decides which.
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
    if state.deck_reserve.is_some() {
        return Err(Disabled::TurnRedo);
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
        assert_eq!(
            check_playable(&state(&["sol", "hacan", "mentak"])),
            Ok(())
        );
    }

    #[test]
    fn every_dropped_faction_is_refused_by_id() {
        for faction in DISABLED_FACTIONS {
            assert_eq!(
                check_playable(&state(&["sol", faction])),
                Err(Disabled::Faction(faction.to_owned())),
                "{faction}"
            );
        }
        let message = Disabled::Faction("crimson".to_owned()).to_string();
        assert!(message.contains("crimson") && message.contains("switched off"));
    }

    #[test]
    fn structured_diplomacy_is_refused() {
        let mut state = state(&["sol", "hacan", "mentak"]);
        state.diplomacy.enabled = true;
        assert_eq!(check_playable(&state), Err(Disabled::Diplomacy));
    }

    #[test]
    fn thunders_edge_content_is_refused_and_the_message_names_it() {
        let mut state = state(&["sol", "hacan", "mentak"]);
        state.thunders_edge = true;
        assert_eq!(check_playable(&state), Err(Disabled::ThundersEdge));
        let message = Disabled::ThundersEdge.to_string();
        assert!(message.contains("Thunder's Edge") && message.contains("switched off"));
        // The strategy card set is not content: the lobby option check passes without the switch.
        assert_eq!(check_table_options(false, None), Ok(()));
        assert_eq!(check_table_options(true, None), Err(Disabled::ThundersEdge));
        assert_eq!(
            check_table_options(false, Some("expedition+rot")),
            Err(Disabled::ThundersEdge)
        );
        assert_eq!(check_table_options(false, Some("combat")), Ok(()));
    }

    #[test]
    fn a_game_of_an_older_rules_version_is_refused_with_both_numbers() {
        let mut state = state(&["sol", "hacan", "mentak"]);
        assert_eq!(check_playable(&state), Ok(()), "a new game is the newest");
        for older in [0, 1, MIN_SUPPORTED_RULES_VERSION - 1] {
            state.rules_version = older;
            let refused = check_playable(&state).expect_err("older than the newest");
            assert_eq!(
                refused,
                Disabled::OlderVersion {
                    found: older,
                    min: MIN_SUPPORTED_RULES_VERSION
                }
            );
            assert!(refused.is_older_version());
            assert!(refused.to_string().contains("older version"), "{refused}");
        }
        assert!(!Disabled::Diplomacy.is_older_version());
    }

    #[test]
    fn an_older_version_wins_over_the_other_reasons() {
        let mut state = state(&["sol", "bastion"]);
        state.thunders_edge = true;
        state.rules_version = 0;
        assert!(matches!(
            check_playable(&state),
            Err(Disabled::OlderVersion { .. })
        ));
    }

    #[test]
    fn a_deck_reserve_is_a_turn_redo() {
        let mut state = state(&["sol", "hacan", "mentak"]);
        state.deck_reserve = Some(ti4_model::deck_reserve::DeckReserve::new(
            std::sync::Arc::default(),
            ti4_model::deck_reserve::DeckPlan::default(),
        ));
        assert_eq!(check_playable(&state), Err(Disabled::TurnRedo));
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
