//! Lane B (package KP): the native turn-flow jobs (`ti4_engine_core::driver::turn`) run through `live::live_step` against the string
//! bodies they replace (`advance_turn_inner`, `finish_action`, `step_phase`, the `turn_prelude` block of `step_flow`), question by
//! question: the questions as rendered (prompt, options, display facts, context), the answers, the events and the position.
//!
//! Each scenario builds ONE position, runs the string body on a copy with a scripted table that records what it was asked, and the
//! native job on a second copy with the same answers. The harness fails on any difference in the questions asked, the plain and typed
//! events pushed, or the position after.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use serde_json::Value;
use ti4_content::ContentStore;
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::driver::turn::open_job;
use ti4_engine_core::state::{SliceSet, TurnJob};
use ti4_model::content_types::DEFAULT;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

type Seen = Rc<RefCell<Vec<Choice>>>;

/// A table that answers with the next scripted option id and records every question it was asked. A question with no scripted answer
/// stops the string body (an error report), which is how the harness cuts the block off where the string's step goes on to the menu.
struct Script {
    ids: VecDeque<String>,
    seen: Seen,
}

impl Decider for Script {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        let Some(id) = self.ids.pop_front() else { return Err(IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() }) };
        choice.options.iter().find(|o| o.id == id).cloned().ok_or_else(|| panic!("the script answers {id:?}, the question {:?} offers {:?}", choice.prompt, choice.ids()))
    }
}

pub fn pid(s: &str) -> PlayerId {
    PlayerId::new(s)
}

/// What a run left.
struct Run {
    questions: Vec<Choice>,
    events: Vec<String>,
    state: Value,
}

/// The string body on a copy of `state`, answered by `ids`.
fn string_run(state: &GameState, ids: &[&str], body: impl FnOnce(&mut super::Game<'static>)) -> Run {
    let _serial = crate::dual::tests::serial();
    let seen: Seen = Rc::default();
    let table = Table::with_default(Box::new(Script { ids: ids.iter().map(|s| (*s).to_owned()).collect(), seen: seen.clone() }));
    let mut game = super::Game::with_table(state.clone(), ContentStore::embedded(), table);
    body(&mut game);
    let questions = seen.borrow().clone();
    Run { questions, events: game.events.clone(), state: serde_json::to_value(&game.state).expect("serializes") }
}

/// The native job on a copy of `state`: open it (when `job` is given), then one `live_step` of `arm` per string step until the window
/// ends. Every answer is consumed in order.
fn native_run(state: &GameState, arm: Arm, job: Option<TurnJob>, ids: &[&str]) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = Render::new(cx.owners.ids().to_vec());
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    if let Some(job) = job {
        open_job(&mut core, job);
    }
    let mut asker = Scripted::new(ids.iter().copied());
    let mut questions = Vec::new();
    loop {
        let step = live::live_step(&mut core, arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native arm stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    questions.extend(asker.seen.clone());
    assert_eq!(asker.left(), 0, "every scripted answer was taken");
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    Run { questions, events, state: serde_json::to_value(&after).expect("serializes") }
}

fn shape(c: &Choice) -> Value {
    serde_json::json!({
        "player": c.player.as_str(),
        "prompt": c.prompt,
        "options": c.options.iter().map(|o| serde_json::json!([o.id, o.kind, o.label])).collect::<Vec<_>>(),
        "details": c.details,
        "context": c.context.as_ref().map(|x| serde_json::json!([x.actor.as_str(), format!("{:?}", x.source), x.subtype])),
    })
}

/// Both runs agree on the questions, the events and the position.
fn assert_same(string: &Run, native: &Run) {
    assert_eq!(string.questions.iter().map(shape).collect::<Vec<_>>(), native.questions.iter().map(shape).collect::<Vec<_>>(), "the questions");
    assert_eq!(string.events, native.events, "the events");
    let diffs = crate::dual::diff_json(&string.state, &native.state, 12);
    assert!(diffs.is_empty(), "the position differs: {diffs:?}");
}

/// The position a game starts from: what the constructor does to a state (the baselines `announce_gains` keeps) is part of it for both
/// sides.
fn based(state: GameState) -> GameState {
    super::Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption))).state
}

/// A table of `seats` in the action phase, seat `a` active, its turn prepared.
fn action_table(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid(seats[0].0));
    based(state)
}

const THREE: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

#[test]
fn the_end_of_a_plain_turn_passes_on_like_the_string() {
    let state = action_table(&THREE);
    let string = string_run(&state, &[], |g| g.advance_turn().expect("the turn passes"));
    let native = native_run(&state, Arm::Ask, Some(TurnJob::End), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TURN_PASSED"), "{:?}", string.events);
}

fn end_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let string = string_run(state, ids, |g| g.advance_turn().expect("the turn passes"));
    let native = native_run(state, Arm::Ask, Some(TurnJob::End), ids);
    (string, native)
}

#[test]
fn an_additional_action_retains_the_turn_like_the_string() {
    let mut state = action_table(&THREE);
    state.transient_flags.set(ti4_model::state::TransientFlags::ADDITIONAL_ACTION);
    let (string, native) = end_both(&state, &[]);
    assert_same(&string, &native);
    assert_eq!(string.events, ["TURN_RETAINED"]);
}

#[test]
fn crisis_skips_and_puppets_hands_the_turn_back_like_the_string() {
    let mut state = action_table(&THREE);
    state.transient_flags.set(ti4_model::state::TransientFlags::SKIP_NEXT_TURN);
    let (string, native) = end_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("TURN_SKIPPED:")), "{:?}", string.events);
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().passed = true;
    state.transient_flags.set(ti4_model::state::TransientFlags::PUPPET_ACTION);
    let (string, native) = end_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("TURN_PUPPET:")), "{:?}", string.events);
}

#[test]
fn the_last_seat_to_pass_still_ends_its_turn_like_the_string() {
    let mut state = action_table(&THREE);
    for p in ["a", "b", "c"] {
        state.player_mut(&pid(p)).unwrap().passed = true;
    }
    let (string, native) = end_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TURN_PASSED"));
}

fn minister_table() -> GameState {
    let mut state = action_table(&THREE);
    state.laws.insert("minister_war".to_owned(), "a".to_owned());
    let (system, _) = crate::fixtures::a_placed_planet();
    state.system_mut(&system).command_tokens.insert(pid("a"));
    state
}

#[test]
fn the_minister_of_war_is_offered_the_discard_and_the_token_like_the_string() {
    let state = minister_table();
    let (system, _) = crate::fixtures::a_placed_planet();
    let (string, native) = end_both(&state, &["discard", system.as_str()]);
    assert_same(&string, &native);
    assert_eq!(string.events, ["TURN_RETAINED"]);
    assert_eq!(string.questions.len(), 2, "the offer, then the token");
}

#[test]
fn declining_the_minister_of_war_passes_the_turn_like_the_string() {
    let (string, native) = end_both(&minister_table(), &["decline"]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TURN_PASSED"));
}

#[test]
fn predictive_intelligence_restacks_one_token_per_answer_like_the_string() {
    let mut state = action_table(&THREE);
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("pi"));
    let (string, native) = end_both(&state, &["tactic|fleet", "fleet|strategy", "done"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 3);
    let exhausted = string.state["players"][0]["exhausted_technologies"].to_string();
    assert!(exhausted.contains("pi"), "the card is exhausted once a token moved: {exhausted}");
}
