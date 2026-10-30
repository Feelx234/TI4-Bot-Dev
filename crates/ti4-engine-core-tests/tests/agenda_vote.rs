//! D4-2a: `rules::agenda::window::Vote` (the voting of a whole table over one agenda) and the vote arithmetic it stands on.

use ti4_engine_core::choice::agenda::Opt as AOpt;
use ti4_engine_core::generated::{Agenda, Planet, Source};
use ti4_engine_core::rules::agenda::window::{Facts, Vote};
use ti4_engine_core::rules::agenda::{tiebreak_candidates, undisputed_winner, Tiebreak};
use ti4_engine_core::state::Outcome;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn facts() -> Facts {
    Facts::new(ALL_SOURCES, false)
}

fn vote() -> Vote {
    Vote::new(Agenda::ALL[0], &[Outcome::For, Outcome::Against], &[Seat(0), Seat(1), Seat(2)], facts())
}

/// Three planets of base game tiles with 1, 2 and 3 influence and no special rule (found by their printed influence).
fn planets_by_influence() -> [Planet; 3] {
    let find = |i: u8| {
        plain_systems()
            .iter()
            .flat_map(|s| s.data().planets.iter().copied())
            .find(|p| p.data().influence == i && p.data().source == Source::Base && !p.data().legendary)
            .unwrap_or_else(|| panic!("a plain planet with {i} influence"))
    };
    [find(1), find(2), find(3)]
}

fn table() -> Scenario {
    let [a, b, c] = planets_by_influence();
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes]).map_from(&unique(&[system_of(a), system_of(b), system_of(c)])).control(a, 0).control(b, 1).control(c, 2)
}

fn unique(systems: &[ti4_engine_core::generated::System]) -> Vec<ti4_engine_core::generated::System> {
    let mut out: Vec<ti4_engine_core::generated::System> = Vec::new();
    for s in systems {
        if !out.contains(s) {
            out.push(*s);
        }
    }
    out
}

fn back(o: Outcome) -> Opt {
    Opt::Agenda(AOpt::Back(o, Default::default()))
}

fn votes_of(rig: &Rig<Vote>, o: Outcome) -> i64 {
    rig.driver.window.counts().as_slice().iter().find(|(x, _)| *x == o).map_or(0, |(_, n)| *n)
}

#[test]
fn each_voter_backs_an_outcome_and_exhausts_planets_for_their_influence() {
    covers!("rules::agenda::window::Vote");
    let [a, b, c] = planets_by_influence();
    let mut rig = Rig::new(table(), vote());
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    assert!(rig.opts().contains(&back(Outcome::For)) && rig.opts().contains(&back(Outcome::Against)) && rig.opts().last() == Some(&Opt::Decline));
    rig.assert_options_sound();
    rig.pick_opt(back(Outcome::For));
    assert_eq!(rig.asked().actor, Seat(0), "the same voter now names the planets");
    assert!(rig.opts().iter().any(|o| matches!(o, Opt::Agenda(AOpt::Exhaust { planet, .. }) if *planet == a)));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Exhaust { planet, .. }) if *planet == a));
    assert!(rig.state.board().planet(a).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "the planet is spent");
    assert_eq!(rig.asked().actor, Seat(1), "no planet left to name: the vote is banked and the next voter is asked");
    rig.pick_opt(back(Outcome::Against));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Exhaust { planet, .. }) if *planet == b));
    assert_eq!(rig.asked().actor, Seat(2));
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(votes_of(&rig, Outcome::For), 1);
    assert_eq!(votes_of(&rig, Outcome::Against), 2);
    assert_eq!(rig.driver.window.winner(), Some(Outcome::Against));
    assert!(!rig.state.board().planet(c).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "the abstainer's planet stays ready");
}

#[test]
fn a_tie_goes_to_the_speaker_who_picks_among_the_tied_outcomes() {
    covers!("rules::agenda::window::Vote");
    let ones: Vec<Planet> = plain_systems().iter().flat_map(|s| s.data().planets.iter().copied()).filter(|p| p.data().influence == 1 && p.data().source == Source::Base && !p.data().legendary).take(2).collect();
    assert_eq!(ones.len(), 2, "two one-influence planets");
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes]).map_from(&unique(&[system_of(ones[0]), system_of(ones[1])])).control(ones[0], 0).control(ones[1], 1).speaker(2);
    let mut rig = Rig::new(sc, vote());
    rig.run().pick_opt(back(Outcome::For));
    rig.pick(0);
    rig.pick_opt(back(Outcome::Against));
    rig.pick(0);
    rig.decline();
    assert!(!rig.is_done(), "one vote each: a tie");
    assert_eq!(rig.asked().actor, Seat(2), "the speaker breaks it");
    assert_eq!(rig.opts(), vec![back(Outcome::Against), back(Outcome::For)], "the tied outcomes in text order");
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_opt(back(Outcome::For));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.winner(), Some(Outcome::For));
    assert_eq!((votes_of(&rig, Outcome::For), votes_of(&rig, Outcome::Against)), (1, 1));
}

#[test]
fn nobody_voting_leaves_no_winner_and_the_speaker_breaks_the_silence() {
    covers!("rules::agenda::window::Vote");
    let mut rig = Rig::new(table().speaker(1), vote());
    rig.run();
    rig.decline();
    rig.decline();
    rig.decline();
    assert!(!rig.is_done(), "nobody voted: everything on offer is tied");
    assert_eq!(rig.asked().actor, Seat(1), "the speaker decides");
    assert_eq!(rig.opts(), vec![back(Outcome::For), back(Outcome::Against)]);
    rig.pick_opt(back(Outcome::Against));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.winner(), Some(Outcome::Against));
    smoke("agenda_vote/silence", || Rig::new(table().speaker(1), vote()));
}

#[test]
fn a_vote_with_no_outcome_is_done_at_once() {
    covers!("rules::agenda::window::Vote");
    let mut rig = Rig::new(table(), Vote::new(Agenda::ALL[0], &[], &[Seat(0)], facts()));
    rig.run();
    assert!(rig.is_done() && rig.driver.window.winner().is_none());
    assert!(rig.driver.window.is_done());
}

#[test]
fn trade_goods_buy_extra_votes_only_for_the_spender_of_an_executive_order() {
    covers!("rules::agenda::window::Vote");
    let sc = table().goods(0, 3);
    let [a, _, _] = planets_by_influence();
    let exhaust_a = |o: &Opt| matches!(o, Opt::Agenda(AOpt::Exhaust { planet, .. }) if *planet == a);
    let mut plain = Rig::new(sc.clone(), vote());
    plain.run().pick_opt(back(Outcome::For));
    plain.pick_where(exhaust_a);
    assert_eq!(plain.asked().actor, Seat(1), "no goods question: the vote is banked");
    let mut spender = Rig::new(sc, vote().with_spender(Seat(0)));
    spender.run().pick_opt(back(Outcome::For));
    spender.pick_where(exhaust_a);
    assert!(spender.asked().opts.iter().any(|o| matches!(o, Opt::Agenda(AOpt::Spend(_)))), "the spender may buy votes with goods");
}

#[test]
fn the_undisputed_winner_needs_a_single_best_count_above_zero() {
    let choices = [Outcome::For, Outcome::Against];
    assert_eq!(undisputed_winner(&[(Outcome::For, 3), (Outcome::Against, 1)], &choices), Some(Outcome::For));
    assert_eq!(undisputed_winner(&[(Outcome::For, 2), (Outcome::Against, 2)], &choices), None);
    assert_eq!(undisputed_winner(&[], &choices), None);
    assert_eq!(undisputed_winner(&[(Outcome::For, 0)], &choices), None, "zero votes win nothing");
    assert_eq!(undisputed_winner(&[(Outcome::For, 2)], &[]), None);
}

#[test]
fn tied_candidates_are_the_best_counts_in_text_order_or_everything_when_nobody_voted() {
    let choices = [Outcome::For, Outcome::Against];
    match tiebreak_candidates(&[(Outcome::For, 2), (Outcome::Against, 2)], &choices) {
        Tiebreak::Tied(list) => assert_eq!(list.as_slice(), &[Outcome::Against, Outcome::For], "against sorts before for as text"),
        Tiebreak::Silent(_) => panic!("a tie"),
    }
    match tiebreak_candidates(&[(Outcome::For, 1), (Outcome::Against, 3)], &choices) {
        Tiebreak::Tied(list) => assert_eq!(list.as_slice(), &[Outcome::Against]),
        Tiebreak::Silent(_) => panic!("a single best"),
    }
    match tiebreak_candidates(&[], &choices) {
        Tiebreak::Silent(list) => assert_eq!(list.as_slice(), &choices),
        Tiebreak::Tied(_) => panic!("nobody voted"),
    }
}
