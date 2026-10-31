//! The agenda phase as native arms (package KV, lane D; `Game::step_agenda`, `open_next_vote`, `step_vote`, `close_vote`).
//!
//! Two arms and one held window:
//!
//! * `Arm::Agenda` ([`advance_agenda`], KV1): a self-opening job of the turn flow ([`TurnJob::AgendaOpen`], `flow.rs`): the flag, the
//!   agendas off the deck, then [`flow::open_next_vote`]: the reveal chain (Quash, Political Favor, Veto, Political Secret), Committee
//!   Formation, and the vote of the first agenda put on the stack ([`VotingWin`]).
//! * `Arm::Voting` ([`advance_voting`], KV2): the held vote ([`VotingWin`], kind `Voting`): its stage questions are the arm's questions; each
//!   answer is followed by `VOTES_CAST` (the reaction windows: Bribery); a vote with nothing left to ask is CLOSED in the same call
//!   (`close.rs`, KV3: the close, the rider payoffs, the agenda's effect, the feat, the agenda-timed scoring window), and the next agenda
//!   of the phase is revealed and put to a vote when no scoring window was opened.
//! * [`TurnJob::AgendaNext`] (KV3): the agendas an agenda close held back while its scoring window was open (`AgendaQueueAfterEventScoring`):
//!   lane C's `Arm::EventScoring` runs it when the window ends.
//!
//! A question asked from INSIDE a step (the reaction windows of the events, the windows the close composes) goes to the tape
//! (`driver::turn::inline`), so a caller that returns between questions, a clone of the position mid-job and a fresh driver all continue
//! identically.
//!
//! The string closes a window in the step AFTER the last answer; the native call that applies the last answer also runs the close (the
//! decision of lane C), so the questions and the events of the two string steps come out in the string's order inside one `live_step`.

pub mod close;
pub mod flow;

use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::driver::live::{advance, ask, Asking, Env, Live, LiveError, Out};
use crate::driver::turn::advance_job;
use crate::driver::turn::inline::{emit_typed, Flow};
use crate::generated::Agenda;
use crate::kernel::agenda_codes::AgendaGap;
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::rules::agenda::window::{Facts, Vote};
use crate::state::{CoreState, EventKind, Key, Payload, TurnJob, Val, VoteStage, Voting, Window as OpenWindow, WindowKind};

/// `ti4_model::state::RULES_BRIBERY_PER_PLAYER`: from here `VOTES_CAST` fires once per voter, when they have named their outcome.
pub const RULES_BRIBERY_PER_PLAYER: u32 = 7;

/// The vote of one agenda and the agendas still to come (`Game.voting`).
#[derive(Debug, Clone)]
pub struct VotingWin {
    pub vote: Vote,
    /// The agendas still to be revealed after this one, the next first.
    pub queue: SmallList<Agenda, 4>,
}

impl VotingWin {
    #[must_use]
    pub const fn new(vote: Vote, queue: SmallList<Agenda, 4>) -> Self {
        Self { vote, queue }
    }
}

/// Note a gap: the live agenda phase cannot follow the string here (a counted bail of the live path, never a silent skip).
pub fn gap(sx: &Sx<'_>, why: AgendaGap) {
    if sx.tape.unknown.get().is_none() {
        sx.tape.unknown.set(Some(crate::kernel::ability::Unknown::AgendaGap(why)));
    }
}

/// `state.agenda_casts = window.casts()`: the display list of what each voter cast, mirrored after every answer.
pub fn mirror_casts(core: &mut CoreState, vote: &Vote) {
    let mut casts = crate::containers::Log::new();
    for (player, outcome, votes) in vote.casts() {
        casts.push(crate::state::AgendaCastRec { player: *player, outcome: *outcome, votes: *votes });
    }
    core.agenda_mut().casts = casts;
}

impl Window for VotingWin {
    type Stage = VoteStage;

    fn stage(&self) -> VoteStage {
        self.vote.stage_now()
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            self.vote.settle(sx.state, sx.kernel);
            mirror_casts(sx.state, &self.vote);
            if let Some(choice) = self.vote.ask_of(sx.state, sx.kernel) {
                return Next::Continue(choice.ask);
            }
            if !self.vote.is_done() {
                // A stage with no question that is not the end: the payload and the position disagree.
                gap(sx, AgendaGap::ReplayDiverged);
                return Next::Done;
            }
            // Nothing left to ask: the vote is closed.
            match close::close_vote(sx, &self.vote, self.queue.as_slice()) {
                close::Closed::Halt | close::Closed::Ended => return Next::Done,
                close::Closed::Next(next) => *self = next,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.vote.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let voter = self.vote.ask_of(sx.state, sx.kernel).map(|c| c.actor).ok_or(Illegal)?;
        let answered_outcome = self.vote.is_outcome_question();
        self.vote.answer(sx, pick)?;
        // The string's `resolve` settles the window as part of the answer.
        self.vote.settle(sx.state, sx.kernel);
        mirror_casts(sx.state, &self.vote);
        // "After you cast votes on an outcome of an agenda", and "after the speaker votes". Before RULES_BRIBERY_PER_PLAYER this fired after
        // every answer of the vote (the outcome, each planet exhausted, the trade goods), asking a Bribery holder again at each; now it
        // fires once per voter, when they have named their outcome. The string ignores a refusal of the emission.
        if answered_outcome || !sx.state.meta().rules_at_least(RULES_BRIBERY_PER_PLAYER) {
            let payload = Payload::new().with(Key::Player, Val::Seat(voter));
            let _: Flow = emit_typed(sx, EventKind::VotesCast, payload);
        }
        Ok(())
    }
}

impl Held for VotingWin {
    const KIND: WindowKind = WindowKind::Voting;

    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::Voting)? {
            OpenWindow::Voting(v) => Some(Self { vote: Vote::restore(&v.vote, Facts::of_core(core, k)), queue: v.queue.clone() }),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Voting(Box::new(Voting { vote: self.vote.to_payload(), queue: self.queue.clone() })));
    }
}

/// `Arm::Agenda`.
///
/// # Errors
/// As [`advance_job`].
pub fn advance_agenda(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::AgendaOpen, core, env, answer)
}

/// `Arm::Voting`.
///
/// # Errors
/// As [`advance`].
pub fn advance_voting(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    // A close that asked a question from inside its step is a job on the stack (the next vote's reveal): it continues first.
    advance::<VotingWin>(core, env, answer)
}

/// The question of `Arm::Voting`.
///
/// # Errors
/// As [`ask`].
pub fn ask_voting(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<VotingWin>(core, env)
}
