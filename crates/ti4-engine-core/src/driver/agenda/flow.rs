//! The phase composer (KV1): `Game::step_agenda` and `open_next_vote`, run as jobs of the turn flow.
//!
//! [`run_open`] is `step_agenda`: the flag, the top agendas of the deck, then [`open_next_vote`]. That is the loop of the phase: reveal an
//! agenda (the chain `rules::agenda::reveal::Reveal` follows: Quash, Political Favor, Veto, Political Secret), offer Committee Formation
//! when the agenda elects a player, put what is left to a vote ([`VotingWin`] on the stack). When no agenda is left the phase ends: the
//! planets ready (8.4) and `AGENDA_PHASE_RESOLVED`.

use super::VotingWin;
use crate::containers::SmallList;
use super::gap;
use crate::driver::turn::inline::{run, Flow};
use crate::generated::{Agenda, AgendaElects};
use crate::ids::Seat;
use crate::kernel::agenda_codes::AgendaGap;
use crate::kernel::fx;
use crate::kernel::stage::{Held, Sx};
use crate::query::{board, reach};
use crate::rules::agenda::committee::Committee;
use crate::rules::agenda::offer_discard::OfferDiscard;
use crate::rules::agenda::reveal::{Facts as RevealFacts, Reveal};
use crate::rules::agenda::window::{voting_order_of, Facts as VoteFacts, Vote};
use crate::state::{Code, CoreState, DriverFlags, MetaFlags, Outcome, Phase, PlainKind, PlainRec, Window as OpenWindow, CODE_CAPACITY};

/// `agenda::AGENDAS_PER_PHASE` (LRR 8.2, 8.3): two agendas are revealed in each agenda phase.
pub const AGENDAS_PER_PHASE: usize = 2;

/// Where the loop of the phase stopped.
#[derive(Debug)]
#[allow(clippy::large_enum_variant, reason = "the window of an agenda is held by value on the step's stack; boxing it would only move the bytes")]
pub enum Opened {
    /// The step is doomed (a question found the tape dry, or a gap was noted).
    Halt,
    /// An agenda goes to a vote: the window to put on the stack.
    Vote(VotingWin),
    /// Nothing is left to vote on: the phase is over.
    Finished,
}

/// A text of the state's own table (`AgendaRecord::discard_reason`).
fn state_code(core: &mut CoreState, text: &[u8]) -> Code {
    // The state's own text table mints the code (a code means the same text in every lineage of the state); the table holds 65 535 texts.
    Code(u16::try_from(core.texts.codes.intern(text, CODE_CAPACITY).unwrap_or(0)).unwrap_or(0)) // mutants:skip: the fallbacks are unreachable (the table is never full here and an index fits a code)
}

/// What the reveal reads besides the position.
fn reveal_facts(sx: &mut Sx<'_>) -> RevealFacts {
    let codes = sx.table.codes.agenda;
    RevealFacts {
        scope: reach::scope_of(sx.kernel),
        blanked: reach::blanked(sx.state),
        no_choice: state_code(sx.state, b"no_choice"),
        replaced: state_code(sx.state, b"replaced"),
        quash: codes.quash,
        favor: codes.favor,
        secret: codes.secret,
    }
}

/// 8.4: after both agendas are resolved, every player readies their planets. Checks and Balances (Against): "Each player readies only 3
/// of their planets at the end of this agenda phase."
fn ready_after_agenda_phase(core: &mut CoreState) {
    let limit = crate::rules::agenda::ready_limit(core, reach::blanked(core));
    match limit {
        Some(limit) => {
            let seats: Vec<Seat> = core.players().iter().map(|p| p.seat).collect();
            for seat in seats {
                let theirs: Vec<_> = board::controlled_planets(core, seat).as_slice().iter().map(|(_, planet)| *planet).filter(|planet| board::exhausted(core, *planet)).take(limit).collect();
                for planet in theirs {
                    fx::ready_planet(core, planet);
                }
            }
        }
        None => {
            let exhausted: Vec<_> = core.board().planets().filter(|(_, cell)| cell.flags.has(crate::state::PlanetFlags::EXHAUSTED)).map(|(p, _)| p).collect();
            for planet in exhausted {
                fx::ready_planet(core, planet);
            }
        }
    }
}

/// The phase is over: the planets ready and `AGENDA_PHASE_RESOLVED`.
fn finish_phase(sx: &mut Sx<'_>) {
    ready_after_agenda_phase(sx.state);
    sx.plain(PlainRec::bare(PlainKind::AgendaPhaseResolved));
}

/// Committee Formation, offered before a vote that would elect a player: whether the owner discarded it and elected someone (the vote
/// is then skipped).
fn committee_formation(sx: &mut Sx<'_>, agenda: Agenda) -> Result<bool, ()> {
    let blanked = reach::blanked(sx.state);
    let Some(Outcome::Seat(owner)) = crate::query::laws::elected(sx.state, blanked, Agenda::CommitteeFormation) else { return Ok(false) };
    if sx.state.players().by_seat(owner).is_none() {
        return Ok(false); // mutants:skip: a law is elected onto a seat of the table, a position the rules never leave
    }
    let codes = sx.table.codes.agenda;
    let mut offer = OfferDiscard::new(Agenda::CommitteeFormation, owner, codes.committee_prompt, codes.committee);
    if run(sx, &mut offer) == Flow::Halt {
        return Err(());
    }
    if !offer.taken() {
        return Ok(false);
    }
    let mut election = Committee::new(agenda, owner, codes.committee);
    if run(sx, &mut election) == Flow::Halt {
        return Err(());
    }
    Ok(election.chosen().is_some())
}

/// Put a revealed agenda to its vote (`Game::start_vote`): the window opened and settled, its casts mirrored.
fn start_vote(sx: &mut Sx<'_>, agenda: Agenda, choices: &[Outcome], queue: &[Agenda]) -> VotingWin {
    let order = voting_order_of(sx.state, sx.kernel);
    let mut vote = Vote::new(agenda, choices, order.as_slice(), VoteFacts::of_core(sx.state, sx.kernel));
    vote.settle(sx.state, sx.kernel);
    super::mirror_casts(sx.state, &vote);
    VotingWin::new(vote, SmallList::from_slice(queue))
}

/// `Game::open_next_vote`: reveal the next agenda of `queue` and put it to a vote, or finish the phase when none remain. An agenda whose
/// election has no legal candidate is discarded rather than voted on; Veto and the replacements are the reveal chain's.
pub fn open_next_vote(sx: &mut Sx<'_>, mut queue: Vec<Agenda>) -> Opened {
    if sx.state.cards().reserve.is_some() {
        gap(sx, AgendaGap::Refused);
        return Opened::Halt;
    }
    while !queue.is_empty() {
        let alias = queue.remove(0);
        let facts = reveal_facts(sx);
        let mut reveal = Reveal::new(facts, alias);
        if run(sx, &mut reveal) == Flow::Halt {
            return Opened::Halt;
        }
        if reveal.failed() {
            gap(sx, AgendaGap::RevealFailed);
            return Opened::Halt;
        }
        // This agenda, and every Veto replacement it drew, elected nothing: the queue moves on.
        let Some((agenda, choices)) = reveal.result() else { continue };
        let choices: Vec<Outcome> = choices.to_vec();
        // Committee Formation: "Before players vote on an agenda that requires a player to be elected, the owner of this card may discard
        // this card to choose a player to be elected. Players do not vote on that agenda."
        if agenda.data().elects == AgendaElects::Player {
            match committee_formation(sx, agenda) {
                Err(()) => return Opened::Halt,
                Ok(true) => continue,
                Ok(false) => {}
            }
        }
        return Opened::Vote(start_vote(sx, agenda, &choices, &queue));
    }
    finish_phase(sx);
    Opened::Finished
}

/// Open the window an [`Opened::Vote`] names on the stack.
pub fn put_to_vote(core: &mut CoreState, window: &VotingWin) {
    window.to_pending(core);
}

/// `Game::step_agenda`: the flag, the top agendas, the loop.
pub fn run_open(sx: &mut Sx<'_>) -> Flow {
    sx.state.pending_mut().driver.set(DriverFlags::AGENDA_RESOLVED, true);
    // `resolve_agenda_phase`: the phase exists once the custodians token has been lifted.
    if sx.state.meta().phase != Phase::Agenda || !sx.state.meta().flags.has(MetaFlags::CUSTODIANS_REMOVED) {
        gap(sx, AgendaGap::PhaseNotReady);
        return Flow::Halt;
    }
    if sx.state.cards().reserve.is_some() {
        gap(sx, AgendaGap::Refused);
        return Flow::Halt;
    }
    let mut queue = Vec::new();
    for _ in 0..AGENDAS_PER_PHASE {
        let Some(top) = sx.state.decks_mut().agenda.draw() else { break };
        queue.push(top);
    }
    if queue.is_empty() {
        finish_phase(sx);
        return Flow::Done;
    }
    let opened = open_next_vote(sx, queue);
    settle_opened(sx, opened)
}

/// The agendas an agenda close held back while its scoring window was open go to a vote next (`step_event_scoring`'s close).
pub fn run_next(sx: &mut Sx<'_>) -> Flow {
    let Some(OpenWindow::AgendaQueueAfterEventScoring(queue)) = sx.state.pending_mut().close(crate::state::WindowKind::AgendaQueueAfterEventScoring) else {
        // No queue is held: the window the job belongs to is not open (a defect of the caller).
        gap(sx, AgendaGap::ReplayDiverged);
        return Flow::Halt;
    };
    let opened = open_next_vote(sx, queue.as_slice().to_vec());
    settle_opened(sx, opened)
}

/// What a step does with where the loop stopped: a vote goes on the stack, the end of the phase writes nothing more.
pub fn settle_opened(sx: &mut Sx<'_>, opened: Opened) -> Flow {
    match opened {
        Opened::Halt => Flow::Halt,
        Opened::Finished => Flow::Done,
        Opened::Vote(window) => {
            put_to_vote(sx.state, &window);
            Flow::Done
        }
    }
}
