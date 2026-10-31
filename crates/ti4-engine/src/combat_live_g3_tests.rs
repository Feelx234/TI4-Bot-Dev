// Lane G3: abilities of the space combat run natively against the string's (`include!`d into `combat_live::tests`, which owns the harness).

/// Nekro (seat `a`) fights Hacan (seat `b`) in the centre of a hub with a planet; the Nekro flagship stands in space with `infantry` infantry and `mechs` mechs on the planet.
fn alastor_table(seed: u64, infantry: usize, mechs: usize) -> (GameState, crate::fixtures::Hub, SystemId) {
    let (hub, _origin, target, planet) = planet_hub();
    let mut state = table(&[("a", "nekro"), ("b", "hacan")]);
    state.rng_seed = seed;
    crate::fixtures::put(&mut state, &target, "nekro_flagship", &a(), 1);
    crate::fixtures::put(&mut state, &target, "cruiser", &a(), 1);
    crate::fixtures::put_on_planet(&mut state, &target, &planet, "infantry", &a(), infantry);
    crate::fixtures::put_on_planet(&mut state, &target, &planet, "nekro_mech", &a(), mechs);
    crate::fixtures::put(&mut state, &target, "dreadnought", &b(), 2);
    crate::fixtures::put(&mut state, &target, "cruiser", &b(), 2);
    (state, hub, target)
}

#[test]
fn the_alastor_asks_per_planet_and_unit_how_many_ground_forces_fight_as_ships_like_the_string() {
    for (seed, infantry, mechs, answers) in [
        (1_u64, 2_usize, 1_usize, vec!["alastor|2", "alastor|1"]),
        (2, 3, 1, vec!["alastor|1", "decline"]),
        (3, 2, 0, vec!["decline"]),
        (4, 1, 1, vec!["alastor|1", "alastor|1"]),
    ] {
        let (state, hub, target) = alastor_table(seed, infantry, mechs);
        let mut script = vec![TACTICAL_ACTION_ID, target.as_str(), "done_moving", "unit:nekro:nekro_flagship:SPACE_COMBAT_STARTED:after"];
        script.extend(answers);
        let diffs = lockstep(state, &hub.galaxy, &script);
        assert!(diffs.is_empty(), "seed {seed}: {diffs:#?}");
    }
}

/// Sardakk (seat `a`) fights Hacan (seat `b`) in the centre of the hub with `exotriremes` Exotrireme IIs (one damaged when `damaged`) and a cruiser; Hacan has `ships` ships.
fn exo_table(seed: u64, exotriremes: usize, damaged: bool, ships: usize) -> (GameState, crate::fixtures::Hub, SystemId) {
    let (hub, _origin, target) = hub();
    let mut state = table(&[("a", "sardakk"), ("b", "hacan")]);
    state.rng_seed = seed;
    crate::technology::grant(&mut state, &a(), &ti4_model::id::TechnologyId::new("exo2"));
    crate::fixtures::put(&mut state, &target, "sardakk_dreadnought2", &a(), exotriremes);
    if damaged {
        if let Some(unit) = state.system_mut(&target).units.iter_mut().find(|u| u.type_id.as_str() == "sardakk_dreadnought2") {
            unit.sustained_damage = true;
        }
    }
    crate::fixtures::put(&mut state, &target, "cruiser", &a(), 1);
    crate::fixtures::put(&mut state, &target, "dreadnought", &b(), 1);
    crate::fixtures::put(&mut state, &target, "cruiser", &b(), ships.saturating_sub(1));
    (state, hub, target)
}

#[test]
fn exotrireme_ii_destroys_itself_and_up_to_two_ships_after_a_round_like_the_string() {
    // Whether the window comes depends on the dice (the combat may be over after the first round): scan seeds, and demand the questions were seen.
    let (mut self_asked, mut victims_asked, mut windows) = (0, 0, 0);
    for seed in 1..=40_u64 {
        let (exotriremes, damaged, ships) = [(1, false, 4), (2, false, 5), (2, true, 4), (1, true, 6)][usize::try_from(seed).unwrap() % 4];
        let (state, hub, target) = exo_table(seed, exotriremes, damaged, ships);
        let script = [TACTICAL_ACTION_ID, target.as_str(), "done_moving"];
        let seen = string_run(&state, &hub.galaxy, &script, 0);
        windows += seen.asked.iter().filter(|q| q.prompt == "after SPACE_COMBAT_ROUND_ENDED").count();
        self_asked += seen.asked.iter().filter(|q| q.prompt == "Exotrireme II: which one is destroyed").count();
        victims_asked += seen.asked.iter().filter(|q| q.prompt.starts_with("Exotrireme II: destroy ship")).count();
        let diffs = lockstep(state, &hub.galaxy, &script);
        assert!(diffs.is_empty(), "seed {seed}: {diffs:#?}");
    }
    assert!(windows > 0 && victims_asked > 0 && self_asked > 0, "the scan saw {windows} windows, {self_asked} self questions, {victims_asked} victim questions");
}
