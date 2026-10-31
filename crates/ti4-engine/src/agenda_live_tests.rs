//! Lane D (package KV): the native agenda phase (`ti4_engine_core::driver::agenda`) run through `live::live_step` against the string steps
//! it replaces (`step_agenda`, `open_next_vote`, `step_vote`, `close_vote` and the close of the agenda-timed scoring window), question by
//! question: the questions as rendered, the events, the position and the dice streams, from the opening of the agenda phase to
//! `AGENDA_PHASE_RESOLVED`.
//!
//! The string side plays `Game::step` until the phase's last event, answering the scripted ids and then the first option; the native side
//! takes the answers the string was given and runs one `live_step` per step the position's arm names.

use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_engine_core::driver::session::{step_arm, Arm, Facts};
use ti4_engine_core::state::SliceSet;
use ti4_model::content_types::DEFAULT;
use ti4_model::state::{GameState, Phase};

use super::turn_live_tests::{assert_same, based, pid, Run};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// How the table answers once the script has run out: the id of the first option whose id is in `prefer` (in that order of preference),
/// else the first option.
type Policy = &'static [&'static str];

/// A table that answers the scripted ids, then by `prefer`; it records the questions and the answers it took.
struct Chooser {
    ids: std::collections::VecDeque<String>,
    prefer: Policy,
    seen: Rc<std::cell::RefCell<Vec<crate::choice::Choice>>>,
    taken: Rc<std::cell::RefCell<Vec<String>>>,
}

impl crate::choice::Decider for Chooser {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        let id = self.ids.pop_front().or_else(|| {
            let pick = self.prefer.iter().find_map(|p| choice.options.iter().find(|o| o.id == *p)).or_else(|| choice.options.first())?;
            self.taken.borrow_mut().push(pick.id.clone());
            Some(pick.id.clone())
        });
        let Some(id) = id else { return Err(crate::choice::IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() }) };
        choice.options.iter().find(|o| o.id == id).cloned().ok_or_else(|| panic!("the script answers {id:?}, the question {:?} offers {:?}", choice.prompt, choice.ids()))
    }
}

/// The string's agenda phase on a copy of `state`: `Game::step` until `AGENDA_PHASE_RESOLVED`. The answers it took.
fn string_agenda(state: &GameState, ids: &[&str]) -> (Run, Vec<String>) {
    string_agenda_preferring(state, ids, &[])
}

fn string_agenda_preferring(state: &GameState, ids: &[&str], prefer: Policy) -> (Run, Vec<String>) {
    let _serial = crate::dual::tests::serial();
    let seen: Rc<std::cell::RefCell<Vec<crate::choice::Choice>>> = Rc::default();
    let taken: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
    let table = crate::choice::Table::with_default(Box::new(Chooser { ids: ids.iter().map(|s| (*s).to_owned()).collect(), prefer, seen: seen.clone(), taken: taken.clone() }));
    let mut g = super::Game::with_table(state.clone(), ContentStore::embedded(), table);
    {
        let g = &mut g;
        for _ in 0..400 {
            let result = g.step();
            assert_eq!(result.error, None, "the string step refused");
            if g.events.iter().any(|e| e == "AGENDA_PHASE_RESOLVED") || g.state.finished {
                break;
            }
        }
    }
    let questions = seen.borrow().clone();
    let (state, staged) = super::turn_live_tests::without_staged(serde_json::to_value(&g.state).expect("serializes"));
    let mut all: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
    all.extend(taken.borrow().iter().cloned());
    (Run { questions, events: g.events.clone(), state, rng: g.rng_positions(), staged }, all)
}

/// The native agenda phase on a copy of `state`: the reconcile hooks and one `live_step` of the arm the position names, while the arm is
/// one of the agenda phase's.
fn native_agenda(state: &GameState, ids: &[String]) -> Run {
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
    for _ in 0..400 {
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
        if !matches!(arm, Arm::Agenda | Arm::Voting | Arm::EventScoring) {
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
    both_preferring(state, ids, &[])
}

fn both_preferring(state: &GameState, ids: &[&str], prefer: Policy) -> (Run, Run) {
    let (string, taken) = string_agenda_preferring(state, ids, prefer);
    let native = native_agenda(state, &taken);
    (string, native)
}

/// A table in the agenda phase: the custodians gone, seat `a` the speaker, `deck` on top of the agenda deck and two planets with influence for
/// each seat to vote with.
fn agenda_table(seats: &[(&str, &str)], deck: &[&str]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = Phase::Agenda;
    state.active = None;
    state.custodians_removed = true;
    state.speaker = pid(seats[0].0);
    state.agenda_deck = deck.iter().map(|a| (*a).to_owned()).collect();
    let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), ti4_model::content_types::POK);
    let mut free = catalogue.iter().filter(|(_, p)| p.influence() > 0 && p.homeworld_of().is_none() && !p.is_placed_during_play());
    for (name, _) in seats {
        for _ in 0..2 {
            if let Some((id, record)) = free.next() {
                state.system_mut(&ti4_model::id::SystemId::new(record.system_id().unwrap_or("18"))).set_control(ti4_model::id::PlanetId::new(*id), pid(name));
            }
        }
    }
    based(state)
}

const FOUR: [(&str, &str); 4] = [("a", "sol"), ("b", "hacan"), ("c", "arborec"), ("d", "letnev")];

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|e| e == event)
}

#[test]
fn a_for_against_law_is_revealed_voted_and_closed_like_the_string() {
    let (string, native) = both(&agenda_table(&FOUR, &["checks", "conscription"]), &[]);
    assert_same(&string, &native);
    for e in ["AGENDA_PHASE_RESOLVED", "AGENDA_REVEALED:checks"] {
        assert!(has(&string, e), "{e} in {:?}", string.events);
    }
}

#[test]
fn an_empty_agenda_deck_finishes_the_phase_like_the_string() {
    let (string, native) = both(&agenda_table(&FOUR, &[]), &[]);
    assert_same(&string, &native);
    assert!(has(&string, "AGENDA_PHASE_RESOLVED"));
}


/// Every agenda of the corpus, on a table that gives each one something to act on: laws in play, a scored secret, a hand of cards.
fn rich_table(alias: &str) -> GameState {
    let mut state = agenda_table(&FOUR, &[alias, "conscription"]);
    for law in ["minister_war", "minister_peace"] {
        state.laws.insert(law.to_owned(), "b".to_owned());
    }
    state.laws.insert("regulations".to_owned(), "for".to_owned());
    state.scored_objectives.entry(pid("c")).or_default().insert(ti4_model::id::ObjectiveId::new("dtd"));
    based(state)
}

fn sweep(prefer: Policy) -> Vec<String> {
    let mut failures = Vec::new();
    for agenda in ti4_engine_core::generated::Agenda::ALL {
        let alias = crate::generated::IdStr::as_str(agenda);
        let state = rich_table(alias);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (string, native) = both_preferring(&state, &[], prefer);
            assert_same(&string, &native);
        }));
        if let Err(e) = outcome {
            let text = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            failures.push(format!("{alias}: {}", text.lines().next().unwrap_or("")));
        }
    }
    failures
}

#[test]
fn every_agenda_is_voted_and_resolved_like_the_string_when_the_table_backs_the_first_option() {
    let failures = sweep(&[]);
    assert!(failures.is_empty(), "{} agendas differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_agenda_is_voted_and_resolved_like_the_string_when_the_table_votes_against() {
    let failures = sweep(&["against"]);
    assert!(failures.is_empty(), "{} agendas differ:\n{}", failures.len(), failures.join("\n"));
}
