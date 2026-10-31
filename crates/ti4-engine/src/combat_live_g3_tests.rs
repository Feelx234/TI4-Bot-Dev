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
