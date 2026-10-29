//! `Area::TradeDiplomacy` (package B4d): the deal book, the signals and the promise ledger as
//! `ti4_engine_core::rules::diplomacy_book` over the Trade slice (Meta, Seats and Agenda are read: the round, who
//! is seated, the ballot).
//!
//! Every function answers `None` when the position is outside what the core holds (an id with no typed form, a
//! seat the table does not know, more than 16 owners): the caller keeps the string write, which writes nothing
//! before it answers. The string side is `diplomacy/book.rs`, which wraps each call in `dual::write`.

use ti4_engine_core::generated::{Agenda, Leader, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::rules::diplomacy_book as book;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::diplomacy as md;
use ti4_model::state::GameState;
use ti4_model::{DiplomacyError, PlayerId};

use crate::core_adapter::typed;
use crate::core_sync::codec::trade as codec;
use crate::core_sync::{Ctx, Footprint, Sources, with_core};
use crate::diplomacy::promises::{DiplomacyEventContext, PromiseError};
use crate::diplomacy::signals::SignalError;

/// What the book reads (the round, the seats, the ballot) and writes (the diplomacy state).
const BOOK: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Agenda, Slice::Trade], &[Slice::Trade]);

fn run<T>(gs: &mut GameState, cx: &mut Ctx, f: impl FnOnce(&mut CoreState) -> T) -> Option<T> {
    with_core(gs, cx, Sources::default(), BOOK, |core, _| f(core)).ok()
}

fn seat(cx: &Ctx, p: &PlayerId) -> Option<Seat> {
    cx.owners.seat(p.as_str())
}

/// `DiplomacyError` of a book error.
const fn error(e: book::BookError) -> DiplomacyError {
    use book::BookError as B;
    match e {
        B::SelfPair => DiplomacyError::SelfPair,
        B::PlayerMissing => DiplomacyError::PlayerMissing,
        B::TooManyTerms => DiplomacyError::TooManyTerms,
        B::EmptyDeal => DiplomacyError::EmptyDeal,
        B::InvalidDeadline => DiplomacyError::InvalidDeadline,
        B::InvalidAsset => DiplomacyError::InvalidAsset,
        B::TooManyCounters => DiplomacyError::TooManyCounters,
        B::InvalidRevision => DiplomacyError::InvalidRevision,
        B::TerminalDeal => DiplomacyError::TerminalDeal,
        B::UnknownDeal => DiplomacyError::UnknownDeal,
        B::IdExhausted => DiplomacyError::IdExhausted,
    }
}

fn signal_error(e: book::SignalError) -> SignalError {
    use book::SignalError as S;
    match e {
        S::Disabled => SignalError::Disabled,
        S::SelfSignal => SignalError::SelfSignal,
        S::PlayerMissing => SignalError::PlayerMissing,
        S::InvalidExpiry => SignalError::InvalidExpiry,
        S::AlreadyInitiated => SignalError::AlreadyInitiated,
        S::IdExhausted => SignalError::IdExhausted,
        S::SelfPair => SignalError::Diplomacy(DiplomacyError::SelfPair),
    }
}

fn promise_error(e: book::PromiseError, deal: md::DealId) -> PromiseError {
    use book::PromiseError as P;
    match e {
        P::UnknownDeal => PromiseError::UnknownDeal(deal),
        P::UnknownTerm => PromiseError::UnknownTerm,
        P::AlreadySettled => PromiseError::AlreadySettled,
        P::SelfPair => PromiseError::Diplomacy(DiplomacyError::SelfPair),
        P::Book(b) => PromiseError::Diplomacy(error(b)),
    }
}

/// `DiplomacyState::create_deal`.
pub fn create_deal(gs: &mut GameState, proposer: &PlayerId, recipient: &PlayerId, round: u32, revision: &md::DealRevision) -> Option<Result<md::DealId, DiplomacyError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let (p, r) = (seat(&cx, proposer)?, seat(&cx, recipient)?);
    let revision = codec::revision(&mut cx, revision).ok()?;
    let made = run(gs, &mut cx, |core| book::create_deal(core, p, r, round, revision))?;
    Some(made.map(|id| md::DealId(id.0)).map_err(error))
}

/// `Deal::add_counter` and the `Countered` row.
pub fn counter(gs: &mut GameState, id: md::DealId, revision: &md::DealRevision) -> Option<Result<(), DiplomacyError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let revision = codec::revision(&mut cx, revision).ok()?;
    let done = run(gs, &mut cx, |core| book::counter(core, ti4_engine_core::state::DealId(id.0), revision))?;
    Some(done.map_err(error))
}

/// The decline of the window.
pub fn decline(gs: &mut GameState, id: md::DealId, actor: &PlayerId) -> Option<Result<(), DiplomacyError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let a = seat(&cx, actor)?;
    let done = run(gs, &mut cx, |core| book::decline(core, ti4_engine_core::state::DealId(id.0), a))?;
    Some(done.map_err(error))
}

/// The book half of `apply_acceptance`.
pub fn accept(gs: &mut GameState, id: md::DealId, actor: &PlayerId, fair: bool) -> Option<Result<(), DiplomacyError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let a = seat(&cx, actor)?;
    let done = run(gs, &mut cx, |core| book::accept(core, ti4_engine_core::state::DealId(id.0), a, fair))?;
    Some(done.map_err(error))
}

/// `emit_signal` (`consume`) / `emit_signal_in_open_contact`.
pub fn emit_signal(gs: &mut GameState, speaker: &PlayerId, target: &PlayerId, statement: &md::SignalStatement, expires_round: u32, consume: bool) -> Option<Result<md::SignalId, SignalError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let (s, t) = (seat(&cx, speaker)?, seat(&cx, target)?);
    let statement = codec::statement(&mut cx, statement).ok()?;
    let made = run(gs, &mut cx, |core| book::emit_signal(core, s, t, statement, expires_round, consume))?;
    Some(made.map(|id| md::SignalId(id.0)).map_err(signal_error))
}

/// The moment of a gameplay event over typed ids.
fn moment(cx: &Ctx, event: &DiplomacyEventContext) -> Option<book::Moment> {
    Some(match event {
        DiplomacyEventContext::SystemActivated { player, system } => book::Moment::SystemActivated { player: seat(cx, player)?, system: typed::<System>(system.as_str())? },
        DiplomacyEventContext::HostileEngagement { attacker, victim, activation_seq } => {
            book::Moment::HostileEngagement { attacker: seat(cx, attacker)?, victim: seat(cx, victim)?, activation_seq: *activation_seq }
        }
        DiplomacyEventContext::VotesRecorded { agenda } => book::Moment::VotesRecorded { agenda: typed::<Agenda>(agenda)? },
        DiplomacyEventContext::CommoditiesReplenished { by, beneficiary } => book::Moment::CommoditiesReplenished { by: seat(cx, by)?, beneficiary: seat(cx, beneficiary)? },
        DiplomacyEventContext::LeaderUsedFor { user, leader, beneficiary } => {
            book::Moment::LeaderUsedFor { user: seat(cx, user)?, leader: typed::<Leader>(leader)?, beneficiary: seat(cx, beneficiary)? }
        }
    })
}

/// `signals::judge_event`.
pub fn judge_event(gs: &mut GameState, event: &DiplomacyEventContext) -> Option<Result<(), DiplomacyError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let m = moment(&cx, event)?;
    let done = run(gs, &mut cx, |core| book::judge_event(core, m))?;
    Some(done.map_err(error))
}

/// `signals::settle_deadlines`.
pub fn settle_signals(gs: &mut GameState, completed_round: u32) -> Option<Result<(), DiplomacyError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let done = run(gs, &mut cx, |core| book::settle_signals(core, completed_round))?;
    Some(done.map_err(error))
}

/// `promises::evaluate_event`.
pub fn evaluate_event(gs: &mut GameState, event: &DiplomacyEventContext) -> Option<Result<(), PromiseError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let m = moment(&cx, event)?;
    let done = run(gs, &mut cx, |core| book::evaluate_event(core, m))?;
    Some(done.map_err(|e| promise_error(e, md::DealId(0))))
}

/// `promises::settle_deadlines`.
pub fn settle_deadlines(gs: &mut GameState, completed_round: u32) -> Option<Result<(), PromiseError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let done = run(gs, &mut cx, |core| book::settle_deadlines(core, completed_round))?;
    Some(done.map_err(|e| promise_error(e, md::DealId(0))))
}

/// `promises::settle_game_end`.
pub fn settle_game_end(gs: &mut GameState) -> Option<Result<(), PromiseError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let done = run(gs, &mut cx, book::settle_game_end)?;
    Some(done.map_err(|e| promise_error(e, md::DealId(0))))
}

pub use crate::diplomacy::book::Due;

/// `fulfill_payment` before the goods move (`promises::payment_due`).
pub fn payment_due(gs: &mut GameState, id: md::DealId, promiser: &PlayerId, term_index: usize) -> Option<Result<Due, PromiseError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let who = seat(&cx, promiser)?;
    let found = run(gs, &mut cx, |core| book::payment_due(core, ti4_engine_core::state::DealId(id.0), who, term_index))?;
    match found {
        Ok(due) => {
            // The asset is read back through the term codec.
            let term = codec::term_back(&cx, &ti4_engine_core::state::DealTerm::FuturePayment { asset: due.asset, deadline_round: 0 }).ok()?;
            let md::DealTerm::FuturePayment { asset, .. } = term else { return None };
            let beneficiary = PlayerId::new(cx.owners.id(due.beneficiary)?);
            Some(Ok(Due { proposer_side: due.proposer_side, local: due.local, beneficiary, asset }))
        }
        Err(e) => Some(Err(promise_error(e, id))),
    }
}

/// `fulfill_payment` after the goods moved (`promises::payment_made`).
pub fn payment_made(gs: &mut GameState, id: md::DealId, promiser: &PlayerId, term_index: usize) -> Option<Result<(), PromiseError>> {
    let mut cx = Ctx::of(gs).ok()?;
    let who = seat(&cx, promiser)?;
    let done = run(gs, &mut cx, |core| book::payment_made(core, ti4_engine_core::state::DealId(id.0), who, term_index))?;
    Some(done.map_err(|e| promise_error(e, id)))
}
