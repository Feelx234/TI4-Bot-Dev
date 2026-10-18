//! Depth tests, group d4: Argent Flight, Empyrean, Mahact, Naaz-Rokha, Nomad (see mod.rs and
//! docs/FACTION_EFFECT_COVERAGE.md). Each test drives the engine's own flow (a `Game` with a
//! scripted table, a whole space combat or invasion with loaded dice, or the armed resolver
//! emitting the real event) and asserts what the card promises, plus the opposite case where cheap.
//!
//! Test names: `<card id>_<effect no.>_<what>`; unit tests `unit_<unit id>_<n>_<what>`.

use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{LeaderId, ObjectiveId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus, Phase};

use super::d2::{
    Pick, Seen, action_game, eager_game, first_roll, invade, leader_status, on_planet, padded_hits,
    run_status_phase, score_objectives, set_leader, ships, space_fight, step_until,
    unlocks_in_the_game, was_offered, was_offered_like,
};
use super::{content, scripted, seated_all};
use crate::choice::{AlwaysDecline, Choice, ChoiceOption, Decider, IllegalChoice, Table, Window as _};
use crate::fixtures::{put, put_on_planet};
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

/// A table answering from `rules` (see [`Pick`]); unmatched questions decline.
fn table(rules: &[&str]) -> Table {
    Table::with_default(Box::new(Pick::new(rules).0))
}

/// Three real objectives scored through `objectives::award` (the scoring path every phase uses)
/// unlock a hero; two do not.
fn hero_unlocks_by_scoring(faction: &str, hero: &str) {
    let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
    assert_eq!(
        leader_status(&state, &a(), hero),
        Some(LeaderStatus::Locked)
    );
    let score = |state: &mut GameState, alias: &str| {
        crate::objectives::award(state, content(), DEFAULT, &a(), &ObjectiveId::new(alias))
            .expect("the objective scores");
    };
    score(&mut state, "corner");
    score(&mut state, "develop");
    assert_eq!(
        leader_status(&state, &a(), hero),
        Some(LeaderStatus::Locked),
        "two scored objectives"
    );
    score(&mut state, "diversify");
    assert_eq!(
        leader_status(&state, &a(), hero),
        Some(LeaderStatus::Unlocked),
        "the third scored objective unlocks the hero the moment it is scored"
    );
}

/// An action-phase game of `state` on the one-ring map `hub`: `active` takes a tactical action
/// into `target`, then the table answers `rules` until the action completes.
fn tactical(
    state: GameState,
    active: &str,
    galaxy: Galaxy,
    target: &str,
    rules: &[&str],
) -> (Game<'static>, Seen) {
    let mut all = vec![TACTICAL_ACTION_ID, target];
    all.extend_from_slice(rules);
    let (mut game, seen) = action_game(state, active, Some(galaxy), &all);
    for _ in 0..60 {
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        if game
            .events
            .iter()
            .any(|event| event == "TACTICAL_ACTION_COMPLETE")
        {
            break;
        }
    }
    (game, seen)
}

/// The score of votes behind each outcome of the first agenda a driven agenda phase resolved.
fn agenda_votes(game: &Game<'_>) -> std::collections::BTreeMap<String, i64> {
    game.state
        .agenda_log
        .first()
        .expect("an agenda was resolved")
        .votes
        .clone()
}

/// An agenda-phase game with a For/Against law on top of the deck; `speaker` is the speaker.
fn agenda_game(mut state: GameState, speaker: &str, rules: &[&str]) -> (Game<'static>, Seen) {
    let law = content()
        .records(ti4_model::content_types::ContentType::Agendas)
        .iter()
        .find(|record| {
            record.text("type") == Some("Law") && record.text("target") == Some("For/Against")
        })
        .and_then(|record| record.text("alias"))
        .expect("the corpus has a For/Against law")
        .to_owned();
    state.phase = Phase::Agenda;
    state.custodians_removed = true;
    state.agenda_deck = vec![law];
    state.speaker = PlayerId::new(speaker);
    let (pick, seen) = Pick::new(rules);
    let game = Game::with_table(state, content(), Table::with_default(Box::new(pick)))
        .with_sources(DEFAULT);
    (game, seen)
}

/// Step an agenda-phase game to its end.
fn run_agenda(game: &mut Game<'_>) {
    for _ in 0..200 {
        if game.state.phase != Phase::Agenda {
            return;
        }
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    panic!("the agenda phase never ended; log: {:?}", game.events);
}

/// Seven ordinary systems that all hold a planet and are nobody's home, as a one-ring map.
fn planet_hub() -> crate::fixtures::Hub {
    let ids: Vec<String> = crate::fixtures::plain_systems(120)
        .into_iter()
        .filter(|id| {
            !crate::seating::is_mecatol(id)
                && !id.starts_with("fracture")
                && !ti4_content::galaxy::is_home_system(content(), id, DEFAULT)
                && !ti4_content::galaxy::planets_in(content(), id, DEFAULT).is_empty()
        })
        .take(7)
        .collect();
    crate::fixtures::hub_from(&ids)
}

fn first_planet(system: &SystemId) -> PlanetId {
    PlanetId::new(ti4_content::galaxy::planets_in(content(), system.as_str(), DEFAULT)[0].id())
}

/// Every roll `who` made for `reason` in `dice`: `(hits_on, number of dice)`.
fn rolls(dice: &crate::dice::Dice, who: &str, reason: &str) -> Vec<(Option<u32>, usize)> {
    dice.history()
        .iter()
        .filter(|roll| roll.by.as_deref() == Some(who) && roll.reason == reason)
        .map(|roll| (roll.hits_on, roll.faces.len()))
        .collect()
}

/// Was `id` offered to `player` in some question?
fn offered_to(seen: &Seen, player: &str, id: &str) -> bool {
    seen.borrow()
        .iter()
        .any(|asked| asked.player == player && asked.options.iter().any(|o| o == id))
}

/// Number of `kind` units `owner` has in the space area of `system`, damaged or not.
fn space_count(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
    ships(state, system, owner, kind)
}

/// [`space_fight`] with the typed timing windows open (a resolver armed as the game arms it), so
/// the reactions that hang on combat events (`UNIT_ABILITY_ROLLED`, `SPACE_COMBAT_STARTED`, ...)
/// fire as they do in a tactical action. Dice faces are loaded first, then hits.
fn armed_fight(
    state: &mut GameState,
    system: &SystemId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::combat::CombatOutcome, crate::dice::Dice) {
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut sequence = crate::event::EventSequence::new();
    let mut dice = crate::dice::Dice::from_faces(padded_hits(faces));
    let mut rng = crate::rng::GameRng::new(11);
    state.active_system = Some(system.clone());
    let outcome = {
        let mut ctx = crate::choice::Resolving {
            content: content(),
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        crate::combat::resolve_resolving(state, &mut ctx, system, None)
            .expect("the combat resolves")
    };
    (outcome, dice)
}

/// Take every unit of `owner` off the board (ships and ground forces).
fn strip_units(state: &mut GameState, owner: &PlayerId) {
    for board in state.board.values_mut() {
        board.units.retain(|unit| unit.owner != *owner);
        for units in board.planet_units.values_mut() {
            units.retain(|unit| unit.owner != *owner);
        }
    }
}

/// [`invade`] with the typed timing windows open, as [`armed_fight`] does for a space combat.
fn armed_invade(
    state: &mut GameState,
    system: &SystemId,
    invader: &PlayerId,
    faces: &[u32],
    table: &mut Table,
) -> (crate::invasion::InvasionReport, crate::dice::Dice) {
    let mut resolver = crate::fixtures::armed_resolver(state);
    let mut sequence = crate::event::EventSequence::new();
    let mut dice = crate::dice::Dice::from_faces(padded_hits(faces));
    let mut rng = crate::rng::GameRng::new(11);
    state.active_system = Some(system.clone());
    let report = {
        let mut window = crate::invasion::InvasionWindow::new(
            state, content(), DEFAULT, &mut dice, &mut rng, invader, system,
        );
        let mut ctx = crate::choice::Resolving {
            content: content(),
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        while !window.is_done() {
            window.drive(state, &mut ctx).expect("the invasion resolves");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(state, &mut ctx);
        }
        window.into_report()
    };
    (report, dice)
}

// -- Argent Flight --------------------------------------------------------------------------------

fn argent_game() -> GameState {
    seated_all(&[("a", "argent"), ("b", "sol")])
}

#[test]
fn argenthero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("argent", "argenthero");
}

#[test]
fn raid_formation_1_excess_barrage_hits_damage_a_sustain_ship_in_a_real_combat() {
    // A destroyer barrages 9 x2 (both dice hit): the opponent's one fighter takes one hit, the
    // other is in excess and, for the Argent only, damages one of two dreadnoughts. The rest of
    // the fight is all misses until the opponent's last round, so nothing else touches them.
    let run = |faction: &str, destroyer: &str| {
        let mut state = seated_all(&[("a", faction), ("b", "sol")]);
        put(&mut state, &sys("19"), destroyer, &a(), 1);
        put(&mut state, &sys("19"), "fighter", &b(), 1);
        put(&mut state, &sys("19"), "dreadnought", &b(), 2);
        let _ = space_fight(
            &mut state,
            &sys("19"),
            &[10, 10, 1, 1, 1, 1, 1, 1, 1, 1],
            &mut table(&[]),
        );
        let units = &state.system_state(&sys("19")).units;
        let count = |kind: &str, damaged: bool| {
            units
                .iter()
                .filter(|u| {
                    u.owner == b() && u.type_id.as_str() == kind && u.sustained_damage == damaged
                })
                .count()
        };
        (count("fighter", false), count("dreadnought", true))
    };
    assert_eq!(run("argent", "argent_destroyer"), (0, 1), "Raid Formation");
    assert_eq!(run("sol", "destroyer"), (0, 0), "another faction's barrage");
}

/// Argent (`a`, the speaker) votes for with one planet, Sol (`b`) against with one planet, the
/// Hacan (`c`) abstains. Returns who was asked the first vote question and the tally.
fn zeal_vote(argent: &str) -> (String, std::collections::BTreeMap<String, i64>, i64, i64) {
    let state = seated_all(&[("a", argent), ("b", "sol"), ("c", "hacan")]);
    let planet_a = state.controlled_planets(&a())[0].1.to_string();
    let planet_b = state.controlled_planets(&b())[0].1.to_string();
    let worth = |planet: &str| {
        crate::vote::influence_of(&state, content(), DEFAULT, &PlanetId::new(planet))
    };
    let (own, rival) = (worth(&planet_a), worth(&planet_b));
    // The questions come in voting order: with Zeal the Argent seat first, otherwise last.
    let rules: Vec<&str> = if argent == "argent" {
        vec!["for", planet_a.as_str(), "against", planet_b.as_str()]
    } else {
        vec![
            "against",
            planet_b.as_str(),
            "decline",
            "for",
            planet_a.as_str(),
        ]
    };
    let (mut game, seen) = agenda_game(state, "a", &rules);
    run_agenda(&mut game);
    let first = seen
        .borrow()
        .iter()
        .find(|asked| asked.prompt.starts_with("a vote") || asked.options.contains(&"for".into()))
        .map(|asked| asked.player.clone())
        .expect("a vote question");
    (first, agenda_votes(&game), own, rival)
}

#[test]
fn zeal_1_the_argent_votes_first_even_as_the_speaker() {
    // The speaker votes last (8.2ii); the Argent seat, speaker or not, is asked first.
    let (first, _, _, _) = zeal_vote("argent");
    assert_eq!(first, "a");
    let (first, _, _, _) = zeal_vote("jolnar");
    assert_eq!(
        first, "b",
        "without Zeal the seat left of the speaker opens the vote"
    );
}

#[test]
fn zeal_2_casting_a_vote_adds_one_vote_per_player_including_the_argent() {
    let (_, votes, own, rival) = zeal_vote("argent");
    assert_eq!(votes.get("for"), Some(&(own + 3)), "three seats: +3");
    assert_eq!(votes.get("against"), Some(&rival), "Sol gets no bonus");
    // Another faction casting the same planet gets only its influence.
    let (_, votes, own, _) = zeal_vote("jolnar");
    assert_eq!(votes.get("for"), Some(&own));
}

#[test]
fn ah_1_other_players_cannot_move_ships_through_a_system_with_argent_structures() {
    // Sol's cruiser (move 2) crosses the hub centre to reach the far side, unless the Argent has
    // a structure in the centre system and Aerie Hololattice.
    let run = |tech: bool, mover: &str| {
        let hub = planet_hub();
        let (origin, centre) = (hub.outer[0].clone(), SystemId::new(hub.centre.clone()));
        let target = hub.across(&origin);
        let mut state = argent_game();
        if tech {
            super::give(&mut state, "a", "ah");
        }
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), a());
        put_on_planet(&mut state, &centre, &planet, "pds", &a(), 1);
        put(
            &mut state,
            &sys(&origin),
            "cruiser",
            &PlayerId::new(mover),
            1,
        );
        let move_rule = format!("move|{origin}|0");
        let (game, seen) = tactical(
            state,
            mover,
            hub.galaxy,
            &target,
            &[move_rule.as_str(), "done_moving"],
        );
        (
            was_offered(&seen, &move_rule),
            ships(&game.state, &sys(&target), &PlayerId::new(mover), "cruiser"),
        )
    };
    assert_eq!(run(true, "b"), (false, 0), "the wall stops Sol");
    assert_eq!(run(false, "b"), (true, 1), "no card, no wall");
    assert_eq!(run(true, "a"), (true, 1), "the owner passes");
}
#[test]
fn ah_2_a_planet_with_an_argent_structure_produces_one_unit_for_its_owner() {
    // A PDS is no space dock: only the card gives its planet PRODUCTION 1. Activating the system
    // opens a production window with one unit to place, and a second build is not offered.
    let run = |tech: bool, owner: &str| {
        let hub = planet_hub();
        let centre = SystemId::new(hub.centre.clone());
        let mut state = seated_all(&[("a", "argent"), ("b", "sol")]);
        if tech {
            super::give(&mut state, "a", "ah");
        }
        let planet = first_planet(&centre);
        let owner = PlayerId::new(owner);
        state
            .system_mut(&centre)
            .set_control(planet.clone(), owner.clone());
        put_on_planet(&mut state, &centre, &planet, "pds", &owner, 1);
        state.player_mut(&owner).unwrap().trade_goods = 10;
        let (mut game, seen) = eager_game(
            state,
            owner.as_str(),
            Some(hub.galaxy),
            &[
                TACTICAL_ACTION_ID,
                centre.as_str(),
                "done_moving",
                "build|cruiser|1",
            ],
        );
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE") {
                break;
            }
        }
        let asked_to_produce = seen
            .borrow()
            .iter()
            .filter(|q| q.prompt.starts_with("produce in"))
            .count();
        (
            asked_to_produce,
            ships(&game.state, &centre, &owner, "cruiser"),
        )
    };
    assert_eq!(run(true, "a"), (1, 1), "one unit, then the window closes");
    assert_eq!(run(false, "a").0, 0, "no card, no production");
    assert_eq!(run(true, "b").0, 0, "the card is the Argent player's only");
}

/// Argent `a` with `destroyer` against Sol's cruiser and three infantry riding in space. The barrage
/// dice `afb` come first, then the cruiser kills the lone destroyer and the fight is over.
fn strike_wing_fight(
    tech: bool,
    destroyer: &str,
    afb: &[u32],
) -> (usize, usize, crate::dice::Dice) {
    let mut state = argent_game();
    if tech {
        super::give(&mut state, "a", "swa2");
    }
    put(&mut state, &sys("19"), destroyer, &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    put(&mut state, &sys("19"), "infantry", &b(), 3);
    let mut faces = afb.to_vec();
    faces.extend([1, 10]);
    let (_, dice) = armed_fight(&mut state, &sys("19"), &faces, &mut table(&[]));
    (
        ships(&state, &sys("19"), &b(), "infantry"),
        ships(&state, &sys("19"), &b(), "cruiser"),
        dice,
    )
}

#[test]
fn swa2_3_each_nine_or_ten_of_the_barrage_destroys_an_infantry_in_the_space_area() {
    let (infantry, cruiser, _) = strike_wing_fight(true, "argent_destroyer2", &[9, 10, 4]);
    assert_eq!(
        (infantry, cruiser),
        (1, 1),
        "the 9 and the 10 each kill one infantry"
    );
    // The first generation has the same dice but no such ability; a barrage of 9s kills nothing.
    let (infantry, cruiser, _) = strike_wing_fight(false, "argent_destroyer", &[9, 10]);
    assert_eq!((infantry, cruiser), (3, 1));
}

#[test]
fn unit_argent_destroyer2_0_strike_wing_alpha_ii_barrages_6_x3_and_fights_on_7() {
    let (_, _, dice) = strike_wing_fight(true, "argent_destroyer2", &[1, 1, 1]);
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage"), [(Some(6), 3)]);
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(7), 1));
}

#[test]
fn swa2_1_and_2_researching_the_upgrade_turns_the_destroyers_into_strike_wing_alpha_ii() {
    let mut state = argent_game();
    put(&mut state, &sys("19"), "argent_destroyer", &a(), 1);
    super::give(&mut state, "a", "swa2");
    crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
    assert_eq!(
        space_count(&state, &sys("19"), &a(), "argent_destroyer2"),
        1
    );
    assert_eq!(space_count(&state, &sys("19"), &a(), "argent_destroyer"), 0);
    // ... and the upgraded unit fights as one: barrage 6 x3, combat 7.
    put(&mut state, &sys("19"), "fighter", &b(), 1);
    let (_, dice) = space_fight(&mut state, &sys("19"), &[1, 1, 1, 1], &mut table(&[]));
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage"), [(Some(6), 3)]);
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(7), 1));
}

#[test]
fn unit_argent_destroyer_0_strike_wing_alpha_i_fights_on_8_and_barrages_9_x2() {
    let (_, _, dice) = strike_wing_fight(false, "argent_destroyer", &[1, 1]);
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage"), [(Some(9), 2)]);
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(8), 1));
}

#[test]
fn argentcommander_1_six_armed_units_unlock_it_in_the_game() {
    // Units with ANTI-FIGHTER BARRAGE, SPACE CANNON or BOMBARDMENT count: five do not, six do, and
    // another faction's armed units are not the Argent's.
    let mut state = argent_game();
    strip_units(&mut state, &a());
    put(&mut state, &sys("19"), "argent_destroyer", &a(), 3);
    put(&mut state, &sys("20"), "dreadnought", &a(), 1);
    put(&mut state, &sys("21"), "pds", &a(), 1);
    put(&mut state, &sys("21"), "fighter", &a(), 4);
    put(&mut state, &sys("22"), "destroyer", &b(), 4);
    assert!(
        !unlocks_in_the_game(&state, &a(), "argentcommander"),
        "five armed units"
    );
    put(&mut state, &sys("22"), "cruiser", &a(), 1);
    assert!(
        !unlocks_in_the_game(&state, &a(), "argentcommander"),
        "a cruiser has no such ability"
    );
    put(&mut state, &sys("23"), "destroyer", &a(), 1);
    assert!(
        unlocks_in_the_game(&state, &a(), "argentcommander"),
        "six armed units"
    );
}

/// Argent `a` (commander unlocked or not) barrages with one destroyer against Sol's lone fighter.
/// The commander's window is answered when it is offered. Returns whether Sol's fighter survived
/// the barrage (it rolls in the combat that follows), and what was offered.
fn commander_barrage(unlocked: bool, faces: &[u32]) -> (bool, Seen) {
    let mut state = argent_game();
    let status = if unlocked {
        LeaderStatus::Unlocked
    } else {
        LeaderStatus::Locked
    };
    set_leader(&mut state, &a(), "argentcommander", status);
    put(&mut state, &sys("19"), "argent_destroyer", &a(), 1);
    put(&mut state, &sys("19"), "fighter", &b(), 1);
    let (pick, seen) = Pick::new(&["leader:argent:argentcommander:UNIT_ABILITY_ROLLED:when"]);
    let (_, dice) = armed_fight(
        &mut state,
        &sys("19"),
        faces,
        &mut Table::with_default(Box::new(pick)),
    );
    (first_roll(&dice, "b").is_some(), seen)
}

#[test]
fn argentcommander_2_one_unit_rolls_an_extra_die_for_a_unit_ability_only_once_unlocked() {
    // Barrage 9 x2 rolls two misses; only the extra third die hits, and it kills the fighter before
    // it can roll in the combat.
    let faces = [1, 1, 10, 1];
    let (survived, seen) = commander_barrage(true, &faces);
    assert!(was_offered(
        &seen,
        "leader:argent:argentcommander:UNIT_ABILITY_ROLLED:when"
    ));
    assert!(!survived, "the extra die hit");
    let (survived, seen) = commander_barrage(false, &faces);
    assert!(
        !was_offered_like(&seen, "argentcommander"),
        "locked: no offer"
    );
    assert!(survived, "no extra die, the fighter lives to roll");
}

/// Sol `a` holds Argent's Ambuscade and barrages with a destroyer against Argent's fighter.
fn ambuscade_barrage(answers: &[&str], faces: &[u32], destroyer: &str) -> (GameState, bool, Seen) {
    let mut state = seated_all(&[("a", "sol"), ("b", "argent")]);
    crate::promissory::take(&mut state, content(), &a(), "ambuscade:argent");
    put(&mut state, &sys("19"), destroyer, &a(), 1);
    put(&mut state, &sys("19"), "fighter", &b(), 1);
    let (pick, seen) = Pick::new(answers);
    let (_, dice) = armed_fight(
        &mut state,
        &sys("19"),
        faces,
        &mut Table::with_default(Box::new(pick)),
    );
    let survived = first_roll(&dice, "b").is_some();
    (state, survived, seen)
}

#[test]
fn ambuscade_1_the_holder_adds_a_die_to_a_unit_ability_roll_and_the_note_goes_home() {
    let play = "promissory:sol:ambuscade:UNIT_ABILITY_ROLLED:when";
    let (state, survived, seen) = ambuscade_barrage(&[play], &[1, 1, 10, 1], "destroyer");
    assert!(was_offered(&seen, play));
    assert!(!survived, "the additional die hit the fighter");
    assert_eq!(
        state.promissory_notes.get("ambuscade:argent"),
        Some(&b()),
        "back with Argent"
    );
    // Declined: the same dice miss and the holder keeps the note.
    let (state, survived, _) = ambuscade_barrage(&["decline"], &[1, 1, 10, 1], "destroyer");
    assert!(survived);
    assert_eq!(state.promissory_notes.get("ambuscade:argent"), Some(&a()));
}

#[test]
fn ambuscade_2_it_is_only_offered_on_a_unit_ability_roll() {
    let play = "promissory:sol:ambuscade:UNIT_ABILITY_ROLLED:when";
    // Cruisers alone make no unit-ability roll: nothing to play the note on.
    let mut state = seated_all(&[("a", "sol"), ("b", "argent")]);
    crate::promissory::take(&mut state, content(), &a(), "ambuscade:argent");
    put(&mut state, &sys("19"), "cruiser", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (pick, seen) = Pick::new(&[play]);
    let _ = armed_fight(
        &mut state,
        &sys("19"),
        &[1, 1, 1, 1],
        &mut Table::with_default(Box::new(pick)),
    );
    assert!(!was_offered(&seen, play));
    assert_eq!(state.promissory_notes.get("ambuscade:argent"), Some(&a()));
    // Baseline: with a barrage to roll for, the holder is asked.
    let (_, _, seen) = ambuscade_barrage(&[], &[1, 1, 1, 1], "destroyer");
    assert!(was_offered(&seen, play), "baseline: the holder is asked");
}

#[test]
fn unit_argent_flagship_1_quetzecoatl_bars_other_players_space_cannon_in_its_system() {
    // Sol's PDS in the hub centre fires at whatever Argent moves in, unless Quetzecoatl is among
    // the ships there ("other players cannot use SPACE CANNON against your ships in this system").
    let run = |flagship: bool| {
        let hub = planet_hub();
        let (origin, centre) = (hub.outer[0].clone(), SystemId::new(hub.centre.clone()));
        let mut state = argent_game();
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), b());
        put_on_planet(&mut state, &centre, &planet, "pds", &b(), 1);
        put(&mut state, &sys(&origin), "cruiser", &a(), 1);
        if flagship {
            put(&mut state, &sys(&origin), "argent_flagship", &a(), 1);
        }
        let (game, _) = tactical(
            state,
            "a",
            hub.galaxy,
            centre.as_str(),
            &["~move|", "~move|", "done_moving"],
        );
        let cannon_rolls = game
            .rolls()
            .iter()
            .filter(|roll| roll.reason.contains("space cannon") && roll.by.as_deref() == Some("b"))
            .count();
        (cannon_rolls, ships(&game.state, &centre, &a(), "cruiser"))
    };
    let (shots, arrived) = run(true);
    assert_eq!(
        (shots, arrived),
        (0, 1),
        "the flagship's escort is not shot at"
    );
    let (shots, arrived) = run(false);
    assert!(shots > 0, "without the flagship the PDS fires");
    assert!(arrived <= 1);
}
#[test]
fn unit_argent_mech_1_aerie_sentinel_rides_without_using_capacity() {
    // A carrier (capacity 4) is loaded with the mech first: the Argent mech leaves all 4 places
    // free, Sol's mech takes one.
    let free_after_the_mech = |faction: &str, mech: &str| {
        let hub = planet_hub();
        let (origin, centre) = (sys(&hub.outer[0]), SystemId::new(hub.centre.clone()));
        let mut state = seated_all(&[("a", faction), ("b", "hacan")]);
        let planet = first_planet(&origin);
        state.system_mut(&origin).set_control(planet.clone(), a());
        put(&mut state, &origin, "carrier", &a(), 1);
        put_on_planet(&mut state, &origin, &planet, "infantry", &a(), 4);
        put_on_planet(&mut state, &origin, &planet, mech, &a(), 1);
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy),
            &[
                TACTICAL_ACTION_ID,
                centre.as_str(),
                "~move|",
                "load|4",
                "done_loading",
            ],
        );
        step_until(&mut game, "TACTICAL_ACTION_COMPLETE", 30);
        let prompts: Vec<String> = seen
            .borrow()
            .iter()
            .filter(|asked| asked.prompt.starts_with("load carrier"))
            .map(|asked| asked.prompt.clone())
            .collect();
        prompts
    };
    let argent = free_after_the_mech("argent", "argent_mech");
    assert!(
        argent[0].contains("(4 free)") && argent[1].contains("(4 free)"),
        "{argent:?}"
    );
    let sol = free_after_the_mech("sol", "sol_mech");
    assert!(
        sol[0].contains("(4 free)") && sol[1].contains("(3 free)"),
        "{sol:?}"
    );
}
#[test]
fn argenthero_2_flock_migration_moves_ships_to_safe_tokened_systems_and_is_purged() {
    let hub = crate::fixtures::plain_hub();
    let (from, safe, held) = (sys(&hub.outer[0]), sys(&hub.outer[2]), sys(&hub.outer[4]));
    let board = |status: LeaderStatus| {
        let mut state = argent_game();
        strip_units(&mut state, &a());
        put(&mut state, &from, "carrier", &a(), 1);
        put(&mut state, &from, "infantry", &a(), 1);
        put(&mut state, &from, "cruiser", &a(), 1);
        state.system_mut(&safe).command_tokens.insert(a());
        state.system_mut(&held).command_tokens.insert(a());
        put(&mut state, &held, "cruiser", &b(), 1);
        set_leader(&mut state, &a(), "argenthero", status);
        state
    };
    let hero = "component|leader|argenthero";
    // Locked: not offered.
    let (mut game, seen) = action_game(
        board(LeaderStatus::Locked),
        "a",
        Some(hub.galaxy.clone()),
        &[hero],
    );
    assert_eq!(game.step().error, None);
    assert!(!was_offered(&seen, hero));
    // Unlocked: both ships fly (the carrier takes the infantry along) and the hero is purged.
    let to_safe = format!("to|{safe}");
    let (mut game, seen) = action_game(
        board(LeaderStatus::Unlocked),
        "a",
        Some(hub.galaxy.clone()),
        &[hero, "ship|0", &to_safe, "cargo|0", "ship|0", &to_safe],
    );
    step_until(&mut game, "COMPONENT_ACTION_RESOLVED", 12);
    assert!(was_offered(&seen, &to_safe));
    assert!(
        !was_offered_like(&seen, &format!("to|{held}")),
        "never beside a foreign ship"
    );
    assert_eq!(ships(&game.state, &safe, &a(), "carrier"), 1);
    assert_eq!(ships(&game.state, &safe, &a(), "cruiser"), 1);
    assert_eq!(
        ships(&game.state, &safe, &a(), "infantry"),
        1,
        "the cargo came along"
    );
    assert_eq!(ships(&game.state, &from, &a(), "carrier"), 0);
    assert_eq!(
        leader_status(&game.state, &a(), "argenthero"),
        Some(LeaderStatus::Purged)
    );
}
#[test]
fn argentbt_1_wing_transfer_tokens_an_adjacent_system_then_moves_ships_among_them() {
    let hub = crate::fixtures::plain_hub();
    let (active, near) = (sys(&hub.outer[1]), sys(&hub.outer[0]));
    let window = "breakthrough:argent:argentbt:SYSTEM_ACTIVATED:when";
    let run = |breakthrough: bool| {
        let mut state = argent_game();
        strip_units(&mut state, &a());
        put(&mut state, &active, "carrier", &a(), 1);
        put(&mut state, &near, "cruiser", &a(), 1);
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("argentbt"));
        }
        let to_active = format!("to|{active}");
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy.clone()),
            &[
                TACTICAL_ACTION_ID,
                active.as_str(),
                window,
                &format!("token|{near}"),
                "done_moving",
                "ship|0",
                &to_active,
            ],
        );
        step_until(&mut game, "TACTICAL_ACTION_COMPLETE", 20);
        // The follow-up move is asked after the action completes.
        for _ in 0..3 {
            let _ = game.step();
        }
        (game, seen)
    };
    let (game, seen) = run(true);
    assert!(was_offered(&seen, window));
    assert!(
        game.state.system_state(&near).command_tokens.contains(&a()),
        "the token went to the adjacent system"
    );
    assert_eq!(
        ships(&game.state, &active, &a(), "cruiser"),
        1,
        "the cruiser joined the carrier"
    );
    assert_eq!(ships(&game.state, &near, &a(), "cruiser"), 0);
    let (game, seen) = run(false);
    assert!(!was_offered(&seen, window));
    assert!(!game.state.system_state(&near).command_tokens.contains(&a()));
    assert_eq!(
        ships(&game.state, &near, &a(), "cruiser"),
        1,
        "no card, nothing moves"
    );
}

#[test]
fn unit_argent_flagship_0_quetzecoatl_fights_with_two_dice_on_7_and_sustains() {
    let mut state = argent_game();
    put(&mut state, &sys("19"), "argent_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (outcome, dice) = space_fight(
        &mut state,
        &sys("19"),
        &[1, 1, 10, 10, 10, 1],
        &mut table(&["sustain|0"]),
    );
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(7), 2));
    assert_eq!(outcome.winner, Some(a()));
    let here = state.system_state(&sys("19"));
    let flagship = here
        .units
        .iter()
        .find(|unit| unit.type_id.as_str() == "argent_flagship")
        .expect("the flagship lived through the cruiser's hit");
    assert!(
        flagship.sustained_damage,
        "it sustained the hit instead of dying"
    );
}

// -- Empyrean -------------------------------------------------------------------------------------

fn empyrean_game() -> GameState {
    seated_all(&[("a", "empyrean"), ("b", "sol")])
}

/// `mover` takes a tactical action into `target` and moves their cruiser at `origin` there (Pick
/// leaves the move out when it is not offered). Returns whether the move was offered and whether
/// the cruiser arrived.
fn cruiser_trip(
    mut state: GameState,
    mover: &str,
    galaxy: Galaxy,
    origin: &str,
    target: &str,
) -> (bool, bool) {
    let who = PlayerId::new(mover);
    put(&mut state, &sys(origin), "cruiser", &who, 1);
    let rule = format!("move|{origin}|0");
    let (game, seen) = tactical(
        state,
        mover,
        galaxy,
        target,
        &[rule.as_str(), "done_moving"],
    );
    (
        was_offered(&seen, &rule),
        ships(&game.state, &sys(target), &who, "cruiser") == 1,
    )
}

/// A game board with no units or control anywhere, so the test map's ordinary systems are empty.
fn blank(mut state: GameState) -> GameState {
    for system in state.board.keys().cloned().collect::<Vec<_>>() {
        let board = state.system_mut(&system);
        board.units.clear();
        board.planet_units.clear();
        board.planet_control.clear();
    }
    state
}

#[test]
fn voidborn_1_empyrean_ships_cross_and_leave_a_nebula_that_stops_everyone_else() {
    let nebula = crate::fixtures::a_system_where("nebula");
    // Across: the nebula is the hub centre between origin and target (distance 2).
    let through = crate::fixtures::hub_with_centre(&nebula);
    let origin = through.outer[0].clone();
    let target = through.across(&origin);
    for (mover, moves) in [("a", true), ("b", false)] {
        let state = blank(empyrean_game());
        let trip = cruiser_trip(state, mover, through.galaxy.clone(), &origin, &target);
        assert_eq!(trip, (moves, moves), "across the nebula, {mover}");
    }
    // Out of: a ship that starts in the nebula treats its move as 1, unless it is the Empyrean's.
    let inside = crate::fixtures::hub_with_outer(&nebula);
    let far = inside.across(&nebula);
    for (mover, moves) in [("a", true), ("b", false)] {
        let state = blank(empyrean_game());
        let trip = cruiser_trip(state, mover, inside.galaxy.clone(), &nebula, &far);
        assert_eq!(trip, (moves, moves), "out of the nebula, {mover}");
    }
}

#[test]
fn aetherpassage_1_the_activating_player_may_move_through_the_empyrean_ships() {
    // Sol's cruiser has to cross the hub centre, where the Empyrean (and, in the third case, a
    // Hacan ship too) has a cruiser: ships cannot move through a system with another player's ships.
    let passage = "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after";
    let run = |accept: bool, third_player: bool| {
        let hub = crate::fixtures::plain_hub();
        let (origin, centre) = (hub.outer[0].clone(), SystemId::new(hub.centre.clone()));
        let target = hub.across(&origin);
        let mut state = blank(seated_all(&[
            ("a", "empyrean"),
            ("b", "sol"),
            ("c", "hacan"),
        ]));
        put(&mut state, &centre, "cruiser", &a(), 1);
        if third_player {
            put(&mut state, &centre, "cruiser", &c(), 1);
        }
        put(&mut state, &sys(&origin), "cruiser", &b(), 1);
        let rule = format!("move|{origin}|0");
        let answers: Vec<&str> = if accept {
            vec![passage, rule.as_str(), "done_moving"]
        } else {
            vec![rule.as_str(), "done_moving"]
        };
        let (game, seen) = tactical(state, "b", hub.galaxy, &target, &answers);
        (
            was_offered(&seen, passage),
            ships(&game.state, &sys(&target), &b(), "cruiser"),
        )
    };
    assert_eq!(run(true, false), (true, 1), "passage granted");
    assert_eq!(run(false, false), (true, 0), "declined: still blocked");
    assert_eq!(
        run(true, true),
        (true, 0),
        "a third player's ship still blocks"
    );
}

#[test]
fn vw_1_a_player_moving_in_beside_empyrean_units_gives_them_a_promissory_note() {
    let run = |tech: bool| {
        let hub = planet_hub();
        let (origin, centre) = (hub.outer[0].clone(), SystemId::new(hub.centre.clone()));
        let mut state = blank(empyrean_game());
        if tech {
            super::give(&mut state, "a", "vw");
        }
        // The Empyrean's infantry on a planet: a unit in the system, but no space fight.
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), a());
        put_on_planet(&mut state, &centre, &planet, "infantry", &a(), 1);
        put(&mut state, &sys(&origin), "cruiser", &b(), 1);
        let pick = crate::promissory::available_notes(&state, content(), &b())
            .into_iter()
            .find(|note| !note.starts_with(crate::promissory::SUPPORT_PREFIX))
            .expect("a note in Sol's hand");
        assert_eq!(state.promissory_notes.get(&pick), Some(&b()));
        let (note, rule) = (format!("note|{pick}"), format!("move|{origin}|0"));
        let (game, seen) = tactical(
            state,
            "b",
            hub.galaxy,
            centre.as_str(),
            &[rule.as_str(), "done_moving", note.as_str()],
        );
        (
            game.state.promissory_notes.get(&pick) == Some(&a()),
            was_offered(&seen, &note),
            ships(&game.state, &centre, &b(), "cruiser"),
        )
    };
    assert_eq!(
        run(true),
        (true, true, 1),
        "the chosen note went to the Empyrean"
    );
    assert_eq!(run(false), (false, false, 1), "no Voidwatch, no note");
}

#[test]
fn as_1_aetherstream_adds_one_move_when_a_neighbour_activates_next_to_an_anomaly() {
    // The asteroid field is the hub centre. Sol's carrier (move 1) on the ring is two steps from
    // the activated system, so it only gets there with the Empyrean's +1.
    let stream = "technology:empyrean:as:SYSTEM_ACTIVATED:after";
    let run = |tech: bool, accept: bool| {
        let asteroid = crate::fixtures::a_system_where("asteroid field");
        let hub = crate::fixtures::hub_with_centre(&asteroid);
        let mut state = blank(empyrean_game());
        if tech {
            super::give(&mut state, "a", "as");
        }
        put(&mut state, &sys(&hub.outer[0]), "carrier", &a(), 1);
        put(&mut state, &sys(&hub.outer[1]), "carrier", &b(), 1);
        let target = hub.outer[3].clone();
        let rule = format!("move|{}|0", hub.outer[1]);
        let answers: Vec<&str> = if accept && tech {
            vec![stream, rule.as_str(), "done_moving"]
        } else {
            vec![rule.as_str(), "done_moving"]
        };
        let (game, seen) = tactical(state, "b", hub.galaxy, &target, &answers);
        (
            was_offered(&seen, stream),
            ships(&game.state, &sys(&target), &b(), "carrier"),
        )
    };
    assert_eq!(run(true, true), (true, 1), "+1 move: the carrier arrives");
    assert_eq!(run(true, false), (true, 0), "declined: it cannot reach");
    assert_eq!(run(false, true), (false, 0), "no technology, no bonus");
}

/// An ordinary deep-space system: no planets, no anomaly, not a hyperlane.
fn empty_space() -> String {
    let systems = ti4_content::galaxy::all_systems(content(), DEFAULT);
    systems
        .iter()
        .filter(|(id, system)| {
            system.planets().is_empty()
                && !system.is_anomaly()
                && !system.is_hyperlane()
                && id.chars().all(|c| c.is_ascii_digit())
        })
        .map(|(id, _)| (*id).to_owned())
        .next()
        .expect("a planetless, anomaly-free system")
}

fn token_pools(state: &GameState, who: &PlayerId) -> (i32, i32, i32) {
    let seat = state.player(who).unwrap();
    (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens)
}

#[test]
fn empyreanagent_1_acamar_gives_a_mover_into_an_empty_system_a_command_token_then_readies() {
    let acamar = "leader:empyrean:empyreanagent:MOVEMENT_FINISHED:after";
    let run = |target: &str| {
        let hub = crate::fixtures::hub_with_centre(target);
        let (origin, centre) = (hub.outer[0].clone(), SystemId::new(hub.centre.clone()));
        let mut state = blank(empyrean_game());
        put(&mut state, &sys(&origin), "cruiser", &b(), 1);
        let before = token_pools(&state, &b());
        let rule = format!("move|{origin}|0");
        let (mut game, seen) = tactical(
            state,
            "b",
            hub.galaxy,
            centre.as_str(),
            &[rule.as_str(), "done_moving", acamar, "fleet_tokens"],
        );
        let after = token_pools(&game.state, &b());
        let spent = leader_status(&game.state, &a(), "empyreanagent");
        run_status_phase(&mut game.state);
        (
            was_offered(&seen, acamar),
            (after.0 - before.0, after.1 - before.1, after.2 - before.2),
            spent,
            leader_status(&game.state, &a(), "empyreanagent"),
        )
    };
    let empty = empty_space();
    assert_eq!(
        run(&empty),
        (
            true,
            (-1, 1, 0),
            Some(LeaderStatus::Exhausted),
            Some(LeaderStatus::Readied)
        ),
        "one tactic token spent to activate, the fleet pool chosen; the agent readies at status"
    );
    // A system with planets: no offer, no token, the agent stays ready.
    let planets = planet_hub().centre;
    let (offered, gained, status, _) = run(&planets);
    assert!(!offered);
    assert_eq!(gained, (-1, 0, 0), "only the activation cost");
    assert_eq!(status, Some(LeaderStatus::Readied));
}

#[test]
fn empyreanhero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("empyrean", "empyreanhero");
}

#[test]
fn empyreancommander_1_unlocks_in_the_game_beside_every_other_player() {
    let mut state = blank(seated_all(&[
        ("a", "empyrean"),
        ("b", "sol"),
        ("c", "hacan"),
    ]));
    let hub = crate::fixtures::plain_hub();
    let galaxy = hub.galaxy.clone();
    put(&mut state, &sys(&hub.outer[0]), "cruiser", &a(), 1);
    put(&mut state, &sys(&hub.outer[1]), "cruiser", &b(), 1);
    assert!(
        !super::d2::unlocks_on_the_map(&state, &a(), "empyreancommander", Some(galaxy.clone())),
        "only one neighbour"
    );
    put(&mut state, &sys(&hub.outer[5]), "cruiser", &c(), 1);
    assert!(
        super::d2::unlocks_on_the_map(&state, &a(), "empyreancommander", Some(galaxy)),
        "neighbours with both other players"
    );
}

#[test]
fn empyreancommander_2_returns_a_command_token_when_another_player_moves_in_once_unlocked() {
    let offer = "leader:empyrean:empyreancommander:SHIP_MOVED:after";
    let run = |status: LeaderStatus, answer: &str| {
        let hub = crate::fixtures::plain_hub();
        let (origin, centre) = (hub.outer[0].clone(), SystemId::new(hub.centre.clone()));
        let mut state = blank(empyrean_game());
        set_leader(&mut state, &a(), "empyreancommander", status);
        state.system_mut(&centre).command_tokens.insert(a());
        put(&mut state, &sys(&origin), "cruiser", &b(), 1);
        let rule = format!("move|{origin}|0");
        let (game, seen) = tactical(
            state,
            "b",
            hub.galaxy,
            centre.as_str(),
            &[rule.as_str(), answer, "done_moving"],
        );
        (
            was_offered(&seen, offer),
            game.state
                .system_state(&centre)
                .command_tokens
                .contains(&a()),
            ships(&game.state, &centre, &b(), "cruiser"),
        )
    };
    assert_eq!(
        run(LeaderStatus::Unlocked, offer),
        (true, false, 1),
        "token returned"
    );
    assert_eq!(
        run(LeaderStatus::Unlocked, "decline"),
        (true, true, 1),
        "optional"
    );
    assert_eq!(
        run(LeaderStatus::Locked, offer),
        (false, true, 1),
        "locked: nothing"
    );
}

/// `n` planetless, anomaly-free systems, as a one-ring map.
fn empty_hub() -> crate::fixtures::Hub {
    let systems = ti4_content::galaxy::all_systems(content(), DEFAULT);
    let ids: Vec<String> = systems
        .iter()
        .filter(|(id, system)| {
            system.planets().is_empty()
                && !system.is_anomaly()
                && !system.is_hyperlane()
                && id.chars().all(|c| c.is_ascii_digit())
        })
        .map(|(id, _)| (*id).to_owned())
        .take(7)
        .collect();
    assert_eq!(ids.len(), 7, "seven empty systems exist");
    crate::fixtures::hub_from(&ids)
}

#[test]
fn empyreanhero_2_conservator_procyon_places_frontier_tokens_explores_under_its_ships_and_is_purged()
 {
    let hero = "component|leader|empyreanhero";
    let hub = empty_hub();
    let under = sys(&hub.outer[0]);
    let board = |status: LeaderStatus| {
        let mut state = blank(empyrean_game());
        state.frontier_tokens.clear();
        put(&mut state, &under, "cruiser", &a(), 1);
        set_leader(&mut state, &a(), "empyreanhero", status);
        state
    };
    // Locked: not offered.
    let (mut game, seen) = action_game(
        board(LeaderStatus::Locked),
        "a",
        Some(hub.galaxy.clone()),
        &[hero],
    );
    assert_eq!(game.step().error, None);
    assert!(!was_offered(&seen, hero));
    // Unlocked: tokens in every other empty system, the one under the cruiser explored, purge.
    let (mut game, _) = eager_game(
        board(LeaderStatus::Unlocked),
        "a",
        Some(hub.galaxy.clone()),
        &[hero],
    );
    step_until(&mut game, "COMPONENT_ACTION_RESOLVED", 10);
    let tokens = &game.state.frontier_tokens;
    assert!(!tokens.contains(&under), "explored, so its token is gone");
    for other in &hub.outer[1..] {
        assert!(tokens.contains(&sys(other)), "{other} got a token");
    }
    assert!(tokens.contains(&sys(&hub.centre)));
    assert_eq!(game.state.exploration_log.len(), 1, "one exploration");
    assert_eq!(
        leader_status(&game.state, &a(), "empyreanhero"),
        Some(LeaderStatus::Purged)
    );
}
// -- Empyrean promissory notes ----------------------------------------------------------------------

const BLOOD: &str = "blood_pact:empyrean";
const DARK: &str = "dark_pact:empyrean";

/// Empyrean `a` and Sol `b` side by side in the hub centre (neighbours).
fn pact_board() -> (GameState, crate::fixtures::Hub) {
    let hub = crate::fixtures::plain_hub();
    let mut state = empyrean_game();
    let centre = SystemId::new(hub.centre.clone());
    put(&mut state, &centre, "cruiser", &a(), 1);
    put(&mut state, &centre, "cruiser", &b(), 1);
    (state, hub)
}

/// Sol holds the pact, in hand (`placed` false) or already faceup in its play area.
fn sol_holds(state: &mut GameState, note: &str, placed: bool) {
    crate::promissory::take(state, content(), &b(), note);
    if placed {
        assert!(crate::promissory::play_action_note(state, &b(), note));
    }
}

#[test]
fn dark_whispers_1_the_empyrean_starts_with_both_pacts_and_can_hand_them_over() {
    for note in [BLOOD, DARK] {
        let (state, hub) = pact_board();
        assert_eq!(
            state.promissory_notes.get(note),
            Some(&a()),
            "{note} dealt at setup"
        );
        let offer = format!("pn{note}:0");
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy),
            &["component|trade|sol", offer.as_str(), "accept"],
        );
        step_until(&mut game, "TRANSACTION_RESOLVED", 8);
        assert!(was_offered(&seen, &offer));
        assert_eq!(
            game.state.promissory_notes.get(note),
            Some(&b()),
            "Sol holds {note}"
        );
        assert!(
            !game.state.promissory_faceup.contains(note),
            "in hand, not faceup"
        );
    }
    // Another faction's table has no pacts at all.
    let plain = seated_all(&[("a", "sol"), ("b", "hacan")]);
    assert!(
        plain
            .promissory_notes
            .keys()
            .all(|note| !note.contains("_pact"))
    );
}

#[test]
fn blood_pact_1_and_dark_pact_1_the_holder_places_the_pact_faceup_with_an_action() {
    for (note, action) in [
        (BLOOD, "faction|empyrean|blood_pact"),
        (DARK, "faction|empyrean|dark_pact"),
    ] {
        let (mut state, hub) = pact_board();
        sol_holds(&mut state, note, false);
        let (mut game, seen) = action_game(state, "b", Some(hub.galaxy.clone()), &[action]);
        for _ in 0..2 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        assert!(was_offered(&seen, action), "{note}: offered to the holder");
        assert!(game.state.promissory_faceup.contains(note), "{note} faceup");
        assert_eq!(
            game.state.promissory_notes.get(note),
            Some(&b()),
            "still Sol's"
        );
        // The Empyrean is never offered its own pact's action.
        let (state, hub) = pact_board();
        let (mut game, seen) = action_game(state, "a", Some(hub.galaxy), &[]);
        assert_eq!(game.step().error, None);
        assert!(!was_offered(&seen, action));
    }
}

/// Sol (`b`) holds a faceup Dark Pact and gives the Empyrean `commodities` in a transaction.
fn dark_pact_gift(commodities: i32, placed: bool) -> (Game<'static>, (i32, i32)) {
    let (mut state, hub) = pact_board();
    sol_holds(&mut state, DARK, placed);
    // Sol's maximum is 4; it holds exactly what it gives.
    state.player_mut(&b()).unwrap().commodities = commodities;
    let before = (
        state.player(&a()).unwrap().trade_goods,
        state.player(&b()).unwrap().trade_goods,
    );
    let gift = format!("c{commodities}:0");
    let (mut game, seen) = action_game(
        state,
        "b",
        Some(hub.galaxy),
        &["component|trade|empyrean", gift.as_str(), "accept"],
    );
    step_until(&mut game, "TRANSACTION_RESOLVED", 8);
    assert!(was_offered(&seen, &gift));
    let after = (
        game.state.player(&a()).unwrap().trade_goods,
        game.state.player(&b()).unwrap().trade_goods,
    );
    (game, (after.0 - before.0, after.1 - before.1))
}

#[test]
fn dark_pact_2_a_full_commodity_gift_to_the_empyrean_gives_each_a_trade_good() {
    // Sol's maximum is 4 commodities. Given in full, the Empyrean receives them as trade goods and
    // both gain 1; one short, or with the pact still in hand, nobody gains the bonus.
    let (_, (empyrean, sol)) = dark_pact_gift(4, true);
    assert_eq!((empyrean, sol), (4 + 1, 1));
    let (_, (empyrean, sol)) = dark_pact_gift(3, true);
    assert_eq!((empyrean, sol), (3, 0), "not the maximum");
    let (_, (empyrean, sol)) = dark_pact_gift(4, false);
    assert_eq!((empyrean, sol), (4, 0), "the pact is still in hand");
}

/// Sol activates the hub centre (where the Empyrean has a cruiser) holding `note` faceup.
fn activate_with_pact(note: &str, empyrean_there: bool) -> Game<'static> {
    let (mut state, hub) = pact_board();
    let centre = SystemId::new(hub.centre.clone());
    if !empyrean_there {
        state
            .system_mut(&centre)
            .units
            .retain(|unit| unit.owner != a());
    }
    sol_holds(&mut state, note, true);
    let (mut game, _) = action_game(
        state,
        "b",
        Some(hub.galaxy),
        &[TACTICAL_ACTION_ID, centre.as_str(), "done_moving"],
    );
    step_until(&mut game, "TACTICAL_ACTION_COMPLETE", 20);
    game
}

#[test]
fn blood_pact_3_and_dark_pact_3_the_pact_goes_home_when_the_holder_activates_empyrean_units() {
    for note in [BLOOD, DARK] {
        let game = activate_with_pact(note, true);
        assert_eq!(
            game.state.promissory_notes.get(note),
            Some(&a()),
            "{note} returned"
        );
        assert!(!game.state.promissory_faceup.contains(note));
        let game = activate_with_pact(note, false);
        assert_eq!(
            game.state.promissory_notes.get(note),
            Some(&b()),
            "{note} kept: no Empyrean unit"
        );
        assert!(game.state.promissory_faceup.contains(note));
    }
}

#[test]
fn blood_pact_2_four_additional_votes_when_the_holder_and_the_empyrean_vote_alike() {
    // Empyrean a, Sol b (holder), Hacan c (speaker, votes last). a and b vote for, c against.
    let run = |pact: bool, same: bool| {
        let mut state = seated_all(&[("a", "empyrean"), ("b", "sol"), ("c", "hacan")]);
        if pact {
            sol_holds(&mut state, BLOOD, true);
        }
        let worth =
            |p: &str| crate::vote::influence_of(&state, content(), DEFAULT, &PlanetId::new(p));
        // The planet with the most influence of each seat.
        let planet = |who: &PlayerId| {
            state
                .controlled_planets(who)
                .into_iter()
                .max_by_key(|(_, p)| worth(p.as_str()))
                .map(|(_, p)| p.to_string())
                .unwrap()
        };
        let (pa, pb, pc) = (planet(&a()), planet(&b()), planet(&c()));
        let (wa, wb, wc) = (worth(&pa), worth(&pb), worth(&pc));
        let sol_outcome = if same { "for" } else { "against" };
        let (mut game, _) =
            agenda_game(state, "c", &["for", &pa, sol_outcome, &pb, "against", &pc]);
        run_agenda(&mut game);
        (agenda_votes(&game), (wa, wb, wc))
    };
    let (votes, (wa, wb, wc)) = run(true, true);
    assert_eq!(
        votes.get("for"),
        Some(&(wa + wb + 4)),
        "four additional votes"
    );
    assert_eq!(votes.get("against"), Some(&wc));
    let (votes, (wa, wb, _)) = run(false, true);
    assert_eq!(votes.get("for"), Some(&(wa + wb)), "no note, no bonus");
    let (votes, (wa, wb, wc)) = run(true, false);
    assert_eq!(votes.get("for"), Some(&wa), "different outcomes: no bonus");
    assert_eq!(votes.get("against"), Some(&(wb + wc)));
}

// -- Empyrean units -------------------------------------------------------------------------------

/// Empyrean `a` (a flagship and a cruiser) hits Sol's lone dreadnought once a round. Sol sustains
/// every hit it can; the Empyrean accepts `repairs` offers of Dynamo's repair. Returns the number
/// of combat rounds and the influence the Empyrean has left.
fn dynamo_fight(repairs: usize) -> (u32, i64) {
    let dynamo = "unit:empyrean:empyrean_flagship:SUSTAIN_DAMAGE_USED:after";
    let mut state = empyrean_game();
    put(&mut state, &sys("19"), "empyrean_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &a(), 1);
    put(&mut state, &sys("19"), "dreadnought", &b(), 1);
    let mut rules = Vec::new();
    for round in 0..2 {
        rules.push("~sustain|");
        if round < repairs {
            rules.push(dynamo);
        }
    }
    let rules: Vec<&str> = if repairs == 0 {
        vec!["~sustain|"]
    } else {
        rules
    };
    let (pick, seen) = Pick::new(&rules);
    let (outcome, dice) = armed_fight(
        &mut state,
        &sys("19"),
        &[10, 1, 1, 1, 10, 1, 1, 1],
        &mut Table::with_default(Box::new(pick)),
    );
    assert!(!dice.history().is_empty());
    let _ = &seen;
    let influence = crate::production::available(
        &state,
        content(),
        DEFAULT,
        &a(),
        crate::production::Spend::Influence,
    );
    (outcome.rounds, influence)
}

#[test]
fn unit_empyrean_flagship_1_dynamo_repairs_a_unit_that_sustained_for_two_influence() {
    let (declined, rich) = dynamo_fight(0);
    let (repaired, poorer) = dynamo_fight(2);
    assert_eq!(
        declined, 2,
        "no repair: the second hit destroys the dreadnought"
    );
    assert!(
        repaired > declined,
        "repaired twice, the dreadnought outlasts it: {repaired}"
    );
    assert_eq!(rich - poorer, 4, "two repairs at 2 influence each");
}
#[test]
fn unit_empyrean_mech_1_watcher_removes_itself_to_cancel_an_adjacent_players_action_card() {
    // Sol plays Industrial Initiative (1 trade good per industrial planet) with the Empyrean's mech
    // on a planet next door: cancelled when the Empyrean accepts, played when it declines.
    let watcher = "unit:empyrean:empyrean_mech:ACTION_CARD_PLAYED:when";
    let run = |answer: &str, mech: bool| {
        let hub = crate::fixtures::plain_hub();
        let centre = SystemId::new(hub.centre.clone());
        let mut state = blank(empyrean_game());
        let industrial = crate::fixtures::non_home_planets(300)
            .into_iter()
            .map(PlanetId::new)
            .find(|p| {
                crate::planets::traits_now(&state, content(), DEFAULT, p)
                    .contains(&"INDUSTRIAL".to_owned())
            })
            .expect("an industrial planet");
        let home = ti4_content::galaxy::all_planets(content(), DEFAULT)[industrial.as_str()]
            .system_id()
            .map(SystemId::new)
            .expect("its system");
        state.system_mut(&home).set_control(industrial, b());
        state
            .player_mut(&b())
            .unwrap()
            .action_cards
            .push(ti4_model::id::ActionCardId::new("industrial_initiative"));
        put(&mut state, &sys(&hub.outer[0]), "cruiser", &b(), 1);
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), a());
        if mech {
            put_on_planet(&mut state, &centre, &planet, "empyrean_mech", &a(), 1);
        }
        let goods = state.player(&b()).unwrap().trade_goods;
        let (mut game, seen) =
            action_game(state, "b", Some(hub.galaxy), &["~action_card|", answer]);
        for _ in 0..3 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        (
            was_offered(&seen, watcher),
            game.state.player(&b()).unwrap().trade_goods - goods,
            on_planet(&game.state, &centre, &planet, &a(), "empyrean_mech"),
            game.state.player(&b()).unwrap().action_cards.len(),
        )
    };
    assert_eq!(
        run(watcher, true),
        (true, 0, 0, 0),
        "cancelled; the mech left; the card is spent"
    );
    assert_eq!(
        run("decline", true),
        (true, 1, 1, 0),
        "declined: the card works"
    );
    assert_eq!(run(watcher, false), (false, 1, 0, 0), "no mech, no window");
}

#[test]
fn unit_empyrean_flagship_0_dynamo_fights_with_two_dice_on_5_and_sustains() {
    let mut state = empyrean_game();
    put(&mut state, &sys("19"), "empyrean_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (outcome, dice) = space_fight(
        &mut state,
        &sys("19"),
        &[1, 1, 10, 10, 10, 1],
        &mut table(&["~sustain|"]),
    );
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(5), 2));
    assert_eq!(outcome.winner, Some(a()));
    let here = state.system_state(&sys("19"));
    let flagship = here
        .units
        .iter()
        .find(|unit| unit.type_id.as_str() == "empyrean_flagship")
        .expect("the flagship lived through the hit");
    assert!(flagship.sustained_damage);
}

#[test]
fn empyreanbt_1_void_tether_closes_a_border_to_other_players_unless_allowed() {
    // Round one: the Empyrean activates the hub centre and tethers its border with outer[0].
    // Then Sol (a destroyer on outer[0], move 2) activates the system across the centre: the
    // tethered border is no way through, unless the Empyrean allows it.
    let tether = "breakthrough:empyrean:empyreanbt:SYSTEM_ACTIVATED:after";
    let allow = "breakthrough:empyrean:empyreanbt_allow:SYSTEM_ACTIVATED:after";
    let run = |breakthrough: bool, allowed: bool| {
        let hub = crate::fixtures::plain_hub();
        let (outer, centre) = (hub.outer[0].clone(), hub.centre.clone());
        let target = hub.across(&outer);
        let mut state = blank(empyrean_game());
        put(&mut state, &sys(&outer), "carrier", &a(), 1);
        put(&mut state, &sys(&outer), "destroyer", &b(), 1);
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("empyreanbt"));
        }
        let (lo, hi) = if centre < outer {
            (&centre, &outer)
        } else {
            (&outer, &centre)
        };
        let place = format!("place|{lo}|{hi}");
        let move_rule = format!("move|{outer}|0");
        let second = if allowed { allow } else { "decline" };
        let mut rules = vec![TACTICAL_ACTION_ID, &centre];
        if breakthrough {
            rules.extend([tether, &place]);
        }
        rules.extend(["done_moving", TACTICAL_ACTION_ID, &target]);
        if breakthrough {
            rules.push(second);
        }
        rules.extend([&move_rule, "done_moving"]);
        let (mut game, seen) = action_game(state, "a", Some(hub.galaxy), &rules);
        for _ in 0..40 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game
                .events
                .iter()
                .filter(|e| *e == "TACTICAL_ACTION_COMPLETE")
                .count()
                >= 2
            {
                break;
            }
        }
        (
            was_offered(&seen, &place),
            offered_to(&seen, "b", &move_rule),
            ships(&game.state, &sys(&target), &b(), "destroyer"),
        )
    };
    assert_eq!(
        run(true, false),
        (true, false, 0),
        "the border is shut to Sol"
    );
    let (_, offered_when_allowed, arrived) = run(true, true);
    assert!(
        (offered_when_allowed, arrived) == (true, 1),
        "allowed: Sol passes"
    );
    assert_eq!(
        run(false, false),
        (false, true, 1),
        "no breakthrough, no tether"
    );
}

// -- Mahact ---------------------------------------------------------------------------------------

fn mahact_game() -> GameState {
    seated_all(&[("a", "mahact"), ("b", "sol"), ("c", "hacan")])
}

fn fleet_pool(state: &GameState) -> Vec<PlayerId> {
    crate::factions::mahact::fleet_pool_owners(state, &a())
}

/// Mahact `a` attacks Sol's lone fighter in the hub centre with `attackers` dreadnoughts in a
/// whole tactical action (the dice are the game's seeded ones: the force is overwhelming).
fn mahact_attack(attackers: usize, defenders: usize) -> Game<'static> {
    let hub = crate::fixtures::plain_hub();
    let (origin, centre) = (hub.outer[0].clone(), hub.centre.clone());
    let mut state = blank(mahact_game());
    put(&mut state, &sys(&origin), "dreadnought", &a(), attackers);
    put(&mut state, &sys(&centre), "fighter", &b(), defenders);
    let rules: Vec<&str> = std::iter::repeat_n("~move|", attackers).collect();
    let (game, _) = tactical(state, "a", hub.galaxy, &centre, &[&rules[..], &["done_moving"]].concat());
    game
}

#[test]
fn edict_2_winning_a_space_combat_takes_the_opponents_token_into_the_fleet_pool() {
    let reinforcements = |game: &Game<'_>| game.state.tokens_in_reinforcements(&b());
    let before = mahact_game().tokens_in_reinforcements(&b());
    let won = mahact_attack(4, 1);
    assert!(!won.rolls().is_empty(), "a combat was fought");
    assert_eq!(fleet_pool(&won.state), vec![b()], "Sol's token went to the Mahact fleet pool");
    assert_eq!(before - reinforcements(&won), 1, "and left Sol's reinforcements");
    // Nothing was won by moving in unopposed: no combat, no token.
    let unopposed = mahact_attack(4, 0);
    assert!(fleet_pool(&unopposed.state).is_empty());
}

#[test]
fn edict_2_winning_a_ground_combat_takes_the_opponents_token_too() {
    let (system, planet) = crate::fixtures::a_placed_planet();
    let mut state = mahact_game();
    state.system_mut(&system).set_control(planet.clone(), c());
    put_on_planet(&mut state, &system, &planet, "infantry", &c(), 1);
    put(&mut state, &system, "infantry", &a(), 3);
    let (_, _) = armed_invade(&mut state, &system, &a(), &[10, 10, 10, 1], &mut Table::new());
    assert!(on_planet(&state, &system, &planet, &a(), "infantry") > 0, "the Mahact won the planet");
    assert_eq!(fleet_pool(&state), vec![c()]);
}

#[test]
fn edict_1_a_foreign_token_in_the_fleet_pool_raises_the_fleet_limit() {
    // Four cruisers against a limit of three: the Mahact is asked to remove one when it passes,
    // unless an opponent's token sits in its fleet pool.
    let run = |token: bool| {
        let mut state = mahact_game();
        strip_units(&mut state, &a());
        let limit = usize::try_from(crate::fleet::limit(&state, content(), &a())).unwrap();
        put(&mut state, &sys("19"), "cruiser", &a(), limit + 1);
        let sheet = state.player(&a()).unwrap().fleet_tokens;
        if token {
            assert!(crate::factions::mahact::add_token(&mut state, &a(), &b()));
        }
        let (mut game, seen) = action_game(state, "a", None, &["pass"]);
        for _ in 0..4 {
            assert_eq!(game.step().error, None);
        }
        let asked = seen.borrow().iter().any(|q| q.prompt.starts_with("remove a unit: over fleet supply"));
        (
            asked,
            ships(&game.state, &sys("19"), &a(), "cruiser") - limit,
            game.state.player(&a()).unwrap().fleet_tokens == sheet,
        )
    };
    assert_eq!(run(false), (true, 0, true), "over the limit: one ship is removed");
    assert_eq!(run(true), (false, 1, true), "the foreign token makes room; the sheet is untouched");
}

#[test]
fn mahacthero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("mahact", "mahacthero");
}

#[test]
fn mahacthero_2_airo_shir_aur_moves_the_fleet_in_and_fights_without_retreat_then_is_purged() {
    let hero = "component|leader|mahacthero";
    let run = |status: LeaderStatus| {
        let hub = crate::fixtures::plain_hub();
        let (origin, centre) = (sys(&hub.outer[0]), sys(&hub.centre));
        let mut state = blank(mahact_game());
        put(&mut state, &origin, "dreadnought", &a(), 3);
        put(&mut state, &centre, "cruiser", &b(), 2);
        set_leader(&mut state, &a(), "mahacthero", status);
        let (mut game, seen) = action_game(state, "a", Some(hub.galaxy), &[hero]);
        for _ in 0..6 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        (game, seen)
    };
    let (game, seen) = run(LeaderStatus::Locked);
    assert!(!was_offered(&seen, hero));
    let hub = crate::fixtures::plain_hub();
    assert_eq!(ships(&game.state, &sys(&hub.outer[0]), &a(), "dreadnought"), 3, "nothing moved");
    let (game, seen) = run(LeaderStatus::Unlocked);
    assert!(was_offered(&seen, hero));
    let hub = crate::fixtures::plain_hub();
    let (origin, centre) = (sys(&hub.outer[0]), sys(&hub.centre));
    assert_eq!(ships(&game.state, &origin, &a(), "dreadnought"), 0, "the fleet left");
    assert!(!game.rolls().is_empty(), "and fought in the destination");
    assert!(
        !was_offered_like(&seen, "retreat"),
        "no retreat during the hero's combat"
    );
    assert_eq!(ships(&game.state, &centre, &b(), "cruiser"), 0, "Sol's cruisers fell");
    assert!(ships(&game.state, &centre, &a(), "dreadnought") >= 1, "the survivors hold the system");
    assert_eq!(leader_status(&game.state, &a(), "mahacthero"), Some(LeaderStatus::Purged));
}
#[test]
fn hubris_1_the_mahact_has_no_alliance_note_and_nobody_may_give_it_theirs() {
    let hub = crate::fixtures::plain_hub();
    let mut state = seated_all(&[("a", "mahact"), ("b", "xxcha"), ("c", "hacan")]);
    crate::promissory::deal(&mut state, content(), DEFAULT);
    assert!(!state.promissory_notes.contains_key("an:mahact"), "purged at setup");
    assert!(state.promissory_notes.contains_key("an:xxcha"));
    let centre = SystemId::new(hub.centre.clone());
    for who in [a(), b(), c()] {
        put(&mut state, &centre, "cruiser", &who, 1);
    }
    set_leader(&mut state, &b(), "xxchacommander", LeaderStatus::Unlocked);
    // Xxcha's deal menu with the Mahact has no Alliance to give; with the Hacan it has.
    let menu = |partner: &str| {
        let (mut game, seen) = action_game(
            state.clone(),
            "b",
            Some(hub.galaxy.clone()),
            &[&format!("component|trade|{partner}")],
        );
        for _ in 0..3 {
            let _ = game.step();
        }
        seen
    };
    let seen = menu("mahact");
    assert!(was_offered_like(&seen, "pncf:xxcha"), "other notes can still be given");
    assert!(!was_offered_like(&seen, "an:xxcha"), "no Alliance for the Mahact");
    assert!(was_offered(&menu("hacan"), "pnan:xxcha:0"), "baseline: the Hacan can be given it");
}

/// The planet of `who` with the most influence, and its influence.
fn best_planet(state: &GameState, who: &PlayerId) -> (String, i64) {
    state
        .controlled_planets(who)
        .into_iter()
        .map(|(_, p)| {
            (
                p.to_string(),
                crate::vote::influence_of(state, content(), DEFAULT, p),
            )
        })
        .max_by_key(|(_, worth)| *worth)
        .expect("a planet")
}

#[test]
fn imperia_1_a_token_in_the_fleet_pool_lends_the_owners_unlocked_commander_ability() {
    // Xxcha's Elder Qanoj: each planet exhausted to vote gives 1 additional vote. The Mahact
    // casts a lone vote and gets the extra one only with the Xxcha token in its pool and the
    // Xxcha commander unlocked.
    let vote = |token: bool, status: LeaderStatus| {
        let mut state = seated_all(&[("a", "mahact"), ("b", "xxcha"), ("c", "hacan")]);
        set_leader(&mut state, &b(), "xxchacommander", status);
        if token {
            assert!(crate::factions::mahact::add_token(&mut state, &a(), &b()));
        }
        let (planet, worth) = best_planet(&state, &a());
        let (mut game, _) = agenda_game(state, "c", &["for", &planet]);
        run_agenda(&mut game);
        (agenda_votes(&game).get("for").copied(), worth)
    };
    let (votes, worth) = vote(true, LeaderStatus::Unlocked);
    assert_eq!(votes, Some(worth + 1), "Elder Qanoj's extra vote");
    let (votes, worth) = vote(false, LeaderStatus::Unlocked);
    assert_eq!(votes, Some(worth), "no token in the pool");
    let (votes, worth) = vote(true, LeaderStatus::Locked);
    assert_eq!(votes, Some(worth), "the commander is locked");
}
#[test]
fn gr_1_a_voter_backs_the_holders_outcome_or_returns_a_fleet_token() {
    let run = |tech: bool, tribute: &str| {
        let mut state = mahact_game();
        if tech {
            super::give(&mut state, "a", "gr");
        }
        let tokens = state.player(&b()).unwrap().fleet_tokens;
        let (planet, _) = best_planet(&state, &b());
        // a: decline its own vote, name "against"; b answers the tribute, then votes "for" if it may.
        let (mut game, seen) = agenda_game(
            state,
            "c",
            &["decline", "recombine|against", tribute, "for", &planet],
        );
        run_agenda(&mut game);
        let b_votes: Vec<Vec<String>> = seen
            .borrow()
            .iter()
            .filter(|q| q.player == "b" && q.prompt.starts_with("vote for which"))
            .map(|q| q.options.clone())
            .collect();
        (
            was_offered(&seen, "recombine|against"),
            b_votes.first().cloned().unwrap_or_default(),
            game.state.player(&b()).unwrap().fleet_tokens - tokens,
        )
    };
    // Bound: Sol's only way to vote is for "against".
    let (offered, options, tokens) = run(true, "tribute|vote");
    assert!(offered);
    assert!(options.contains(&"against".to_owned()) && !options.contains(&"for".to_owned()), "{options:?}");
    assert_eq!(tokens, 0);
    // Or Sol returns a fleet token and then votes as it likes.
    let (_, options, tokens) = run(true, "tribute|token");
    assert!(options.contains(&"for".to_owned()), "{options:?}");
    assert_eq!(tokens, -1, "a fleet token went back to reinforcements");
    // Without the technology nobody is asked.
    let (offered, options, tokens) = run(false, "tribute|vote");
    assert!(!offered);
    assert!(options.contains(&"for".to_owned()));
    assert_eq!(tokens, 0);
}
#[test]
fn mahactagent_1_jae_mir_kan_pays_a_secondary_with_the_active_players_board_token() {
    // Sol takes Trade; the Mahact follows to replenish commodities. Ready, the agent pays with one
    // of Sol's command tokens from the board instead of the Mahact's strategy token.
    let run = |agent: LeaderStatus| {
        let mut state = seated_all(&[("a", "mahact"), ("b", "sol")]);
        state.phase = Phase::Action;
        state.active = Some(b());
        let trade = state
            .unclaimed_strategy_cards
            .iter()
            .find(|card| card.as_str().contains("trade"))
            .cloned()
            .expect("the Trade card");
        state.player_mut(&b()).unwrap().strategy_cards = vec![trade];
        state.player_mut(&a()).unwrap().commodities = 0;
        state.system_mut(&sys("27")).command_tokens.insert(b());
        set_leader(&mut state, &a(), "mahactagent", agent);
        let tokens = state.player(&a()).unwrap().strategic_tokens;
        let rules: &[&str] = if agent == LeaderStatus::Readied {
            &["strategic", "done", "~agent|"]
        } else {
            &["strategic", "done", "yes"]
        };
        let (pick, seen) = Pick::eager(rules);
        let mut game =
            Game::with_table(state, content(), Table::with_default(Box::new(pick))).with_sources(DEFAULT);
        step_until(&mut game, "STRATEGIC_ACTION_COMPLETE", 12);
        let mahact = game.state.player(&a()).unwrap();
        let outcome = (
            mahact.commodities > 0,
            tokens - mahact.strategic_tokens,
            game.state.system_state(&sys("27")).command_tokens.contains(&b()),
            leader_status(&game.state, &a(), "mahactagent"),
            was_offered_like(&seen, "agent|"),
        );
        run_status_phase(&mut game.state);
        assert_eq!(
            leader_status(&game.state, &a(), "mahactagent"),
            Some(LeaderStatus::Readied),
            "ready again after the status phase"
        );
        outcome
    };
    assert_eq!(
        run(LeaderStatus::Readied),
        (true, 0, false, Some(LeaderStatus::Exhausted), true),
        "paid with Sol's board token; no strategy token spent"
    );
    assert_eq!(
        run(LeaderStatus::Exhausted),
        (true, 1, true, Some(LeaderStatus::Exhausted), false),
        "exhausted: the Mahact pays its own strategy token"
    );
}
/// The start of round one's strategy phase with the Mahact holding Sol's and Hacan's tokens in its
/// fleet pool and `holder` holding the Scepter; `holder` has ships in systems 26 and 27.
fn scepter_game(holder: &str) -> (Game<'static>, Seen) {
    let mut state = mahact_game();
    assert!(crate::factions::mahact::add_token(&mut state, &a(), &b()));
    assert!(crate::factions::mahact::add_token(&mut state, &a(), &c()));
    let holder = PlayerId::new(holder);
    for system in ["26", "27"] {
        put(&mut state, &sys(system), "carrier", &holder, 1);
    }
    crate::promissory::take(&mut state, content(), &holder, "scepter:mahact");
    let (pick, seen) = Pick::new(&["26"]);
    let mut game =
        Game::with_table(state, content(), Table::with_default(Box::new(pick))).with_sources(DEFAULT);
    assert_eq!(game.step().error, None);
    (game, seen)
}

#[test]
fn scepter_1_each_other_pool_owner_places_a_token_in_the_chosen_system() {
    // Hacan (c) holds it: the Sol token goes where Hacan chooses (26, not 27 or a home system);
    // Hacan places none of its own, though its token is in the pool too.
    let (game, seen) = scepter_game("c");
    assert!(was_offered_like(&seen, "26") && was_offered_like(&seen, "27"));
    let tokens = |system: &str| game.state.system_state(&sys(system)).command_tokens.clone();
    assert!(tokens("26").contains(&b()), "Sol's token landed in the chosen system");
    assert!(!tokens("26").contains(&c()), "the holder places none");
    assert!(!tokens("27").contains(&b()), "only the chosen system");
}

#[test]
fn scepter_2_it_returns_to_the_mahact_player_and_the_mahact_cannot_play_it() {
    let (game, _) = scepter_game("c");
    assert_eq!(game.state.promissory_notes.get("scepter:mahact"), Some(&a()), "returned");
    // Held by its own owner it does nothing at all.
    let (game, _) = scepter_game("a");
    for system in ["26", "27"] {
        let here = &game.state.system_state(&sys(system)).command_tokens;
        assert!(!here.contains(&b()) && !here.contains(&c()), "{system}");
    }
    assert_eq!(game.state.promissory_notes.get("scepter:mahact"), Some(&a()));
}

#[test]
fn mahactcommander_1_tokens_of_two_other_factions_in_the_fleet_pool_unlock_it() {
    let mut state = mahact_game();
    assert!(!unlocks_in_the_game(&state, &a(), "mahactcommander"), "no tokens");
    assert!(crate::factions::mahact::add_token(&mut state, &a(), &b()));
    assert!(!unlocks_in_the_game(&state, &a(), "mahactcommander"), "one faction");
    assert!(crate::factions::mahact::add_token(&mut state, &a(), &c()));
    assert!(unlocks_in_the_game(&state, &a(), "mahactcommander"), "two factions");
}

#[test]
fn mahactbt_1_vaults_of_the_heir_purges_a_technology_for_a_relic_then_readies_at_status() {
    let vaults = "faction|mahact|vaults|pi";
    let run = |breakthrough: bool| {
        let mut state = mahact_game();
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("mahactbt"));
        }
        let relics = state.player(&a()).unwrap().relics.len();
        let (mut game, seen) = action_game(state, "a", None, &[vaults]);
        for _ in 0..3 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        let seat = game.state.player(&a()).unwrap();
        let outcome = (
            was_offered(&seen, vaults),
            seat.technologies.contains(&ti4_model::id::TechnologyId::new("pi")),
            seat.relics.len() - relics,
        );
        (game, outcome)
    };
    let (mut game, outcome) = run(true);
    assert_eq!(outcome, (true, false, 1), "the technology is gone, a relic gained");
    assert!(
        !crate::factions::component_actions(&game.state, content(), &a())
            .iter()
            .any(|o| o.id.starts_with("faction|mahact|vaults|")),
        "exhausted"
    );
    // The game's own status phase (it announces STATUS_PHASE_ENDED, which readies the card).
    game.state.phase = Phase::Status;
    for _ in 0..40 {
        if game.state.phase != Phase::Status {
            break;
        }
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
    }
    assert_ne!(game.state.phase, Phase::Status, "the status phase ended");
    assert!(
        crate::factions::component_actions(&game.state, content(), &a())
            .iter()
            .any(|o| o.id.starts_with("faction|mahact|vaults|")),
        "ready again after the status phase"
    );
    let (_, outcome) = run(false);
    assert_eq!(outcome, (false, true, 0), "no breakthrough, nothing");
}

// -- Mahact units ---------------------------------------------------------------------------------

/// Hacan `c` lands three infantry on a planet held by the Mahact `a`'s single `infantry` (all
/// three invader dice hit; the defender's die misses). Returns the state after the invasion.
fn legionnaire_dies(infantry: &str, answer: &str, commodities: i32) -> (GameState, Option<u32>) {
    let (system, planet) = crate::fixtures::a_placed_planet();
    let mut state = mahact_game();
    state.system_mut(&system).set_control(planet.clone(), a());
    put_on_planet(&mut state, &system, &planet, infantry, &a(), 1);
    put(&mut state, &system, "infantry", &c(), 3);
    state.player_mut(&a()).unwrap().commodities = commodities;
    let (pick, _) = Pick::eager(&[answer]);
    let (_, dice) = armed_invade(
        &mut state,
        &system,
        &c(),
        &[10, 10, 10, 1],
        &mut Table::with_default(Box::new(pick)),
    );
    assert_eq!(on_planet(&state, &system, &planet, &a(), infantry), 0, "the Legionnaire fell");
    let hits_on = rolls(&dice, "a", "ground combat").first().and_then(|roll| roll.0);
    (state, hits_on)
}

#[test]
fn unit_mahact_infantry_1_crimson_legionnaire_i_gains_a_commodity_or_converts_one_when_destroyed() {
    let goods = |s: &GameState| s.player(&a()).unwrap().trade_goods;
    let before = goods(&mahact_game());
    let (gained, hits_on) = legionnaire_dies("mahact_infantry", "gain", 1);
    assert_eq!(hits_on, Some(8), "Crimson Legionnaire I hits on 8");
    assert_eq!(gained.player(&a()).unwrap().commodities, 2);
    assert_eq!(goods(&gained), before);
    let (converted, _) = legionnaire_dies("mahact_infantry", "convert", 1);
    assert_eq!(converted.player(&a()).unwrap().commodities, 0);
    assert_eq!(goods(&converted), before + 1);
}

#[test]
fn unit_mahact_infantry2_1_crimson_legionnaire_ii_pays_out_then_returns_to_the_home_system() {
    let home_infantry = |state: &GameState| {
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        state
            .system_state(&home)
            .planet_units
            .values()
            .flatten()
            .filter(|u| u.owner == a() && u.type_id.as_str().ends_with("infantry"))
            .count()
    };
    let (mut state, hits_on) = legionnaire_dies("mahact_infantry2", "gain", 1);
    assert_eq!(hits_on, Some(7), "Crimson Legionnaire II hits on 7");
    assert_eq!(state.player(&a()).unwrap().commodities, 2);
    let before = home_infantry(&state);
    // The start of the Mahact's next turn: the unit on the card lands on a home planet.
    state.phase = Phase::Action;
    state.active = Some(a());
    let (pick, _) = Pick::new(&[]);
    let mut game =
        Game::with_table(state, content(), Table::with_default(Box::new(pick))).with_sources(DEFAULT);
    assert_eq!(game.step().error, None);
    assert_eq!(home_infantry(&game.state), before + 1, "back home");
    // The first generation does not come back.
    let (mut state, _) = legionnaire_dies("mahact_infantry", "gain", 1);
    let before = home_infantry(&state);
    state.phase = Phase::Action;
    state.active = Some(a());
    let mut game = Game::with_table(state, content(), table(&[])).with_sources(DEFAULT);
    assert_eq!(game.step().error, None);
    assert_eq!(home_infantry(&game.state), before);
}
#[test]
fn unit_mahact_mech_1_starlancer_spends_a_pool_token_to_end_the_activators_turn() {
    let run = |answer: &str, token: bool, mech: bool| {
        let hub = planet_hub();
        let centre = SystemId::new(hub.centre.clone());
        let mut state = blank(mahact_game());
        if token {
            assert!(crate::factions::mahact::add_token(&mut state, &a(), &b()));
        }
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), a());
        if mech {
            put_on_planet(&mut state, &centre, &planet, "mahact_mech", &a(), 1);
        }
        put(&mut state, &sys(&hub.outer[0]), "cruiser", &b(), 1);
        let (mut game, seen) = action_game(state, "b", Some(hub.galaxy), &[TACTICAL_ACTION_ID, centre.as_str(), answer]);
        for _ in 0..5 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        (
            was_offered(&seen, "use"),
            offered_to(&seen, "b", "done_moving"),
            fleet_pool(&game.state).len(),
        )
    };
    assert_eq!(run("use", true, true), (true, false, 0), "b's turn ended before it could move; the token went back");
    assert_eq!(run("decline", true, true), (true, true, 1), "declined: b moves and the token stays");
    assert_eq!(run("use", false, true), (false, true, 0), "no token of b's in the pool: no offer");
    assert_eq!(run("use", true, false), (false, true, 1), "no mech in the system: no offer");
}

#[test]
fn unit_mahact_flagship_1_arvicon_rex_adds_two_unless_the_opponents_token_is_in_the_fleet_pool() {
    // The flagship hits on 5; a 3 only hits with the +2. With the opponent's token in the pool the
    // bonus is gone, so the first round misses and the fight takes another round.
    let rounds = |token: bool| {
        let mut state = mahact_game();
        if token {
            assert!(crate::factions::mahact::add_token(&mut state, &a(), &b()));
        }
        put(&mut state, &sys("19"), "mahact_flagship", &a(), 1);
        put(&mut state, &sys("19"), "cruiser", &b(), 1);
        let (outcome, dice) = space_fight(&mut state, &sys("19"), &[3, 3, 1], &mut table(&[]));
        assert_eq!(rolls(&dice, "a", "space combat")[0].1, 2, "two dice");
        outcome.rounds
    };
    assert_eq!(rounds(false), 1, "3 + 2 hits on 5");
    assert!(rounds(true) > 1, "no bonus: a 3 misses");
}

#[test]
fn unit_mahact_flagship_0_arvicon_rex_fights_with_two_dice_and_sustains() {
    let mut state = mahact_game();
    put(&mut state, &sys("19"), "mahact_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (outcome, _) = space_fight(&mut state, &sys("19"), &[1, 1, 10, 10, 10, 1], &mut table(&["~sustain|"]));
    assert_eq!(outcome.winner, Some(a()));
    let here = state.system_state(&sys("19"));
    let flagship = here
        .units
        .iter()
        .find(|unit| unit.type_id.as_str() == "mahact_flagship")
        .expect("it survived by sustaining");
    assert!(flagship.sustained_damage);
}

// -- Nomad ----------------------------------------------------------------------------------------

fn nomad_game() -> GameState {
    seated_all(&[("a", "nomad"), ("b", "sol"), ("c", "hacan")])
}

#[test]
fn tcs_1_the_nomad_readies_another_players_agent_the_moment_it_is_exhausted() {
    let window = "technology:nomad:tcs:AGENT_EXHAUSTED:after";
    let run = |tech: bool, accept: bool| {
        let hub = crate::fixtures::plain_hub();
        let mut state = nomad_game();
        if tech {
            super::give(&mut state, "a", "tcs");
        }
        state.phase = Phase::Action;
        state.active = Some(c());
        let answer = if accept { window } else { "decline" };
        let (pick, seen) = Pick::new(&["component|leader|hacanagent", "self", answer, "transact", "decline"]);
        let mut game = Game::with_table(state, content(), Table::with_default(Box::new(pick)))
            .with_sources(DEFAULT)
            .with_galaxy(hub.galaxy);
        for _ in 0..4 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
        }
        (
            was_offered(&seen, window),
            leader_status(&game.state, &c(), "hacanagent"),
            game.state
                .player(&a())
                .unwrap()
                .exhausted_technologies
                .contains(&ti4_model::id::TechnologyId::new("tcs")),
        )
    };
    assert_eq!(run(true, true), (true, Some(LeaderStatus::Readied), true), "readied, the card exhausted");
    assert_eq!(run(true, false), (true, Some(LeaderStatus::Exhausted), false), "declined");
    assert_eq!(run(false, true), (false, Some(LeaderStatus::Exhausted), false), "no technology");
}

#[test]
fn artuno_1_and_2_a_supply_gain_is_copied_onto_the_card_and_paid_out_when_it_readies() {
    let window = "leader:nomad:nomadagentartuno:TRADE_GOODS_GAINED:after";
    let run = |accept: bool| {
        let mut state = nomad_game();
        state.phase = Phase::Action;
        state.active = Some(a());
        let trade = state
            .unclaimed_strategy_cards
            .iter()
            .find(|card| card.as_str().contains("trade"))
            .cloned()
            .expect("the Trade card");
        state.player_mut(&a()).unwrap().strategy_cards = vec![trade];
        let goods = |s: &GameState| s.player(&a()).unwrap().trade_goods;
        let start = goods(&state);
        let answer = if accept { window } else { "decline" };
        let (pick, seen) = Pick::new(&["strategic", answer, "done"]);
        let mut game = Game::with_table(state, content(), Table::with_default(Box::new(pick))).with_sources(DEFAULT);
        step_until(&mut game, "STRATEGIC_ACTION_COMPLETE", 12);
        let gained = goods(&game.state) - start;
        let pile = crate::factions::nomad_agents::artuno_pile(&game.state, &a());
        let status = leader_status(&game.state, &a(), "nomadagentartuno");
        run_status_phase(&mut game.state);
        (
            was_offered(&seen, window),
            gained,
            pile,
            status,
            goods(&game.state) - start - gained,
            leader_status(&game.state, &a(), "nomadagentartuno"),
        )
    };
    assert_eq!(
        run(true),
        (true, 3, 3, Some(LeaderStatus::Exhausted), 3, Some(LeaderStatus::Readied)),
        "Trade pays 3; a copy sits on Artuno; the status phase readies it and pays the pile"
    );
    assert_eq!(
        run(false),
        (true, 3, 0, Some(LeaderStatus::Readied), 0, Some(LeaderStatus::Readied)),
        "declined: nothing copied, nothing paid later"
    );
}

#[test]
fn nomadagentmercer_1_moves_two_ground_forces_onto_a_planet_in_the_active_system_then_readies() {
    let window = "leader:nomad:nomadagentmercer:TACTICAL_ACTION_ENDED:after";
    let run = |accept: bool, agent: LeaderStatus| {
        let hub = planet_hub();
        let (here, there) = (SystemId::new(hub.centre.clone()), sys(&hub.outer[0]));
        let mut state = blank(nomad_game());
        let (target, far) = (first_planet(&here), first_planet(&there));
        state.system_mut(&here).set_control(target.clone(), b());
        state.system_mut(&there).set_control(far.clone(), b());
        put_on_planet(&mut state, &there, &far, "infantry", &b(), 2);
        set_leader(&mut state, &a(), "nomadagentmercer", agent);
        let take = format!("take|{there}|{far}|infantry|0");
        let mut rules = vec![TACTICAL_ACTION_ID, here.as_str(), "done_moving", window];
        if accept {
            rules.extend([take.as_str(), take.as_str()]);
        }
        let (mut game, seen) = action_game(state, "b", Some(hub.galaxy), &rules);
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.state.active.as_ref() != Some(&b()) {
                break;
            }
        }
        let outcome = (
            was_offered(&seen, window),
            on_planet(&game.state, &here, &target, &b(), "infantry"),
            on_planet(&game.state, &there, &far, &b(), "infantry"),
            leader_status(&game.state, &a(), "nomadagentmercer"),
        );
        run_status_phase(&mut game.state);
        assert_eq!(leader_status(&game.state, &a(), "nomadagentmercer"), Some(LeaderStatus::Readied));
        outcome
    };
    assert_eq!(
        run(true, LeaderStatus::Readied),
        (true, 2, 0, Some(LeaderStatus::Exhausted)),
        "two infantry moved from another planet onto the active system's planet"
    );
    assert_eq!(
        run(false, LeaderStatus::Readied),
        (true, 0, 2, Some(LeaderStatus::Readied)),
        "declined: nothing moved and the agent stays ready"
    );
    assert!(!run(true, LeaderStatus::Exhausted).0, "exhausted: not offered");
}

#[test]
fn nomadcommander_1_one_scored_secret_objective_unlocks_it() {
    let aliases = |kind| -> String {
        content()
            .records(kind)
            .iter()
            .find_map(|r| r.text("alias"))
            .expect("an objective")
            .to_owned()
    };
    let public = aliases(ti4_model::content_types::ContentType::PublicObjectives);
    let secret = aliases(ti4_model::content_types::ContentType::SecretObjectives);
    let mut state = nomad_game();
    let award = |state: &mut GameState, alias: &str| {
        crate::objectives::award(state, content(), DEFAULT, &a(), &ObjectiveId::new(alias))
            .expect("the objective scores");
    };
    award(&mut state, &public);
    assert_eq!(leader_status(&state, &a(), "nomadcommander"), Some(LeaderStatus::Locked), "a public objective");
    award(&mut state, &secret);
    assert_eq!(leader_status(&state, &a(), "nomadcommander"), Some(LeaderStatus::Unlocked), "a secret one");
    assert_eq!(leader_status(&state, &a(), "nomadhero"), Some(LeaderStatus::Locked), "two objectives only");
}

#[test]
fn nomadcommander_2_the_flagship_is_produced_without_spending_resources_once_unlocked() {
    let run = |status: LeaderStatus| {
        let hub = planet_hub();
        let centre = SystemId::new(hub.centre.clone());
        let mut state = blank(nomad_game());
        let planet = first_planet(&centre);
        state.system_mut(&centre).set_control(planet.clone(), a());
        put_on_planet(&mut state, &centre, &planet, "spacedock", &a(), 1);
        state.player_mut(&a()).unwrap().trade_goods = 0;
        state.exhausted_planets.insert(planet);
        set_leader(&mut state, &a(), "nomadcommander", status);
        let (mut game, seen) = action_game(
            state,
            "a",
            Some(hub.galaxy),
            &[TACTICAL_ACTION_ID, centre.as_str(), "done_moving", "build|nomad_flagship|1"],
        );
        for _ in 0..8 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE") {
                break;
            }
        }
        (
            was_offered(&seen, "build|nomad_flagship|1"),
            ships(&game.state, &centre, &a(), "nomad_flagship"),
            game.state.player(&a()).unwrap().trade_goods,
        )
    };
    assert_eq!(run(LeaderStatus::Unlocked), (true, 1, 0), "a free flagship");
    let (offered, built, _) = run(LeaderStatus::Locked);
    assert_eq!((offered, built), (false, 0), "locked: nothing to pay 8 with");
}

#[test]
fn nomadhero_1_scoring_a_third_objective_unlocks_the_hero() {
    hero_unlocks_by_scoring("nomad", "nomadhero");
}

#[test]
fn nomadhero_2_ahk_syl_siven_frees_the_flagship_and_its_cargo_from_command_tokens() {
    // The Nomad's token sits on the origin system, which pins every ship there. After the hero's
    // ACTION (and the other seats passing) the flagship and the fighter it carries may leave; the
    // carrier may not.
    let hero = "component|leader|nomadhero";
    let run = |status: LeaderStatus| {
        let hub = crate::fixtures::plain_hub();
        let (origin, target) = (sys(&hub.outer[0]), sys(&hub.outer[1]));
        let mut state = blank(nomad_game());
        put(&mut state, &origin, "nomad_flagship", &a(), 1);
        put(&mut state, &origin, "fighter", &a(), 1);
        put(&mut state, &origin, "carrier", &a(), 1);
        state.system_mut(&origin).command_tokens.insert(a());
        set_leader(&mut state, &a(), "nomadhero", status);
        let move_rule = format!("move|{origin}|");
        let mut rules = vec![TACTICAL_ACTION_ID, target.as_str()];
        let move_any = format!("~{move_rule}");
        rules.extend([move_any.as_str(), "~load|", "done_loading", "done_moving"]);
        if status == LeaderStatus::Unlocked {
            rules.insert(0, hero);
        }
        let (pick, seen) = Pick::new(&rules);
        let mut table = Table::with_default(Box::new(pick));
        table.seat(b(), Box::new(Pick::new(&[]).0));
        table.seat(c(), Box::new(Pick::new(&[]).0));
        let mut game = Game::with_table(
            {
                state.phase = Phase::Action;
                state.active = Some(a());
                state
            },
            content(),
            table,
        )
        .with_sources(DEFAULT)
        .with_galaxy(hub.galaxy);
        for _ in 0..40 {
            assert_eq!(game.step().error, None, "log: {:?}", game.events);
            if game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE") {
                break;
            }
        }
        (game, seen, origin, target)
    };
    let (game, seen, origin, target) = run(LeaderStatus::Unlocked);
    assert!(was_offered(&seen, hero));
    assert_eq!(leader_status(&game.state, &a(), "nomadhero"), Some(LeaderStatus::Purged));
    assert_eq!(ships(&game.state, &target, &a(), "nomad_flagship"), 1, "the flagship left the pinned system");
    assert_eq!(ships(&game.state, &target, &a(), "fighter"), 1, "with the fighter it carried");
    assert_eq!(ships(&game.state, &origin, &a(), "carrier"), 1, "only the flagship is freed");
    let (game, seen, _, target) = run(LeaderStatus::Locked);
    assert!(!was_offered(&seen, hero));
    assert_eq!(ships(&game.state, &target, &a(), "nomad_flagship"), 0, "pinned without the hero");
}

#[test]
fn unit_nomad_flagship_1_memoria_is_adjacent_to_systems_with_the_nomads_mechs() {
    // Move 1 is one step; the active system is two steps away. Memoria treats itself as adjacent to
    // a system holding a Nomad mech, so it gets there; a carrier (no Memoria) does not.
    let run = |mech_owner: Option<&str>, ship: &str| {
        let hub = planet_hub();
        let origin = sys(&hub.outer[0]);
        let target = sys(&hub.across(&hub.outer[0]));
        let mut state = blank(nomad_game());
        put(&mut state, &origin, ship, &a(), 1);
        if let Some(owner) = mech_owner {
            let planet = first_planet(&target);
            let kind = if owner == "a" { "nomad_mech" } else { "sol_mech" };
            put_on_planet(&mut state, &target, &planet, kind, &PlayerId::new(owner), 1);
        }
        let rule = format!("move|{origin}|0");
        let (game, seen) = tactical(state, "a", hub.galaxy, target.as_str(), &[rule.as_str(), "done_moving"]);
        (was_offered(&seen, &rule), ships(&game.state, &target, &a(), ship))
    };
    assert_eq!(run(Some("a"), "nomad_flagship"), (true, 1), "a Nomad mech in the active system");
    assert_eq!(run(None, "nomad_flagship"), (false, 0), "no mech, too far");
    assert_eq!(run(Some("b"), "nomad_flagship"), (false, 0), "another player's mech does not count");
    assert_eq!(run(Some("a"), "carrier"), (false, 0), "only the flagship has Memoria");
}

#[test]
fn unit_nomad_flagship_0_memoria_i_barrages_8_x3_fights_on_7_x2_and_sustains() {
    let mut state = nomad_game();
    put(&mut state, &sys("19"), "nomad_flagship", &a(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (outcome, dice) = space_fight(
        &mut state,
        &sys("19"),
        &[1, 1, 1, 1, 1, 10, 10, 10, 1],
        &mut table(&["~sustain|"]),
    );
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage"), [(Some(8), 3)]);
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(7), 2));
    assert_eq!(outcome.winner, Some(a()), "it sustained the cruiser's hit and won round two");
    let here = state.system_state(&sys("19"));
    let flagship = here.units.iter().find(|u| u.type_id.as_str() == "nomad_flagship");
    assert!(flagship.is_some_and(|unit| unit.sustained_damage));
}

#[test]
fn m2_1_to_3_memoria_ii_moves_2_barrages_5_x3_fights_on_5_x2_and_keeps_memoria() {
    let mut state = nomad_game();
    put(&mut state, &sys("19"), "nomad_flagship", &a(), 1);
    super::give(&mut state, "a", "m2");
    crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
    assert_eq!(ships(&state, &sys("19"), &a(), "nomad_flagship2"), 1, "upgraded in place");
    put(&mut state, &sys("19"), "fighter", &b(), 1);
    put(&mut state, &sys("19"), "cruiser", &b(), 1);
    let (_, dice) = space_fight(&mut state, &sys("19"), &[1; 8], &mut table(&[]));
    assert_eq!(rolls(&dice, "a", "anti-fighter barrage"), [(Some(5), 3)]);
    assert_eq!(rolls(&dice, "a", "space combat")[0], (Some(5), 2));
    // Move 2: two steps across the hub with no mech needed; and Memoria still shortens the trip.
    let run = |blocked: bool, mech: bool| {
        let hub = planet_hub();
        let origin = sys(&hub.outer[0]);
        let target = sys(&hub.across(&hub.outer[0]));
        let mut state = blank(nomad_game());
        super::give(&mut state, "a", "m2");
        put(&mut state, &origin, "nomad_flagship", &a(), 1);
        crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
        if blocked {
            // A rival ship in the centre: no way through, the long way round is three steps.
            put(&mut state, &sys(&hub.centre), "destroyer", &b(), 1);
        }
        if mech {
            let planet = first_planet(&sys(&hub.outer[2]));
            put_on_planet(&mut state, &sys(&hub.outer[2]), &planet, "nomad_mech", &a(), 1);
        }
        let rule = format!("move|{origin}|0");
        let (game, _) = tactical(state, "a", hub.galaxy, target.as_str(), &[rule.as_str(), "done_moving"]);
        ships(&game.state, &target, &a(), "nomad_flagship2")
    };
    assert_eq!(run(false, false), 1, "move 2 reaches the far side");
    assert_eq!(run(true, false), 0, "a blockaded centre stops it");
    assert_eq!(run(true, true), 1, "but the mech beside the target is adjacent by Memoria");
}

#[test]
fn unit_nomad_mech_1_quantum_manipulator_sustains_to_cancel_a_hit_against_the_fleet() {
    let run = |mech: Option<&str>| {
        let mut state = nomad_game();
        put(&mut state, &sys("19"), "cruiser", &a(), 1);
        if let Some(kind) = mech {
            put(&mut state, &sys("19"), kind, &a(), 1);
        }
        put(&mut state, &sys("19"), "cruiser", &b(), 1);
        let (outcome, _) = armed_fight(&mut state, &sys("19"), &[1, 10, 10, 1], &mut table(&["~sustain|"]));
        (outcome.winner, ships(&state, &sys("19"), &a(), "cruiser"))
    };
    assert_eq!(run(Some("nomad_mech")), (Some(a()), 1), "the mech took the hit instead of the cruiser");
    assert_eq!(run(None), (Some(b()), 0), "without it the cruiser dies");
    assert_eq!(run(Some("sol_mech")), (Some(b()), 0), "another faction's mech has no such ability");
}

#[test]
fn cavalry_2_it_is_never_offered_against_the_nomad_and_returns_when_the_combat_ends() {
    let play = "promissory:sol:cavalry:SPACE_COMBAT_STARTED:after";
    // Sol (b, holding The Cavalry) attacks `defender` with a destroyer.
    let fight = |defender: &str| {
        let mut state = nomad_game();
        state.active = Some(b());
        set_leader(&mut state, &a(), "nomadagentthundarian", LeaderStatus::Exhausted);
        crate::promissory::take(&mut state, content(), &b(), "cavalry:nomad");
        let defender = PlayerId::new(defender);
        put(&mut state, &sys("19"), "destroyer", &b(), 1);
        put(&mut state, &sys("19"), "destroyer", &defender, 1);
        let (pick, seen) = Pick::new(&[play]);
        let _ = armed_fight(&mut state, &sys("19"), &[1, 1, 1, 1, 10, 1], &mut Table::with_default(Box::new(pick)));
        (was_offered(&seen, play), state.promissory_notes.get("cavalry:nomad").cloned())
    };
    assert_eq!(fight("c"), (true, Some(a())), "played against Hacan; back with the Nomad after the combat");
    assert_eq!(fight("a"), (false, Some(b())), "not against the Nomad: Sol keeps it");
}

#[test]
fn nomadbt_1_thunders_paradox_exhausts_one_nomad_agent_to_ready_another_players() {
    let paradox = "breakthrough:nomad:nomadbt:TURN_BEGAN:after";
    let run = |breakthrough: bool| {
        let mut state = nomad_game();
        if breakthrough {
            state.player_mut(&a()).unwrap().breakthrough =
                Some(ti4_model::id::BreakthroughId::new("nomadbt"));
        }
        set_leader(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        state.phase = Phase::Action;
        state.active = Some(b());
        let rules: &[&str] = if breakthrough { &[paradox, "pair|0"] } else { &[] };
        let (pick, seen) = Pick::new(rules);
        let mut game = Game::with_table(state, content(), Table::with_default(Box::new(pick))).with_sources(DEFAULT);
        assert_eq!(game.step().error, None, "log: {:?}", game.events);
        let spent = ["nomadagentartuno", "nomadagentmercer", "nomadagentthundarian"]
            .iter()
            .filter(|agent| leader_status(&game.state, &a(), agent) == Some(LeaderStatus::Exhausted))
            .count();
        (was_offered(&seen, paradox), leader_status(&game.state, &b(), "solagent"), spent)
    };
    assert_eq!(run(true), (true, Some(LeaderStatus::Readied), 1), "one Nomad agent paid for Sol's");
    assert_eq!(run(false), (false, Some(LeaderStatus::Exhausted), 0), "no breakthrough");
}

#[test]
fn the_company_1_the_nomad_has_three_agents_and_all_of_them_ready_at_the_status_phase() {
    let mut state = nomad_game();
    let agents = ["nomadagentartuno", "nomadagentmercer", "nomadagentthundarian"];
    for agent in agents {
        assert_eq!(leader_status(&state, &a(), agent), Some(LeaderStatus::Readied), "{agent} dealt ready");
        set_leader(&mut state, &a(), agent, LeaderStatus::Exhausted);
    }
    // Another faction keeps its one agent.
    assert!(
        agents.iter().all(|agent| leader_status(&state, &b(), agent).is_none()),
        "the Nomad agents are the Nomad's only"
    );
    run_status_phase(&mut state);
    for agent in agents {
        assert_eq!(leader_status(&state, &a(), agent), Some(LeaderStatus::Readied), "{agent} readied");
    }
}
