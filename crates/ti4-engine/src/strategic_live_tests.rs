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

use super::turn_live_tests::{assert_same as assert_same_plain, based, pid, shape, string_run_first, without_staged, Run};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// The renderer of a native table: the seats and the faction text the options that name a seat by faction use.
fn named(cx: &Ctx, state: &GameState) -> Render {
    let r = Render::new(cx.owners.ids().to_vec());
    let factions = r.seats.iter().map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned())).collect();
    r.with_factions(factions)
}

/// [`assert_same_plain`] that names the first question that differs and how.
fn assert_same(string: &Run, native: &Run) {
    for (i, (a, b)) in string.questions.iter().zip(&native.questions).enumerate() {
        let (a, b) = (shape(a), shape(b));
        let diffs = crate::dual::diff_json(&a, &b, 8);
        assert!(diffs.is_empty(), "question {i} ({}) differs: {diffs:?}", string.questions[i].prompt);
    }
    assert_eq!(string.questions.len(), native.questions.len(), "the number of questions: {:?} against {:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>(), native.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert_same_plain(string, native);
}

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
    native_arms(state, ids, &|core, render| {
        // The window `begin_strategic_action` opens: the followers clockwise from the primary player, the player excluded.
        let primary = render.seat("a").expect("seat a");
        let followers: Vec<_> = state.clockwise_from(&pid("a")).iter().skip(1).map(|p| render.seat(p.as_str()).expect("a seated follower")).collect();
        let typed_card = <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id(card).expect("a card of the tables");
        core.pending_mut().open(ti4_engine_core::state::Window::StrategySecondary(secondary_of(primary, typed_card, &followers, true)));
    })
}

/// The native arms of the strategic action on a copy of `state`, after `setup` opened the window, with the answers the string was given.
fn native_arms(state: &GameState, ids: &[String], setup: &dyn Fn(&mut ti4_engine_core::state::CoreState, &Render)) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = named(&cx, state);
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    setup(&mut core, &render);
    let mut asker = Scripted::new(ids.iter().cloned());
    for _ in 0..80 {
        // (The string's loop ends with the step that completes the action: the reconcile of the next step is not part of it.)
        let arm = step_arm(&Facts::of_core(&core, false), false);
        if !matches!(arm, Arm::Secondary | Arm::LeaderFollowers) {
            break;
        }
        loop {
            let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
            if step.finished {
                break;
            }
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

/// A table where every follower holds tokens and can do something with `card`, played by `a`.
fn table_for(card: &str) -> GameState {
    let mut state = action_table(&FOUR, card);
    for p in ["b", "c", "d"] {
        let seat = state.player_mut(&pid(p)).unwrap();
        seat.strategic_tokens = 2;
        seat.commodities = 0;
        // Influence to buy a token with, and a planet to ready.
        seat.trade_goods = 6;
    }
    if card == "pok2diplomacy" {
        for p in ["b", "c", "d"] {
            let planets: Vec<_> = state.controlled_planets(&pid(p)).into_iter().map(|(_, planet)| planet.clone()).collect();
            for planet in planets {
                state.exhausted_planets.insert(planet);
            }
        }
    }
    based(state)
}

macro_rules! secondary_case {
    ($name:ident, $card:expr, [$($id:expr),*]) => {
        #[test]
        fn $name() {
            let (string, native) = both(&table_for($card), $card, &[$($id),*]);
            assert_same(&string, &native);
            assert!(has(&string, "STRATEGIC_ACTION_COMPLETE"), "{:?}", string.events);
        }
    };
}

secondary_case!(a_leadership_secondary_runs_like_the_string, "pok1leadership", ["yes"]);
secondary_case!(a_diplomacy_secondary_runs_like_the_string, "pok2diplomacy", ["yes"]);
secondary_case!(a_politics_secondary_runs_like_the_string, "pok3politics", ["yes"]);
secondary_case!(a_construction_secondary_runs_like_the_string, "pok4construction", ["yes"]);
secondary_case!(a_warfare_secondary_runs_like_the_string, "pok6warfare", ["yes"]);
secondary_case!(a_technology_secondary_runs_like_the_string, "pok7technology", ["yes"]);
secondary_case!(an_imperial_secondary_runs_like_the_string, "pok8imperial", ["yes"]);
secondary_case!(a_thunders_edge_construction_secondary_runs_like_the_string, "te4construction", ["yes"]);

#[test]
fn nobody_who_can_follow_ends_the_action_without_a_question_like_the_string() {
    let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "yssaril"), ("c", "arborec"), ("d", "letnev")], DEFAULT);
    state.phase = Phase::Action;
    state.active = Some(pid("a"));
    state.player_mut(&pid("a")).unwrap().strategy_cards.push(card_id("pok5trade"));
    for p in ["b", "c", "d"] {
        state.player_mut(&pid(p)).unwrap().strategic_tokens = 0;
    }
    let (string, native) = both(&based(state), "pok5trade", &[]);
    assert_same(&string, &native);
    assert!(string.questions.is_empty());
    assert!(has(&string, "STRATEGIC_ACTION_COMPLETE"));
}

// -- the end of the free tactical action of Thunder's Edge Warfare ---------------------------------------------------------------------

#[test]
fn the_end_of_the_free_tactical_action_redistributes_and_holds_the_follower_window_like_the_string() {
    let state = table_for("te6warfare");
    let (string, taken) = string_run_first(&state, &["0|3|5"], |g| {
        let window = crate::strategy::begin_strategic_action(
            &mut g.state,
            g.content,
            &pid("a"),
            crate::choice::ChoiceOption::labelled(crate::strategy::STRATEGIC_ACTION_ID, crate::strategy::ACTION_KIND, "strategic"),
        )
        .expect("the seat holds the card");
        // `close_tactical`: the tactical action ended with a Warfare window held for its end.
        let player = pid("a");
        g.redistribute_after_te_warfare(&player).expect("the redistribution");
        g.secondary = Some(window);
    });
    let (native, open) = native_job_with(&state, &taken, false, &|core, render| {
        let primary = render.seat("a").unwrap();
        let followers: Vec<_> = state.clockwise_from(&pid("a")).iter().skip(1).map(|p| render.seat(p.as_str()).unwrap()).collect();
        core.pending_mut().open(ti4_engine_core::state::Window::StrategySecondaryAfterTactical(secondary_of(primary, ti4_engine_core::generated::StrategyCard::WarfareThundersEdge, &followers, true)));
        ti4_engine_core::driver::windows::strategic::after::open_after(core);
    });
    assert!(open);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.contains("after the tactical action")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

// -- a planning fork's one-follower window --------------------------------------------------------------------------------------------

#[test]
fn a_preview_window_asks_one_follower_and_neither_finishes_the_action_nor_passes_the_turn_like_the_string() {
    let state = table_for("pok5trade");
    let (string, taken) = string_run_first(&state, &["yes"], |g| {
        g.secondary = Some(crate::strategy::StrategySecondaryWindow::preview(pid("a"), card_id("pok5trade"), pid("c")));
        for _ in 0..10 {
            let _ = g.step();
            if g.events.iter().any(|e| e == "STRATEGY_SECONDARY_PREVIEW_COMPLETE") {
                break;
            }
        }
    });
    let native = native_arms(&state, &taken, &|core, render| {
        let mut window = secondary_of(render.seat("a").unwrap(), ti4_engine_core::generated::StrategyCard::Trade, &[render.seat("c").unwrap()], false);
        window.preview = true;
        core.pending_mut().open(ti4_engine_core::state::Window::StrategySecondary(window));
    });
    assert_same(&string, &native);
    assert!(has(&string, "STRATEGY_SECONDARY_PREVIEW_COMPLETE") && !has(&string, "STRATEGIC_ACTION_COMPLETE") && !has(&string, "TURN_PASSED"), "{:?}", string.events);
}

// -- a hero's strategy: the Winnu hero's follower selection ----------------------------------------------------------------------------

/// The string's Winnu hero resolving `card`: the follower selection, then the follower window, until the action completes.
fn string_hero(state: &GameState, card: &str, ids: &[&str]) -> (Run, Vec<String>) {
    string_run_first(state, ids, |g| {
        g.leader_strategy = Some((pid("a"), ti4_model::id::LeaderId::new("winnuhero"), card_id(card)));
        g.select_leader_strategy_followers().expect("the selection opens");
        for _ in 0..80 {
            let _ = g.step();
            if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") || g.state.finished {
                break;
            }
        }
    })
}

fn native_hero(state: &GameState, card: &str, ids: &[String]) -> Run {
    native_arms(state, ids, &|core, render| {
        let typed_card = <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id(card).expect("a card of the tables");
        core.pending_mut().open(ti4_engine_core::state::Window::LeaderStrategy { player: render.seat("a").unwrap(), leader: ti4_engine_core::generated::Leader::MathisMathinus, card: typed_card });
        ti4_engine_core::driver::windows::strategic::select_leader_strategy_followers(core);
    })
}

#[test]
fn the_winnu_hero_asks_who_may_follow_then_the_followers_decide_like_the_string() {
    let seats = [("a", "winnu"), ("b", "hacan"), ("c", "arborec"), ("d", "letnev")];
    let mut state = crate::fixtures::seated_dealt(&seats, DEFAULT);
    state.phase = Phase::Action;
    state.active = Some(pid("a"));
    for p in ["b", "c", "d"] {
        state.player_mut(&pid(p)).unwrap().strategic_tokens = 2;
    }
    let state = based(state);
    let (string, taken) = string_hero(&state, "pok5trade", &["yes|b", "no|c", "yes|d", "yes", "no"]);
    let native = native_hero(&state, "pok5trade", &taken);
    assert_same(&string, &native);
    assert!(has(&string, "COMPONENT_ACTION_RESOLVED") && has(&string, "STRATEGIC_ACTION_COMPLETE"), "{:?}", string.events);
}

// -- the primary: the strategic action chosen from the menu -----------------------------------------------------------------------------

/// The string's step in which `a` answers the action menu with its strategic action of `card` (the menu is the first question, answered
/// `strategic`), and what the step leaves open.
fn string_primary(state: &GameState, ids: &[&str]) -> (Run, Vec<String>, bool) {
    let mut open = false;
    let mut all = vec!["strategic"];
    all.extend_from_slice(ids);
    let (run, taken) = string_run_first(state, &all, |g| {
        g.prepared_turn_seq = Some(g.state.turn_seq);
        let _ = g.step();
        open = g.secondary.is_some() || g.secondary_after_tactical.is_some();
    });
    (run, taken, open)
}

/// The native job on a copy of `state` with the answers the string was given after the menu's.
fn native_primary(state: &GameState, card: &str, ids: &[String]) -> (Run, bool) {
    native_job(state, ids, &|core, _| {
        let typed_card = <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id(card).expect("a card of the tables");
        ti4_engine_core::driver::turn::open_job(core, ti4_engine_core::state::TurnJob::Strategic(typed_card));
    })
}

/// A job of the turn flow on a copy of `state`: `setup` opens it (and whatever windows it reads), then one `live_step` of `Arm::Ask` per
/// string step until it ends. Whether a follower window is open after it.
fn native_job(state: &GameState, ids: &[String], setup: &dyn Fn(&mut ti4_engine_core::state::CoreState, &Render)) -> (Run, bool) {
    native_job_with(state, ids, true, setup)
}

/// [`native_job`] with or without the reconcile hooks first (the string body of a test that does not step the game does not run them).
fn native_job_with(state: &GameState, ids: &[String], reconcile: bool, setup: &dyn Fn(&mut ti4_engine_core::state::CoreState, &Render)) -> (Run, bool) {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = named(&cx, state);
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    let mut asker = Scripted::new(ids.iter().cloned());
    loop {
        if !reconcile {
            break;
        }
        let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    setup(&mut core, &render);
    loop {
        let step = live::live_step(&mut core, Arm::Ask, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native job stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    let open = core.pending().window(WindowKind::StrategySecondary).is_some() || core.pending().window(WindowKind::StrategySecondaryAfterTactical).is_some() || core.pending().window(WindowKind::LeaderFollowers).is_some();
    let questions = asker.seen.clone();
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
    (Run { questions, events, state, rng, staged }, open)
}

fn primary_both(state: &GameState, card: &str, ids: &[&str]) -> (Run, Run) {
    let (mut string, taken, string_open) = string_primary(state, ids);
    // The menu is the string's first question, and its answer the first answer: the native job starts after them.
    assert_eq!(string.questions.first().and_then(|q| q.context.as_ref()).map(|c| c.subtype.as_str()), Some("action_menu"));
    string.questions.remove(0);
    let (native, native_open) = native_primary(state, card, &taken[1..]);
    assert_eq!(string_open, native_open, "the follower window is open on both sides");
    (string, native)
}

macro_rules! primary_case {
    ($name:ident, $card:expr, [$($id:expr),*]) => {
        #[test]
        fn $name() {
            let (string, native) = primary_both(&table_for($card), $card, &[$($id),*]);
            assert_same(&string, &native);
            assert!(has(&string, "STRATEGIC_ACTION_BEGAN"), "{:?}", string.events);
        }
    };
}

primary_case!(a_trade_primary_runs_like_the_string, "pok5trade", []);
primary_case!(a_politics_primary_runs_like_the_string, "pok3politics", []);
primary_case!(a_leadership_primary_runs_like_the_string, "pok1leadership", []);
primary_case!(a_diplomacy_primary_runs_like_the_string, "pok2diplomacy", []);
primary_case!(a_warfare_primary_runs_like_the_string, "pok6warfare", []);
primary_case!(a_technology_primary_runs_like_the_string, "pok7technology", []);
primary_case!(a_construction_primary_runs_like_the_string, "pok4construction", []);
primary_case!(an_imperial_primary_runs_like_the_string, "pok8imperial", []);
primary_case!(a_thunders_edge_construction_primary_runs_like_the_string, "te4construction", []);

/// Thunder's Edge Warfare's primary on a map (a hub of seven systems beside the homes): the redistribution, the choice of the system, the free
/// tactical action (activation, no movement, no fight), the redistribution after it and the follower window, from the menu's answer to the end
/// of the action. `ids` are the scripted answers after the menu's; the rest take the first option. The string `Game` and the native arms
/// (the job, the tactical window, the job after the tactical action, the follower window) agree on questions, events and position.
fn te_warfare_both(state: &GameState, galaxy: &ti4_content::galaxy::Galaxy, ids: &[&str]) -> (Run, Run) {
    let mut all = vec!["strategic"];
    all.extend_from_slice(ids);
    let (mut string, taken) = string_run_first(state, &all, |g| {
        g.galaxy = Some(galaxy.clone());
        g.prepared_turn_seq = Some(g.state.turn_seq);
        for _ in 0..40 {
            let _ = g.step();
            if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") || g.state.finished {
                break;
            }
        }
    });
    assert_eq!(string.questions.first().and_then(|q| q.context.as_ref()).map(|c| c.subtype.as_str()), Some("action_menu"));
    string.questions.remove(0);
    let native = native_te_warfare(state, galaxy, &taken[1..]);
    (string, native)
}

fn native_te_warfare(state: &GameState, galaxy: &ti4_content::galaxy::Galaxy, ids: &[String]) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = named(&cx, state);
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: Some(galaxy), rng: Some(&positions) }, SliceSet::ALL).expect("the position loads");
    let kernel = live::kernel::kernel_for(DEFAULT);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), DEFAULT);
    let lcx = live::Cx::new(&render, &env);
    let mut asker = Scripted::new(ids.iter().cloned());
    loop {
        let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    ti4_engine_core::driver::turn::open_job(&mut core, ti4_engine_core::state::TurnJob::Strategic(ti4_engine_core::generated::StrategyCard::WarfareThundersEdge));
    for _ in 0..80 {
        let chain = step_arm(&Facts::of_core(&core, false), true);
        let arm = match chain {
            Arm::Secondary | Arm::LeaderFollowers | Arm::Tactical | Arm::Aftermath | Arm::EventScoring => chain,
            _ if core.pending().window(WindowKind::TurnFlow).is_some() => Arm::Ask,
            _ => break,
        };
        live::live_step(&mut core, arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native {arm:?} stopped: {e:?} after {:?}", asker.seen.iter().map(|q| q.prompt.clone()).collect::<Vec<_>>()));
        if arm == Arm::Tactical && core.pending().window(WindowKind::Tactical).is_none() && core.pending().window(WindowKind::Aftermath).is_none() && ti4_engine_core::driver::turn::open_job_of(&core).is_none() {
            panic!("a plain tactical action ended: the Warfare action must go on to its follower window");
        }
    }
    assert!(core.pending().window(WindowKind::StrategySecondary).is_none(), "the follower window ended");
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let questions = asker.seen.clone();
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

#[test]
fn a_thunders_edge_warfare_primary_runs_its_free_tactical_action_and_the_follower_window_like_the_string() {
    let hub = crate::fixtures::plain_hub();
    // (The centre only: a ship with several routes asks a movement question whose route alternatives the native path does not produce.)
    for system in [hub.centre.clone()] {
        // (No Arborec at the table, its flagship's activation window is a loud gap row of the silent-ability audit (lane M6d's), and no Hacan, whose Guild Ships make every seat a transaction partner: the closing menu would stop at lane F's `MenuSources`.)
        let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "xxcha"), ("c", "letnev"), ("d", "jolnar")], DEFAULT);
        state.phase = Phase::Action;
        state.active = Some(pid("a"));
        state.player_mut(&pid("a")).unwrap().strategy_cards.push(card_id("te6warfare"));
        // The followers decline (no token spent, nothing else asked), and the primary's second redistribution is the first option.
        for p in ["b", "c", "d"] {
            state.player_mut(&pid(p)).unwrap().strategic_tokens = 0;
        }
        let (mut string, native) = te_warfare_both(&based(state), &hub.galaxy, &["0|3|5", &system]);
        // The typed events of the aftermath's windows (the invasion, the use of PRODUCTION) are announced by the arms and not written to the event
        // log the native run is read from; they are compared in `combat_live` and `invasion_live`.
        let typed = ["INVASION_BEGAN", "PRODUCTION_USED", "UNITS_PRODUCED"];
        let kept: Vec<String> = string.events.iter().filter(|e| !typed.contains(&e.as_str())).cloned().collect();
        let all = std::mem::replace(&mut string.events, kept);
        assert_same(&string, &native);
        string.events = all;
        let started = string.events.iter().position(|e| e == "SYSTEM_ACTIVATED").expect("the free action activates the system");
        let began = string.events.iter().rposition(|e| e == "STRATEGIC_ACTION_BEGAN").expect("the action began");
        assert!(started < began, "the string activates the system before it says the strategic action began: {:?}", string.events);
        assert!(has(&string, "FREE_TACTICAL_ACTION") && has(&string, "STRATEGIC_ACTION_COMPLETE"), "{:?}", string.events);
        assert!(string.questions.iter().any(|q| q.prompt.contains("after the tactical action")), "the second redistribution is asked");
    }
}

// -- bail as failure: whole strategic actions at tables of every lobby faction, both card sets ------------------------------------------

/// Answers every question with its first option.
struct FirstOption;

impl live::Asker for FirstOption {
    fn ask(&mut self, q: &live::Question<'_>) -> Result<String, live::Stop> {
        q.rendered.options.first().map(|o| o.id.clone()).ok_or(live::Stop::Exhausted)
    }
}

/// A native strategic action of `card` at a table of four: `faction` at seat `a`, three others after it. The job, then the follower window
/// until the action ends. Returns the steps run. Any gap of the live path is a failure, except the closing menu's sources (lane F's).
fn native_action(faction: &str, others: [&str; 3], card: &str) -> usize {
    use ti4_engine_core::driver::live::LiveError;
    use ti4_engine_core::kernel::ability::Unknown;
    use ti4_engine_core::kernel::turn_codes::TurnGap;
    let seats = [("a", faction), ("b", others[0]), ("c", others[1]), ("d", others[2])];
    let mut state = crate::fixtures::seated_dealt(&seats, DEFAULT);
    state.phase = Phase::Action;
    state.active = Some(pid("a"));
    state.player_mut(&pid("a")).unwrap().strategy_cards.push(card_id(card));
    for p in ["b", "c", "d"] {
        let seat = state.player_mut(&pid(p)).unwrap();
        seat.strategic_tokens = 2;
        seat.trade_goods = 6;
    }
    let state = based(state);
    let mut cx = Ctx::of(&state).expect("a table the codecs carry");
    let render = named(&cx, &state);
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, &state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let kernel = live::kernel::kernel_for(DEFAULT);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), DEFAULT);
    let lcx = live::Cx::new(&render, &env);
    let typed_card = <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id(card).expect("a card of the tables");
    let mut asker = FirstOption;
    // The reconcile of the step that takes the menu's answer, then the job.
    let mut steps = 0;
    let known = |stop: &live::Stop| -> bool {
        match stop {
            // Nekro's Propagation (a technology gained at the table) is the Nekro lane's counted gap (`docs/KP_RESUME.md`).
            live::Stop::Live(LiveError::Unknown(Unknown::TurnGap(TurnGap::Propagation))) => true,
            // The closing menu's free options are the menu lane's: the action itself ended.
            live::Stop::Live(LiveError::Unknown(Unknown::TurnGap(TurnGap::MenuSources))) => true,
            // A loud row of the silent-ability audit (`silent::gaps`, lanes G3 / M6d): a counted gap of the live path, owned by the lane that ports it.
            live::Stop::Live(LiveError::Unknown(Unknown::AbilityGap(_))) => true,
            // Doctor Sucaban's offer in a research (the Jol-Nar agent readied at the table, infantry on the board) is not ported: counted.
            live::Stop::Live(LiveError::Unknown(Unknown::TurnGap(TurnGap::StrategyAbility))) => card.contains("technology") && (faction == "jolnar" || others.contains(&"jolnar")),
            _ => false,
        }
    };
    match live::live_reconcile(&mut core, &lcx, &mut asker) {
        Ok(_) => {}
        Err(stop) if known(&stop) => return steps,
        Err(stop) => panic!("{faction} {card}: the reconcile stopped: {stop:?}"),
    }
    ti4_engine_core::driver::turn::open_job(&mut core, ti4_engine_core::state::TurnJob::Strategic(typed_card));
    for _ in 0..60 {
        // The follower windows are arms of their own; a job of the turn flow left open (the dispatch, the end of the action) is the `Ask` arm's.
        let chain = step_arm(&Facts::of_core(&core, false), true);
        let arm = if matches!(chain, Arm::Secondary | Arm::LeaderFollowers) {
            chain
        } else if core.pending().window(WindowKind::TurnFlow).is_some() {
            Arm::Ask
        } else {
            break;
        };
        match live::live_step(&mut core, arm, &lcx, &mut asker) {
            Ok(_) => steps += 1,
            Err(stop) if known(&stop) => break,
            Err(stop) => panic!("{faction} {card} (with {others:?}): the native {arm:?} stopped: {stop:?}"),
        }
        match live::live_reconcile(&mut core, &lcx, &mut asker) {
            Ok(_) => {}
            Err(stop) if known(&stop) => break,
            Err(stop) => panic!("{faction} {card}: the reconcile stopped: {stop:?}"),
        }
    }
    steps
}

const OTHERS: [&str; 6] = ["sol", "hacan", "letnev", "xxcha", "jolnar", "l1z1x"];

#[test]
fn a_native_strategic_action_of_every_lobby_faction_and_every_standard_card_bails_nowhere() {
    let cards = ["pok1leadership", "pok2diplomacy", "pok3politics", "pok4construction", "pok5trade", "pok6warfare", "pok7technology", "pok8imperial"];
    let mut ran = 0;
    for (i, faction) in crate::seating::LOBBY_FACTIONS.iter().enumerate() {
        let others: Vec<&str> = (0..).map(|k| OTHERS[(i + k) % OTHERS.len()]).filter(|o| o != faction).take(3).collect();
        for card in cards {
            ran += native_action(faction, [others[0], others[1], others[2]], card);
        }
    }
    assert!(ran > 24 * 8, "the actions ran: {ran} steps");
}

#[test]
fn a_native_thunders_edge_construction_of_every_lobby_faction_bails_nowhere() {
    for (i, faction) in crate::seating::LOBBY_FACTIONS.iter().enumerate() {
        let others: Vec<&str> = (0..).map(|k| OTHERS[(i + k) % OTHERS.len()]).filter(|o| o != faction).take(3).collect();
        native_action(faction, [others[0], others[1], others[2]], "te4construction");
    }
}

#[allow(dead_code)]
fn unused(_: PlayerId) {}

