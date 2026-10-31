// Lane G3: abilities of the invasion window run natively against the string window (`include!`d into `invasion_live::tests`, which owns the harness).

/// The events an ability announces inside the window (the native run lists the top-level emissions only).
const NESTED: &[&str] = &["TRADE_GOODS_GAINED"];

/// A Saar invader (seat `a`) takes the planet of seat `b` with `goods` trade goods; `mechs_out` of its mechs stand elsewhere (the box shrinks).
fn saar_conquest(seed: u64, goods: i32, mechs_out: usize) -> (GameState, SystemId) {
    let (mut state, system, planet) = game(&[("a", "saar"), ("b", "jolnar")], seed);
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
    crate::fixtures::put(&mut state, &system, "infantry", &a, 6);
    state.system_mut(&system).set_control(planet.clone(), b.clone());
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
    state.player_mut(&a).unwrap().trade_goods = goods;
    if mechs_out > 0 {
        let elsewhere = SystemId::new(crate::seating::MECATOL);
        crate::fixtures::put(&mut state, &elsewhere, "saar_mech", &a, mechs_out);
    }
    (state, system)
}

#[test]
fn scavenger_zeta_is_offered_after_a_conquest_and_deploys_a_mech_like_the_string() {
    let _guard = crate::dual::tests::serial();
    let mut asked = 0;
    for (seed, answer) in (1..=6_u64).zip(["use", "decline"].into_iter().cycle()) {
        let (state, system) = saar_conquest(seed, 2, 0);
        let pick: &'static [&'static str] = if answer == "use" { &["unit:saar:saar_mech"] } else { &["decline"] };
        asked += check_ignoring(&format!("zeta {answer} {seed}"), &state, seed, &system, false, prefer(pick), NESTED);
    }
    assert!(asked > 6);
}

#[test]
fn scavenger_zeta_is_offered_only_with_a_mech_in_the_box_and_after_scavenge_paid_the_trade_good() {
    use crate::dual::{take_report, with_bail_is_failure};
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    for (seed, goods, out) in [(1_u64, 0, 0), (2, 3, 4), (3, 1, 3)] {
        let (state, system) = saar_conquest(seed, goods, out);
        let string = string_run(state.clone(), seed, &system, "a", false, prefer(&["unit:saar:saar_mech"]));
        let zeta = string.asked.iter().any(|c| c.options.iter().any(|o| o.id.starts_with("unit:saar:saar_mech")));
        // Scavenge runs first and pays the good the deploy costs: no trade good at the start is no obstacle.
        assert_eq!(zeta, out < 4, "the string offers it with a mech left in the box ({goods}, {out})");
        with_bail_is_failure(true, || check_ignoring(&format!("zeta gate {seed}"), &state, seed, &system, false, prefer(&["unit:saar:saar_mech"]), NESTED));
    }
    let _ = take_report();
}
