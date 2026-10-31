//! Lane C (package KS): the native status phase (`ti4_engine_core::driver::windows::{status, scoring, status_bookkeeping,
//! status_finish}`) run through `live::live_step` against the string steps it replaces (`step_status`, `step_scoring`,
//! `step_event_scoring`, `begin_status_bookkeeping`, `step_token_gain`, `finish_status_phase`): the questions as rendered, the events, the
//! position and the dice streams, from the opening of the status phase to its last event.
//!
//! The string side plays `Game::step` until the phase's last event, answering the scripted ids and then the first option; the native side
//! takes the answers the string was given and runs one `live_step` per step the position's arm names.

use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_engine_core::driver::session::{step_arm, Arm, Facts};
use ti4_engine_core::state::SliceSet;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{ActionCardId, ObjectiveId, PlayerId, RelicId, StrategyCardId, TechnologyId};
use ti4_model::state::{GameState, Phase};

use super::turn_live_tests::{assert_same, based, pid, string_run_first, Run};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// The string's status phase on a copy of `state`: `Game::step` until `STATUS_PHASE_RESOLVED` (or the game ends). The answers it took.
fn string_status(state: &GameState, ids: &[&str]) -> (Run, Vec<String>) {
    string_run_first(state, ids, |g| {
        for _ in 0..200 {
            let _ = g.step();
            if g.events.iter().any(|e| e == "STATUS_PHASE_RESOLVED") || g.state.finished {
                break;
            }
        }
    })
}

/// The native status phase on a copy of `state`: the reconcile hooks and one `live_step` of the arm the position names, while the arm is one
/// of the status phase's.
fn native_status(state: &GameState, ids: &[String]) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = Render::new(cx.owners.ids().to_vec());
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    let mut asker = Scripted::new(ids.iter().cloned());
    for _ in 0..200 {
        loop {
            let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
            if step.finished {
                break;
            }
        }
        if core.meta().finished {
            break;
        }
        let arm = step_arm(&Facts::of_core(&core, false), false);
        if !matches!(arm, Arm::Status | Arm::Scoring | Arm::EventScoring | Arm::Tokens) {
            break;
        }
        live::live_step(&mut core, arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native {arm:?} stopped: {e:?}"));
    }
    let questions = asker.seen.clone();
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = super::turn_live_tests::without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

/// Both sides on one position.
fn both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let (string, taken) = string_status(state, ids);
    let native = native_status(state, &taken);
    (string, native)
}

/// A table in the status phase, every seat holding a strategy card (the first, second, ... of the mat).
fn status_table(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = Phase::Status;
    state.active = None;
    let cards = ["pok1leadership", "pok2diplomacy", "pok3politics", "pok4construction"];
    for (i, (p, _)) in seats.iter().enumerate() {
        state.player_mut(&pid(p)).unwrap().strategy_cards.push(StrategyCardId::new(cards[i]));
    }
    // A public objective to reveal.
    assert!(!state.objective_deck.is_empty(), "the dealt table has an objective deck");
    based(state)
}

const FOUR: [(&str, &str); 4] = [("a", "sol"), ("b", "hacan"), ("c", "yssaril"), ("d", "letnev")];

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|e| e == event)
}

#[test]
fn a_plain_status_phase_runs_like_the_string() {
    let (string, native) = both(&status_table(&FOUR), &[]);
    assert_same(&string, &native);
    for e in ["STATUS_SCORING_BEGAN", "STATUS_BOOKKEEPING_RESOLVED", "COMMAND_TOKENS_GAINED", "STATUS_PHASE_RESOLVED", "STATUS_PHASE_ENDED"] {
        assert!(has(&string, e), "{e} in {:?}", string.events);
    }
    assert!(string.questions.iter().any(|q| q.prompt == "gain a command token into which pool"));
    assert!(string.questions.iter().any(|q| q.prompt == "redistribute your command tokens"));
}

#[test]
fn yssarils_scheming_draws_an_extra_card_and_discards_one_like_the_string() {
    let (string, native) = both(&status_table(&FOUR), &[]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.starts_with("Scheming")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

#[test]
fn the_hand_limit_follows_the_draw_like_the_string() {
    let mut state = status_table(&FOUR);
    for i in 0..7 {
        let card = state.action_card_deck.remove(0);
        let _ = i;
        state.player_mut(&pid("b")).unwrap().action_cards.push(card);
    }
    let state = based(state);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.starts_with("over the hand limit")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

#[test]
fn an_empty_objective_deck_ends_the_game_at_the_reveal_like_the_string() {
    let mut state = status_table(&FOUR);
    state.objective_deck.clear();
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
    assert!(has(&string, "GAME_FINISHED"));
    assert!(!has(&string, "STATUS_PHASE_RESOLVED"));
}

#[test]
fn a_seat_that_keeps_its_strategy_card_and_a_crown_holder_finish_the_phase_like_the_string() {
    let mut state = status_table(&FOUR);
    state.player_mut(&pid("a")).unwrap().stability = true;
    state.player_mut(&pid("b")).unwrap().passed = true;
    state.player_mut(&pid("c")).unwrap().exhausted_technologies.insert(TechnologyId::new("pa"));
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
}

#[test]
fn an_objective_a_seat_can_score_is_offered_and_scored_like_the_string() {
    let mut state = status_table(&FOUR);
    state.revealed_objectives.push(ObjectiveId::new("engineer_marvel"));
    let (system, _) = crate::fixtures::a_placed_planet();
    crate::fixtures::put(&mut state, &system, "sol_flagship", &pid("a"), 1);
    let (string, native) = both(&based(state), &["engineer_marvel"]);
    assert_same(&string, &native);
    assert!(has(&string, "OBJECTIVE_SCORED:engineer_marvel"), "{:?}", string.events);
}

#[test]
fn the_third_objective_unlocks_the_hero_before_the_bookkeeping_like_the_string() {
    let mut state = status_table(&FOUR);
    state.revealed_objectives.push(ObjectiveId::new("engineer_marvel"));
    let (system, _) = crate::fixtures::a_placed_planet();
    crate::fixtures::put(&mut state, &system, "sol_flagship", &pid("a"), 1);
    for o in ["expand_borders", "corner"] {
        state.scored_objectives.entry(pid("a")).or_default().insert(ObjectiveId::new(o));
    }
    let (string, native) = both(&based(state), &["engineer_marvel"]);
    assert_same(&string, &native);
}

#[test]
fn relics_and_the_neuraloop_are_handled_like_the_string() {
    let mut state = status_table(&FOUR);
    state.player_mut(&pid("d")).unwrap().relics.push(RelicId::new("neuraloop"));
    // Seat c (Yssaril) is asked its Scheming discard first (the first card), then the Neuraloop's holder which relic to purge.
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
    assert!(has(&string, "STATUS_PHASE_RESOLVED"));
}

#[test]
fn the_sol_flagship_places_its_infantry_at_the_end_of_the_phase_like_the_string() {
    let mut state = status_table(&FOUR);
    let (system, _) = crate::fixtures::a_placed_planet();
    crate::fixtures::put(&mut state, &system, "sol_flagship", &pid("a"), 1);
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("GENESIS_INFANTRY:a:")), "{:?}", string.events);
}

#[allow(dead_code)]
fn _unused(_: &PlayerId, _: &ActionCardId) {}
