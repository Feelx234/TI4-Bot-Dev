//! The live loop's engine half (package KF2/M0, `docs/D2_LF_RESUME.md`): ONE table decision of a native arm.
//!
//! The core half (`ti4_engine_core::driver::live`) owns the windows: they are payloads on the `Pending` stack, run by a fresh
//! driver per call. This half is the part that talks to the table: it renders the core's question (`render::Typed`, the display
//! facts of [`details`]), asks an [`Asker`] (the wrapper's `Table`, a script, a bot), reads the answer back (`render::parse_pick`,
//! the option-id fallback) and applies it; a question asked from INSIDE the answer (a reaction window) is put to the same asker
//! in the same call, as the string's synchronous `ask_seeing` does. It never imports `dual*`, `Recording` or `d1_*`: it moves to
//! the wrapper crate with `render` at D2-8b.
//!
//! One call is one string `step()`: the arm's pending question is asked once, the answer applied, nested questions asked on
//! the way, and the call returns at the arm's NEXT stage question (the next step's `legal_options`) or when its window ended.
//!
//! A failed call (illegal answer, a script that ran out, a position the core does not run) restores the position the call started
//! from: the generic rollback of a step (a `CoreState` clone; the string clones `state` in `step_secondary`, `step_vote`, ...).

pub mod agenda;
pub mod arms;
pub mod coverage;
pub mod details;
pub mod env;
pub mod kernel;
pub mod menu;
pub mod plays;
pub mod trade;
pub mod strategic;
pub mod turn;

use std::collections::VecDeque;

use ti4_engine_core::choice::Choice as CoreChoice;
use ti4_engine_core::driver::live::{ask_arm_fn, live_arm, Asking, AskFn, Env, Live, LiveError, LiveFn, Out};
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::choice::Choice;
use crate::render::{self, Render, Typed};

/// Why a live call stopped without finishing the step.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// The answer is no option of the question (the string's `IllegalChoice`); the position is as before the call.
    Illegal,
    /// The asker has no answer (a replay ran out, the table is waiting for a human).
    Exhausted,
    /// The asker refuses to go on.
    Blocked,
    /// The core could not run the arm. `LiveError::Unknown` is the bail: in a live game it is a defect, never a lend.
    Live(LiveError),
    /// The arm is in the native set but has no live function (`step::<arm>`): a defect of the caller, never a lend.
    NoLiveFn(Arm),
}

impl From<LiveError> for Stop {
    fn from(e: LiveError) -> Self {
        match e {
            LiveError::Illegal => Self::Illegal,
            other => Self::Live(other),
        }
    }
}

/// A question as the table sees it.
pub struct Question<'a> {
    /// What the core asked (the typed question, and for a nested one its event and position).
    pub asking: &'a Asking,
    /// The string engine's form of it (`Choice.options[i].id` are the answers an asker gives back).
    pub rendered: Choice,
}

/// Whoever answers: the wrapper's table, a script, a bot.
pub trait Asker {
    /// The id of the chosen option.
    ///
    /// # Errors
    /// [`Stop::Exhausted`] / [`Stop::Blocked`] when there is no answer to give.
    fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop>;
}

/// The table's rule for the draft (`Game::step_flow`: `table.auto_resolve(&choice, "only one strategy card left")`, never journaled): the
/// last card is not a choice, the picker takes it and is told so. Wrap the asker of a live loop in it so that question is not put (and, in a
/// recording, not counted as a decision).
pub struct LoneCard<'a>(pub &'a mut dyn Asker);

impl Asker for LoneCard<'_> {
    fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop> {
        if q.asking.choice.ask == ti4_engine_core::choice::Ask::Strategy(ti4_engine_core::choice::strategy::Ask::Draft) && q.rendered.options.len() == 1 {
            return Ok(q.rendered.options[0].id.clone());
        }
        self.0.ask(q)
    }
}

/// A fixed list of option ids (a recorded game, a test); asking past its end is [`Stop::Exhausted`].
#[derive(Debug, Clone, Default)]
pub struct Scripted {
    ids: VecDeque<String>,
    /// The questions it was asked, as rendered, in order.
    pub seen: Vec<Choice>,
}

impl Scripted {
    #[must_use]
    pub fn new<S: Into<String>>(ids: impl IntoIterator<Item = S>) -> Self {
        Self { ids: ids.into_iter().map(Into::into).collect(), seen: Vec::new() }
    }

    /// Answers not given yet.
    #[must_use]
    pub fn left(&self) -> usize {
        self.ids.len()
    }
}

impl Asker for Scripted {
    fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop> {
        self.seen.push(q.rendered.clone());
        self.ids.pop_front().ok_or(Stop::Exhausted)
    }
}

/// What a live call renders with.
pub struct Cx<'a> {
    pub render: &'a Render,
    pub env: &'a Env<'a>,
    /// The display facts only the state knows (`Choice.details`), given the core question and the position it is asked in.
    pub complete: &'a dyn Fn(&CoreState, &Kernel, &CoreChoice, &mut Choice),
    /// The texts of the space combat at the position (`combat_live::combat_texts`), minted before every call: the core holds no text.
    pub texts: fn(&mut CoreState, &Render) -> ti4_engine_core::kernel::ability::CombatTexts,
}

impl<'a> Cx<'a> {
    /// The context with the display facts of [`details::complete`] (everything the core can say about its own questions).
    #[must_use]
    pub const fn new(render: &'a Render, env: &'a Env<'a>) -> Self {
        Self { render, env, complete: &details::complete, texts: crate::combat_live::combat_texts }
    }
}

/// What one live call did.
#[derive(Debug, Clone, PartialEq)]
pub struct StepOut {
    /// The window of the arm ended (and left the stack).
    pub finished: bool,
    /// The questions put to the asker, as rendered, in order (the decisions of the step).
    pub asked: Vec<Choice>,
    /// The events the window emitted itself and the windows the rules asked to open.
    pub out: Out,
    /// `ledger.events.len()` when the call began: the plain/typed events of the step are the queue from here
    /// (`render::events::pushed_since`).
    pub events_from: usize,
}

/// Render `asking` for the asker: the string form of the core question with its display facts.
#[must_use]
pub fn render_asking(cx: &Cx<'_>, core: &CoreState, asking: &Asking) -> Choice {
    let (phase, round) = (core.meta().phase, core.meta().round);
    // The position the question is asked in: the options of a production question carry numbers only the position gives (`Render::position`). The
    // display facts read the position the question was put in: a question asked from inside an emission or from inside a step (a job of the turn
    // flow) was put in a position the driver has rolled back since. The innermost one wins: an emission's step is nested in the job that asked.
    let at = asking.nested.as_ref().map(|(_, at)| &**at).or(asking.at.as_deref()).unwrap_or(core);
    if matches!(asking.choice.ask, ti4_engine_core::choice::Ask::Trade(_)) {
        trade::sync(cx.render, at);
    }
    let position = render::Position { state: std::rc::Rc::new(at.clone()), kernel: kernel::static_of(cx.env.kernel) };
    let mut rendered = Typed::new_at(cx.render, position, asking.choice.clone(), phase, round).rendered;
    (cx.complete)(at, cx.env.kernel, &asking.choice, &mut rendered);
    details::infiltrate_preview(cx.render, at, cx.env.kernel, cx.env.table.facts.plays_provider.as_ref(), &asking.choice, &mut rendered);
    // A question asked from inside an emission carries the trigger of its window and the battle / invasion it is asked in; its display
    // facts read the position it was put in (the driver rolled the step back).
    if let Some((facts, at)) = &asking.nested {
        crate::dual_window::complete_nested(cx.render, &asking.choice, facts, &crate::dual_window::NestedPos::of_core(cx.render, at), at, &mut rendered);
        crate::combat_live::after_nested(&asking.choice, &mut rendered);
    }
    rendered
}

/// The pick an option id means in `choice`: what the render layer reads back, else the option whose rendered id it is.
#[must_use]
pub fn pick_of(r: &Render, choice: &CoreChoice, id: &str) -> Option<ti4_engine_core::choice::Pick> {
    use ti4_engine_core::choice::{tactical::Opt as TOpt, Opt};
    let own = |id: &str| render::parse_pick(r, &choice.ask, id).or_else(|| choice.opts.iter().find(|o| render::option(r, &choice.ask, o).id == id).map(|o| ti4_engine_core::choice::Pick::Opt(*o)));
    own(id).or_else(|| {
        // `move|origin|i|rN`: route N of the option `move|origin|i` (not an option of the question; the machine checks it against its list).
        let (base, n) = crate::tactical::route_suffix(id)?;
        let route = u8::try_from(n).ok()?;
        match own(base)? {
            ti4_engine_core::choice::Pick::Opt(Opt::Tactical(TOpt::Move { origin, index, kind, damaged, capacity, gd, ionian, .. })) => {
                Some(ti4_engine_core::choice::Pick::Opt(Opt::Tactical(TOpt::Move { origin, index, kind, damaged, capacity, gd, ionian, route })))
            }
            _ => None,
        }
    })
}

/// One string `step()` of the native `arm`: ask its pending question (or run the window to its first one), apply the answer, put
/// the nested questions of the answer to the same asker, and stop at the next question of the arm or at its end.
///
/// # Errors
/// [`Stop`]; the position is then exactly what it was.
pub fn live_step(core: &mut CoreState, arm: Arm, cx: &Cx<'_>, asker: &mut dyn Asker) -> Result<StepOut, Stop> {
    let (Some(advance), Some(ask)) = (live_arm(arm), ask_arm_fn(arm)) else {
        let stop = Stop::NoLiveFn(arm);
        crate::dual::note_bail(&format!("step::{}", arms::name(arm)), "no live function");
        return Err(stop);
    };
    run_live(core, arms::name(arm), advance, ask, cx, asker)
}

/// The reconcile hooks every step starts with (`Game::step_flow` before the arm is chosen): the events effects staged, the gains no site
/// announced, the nexus latch, Styx's point, the Spec Ops rolls. Run it before [`live_step`], with the same asker: an emission here can
/// open a reaction window.
///
/// # Errors
/// As [`live_step`].
pub fn live_reconcile(core: &mut CoreState, cx: &Cx<'_>, asker: &mut dyn Asker) -> Result<StepOut, Stop> {
    run_live(core, "reconcile", ti4_engine_core::driver::turn::advance_reconcile, ti4_engine_core::driver::turn::ask_job, cx, asker)
}

/// [`live_step`] over the two functions of an arm (or of a job that belongs to no arm), `name` for the census.
fn run_live(core: &mut CoreState, name: &'static str, advance: LiveFn, ask: AskFn, cx: &Cx<'_>, asker: &mut dyn Asker) -> Result<StepOut, Stop> {
    let before = core.clone();
    let result = live_step_inner(core, advance, ask, cx, asker);
    // The string stages an event for the next step's announcement only while some seated faction has a module (`supply::stage_event` is a
    // no-op otherwise: nothing else listens). The core's windows stage unconditionally; at a table without a module the staged events are
    // dropped with the step, so none of them is ever announced (a `TRADE_GOODS_GAINED` or a `STRATEGY_TOKEN_SPENT` the string never made).
    if result.is_ok() && !cx.env.table.facts.staging {
        core.pending_mut().staged.clear();
    }
    if let Err(stop) = &result {
        *core = before;
        // A position the core does not run is a bail of the live path (the zero-bail acceptance counts it, `TI4_BAIL_IS_FAILURE`).
        if let Stop::Live(LiveError::Unknown(why)) = stop {
            crate::dual::note_bail(&format!("live::{name}"), &format!("{why:?}"));
        }
    }
    if result.is_ok() {
        coverage::note(name);
    }
    result
}

fn live_step_inner(core: &mut CoreState, advance: LiveFn, ask: AskFn, cx: &Cx<'_>, asker: &mut dyn Asker) -> Result<StepOut, Stop> {
    // The environment of this call: the game's, with the texts of the space combat at the position.
    let mut prepared = cx.env.clone();
    prepared.table.facts.combat = (cx.texts)(core, cx.render);
    let env = &prepared;
    let events_from = core.ledger.events.len();
    let mut step = StepOut { finished: false, asked: Vec::new(), out: Out::default(), events_from };
    let merge = |step: &mut StepOut, out: Out| {
        step.out.emitted.extend(out.emitted);
        step.out.opened.extend(out.opened);
    };
    // The arm's pending question; a fresh window (or one about to end) asks nothing yet: run it.
    let mut asking = match ask(core, env)? {
        Some(a) => a,
        None => {
            let (live, out) = advance(core, env, None)?;
            merge(&mut step, out);
            match live {
                Live::Done => {
                    step.finished = true;
                    return Ok(step);
                }
                Live::Asks(a) => *a,
            }
        }
    };
    loop {
        let rendered = render_asking(cx, core, &asking);
        step.asked.push(rendered.clone());
        let id = asker.ask(&Question { asking: &asking, rendered })?;
        // A free-form deal arrives as text: its terms are written to the window before the pick that names them is applied.
        let named = trade::pick(core, &asking.choice, &id);
        let pick = match named {
            Some(Ok(pick)) => pick,
            Some(Err(why)) => return Err(Stop::Live(LiveError::Unknown(why))),
            None => match pick_of(cx.render, &asking.choice, &id) {
                Some(pick) => pick,
                None => return Err(Stop::Illegal),
            },
        };
        let (live, out) = advance(core, env, Some(pick))?;
        merge(&mut step, out);
        match live {
            Live::Done => {
                step.finished = true;
                return Ok(step);
            }
            // A question from inside the answer belongs to this step (the string asks it synchronously: the reaction windows of an
            // emission, the free windows a job of the turn flow composes; the step is suspended in `Pending::nested`); a stage question
            // of the arm is the next step's.
            Live::Asks(next) if next.nested.is_some() || core.pending().nested.is_some() => asking = *next,
            Live::Asks(_) => return Ok(step),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ti4_engine_core::containers::{Cow, SmallList};
    use ti4_engine_core::driver::live::{open, Env};
    use ti4_engine_core::driver::NoSeeds;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Registry};
    use ti4_engine_core::rules::token_gain::TokenGain;
    use ti4_engine_core::state::{Slice, StatusReport, Tokens};
    use ti4_model::id::PlayerId;

    use super::*;
    use crate::core_sync::{with_core, Footprint, Sources};
    use crate::tokens::{with_pool_details, TokenGain as StringGain};

    /// The scripted answers, then the first option of whatever the close of the window asks.
    struct FirstAfter(Scripted);

    impl Asker for FirstAfter {
        fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop> {
            if self.0.left() > 0 {
                return self.0.ask(q);
            }
            // A redistribution keeps the sheet as it stands, so the pools can still be compared with the string's.
            let pools = q.rendered.details.get("pools").cloned();
            let keep = pools.and_then(|p| {
                let n = |k: &str| p.get(k).and_then(serde_json::Value::as_i64);
                Some(format!("{}|{}|{}", n("tactic")?, n("fleet")?, n("strategic")?))
            });
            Ok(keep.filter(|id| q.rendered.options.iter().any(|o| o.id == *id)).unwrap_or_else(|| q.rendered.options[0].id.clone()))
        }
    }

    /// The vertical slice of the live loop: `Arm::Tokens` run natively end to end against the string engine's own window
    /// (`tokens::TokenGain`, `Game::step_token_gain`). Same questions (prompt, option ids, kinds, labels, context, display
    /// facts), same answers, same positions, same plain events, and the window leaves the stack when the string's `is_complete`.
    #[test]
    fn the_token_gain_arm_run_by_the_core_asks_what_the_string_asks_and_leaves_the_same_position() {
        let mut gs = crate::fixtures::seated_dealt(&[("a", "mentak"), ("b", "letnev")], ti4_model::content_types::POK);
        let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).expect("a position the codecs carry");
        let order = [PlayerId::new("a"), PlayerId::new("b")];
        // Seat a places two (it keeps one in reinforcements to spare), seat b one; Letnev's hint shows on b.
        let answers = ["fleet_tokens", "tactic_tokens", "strategic_tokens"];
        let owed = [0u8, 0, 1];

        // The string run.
        let mut string_state = gs.clone();
        let mut window = StringGain::from_parts(vec![order[0].clone(), order[0].clone(), order[1].clone()], Vec::new());
        let mut string_questions = Vec::new();
        let mut string_events = Vec::new();
        for id in answers {
            let choice = window.pending_choice().expect("a token is owed");
            let actor = choice.player.clone();
            let choice = choice.contextualized(crate::decision_context::DecisionContext::new(
                actor,
                crate::decision_context::DecisionSource::Rule("52.4".to_owned()),
                "gain_command_token",
                string_state.phase,
                string_state.round,
            ));
            string_questions.push(with_pool_details(choice, &string_state, "gain", Some(window.remaining_for_next_player())));
            let option = string_questions.last().unwrap().options.iter().find(|o| o.id == id).unwrap().clone();
            let pool = window.resolve(&mut string_state, option).expect("legal");
            string_events.push(format!("COMMAND_TOKEN_GAINED:{pool:?}"));
        }
        assert!(window.is_complete());

        // The live run, one `live_step` per string step.
        let kernel = Kernel::plain();
        let env = Env { kernel: &kernel, registry: Cow::new(Registry::default()), table: AbilityTable::default(), seeds: Rc::new(NoSeeds) };
        let render = &r;
        let reads: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| *s != Slice::Map).collect();
        let fp = Footprint::new(&reads, &[Slice::Players, Slice::Pending]);
        let mut live_questions = Vec::new();
        let mut emitted = Vec::new();
        let mut finished = Vec::new();
        with_core(&mut gs, &mut cx, Sources::default(), fp, |core, _| {
            let seats: Vec<_> = owed.iter().map(|i| ti4_engine_core::Seat(*i)).collect();
            open(core, &TokenGain::new(Tokens { pending: SmallList::from_slice(&seats), placed: SmallList::new(), report: StatusReport::default() }));
            let live = Cx::new(render, &env);
            for id in answers {
                let mut asker = FirstAfter(Scripted::new([id]));
                let step = live_step(core, Arm::Tokens, &live, &mut asker).expect("the arm runs");
                live_questions.extend(asker.0.seen);
                finished.push(step.finished);
            }
            emitted = core.ledger.events.iter().cloned().collect::<Vec<_>>();
        })
        .expect("lendable");
        let live_events: Vec<String> =
            emitted.iter().map(|e| crate::render::events::pushed_of(render, &cx, e).expect("every event has a text").text().to_owned()).collect();

        // The last token also runs the close (the redistributions, answered with their first option): what the string asks comes first.
        live_questions.truncate(string_questions.len());
        for (live, string) in live_questions.iter().zip(&string_questions) {
            assert_eq!(live.player, string.player);
            assert_eq!(live.prompt, string.prompt);
            assert_eq!(live.options, string.options);
            assert_eq!(live.details, string.details);
            assert_eq!(live.context, string.context);
        }
        assert_eq!(live_events[..string_events.len()], string_events, "the plain events, in order (the close's follow)");
        assert_eq!(finished, [false, false, true], "the window ends with the last token, as `is_complete`");
        // Position: the tokens landed in the pools the string put them in.
        for who in &order {
            let (a, b) = (gs.player(who).unwrap(), string_state.player(who).unwrap());
            assert_eq!((a.tactic_tokens, a.fleet_tokens, a.strategic_tokens), (b.tactic_tokens, b.fleet_tokens, b.strategic_tokens), "{who:?}");
        }
    }

    /// The arm stops with a script that ran out, and with an answer that is no option, and puts the position back each time.
    #[test]
    fn a_failed_live_step_restores_the_position_it_started_from() {
        let gs = crate::fixtures::seated_dealt(&[("a", "mentak"), ("b", "letnev")], ti4_model::content_types::POK);
        let mut gs = gs;
        let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).unwrap();
        let kernel = Kernel::plain();
        let env = Env { kernel: &kernel, registry: Cow::new(Registry::default()), table: AbilityTable::default(), seeds: Rc::new(NoSeeds) };
        let reads: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| *s != Slice::Map).collect();
        with_core(&mut gs, &mut cx, Sources::default(), Footprint::new(&reads, &[Slice::Players, Slice::Pending]), |core, _| {
            open(core, &TokenGain::new(Tokens { pending: SmallList::from_slice(&[ti4_engine_core::Seat(0)]), placed: SmallList::new(), report: StatusReport::default() }));
            let live = Cx::new(&r, &env);
            let held = core.clone();
            assert_eq!(live_step(core, Arm::Tokens, &live, &mut Scripted::new(Vec::<String>::new())), Err(Stop::Exhausted));
            assert_eq!(*core, held);
            assert_eq!(live_step(core, Arm::Tokens, &live, &mut Scripted::new(["no_such_pool"])), Err(Stop::Illegal));
            assert_eq!(*core, held);
            assert_eq!(live_step(core, Arm::Diplomacy, &live, &mut Scripted::new(["x"])), Err(Stop::NoLiveFn(Arm::Diplomacy)));
            assert_eq!(*core, held);
        })
        .unwrap();
    }
}
