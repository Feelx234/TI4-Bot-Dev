//! Lane B, KP0 to KP4: the jobs of the turn flow (`driver::turn`) run the way the live loop runs them: a held window, `advance` to a question
//! or the end, a clone of the position standing for a checkpoint. The lockstep against the string bodies is `ti4-engine`'s
//! `turn_live_tests`; these tests pin what the jobs write, announce and refuse, without an engine.

use std::rc::Rc;

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::choice::technology::{Ask as TAsk, Opt as TOpt, StartRule};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, ask, is_open, Env, Live, LiveError};
use ti4_engine_core::driver::turn::{advance_announce, advance_job, advance_phase, advance_prelude, advance_reconcile, TurnFlowWin};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::turn_codes::TurnGap;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Outcome, Word};
use ti4_engine_core::state::{Emit, EventKind, MetaFlags, PlainArg, PlainKind, PlainRec, StartingTechPick, StartingTechSource, TurnJob};
use ti4_engine_core_tests::prelude::*;

fn env(k: &Kernel) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

fn plain(s: &CoreState) -> Vec<PlainRec> {
    s.ledger.events.iter().filter_map(|e| e.plain().copied()).collect()
}

fn typed(s: &CoreState) -> Vec<EventKind> {
    s.ledger.events.iter().filter_map(|e| if let Emit::Typed(t) = e { Some(t.kind) } else { None }).collect()
}

fn gap(why: TurnGap) -> Result<(Live, ti4_engine_core::driver::live::Out), LiveError> {
    Err(LiveError::Unknown(Unknown::TurnGap(why)))
}

/// The eight cards of the Prophecy of Kings set on the mat and in the initiative table, a card dealt to each seat in `dealt`.
fn mat(s: &mut CoreState) {
    let cards = [StrategyCard::Leadership, StrategyCard::DiplomacyCodex, StrategyCard::Politics, StrategyCard::Construction, StrategyCard::Trade, StrategyCard::Warfare, StrategyCard::Technology, StrategyCard::Imperial];
    for (i, c) in cards.iter().enumerate() {
        s.strategy_mut().initiative.insert(*c, i as i32 + 1);
    }
    ti4_engine_core::rules::strategy_flow::lay_out_mat(s, &cards);
}

// -- the start of a turn --------------------------------------------------------------------------------------------------------

#[test]
fn the_start_of_a_turn_prepares_it_and_announces_it() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.meta_mut().turn_seq = 4;
    let (live, _) = advance_prelude(&mut s, &env(&k), None).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(s.pending().prepared_turn_seq, Some(4), "the turn is the prepared one");
    assert_eq!(typed(&s), [EventKind::TurnBegan]);
    assert!(!is_open::<TurnFlowWin>(&s));
}

#[test]
fn a_spec_ops_unit_waiting_on_the_card_is_placed_at_the_start_of_the_turn_and_announced() {
    let (planet, home) = {
        let p = Planet::ALL.iter().copied().find(|p| p.data().system.is_some()).unwrap();
        (p, p.data().system.unwrap())
    };
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).control(planet, 0).build();
    s.players_mut().edit(0).home_system = Some(home);
    s.players_mut().edit(0).misc.spec_ops_card = 2;
    advance_prelude(&mut s, &env(&k), None).unwrap();
    assert_eq!(s.board().planet(planet).units.len(), 2);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().misc.spec_ops_card, 0);
    assert_eq!(plain(&s), [PlainRec::two(PlainKind::SpecOpsReturned, PlainArg::Seat(Seat(0)), PlainArg::Count(2))]);
}

#[test]
fn psychoarchaeology_asks_for_a_ready_specialty_planet_and_a_clone_continues_identically() {
    let planet = Planet::ALL.iter().copied().find(|p| !p.data().specialties.is_empty() && p.data().system.is_some()).unwrap();
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).control(planet, 0).tech(0, Technology::Psychoarchaeology).build();
    let e = env(&k);
    let (live, _) = advance_prelude(&mut s, &e, None).unwrap();
    let Live::Asks(q) = live else { panic!("the planet is offered") };
    assert_eq!(q.choice.ask, Ask::Technology(TAsk::Psychoarchaeology));
    assert!(s.pending().prepared_turn_seq.is_none(), "nothing of a half-run job is kept");
    let mut fork = s.clone();
    assert_eq!(ask::<TurnFlowWin>(&fork, &e).unwrap().unwrap().choice, q.choice, "a replay of the suspension asks the same question");
    let exhaust = q.choice.pick(0).unwrap();
    for state in [&mut s, &mut fork] {
        let (live, _) = advance::<TurnFlowWin>(state, &e, Some(exhaust)).unwrap();
        assert_eq!(live, Live::Done);
    }
    assert_eq!(s, fork, "a clone mid-window continues identically");
    assert!(s.board().planet(planet).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED));
    assert_eq!(s.pending().staged.len(), 1, "the trade good is staged for the next step's announcement");
    assert_eq!(s.pending().prepared_turn_seq, Some(0));
}

#[test]
fn a_start_of_turn_the_core_cannot_follow_is_a_counted_gap_and_leaves_the_position_alone_and_a_locked_commander_is_judged() {
    // Chaos Mapping needs the unit roster of the play facts (the forms the seat may produce).
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).tech(0, Technology::ChaosMapping).build();
    let held = s.clone();
    assert_eq!(advance_prelude(&mut s, &env(&k), None), gap(TurnGap::NoRoster));
    assert_eq!(s, held);
    // A locked commander of a faction whose unlock condition a hook states (lane F2) is judged at the start of the turn, not left to the string: it stays
    // locked here (the Arborec condition is not met) and the prelude goes on.
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).build();
    let commander = Leader::ALL.iter().copied().find(|l| l.data().faction == FactionTag::Arborec && l.data().kind == LeaderType::Commander).unwrap();
    s.players_mut().edit(0).leaders.status.insert(commander, ti4_engine_core::state::LeaderStatus::Locked);
    let (live, _) = advance_prelude(&mut s, &env(&k), None).expect("the commander is judged, not a gap");
    assert_eq!(live, Live::Done);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&commander), Some(&ti4_engine_core::state::LeaderStatus::Locked));
    assert_eq!(s.pending().prepared_turn_seq, Some(0));
}

// -- the phases -----------------------------------------------------------------------------------------------------------------

fn strategy_end() -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Strategy).active(None).build();
    mat(&mut s);
    for (i, c) in [StrategyCard::Trade, StrategyCard::Leadership, StrategyCard::Imperial].iter().enumerate() {
        ti4_engine_core::rules::strategy_flow::take_card(&mut s, Seat(i as u8), *c);
    }
    (s, k)
}

#[test]
fn the_strategy_phase_ends_into_the_action_phase_with_the_first_seat_in_initiative_order() {
    let (mut s, k) = strategy_end();
    let (live, _) = advance_phase(&mut s, &env(&k), None).unwrap();
    assert_eq!(live, Live::Done);
    // Seat 1 holds Leadership (initiative 1).
    assert_eq!((s.meta().phase, s.meta().active, s.meta().turn_seq), (Phase::Action, Some(Seat(1)), 1));
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::ActionPhaseBegan)]);
    assert_eq!(typed(&s), [EventKind::StrategyPhaseEnded]);
    // The cards nobody took gathered a trade good each (83.4); the cards taken keep what they had.
    assert_eq!(s.strategy().goods.get(&StrategyCard::Politics).copied(), Some(1));
}

#[test]
fn the_action_phase_ends_into_the_status_phase_only_when_everyone_passed() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(None).build();
    let held = s.clone();
    assert_eq!(advance_phase(&mut s, &env(&k), None), gap(TurnGap::PhaseNotReady), "the string reports MissingActivePlayer");
    assert_eq!(s, held);
    for i in 0..3 {
        s.players_mut().edit(i).econ.passed = true;
    }
    advance_phase(&mut s, &env(&k), None).unwrap();
    assert_eq!((s.meta().phase, s.meta().active), (Phase::Status, None));
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::StatusPhaseBegan)]);
    assert_eq!(typed(&s), [EventKind::StatusPhaseBegan]);
}

#[test]
fn the_status_phase_ends_into_the_agenda_phase_after_the_custodians_and_into_the_next_round_before() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Status).active(None).build();
    s.meta_mut().flags.set(MetaFlags::CUSTODIANS_REMOVED, true);
    advance_phase(&mut s, &env(&k), None).unwrap();
    assert_eq!(s.meta().phase, Phase::Agenda);
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::AgendaPhaseBegan)]);
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Status).active(None).build();
    mat(&mut s);
    s.pending_mut().driver.set(ti4_engine_core::state::DriverFlags::STATUS_RESOLVED, true);
    advance_phase(&mut s, &env(&k), None).unwrap();
    assert_eq!((s.meta().phase, s.meta().round), (Phase::Strategy, 2));
    assert_eq!(s.strategy().unclaimed.len(), 8, "the mat is laid out again from the initiative table");
    assert!(!s.pending().driver.has(ti4_engine_core::state::DriverFlags::STATUS_RESOLVED), "the round's flags start again");
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::RoundBegan)]);
    assert_eq!(typed(&s), [EventKind::StrategyPhaseBegan]);
}

#[test]
fn the_revolution_levies_one_planet_per_technology_at_the_start_of_the_next_strategy_phase() {
    let planets: Vec<Planet> = Planet::ALL.iter().copied().filter(|p| p.data().system.is_some()).take(3).collect();
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Status).active(None).control(planets[0], 0).control(planets[1], 0).control(planets[2], 0).tech(0, Technology::Psychoarchaeology).tech(0, Technology::AntimassDeflectors).build();
    mat(&mut s);
    s.agenda_mut().laws.insert(Agenda::AntiIntellectualRevolution, Outcome::Against);
    advance_phase(&mut s, &env(&k), None).unwrap();
    let exhausted = planets.iter().filter(|p| s.board().planet(**p).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED)).count();
    assert_eq!(exhausted, 2, "two technologies, two planets");
}

#[test]
fn the_first_strategy_phase_announces_itself_once() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Strategy).active(None).build();
    advance_announce(&mut s, &env(&k), None).unwrap();
    assert!(s.pending().driver.has(ti4_engine_core::state::DriverFlags::STRATEGY_PHASE_ANNOUNCED));
    assert_eq!(typed(&s), [EventKind::StrategyPhaseBegan]);
}

// -- the draft ------------------------------------------------------------------------------------------------------------------

#[test]
fn the_draft_asks_the_picker_for_a_card_and_the_pick_pays_the_goods_on_it() {
    use ti4_engine_core::driver::turn::ask::{advance_ask, ask_ask};
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Strategy).active(None).build();
    mat(&mut s);
    s.meta_mut().strategy_cards_per_player = 2;
    s.strategy_mut().goods.insert(StrategyCard::Trade, 3);
    let e = env(&k);
    let q = ask_ask(&s, &e).unwrap().expect("the speaker picks").choice;
    assert_eq!((q.actor, q.ask), (Seat(0), Ask::Strategy(SAsk::Draft)));
    assert_eq!(q.opts.len(), 8);
    let pick = Pick::Opt(Opt::Strategy(SOpt::Card(StrategyCard::Trade)));
    let (live, _) = advance_ask(&mut s, &e, Some(pick)).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().strat.as_slice(), &[StrategyCard::Trade]);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3);
    assert_eq!(s.strategy().unclaimed.len(), 7);
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::StrategyCardChosen)]);
    assert_eq!(s.pending().staged.len(), 1, "the gain is announced by the next step");
    assert_eq!(s.transient().misc.last_strategy_choice, Some((Seat(0), StrategyCard::Trade)));
    // The next picker is the next seat; a card no longer on the mat is no option.
    let q = ask_ask(&s, &e).unwrap().unwrap().choice;
    assert_eq!(q.actor, Seat(1));
    let held = s.clone();
    assert_eq!(advance_ask(&mut s, &e, Some(pick)), Err(LiveError::Illegal));
    assert_eq!(s, held);
}

#[test]
fn the_end_of_the_draft_offers_the_arbiter_swap_where_one_could_be_made() {
    use ti4_engine_core::choice::agenda::Ask as AAsk;
    use ti4_engine_core::driver::turn::ask::advance_ask;
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Strategy).active(None).build();
    mat(&mut s);
    s.meta_mut().strategy_cards_per_player = 1;
    s.strategy_mut().unclaimed = [StrategyCard::Imperial].into_iter().collect();
    ti4_engine_core::rules::strategy_flow::deal(&mut s, Seat(0), StrategyCard::Leadership);
    let mut arbiter = s.clone();
    arbiter.agenda_mut().laws.insert(Agenda::ImperialArbiter, Outcome::Seat(Seat(0)));
    let pick = Pick::Opt(Opt::Strategy(SOpt::Card(StrategyCard::Imperial)));
    let e = env(&k);
    // Without the law the draft simply ends.
    let (live, _) = advance_ask(&mut s, &e, Some(pick)).unwrap();
    assert_eq!(live, Live::Done);
    assert!(ti4_engine_core::rules::strategy::draft(&s).is_none());
    // With Imperial Arbiter elected and a card on both sides, the owner is asked whether to discard it.
    let (live, _) = advance_ask(&mut arbiter, &e, Some(pick)).unwrap();
    let Live::Asks(q) = live else { panic!("the owner is offered the swap") };
    assert!(matches!(q.choice.ask, Ask::Agenda(AAsk::DiscardLaw { law: Agenda::ImperialArbiter, .. })));
    assert_eq!(q.choice.actor, Seat(0));
    assert!(arbiter.agenda().laws.contains_key(&Agenda::ImperialArbiter), "nothing of a half-run job is kept");
}

// -- the starting technology ----------------------------------------------------------------------------------------------------

/// An `Env` whose play facts hold the unit roster of the sources (the step needs it to upgrade the units a technology improves).
fn env_with_roster(k: &Kernel) -> Env<'_> {
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let mut e = env(k);
    e.table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(move |core, _| {
        Some(PlayFacts {
            scope: 0,
            blanked: false,
            action_card: ti4_engine_core::state::Code(0),
            supply_base: ti4_engine_core::state::Code(0),
            note_order: |a, b| a.cmp(&b),
            seats: Vec::new(),
            name_group: Vec::new(),
            has_effect: Vec::new(),
            skimmer: false,
            roster: Some(roster),
            rules_version: core.meta().rules_version,
            tech_order: |a, b| a.cmp(&b),
            shared: SharedPlay::default(),
        })
    })));
    e
}

#[test]
fn a_listed_starting_technology_is_asked_one_at_a_time_and_the_last_one_left_is_given() {
    use ti4_engine_core::driver::turn::ask::{advance_ask, ask_ask};
    let list = ti4_engine_core::state::TechList::from_slice(&[Technology::NeuralMotivator, Technology::SarweenTools, Technology::PlasmaScoring]).unwrap();
    let (mut s, k) = Scenario::new(&[Faction::TheArgentFlight, Faction::TheFederationOfSol]).phase(Phase::Strategy).active(None).build();
    s.transient_mut().misc.starting_tech_picks.push(StartingTechPick { player: Seat(0), of: 2, remaining: 2, source: StartingTechSource::Listed(list) });
    let e = env_with_roster(&k);
    let q = ask_ask(&s, &e).unwrap().unwrap().choice;
    assert_eq!(q.ask, Ask::Technology(TAsk::StartingTech { number: 1, of: 2, rule: StartRule::Listed }));
    assert_eq!(q.opts.len(), 3);
    // An answer that is not offered is refused and changes nothing.
    let held = s.clone();
    assert_eq!(advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::Tech(Technology::Psychoarchaeology))))), Err(LiveError::Illegal));
    assert_eq!(s, held);
    let first = Pick::Opt(Opt::Technology(TOpt::Tech(Technology::SarweenTools)));
    let mut fork = s.clone();
    for state in [&mut s, &mut fork] {
        assert_eq!(advance_ask(state, &e, Some(first)).unwrap().0, Live::Done);
    }
    assert_eq!(s, fork, "a clone continues identically");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::SarweenTools));
    assert_eq!(plain(&s), [PlainRec::bare(PlainKind::StartingTechnologyChosen)]);
    // One pick left, two technologies on the list: still a decision.
    let q = ask_ask(&s, &e).unwrap().unwrap().choice;
    assert_eq!(q.ask, Ask::Technology(TAsk::StartingTech { number: 2, of: 2, rule: StartRule::Listed }));
    assert_eq!(q.opts.len(), 2, "what the seat owns is no longer offered");
    advance_ask(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::Tech(Technology::NeuralMotivator))))).unwrap();
    assert!(s.transient().misc.starting_tech_picks.is_empty(), "the picks are spent");
    assert!(ask_ask(&s, &e).is_err() || ask_ask(&s, &e).unwrap().is_none());
}

#[test]
fn a_pick_with_one_technology_left_to_give_is_settled_without_a_decision_by_the_reconcile() {
    let list = ti4_engine_core::state::TechList::from_slice(&[Technology::NeuralMotivator, Technology::SarweenTools]).unwrap();
    let (mut s, k) = Scenario::new(&[Faction::TheArgentFlight, Faction::TheFederationOfSol]).phase(Phase::Strategy).active(None).build();
    s.transient_mut().misc.starting_tech_picks.push(StartingTechPick { player: Seat(0), of: 2, remaining: 2, source: StartingTechSource::Listed(list) });
    let (live, _) = advance_reconcile(&mut s, &env_with_roster(&k), None).unwrap();
    assert_eq!(live, Live::Done);
    let owned = &s.players().by_seat(Seat(0)).unwrap().tech.owned;
    assert!(owned.contains(Technology::NeuralMotivator) && owned.contains(Technology::SarweenTools), "as many options as picks: both are given");
    assert!(s.transient().misc.starting_tech_picks.is_empty());
}

// -- the reconcile hooks --------------------------------------------------------------------------------------------------------

#[test]
fn the_nexus_latch_is_set_by_a_unit_in_the_locked_nexus_or_a_controlled_mallice_and_stays_set() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).build();
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(!s.meta().flags.has(MetaFlags::NEXUS_UNLOCKED));
    s.board_mut().planet_mut(Planet::Mallice).control = Some(Seat(1));
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(s.meta().flags.has(MetaFlags::NEXUS_UNLOCKED));
    s.board_mut().planet_mut(Planet::Mallice).control = None;
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(s.meta().flags.has(MetaFlags::NEXUS_UNLOCKED), "sticky");
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).unit(System::MalliceLockedNexus, None, Unit::CruiserI, 1, 1).build();
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(s.meta().flags.has(MetaFlags::NEXUS_UNLOCKED), "a unit in the nexus opens it");
}

/// Lane S: the wormholes that came from tokens (a gamma token of Gamma Wormhole, the Creuss tokens, the open Nexus, the ion storm's face) join the core's
/// map at the reconcile every step starts with, and leave it with the token, as the string rebuilds `Galaxy::token_wormholes` each step. Before, the map kept
/// what it was loaded with: Signal Jamming's reach and the routes of a move never saw a token the game placed (sol, 5 players, seed 1: `82a` missing).
#[test]
fn the_wormholes_the_tokens_give_join_the_map_at_the_reconcile_and_leave_with_the_token() {
    use ti4_engine_core::hex::Hex;
    use ti4_engine_core::ids::system_at;
    let plain = ti4_engine_core_tests::scenario::plain_systems();
    let (a, b, nexus) = (plain[0], plain[1], System::MalliceLockedNexus);
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).map_hexes(&[(a, Hex::new(0, 0)), (b, Hex::new(4, 0)), (nexus, Hex::new(-4, 0))]).build();
    let next_to = |s: &CoreState, x: System, y: System| {
        s.galaxy().unwrap().adjacent(system_at(x as usize)).contains(y as usize) && s.map().adjacency().unwrap().of(system_at(x as usize)).contains(y as usize)
    };
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(!next_to(&s, a, nexus) && !next_to(&s, b, nexus), "far apart, and the locked nexus holds only its gamma wormhole");
    // A gamma token joins the system to the nexus (a gamma wormhole), in both directions, in the map and in the adjacency the rules read.
    s.explore_mut().wormhole_tokens.insert(Wormhole::Gamma, a);
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(next_to(&s, a, nexus) && next_to(&s, nexus, a), "the gamma token joined the gamma wormhole of the nexus");
    assert!(!next_to(&s, b, nexus));
    // The token leaves (a tile replaced by Nova Seed): so does the link.
    s.explore_mut().wormhole_tokens.remove(&Wormhole::Gamma);
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(!next_to(&s, a, nexus), "a token that is gone links nothing");
    // The open nexus has an alpha and a beta wormhole: an alpha token joins it, a gamma one no longer matters.
    s.explore_mut().wormhole_tokens.insert(Wormhole::Alpha, b);
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(!next_to(&s, b, nexus), "the nexus is still locked");
    s.board_mut().planet_mut(Planet::Mallice).control = Some(Seat(1));
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(next_to(&s, b, nexus), "the open nexus holds an alpha wormhole");
    // The ion storm's face is a wormhole too, and it turns over.
    s.explore_mut().wormhole_tokens.clear();
    s.meta_mut().ion_storm = Some((a, Wormhole::Beta));
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(next_to(&s, a, nexus) && !next_to(&s, b, nexus), "the storm shows a beta wormhole, which the open nexus holds");
    s.meta_mut().ion_storm = None;
    advance_reconcile(&mut s, &env(&k), None).unwrap();
    assert!(!next_to(&s, a, nexus), "no storm, no wormhole");
}

#[test]
fn spec_ops_destroyed_roll_a_die_each_and_the_survivors_wait_on_the_card() {
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).build();
    s.players_mut().edit(0).misc.spec_ops_destroyed = 5;
    s.rng = ti4_engine_core::state::Rng::new(3);
    let mut e = env(&k);
    e.seeds = Rc::new(ti4_engine_core_tests::rig::KitSeeds(7));
    advance_reconcile(&mut s, &e, None).unwrap();
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.misc.spec_ops_destroyed, 0);
    let lines = plain(&s);
    assert_eq!(lines.len(), 5, "one line per die");
    let survived = lines.iter().filter(|l| l.kind == PlainKind::SpecOpsSurvived).count();
    assert_eq!(u32::try_from(survived).unwrap(), p.misc.spec_ops_card);
    assert_eq!(s.ledger.dice.len(), 1, "one roll of five dice is recorded");
    // The same position and seeds roll the same faces.
    let (mut again, _) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).build();
    again.players_mut().edit(0).misc.spec_ops_destroyed = 5;
    again.rng = ti4_engine_core::state::Rng::new(3);
    advance_reconcile(&mut again, &e, None).unwrap();
    assert_eq!(plain(&again), lines);
}

#[test]
fn a_gain_nobody_announced_is_announced_once_and_the_first_look_announces_nothing() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Action).build();
    let mut e = env(&k);
    e.table.facts.staging = true;
    advance_reconcile(&mut s, &e, None).unwrap();
    assert!(typed(&s).is_empty(), "the first look at the technologies records the starting set");
    assert!(s.marks().tail.contains_key(&MarkKey::PrivateSeen(Word::Technologies, Seat(0))));
    s.players_mut().edit(0).tech.owned.insert(Technology::SarweenTools);
    advance_reconcile(&mut s, &e, None).unwrap();
    assert_eq!(typed(&s), [EventKind::TechnologyGained]);
    advance_reconcile(&mut s, &e, None).unwrap();
    assert_eq!(typed(&s), [EventKind::TechnologyGained], "announced once");
    assert_eq!(
        s.marks().tail.get(&MarkKey::PrivateSeen(Word::Technologies, Seat(0))),
        Some(&MarkVal::List(vec![MarkVal::Atom(Atom::Tech(Technology::SarweenTools))]))
    );
    // Without a faction module at the table nothing is announced (nothing listens).
    let (mut quiet, k) = Scenario::lobby(2).phase(Phase::Action).build();
    quiet.players_mut().edit(0).tech.owned.insert(Technology::SarweenTools);
    advance_reconcile(&mut quiet, &env(&k), None).unwrap();
    assert!(typed(&quiet).is_empty());
}

#[test]
fn a_recorded_map_edit_and_a_pending_propagation_are_counted_gaps() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Action).build();
    s.marks_mut().tail.edit().insert(MarkKey::MapEdit(0), MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Swap)), MarkVal::Atom(Atom::System(System::ALL[0])), MarkVal::Atom(Atom::System(System::ALL[1]))]));
    let held = s.clone();
    assert_eq!(advance_reconcile(&mut s, &env(&k), None), gap(TurnGap::MapEdits));
    assert_eq!(s, held);
    let (mut s, k) = Scenario::new(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]).phase(Phase::Action).build();
    s.marks_mut().tail.edit().insert(MarkKey::NekroPropagation(Seat(0)), MarkVal::Atom(Atom::Num(1)));
    let mut e = env(&k);
    e.table.facts.staging = true;
    // Propagation is announced as an event of the table (the Nekro rule settles it): the step no longer stops.
    assert!(advance_reconcile(&mut s, &e, None).is_ok());
}

#[test]
fn a_job_that_is_not_open_refuses_an_answer() {
    let (mut s, k) = Scenario::lobby(2).phase(Phase::Action).build();
    assert_eq!(advance_job(TurnJob::End, &mut s, &env(&k), Some(Pick::Number(1))), Err(LiveError::NotOpen));
}
