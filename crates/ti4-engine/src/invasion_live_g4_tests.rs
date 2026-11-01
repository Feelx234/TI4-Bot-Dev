// Lane G4: abilities of the invasion window run natively against the string window (`include!`d into `invasion_live::tests`, which owns the harness).

// -- Magen Defense Grid's hit -----------------------------------------------------------------------------------------------------

/// Seat `a` (Hacan) invades the planet of `b` (Jolnar), who holds the Magen Defense Grid and a PDS there; `a` lands `infantry` infantry and `mechs` mechs, `damaged` of the mechs damaged already.
fn magen_table(seed: u64, infantry: usize, mechs: usize, damaged: bool) -> (GameState, SystemId) {
    let (mut state, system, planet) = game(&[("a", "hacan"), ("b", "jolnar")], seed);
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
    crate::fixtures::put(&mut state, &system, "infantry", &a, infantry);
    if mechs > 0 {
        crate::fixtures::put(&mut state, &system, "hacan_mech", &a, mechs);
        if damaged {
            let units = &mut state.system_mut(&system).units;
            if let Some(mech) = units.iter_mut().find(|u| u.type_id.as_str() == "hacan_mech") {
                mech.sustained_damage = true;
            }
        }
    }
    state.system_mut(&system).set_control(planet.clone(), b.clone());
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &b, 1);
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
    crate::technology::grant(&mut state, &b, &ti4_model::id::TechnologyId::new("md"));
    (state, system)
}

/// The hit goes to the option `seed % n` of the owner's question; the victim sustains (when asked) on odd seeds; the rest is the first option.
fn magen_pick(seed: u64) -> Box<dyn FnMut(&Choice) -> usize> {
    Box::new(move |c| {
        if c.prompt.starts_with("Magen Defense Grid") {
            usize::try_from(seed).unwrap() % c.options.len()
        } else if c.prompt.starts_with("sustain the hit") {
            c.options.iter().position(|o| o.id == if seed % 2 == 1 { "sustain" } else { "decline" }).unwrap_or(0)
        } else {
            0
        }
    })
}

#[test]
fn magen_defense_grid_hits_a_ground_force_the_owner_assigns_and_the_victim_may_sustain_like_the_string() {
    let _guard = crate::dual::tests::serial();
    let ignore = ["GROUND_FORCE_DESTROYED", "GROUND_FORCE_SUSTAINED"];
    let mut asked_hit = 0;
    let mut asked_sustain = 0;
    // (infantry, mechs, a mech damaged): several kinds (a question), infantry only (no question, no sustain), a lone mech (no question, a sustain), a damaged mech beside infantry.
    for (seed, (infantry, mechs, damaged)) in (1..=8_u64).zip([(3, 1, false), (3, 1, false), (3, 0, false), (3, 0, false), (0, 1, false), (0, 1, false), (2, 1, true), (2, 1, true)]) {
        let (state, system) = magen_table(seed, infantry, mechs, damaged);
        let string = string_run(state.clone(), seed, &system, "a", false, magen_pick(seed));
        asked_hit += string.asked.iter().filter(|c| c.prompt.starts_with("Magen Defense Grid")).count();
        asked_sustain += string.asked.iter().filter(|c| c.prompt.starts_with("sustain the hit")).count();
        check_ignoring(&format!("magen {seed}"), &state, seed, &system, false, magen_pick(seed), &ignore);
    }
    assert!(asked_hit > 0 && asked_sustain > 0, "both questions were seen ({asked_hit}, {asked_sustain})");
}

#[test]
fn magen_defense_grid_needs_a_structure_on_the_planet_and_asks_nothing_when_there_is_no_ground_force() {
    use crate::dual::{take_report, with_bail_is_failure};
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    // No structure of the holder on the planet: no hit.
    let (mut state, system) = magen_table(3, 3, 1, false);
    for planet_units in state.system_mut(&system).planet_units.values_mut() {
        planet_units.retain(|u| u.type_id.as_str() != "pds");
    }
    let string = string_run(state.clone(), 3, &system, "a", false, magen_pick(3));
    assert!(!string.asked.iter().any(|c| c.prompt.starts_with("Magen Defense Grid")), "no structure, no hit");
    with_bail_is_failure(true, || check_ignoring("magen without a structure", &state, 3, &system, false, magen_pick(3), &["GROUND_FORCE_DESTROYED", "GROUND_FORCE_SUSTAINED"]));
    let _ = take_report();
}
