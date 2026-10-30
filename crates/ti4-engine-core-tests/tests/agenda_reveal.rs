//! D4-2a: `rules::agenda::reveal::Reveal` (an agenda is revealed; the Quash ability, Political Favor and Political Secret may act on it).

use ti4_engine_core::choice::agenda::{Ask as AAsk, Opt as AOpt, UseSource};
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{Ability, Agenda, AgendaElects, PromissoryNote};
use ti4_engine_core::rules::agenda::reveal::{Facts, Reveal};
use ti4_engine_core::state::{EventKind, NoteKey, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

fn for_against(n: usize) -> Vec<Agenda> {
    Agenda::ALL.into_iter().filter(|a| a.data().elects == AgendaElects::ForAgainst).take(n + 1).collect()
}

fn quasher() -> Faction {
    Faction::ALL.into_iter().find(|f| f.data().abilities.contains(&Ability::Quash)).expect("a faction with Quash")
}

fn facts() -> Facts {
    Facts {
        scope: ALL_SOURCES,
        blanked: false,
        no_choice: ti4_engine_core::state::Code(1),
        replaced: ti4_engine_core::state::Code(2),
        quash: ti4_engine_core::state::Code(3),
        favor: ti4_engine_core::state::Code(4),
        secret: ti4_engine_core::state::Code(5),
    }
}

fn state(with_quash: bool, deck: &[Agenda], tokens: i16) -> CoreState {
    let second = if with_quash { quasher() } else { Faction::TheArborec };
    let mut s = Scenario::new(&[Faction::TheEmbersOfMuaat, second]).tokens(1, 1, 1, tokens).state();
    s.decks_mut().agenda = Deck::new(deck.to_vec());
    s
}

fn reveal(s: CoreState, agenda: Agenda) -> Rig<Reveal> {
    Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), Reveal::new(facts(), agenda))
}

#[test]
fn an_agenda_nobody_acts_on_is_revealed_and_its_outcomes_listed() {
    covers!("rules::agenda::reveal::Reveal");
    let agendas = for_against(1);
    let mut rig = reveal(state(false, &[agendas[1]], 0), agendas[0]);
    rig.run();
    assert!(rig.is_done());
    let (agenda, choices) = rig.driver.window.result().expect("a result");
    assert_eq!((agenda, choices), (agendas[0], &[Outcome::For, Outcome::Against][..]));
    assert_eq!(rig.state.agenda().in_play, Some(agendas[0]));
    assert_eq!(rig.state.agenda().choices.as_slice(), &[Outcome::For, Outcome::Against]);
    assert_eq!(rig.state.meta().agenda_seq, 1);
    assert!(!rig.driver.window.failed());
    assert!(rig.driver.emitted.iter().any(|e| e.kind == EventKind::AgendaRevealed));
}

#[test]
fn the_quash_ability_replaces_the_agenda_with_the_next_one_for_a_strategy_token() {
    covers!("rules::agenda::reveal::Reveal");
    let agendas = for_against(2);
    let mut rig = reveal(state(true, &[agendas[1], agendas[2]], 1), agendas[0]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Agenda(AAsk::UseOnAgenda { source: UseSource::Quash, .. })));
    assert_eq!(rig.asked().actor, Seat(1));
    assert_eq!(rig.opts(), vec![Opt::Agenda(AOpt::UseCard), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Agenda(AOpt::UseCard));
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 0, "a strategy token paid, none left to quash the replacement");
    let (agenda, _) = rig.driver.window.result().expect("a result");
    assert_eq!(agenda, agendas[1], "the next agenda from the deck replaced it");
    assert_eq!(rig.state.agenda().in_play, Some(agendas[1]));
    assert_eq!(rig.state.decks().agenda.cards(), &[agendas[2]]);
    assert_eq!(rig.state.meta().agenda_seq, 2, "two reveals");
    ti4_engine_core_tests::assert_snapshot("agenda_reveal/quash", &rig.snapshot());
}

#[test]
fn declining_quash_keeps_the_agenda_and_it_is_not_offered_without_a_token_or_a_deck() {
    covers!("rules::agenda::reveal::Reveal");
    let agendas = for_against(2);
    let mut rig = reveal(state(true, &[agendas[1]], 2), agendas[0]);
    rig.run().decline();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.result().unwrap().0, agendas[0]);
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 2);
    let mut no_token = reveal(state(true, &[agendas[1]], 0), agendas[0]);
    no_token.run();
    assert!(no_token.is_done());
    let mut no_deck = reveal(state(true, &[], 2), agendas[0]);
    no_deck.run();
    assert!(no_deck.is_done(), "an empty deck has nothing to replace it with");
    smoke("agenda_reveal/quash_smoke", || reveal(state(true, &[agendas[1], agendas[2]], 2), agendas[0]));
}

#[test]
fn political_favor_lent_to_another_seat_replaces_the_agenda_at_the_price_of_its_owners_token_and_goes_home() {
    covers!("rules::agenda::reveal::Reveal");
    let agendas = for_against(1);
    let mut s = state(false, &[agendas[1]], 1);
    let key = NoteKey { note: PromissoryNote::PoliticalFavor, faction: Faction::TheArborec };
    s.cards_mut().promissory.edit().insert(key, Seat(0));
    let mut rig = reveal(s, agendas[0]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Agenda(AAsk::UseOnAgenda { source: UseSource::Favor, owner: Seat(1), .. })));
    assert_eq!(rig.asked().actor, Seat(0), "the holder decides");
    rig.pick_opt(Opt::Agenda(AOpt::UseCard));
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 0, "the OWNER pays the token");
    assert_eq!(rig.driver.window.result().unwrap().0, agendas[1]);
    assert_eq!(rig.state.cards().promissory.get(&key), Some(&Seat(1)), "the note went home");
}

#[test]
fn political_secret_bars_its_owner_from_voting_and_goes_home() {
    covers!("rules::agenda::reveal::Reveal");
    let agendas = for_against(1);
    let mut s = state(false, &[agendas[1]], 0);
    let key = NoteKey { note: PromissoryNote::PoliticalSecret, faction: Faction::TheArborec };
    s.cards_mut().promissory.edit().insert(key, Seat(0));
    let mut rig = reveal(s, agendas[0]);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Agenda(AAsk::UseOnAgenda { source: UseSource::Secret, .. })));
    rig.pick_opt(Opt::Agenda(AOpt::UseCard));
    assert!(rig.is_done());
    assert!(rig.state.agenda().predictions.contains_key(&Seat(1)), "the owner may not vote on this agenda");
    assert_eq!(rig.state.cards().promissory.get(&key), Some(&Seat(1)));
    let mut again = Rig::from_parts({
        let mut s2 = state(false, &[], 0);
        s2.cards_mut().promissory.edit().insert(key, Seat(0));
        s2.agenda_mut().predictions.insert(Seat(1), ti4_engine_core::keys::Prediction::Barred(ti4_engine_core::keys::VoteBar::PoliticalSecret));
        s2
    }, ti4_engine_core_tests::kernel_all(), Reveal::new(facts(), agendas[0]));
    again.run();
    assert!(again.is_done(), "an owner who is barred already is not offered the note again");
}
