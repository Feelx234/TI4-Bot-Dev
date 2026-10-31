//! Lane G: the formerly silent abilities (`ti4_engine_core_factions::silent`) run natively through `live::live_step` against the string bodies they
//! replace, in the turn-flow harness of lane B (`turn_live_tests`): the questions as rendered, the answers, the events and the position must agree.

use ti4_content::ContentStore;
use ti4_model::id::{PlanetId, SystemId, TechnologyId};
use ti4_model::state::{GameState, Phase as StringPhase};

use super::turn_live_tests::{action_table, assert_same, based, drafted, phase_both, pid, prelude_both};

/// Seat `who` controls every planet of `system`.
fn control_all(state: &mut GameState, who: &str, system: &SystemId) {
    let planets = ti4_content::galaxy::planets_in(ContentStore::embedded(), system.as_str(), ti4_model::content_types::DEFAULT);
    for planet in planets {
        state.system_mut(system).set_control(PlanetId::new(planet.id()), pid(who));
    }
}

fn home_of(state: &GameState, who: &str) -> SystemId {
    state.player(&pid(who)).and_then(|p| p.home_system.clone()).expect("a home system")
}

// -- Gift of Prescience -----------------------------------------------------------------------------------------------------------

/// A table at the end of the strategy phase where seat `c` (Hacan) holds the Naalu's Gift of Prescience.
fn gift_table() -> GameState {
    let mut state = drafted(&[("a", "sol"), ("b", "naalu"), ("c", "hacan")]);
    state.promissory_notes.insert("gift:naalu".to_owned(), pid("c"));
    state
}

#[test]
fn gift_of_prescience_is_offered_at_the_end_of_the_strategy_phase_like_the_string() {
    for answer in ["promissory:hacan:gift:STRATEGY_PHASE_ENDED:after", "decline"] {
        let state = gift_table();
        let string = super::turn_live_tests::string_run(&state, &[answer], |g| {
            let result = g.step_phase();
            assert!(result.error.is_none(), "the phase moves: {:?}", result.error);
        });
        let native = super::turn_live_tests::native_steps(&state, &[(ti4_engine_core::driver::session::Arm::Phase, None)], false, &[answer]);
        assert_same(&string, &native);
        assert_eq!(string.questions.len(), 1, "the holder is asked once: {answer}");
    }
}

#[test]
fn a_gift_played_this_round_switches_telepathic_off_like_the_string() {
    let state = gift_table();
    let (string, native) = {
        let string = super::turn_live_tests::string_run(&state, &["promissory:hacan:gift:STRATEGY_PHASE_ENDED:after"], |g| {
            let _ = g.step_phase();
        });
        let native = super::turn_live_tests::native_steps(&state, &[(ti4_engine_core::driver::session::Arm::Phase, None)], false, &["promissory:hacan:gift:STRATEGY_PHASE_ENDED:after"]);
        (string, native)
    };
    assert_same(&string, &native);
    // The Naalu "0" token sits on the holder's card, not on the Naalu's: the holder is first, the Naalu keeps its own initiative.
    assert!(string.state["initiative_overrides"].get("c").is_some() && string.state["initiative_overrides"].get("b").is_none(), "{}", string.state["initiative_overrides"]);
    // Without the note in other hands the phase ends with no question.
    let plain = drafted(&[("a", "sol"), ("b", "naalu"), ("c", "hacan")]);
    let (string, native) = phase_both(&plain);
    assert_same(&string, &native);
    assert!(string.questions.is_empty());
}

// -- the units that wait on a card ------------------------------------------------------------------------------------------------

#[test]
fn infantry_ii_units_on_the_card_land_on_the_first_planet_of_the_home_system_like_the_string() {
    for (marked, owns) in [("2", true), ("1", true), ("2", false)] {
        let mut state = crate::fixtures::seated_dealt(&[("a", "hacan"), ("b", "sol"), ("c", "arborec")], ti4_model::content_types::DEFAULT);
        state.phase = StringPhase::Action;
        state.active = Some(pid("a"));
        if owns {
            crate::technology::grant(&mut state, &pid("a"), &TechnologyId::new("inf2"));
        }
        state.faction_marks.insert("inf2:card:a".to_owned(), marked.to_owned());
        let state = based(state);
        let (string, native) = prelude_both(&state, &[]);
        assert_same(&string, &native);
        assert_eq!(string.state["faction_marks"].get("inf2:card:a").is_some(), false, "the card is empty again ({marked}, owns {owns})");
    }
}

/// `who`'s home system is `system`, whose planets it controls (a home system with several planets).
fn homed_in(state: &mut GameState, who: &str, system: &SystemId) {
    state.player_mut(&pid(who)).unwrap().home_system = Some(system.clone());
    control_all(state, who, system);
}

#[test]
fn letani_warriors_and_crimson_legionnaires_ask_where_they_land_like_the_string() {
    for (faction, mark) in [("arborec", "arborec:lw2:a"), ("mahact", "mahact:cl2:a")] {
        let probe = crate::fixtures::seated_dealt(&[("a", faction), ("b", "hacan"), ("c", "sol")], ti4_model::content_types::DEFAULT);
        let hacan_home = home_of(&probe, "b");
        let planets: Vec<String> = ti4_content::galaxy::planets_in(ContentStore::embedded(), hacan_home.as_str(), ti4_model::content_types::DEFAULT).iter().map(|p| p.id().to_owned()).collect();
        assert!(planets.len() >= 2, "{planets:?}");
        for (count, picks) in [(1usize, vec![planets[1].as_str()]), (2, vec![planets[0].as_str(), planets[1].as_str()]), (2, vec![planets[1].as_str(), planets[1].as_str()])] {
            let mut state = probe.clone();
            state.phase = StringPhase::Action;
            state.active = Some(pid("a"));
            homed_in(&mut state, "a", &hacan_home);
            state.faction_marks.insert(mark.to_owned(), count.to_string());
            let state = based(state);
            let (string, native) = prelude_both(&state, &picks);
            assert_same(&string, &native);
            assert_eq!(string.questions.len(), count, "one question per unit ({faction})");
        }
        // One planet: nothing is asked.
        let mut state = probe.clone();
        state.phase = StringPhase::Action;
        state.active = Some(pid("a"));
        state.faction_marks.insert(mark.to_owned(), "2".to_owned());
        let state = based(state);
        let (string, native) = prelude_both(&state, &[]);
        assert_same(&string, &native);
    }
}

// -- Mitosis ----------------------------------------------------------------------------------------------------------------------

#[test]
fn mitosis_asks_where_the_infantry_goes_at_the_start_of_the_status_phase_like_the_string() {
    let mut state = drafted(&[("a", "sol"), ("b", "arborec"), ("c", "hacan")]);
    state.phase = StringPhase::Action;
    state.active = None;
    for p in ["a", "b", "c"] {
        state.player_mut(&pid(p)).unwrap().passed = true;
    }
    let home = home_of(&state, "b");
    let planet = ti4_content::galaxy::planets_in(ContentStore::embedded(), home.as_str(), ti4_model::content_types::DEFAULT)[0].id().to_owned();
    let state = based(state);
    // The question lists the placement and the replacement of the infantry the home planet holds; every way is the same in both engines.
    for answer in [format!("infantry|{home}|{planet}"), format!("mech|{home}|{planet}")] {
        let (string, native) = {
            let string = super::turn_live_tests::string_run(&state, &[answer.as_str()], |g| {
                let _ = g.step_phase();
            });
            let native = super::turn_live_tests::native_steps(&state, &[(ti4_engine_core::driver::session::Arm::Phase, None)], false, &[answer.as_str()]);
            (string, native)
        };
        assert_same(&string, &native);
        assert_eq!(string.questions.len(), 1, "{answer}");
    }
}

// -- Winnu (lane G2): Reclamation, the Reclaimer, the marks, Hegemonic Trade Policy and Berekar Berekon -----------------------------------------

/// `assert_same` looks at the source and the subtype of a question's context; the questions of these abilities agree on the whole context (optional, battle,
/// invasion, target, trigger).
fn assert_same_contexts(string: &super::turn_live_tests::Run, native: &super::turn_live_tests::Run) {
    let contexts = |run: &super::turn_live_tests::Run| run.questions.iter().map(|q| serde_json::to_value(&q.context).expect("serializes")).collect::<Vec<_>>();
    assert_eq!(contexts(string), contexts(native), "the contexts of the questions");
}

use ti4_engine_core::driver::turn::open_job;
use ti4_engine_core::state::TurnJob;

/// A table in the action phase where seat `a` (Winnu) took Mecatol Rex this turn: it controls the planet and the notes name it.
fn winnu_took_mecatol() -> GameState {
    let mut state = crate::fixtures::seated_dealt(&[("a", "winnu"), ("b", "hacan"), ("c", "sol")], ti4_model::content_types::DEFAULT);
    state.phase = StringPhase::Action;
    state.active = Some(pid("a"));
    let mecatol = SystemId::new(crate::seating::MECATOL);
    state.system_mut(&mecatol).set_control(PlanetId::new("mr"), pid("a"));
    state.faction_marks.insert("private:a:winnu:gained".to_owned(), format!("{}|mr", crate::seating::MECATOL));
    state.faction_marks.insert("private:a:winnu:tactical".to_owned(), "1".to_owned());
    based(state)
}

/// The end of the action in both engines with scripted answers.
fn finish_with(state: &GameState, ids: &[&str]) -> (super::turn_live_tests::Run, super::turn_live_tests::Run) {
    let string = super::turn_live_tests::string_run(state, ids, |g| g.finish_action().expect("the action finishes"));
    let native = super::turn_live_tests::native_run(state, ti4_engine_core::driver::session::Arm::Ask, Some(TurnJob::Finish), ids);
    (string, native)
}

#[test]
fn reclamation_places_a_pds_and_a_space_dock_on_mecatol_rex_like_the_string() {
    let _ = open_job;
    for answer in ["ability:winnu:reclamation:ACTION_COMPLETED:after", "decline"] {
        let state = winnu_took_mecatol();
        let (string, native) = finish_with(&state, &[answer]);
        assert_same(&string, &native);
        assert_eq!(string.questions.len(), 1, "{answer}");
        let placed = answer != "decline";
        assert_eq!(string.state["board"][crate::seating::MECATOL]["planet_units"].get("mr").is_some(), placed, "{answer}: {}", string.state["board"][crate::seating::MECATOL]);
    }
}

#[test]
fn reclamation_needs_a_pds_and_a_space_dock_in_the_box_and_room_on_the_planet_like_the_string() {
    // A planet that already holds a space dock of the seat takes no second one: both structures or nothing, so no window opens.
    let mut state = winnu_took_mecatol();
    crate::fixtures::put_on_planet(&mut state, &SystemId::new(crate::seating::MECATOL), &PlanetId::new("mr"), "spacedock", &pid("a"), 1);
    let (string, native) = finish_with(&state, &[]);
    assert_same(&string, &native);
    assert!(string.questions.is_empty(), "{:?}", string.questions.iter().map(|q| q.prompt.clone()).collect::<Vec<_>>());
}

#[test]
fn the_turn_marks_of_the_winnu_are_cleared_at_the_next_turn_like_the_string() {
    let state = winnu_took_mecatol();
    let (string, native) = prelude_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.state["faction_marks"].get("private:a:winnu:gained").is_none(), "{}", string.state["faction_marks"]);
}

/// [`finish_with`] where every question after the scripted answers takes its first option (and the native run is given the same answers).
fn finish_first(state: &GameState, ids: &[&str]) -> (super::turn_live_tests::Run, super::turn_live_tests::Run) {
    let (string, all) = super::turn_live_tests::string_run_first(state, ids, |g| g.finish_action().expect("the action finishes"));
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    let native = super::turn_live_tests::native_run(state, ti4_engine_core::driver::session::Arm::Ask, Some(TurnJob::Finish), &all);
    (string, native)
}

#[test]
fn the_reclaimer_places_one_structure_on_a_planet_it_stands_on_like_the_string() {
    for (answers, owns_mech) in [(vec!["unit:winnu:winnu_mech:ACTION_COMPLETED:after", "spacedock|18|mr"], true), (vec!["unit:winnu:winnu_mech:ACTION_COMPLETED:after", "pds|18|mr"], true), (vec!["decline"], true), (vec![], false)] {
        let mut state = winnu_took_mecatol();
        if owns_mech {
            crate::fixtures::put_on_planet(&mut state, &SystemId::new(crate::seating::MECATOL), &PlanetId::new("mr"), "winnu_mech", &pid("a"), 1);
        }
        let (string, native) = finish_first(&state, &answers);
        assert_same(&string, &native);
        assert_same_contexts(&string, &native);
        assert!(!string.questions.is_empty());
    }
}

// -- Naaz (lane G2): Garv and Gunn, Ssruu's copy and Dart and Tai ----------------------------------------------------------------------

/// An explorable planet outside every home system, put under `who`'s control (the first of `non_home_planets`).
fn take_explorable(state: &mut GameState, who: &str) -> PlanetId {
    let content = ContentStore::embedded();
    let catalogue = ti4_content::galaxy::all_planets(content, ti4_model::content_types::DEFAULT);
    let planet = crate::fixtures::non_home_planets(200)
        .into_iter()
        .map(PlanetId::new)
        .find(|p| !crate::planets::traits_now(state, content, ti4_model::content_types::DEFAULT, p).is_empty() && catalogue.get(p.as_str()).and_then(ti4_content::Planet::system_id).is_some())
        .expect("an explorable planet");
    let system = SystemId::new(catalogue[planet.as_str()].system_id().unwrap());
    state.system_mut(&system).planet_control.insert(planet.clone(), pid(who));
    planet
}

/// The end of seat `a`'s turn in both engines (every question after the scripted ones takes its first option, the native run gets the same answers).
fn end_first(state: &GameState, ids: &[&str]) -> (super::turn_live_tests::Run, super::turn_live_tests::Run) {
    let (string, all) = super::turn_live_tests::string_run_first(state, ids, |g| g.advance_turn().expect("the turn passes"));
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    let native = super::turn_live_tests::native_run(state, ti4_engine_core::driver::session::Arm::Ask, Some(TurnJob::End), &all);
    (string, native)
}

#[test]
fn garv_and_gunn_explore_a_planet_of_the_player_whose_turn_ends_like_the_string() {
    for answers in [vec!["leader:naaz:naazagent:TURN_PASSED:after"], vec!["decline"]] {
        let mut state = action_table(&[("a", "hacan"), ("b", "naaz"), ("c", "sol")]);
        let planet = take_explorable(&mut state, "a");
        let state = based(state);
        let mut answers: Vec<String> = answers.iter().map(|s| (*s).to_owned()).collect();
        if answers[0] != "decline" {
            answers.push(planet.to_string());
        }
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
        let (string, native) = end_first(&state, &answers);
        assert_same(&string, &native);
        assert_same_contexts(&string, &native);
        let explored = string.state["exploration_log"].as_array().map_or(0, Vec::len);
        assert_eq!(explored, usize::from(answers[0] != "decline"), "{answers:?}");
    }
}

#[test]
fn ssruus_copy_of_garv_and_gunn_names_the_planet_by_its_name_like_the_string() {
    let mut state = action_table(&[("a", "hacan"), ("b", "naaz"), ("c", "yssaril")]);
    let planet = take_explorable(&mut state, "a");
    // The Ssruu seat holds the card readied, the Naaz agent is exhausted (a lent agent may be exhausted or readied).
    state.player_mut(&pid("c")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("yssarilagent"), ti4_model::state::LeaderStatus::Readied);
    state.player_mut(&pid("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("naazagent"), ti4_model::state::LeaderStatus::Exhausted);
    let state = based(state);
    let planet = planet.to_string();
    let (string, native) = end_first(&state, &["leader:naaz:b:yssarilagent:naazagent:TURN_PASSED:after", &planet]);
    assert_same(&string, &native);
    assert_same_contexts(&string, &native);
    assert_eq!(string.state["exploration_log"].as_array().map_or(0, Vec::len), 1);
}
