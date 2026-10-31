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

// -- Ragh's Call ------------------------------------------------------------------------------------------------------------------

/// Seat `a` (Hacan) invades the planet of seat `b` (Saar) holding the Saar's Ragh's Call; the Saar holds `others` more planets to retreat to.
fn ragh_table(seed: u64, others: usize, holder_is_invader: bool) -> (GameState, SystemId) {
    let (mut state, system, planet) = game(&[("a", "hacan"), ("b", "saar")], seed);
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
    crate::fixtures::put(&mut state, &system, "infantry", &a, 6);
    state.system_mut(&system).set_control(planet.clone(), b.clone());
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "saar_mech", &b, 1);
    // The Saar's home planets are the planets it can retreat to (every other planet it controls).
    for (index, (home_system, home_planet)) in state.controlled_planets(&b).into_iter().filter(|(_, p)| **p != planet).map(|(s, p)| (s.clone(), p.clone())).collect::<Vec<_>>().into_iter().enumerate() {
        if index >= others {
            state.system_mut(&home_system).planet_control.remove(&home_planet);
        }
    }
    state.promissory_notes.insert("ragh:saar".to_owned(), if holder_is_invader { a } else { b });
    (state, system)
}

#[test]
fn ragh_s_call_moves_the_saar_ground_forces_to_a_planet_the_saar_picks_like_the_string() {
    let _guard = crate::dual::tests::serial();
    let mut asked = 0;
    for seed in 1..=6_u64 {
        let (state, system) = ragh_table(seed, 1 + usize::try_from(seed % 2).unwrap(), true);
        let pick: Box<dyn FnMut(&Choice) -> usize> = if seed % 3 == 0 {
            // declined
            prefer(&["decline"])
        } else {
            Box::new(move |c| {
                if c.prompt.starts_with("Ragh's Call") {
                    usize::try_from(seed).unwrap() % c.options.len()
                } else {
                    c.options.iter().position(|o| o.id.starts_with("promissory:hacan:ragh")).unwrap_or(0)
                }
            })
        };
        let string = string_run(state.clone(), seed, &system, "a", false, Box::new(|_| 0));
        let _ = string;
        asked += check(&format!("ragh {seed}"), &state, seed, &system, false, pick);
    }
    assert!(asked > 6);
}

// -- Tekklar Legion ---------------------------------------------------------------------------------------------------------------

/// Seat `a` (Hacan) invades the planet of `b` holding the Sardakk's Tekklar Legion; `b` is the N'orr player, or Jolnar with Sardakk as a third seat.
fn tekklar_table(seed: u64, norr_defends: bool) -> (GameState, SystemId) {
    let seats: &[(&str, &str)] = if norr_defends { &[("a", "hacan"), ("b", "sardakk")] } else { &[("a", "hacan"), ("b", "jolnar"), ("c", "sardakk")] };
    let (mut state, system, planet) = game(seats, seed);
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
    crate::fixtures::put(&mut state, &system, "infantry", &a, 5);
    state.system_mut(&system).set_control(planet.clone(), b.clone());
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 4);
    state.promissory_notes.insert("tekklar:sardakk".to_owned(), a);
    (state, system)
}

#[test]
fn tekklar_legion_shifts_the_rolls_of_the_combat_it_was_played_into_like_the_string() {
    let _guard = crate::dual::tests::serial();
    let mut asked = 0;
    for seed in 1..=8_u64 {
        let (state, system) = tekklar_table(seed, seed % 2 == 0);
        let pick = if seed % 4 < 2 { prefer(&["promissory:hacan:tekklar"]) } else { prefer(&["decline"]) };
        asked += check(&format!("tekklar {seed}"), &state, seed, &system, false, pick);
    }
    assert!(asked > 8);
}
