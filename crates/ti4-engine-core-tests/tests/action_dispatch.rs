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
    s.players_mut().edit(0).leaders.status.insert(ti4_engine_core::generated::Leader::CarthOfGoldenSands, ti4_engine_core::state::LeaderStatus::Readied);
    let e = env_r(&k);
    let held = s.clone();
    let result = pick(&mut s, &e, |o| matches!(o, Opt::Turn(TuOpt::Leader(_))));
    assert!(matches!(result, Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::Menu(MenuGap::Unrouted(5, _)))))), "{result:?}");
    assert_eq!(s, held, "nothing was written");
}
