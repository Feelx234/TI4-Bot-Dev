//! Lane C0 of `docs/FORK_RNG_THEORY.md` (tests T1 and T2 of 1.6, the draw census of 3.2); results in `docs/C0_RESUME.md`.
//!
//! The games are played DIRECTLY through the core live API (`driver::live::{ask, advance}` of each arm), as `perf_kernel` does: no thin
//! `Game`, no string mirror, no table. What the driver below keeps between two core calls is only what a caller of the live API has to
//! keep anyway: the arm it runs, the `Env` of the step and of the run, the bot's counters, and the step's `after_glue` (V1 of the theory:
//! the caller's, not the core's). A fork is `CoreState::clone` plus a clone of that caller state, with a COLD `Render` rebuilt from the
//! code table (what `NativeGame::fork` does), never a shared one.
//!
//! * T1: at chosen questions (nested ones, ones right after a die was rolled in the same step, stage questions, step boundaries after a
//!   roll), the game is forked. The original and a copy go on with the same answers and must reach the same position as a game that was
//!   never forked (`CoreState ==`: board, players, decks, the streams, the dice and event ledger, the pending stack), with the same
//!   answers, the same question at the fork (re-derived by `ask` from the copy alone) and the same events out of every `advance`. A second
//!   copy goes on with different answers: it must leave the original untouched.
//! * T2: at quiet step boundaries (no open window, no nested question), the position is stored into the string `GameState` and loaded
//!   back (`Codec`), with the resolver and the stream positions handed in, as the product does; the loaded core must equal the original
//!   and play on identically. The positions the codec cannot carry (an open window, a nested question: V5) are pinned as such.
//! * The census (`ti4_engine_core::state::census`, feature `probe-counters`, on for this crate's tests only): draws made against draws
//!   committed, and the rollbacks they were made in. `cargo test -p ti4-engine-core-bridge --test fork_core -- --nocapture` prints it.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use ti4_engine::live;
use ti4_engine::render::Render;
use ti4_engine_core::choice::{Ask as CAsk, Choice, Opt, Pick};
use ti4_engine_core::driver::live::{ask_arm_fn, is_open, live_arm, AskFn, Asking, Env, Live, LiveFn};
use ti4_engine_core::driver::session::{live_ask_arm, step_arm, Arm, AskArm, Facts};
use ti4_engine_core::driver::turn::{self, TurnFlowWin};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{census, CoreState, RngDomain, SeedSource, WindowKind};
use ti4_engine_core_bridge::native_play::{lobby_spec, NativeGame};
use ti4_model::content_types::SourceSet;

/// splitmix64 (the bench's).
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn hash_of<T: Hash>(chain: u64, x: &T) -> u64 {
    let mut h = DefaultHasher::new();
    (chain, x).hash(&mut h);
    h.finish()
}

/// Where in a step the driver stopped: the reconcile hooks (at their `round`-th run) or the arm.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Part {
    Reconcile { round: u8, left_open: bool },
    Arm { arm: Arm, asked_before: usize },
}

/// A step stopped at a question, before its answer: what the caller holds besides the core.
#[derive(Clone)]
struct Mid {
    part: Part,
    /// The step's `Env` (built from the position the step started at).
    step_env: Env<'static>,
    /// The run's `Env` (the step's, with the combat texts of the position the run started at).
    run_env: Env<'static>,
    /// The question the original was asked (the copy re-derives it with `ask` and must find the same).
    choice: Choice,
    nested: bool,
}

enum RunEnd {
    /// The window finished (`true`) or stopped at the arm's next stage question (`false`).
    Ran(bool),
    Stopped(Box<Mid>),
}

enum Stepped {
    Over,
    Next,
    Stopped(Box<Mid>),
}

/// What the census sees around one decision: whether the step had rolled a die before it, and whether it was nested.
#[derive(Debug, Clone, Copy)]
struct Seen {
    step: usize,
    nested: bool,
    rolled_before: bool,
}

/// The harness-level split of the draws (the core's census splits the `advance` side further).
#[derive(Debug, Default, Clone, Copy)]
struct Calls {
    /// Draws inside `ask` (R4: a nested question re-derived on a scratch clone; all discarded).
    ask_draws: u64,
    /// Draws inside `advance` calls that returned `Ok`, and those of the calls that failed (rolled back whole, R5).
    advance_draws: u64,
    failed_advances: u64,
    failed_draws: u64,
}

/// A game on the core live API, the bot of `perf_kernel` (seeded weighted random pick over the typed options).
struct Play {
    core: CoreState,
    render: Render,
    ids: Vec<String>,
    sources: SourceSet,
    seeds: Rc<dyn SeedSource>,
    kernel: &'static Kernel,
    /// The bot's seed: a copy with another seed answers differently.
    seed: u64,
    decided: usize,
    steps: usize,
    /// The answers given (actor, options, pick), chained.
    chain: u64,
    /// Every `advance`'s output (`Out::emitted`, `Out::journal`, `Out::opened`), chained.
    outs: u64,
    after_glue: bool,
    rolled_in_step: bool,
    /// A die was committed since the last answer (the next question is the first one after a roll).
    rolled_since_answer: bool,
    /// Per decision index, what the census saw (filled while `record` is on).
    seen: Vec<Seen>,
    /// The step boundaries (steps played) right after a step that committed a die (filled while `record` is on).
    rolled_steps: Vec<usize>,
    record: bool,
    calls: Calls,
}

fn weight(o: &Opt) -> u64 {
    if matches!(o, Opt::Turn(ti4_engine_core::choice::turn::Opt::Open { .. })) { 10 } else { 100 }
}

/// Whether answering `choice` is a decision of the bot (`Play::decided` counts it): the lone card of the draft is not.
fn counts(choice: &Choice) -> bool {
    let opts = choice.opts.as_slice();
    !opts.is_empty() && !(opts.len() == 1 && choice.ask == CAsk::Strategy(ti4_engine_core::choice::strategy::Ask::Draft))
}

fn positions(core: &CoreState) -> Vec<(RngDomain, u128)> {
    core.rng.positions().collect()
}

impl Play {
    fn new(players: usize, seed: u64, faction: &str) -> Self {
        let g = NativeGame::from_spec(&lobby_spec(faction, players, seed)).expect("the lobby game builds");
        let (core, cx, render, _base, kernel, seeds, sources) = g.probe();
        let ids = cx.owners.ids().to_vec();
        let render = Self::cold_render(&ids, render);
        Self { core: core.clone(), render, ids, sources, seeds, kernel, seed, decided: 0, steps: 0, chain: 0xcbf2_9ce4_8422_2325, outs: 0, after_glue: false, rolled_in_step: false, rolled_since_answer: false, seen: Vec::new(), rolled_steps: Vec::new(), record: false, calls: Calls::default() }
    }

    /// A render that knows nothing but the code table of `r` (the memos are rebuilt on demand).
    fn cold_render(ids: &[String], r: &Render) -> Render {
        let mut fresh = Render::new(ids.to_vec()).with_codes(r.codes_snapshot());
        fresh.factions.clone_from(&r.factions);
        fresh
    }

    /// The copy a search would take: the core cloned, the caller's counters copied, a cold render.
    fn fork(&self) -> Self {
        Self {
            core: self.core.clone(),
            render: Self::cold_render(&self.ids, &self.render),
            ids: self.ids.clone(),
            sources: self.sources,
            seeds: Rc::clone(&self.seeds),
            kernel: self.kernel,
            seed: self.seed,
            decided: self.decided,
            steps: self.steps,
            chain: self.chain,
            outs: self.outs,
            after_glue: self.after_glue,
            rolled_in_step: self.rolled_in_step,
            rolled_since_answer: self.rolled_since_answer,
            seen: Vec::new(),
            rolled_steps: Vec::new(),
            record: false,
            calls: Calls::default(),
        }
    }

    fn pick(&mut self, choice: &Choice) -> Option<Pick> {
        let opts = choice.opts.as_slice();
        if opts.len() == 1 && choice.ask == CAsk::Strategy(ti4_engine_core::choice::strategy::Ask::Draft) {
            return Some(Pick::Opt(opts[0]));
        }
        if opts.is_empty() {
            return None;
        }
        let total: u64 = opts.iter().map(weight).sum();
        let mut roll = mix(self.seed ^ mix(self.decided as u64 ^ 0xA5A5_5A5A_0000_0000) ^ mix(u64::from(choice.actor.0) ^ 0x5EA7_0000_0000_0000)) % total;
        let mut at = opts.len() - 1;
        for (i, o) in opts.iter().enumerate() {
            let w = weight(o);
            if roll < w {
                at = i;
                break;
            }
            roll -= w;
        }
        self.decided += 1;
        self.rolled_since_answer = false;
        self.chain = hash_of(self.chain, &(choice.actor.0, opts.len(), opts[at]));
        // A multi-pick (a payment, a purchase, a discard, a removal): the option leads the set the question takes (`Choice::pick`).
        choice.pick(at).or(Some(Pick::Opt(opts[at])))
    }

    fn ask(&mut self, ask: AskFn, env: &Env<'_>) -> Result<Option<Asking>, String> {
        let mark = census::mark();
        let r = ask(&self.core, env).map_err(|e| format!("ask: {e:?}"));
        self.calls.ask_draws += census::mark() - mark;
        r
    }

    fn advance(&mut self, advance: LiveFn, env: &Env<'_>, pick: Option<Pick>) -> Result<(Live, bool), String> {
        let mark = census::mark();
        let before = positions(&self.core);
        match advance(&mut self.core, env, pick) {
            Ok((live, out)) => {
                self.calls.advance_draws += census::mark() - mark;
                if positions(&self.core) != before {
                    self.rolled_in_step = true;
                    self.rolled_since_answer = true;
                }
                self.outs = hash_of(self.outs, &format!("{:?}{:?}{:?}", out.emitted, out.journal, out.opened));
                Ok((live, out.held))
            }
            Err(e) => {
                self.calls.failed_advances += 1;
                self.calls.failed_draws += census::mark() - mark;
                Err(format!("advance({pick:?}): {e:?}"))
            }
        }
    }

    /// `live::run_live` of the thin layer without rendering: the arm's pending question (or the window run to its first), the picks, the
    /// nested questions, up to the arm's next stage question or its end. Stops before answering decision `stop` (`resume` goes on). With
    /// `resumed`, the run continues a stopped one: its `Env` is the stopped run's and its question is re-derived from the core alone.
    fn run_live(&mut self, advance: LiveFn, ask: AskFn, env: &Env<'static>, render: &Render, part: Part, resumed: Option<&Mid>, stop: Option<usize>) -> Result<RunEnd, String> {
        let prepared = match resumed {
            Some(mid) => mid.run_env.clone(),
            None => {
                let lcx = live::Cx::new(render, env);
                let mut prepared = env.clone();
                prepared.table.facts.combat = (lcx.texts)(&mut self.core, render);
                render.sync_codes(&mut self.core.texts.codes);
                prepared
            }
        };
        let env = &prepared;
        let first = self.ask(ask, env)?;
        if let Some(mid) = resumed {
            // The claim under test: the copy alone says what was asked.
            let again = first.as_ref().ok_or("a resumed run asks nothing")?;
            if again.choice != mid.choice {
                return Err(format!("the resumed run asks {:?}, the original was asked {:?}", again.choice, mid.choice));
            }
            if again.nested.is_some() != mid.nested {
                return Err("the resumed question is not nested as the original was".to_owned());
            }
        }
        let mut asking = match first {
            Some(a) => a,
            None => {
                let (live, held) = self.advance(advance, env, None)?;
                render.sync_codes(&mut self.core.texts.codes);
                match live {
                    Live::Done if held => return Ok(RunEnd::Ran(false)),
                    Live::Done => return Ok(RunEnd::Ran(true)),
                    Live::Asks(a) => *a,
                }
            }
        };
        let mut first_question = resumed.is_some();
        loop {
            let nested = asking.nested.is_some() || self.core.pending().nested.is_some();
            if !first_question && counts(&asking.choice) {
                if stop == Some(self.decided) {
                    return Ok(RunEnd::Stopped(Box::new(Mid { part, step_env: Env::clone(&prepared), run_env: prepared.clone(), choice: asking.choice.clone(), nested: asking.nested.is_some() })));
                }
                if self.record {
                    self.seen.push(Seen { step: self.steps, nested, rolled_before: self.rolled_since_answer });
                }
            }
            first_question = false;
            let Some(pick) = self.pick(&asking.choice) else { return Err("a question with no option".to_owned()) };
            let (live, held) = self.advance(advance, env, Some(pick))?;
            render.sync_codes(&mut self.core.texts.codes);
            match live {
                Live::Done if held => return Ok(RunEnd::Ran(false)),
                Live::Done => return Ok(RunEnd::Ran(true)),
                Live::Asks(next) if next.nested.is_some() || self.core.pending().nested.is_some() => asking = *next,
                Live::Asks(_) => return Ok(RunEnd::Ran(false)),
            }
        }
    }

    fn asks(&self) -> bool {
        if is_open::<TurnFlowWin>(&self.core) {
            return true;
        }
        let facts = Facts::of_core(&self.core, false);
        match live_ask_arm(&facts) {
            AskArm::None | AskArm::Nothing => false,
            AskArm::Draft => turn::draft::question(&self.core).is_some(),
            AskArm::StartingTech | AskArm::LeaderFollowers | AskArm::EventScoring | AskArm::Scoring | AskArm::Tokens | AskArm::Voting | AskArm::Diplomacy | AskArm::Aftermath | AskArm::AgendaTalks | AskArm::Tactical => true,
            AskArm::Secondary | AskArm::Closing | AskArm::Trade | AskArm::ActionMenu => self.core.meta().active.is_some() && !ti4_engine_core::driver::turn_flow::all_passed(&self.core),
        }
    }

    /// One step (`perf_kernel`'s `step_with`), or the rest of a stopped one (`from`). Stops before decision `stop`.
    fn step(&mut self, from: Option<Box<Mid>>, stop: Option<usize>) -> Result<Stepped, String> {
        let render = std::mem::take(&mut self.render);
        let r = self.step_with(&render, from, stop);
        self.render = render;
        r
    }

    fn step_with(&mut self, render: &Render, from: Option<Box<Mid>>, stop: Option<usize>) -> Result<Stepped, String> {
        let env: Env<'static> = match &from {
            Some(mid) => mid.step_env.clone(),
            None => {
                self.rolled_in_step = false;
                live::env::env_with_plays(render, &self.core, self.kernel, self.seeds.clone(), self.sources)
            }
        };
        let mut resumed = from;
        // 1. The reconcile hooks.
        let start_round = match resumed.as_deref().map(|m| m.part) {
            Some(Part::Reconcile { round, .. }) => Some(round),
            Some(Part::Arm { .. }) => None,
            None => Some(0),
        };
        if let Some(start) = start_round {
            for round in start..8 {
                if self.after_glue {
                    break;
                }
                let (left_open, mid) = match resumed.take() {
                    Some(mid) => match mid.part {
                        Part::Reconcile { left_open, .. } => (left_open, Some(mid)),
                        Part::Arm { .. } => unreachable!(),
                    },
                    None => (is_open::<TurnFlowWin>(&self.core), None),
                };
                let part = Part::Reconcile { round, left_open };
                let fin = match self.run_live(turn::advance_reconcile, turn::ask_job, &env, render, part, mid.as_deref(), stop)? {
                    RunEnd::Ran(fin) => fin,
                    RunEnd::Stopped(mut mid) => {
                        mid.step_env = env.clone();
                        return Ok(Stepped::Stopped(mid));
                    }
                };
                if fin && !left_open {
                    break;
                }
            }
        }
        // 2. The arm.
        let (arm, asked_before, mid) = match resumed.take() {
            Some(mid) => match mid.part {
                Part::Arm { arm, asked_before } => (arm, asked_before, Some(mid)),
                Part::Reconcile { .. } => unreachable!(),
            },
            None => {
                let facts = Facts::of_core(&self.core, false);
                let arm = step_arm(&facts, self.asks());
                if arm == Arm::Finished {
                    return Ok(Stepped::Over);
                }
                (arm, self.decided, None)
            }
        };
        let (Some(advance), Some(ask)) = (live_arm(arm), ask_arm_fn(arm)) else { return Err(format!("arm {arm:?} has no live function")) };
        let finished = match self.run_live(advance, ask, &env, render, Part::Arm { arm, asked_before }, mid.as_deref(), stop)? {
            RunEnd::Ran(fin) => fin,
            RunEnd::Stopped(mut mid) => {
                mid.step_env = env.clone();
                return Ok(Stepped::Stopped(mid));
            }
        };
        if !env.table.facts.staging {
            self.core.pending_mut().staged.clear();
        }
        // 3. The hand-overs between the arms.
        if finished && arm == Arm::Tactical && self.core.pending().window(WindowKind::Tactical).is_none() && self.core.pending().window(WindowKind::Aftermath).is_none() && turn::open_job_of(&self.core).is_none() {
            turn::open_finish(&mut self.core);
        }
        self.steps += 1;
        if self.record && self.rolled_in_step {
            self.rolled_steps.push(self.steps);
        }
        let asked_none = self.decided == asked_before;
        let handed_off = arm == Arm::Tactical && asked_none && self.core.pending().window(WindowKind::Aftermath).is_some();
        let parked_for_scoring = self.core.pending().window(WindowKind::EventScoring).is_some();
        let removal_next = !finished
            && arm == Arm::Aftermath
            && ti4_engine_core::driver::windows::chain::ask_aftermath(&self.core, &env)
                .ok()
                .flatten()
                .is_some_and(|asking| matches!(asking.choice.ask, ti4_engine_core::choice::Ask::Tactical(ti4_engine_core::choice::tactical::Ask::Remove { .. })));
        let glued = matches!(arm, Arm::TurnPrelude | Arm::StrategyAnnounce);
        self.after_glue = !parked_for_scoring && (removal_next || (finished && (glued || arm == Arm::Aftermath || handed_off || (self.after_glue && arm == Arm::Tactical && asked_none))));
        Ok(Stepped::Next)
    }

    /// Play until the game ends or `until` steps were played (counting from the start of the game). A stopped step (`from`) is finished first.
    fn play_until(&mut self, mut from: Option<Box<Mid>>, until: usize) -> Result<bool, String> {
        while self.steps < until {
            match self.step(from.take(), None)? {
                Stepped::Over => return Ok(true),
                Stepped::Next => {}
                Stepped::Stopped(_) => unreachable!("no stop was asked"),
            }
        }
        Ok(false)
    }

    /// Play until decision `at` is about to be answered.
    fn play_to_decision(&mut self, at: usize) -> Result<Box<Mid>, String> {
        loop {
            match self.step(None, Some(at))? {
                Stepped::Stopped(mid) => return Ok(mid),
                Stepped::Next => {}
                Stepped::Over => return Err(format!("the game ended before decision {at}")),
            }
        }
    }

    /// The position and the caller's record as one comparable value.
    fn result(&self) -> Snap {
        Snap { core: self.core.clone(), decided: self.decided, steps: self.steps, chain: self.chain, outs: self.outs, after_glue: self.after_glue }
    }
}

#[derive(Clone, PartialEq)]
struct Snap {
    core: CoreState,
    decided: usize,
    steps: usize,
    chain: u64,
    outs: u64,
    after_glue: bool,
}

impl Snap {
    /// Which parts differ (for the failure message).
    fn diff(&self, other: &Self) -> String {
        format!(
            "core fields {:?} ({}); decided {} vs {}; steps {} vs {}; answers equal {}; outputs equal {}; after_glue {} vs {}",
            fields_differing(&self.core, &other.core),
            pending_differing(&self.core, &other.core).join("; "),
            self.decided,
            other.decided,
            self.steps,
            other.steps,
            self.chain == other.chain,
            self.outs == other.outs,
            self.after_glue,
            other.after_glue
        )
    }
}

/// The fields of two cores that differ (`pending` split into its own fields).
fn fields_differing(a: &CoreState, b: &CoreState) -> Vec<&'static str> {
    let mut out = Vec::new();
    macro_rules! cmp {
        ($($f:ident),*) => {$( if a.$f != b.$f { out.push(stringify!($f)); } )*};
    }
    cmp!(meta, seats, players, board, map, decks, cards, objectives, agenda, strategy, explore, trade, marks, transient, rng, ledger, texts, owners, loaded);
    let (p, q) = (&a.pending, &b.pending);
    macro_rules! cmp_pending {
        ($($f:ident),*) => {$( if p.$f != q.$f { out.push(concat!("pending.", stringify!($f))); } )*};
    }
    cmp_pending!(staged, stack, top, tag, driver, turn_closing, actions_this_turn, prepared_turn_seq, resolver, failed_component_actions, failed_keys, nested);
    out
}

/// The values of the `pending` fields that differ (`a` vs `b`).
fn pending_differing(a: &CoreState, b: &CoreState) -> Vec<String> {
    let (p, q) = (&a.pending, &b.pending);
    let mut out = Vec::new();
    macro_rules! cmp_pending {
        ($($f:ident),*) => {$( if p.$f != q.$f { out.push(format!("{}: {:?} vs {:?}", stringify!($f), p.$f, q.$f)); } )*};
    }
    cmp_pending!(staged, top, tag, driver, turn_closing, actions_this_turn, prepared_turn_seq, failed_component_actions, failed_keys);
    out
}

/// The games of the fork test: the quick game's table (4p seed 1 Sol) and three seeded lobbies with other factions (Jol-Nar's commander
/// walk and rerolls, L1Z1X's Harrow, Yssaril's hand questions); every table has action cards, so reaction windows (nested questions) occur.
const GAMES: &[(usize, u64, &str)] = &[(4, 1, "sol"), (3, 3, "yssaril"), (4, 3, "jolnar"), (5, 2, "l1z1x")];

/// How far a fork is played on and compared (steps after the fork's step); the last fork of each game is played to the end.
const HORIZON: usize = 40;

/// Choose the decisions to fork at from a recorded game: nested questions, questions right after a die in the same step, plain ones.
fn fork_points(seen: &[Seen], seed: u64) -> Vec<usize> {
    let pick_from = |filter: &dyn Fn(&Seen) -> bool, n: usize, salt: u64| -> Vec<usize> {
        let idx: Vec<usize> = seen.iter().enumerate().filter(|(_, s)| filter(s)).map(|(i, _)| i).collect();
        if idx.is_empty() {
            return Vec::new();
        }
        (0..n).map(|k| idx[(mix(seed ^ salt ^ k as u64) as usize) % idx.len()]).collect()
    };
    let mut at: Vec<usize> = Vec::new();
    at.extend(pick_from(&|s| s.nested && s.rolled_before, 8, 1));
    at.extend(pick_from(&|s| s.nested && !s.rolled_before, 6, 2));
    at.extend(pick_from(&|s| !s.nested && s.rolled_before, 6, 3));
    at.extend(pick_from(&|_| true, 6, 4));
    at.sort_unstable();
    // One fork per step: the original finishes the step of a fork before it goes to the next.
    at.dedup_by_key(|d| seen[*d].step);
    at
}

/// T1: fork at questions inside steps (nested ones included, ones after a die), go on with the same answers (equal to the game that was
/// never forked) and with different ones (the original stays equal).
#[test]
fn a_core_clone_at_any_question_resumes_to_the_same_game_and_is_independent() {
    let mut report = Vec::new();
    let mut after_die = 0;
    for &(players, seed, faction) in GAMES {
        // The reference: never stopped, never forked. Records what each decision was.
        let mut reference = Play::new(players, seed, faction);
        reference.record = true;
        let mut by_step: BTreeMap<usize, Snap> = BTreeMap::new();
        let over = reference.play_until(None, 20_000).expect("the reference game plays");
        assert!(over, "{players}p seed {seed} {faction}: the reference game ends");
        let end = reference.result();
        let points = fork_points(&reference.seen, seed);
        assert!(points.len() >= 5, "{players}p seed {seed} {faction}: only {} fork points", points.len());
        assert!(points.iter().any(|&d| reference.seen[d].nested), "a nested fork point");
        after_die += points.iter().filter(|&&d| reference.seen[d].rolled_before).count();
        // The reference positions the forks are compared with.
        let targets: Vec<usize> = points.iter().map(|&d| reference.seen[d].step + 1 + HORIZON).collect();
        {
            let mut again = Play::new(players, seed, faction);
            let mut wanted: Vec<usize> = targets.clone();
            wanted.sort_unstable();
            wanted.dedup();
            for t in wanted {
                let over = again.play_until(None, t).expect("plays");
                by_step.insert(again.steps, again.result());
                if over {
                    break;
                }
            }
        }
        // The original: stopped at each fork point, forked, resumed.
        let mut original = Play::new(players, seed, faction);
        let (mut diverged, mut forks, mut boundaries) = (0, 0, 0);
        for (n, &d) in points.iter().enumerate() {
            let mid = original.play_to_decision(d).unwrap_or_else(|e| panic!("{players}p seed {seed} {faction}: to decision {d}: {e}"));
            assert!(original.steps == reference.seen[d].step, "decision {d} is in the step it was in");
            let at_step = original.steps;
            let last = n + 1 == points.len();
            let until = if last { 20_000 } else { at_step + 1 + HORIZON };
            // The copy with the same answers.
            let mut same = original.fork();
            assert!(same.core == original.core, "a clone equals its original");
            same.play_until(Some(mid.clone()), until).unwrap_or_else(|e| panic!("{players}p seed {seed} {faction}: fork at decision {d} (step {at_step}, nested {}): {e}", mid.nested));
            let expected = if last { end.clone() } else { by_step.get(&same.steps).cloned().unwrap_or_else(|| end.clone()) };
            let got = same.result();
            assert!(got == expected, "{players}p seed {seed} {faction}: the fork at decision {d} (step {at_step}, nested {}, after a die {}) differs from the game never forked after {} steps: {}", mid.nested, reference.seen[d].rolled_before, same.steps, got.diff(&expected));
            // The copy with other answers.
            let mut other = original.fork();
            other.seed ^= 0x0D1F_FE4E_47A1_5E55;
            if let Ok(over) = other.play_until(Some(mid.clone()), at_step + 1 + HORIZON) {
                let _ = over;
                if other.result() != by_step.get(&other.steps).cloned().unwrap_or_else(|| end.clone()) {
                    diverged += 1;
                }
            }
            // A fork at the step boundary after this step (after a die, when the step rolled): the original stays put meanwhile.
            original.play_until(Some(mid), at_step + 1).expect("the original resumes");
            if original.rolled_in_step && !last {
                let mut boundary = original.fork();
                boundary.play_until(None, original.steps + HORIZON).expect("the boundary fork plays");
                let expected = by_step.get(&boundary.steps).cloned().unwrap_or_else(|| end.clone());
                boundaries += 1;
                assert!(boundary.result() == expected, "{players}p seed {seed} {faction}: the step-boundary fork after step {at_step}: {}", boundary.result().diff(&expected));
            }
            forks += 1;
        }
        // The original, stopped and forked again and again, ends as the game that never was.
        original.play_until(None, 20_000).expect("the original ends");
        let got = original.result();
        assert!(got == end, "{players}p seed {seed} {faction}: the original after its forks: {}", got.diff(&end));
        report.push(format!("{players}p seed {seed} {faction}: {} steps, {} decisions, {forks} forks ({} nested, {} after a die), {diverged} of them diverged with other answers, {boundaries} step-boundary forks after a roll", end.steps, end.decided, points.iter().filter(|&&d| reference.seen[d].nested).count(), points.iter().filter(|&&d| reference.seen[d].rolled_before).count()));
    }
    assert!(after_die >= 3, "{after_die} fork points right after a die");
    for line in report {
        eprintln!("T1 {line}");
    }
}

/// T1 at step boundaries right after a step that committed a die: a copy and the original play on to the same game, and a copy with other
/// answers leaves the original as the game that was never forked.
#[test]
fn a_core_clone_right_after_a_roll_at_a_step_boundary_resumes_to_the_same_game() {
    let mut checked = 0;
    for &(players, seed, faction) in GAMES {
        let mut reference = Play::new(players, seed, faction);
        reference.record = true;
        reference.play_until(None, 20_000).expect("plays");
        let end = reference.result();
        let mut original = Play::new(players, seed, faction);
        for &at in reference.rolled_steps.iter().take(8) {
            original.play_until(None, at).expect("plays to the boundary");
            assert!(original.rolled_in_step, "{players}p seed {seed} {faction}: step {at} rolled");
            let mut same = original.fork();
            let mut other = original.fork();
            other.seed ^= 0x0D1F_FE4E_47A1_5E55;
            let _ = other.play_until(None, at + HORIZON);
            same.play_until(None, at + HORIZON).expect("the copy plays");
            let mut alone = original.fork();
            alone.play_until(None, at + HORIZON).expect("plays");
            assert!(same.result() == alone.result(), "{players}p seed {seed} {faction}: the fork after the roll of step {at}: {}", same.result().diff(&alone.result()));
            checked += 1;
        }
        original.play_until(None, 20_000).expect("the original ends");
        assert!(original.result() == end, "{players}p seed {seed} {faction}: the original after its forks: {}", original.result().diff(&end));
    }
    eprintln!("T1 boundary forks right after a roll: {checked}");
    assert!(checked >= 10, "{checked} boundary forks");
}

// -- T2: the codec round trip ----------------------------------------------------------------------------------------------------------------

/// Store `core` into the string `GameState` and load it back as the product loads a save (`Native::load`): the resolver and the stream
/// positions handed in (`Ctx::resolver`, `Sources::rng`), the map the caller's (the codec stores none: `materialize` docs), and the dice and
/// event logs lent by the caller (the codec carries neither: `codec/ledger.rs`, V5).
fn round_trip(core: &CoreState, base: &ti4_model::state::GameState) -> Result<CoreState, String> {
    use ti4_engine::core_sync::{materialize, Codec, Ctx, Sources};
    use ti4_engine_core::state::{Slice, SliceSet};
    let cx = Ctx::of_core(core).map_err(|e| format!("ctx: {e}"))?;
    let gs = materialize::materialize(core, &cx, base).map_err(|e| format!("store: {e}"))?;
    let positions = materialize::rng_positions(core).map_err(|e| format!("positions: {e}"))?;
    let mut cx = Ctx::of_core(core).map_err(|e| format!("ctx: {e}"))?;
    cx.resolver = Some(core.pending().resolver.clone());
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32));
    let mut loaded = Codec::load(&mut cx, &gs, Sources { galaxy: None, rng: Some(&positions) }, slices).map_err(|e| format!("load: {e}"))?;
    loaded.map = core.map.clone();
    loaded.loaded = core.loaded;
    loaded.ledger.events = core.ledger.events.clone();
    loaded.ledger.dice = core.ledger.dice.clone();
    Ok(loaded)
}

/// No window open and no question suspended: the only positions a save (a `GameState`) can hold.
fn quiet(core: &CoreState) -> bool {
    core.pending().stack.is_empty() && core.pending().nested.is_none()
}

fn base_of(players: usize, seed: u64, faction: &str) -> ti4_model::state::GameState {
    NativeGame::from_spec(&lobby_spec(faction, players, seed)).expect("builds").probe().3.clone()
}

/// The `Pending` fields the codec does not carry even at a quiet boundary (`codec/pending.rs` stores the pending tag only; V5b of
/// docs/C0_RESUME.md): the driver's progress flags and the turn's bookkeeping. They are `Game` fields of the string engine, outside `GameState`.
const CODEC_DROPS: &[&str] = &["pending.staged", "pending.top", "pending.driver", "pending.turn_closing", "pending.actions_this_turn", "pending.prepared_turn_seq", "pending.failed_component_actions", "pending.failed_keys"];

/// The round trip at every 7th quiet step boundary of the games. `strict`: every field must survive the codec alone. Otherwise only the
/// fields of [`CODEC_DROPS`] may be lost; with them handed in, the loaded core equals the original and plays on to the same game; and the
/// load without them is played on too, to count the positions where the loss shows.
fn codec_round_trips(strict: bool) {
    let mut report = Vec::new();
    let mut lost_by_field: BTreeMap<&'static str, usize> = BTreeMap::new();
    for &(players, seed, faction) in GAMES {
        let base = base_of(players, seed, faction);
        let mut play = Play::new(players, seed, faction);
        let (mut checked, mut lossy, mut lossy_diverged, mut skipped) = (0, 0, 0, 0);
        loop {
            if play.steps % 7 == 3 {
                if quiet(&play.core) {
                    let loaded = round_trip(&play.core, &base).unwrap_or_else(|e| panic!("{players}p seed {seed} {faction} step {}: {e}", play.steps));
                    let lost = fields_differing(&loaded, &play.core);
                    let unexpected: Vec<_> = lost.iter().filter(|f| strict || !CODEC_DROPS.contains(f)).collect();
                    assert!(unexpected.is_empty(), "{players}p seed {seed} {faction} step {}: the round trip changed {unexpected:?} ({})", play.steps, pending_differing(&loaded, &play.core).join("; "));
                    for f in &lost {
                        *lost_by_field.entry(f).or_default() += 1;
                    }
                    let mut original = play.fork();
                    original.play_until(None, play.steps + HORIZON).expect("the original plays");
                    // With the dropped fields handed in: the same core, the same game.
                    let mut lent = loaded.clone();
                    lend_dropped(&mut lent, &play.core);
                    assert!(lent == play.core, "{players}p seed {seed} {faction} step {}: the loaded core with the lent fields differs: {:?}", play.steps, fields_differing(&lent, &play.core));
                    let mut copy = loaded_copy(&play, lent);
                    copy.play_until(None, play.steps + HORIZON).unwrap_or_else(|e| panic!("{players}p seed {seed} {faction}: loaded at step {}: {e}", play.steps));
                    assert!(copy.result() == original.result(), "{players}p seed {seed} {faction}: loaded at step {}, after {} steps: {}", play.steps, copy.steps, copy.result().diff(&original.result()));
                    checked += 1;
                    // Without them: does the loss show?
                    if !lost.is_empty() {
                        lossy += 1;
                        let mut bare = loaded_copy(&play, loaded);
                        // (The lost fields themselves are not counted: a flag that is never set again differs to the end.)
                        let played = bare.play_until(None, play.steps + HORIZON).is_ok();
                        if !played || bare.chain != original.chain || fields_differing(&bare.core, &original.core).iter().any(|f| !CODEC_DROPS.contains(f)) {
                            lossy_diverged += 1;
                        }
                    }
                } else {
                    skipped += 1;
                }
            }
            if play.play_until(None, play.steps + 1).expect("plays") {
                break;
            }
        }
        assert!(checked >= 20, "{players}p seed {seed} {faction}: {checked} round trips");
        report.push(format!("{players}p seed {seed} {faction}: {checked} round trips at quiet boundaries (played on {HORIZON} steps, identical); {lossy} lost a pending field, {lossy_diverged} of those then played differently without it; {skipped} sampled boundaries not quiet"));
    }
    for line in report {
        eprintln!("T2 {line}");
    }
    eprintln!("T2 fields lost by the codec alone (count of quiet boundaries): {lost_by_field:?}");
}

/// Hand the [`CODEC_DROPS`] fields of `from` to a loaded core (as the caller hands in the resolver and the stream positions).
fn lend_dropped(loaded: &mut CoreState, from: &CoreState) {
    let (p, q) = (&mut loaded.pending, &from.pending);
    p.staged.clone_from(&q.staged);
    p.top.clone_from(&q.top);
    p.driver = q.driver;
    p.turn_closing = q.turn_closing;
    p.actions_this_turn = q.actions_this_turn;
    p.prepared_turn_seq = q.prepared_turn_seq;
    p.failed_component_actions.clone_from(&q.failed_component_actions);
    p.failed_keys.clone_from(&q.failed_keys);
}

/// A copy of `play` on `core`, with a render built from that core's code table.
fn loaded_copy(play: &Play, core: CoreState) -> Play {
    let mut copy = play.fork();
    copy.core = core;
    copy.render = Render::new(play.ids.clone()).with_codes(copy.core.texts.codes.clone());
    copy.render.factions.clone_from(&play.render.factions);
    copy
}

/// T2: at quiet step boundaries the round trip keeps the position except the known `Pending` fields, and with those handed in the loaded core
/// plays on to the same game.
#[test]
fn the_codec_round_trip_at_a_quiet_step_boundary_gives_the_same_core_and_plays_on_identically() {
    codec_round_trips(false);
}

/// T2 strict: every quiet boundary survives the round trip whole.
#[test]
#[ignore = "known violation V5b (docs/C0_RESUME.md): codec/pending.rs stores only the pending tag, so Pending::driver (STRATEGY_PHASE_ANNOUNCED, first at 4p seed 1 sol step 3 of this driver), prepared_turn_seq, turn_closing, actions_this_turn and staged events are lost at quiet boundaries (154 of 282 sampled loads then play differently); repro: cargo test -p ti4-engine-core-bridge --test fork_core -- --ignored strict"]
fn the_codec_round_trip_at_a_quiet_step_boundary_is_exact_strict() {
    codec_round_trips(true);
}

/// T2, the other half (V5): a position with an open window or a suspended nested question has no `GameState` form. The round trip drops the
/// window stack and the nested question (`codec/pending.rs` stores only the pending tag), so such a position cannot be saved, only cloned.
#[test]
fn the_codec_cannot_carry_an_open_window_or_a_nested_question_v5() {
    let (players, seed, faction) = GAMES[0];
    let base = base_of(players, seed, faction);
    let mut play = Play::new(players, seed, faction);
    play.record = true;
    play.play_until(None, 20_000).expect("plays");
    let nested_at = play.seen.iter().position(|s| s.nested).expect("a nested question");
    let stage_at = play.seen.iter().position(|s| !s.nested).expect("a stage question");
    let mut lost = 0;
    for at in [nested_at, stage_at] {
        let mut p = Play::new(players, seed, faction);
        let _mid = p.play_to_decision(at).expect("plays to the question");
        if quiet(&p.core) {
            continue;
        }
        let loaded = round_trip(&p.core, &base).expect("the store and the load themselves succeed");
        if !p.core.pending().stack.is_empty() {
            assert!(loaded.pending().stack.is_empty(), "the codec carried an open window: V5 is fixed, turn this into a T2 equality");
        }
        if p.core.pending().nested.is_some() {
            assert!(loaded.pending().nested.is_none(), "the codec carried a nested question: V5 is fixed, turn this into a T2 equality");
        }
        assert!(loaded != p.core, "a position with an open window does not survive the codec");
        lost += 1;
    }
    assert!(lost >= 1, "a question with an open window was checked");
}

// -- the draw census (3.2) -----------------------------------------------------------------------------------------------------------------

/// What one game drew and kept.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    steps: usize,
    draws: u64,
    words: u64,
    committed_draws: u64,
    committed_words: u64,
    ledger_faces: u64,
    dry_rollbacks: u64,
    dry_draws: u64,
    err_rollbacks: u64,
    err_draws: u64,
    ask_draws: u64,
    failed_advances: u64,
    failed_draws: u64,
}

impl Tally {
    fn of(start: &[(RngDomain, u128)], end: &CoreState, counts: &census::Counts, calls: Calls, steps: usize) -> Self {
        let from = |d: RngDomain| start.iter().find(|(x, _)| *x == d).map_or(0, |(_, p)| *p);
        let to: Vec<(RngDomain, u128)> = end.rng.positions().collect();
        let upto = |d: RngDomain| to.iter().find(|(x, _)| *x == d).map_or(0, |(_, p)| *p);
        let committed_words = to.iter().map(|(d, p)| u64::try_from(p - from(*d)).unwrap_or(0)).sum();
        let committed_draws = counts.starts.iter().filter(|(d, p)| *p >= from(*d) && *p < upto(*d)).count() as u64;
        let ledger_faces = end.ledger.dice.iter().map(|r| r.faces.len() as u64).sum();
        Self {
            steps,
            draws: counts.draws,
            words: counts.words,
            committed_draws,
            committed_words,
            ledger_faces,
            dry_rollbacks: counts.dry_rollbacks,
            dry_draws: counts.dry_draws,
            err_rollbacks: counts.err_rollbacks,
            err_draws: counts.err_draws,
            ask_draws: calls.ask_draws,
            failed_advances: calls.failed_advances,
            failed_draws: calls.failed_draws,
        }
    }

    fn add(&mut self, o: &Self) {
        self.steps += o.steps;
        self.draws += o.draws;
        self.words += o.words;
        self.committed_draws += o.committed_draws;
        self.committed_words += o.committed_words;
        self.ledger_faces += o.ledger_faces;
        self.dry_rollbacks += o.dry_rollbacks;
        self.dry_draws += o.dry_draws;
        self.err_rollbacks += o.err_rollbacks;
        self.err_draws += o.err_draws;
        self.ask_draws += o.ask_draws;
        self.failed_advances += o.failed_advances;
        self.failed_draws += o.failed_draws;
    }

    fn line(&self) -> String {
        let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };
        format!(
            "steps {} | draws {} (words {}) | committed {} (words {}, ledger faces {}) = {:.1} % kept, {:.2} draws per committed | dry rollbacks {} with {} draws ({:.1} %) | error rollbacks {} with {} draws | ask (scratch) draws {} ({:.1} %) | failed advances {} with {} draws",
            self.steps,
            self.draws,
            self.words,
            self.committed_draws,
            self.committed_words,
            self.ledger_faces,
            pct(self.committed_draws, self.draws),
            if self.committed_draws == 0 { 0.0 } else { self.draws as f64 / self.committed_draws as f64 },
            self.dry_rollbacks,
            self.dry_draws,
            pct(self.dry_draws, self.draws),
            self.err_rollbacks,
            self.err_draws,
            self.ask_draws,
            pct(self.ask_draws, self.draws),
            self.failed_advances,
            self.failed_draws
        )
    }
}

/// The census on the core live API: every die drawn against the dice the game kept, and where the others were thrown away.
#[test]
fn the_draw_census_counts_the_draws_the_rollbacks_throw_away() {
    let mut total = Tally::default();
    for &(players, seed, faction) in GAMES {
        let mut play = Play::new(players, seed, faction);
        let start = positions(&play.core);
        census::reset();
        assert!(play.play_until(None, 20_000).expect("plays"), "the game ends");
        let counts = census::counts();
        let t = Tally::of(&start, &play.core, &counts, play.calls, play.steps);
        eprintln!("CENSUS core {players}p seed {seed} {faction}: {}", t.line());
        // A committed draw was drawn; every other draw was thrown away with a rolled-back run (or a scratch / failed call).
        assert!(t.committed_draws <= t.draws && t.committed_draws > 0);
        assert!(counts.shuffles == 0, "no in-game shuffle");
        total.add(&t);
    }
    eprintln!("CENSUS core total: {}", total.line());
}

/// The rule bot of the native playthrough as a table decider (`wrapper_native.rs`).
struct BotDecider {
    seed: u64,
    seats: Vec<String>,
    index: usize,
}

impl ti4_engine::wire::choice::Decider for BotDecider {
    fn choose(&mut self, choice: &ti4_engine::wire::choice::Choice) -> Result<ti4_engine::wire::choice::ChoiceOption, ti4_engine::wire::choice::IllegalChoice> {
        let place = self.seats.iter().position(|s| s == choice.player.as_str()).unwrap_or(0);
        let (picked, _, _) = ti4_rule_bot::choose_explained(choice, None, self.seed, place, self.index);
        self.index += 1;
        Ok(choice.options[picked.min(choice.options.len() - 1)].clone())
    }
}

/// The census on the thin `Game` (the product's step: reconcile, arm, nested questions answered inside the step) with the rule bot: the quick
/// game (`c5a8d4b9086367c7` since the multi-ask conversion, 1,110 steps; `538dc68f23dddcc9`, 1,165 before) and two more. Committed = the stream words (and `Game::rolls()` faces).
#[test]
fn the_draw_census_of_the_quick_game_and_two_bot_games_on_the_thin_game() {
    use ti4_content::ContentStore;
    use ti4_engine::native::Game;
    use ti4_engine::wire::choice::Table;
    let mut total = Tally::default();
    for &(players, seed, faction) in &[(4, 1, "sol"), (3, 3, "yssaril"), (5, 2, "l1z1x")] {
        let spec = lobby_spec(faction, players, seed);
        let legacy = spec.build().expect("a lobby game builds");
        let seats: Vec<String> = legacy.state.players.iter().map(|p| p.id.as_str().to_owned()).collect();
        let galaxy = legacy.galaxy().cloned().expect("a lobby game has a map");
        let table = Table::with_default(Box::new(BotDecider { seed, seats, index: 0 }));
        let mut game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).expect("playable").with_galaxy(galaxy);
        census::reset();
        let start = game.rng_positions();
        let mut steps = 0;
        while !game.state.finished {
            assert_eq!(game.step().error, None, "{players}p seed {seed} {faction} step {steps}");
            steps += 1;
            assert!(steps < 5_000);
        }
        let counts = census::counts();
        let (from, to) = (start, game.rng_positions());
        let committed_words: u64 = to.iter().map(|(d, p)| u64::try_from(p - from.get(d).copied().unwrap_or(0)).unwrap_or(0)).sum();
        let faces: u64 = game.rolls().iter().map(|r| r.faces.len() as u64).sum();
        let t = Tally { steps, draws: counts.draws, words: counts.words, committed_draws: committed_words, committed_words, ledger_faces: faces, dry_rollbacks: counts.dry_rollbacks, dry_draws: counts.dry_draws, err_rollbacks: counts.err_rollbacks, err_draws: counts.err_draws, ..Tally::default() };
        eprintln!("CENSUS thin {players}p seed {seed} {faction}: {} (committed = stream words; {} roll rows)", t.line(), game.rolls().len());
        assert!(t.committed_words <= t.words);
        total.add(&t);
    }
    eprintln!("CENSUS thin total: {}", total.line());
}

/// Integration 28 (re-check of lane nestq-1, `docs/INTEGRATION28.md`): how often a nested question is served from the question the
/// suspension remembers (`driver::live::memo_census`), on the core live API (the four games above) and on the thin `Game` with the rule bot
/// (the quick game and two more: the render mints codes between calls, so more answers may fall back to the re-ask). In a debug build the
/// check `live::verify_remembered` is on (the default there): every memo use is rebuilt from scratch and compared, and a mismatch panics.
#[test]
fn the_remembered_nested_question_serves_the_answers_and_the_debug_check_finds_no_mismatch() {
    use ti4_content::ContentStore;
    use ti4_engine::native::Game;
    use ti4_engine::wire::choice::Table;
    use ti4_engine_core::driver::live::memo_census;
    let line = |c: &memo_census::Counts| {
        let answers = c.answer_hits + c.answer_fallbacks;
        format!(
            "answers {answers}: from the memo {} ({:.1} %), re-asked {}; reads with a nested question {}: from the memo {}, replayed {}; debug-checked {}",
            c.answer_hits,
            if answers == 0 { 0.0 } else { 100.0 * c.answer_hits as f64 / answers as f64 },
            c.answer_fallbacks,
            c.read_hits + c.read_fallbacks,
            c.read_hits,
            c.read_fallbacks,
            c.checked
        )
    };
    let debug = cfg!(debug_assertions);
    for &(players, seed, faction) in GAMES {
        let mut play = Play::new(players, seed, faction);
        memo_census::reset();
        assert!(play.play_until(None, 20_000).expect("plays"), "the game ends");
        let c = memo_census::counts();
        eprintln!("MEMO core {players}p seed {seed} {faction} ({} steps): {}", play.steps, line(&c));
        assert!(c.answer_hits > 0, "the core driver answers nested questions from the memo");
        if debug {
            assert_eq!(c.checked, c.answer_hits + c.read_hits, "every memo use was checked");
        }
    }
    for &(players, seed, faction) in &[(4, 1, "sol"), (3, 3, "yssaril"), (5, 2, "l1z1x")] {
        let spec = lobby_spec(faction, players, seed);
        let legacy = spec.build().expect("a lobby game builds");
        let seats: Vec<String> = legacy.state.players.iter().map(|p| p.id.as_str().to_owned()).collect();
        let galaxy = legacy.galaxy().cloned().expect("a lobby game has a map");
        let table = Table::with_default(Box::new(BotDecider { seed, seats, index: 0 }));
        let mut game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).expect("playable").with_galaxy(galaxy);
        memo_census::reset();
        let mut steps = 0;
        while !game.state.finished {
            assert_eq!(game.step().error, None, "{players}p seed {seed} {faction} step {steps}");
            steps += 1;
            assert!(steps < 5_000);
        }
        let c = memo_census::counts();
        eprintln!("MEMO thin {players}p seed {seed} {faction} ({steps} steps): {}", line(&c));
        if debug {
            assert_eq!(c.checked, c.answer_hits + c.read_hits, "every memo use was checked");
        }
    }
}
