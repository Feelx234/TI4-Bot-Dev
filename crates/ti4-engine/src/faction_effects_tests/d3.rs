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
