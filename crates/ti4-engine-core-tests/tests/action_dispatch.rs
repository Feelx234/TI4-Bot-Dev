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
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).goods(0, 8).build();
    s.players_mut().edit(0).cards.explore.push(ti4_engine_core::generated::Explore::EnigmaticDevice1);
    let e = env_r(&k);
    let held = s.clone();
    let result = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Device(_))));
    assert!(matches!(result, Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::Menu(MenuGap::Unrouted(8, _)))))), "{result:?}");
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

fn status(s: &CoreState, seat: u8, leader: ti4_engine_core::generated::Leader) -> Option<ti4_engine_core::state::LeaderStatus> {
    s.players().by_seat(Seat(seat)).unwrap().leaders.status.get(&leader).copied()
}

/// Lane F2d: Nekro Malleon is printed "during the action phase" (a free use). The Nekro seat names a player who could pay, that player pays a
/// command token and gains 2 trade goods; the agent exhausts, `LEADER_ABILITY_RESOLVED` is told and the turn stays open.
#[test]
fn nekro_malleon_chosen_from_the_menu_is_a_free_use_where_the_payer_spends_a_token_for_two_goods() {
    use ti4_engine_core::choice::leaders::Opt as LOpt;
    use ti4_engine_core::choice::strategy::Pool;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheNekroVirus, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 3, 2)
        .goods(1, 0)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::NekroMalleon, LeaderStatus::Readied);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::NekroMalleon)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the player who pays is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Leaders(LOpt::Player(Seat(1))))).unwrap();
    pick(&mut s, &e, |o| matches!(o, Opt::Leaders(LOpt::Token(Pool::Tactic)))).unwrap();
    let payer = s.players().by_seat(Seat(1)).unwrap();
    assert_eq!((payer.econ.trade_goods, payer.econ.tactic_tokens), (2, 2), "the payer spent a tactic token for two goods");
    assert_eq!(status(&s, 0, Leader::NekroMalleon), Some(LeaderStatus::Exhausted));
    let kinds = plain(&s);
    assert!(kinds.contains(&PlainKind::LeaderAbilityResolved), "{kinds:?}");
    assert!(!kinds.contains(&PlainKind::ComponentActionResolved) && typed(&s, EventKind::ActionCompleted) == 0, "a free use is not the action: {kinds:?}");
}

/// Lane F2d: Jace X. returns every command token of the seat to its reinforcements, is purged, and (an ACTION) ends the action.
#[test]
fn jace_x_chosen_from_the_menu_returns_the_command_tokens_is_purged_and_ends_the_action() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, Faction::TheArborec])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .command_token(system, 0)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::JaceX4thAirLegion, LeaderStatus::Unlocked);
    let e = env_r(&k);
    assert!(s.board().sys(system).cmd.contains(Seat(0)));
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::JaceX4thAirLegion)))).unwrap();
    assert!(!s.board().sys(system).cmd.contains(Seat(0)), "the token went back");
    assert_eq!(status(&s, 0, Leader::JaceX4thAirLegion), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Darktalon Treilla lifts the fleet supply for the round. She is not purged by the use: she stays in play (`leaders::exhaust` moves only
/// a readied card) and the end of the round purges her.
#[test]
fn darktalon_treilla_chosen_from_the_menu_lifts_the_fleet_supply_and_stays_in_play_until_the_round_ends() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).round(2).tokens(0, 3, 3, 2).build();
    s.players_mut().edit(0).leaders.status.insert(Leader::DarktalonTreilla, LeaderStatus::Unlocked);
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::DarktalonTreilla)))).unwrap();
    assert!(ti4_engine_core::query::fleet::is_unlimited(&s, Seat(0)), "the fleet supply is lifted");
    assert_eq!(status(&s, 0, Leader::DarktalonTreilla), Some(LeaderStatus::Unlocked), "not purged by the use");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    assert_eq!(ti4_engine_core::rules::leaders::end_of_round(&mut s).as_slice(), &[Seat(0)]);
    assert_eq!(status(&s, 0, Leader::DarktalonTreilla), Some(LeaderStatus::Purged), "the end of the round purges her");
}

/// Lane F2d: a copy of Ggrocuto Rinn through Ssruu runs the same body and exhausts Ssruu, not the copied agent.
#[test]
fn a_copy_of_an_agent_through_ssruu_exhausts_ssruu_and_not_the_copied_agent() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheXxchaKingdom, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .exhaust(planet)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
    s.players_mut().edit(1).leaders.status.insert(Leader::GgrocutoRinn, LeaderStatus::Readied);
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Borrowed(Leader::GgrocutoRinn)))).unwrap();
    assert!(!ti4_engine_core::query::board::exhausted(&s, planet), "the planet is ready");
    assert_eq!(status(&s, 0, Leader::Ssruu), Some(LeaderStatus::Exhausted), "Ssruu carries the text, so Ssruu exhausts");
    assert_eq!(status(&s, 1, Leader::GgrocutoRinn), Some(LeaderStatus::Readied), "the copied agent stays ready");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: the promissory notes that are component actions of the holder (Trade Convoys, Blood Pact, Dark Pact, Promise of Protection) go
/// faceup into the play area and end the action.
#[test]
fn an_action_note_chosen_from_the_menu_goes_faceup_and_ends_the_action() {
    use ti4_engine_core::generated::PromissoryNote as N;
    use ti4_engine_core::kernel::hooks::components as c;
    use ti4_engine_core::state::NoteKey;
    for (note, owner, code) in [
        (N::TradeConvoys, Faction::TheEmiratesOfHacan, c::TRADE_CONVOYS),
        (N::BloodPact, Faction::TheEmpyrean, c::BLOOD_PACT),
        (N::DarkPact, Faction::TheEmpyrean, c::DARK_PACT),
        (N::PromiseOfProtection, Faction::TheMentakCoalition, c::PROMISE_OF_PROTECTION),
    ] {
        let others: Vec<Faction> = [Faction::TheFederationOfSol, Faction::TheArborec, Faction::TheXxchaKingdom].into_iter().filter(|f| *f != owner).take(2).collect();
        let (mut s, k) = Scenario::new(&[others[0], others[1], owner]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).build();
        let key = NoteKey { note, faction: owner };
        s.cards_mut().promissory.edit().insert(key, Seat(0));
        let e = env_r(&k);
        pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == code)).unwrap();
        assert!(s.cards().promissory_faceup.contains_key(&key), "{note:?} is faceup");
        assert!(plain(&s).contains(&PlainKind::ComponentActionResolved), "{note:?}");
    }
}

/// Lane F2d: Production Biomes spends a strategy token, gains 4 goods for the holder and 2 for the named player, and ends the action.
#[test]
fn production_biomes_chosen_from_the_menu_spends_a_token_and_pays_the_holder_four_and_the_named_player_two() {
    use ti4_engine_core::choice::technology::Opt as TOpt;
    let (mut s, k) = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheFederationOfSol, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tech(0, Technology::ProductionBiomes).tokens(0, 3, 3, 2).goods(0, 0).goods(1, 0).build();
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == ti4_engine_core::kernel::hooks::components::PRODUCTION_BIOMES)).unwrap();
    assert!(matches!(live, Live::Asks(_)), "who gains is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Technology(TOpt::Player(Seat(1))))).unwrap();
    let goods = |seat: u8| s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods;
    assert_eq!((goods(0), goods(1)), (4, 2));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1, "the token is spent");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// A window that only announces what an effect staged, the way a component action does after its body (`component::announce_staged`).
#[derive(Clone)]
struct Announces;

impl ti4_engine_core::kernel::stage::Window for Announces {
    type Stage = ();

    fn stage(&self) {}

    fn step(&mut self, sx: &mut ti4_engine_core::kernel::stage::Sx<'_>) -> ti4_engine_core::kernel::Next {
        ti4_engine_core::driver::turn::component::announce_staged(sx);
        ti4_engine_core::kernel::Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<ti4_engine_core::choice::Choice> {
        None
    }

    fn answer(&mut self, _: &mut ti4_engine_core::kernel::stage::Sx<'_>, _: Pick) -> Result<(), ti4_engine_core::kernel::stage::Illegal> {
        Err(ti4_engine_core::kernel::stage::Illegal)
    }
}

/// KM2 section 8: a ship an effect destroyed and staged (`pending_destructions`) is announced as `SHIP_DESTROYED` after a component action's body,
/// once, and the queue is empty afterwards. (No route stages one yet; the first leader or faction body that does is covered by this announcement.)
#[test]
fn a_ship_destruction_an_effect_staged_is_announced_after_the_body_of_a_component_action() {
    use ti4_engine_core::state::PendingDestruction;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let mut rig = Rig::new(Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_from(&[system]).unit(system, None, Unit::CruiserI, 1, 1), Announces);
    rig.state.transient_mut().combat.pending_destructions.push(PendingDestruction { system, owner: Seat(1), unit: Unit::CruiserI, reason: ti4_engine_core::state::Code(0), during_space_combat: false });
    rig.run();
    assert!(rig.state.transient().combat.pending_destructions.is_empty(), "the staged destruction was taken");
    assert_eq!(rig.state.transient().combat.last_ship_destroyed, Some((system, Seat(1), Unit::CruiserI)));
    assert_eq!(rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).count(), 1, "announced once");
}

/// Lane F2d: Sling Relay chosen from the menu runs the core's `SlingRelay` window (the dock system, the ship, the payment), places the ship, exhausts the
/// technology and ends the action.
#[test]
fn sling_relay_chosen_from_the_menu_produces_a_ship_exhausts_the_technology_and_ends_the_action() {
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, Faction::TheArborec])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .tech(0, Technology::SlingRelay)
        .tokens(0, 3, 3, 2)
        .build();
    let e = env_r(&k);
    let ships = |s: &CoreState| s.board().sys(system).units.as_slice().iter().filter(|u| u.owner() == Seat(0)).count();
    let before = ships(&s);
    let (mut live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Tech(Technology::SlingRelay)))).unwrap();
    // The window asks the ship and the payment; the first option of each is taken until the use ends.
    for _ in 0..8 {
        if !matches!(live, Live::Asks(_)) {
            break;
        }
        live = pick(&mut s, &e, |_| true).unwrap().0;
    }
    assert_eq!(ships(&s), before + 1, "a ship was produced at the dock");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::SlingRelay), "the technology is exhausted");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Answer the questions a route asks with the first option until it stops asking (at most `n`).
fn first_options(s: &mut CoreState, e: &Env<'_>, mut live: Live, n: usize) {
    for _ in 0..n {
        if !matches!(live, Live::Asks(_)) {
            return;
        }
        live = pick(s, e, |_| true).unwrap().0;
    }
}

fn units_of(s: &CoreState, system: System, seat: u8, unit: Unit) -> usize {
    s.board().sys(system).units.as_slice().iter().filter(|u| u.owner() == Seat(seat) && u.kind() == unit).count()
}

/// Lane F2d: The Helmsman chosen from the menu places the flagship and the dreadnoughts in the chosen system, is purged and ends the action.
#[test]
fn the_helmsman_chosen_from_the_menu_gathers_the_dreadnoughts_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let systems = ti4_engine_core_tests::scenario::plain_systems();
    let (a, b, c) = (systems[0], systems[1], systems[2]);
    let (mut s, k) = Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(7)
        .unit(b, None, Unit::DreadnoughtI, 0, 2)
        .unit(c, None, Unit::DreadnoughtI, 0, 1)
        .unit(a, None, Unit::CruiserI, 1, 1)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::TheHelmsman, LeaderStatus::Unlocked);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::TheHelmsman)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the destination is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Leaders(ti4_engine_core::choice::leaders::Opt::System(x)) if *x == systems[3])).unwrap();
    assert_eq!(units_of(&s, systems[3], 0, Unit::DreadnoughtI), 3, "all three dreadnoughts stand in the chosen system");
    assert_eq!(status(&s, 0, Leader::TheHelmsman), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Gurno Aggero destroys the other players' infantry and fighters in the chosen system next to a dock, and is purged.
#[test]
fn gurno_aggero_chosen_from_the_menu_destroys_the_rivals_fighters_next_to_a_dock_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let sys = ti4_engine_core_tests::scenario::plain_systems();
    let (home, near) = (sys[0], sys[1]);
    let dock = home.data().planets[0];
    let (mut s, k) = Scenario::new(&[Faction::TheClanOfSaar, Faction::TheArborec, Faction::TheYinBrotherhood])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(19)
        .control(dock, 0)
        .unit(home, Some(dock), Unit::SpaceDockI, 0, 1)
        .unit(near, None, Unit::FighterI, 1, 2)
        .unit(near, None, Unit::CruiserI, 1, 1)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::GurnoAggero, LeaderStatus::Unlocked);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::GurnoAggero)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the system is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Leaders(ti4_engine_core::choice::leaders::Opt::System(x)) if *x == near)).unwrap();
    assert_eq!((units_of(&s, near, 1, Unit::FighterI), units_of(&s, near, 1, Unit::CruiserI)), (0, 1), "fighters destroyed, the cruiser stays");
    assert_eq!(status(&s, 0, Leader::GurnoAggero), Some(LeaderStatus::Purged));
}

/// Lane F2d: Xxekir Grom (the Thunder's Edge hero) places units on controlled planets and is purged; he is offered only with a planet to place on.
#[test]
fn xxekir_grom_chosen_from_the_menu_places_a_pds_readies_the_planet_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let planets = ti4_engine_core_tests::scenario::planets_with_influence(&[1, 2]);
    let systems = ti4_engine_core_tests::scenario::systems_of(&planets);
    let mut sc = Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheArborec, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).map_from(&systems).tokens(0, 3, 3, 2);
    for p in &planets {
        sc = sc.control(*p, 0).exhaust(*p);
    }
    let (mut s, k) = sc.build();
    s.players_mut().edit(0).leaders.status.insert(Leader::XxekirGromTe, LeaderStatus::Unlocked);
    let mut roster = ti4_engine_core::query::tech::UnitRoster::default();
    roster.set(Faction::TheXxchaKingdom, ti4_engine_core::generated::UnitBaseType::Pds, Some(Unit::PDSI));
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::new(roster));
    let e = env_roster(&k, roster);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::XxekirGromTe)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the placements are asked");
    first_options(&mut s, &e, live, 12);
    let pds: usize = planets.iter().map(|p| s.board().planet(*p).units.as_slice().iter().filter(|u| u.owner() == Seat(0) && u.kind() == Unit::PDSI).count()).sum();
    assert!(pds >= 1, "a PDS stands on a controlled planet");
    assert_eq!(status(&s, 0, Leader::XxekirGromTe), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Letani Ospha replaces a non-fighter ship with a bigger one of its owner's choice and exhausts; the form each seat places comes from the roster.
#[test]
fn letani_ospha_chosen_from_the_menu_replaces_the_ship_and_exhausts_the_agent() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(7)
        .unit(system, None, Unit::DestroyerI, 1, 1)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::LetaniOspha, LeaderStatus::Readied);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::LetaniOspha)))).unwrap();
    first_options(&mut s, &e, live, 6);
    assert_eq!(units_of(&s, system, 1, Unit::DestroyerI), 0, "the destroyer was replaced");
    assert_eq!(status(&s, 0, Leader::LetaniOspha), Some(LeaderStatus::Exhausted));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Ahk-Syl Siven marks the round and is purged; Ul the Progenitor readies Elysium and attaches Geoform.
#[test]
fn ahk_syl_siven_and_ul_the_progenitor_chosen_from_the_menu_resolve_and_are_purged() {
    use ti4_engine_core::generated::{Attachment, Leader, Planet};
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheNomad, Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, None, Unit::MemoriaI, 0, 1)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::AhkSylSiven, LeaderStatus::Unlocked);
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::AhkSylSiven)))).unwrap();
    assert_eq!(status(&s, 0, Leader::AhkSylSiven), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));

    let elysium = Planet::Elysium;
    let (mut s, k) = Scenario::new(&[Faction::TheTitansOfUl, Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[ti4_engine_core_tests::scenario::system_of(elysium)])
        .control(elysium, 0)
        .exhaust(elysium)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::UlTheProgenitor, LeaderStatus::Unlocked);
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::UlTheProgenitor)))).unwrap();
    assert!(!ti4_engine_core::query::board::exhausted(&s, elysium), "Elysium is ready");
    assert!(ti4_engine_core::query::board::attachments(&s, elysium).as_slice().contains(&Attachment::Geoform));
    assert_eq!(status(&s, 0, Leader::UlTheProgenitor), Some(LeaderStatus::Purged));
}

fn muaat_table(extra: impl FnOnce(Scenario) -> Scenario) -> (CoreState, Kernel) {
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    extra(Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).map_ring(1).unit(system, None, Unit::WarSun, 0, 1).tokens(0, 3, 3, 2)).build()
}

/// Lane F2d: Star Forge places 2 fighters at a war sun and spends a strategy token; The Nucleus places them without the token.
#[test]
fn star_forge_chosen_from_the_menu_places_two_fighters_and_spends_a_token() {
    use ti4_engine_core::kernel::hooks::components as c;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = muaat_table(|sc| sc);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::STAR_FORGE)).unwrap();
    first_options(&mut s, &e, live, 6);
    assert_eq!(units_of(&s, system, 0, Unit::FighterI), 2, "two fighters stand at the war sun");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1, "the token is spent");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    // The strategy token was announced at once (a module is seated: Muaat).
    assert!(s.pending().staged.is_empty(), "the staged token event was announced");
}

/// Lane F2d: The Inferno places a cruiser in the system of the Muaat flagship and spends a strategy token.
#[test]
fn the_inferno_chosen_from_the_menu_places_a_cruiser_and_spends_a_token() {
    use ti4_engine_core::kernel::hooks::components as c;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = muaat_table(|sc| sc.unit(system, None, Unit::TheInferno, 0, 1));
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::INFERNO)).unwrap();
    assert_eq!(units_of(&s, system, 0, Unit::CruiserI), 1);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1);
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Fires of the Gashlai returns a Muaat fleet token and gains the war sun technology for the holder.
#[test]
fn fires_of_the_gashlai_chosen_from_the_menu_gains_the_war_sun_technology() {
    use ti4_engine_core::generated::PromissoryNote;
    use ti4_engine_core::kernel::hooks::components as c;
    use ti4_engine_core::state::NoteKey;
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmbersOfMuaat, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).tokens(1, 3, 3, 2).build();
    s.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::FiresOfTheGashlai, faction: Faction::TheEmbersOfMuaat }, Seat(0));
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::FIRES)).unwrap();
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.owned.contains(Technology::WarSun), "the holder gained the war sun technology");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Naaz Fabrication spends 1 fragment for a command token (token pool asked), and 2 fragments for the top relic.
#[test]
fn naaz_fabrication_chosen_from_the_menu_spends_fragments_for_a_token_or_a_relic() {
    use ti4_engine_core::generated::ExploreType;
    use ti4_engine_core::kernel::hooks::components as c;
    for (code, spent) in [(c::FABRICATION_TOKEN, 1), (c::FABRICATION_RELIC, 2)] {
        let (mut s, k) = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).tokens(0, 2, 2, 1).build();
        s.players_mut().edit(0).cards.fragments.set(ExploreType::Cultural, 2);
        s.decks_mut().relic = ti4_engine_core::Deck::new(vec![ti4_engine_core::generated::Relic::DynamisCore]);
        let e = env_r(&k);
        let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == code)).unwrap();
        first_options(&mut s, &e, live, 6);
        let p = s.players().by_seat(Seat(0)).unwrap();
        assert_eq!(p.cards.fragments.get(ExploreType::Cultural), Some(2 - spent), "fragments spent");
        assert!(plain(&s).contains(&PlainKind::ComponentActionResolved), "{code}");
        if code == c::FABRICATION_RELIC {
            assert!(p.cards.relics.as_slice().contains(&ti4_engine_core::generated::Relic::DynamisCore), "the relic is gained");
        }
    }
}

/// Lane F2d: the relic bodies. Dynamis Core pays the faction's commodity value + 2 as trade goods, the Book of Latvinia makes a seat that holds
/// not all four specialties the speaker, the Codex takes cards from the discard pile; each relic is purged by its use.
#[test]
fn dynamis_core_the_book_of_latvinia_and_the_codex_chosen_from_the_menu_resolve_and_are_purged() {
    use ti4_engine_core::generated::{ActionCard, Relic};
    let table = |relic: Relic| {
        let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).goods(0, 0).build();
        s.players_mut().edit(0).cards.relics.push(relic);
        (s, k)
    };
    let held = |s: &CoreState, r: Relic| s.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().contains(&r);
    let (mut s, k) = table(Relic::DynamisCore);
    pick(&mut s, &env_r(&k), |o| matches!(o, Opt::Turn(TuOpt::UseRelic(Relic::DynamisCore)))).unwrap();
    let value = i16::from(Faction::TheFederationOfSol.data().commodities) + 2;
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, value, "paid commodity value + 2");
    assert!(!held(&s, Relic::DynamisCore), "purged");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));

    let (mut s, k) = table(Relic::BookOfLatvinia);
    s.meta_mut().speaker = Seat(1);
    pick(&mut s, &env_r(&k), |o| matches!(o, Opt::Turn(TuOpt::UseRelic(Relic::BookOfLatvinia)))).unwrap();
    assert_eq!(s.meta().speaker, Seat(0), "without all four specialties the user becomes the speaker");
    assert!(!held(&s, Relic::BookOfLatvinia));

    let (mut s, k) = table(Relic::TheCodex);
    s.cards_mut().discarded_action.edit().push(ActionCard::ALL[0]);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::UseRelic(Relic::TheCodex)))).unwrap();
    first_options(&mut s, &e, live, 6);
    assert!(!held(&s, Relic::TheCodex));
    assert!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().contains(&ActionCard::ALL[0]), "the card came from the pile");
}

/// Lane F2d: a copy of Carth (a free use) and of Letani Ospha (an ACTION) through Ssruu follow the copied agent's window: Carth leaves the turn open,
/// Ospha ends the action; Ssruu exhausts in both and the copied agent stays as it was.
#[test]
fn ssruu_copies_of_carth_and_letani_ospha_follow_the_copied_agents_timing() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    // Carth: free.
    let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheEmiratesOfHacan, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).commodities(0, 0).build();
    s.players_mut().edit(0).leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
    s.players_mut().edit(1).leaders.status.insert(Leader::CarthOfGoldenSands, LeaderStatus::Readied);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Borrowed(Leader::CarthOfGoldenSands)))).unwrap();
    first_options(&mut s, &e, live, 4);
    assert_eq!(status(&s, 0, Leader::Ssruu), Some(LeaderStatus::Exhausted));
    assert_eq!(status(&s, 1, Leader::CarthOfGoldenSands), Some(LeaderStatus::Readied));
    let kinds = plain(&s);
    assert!(kinds.contains(&PlainKind::LeaderAbilityResolved) && !kinds.contains(&PlainKind::ComponentActionResolved), "{kinds:?}");
    // Ospha: an action.
    let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheArborec, Faction::TheEmbersOfMuaat]).phase(Phase::Action).active(Some(0)).map_ring(7).unit(system, None, Unit::DestroyerI, 2, 1).tokens(0, 3, 3, 2).build();
    s.players_mut().edit(0).leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
    s.players_mut().edit(1).leaders.status.insert(Leader::LetaniOspha, LeaderStatus::Readied);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Borrowed(Leader::LetaniOspha)))).unwrap();
    first_options(&mut s, &e, live, 6);
    assert_eq!(status(&s, 0, Leader::Ssruu), Some(LeaderStatus::Exhausted));
    assert_eq!(status(&s, 1, Leader::LetaniOspha), Some(LeaderStatus::Readied));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: The Nucleus (Avernus) places the fighters without a strategy token and exhausts the legendary card.
#[test]
fn the_nucleus_chosen_from_the_menu_places_fighters_without_a_token_and_exhausts_avernus() {
    use ti4_engine_core::generated::Planet;
    use ti4_engine_core::kernel::hooks::components as c;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let avernus = Planet::Avernus;
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, Faction::TheArborec])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .control(avernus, 0)
        .unit(system, None, Unit::WarSun, 0, 1)
        .tokens(0, 3, 3, 0)
        .build();
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::NUCLEUS)).unwrap();
    first_options(&mut s, &e, live, 6);
    assert_eq!(units_of(&s, system, 0, Unit::FighterI), 2, "two fighters stand at the war sun");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 0, "no token was needed");
    assert!(s.players().by_seat(Seat(0)).unwrap().cards.legendary_exhausted.contains_key(&avernus), "the legendary card is exhausted");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2d: Black Market Forgery (the Naaz note) purges 2 fragments for the top relic and the note goes home.
#[test]
fn black_market_forgery_chosen_from_the_menu_purges_two_fragments_for_a_relic_and_returns_the_note() {
    use ti4_engine_core::generated::{ExploreType, PromissoryNote, Relic};
    use ti4_engine_core::kernel::hooks::components as c;
    use ti4_engine_core::state::NoteKey;
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheNaazRokhaAlliance, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).build();
    let note = NoteKey { note: PromissoryNote::BlackMarketForgery, faction: Faction::TheNaazRokhaAlliance };
    s.cards_mut().promissory.edit().insert(note, Seat(0));
    s.players_mut().edit(0).cards.fragments.set(ExploreType::Hazardous, 2);
    s.decks_mut().relic = ti4_engine_core::Deck::new(vec![Relic::DynamisCore]);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::BLACK_MARKET_FORGERY)).unwrap();
    first_options(&mut s, &e, live, 6);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(p.cards.relics.as_slice().contains(&Relic::DynamisCore), "the relic is gained");
    assert_eq!(p.cards.fragments.get(ExploreType::Hazardous), Some(0));
    assert_eq!(s.cards().promissory.get(&note), Some(&Seat(1)), "the note went home to the Naaz");
}

// ---- lane F2e: the production leaders ----

/// A table whose seat 0 (Muaat) holds Umbat, with a war sun at the first system, a planet there and trade goods; `extra` adds to the scenario.
fn umbat_table(extra: impl FnOnce(Scenario) -> Scenario) -> (CoreState, Kernel) {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(1)
        .control(planet, 0)
        .unit(system, None, Unit::WarSun, 0, 1)
        .goods(0, 6)
        .tokens(0, 3, 3, 2);
    let (mut s, k) = extra(sc).build();
    s.players_mut().edit(0).leaders.status.insert(Leader::Umbat, LeaderStatus::Readied);
    (s, k)
}

/// Answer a production's questions: build `wanted` whenever it is offered, stop producing once it was built `times` times, else take the first option.
fn build_only(s: &mut CoreState, e: &Env<'_>, mut live: Live, wanted: Unit, times: usize) {
    use ti4_engine_core::choice::production::Opt as POpt;
    let mut built = 0;
    for _ in 0..60 {
        if !matches!(live, Live::Asks(_)) {
            return;
        }
        let q = ask_ask(s, e).unwrap().unwrap().choice;
        let opts = q.opts.as_slice();
        let take = if built >= times {
            opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::Stop) | Opt::Decline)).or_else(|| opts.first().copied())
        } else {
            opts.iter().copied().find(|o| matches!(o, Opt::Production(POpt::Build { unit, .. }) if *unit == wanted))
        };
        let take = take.unwrap_or_else(|| opts[0]);
        if matches!(take, Opt::Production(POpt::Build { unit, .. }) if unit == wanted) {
            built += 1;
        }
        live = advance_ask(s, e, Some(Pick::Opt(take))).unwrap().0;
    }
}

/// The `UNITS_PRODUCED` announcements told so far (the leader's use announces at once, as the string does).
fn produced_events(s: &CoreState) -> Vec<&ti4_engine_core::state::EventRec> {
    s.ledger.events.iter().filter_map(|e| match e {
        Emit::Typed(t) if t.kind == EventKind::UnitsProduced => Some(t),
        _ => None,
    }).collect()
}

/// Lane F2e: Umbat chosen from the menu: the holder names a player, that player produces up to 2 units of cost 4 or less in a system with a war sun,
/// the agent exhausts, the action ends, and the use stages `UNITS_PRODUCED` with the source `ability` for the chosen player.
#[test]
fn umbat_chosen_from_the_menu_lets_the_named_player_produce_two_units_and_exhausts_the_agent() {
    use ti4_engine_core::choice::leaders::Opt as LOpt;
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = umbat_table(|sc| sc);
    let mut e = env_r(&k);
    e.table.facts.staging = true;
    let before = s.board().sys(system).units.as_slice().len();
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::Umbat)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the player is asked");
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Leaders(LOpt::Player(Seat(0))))).unwrap();
    first_options(&mut s, &e, live, 12);
    let made = s.board().sys(system).units.as_slice().len() - before;
    assert!((1..=2).contains(&made), "up to two units: {made}");
    assert_eq!(status(&s, 0, Leader::Umbat), Some(LeaderStatus::Exhausted));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    let staged = produced_events(&s);
    assert_eq!(staged.len(), 1, "one use, one announcement");
    assert!(s.pending().staged.is_empty(), "announced at once, nothing left staged");
    assert_eq!(staged[0].payload.seat(ti4_engine_core::state::Key::Player), Some(Seat(0)), "the chosen player is named");
}

/// Lane F2e: a decline of the player spends nothing: the agent stays ready and no action is taken (the string's `Some(false)`: the menu is asked again).
#[test]
fn umbat_declined_at_the_player_spends_nothing() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (mut s, k) = umbat_table(|sc| sc);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::Umbat)))).unwrap();
    assert!(matches!(live, Live::Asks(_)));
    pick(&mut s, &e, |o| matches!(o, Opt::Decline)).unwrap();
    assert_eq!(status(&s, 0, Leader::Umbat), Some(LeaderStatus::Readied), "a decline spends nothing");
    assert!(plain(&s).contains(&PlainKind::ComponentActionFailed) && !plain(&s).contains(&PlainKind::ComponentActionResolved), "{:?}", plain(&s));
}

/// Lane F2e: the end of the use is the string adapter's tail: the Prophecy of Ixth is discarded by a use that made fewer than two fighters.
#[test]
fn the_end_of_a_leaders_production_discards_the_prophecy_of_ixth_unless_two_fighters_were_made() {
    use ti4_engine_core::generated::{Agenda, Leader};
    use ti4_engine_core::state::Outcome;
    let (mut s, k) = umbat_table(|sc| sc);
    s.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(0)));
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::Umbat)))).unwrap();
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Leaders(ti4_engine_core::choice::leaders::Opt::Player(Seat(0))))).unwrap();
    build_only(&mut s, &e, live, Unit::DestroyerI, 1);
    assert!(units_of(&s, ti4_engine_core_tests::scenario::first_planet().0, 0, Unit::DestroyerI) == 1, "a destroyer was built");
    assert!(!s.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), "fewer than two fighters: the law is discarded with the use");
}

/// Lane F2e: Letani Miasmiala produces in every system holding a ground force of the seat with no limit, is purged when something was made, and
/// the use announces `UNITS_PRODUCED` for the system.
#[test]
fn letani_miasmiala_chosen_from_the_menu_produces_where_the_ground_forces_stand_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(1)
        .control(planet, 0)
        .unit(system, Some(planet), Unit::InfantryI, 0, 1)
        .unit(system, None, Unit::CarrierI, 0, 1)
        .goods(0, 6)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::LetaniMiasmiala, LeaderStatus::Unlocked);
    let mut e = env_r(&k);
    e.table.facts.staging = true;
    let before = s.board().sys(system).units.as_slice().len() + ti4_engine_core::query::board::planet_units(&s, planet).len();
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::LetaniMiasmiala)))).unwrap();
    first_options(&mut s, &e, live, 16);
    let after = s.board().sys(system).units.as_slice().len() + ti4_engine_core::query::board::planet_units(&s, planet).len();
    assert!(after > before, "something was produced: {before} -> {after}");
    assert_eq!(status(&s, 0, Leader::LetaniMiasmiala), Some(LeaderStatus::Purged), "the hero is purged");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    assert_eq!(produced_events(&s).len(), 1, "announced once for the one system");
}

/// Lane F2e: Hacan's Auto-Factories pays a fleet token for three ships of a leader's production (Letani Miasmiala has no limit).
#[test]
fn a_leaders_production_of_three_ships_pays_hacans_auto_factories_token() {
    use ti4_engine_core::generated::{Breakthrough, Leader};
    use ti4_engine_core::state::LeaderStatus;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(1)
        .control(planet, 0)
        .unit(system, Some(planet), Unit::InfantryI, 0, 1)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .goods(0, 30)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::LetaniMiasmiala, LeaderStatus::Unlocked);
    s.players_mut().edit(0).cards.breakthrough = Some(Breakthrough::AutoFactories);
    let e = env_r(&k);
    let fleet = |s: &CoreState| s.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens;
    let before = fleet(&s);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::LetaniMiasmiala)))).unwrap();
    build_only(&mut s, &e, live, Unit::DestroyerI, 3);
    assert_eq!(units_of(&s, system, 0, Unit::DestroyerI), 3, "three destroyers were built");
    assert_eq!(fleet(&s), before + 1, "a token for three non-fighter ships");
}

/// Lane F2e (mutation debt of F2d): the question of Black Market Forgery names the `bmf` source and Naaz's own Fabrication the `fabrication` one. The
/// test table interns nothing, so the two codes are set apart here; the window must be built with the right one.
#[test]
fn black_market_forgery_and_fabrication_ask_under_their_own_source() {
    use ti4_engine_core::choice::generic::Content;
    use ti4_engine_core::choice::Origin;
    use ti4_engine_core::generated::{ExploreType, PromissoryNote};
    use ti4_engine_core::kernel::hooks::components as c;
    use ti4_engine_core::state::{Code, NoteKey};
    for (code, expected) in [(c::BLACK_MARKET_FORGERY, Code(31)), (c::FABRICATION_RELIC, Code(32))] {
        let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheNaazRokhaAlliance, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).build();
        let note = NoteKey { note: PromissoryNote::BlackMarketForgery, faction: Faction::TheNaazRokhaAlliance };
        s.cards_mut().promissory.edit().insert(note, Seat(0));
        // Two types of fragments, so the window asks which type (a question that carries its source).
        s.players_mut().edit(0).cards.fragments.set(ExploreType::Hazardous, 2);
        s.players_mut().edit(0).cards.fragments.set(ExploreType::Cultural, 2);
        s.decks_mut().relic = ti4_engine_core::Deck::new(vec![ti4_engine_core::generated::Relic::DynamisCore]);
        if code == c::FABRICATION_RELIC {
            s.players_mut().edit(0).faction = Faction::TheNaazRokhaAlliance;
        }
        let mut e = env_r(&k);
        e.table.codes.turn.bmf = Code(31);
        e.table.codes.turn.fabrication = Code(32);
        let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == code)).unwrap();
        assert!(matches!(live, Live::Asks(_)), "the type of fragment is asked");
        let q = ask_ask(&mut s, &e).unwrap().unwrap().choice;
        assert_eq!(q.ctx.origin, Origin::FactionAbility(Content::Other(expected)), "{code}");
    }
}

// ---- lane F2e: Rin, Riftwalker Meian and the relics with a window of their own ----

fn plain_tech(skip: &[Technology]) -> Technology {
    use ti4_engine_core::generated::{Source, TechType};
    *Technology::ALL
        .iter()
        .find(|t| {
            let d = t.data();
            d.source == Source::Base && d.faction.is_none() && d.researchable && !d.types.is_empty() && !d.types.contains(&TechType::Unitupgrade) && !skip.contains(t)
        })
        .expect("a plain technology")
}

/// Lane F2e: Rin swaps a technology of the seat for another of its colour (the question per technology, the trade is the first option) and is purged.
#[test]
fn rin_chosen_from_the_menu_swaps_a_technology_for_one_of_its_colour_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let held = plain_tech(&[]);
    let (mut s, k) = Scenario::new(&[Faction::TheUniversitiesOfJolNar, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .tech(0, held)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::RinTheMastersLegacy, LeaderStatus::Unlocked);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::RinTheMastersLegacy)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the swap of the technology is asked");
    first_options(&mut s, &e, live, 6);
    let owned = &s.players().by_seat(Seat(0)).unwrap().tech.owned;
    assert!(!owned.contains(held), "the held technology went back to the deck");
    assert_eq!(status(&s, 0, Leader::RinTheMastersLegacy), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: a Jol-Nar seat holding no technology it could swap is not offered the hero (the string's list of replacements is empty).
#[test]
fn rin_is_not_offered_when_nothing_can_be_swapped() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let (mut s, k) = Scenario::new(&[Faction::TheUniversitiesOfJolNar, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).build();
    s.players_mut().edit(0).leaders.status.insert(Leader::RinTheMastersLegacy, LeaderStatus::Unlocked);
    let e = env_r(&k);
    let q = ask_ask(&mut s, &e).unwrap().unwrap().choice;
    assert!(!q.opts.as_slice().iter().any(|o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::RinTheMastersLegacy)))), "no technology, no swap");
}

/// Lane F2e: Riftwalker Meian swaps two systems that hold the seat's units and records the map edit; the hero is purged.
#[test]
fn riftwalker_meian_chosen_from_the_menu_records_the_swap_of_two_systems_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::{LeaderStatus, MarkKey};
    let ps = ti4_engine_core_tests::scenario::plain_systems();
    let (mut s, k) = Scenario::new(&[Faction::TheGhostsOfCreuss, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(7)
        .unit(ps[0], None, Unit::CruiserI, 0, 1)
        .unit(ps[1], None, Unit::CruiserI, 0, 1)
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::RiftwalkerMeian, LeaderStatus::Unlocked);
    // The string board has an entry for a system that holds a unit (the menu counts those).
    for system in [ps[0], ps[1]] {
        s.board_mut().sys_mut(system).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    }
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::RiftwalkerMeian)))).unwrap();
    first_options(&mut s, &e, live, 4);
    let edits = s.marks().tail.iter().filter(|(k, _)| matches!(k, MarkKey::MapEdit(_))).count();
    assert_eq!(edits, 1, "one swap was recorded");
    assert_eq!(status(&s, 0, Leader::RiftwalkerMeian), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

fn relic_table(relic: ti4_engine_core::generated::Relic, extra: impl FnOnce(Scenario) -> Scenario) -> (CoreState, Kernel) {
    let (mut s, k) = extra(Scenario::new(&[Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, Faction::TheArborec]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2)).build();
    s.players_mut().edit(0).cards.relics.push(relic);
    (s, k)
}

fn use_relic(s: &mut CoreState, e: &Env<'_>, relic: ti4_engine_core::generated::Relic) -> Live {
    pick(s, e, |o| matches!(o, Opt::Turn(TuOpt::UseRelic(r)) if *r == relic)).unwrap().0
}

/// Lane F2e: the Maw of Worlds exhausts every planet of the seat, gains the technology asked (no prerequisites) and is purged.
#[test]
fn the_maw_of_worlds_chosen_from_the_menu_exhausts_the_planets_gains_a_technology_and_is_purged() {
    use ti4_engine_core::generated::Relic;
    let (_, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = relic_table(Relic::MawOfWorlds, |sc| sc.map_ring(1).control(planet, 0));
    let e = env_r(&k);
    let before = s.players().by_seat(Seat(0)).unwrap().tech.owned.len();
    let live = use_relic(&mut s, &e, Relic::MawOfWorlds);
    assert!(matches!(live, Live::Asks(_)), "the technology is asked");
    first_options(&mut s, &e, live, 4);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.tech.owned.len(), before + 1, "one technology gained");
    assert!(s.board().planet(planet).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "every planet is exhausted first");
    assert!(!p.cards.relics.as_slice().contains(&Relic::MawOfWorlds), "the relic is purged");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: the Enigmatic Device (the relic) takes six resources, researches the technology asked and is purged.
#[test]
fn the_enigmatic_device_relic_chosen_from_the_menu_pays_six_and_researches_a_technology() {
    use ti4_engine_core::generated::Relic;
    let (mut s, k) = relic_table(Relic::EnigmaticDevice, |sc| sc.goods(0, 6));
    let e = env_r(&k);
    let before = s.players().by_seat(Seat(0)).unwrap().tech.owned.len();
    let live = use_relic(&mut s, &e, Relic::EnigmaticDevice);
    first_options(&mut s, &e, live, 12);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.trade_goods, 0, "six resources were paid");
    assert_eq!(p.tech.owned.len(), before + 1, "one technology researched");
    assert!(!p.cards.relics.as_slice().contains(&Relic::EnigmaticDevice), "purged");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: the Stellar Converter destroys the planet chosen next to a bombarding unit and is purged.
#[test]
fn the_stellar_converter_chosen_from_the_menu_destroys_a_planet_next_to_a_bombarding_unit() {
    use ti4_engine_core::generated::Relic;
    let ps = ti4_engine_core_tests::scenario::plain_systems();
    let (mut s, k) = relic_table(Relic::StellarConverter, |sc| sc.map_ring(7).unit(ps[0], None, Unit::DreadnoughtI, 0, 1));
    for system in &ps[..7] {
        s.board_mut().sys_mut(*system).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    }
    let e = env_r(&k);
    let live = use_relic(&mut s, &e, Relic::StellarConverter);
    let q = ask_ask(&mut s, &e).unwrap().unwrap().choice;
    let target = q.opts.as_slice().iter().find_map(|o| match o {
        Opt::Relic(ti4_engine_core::choice::relic::Opt::Destroy(planet, _)) => Some(*planet),
        _ => None,
    });
    let target = target.expect("a target is offered");
    first_options(&mut s, &e, live, 3);
    assert!(s.board().planet(target).flags.has(ti4_engine_core::state::PlanetFlags::PURGED), "the planet is destroyed");
    assert!(!s.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().contains(&Relic::StellarConverter));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: the Silver Flame rolls a die, is purged either way and, on anything but a ten, bars the seat from public objectives and purges its
/// home system. The roll is on the dice stream and in the ledger.
#[test]
fn the_silver_flame_chosen_from_the_menu_rolls_a_die_and_is_purged_either_way() {
    use ti4_engine_core::generated::Relic;
    let (mut s, k) = relic_table(Relic::TheSilverFlame, |sc| sc);
    let e = env_r(&k);
    let dice = s.ledger.dice.len();
    use_relic(&mut s, &e, Relic::TheSilverFlame);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(!p.cards.relics.as_slice().contains(&Relic::TheSilverFlame), "purged");
    assert_eq!(s.ledger.dice.len(), dice + 1, "one die rolled");
    let roll = s.ledger.dice.last().unwrap();
    assert_eq!(roll.faces.len(), 1);
    let ten = roll.faces[0] == 10;
    assert_eq!(p.econ.public_objectives_forbidden, !ten, "anything but a ten bars the seat from public objectives");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: the Titan Prototype: the owner names the builder, who builds a structure for three resources or gains a trade good.
#[test]
fn the_titan_prototype_chosen_from_the_menu_lets_the_named_player_build_or_gain_a_trade_good() {
    use ti4_engine_core::generated::Relic;
    // Seat 1 has nothing to build on: it takes the trade good.
    let (mut s, k) = relic_table(Relic::JRXS455O, |sc| sc.goods(1, 0));
    let e = env_r(&k);
    let live = use_relic(&mut s, &e, Relic::JRXS455O);
    assert!(matches!(live, Live::Asks(_)), "the builder is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Relic(ti4_engine_core::choice::relic::Opt::Builder(Seat(1))))).unwrap();
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 1, "the consolation");
    assert!(!s.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().contains(&Relic::JRXS455O), "purged");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: Wormhole Generator places a Creuss token in a system that holds no other player's ship (or a planet the seat controls), exhausts the
/// technology and ends the action. A system with a rival's ship that the seat does not control is not offered.
#[test]
fn wormhole_generator_chosen_from_the_menu_places_a_token_and_exhausts_the_technology() {
    use ti4_engine_core::choice::technology::Opt as TecOpt;
    use ti4_engine_core::generated::Wormhole;
    use ti4_engine_core::kernel::hooks::components as c;
    let ps = ti4_engine_core_tests::scenario::plain_systems();
    let (mut s, k) = Scenario::new(&[Faction::TheGhostsOfCreuss, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(7)
        .unit(ps[1], None, Unit::CruiserI, 1, 1)
        .tech(0, Technology::WormholeGenerator)
        .tokens(0, 3, 3, 2)
        .build();
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::WORMHOLE_GENERATOR)).unwrap();
    assert!(matches!(live, Live::Asks(_)));
    let q = ask_ask(&mut s, &e).unwrap().unwrap().choice;
    let offered: Vec<(Wormhole, System)> = q.opts.as_slice().iter().filter_map(|o| match o {
        Opt::Technology(TecOpt::Token(kind, system)) => Some((*kind, *system)),
        _ => None,
    }).collect();
    assert!(!offered.is_empty() && offered.iter().all(|(_, system)| *system != ps[1]), "a rival's ships keep the token out: {offered:?}");
    assert_eq!(offered[0].0, Wormhole::Alpha, "alpha first");
    let want = offered[0];
    pick(&mut s, &e, |o| matches!(o, Opt::Technology(TecOpt::Token(kind, system)) if (*kind, *system) == want)).unwrap();
    assert_eq!(s.explore().wormhole_tokens.get(&want.0), Some(&want.1), "the token stands there");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::WormholeGenerator));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

// ---- lane F2e: the heroes that read the position for their facts ----

fn touch(s: &mut CoreState, systems: &[System]) {
    for system in systems {
        s.board_mut().sys_mut(*system).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    }
}

/// Lane F2e: UNIT.DSGN.FLAYESH devours a planet with a technology specialty in a system holding the seat's unit: the other seats' units on it die, the
/// seat gains the planet's resources and influence as trade goods and a technology of the specialty's colour, and the hero is purged.
#[test]
fn flayesh_chosen_from_the_menu_devours_the_planet_and_gains_goods_and_a_technology() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let planet = *Planet::ALL.iter().find(|p| !p.data().specialties.is_empty() && p.data().system.is_some() && !p.data().legendary).expect("a planet with a specialty");
    let system = planet.data().system.expect("on a tile");
    let (mut s, k) = Scenario::new(&[Faction::TheNekroVirus, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[system])
        .unit(system, None, Unit::CruiserI, 0, 1)
        .unit(system, Some(planet), Unit::InfantryI, 1, 2)
        .goods(0, 0)
        .tokens(0, 3, 3, 2)
        .build();
    touch(&mut s, &[system]);
    s.players_mut().edit(0).leaders.status.insert(Leader::UNITDSGNFLAYESH, ti4_engine_core::state::LeaderStatus::Unlocked);
    let e = env_r(&k);
    let owned = s.players().by_seat(Seat(0)).unwrap().tech.owned.len();
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::UNITDSGNFLAYESH)))).unwrap();
    first_options(&mut s, &e, live, 6);
    assert_eq!(ti4_engine_core::query::board::planet_units(&s, planet).iter().filter(|p| p.owner() == Seat(1)).count(), 0, "the other seats' units are gone");
    assert_eq!(typed(&s, EventKind::GroundForceDestroyed), 2, "each ground force that fell is announced");
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(p.econ.trade_goods > 0, "the planet's value is gained as trade goods");
    assert_eq!(p.tech.owned.len(), owned + 1, "one technology gained");
    assert_eq!(status(&s, 0, Leader::UNITDSGNFLAYESH), Some(ti4_engine_core::state::LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    let _ = LeaderStatus::Purged;
}

/// Lane F2e: Hesh and Prit gains a relic (the choice of strategy cards declined) and is purged.
#[test]
fn hesh_and_prit_chosen_from_the_menu_gains_a_relic_and_is_purged() {
    use ti4_engine_core::generated::{Leader, Relic};
    use ti4_engine_core::state::LeaderStatus;
    let (mut s, k) = Scenario::new(&[Faction::TheNaazRokhaAlliance, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).tokens(0, 3, 3, 2).build();
    s.players_mut().edit(0).leaders.status.insert(Leader::HeshAndPrit, LeaderStatus::Unlocked);
    s.decks_mut().relic = ti4_engine_core::Deck::new(vec![Relic::DynamisCore]);
    s.strategy_mut().unclaimed.push(ti4_engine_core::generated::StrategyCard::ALL[0]);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::HeshAndPrit)))).unwrap();
    assert!(matches!(live, Live::Asks(_)), "the card is asked");
    pick(&mut s, &e, |o| matches!(o, Opt::Decline)).unwrap();
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(p.cards.relics.as_slice().contains(&Relic::DynamisCore), "the relic is gained");
    assert_eq!(status(&s, 0, Leader::HeshAndPrit), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: Kyver shows a card of each other player; the seat takes the first and the hero is purged.
#[test]
fn kyver_chosen_from_the_menu_takes_a_shown_card_and_is_purged() {
    use ti4_engine_core::generated::{ActionCard, Leader};
    use ti4_engine_core::state::LeaderStatus;
    let (mut s, k) = Scenario::new(&[Faction::TheYssarilTribes, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .hand(1, &[ActionCard::ALL[0], ActionCard::ALL[3]])
        .hand(2, &[ActionCard::ALL[5]])
        .tokens(0, 3, 3, 2)
        .build();
    s.players_mut().edit(0).leaders.status.insert(Leader::KyverBladeAndKey, LeaderStatus::Unlocked);
    let mut e = env_r(&k);
    // The printed-name groups: every card is its own name.
    e.table.facts.card_groups = (0..ActionCard::ALL.len()).map(|i| u8::try_from(i % 250).unwrap_or(0)).collect();
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::KyverBladeAndKey)))).unwrap();
    first_options(&mut s, &e, live, 12);
    assert_eq!(status(&s, 0, Leader::KyverBladeAndKey), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: It Feeds on Carrion rolls a die for each other player's non-fighter ship next to a dimensional tear and is purged.
#[test]
fn it_feeds_on_carrion_chosen_from_the_menu_rolls_for_the_ships_next_to_a_tear_and_is_purged() {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::state::LeaderStatus;
    let tempesta = ti4_engine_core_tests::scenario::system_of(Planet::Tempesta);
    let mut ring = vec![tempesta];
    ring.extend(ti4_engine_core_tests::scenario::plain_systems().into_iter().filter(|s| *s != tempesta).take(6));
    let (mut s, k) = Scenario::new(&[Faction::TheVuilraithCabal, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&ring)
        .unit(tempesta, Some(Planet::Tempesta), Unit::DimensionalTearI, 0, 1)
        .unit(tempesta, None, Unit::CruiserI, 1, 1)
        .unit(ring[1], None, Unit::DestroyerI, 2, 1)
        .tokens(0, 3, 3, 2)
        .build();
    touch(&mut s, &ring[..2]);
    s.players_mut().edit(0).leaders.status.insert(Leader::ItFeedsOnCarrion, LeaderStatus::Unlocked);
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(Leader::ItFeedsOnCarrion)))).unwrap();
    first_options(&mut s, &e, live, 8);
    assert_eq!(status(&s, 0, Leader::ItFeedsOnCarrion), Some(LeaderStatus::Purged));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

// ---- lane F2e: the three faction actions the menu used to refuse (`MenuGap::Faction`) ----

fn nekro_table(extra: impl FnOnce(Scenario) -> Scenario) -> (CoreState, Kernel) {
    let (_, planet) = ti4_engine_core_tests::scenario::first_planet();
    extra(Scenario::new(&[Faction::TheNekroVirus, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).map_ring(1).control(planet, 0).tech(0, Technology::ERRORERROR).tokens(0, 3, 3, 2)).build()
}

fn c4r(s: &mut CoreState, e: &Env<'_>, code: u16) -> Result<(Live, Out), LiveError> {
    use ti4_engine_core::kernel::hooks::components as c;
    let _ = c::C4R_PLACE;
    pick(s, e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == code))
}

/// Lane F2e: Nekro's ???_ERROR_ERROR_??? places a PDS on a planet the seat controls and exhausts the card.
#[test]
fn nekros_error_error_card_chosen_from_the_menu_places_a_pds_and_exhausts_the_card() {
    use ti4_engine_core::kernel::hooks::components as c;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = nekro_table(|sc| sc);
    let e = env_r(&k);
    let before = ti4_engine_core::query::board::planet_units(&s, planet).len();
    let (live, _) = c4r(&mut s, &e, c::C4R_PLACE).unwrap();
    first_options(&mut s, &e, live, 3);
    let _ = system;
    assert_eq!(ti4_engine_core::query::board::planet_units(&s, planet).len(), before + 1, "a PDS was placed");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::ERRORERROR));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}

/// Lane F2e: the repair mends every damaged unit of the seat (and only the seat's) and exhausts the card; it is listed only with a damaged unit.
#[test]
fn nekros_error_error_card_repairs_the_damaged_units_and_is_listed_only_when_one_is_damaged() {
    use ti4_engine_core::kernel::hooks::components as c;
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = nekro_table(|sc| sc.unit(system, None, Unit::DreadnoughtI, 0, 1).unit(system, None, Unit::DreadnoughtI, 1, 1));
    let e = env_r(&k);
    let listed = |s: &mut CoreState| ask_ask(s, &e).unwrap().unwrap().choice.opts.as_slice().iter().any(|o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::C4R_REPAIR));
    assert!(!listed(&mut s), "nothing is damaged");
    for p in s.board_mut().sys_mut(system).units.as_mut_slice() {
        *p = p.with_sustained(true);
    }
    assert!(listed(&mut s));
    c4r(&mut s, &e, c::C4R_REPAIR).unwrap();
    let units = s.board().sys(system).units.as_slice().to_vec();
    assert!(units.iter().filter(|p| p.owner() == Seat(0)).all(|p| !p.sustained()), "the seat's units are mended");
    assert!(units.iter().filter(|p| p.owner() == Seat(1)).all(|p| p.sustained()), "a rival's are not");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::ERRORERROR));
}

/// Lane F2e: the draw discards a card of the hand, exhausts the card and draws one; it needs a card in the hand and one in the deck.
#[test]
fn nekros_error_error_card_discards_a_card_and_draws_one() {
    use ti4_engine_core::generated::ActionCard;
    use ti4_engine_core::kernel::hooks::components as c;
    let (mut s, k) = nekro_table(|sc| sc.hand(0, &[ActionCard::ALL[0], ActionCard::ALL[3]]));
    s.decks_mut().action = ti4_engine_core::Deck::new(vec![ActionCard::ALL[7]]);
    let mut e = env_r(&k);
    e.table.facts.card_groups = (0..ActionCard::ALL.len()).map(|i| u8::try_from(i % 250).unwrap_or(0)).collect();
    let (live, _) = c4r(&mut s, &e, c::C4R_DRAW).unwrap();
    first_options(&mut s, &e, live, 6);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.cards.hand.as_slice().len(), 2, "one card out, one in");
    assert!(p.cards.hand.as_slice().contains(&ActionCard::ALL[7]), "the deck's card was drawn");
    assert!(p.tech.exhausted.contains(Technology::ERRORERROR));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    // An empty deck takes the option off the menu.
    let (mut t, k) = nekro_table(|sc| sc.hand(0, &[ActionCard::ALL[0]]));
    t.decks_mut().action = ti4_engine_core::Deck::new(vec![]);
    let e = env_r(&k);
    let q = ask_ask(&mut t, &e).unwrap().unwrap().choice;
    assert!(!q.opts.as_slice().iter().any(|o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::C4R_DRAW)));
}

/// Lane F2e: Terraform attaches to a controlled planet that is no home planet and not Mecatol Rex, once; the note turns faceup.
#[test]
fn terraform_chosen_from_the_menu_attaches_to_the_planet_the_option_names() {
    use ti4_engine_core::generated::{Attachment, PromissoryNote};
    use ti4_engine_core::kernel::hooks::components as c;
    use ti4_engine_core::state::NoteKey;
    let (_, planet) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::new(&[Faction::TheFederationOfSol, Faction::TheTitansOfUl, Faction::TheEmiratesOfHacan]).phase(Phase::Action).active(Some(0)).map_ring(1).control(planet, 0).tokens(0, 3, 3, 2).build();
    let note = NoteKey { note: PromissoryNote::Terraform, faction: Faction::TheTitansOfUl };
    s.cards_mut().promissory.edit().insert(note, Seat(0));
    let e = env_r(&k);
    pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::TERRAFORM && a.arg == planet as u16)).unwrap();
    assert!(s.board().planet(planet).attach.as_slice().contains(&Attachment::Terraform));
    assert!(s.cards().promissory_faceup.contains_key(&note), "the note is faceup");
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
    // Once attached, the option is gone.
    let q = ask_ask(&mut s, &e).unwrap().unwrap().choice;
    assert!(!q.opts.as_slice().iter().any(|o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::TERRAFORM)));
}

/// Lane F2e: Cabal's Vortex captures a non-structure unit type of another player that has a unit next to one of the seat's space docks.
#[test]
fn vortex_chosen_from_the_menu_captures_a_unit_type_next_to_a_space_dock_and_exhausts_the_card() {
    use ti4_engine_core::kernel::hooks::components as c;
    let ps = ti4_engine_core_tests::scenario::plain_systems();
    let planet = ps[0].data().planets[0];
    let (mut s, k) = Scenario::new(&[Faction::TheVuilraithCabal, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan])
        .phase(Phase::Action)
        .active(Some(0))
        .map_ring(7)
        .control(planet, 0)
        .unit(ps[0], Some(planet), Unit::SpaceDockI, 0, 1)
        .unit(ps[1], None, Unit::CruiserI, 1, 1)
        .tech(0, Technology::Vortex)
        .tokens(0, 3, 3, 2)
        .build();
    let e = env_r(&k);
    let (live, _) = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Faction(a)) if a.code == c::VORTEX)).unwrap();
    first_options(&mut s, &e, live, 3);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(p.misc.captured_units.as_slice().contains(&(Seat(1), Unit::CruiserI)), "the cruiser type was captured");
    assert!(p.tech.exhausted.contains(Technology::Vortex));
    assert!(plain(&s).contains(&PlainKind::ComponentActionResolved));
}
