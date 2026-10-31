//! Lane M2: the production step as one held window (`driver::windows::production`), run through the live arm (`advance_production`): the start
//! effects of its three openers (`enter_production`, `resolve_timed`, `produce_by_ability`), the production window, and the effects at the end of a
//! use (`finish_use`), held as a payload on the `Pending` stack between the questions.

use std::rc::Rc;

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{ask, is_open, Env, Live, LiveError, Out};
use ti4_engine_core::driver::windows::production::{advance_production, ask_production, open_production, ProductionMachine, UseOpen};
use ti4_engine_core::generated::Agenda;
use ti4_engine_core::kernel::children::ProduceFacts;
use ti4_engine_core::kernel::stage::{AbilityTable, Held};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::keys::Outcome;
use ti4_engine_core::state::{Code, EventKind, Key, LeaderStatus, List, UsePhase, Val, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::rig::KitSeeds;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn planet() -> Planet {
    sys().data().planets[0]
}

/// Seat 0 (an Arborec) holds a space dock on the first planet of the system and `goods` trade goods.
fn arena(goods: i16) -> (CoreState, Kernel) {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .map_from(&[sys()])
        .control(planet(), 0)
        .unit(sys(), Some(planet()), Unit::SpaceDockI, 0, 1)
        .goods(0, goods)
        .tokens(0, 3, 3, 2)
        .build()
}

fn mkenv(k: &Kernel, seed: u64) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    ti4_engine_core_factions::self_assembly::register_links(&mut table);
    table.facts.roster = Some(Box::leak(Box::default()));
    table.codes.production.ability = Code(101);
    table.codes.production.production = Code(102);
    table.codes.production.units = Code(103);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(KitSeeds(seed)) }
}

fn goods(s: &CoreState) -> i16 {
    s.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

/// What a run of the live arm left: the questions in order, the events the window emitted, how it ended.
struct Played {
    asked: Vec<Choice>,
    events: Vec<EventKind>,
    end: Result<(), LiveError>,
}

fn pick_where(c: &Choice, f: &dyn Fn(&Opt) -> bool) -> Option<Pick> {
    c.opts.iter().find(|o| f(o)).map(|o| Pick::Opt(*o))
}

/// Run the open use to its end; each question is answered by `pol` (`None`: the first option).
fn play(core: &mut CoreState, env: &Env<'_>, pol: &mut dyn FnMut(&Choice) -> Option<Pick>) -> Played {
    let mut played = Played { asked: Vec::new(), events: Vec::new(), end: Ok(()) };
    let mut answer: Option<Pick> = None;
    for _ in 0..300 {
        match advance_production(core, env, answer.take()) {
            Err(e) => {
                played.end = Err(e);
                return played;
            }
            Ok((live, out)) => {
                played.events.extend(out.emitted.iter().map(|e| e.kind));
                match live {
                    Live::Asks(a) => {
                        answer = Some(pol(&a.choice).unwrap_or(Pick::Opt(a.choice.opts.as_slice()[0])));
                        played.asked.push(a.choice);
                    }
                    Live::Done => return played,
                }
            }
        }
    }
    panic!("the use does not end");
}

/// Build `unit` once (paying with trade goods), then stop.
fn build_then_stop(unit: Unit) -> impl FnMut(&Choice) -> Option<Pick> {
    let mut built = false;
    move |c: &Choice| match c.ask {
        Ask::Production(PAsk::Choose(_)) if !built => {
            built = true;
            pick_where(c, &|o| matches!(o, Opt::Production(POpt::Build { unit: u, .. }) if *u == unit))
        }
        Ask::Production(PAsk::Choose(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::Stop))),
        Ask::Production(PAsk::Pay(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))),
        _ => None,
    }
}

fn tactical() -> UseOpen {
    UseOpen::Tactical { who: Seat(0), system: sys() }
}

fn units_of(s: &CoreState, unit: Unit) -> usize {
    s.board().sys(sys()).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind() == unit).count()
}

#[test]
fn a_tactical_use_announces_itself_builds_and_announces_what_it_made() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    let seq = core.meta().production_seq;
    open_production(&mut core, tactical());
    let played = play(&mut core, &env, &mut build_then_stop(Unit::DestroyerI));
    assert_eq!(played.end, Ok(()));
    assert_eq!(core.meta().production_seq, seq + 1, "the use has a number");
    assert_eq!(played.events, [EventKind::ProductionUsed, EventKind::UnitsProduced], "announced when it starts and when it ended");
    assert_eq!(goods(&core), 5, "a destroyer costs one resource");
    assert_eq!(units_of(&core, Unit::DestroyerI), 1);
    assert!(!is_open::<ProductionMachine>(&core), "the window left the stack");
    assert!(matches!(played.asked[0].ask, Ask::Production(PAsk::Choose(_))), "the first question is what to build, the start effects asked nothing");
}

#[test]
fn units_produced_names_the_source_and_every_unit_with_its_place() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    open_production(&mut core, tactical());
    let mut announced = None;
    let mut answer = None;
    let mut pol = build_then_stop(Unit::DestroyerI);
    loop {
        let (live, out) = advance_production(&mut core, &env, answer.take()).expect("legal");
        announced = announced.or_else(|| out.emitted.iter().find(|e| e.kind == EventKind::UnitsProduced).cloned());
        match live {
            Live::Asks(a) => answer = Some(pol(&a.choice).unwrap_or(Pick::Opt(a.choice.opts.as_slice()[0]))),
            Live::Done => break,
        }
    }
    let rec = announced.expect("UNITS_PRODUCED was announced");
    assert_eq!(rec.payload.get(Key::Source), Some(Val::Code(Code(102))), "the PRODUCTION of a unit");
    assert_eq!(rec.payload.get(Key::Count), Some(Val::Int(1)));
    assert_eq!(rec.payload.get(Key::Player), Some(Val::Seat(Seat(0))));
    let Some(List::Produced(made)) = rec.payload.list(Key::Other(Code(103))) else { panic!("the units list") };
    assert_eq!(made.as_slice().len(), 1);
    assert_eq!(made.as_slice()[0].0, Unit::DestroyerI);
}

#[test]
fn a_system_with_no_capacity_has_no_effects_and_no_use_number() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[sys()]).control(planet(), 0).goods(0, 6).build();
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    let seq = core.meta().production_seq;
    open_production(&mut core, tactical());
    let (live, out) = advance_production(&mut core, &env, None).expect("runs");
    assert_eq!(live, Live::Done);
    assert!(out.emitted.is_empty(), "no PRODUCTION_USED for a system that produces nothing");
    assert_eq!(core.meta().production_seq, seq);
    assert_eq!(core.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Unlocked), "the hero is not offered");
}

#[test]
fn the_hacan_hero_is_offered_after_the_use_has_its_number_and_makes_every_unit_free() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    let seq = core.meta().production_seq;
    open_production(&mut core, tactical());
    let (live, _) = advance_production(&mut core, &env, None).expect("runs");
    let Live::Asks(first) = live else { panic!("the hero is offered") };
    assert!(matches!(first.choice.ask, Ask::Production(PAsk::Hero(_))), "{:?}", first.choice.ask);
    assert_eq!(core.meta().production_seq, seq, "the question was asked from a replayed step: the position is the one before it");
    let take = Pick::Opt(first.choice.opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::HeroUse))).unwrap());
    let (live, _) = advance_production(&mut core, &env, Some(take)).expect("legal");
    let Live::Asks(second) = live else { panic!("then what to build") };
    assert!(matches!(second.choice.ask, Ask::Production(PAsk::Choose(_))));
    assert_eq!(core.meta().production_seq, seq + 1, "the counter went up before the marker named it");
    let played = play(&mut core, &env, &mut build_then_stop(Unit::CarrierI));
    assert_eq!(played.end, Ok(()));
    assert_eq!(goods(&core), 6, "the carrier cost nothing");
    assert_eq!(core.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Purged));
}

#[test]
fn sarween_tools_takes_a_resource_off_the_bill() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SarweenTools);
    open_production(&mut core, tactical());
    let played = play(&mut core, &env, &mut build_then_stop(Unit::DestroyerI));
    assert_eq!(played.end, Ok(()));
    assert_eq!(goods(&core), 6, "the destroyer's one resource is covered by the discount");
}

#[test]
fn a_question_of_the_start_effects_is_a_nested_question_that_survives_a_clone() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    {
        let p = core.players_mut().by_seat_mut(Seat(0)).unwrap();
        p.tech.owned.insert(Technology::AIDevelopmentAlgorithm);
        p.tech.owned.insert(Technology::CruiserII);
    }
    open_production(&mut core, tactical());
    let (live, _) = advance_production(&mut core, &env, None).expect("runs");
    let Live::Asks(first) = live else { panic!("AI Development Algorithm asks") };
    assert!(matches!(first.choice.ask, Ask::Technology(_)), "{:?}", first.choice.ask);
    assert!(core.pending().nested.is_some(), "the question of a start effect is in the journal, not in the payload");
    let held = core.clone();
    let again = ask_production(&held, &env).expect("asks").expect("a question");
    assert_eq!(again.choice, first.choice, "the copy asks the same question");
    let exhaust = Pick::Opt(first.choice.opts.iter().copied().find(|o| matches!(o, Opt::Technology(_))).unwrap());
    let mut copy = held.clone();
    let (a, _) = advance_production(&mut core, &env, Some(exhaust)).expect("legal");
    let (b, _) = advance_production(&mut copy, &env, Some(exhaust)).expect("legal");
    assert_eq!(a, b);
    assert_eq!(core, copy, "a clone mid-window continues identically");
    assert!(core.pending().nested.is_none());
}

#[test]
fn a_clone_in_the_middle_of_the_window_continues_identically_and_the_payload_is_the_machine() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(9);
    let env = mkenv(&k, 7);
    open_production(&mut core, tactical());
    let mut pol = build_then_stop(Unit::DestroyerI);
    let mut answer: Option<Pick> = None;
    let mut questions = 0;
    loop {
        let (live, _) = advance_production(&mut core, &env, answer.take()).expect("legal");
        match live {
            Live::Asks(a) => {
                questions += 1;
                let machine = ProductionMachine::from_pending(&core, &k).expect("open");
                let mut again = core.clone();
                machine.to_pending(&mut again);
                assert_eq!(again, core, "to_pending(from_pending(c)) is the payload after question {questions}");
                let pick = pol(&a.choice).unwrap_or(Pick::Opt(a.choice.opts.as_slice()[0]));
                let mut copy = core.clone();
                let mut original = core.clone();
                let ra = play_rest(&mut original, &env, pick);
                let rb = play_rest(&mut copy, &env, pick);
                assert_eq!(ra, rb);
                assert_eq!(original, copy, "after question {questions}");
                answer = Some(pick);
            }
            Live::Done => break,
        }
    }
    assert!(questions >= 3, "a choice, a payment (or none), a second choice");
}

fn play_rest(core: &mut CoreState, env: &Env<'_>, pick: Pick) -> usize {
    let mut answer = Some(pick);
    let mut pol = build_then_stop(Unit::DestroyerI);
    let mut n = 0;
    loop {
        let (live, _) = advance_production(core, env, answer.take()).expect("legal");
        match live {
            Live::Asks(a) => {
                n += 1;
                answer = Some(pol(&a.choice).unwrap_or(Pick::Opt(a.choice.opts.as_slice()[0])));
            }
            Live::Done => return n,
        }
    }
}

#[test]
fn the_prophecy_of_ixth_is_discarded_by_a_use_that_made_fewer_than_two_fighters() {
    covers!("driver::windows::production::ProductionMachine");
    for (unit, kept) in [(Unit::DestroyerI, false), (Unit::FighterI, true)] {
        let (mut core, k) = arena(6);
        let env = mkenv(&k, 1);
        core.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(0)));
        open_production(&mut core, tactical());
        let played = play(&mut core, &env, &mut build_then_stop(unit));
        assert_eq!(played.end, Ok(()));
        assert_eq!(core.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), kept, "{unit:?}");
    }
    // Two purchases of fighters keep it as well (a fighter is bought two at a time).
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(0)));
    open_production(&mut core, tactical());
    let mut builds = 0;
    let mut pol = |c: &Choice| match c.ask {
        Ask::Production(PAsk::Choose(_)) if builds < 2 => {
            builds += 1;
            pick_where(c, &|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::FighterI, .. })))
        }
        Ask::Production(PAsk::Choose(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::Stop))),
        Ask::Production(PAsk::Pay(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))),
        _ => None,
    };
    let played = play(&mut core, &env, &mut pol);
    assert_eq!(played.end, Ok(()));
    let fighters = units_of(&core, Unit::FighterI);
    assert!(fighters >= 2, "two fighters stand ({fighters})");
    assert!(core.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), "two fighters keep the law");
}

#[test]
fn self_assembly_routines_offers_a_mech_after_a_use_that_made_something() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    open_production(&mut core, tactical());
    let mut pol = |c: &Choice| match c.ask {
        Ask::Technology(_) => pick_where(c, &|o| matches!(o, Opt::Technology(_))),
        _ => build_then_stop(Unit::DestroyerI)(c),
    };
    // `build_then_stop` is rebuilt per call above, so drive the build by hand.
    let mut built = false;
    let mut pol2 = |c: &Choice| match c.ask {
        Ask::Technology(_) => pick_where(c, &|o| matches!(o, Opt::Technology(_))),
        Ask::Production(PAsk::Choose(_)) if !built => {
            built = true;
            pick_where(c, &|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DestroyerI, .. })))
        }
        Ask::Production(PAsk::Choose(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::Stop))),
        Ask::Production(PAsk::Pay(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))),
        _ => None,
    };
    let _ = &mut pol;
    let played = play(&mut core, &env, &mut pol2);
    assert_eq!(played.end, Ok(()));
    assert!(played.asked.iter().any(|c| matches!(c.ask, Ask::Technology(_))), "the mech is offered at the end of the use");
    assert!(core.board().planet(planet()).units.as_slice().iter().any(|p| p.owner() == Seat(0) && p.kind().data().base_type == UnitBaseType::Mech));
    // A use that made nothing is offered nothing.
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    open_production(&mut core, tactical());
    let played = play(&mut core, &env, &mut |c: &Choice| pick_where(c, &|o| matches!(o, Opt::Production(POpt::Stop))));
    assert_eq!(played.end, Ok(()));
    assert!(!played.asked.iter().any(|c| matches!(c.ask, Ask::Technology(_))));
    assert!(!played.events.contains(&EventKind::UnitsProduced), "a use that made nothing announces nothing");
}

#[test]
fn a_table_without_the_factions_link_stops_the_use_with_a_named_gap() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let mut env = mkenv(&k, 1);
    env.table = {
        let mut t = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut t);
        t.facts.roster = Some(Box::leak(Box::default()));
        t
    };
    core.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    open_production(&mut core, tactical());
    let held = core.clone();
    let mut built = false;
    let mut pol = |c: &Choice| match c.ask {
        Ask::Production(PAsk::Choose(_)) if !built => {
            built = true;
            pick_where(c, &|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DestroyerI, .. })))
        }
        Ask::Production(PAsk::Choose(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::Stop))),
        Ask::Production(PAsk::Pay(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))),
        _ => None,
    };
    let played = play(&mut core, &env, &mut pol);
    assert_eq!(played.end, Err(LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::UnportedProduction(ti4_engine_core::kernel::ability::ProductionGap::NoLink))));
    let _ = held;
    // A table with no roster cannot name the buildable units.
    let (mut core, k) = arena(6);
    let mut env = mkenv(&k, 1);
    env.table.facts.roster = None;
    open_production(&mut core, tactical());
    let before = core.clone();
    assert_eq!(advance_production(&mut core, &env, None), Err(LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::UnportedProduction(ti4_engine_core::kernel::ability::ProductionGap::NoRoster))));
    assert_eq!(core, before, "a failed call restores the position");
}

#[test]
fn a_home_use_drops_the_stale_discount_announces_nothing_before_and_offers_the_hero() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.transient_mut().production.discount_remaining = 5;
    open_production(&mut core, UseOpen::Home { who: Seat(0), system: sys() });
    let played = play(&mut core, &env, &mut build_then_stop(Unit::DestroyerI));
    assert_eq!(played.end, Ok(()));
    assert_eq!(goods(&core), 5, "the stale discount did not make the destroyer free");
    assert_eq!(core.transient().production.discount_remaining, 0);
    assert_eq!(played.events, [EventKind::UnitsProduced], "no PRODUCTION_USED outside a tactical action's step");
    // The hero is offered when unlocked; the use counter goes up first.
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    open_production(&mut core, UseOpen::Home { who: Seat(0), system: sys() });
    let (live, _) = advance_production(&mut core, &env, None).expect("runs");
    let Live::Asks(a) = live else { panic!("the hero is offered") };
    assert!(matches!(a.choice.ask, Ask::Production(PAsk::Hero(_))));
}

#[test]
fn an_ability_use_announces_its_units_as_an_ability_and_offers_no_self_assembly() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    let mut buildable = [None; 9];
    buildable[0] = Some(Unit::DestroyerI);
    let facts = ProduceFacts { who: Seat(0), system: sys(), limit: 2, max_unit_cost: None, small_exemptions: 0, discount: 0, fleet_limit: 8, armada: 0, flagship_free: false, buildable };
    open_production(&mut core, UseOpen::Ability { facts });
    let seq = core.meta().production_seq;
    let mut announced = None;
    let mut answer = None;
    let mut pol = build_then_stop(Unit::DestroyerI);
    let mut asked_tech = false;
    loop {
        let (live, out): (Live, Out) = advance_production(&mut core, &env, answer.take()).expect("legal");
        announced = announced.or_else(|| out.emitted.iter().find(|e| e.kind == EventKind::UnitsProduced).cloned());
        match live {
            Live::Asks(a) => {
                asked_tech |= matches!(a.choice.ask, Ask::Technology(_));
                answer = Some(pol(&a.choice).unwrap_or(Pick::Opt(a.choice.opts.as_slice()[0])));
            }
            Live::Done => break,
        }
    }
    assert_eq!(core.meta().production_seq, seq, "an ability's units are no use of PRODUCTION");
    assert!(!asked_tech);
    assert_eq!(announced.expect("announced").payload.get(Key::Source), Some(Val::Code(Code(101))), "source: ability");
}

#[test]
fn a_planet_value_swap_lasts_one_use() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.transient_mut().production.value_swapped_planet = Some(planet());
    core.marks_mut().tail.edit().insert(ti4_engine_core::state::MarkKey::ProductionValueSwapSeq(), ti4_engine_core::state::MarkVal::Atom(ti4_engine_core::keys::Atom::Num(1)));
    open_production(&mut core, tactical());
    let played = play(&mut core, &env, &mut build_then_stop(Unit::DestroyerI));
    assert_eq!(played.end, Ok(()));
    assert_eq!(core.transient().production.value_swapped_planet, None);
    assert!(core.marks().tail.get(&ti4_engine_core::state::MarkKey::ProductionValueSwapSeq()).is_none(), "and the mark that declared it");
}

#[test]
fn an_answer_that_is_no_option_is_refused_and_a_window_that_is_not_open_is_named() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    assert_eq!(advance_production(&mut core, &env, None), Err(LiveError::NotOpen));
    open_production(&mut core, tactical());
    let (live, _) = advance_production(&mut core, &env, None).expect("runs");
    assert!(matches!(live, Live::Asks(_)));
    let held = core.clone();
    assert_eq!(advance_production(&mut core, &env, Some(Pick::Number(3))), Err(LiveError::Illegal));
    assert_eq!(core, held);
    assert_eq!(ProductionMachine::from_pending(&core, &k).unwrap().p.phase, UsePhase::Using);
    let asked = ask::<ProductionMachine>(&core, &env).unwrap().unwrap();
    assert!(matches!(asked.choice.ask, Ask::Production(PAsk::Choose(_))));
    assert!(core.pending().window(WindowKind::Production).is_some());
}

/// A use at the home system reads the hero's answer off the offer it ran (`HeroOffer::used`): the window is refreshed and every unit of the use is free.
/// (Regression: the finished offer was dropped by the tape runner, so a taken hero left the window at full price.)
#[test]
fn a_home_use_with_the_hero_taken_makes_every_unit_free() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    open_production(&mut core, UseOpen::Home { who: Seat(0), system: sys() });
    let mut built = false;
    let mut pol = |c: &Choice| match c.ask {
        Ask::Production(PAsk::Hero(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::HeroUse))),
        Ask::Production(PAsk::Choose(_)) if !built => {
            built = true;
            pick_where(c, &|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::CarrierI, .. })))
        }
        Ask::Production(PAsk::Choose(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::Stop))),
        Ask::Production(PAsk::Pay(_)) => pick_where(c, &|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))),
        _ => None,
    };
    let played = play(&mut core, &env, &mut pol);
    assert_eq!(played.end, Ok(()));
    assert!(!played.asked.iter().any(|c| matches!(c.ask, Ask::Production(PAsk::Pay(_)))), "nothing is paid");
    assert_eq!(goods(&core), 6, "the carrier cost nothing");
    assert_eq!(core.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Purged));
}

#[test]
fn a_home_use_offers_the_hero_only_to_a_unlocked_hero_with_capacity_and_ends_with_its_swap_cleared() {
    covers!("driver::windows::production::ProductionMachine");
    // A locked hero is not offered and the counter stays; an unlocked one goes up by exactly one.
    for (status, up) in [(LeaderStatus::Locked, 0), (LeaderStatus::Unlocked, 1)] {
        let (mut core, k) = arena(6);
        let env = mkenv(&k, 1);
        core.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, status);
        let seq = core.meta().production_seq;
        open_production(&mut core, UseOpen::Home { who: Seat(0), system: sys() });
        let (live, _) = advance_production(&mut core, &env, None).expect("runs");
        let Live::Asks(a) = live else { panic!("asks") };
        assert_eq!(matches!(a.choice.ask, Ask::Production(PAsk::Hero(_))), up == 1, "{status:?}");
        let answered = if up == 1 { Pick::Opt(Opt::Production(POpt::HeroUse)) } else { Pick::Opt(Opt::Production(POpt::Stop)) };
        advance_production(&mut core, &env, Some(answered)).expect("legal");
        assert_eq!(core.meta().production_seq, seq + up, "{status:?}");
    }
    // No capacity: the hero is not offered, the use ends at once, and a planet-value swap does not outlast the use.
    let (mut core, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[sys()]).control(planet(), 0).goods(0, 6).build();
    let env = mkenv(&k, 1);
    core.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    core.transient_mut().production.value_swapped_planet = Some(planet());
    let seq = core.meta().production_seq;
    open_production(&mut core, UseOpen::Home { who: Seat(0), system: sys() });
    let (live, _) = advance_production(&mut core, &env, None).expect("runs");
    assert_eq!(live, Live::Done);
    assert_eq!(core.meta().production_seq, seq);
    assert_eq!(core.transient().production.value_swapped_planet, None, "a home use clears the swap when it closes");
    // A tactical use of such a system asked nothing and ran no effect: the swap stays.
    let (mut core, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[sys()]).control(planet(), 0).build();
    let env = mkenv(&k, 1);
    core.transient_mut().production.value_swapped_planet = Some(planet());
    open_production(&mut core, tactical());
    advance_production(&mut core, &env, None).expect("runs");
    assert_eq!(core.transient().production.value_swapped_planet, Some(planet()));
}

#[test]
fn the_machine_reports_what_the_use_made_so_far() {
    covers!("driver::windows::production::ProductionMachine");
    let (mut core, k) = arena(6);
    let env = mkenv(&k, 1);
    open_production(&mut core, tactical());
    let (live, _) = advance_production(&mut core, &env, None).expect("runs");
    let Live::Asks(first) = live else { panic!() };
    assert!(ProductionMachine::from_pending(&core, &k).unwrap().made().as_slice().is_empty());
    let build = Pick::Opt(first.choice.opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DestroyerI, .. }))).unwrap());
    let (live, _) = advance_production(&mut core, &env, Some(build)).expect("legal");
    let mut live = live;
    while let Live::Asks(a) = &live {
        if matches!(a.choice.ask, Ask::Production(PAsk::Choose(_))) {
            break;
        }
        let pick = pick_where(&a.choice, &|o| matches!(o, Opt::Production(POpt::TradeGood { .. }))).unwrap_or(Pick::Opt(a.choice.opts.as_slice()[0]));
        live = advance_production(&mut core, &env, Some(pick)).expect("legal").0;
    }
    let made = ProductionMachine::from_pending(&core, &k).unwrap().made();
    assert_eq!(made.as_slice().iter().map(|(u, _)| *u).collect::<Vec<_>>(), [Unit::DestroyerI]);
}
