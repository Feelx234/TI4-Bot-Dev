//! The D1 dual of the driver's `Pending` (package D1a; docs/ENGINE_AREA_STATUS.md 4.1): after every `Game::step` the typed
//! windows the core holds in `Pending.stack` are compared with the window fields of the string `Game`.
//!
//! The windows of the finished areas that have a typed payload in the core (`StrategySecondary`,
//! `StrategySecondaryAfterTactical`, `LeaderStrategy`, `ExecutiveOrder`, `AgendaQueueAfterEventScoring`) are copied from the
//! string fields when the string opens them (the open is phase flow, which is D1a's next piece). From then on the window is
//! **held in the core**: the follower window is moved on by the core's own outcome of each step
//! (`rules::strategy_secondary::advance`, fed by the `strategy::secondary` window dual) and closed when the core says it
//! is complete; the other kinds have no core transition yet and follow the string. Every step then
//!
//! 1. compares the held window with the string field it stands for (`compare_value`, area `driver`, label
//!    `d1::pending.<kind>`), and
//! 2. checks that the typed copy of every open window turns back into the string field it was made from
//!    (`d1::pending.roundtrip`): the codec is total on the positions the games reach.
//!
//! The kinds with no typed payload (`Scoring`, `Tokens`, `Voting`, the tactical and trade windows, ...) are counted under
//! `d1::pending.untyped [bail: <field>]`. A step whose result is an error, a preview window (a planning fork), an id the typed
//! tables do not know and a window the core step does not cover (a card nobody holds) are counted as bails and re-seed the
//! held copy from the string. Runs only with `driver=dual`; a pure observer (the string result stands).

use ti4_engine_core::containers::{SeatSet, SmallList};
use ti4_engine_core::generated::{Agenda, Leader, StrategyCard};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::rules::strategy_secondary::{self as core_secondary, Outcome, Resolution};
use ti4_engine_core::state::{Pending, PlainArg, PlainKind, PlainRec, Scoring, ScoringTiming, Secondary, SecondaryHow, Window, WindowKind};
use ti4_model::id::{LeaderId, PlayerId, StrategyCardId};

use super::{Game, StepResult};
use crate::dual::{Area, Backend, compare_value, note_bail, note_window};
use crate::generated::IdStr;
use crate::objectives::Book;
use crate::secrets::Timing;
use crate::render::events::plain_text;
use crate::render::Render;
use crate::strategy::{SecondaryResolution, StepOutcome, StrategySecondaryWindow};

#[path = "d1_windows.rs"]
mod windows;
#[path = "d1_action.rs"]
mod action;

const LABEL: &str = "d1::pending";

/// What the driver holds of the windows between steps (a field of `Game`; the fork rule copies it).
#[derive(Debug, Clone, Default)]
pub(super) struct Shadow {
    /// The core's `Pending` as the last checked step left it.
    pending: Pending,
    /// The outcome the core half of this step's follower window gave (set by `Game::secondary_step`, taken by [`check`]).
    pub(super) outcome: Option<StepOutcome>,
    /// The book of the scoring window after the core half of this step's scoring answer (`event`: the event window),
    /// set by `Game::score_step`, taken by [`check`].
    pub(super) scoring: Option<(bool, Book, Option<ti4_model::id::ObjectiveId>)>,
    /// The facts the step loop read before choosing its arm, and the name of the arm the string took (package D1b-7).
    facts: Option<ti4_engine_core::driver::session::Facts>,
    arm: Option<&'static str>,
    /// The source of the question the step put to the table (`Game::legal_options`), when it did.
    ask: Option<&'static str>,
}

impl Shadow {
    /// The string took the arm `name` of `Game::step_flow`.
    pub(super) fn note_arm(&mut self, name: &'static str) {
        if active() {
            // The first arm only: the turn prelude and the announcement fall through to the question of the same step.
            self.arm.get_or_insert(name);
        }
    }

    /// The question of the step came from `legal_options`: read what the last call of this thread said.
    pub(super) fn note_ask_seen(&mut self) {
        if active() {
            self.ask = ASK.with(std::cell::Cell::take);
        }
    }
}

thread_local! {
    /// The source `Game::legal_options` answered from in its last call on this thread.
    static ASK: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// `Game::legal_options` answered from `source` (a name of `AskArm`).
pub(super) fn note_ask(source: &'static str) {
    if active() {
        ASK.with(|c| c.set(Some(source)));
    }
}

/// Read the facts the arm of this step is chosen from (after the reconcile hooks) and keep them for [`check`].
pub(super) fn snapshot_facts(g: &mut Game<'_>) {
    if !active() {
        return;
    }
    use ti4_engine_core::driver::session::{Facts, bit};
    let open = [
        (g.leader_followers.is_some(), WindowKind::LeaderFollowers),
        (g.secondary.is_some(), WindowKind::StrategySecondary),
        (g.event_scoring.is_some(), WindowKind::EventScoring),
        (g.scoring.is_some(), WindowKind::Scoring),
        (g.tokens.is_some(), WindowKind::Tokens),
        (g.voting.is_some(), WindowKind::Voting),
        (g.diplomacy.is_some(), WindowKind::Diplomacy),
        (g.aftermath.is_some(), WindowKind::Aftermath),
        (g.agenda_talks.is_some(), WindowKind::AgendaTalks),
        (g.trade.is_some(), WindowKind::Trade),
        (g.tactical.is_some(), WindowKind::Tactical),
    ]
    .into_iter()
    .filter(|(is_open, _)| *is_open)
    .fold(0, |acc, (_, kind)| acc | bit(kind));
    let phase = match g.state.phase {
        ti4_model::state::Phase::Strategy => ti4_engine_core::state::Phase::Strategy,
        ti4_model::state::Phase::Action => ti4_engine_core::state::Phase::Action,
        ti4_model::state::Phase::Status => ti4_engine_core::state::Phase::Status,
        ti4_model::state::Phase::Agenda => ti4_engine_core::state::Phase::Agenda,
    };
    g.d1.arm = None;
    g.d1.ask = None;
    g.d1.facts = Some(Facts {
        finished: g.state.finished,
        blocked: g.blocked.is_some(),
        open,
        phase,
        turn_prepared: g.prepared_turn_seq == Some(g.state.turn_seq),
        has_active: g.state.active.is_some(),
        status_resolved: g.status_resolved,
        agenda_resolved: g.agenda_resolved,
        strategy_announced: g.strategy_phase_announced,
        round: g.state.round,
        starting_picks_done: g.state.starting_tech_picks.is_empty(),
        turn_closing: g.turn_closing.is_some(),
    });
}

pub(super) use action::{Ended, transition};

/// The report name of an arm.
fn arm_name(a: ti4_engine_core::driver::session::Arm) -> &'static str {
    match a {
        ti4_engine_core::driver::session::Arm::Finished => "finished",
        ti4_engine_core::driver::session::Arm::Blocked => "blocked",
        ti4_engine_core::driver::session::Arm::LeaderFollowers => "leader_followers",
        ti4_engine_core::driver::session::Arm::Secondary => "secondary",
        ti4_engine_core::driver::session::Arm::EventScoring => "event_scoring",
        ti4_engine_core::driver::session::Arm::Scoring => "scoring",
        ti4_engine_core::driver::session::Arm::Tokens => "tokens",
        ti4_engine_core::driver::session::Arm::Voting => "voting",
        ti4_engine_core::driver::session::Arm::Diplomacy => "diplomacy",
        ti4_engine_core::driver::session::Arm::Aftermath => "aftermath",
        ti4_engine_core::driver::session::Arm::AgendaTalks => "agenda_talks",
        ti4_engine_core::driver::session::Arm::Trade => "trade",
        ti4_engine_core::driver::session::Arm::Tactical => "tactical",
        ti4_engine_core::driver::session::Arm::TurnPrelude => "turn_prelude",
        ti4_engine_core::driver::session::Arm::Status => "status",
        ti4_engine_core::driver::session::Arm::Agenda => "agenda",
        ti4_engine_core::driver::session::Arm::StrategyAnnounce => "strategy_announce",
        ti4_engine_core::driver::session::Arm::Ask => "ask",
        ti4_engine_core::driver::session::Arm::Phase => "phase",
    }
}

/// The report name of the source of a question.
fn ask_name(a: ti4_engine_core::driver::session::AskArm) -> &'static str {
    match a {
        ti4_engine_core::driver::session::AskArm::None => "none",
        ti4_engine_core::driver::session::AskArm::LeaderFollowers => "leader_followers",
        ti4_engine_core::driver::session::AskArm::EventScoring => "event_scoring",
        ti4_engine_core::driver::session::AskArm::Scoring => "scoring",
        ti4_engine_core::driver::session::AskArm::Tokens => "tokens",
        ti4_engine_core::driver::session::AskArm::Voting => "voting",
        ti4_engine_core::driver::session::AskArm::Diplomacy => "diplomacy",
        ti4_engine_core::driver::session::AskArm::Aftermath => "aftermath",
        ti4_engine_core::driver::session::AskArm::AgendaTalks => "agenda_talks",
        ti4_engine_core::driver::session::AskArm::Tactical => "tactical",
        ti4_engine_core::driver::session::AskArm::StartingTech => "starting_tech",
        ti4_engine_core::driver::session::AskArm::Draft => "draft",
        ti4_engine_core::driver::session::AskArm::Secondary => "secondary",
        ti4_engine_core::driver::session::AskArm::Closing => "closing",
        ti4_engine_core::driver::session::AskArm::ActionMenu => "action_menu",
        ti4_engine_core::driver::session::AskArm::Nothing => "nothing",
    }
}

/// Compare the arm the string took with the core's precedence over the same facts (`d1::step.arm`, `d1::step.ask`).
fn check_arm(g: &mut Game<'_>) {
    use ti4_engine_core::driver::session::{ask_arm, step_arm};
    let (Some(facts), Some(arm)) = (g.d1.facts.take(), g.d1.arm.take()) else {
        g.d1.ask = None;
        return;
    };
    let ask = g.d1.ask.take();
    let core = arm_name(step_arm(&facts, arm == "ask"));
    let _ = compare_value(Area::Driver, "d1::step.arm", &arm, &core);
    // The census of the step loop (package D1b-8): every step is counted, and the arms the core does not run are the bails
    // by name (the worklist of the lent bodies). The follower and scoring windows are held and moved by the core
    // (`d1::pending`); a phase move is the turn-flow port (`turn::`); a question is a lent dispatch by its source.
    note_window("d1::step", true);
    match arm {
        "secondary" | "scoring" | "event_scoring" | "phase" => {}
        "ask" => note_bail("d1::step", &format!("ask.{}", ask.unwrap_or("?"))),
        other => note_bail("d1::step", other),
    }
    if arm == "ask" || arm == "phase" {
        // A step that put a question to the table: the source of the question is the core's too. A step that found nothing
        // to ask (`phase`) asked `legal_options` as well, and it answered `None`.
        if let Some(string) = ask {
            let _ = compare_value(Area::Driver, "d1::step.ask", &string, &ask_name(ask_arm(&facts)));
        }
    }
}

/// Whether the D1 dual is on for this thread.
pub(super) fn active() -> bool {
    cfg!(feature = "typed-core") && crate::dual::backend(Area::Driver) == Backend::Dual
}

fn any_open(g: &Game<'_>) -> bool {
    g.secondary.is_some()
        || g.secondary_after_tactical.is_some()
        || g.leader_strategy.is_some()
        || g.executive_order.is_some()
        || g.agenda_queue_after_event_scoring.is_some()
        || g.scoring.is_some()
        || g.event_scoring.is_some()
        || g.leader_followers.is_some()
        || g.tokens.is_some()
        || g.voting.is_some()
        || g.trade.is_some()
        || g.tactical.is_some()
        || g.aftermath.is_some()
}

/// The string fields that hold a window no typed payload exists for, by name.
fn untyped_open(g: &Game<'_>) -> Vec<&'static str> {
    [
        ("diplomacy", g.diplomacy.is_some()),
        ("agenda_talks", g.agenda_talks.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, open)| open.then_some(name))
    .collect()
}

// --- string -> typed -------------------------------------------------------------------------------------------------

fn how_of(r: SecondaryResolution) -> SecondaryHow {
    match r {
        SecondaryResolution::Ineligible => SecondaryHow::Ineligible,
        SecondaryResolution::Declined => SecondaryHow::Declined,
        SecondaryResolution::Followed => SecondaryHow::Followed,
    }
}

fn resolution_of(h: SecondaryHow) -> SecondaryResolution {
    match h {
        SecondaryHow::Ineligible => SecondaryResolution::Ineligible,
        SecondaryHow::Declined => SecondaryResolution::Declined,
        SecondaryHow::Followed => SecondaryResolution::Followed,
    }
}

/// The typed copy of a follower window; `Err` for a preview window and an id the tables do not know.
fn secondary_of(w: &StrategySecondaryWindow, r: &Render) -> Result<Secondary, &'static str> {
    let parts = w.parts();
    if parts.preview {
        return Err("a preview window");
    }
    let seat = |p: &PlayerId| r.seat(p.as_str()).ok_or("a seat the table does not know");
    let mut followers = SmallList::new();
    for f in parts.followers {
        followers.push(seat(f)?);
    }
    let mut resolved = SmallList::new();
    for (p, how) in parts.resolutions {
        resolved.push((seat(p)?, how_of(*how)));
    }
    Ok(Secondary {
        primary: seat(parts.primary)?,
        card: StrategyCard::from_id(parts.card.as_str()).ok_or("a strategy card the tables do not know")?,
        followers,
        next: u8::try_from(parts.next).map_err(|_| "more than 255 followers")?,
        resolved,
        exhausts_card: parts.exhausts_card,
    })
}

/// The string window a typed copy stands for (the inverse of [`secondary_of`]).
fn secondary_back(s: &Secondary, r: &Render) -> StrategySecondaryWindow {
    StrategySecondaryWindow::from_parts(
        r.player(s.primary),
        StrategyCardId::new(s.card.as_str()),
        s.followers.as_slice().iter().map(|f| r.player(*f)).collect(),
        usize::from(s.next),
        s.resolved.as_slice().iter().map(|(p, how)| (r.player(*p), resolution_of(*how))).collect(),
        s.exhausts_card,
    )
}

fn timing_of(t: Timing) -> ScoringTiming {
    match t {
        Timing::Status => ScoringTiming::Status,
        Timing::Action => ScoringTiming::Action,
        Timing::Agenda => ScoringTiming::Agenda,
    }
}

fn timing_back(t: ScoringTiming) -> Timing {
    match t {
        ScoringTiming::Status => Timing::Status,
        ScoringTiming::Action => Timing::Action,
        ScoringTiming::Agenda => Timing::Agenda,
    }
}

/// The typed copy of a scoring window's book; `Err` for a seat the table does not know.
fn scoring_of(book: &Book, r: &Render) -> Result<Scoring, &'static str> {
    let seat = |p: &PlayerId| r.seat(p.as_str()).ok_or("a seat the table does not know");
    let set = |ps: &[PlayerId]| -> Result<SeatSet, &'static str> {
        let mut set = SeatSet::EMPTY;
        for p in ps {
            set.insert(seat(p)?);
        }
        Ok(set)
    };
    let mut pending = SmallList::new();
    for p in &book.pending {
        pending.push(seat(p)?);
    }
    Ok(Scoring {
        pending,
        public_used: set(&book.public_used)?,
        secret_used: set(&book.secret_used)?,
        plot_card: match &book.plot_card {
            Some((a, b)) => Some((seat(a)?, seat(b)?)),
            None => None,
        },
        timing: timing_of(book.timing),
        occurrence: book.occurrence.map(|o| o.0),
        one_per_player: book.one_per_player,
    })
}

/// The string book a typed copy stands for (the inverse of [`scoring_of`]).
fn scoring_back(s: &Scoring, r: &Render) -> Book {
    let players = |set: SeatSet| -> Vec<PlayerId> {
        let mut ids: Vec<PlayerId> = (0..16u8).map(Seat).filter(|seat| set.contains(*seat)).map(|seat| r.player(seat)).collect();
        // The string keeps these in the order of its `BTreeSet<PlayerId>`.
        ids.sort();
        ids
    };
    Book {
        pending: s.pending.as_slice().iter().map(|seat| r.player(*seat)).collect(),
        public_used: players(s.public_used),
        secret_used: players(s.secret_used),
        plot_card: s.plot_card.map(|(a, b)| (r.player(a), r.player(b))),
        timing: timing_back(s.timing),
        occurrence: s.occurrence.map(ti4_model::state::FeatOccurrence),
        one_per_player: s.one_per_player,
    }
}

/// The typed windows the string fields hold now, in the order of the `Game` fields.
fn observe(g: &Game<'_>, r: &Render, cx: &crate::core_sync::Ctx) -> Result<Vec<Window>, &'static str> {
    let seat = |p: &PlayerId| r.seat(p.as_str()).ok_or("a seat the table does not know");
    let mut out = Vec::new();
    if let Some(w) = &g.secondary {
        out.push(Window::StrategySecondary(secondary_of(w, r)?));
    }
    if let Some(w) = &g.secondary_after_tactical {
        out.push(Window::StrategySecondaryAfterTactical(secondary_of(w, r)?));
    }
    if let Some((player, leader, card)) = &g.leader_strategy {
        out.push(Window::LeaderStrategy {
            player: seat(player)?,
            leader: Leader::from_id(leader.as_str()).ok_or("a leader the tables do not know")?,
            card: StrategyCard::from_id(card.as_str()).ok_or("a strategy card the tables do not know")?,
        });
    }
    if let Some((owner, speaker)) = &g.executive_order {
        out.push(Window::ExecutiveOrder { owner: seat(owner)?, speaker: seat(speaker)? });
    }
    if let Some(w) = &g.scoring {
        out.push(Window::Scoring(scoring_of(&w.book(), r)?));
    }
    if let Some(w) = &g.event_scoring {
        out.push(Window::EventScoring(scoring_of(&w.book(), r)?));
    }
    if let Some(queue) = &g.agenda_queue_after_event_scoring {
        let mut agendas = SmallList::new();
        for alias in queue {
            agendas.push(Agenda::from_id(alias).ok_or("an agenda the tables do not know")?);
        }
        out.push(Window::AgendaQueueAfterEventScoring(agendas));
    }
    // The kinds D1a's remainder types: they follow the string (no core transition yet).
    out.extend(windows::leader_followers_of(g, r)?);
    out.extend(windows::tokens_of(g, r)?);
    out.extend(windows::voting_of(g, r, cx)?);
    out.extend(windows::trade_of(g, r)?);
    // The windows of the action phase (package D1b).
    out.extend(action::tactical_of(g, r)?);
    out.extend(action::aftermath_of(g, r)?);
    out.extend(action::sub_of(g, r)?);
    Ok(out)
}

/// Whether every typed window turns back into the string field it was made from.
fn roundtrip(g: &Game<'_>, windows: &[Window], r: &Render, cx: &crate::core_sync::Ctx) -> bool {
    let mut ok = true;
    for w in windows {
        ok &= match w {
            Window::StrategySecondary(s) => compare_value(Area::Driver, "d1::pending.roundtrip", &g.secondary.clone(), &Some(secondary_back(s, r))),
            Window::StrategySecondaryAfterTactical(s) => {
                compare_value(Area::Driver, "d1::pending.roundtrip", &g.secondary_after_tactical.clone(), &Some(secondary_back(s, r)))
            }
            Window::LeaderStrategy { player, leader, card } => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.leader_strategy.clone(),
                &Some((r.player(*player), LeaderId::new(leader.as_str()), StrategyCardId::new(card.as_str()))),
            ),
            Window::ExecutiveOrder { owner, speaker } => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.executive_order.clone(),
                &Some((r.player(*owner), r.player(*speaker))),
            ),
            Window::AgendaQueueAfterEventScoring(q) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.agenda_queue_after_event_scoring.clone(),
                &Some(q.as_slice().iter().map(|a| a.as_str().to_owned()).collect::<Vec<_>>()),
            ),
            Window::Scoring(sc) => {
                compare_value(Area::Driver, "d1::pending.roundtrip", &g.scoring.as_ref().map(|w| w.book()), &Some(scoring_back(sc, r)))
            }
            Window::EventScoring(sc) => {
                compare_value(Area::Driver, "d1::pending.roundtrip", &g.event_scoring.as_ref().map(|w| w.book()), &Some(scoring_back(sc, r)))
            }
            Window::LeaderFollowers(w) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.leader_followers.clone(),
                &windows::leader_followers_back(g, r, w),
            ),
            Window::Tokens(t) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.tokens.clone(),
                &Some(windows::tokens_back(t, r)),
            ),
            Window::Voting(v) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.voting.clone(),
                &windows::voting_back(v, r, cx),
            ),
            Window::Trade(t) => compare_value(Area::Driver, "d1::pending.roundtrip", &windows::trade_key_of(g), &Some(windows::trade_key(t, r))),
            Window::Tactical(t) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.tactical.as_ref().map(action::tactical_key),
                &Some(action::tactical_back(t, r)),
            ),
            Window::Aftermath(a) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.aftermath.as_ref().map(action::aftermath_key),
                &Some(action::aftermath_back(a, r)),
            ),
            Window::Combat(f) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &action::combat_key(g),
                &Some(crate::dual_window::fields_back(f, r)),
            ),
            // The invasion and the production step have no machine state: the placeholder stands for the string's window.
            Window::Untyped { kind: WindowKind::Invasion | WindowKind::Production, .. } => true,
            Window::Untyped { .. } => true,
        };
    }
    ok
}

// --- the held follower window ---------------------------------------------------------------------------------------

/// The core's outcome of the step, from the string-typed value the follower-window dual compared.
fn core_outcome(out: &StepOutcome, r: &Render) -> Option<Outcome> {
    let mut skipped = SmallList::new();
    for p in &out.skipped {
        skipped.push(r.seat(p.as_str())?);
    }
    let answered = match &out.answered {
        Some((p, how)) => Some((
            r.seat(p.as_str())?,
            match how {
                SecondaryResolution::Declined => Resolution::Declined,
                SecondaryResolution::Followed => Resolution::Followed,
                SecondaryResolution::Ineligible => return None,
            },
        )),
        None => None,
    };
    Some(outcome_of(skipped, answered, out.complete))
}

fn outcome_of(skipped: SmallList<Seat, 16>, answered: Option<(Seat, Resolution)>, complete: bool) -> Outcome {
    Outcome { skipped, answered, complete }
}

/// Move the held follower window on by the core's outcome of this step; `true` when it was moved (and closed when complete).
fn advance_held(pending: &mut Pending, out: &Outcome) -> bool {
    let Some(Window::StrategySecondary(w)) = pending.window_mut(WindowKind::StrategySecondary) else { return false };
    core_secondary::advance(w, out);
    if core_secondary::is_complete(w) {
        pending.close(WindowKind::StrategySecondary);
    }
    true
}

/// The plain events among `pushed` whose kind is one of `kinds`, as pushed.
fn pushed_named(pushed: &[crate::dual_window::Pushed], kinds: &[PlainKind]) -> Vec<String> {
    use crate::render::events::kind_text;
    pushed
        .iter()
        .filter_map(|p| match p {
            crate::dual_window::Pushed::Plain(t) if kinds.iter().any(|k| crate::dual_window::plain_name(t) == kind_text(*k)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

// --- the census of phase flow and reconcile hooks (W3, W4) -------------------------------------------------------------

/// The kinds of the windows a `Pending` holds.
fn kinds(stack: &[Window]) -> Vec<WindowKind> {
    stack.iter().map(Window::kind).collect()
}

/// Which window kinds the string opened and closed in this step (`d1::pending.open [bail: <kind>]`,
/// `d1::pending.close [bail: <kind>]`): the openers and closers of phase flow the core does not run yet, the worklist of
/// D1b's `Session::step`. Counted, never compared.
fn census_flow(held: &Pending, observed: &[Window]) {
    let (before, now) = (kinds(&held.stack), kinds(observed));
    for kind in &now {
        if !before.contains(kind) {
            note_bail("d1::pending.open", &format!("{kind:?}"));
        }
    }
    for kind in &before {
        if !now.contains(kind) {
            note_bail("d1::pending.close", &format!("{kind:?}"));
        }
    }
}

/// A probe of the per-step reconcile hooks (`starting_tech::settle` ... `roll_spec_ops`): the state before a hook, taken
/// only under the D1 dual. [`probed`] counts the hooks that changed it (`d1::reconcile [bail: <hook>]`), the lent bodies the
/// core step would have to run first; none of them is ported.
pub(super) fn probe(g: &Game<'_>) -> Option<serde_json::Value> {
    if active() { serde_json::to_value(&g.state).ok() } else { None }
}

/// Count `hook` when the state differs from the probe taken before it ran; the probe is refreshed for the next hook.
pub(super) fn probed(g: &Game<'_>, probe: &mut Option<serde_json::Value>, hook: &str) {
    let Some(before) = probe.as_ref() else { return };
    let now = serde_json::to_value(&g.state).ok();
    if now.as_ref() != Some(before) {
        note_bail("d1::reconcile", hook);
        *probe = now;
    }
}

// --- the check ------------------------------------------------------------------------------------------------------

/// Compare the held `Pending` with the string fields after `Game::step`, then re-seed it from them.
pub(super) fn check(g: &mut Game<'_>, result: &StepResult) {
    let outcome = g.d1.outcome.take();
    check_arm(g);
    let pushed = crate::dual_window::step_capture_take();
    for field in untyped_open(g) {
        note_bail("d1::pending.untyped", field);
    }
    if !any_open(g) && g.d1.pending.stack.is_empty() {
        return;
    }
    // (A step that closed the last window still reaches the census below: the held copy is not empty.)
    let Ok(cx) = crate::core_sync::Ctx::of(&g.state) else {
        note_bail(LABEL, "more than 16 owners");
        g.d1.pending = Pending::default();
        return;
    };
    let r = Render::new(cx.owners.ids().to_vec());
    let observed = match observe(g, &r, &cx) {
        Ok(w) => w,
        Err(why) => {
            note_bail(LABEL, why);
            g.d1.pending = Pending::default();
            return;
        }
    };
    census_flow(&g.d1.pending, &observed);
    let mut compared = false;
    let mut ok = true;
    // The held follower window, moved on by the core's own outcome of this step.
    let mut held = g.d1.pending.clone();
    let moved = result.error.is_none() && outcome.as_ref().and_then(|o| core_outcome(o, &r)).is_some_and(|o| advance_held(&mut held, &o));
    if moved {
        compared = true;
        let expected = held.window(WindowKind::StrategySecondary);
        let actual = observed.iter().find(|w| w.kind() == WindowKind::StrategySecondary);
        ok &= compare_value(Area::Driver, "d1::pending.secondary", &actual, &expected);
        if let Some(core) = outcome.as_ref().and_then(|o| core_outcome(o, &r)) {
            // The plain events of the step the core outcome decides (package P1-7), against what the string pushed.
            let expected: Vec<String> = core_secondary::plain_events(&core).as_slice().iter().filter_map(|p| plain_text(&r, &cx, p)).collect();
            let kinds = [PlainKind::StrategySecondaryDeclined, PlainKind::StrategySecondaryFollowed, PlainKind::StrategicActionComplete];
            ok &= compare_value(Area::Driver, "d1::pending.plain.secondary", &pushed_named(&pushed, &kinds), &expected);
        }
    } else if outcome.is_some() {
        note_bail(LABEL, "the held follower window did not take the step (not open, an error, or an id the table does not know)");
    }
    // The scoring window's book after the core half of this step's answer.
    if let Some((event, book, scored)) = g.d1.scoring.take() {
        let kind = if event { WindowKind::EventScoring } else { WindowKind::Scoring };
        match scoring_of(&book, &r) {
            Ok(typed) if result.error.is_none() => {
                compared = true;
                // A score that ends the game closes the window at once (`step_scoring`, `step_event_scoring`), pending seats and all.
                let expected = (!g.state.finished).then(|| if event { Window::EventScoring(typed) } else { Window::Scoring(typed) });
                let actual = observed.iter().find(|w| w.kind() == kind);
                ok &= compare_value(Area::Driver, "d1::pending.scoring", &actual, &expected.as_ref());
                // The `OBJECTIVE_SCORED` the string pushes after the answer is the core half's scored objective (package P1-7).
                let expected: Vec<String> = scored
                    .as_ref()
                    .and_then(|id| windows::objective_ref(id))
                    .and_then(|o| plain_text(&r, &cx, &PlainRec::one(PlainKind::ObjectiveScored, PlainArg::Obj(o))))
                    .into_iter()
                    .collect();
                ok &= compare_value(Area::Driver, "d1::pending.plain.scoring", &pushed_named(&pushed, &[PlainKind::ObjectiveScored]), &expected);
            }
            _ => note_bail(LABEL, "the scoring book of the core half could not be compared (an error, or a seat the table does not know)"),
        }
    }
    // The kinds without a core transition follow the string: what is compared is that the typed copy is total.
    if !observed.is_empty() {
        compared = true;
        ok &= roundtrip(g, &observed, &r, &cx);
    }
    note_window(LABEL, compared);
    let _ = ok;
    g.d1.pending.stack = observed;
}

#[cfg(test)]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::units::Unit;

    use super::*;
    use crate::choice::Table;
    use crate::dual::{AreaMask, report, take_report, with_mask};
    use crate::strategy_secondary_dual_tests::{Followers, dealt, pid};

    const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

    fn both() -> AreaMask {
        AreaMask::STRING.with(Area::Strategy, Backend::Dual).with(Area::Driver, Backend::Dual)
    }

    fn window_of(g: &Game<'_>) -> Option<Secondary> {
        match g.d1.pending.window(WindowKind::StrategySecondary) {
            Some(Window::StrategySecondary(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Play a strategic action of `a` to its end with the driver and the strategy area on Dual; returns what the held window
    /// looked like after each step (`(next, resolved)`, `None` once closed) and the report.
    fn play(name: &str, tokens: &[(&str, i32)], answer: &'static str) -> (Vec<Option<(u8, usize)>>, crate::dual::DualReport, Game<'static>) {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = dealt(&F3, name);
        for (who, n) in tokens {
            state.player_mut(&pid(who)).unwrap().strategic_tokens = *n;
        }
        let mut seen = Vec::new();
        let mut game = None;
        with_mask(both(), || {
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: answer })));
            for _ in 0..40 {
                let step = g.step();
                assert!(step.error.is_none(), "{:?}", step.error);
                seen.push(window_of(&g).map(|s| (s.next, s.resolved.len())));
                if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") {
                    break;
                }
            }
            game = Some(g);
        });
        let r = report();
        let _ = take_report();
        (seen, r, game.expect("played"))
    }

    #[test]
    fn the_follower_window_is_held_in_the_core_across_steps_and_agrees_with_the_string() {
        let (seen, r, g) = play("Politics", &[], "yes");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        // Opened by the strategic action, then one follower at a time (b asked, c asked), closed when none is left.
        assert!(seen.iter().flatten().count() >= 2, "the window stayed open over several steps: {seen:?}");
        assert!(seen.iter().flatten().any(|(next, _)| *next >= 1), "{seen:?}");
        assert_eq!(seen.last(), Some(&None), "closed with the action: {seen:?}");
        assert!(g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE"));
        let w = r.windows.get(LABEL).copied().unwrap_or_default();
        assert!(w.compared >= 2 && w.bailed == 0, "{:?}", r.windows);
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending [bail")), "{:?}", r.windows);
    }

    #[test]
    fn followers_without_a_token_are_skipped_by_the_core_outcome_and_the_window_closes_in_one_step() {
        let (seen, r, _) = play("Warfare", &[("b", 0), ("c", 0)], "yes");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        // Two ineligible followers are recorded and the window closes in the very step that skips them.
        assert_eq!(seen.last(), Some(&None), "{seen:?}");
        assert!(r.windows.get(LABEL).is_some_and(|w| w.compared >= 1), "{:?}", r.windows);
    }

    /// The mutation check of the harness half: a held window that does not take the core's skipped followers is a difference.
    #[test]
    fn a_held_window_that_forgets_the_skipped_followers_is_a_difference() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let state = dealt(&F3, "Warfare");
        with_mask(both(), || {
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
            for _ in 0..3 {
                if g.secondary.is_some() {
                    break;
                }
                let _ = g.step();
            }
            assert!(g.secondary.is_some(), "the action opened the follower window");
            // Hand the held copy a window that is one follower behind the string's: the next step must find it.
            let mut stale = window_of(&g).expect("held");
            stale.followers = SmallList::new();
            g.d1.pending.open(Window::StrategySecondary(stale));
            let _ = g.step();
        });
        let r = report();
        let _ = take_report();
        assert!(r.differences.iter().any(|d| d.label == "d1::pending.secondary"), "{:?} {:?}", r.differences, r.windows);
    }

    /// The status phase of a real position: the scoring window opens over both seats, seat a scores a public objective, the book
    /// moves on by the core's outcome of the answer and the window closes.
    #[test]
    fn the_scoring_window_is_held_and_its_book_agrees_with_the_core_half_after_every_answer() {
        use ti4_model::id::{ObjectiveId, PlanetId, SystemId};
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = crate::fixtures::seated_game(&[("a", "letnev"), ("b", "sol")], ti4_model::content_types::POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), ti4_model::content_types::POK);
        let planets: Vec<String> = catalogue
            .iter()
            .filter(|(_, planet)| planet.homeworld_of().is_none() && !planet.is_placed_during_play())
            .map(|(id, _)| (*id).to_owned())
            .take(6)
            .collect();
        for planet in planets {
            let system = catalogue.get(planet.as_str()).and_then(ti4_content::galaxy::Planet::system_id).unwrap_or("18");
            state.system_mut(&SystemId::new(system)).set_control(PlanetId::new(planet), pid("a"));
        }
        state.phase = ti4_model::state::Phase::Status;
        let mut seen = Vec::new();
        let mut finished = None;
        with_mask(both().with(Area::Objectives, Backend::Dual), || {
            let (decider, _) = crate::choice::Capturing::new(Box::new(crate::choice::Scripted::new(["expand_borders".to_owned()])));
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(decider)));
            for _ in 0..12 {
                let step = g.step();
                assert!(step.error.is_none(), "{:?}", step.error);
                seen.push(match g.d1.pending.window(WindowKind::Scoring) {
                    Some(Window::Scoring(sc)) => Some((sc.pending.len(), sc.public_used.len())),
                    _ => None,
                });
                if g.state.phase != ti4_model::state::Phase::Status {
                    break;
                }
            }
            finished = Some(g.state.player(&pid("a")).map(|p| p.victory_points));
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert_eq!(finished, Some(Some(1)), "a scored the public objective: {seen:?}");
        let held: Vec<(usize, usize)> = seen.iter().flatten().copied().collect();
        assert_eq!(held.first(), Some(&(2, 0)), "opened over both seats: {seen:?}");
        assert!(held.iter().any(|(_, used)| *used == 1), "the book recorded the public score: {seen:?}");
        assert!(held.windows(2).any(|w| w[0].0 > w[1].0), "the pending seats shrink as seats answer: {seen:?}");
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending [bail")), "{:?}", r.windows);
    }

    /// Found by the golden dual (gate of D1a): a score that reaches the target ends the game and the string closes the window with
    /// seats still pending; the held copy kept the book the core half left.
    #[test]
    fn a_score_that_ends_the_game_closes_the_held_scoring_window() {
        use ti4_model::id::{ObjectiveId, PlanetId, SystemId};
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = crate::fixtures::seated_game(&[("a", "letnev"), ("b", "sol")], ti4_model::content_types::POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), ti4_model::content_types::POK);
        let planets: Vec<String> = catalogue
            .iter()
            .filter(|(_, planet)| planet.homeworld_of().is_none() && !planet.is_placed_during_play())
            .map(|(id, _)| (*id).to_owned())
            .take(6)
            .collect();
        for planet in planets {
            let system = catalogue.get(planet.as_str()).and_then(ti4_content::galaxy::Planet::system_id).unwrap_or("18");
            state.system_mut(&SystemId::new(system)).set_control(PlanetId::new(planet), pid("a"));
        }
        state.player_mut(&pid("a")).unwrap().victory_points = 9;
        state.phase = ti4_model::state::Phase::Status;
        let mut finished = false;
        with_mask(both().with(Area::Objectives, Backend::Dual), || {
            let (decider, _) = crate::choice::Capturing::new(Box::new(crate::choice::Scripted::new(["expand_borders".to_owned()])));
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(decider)));
            for _ in 0..6 {
                let step = g.step();
                assert!(step.error.is_none(), "{:?}", step.error);
                if g.state.finished {
                    break;
                }
            }
            finished = g.state.finished && g.scoring.is_none() && g.d1.pending.window(WindowKind::Scoring).is_none();
        });
        let r = report();
        let _ = take_report();
        assert!(finished, "the game ended and no window is held: {:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    }

    fn checked(g: &mut Game<'static>) -> (crate::dual::DualReport, Pending) {
        let _ = take_report();
        let result = g.result(false, None);
        with_mask(both(), || check(g, &result));
        let r = report();
        let _ = take_report();
        (r, g.d1.pending.clone())
    }

    #[test]
    fn the_other_typed_windows_turn_back_into_their_string_fields() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let card = state.player(&pid("a")).unwrap().strategy_cards[0].clone();
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.executive_order = Some((pid("b"), pid("c")));
        g.leader_strategy = Some((pid("a"), ti4_model::id::LeaderId::new(crate::factions::mahact::AGENT), card.clone()));
        g.agenda_queue_after_event_scoring = Some(vec!["checks".to_owned(), "committee".to_owned()]);
        g.secondary_after_tactical = Some(StrategySecondaryWindow::foreign(pid("a"), card, vec![pid("b"), pid("c")]));
        g.scoring = Some(crate::objectives::ScoringWindow::new(&[pid("c"), pid("a"), pid("b")]));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let kinds: Vec<WindowKind> = held.stack.iter().map(Window::kind).collect();
        assert_eq!(
            kinds,
            [
                WindowKind::StrategySecondaryAfterTactical,
                WindowKind::LeaderStrategy,
                WindowKind::ExecutiveOrder,
                WindowKind::Scoring,
                WindowKind::AgendaQueueAfterEventScoring
            ],
            "{:?} {:?}",
            held.stack,
            r.windows
        );
        assert_eq!(held.window(WindowKind::ExecutiveOrder), Some(&Window::ExecutiveOrder { owner: Seat(1), speaker: Seat(2) }));
        match held.window(WindowKind::AgendaQueueAfterEventScoring) {
            Some(Window::AgendaQueueAfterEventScoring(q)) => assert_eq!(q.len(), 2),
            other => panic!("{other:?}"),
        }
        // Closing the string fields closes the held copies.
        g.executive_order = None;
        g.agenda_queue_after_event_scoring = None;
        g.leader_strategy = None;
        g.secondary_after_tactical = None;
        g.scoring = None;
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(held.stack.is_empty());
    }

    #[test]
    fn an_agenda_or_seat_the_tables_do_not_know_is_a_counted_bail_not_a_difference() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.agenda_queue_after_event_scoring = Some(vec!["a_homebrew_agenda".to_owned()]);
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.windows.keys().any(|k| k == "d1::pending [bail: an agenda the tables do not know]"), "{:?}", r.windows);
        assert!(held.stack.is_empty());
        g.agenda_queue_after_event_scoring = None;
        g.executive_order = Some((PlayerId::new("nobody"), pid("a")));
        let (r, _) = checked(&mut g);
        assert!(r.windows.keys().any(|k| k.contains("a seat the table does not know")), "{:?}", r.windows);
    }

    #[test]
    fn windows_without_a_typed_payload_are_counted_by_field() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.agenda_talks = Some(super::super::AgendaTalks { alias: "checks".to_owned(), choices: vec![], queue: vec![], seats: vec![] });
        let (r, _) = checked(&mut g);
        assert!(r.windows.keys().any(|k| k == "d1::pending.untyped [bail: agenda_talks]"), "{:?}", r.windows);
    }

    // --- the windows of D1a's remainder: leader followers, tokens, voting, trade ---------------------------------------------

    fn winnu_game() -> Game<'static> {
        const W: [(&str, &str); 3] = [("a", "winnu"), ("b", "hacan"), ("c", "arborec")];
        let state = dealt(&W, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let card = g.state.player(&pid("a")).unwrap().strategy_cards[0].clone();
        g.leader_strategy = Some((pid("a"), ti4_model::id::LeaderId::new("winnuhero"), card));
        g
    }

    #[test]
    fn the_follower_selection_is_typed_by_the_seats_still_to_ask_and_turns_back_into_the_questions() {
        let _serial = crate::dual::tests::serial();
        let mut g = winnu_game();
        let (player, leader, card) = g.leader_strategy.clone().unwrap();
        let mut choices = crate::factions::leader_strategy_followers(&g.state, &player, &leader, &card);
        assert_eq!(choices.len(), 2, "b and c are asked, clockwise from a");
        g.leader_followers = Some((choices.clone(), Vec::new()));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        match held.window(WindowKind::LeaderFollowers) {
            Some(Window::LeaderFollowers(w)) => assert_eq!((w.asks.as_slice(), w.followers.len()), (&[Seat(1), Seat(2)][..], 0)),
            other => panic!("{other:?}"),
        }
        assert!(r.windows.keys().all(|k| !k.contains("untyped [bail: leader_followers]")), "{:?}", r.windows);
        // The first question is answered "yes": one seat left to ask, one follower.
        choices.remove(0);
        g.leader_followers = Some((choices, vec![pid("b")]));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        match held.window(WindowKind::LeaderFollowers) {
            Some(Window::LeaderFollowers(w)) => assert_eq!((w.asks.as_slice(), w.followers.as_slice()), (&[Seat(2)][..], &[Seat(1)][..])),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_follower_selection_without_a_hero_is_a_difference_of_the_roundtrip() {
        let _serial = crate::dual::tests::serial();
        let mut g = winnu_game();
        let (player, leader, card) = g.leader_strategy.clone().unwrap();
        let choices = crate::factions::leader_strategy_followers(&g.state, &player, &leader, &card);
        g.leader_followers = Some((choices, Vec::new()));
        g.leader_strategy = None;
        let (r, _) = checked(&mut g);
        assert!(r.differences.iter().any(|d| d.label == "d1::pending.roundtrip"), "{:?}", r.differences);
    }

    fn token_window(g: &Game<'_>) -> (crate::tokens::TokenGain, Box<crate::status::StatusPhaseReport>) {
        let order = [pid("b"), pid("a"), pid("c")];
        let gain = crate::tokens::TokenGain::from_parts(
            vec![pid("a"), pid("a"), pid("c"), pid("c")],
            vec![crate::tokens::TokenPlacement { player: pid("b"), pool: ti4_model::state::TokenPool::Fleet }],
        );
        let report = crate::status::StatusPhaseReport {
            action_card_draws: vec![pid("c")],
            revealed_objective: Some(ti4_model::id::ObjectiveId::new(g.state.revealed_objectives.first().map_or("expand_borders", |o| o.as_str()))),
            action_cards_drawn: vec![(pid("b"), 1), (pid("a"), 1)],
            returned_command_tokens: vec![(ti4_model::id::SystemId::new("18"), pid("a"))],
            crown_of_emphidia: Some(pid("c")),
            readied_planets: vec![],
            readied_leaders: vec![(pid("a"), 3)],
            returned_strategy_cards: vec![(pid("a"), g.state.player(&pid("a")).unwrap().strategy_cards[0].clone())],
            initiative_order: order.to_vec(),
            repaired_units: 2,
            game_ended: false,
        };
        (gain, Box::new(report))
    }

    #[test]
    fn the_token_gain_and_its_report_are_typed_and_turn_back_into_the_string_field() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.tokens = Some(token_window(&g));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let Some(Window::Tokens(t)) = held.window(WindowKind::Tokens) else { panic!("{:?}", held.stack) };
        assert_eq!(t.pending.as_slice(), [Seat(0), Seat(0), Seat(2), Seat(2)], "the next token first");
        assert_eq!(t.placed.as_slice(), [(Seat(1), ti4_engine_core::choice::strategy::Pool::Fleet)]);
        assert_eq!((t.report.repaired_units, t.report.crown_of_emphidia, t.report.initiative_order.as_slice()), (2, Some(Seat(2)), &[Seat(1), Seat(0), Seat(2)][..]));
        assert!(t.report.revealed_objective.is_some() && t.report.returned_command_tokens.len() == 1);
        assert!(r.windows.keys().all(|k| !k.contains("untyped [bail: tokens]")), "{:?}", r.windows);
        // The roundtrip catches a typed window that lost a placed token.
        let cx = crate::core_sync::Ctx::of(&g.state).unwrap();
        let rr = Render::new(cx.owners.ids().to_vec());
        let mut broken = (**t).clone();
        broken.placed = SmallList::new();
        let _ = take_report();
        let ok = with_mask(both(), || roundtrip(&g, &[Window::Tokens(Box::new(broken))], &rr, &cx));
        let r = report();
        let _ = take_report();
        assert!(!ok && r.differences.iter().any(|d| d.label == "d1::pending.roundtrip"), "{:?}", r.differences);
    }

    #[test]
    fn a_report_with_an_objective_the_tables_do_not_know_is_a_counted_bail() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let (gain, mut report) = token_window(&g);
        report.revealed_objective = Some(ti4_model::id::ObjectiveId::new("a_homebrew_objective"));
        g.tokens = Some((gain, report));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.windows.keys().any(|k| k == "d1::pending [bail: an objective the tables do not know]"), "{:?}", r.windows);
        assert!(held.stack.is_empty());
    }

    #[test]
    fn the_vote_in_progress_is_typed_and_turns_back_into_the_string_window() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        // A law voted For or Against, nobody has voted yet, and a second agenda waits.
        let open = crate::vote::VoteWindow::new(&g.state, "checks", vec!["for".to_owned(), "against".to_owned()]);
        g.voting = Some((Box::new(open), vec!["committee".to_owned()]));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let Some(Window::Voting(v)) = held.window(WindowKind::Voting) else { panic!("{:?} {:?}", held.stack, r.windows) };
        assert_eq!((v.vote.agenda, v.vote.stage, v.vote.choices.len(), v.queue.as_slice()), (Agenda::ChecksAndBalances, ti4_engine_core::state::VoteStage::Outcome(0), 2, &[Agenda::CommitteeFormation][..]));
        assert_eq!(v.vote.order.len(), 3, "everyone votes");
        // A finished election of a player, with a ballot, counts and a winner.
        let done = crate::vote::VoteWindow::finished("censure", &["a", "b", "c"], &[("a", "b"), ("c", "a")], &[("b", 2), ("a", 1)], Some("b"));
        g.voting = Some((Box::new(done), Vec::new()));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let Some(Window::Voting(v)) = held.window(WindowKind::Voting) else { panic!("{:?} {:?}", held.stack, r.windows) };
        assert_eq!(v.vote.stage, ti4_engine_core::state::VoteStage::Done(Some(ti4_engine_core::keys::Outcome::Seat(Seat(1)))));
        assert_eq!(v.vote.ballot.as_slice(), [(Seat(0), ti4_engine_core::keys::Outcome::Seat(Seat(1))), (Seat(2), ti4_engine_core::keys::Outcome::Seat(Seat(0)))]);
        assert_eq!(v.vote.counts.len(), 2);
    }

    #[test]
    fn a_vote_on_an_agenda_the_tables_do_not_know_is_a_counted_bail() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let done = crate::vote::VoteWindow::finished("a_homebrew_agenda", &["for"], &[], &[], None);
        g.voting = Some((Box::new(done), Vec::new()));
        let (r, held) = checked(&mut g);
        assert!(r.windows.keys().any(|k| k == "d1::pending [bail: an agenda the tables do not know]"), "{:?}", r.windows);
        assert!(held.stack.is_empty());
    }

    #[test]
    fn an_open_transaction_is_typed_by_its_chairs_and_turns_back_into_the_string_window() {
        let _serial = crate::dual::tests::serial();
        let mut state = dealt(&F3, "Politics");
        let window = crate::transactions::TradeWindow::open(&mut state, &pid("a"), &pid("b"));
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.trade = Some(window);
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let Some(Window::Trade(t)) = held.window(WindowKind::Trade) else { panic!("{:?} {:?}", held.stack, r.windows) };
        assert_eq!((t.proposer, t.partner, t.rounds_left, t.deal), (Seat(0), Seat(1), 2, None));
        assert!(r.windows.keys().all(|k| !k.contains("untyped [bail: trade]")), "{:?}", r.windows);
        // The roundtrip catches a typed window with the chairs swapped.
        let cx = crate::core_sync::Ctx::of(&g.state).unwrap();
        let rr = Render::new(cx.owners.ids().to_vec());
        let mut swapped = t.clone();
        std::mem::swap(&mut swapped.proposer, &mut swapped.partner);
        let _ = take_report();
        let ok = with_mask(both(), || roundtrip(&g, &[Window::Trade(swapped)], &rr, &cx));
        let r = report();
        let _ = take_report();
        assert!(!ok && r.differences.iter().any(|d| d.label == "d1::pending.roundtrip"), "{:?}", r.differences);
    }

    #[test]
    fn the_windows_the_string_opens_and_closes_are_counted_by_kind() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.executive_order = Some((pid("b"), pid("c")));
        let (r, _) = checked(&mut g);
        assert_eq!(r.windows.get("d1::pending.open [bail: ExecutiveOrder]").map(|w| w.bailed), Some(1), "{:?}", r.windows);
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending.close")), "{:?}", r.windows);
        // The window stays open: nothing new is counted. Closing it is counted once.
        let (r, _) = checked(&mut g);
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending.open") || k.starts_with("d1::pending.close")), "{:?}", r.windows);
        g.executive_order = None;
        let (r, held) = checked(&mut g);
        assert_eq!(r.windows.get("d1::pending.close [bail: ExecutiveOrder]").map(|w| w.bailed), Some(1), "{:?}", r.windows);
        assert!(held.stack.is_empty());
    }

    #[test]
    fn a_reconcile_hook_that_changes_the_state_is_counted_by_name_and_one_that_does_not_is_not() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let _ = take_report();
        with_mask(both(), || {
            let mut p = probe(&g);
            assert!(p.is_some(), "the probe is taken under the D1 dual");
            probed(&g, &mut p, "quiet_hook");
            g.state.nexus_unlocked = !g.state.nexus_unlocked;
            probed(&g, &mut p, "nexus_unlocked");
            probed(&g, &mut p, "after_it");
        });
        let r = report();
        let _ = take_report();
        let keys: Vec<&String> = r.windows.keys().filter(|k| k.starts_with("d1::reconcile")).collect();
        assert_eq!(keys, ["d1::reconcile [bail: nexus_unlocked]"], "{:?}", r.windows);
        // Off the dual the probe is free: nothing is taken.
        with_mask(AreaMask::STRING, || assert!(probe(&g).is_none()));
    }

    // --- the windows of the action phase: tactical and aftermath (package D1b-1) -----------------------------------------------

    fn notes() -> crate::combat::NoteHoldings {
        std::iter::once((pid("a"), std::iter::once(pid("b")).collect())).collect()
    }

    fn tactical_game(stage: impl FnOnce(&Game<'static>) -> super::super::TacticalStage) -> Game<'static> {
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let stage = stage(&g);
        g.tactical = Some(super::super::TacticalWindow { player: pid("a"), stage, notes_at_start: notes() });
        g
    }

    fn held_tactical(g: &mut Game<'static>) -> ti4_engine_core::state::Tactical {
        let (r, held) = checked(g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending [bail")), "{:?}", r.windows);
        match held.window(WindowKind::Tactical) {
            Some(Window::Tactical(t)) => t.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_tactical_action_is_typed_in_the_activating_and_moving_stages_with_its_note_holdings() {
        use ti4_engine_core::state::TStage;
        let _serial = crate::dual::tests::serial();
        let mut g = tactical_game(|_| super::super::TacticalStage::Activating);
        let t = held_tactical(&mut g);
        assert_eq!((t.player, t.stage.clone(), t.notes_at_start.as_slice()), (Seat(0), TStage::Activating, &[(Seat(0), Seat(1))][..]));
        g.tactical.as_mut().unwrap().stage = super::super::TacticalStage::Moving;
        assert_eq!(held_tactical(&mut g).stage, TStage::Moving);
        g.tactical = None;
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(held.stack.is_empty());
    }

    #[test]
    fn a_tactical_action_with_a_hold_open_keeps_the_ship_its_route_and_what_is_aboard() {
        use ti4_engine_core::state::TStage;
        let _serial = crate::dual::tests::serial();
        let mut g = tactical_game(|g| {
            let home = g.state.player(&pid("a")).unwrap().home_system.clone().unwrap();
            let ship = Unit::new(ti4_model::id::UnitTypeId::new("carrier"), pid("a"));
            let hold = crate::transit::CargoWindow::for_ship(&g.state, g.content, g.sources, &pid("a"), &home, &ship, &[]);
            super::super::TacticalStage::Loading { origin: home, ship: Box::new(ship.sustained()), path: vec!["18".to_owned()], window: Box::new(hold) }
        });
        let t = held_tactical(&mut g);
        let TStage::Loading { ship, path, loaded, closed, .. } = t.stage else { panic!("{:?}", t.stage) };
        assert!(ship.sustained() && !ship.galvanized());
        assert_eq!((path.len(), loaded.len(), closed), (1, 0, false));
    }

    #[test]
    fn a_tactical_payload_that_forgets_the_hold_or_the_notes_is_a_difference_of_the_roundtrip() {
        let _serial = crate::dual::tests::serial();
        let mut g = tactical_game(|_| super::super::TacticalStage::Moving);
        let cx = crate::core_sync::Ctx::of(&g.state).unwrap();
        let r = Render::new(cx.owners.ids().to_vec());
        let Some(Window::Tactical(t)) = super::action::tactical_of(&g, &r).unwrap() else { panic!() };
        let mut no_notes = t.clone();
        no_notes.notes_at_start = SmallList::new();
        let mut other_stage = t.clone();
        other_stage.stage = ti4_engine_core::state::TStage::Activating;
        let _ = take_report();
        let verdict = with_mask(both(), || {
            [Window::Tactical(t), Window::Tactical(no_notes), Window::Tactical(other_stage)].map(|w| roundtrip(&g, &[w], &r, &cx))
        });
        let _ = take_report();
        assert_eq!(verdict, [true, false, false]);
        g.tactical = None;
    }

    fn aftermath_game(stage: impl FnOnce(&mut Game<'static>, &SystemId) -> super::super::Aftermath, pause: bool) -> Game<'static> {
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let system = g.state.player(&pid("a")).unwrap().home_system.clone().unwrap();
        let stage = stage(&mut g, &system);
        let before_combat = crate::combat::before_combat_with_notes(&g.state, g.content, g.sources, &system, notes());
        g.aftermath = Some(super::super::AftermathWindow {
            player: pid("a"),
            system,
            stage,
            log: Vec::new(),
            before_combat,
            feats_noted: true,
            pending_event_scoring: pause.then_some((ti4_model::state::FeatOccurrence(7), crate::objectives::EventScoreLimit::OnePerPlayer)),
            notes_at_tactical_start: notes(),
        });
        g
    }

    fn held_aftermath(g: &mut Game<'static>) -> ti4_engine_core::state::Aftermath {
        let (r, held) = checked(g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending [bail")), "{:?}", r.windows);
        match held.window(WindowKind::Aftermath) {
            Some(Window::Aftermath(a)) => (**a).clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_aftermath_is_typed_in_each_stage_with_the_system_before_the_fight() {
        use ti4_engine_core::state::{AStage, PauseLimit};
        let _serial = crate::dual::tests::serial();
        type A = super::super::Aftermath;
        let mut g = aftermath_game(|g, system| A::Fighting(Box::new(crate::combat::CombatWindow::new(&g.state, g.content, g.sources, system))), false);
        let a = held_aftermath(&mut g);
        assert_eq!((a.stage, a.player, a.feats_noted, a.event_scoring), (AStage::Fighting, Seat(0), true, None));
        assert_eq!(a.before_combat.sides.len(), 1, "only the owner has ships at home");
        assert_eq!(a.before_combat.notes.as_slice(), &[(Seat(0), Seat(1))]);
        assert_eq!(a.notes_at_start, a.before_combat.notes);
        let mut g = aftermath_game(|_, _| A::Negotiating { invade: true }, true);
        let a = held_aftermath(&mut g);
        assert_eq!((a.stage, a.event_scoring), (AStage::Negotiating { invade: true }, Some((7, PauseLimit::OnePerPlayer))));
        let mut g = aftermath_game(|g, system| A::Invading(Box::new(crate::invasion::InvasionWindow::at_commit_step(&mut g.state, &pid("a"), system))), false);
        assert_eq!(held_aftermath(&mut g).stage, AStage::Invading);
        let mut g = aftermath_game(|g, system| A::Producing(Box::new(crate::production::ProductionWindow::new(&g.state, g.content, g.sources, &pid("a"), system))), false);
        assert_eq!(held_aftermath(&mut g).stage, AStage::Producing);
        let mut g = aftermath_game(|_, _| A::Done, false);
        assert_eq!(held_aftermath(&mut g).stage, AStage::Done);
    }

    #[test]
    fn an_aftermath_payload_with_the_invade_flag_swapped_or_the_pause_dropped_is_a_difference_of_the_roundtrip() {
        use ti4_engine_core::state::AStage;
        let _serial = crate::dual::tests::serial();
        let g = aftermath_game(|_, _| super::super::Aftermath::Negotiating { invade: true }, true);
        let cx = crate::core_sync::Ctx::of(&g.state).unwrap();
        let r = Render::new(cx.owners.ids().to_vec());
        let Some(Window::Aftermath(a)) = super::action::aftermath_of(&g, &r).unwrap() else { panic!() };
        let mut swapped = a.clone();
        swapped.stage = AStage::Negotiating { invade: false };
        let mut no_pause = a.clone();
        no_pause.event_scoring = None;
        let mut feats = a.clone();
        feats.feats_noted = false;
        let mut capitals = a.clone();
        capitals.before_combat.capitals.push((Seat(1), 1));
        let _ = take_report();
        let verdict = with_mask(both(), || {
            [a, swapped, no_pause, feats, capitals].map(|w| roundtrip(&g, &[Window::Aftermath(w)], &r, &cx))
        });
        let _ = take_report();
        assert_eq!(verdict, [true, false, false, false, false]);
    }

    #[test]
    fn the_capitals_of_a_before_combat_are_read_through_the_accessor() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
        let system = ti4_model::id::SystemId::new("18");
        crate::fixtures::put(&mut state, &system, "warsun", &pid("a"), 2);
        crate::fixtures::put(&mut state, &system, "cruiser", &pid("b"), 1);
        let before = crate::combat::before_combat(&state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &system);
        assert_eq!(super::action::capitals_of(&before), vec![("a".to_owned(), 2), ("b".to_owned(), 0)], "one entry per side, in the map's order");
    }

    // --- the precedence of the step loop (package D1b-7) -----------------------------------------------------------------------

    #[test]
    fn the_arm_of_every_step_of_a_whole_round_is_the_one_the_core_precedence_picks() {
        let _serial = crate::dual::tests::serial();
        let players = [PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c")];
        let state = crate::setup::start_game(ContentStore::embedded(), &players, ti4_model::content_types::POK, None).unwrap();
        let _ = take_report();
        let steps = with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            let mut g = Game::with_seeded_random(state, ContentStore::embedded(), 42);
            let mut steps = 0;
            while g.state.round < 2 && !g.state.finished && steps < 3000 {
                let r = g.step();
                assert!(r.error.is_none(), "{:?}", r.error);
                steps += 1;
            }
            steps
        });
        let r = report();
        let _ = take_report();
        assert!(steps > 20, "a round is many steps: {steps}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let compared: u64 = r.areas.get(&Area::Driver).map_or(0, |c| c.compared.iter().sum());
        assert!(compared >= steps as u64, "every step compared its arm: {compared} of {steps}");
    }

    #[test]
    fn an_arm_the_core_does_not_pick_is_a_difference() {
        use ti4_engine_core::driver::session::{Facts, step_arm};
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        let _ = take_report();
        let ok = with_mask(both(), || {
            snapshot_facts(&mut g);
            let facts: Facts = g.d1.facts.unwrap();
            // The string is made to say it took the status arm in the action phase: a difference.
            g.d1.arm = Some("status");
            let before = arm_name(step_arm(&facts, true));
            check_arm(&mut g);
            before
        });
        let r = report();
        let _ = take_report();
        assert_eq!(ok, "turn_prelude", "the dealt position has an active seat whose turn is not prepared");
        assert_eq!(r.total_differences(), 1, "{:?}", r.differences);
    }

    #[test]
    fn a_step_of_the_aftermath_chain_that_goes_where_the_core_does_not_is_a_difference() {
        type A = super::super::Aftermath;
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        with_mask(both(), || {
            // The chain goes where the core's table says.
            transition(Ended::Producing, &A::Done);
            transition(Ended::Fighting(true, true), &A::Negotiating { invade: true });
            // The invade flag swapped, the pause skipped, the end of production not reached: three differences.
            transition(Ended::Fighting(true, true), &A::Negotiating { invade: false });
            transition(Ended::Invading(true), &A::Done);
            transition(Ended::Producing, &A::Negotiating { invade: false });
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 3, "{:?}", r.differences);
    }
}
