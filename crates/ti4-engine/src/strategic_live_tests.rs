//! Lane E (package KX): the native strategic action (`ti4_engine_core::driver::windows::strategic`) run through `live::live_step` against
//! the string steps it replaces (`step_secondary`, `step_leader_followers`, `select_leader_strategy_followers`,
//! `complete_leader_strategy`): the questions as rendered, the events, the position and the dice streams, from the first follower to the end of
//! the action.
//!
//! The string side plays `Game::step` with the follower window open until `STRATEGIC_ACTION_COMPLETE`, answering the scripted ids and then the
//! first option; the native side takes the answers the string was given and runs one `live_step` per step the position's arm names.

use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_engine_core::driver::session::{step_arm, Arm, Facts};
use ti4_engine_core::driver::windows::strategic::secondary_of;
use ti4_engine_core::state::{SliceSet, WindowKind};
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::{GameState, Phase};

use super::turn_live_tests::{assert_same, based, pid, string_run_first, without_staged, Run};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// The text of a strategy card's id for the table's card set.
fn card_id(card: &str) -> StrategyCardId {
    StrategyCardId::new(card)
}

/// A table in the action phase, seat `a` active and holding `card`, the turn begun.
fn action_table(seats: &[(&str, &str)], card: &str) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = Phase::Action;
    state.active = Some(pid(seats[0].0));
    state.player_mut(&pid(seats[0].0)).unwrap().strategy_cards.push(card_id(card));
    based(state)
}

const FOUR: [(&str, &str); 4] = [("a", "sol"), ("b", "hacan"), ("c", "arborec"), ("d", "letnev")];

/// The string's follower window of `a`'s `card`, stepped until `STRATEGIC_ACTION_COMPLETE`.
fn string_secondary(state: &GameState, card: &str, ids: &[&str]) -> (Run, Vec<String>) {
    string_run_first(state, ids, |g| {
        let active = pid("a");
        let answer = crate::choice::ChoiceOption::labelled(crate::strategy::STRATEGIC_ACTION_ID, crate::strategy::ACTION_KIND, "strategic");
        let window = crate::strategy::begin_strategic_action(&mut g.state, g.content, &active, answer).expect("the seat holds the card");
        assert_eq!(window.card().as_str(), card);
        g.secondary = Some(window);
        for _ in 0..80 {
            let _ = g.step();
            if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") || g.state.finished {
                break;
            }
        }
    })
}

/// The native follower window of `a`'s `card` on a copy of `state`, with the answers the string was given.
fn native_secondary(state: &GameState, card: &str, ids: &[String]) -> Run {
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
    // The window `begin_strategic_action` opens: the followers clockwise from the primary player, the player excluded.
    let primary = render.seat("a").expect("seat a");
    let followers: Vec<_> = state.clockwise_from(&pid("a")).iter().skip(1).map(|p| render.seat(p.as_str()).expect("a seated follower")).collect();
    let typed_card = <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id(card).expect("a card of the tables");
    core.pending_mut().open(ti4_engine_core::state::Window::StrategySecondary(secondary_of(primary, typed_card, &followers, true)));
    let mut asker = Scripted::new(ids.iter().cloned());
    for _ in 0..80 {
        loop {
            let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
            if step.finished {
                break;
            }
        }
        let arm = step_arm(&Facts::of_core(&core, false), false);
        if arm != Arm::Secondary {
            break;
        }
        live::live_step(&mut core, arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native {arm:?} stopped: {e:?}"));
    }
    assert!(core.pending().window(WindowKind::StrategySecondary).is_none(), "the follower window ended");
    let questions = asker.seen.clone();
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

/// Both sides on one position.
fn both(state: &GameState, card: &str, ids: &[&str]) -> (Run, Run) {
    let (string, taken) = string_secondary(state, card, ids);
    let native = native_secondary(state, card, &taken);
    (string, native)
}

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|e| e == event)
}

#[test]
fn a_trade_secondary_followed_by_everyone_runs_like_the_string() {
    let mut state = action_table(&FOUR, "pok5trade");
    for p in ["b", "c", "d"] {
        state.player_mut(&pid(p)).unwrap().commodities = 0;
    }
    let (string, native) = both(&based(state), "pok5trade", &["yes", "no", "yes"]);
    assert_same(&string, &native);
    assert!(has(&string, "STRATEGIC_ACTION_COMPLETE"));
    assert!(has(&string, "STRATEGY_SECONDARY_FOLLOWED") && has(&string, "STRATEGY_SECONDARY_DECLINED"), "{:?}", string.events);
}

#[allow(dead_code)]
fn unused(_: PlayerId) {}
