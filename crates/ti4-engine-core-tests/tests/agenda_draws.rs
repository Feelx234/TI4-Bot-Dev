//! D4A4: the agenda windows that draw action cards through the action-card draw window (a child), where the draw goes over the hand limit:
//! the Politics Rider paid by `Close` and Unconventional Measures (`Resolve`).
//!
//! Printed text: Politics Rider: "After this agenda is revealed, you may predict the outcome; if you are correct, draw 3 action cards and
//! gain the speaker token." Unconventional Measures: "For: Each player who voted for draws 2 action cards." Hand limit: "A player
//! cannot have more than 7 action cards in hand; at the end of a draw they discard down".

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as CO};
use ti4_engine_core::containers::{Deck, SmallList};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::keys::{Prediction, PredictionTag};
use ti4_engine_core::rules::agenda::close::{Close, Facts};
use ti4_engine_core::rules::agenda::effects::Resolve;
use ti4_engine_core::rules::agenda::table_effects::EffectFacts;
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

const SEVEN: [ActionCard; 7] = [ActionCard::Rally, ActionCard::NavSuite, ActionCard::CoupDEtat, ActionCard::Spy, ActionCard::Sabotage1, ActionCard::Veto, ActionCard::Uprising];

fn table_with_groups() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut t);
    t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
    t
}

fn directive() -> Agenda {
    Agenda::ALL.into_iter().find(|a| a.data().kind == AgendaType::Directive && a.data().elects == AgendaElects::ForAgainst).expect("a for/against directive")
}

fn hand(s: &CoreState, seat: u8) -> usize {
    s.players().by_seat(Seat(seat)).unwrap().cards.hand.len()
}

#[test]
fn the_politics_rider_draws_three_cards_asks_what_to_discard_over_the_limit_and_gives_the_speaker_token() {
    covers!("rules::agenda::close::Close");
    let facts = Facts { trade_rider: Code(1), keleres_rider: Code(2), drop_unofficial: false, blanked: false, hooked: 0, research: None };
    let st = || {
        let mut s = Scenario::lobby(3).round(2).hand(1, &SEVEN[..5]).speaker(0).state();
        s.agenda_mut().predictions.insert(Seat(1), Prediction::Tagged(Outcome::For, PredictionTag::PoliticRider));
        s.decks_mut().action = Deck::new(vec![ActionCard::Plague, ActionCard::Rout, ActionCard::Upgrade, ActionCard::Spy]);
        s
    };
    let window = || Close::new(facts, directive(), None, Some(Outcome::For), &[(Seat(1), Outcome::For)], &[(Outcome::For, 3), (Outcome::Against, 1)], &[Outcome::For, Outcome::Against]);
    let mut rig = Rig::from_parts(st(), kernel_all(), window()).with_table(table_with_groups());
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::HandLimit { held: 8 })), "five cards and three drawn: {:?}", rig.asked().ask);
    assert_eq!(rig.asked().actor, Seat(1));
    rig.assert_options_sound();
    assert_eq!(rig.state.decks().action.cards().len(), 1, "3 cards came off the top");
    rig.pick(0);
    assert!(rig.is_done() && !rig.driver.window.unsupported());
    assert_eq!(hand(&rig.state, 1), 7, "back to the limit");
    assert_eq!(rig.state.meta().speaker, Seat(1), "and gain the speaker token");
    assert!(rig.stages.iter().any(|s| s == "Draw"));
    smoke("agenda_draws/politics_rider", || Rig::from_parts(st(), kernel_all(), window()).with_table(table_with_groups()));
}

#[test]
fn unconventional_measures_for_asks_a_voter_over_the_limit_which_card_goes() {
    covers!("rules::agenda::effects::Resolve");
    let voters: SmallList<Seat, 8> = [Seat(0), Seat(1)].into_iter().collect();
    let facts = EffectFacts {
        for_voters: voters,
        against_voters: SmallList::new(),
        mahact_held: [0; 8],
        scope: ALL_SOURCES,
        default_scope: ALL_SOURCES,
        source: Code(1),
        pool: Code(2),
        has_galaxy: true,
        defense_act: Code(3),
        hooked: 0,
        drop_unofficial: false,
        blanked: false,
    };
    let st = || {
        let mut s = Scenario::lobby(3).hand(1, &SEVEN[..6]).state();
        s.decks_mut().action = Deck::new(vec![ActionCard::Plague, ActionCard::Rout, ActionCard::Upgrade, ActionCard::Spy]);
        s
    };
    let window = || Resolve::of_facts(Agenda::UnconventionalMeasures, Outcome::For, &facts).expect("an effect");
    let mut rig = Rig::from_parts(st(), kernel_all(), window()).with_table(table_with_groups());
    rig.run();
    assert_eq!(hand(&rig.state, 0), 2, "the first voter draws 2");
    assert!(matches!(rig.asked().ask, Ask::Cards(CAsk::HandLimit { held: 8 })), "six cards and two drawn: {:?}", rig.asked().ask);
    assert_eq!(rig.asked().actor, Seat(1));
    rig.pick_opt(Opt::Cards(CO::Discard { index: 0, card: ActionCard::Rally }));
    assert!(rig.is_done() && !rig.driver.window.unsupported());
    assert_eq!(hand(&rig.state, 1), 7);
    assert!(rig.stages.iter().any(|s| s == "Drawing"));
    smoke("agenda_draws/unconventional", || Rig::from_parts(st(), kernel_all(), window()).with_table(table_with_groups()));
}
