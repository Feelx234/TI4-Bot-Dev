//! The native production step against the string's (lane M2, `docs/M2_RESUME.md`): `AftermathWindow::enter_production` and the production window it
//! returns (`pending_choice` / `resolve`, which ends with `finish_use`) run twice from the same position, once as the string and once as the native
//! `Arm::Production` (`open_production(Tactical)`) through `live_step`. Same questions (player, prompt, options, details, context), same answers,
//! same events (name and payload), same position.
//!
//! A child of `game` (like `cannon_prelude_tests`): `enter_production` is a private method of the aftermath window.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::Value;
use ti4_content::ContentStore;
use ti4_engine_core::driver::live::{is_open, Env};
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::driver::windows::production::{open_production, ProductionMachine, UseOpen};
use ti4_engine_core::state::{Slice, WindowKind};
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use super::{Aftermath, AftermathWindow};
use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Resolving, Table, Window as _};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{with_core, Footprint, Sources};
use crate::dice::Dice;
use crate::live::{live_step, Cx, Scripted, Stop};
use crate::rng::GameRng;

fn a() -> PlayerId {
    PlayerId::new("a")
}

/// How a run answers: a question's option index, by what is asked. Both sides get a fresh copy, so they answer alike.
#[derive(Clone)]
struct Policy {
    /// Ids (prefixes) preferred on any question, in order; then the build rule below; then the first option.
    prefer: Vec<&'static str>,
    /// How many units to buy before stopping.
    builds: u32,
    /// The unit (prefix of the option id after `build|`) to buy; any when empty.
    unit: &'static str,
}

impl Policy {
    fn new(builds: u32) -> Self {
        Self { prefer: Vec::new(), builds, unit: "" }
    }

    fn pick(&mut self, c: &Choice) -> usize {
        if c.prompt.starts_with("produce in") {
            if self.builds > 0
                && let Some(i) = c.options.iter().position(|o| o.id.strip_prefix("build|").is_some_and(|rest| rest.starts_with(self.unit)))
            {
                self.builds -= 1;
                return i;
            }
            return c.options.iter().position(|o| o.id == "done_producing").unwrap_or(0);
        }
        if let Some(i) = self.prefer.iter().find_map(|p| c.options.iter().position(|o| o.id.starts_with(p))) {
            return i;
        }
        if c.prompt.starts_with("pay ") {
            return c.options.iter().position(|o| o.id == "trade_good").unwrap_or(0);
        }
        c.options.iter().position(|o| o.id == "decline").unwrap_or(0)
    }
}

struct Recorder {
    asked: Rc<RefCell<Vec<Choice>>>,
    policy: Rc<RefCell<Policy>>,
}

impl Decider for Recorder {
    fn choose(&mut self, c: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let i = self.policy.borrow_mut().pick(c).min(c.options.len() - 1);
        self.asked.borrow_mut().push(c.clone());
        Ok(c.options[i].clone())
    }
}

type Events = Vec<(String, BTreeMap<String, Value>)>;

struct Run {
    asked: Vec<Choice>,
    events: Events,
    state: GameState,
}

fn window(state: &GameState, system: &SystemId) -> AftermathWindow {
    AftermathWindow {
        player: a(),
        system: system.clone(),
        stage: Aftermath::Done,
        log: Vec::new(),
        before_combat: crate::combat::before_combat_with_notes(state, ContentStore::embedded(), POK, system, crate::combat::note_holdings(state)),
        feats_noted: true,
        pending_event_scoring: None,
        notes_at_tactical_start: crate::combat::note_holdings(state),
    }
}

/// The string run: `enter_production` (the start effects), then the window question by question (`resolve` ends with `finish_use`).
fn string_run(mut state: GameState, system: &SystemId, policy: &Policy) -> Run {
    let content = ContentStore::embedded();
    let asked = Rc::new(RefCell::new(Vec::new()));
    let shared = Rc::new(RefCell::new(policy.clone()));
    let mut table = Table::with_default(Box::new(Recorder { asked: asked.clone(), policy: shared.clone() }));
    let (mut dice, mut rng) = (Dice::new(), GameRng::new(state.rng_seed));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    let aw = window(&state, system);
    {
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        let mut production = aw.enter_production(&mut state, &mut ctx).expect("the start effects are legal");
        let mut guard = 0;
        while let Some(choice) = production.pending_choice(&state, content, POK) {
            let i = shared.borrow_mut().pick(&choice).min(choice.options.len() - 1);
            asked.borrow_mut().push(choice.clone());
            production.resolve(&mut state, &mut ctx, choice.options[i].clone()).expect("the string run is legal");
            guard += 1;
            assert!(guard < 200, "the string production does not end");
        }
    }
    let events = resolver.applied_events().iter().map(|e| (e.event_type.clone(), e.payload.clone())).collect();
    let asked = asked.borrow().clone();
    Run { asked, events, state }
}

const FP: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger, Slice::Strategy, Slice::Trade, Slice::Explore, Slice::Decks, Slice::Objectives],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger, Slice::Objectives, Slice::Decks, Slice::Explore, Slice::Agenda],
);

/// The native run: `Arm::Production` through `live_step`, answered like the string. `Err`: the arm stopped (the bail), with the questions seen.
fn live_run(mut gs: GameState, system: &SystemId, policy: &Policy, open: fn(ti4_engine_core::ids::Seat, ti4_engine_core::generated::System) -> UseOpen) -> Result<Run, (Stop, Vec<Choice>)> {
    let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).expect("a position the codecs carry");
    let rng_before = GameRng::new(gs.rng_seed).positions();
    let seeds: Rc<dyn ti4_engine_core::state::SeedSource> = Rc::new(TextSeeds::of(&cx));
    let seat = r.seat("a").expect("a seat");
    let sys = <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(system.as_str()).expect("a system");
    let src = Sources { galaxy: None, rng: Some(&rng_before) };
    let mut asked: Vec<Choice> = Vec::new();
    let mut events: Events = Vec::new();
    let mut outcome: Result<(), Stop> = Ok(());
    let shared = Rc::new(RefCell::new(policy.clone()));
    let r_ref = &r;
    with_core(&mut gs, &mut cx, src, FP, |core, _| {
        let kernel = crate::live::kernel::kernel_for(POK);
        let env: Env<'_> = crate::live::env::env_with_plays(r_ref, core, kernel, seeds.clone(), POK);
        open_production(core, open(seat, sys));
        let live = Cx::new(r_ref, &env);
        struct Answers {
            policy: Rc<RefCell<Policy>>,
        }
        impl crate::live::Asker for Answers {
            fn ask(&mut self, q: &crate::live::Question<'_>) -> Result<String, Stop> {
                let i = self.policy.borrow_mut().pick(&q.rendered).min(q.rendered.options.len() - 1);
                Ok(q.rendered.options[i].id.clone())
            }
        }
        let mut asker = Answers { policy: shared.clone() };
        for _ in 0..200 {
            match live_step(core, Arm::Production, &live, &mut asker) {
                Ok(step) => {
                    asked.extend(step.asked);
                    for e in &step.out.emitted {
                        if let Some(ev) = crate::dual_window::event_of(r_ref, e) {
                            events.push((ev.event_type, ev.payload));
                        }
                    }
                    if !is_open::<ProductionMachine>(core) {
                        break;
                    }
                }
                Err(stop) => {
                    outcome = Err(stop);
                    break;
                }
            }
        }
        assert!(core.pending().window(WindowKind::Production).is_none() || outcome.is_err(), "the use ended");
    })
    .expect("lendable");
    let _ = Scripted::new(Vec::<String>::new());
    match outcome {
        Ok(()) => Ok(Run { asked, events, state: gs }),
        Err(stop) => Err((stop, asked)),
    }
}

fn tactical(seat: ti4_engine_core::ids::Seat, system: ti4_engine_core::generated::System) -> UseOpen {
    UseOpen::Tactical { who: seat, system }
}

fn home_use(seat: ti4_engine_core::ids::Seat, system: ti4_engine_core::generated::System) -> UseOpen {
    UseOpen::Home { who: seat, system }
}

/// Seat `a` (`faction`) active at its home system, against a second seat.
fn game(faction: &str, seed: u64) -> (GameState, SystemId) {
    let other = if faction == "hacan" { "sol" } else { "hacan" };
    let mut state = crate::fixtures::seated_dealt(&[("a", faction), ("b", other)], POK);
    state.rng_seed = seed;
    let system = state.player(&a()).and_then(|p| p.home_system.clone()).expect("a home system");
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(a());
    state.active_system = Some(system.clone());
    (state, system)
}

fn show(c: &[Choice]) -> Vec<(String, Vec<String>)> {
    c.iter().map(|q| (q.prompt.clone(), q.options.iter().map(|o| o.id.clone()).collect())).collect()
}

/// Run both and report every difference of the questions, the events and the position (empty: lockstep).
fn lockstep(label: &str, state: &GameState, system: &SystemId, policy: &Policy, open: fn(ti4_engine_core::ids::Seat, ti4_engine_core::generated::System) -> UseOpen) -> Vec<String> {
    let _guard = crate::dual::tests::serial();
    let string = string_run(state.clone(), system, policy);
    let live = match live_run(state.clone(), system, policy, open) {
        Ok(run) => run,
        Err((stop, seen)) => return vec![format!("{label}: the native run stopped: {stop:?}\nnative {:?}\nstring {:?}", show(&seen), show(&string.asked))],
    };
    let mut out = Vec::new();
    if live.asked.len() != string.asked.len() {
        out.push(format!("{label}: questions: string {:?} native {:?}", show(&string.asked), show(&live.asked)));
    }
    for (i, (s, n)) in string.asked.iter().zip(&live.asked).enumerate() {
        for (what, sv, nv) in [
            ("player", serde_json::json!(s.player), serde_json::json!(n.player)),
            ("prompt", serde_json::json!(s.prompt), serde_json::json!(n.prompt)),
            ("options", serde_json::json!(s.options), serde_json::json!(n.options)),
            ("details", serde_json::json!(s.details), serde_json::json!(n.details)),
            ("context", serde_json::json!(s.context), serde_json::json!(n.context)),
        ] {
            if sv != nv {
                out.push(format!("{label}: question {i} {what}: string {sv} native {nv}"));
            }
        }
    }
    // The events the use announced: the string's applied events are the whole timing trace, the native ones are what the window emitted; compare the
    // names the native side announced, in order, with their payloads.
    let names: std::collections::BTreeSet<&str> = live.events.iter().map(|(n, _)| n.as_str()).collect();
    let string_events: Events = string.events.iter().filter(|(n, _)| names.contains(n.as_str())).cloned().collect();
    if string_events != live.events {
        out.push(format!("{label}: events: string {string_events:?} native {:?}", live.events));
    }
    let (s, n) = (serde_json::to_value(&string.state).expect("serialises"), serde_json::to_value(&live.state).expect("serialises"));
    for d in crate::dual::diff_json(&s, &n, 30) {
        // The native window leaves the production counters and the staged events where the string keeps them; compare the rest.
        out.push(format!("{label}: state {} string={} native={}", d.path, d.left, d.right));
    }
    out
}

#[test]
fn a_plain_production_step_runs_native_with_the_questions_and_the_position_of_the_string() {
    let (state, system) = game("sol", 5);
    let diffs = lockstep("sol", &state, &system, &Policy::new(1), tactical);
    assert!(diffs.is_empty(), "{diffs:#?}");
}
