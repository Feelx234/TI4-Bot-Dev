//! Depth tests, group d3: Jol-Nar, Winnu, Xxcha, Yin, Yssaril and the generic promissory notes
//! (see mod.rs and docs/FACTION_EFFECT_COVERAGE.md). Each test drives the engine's own flow (a
//! `Game` with a scripted table, a whole space combat or invasion with loaded dice, or the armed
//! resolver emitting the real event) and asserts what the card promises, plus the opposite case
//! where cheap.
//!
//! Test names: `<card id>_<effect no.>_<what>`; unit tests `unit_<unit id>_<n>_<what>`.

use ti4_model::content_types::DEFAULT;
use ti4_model::id::{PlanetId, PlayerId, StrategyCardId, SystemId};
use ti4_model::state::{GameState, LeaderStatus};

use super::d2::{
    Seen, action_game, first_roll, invade, leader_status, on_planet, run_status_phase,
    score_objectives, set_leader, ships, space_fight, unlocks_in_the_game, was_offered,
};
use super::{content, seated_all};
use crate::choice::Table;
use ti4_content::galaxy::Galaxy;
use crate::fixtures::{Hub, plain_hub, put, put_on_planet};
use crate::game::{Game, TACTICAL_ACTION_ID};

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}
fn c() -> PlayerId {
    PlayerId::new("c")
}
fn sys(id: &str) -> SystemId {
    SystemId::new(id)
}

// -- shared drivers -------------------------------------------------------------------------------

/// Step `game` until it has logged `event` (true) or `max` steps went by (false); every step must
/// be legal.
fn drive(game: &mut Game<'_>, event: &str, max: usize) -> bool {
    for _ in 0..max {
        let step = game.step();
        assert_eq!(step.error, None, "log: {:?}", game.events);
        if game.events.iter().any(|logged| logged == event) {
            return true;
        }
    }
    false
}

/// Step `game` exactly `count` times; every step must be legal.
fn steps(game: &mut Game<'_>, count: usize) {
    for _ in 0..count {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
}

/// Did the game log this event?
fn logged(game: &Game<'_>, event: &str) -> bool {
    game.events.iter().any(|logged| logged == event)
}

/// `active` takes a tactical action into `target` on `hub`'s map; `rules` answer what follows the
/// activation (then `done_moving`). Steps until the action completes or `extra_stop` is logged.
fn tactical(
    state: GameState,
    active: &str,
    galaxy: &Galaxy,
    target: &str,
    rules: &[&str],
    stop: &str,
) -> (Game<'static>, Seen) {
    let mut all = vec![TACTICAL_ACTION_ID, target];
    all.extend_from_slice(rules);
    all.push("done_moving");
    let (mut game, seen) = action_game(state, active, Some(galaxy.clone()), &all);
    for _ in 0..30 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, stop) || logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    (game, seen)
}

/// `player` holds this strategy card; the game is in the action phase with `active` to act.
fn with_card(state: &mut GameState, player: &PlayerId, card: &str) {
    state.player_mut(player).expect("seat").strategy_cards = vec![StrategyCardId::new(card)];
}

/// Is `who`'s technology `alias` exhausted?
fn tech_exhausted(state: &GameState, who: &PlayerId, alias: &str) -> bool {
    state
        .player(who)
        .expect("seat")
        .exhausted_technologies
        .contains(&ti4_model::id::TechnologyId::new(alias))
}

/// Total command tokens `who` holds.
fn tokens(state: &GameState, who: &PlayerId) -> i32 {
    let seat = state.player(who).expect("seat");
    seat.tactic_tokens + seat.fleet_tokens + seat.strategic_tokens
}

/// `(hits_on, dice)` of the first space-combat roll `kind` of seat `a` makes against one Sol
/// cruiser in system 19, every die a 10.
fn space_profile(faction: &str, kind: &str) -> (Option<u32>, usize) {
    let mut state = seated_all(&[("a", faction), ("b", "sol")]);
    put(&mut state, &sys("19"), kind, &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (_, dice) = space_fight(&mut state, &sys("19"), &[10; 8], &mut Table::new());
    first_roll(&dice, "a").expect("the unit rolled")
}

// -- Jol-Nar --------------------------------------------------------------------------------------

fn jolnar_game() -> GameState {
    seated_all(&[("a", "jolnar"), ("b", "sol")])
}

#[test]
fn fragile_1_jolnar_units_hit_on_a_higher_number_in_a_real_space_combat() {
    // A cruiser hits on 7; Fragile is -1 to the result, so Jol-Nar needs an 8.
    let (jolnar, sol) = (space_profile("jolnar", "cruiser"), space_profile("sol", "cruiser"));
    assert_eq!(sol.0, Some(7));
    assert_eq!(jolnar.0, Some(8), "-1 to the roll: one number higher is needed");
    // The same die decides it: a 7 hits for Sol and misses for Jol-Nar.
    for (faction, hits) in [("jolnar", false), ("sol", true)] {
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        put(&mut state, &sys("19"), "cruiser", &a(), 1);
        put(&mut state, &sys("19"), "dreadnought", &b(), 1);
        // The first die is the attacker's: a 7, then every die misses for the rest of round one.
        let (_, dice) = space_fight(&mut state, &sys("19"), &[7, 1, 1, 1, 1, 1], &mut Table::new());
        let first = dice.history().first().expect("a roll");
        assert_eq!(first.by.as_deref(), Some("a"));
        let landed = first.faces.iter().filter(|face| first.hits_on.is_some_and(|n| **face >= n)).count();
        assert_eq!(landed == 1, hits, "{faction}: a 7");
    }
}

#[test]
fn ers_1_four_trade_goods_when_another_player_activates_a_system_with_your_ships() {
    // b activates the system a's cruiser sits in; a, holding E-Res Siphons, gains 4 trade goods.
    // Your own activation and a game without the card pay nothing.
    for (tech, active, gains) in [(true, "b", 4), (true, "a", 0), (false, "b", 0)] {
        let hub = plain_hub();
        let target = hub.outer[0].clone();
        let mut state = jolnar_game();
        if tech {
            super::give(&mut state, "a", "ers");
        }
        put(&mut state, &sys(&target), "cruiser", &a(), 1);
        let before = state.player(&a()).unwrap().trade_goods;
        let (game, _) = tactical(state, active, &hub.galaxy, &target, &[], "E_RES_SIPHONS:a");
        assert_eq!(
            game.state.player(&a()).unwrap().trade_goods - before,
            gains,
            "tech {tech}, activated by {active}"
        );
        assert_eq!(logged(&game, "E_RES_SIPHONS:a"), gains > 0);
    }
}

#[test]
fn scc_1_the_activated_system_becomes_adjacent_to_every_system_with_your_units() {
    // The carrier waits across the centre (two jumps for a move-1 ship). Activating a system where
    // Jol-Nar already has a ship and exhausting the cylinders makes them adjacent.
    for (use_it, arrives) in [(true, true), (false, false)] {
        let hub = plain_hub();
        let from = hub.outer[0].clone();
        let target = hub.across(&from);
        let mut state = jolnar_game();
        super::give(&mut state, "a", "scc");
        put(&mut state, &sys(&from), "carrier", &a(), 1);
        put(&mut state, &sys(&target), "cruiser", &a(), 1);
        let answer = if use_it { "use" } else { "decline" };
        let move_it = format!("move|{from}|0");
        let (game, seen) = tactical(state, "a", &hub.galaxy, &target, &[answer, &move_it], "never");
        assert_eq!(was_offered(&seen, &move_it), arrives, "use: {use_it}");
        assert_eq!(ships(&game.state, &sys(&target), &a(), "carrier"), usize::from(arrives));
        assert_eq!(
            tech_exhausted(&game.state, &a(), "scc"),
            use_it,
            "exhausted when used"
        );
    }
}

/// Every question `seen` recorded, for a failing assertion's message.
fn asked(seen: &Seen) -> Vec<String> {
    seen.borrow()
        .iter()
        .map(|question| format!("{} {}: {:?}", question.player, question.prompt, question.options))
        .collect()
}

/// The technologies `who` could research right now.
fn researchable(state: &GameState, who: &PlayerId) -> Vec<String> {
    crate::technology::researchable(state, content(), DEFAULT, who)
        .into_iter()
        .map(|tech| tech.to_string())
        .collect()
}

fn owns(state: &GameState, who: &PlayerId, tech: &str) -> bool {
    state
        .player(who)
        .expect("seat")
        .technologies
        .contains(&ti4_model::id::TechnologyId::new(tech))
}

/// `faction` in seat a with no technologies at all, against a Hacan seat b.
fn bare_game(faction: &str) -> GameState {
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    state.player_mut(&a()).unwrap().technologies.clear();
    state
}

#[test]
fn analytical_1_a_missing_prerequisite_is_ignored_when_researching_in_the_game() {
    // What Jol-Nar can research beyond a plain faction with the same (empty) technology set.
    let plain = researchable(&bare_game("sol"), &a());
    let extra: Vec<String> = researchable(&bare_game("jolnar"), &a())
        .into_iter()
        .filter(|tech| !plain.contains(tech) && !["ers", "scc"].contains(&tech.as_str()))
        .collect();
    let target = extra.first().expect("Analytical opens a technology").clone();
    for (faction, researched) in [("jolnar", true), ("sol", false)] {
        let mut state = bare_game(faction);
        with_card(&mut state, &a(), "pok7technology");
        let (mut game, seen) = action_game(state, "a", None, &["strategic", &target]);
        steps(&mut game, 6);
        assert_eq!(was_offered(&seen, &target), researched, "{faction}: {:?}", asked(&seen));
        assert_eq!(owns(&game.state, &a(), &target), researched, "{faction}");
    }
}

#[test]
fn brilliant_1_jolnar_follows_the_technology_card_with_its_primary_and_not_the_secondary() {
    // Sol plays Technology; Jol-Nar follows with "yes" and researches as for the primary: free,
    // no resources spent. Following Trade stays the secondary (no trade goods are gained).
    let state = jolnar_game();
    let sol_tech = researchable(&state, &b())[0].clone();
    let target = researchable(&state, &a())
        .into_iter()
        .find(|tech| tech != &sol_tech)
        .expect("a technology");
    let mut state = state;
    let goods = state.player(&a()).unwrap().trade_goods;
    state.player_mut(&a()).unwrap().strategic_tokens = 2;
    set_leader(&mut state, &a(), "jolnaragent", LeaderStatus::Exhausted);
    for planet in state
        .controlled_planets(&a())
        .into_iter()
        .map(|(_, planet)| planet.clone())
        .collect::<Vec<_>>()
    {
        state.exhausted_planets.insert(planet);
    }
    let (mut game, seen) = action_game(
        {
            with_card(&mut state, &b(), "pok7technology");
            state.clone()
        },
        "b",
        None,
        &["strategic", &sol_tech, "yes", &target],
    );
    steps(&mut game, 8);
    assert!(was_offered(&seen, &target), "{:?}", asked(&seen));
    assert!(owns(&game.state, &a(), &target), "{:?}", asked(&seen));
    assert_eq!(game.state.player(&a()).unwrap().trade_goods, goods, "nothing was paid");
    assert_eq!(game.state.player(&a()).unwrap().strategic_tokens, 1, "a token pays the follow");
    // Trade: Brilliant does not apply, so Jol-Nar replenishes commodities (the secondary) and
    // gains none of the primary's trade goods.
    state.player_mut(&a()).unwrap().commodities = 0;
    with_card(&mut state, &b(), "pok5trade");
    let (mut game, seen) = action_game(state, "b", None, &["strategic", "yes"]);
    steps(&mut game, 8);
    let limit = ti4_content::factions::get(content(), "jolnar").unwrap().commodities();
    assert_eq!(game.state.player(&a()).unwrap().commodities, limit, "{:?}", asked(&seen));
    assert_eq!(game.state.player(&a()).unwrap().trade_goods, goods, "no primary effect");
}

/// Every planet `who` controls exhausted, no trade goods: nothing to pay with.
fn broke(state: &mut GameState, who: &PlayerId) {
    state.player_mut(who).unwrap().trade_goods = 0;
    for planet in state
        .controlled_planets(who)
        .into_iter()
        .map(|(_, planet)| planet.clone())
        .collect::<Vec<_>>()
    {
        state.exhausted_planets.insert(planet);
    }
}

#[test]
fn jolnaragent_1_doctor_sucaban_pays_for_the_second_research_with_infantry_and_readies_at_status() {
    // Jol-Nar plays Technology: the free research, then a second for 6 resources it cannot pay.
    // The agent exhausts and six infantry pay for it; without the agent there is no second one.
    let mut state = jolnar_game();
    let (system, planet) = {
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let planet = state.controlled_planets(&a())[0].1.clone();
        (home, planet)
    };
    put_on_planet(&mut state, &system, &planet, "infantry", &a(), 6);
    broke(&mut state, &a());
    let infantry = |state: &GameState| on_planet(state, &system, &planet, &a(), "infantry");
    let before = infantry(&state);
    let free = researchable(&state, &a())[0].clone();
    for (agent, second) in [(true, true), (false, false)] {
        let mut state = state.clone();
        with_card(&mut state, &a(), "pok7technology");
        set_leader(
            &mut state,
            &a(),
            "jolnaragent",
            if agent { LeaderStatus::Readied } else { LeaderStatus::Exhausted },
        );
        state.player_mut(&b()).unwrap().strategic_tokens = 0;
        let owned = state.player(&a()).unwrap().technologies.len();
        let (mut game, seen) = super::d2::eager_game(state, "a", None, &["strategic", &free]);
        steps(&mut game, 10);
        let gained = game.state.player(&a()).unwrap().technologies.len() - owned;
        assert_eq!(gained, if second { 2 } else { 1 }, "agent: {agent}: {:?}", asked(&seen));
        assert_eq!(was_offered_like_prompt(&seen, "Doctor Sucaban"), agent);
        if second {
            assert_eq!(infantry(&game.state), before - 6, "six infantry paid the six resources");
            assert_eq!(
                leader_status(&game.state, &a(), "jolnaragent"),
                Some(LeaderStatus::Exhausted)
            );
            run_status_phase(&mut game.state);
            assert_eq!(
                leader_status(&game.state, &a(), "jolnaragent"),
                Some(LeaderStatus::Readied)
            );
        } else {
            assert_eq!(infantry(&game.state), before, "no infantry was traded");
        }
    }
}

/// Did any question's prompt contain `part`?
fn was_offered_like_prompt(seen: &Seen, part: &str) -> bool {
    seen.borrow().iter().any(|question| question.prompt.contains(part))
}

/// A planet with a technology specialty, alone in its tile with that specialty.
fn specialty_planet() -> (SystemId, PlanetId, &'static str) {
    for (id, record) in ti4_content::galaxy::all_planets(content(), DEFAULT) {
        let (Some(specialty), Some(tile)) = (
            record.tech_specialties().first().copied(),
            record.system_id(),
        ) else {
            continue;
        };
        let colour = match specialty.to_ascii_uppercase().as_str() {
            "BIOTIC" => "BIOTIC",
            "CYBERNETIC" => "CYBERNETIC",
            "PROPULSION" => "PROPULSION",
            "WARFARE" => "WARFARE",
            _ => continue,
        };
        return (SystemId::new(tile), PlanetId::new(id), colour);
    }
    panic!("a planet with a technology specialty");
}

#[test]
fn jolnarbt_1_specialist_compounds_pays_the_second_research_with_an_exhausted_specialty() {
    let (system, planet, colour) = specialty_planet();
    for (breakthrough, researches) in [(true, true), (false, false)] {
        let mut state = jolnar_game();
        state.system_mut(&system).set_control(planet.clone(), a());
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("jolnarbt"));
        }
        broke(&mut state, &a());
        // The specialty planet itself is ready: it pays instead of resources.
        state.exhausted_planets.remove(&planet);
        with_card(&mut state, &a(), "pok7technology");
        set_leader(&mut state, &a(), "jolnaragent", LeaderStatus::Exhausted);
        state.player_mut(&b()).unwrap().strategic_tokens = 0;
        let free = researchable(&state, &a())
            .into_iter()
            .find(|tech| {
                crate::technology::colour_type(content(), &ti4_model::id::TechnologyId::new(tech))
                    != Some(colour)
            })
            .expect("a technology of another colour");
        let owned = state.player(&a()).unwrap().technologies.clone();
        let pick = format!("{planet}:{colour}");
        let (mut game, seen) =
            super::d2::eager_game(state, "a", None, &["strategic", &free, &pick]);
        steps(&mut game, 10);
        let gained: Vec<_> = game.state.player(&a()).unwrap().technologies.difference(&owned).cloned().collect();
        assert_eq!(was_offered(&seen, &pick), breakthrough, "{:?}", asked(&seen));
        assert_eq!(gained.len(), if researches { 2 } else { 1 }, "{gained:?} {:?}", asked(&seen));
        assert_eq!(game.state.exhausted_planets.contains(&planet), researches);
        if researches {
            assert!(
                gained.iter().any(|tech| crate::technology::colour_type(content(), tech) == Some(colour)),
                "a technology of the specialty's colour"
            );
        }
    }
}

#[test]
fn ra_1_and_2_the_holder_gains_the_researched_technology_and_the_note_goes_home() {
    let note = crate::promissory::note_id("ra", "jolnar");
    let state = jolnar_game();
    let ordinary = researchable(&state, &a())
        .into_iter()
        .find(|tech| !owns(&state, &b(), tech) && !["ers", "scc"].contains(&tech.as_str()))
        .expect("an ordinary technology");
    for (tech, copied) in [(ordinary.as_str(), true), ("ers", false)] {
        let mut state = jolnar_game();
        state.promissory_notes.insert(note.clone(), b());
        with_card(&mut state, &a(), "pok7technology");
        set_leader(&mut state, &a(), "jolnaragent", LeaderStatus::Exhausted);
        state.player_mut(&b()).unwrap().strategic_tokens = 0;
        let (mut game, seen) = action_game(state, "a", None, &["strategic", tech]);
        steps(&mut game, 6);
        assert!(owns(&game.state, &a(), tech), "{tech}: {:?}", asked(&seen));
        assert_eq!(owns(&game.state, &b(), tech), copied, "{tech} reaches the holder");
        assert_eq!(
            game.state.promissory_notes.get(&note),
            Some(&if copied { a() } else { b() }),
            "{tech}: the card goes home only once it was used"
        );
    }
}

#[test]
fn jolnarcommander_1_unlocks_in_the_game_with_eight_technologies() {
    let mut state = bare_game("jolnar");
    for tech in ["amd", "gd", "fl", "lwd", "det", "nm", "dxa"] {
        super::give(&mut state, "a", tech);
    }
    assert!(!unlocks_in_the_game(&state, &a(), "jolnarcommander"), "seven");
    super::give(&mut state, "a", "hm");
    assert!(unlocks_in_the_game(&state, &a(), "jolnarcommander"), "eight");
}

#[test]
fn jolnarhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = jolnar_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "jolnarhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "jolnarhero"));
}

/// Three ordinary (not faction, not unit upgrade) technologies of one colour, in id order.
fn three_of_a_colour() -> [String; 3] {
    let mut by_colour: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for record in content().from_sources(ti4_model::content_types::ContentType::Technologies, DEFAULT) {
        let types = record.strings("types");
        let (Some(alias), Some(colour)) = (record.text("alias"), types.first()) else {
            continue;
        };
        if record.text("faction").is_none()
            && !types.contains(&"UNITUPGRADE")
            && !crate::technology::is_unit_upgrade(content(), &ti4_model::id::TechnologyId::new(alias))
        {
            by_colour.entry((*colour).to_owned()).or_default().push(alias.to_owned());
        }
    }
    let mut techs = by_colour.into_values().find(|techs| techs.len() >= 3).expect("a colour");
    techs.sort();
    [techs[0].clone(), techs[1].clone(), techs[2].clone()]
}

#[test]
fn jolnarhero_2_rin_swaps_a_technology_for_another_of_its_colour_and_is_purged() {
    let [first, second, third] = three_of_a_colour();
    let mut state = bare_game("jolnar");
    super::give(&mut state, "a", &first);
    super::give(&mut state, "a", &second);
    set_leader(&mut state, &a(), "jolnarhero", LeaderStatus::Unlocked);
    let keep = format!("keep|{second}");
    let (mut game, seen) = action_game(
        state,
        "a",
        None,
        &["component|leader|jolnarhero", &third, &keep],
    );
    steps(&mut game, 4);
    assert!(owns(&game.state, &a(), &third), "{:?}", asked(&seen));
    assert!(!owns(&game.state, &a(), &first), "the traded technology went back to the deck");
    assert!(owns(&game.state, &a(), &second), "the kept one stayed");
    assert_eq!(leader_status(&game.state, &a(), "jolnarhero"), Some(LeaderStatus::Purged));
}

/// A lone-planet system `(system, planet)` the corpus places.
fn one_planet() -> (SystemId, PlanetId) {
    crate::fixtures::a_placed_planet()
}

#[test]
fn unit_jolnar_flagship_0_hylarim_fights_with_two_dice_on_6_and_sustains() {
    // Fragile makes the printed 6 a 7 for every Jol-Nar unit.
    assert_eq!(space_profile("jolnar", "jolnar_flagship"), (Some(7), 2));
    let mut state = jolnar_game();
    put(&mut state, &sys("19"), "jolnar_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (_, _) = space_fight(&mut state, &sys("19"), &[10; 8], &mut Table::new());
    assert!(
        state
            .system_state(&sys("19"))
            .units
            .iter()
            .any(|unit| unit.type_id.as_str() == "jolnar_flagship" && unit.sustained_damage),
        "one hit is sustained and the ship fights on"
    );
}

#[test]
fn unit_jolnar_flagship_1_hylarim_a_nine_or_ten_before_modifiers_adds_two_hits() {
    // The flagship's dice are 9 and 1 (7 needed under Fragile): one hit, +2 for the 9 = three hits
    // kill three Sol cruisers in round one, so Sol rolls its three dice once. A natural 8 is a hit
    // after Fragile too (8-1=7) but earns no extra hits: round two follows.
    let sol_dice = |first: u32| {
        let mut state = jolnar_game();
        put(&mut state, &sys("19"), "jolnar_flagship", &a(), 1);
        put(&mut state, &sys("19"), "cruiser", &b(), 3);
        let (_, dice) = space_fight(&mut state, &sys("19"), &[first, 1, 1, 1, 1], &mut Table::new());
        dice.history()
            .iter()
            .filter(|roll| roll.by.as_deref() == Some("b"))
            .map(|roll| roll.faces.len())
            .sum::<usize>()
    };
    assert_eq!(sol_dice(9), 3, "three hits destroy all three cruisers in round one");
    assert!(sol_dice(8) > 3, "one hit only: Sol fights a second round");
}

#[test]
fn unit_jolnar_mech_1_shield_paling_exempts_the_infantry_on_its_planet_from_fragile() {
    let infantry_roll = |mech: bool| {
        let mut state = jolnar_game();
        let (system, planet) = one_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        put_on_planet(&mut state, &system, &planet, "infantry", &b(), 3);
        put(&mut state, &system, "infantry", &a(), 2);
        if mech {
            put(&mut state, &system, "jolnar_mech", &a(), 1);
        }
        let (_, dice) = invade(&mut state, &system, &a(), &[10; 8], &mut Table::new());
        dice.history()
            .iter()
            .find(|roll| roll.by.as_deref() == Some("a") && roll.faces.len() == 2)
            .map(|roll| roll.hits_on)
    };
    assert_eq!(infantry_roll(false), Some(Some(9)), "Fragile: the printed 8 becomes 9");
    assert_eq!(infantry_roll(true), Some(Some(8)), "with Shield Paling on the planet: the printed 8");
}

// -- Winnu ----------------------------------------------------------------------------------------

fn winnu_game() -> GameState {
    seated_all(&[("a", "winnu"), ("b", "sol")])
}

/// Everything `who` controls exhausted and no trade goods, so no influence can be paid.
fn mecatol_invasion(faction: &str) -> (bool, i32) {
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    broke(&mut state, &a());
    state.custodians_removed = false;
    let mecatol = sys(crate::seating::MECATOL);
    put(&mut state, &mecatol, "infantry", &a(), 2);
    let points = state.player(&a()).unwrap().victory_points;
    let (pick, _) = super::d2::Pick::new(&["yes"]);
    let mut table = Table::with_default(Box::new(pick));
    let (report, _) = invade(&mut state, &mecatol, &a(), &[10; 4], &mut table);
    (report.custodians_removed, state.player(&a()).unwrap().victory_points - points)
}

#[test]
fn blood_ties_1_the_custodians_token_comes_off_without_spending_influence() {
    // Nothing to pay with: Winnu lifts the token and scores; Sol is never even offered it.
    assert_eq!(mecatol_invasion("winnu"), (true, 1));
    assert_eq!(mecatol_invasion("sol"), (false, 0));
}

const RECLAMATION: &str = "ability:winnu:reclamation:ACTION_COMPLETED:after";
const RECLAIMER: &str = "unit:winnu:winnu_mech:ACTION_COMPLETED:after";

/// Winnu `a` takes a tactical action into Mecatol Rex with two infantry and a mech waiting in its
/// space area (and the custodians token still there). Returns the game and the questions.
fn take_mecatol(mech: bool, rules: &[&str]) -> (Game<'static>, Seen) {
    let hub = crate::fixtures::hub_with_centre(crate::seating::MECATOL);
    let mut state = winnu_game();
    state.custodians_removed = false;
    broke(&mut state, &a());
    let mecatol = sys(crate::seating::MECATOL);
    put(&mut state, &mecatol, "carrier", &a(), 1);
    put(&mut state, &mecatol, "infantry", &a(), 2);
    if mech {
        put(&mut state, &mecatol, "winnu_mech", &a(), 1);
    }
    let mut all = vec![TACTICAL_ACTION_ID, crate::seating::MECATOL, "done_moving", "yes"];
    all.extend_from_slice(rules);
    let (mut game, seen) = action_game(state, "a", Some(hub.galaxy), &all);
    for _ in 0..25 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    (game, seen)
}

#[test]
fn reclamation_1_the_tactical_action_that_takes_mecatol_ends_with_a_pds_and_a_space_dock() {
    let mecatol = sys(crate::seating::MECATOL);
    let mr = PlanetId::new("mr");
    let (game, seen) = take_mecatol(false, &["~commit|", "done_committing", RECLAMATION]);
    assert_eq!(
        game.state.system_state(&mecatol).planet_control.get(&mr),
        Some(&a()),
        "{:?}",
        asked(&seen)
    );
    assert!(was_offered(&seen, RECLAMATION), "{:?}", asked(&seen));
    assert_eq!(on_planet(&game.state, &mecatol, &mr, &a(), "pds"), 1);
    assert_eq!(on_planet(&game.state, &mecatol, &mr, &a(), "spacedock"), 1);
    // Declined, nothing is placed.
    let (game, _) = take_mecatol(false, &["~commit|", "done_committing"]);
    assert_eq!(on_planet(&game.state, &mecatol, &mr, &a(), "pds"), 0);
    assert_eq!(on_planet(&game.state, &mecatol, &mr, &a(), "spacedock"), 0);
}

#[test]
fn winnu_mech_1_reclaimer_places_one_structure_on_the_planet_it_stands_on_after_the_action() {
    let mecatol = sys(crate::seating::MECATOL);
    let mr = PlanetId::new("mr");
    let mut rules = vec!["~commit|", "~commit|", "~commit|"];
    rules.extend([RECLAIMER, "~pds|"]);
    let (game, seen) = take_mecatol(true, &rules);
    assert!(was_offered(&seen, RECLAIMER), "{:?}", asked(&seen));
    assert_eq!(on_planet(&game.state, &mecatol, &mr, &a(), "pds"), 1, "{:?}", asked(&seen));
    assert_eq!(
        on_planet(&game.state, &mecatol, &mr, &a(), "spacedock"),
        0,
        "one structure: the choice was the PDS"
    );
    // Without the mech on the planet it is never offered.
    let (_, seen) = take_mecatol(false, &["~commit|", "~commit|", RECLAIMER]);
    assert!(!was_offered(&seen, RECLAIMER), "{:?}", asked(&seen));
}

/// Mecatol Rex in the centre and the alpha wormhole system 39 two rings out, so the two are not
/// physically adjacent.
fn gate_galaxy() -> Galaxy {
    let mut tiles: Vec<String> = vec![crate::seating::MECATOL.to_owned()];
    tiles.extend(
        crate::fixtures::plain_systems(12)
            .into_iter()
            .filter(|id| id != "39" && id != crate::seating::MECATOL)
            .take(6),
    );
    tiles.push("39".to_owned());
    let refs: Vec<&str> = tiles.iter().map(String::as_str).collect();
    Galaxy::build(content(), &refs, DEFAULT, 2).expect("galaxy")
}

#[test]
fn lgf_1_a_ship_in_a_wormhole_system_reaches_mecatol_rex_only_while_it_is_not_held() {
    let galaxy = gate_galaxy();
    assert!(!galaxy.adjacent(crate::seating::MECATOL).contains("39"));
    for (tech, holds, arrives) in [(true, false, true), (true, true, false), (false, false, false)] {
        let mut state = winnu_game();
        if tech {
            super::give(&mut state, "a", "lgf");
        }
        state
            .system_mut(&sys(crate::seating::MECATOL))
            .planet_control
            .clear();
        if holds {
            state
                .system_mut(&sys(crate::seating::MECATOL))
                .set_control(PlanetId::new("mr"), a());
        }
        put(&mut state, &sys("39"), "carrier", &a(), 1);
        let (game, seen) = tactical(
            state,
            "a",
            &galaxy,
            crate::seating::MECATOL,
            &["move|39|0"],
            "never",
        );
        assert_eq!(
            was_offered(&seen, "move|39|0"),
            arrives,
            "tech {tech}, holds {holds}: {:?}",
            asked(&seen)
        );
        assert_eq!(
            ships(&game.state, &sys(crate::seating::MECATOL), &a(), "carrier"),
            usize::from(arrives)
        );
    }
}

#[test]
fn lgf_2_the_component_action_places_an_infantry_on_mecatol_rex_and_exhausts_the_card() {
    for (holds, placed) in [(true, true), (false, false)] {
        let mut state = winnu_game();
        super::give(&mut state, "a", "lgf");
        state.system_mut(&sys(crate::seating::MECATOL)).planet_control.clear();
        if holds {
            state
                .system_mut(&sys(crate::seating::MECATOL))
                .set_control(PlanetId::new("mr"), a());
        }
        let before = on_planet(&state, &sys(crate::seating::MECATOL), &PlanetId::new("mr"), &a(), "infantry");
        let (mut game, seen) = action_game(state, "a", None, &["faction|winnu|lgf", "pass"]);
        steps(&mut game, 3);
        assert_eq!(was_offered(&seen, "faction|winnu|lgf"), placed, "{:?}", asked(&seen));
        let after = on_planet(&game.state, &sys(crate::seating::MECATOL), &PlanetId::new("mr"), &a(), "infantry");
        assert_eq!(after - before, usize::from(placed));
        assert_eq!(tech_exhausted(&game.state, &a(), "lgf"), placed);
    }
}

/// A planet in no home system worth no resources and at least two influence.
fn influence_planet() -> (SystemId, PlanetId) {
    use crate::production::{Spend, planet_value};
    ti4_content::galaxy::all_planets(content(), DEFAULT)
        .iter()
        .find_map(|(id, record)| {
            let planet = PlanetId::new(*id);
            let system = record.system_id()?;
            (record.homeworld_of().is_none()
                && !record.is_placed_during_play()
                && planet_value(content(), DEFAULT, &planet, Spend::Resources) == 0
                && planet_value(content(), DEFAULT, &planet, Spend::Influence) >= 2)
                .then(|| (SystemId::new(system), planet))
        })
        .expect("an influence-only planet")
}

#[test]
fn htp_1_swaps_a_planets_influence_for_its_resources_for_one_use_of_production() {
    // The only ready planet is worth 0 resources and 2+ influence: a cruiser (cost 2) is
    // affordable only after Hegemonic Trade Policy swaps its values.
    let (system, planet) = influence_planet();
    let ability = "technology:winnu:htp:PRODUCTION_USED:after";
    for (tech, builds) in [(true, true), (false, false)] {
        let mut state = winnu_game();
        if tech {
            super::give(&mut state, "a", "htp");
        }
        broke(&mut state, &a());
        state.system_mut(&system).set_control(planet.clone(), a());
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let planet_name = planet.to_string();
        let cruisers = ships(&state, &home, &a(), "cruiser");
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(plain_hub().galaxy),
            &[
                TACTICAL_ACTION_ID,
                home.as_str(),
                "done_moving",
                ability,
                &planet_name,
                "build|cruiser|1",
                "done_producing",
            ],
        );
        steps(&mut game, 10);
        assert_eq!(was_offered(&seen, "build|cruiser|1"), builds, "{:?}", asked(&seen));
        assert_eq!(
            ships(&game.state, &home, &a(), "cruiser") - cruisers,
            usize::from(builds)
        );
        assert_eq!(tech_exhausted(&game.state, &a(), "htp"), builds);
    }
}

#[test]
fn winnuagent_1_berekar_cuts_any_players_production_by_two_and_readies_at_status() {
    // Sol has two trade goods and builds a carrier (cost 3): affordable only with the discount.
    let agent = "leader:winnu:winnuagent:PRODUCTION_USED:after";
    for (offered_agent, builds) in [(true, true), (false, false)] {
        let mut state = winnu_game();
        broke(&mut state, &b());
        state.player_mut(&b()).unwrap().trade_goods = 2;
        state.player_mut(&b()).unwrap().fleet_tokens = 10;
        let home = state.player(&b()).unwrap().home_system.clone().unwrap();
        if !offered_agent {
            set_leader(&mut state, &a(), "winnuagent", LeaderStatus::Exhausted);
        }
        let carriers = ships(&state, &home, &b(), "sol_carrier");
        let (mut game, seen) = action_game(
            state,
            "b",
            Some(plain_hub().galaxy),
            &[
                TACTICAL_ACTION_ID,
                home.as_str(),
                "done_moving",
                agent,
                "build|sol_carrier|1",
                "done_producing",
            ],
        );
        steps(&mut game, 10);
        assert_eq!(was_offered(&seen, "build|sol_carrier|1"), builds, "{:?}", asked(&seen));
        assert_eq!(
            ships(&game.state, &home, &b(), "sol_carrier") - carriers,
            usize::from(builds),
            "{:?}",
            asked(&seen)
        );
        if builds {
            assert_eq!(game.state.player(&b()).unwrap().trade_goods, 1, "3 - 2 = 1 good paid");
            assert_eq!(
                leader_status(&game.state, &a(), "winnuagent"),
                Some(LeaderStatus::Exhausted)
            );
            run_status_phase(&mut game.state);
            assert_eq!(leader_status(&game.state, &a(), "winnuagent"), Some(LeaderStatus::Readied));
        }
    }
}

/// A system holding a legendary planet.
fn legendary_system() -> SystemId {
    use ti4_model::content_types::POK;
    ti4_content::galaxy::all_systems(content(), POK)
        .into_iter()
        .find(|(id, _)| {
            ti4_content::galaxy::system(content(), id, POK).is_some_and(|system| {
                system.planets().iter().any(|planet| {
                    ti4_content::galaxy::planet(content(), planet, POK)
                        .is_some_and(|planet| planet.is_legendary())
                })
            })
        })
        .map(|(id, _)| SystemId::new(id))
        .expect("a legendary system")
}

#[test]
fn winnucommander_1_unlocks_in_the_game_by_holding_mecatol_rex() {
    let mut state = winnu_game();
    state.system_mut(&sys(crate::seating::MECATOL)).planet_control.clear();
    assert!(!unlocks_in_the_game(&state, &a(), "winnucommander"));
    state
        .system_mut(&sys(crate::seating::MECATOL))
        .set_control(PlanetId::new("mr"), a());
    assert!(unlocks_in_the_game(&state, &a(), "winnucommander"));
}

#[test]
fn winnucommander_1_unlocks_in_the_game_when_a_combat_starts_in_the_mecatol_system() {
    // Sol's cruiser holds Mecatol Rex's system; the Winnu carrier jumps in and the fight unlocks it.
    let hub = crate::fixtures::hub_with_centre(crate::seating::MECATOL);
    let mut state = winnu_game();
    state.system_mut(&sys(crate::seating::MECATOL)).planet_control.clear();
    put(&mut state, &sys(crate::seating::MECATOL), "cruiser", &b(), 1);
    let from = sys(&hub.outer[0]);
    put(&mut state, &from, "dreadnought", &a(), 1);
    let move_it = format!("move|{from}|0");
    let (mut game, _) = action_game(
        state,
        "a",
        Some(hub.galaxy),
        &[TACTICAL_ACTION_ID, crate::seating::MECATOL, &move_it, "done_moving"],
    );
    assert_eq!(leader_status(&game.state, &a(), "winnucommander"), Some(LeaderStatus::Locked));
    assert!(drive(&mut game, "SPACE_COMBAT_STARTED", 25), "log: {:?}", game.events);
    steps(&mut game, 1);
    assert_eq!(leader_status(&game.state, &a(), "winnucommander"), Some(LeaderStatus::Unlocked));
}

#[test]
fn winnucommander_2_rolls_two_better_in_mecatol_home_and_legendary_systems_once_unlocked() {
    // A Winnu cruiser (7) against one Sol cruiser, in four systems and locked/unlocked.
    let home = winnu_game().player(&a()).unwrap().home_system.clone().unwrap();
    let legendary = legendary_system();
    let roll = |system: &SystemId, status: LeaderStatus| {
        let mut state = winnu_game();
        set_leader(&mut state, &a(), "winnucommander", status);
        state.system_mut(system).units.clear();
        put(&mut state, system, "cruiser", &a(), 1);
        put(&mut state, system, "cruiser", &b(), 1);
        let (_, dice) = space_fight(&mut state, system, &[10; 8], &mut Table::new());
        first_roll(&dice, "a").and_then(|(hits_on, _)| hits_on)
    };
    for system in [sys(crate::seating::MECATOL), home.clone(), legendary] {
        assert_eq!(roll(&system, LeaderStatus::Locked), Some(7), "{system} locked");
        assert_eq!(roll(&system, LeaderStatus::Unlocked), Some(5), "{system} unlocked: +2");
    }
    assert_eq!(roll(&sys("19"), LeaderStatus::Unlocked), Some(7), "an ordinary system");
}

#[test]
fn winnuhero_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = winnu_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "winnuhero"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "winnuhero"));
}

#[test]
fn winnuhero_2_mathis_performs_a_primary_lets_chosen_players_follow_and_is_purged() {
    // Winnu uses the unclaimed Politics card: the primary draws Winnu 2 action cards, and Sol, if
    // chosen to follow, draws 2 as well. (Whether a chosen player also spends a strategy token is
    // not asserted: the engine asks them, the card text does not say.)
    let followed = |choose: &str| {
        let mut state = winnu_game();
        state.unclaimed_strategy_cards = vec![StrategyCardId::new("pok3politics")];
        for seat in &mut state.players {
            seat.strategy_cards.clear();
        }
        set_leader(&mut state, &a(), "winnuhero", LeaderStatus::Unlocked);
        let hand = |state: &GameState, who: &PlayerId| state.player(who).unwrap().action_cards.len();
        let (mine, theirs) = (hand(&state, &a()), hand(&state, &b()));
        let (mut game, seen) = action_game(
            state,
            "a",
            None,
            &["component|leader|winnuhero", "pok3politics", choose, "yes"],
        );
        steps(&mut game, 6);
        assert_eq!(hand(&game.state, &a()) - mine, 2, "the primary resolved: {:?}", asked(&seen));
        assert_eq!(leader_status(&game.state, &a(), "winnuhero"), Some(LeaderStatus::Purged));
        hand(&game.state, &b()) - theirs
    };
    assert_eq!(followed("yes|b"), 2, "Sol followed and drew");
    assert_eq!(followed("no|b"), 0, "a player not chosen does nothing");
}

#[test]
fn acq_1_the_holder_follows_the_winnu_card_without_a_token_and_the_note_goes_home() {
    let note = crate::promissory::note_id("acq", "winnu");
    for (held, follows) in [(true, true), (false, false)] {
        let mut state = winnu_game();
        with_card(&mut state, &a(), "pok3politics");
        state.player_mut(&b()).unwrap().strategic_tokens = 0;
        if held {
            state.promissory_notes.insert(note.clone(), b());
        }
        let cards = state.player(&b()).unwrap().action_cards.len();
        let (mut game, seen) = action_game(state, "a", None, &["strategic", "~acq"]);
        steps(&mut game, 6);
        assert_eq!(
            game.state.player(&b()).unwrap().action_cards.len() - cards,
            if follows { 2 } else { 0 },
            "{:?}",
            asked(&seen)
        );
        assert_eq!(game.state.player(&b()).unwrap().strategic_tokens, 0, "no token was spent");
        if held {
            assert_eq!(game.state.promissory_notes.get(&note), Some(&a()), "the card went home");
        }
    }
}

fn winnubt(state: &mut GameState) {
    state.player_mut(&a()).unwrap().breakthrough =
        Some(ti4_model::id::BreakthroughId::new("winnubt"));
}

#[test]
fn winnubt_1_imperator_adds_one_to_combat_rolls_per_support_in_the_opponents_play_area() {
    let roll = |breakthrough: bool, supports: &[(&str, &PlayerId)]| {
        let mut state = winnu_game();
        if breakthrough {
            winnubt(&mut state);
        }
        for (owner, holder) in supports {
            state.support_holders.insert(PlayerId::new(*owner), (*holder).clone());
        }
        put(&mut state, &sys("19"), "cruiser", &a(), 1);
        put(&mut state, &sys("19"), "cruiser", &b(), 1);
        let (_, dice) = space_fight(&mut state, &sys("19"), &[10; 8], &mut Table::new());
        first_roll(&dice, "a").and_then(|(hits_on, _)| hits_on)
    };
    let (sol, winnu) = (b(), a());
    assert_eq!(roll(true, &[]), Some(7));
    assert_eq!(
        roll(true, &[("c", &sol), ("d", &sol), ("e", &winnu)]),
        Some(5),
        "+1 for each of the two in Sol's area; the one in Winnu's own does not count"
    );
    assert_eq!(roll(false, &[("c", &sol), ("d", &sol)]), Some(7), "no breakthrough, no bonus");
}

#[test]
fn winnubt_2_imperator_gives_one_ship_plus_one_move_after_a_legendary_activation() {
    // The carrier waits across the centre from the legendary system: two jumps for a move-1 ship.
    let ability = "breakthrough:winnu:winnubt:SYSTEM_ACTIVATED:after";
    let legendary = legendary_system();
    let hub = crate::fixtures::hub_with_outer(legendary.as_str());
    let from = hub.across(legendary.as_str());
    for (breakthrough, arrives) in [(true, true), (false, false)] {
        let mut state = winnu_game();
        if breakthrough {
            winnubt(&mut state);
        }
        // The carrier is Winnu's only ship, so "ship|0" is the one that gets the move.
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        state.system_mut(&home).units.clear();
        // The hub's centre is a home tile with its owner's ships, which would block the route.
        for system in &hub.outer {
            state.system_mut(&sys(system)).units.clear();
        }
        state.system_mut(&sys(&hub.centre)).units.clear();
        put(&mut state, &sys(&from), "carrier", &a(), 1);
        let move_it = format!("move|{from}|0");
        let (game, seen) = tactical(
            state,
            "a",
            &hub.galaxy,
            legendary.as_str(),
            &[ability, "ship|0", &move_it],
            "never",
        );
        assert_eq!(was_offered(&seen, &move_it), arrives, "{:?}", asked(&seen));
        assert_eq!(ships(&game.state, &legendary, &a(), "carrier"), usize::from(arrives));
    }
}

#[test]
fn unit_winnu_flagship_0_salai_sai_corian_hits_on_7_and_sustains() {
    let (hits_on, dice) = space_profile("winnu", "winnu_flagship");
    assert_eq!(hits_on, Some(7));
    assert_eq!(dice, 1, "one opposing non-fighter ship: one die");
    let mut state = winnu_game();
    put(&mut state, &sys("19"), "winnu_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let _ = space_fight(&mut state, &sys("19"), &[10; 8], &mut Table::new());
    assert!(
        state
            .system_state(&sys("19"))
            .units
            .iter()
            .any(|unit| unit.type_id.as_str() == "winnu_flagship" && unit.sustained_damage),
        "the ship absorbed the hit and stands on"
    );
}

#[test]
fn unit_winnu_flagship_1_salai_sai_corian_rolls_a_die_per_opposing_non_fighter_ship() {
    let dice = |opponents: &[(&str, usize)]| {
        let mut state = winnu_game();
        put(&mut state, &sys("19"), "winnu_flagship", &a(), 1);
        put(&mut state, &sys("19"), "carrier", &a(), 1);
        for (kind, count) in opponents {
            put(&mut state, &sys("19"), kind, &b(), *count);
        }
        let (_, dice) = space_fight(&mut state, &sys("19"), &[1; 40], &mut Table::new());
        dice.history()
            .iter()
            .find(|roll| roll.by.as_deref() == Some("a") && roll.hits_on == Some(7))
            .map(|roll| roll.faces.len())
    };
    assert_eq!(
        dice(&[("cruiser", 2), ("destroyer", 1), ("fighter", 4)]),
        Some(3),
        "fighters do not count; neither do its own ships"
    );
    assert_eq!(dice(&[("dreadnought", 1), ("fighter", 6)]), Some(1));
}

// -- Xxcha ----------------------------------------------------------------------------------------

fn xxcha_game() -> GameState {
    seated_all(&[("a", "xxcha"), ("b", "hacan")])
}

/// Every ring system of `hub` emptied of units and control, so the map is a blank slate.
fn blank_hub(state: &mut GameState, hub: &Hub) {
    for id in std::iter::once(&hub.centre).chain(&hub.outer) {
        let system = state.system_mut(&sys(id));
        system.units.clear();
        system.planet_units.clear();
        system.planet_control.clear();
    }
}

/// The first planet of `system`.
fn first_planet(system: &str) -> PlanetId {
    PlanetId::new(
        ti4_content::galaxy::planets_in(content(), system, DEFAULT)
            .first()
            .expect("a planet")
            .id(),
    )
}

#[test]
fn peace_accords_1_after_diplomacy_xxcha_gains_an_empty_planet_next_to_one_it_controls() {
    let hub = plain_hub();
    let has_planets =
        |id: &&String| !ti4_content::galaxy::planets_in(content(), id, DEFAULT).is_empty();
    let mine = hub.outer.iter().find(has_planets).expect("a ring system with a planet");
    // Two other systems next to it that hold planets (the centre counts).
    let neighbours: Vec<&String> = std::iter::once(&hub.centre)
        .chain(&hub.outer)
        .filter(|id| *id != mine && hub.galaxy.adjacent(mine).contains(id.as_str()))
        .filter(has_planets)
        .collect();
    let (open, occupied) = (neighbours[0], neighbours[1]);
    for (has_card, gains) in [(true, true), (false, false)] {
        let mut state = if has_card {
            xxcha_game()
        } else {
            seated_all(&[("a", "sol"), ("b", "hacan")])
        };
        blank_hub(&mut state, &hub);
        state.system_mut(&sys(mine)).set_control(first_planet(mine), a());
        let target = first_planet(open);
        with_card(&mut state, &a(), "pok2diplomacy");
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy.clone()),
            &["strategic", mine, target.as_str()],
        );
        steps(&mut game, 6);
        assert_eq!(was_offered(&seen, target.as_str()), gains, "{:?}", asked(&seen));
        assert_eq!(
            game.state.system_state(&sys(open)).planet_control.get(&target),
            gains.then_some(&a()),
            "{:?}",
            asked(&seen)
        );
    }
    // A planet that holds a unit (anybody's) is not offered.
    let mut state = xxcha_game();
    blank_hub(&mut state, &hub);
    state.system_mut(&sys(mine)).set_control(first_planet(mine), a());
    put_on_planet(&mut state, &sys(occupied), &first_planet(occupied), "infantry", &b(), 1);
    with_card(&mut state, &a(), "pok2diplomacy");
    let (mut game, seen) = action_game(state, "a", Some(hub.galaxy.clone()), &["strategic", mine]);
    steps(&mut game, 6);
    assert!(was_offered(&seen, first_planet(open).as_str()), "{:?}", asked(&seen));
    assert!(!was_offered(&seen, first_planet(occupied).as_str()), "{:?}", asked(&seen));
}

/// A two-player agenda phase of seated factions: `a` holds two influence planets and `b` two more,
/// so `a` carries any vote the two take the same side of. `deck` is the agenda deck, `card` an
/// action card given to a seat.
fn agenda_position(
    factions: (&str, &str),
    deck: &[&str],
    card: Option<(&str, &PlayerId)>,
) -> GameState {
    let mut state = seated_all(&[("a", factions.0), ("b", factions.1)]);
    state.phase = ti4_model::state::Phase::Agenda;
    state.custodians_removed = true;
    for seat in &state.players.clone() {
        for planet in state
            .controlled_planets(&seat.id)
            .into_iter()
            .map(|(_, planet)| planet.clone())
            .collect::<Vec<_>>()
        {
            state.exhausted_planets.remove(&planet);
        }
    }
    if let Some((alias, holder)) = card {
        state.player_mut(holder).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new(alias)];
    }
    state.agenda_deck = deck.iter().map(ToString::to_string).collect();
    state
}

/// Answers by prompt: the first `(part, answer)` whose `part` the prompt contains and whose
/// `answer` is offered; otherwise the first option that is not a decline. Every question is logged.
struct ByPrompt {
    rules: Vec<(String, String)>,
    seen: Seen,
}

impl crate::choice::Decider for ByPrompt {
    fn choose(
        &mut self,
        choice: &crate::choice::Choice,
    ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        self.seen.borrow_mut().push(super::d2::Asked {
            player: choice.player.to_string(),
            prompt: choice.prompt.clone(),
            options: choice.options.iter().map(|option| option.id.clone()).collect(),
        });
        let ruled = self.rules.iter().find_map(|(part, answer)| {
            choice
                .prompt
                .contains(part.as_str())
                .then(|| choice.options.iter().find(|option| option.id == *answer))
                .flatten()
        });
        ruled
            .or_else(|| choice.options.iter().find(|option| !option.is_decline()))
            .or_else(|| choice.options.first())
            .cloned()
            .ok_or_else(|| crate::choice::IllegalChoice::NoOptions {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
            })
    }
}

/// Play the agenda phase of `state`; `rules` answer questions by prompt (see [`ByPrompt`]).
fn play_agenda(state: GameState, rules: &[(&str, &str)]) -> (Game<'static>, Seen) {
    let seen = Seen::default();
    let decider = ByPrompt {
        rules: rules.iter().map(|(p, a)| ((*p).to_owned(), (*a).to_owned())).collect(),
        seen: seen.clone(),
    };
    let table = Table::with_default(Box::new(decider));
    let mut game = Game::with_table(state, content(), table).with_sources(DEFAULT);
    for _ in 0..200 {
        if game.state.phase != ti4_model::state::Phase::Agenda {
            break;
        }
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    (game, seen)
}

/// The aliases of For/Against laws, in corpus order.
fn for_against_laws() -> Vec<String> {
    content()
        .records(ti4_model::content_types::ContentType::Agendas)
        .iter()
        .filter(|record| {
            record.text("type") == Some("Law") && record.text("target") == Some("For/Against")
        })
        .filter_map(|record| record.text("alias").map(ToOwned::to_owned))
        .collect()
}

#[test]
fn favor_1_and_2_political_favor_replaces_the_agenda_for_a_strategy_token_and_goes_home() {
    let laws = for_against_laws();
    let (first, second) = (laws[0].as_str(), laws[1].as_str());
    let note = crate::promissory::note_id("favor", "xxcha");
    for (use_it, replaced) in [(true, true), (false, false)] {
        let mut state = agenda_position(("hacan", "xxcha"), &[first, second, &laws[2]], None);
        state.promissory_notes.insert(note.clone(), a());
        state.player_mut(&b()).unwrap().strategic_tokens = 1;
        // Quash (Xxcha's own ability) is always declined; the note's holder answers.
        let answer = if use_it { "use" } else { "decline" };
        let (game, seen) =
            play_agenda(state, &[("Quash", "decline"), ("Political Favor", answer)]);
        let resolved = |alias: &str| {
            game.events
                .iter()
                .any(|event| event.starts_with(&format!("AGENDA_RESOLVED:{alias}:")))
        };
        assert_eq!(resolved(first), !replaced, "{:?}", asked(&seen));
        // The next agenda is voted on instead of the discarded one (the phase decides two).
        assert!(resolved(second), "{:?}", asked(&seen));
        assert_eq!(resolved(&laws[2]), replaced, "the third is reached only after a replacement");
        assert_eq!(logged(&game, &format!("AGENDA_DISCARDED:{first}")), replaced);
        assert_eq!(
            game.state.player(&b()).unwrap().strategic_tokens,
            if replaced { 0 } else { 1 },
            "the Xxcha player pays one strategy token"
        );
        assert_eq!(
            game.state.promissory_notes.get(&note),
            Some(&if replaced { b() } else { a() }),
            "the card goes home only once it was used"
        );
    }
}

#[test]
fn ps_1_political_secret_bars_its_owner_from_voting_on_that_agenda_and_goes_home() {
    let laws = for_against_laws();
    let note = crate::promissory::note_id("ps", "sol");
    for (use_it, barred) in [(true, true), (false, false)] {
        let mut state = agenda_position(("hacan", "sol"), &[&laws[0]], None);
        state.promissory_notes.insert(note.clone(), a());
        let (game, seen) = play_agenda(state, &[("Political Secret", if use_it { "use" } else { "decline" })]);
        let voted_by_b = seen
            .borrow()
            .iter()
            .any(|question| question.player == "b" && question.prompt.contains("vote"));
        assert_eq!(voted_by_b, !barred, "{:?}", asked(&seen));
        assert_eq!(
            game.state.promissory_notes.get(&note),
            Some(&if use_it { b() } else { a() }),
            "returned to its owner after use"
        );
    }
}

#[test]
#[ignore = "MISSING/BUG: ps Political Secret does not stop its owner playing action cards or using faction abilities (only the vote bar exists)"]
fn ps_1_political_secret_also_bars_its_owner_from_action_cards_until_the_agenda_resolves() {
    // Sol (b) holds Confounding Legal Text, playable when another player is elected. a plays
    // Political Secret on b when the agenda is revealed, so b may not play it before the agenda has
    // resolved. (Card text: "cannot vote, play action cards, or use faction abilities until after
    // that agenda has been resolved".)
    let note = crate::promissory::note_id("ps", "sol");
    let b_id = b();
    let mut state = agenda_position(("hacan", "sol"), &["secret"], Some(("confounding", &b_id)));
    state.promissory_notes.insert(note, a());
    let (game, seen) = play_agenda(state, &[("Political Secret", "use")]);
    assert!(logged(&game, "AGENDA_RESOLVED:secret:a"), "a is elected: {:?}", game.events);
    assert!(
        !logged(&game, "AGENDA_OUTCOME_REDIRECTED:a:b"),
        "b played an action card while barred by Political Secret: {:?}",
        asked(&seen)
    );
}

#[test]
fn xxchaagent_1_ggrocuto_readies_a_planet_and_removes_an_infantry_next_to_a_planet_you_control() {
    let hub = plain_hub();
    let has_planets =
        |id: &&String| !ti4_content::galaxy::planets_in(content(), id, DEFAULT).is_empty();
    let mine = hub.outer.iter().find(has_planets).expect("a ring system with a planet");
    let near = std::iter::once(&hub.centre)
        .chain(&hub.outer)
        .find(|id| *id != mine && hub.galaxy.adjacent(mine).contains(id.as_str()) && has_planets(id))
        .expect("a neighbour with a planet");
    let far = hub
        .outer
        .iter()
        .find(|id| {
            *id != mine && *id != near && hub.galaxy.distance(mine, id).is_some_and(|d| d > 1) && has_planets(id)
        })
        .expect("a system two jumps away");
    for (system, removes) in [(near, true), (far, false)] {
        let planet = first_planet(system);
        let mut state = xxcha_game();
        blank_hub(&mut state, &hub);
        state.exhausted_planets.clear();
        state.system_mut(&sys(mine)).set_control(first_planet(mine), a());
        state.system_mut(&sys(system)).set_control(planet.clone(), b());
        put_on_planet(&mut state, &sys(system), &planet, "infantry", &a(), 1);
        state.exhausted_planets.insert(planet.clone());
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy.clone()),
            &["component|leader|xxchaagent", "remove"],
        );
        steps(&mut game, 3);
        assert!(!game.state.exhausted_planets.contains(&planet), "readied: {:?}", asked(&seen));
        assert_eq!(was_offered(&seen, "remove"), removes, "{:?}", asked(&seen));
        assert_eq!(
            on_planet(&game.state, &sys(system), &planet, &a(), "infantry"),
            usize::from(!removes)
        );
        assert_eq!(leader_status(&game.state, &a(), "xxchaagent"), Some(LeaderStatus::Exhausted));
        run_status_phase(&mut game.state);
        assert_eq!(leader_status(&game.state, &a(), "xxchaagent"), Some(LeaderStatus::Readied));
    }
}

#[test]
fn xxchacommander_1_unlocks_in_the_game_at_twelve_influence() {
    let mut state = xxcha_game();
    state.board.iter_mut().for_each(|(_, system)| system.planet_control.clear());
    let mut total = 0;
    for (id, record) in ti4_content::galaxy::all_planets(content(), DEFAULT) {
        let planet = PlanetId::new(id);
        let worth = crate::production::planet_value_now(
            &state,
            content(),
            DEFAULT,
            &planet,
            crate::production::Spend::Influence,
        );
        let Some(system) = record.system_id() else { continue };
        if worth == 0 || record.is_placed_during_play() {
            continue;
        }
        state.system_mut(&sys(system)).set_control(planet, a());
        total += worth;
        let unlocked = unlocks_in_the_game(&state, &a(), "xxchacommander");
        assert_eq!(unlocked, total >= 12, "{total} influence");
        if unlocked {
            return;
        }
    }
    panic!("never reached 12 influence");
}

#[test]
fn xxchacommander_2_each_planet_exhausted_to_vote_gives_one_more_vote() {
    let laws = for_against_laws();
    let votes_for = |status: LeaderStatus| {
        let mut state = agenda_position(("xxcha", "hacan"), &[&laws[0]], None);
        set_leader(&mut state, &a(), "xxchacommander", status);
        // Hacan's planets are exhausted: only the Xxcha player votes.
        for planet in state
            .controlled_planets(&b())
            .into_iter()
            .map(|(_, planet)| planet.clone())
            .collect::<Vec<_>>()
        {
            state.exhausted_planets.insert(planet);
        }
        let planets = state.controlled_planets(&a()).len() as i64;
        let (game, seen) = play_agenda(state, &[("vote for which", "for")]);
        let record = game.state.agenda_log.last().expect("the agenda resolved");
        (record.votes.get("for").copied().unwrap_or(0), planets, asked(&seen))
    };
    let (locked, planets, _) = votes_for(LeaderStatus::Locked);
    let (unlocked, _, asked) = votes_for(LeaderStatus::Unlocked);
    assert!(planets >= 2, "a seat with two home planets");
    assert_eq!(unlocked - locked, planets, "one more per planet exhausted: {asked:?}");
}

#[test]
fn xxchacommander_3_game_effects_cannot_stop_the_xxcha_player_voting() {
    let laws = for_against_laws();
    let note = crate::promissory::note_id("ps", "xxcha");
    for (status, votes) in [(LeaderStatus::Unlocked, true), (LeaderStatus::Locked, false)] {
        let mut state = agenda_position(("xxcha", "hacan"), &[&laws[0]], None);
        set_leader(&mut state, &a(), "xxchacommander", status);
        state.promissory_notes.insert(note.clone(), b());
        let (game, seen) = play_agenda(state, &[("Political Secret", "use")]);
        let a_voted = seen
            .borrow()
            .iter()
            .any(|question| question.player == "a" && question.prompt.contains("vote"));
        assert_eq!(a_voted, votes, "{status:?}: {:?}", asked(&seen));
        assert_eq!(game.state.promissory_notes.get(&note), Some(&a()), "the note was used and went home");
    }
}

#[test]
fn xxchahero_te_1_unlocks_in_the_game_on_the_third_scored_objective() {
    let mut state = xxcha_game();
    score_objectives(&mut state, &a(), 2);
    assert!(!unlocks_in_the_game(&state, &a(), "xxchahero-te"));
    score_objectives(&mut state, &a(), 3);
    assert!(unlocks_in_the_game(&state, &a(), "xxchahero-te"));
}

#[test]
fn xxchahero_te_2_xxekir_grom_places_units_on_controlled_planets_readies_them_and_is_purged() {
    let mut state = xxcha_game();
    set_leader(&mut state, &a(), "xxchahero-te", LeaderStatus::Unlocked);
    let home = state.player(&a()).unwrap().home_system.clone().unwrap();
    let planets: Vec<PlanetId> = state
        .controlled_planets(&a())
        .into_iter()
        .filter(|(system, _)| **system == home)
        .map(|(_, planet)| planet.clone())
        .collect();
    assert!(planets.len() >= 2, "two home planets");
    let (first, second) = (planets[0].clone(), planets[1].clone());
    state.exhausted_planets.insert(first.clone());
    state.exhausted_planets.insert(second.clone());
    let (pds, mechs) = (
        on_planet(&state, &home, &first, &a(), "pds"),
        on_planet(&state, &home, &second, &a(), "xxcha_mech"),
    );
    let (first_pick, second_pick) = (format!("planet|{first}"), format!("planet|{second}"));
    let (mut game, seen) = action_game(
        state,
        "a",
        None,
        &["component|leader|xxchahero-te", "place|pds", &first_pick, "place|mech", &second_pick, "stop"],
    );
    steps(&mut game, 8);
    assert_eq!(on_planet(&game.state, &home, &first, &a(), "pds") - pds, 1, "{:?}", asked(&seen));
    assert_eq!(on_planet(&game.state, &home, &second, &a(), "xxcha_mech") - mechs, 1);
    assert!(!game.state.exhausted_planets.contains(&first), "ready each planet that you place a unit on");
    assert!(!game.state.exhausted_planets.contains(&second));
    assert_eq!(leader_status(&game.state, &a(), "xxchahero-te"), Some(LeaderStatus::Purged));
}

/// The sources of the Prophecy of Kings scope (where Xxekir Grom is the original hero).
fn pok_sources() -> ti4_model::content_types::SourceSet {
    ti4_model::content_types::POK
}

#[test]
fn xxchahero_1_unlocks_in_the_prophecy_of_kings_game_on_the_third_scored_objective() {
    let unlocked = |objectives: usize| {
        let mut state = crate::fixtures::seated_game(&[("a", "xxcha"), ("b", "hacan")], pok_sources());
        score_objectives(&mut state, &a(), objectives);
        let mut game = Game::with_table(
            {
                state.phase = ti4_model::state::Phase::Action;
                state.active = Some(a());
                state
            },
            content(),
            Table::with_default(Box::new(crate::choice::AlwaysDecline)),
        )
        .with_sources(pok_sources());
        let _ = game.step();
        leader_status(&game.state, &a(), "xxchahero") == Some(LeaderStatus::Unlocked)
    };
    assert!(!unlocked(2));
    assert!(unlocked(3));
}

#[test]
fn xxchahero_2_exhausting_planets_counts_resources_and_influence_together_when_building() {
    // The only ready planet is worth 0 resources and 2+ influence. A cruiser costs 2 resources:
    // affordable only with Xxekir Grom, which treats the planet's total as both.
    let (system, planet) = influence_planet();
    for (hero, builds) in [(true, true), (false, false)] {
        let mut state = crate::fixtures::seated_game(&[("a", "xxcha"), ("b", "hacan")], pok_sources());
        state.system_mut(&system).set_control(planet.clone(), a());
        broke(&mut state, &a());
        state.exhausted_planets.remove(&planet);
        if hero {
            set_leader(&mut state, &a(), "xxchahero", LeaderStatus::Unlocked);
        }
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let cruisers = ships(&state, &home, &a(), "cruiser");
        let (pick, seen) = super::d2::Pick::new(&[
            TACTICAL_ACTION_ID,
            home.as_str(),
            "done_moving",
            "build|cruiser|1",
            "done_producing",
        ]);
        let mut state = state;
        state.phase = ti4_model::state::Phase::Action;
        state.active = Some(a());
        let mut game = Game::with_table(state, content(), Table::with_default(Box::new(pick)))
            .with_sources(pok_sources())
            .with_galaxy(plain_hub().galaxy);
        steps(&mut game, 10);
        assert_eq!(was_offered(&seen, "build|cruiser|1"), builds, "{:?}", asked(&seen));
        assert_eq!(ships(&game.state, &home, &a(), "cruiser") - cruisers, usize::from(builds));
    }
}

#[test]
fn nf_1_nullification_field_ends_the_activating_players_turn_for_a_token_and_exhausts() {
    // Sol activates the system Xxcha's cruiser sits in, with a carrier next door ready to move in.
    for (answer, ended) in [("use", true), ("decline", false)] {
        let hub = plain_hub();
        let (target, from) = (hub.outer[0].clone(), hub.outer[1].clone());
        let mut state = seated_all(&[("a", "sol"), ("b", "xxcha")]);
        blank_hub(&mut state, &hub);
        super::give(&mut state, "b", "nf");
        state.player_mut(&b()).unwrap().strategic_tokens = 2;
        put(&mut state, &sys(&target), "cruiser", &b(), 1);
        put(&mut state, &sys(&from), "carrier", &a(), 1);
        let move_it = format!("move|{from}|0");
        let (game, seen) = tactical(
            state,
            "a",
            &hub.galaxy,
            &target,
            &[answer, &move_it],
            "TURN_ENDED_BY_NULLIFICATION_FIELD:b",
        );
        let nf = "TURN_ENDED_BY_NULLIFICATION_FIELD:b";
        assert_eq!(logged(&game, nf), ended, "{:?}", asked(&seen));
        assert_eq!(was_offered(&seen, &move_it), !ended, "no movement once the turn is over");
        assert_eq!(tech_exhausted(&game.state, &b(), "nf"), ended);
        assert_eq!(game.state.player(&b()).unwrap().strategic_tokens, if ended { 1 } else { 2 });
    }
}

#[test]
fn it_1_instinct_training_cancels_an_action_card_played_in_a_running_game_for_a_strategy_token() {
    // Sol plays Frontline Deployment (3 infantry on a planet it controls); Xxcha cancels it.
    let ability = "technology:xxcha:it:ACTION_CARD_PLAYED:when";
    for (tech, cancelled) in [(true, true), (false, false)] {
        let mut state = seated_all(&[("a", "sol"), ("b", "xxcha")]);
        if tech {
            super::give(&mut state, "b", "it");
        }
        state.player_mut(&b()).unwrap().strategic_tokens = 2;
        state.player_mut(&a()).unwrap().action_cards =
            vec![ti4_model::id::ActionCardId::new("f_deployment")];
        let infantry = |state: &GameState| {
            state
                .controlled_planets(&a())
                .into_iter()
                .map(|(system, planet)| {
                    state
                        .system_state(system)
                        .on_planet_of(planet, &a())
                        .into_iter()
                        .filter(|unit| unit.type_id.as_str().contains("infantry"))
                        .count()
                })
                .sum::<usize>()
        };
        let before = infantry(&state);
        let (mut game, seen) =
            super::d2::eager_game(state, "a", None, &["action_card|0", ability, "use"]);
        steps(&mut game, 4);
        assert_eq!(was_offered(&seen, ability), tech, "{:?}", asked(&seen));
        assert_eq!(
            infantry(&game.state) > before,
            !cancelled,
            "{:?}",
            asked(&seen)
        );
        assert_eq!(
            game.state.player(&b()).unwrap().strategic_tokens,
            if cancelled { 1 } else { 2 }
        );
        assert_eq!(tech_exhausted(&game.state, &b(), "it"), cancelled);
    }
}

/// Hacan (`b`) activates the ring system `target` (holding its cruiser) on a blank hub; Xxcha (`a`)
/// has `units` placed by `place` first. Returns the space cannon rolls Xxcha made: `(hits_on, dice)`.
fn cannon_rolls_at(
    hub: &Hub,
    place: impl Fn(&mut GameState),
) -> Vec<(Option<u32>, usize)> {
    let target = hub.outer[0].clone();
    let mut state = seated_all(&[("a", "xxcha"), ("b", "hacan")]);
    // Antimass Deflectors (a starting technology) would make every cannon die one harder.
    state
        .player_mut(&b())
        .unwrap()
        .technologies
        .remove(&ti4_model::id::TechnologyId::new("amd"));
    blank_hub(&mut state, hub);
    put(&mut state, &sys(&target), "cruiser", &b(), 1);
    place(&mut state);
    let (mut game, _) = action_game(
        state,
        "b",
        Some(hub.galaxy.clone()),
        &[TACTICAL_ACTION_ID, &target, "done_moving"],
    );
    for _ in 0..12 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if logged(&game, "TACTICAL_ACTION_COMPLETE") {
            break;
        }
    }
    game.rolls()
        .iter()
        .filter(|roll| roll.reason == "space cannon" && roll.by.as_deref() == Some("a"))
        .map(|roll| (roll.hits_on, roll.faces.len()))
        .collect()
}

#[test]
fn unit_xxcha_flagship_0_loncara_ssodu_fights_with_two_dice_on_7_and_sustains() {
    assert_eq!(space_profile("xxcha", "xxcha_flagship"), (Some(7), 2));
    let mut state = xxcha_game();
    put(&mut state, &sys("19"), "xxcha_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let _ = space_fight(&mut state, &sys("19"), &[10; 8], &mut Table::new());
    assert!(
        state
            .system_state(&sys("19"))
            .units
            .iter()
            .any(|unit| unit.type_id.as_str() == "xxcha_flagship" && unit.sustained_damage),
        "the hit is sustained"
    );
}

#[test]
fn unit_xxcha_flagship_1_loncara_ssodu_fires_three_space_cannon_dice_on_5_at_ships_in_adjacent_systems() {
    let hub = plain_hub();
    let (next_door, far) = (hub.outer[1].clone(), hub.across(&hub.outer[0]));
    let adjacent = cannon_rolls_at(&hub, |state| {
        put(state, &sys(&next_door), "xxcha_flagship", &a(), 1);
    });
    assert_eq!(adjacent, vec![(Some(5), 3)], "SPACE CANNON 5 (x3) from the neighbouring system");
    let distant = cannon_rolls_at(&hub, |state| {
        put(state, &sys(&far), "xxcha_flagship", &a(), 1);
    });
    assert!(distant.is_empty(), "two systems away it cannot fire: {distant:?}");
}

#[test]
fn unit_xxcha_mech_1_indomitus_fires_one_space_cannon_die_on_8_at_ships_in_adjacent_systems() {
    let hub = plain_hub();
    let (next_door, far) = (hub.outer[1].clone(), hub.across(&hub.outer[0]));
    let adjacent = cannon_rolls_at(&hub, |state| {
        put_on_planet(state, &sys(&next_door), &first_planet(&next_door), "xxcha_mech", &a(), 1);
    });
    assert_eq!(adjacent, vec![(Some(8), 1)], "SPACE CANNON 8 from a planet next door");
    let distant = cannon_rolls_at(&hub, |state| {
        put_on_planet(state, &sys(&far), &first_planet(&far), "xxcha_mech", &a(), 1);
    });
    assert!(distant.is_empty(), "two systems away it cannot fire: {distant:?}");
}

#[test]
fn xxchabt_1_archons_gift_lets_an_influence_planet_pay_for_a_unit() {
    // The only ready planet is worth 0 resources and 2+ influence: a cruiser (2) is affordable
    // only when resources and influence are interchangeable.
    let (system, planet) = influence_planet();
    for (breakthrough, builds) in [(true, true), (false, false)] {
        let mut state = xxcha_game();
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("xxchabt"));
        }
        state.system_mut(&system).set_control(planet.clone(), a());
        broke(&mut state, &a());
        state.exhausted_planets.remove(&planet);
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        let cruisers = ships(&state, &home, &a(), "cruiser");
        let pay = format!("exhaust|{planet}");
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(plain_hub().galaxy),
            &[
                TACTICAL_ACTION_ID,
                home.as_str(),
                "done_moving",
                "build|cruiser|1",
                &pay,
                "done_producing",
            ],
        );
        steps(&mut game, 10);
        assert_eq!(was_offered(&seen, "build|cruiser|1"), builds, "{:?}", asked(&seen));
        assert_eq!(ships(&game.state, &home, &a(), "cruiser") - cruisers, usize::from(builds));
        assert_eq!(game.state.exhausted_planets.contains(&planet), builds);
    }
}
