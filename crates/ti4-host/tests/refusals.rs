//! The host's replay refuses what the product does not play (docs/D28E_RESUME.md): a game of an
//! older rules version, with Thunder's Edge content, with a dropped faction, with structured
//! diplomacy. The refusal comes before the engine sees the game, with the reason.

use std::collections::BTreeMap;

use ti4_host::disabled::{DISABLED_FACTIONS, Disabled};
use ti4_host::replay::{ReplayError, replay_session, replay_session_forced, replay_session_to_next_question, replay_session_with_copy};
use ti4_host::rng_force::RngMarks;
use ti4_model::id::{FactionId, PlayerId};
use ti4_model::state::{GameState, MIN_SUPPORTED_RULES_VERSION, RULES_VERSION};

fn state() -> GameState {
    let ids: Vec<PlayerId> = ["a", "b", "c"].map(PlayerId::new).to_vec();
    let mut state = GameState::new(&ids, &[], BTreeMap::new(), None, 7);
    for (player, faction) in state.players.iter_mut().zip(["sol", "hacan", "mentak"]) {
        player.faction = FactionId::new(faction);
    }
    state
}

fn refusal(result: Result<ti4_host::replay::ReplayReport, ReplayError>) -> Disabled {
    match result {
        Err(ReplayError::Refused(reason)) => reason,
        other => panic!("not refused: {:?}", other.map(|report| report.decision_count)),
    }
}

#[test]
fn the_newest_version_replays_and_every_older_one_is_refused() {
    let current = state();
    assert_eq!(current.rules_version, RULES_VERSION);
    assert!(replay_session(&current, None, &[]).is_ok());
    for older in [0, 1, 27, MIN_SUPPORTED_RULES_VERSION - 1] {
        let mut old = state();
        old.rules_version = older;
        assert_eq!(
            refusal(replay_session(&old, None, &[])),
            Disabled::OlderVersion { found: older, min: MIN_SUPPORTED_RULES_VERSION }
        );
    }
}

#[test]
fn every_way_into_the_replay_refuses() {
    let mut old = state();
    old.rules_version = 0;
    let marks = RngMarks::new();
    assert!(refusal(replay_session_forced(&old, None, &[], &marks)).is_older_version());
    assert!(refusal(replay_session_to_next_question(&old, None, &[], &marks)).is_older_version());
    match replay_session_with_copy(&old, None, &[], &marks, 1) {
        Err(ReplayError::Refused(reason)) => assert!(reason.is_older_version()),
        other => panic!("not refused: {:?}", other.map(|(report, _)| report.decision_count)),
    }
}

#[test]
fn thunders_edge_content_and_the_dropped_features_are_refused() {
    let mut te = state();
    te.thunders_edge = true;
    assert_eq!(refusal(replay_session(&te, None, &[])), Disabled::ThundersEdge);
    for faction in DISABLED_FACTIONS {
        let mut dropped = state();
        dropped.players[1].faction = FactionId::new(faction);
        assert_eq!(
            refusal(replay_session(&dropped, None, &[])),
            Disabled::Faction(faction.to_owned()),
            "{faction}"
        );
    }
    let mut diplomacy = state();
    diplomacy.diplomacy.enabled = true;
    assert_eq!(refusal(replay_session(&diplomacy, None, &[])), Disabled::Diplomacy);
}
