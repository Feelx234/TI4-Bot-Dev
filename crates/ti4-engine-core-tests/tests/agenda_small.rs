//! D4-2a: the small agenda windows: `CovertDraw`, `DeadlyPlot`, `OfferDiscard`.

use ti4_engine_core::choice::agenda::{Ask as AAsk, Opt as AOpt};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{Agenda, AgendaElects};
use ti4_engine_core::rules::agenda::covert::CovertDraw;
use ti4_engine_core::rules::agenda::deadly_plot::DeadlyPlot;
use ti4_engine_core::rules::agenda::offer_discard::OfferDiscard;
use ti4_engine_core::state::{Code, Outcome, PlanetFlags, TransientFlags};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

fn with_elects(kind: AgendaElects) -> Vec<Agenda> {
    Agenda::ALL.into_iter().filter(|a| a.data().elects == kind).collect()
}

fn covert(deck: Vec<Agenda>) -> Rig<CovertDraw> {
    let mut s = Scenario::lobby(3).state();
    s.decks_mut().agenda = Deck::new(deck);
    Rig::from_parts(s, kernel_all(), CovertDraw::new(ALL_SOURCES, false))
}

#[test]
fn covert_draw_keeps_the_next_agenda_face_down_with_its_outcomes() {
    covers!("rules::agenda::covert::CovertDraw");
    let for_against = with_elects(AgendaElects::ForAgainst);
    let mut rig = covert(vec![for_against[0], for_against[1]]);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.agenda().covert, Some(for_against[0]));
    assert_eq!(rig.driver.window.choices(), &[Outcome::For, Outcome::Against]);
    assert_eq!(rig.state.decks().agenda.cards(), &[for_against[1]], "only the first card left the deck");
    assert_eq!(rig.state.agenda().in_play, None, "not revealed");
}

#[test]
fn covert_draw_discards_cards_without_an_eligible_outcome_and_an_empty_deck_leaves_nothing() {
    covers!("rules::agenda::covert::CovertDraw");
    let law = with_elects(AgendaElects::Law)[0];
    let for_against = with_elects(AgendaElects::ForAgainst)[0];
    let mut rig = covert(vec![law, for_against]);
    rig.run();
    assert_eq!(rig.state.agenda().covert, Some(for_against), "no law in play: the law election has no candidate, the next card is drawn");
    assert!(rig.state.decks().agenda.cards().is_empty());
    let mut only_law = covert(vec![law]);
    only_law.run();
    assert!(only_law.is_done());
    assert_eq!(only_law.state.agenda().covert, None);
    assert!(only_law.driver.window.choices().is_empty());
    let mut empty = covert(vec![]);
    empty.run();
    assert_eq!(empty.state.agenda().covert, None);
    assert!(empty.driver.window.choices().is_empty());
}

#[test]
fn covert_draw_clears_a_previous_card_and_asks_nothing() {
    covers!("rules::agenda::covert::CovertDraw");
    let mut rig = covert(vec![]);
    rig.state.agenda_mut().covert = Some(Agenda::ALL[0]);
    rig.run();
    assert_eq!(rig.state.agenda().covert, None);
    assert!(rig.try_answer(Pick::Number(0)).is_err(), "nothing is asked");
}

#[test]
fn deadly_plot_discards_the_agenda_and_exhausts_every_planet_of_its_holder_only() {
    covers!("rules::agenda::deadly_plot::DeadlyPlot");
    let planets = planets_of(4);
    let sys = systems_of(&planets);
    let mut sc = Scenario::lobby(2).map_from(&sys);
    for p in &planets[..3] {
        sc = sc.control(*p, 0);
    }
    sc = sc.control(planets[3], 1);
    let mut rig = Rig::new(sc, DeadlyPlot::new(Seat(0)));
    rig.run();
    assert!(rig.is_done());
    assert!(rig.state.transient().misc.flags.has(TransientFlags::AGENDA_DISCARDED));
    for p in &planets[..3] {
        assert!(rig.state.board().planet(*p).flags.has(PlanetFlags::EXHAUSTED), "{p:?}");
    }
    assert!(!rig.state.board().planet(planets[3]).flags.has(PlanetFlags::EXHAUSTED), "another seat's planet stays ready");
    ti4_engine_core_tests::assert_snapshot("agenda_small/deadly_plot", &rig.snapshot());
}

fn planets_of(n: usize) -> Vec<Planet> {
    ti4_engine_core_tests::scenario::planets_with_influence(&[1, 1, 1, 1, 1, 1][..n])
}

fn offer(law: Agenda, laws: &[Agenda]) -> Rig<OfferDiscard> {
    let mut s = Scenario::lobby(2).state();
    for l in laws {
        s.agenda_mut().laws.insert(*l, Outcome::For);
    }
    Rig::from_parts(s, kernel_all(), OfferDiscard::new(law, Seat(1), Code(7), Code(8)))
}

#[test]
fn offer_discard_asks_the_owner_and_removes_the_law_when_taken() {
    covers!("rules::agenda::offer_discard::OfferDiscard");
    let law = Agenda::MinisterOfCommerce;
    let mut rig = offer(law, &[law, Agenda::MinisterOfPeace]);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1));
    assert!(matches!(rig.asked().ask, Ask::Agenda(AAsk::DiscardLaw { law: l, prompt: Code(7) }) if l == law));
    assert_eq!(rig.opts(), vec![Opt::Agenda(AOpt::DiscardLaw), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Agenda(AOpt::DiscardLaw));
    assert!(rig.is_done());
    assert!(rig.driver.window.taken());
    assert!(!rig.state.agenda().laws.contains_key(&law));
    assert!(rig.state.agenda().laws.contains_key(&Agenda::MinisterOfPeace), "other laws stay");
}

#[test]
fn offer_discard_declined_keeps_the_law_and_a_missing_law_is_not_taken() {
    covers!("rules::agenda::offer_discard::OfferDiscard");
    let law = Agenda::MinisterOfCommerce;
    let mut rig = offer(law, &[law]);
    rig.run().decline();
    assert!(rig.is_done());
    assert!(!rig.driver.window.taken());
    assert!(rig.state.agenda().laws.contains_key(&law));
    let mut gone = offer(law, &[]);
    gone.run().pick_opt(Opt::Agenda(AOpt::DiscardLaw));
    assert!(!gone.driver.window.taken(), "nothing to remove");
    let mut illegal = offer(law, &[law]);
    illegal.run();
    illegal.expect_illegal(Pick::Number(3));
    smoke_quiet(|| offer(law, &[law]));
}

fn committee(agenda: Agenda) -> Rig<ti4_engine_core::rules::agenda::committee::Committee> {
    Rig::new(Scenario::lobby(3).round(4), ti4_engine_core::rules::agenda::committee::Committee::new(agenda, Seat(1), Code(3)))
}

#[test]
fn committee_formation_elects_the_named_player_without_a_vote_and_enacts_a_law() {
    covers!("rules::agenda::committee::Committee");
    let law = Agenda::PoliticalCensure;
    let mut rig = committee(law);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1), "the owner chooses");
    assert_eq!(rig.opts(), vec![Opt::Agenda(AOpt::Elect(Seat(0))), Opt::Agenda(AOpt::Elect(Seat(1))), Opt::Agenda(AOpt::Elect(Seat(2)))]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Agenda(AOpt::Elect(Seat(2))));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen(), Some(Seat(2)));
    assert_eq!(rig.state.agenda().laws.get(&law), Some(&Outcome::Seat(Seat(2))));
    let rec = rig.state.agenda().log.last().expect("a log entry");
    assert!(rec.law && rec.enacted && !rec.discarded && !rec.speaker_decided && rec.votes.is_empty());
    assert_eq!(rec.round, 4);
    ti4_engine_core_tests::assert_snapshot("agenda_small/committee_law", &rig.snapshot());
}

#[test]
fn committee_formation_on_a_directive_lists_but_does_not_enact_and_refuses_other_picks() {
    covers!("rules::agenda::committee::Committee");
    let directive = Agenda::ALL.into_iter().find(|a| a.data().kind == AgendaType::Directive && a.data().elects == AgendaElects::Player).expect("a player-electing directive");
    let mut rig = committee(directive);
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.expect_illegal(Pick::Number(1));
    rig.pick(0);
    assert!(rig.is_done());
    assert!(rig.state.agenda().laws.is_empty());
    assert!(!rig.state.agenda().log.last().unwrap().law);
    smoke_quiet(|| committee(directive));
}
