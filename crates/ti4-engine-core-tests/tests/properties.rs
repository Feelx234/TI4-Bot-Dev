//! P1-P6 (docs/todo/D4-core-tests.md 5.2) over random positions and pick scripts, on three leader windows (the pilot: the same
//! functions are reused by every rule-area file for its own windows). 64 cases each (`PROPTEST_CASES` raises it).

use proptest::prelude::*;
use ti4_engine_core::generated::Planet;
use ti4_engine_core::rules::leader_windows::{HacanAgent, Helmsman, XxchaAgent};
use ti4_engine_core_tests::generators::{arb_play, arb_scenario, config};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::props;
use ti4_engine_core_tests::scenario::Op;

fn xxcha(sc: Scenario) -> (Scenario, XxchaAgent) {
    // Exhaust every planet the position gives to seat 0, so that the window sees 0, 1 or several.
    let planets: Vec<Planet> = sc.ops().iter().filter_map(|o| if let Op::Control { planet, seat: 0 } = o { Some(*planet) } else { None }).collect();
    let mut sc = sc;
    for p in planets {
        sc = sc.exhaust(p);
    }
    (sc, XxchaAgent::new(Seat(0), false))
}

fn hacan(sc: Scenario) -> (Scenario, HacanAgent) {
    (sc, HacanAgent::new(Seat(0)))
}

fn helmsman(sc: Scenario) -> (Scenario, Helmsman) {
    (sc, Helmsman::new(Seat(0), true))
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn clone_equals_and_diverges_independently(sc in arb_scenario(), script in arb_play()) {
        props::p1_clone_independence(&xxcha, sc.clone(), &script);
        props::p1_clone_independence(&hacan, sc.clone(), &script);
        props::p1_clone_independence(&helmsman, sc, &script);
    }

    #[test]
    fn driver_rollback_is_exact(sc in arb_scenario(), script in arb_play()) {
        props::p2_rollback_is_exact(&xxcha, sc.clone(), &script);
        props::p2_rollback_is_exact(&hacan, sc.clone(), &script);
        props::p2_rollback_is_exact(&helmsman, sc, &script);
    }

    #[test]
    fn replay_is_deterministic(sc in arb_scenario(), script in arb_play()) {
        props::p3_replay_is_deterministic(&xxcha, sc.clone(), &script);
        props::p3_replay_is_deterministic(&hacan, sc.clone(), &script);
        props::p3_replay_is_deterministic(&helmsman, sc, &script);
    }

    #[test]
    fn options_are_sound(sc in arb_scenario(), script in arb_play()) {
        props::p4_options_are_sound(&xxcha, sc.clone(), &script);
        props::p4_options_are_sound(&hacan, sc.clone(), &script);
        props::p4_options_are_sound(&helmsman, sc, &script);
    }

    #[test]
    fn option_order_is_stable(sc in arb_scenario(), script in arb_play(), shuffle in any::<u64>()) {
        props::p5_option_order_is_stable(&xxcha, sc.clone(), &script, shuffle);
        props::p5_option_order_is_stable(&hacan, sc.clone(), &script, shuffle);
        props::p5_option_order_is_stable(&helmsman, sc, &script, shuffle);
    }

    #[test]
    fn conservation(sc in arb_scenario(), script in arb_play()) {
        props::p6_conservation(&xxcha, sc.clone(), &script);
        props::p6_conservation(&hacan, sc.clone(), &script);
        props::p6_conservation(&helmsman, sc, &script);
    }
}

/// The scenario generator must produce what it promises (a vacuous property would pass on empty positions).
#[test]
fn the_generator_makes_positions_with_pieces_and_controlled_planets() {
    let mut pieces = 0;
    let mut controlled = 0;
    for seed in 0..40 {
        let sc = ti4_engine_core_tests::generators::random_scenario(2 + (seed % 7) as usize, seed);
        for op in sc.ops() {
            match op {
                Op::Unit { n, .. } => pieces += n,
                Op::Control { .. } => controlled += 1,
                _ => {}
            }
        }
        let s = sc.state();
        assert!(ti4_engine_core_tests::inv::conservation(&s, &s).is_ok());
    }
    assert!(pieces > 100 && controlled > 20, "{pieces} pieces, {controlled} controlled planets");
}
