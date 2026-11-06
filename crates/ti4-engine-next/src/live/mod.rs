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
pub mod explore;
pub mod kernel;
pub mod menu;
pub mod plays;
pub mod reaction_preview;
pub mod trade;
pub mod strategic;
pub mod turn;

use std::collections::VecDeque;

use ti4_engine_core::choice::Choice as CoreChoice;
use ti4_engine_core::driver::live::{ask_arm_fn, live_arm, Asking, AskFn, Env, Live, LiveError, LiveFn, Out};
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::wire::choice::{Choice, IllegalChoice};
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
    /// The table's decider failed or refused (`IllegalChoice::DeciderFailed`: the host changed its seats, the move was taken back; a replay
    /// diverged): the wrapper returns it as the step's error, exactly as the string did, after restoring the position.
    Asker(IllegalChoice),
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
    /// The position the question is asked in (the live core; a nested question's own position is `asking.nested` / `asking.at`).
    pub core: &'a CoreState,
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
        Self { render, env, complete: &details::complete, texts: crate::ties::combat_live::combat_texts }
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
    // (A use of PRODUCTION an ability runs inside a window is read with the production kernel, whose hooks give the numbers of its options.)
    let read_with = if matches!(asking.choice.ask, ti4_engine_core::choice::Ask::Production(_)) { crate::ties::production_view::kernel_for_core(cx.env.kernel.sources) } else { kernel::static_of(cx.env.kernel) };
    let position = render::Position { state: std::rc::Rc::new(at.clone()), kernel: read_with };
    let mut rendered = Typed::new_at(cx.render, position, asking.choice.clone(), phase, round).rendered;
    (cx.complete)(at, cx.env.kernel, &asking.choice, &mut rendered);
    // A question asked from inside an emission carries the trigger of its window and the battle / invasion it is asked in; its display
    // facts read the position it was put in (the driver rolled the step back).
    if let Some((facts, at)) = &asking.nested {
        crate::render::nested::complete_nested(cx.render, &asking.choice, facts, &crate::render::nested::NestedPos::of_core(cx.render, at), at, &mut rendered);
        crate::ties::combat_live::after_nested(&asking.choice, &mut rendered);
        reaction_preview::complete(cx.render, at, cx.env.kernel, facts, &asking.choice, &mut rendered);
    } else if matches!(asking.choice.ctx.origin, ti4_engine_core::choice::Origin::ActionCard(_)) {
        // A question of an action card played from a window (Crash Landing's planet, asked while the combat or the invasion goes on) is asked by the
        // string through `ask_seeing` too: it names the battle and the invasion the position is in.
        if let Some(ctx) = rendered.context.take() {
            rendered.context = Some(crate::render::nested::nested_position(&asking.choice, ctx, &crate::render::nested::NestedPos::of_core(cx.render, at)));
        }
    } else if let Some(ctx) = rendered.context.take() {
        // Every other question of a window hosted inside an invasion (the table's `ask_seeing` names the invasion in progress in the context of any
        // question): the core's own contexts carry it only where the area that raised the question knew it.
        rendered.context = Some(ctx.about_invasion_in(crate::render::nested::NestedPos::of_core(cx.render, at).invasion));
    }
    rendered
}

/// The pick an option id means in `choice`: what the render layer reads back, else the option whose rendered id it is.
#[must_use]
pub fn pick_of(r: &Render, choice: &CoreChoice, id: &str) -> Option<ti4_engine_core::choice::Pick> {
    use ti4_engine_core::choice::{tactical::Opt as TOpt, Opt};
    // A multi-pick (a payment, Leadership's purchase): the set the id names; the core checks it.
    // One atom's id alone means "pay with this one first, then the canonical order" (`Choice::pick`), so a client or bot that answers
    // with an option of the list still answers.
    if choice.is_multi_pick() {
        let rendered: Vec<_> = choice.opts.iter().map(|o| render::option(r, &choice.ask, o)).collect();
        if let Some(at) = rendered.iter().position(|o| o.id == id) {
            return choice.pick(at);
        }
        if let ti4_engine_core::choice::Ask::Cards(_) = choice.ask {
            // The hand limit: each part names an option, once per card of that printed name.
            if let Some(Opt::Cards(set)) = render::parse(r, &choice.ask, id) {
                return Some(ti4_engine_core::choice::Pick::Opt(Opt::Cards(set)));
            }
            let mut counts = 0u64;
            for part in id.split(crate::wire::choice::MULTI_PICK_SEPARATOR) {
                let at = rendered.iter().position(|o| o.id == part)?;
                let n = ti4_engine_core::choice::cards::discard_count(counts, at);
                counts = ti4_engine_core::choice::cards::with_discard_count(counts, at, n.checked_add(1)?);
            }
            return Some(ti4_engine_core::choice::Pick::Opt(Opt::Cards(ti4_engine_core::choice::cards::Opt::DiscardSet { counts: ti4_engine_core::choice::cards::Counts::of(counts) })));
        }
        let set = render::production::set_of(choice.opts.as_slice(), &rendered, id)?;
        return Some(ti4_engine_core::choice::Pick::Opt(Opt::Production(ti4_engine_core::choice::production::Opt::Set(set))));
    }
    let own = |id: &str| render::parse_pick(r, &choice.ask, id).or_else(|| choice.opts.iter().find(|o| render::option(r, &choice.ask, o).id == id).map(|o| ti4_engine_core::choice::Pick::Opt(*o)));
    own(id).or_else(|| {
        // `move|origin|i|rN`: route N of the option `move|origin|i` (not an option of the question; the machine checks it against its list).
        let (base, n) = crate::ties::tactical::route_suffix(id)?;
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
        return Err(Stop::NoLiveFn(arm));
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
    if result.is_err() {
        *core = before;
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
    // One code table for the render and the state: a window that mints a text straight into the state (the exploration's words) must not lose it
    // to the render's copy, and a code the render handed out must exist in the state.
    cx.render.sync_codes(&mut core.texts.codes);
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
            cx.render.sync_codes(&mut core.texts.codes);
            let held = out.held;
            merge(&mut step, out);
            match live {
                Live::Done if held => return Ok(step),
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
        let id = asker.ask(&Question { asking: &asking, rendered, core })?;
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
        cx.render.sync_codes(&mut core.texts.codes);
        let held = out.held;
        merge(&mut step, out);
        match live {
            // The string's step ends with the window open and asks nothing (its next step closes it).
            Live::Done if held => return Ok(step),
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

    use super::*;
    use crate::core_sync::{with_core, Footprint, Sources};

    #[test]
    fn a_question_asked_inside_an_invasion_names_the_invasion_in_its_context() {
        use ti4_engine_core::choice::generic::Rule;
        use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub};
        use ti4_engine_core::generated::{StrategyCard, System};
        use ti4_engine_core::state::{ActiveInvasion, Code};
        let mut gs = ti4_engine_legacy::fixtures::seated_dealt(&[("a", "mentak"), ("b", "letnev")], ti4_model::content_types::POK);
        let (mut cx, r) = crate::render::nested::tables(&gs).expect("a position the codecs carry");
        let kernel = Kernel::plain();
        let env = Env { kernel: &kernel, registry: Cow::new(Registry::default()), table: AbilityTable::default(), seeds: Rc::new(NoSeeds) };
        let reads: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| *s != Slice::Map).collect();
        with_core(&mut gs, &mut cx, Sources::default(), Footprint::new(&reads, &[Slice::Transient]), |core, _| {
            let choice = CoreChoice {
                actor: ti4_engine_core::Seat(0),
                ask: ti4_engine_core::choice::Ask::Strategy(SAsk::Draft),
                ctx: ti4_engine_core::choice::strategy::rule_ctx(Rule::R26, SSub::Draft),
                opts: [ti4_engine_core::choice::Opt::Strategy(SOpt::Card(StrategyCard::Leadership))].into_iter().collect(),
            };
            let asking = Asking { choice, nested: None, at: None };
            let live = Cx::new(&r, &env);
            let seq_of = |core: &CoreState| render_asking(&live, core, &asking).context.expect("a context").invasion_seq;
            assert_eq!(seq_of(core), None, "no invasion in progress");
            core.transient_mut().invasion.active = Some(ti4_engine_core::containers::Cow::new(ActiveInvasion {
                system: System::ALL[0],
                invader: ti4_engine_core::Seat(0),
                seq: 7,
                phase: Code(0),
                planet: None,
                defender: None,
                ground_round: 0,
                last_step: None,
            }));
            assert_eq!(seq_of(core), Some(7), "the invasion in progress");
        })
        .unwrap();
    }

    /// The arm stops with a script that ran out, and with an answer that is no option, and puts the position back each time.
    #[test]
    fn a_failed_live_step_restores_the_position_it_started_from() {
        let gs = ti4_engine_legacy::fixtures::seated_dealt(&[("a", "mentak"), ("b", "letnev")], ti4_model::content_types::POK);
        let mut gs = gs;
        let (mut cx, r) = crate::render::nested::tables(&gs).unwrap();
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

    /// Lane S: a Skilled Retreat destination is the option of the question whole -- with the ships already there -- not what the id alone gives back (a
    /// pick with `there: 0` is no option of a question that lists three ships there, and the live step refused it: winnu, 4 players, seed 4, step 416).
    #[test]
    fn the_answer_to_a_withdraw_question_is_the_option_the_question_lists() {
        use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
        use ti4_engine_core::choice::{Ask, Ctx, Opt, Origin, Pick, Subtype, Target};
        let r = Render::default();
        let [home, near, far] = [ti4_engine_core::generated::System::ALL[1], ti4_engine_core::generated::System::ALL[2], ti4_engine_core::generated::System::ALL[3]];
        let opts = [COpt::Withdraw { system: near, there: 3 }, COpt::Withdraw { system: far, there: 0 }];
        let choice = CoreChoice {
            actor: ti4_engine_core::Seat(0),
            ask: Ask::Combat(CAsk::Withdraw { system: home, fleet: 2 }),
            ctx: Ctx {
                origin: Origin::ActionCard(ti4_engine_core::generated::ActionCard::SkilledRetreat2),
                subtype: Subtype::Combat(CSub::Withdraw),
                optional: false,
                target: Some(Target::System(home)),
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: opts.iter().map(|o| Opt::Combat(*o)).collect(),
        };
        for o in opts {
            let id = render::option(&r, &choice.ask, &Opt::Combat(o)).id;
            assert_eq!(pick_of(&r, &choice, &id), Some(Pick::Opt(Opt::Combat(o))), "{id}");
        }
        assert_eq!(pick_of(&r, &choice, "no_such_system"), None);
    }
}
