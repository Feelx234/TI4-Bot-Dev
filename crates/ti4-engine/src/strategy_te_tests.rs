//! Lockstep tests of B8c-3: Imperial's primary scoring a public objective (the price paid by the core window, the
//! tail by the string) and the primaries of the Thunder's Edge Warfare and Construction (`te6warfare`,
//! `te4construction`). Each plays the real card on a real seated game under the `Dual` strategy backend, so the
//! string and the core windows are compared question by question and position by position, and then reads the
//! string position to see that the effect happened.

use ti4_content::ContentStore;
use ti4_model::content_types::{DEFAULT, POK};
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, Phase};

use crate::choice::{Scripted, Table};
use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

fn a() -> PlayerId {
    PlayerId::new("a")
}

fn content() -> &'static ContentStore {
    ContentStore::embedded()
}

/// Play `card` of `a` under the Dual backend on a seated game (default sources), the table answering `answers`:
/// `(compared, bailed)` of `label`, what the card returned (`None` when an answer was refused) and the position after.
fn play(
    label: &str,
    setup: impl FnOnce(&mut GameState),
    answers: &[&str],
    card: &str,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> ((u64, u64), Option<crate::strategy_cards::Ability>, GameState) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let mut out = None;
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        let mut state = crate::fixtures::seated_game(&F3, DEFAULT);
        crate::promissory::deal(&mut state, content(), DEFAULT);
        state.phase = Phase::Action;
        state.active = Some(a());
        setup(&mut state);
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|x| (*x).to_owned()))));
        let ability = crate::strategy_cards::primary(&mut state, content(), DEFAULT, galaxy, &mut table, &a(), card).ok();
        out = Some((ability, state));
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let counts = crate::dual::thread_window_counts(label);
    let _ = take_report();
    let (ability, state) = out.expect("played");
    (counts, ability, state)
}

fn galaxy() -> ti4_content::galaxy::Galaxy {
    let ids: Vec<String> = ti4_content::galaxy::all_systems(content(), POK)
        .iter()
        .filter(|(_, system)| !system.is_anomaly() && !system.is_hyperlane())
        .map(|(id, _)| (*id).to_owned())
        .take(7)
        .collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    ti4_content::galaxy::Galaxy::build(content(), &refs, DEFAULT, 1).unwrap()
}

// -- Imperial ---------------------------------------------------------------------------------------------

const IMPERIAL: &str = "strategy_cards::imperial_primary";

fn scoreable(s: &mut GameState, objective: &str) {
    s.revealed_objectives.push(ObjectiveId::new(objective));
}

#[test]
fn imperial_scores_a_bought_objective_paying_its_price_in_lockstep() {
    // Lead From the Front: three tokens of the strategy and tactic pools.
    let lead = |s: &mut GameState| scoreable(s, "lead");
    let mut probe = crate::fixtures::seated_game(&F3, DEFAULT);
    lead(&mut probe);
    let pools = |s: &GameState| {
        let p = s.player(&a()).unwrap();
        p.strategic_tokens + p.tactic_tokens
    };
    let (counts, _, after) = play(IMPERIAL, lead, &["lead"], "pok8imperial", None);
    assert_eq!(counts, (1, 0));
    assert_eq!(pools(&after), pools(&probe) - 3, "the tokens were paid");
    assert!(after.scored_by(&a()).contains(&ObjectiveId::new("lead")), "and the objective is scored");
    assert!(after.player(&a()).unwrap().victory_points >= 2 || !after.player(&a()).unwrap().secret_objectives.is_empty(), "the objective's point, and the ability's secret");
    // Declined: nothing is paid or scored.
    let (counts, _, after) = play(IMPERIAL, lead, &["decline"], "pok8imperial", None);
    assert_eq!(counts, (1, 0));
    assert_eq!(pools(&after), pools(&probe));
    assert!(after.scored_by(&a()).is_empty());
}

#[test]
fn imperial_pays_trade_goods_and_the_first_plan_for_the_other_prices() {
    let goods = |s: &mut GameState| {
        scoreable(s, "trade_routes");
        s.player_mut(&a()).unwrap().trade_goods = 6;
    };
    let (counts, _, after) = play(IMPERIAL, goods, &["trade_routes"], "pok8imperial", None);
    assert_eq!(counts, (1, 0));
    assert_eq!(after.player(&a()).unwrap().trade_goods, 1, "five trade goods paid");
    assert!(after.scored_by(&a()).contains(&ObjectiveId::new("trade_routes")));
    // Eight influence: the first plan of planets and goods that pays it.
    let influence = |s: &mut GameState| {
        scoreable(s, "sway_council");
        s.player_mut(&a()).unwrap().trade_goods = 8;
    };
    let (counts, _, after) = play(IMPERIAL, influence, &["sway_council"], "pok8imperial", None);
    assert_eq!(counts, (1, 0));
    assert!(after.scored_by(&a()).contains(&ObjectiveId::new("sway_council")));
}

#[test]
fn imperial_with_the_mecatol_point_and_a_scored_objective_reaches_both() {
    let mecatol = |s: &mut GameState| {
        scoreable(s, "lead");
        s.system_mut(&SystemId::new("18")).set_control(PlanetId::new("mr"), a());
    };
    let (counts, _, after) = play(IMPERIAL, mecatol, &["lead"], "pok8imperial", None);
    assert_eq!(counts, (1, 0));
    assert_eq!(after.player(&a()).unwrap().victory_points, 2, "the objective's point and Mecatol's");
}

#[test]
fn bail_imperial_scoring_where_the_price_or_the_target_is_the_strings() {
    // Past the target with the point of the objective: the string clamps.
    let near = |s: &mut GameState| {
        scoreable(s, "lead");
        s.player_mut(&a()).unwrap().victory_points = 10;
    };
    let (counts, _, _) = play(IMPERIAL, near, &["lead"], "pok8imperial", None);
    assert_eq!(counts, (0, 1));
    // A Last Bastion at the table: the production kernel does not follow its economy.
    let goods = |s: &mut GameState| {
        scoreable(s, "trade_routes");
        s.player_mut(&a()).unwrap().trade_goods = 6;
    };
    let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "bastion")], DEFAULT);
    goods(&mut state);
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        crate::promissory::deal(&mut state, content(), DEFAULT);
        let mut table = Table::with_default(Box::new(Scripted::new(["trade_routes".to_owned()])));
        let _ = crate::strategy_cards::primary(&mut state, content(), DEFAULT, None, &mut table, &a(), "pok8imperial");
    });
    assert_eq!(crate::dual::thread_window_counts(IMPERIAL), (0, 1), "a Bastion table keeps the price with the string");
    assert_eq!(report().total_differences(), 0);
    let _ = take_report();
}

// -- Thunder's Edge Warfare -----------------------------------------------------------------------------

const WARFARE: &str = "strategy_cards::te6_warfare";

fn same_split(s: &GameState) -> String {
    let p = s.player(&a()).unwrap();
    format!("{}|{}|{}", p.tactic_tokens, p.fleet_tokens, p.strategic_tokens)
}

#[test]
fn the_te_warfare_redistributes_and_names_the_system_of_the_free_action_in_lockstep() {
    let galaxy = galaxy();
    let mut probe = crate::fixtures::seated_game(&F3, DEFAULT);
    probe.phase = Phase::Action;
    let split = same_split(&probe);
    let system = galaxy.system_ids_holding_things().into_iter().next().expect("the map has a system").to_owned();
    let (counts, ability, _) = play(WARFARE, |_| {}, &[&split, &system], "te6warfare", Some(&galaxy));
    assert_eq!(counts, (1, 0));
    assert_eq!(ability, Some(crate::strategy_cards::Ability::FreeTactical(SystemId::new(system.clone()))));
    // A different split is applied (one tactic token moved to the strategy pool).
    let p = probe.player(&a()).unwrap();
    let moved = format!("{}|{}|{}", p.tactic_tokens - 1, p.fleet_tokens, p.strategic_tokens + 1);
    let (counts, _, after) = play(WARFARE, |_| {}, &[&moved, &system], "te6warfare", Some(&galaxy));
    assert_eq!(counts, (1, 0));
    assert_eq!(same_split(&after), moved);
    // Nothing on the sheet: no redistribution is asked, only the system.
    let empty = |s: &mut GameState| {
        let p = s.player_mut(&a()).unwrap();
        (p.tactic_tokens, p.fleet_tokens, p.strategic_tokens) = (0, 0, 0);
    };
    let (counts, ability, _) = play(WARFARE, empty, &[&system], "te6warfare", Some(&galaxy));
    assert_eq!(counts, (1, 0));
    assert!(matches!(ability, Some(crate::strategy_cards::Ability::FreeTactical(_))));
}

#[test]
fn a_letnev_may_empty_the_fleet_pool_before_the_te_warfare() {
    let galaxy = galaxy();
    let letnev = [("a", "letnev"), ("b", "hacan")];
    let mut state = crate::fixtures::seated_game(&letnev, DEFAULT);
    state.phase = Phase::Action;
    let p = state.player(&a()).unwrap();
    let emptied = format!("{}|0|{}", p.tactic_tokens + p.fleet_tokens, p.strategic_tokens);
    let system = galaxy.system_ids_holding_things().into_iter().next().unwrap().to_owned();
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        crate::promissory::deal(&mut state, content(), DEFAULT);
        let mut table = Table::with_default(Box::new(Scripted::new([emptied.clone(), system.clone()])));
        crate::strategy_cards::primary(&mut state, content(), DEFAULT, Some(&galaxy), &mut table, &a(), "te6warfare").unwrap();
    });
    assert_eq!(crate::dual::thread_window_counts(WARFARE), (1, 0));
    assert_eq!(report().total_differences(), 0);
    assert_eq!(state.player(&a()).unwrap().fleet_tokens, 0);
    let _ = take_report();
}

// -- Thunder's Edge Construction --------------------------------------------------------------------------

const CONSTRUCTION: &str = "strategy_cards::te4_construction";

fn lodor(s: &mut GameState) {
    s.system_mut(&SystemId::new("26")).set_control(PlanetId::new("lodor"), a());
}

#[test]
fn the_te_construction_places_two_structures_neither_limited_to_a_pds_in_lockstep() {
    let (counts, _, after) = play(CONSTRUCTION, lodor, &["structure", "pds|01|jord", "spacedock|26|lodor"], "te4construction", None);
    assert_eq!(counts, (1, 0));
    let units_on = |s: &GameState, system: &str, planet: &str, kind: &str| {
        s.board
            .get(&SystemId::new(system))
            .and_then(|b| b.planet_units.get(&PlanetId::new(planet)))
            .map_or(0, |u| u.iter().filter(|u| u.owner == a() && u.type_id.as_str() == kind).count())
    };
    assert!(units_on(&after, "26", "lodor", "spacedock") == 1, "the second structure may be a dock");
    // Declining the first and taking the second; declining both.
    let (counts, _, _) = play(CONSTRUCTION, lodor, &["structure", "decline", "spacedock|26|lodor"], "te4construction", None);
    assert_eq!(counts, (1, 0));
    let (counts, _, _) = play(CONSTRUCTION, lodor, &["structure", "decline", "decline"], "te4construction", None);
    assert_eq!(counts, (1, 0));
}

#[test]
fn bail_the_te_construction_production_keeps_the_nested_window_with_the_string() {
    let galaxy = galaxy();
    let _ = galaxy;
    let (counts, _, _) = play(CONSTRUCTION, lodor, &["produce|01", "done", "pds|01|jord"], "te4construction", None);
    assert_eq!(counts, (0, 1), "the production of the card is a whole window: the string's");
}
