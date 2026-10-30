//! P1-P6 on the windows of the economy, strategy, explore and relic areas (D4-2a/2b), over random positions and pick scripts.

use proptest::prelude::*;
use ti4_engine_core::generated::{ExploreType, Planet};
use ti4_engine_core::rules::explore_windows::{ChooseDeck, IonStorm};
use ti4_engine_core::rules::relic_windows::{StellarConverter, TitanPrototype};
use ti4_engine_core::rules::strategy_primaries::{DMode, Diplomacy, WarfarePrimary};
use ti4_engine_core::rules::strategy_windows::TradePrimary;
use ti4_engine_core_tests::generators::{arb_play, arb_scenario, config};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::props;
use ti4_engine_core_tests::scenario::{Op, plain_systems, system_of};

fn controlled_by_zero(sc: &Scenario) -> Vec<Planet> {
    sc.ops().iter().filter_map(|o| if let Op::Control { planet, seat: 0 } = o { Some(*planet) } else { None }).collect()
}

fn trade(sc: Scenario) -> (Scenario, TradePrimary) {
    (sc, TradePrimary::new(Seat(0)))
}

fn warfare(sc: Scenario) -> (Scenario, WarfarePrimary) {
    let systems: Vec<System> = sc.ops().iter().filter_map(|o| if let Op::Unit { system, seat: 0, .. } = o { Some(*system) } else { None }).collect();
    let mut sc = sc;
    for (i, s) in systems.iter().enumerate() {
        if systems[..i].contains(s) {
            continue;
        }
        sc = sc.command_token(*s, 0);
    }
    (sc, WarfarePrimary::new(Seat(0), false))
}

fn diplomacy(sc: Scenario) -> (Scenario, Diplomacy) {
    let mut sc = sc;
    for p in controlled_by_zero(&sc.clone()) {
        sc = sc.exhaust(p);
    }
    (sc, Diplomacy::new(Seat(0), StrategyCard::Diplomacy, DMode::Primary { base_printing: false }))
}

fn converter(sc: Scenario) -> (Scenario, StellarConverter) {
    let mut targets: Vec<(System, Planet)> = controlled_by_zero(&sc).into_iter().map(|p| (system_of(p), p)).collect();
    // The order of the targets is the caller's: the same for any construction order of the position.
    targets.sort();
    (sc, StellarConverter::new(Seat(0), &targets))
}

fn titan(sc: Scenario) -> (Scenario, TitanPrototype) {
    (sc, TitanPrototype::new(Seat(0)))
}

fn deck(sc: Scenario) -> (Scenario, ChooseDeck) {
    (sc, ChooseDeck::new(Seat(0), &[ExploreType::Cultural, ExploreType::Hazardous, ExploreType::Industrial]))
}

fn ion(sc: Scenario) -> (Scenario, IonStorm) {
    (sc, IonStorm::new(Seat(0), plain_systems()[0]))
}

macro_rules! all_properties {
    ($sc:expr, $script:expr, $shuffle:expr, $($make:ident),*) => {{
        $( props::check_all(&$make, &$sc, &$script, $shuffle); )*
    }};
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_economy_and_strategy_windows_keep_the_contract(sc in arb_scenario(), script in arb_play(), shuffle in any::<u64>()) {
        all_properties!(sc, script, shuffle, trade, warfare, diplomacy);
    }

    #[test]
    fn the_explore_and_relic_windows_keep_the_contract(sc in arb_scenario(), script in arb_play(), shuffle in any::<u64>()) {
        all_properties!(sc, script, shuffle, converter, titan, deck, ion);
    }
}
