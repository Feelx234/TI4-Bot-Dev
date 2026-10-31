//! The steps after a movement step as ONE window the core drives and holds (lane M5; the string `AftermathWindow::{new, settle, next_step,
//! enter_production}` and `Game::{finish_tactical_inner, step_aftermath_inner}` after the head). L5: it chains the space combat
//! (`driver::windows::combat`), the feats and the scoring pause they open, the end of the fight (`aftermath::FightOver`: the win announcement,
//! every fleet's enforcement), the invasion (`driver::windows::invasion`) and the production step, in the order of the string, and keeps where
//! the chain stands as the typed payload [`crate::state::Aftermath`] on the `Pending` stack. The tactical action hands over by writing that
//! entry above its own ([`crate::state::TStage::Aftermath`]) and runs its tail once this entry has left the stack.
//!
//! # Stages and the string
//!
//! | `stage` / `fight`        | string                                        | who asks |
//! |--------------------------|-----------------------------------------------|----------|
//! | `Fighting` / `Opening`   | `CombatWindow::new` + `settle_open`           | none: `Window::Combat` goes on the stack |
//! | `Fighting` / `Fighting`  | `Aftermath::Fighting(window)`                 | the combat arm ([`super::combat`]) |
//! | `Fighting` / `Closed`    | `settle`: the feats, the scoring pause        | none |
//! | `Fighting` / `Announced` | `fight_over`: the win, the enforcement        | a unit to remove ([`FightOver`]) |
//! | `Invading`               | `Aftermath::Invading(window)`                 | the invasion arm ([`super::invasion`]) |
//! | `Producing`              | `Aftermath::Producing(window)`                | the production island (a counted gap while it has a capacity) |
//!
//! The pause for a scoring window (`Aftermath.event_scoring`) opens [`super::scoring`] above this entry; the arm that runs it has precedence and
//! this chain goes on when it has left the stack. The plain events the string pushes into `AftermathWindow::log` (`SPACE_COMBAT_RESOLVED`,
//! `INVASION_RESOLVED`, `PRODUCTION_RESOLVED`) are written to the queue when the step ends, after everything the step emitted.
//!
//! The contact pause (`Aftermath::Negotiating`) cannot be reached with diplomacy off: it is a counted gap.

use super::aftermath::{FightOver, OverFacts};
use super::combat::{advance_combat, ask_combat, open_combat, Opened};
use super::invasion::{advance_invasion, ask_invasion, open_at_commit, open_invasion};
use super::live_common::{advance_parkable, After};
use super::scoring::open_event_scoring;
use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::driver::aftermath::{announced, next, note_feats, Settled};
use crate::driver::live::{ask, Asking, Env, Live, LiveError, Out};
use crate::driver::windows::tactical::limits;
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::ability::{AftermathGap, Unknown};
use crate::kernel::fx;
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board, reach};
use crate::rules::combat::feats::{self, Outcome};
use crate::rules::combat::Rd;
use crate::state::{AFight, AStage, Aftermath, CoreState, PauseLimit, PlainRec, Window as OpenWindow, WindowKind};

/// The suspension points, as the payload names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Fighting,
    Over,
    Invading,
    Producing,
    Done,
}

/// The open aftermath.
#[derive(Debug, Clone)]
pub struct AftermathChain {
    /// The payload: the source of truth.
    pub p: Aftermath,
    /// The end of the fight while the enforcement of the fleets asks (rebuilt from the payload and the position).
    over: Option<FightOver>,
    /// The step ended with a sub-window or a scoring window on the stack above this entry: the entry stays.
    parked: bool,
    /// The scoring window the step opened (reported to the caller).
    opened: Vec<OpenWindow>,
}

impl AftermathChain {
    #[must_use]
    pub const fn new(p: Aftermath) -> Self {
        Self { p, over: None, parked: false, opened: Vec::new() }
    }

    fn bail(sx: &mut Sx<'_>, gap: AftermathGap) -> Next {
        if sx.tape.unknown.get().is_none() {
            sx.tape.unknown.set(Some(Unknown::UnportedAftermath(gap)));
        }
        Next::Done
    }

    /// The facts of the end of the fight (`cannon_dual::over_facts`) from the position.
    fn over_facts(&self, core: &CoreState, k: &Kernel, opponents: crate::state::Code) -> OverFacts {
        let (winner, rounds) = self.p.outcome.unwrap_or((None, 0));
        let pairs = core
            .players()
            .iter()
            .map(|p| p.seat)
            .filter(|seat| board::space_units(core, self.p.system).iter().any(|u| u.owner() == *seat))
            .map(|seat| (seat, self.p.system, limits(core, k, seat, self.p.system)))
            .collect();
        OverFacts {
            player: self.p.player,
            system: self.p.system,
            outcome: Outcome { winner, rounds },
            sides: self.p.before_combat.sides.clone(),
            announce: true,
            opponents_key: opponents,
            pairs,
        }
    }

    /// Open the invasion step (`next_step` with `invade`): `INVASION_BEGAN` and the bombardment are the invasion's first run.
    fn open_invasion(&mut self, sx: &mut Sx<'_>) {
        open_invasion(sx.state, self.p.player, self.p.system, self.p.notes_at_start.clone());
        self.p.invaded = true;
        self.parked = true;
    }

    /// The feats of the fight, once (`settle`'s `note_combat_event_feats`). `true`: they open a scoring pause.
    fn note_feats(&mut self, sx: &mut Sx<'_>) -> bool {
        let outcome = self.p.outcome;
        if !note_feats(self.p.feats_noted, outcome.is_some()) {
            return false;
        }
        self.p.feats_noted = true;
        let Some((winner, rounds)) = outcome else { return false };
        if self.p.occurrence == 0 {
            return false;
        }
        let rd = Rd::new(sx.state, sx.kernel);
        let list = feats::feats(rd, self.p.system, &self.p.before_combat, Outcome { winner, rounds });
        feats::record(sx.state, &list, self.p.occurrence);
        if list.is_empty() {
            return false;
        }
        self.p.event_scoring = Some((self.p.occurrence, PauseLimit::OnePerPlayer));
        true
    }

    /// The enforcement of every fleet in the system after the win is announced (`fight_over`).
    fn finish_fight(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        let opponents = sx.table.facts.combat.opponents;
        let mut over = match self.over.take() {
            Some(mut over) => {
                over.f.opponents_key = opponents;
                over
            }
            None => FightOver::new(self.over_facts(sx.state, sx.kernel, opponents)),
        };
        match over.step(sx) {
            Next::Continue(ask) => {
                self.p.fight = AFight::Announced;
                self.p.holds = over.holds;
                self.over = Some(over);
                Some(Next::Continue(ask))
            }
            Next::Done => {
                self.p.holds = over.holds;
                self.p.fight = AFight::Over;
                let settled = Settled::Fighting { holds: self.p.holds, contacts: false };
                if let Some(kind) = announced(settled, self.p.outcome.is_some_and(|(_, rounds)| rounds > 0)) {
                    self.p.log.push(kind);
                }
                self.p.stage = next(settled);
                None
            }
            other => Some(other),
        }
    }

    /// The stage `Fighting`. `None`: go round the loop again.
    fn fighting(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        match self.p.fight {
            AFight::Opening => {
                match open_combat(sx.state, sx.kernel, self.p.system) {
                    Opened::Fight => {
                        self.p.fight = AFight::Fighting;
                        self.parked = true;
                        return Some(Next::Done);
                    }
                    Opened::NoFight(winner) => {
                        // The string's settle presents the finished (empty) fight once.
                        sx.state.transient_mut().combat.presentation.edit().phase = sx.table.facts.combat.phases[4];
                        self.p.outcome = Some((winner, 0));
                        self.p.fight = AFight::Closed;
                    }
                }
                None
            }
            AFight::Fighting => {
                if sx.state.pending().window(WindowKind::Combat).is_some() {
                    self.parked = true;
                    return Some(Next::Done);
                }
                // The combat left the stack: its outcome is in the payload (`CombatLive::closed`).
                self.p.fight = AFight::Closed;
                None
            }
            AFight::Closed => {
                if self.note_feats(sx) {
                    return None;
                }
                self.finish_fight(sx)
            }
            AFight::Announced => self.finish_fight(sx),
            AFight::Over => {
                // (The skip to the commit step, or a fight that is over: the stage moves on.)
                self.p.stage = next(Settled::Fighting { holds: self.p.holds, contacts: false });
                None
            }
        }
    }

    /// The stage `Invading`.
    fn invading(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        if sx.state.pending().window(WindowKind::Invasion).is_some() {
            self.parked = true;
            return Some(Next::Done);
        }
        if !self.p.invaded {
            if self.p.fight == AFight::Over && self.p.feats_noted && !self.p.holds && self.p.outcome.is_none() {
                // The Sardakk hero's skip: the invasion opens at its commit step.
                open_at_commit(sx.state, self.p.player, self.p.system, self.p.notes_at_start.clone());
                self.p.invaded = true;
                self.parked = true;
                return Some(Next::Done);
            }
            self.open_invasion(sx);
            return Some(Next::Done);
        }
        // The invasion left the stack: it is resolved.
        if let Some(kind) = announced(Settled::Invading { contacts: false }, false) {
            self.p.log.push(kind);
        }
        self.p.stage = next(Settled::Invading { contacts: false });
        None
    }

    /// The stage `Producing` (`enter_production`): the step of a seat with no production at all asks nothing and ends the chain.
    fn producing(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        let cx = crate::rules::production::Cx::new(sx.state, sx.kernel, reach::scope_of(sx.kernel), reach::blanked(sx.state));
        if crate::rules::production::capacity(&cx, self.p.player, self.p.system) > 0 {
            return Some(Self::bail(sx, AftermathGap::ProductionStep));
        }
        if let Some(kind) = announced(Settled::Producing, false) {
            self.p.log.push(kind);
        }
        self.p.stage = next(Settled::Producing);
        None
    }
}

impl Window for AftermathChain {
    type Stage = Tag;

    fn stage(&self) -> Tag {
        match (self.p.stage, self.p.fight) {
            (AStage::Fighting, AFight::Announced) => Tag::Over,
            (AStage::Fighting | AStage::Negotiating { .. }, _) => Tag::Fighting,
            (AStage::Invading, _) => Tag::Invading,
            (AStage::Producing, _) => Tag::Producing,
            (AStage::Done, _) => Tag::Done,
        }
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.parked = false;
        let mut idle = 0_u8;
        loop {
            if sx.tape.unknown.get().is_some() {
                return Next::Done;
            }
            // A scoring pause: the window of the occurrence opens above this entry, which waits for it.
            if let Some((occurrence, limit)) = self.p.event_scoring.take() {
                let w = open_event_scoring(sx.state, occurrence, limit == PauseLimit::OnePerPlayer);
                self.opened.push(w);
                self.parked = true;
                return Next::Done;
            }
            let before = (self.p.stage, self.p.fight, self.p.invaded, self.p.feats_noted);
            let stopped = match self.p.stage {
                AStage::Fighting => self.fighting(sx),
                AStage::Invading => self.invading(sx),
                AStage::Producing => self.producing(sx),
                AStage::Negotiating { .. } => Some(Self::bail(sx, AftermathGap::Negotiating)),
                AStage::Done => return Next::Done,
            };
            if let Some(next) = stopped {
                return next;
            }
            idle = if before == (self.p.stage, self.p.fight, self.p.invaded, self.p.feats_noted) && self.p.event_scoring.is_none() { idle + 1 } else { 0 };
            if idle >= 3 {
                return Self::bail(sx, AftermathGap::NoProgress);
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.over.as_ref().and_then(|o| o.ask_of(s, k))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let opponents = sx.table.facts.combat.opponents;
        let over = self.over.as_mut().ok_or(Illegal)?;
        over.f.opponents_key = opponents;
        over.answer(sx, pick)
    }
}

impl Held for AftermathChain {
    const KIND: WindowKind = WindowKind::Aftermath;

    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self> {
        let OpenWindow::Aftermath(p) = core.pending().window(WindowKind::Aftermath)? else { return None };
        let mut me = Self::new((**p).clone());
        if p.stage == AStage::Fighting && p.fight == AFight::Announced {
            let f = me.over_facts(core, k, crate::state::Code(0));
            me.over = Some(FightOver::enforcing(f, p.holds, core, k));
        }
        Some(me)
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Aftermath(Box::new(self.p.clone())));
    }
}

/// The plain events the steps of the chain announced, written to the queue (`events.append(&mut window.log)`).
fn flush(log: &mut SmallList<crate::state::PlainKind, 4>, core: &mut CoreState) {
    for kind in log.as_slice().to_vec() {
        fx::plain(core, PlainRec::bare(kind));
    }
    *log = SmallList::new();
}

/// The seat whose action the open aftermath is (the tail of the tactical action reads it).
#[must_use]
pub fn actor(core: &CoreState) -> Option<Seat> {
    match core.pending().window(WindowKind::Aftermath) {
        Some(OpenWindow::Aftermath(a)) => Some(a.player),
        _ => None,
    }
}

/// The system of the open aftermath.
#[must_use]
pub fn system_of(core: &CoreState) -> Option<System> {
    match core.pending().window(WindowKind::Aftermath) {
        Some(OpenWindow::Aftermath(a)) => Some(a.system),
        _ => None,
    }
}

fn is_open(core: &CoreState, kind: WindowKind) -> bool {
    core.pending().window(kind).is_some()
}

/// The question the open aftermath waits on (the arm's `AskFn`): the question of the sub-window on top, else the chain's own (an enforcement).
///
/// # Errors
/// As [`ask`].
pub fn ask_aftermath(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    if is_open(core, WindowKind::EventScoring) {
        return Ok(None);
    }
    if is_open(core, WindowKind::Combat) {
        return ask_combat(core, env);
    }
    if is_open(core, WindowKind::Invasion) {
        return ask_invasion(core, env);
    }
    ask::<AftermathChain>(core, env)
}

/// Run the open aftermath to its next question or its end (the arm's `LiveFn`). One call is one string `step_aftermath`: the sub-window on top
/// (the fight, the invasion) gets the answer, and whatever its end hands on runs until the next question or until a scoring window is opened
/// above the aftermath. `Live::Done` is the end of the STEP: the aftermath has left the stack, or it stays on it with a scoring window above.
///
/// # Errors
/// See [`LiveError`]; the position is then what it was before the call.
pub fn advance_aftermath(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    let result = advance_inner(core, env, answer);
    if result.is_err() {
        *core = before;
    }
    result
}

/// The resolver's seat order is the position's initiative order: the windows of an emission go to the seats in that order, and a resolver
/// carried by a position that never set it (the first window of a game) would offer them to the first seat alone.
pub fn sync_initiative(core: &mut CoreState) {
    let order: SmallList<Seat, 8> = crate::query::seats::initiative_order(core).iter().copied().collect();
    core.pending_mut().resolver.initiative = order;
}

fn merge(out: &mut Out, more: Out) {
    out.emitted.extend(more.emitted);
    out.opened.extend(more.opened);
}

fn flush_open(core: &mut CoreState) {
    let mut log = SmallList::new();
    if let Some(OpenWindow::Aftermath(a)) = core.pending_mut().window_mut(WindowKind::Aftermath) {
        std::mem::swap(&mut log, &mut a.log);
    }
    flush(&mut log, core);
}

fn advance_inner(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    if !is_open(core, WindowKind::Aftermath) {
        return Err(LiveError::NotOpen);
    }
    sync_initiative(core);
    let mut out = Out::default();
    let mut answer = answer;
    // (A step that runs sub-windows back to back always makes progress; the bound only guards a defect.)
    for _ in 0..64 {
        if is_open(core, WindowKind::EventScoring) {
            flush_open(core);
            return Ok((Live::Done, out));
        }
        let sub = if is_open(core, WindowKind::Combat) {
            Some(advance_combat(core, env, answer.take())?)
        } else if is_open(core, WindowKind::Invasion) {
            Some(advance_invasion(core, env, answer.take())?)
        } else {
            None
        };
        let (live, more) = match sub {
            Some(r) => r,
            None => advance_parkable::<AftermathChain>(core, env, answer.take(), &|w, core, out| {
                out.opened.extend(std::mem::take(&mut w.opened));
                if w.parked {
                    After::Park
                } else {
                    flush(&mut w.p.log, core);
                    After::Close
                }
            })?,
        };
        merge(&mut out, more);
        match live {
            Live::Asks(a) => {
                if a.nested.is_none() {
                    flush_open(core);
                }
                return Ok((Live::Asks(a), out));
            }
            Live::Done => {
                if !is_open(core, WindowKind::Aftermath) {
                    return Ok((Live::Done, out));
                }
            }
        }
    }
    Err(LiveError::Driver(crate::driver::DriverError::Contract(crate::driver::Breach::ContinueWithoutQuestion)))
}
