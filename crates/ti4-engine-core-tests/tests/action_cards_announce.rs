//! D4A4: `rules::action_cards::announce::Announce` (the announce / resolve / discard flow of a played action card) and
//! `rules::action_cards::draw::DrawCards` (draw, then discard down to the hand limit).
//!
//! Rules text: "1.15 ... when a player plays an action card, it is placed in the discard pile after its ability resolves" (a cancelled card is
//! still spent), "2.4 / hand limit: a player cannot have more than seven action cards in their hand; a player over the limit discards cards
//! of their choice until they have seven".

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as CO};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{ActionCard, Unit};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::action_cards::announce::Announce;
use ti4_engine_core::rules::action_cards::draw::DrawCards;
use ti4_engine_core::rules::action_cards::{Env, Facts, Placed};
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

fn four() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheWinnu]).map_ring(7)
}

fn linked() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t
}

fn env() -> Env {
    Env {
        mahact_held: 0,
        scope: ALL_SOURCES,
        blanked: false,
        action_card: Code(0),
        supply_base: Code(0),
        note_order: |a, b| a.cmp(&b),
        infantry: Placed { unit: Some(Unit::InfantryI), on_card: 0 },
        cruiser: Placed { unit: Some(Unit::CruiserI), on_card: 0 },
        fighter: Placed { unit: Some(Unit::FighterI), on_card: 0 },
        facts: Facts { neutral: Some(Seat(1)), rules_version: 35, ..Facts::default() },
    }
}

fn announce(card: ActionCard, st: CoreState, has_effect: bool) -> Rig<Announce> {
    let a = Announce::of(&st, card, Seat(0), env(), has_effect, false).unwrap_or_else(|| panic!("{card:?} is announced"));
    Rig::from_parts(st, kernel_all(), a).with_table(linked()).seeded(3)
}

fn discard_top(s: &CoreState) -> Option<ActionCard> {
    s.cards().discarded_action.as_slice().last().copied()
}

#[test]
fn a_simple_card_is_announced_resolved_and_then_discarded_and_remembered_as_the_last_discard() {
    covers!("rules::action_cards::announce::Announce");
    let mut st = four().state();
    st.meta_mut().combat_round_seq = 4;
    let mut rig = announce(ActionCard::MoraleBoost1, st, true);
    rig.run();
    assert!(rig.is_done(), "a card that only sets a bonus asks nothing");
    assert!(rig.driver.window.played(), "the play stood");
    assert_eq!(rig.state.transient().misc.action_card_plays.as_slice(), &[(Seat(0), ActionCard::MoraleBoost1)]);
    assert_eq!(discard_top(&rig.state), Some(ActionCard::MoraleBoost1), "1.15: the card ends in the discard pile");
    assert_eq!(rig.state.transient().combat.last_action_discarded, Some((Seat(0), ActionCard::MoraleBoost1)));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().card_timers.combat_bonus_round.get(), Some(4), "+1 to the rolls of this round");
    assert!(rig.driver.window.faces().is_empty() && rig.driver.window.placed_tokens().is_none());
    assert_eq!(rig.stages.first().map(String::as_str), Some("Done"));
    smoke_quiet(|| announce(ActionCard::MoraleBoost1, four().state(), true));
}

#[test]
fn a_card_with_a_question_is_resolved_while_the_window_waits_and_discarded_when_the_answer_is_in() {
    covers!("rules::action_cards::announce::Announce");
    let hand = [ActionCard::ALL[3], ActionCard::ALL[4]];
    let st = || four().hand(1, &hand).hand(2, &[ActionCard::ALL[9]]).state();
    let mut rig = announce(ActionCard::Spy, st(), true);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::Pick { .. })), "Spy asks whose hand");
    assert_eq!(rig.state.transient().misc.action_card_plays.len(), 1, "announced before the effect asks");
    assert_eq!(discard_top(&rig.state), None, "not discarded while it still resolves");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Cards(CO::Seat(Seat(1))));
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 1, "Spy took one card");
    assert_eq!(discard_top(&rig.state), Some(ActionCard::Spy));
    assert_eq!(rig.driver.window.faces().len(), 1, "the die that picked the card");
    assert!(rig.stages.iter().any(|s| s == "Resolving"));
    smoke("action_cards_announce/spy", || announce(ActionCard::Spy, st(), true));
}

#[test]
fn a_card_the_string_has_no_effect_for_is_announced_unresolved_and_still_discarded() {
    covers!("rules::action_cards::announce::Announce");
    let mut rig = announce(ActionCard::MoraleBoost1, four().state(), false);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(discard_top(&rig.state), Some(ActionCard::MoraleBoost1));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().card_timers.combat_bonus_round.get(), None, "no effect resolved");
}

#[test]
fn a_card_the_core_does_not_play_is_not_announced_at_all() {
    covers!("rules::action_cards::announce::Announce");
    assert!(Announce::of(&four().state(), ActionCard::DirectHit1, Seat(0), env(), true, false).is_none(), "a card with a window of its own: only a registered play (inline) runs it");
    assert!(Announce::of(&four().state(), ActionCard::DirectHit1, Seat(0), env(), true, true).is_some(), "a rules package registered its effect");
}

// ---- DrawCards --------------------------------------------------------------------------------

fn groups() -> Vec<u8> {
    (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect()
}

fn table_with_groups() -> AbilityTable {
    let mut t = linked();
    t.facts.card_groups = groups();
    t
}

const HAND7: [ActionCard; 7] = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::CoupDEtat, ActionCard::Spy, ActionCard::Sabotage1, ActionCard::Veto, ActionCard::Uprising];

fn drawing(hand: &[ActionCard], deck: &[ActionCard], count: usize) -> Rig<DrawCards> {
    let mut st = four().hand(0, hand).state();
    st.decks_mut().action = Deck::new(deck.to_vec());
    Rig::from_parts(st, kernel_all(), DrawCards::new(Seat(0), count, false)).with_table(table_with_groups())
}

#[test]
fn drawing_within_the_hand_limit_takes_the_cards_from_the_top_of_the_deck_and_asks_nothing() {
    covers!("rules::action_cards::draw::DrawCards");
    let deck = [ActionCard::Plague, ActionCard::Upgrade, ActionCard::Rout];
    let mut rig = drawing(&HAND7[..3], &deck, 2);
    rig.run();
    assert!(rig.is_done());
    let hand = rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec();
    assert_eq!(hand.len(), 5);
    assert_eq!(&hand[3..], &[ActionCard::Plague, ActionCard::Upgrade], "from the top, in order");
    assert_eq!(rig.state.decks().action.len(), 1);
    assert!(!rig.driver.window.unsupported);
    let mut empty = drawing(&HAND7, &[], 2);
    empty.run();
    assert!(empty.is_done(), "an empty deck draws nothing; seven cards are at the limit, not over it");
    assert_eq!(empty.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 7);
}

#[test]
fn a_draw_over_the_limit_asks_which_cards_go_in_one_question() {
    covers!("rules::action_cards::draw::DrawCards");
    let deck = [ActionCard::Plague, ActionCard::Upgrade];
    let mut rig = drawing(&HAND7, &deck, 2);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::HandLimit { held: 9, discard: 2 })), "seven plus two drawn: {:?}", rig.asked().ask);
    assert_eq!(rig.opts().len(), 9, "every card has its own printed name here: one option each");
    rig.assert_options_sound();
    // Two over: both cards in one answer (the multi-ask conversion); one card alone or an option alone is no answer.
    let at = |card: ActionCard| rig.opts().iter().position(|o| matches!(o, Opt::Cards(CO::Discard { card: c, .. }) if *c == card)).unwrap();
    let (rally, upgrade) = (at(ActionCard::Rally), at(ActionCard::Upgrade));
    let one = ti4_engine_core::choice::cards::with_discard_count(0, rally, 1);
    rig.expect_illegal(Pick::Opt(Opt::Cards(CO::DiscardSet { counts: ti4_engine_core::choice::cards::Counts::of(one) })));
    rig.expect_illegal(Pick::Opt(Opt::Cards(CO::Discard { index: 0, card: ActionCard::Rally, copies: 1 })));
    let both = ti4_engine_core::choice::cards::with_discard_count(one, upgrade, 1);
    rig.answer_pick(Pick::Opt(Opt::Cards(CO::DiscardSet { counts: ti4_engine_core::choice::cards::Counts::of(both) })));
    assert!(rig.is_done());
    let hand = rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec();
    assert_eq!(hand.len(), 7);
    assert!(!hand.contains(&ActionCard::Rally) && !hand.contains(&ActionCard::Upgrade) && hand.contains(&ActionCard::Plague));
    assert!(rig.stages.iter().any(|s| s == "Asking"));
    smoke("action_cards_announce/draw_over", || drawing(&HAND7, &deck, 2));
}

#[test]
fn copies_of_one_printed_name_are_one_option_and_a_table_without_name_groups_declines_the_run() {
    covers!("rules::action_cards::draw::DrawCards");
    let mut st = four().hand(0, &HAND7).state();
    st.decks_mut().action = Deck::new(vec![ActionCard::FlankSpeed1, ActionCard::FlankSpeed2]);
    let mut t = linked();
    let mut g = groups();
    g[ActionCard::FlankSpeed2 as usize] = g[ActionCard::FlankSpeed1 as usize];
    t.facts.card_groups = g;
    let mut rig = Rig::from_parts(st.clone(), kernel_all(), DrawCards::new(Seat(0), 2, false)).with_table(t);
    rig.run();
    assert_eq!(rig.opts().len(), 8, "nine cards, the two Flank Speeds one name: eight options");
    assert!(!rig.opts().contains(&Opt::Cards(CO::Discard { index: 8, card: ActionCard::FlankSpeed2, copies: 1 })));
    let mut bare = Rig::from_parts(st, kernel_all(), DrawCards::new(Seat(0), 2, false)).with_table(linked());
    bare.run();
    assert!(bare.is_done() && bare.driver.window.unsupported, "no printed-name groups: the question cannot be built, the caller declines");
}
