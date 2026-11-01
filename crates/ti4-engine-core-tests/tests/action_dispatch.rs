//! Lane F2b (package KM2): the options of the action menu the core runs -- tactical, strategic, a transaction, an action card -- chosen from the
//! menu question: what the step writes, Extreme Duress, the verdict events, the failed-action withholding, and the counted gap that names an option
//! no window runs yet. (The lockstep against the string is `ti4-engine`'s `action_dispatch_live_tests`.)

use std::rc::Rc;

use ti4_engine_core::choice::turn::Opt as TuOpt;
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{Env, Live, LiveError, Out};
use ti4_engine_core::driver::turn::ask::{advance_ask, ask_ask};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::turn_codes::{MenuGap, TurnGap};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{Emit, EventKind, PlainKind, PlainRec, WindowKind};
use ti4_engine_core_tests::prelude::*;

fn env(k: &Kernel) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

/// [`env`] whose play facts hold the unit roster (the menu sources and the finish of an action need it).
fn env_r(k: &Kernel) -> Env<'_> {
    env_roster(k, Box::leak(Box::default()))
}

/// [`env_r`] over a roster that gives the forms seats place.
fn env_roster<'a>(k: &'a Kernel, roster: &'static ti4_engine_core::query::tech::UnitRoster) -> Env<'a> {
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
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


fn plain(s: &CoreState) -> Vec<PlainKind> {
    s.ledger.events.iter().filter_map(|e| e.plain().map(|p: &PlainRec| p.kind)).collect()
}

fn typed(s: &CoreState, kind: EventKind) -> usize {
    s.ledger.events.iter().filter(|e| matches!(e, Emit::Typed(t) if t.kind == kind)).count()
}

fn pick(s: &mut CoreState, e: &Env<'_>, want: impl Fn(&Opt) -> bool) -> Result<(Live, Out), LiveError> {
    let q = ask_ask(s, e).unwrap().unwrap().choice;
    let opt = q.opts.as_slice().iter().copied().find(|o| want(o)).expect("the option is on the menu");
    advance_ask(s, e, Some(Pick::Opt(opt)))
}

fn with_map(seats: usize) -> (CoreState, Kernel) {
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    Scenario::lobby(seats).phase(Phase::Action).active(Some(0)).map_ring(1).unit(system, None, Unit::CruiserI, 0, 1).tokens(0, 3, 3, 2).build()
}

#[test]
fn a_tactical_action_chosen_from_the_menu_opens_the_tactical_window_and_announces_it() {
    let (mut s, k) = with_map(3);
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Tactical))).unwrap();
    assert!(s.pending().window(WindowKind::Tactical).is_some(), "the activation question is the next step's");
    assert!(plain(&s).contains(&PlainKind::TacticalActionBegan));
    assert_eq!(s.pending().window(WindowKind::TurnFlow), None, "the job is gone");
}

#[test]
fn extreme_duress_is_settled_before_a_tactical_action_begins() {
    let (mut s, k) = with_map(3);
    s.players_mut().edit(0).econ.duress_by = Some(Seat(1));
    s.players_mut().edit(0).econ.trade_goods = 3;
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Tactical))).unwrap();
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0, "the goods went to the holder");
    assert!(s.players().by_seat(Seat(1)).unwrap().econ.trade_goods >= 3);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.duress_by, None);
    let kinds = plain(&s);
    let at = |kind| kinds.iter().position(|k| *k == kind).expect("announced");
    assert!(at(PlainKind::ExtremeDuress) < at(PlainKind::TacticalActionBegan), "the punishment first");
}

#[test]
fn a_strategic_action_chosen_from_the_menu_stops_nowhere_and_opens_the_follower_window() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    let card = ti4_engine_core::generated::StrategyCard::ALL[0];
    s.players_mut().edit(0).strat.push(card);
    let e = env_r(&k);
    let result = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Strategic { .. })));
    let outcome = result.as_ref().map(|r| matches!(r.0, Live::Asks(_)));
    assert!(!matches!(result, Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::Menu(MenuGap::Unrouted(..)))))), "the dispatch is native: {result:?}");
    assert_eq!(typed(&s, EventKind::StrategicActionBegan), 1, "announced once");
    assert!(s.pending().window(WindowKind::StrategySecondary).is_some() || outcome == Ok(true), "the follower window stands");
}

#[test]
fn an_option_no_window_runs_yet_is_a_gap_that_names_it_and_leaves_the_position() {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build();
    s.players_mut().edit(0).leaders.status.insert(ti4_engine_core::generated::Leader::DarktalonTreilla, ti4_engine_core::state::LeaderStatus::Unlocked);
    let e = env_r(&k);
    let held = s.clone();
    let result = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(_))));
    assert!(matches!(result, Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::Menu(MenuGap::Unrouted(5, _)))))), "{result:?}");
    assert_eq!(s, held, "nothing was written");
}

/// Found by the shadow behind a real game of ghost/argent/cabal/empyrean (lane F2b): the Trade primary of the strategic action replenished without
/// telling the table, while the string stages Cabal's `COMMODITIES_REPLENISHED` (and the Trade Agreement's gain). The window now replenishes through
/// `fx::replenish_announced`.
#[test]
fn the_trade_primary_stages_cabals_replenish_event_like_the_string() {
    use ti4_engine_core::driver::{Driver, Status};
    use ti4_engine_core::kernel::Resolver;
    use ti4_engine_core::rules::strategy_windows::TradePrimary;
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheVuilraithCabal]).phase(Phase::Action).active(Some(0)).build();
    s.players_mut().edit(0).econ.commodities = 0;
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    let mut d = Driver::new(TradePrimary::new(Seat(0)), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table, 1);
    let status = d.run(&mut s, &k).unwrap();
    assert!(matches!(status, Status::Asking(_) | Status::Done));
    assert!(s.pending().staged.iter().any(|e| e.kind == EventKind::CommoditiesReplenished), "{:?}", s.pending().staged.iter().map(|e| e.kind).collect::<Vec<_>>());
    // Without a Cabal at the table nothing is staged.
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).build();
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    let mut d = Driver::new(TradePrimary::new(Seat(0)), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table, 1);
    d.run(&mut s, &k).unwrap();
    assert!(s.pending().staged.is_empty());
}

/// KM2 section 8: the second action out of the closing menu (Fleet Logistics). Anything that takes an action closes the closing menu (and
/// clears the negotiations noted for the first action); the free options leaving it open is lane F's lockstep.
#[test]
fn an_action_taken_from_the_closing_menu_is_the_second_action_and_ends_the_closing_menu() {
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let build = || {
        let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_ring(1).unit(system, None, Unit::CruiserI, 0, 1).tokens(0, 3, 3, 2).tech(0, Technology::FleetLogistics).build();
        s.pending_mut().turn_closing = Some(Seat(0));
        s.pending_mut().actions_this_turn = 1;
        (s, k)
    };
    let (mut s, k) = build();
    let e = env_r(&k);
    let menu = ask_ask(&s, &e).unwrap().unwrap().choice;
    assert!(menu.opts.as_slice().iter().any(|o| matches!(o, Opt::Turn(TuOpt::Tactical))), "the second action is offered: {:?}", menu.opts.as_slice());
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Tactical))).unwrap();
    assert_eq!(s.pending().turn_closing, None, "a non-free option is the second action: the turn is no longer closing");
}

/// KM2 section 8: Orbital Drop's mech is offered exactly when the seat can pay 3 resources (`resources >= 3`): two resources drop the infantry and
/// stop, three ask the deploy question.
#[test]
fn orbital_drop_chosen_from_the_menu_offers_the_mech_at_three_resources_and_not_at_two() {
    use ti4_engine_core::choice::promissory::Ask as PAsk;
    use ti4_engine_core::choice::Ask;
    use ti4_engine_core::kernel::hooks::components as c;
    let mut roster = ti4_engine_core::query::tech::UnitRoster::default();
    roster.set(Faction::TheFederationOfSol, ti4_engine_core::generated::UnitBaseType::Infantry, Some(Unit::InfantryI));
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::new(roster));
    let planet_worth = |n: u8| {
        ti4_engine_core_tests::scenario::plain_systems()
            .into_iter()
            .flat_map(|sys| sys.data().planets.iter().copied().collect::<Vec<_>>())
            .find(|p| p.data().resources == n && p.data().source == ti4_engine_core::generated::Source::Base)
            .expect("a planet of that value")
    };
    for (resources, offered) in [(2u8, false), (3u8, true)] {
        let p = planet_worth(resources);
        let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheArborec, Faction::TheEmiratesOfHacan])
            .phase(Phase::Action)
            .active(Some(0))
            .map_from(&[ti4_engine_core_tests::scenario::system_of(p)])
            .control(p, 0)
            .tokens(0, 3, 3, 2)
            .build();
        let e = env_roster(&k, roster);
        let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::ORBITAL_DROP)).unwrap();
        let asks_mech = matches!(&live, Live::Asks(a) if matches!(a.choice.ask, Ask::Promissory(PAsk::OrbitalDeploy { .. })));
        assert_eq!(asks_mech, offered, "{resources} resources: the mech question");
    }
}

/// Lane F2c: Ggrocuto Rinn chosen from the menu readies the planet, exhausts the agent and ends the action (an ACTION leader). Found by
/// `native_play --shadow` (sol, 4 players, seed 1): `Unrouted(5, ..)` at the first leader use of a real game.
#[test]
fn ggrocuto_rinn_chosen_from_the_menu_readies_the_planet_exhausts_the_agent_and_ends_the_action() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .exhaust(planet)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::GgrocutoRinn, LeaderStatus::Readied);
    let e = env_r(&k);
    assert!(ti4_engine_core::query::board::exhausted(&s, planet));
    let result = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::GgrocutoRinn))));
    assert!(result.is_ok(), "{result:?}");
    assert!(!ti4_engine_core::query::board::exhausted(&s, planet), "the planet is ready");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::GgrocutoRinn), Some(&LeaderStatus::Exhausted), "the agent exhausts");
    let kinds = plain(&s);
    assert!(kinds.contains(&PlainKind::ComponentActionResolved), "{kinds:?}");
}

/// Lane F2c: Carth of Golden Sands is printed "during the action phase": a free use. It gains the commodities, exhausts the agent, says
/// `LEADER_ABILITY_RESOLVED` and leaves the turn open (no end of the action, no Extreme Duress).
#[test]
fn carth_chosen_from_the_menu_is_a_free_use_that_gains_commodities_and_leaves_the_turn_open() {
    use ti4_engine_core::choice::leaders::Opt as LOpt;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheArborec])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .commodities(0, 0)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::CarthOfGoldenSands, LeaderStatus::Readied);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::CarthOfGoldenSands)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the branch (gain or replenish another) is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Leaders(LOpt::GainCommodities))).unwrap();
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.commodities, 2);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::CarthOfGoldenSands), Some(&LeaderStatus::Exhausted));
    let kinds = plain(&s);
    assert!(kinds.contains(&PlainKind::LeaderAbilityResolved), "{kinds:?}");
    assert!(!kinds.contains(&PlainKind::ComponentActionResolved) && typed(&s, EventKind::ActionCompleted) == 0, "a free use is not the action: {kinds:?}");
}
