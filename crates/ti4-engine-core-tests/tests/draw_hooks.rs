//! Package W14: `rules::action_cards::draw::DrawCards` follows the hooks of the kernel: Yssaril's Scheming (a card more, then "choose and
//! discard 1 action card from your hand", the discard staged as the string stages it) and Crafty (no hand limit), a bonus card with the
//! draw, the blank turn of Law's Order that lifts Sanctions' cap.

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt, Reason, Subtype as CSub};
use ti4_engine_core::choice::Subtype;
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::Origin;
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::Ability;
use ti4_engine_core::keys::{MarkKey, Word};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::rules::action_cards::draw::DrawCards;
use ti4_engine_core_tests::prelude::*;

/// Each card its own printed name, except the copies of Flank Speed.
fn groups() -> Vec<u8> {
    let mut g: Vec<u8> = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
    g[ActionCard::FlankSpeed2 as usize] = u8::try_from(ActionCard::FlankSpeed1 as usize).unwrap();
    g
}

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    t.facts.card_groups = groups();
    t
}

/// Seat 0 plays `faction`, holds `hand`, and the deck is `deck`.
fn draw(faction: Faction, hand: &[ActionCard], deck: &[ActionCard], count: usize) -> Rig<DrawCards> {
    let other = if faction == Faction::TheArborec { Faction::TheEmbersOfMuaat } else { Faction::TheArborec };
    let sc = Scenario::new(&[faction, other]).kernel(kernel_all()).hand(0, hand);
    let mut rig = Rig::new(sc, DrawCards::new(Seat(0), count, false)).with_table(table());
    rig.state.decks_mut().action = Deck::new(deck.to_vec());
    rig
}

fn hand(rig: &Rig<DrawCards>) -> Vec<ActionCard> {
    rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec()
}

fn staged_discards(rig: &Rig<DrawCards>) -> Vec<MarkKey> {
    rig.state.marks().tail.iter().filter_map(|(k, _)| if let MarkKey::CardsStaged(_) = k { Some(*k) } else { None }).collect()
}

const THREE: [ActionCard; 3] = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::Spy];

#[test]
fn scheming_draws_a_card_more_then_asks_which_card_of_the_hand_goes_and_stages_the_discard() {
    covers!("rules::action_cards::draw::DrawCards");
    let mut rig = draw(Faction::TheYssarilTribes, &THREE, &[ActionCard::Sabotage1, ActionCard::CoupDEtat, ActionCard::Bribery], 2);
    rig.run();
    let q = rig.asked().clone();
    assert_eq!(q.ask, Ask::Cards(CAsk::OwnHand { reason: Reason::Scheming }), "{}", rig.transcript());
    assert_eq!((q.actor, q.ctx.origin, q.ctx.subtype, q.ctx.optional), (Seat(0), Origin::FactionAbility(Content::Ability(Ability::Scheming)), Subtype::Cards(CSub::SchemingDiscard), false));
    assert_eq!(hand(&rig).len(), 6, "two cards and the one Scheming adds");
    // The options are the first copy of each printed name in hand order (the hand order, not the order of the names), no decline.
    let offered: Vec<ActionCard> = q.opts.iter().map(|o| match o { Opt::Cards(COpt::Card(c)) => *c, other => panic!("{other:?}") }).collect();
    assert_eq!(offered, hand(&rig));
    rig.pick_opt(Opt::Cards(COpt::Card(ActionCard::NavSuite)));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(hand(&rig).len(), 5);
    assert!(!hand(&rig).contains(&ActionCard::NavSuite));
    assert_eq!(staged_discards(&rig).len(), 1, "the discard waits for the game step as `hooks_cards::discard_chosen` stages it");
    let row = rig.state.marks().tail.get(&staged_discards(&rig)[0]).cloned().unwrap();
    assert_eq!(format!("{row:?}").contains(&format!("{:?}", Word::Discarded)), true);
}

#[test]
fn a_hand_whose_cards_are_one_printed_name_discards_without_a_question() {
    covers!("rules::action_cards::draw::DrawCards");
    // Flank Speed's two copies are one name: the hand after the draw holds one name, so the only option goes.
    let mut rig = draw(Faction::TheYssarilTribes, &[], &[ActionCard::FlankSpeed1, ActionCard::FlankSpeed2, ActionCard::FlankSpeed1], 2);
    rig.state.decks_mut().action = Deck::new(vec![ActionCard::FlankSpeed1, ActionCard::FlankSpeed2, ActionCard::FlankSpeed2]);
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(hand(&rig).len(), 2, "three drawn, the only name's first copy discarded");
    assert_eq!(staged_discards(&rig).len(), 1);
}

#[test]
fn nothing_drawn_means_no_discard_and_a_draw_of_nothing_neither() {
    covers!("rules::action_cards::draw::DrawCards");
    let mut rig = draw(Faction::TheYssarilTribes, &THREE, &[], 2);
    rig.run();
    assert!(rig.is_done() && hand(&rig).len() == 3 && staged_discards(&rig).is_empty(), "an empty deck draws nothing: no discard");
    let mut rig = draw(Faction::TheYssarilTribes, &THREE, &[ActionCard::Bribery], 0);
    rig.run();
    assert!(rig.is_done() && hand(&rig).len() == 3 && staged_discards(&rig).is_empty(), "a draw of nothing is the limit's enforcement only");
}

#[test]
fn crafty_has_no_hand_limit_and_other_factions_keep_theirs() {
    covers!("rules::action_cards::draw::DrawCards");
    let eight = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::Spy, ActionCard::Sabotage1, ActionCard::CoupDEtat, ActionCard::Bribery, ActionCard::Reparations, ActionCard::Plague];
    // Seven cards in hand and a draw of one: Yssaril draws two (Scheming), discards one, and ends at eight with no limit question.
    let mut rig = draw(Faction::TheYssarilTribes, &eight[..7], &[eight[7], ActionCard::Veto], 1);
    rig.run();
    rig.pick(0);
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(hand(&rig).len(), 8, "Crafty: any number of cards");
    // Another faction over the limit is asked.
    let mut rig = draw(Faction::TheArborec, &eight[..7], &[eight[7]], 1);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::HandLimit { held: 8 })), "{}", rig.transcript());
}

#[test]
fn sanctions_cap_the_hand_at_three_and_a_faction_that_changes_nothing_is_asked() {
    covers!("rules::action_cards::draw::DrawCards");
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::keys::Outcome;
    let five = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::Spy, ActionCard::Sabotage1, ActionCard::CoupDEtat];
    let mut rig = draw(Faction::TheArborec, &five, &[], 0);
    rig.state.agenda_mut().laws.insert(Agenda::ExecutiveSanctions, Outcome::Empty);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::HandLimit { held: 5 })), "Sanctions caps the hand at three: {}", rig.transcript());
    // Crafty beats the law: Yssaril keeps all five.
    let mut rig = draw(Faction::TheYssarilTribes, &five, &[], 0);
    rig.state.agenda_mut().laws.insert(Agenda::ExecutiveSanctions, Outcome::Empty);
    rig.run();
    assert!(rig.is_done(), "Crafty: \"game effects cannot prevent you\": {}", rig.transcript());
}
