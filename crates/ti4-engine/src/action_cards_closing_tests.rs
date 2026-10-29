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

/// The decline of a reaction window (a slot among the options): the rivals' cards stay in their hands, so the play
/// under test is never cancelled by a Sabotage.
fn decline_slots(choice: &Choice) -> Option<ChoiceOption> {
    choice.options.iter().any(|o| o.id.starts_with("reaction:")).then(|| choice.options.iter().find(|o| o.is_decline()).cloned()).flatten()
}

impl Decider for FirstOption {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(decline) = decline_slots(choice) {
            return Ok(decline);
        }
        choice.options.first().cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

/// Takes the last option that does not stop the effect (the picks of a two-pick card come out in descending order).
struct LastGo;

impl Decider for LastGo {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if let Some(decline) = decline_slots(choice) {
            return Ok(decline);
        }
        choice
            .options
            .iter()
            .rev()
            .find(|o| o.id != "stop" && !o.is_decline())
            .or(choice.options.last())
            .cloned()
            .ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
    }
}

/// Play each of `cases` (alias, position) as the component action of seat `a` and compare the announce window; the
/// position must change and the card must be on the pile.
fn announce_all(cases: &[(&str, GameState)]) {
    let a = PlayerId::new("a");
    let _ = crate::dual::take_report();
    for pass in 0..3 {
        let first = pass == 1;
        for (alias, start) in cases {
            let mut state = start.clone();
            state.player_mut(&a).unwrap().action_cards = vec![ActionCardId::new(*alias)];
            let before = state.clone();
            crate::action_cards::discard(&mut state, &a, 0);
            let decider: Box<dyn Decider> = match pass {
                0 => Box::new(DeclineSlots),
                1 => Box::new(FirstOption),
                _ => Box::new(LastGo),
            };
            with_mask(dual(), || announce(&mut state, alias, &a, decider));
            assert_eq!(state.discarded_action_cards.last(), Some(&ActionCardId::new(*alias)), "{alias} is on the pile");
            assert!(serde_json::to_string(&state).unwrap() != serde_json::to_string(&before).unwrap(), "{alias} changed nothing ({first})");
        }
    }
    let report = crate::dual::report();
    let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
    assert!(report.differences.is_empty(), "{} differences: {shown}", report.differences.len());
    assert_eq!(windows("action_cards::announce"), (3 * cases.len() as u64, 0), "{:?}", report.windows);
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

/// Lucky Shot and Scuttle through the announcement (the per-ship options, the stop of the second Scuttle question).
#[test]
fn lucky_shot_and_scuttle_are_announced_and_resolved_in_the_core_window() {
    let _serial = crate::dual::tests::serial();
    let cases: Vec<(&str, GameState)> = ["lucky", "scuttle"].iter().map(|alias| (*alias, closing_position())).collect();
    announce_all(&cases);
}

/// Scuttle with one ship of its own (taken without a question) and Lucky Shot where only one hull can be shot.
#[test]
fn scuttle_with_one_ship_and_lucky_shot_with_one_target() {
    let _serial = crate::dual::tests::serial();
    let a = PlayerId::new("a");
    let mut one = closing_position();
    let mut kept = false;
    for board in one.board.values_mut() {
        board.units.retain(|u| {
            if u.owner != a {
                return true;
            }
            let keep = !kept && u.type_id.as_str() == "cruiser";
            kept |= keep;
            keep
        });
    }
    assert!(kept);
    let mut single = closing_position();
    let mut shot = false;
    for board in single.board.values_mut() {
        board.units.retain(|u| {
            if u.owner == a || !matches!(u.type_id.as_str(), "cruiser" | "destroyer" | "dreadnought") {
                return true;
            }
            let keep = !shot;
            shot = true;
            keep
        });
    }
    // Two ships of one system: they leave from the highest place down.
    let mut pair = closing_position();
    let mut system = None;
    for (id, board) in pair.board.iter_mut() {
        let mine = board.units.iter().filter(|u| u.owner == a && u.type_id.as_str() != "fighter").count();
        if system.is_none() && mine >= 2 {
            system = Some(id.clone());
            board.units.retain(|u| u.owner == a && matches!(u.type_id.as_str(), "carrier" | "cruiser"));
        } else {
            board.units.retain(|u| u.owner != a);
        }
    }
    assert!(system.is_some());
    announce_all(&[("scuttle", one), ("lucky", single), ("scuttle", pair)]);
}
