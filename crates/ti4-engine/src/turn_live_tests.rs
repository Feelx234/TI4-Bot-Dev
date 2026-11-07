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

pub(super) type Seen = Rc<RefCell<Vec<Choice>>>;

/// A table that answers with the next scripted option id and records every question it was asked. A question with no scripted answer
/// stops the string body (an error report), which is how the harness cuts the block off where the string's step goes on to the menu.
pub(super) struct Script {
    ids: VecDeque<String>,
    seen: Seen,
    /// Take the first option when the script has run out (and record it in `taken`), instead of stopping the body.
    first_after: Option<Rc<RefCell<Vec<String>>>>,
}

impl Decider for Script {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        let Some(id) = self.ids.pop_front().or_else(|| {
            let taken = self.first_after.as_ref()?;
            let id = choice.options.first()?.id.clone();
            taken.borrow_mut().push(id.clone());
            Some(id)
        }) else {
            return Err(IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() });
        };
        choice.answer_for(&id).ok_or_else(|| panic!("the script answers {id:?}, the question {:?} offers {:?}", choice.prompt, choice.ids()))
    }
}

pub fn pid(s: &str) -> PlayerId {
    PlayerId::new(s)
}

/// What a run left.
pub(super) struct Run {
    pub(super) questions: Vec<Choice>,
    pub(super) events: Vec<String>,
    pub(super) state: Value,
    pub(super) rng: crate::rng::RngPositions,
    /// The events effects staged for a later announcement (the string keeps them as `private:#staged:event:*` marks, the core in
    /// `Pending::staged`), by name, in order.
    pub(super) staged: Vec<String>,
}

/// Take the staged-event marks out of a string position: their names, and the position without them.
pub(super) fn without_staged(mut state: Value) -> (Value, Vec<String>) {
    let mut staged = Vec::new();
    if let Some(marks) = state.get_mut("faction_marks").and_then(Value::as_object_mut) {
        let keys: Vec<String> = marks.keys().filter(|k| k.starts_with("private:#staged:event:")).cloned().collect();
        for key in keys {
            let text = marks.remove(&key).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            let event: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            staged.push(event.get("type").and_then(Value::as_str).unwrap_or("?").to_owned());
        }
    }
    (state, staged)
}

/// The string body on a copy of `state`, answered by `ids`.
pub(super) fn string_run(state: &GameState, ids: &[&str], body: impl FnOnce(&mut super::Game<'static>)) -> Run {
    let _serial = crate::dual::tests::serial();
    let seen: Seen = Rc::default();
    let table = Table::with_default(Box::new(Script { ids: ids.iter().map(|s| (*s).to_owned()).collect(), seen: seen.clone(), first_after: None }));
    let mut game = super::Game::with_table(state.clone(), ContentStore::embedded(), table);
    body(&mut game);
    let questions = seen.borrow().clone();
    crate::multiask_dual::remember_string_questions(questions.clone());
    let (state, staged) = without_staged(serde_json::to_value(&game.state).expect("serializes"));
    Run { questions, events: game.events.clone(), state, rng: game.rng_positions(), staged }
}

/// [`string_run`] that takes the first option of every question after the scripted ones: the answers it took, for the native run.
pub(super) fn string_run_first(state: &GameState, ids: &[&str], body: impl FnOnce(&mut super::Game<'static>)) -> (Run, Vec<String>) {
    let _serial = crate::dual::tests::serial();
    let seen: Seen = Rc::default();
    let taken: Rc<RefCell<Vec<String>>> = Rc::default();
    let table = Table::with_default(Box::new(Script { ids: ids.iter().map(|s| (*s).to_owned()).collect(), seen: seen.clone(), first_after: Some(taken.clone()) }));
    let mut game = super::Game::with_table(state.clone(), ContentStore::embedded(), table);
    body(&mut game);
    let questions = seen.borrow().clone();
    crate::multiask_dual::remember_string_questions(questions.clone());
    let (state, staged) = without_staged(serde_json::to_value(&game.state).expect("serializes"));
    let mut all: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
    all.extend(taken.borrow().iter().cloned());
    (Run { questions, events: game.events.clone(), state, rng: game.rng_positions(), staged }, all)
}

/// The native job on a copy of `state`: open it (when `job` is given), then one `live_step` of `arm` per string step until the window
/// ends. Every answer is consumed in order.
pub(super) fn native_run(state: &GameState, arm: Arm, job: Option<TurnJob>, ids: &[&str]) -> Run {
    native_steps(state, &[(arm, job)], false, ids)
}

/// The native steps in order (`reconcile` first when asked, as a step does), one asker for all of them.
pub(super) fn native_steps(state: &GameState, steps: &[(Arm, Option<TurnJob>)], reconcile: bool, ids: &[&str]) -> Run {
    let marked: Vec<(bool, Arm, Option<TurnJob>)> = steps.iter().enumerate().map(|(i, (arm, job))| (reconcile && i == 0, *arm, *job)).collect();
    native_marked(state, &marked, ids)
}

/// Like [`native_steps`], each step saying whether the reconcile hooks run before it (a string step starts with them).
pub(super) fn native_marked(state: &GameState, steps: &[(bool, Arm, Option<TurnJob>)], ids: &[&str]) -> Run {
    let steps: Vec<(bool, Option<Arm>, Option<TurnJob>)> = steps.iter().map(|(r, a, j)| (*r, Some(*a), *j)).collect();
    native_marked_arms(state, &steps, ids)
}

/// [`native_marked`] where a step may be the reconcile alone (`None`).
fn native_marked_arms(state: &GameState, steps: &[(bool, Option<Arm>, Option<TurnJob>)], ids: &[&str]) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    // (The options that name a seat by faction -- Politics, Trade -- need the faction texts.)
    let factions = cx.owners.ids().iter().map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned())).collect();
    let render = Render::new(cx.owners.ids().to_vec()).with_factions(factions);
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    let mut scripted = Scripted::new(ids.iter().copied());
    // The table's rule for the draft: the last card is not a choice and is never put (nor journaled).
    let mut asker = live::LoneCard(&mut scripted);
    for (reconcile, arm, job) in steps {
        if *reconcile {
            loop {
                let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
                if step.finished {
                    break;
                }
            }
        }
        if let Some(job) = job {
            open_job(&mut core, *job);
        }
        let Some(arm) = arm else { continue };
        loop {
            let step = live::live_step(&mut core, *arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native arm stopped: {e:?}"));
            if step.finished {
                break;
            }
        }
    }
    let questions = scripted.seen.clone();
    assert_eq!(scripted.left(), 0, "every scripted answer was taken");
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

pub(super) fn shape(c: &Choice) -> Value {
    serde_json::json!({
        "player": c.player.as_str(),
        "prompt": c.prompt,
        "options": c.options.iter().map(|o| serde_json::json!([o.id, o.kind, o.label])).collect::<Vec<_>>(),
        "details": c.details,
        "context": c.context.as_ref().map(|x| serde_json::json!([x.actor.as_str(), format!("{:?}", x.source), x.subtype])),
    })
}

/// Both runs agree on the questions, the events and the position.
pub(super) fn assert_same(string: &Run, native: &Run) {
    assert_eq!(string.questions.iter().map(shape).collect::<Vec<_>>(), native.questions.iter().map(shape).collect::<Vec<_>>(), "the questions");
    assert_eq!(string.events, native.events, "the events");
    assert_eq!(string.staged, native.staged, "the events left staged for the next step");
    let diffs = crate::dual::diff_json(&string.state, &native.state, 12);
    assert!(diffs.is_empty(), "the position differs: {diffs:?}");
    assert_eq!(format!("{:?}", string.rng), format!("{:?}", native.rng), "the dice streams stand at the same place");
}

/// The position a game starts from: what the constructor does to a state (the baselines `announce_gains` keeps) is part of it for both
/// sides.
pub(super) fn based(state: GameState) -> GameState {
    super::Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption))).state
}

/// A table of `seats` in the action phase, seat `a` active, its turn prepared.
pub(super) fn action_table(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid(seats[0].0));
    based(state)
}

pub(super) const THREE: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

#[test]
fn the_end_of_a_plain_turn_passes_on_like_the_string() {
    let state = action_table(&THREE);
    let string = string_run(&state, &[], |g| g.advance_turn().expect("the turn passes"));
    let native = native_run(&state, Arm::Ask, Some(TurnJob::End), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TURN_PASSED"), "{:?}", string.events);
}

pub(super) fn end_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
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

// -- the start of a turn ---------------------------------------------------------------------------------------------------------

/// The string's step up to the question of the action menu (the table has no answer for it, which cuts the step off there), the native
/// reconcile and prelude on a copy. `ids` answer the questions of the prelude.
pub(super) fn prelude_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let string = string_run(state, ids, |g| {
        let _ = g.step();
    });
    let native = native_steps(state, &[(Arm::TurnPrelude, None)], true, ids);
    // The action menu is the question the string's step was cut off at: it is not the prelude's.
    let mut string = string;
    if string.questions.last().is_some_and(|q| q.context.as_ref().is_some_and(|c| c.subtype == "action_menu")) {
        string.questions.pop();
    }
    (string, native)
}

#[test]
fn a_plain_turn_starts_like_the_string() {
    let state = action_table(&THREE);
    let (string, native) = prelude_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TURN_BEGAN"), "{:?}", string.events);
}

#[test]
fn psychoarchaeology_and_the_commander_unlock_start_the_turn_like_the_string() {
    // A Psychoarchaeology owner with a ready specialty planet is asked to exhaust it, then asked again until it declines.
    let mut state = crate::fixtures::seated_dealt(&[("a", "l1z1x"), ("b", "hacan"), ("c", "arborec")], DEFAULT);
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid("a"));
    let content = ContentStore::embedded();
    let planet = ti4_content::galaxy::planet(content, "cealdri", DEFAULT).expect("a specialty planet");
    let system = SystemId::new(planet.system_id().expect("a placed planet"));
    state.system_mut(&system).set_control(ti4_model::id::PlanetId::new("cealdri"), pid("a"));
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("pa"));
    // Four dreadnoughts: the L1Z1X commander unlocks.
    crate::fixtures::put(&mut state, &system, "l1z1x_dreadnought", &pid("a"), 4);
    let state = based(state);
    let (string, native) = prelude_both(&state, &["cealdri"]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("LEADER_UNLOCKED:")), "{:?}", string.events);
    assert_eq!(string.questions.len(), 1, "the planet is exhausted, no other specialty is ready: nothing is asked again");
}

#[test]
fn spec_ops_return_and_a_military_support_note_start_the_turn_like_the_string() {
    let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")], DEFAULT);
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid("a"));
    state.player_mut(&pid("a")).unwrap().spec_ops_card = 2;
    let state = based(state);
    let (string, native) = prelude_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("SPEC_OPS_RETURNED:a:2")), "{:?}", string.events);
}

/// Chaos Mapping's owner `a` (Sol) has a space dock on a planet it controls and `goods` trade goods; `b` (Hacan) sits at the table.
fn chaos_table(goods: i32) -> (GameState, SystemId) {
    let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "hacan")], DEFAULT);
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(pid("a"));
    let (system, planet) = crate::fixtures::a_placed_planet();
    state.system_mut(&system).set_control(planet.clone(), pid("a"));
    crate::fixtures::put_on_planet(&mut state, &system, &planet, "spacedock", &pid("a"), 1);
    let seat = state.player_mut(&pid("a")).unwrap();
    seat.technologies.insert(ti4_model::id::TechnologyId::new("cm"));
    seat.trade_goods = goods;
    (based(state), system)
}

#[test]
fn chaos_mapping_produces_a_unit_at_the_start_of_the_turn_like_the_string() {
    let (state, system) = chaos_table(1);
    let system = system.to_string();
    // A destroyer paid with the trade good, a decline of the system and a decline of the unit.
    let (string, native) = prelude_both(&state, &[system.as_str(), "build|destroyer|1", "trade_good"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 3, "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert_eq!(string.questions[0].prompt, "Chaos Mapping");
    let (string, native) = prelude_both(&state, &["decline"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 1);
    let (string, native) = prelude_both(&state, &[system.as_str(), "decline"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 2);
}

#[test]
fn a_chaos_mapping_unit_over_the_capacity_asks_which_unit_goes_like_the_string() {
    let (mut state, system) = chaos_table(3);
    // Three fighters fill the dock's capacity: the fourth stands over it and the string asks which unit goes.
    crate::fixtures::put(&mut state, &system, "fighter", &pid("a"), 3);
    let state = based(state);
    let system = system.to_string();
    let (string, native) = prelude_both(&state, &[system.as_str(), "build|fighter|1", "trade_good", "remove|0"]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.contains("remove")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

// -- the phases ------------------------------------------------------------------------------------------------------------------

use ti4_model::id::{StrategyCardId, SystemId};
use ti4_model::state::Phase as StringPhase;

pub(super) fn phase_both(state: &GameState) -> (Run, Run) {
    let string = string_run(state, &[], |g| {
        let result = g.step_phase();
        assert!(result.error.is_none(), "the phase moves: {:?}", result.error);
    });
    let native = native_steps(state, &[(Arm::Phase, None)], false, &[]);
    (string, native)
}

/// A table at the end of the strategy phase: every seat holds a card.
pub(super) fn drafted(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = StringPhase::Strategy;
    let cards: Vec<StrategyCardId> = state.unclaimed_strategy_cards.clone();
    for (seat, card) in seats.iter().zip(cards) {
        state.deal_strategy_card(&pid(seat.0), card);
    }
    based(state)
}

#[test]
fn the_strategy_phase_ends_into_the_action_phase_like_the_string() {
    let (string, native) = phase_both(&drafted(&THREE));
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "ACTION_PHASE_BEGAN"), "{:?}", string.events);
    // The Naalu "0" token makes its seat first in initiative order.
    let (string, native) = phase_both(&drafted(&[("a", "sol"), ("b", "naalu"), ("c", "arborec")]));
    assert_same(&string, &native);
}

#[test]
fn the_action_phase_ends_into_the_status_phase_like_the_string() {
    let mut state = drafted(&[("a", "sol"), ("b", "hacan"), ("c", "mentak")]);
    state.phase = StringPhase::Action;
    state.active = None;
    for p in ["a", "b", "c"] {
        state.player_mut(&pid(p)).unwrap().passed = true;
    }
    let (string, native) = phase_both(&state);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "STATUS_PHASE_BEGAN"), "{:?}", string.events);
}

#[test]
fn the_status_phase_ends_into_the_agenda_phase_or_the_next_round_like_the_string() {
    let mut state = drafted(&THREE);
    state.phase = StringPhase::Status;
    state.custodians_removed = true;
    let (string, native) = phase_both(&state);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "AGENDA_PHASE_BEGAN"), "{:?}", string.events);
    state.custodians_removed = false;
    let (string, native) = phase_both(&state);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "ROUND_BEGAN"), "{:?}", string.events);
    state.phase = StringPhase::Agenda;
    state.custodians_removed = true;
    let (string, native) = phase_both(&state);
    assert_same(&string, &native);
}

#[test]
fn the_anti_intellectual_revolution_exhausts_planets_for_technologies_at_the_next_strategy_phase_like_the_string() {
    let mut state = drafted(&THREE);
    state.phase = StringPhase::Status;
    state.laws.insert("revolution".to_owned(), "against".to_owned());
    let (string, native) = phase_both(&state);
    assert_same(&string, &native);
}

#[test]
fn the_first_strategy_phase_announces_itself_like_the_string() {
    let state = based(crate::fixtures::seated_dealt(&THREE, DEFAULT));
    let string = {
        let mut run = string_run(&state, &[], |g| {
            let _ = g.step();
        });
        // The step went on to the draft; the draft question is not the announcement's.
        run.questions.clear();
        run
    };
    let native = native_steps(&state, &[(Arm::StrategyAnnounce, None)], true, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "STRATEGY_PHASE_BEGAN"), "{:?}", string.events);
}

// -- the draft and the starting technology ---------------------------------------------------------------------------------------

/// The first pick of round 1: the step announces the strategy phase and asks the speaker to choose a card.
#[test]
fn the_first_pick_of_the_draft_announces_the_phase_and_takes_the_card_like_the_string() {
    let state = based(crate::fixtures::seated_dealt(&THREE, DEFAULT));
    let card = state.unclaimed_strategy_cards[2].to_string();
    let string = string_run(&state, &[card.as_str()], |g| {
        let r = g.step();
        assert!(r.error.is_none(), "the step runs: {:?}", r.error);
    });
    let native = native_steps(&state, &[(Arm::StrategyAnnounce, None), (Arm::Ask, None)], true, &[card.as_str()]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 1);
    assert!(string.events.iter().any(|e| e == "STRATEGY_CARD_CHOSEN"), "{:?}", string.events);
}

#[test]
fn a_card_with_trade_goods_on_it_pays_them_to_the_picker_like_the_string() {
    let mut state = crate::fixtures::seated_dealt(&THREE, DEFAULT);
    let card = state.unclaimed_strategy_cards[4].clone();
    state.strategy_card_goods.insert(card.clone(), 3);
    let state = based(state);
    let string = string_run(&state, &[card.as_str()], |g| {
        assert!(g.step().error.is_none());
    });
    let native = native_steps(&state, &[(Arm::StrategyAnnounce, None), (Arm::Ask, None)], true, &[card.as_str()]);
    assert_same(&string, &native);
    // The gain is announced at the next step (staged).
    assert_eq!(string.staged, ["TRADE_GOODS_GAINED"]);
}

fn starting_table(seats: &[(&str, &str)]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    crate::starting_tech::open(&mut state, ContentStore::embedded(), DEFAULT);
    based(state)
}

#[test]
fn the_winnu_choose_a_starting_technology_like_the_string() {
    let state = starting_table(&[("a", "winnu"), ("b", "sol"), ("c", "hacan")]);
    assert!(!state.starting_tech_picks.is_empty(), "the Winnu owe a pick");
    let string = string_run(&state, &["st"], |g| {
        assert!(g.step().error.is_none());
    });
    let native = native_steps(&state, &[(Arm::Ask, None)], true, &["st"]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "STARTING_TECHNOLOGY_CHOSEN"), "{:?}", string.events);
}

#[test]
fn argent_flight_chooses_two_of_three_starting_technologies_like_the_string() {
    let state = starting_table(&[("a", "argent"), ("b", "sol"), ("c", "hacan")]);
    let string = string_run(&state, &["nm", "ps"], |g| {
        assert!(g.step().error.is_none());
        assert!(g.step().error.is_none());
    });
    let native = native_marked(&state, &[(true, Arm::Ask, None), (true, Arm::Ask, None)], &["nm", "ps"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 2);
    assert!(string.questions[0].prompt.contains("1 of 2"), "{}", string.questions[0].prompt);
}

// -- the end of an action ----------------------------------------------------------------------------------------------------------

pub(super) fn finish_both(state: &GameState) -> (Run, Run) {
    let string = string_run(state, &[], |g| g.finish_action().expect("the action finishes"));
    let native = native_run(state, Arm::Ask, Some(TurnJob::Finish), &[]);
    (string, native)
}

#[test]
fn the_end_of_an_action_announces_it_and_passes_the_turn_when_nothing_else_is_offered_like_the_string() {
    let state = action_table(&THREE);
    let (string, native) = finish_both(&state);
    assert_same(&string, &native);
    assert_eq!(string.events.first().map(String::as_str), Some("ACTION_COMPLETED"), "{:?}", string.events);
    assert!(string.events.iter().any(|e| e == "TURN_PASSED"));
}

#[test]
fn a_retained_turn_skips_the_closing_menu_like_the_string() {
    let mut state = action_table(&THREE);
    state.transient_flags.set(ti4_model::state::TransientFlags::ADDITIONAL_ACTION);
    let (string, native) = finish_both(&state);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TURN_RETAINED"), "{:?}", string.events);
}

// -- the reconcile hooks and the rest of the end of a turn ---------------------------------------------------------------------

/// One string step of a position whose turn is prepared (the arm is `Ask`, cut off at the menu) against the native reconcile alone.
pub(super) fn reconcile_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let string = string_run(state, ids, |g| {
        g.prepared_turn_seq = Some(g.state.turn_seq);
        let _ = g.step();
    });
    let mut string = string;
    if string.questions.last().is_some_and(|q| q.context.as_ref().is_some_and(|c| c.subtype == "action_menu")) {
        string.questions.pop();
    }
    let native = native_marked_arms(state, &[(true, None, None)], ids);
    (string, native)
}

#[test]
fn a_technology_nobody_announced_is_announced_by_the_next_step_like_the_string() {
    let mut state = action_table(&THREE);
    // The first look records the starting set; a technology gained after it is announced.
    {
        let mut game = super::Game::with_table(state.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption)));
        game.prepared_turn_seq = Some(game.state.turn_seq);
        let _ = game.step();
        state = game.state.clone();
    }
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("st"));
    let (string, native) = reconcile_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "TECHNOLOGY_GAINED"), "{:?}", string.events);
}

#[test]
fn spec_ops_destroyed_roll_to_survive_from_the_streams_of_the_game_like_the_string() {
    let mut state = action_table(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")]);
    state.player_mut(&pid("a")).unwrap().spec_ops_destroyed = 4;
    let (string, native) = reconcile_both(&state, &[]);
    assert_same(&string, &native);
    assert_eq!(string.events.iter().filter(|e| e.starts_with("SPEC_OPS_")).count(), 4, "{:?}", string.events);
}

#[test]
fn the_nexus_latch_and_styxs_victory_point_are_reconciled_like_the_string() {
    let mut state = action_table(&THREE);
    crate::fixtures::put(&mut state, &SystemId::new("82a"), "cruiser", &pid("b"), 1);
    let content = ContentStore::embedded();
    let styx = ti4_content::galaxy::planet(content, "styx", DEFAULT).expect("Styx is in the corpus");
    state.system_mut(&SystemId::new(styx.system_id().expect("a placed planet"))).set_control(ti4_model::id::PlanetId::new("styx"), pid("c"));
    let (string, native) = reconcile_both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.state["nexus_unlocked"].as_bool().unwrap_or(false), "the latch is set");
}

#[test]
fn ships_over_the_fleet_supply_are_removed_one_question_at_a_time_at_the_end_of_a_turn_like_the_string() {
    let mut state = action_table(&THREE);
    let (system, _) = crate::fixtures::a_placed_planet();
    crate::fixtures::put(&mut state, &system, "cruiser", &pid("b"), 9);
    let (string, answers) = string_run_first(&state, &[], |g| g.advance_turn().expect("the turn passes"));
    assert!(!string.questions.is_empty(), "the fleet is over its supply: the owner is asked what goes");
    let ids: Vec<&str> = answers.iter().map(String::as_str).collect();
    let native = native_run(&state, Arm::Ask, Some(TurnJob::End), &ids);
    assert_same(&string, &native);
}

#[test]
fn bio_stims_readies_a_specialty_planet_at_the_end_of_a_turn_like_the_string() {
    let mut state = action_table(&THREE);
    let content = ContentStore::embedded();
    let planet = ti4_content::galaxy::planet(content, "cealdri", DEFAULT).expect("a specialty planet");
    let system = SystemId::new(planet.system_id().expect("a placed planet"));
    state.system_mut(&system).set_control(ti4_model::id::PlanetId::new("cealdri"), pid("a"));
    state.exhausted_planets.insert(ti4_model::id::PlanetId::new("cealdri"));
    state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("bs"));
    let (string, native) = end_both(&state, &["ready|planet|cealdri"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 1);
}

#[test]
fn the_last_card_of_the_draft_ends_it_like_the_string() {
    // Two seats, one card each: the second pick is the last.
    let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "hacan")], DEFAULT);
    state.strategy_cards_per_player = 1;
    let first = state.unclaimed_strategy_cards.remove(0);
    state.deal_strategy_card(&pid("a"), first);
    let state = based(state);
    let last = state.unclaimed_strategy_cards[0].to_string();
    let string = string_run(&state, &[last.as_str()], |g| {
        g.strategy_phase_announced = true;
        assert!(g.step().error.is_none());
    });
    let native = native_marked_arms(&state, &[(true, Some(Arm::Ask), None)], &[last.as_str()]);
    assert_same(&string, &native);
}

/// A table of `a` (sol), `b` (arborec) at the last pick of a draft of two cards each: `a` holds Technology, `b` Leadership and Politics, and
/// Diplomacy is the last card on the mat. `a` holds the fewest cards and takes it (the string does not ask for the last card).
fn arbiter_table(arbiter_for: Option<&str>) -> GameState {
    // (The Arborec have a module: with one seated the string stages the events of the datahub's swap.)
    let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "arborec")], DEFAULT);
    state.strategy_cards_per_player = 2;
    state.unclaimed_strategy_cards.clear();
    for (who, card) in [("a", "pok7technology"), ("b", "pok1leadership"), ("b", "pok3politics")] {
        state.deal_strategy_card(&pid(who), ti4_model::id::StrategyCardId::new(card));
    }
    state.unclaimed_strategy_cards.push(ti4_model::id::StrategyCardId::new("pok2diplomacy"));
    if let Some(owner) = arbiter_for {
        state.laws.insert("arbiter".to_owned(), owner.to_owned());
    }
    based(state)
}

fn draft_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let string = string_run(state, ids, |g| {
        g.strategy_phase_announced = true;
        assert!(g.step().error.is_none());
    });
    let native = native_marked_arms(state, &[(true, Some(Arm::Ask), None)], ids);
    (string, native)
}

fn cards_of(run: &Run, seat: usize) -> Value {
    run.state["players"][seat]["strategy_cards"].clone()
}

#[test]
fn imperial_arbiter_swaps_cards_at_the_end_of_the_draft_like_the_string() {
    let state = arbiter_table(Some("a"));
    // `a` takes the last card (Diplomacy), discards the law, gives Diplomacy and takes Leadership from `b`: the taken card goes on the END of
    // the holding (Technology, Leadership), which is not the initiative order the datahub's swap keeps.
    let (string, native) = draft_both(&state, &["discard", "pok2diplomacy", "b|pok1leadership"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 3, "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert!(string.events.iter().any(|e| e == "STRATEGY_CARDS_SWAPPED:a:b"), "{:?}", string.events);
    assert_eq!(cards_of(&string, 0), serde_json::json!(["pok7technology", "pok1leadership"]));
    assert_eq!(cards_of(&string, 1), serde_json::json!(["pok3politics", "pok2diplomacy"]));
    assert!(string.state["laws"].get("arbiter").is_none(), "the law was discarded");
    // A decline keeps the law and swaps nothing.
    let (string, native) = draft_both(&state, &["decline"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 1);
    assert!(string.state["laws"].get("arbiter").is_some());
    assert!(!string.events.iter().any(|e| e.starts_with("STRATEGY_CARDS_SWAPPED")));
    // The other card of the owner, and a card of the other holder's second place.
    let (string, native) = draft_both(&state, &["discard", "pok7technology", "b|pok3politics"]);
    assert_same(&string, &native);
    assert_eq!(cards_of(&string, 0), serde_json::json!(["pok2diplomacy", "pok3politics"]));
    // The law is the other seat's: it is offered to `b`, who gives Politics for Technology.
    let (string, native) = draft_both(&arbiter_table(Some("b")), &["discard", "pok3politics", "a|pok7technology"]);
    assert_same(&string, &native);
    // Without the law nothing is offered.
    let (string, native) = draft_both(&arbiter_table(None), &[]);
    assert_same(&string, &native);
    assert!(string.questions.is_empty());
}

#[test]
fn the_quantum_datahub_node_swaps_cards_at_the_end_of_the_draft_like_the_string() {
    let mut state = arbiter_table(None);
    {
        let b = state.player_mut(&pid("b")).unwrap();
        b.technologies.insert(ti4_model::id::TechnologyId::new("qdn"));
        b.strategic_tokens = 2;
        b.trade_goods = 4;
    }
    let state = based(state);
    // `b` (the holder: Leadership and Politics) gives Leadership for the Diplomacy `a` drafted: three trade goods go to `a`, one strategy token
    // is spent, and the events of both wait staged for the next step. Both holdings stay in initiative order.
    let (string, native) = draft_both(&state, &["qdn|a|pok1leadership|pok2diplomacy"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 1);
    assert!(string.events.iter().any(|e| e == "QUANTUM_DATAHUB_SWAP:b:a"), "{:?}", string.events);
    assert_eq!(string.staged, ["STRATEGY_TOKEN_SPENT", "TRADE_GOODS_GAINED"]);
    assert_eq!(cards_of(&string, 0), serde_json::json!(["pok1leadership", "pok7technology"]));
    assert_eq!(cards_of(&string, 1), serde_json::json!(["pok2diplomacy", "pok3politics"]));
    assert_eq!(string.state["players"][1]["trade_goods"], 1);
    // A decline, and a holder who cannot pay (two trade goods) or has no token: no question.
    let (string, native) = draft_both(&state, &["decline"]);
    assert_same(&string, &native);
    assert!(string.staged.is_empty());
    for (tokens, goods) in [(0, 4), (2, 2)] {
        let mut poor = state.clone();
        poor.player_mut(&pid("b")).unwrap().strategic_tokens = tokens;
        poor.player_mut(&pid("b")).unwrap().trade_goods = goods;
        let (string, native) = draft_both(&poor, &[]);
        assert_same(&string, &native);
        assert!(string.questions.is_empty(), "{tokens} {goods}");
    }
}

#[test]
fn the_arbiter_is_offered_first_and_the_datahub_sees_its_swap() {
    let mut state = arbiter_table(Some("a"));
    {
        let b = state.player_mut(&pid("b")).unwrap();
        b.technologies.insert(ti4_model::id::TechnologyId::new("qdn"));
        b.strategic_tokens = 1;
        b.trade_goods = 3;
    }
    let state = based(state);
    // The Arbiter's swap gives `b` Diplomacy for Leadership; the datahub then offers the holder what it holds NOW.
    let (string, native) = draft_both(&state, &["discard", "pok2diplomacy", "b|pok1leadership", "qdn|a|pok3politics|pok7technology"]);
    assert_same(&string, &native);
    assert_eq!(string.questions.len(), 4);
    assert_eq!(string.events.iter().filter(|e| e.starts_with("STRATEGY_CARDS_SWAPPED") || e.starts_with("QUANTUM_DATAHUB_SWAP")).count(), 2);
}
