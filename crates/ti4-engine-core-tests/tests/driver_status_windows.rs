//! The windows of the status phase's bookkeeping and of the hooked draw, each on its own (they were only ever run inside a job of the
//! turn flow): Scheming's discard (`status_bookkeeping::SchemingDiscard`), the status redistribution of command tokens
//! (`status_finish::Redistribute`) and the draw of action cards with the faction hooks (`hooked_draw::HookedDraw`).

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::choice::objectives::Opt as OOpt;
use ti4_engine_core::driver::windows::hooked_draw::HookedDraw;
use ti4_engine_core::driver::windows::scoring::OccurrenceScoring;
use ti4_engine_core::driver::windows::status_bookkeeping::SchemingDiscard;
use ti4_engine_core::driver::windows::status_finish::Redistribute;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::state::{Feat, ObjectiveRef};
use ti4_engine_core_tests::prelude::*;

/// Each card its own printed name.
fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
    t
}

fn hand(state: &CoreState) -> Vec<ActionCard> {
    state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec()
}

const THREE: [ActionCard; 3] = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::Spy];

fn yssaril(hand: &[ActionCard]) -> Scenario {
    Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec]).kernel(kernel_all()).hand(0, hand)
}

// ---- Scheming's discard ------------------------------------------------------------------------

#[test]
fn scheming_discard_asks_among_the_printed_names_of_the_hand_and_stages_the_chosen_card() {
    covers!("driver::windows::status_bookkeeping::SchemingDiscard");
    let mut rig = Rig::new(yssaril(&THREE), SchemingDiscard::new(Seat(0))).with_table(table());
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Cards(CAsk::OwnHand { reason: Reason::Scheming }), "{}", rig.transcript());
    assert_eq!(rig.opts(), THREE.iter().map(|c| Opt::Cards(COpt::Card(*c))).collect::<Vec<_>>(), "the first copy of each name, hand order, no decline");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Cards(COpt::Card(ActionCard::Plague))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_opt(Opt::Cards(COpt::Card(ActionCard::NavSuite)));
    assert!(rig.is_done() && !rig.driver.window.unsupported);
    assert_eq!(hand(&rig.state).len(), 2, "the card leaves the hand at once; the discard is staged for the game step");
    assert!(!hand(&rig.state).contains(&ActionCard::NavSuite));
}

#[test]
fn scheming_discard_of_an_empty_hand_asks_nothing_and_a_table_without_names_is_unsupported() {
    covers!("driver::windows::status_bookkeeping::SchemingDiscard");
    let mut rig = Rig::new(yssaril(&[]), SchemingDiscard::new(Seat(0))).with_table(table());
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported);
    let mut bare = Rig::new(yssaril(&THREE), SchemingDiscard::new(Seat(0)));
    bare.run();
    assert!(bare.is_done() && bare.driver.window.unsupported, "no printed-name groups: the caller declines the run");
    assert_eq!(hand(&bare.state), THREE, "nothing was discarded");
}

// ---- the status redistribution -----------------------------------------------------------------

#[test]
fn the_status_redistribution_offers_every_split_above_the_floor_and_writes_the_chosen_one() {
    covers!("driver::windows::status_finish::Redistribute");
    let mut rig = Rig::new(Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).tokens(0, 3, 3, 2), Redistribute::new(Seat(0)));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::StatusRedistribute { total: 8, floor: 2 })), "{}", rig.transcript());
    let split = |tactic, fleet, strategic| Opt::Strategy(SOpt::Split { tactic, fleet, strategic });
    let offered = rig.opts();
    assert!(offered.contains(&split(3, 3, 2)) && offered.contains(&split(2, 2, 4)), "the present split and a regroup are offered");
    assert!(offered.contains(&split(6, 2, 0)) && offered.contains(&split(0, 8, 0)), "any tactic and strategic share, the fleet pool from the floor");
    assert!(!offered.contains(&split(7, 1, 0)) && !offered.contains(&split(8, 0, 0)), "the fleet pool keeps at least two");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(split(7, 1, 0)));
    rig.pick_opt(split(2, 2, 4));
    assert!(rig.is_done());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.tactic_tokens, p.econ.fleet_tokens, p.econ.strategic_tokens), (2, 2, 4));
}

#[test]
fn a_seat_with_no_token_on_its_sheet_is_not_asked_to_redistribute() {
    covers!("driver::windows::status_finish::Redistribute");
    let mut rig = Rig::new(Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).tokens(0, 0, 0, 0), Redistribute::new(Seat(0)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

// ---- the hooked draw ---------------------------------------------------------------------------

fn hooked(sc: Scenario, count: usize, deck: &[ActionCard]) -> Rig<HookedDraw> {
    let mut rig = Rig::new(sc, HookedDraw::new(Seat(0), count, false)).with_table(table());
    rig.state.decks_mut().action = Deck::new(deck.to_vec());
    rig
}

#[test]
fn a_draw_without_a_hook_is_the_plain_draw() {
    covers!("driver::windows::hooked_draw::HookedDraw");
    let mut rig = hooked(Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).kernel(kernel_all()).hand(0, &THREE), 2, &[ActionCard::Sabotage1, ActionCard::CoupDEtat, ActionCard::Bribery]);
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(hand(&rig.state).len(), 5, "two cards, nothing more");
}

#[test]
fn a_yssaril_draw_adds_the_scheming_card_asks_the_discard_and_keeps_no_hand_limit() {
    covers!("driver::windows::hooked_draw::HookedDraw");
    let mut rig = hooked(yssaril(&THREE), 2, &[ActionCard::Sabotage1, ActionCard::CoupDEtat, ActionCard::Bribery]);
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Cards(CAsk::OwnHand { reason: Reason::Scheming }), "{}", rig.transcript());
    assert_eq!(hand(&rig.state).len(), 6, "two cards and the one Scheming adds");
    rig.pick(0);
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(hand(&rig.state).len(), 5);
}

#[test]
fn a_draw_that_finds_the_deck_empty_draws_nothing_and_asks_nothing() {
    covers!("driver::windows::hooked_draw::HookedDraw");
    let mut rig = hooked(yssaril(&THREE), 2, &[]);
    rig.run();
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(hand(&rig.state), THREE);
}

// ---- the scoring window of one event occurrence, as a child of the window that earned the feat -----------------------------

fn prove_endurance(version: u32) -> Scenario {
    Scenario::lobby(2).phase(Phase::Action).rules(version)
}

fn with_the_feat(mut rig: Rig<OccurrenceScoring>) -> Rig<OccurrenceScoring> {
    rig.state.players_mut().edit(0).cards.secrets = [SecretObjective::ProveEndurance].into_iter().collect();
    rig.state.players_mut().edit(0).misc.event_feats.push((Feat::LastToPass, 3));
    rig
}

#[test]
fn the_window_of_an_occurrence_asks_the_seat_whose_feat_it_is_and_scoring_uses_its_one_score() {
    covers!("driver::windows::scoring::OccurrenceScoring");
    let score = Opt::Objectives(OOpt::Score(ObjectiveRef::Secret(SecretObjective::ProveEndurance)));
    let mut rig = with_the_feat(Rig::new(prove_endurance(40), OccurrenceScoring::new(3, true)));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "{}", rig.transcript());
    assert!(rig.opts().contains(&score), "the secret of the occurrence is offered");
    rig.pick_opt(score);
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.is_empty(), "the secret left the hand");
    assert!(rig.state.objectives().scored_feats.contains(&(Seat(0), 3)), "the occurrence is the seat's used one");
}

#[test]
fn the_window_of_another_occurrence_or_of_a_table_with_nothing_to_score_asks_nothing() {
    covers!("driver::windows::scoring::OccurrenceScoring");
    let mut other = with_the_feat(Rig::new(prove_endurance(40), OccurrenceScoring::new(4, true)));
    other.run();
    assert!(other.is_done(), "the feat was recorded for occurrence 3");
    let mut bare = Rig::new(prove_endurance(40), OccurrenceScoring::new(3, true));
    bare.run();
    assert!(bare.is_done());
    assert_eq!(delta(bare.start_state(), &bare.state), "");
}
