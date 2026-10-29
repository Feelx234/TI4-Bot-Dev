//! Lockstep tests of the cards ported in the closing package of the action-card area (B6c, docs/B6_CARDS.md): each card
//! through the announce/resolve window (`reactions::announce`) on `Area::ActionCards` = `Dual`. A bail is a failure.
#![cfg(all(test, feature = "typed-core"))]

use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use crate::action_cards::tests::closing_position;
use crate::action_cards_announce_tests::{DeclineSlots, announce};
use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice};
use crate::dual::{AreaMask, Backend, with_mask};

fn dual() -> AreaMask {
    AreaMask::STRING.with(crate::dual::Area::ActionCards, Backend::Dual)
}

fn windows(label: &str) -> (u64, u64) {
    let r = crate::dual::report();
    let w = r.windows.get(label).cloned().unwrap_or_default();
    (w.compared, w.bailed)
}

/// Takes the first option of every question (the default runs take the last: a one-sided default would hide an order
/// difference).
struct FirstOption;

impl Decider for FirstOption {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        choice.options.first().cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

/// Play each of `cases` (alias, position) as the component action of seat `a` and compare the announce window; the
/// position must change and the card must be on the pile.
fn announce_all(cases: &[(&str, GameState)]) {
    let a = PlayerId::new("a");
    let _ = crate::dual::take_report();
    for first in [false, true] {
        for (alias, start) in cases {
            let mut state = start.clone();
            state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new(*alias)];
            let before = state.clone();
            crate::action_cards::discard(&mut state, &a, 0);
            let decider: Box<dyn Decider> = if first { Box::new(FirstOption) } else { Box::new(DeclineSlots) };
            with_mask(dual(), || announce(&mut state, alias, &a, decider));
            assert_eq!(state.discarded_action_cards.last(), Some(&ActionCardId::new(*alias)), "{alias} is on the pile");
            assert!(serde_json::to_string(&state).unwrap() != serde_json::to_string(&before).unwrap(), "{alias} changed nothing ({first})");
        }
    }
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (2 * cases.len() as u64, 0), "{:?}", report.windows);
    let _ = crate::dual::take_report();
}

/// Repeal Law, Spy, Plague and Unstable Planet through the announcement: the dice and the streams are compared with
/// the string's, the staged ground-force deaths are the string's rows.
#[test]
fn repeal_law_spy_plague_and_unstable_planet_are_announced_and_resolved_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let cases: Vec<(&str, GameState)> = ["repeal", "spy", "plague", "unstable"].iter().map(|alias| (*alias, closing_position())).collect();
    announce_all(&cases);
}

/// Seize Artifact, Reparations, Public Disgrace and Archaeological Expedition through the announcement.
#[test]
fn seize_artifact_reparations_public_disgrace_and_archaeological_expedition_are_announced_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let cases: Vec<(&str, GameState)> =
        ["seize", "reparations", "disgrace", "arch_expedition"].iter().map(|alias| (*alias, closing_position())).collect();
    announce_all(&cases);
}

/// Public Disgrace when nothing else is on the mat ("if able": the first choice stands) and Reparations when the new
/// controller has no planet to exhaust (the holder only readies): the branches of the two-step cards the default
/// position does not reach.
#[test]
fn public_disgrace_with_an_empty_mat_and_reparations_without_a_planet_to_exhaust() {
    let _serial = crate::dual::tests::serial();
    let mut bare = closing_position();
    bare.unclaimed_strategy_cards.clear();
    let mut lost = closing_position();
    let b = PlayerId::new("b");
    for board in lost.board.values_mut() {
        board.planet_control.retain(|_, owner| *owner != b);
    }
    announce_all(&[("disgrace", bare), ("reparations", lost)]);
}

/// Seize Artifact at a table of two (one neighbour, taken without a question) and Archaeological Expedition with an
/// emptied deck (the draw stops).
#[test]
fn seize_artifact_with_one_neighbour_and_archaeological_expedition_with_an_empty_deck() {
    let _serial = crate::dual::tests::serial();
    let mut two = closing_position();
    two.seating_order.retain(|p| p.as_str() != "c");
    two.players.retain(|p| p.id.as_str() != "c");
    let mut dry = closing_position();
    for deck in dry.exploration_decks.values_mut() {
        deck.truncate(1);
    }
    announce_all(&[("seize", two), ("arch_expedition", dry)]);
}
