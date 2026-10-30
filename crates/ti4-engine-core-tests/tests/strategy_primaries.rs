//! D4-2a: `rules::strategy_primaries` (Warfare, Diplomacy, Politics, Imperial and the Thunder's Edge Warfare).

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::containers::{Deck, SmallList};
use ti4_engine_core::generated::{ActionCard, Agenda, Planet, PublicObjective, StrategyCard};
use ti4_engine_core::query::objectives;
use ti4_engine_core::rules::strategy_primaries::*;
use ti4_engine_core::state::Code;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn strat(o: SOpt) -> Opt {
    Opt::Strategy(o)
}

fn tokens_on_board(s: &CoreState, seat: u8) -> usize {
    s.board().systems().filter(|(_, c)| c.cmd.contains(Seat(seat))).count()
}

fn sheet(s: &CoreState, seat: u8) -> (i16, i16, i16) {
    let e = s.players().by_seat(Seat(seat)).unwrap().econ;
    (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
}

// ---- Warfare ----------------------------------------------------------------------------------

fn warfare_table() -> Scenario {
    let s = plain_systems();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheBaronyOfLetnev])
        .map_ring(7)
        .command_token(s[0], 0)
        .command_token(s[2], 0)
        .tokens(0, 3, 3, 2)
}

#[test]
fn warfare_recalls_a_token_from_the_board_gains_one_and_redistributes_the_sheet() {
    covers!("rules::strategy_primaries::WarfarePrimary");
    let s = plain_systems();
    let mut rig = Rig::new(warfare_table(), WarfarePrimary::new(Seat(0), false));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::WarfareRecall));
    assert_eq!(rig.opts(), vec![strat(SOpt::System(s[0])), strat(SOpt::System(s[2]))], "one option per system holding a token, in system order");
    rig.pick_opt(strat(SOpt::System(s[2])));
    assert_eq!(tokens_on_board(&rig.state, 0), 1, "the token left the board");
    assert!(!rig.state.board().sys(s[2]).cmd.contains(Seat(0)));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: 1 }));
    rig.pick_opt(strat(SOpt::Pool(Pool::Strategic)));
    assert_eq!(sheet(&rig.state, 0), (3, 3, 3), "the gained token went to the pool chosen");
    // Redistribute: 9 tokens, at least 2 in the fleet pool: 36 splits.
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::WarfareRedistribute { total: 9, floor: 2 }));
    assert_eq!(rig.asked().opts.len(), 36);
    rig.pick_opt(strat(SOpt::Split { tactic: 7, fleet: 2, strategic: 0 }));
    assert!(rig.is_done());
    assert_eq!(sheet(&rig.state, 0), (7, 2, 0));
}

#[test]
fn warfare_refuses_a_system_without_a_token_and_a_split_below_the_fleet_floor() {
    covers!("rules::strategy_primaries::WarfarePrimary");
    let s = plain_systems();
    let mut rig = Rig::new(warfare_table(), WarfarePrimary::new(Seat(0), false));
    rig.run();
    rig.expect_illegal(Pick::Opt(strat(SOpt::System(s[1]))));
    rig.expect_illegal(Pick::Opt(strat(SOpt::Pool(Pool::Tactic))));
    rig.pick(0).pick(0);
    rig.expect_illegal(Pick::Opt(strat(SOpt::Split { tactic: 8, fleet: 1, strategic: 0 })));
    rig.expect_illegal(Pick::Opt(strat(SOpt::Split { tactic: 4, fleet: 4, strategic: 4 })));
    rig.assert_options_sound();
}

#[test]
fn warfare_without_a_token_on_the_board_asks_nothing_and_letnev_may_empty_its_fleet_pool_from_its_version() {
    covers!("rules::strategy_primaries::WarfarePrimary");
    let mut rig = Rig::new(Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).tokens(0, 3, 3, 2), WarfarePrimary::new(Seat(0), false));
    rig.run();
    assert!(rig.is_done(), "nothing to recall: the primary does nothing");
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    let s = plain_systems();
    let letnev = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec]).map_ring(7).command_token(s[0], 0).tokens(0, 3, 3, 2);
    let mut old = Rig::new(letnev.clone(), WarfarePrimary::new(Seat(0), false));
    old.run().pick(0).pick(0);
    assert_eq!(old.asked().ask, Ask::Strategy(SAsk::WarfareRedistribute { total: 9, floor: 2 }), "before the version: the usual floor");
    let mut new = Rig::new(letnev, WarfarePrimary::new(Seat(0), true));
    new.run().pick(0).pick(0);
    assert_eq!(new.asked().ask, Ask::Strategy(SAsk::WarfareRedistribute { total: 9, floor: 0 }));
    assert_eq!(new.asked().opts.len(), 55);
    let mut other = Rig::new(Scenario::new(&[Faction::TheEmbersOfMuaat]).map_ring(7).command_token(s[0], 0).tokens(0, 3, 3, 2), WarfarePrimary::new(Seat(0), true));
    other.run().pick(0).pick(0);
    assert_eq!(other.asked().ask, Ask::Strategy(SAsk::WarfareRedistribute { total: 9, floor: 2 }), "the zero floor is Letnev's only");
}

#[test]
fn warfare_smoke() {
    smoke("strategy_primaries/warfare", || Rig::new(warfare_table(), WarfarePrimary::new(Seat(0), false)));
}

// ---- Diplomacy --------------------------------------------------------------------------------

fn diplomacy_table() -> (Scenario, Vec<ti4_engine_core::generated::System>, Vec<Planet>) {
    let s = plain_systems();
    let (p0, p1, p2) = (s[0].data().planets[0], s[1].data().planets[0], s[2].data().planets[0]);
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheBaronyOfLetnev])
        .map_ring(7)
        .control(p0, 0)
        .control(p1, 0)
        .control(p2, 0)
        .exhaust(p0)
        .exhaust(p1)
        .exhaust(p2);
    (sc, s, vec![p0, p1, p2])
}

#[test]
fn diplomacy_primary_locks_a_system_with_the_other_players_tokens_then_readies_two_planets() {
    covers!("rules::strategy_primaries::Diplomacy");
    let (sc, s, ps) = diplomacy_table();
    let mut rig = Rig::new(sc, Diplomacy::new(Seat(0), StrategyCard::DiplomacyCodex, DMode::Primary { base_printing: false }));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::DiplomacySystem));
    assert_eq!(rig.opts(), vec![strat(SOpt::System(s[0])), strat(SOpt::System(s[1])), strat(SOpt::System(s[2]))]);
    rig.pick_opt(strat(SOpt::System(s[1])));
    assert!(rig.state.board().sys(s[1]).cmd.contains(Seat(1)) && rig.state.board().sys(s[1]).cmd.contains(Seat(2)), "every other player placed a token");
    assert!(!rig.state.board().sys(s[1]).cmd.contains(Seat(0)), "not the holder");
    assert_eq!(rig.driver.window.placed.as_slice(), &[Seat(1), Seat(2)]);
    assert_eq!(rig.driver.window.chosen(), Some(s[1]));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::ReadyPlanet));
    rig.expect_illegal(Pick::Opt(strat(SOpt::System(s[0]))));
    rig.pick(0).pick(0);
    assert!(rig.is_done(), "two planets readied, then it ends");
    let ready = ps.iter().filter(|p| !rig.state.board().planet(**p).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED)).count();
    assert_eq!(ready, 2);
}

#[test]
fn diplomacy_secondary_only_readies_and_the_base_printing_readies_the_chosen_systems_planets_unasked() {
    covers!("rules::strategy_primaries::Diplomacy");
    let (sc, s, ps) = diplomacy_table();
    let mut sec = Rig::new(sc.clone(), Diplomacy::new(Seat(0), StrategyCard::DiplomacyCodex, DMode::Secondary));
    sec.run();
    assert_eq!(sec.asked().ask, Ask::Strategy(SAsk::ReadyPlanet));
    assert_eq!(sec.asked().opts.len(), 3);
    assert!(sec.asked().decline_index().is_none(), "no way to stop: it names the planets to ready");
    sec.pick(0).pick(0);
    assert!(sec.is_done());
    assert_eq!(delta(sec.start_state(), &sec.state).lines().filter(|l| l.starts_with("planet[")).count(), 2);
    let mut base = Rig::new(sc, Diplomacy::new(Seat(0), StrategyCard::DiplomacyBase, DMode::Primary { base_printing: true }));
    base.run();
    base.pick_opt(strat(SOpt::System(s[0])));
    assert!(base.is_done(), "the base printing asks nothing after the system");
    assert!(!base.state.board().planet(ps[0]).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED));
    assert!(base.state.board().planet(ps[1]).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "only that system's planets");
}

#[test]
fn diplomacy_primary_never_offers_mecatol_and_with_no_system_goes_straight_to_readying() {
    covers!("rules::strategy_primaries::Diplomacy");
    let mecatol = ti4_engine_core::generated::System::MecatolRex18;
    let mecatol_planet = mecatol.data().planets[0];
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[mecatol, plain_systems()[0]]).control(mecatol_planet, 0).exhaust(mecatol_planet);
    let mut rig = Rig::new(sc, Diplomacy::new(Seat(0), StrategyCard::DiplomacyCodex, DMode::Primary { base_printing: false }));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::ReadyPlanet), "Mecatol is not a legal choice; with no other system the ready part follows");
    let none = Rig::new(Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]), Diplomacy::new(Seat(0), StrategyCard::DiplomacyCodex, DMode::Secondary)).run().is_done();
    assert!(none, "nothing exhausted: nothing to do");
}

#[test]
fn diplomacy_smoke() {
    smoke("strategy_primaries/diplomacy", || Rig::new(diplomacy_table().0, Diplomacy::new(Seat(0), StrategyCard::DiplomacyCodex, DMode::Primary { base_printing: false })));
}

// ---- Politics ---------------------------------------------------------------------------------

fn politics_table() -> CoreState {
    let mut s = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheBaronyOfLetnev]).speaker(1).state();
    s.decks_mut().agenda = Deck::new(vec![Agenda::ALL[0], Agenda::ALL[1], Agenda::ALL[2], Agenda::ALL[3]]);
    s.decks_mut().action = Deck::new(vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2], ActionCard::ALL[3]]);
    s
}

fn politics(primary: bool) -> Rig<Politics> {
    Rig::from_parts(politics_table(), ti4_engine_core_tests::kernel_all(), Politics::new(Seat(0), primary, false))
}

#[test]
fn politics_primary_picks_a_new_speaker_draws_two_cards_and_puts_two_agendas_back() {
    covers!("rules::strategy_primaries::Politics");
    let mut rig = politics(true);
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::PoliticsSpeaker));
    assert_eq!(rig.opts(), vec![strat(SOpt::Speaker(Seat(0))), strat(SOpt::Speaker(Seat(2)))], "everyone but the speaker, in seating order");
    rig.pick_opt(strat(SOpt::Speaker(Seat(2))));
    assert_eq!(rig.state.meta().speaker, Seat(2));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[0], ActionCard::ALL[1]], "two action cards");
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::PoliticsAgenda { agenda: Agenda::ALL[0] }));
    assert_eq!(rig.opts(), vec![strat(SOpt::Top), strat(SOpt::Bottom)]);
    rig.pick_opt(strat(SOpt::Bottom));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::PoliticsAgenda { agenda: Agenda::ALL[1] }));
    rig.pick_opt(strat(SOpt::Top));
    assert!(rig.is_done());
    assert_eq!(rig.state.decks().agenda.cards(), &[Agenda::ALL[1], Agenda::ALL[2], Agenda::ALL[3], Agenda::ALL[0]], "the first went to the bottom, the second back on top");
}

#[test]
fn politics_secondary_only_draws_and_the_speaker_alone_cannot_be_changed() {
    covers!("rules::strategy_primaries::Politics");
    let mut rig = politics(false);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
    assert_eq!(rig.state.decks().agenda.len(), 4, "the agenda deck is not touched");
    assert_eq!(rig.state.meta().speaker, Seat(1));
    let alone = Rig::new(Scenario::new(&[Faction::TheEmbersOfMuaat]), Politics::new(Seat(0), true, false)).run().is_done();
    assert!(alone, "no deck, no other player: nothing is asked");
}

#[test]
fn politics_refuses_a_second_speaker_pick_and_a_placement_before_the_speaker() {
    covers!("rules::strategy_primaries::Politics");
    let mut rig = politics(true);
    rig.run();
    rig.expect_illegal(Pick::Opt(strat(SOpt::Speaker(Seat(1)))));
    rig.expect_illegal(Pick::Opt(strat(SOpt::Top)));
    rig.assert_options_sound();
}

#[test]
fn politics_smoke() {
    smoke("strategy_primaries/politics", || politics(true));
}

// ---- Imperial ---------------------------------------------------------------------------------

fn free_objective() -> PublicObjective {
    PublicObjective::ALL.into_iter().find(|o| objectives::cost_of(*o).is_none()).expect("an objective without a price")
}

fn imperial(mecatol: bool, scoreable: &[PublicObjective]) -> Rig<ImperialPrimary> {
    let m = ti4_engine_core::generated::System::MecatolRex18;
    let mut sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[m, plain_systems()[0]]);
    if mecatol {
        sc = sc.control(m.data().planets[0], 0);
    }
    let mut state = sc.state();
    state.decks_mut().secret = Deck::new(vec![ti4_engine_core::generated::SecretObjective::ALL[0], ti4_engine_core::generated::SecretObjective::ALL[1]]);
    let list: SmallList<PublicObjective, 16> = scoreable.iter().copied().collect();
    Rig::from_parts(state, ti4_engine_core_tests::kernel_all(), ImperialPrimary::new(Seat(0), list))
}

#[test]
fn imperial_scores_the_chosen_objective_then_the_mecatol_point_or_a_secret() {
    covers!("rules::strategy_primaries::ImperialPrimary");
    let o = free_objective();
    let mut rig = imperial(true, &[o]);
    rig.run();
    assert_eq!(rig.opts().last(), Some(&Opt::Decline), "the decline comes last");
    rig.pick_opt(strat(SOpt::Objective(o)));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.scored, Some(o));
    assert!(rig.state.objectives().scored[0].public.contains(o));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, i16::from(objectives::points_of(o)) + 1, "the objective and the Mecatol point");

    let mut off = imperial(false, &[o]);
    off.run().pick_opt(strat(SOpt::Objective(o)));
    assert_eq!(off.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, i16::from(objectives::points_of(o)), "without Mecatol: the objective only");
    assert_eq!(off.state.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 1, "and a secret objective instead of the point");
}

#[test]
fn imperial_declining_still_pays_the_mecatol_bonus_and_nothing_to_score_goes_straight_there() {
    covers!("rules::strategy_primaries::ImperialPrimary");
    let o = free_objective();
    let mut rig = imperial(true, &[o]);
    rig.run().decline();
    assert!(rig.is_done() && rig.driver.window.scored.is_none());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1);
    let mut none = imperial(true, &[]);
    none.run();
    assert!(none.is_done());
    assert_eq!(none.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1);
    let mut other = imperial(true, &[o]);
    other.run();
    other.expect_illegal(Pick::Opt(strat(SOpt::Objective(PublicObjective::ALL[PublicObjective::ALL.len() - 1]))));
}

#[test]
fn imperial_smoke() {
    smoke("strategy_primaries/imperial", || imperial(true, &[free_objective()]));
}

// ---- Thunder's Edge Warfare -------------------------------------------------------------------

#[test]
fn the_te_warfare_redistributes_then_picks_a_system_for_a_free_tactical_action() {
    covers!("rules::strategy_primaries::TeWarfare");
    let s = plain_systems();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_ring(7).tokens(0, 2, 2, 1);
    let mut rig = Rig::new(sc.clone(), TeWarfare::new(Seat(0), &[s[1], s[3]], false, Code(0)));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::TeWarfareRedistribute { total: 5, floor: 2 }));
    rig.pick_opt(strat(SOpt::Split { tactic: 3, fleet: 2, strategic: 0 }));
    assert_eq!(sheet(&rig.state, 0), (3, 2, 0));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::TeWarfareTactical));
    assert_eq!(rig.opts(), vec![strat(SOpt::System(s[1])), strat(SOpt::System(s[3]))]);
    rig.expect_illegal(Pick::Opt(strat(SOpt::System(s[2]))));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.chosen, Some(s[3]));
    let mut bare = Rig::new(Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]), TeWarfare::new(Seat(0), &[], false, Code(0)));
    bare.run();
    assert!(bare.is_done(), "no tokens and no system: nothing to ask");
    smoke("strategy_primaries/te_warfare", || Rig::new(sc.clone(), TeWarfare::new(Seat(0), &[s[1], s[3]], false, Code(0))));
}

#[test]
fn the_splits_helper_offers_every_distribution_at_or_above_the_floor() {
    let all: Vec<SOpt> = splits(4, 2).collect();
    assert_eq!(all.len(), 6);
    assert!(all.iter().all(|o| matches!(o, SOpt::Split { tactic, fleet, strategic } if tactic + fleet + strategic == 4 && *fleet >= 2)));
    assert_eq!(splits(0, 2).count(), 1, "nothing to split: the one empty split");
    assert_eq!(sheet_total(&Scenario::lobby(2).tokens(0, 1, 2, 3).state(), Seat(0)), 6);
    assert_eq!(sheet_total(&Scenario::lobby(2).state(), Seat(9)), 0);
}
