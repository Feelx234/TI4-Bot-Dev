//! Package A4, the mutation half: the objective writes and questions run on the core under
//! `objectives=dual` and agree with the string side (`objectives_write`, `rules::objectives`,
//! `rules::objective_windows`). A bail is silent, so each test probes the core function itself and requires
//! it to answer.

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SecretObjectiveId, SystemId};
use ti4_model::state::GameState;

use crate::choice::{Capturing, Scripted, Table};
use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::game::Game;

fn a() -> PlayerId {
    PlayerId::new("a")
}

fn clean(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_game(seats, POK);
    state.promissory_notes.clear();
    state.promissory_faceup.clear();
    state
}

fn mask() -> AreaMask {
    AreaMask::STRING.with(Area::Objectives, Backend::Dual)
}

fn compared(label: &str) -> (u64, u64) {
    report().windows.get(label).map_or((0, 0), |w| (w.compared, w.bailed))
}

fn table(answers: &[&str]) -> Table {
    let (decider, _seen) = Capturing::new(Box::new(Scripted::new(answers.iter().map(|s| (*s).to_owned()))));
    Table::with_default(Box::new(decider))
}

/// Ids of public objectives worth `points`, in the order of the generated table, that are not bought.
fn publics(points: u8, count: usize) -> Vec<String> {
    use ti4_engine_core::generated::PublicObjective;
    use ti4_engine_core::query::objectives as q;
    PublicObjective::ALL
        .iter()
        .filter(|o| q::points_of(**o) == points && q::cost_of(**o).is_none())
        .take(count)
        .map(|o| crate::generated::IdStr::as_str(*o).to_owned())
        .collect()
}

fn give(state: &mut GameState, planet: &str, player: &str) {
    let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK);
    let system = catalogue.get(planet).and_then(ti4_content::galaxy::Planet::system_id).unwrap_or("18");
    state.system_mut(&SystemId::new(system)).set_control(PlanetId::new(planet), PlayerId::new(player));
}

fn non_home_planets(count: usize) -> Vec<String> {
    ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
        .iter()
        .filter(|(_, planet)| planet.homeworld_of().is_none() && !planet.is_placed_during_play())
        .map(|(id, _)| (*id).to_owned())
        .take(count)
        .collect()
}

/// Four pds on a planet: the secret `eap` is satisfied.
fn four_pds(state: &mut GameState) {
    let (system, planet) = crate::fixtures::a_placed_planet();
    for _ in 0..4 {
        state
            .system_mut(&system)
            .planet_units
            .entry(planet.clone())
            .or_default()
            .push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("pds"), a()));
    }
}

#[test]
fn victory_points_move_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 3, "objective"), 3);
        // A gain stops at the target, the row carries the change that happened.
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 20, "custodians"), 7);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 1, "imperial_primary"), 0);
        // A loss stops at zero.
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), -30, "styx"), -10);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 2, "agenda:minister_of_policy"), 2);
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &PlayerId::new("nobody"), 1, "objective"), 0);
        assert_eq!(state.vp_ledger.len(), 5);
        // A reason the ledger does not type stays on the string write (bail), with the same result.
        assert_eq!(crate::objectives::adjust_victory_points(&mut state, &a(), 1, "a made up reason"), 1);
        // The core half answers for a typed reason.
        let mut probe = clean(&[("a", "letnev"), ("b", "sol")]);
        assert_eq!(crate::objectives_write::adjust(&mut probe, &a(), 1, "objective"), Some(1));
        assert_eq!(crate::objectives_write::adjust(&mut probe, &a(), 1, "a made up reason"), None);
    });
    let r = report();
    assert!(r.total_calls() >= 7, "the core half ran: {r:?}");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let _ = take_report();
}

#[test]
fn objectives_are_revealed_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        let one = publics(1, 3);
        let two = publics(2, 2);
        // Stage I on top: the second reveal of a stage II skips them.
        state.objective_deck = vec![&one[0], &one[1], &two[0], &one[2], &two[1]].into_iter().map(|s| ObjectiveId::new(s.as_str())).collect();
        state.revealed_objectives.clear();
        assert_eq!(crate::objectives::reveal_top(&mut state), Some(ObjectiveId::new(one[0].as_str())));
        let content = ContentStore::embedded();
        assert_eq!(crate::objectives::reveal_stage(&mut state, content, 2), Some(ObjectiveId::new(two[0].as_str())));
        assert_eq!(crate::objectives::reveal_stage(&mut state, content, 2), Some(ObjectiveId::new(two[1].as_str())));
        assert_eq!(crate::objectives::reveal_stage(&mut state, content, 2), None);
        assert_eq!(crate::objectives::reveal_top(&mut state), Some(ObjectiveId::new(one[1].as_str())));
        assert_eq!(state.objective_deck, vec![ObjectiveId::new(one[2].as_str())]);
        assert_eq!(state.revealed_objectives.len(), 4);
        // An empty deck reveals nothing.
        state.objective_deck.clear();
        assert_eq!(crate::objectives::reveal_top(&mut state), None);
        // The core half answers on a plain position, and bails on a deck reserve (a turn redo).
        let mut probe = clean(&[("a", "letnev"), ("b", "sol")]);
        probe.objective_deck = vec![ObjectiveId::new(one[0].as_str())];
        assert_eq!(crate::objectives_write::reveal_top(&mut probe), Some(Some(ObjectiveId::new(one[0].as_str()))));
        assert_eq!(crate::objectives_write::reveal_stage(&mut probe, 1), Some(None));
    });
    let r = report();
    assert!(r.total_calls() >= 6, "the core half ran: {r:?}");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let _ = take_report();
}

#[test]
fn a_secret_drawn_over_the_hand_limit_is_returned_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        let content = ContentStore::embedded();
        let secret = |s: &str| SecretObjectiveId::new(s);
        // Three in hand and one drawn: the fourth is asked about.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap"), secret("fwm"), secret("gamf")];
        state.secret_deck = vec![secret("ctr"), secret("faa")];
        let mut asked = table(&["fwm"]);
        assert_eq!(crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap(), Some(secret("ctr")));
        assert_eq!(state.player(&a()).unwrap().secret_objectives, vec![secret("eap"), secret("gamf"), secret("ctr")]);
        assert_eq!(state.secret_deck, vec![secret("faa"), secret("fwm")], "the returned secret goes to the bottom");

        // Scored secrets count towards the limit: with two scored, one in hand and one drawn, one is asked.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap")];
        state.record_score(&a(), ObjectiveId::new("fwm"));
        state.record_score(&a(), ObjectiveId::new("gamf"));
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&["eap"]);
        crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap();
        assert_eq!(state.player(&a()).unwrap().secret_objectives, vec![secret("ctr")]);

        // A lone unscored secret over the limit goes back without a question.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives.clear();
        for scored in ["fwm", "gamf", "faa"] {
            state.record_score(&a(), ObjectiveId::new(scored));
        }
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&[]);
        crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap();
        assert!(state.player(&a()).unwrap().secret_objectives.is_empty());
        assert_eq!(state.secret_deck, vec![secret("ctr")]);

        // Every secret scored: a further draw is simply stuck over the limit? (nothing to return, no question)
        // An empty deck draws nothing.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.secret_deck.clear();
        let mut asked = table(&[]);
        assert_eq!(crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap(), None);

        // The Obsidian lifts the limit by one.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap"), secret("fwm"), secret("gamf")];
        state.player_mut(&a()).unwrap().relics = vec![ti4_model::id::RelicId::new("obsidian")];
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&[]);
        crate::secrets::draw(&mut state, content, &mut asked, &a()).unwrap();
        assert_eq!(state.player(&a()).unwrap().secret_objectives.len(), 4);

        // An answer that is no option leaves the question unanswered: both sides stop at the same place.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().secret_objectives = vec![secret("eap"), secret("fwm"), secret("gamf")];
        state.secret_deck = vec![secret("ctr")];
        let mut asked = table(&["not an option"]);
        assert!(crate::secrets::draw(&mut state, content, &mut asked, &a()).is_err());
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let (done, bailed) = compared("secrets::enforce_hand_limit");
    assert!(done >= 5, "the core windows ran: {r:?}");
    assert_eq!(bailed, 0, "{r:?}");
    assert!(r.total_calls() >= 6, "the core writes ran: {r:?}");
    let _ = take_report();
}

/// A game in the status phase, for `player`'s score.
fn status_game(state: GameState, answers: &[&str]) -> Game<'static> {
    let mut state = state;
    state.phase = ti4_model::state::Phase::Status;
    Game::with_table(state, ContentStore::embedded(), table(answers))
}

fn run_status(game: &mut Game<'_>) {
    let mut guard = 0;
    while game.state.phase == ti4_model::state::Phase::Status && guard < 30 {
        assert_eq!(game.step().error, None);
        guard += 1;
    }
}

#[test]
fn the_status_scoring_window_runs_in_lockstep_with_the_core() {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    with_mask(mask(), || {
        // A public (six non-home planets) and a secret (four pds): both asked, both scored, in one window.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
        four_pds(&mut state);
        let mut game = status_game(state, &["expand_borders", "eap"]);
        run_status(&mut game);
        let seat = game.state.player(&a()).unwrap();
        assert_eq!(seat.victory_points, 2);
        assert!(seat.secret_objectives.is_empty());
        assert_eq!(game.state.scored_by(&a()).len(), 2);
        assert_eq!(game.state.vp_ledger.iter().filter(|(p, _, _)| *p == a()).count(), 2);

        // Declining scores nothing.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        let mut game = status_game(state, &["decline"]);
        run_status(&mut game);
        assert_eq!(game.state.player(&a()).unwrap().victory_points, 0);

        // A secret made public (Classified Document Leaks) is scored as a public objective: it stays where it is.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("eap")];
        four_pds(&mut state);
        let mut game = status_game(state, &["eap"]);
        run_status(&mut game);
        assert_eq!(game.state.player(&a()).unwrap().victory_points, 1);
        assert!(game.state.revealed_objectives.contains(&ObjectiveId::new("eap")));

        // The score that reaches the target ends the game.
        let mut state = clean(&[("a", "letnev"), ("b", "sol")]);
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        for planet in non_home_planets(6) {
            give(&mut state, &planet, "a");
        }
        state.player_mut(&a()).unwrap().victory_points = 9;
        let mut game = status_game(state, &["expand_borders"]);
        let mut guard = 0;
        while !game.state.finished && guard < 30 {
            assert_eq!(game.step().error, None);
            guard += 1;
        }
        assert!(game.state.finished);
        assert_eq!(game.state.player(&a()).unwrap().victory_points, 10);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let (done, bailed) = compared("scoring::step");
    assert!(done >= 5, "the core windows ran: {r:?}");
    assert_eq!(bailed, 0, "{r:?}");
    let _ = take_report();
}
