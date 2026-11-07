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

/// Which opener a run starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The aftermath's production step (`enter_production`).
    Tactical,
    /// A use at the home system outside a tactical action (`production::resolve_timed`).
    Home,
    /// Units an ability produces in the system (`production::produce_by_ability`: the Arborec and Muaat leaders), two at most.
    Ability,
}

/// The string run of a use at the home system: `production::resolve_timed` against a table that answers by the policy.
fn string_home(mut state: GameState, system: &SystemId, policy: &Policy) -> Run {
    let content = ContentStore::embedded();
    let asked = Rc::new(RefCell::new(Vec::new()));
    let shared = Rc::new(RefCell::new(policy.clone()));
    let mut table = Table::with_default(Box::new(Recorder { asked: asked.clone(), policy: shared }));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    let timing = Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None });
    crate::production::resolve_timed(&mut state, content, POK, None, &mut table, timing, &a(), system).expect("the string run is legal");
    let events = resolver.applied_events().iter().map(|e| (e.event_type.clone(), e.payload.clone())).collect();
    let asked = asked.borrow().clone();
    Run { asked, events, state }
}

/// The string run of an ability's production: `production::produce_by_ability` (two units), the table answering by the policy.
fn string_ability(mut state: GameState, system: &SystemId, policy: &Policy) -> Run {
    let content = ContentStore::embedded();
    let asked = Rc::new(RefCell::new(Vec::new()));
    let shared = Rc::new(RefCell::new(policy.clone()));
    let mut table = Table::with_default(Box::new(Recorder { asked: asked.clone(), policy: shared }));
    let (mut dice, mut rng) = (Dice::new(), GameRng::new(state.rng_seed));
    let mut resolver = crate::fixtures::armed_resolver(&state);
    let mut sequence = crate::event::EventSequence::new();
    {
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        crate::production::produce_by_ability(&mut state, &mut ctx, None, &a(), system, Some(2)).expect("the string run is legal");
    }
    let events = resolver.applied_events().iter().map(|e| (e.event_type.clone(), e.payload.clone())).collect();
    let asked = asked.borrow().clone();
    Run { asked, events, state }
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
fn live_run(mut gs: GameState, system: &SystemId, policy: &Policy, mode: Mode, script: Vec<String>) -> Result<Run, (Stop, Vec<Choice>)> {
    let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).expect("a position the codecs carry");
    let rng_before = GameRng::new(gs.rng_seed).positions();
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
        let mut env: Env<'_> = crate::live::env::env_with_plays(r_ref, core, kernel, POK);
        // (The live Env names the faction units of the FULL scope; this game is a POK game.)
        env.table.facts.roster = crate::technology_core::roster_for(ContentStore::embedded(), POK);
        let open = match mode {
            Mode::Tactical => tactical(seat, sys),
            Mode::Home => home_use(seat, sys),
            Mode::Ability => {
                // The facts of the use as the adapter hands them: the system's own, with the limit the ability allows.
                let roster = env.table.facts.roster.expect("a roster");
                let mut facts = ti4_engine_core::rules::production::home::home_facts(core, kernel, roster, seat, sys, false).expect("facts").produce;
                facts.limit = 2;
                UseOpen::Ability { facts }
            }
        };
        open_production(core, open);
        let live = Cx::new(r_ref, &env);
        // The string's answers as the core takes them (`multiask_dual::core_script`: a payment is ONE set), then the policy.
        struct Answers {
            script: std::collections::VecDeque<String>,
            policy: Rc<RefCell<Policy>>,
        }
        impl crate::live::Asker for Answers {
            fn ask(&mut self, q: &crate::live::Question<'_>) -> Result<String, Stop> {
                if let Some(id) = self.script.pop_front() {
                    return Ok(id);
                }
                let i = self.policy.borrow_mut().pick(&q.rendered).min(q.rendered.options.len() - 1);
                Ok(q.rendered.options[i].id.clone())
            }
        }
        let mut asker = Answers { script: script.into(), policy: shared.clone() };
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

fn without_previews(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("previews");
    }
    v
}

fn show(c: &[Choice]) -> Vec<(String, Vec<String>)> {
    c.iter().map(|q| (q.prompt.clone(), q.options.iter().map(|o| o.id.clone()).collect())).collect()
}

/// Run both and report every difference of the questions, the events and the position (empty: lockstep).
fn lockstep(label: &str, state: &GameState, system: &SystemId, policy: &Policy, mode: Mode) -> Vec<String> {
    let _guard = crate::dual::tests::serial();
    let string = match mode {
        Mode::Home => string_home(state.clone(), system, policy),
        Mode::Ability => string_ability(state.clone(), system, policy),
        Mode::Tactical => string_run(state.clone(), system, policy),
    };
    // The string's answers (the policy over its questions), as the core takes them.
    let mut replay = policy.clone();
    let answers: Vec<String> = string.asked.iter().map(|c| c.options[replay.pick(c).min(c.options.len() - 1)].id.clone()).collect();
    let script = crate::multiask_dual::core_script(&string.asked, &answers);
    let mut live = match live_run(state.clone(), system, policy, mode, script) {
        Ok(run) => run,
        Err((stop, seen)) => return vec![format!("{label}: the native run stopped: {stop:?}\nnative {:?}\nstring {:?}", show(&seen), show(&string.asked))],
    };
    let mut string = string;
    (string.asked, live.asked) = crate::multiask_dual::align(&string.asked, &answers, &live.asked);
    let mut out = Vec::new();
    // The native run must have run: the use was announced (a use of a system with a producer) unless nothing produces there.
    if string.events.iter().any(|(n, _)| n == "PRODUCTION_USED") && mode == Mode::Tactical && !live.events.iter().any(|(n, _)| n == "PRODUCTION_USED") {
        out.push(format!("{label}: the native run never announced the use"));
    }
    if live.asked.len() != string.asked.len() {
        out.push(format!("{label}: questions: string {:?} native {:?}", show(&string.asked), show(&live.asked)));
    }
    for (i, (s, n)) in string.asked.iter().zip(&live.asked).enumerate() {
        for (what, sv, nv) in [
            ("player", serde_json::json!(s.player), serde_json::json!(n.player)),
            ("prompt", serde_json::json!(s.prompt), serde_json::json!(n.prompt)),
            ("options", serde_json::json!(s.options), serde_json::json!(n.options)),
            ("details", serde_json::json!(s.details), serde_json::json!(n.details)),
            // (The `previews` of a played card's effect are lane A2's display facts of the play slot, not the production step's.)
            ("context", without_previews(serde_json::json!(s.context)), without_previews(serde_json::json!(n.context))),
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
    let diffs = lockstep("sol", &state, &system, &Policy::new(1), Mode::Tactical);
    assert!(diffs.is_empty(), "{diffs:#?}");
}

fn faction_ids() -> Vec<&'static str> {
    use crate::generated::IdStr;
    // The factions of the agreed product (Keleres, Deepwrought, Firmament, Obsidian, Ral Nel, Crimson and the Last Bastion are dropped).
    ti4_engine_core::generated::Faction::ALL
        .iter()
        .map(|f| IdStr::as_str(*f))
        .filter(|id| !["keleres", "deepwrought", "firmament", "obsidian", "ralnel", "crimson", "bastion", "neutral"].iter().any(|d| id.contains(d)))
        .collect()
}

/// Two purchases at the home system of every faction of the product: the start effects (the use counter, `production_used`, the announcement),
/// the faction hooks of the window (capacity, discounts, extra destinations), the end effects. A native bail is a failure.
#[test]
fn the_production_step_of_every_faction_runs_native_in_lockstep_with_the_string() {
    let mut failures = Vec::new();
    for (i, faction) in faction_ids().into_iter().enumerate() {
        let (state, system) = game(faction, 10 + i as u64);
        failures.extend(lockstep(faction, &state, &system, &Policy::new(2), Mode::Tactical));
        failures.extend(lockstep(&format!("{faction} (home)"), &state, &system, &Policy::new(2), Mode::Home));
        failures.extend(lockstep(&format!("{faction} (ability)"), &state, &system, &Policy::new(2), Mode::Ability));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn tech(state: &mut GameState, id: ti4_engine_core::generated::Technology) {
    use crate::generated::IdStr;
    state.player_mut(&a()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(IdStr::as_str(id)));
}

#[test]
fn sarween_tools_and_the_ai_development_algorithm_discount_the_use_in_lockstep() {
    use ti4_engine_core::generated::Technology as T;
    let (mut state, system) = game("sol", 3);
    tech(&mut state, T::SarweenTools);
    let diffs = lockstep("sarween", &state, &system, &Policy::new(2), Mode::Tactical);
    assert!(diffs.is_empty(), "{diffs:#?}");
    let (mut state, system) = game("sol", 4);
    tech(&mut state, T::AIDevelopmentAlgorithm);
    tech(&mut state, T::CruiserII);
    for prefer in [vec!["use", "exhaust"], vec!["decline"]] {
        let mut policy = Policy::new(2);
        policy.prefer = prefer;
        let diffs = lockstep("aida", &state, &system, &policy, Mode::Tactical);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }
}

#[test]
fn the_hacan_hero_makes_the_use_free_and_is_purged_in_lockstep() {
    for mode in [Mode::Tactical, Mode::Home] {
        for prefer in [vec!["use"], vec!["decline"]] {
            let (mut state, system) = game("hacan", 5);
            state.player_mut(&a()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("hacanhero"), ti4_model::state::LeaderStatus::Unlocked);
            let mut policy = Policy::new(2);
            policy.prefer = prefer.clone();
            let diffs = lockstep(&format!("hero {mode:?} {prefer:?}"), &state, &system, &policy, mode);
            assert!(diffs.is_empty(), "{diffs:#?}");
        }
    }
}

#[test]
fn the_prophecy_of_ixth_and_self_assembly_routines_end_the_use_in_lockstep() {
    use ti4_engine_core::generated::Technology as T;
    for unit in ["destroyer", "fighter"] {
        let (mut state, system) = game("sol", 6);
        state.laws.insert("prophecy".to_owned(), "a".to_owned());
        let mut policy = Policy::new(1);
        policy.unit = unit;
        let diffs = lockstep(&format!("prophecy {unit}"), &state, &system, &policy, Mode::Tactical);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }
    for prefer in [vec!["self_assembly", "mech"], vec!["decline"]] {
        let (mut state, system) = game("sol", 7);
        tech(&mut state, T::SelfAssemblyRoutines);
        let mut policy = Policy::new(1);
        policy.prefer = prefer.clone();
        let diffs = lockstep(&format!("self assembly {prefer:?}"), &state, &system, &policy, Mode::Tactical);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }
}

#[test]
fn a_system_that_produces_nothing_runs_native_with_nothing_asked() {
    let (mut state, system) = game("sol", 8);
    state.system_mut(&system).planet_units.clear();
    state.system_mut(&system).units.clear();
    let diffs = lockstep("no capacity", &state, &system, &Policy::new(1), Mode::Tactical);
    assert!(diffs.is_empty(), "{diffs:#?}");
}

#[test]
fn war_machine_played_into_the_use_adds_production_in_lockstep() {
    use crate::generated::IdStr;
    for card in [ti4_engine_core::generated::ActionCard::WarMachine1] {
        let (mut state, system) = game("sol", 9);
        state.player_mut(&a()).unwrap().action_cards.push(ti4_model::id::ActionCardId::new(IdStr::as_str(card)));
        for (label, prefer) in [("played", vec![IdStr::as_str(card), "ability"]), ("declined", vec!["decline"])] {
            let mut policy = Policy::new(3);
            policy.prefer = prefer;
            let diffs = lockstep(&format!("war machine {label}"), &state, &system, &policy, Mode::Tactical);
            assert!(diffs.is_empty(), "{diffs:#?}");
        }
    }
}
