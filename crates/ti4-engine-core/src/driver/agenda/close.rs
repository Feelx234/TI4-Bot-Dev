//! The close of a vote (KV3): `Game::close_vote` as one composition inside the step that applies the last answer.
//!
//! In the string's order: the face-down agenda of Covert Legislation is taken, the choices cleared, the close of the vote
//! (`rules::agenda::close::Close`: `AGENDA_RESOLVED` and the cards played into it, the log, the redirect, the correct predictions, the
//! law), the agenda's own effect (`rules::agenda::effects::Resolve`), the feat of an election and the agenda-timed scoring window with the
//! agendas still to come held behind it (`AgendaQueueAfterEventScoring`); a vote that resolved nothing goes straight on to the next agenda.

use super::flow::{open_next_vote, Opened};
use super::grant::Grant;
use super::VotingWin;
use crate::containers::SmallList;
use super::gap;
use crate::driver::turn::inline::{run, Flow};
use crate::driver::windows::scoring::open_event;
use crate::generated::Agenda;
use crate::kernel::agenda_codes::AgendaGap;
use crate::kernel::stage::Sx;
use crate::keys::Outcome;
use crate::query::reach;
use crate::rules::agenda::close::{Close, Facts};
use crate::rules::agenda::effects::{Done, Resolve};
use crate::rules::agenda::table_effects::EffectFacts;
use crate::rules::agenda::window::Vote;
use crate::state::{CoreState, Feat, PlainArg, PlainKind, PlainRec, ScoringTiming, Slice, Window as OpenWindow, WindowKind};
use crate::tables::Scope;

/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`: from this version the unofficial action cards leave the deck before a draw.
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;

/// How the close ended.
#[derive(Debug)]
pub enum Closed {
    /// The step is doomed (a question found the tape dry, or a gap was noted).
    Halt,
    /// The close is done and nothing is left on this window: a scoring window (and the agendas behind it) were opened, or the phase is over.
    Ended,
    /// The next agenda goes to a vote: the window replaces this one.
    Next(VotingWin),
}

fn close_facts(sx: &Sx<'_>) -> Facts {
    let codes = sx.table.codes.agenda;
    let scope = reach::scope_of(sx.kernel);
    Facts {
        trade_rider: codes.trade_rider,
        keleres_rider: codes.keleres_rider,
        drop_unofficial: sx.state.meta().rules_at_least(RULES_OFFICIAL_ACTION_DECK),
        blanked: reach::blanked(sx.state),
        hooked: 0,
        research: sx.table.facts.plays.as_ref().and_then(|p| p.roster).map(|roster| (scope, roster)),
        fleet_pool: sx.table.codes.agenda.fleet_pool,
    }
}

fn effect_facts(sx: &Sx<'_>, ballot: &[(crate::ids::Seat, Outcome)]) -> EffectFacts {
    let codes = sx.table.codes;
    let voters = |wanted: Outcome| -> SmallList<crate::ids::Seat, 8> { ballot.iter().filter(|(_, o)| *o == wanted).map(|(s, _)| *s).collect() };
    let mut mahact_held = [0; 8]; // mutants:skip: every seat's entry is written below
    for (at, held) in mahact_held.iter_mut().enumerate() {
        if let Ok(seat) = u8::try_from(at) {
            *held = crate::kernel::fx::tokens_held_by_mahact(sx.state, crate::ids::Seat(seat));
        }
    }
    EffectFacts {
        for_voters: voters(Outcome::For),
        against_voters: voters(Outcome::Against),
        mahact_held,
        scope: reach::scope_of(sx.kernel),
        // `ti4_model::content_types::DEFAULT` is every source.
        default_scope: Scope(u32::MAX),
        source: codes.agenda.agenda,
        pool: codes.reinforcements,
        has_galaxy: sx.state.loaded.has(Slice::Map),
        defense_act: codes.agenda.defense_act,
        hooked: 0,
        drop_unofficial: sx.state.meta().rules_at_least(RULES_OFFICIAL_ACTION_DECK),
        blanked: reach::blanked(sx.state),
        roster: sx.table.facts.plays.as_ref().and_then(|p| p.roster),
        redistribution: codes.agenda.redistribution,
    }
}

/// `Game::run_agenda_effect`: the effect of `agenda` resolved with `effective`, then its plain announcement.
fn run_effect(sx: &mut Sx<'_>, agenda: Agenda, effective: Outcome, ballot: &[(crate::ids::Seat, Outcome)]) -> Flow {
    // Research Grant Reallocation composes the technology question of the relics, which an area window may not import.
    if let (Agenda::ResearchGrantReallocation, Outcome::Seat(who)) = (agenda, effective) {
        let mut grant = Grant::new(who, reach::scope_of(sx.kernel), sx.table.facts.plays.as_ref().and_then(|p| p.roster), sx.table.codes.agenda);
        if run(sx, &mut grant) == Flow::Halt {
            return Flow::Halt;
        }
        if grant.unsupported {
            gap(sx, AgendaGap::EffectUnsupported(agenda as u8));
            return Flow::Halt;
        }
        sx.plain(PlainRec::one(PlainKind::AgendaEffectResolved, PlainArg::Agenda(agenda)));
        return Flow::Done;
    }
    let facts = effect_facts(sx, ballot);
    let Some(mut window) = Resolve::of_facts(agenda, effective, &facts) else {
        gap(sx, AgendaGap::NoEffect(agenda as u8));
        return Flow::Halt;
    };
    if run(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.unsupported() {
        gap(sx, AgendaGap::EffectUnsupported(agenda as u8));
        return Flow::Halt;
    }
    let kind = if window.done() == Done::Resolved { PlainKind::AgendaEffectResolved } else { PlainKind::AgendaEffectDeferred };
    sx.plain(PlainRec::one(kind, PlainArg::Agenda(agenda)));
    Flow::Done
}

/// A fresh feat occurrence (`GameState::begin_feat_occurrence`).
fn begin_feat_occurrence(core: &mut CoreState) -> u64 {
    let meta = core.meta_mut();
    meta.feat_occurrence_seq += 1;
    meta.feat_occurrence_seq
}

/// `GameState::record_event_feat`.
fn record_event_feat(core: &mut CoreState, seat: crate::ids::Seat, feat: Feat, occurrence: u64) {
    if let Some(p) = core.players_mut().by_seat_mut(seat) {
        let entry = (feat, occurrence);
        if !p.misc.event_feats.as_slice().contains(&entry) {
            p.misc.event_feats.push(entry);
            p.misc.event_feats.as_mut_slice().sort_unstable();
        }
    }
}

/// `Game::close_vote` for the finished `vote`; `queue` are the agendas still to come.
pub fn close_vote(sx: &mut Sx<'_>, vote: &Vote, queue: &[Agenda]) -> Closed {
    let alias = vote.agenda();
    // Covert Legislation: the vote decides the face-down agenda, which is what resolves.
    let covert = if alias == Agenda::CovertLegislation { sx.state.agenda_mut().covert.take() } else { None };
    sx.state.agenda_mut().choices.clear();
    sx.state.agenda_mut().in_play = None;
    let ballot = vote.ballot();
    let winner = vote.winner();
    let mut close = Close::new(close_facts(sx), alias, covert, winner, ballot.as_slice(), vote.counts().as_slice(), vote.choices());
    if run(sx, &mut close) == Flow::Halt {
        return Closed::Halt;
    }
    if close.failed() {
        gap(sx, AgendaGap::CloseFailed);
        return Closed::Halt;
    }
    if close.unsupported() {
        gap(sx, AgendaGap::CloseUnsupported);
        return Closed::Halt;
    }
    if winner.is_some() {
        let resolved = covert.unwrap_or(alias);
        if let Some(effective) = close.effective()
            && run_effect(sx, resolved, effective, ballot.as_slice()) == Flow::Halt
        {
            return Closed::Halt;
        }
        // Every resolved agenda is its own agenda-phase occurrence: objectives such as Dictate Policy depend on the completed outcome.
        let occurrence = begin_feat_occurrence(sx.state);
        if let Some(elected) = close.elected() {
            record_event_feat(sx.state, elected, Feat::ElectedByAnAgenda, occurrence);
        }
        open_event(sx.state, ScoringTiming::Agenda, occurrence, false);
        // The window is served by the scoring arm; the agendas to come wait behind it.
        sx.state.pending_mut().open(OpenWindow::AgendaQueueAfterEventScoring(queue.iter().copied().collect()));
        debug_assert!(sx.state.pending().window(WindowKind::EventScoring).is_some());
        return Closed::Ended;
    }
    match open_next_vote(sx, queue.to_vec()) {
        Opened::Halt => Closed::Halt,
        Opened::Finished => Closed::Ended,
        Opened::Vote(next) => Closed::Next(next),
    }
}
