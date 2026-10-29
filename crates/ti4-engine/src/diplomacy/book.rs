//! The deal book, the signals and the promise ledger as one write each on both backends (package B4d):
//! `string` is the original code (`window.rs`, `signals.rs`, `promises.rs`), `core` is
//! `ti4_engine_core::rules::diplomacy_book` through `crate::diplomacy_write`. `None` from the core (an id with no
//! typed form, an unseated owner) keeps the string write.

use ti4_model::{
    DealId, DealRevision, DealStatus, DealTerm, DiplomacyError, DiplomacyEvent,
    DiplomacyJournalEntry, GameState, PlayerId, SignalId, TransferAsset,
};

use super::promises::{DiplomacyEventContext, PromiseError};
use super::relations::{RelationshipEvent, apply_relationship_event};
use super::signals::{SignalDraft, SignalError};
use crate::dual::{Area, write};

/// The promise `fulfill_payment` is asked to pay.
#[derive(Debug, Clone, PartialEq)]
pub struct Due {
    pub proposer_side: bool,
    pub local: usize,
    pub beneficiary: PlayerId,
    pub asset: TransferAsset,
}

/// A core half that does not exist: the build without the typed core keeps the string write. The label counts
/// how often the core answered (tests read it, [`counts`]).
macro_rules! core_half {
    ($label:expr, $call:expr) => {{
        #[cfg(feature = "typed-core")]
        let core = |s: &mut GameState| {
            let done = ($call)(s);
            note($label, done.is_some());
            done
        };
        #[cfg(not(feature = "typed-core"))]
        let core = |_: &mut GameState| None;
        core
    }};
}

#[cfg(test)]
thread_local! {
    static COUNTS: std::cell::RefCell<std::collections::BTreeMap<&'static str, (u64, u64)>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

#[cfg(test)]
fn note(label: &'static str, ran: bool) {
    COUNTS.with(|c| {
        let mut c = c.borrow_mut();
        let e = c.entry(label).or_default();
        if ran { e.0 += 1 } else { e.1 += 1 }
    });
}

#[cfg(not(test))]
const fn note(_: &'static str, _: bool) {}

/// `(core answered, core left it to the string)` of the writes of `label` this thread made.
#[cfg(test)]
#[must_use]
pub(crate) fn counts(label: &str) -> (u64, u64) {
    COUNTS.with(|c| c.borrow().get(label).copied().unwrap_or_default())
}

/// Forget the counts of this thread.
#[cfg(test)]
pub(crate) fn reset_counts() {
    COUNTS.with(|c| c.borrow_mut().clear());
}

/// `DiplomacyState::create_deal`.
pub fn create_deal(
    state: &mut GameState,
    proposer: &PlayerId,
    recipient: &PlayerId,
    revision: &DealRevision,
) -> Result<DealId, DiplomacyError> {
    let round = state.round;
    let string = |s: &mut GameState| {
        s.diplomacy
            .create_deal(proposer.clone(), recipient.clone(), round, revision.clone())
    };
    let core = core_half!("diplomacy::create_deal", |s: &mut GameState| crate::diplomacy_write::create_deal(
        s, proposer, recipient, round, revision
    ));
    write(Area::TradeDiplomacy, state, "diplomacy::create_deal", string, core)
}

fn counter_string(
    s: &mut GameState,
    id: DealId,
    revision: &DealRevision,
) -> Result<(), DiplomacyError> {
    let round = s.round;
    s.diplomacy
        .active_deals
        .get_mut(&id)
        .ok_or(DiplomacyError::UnknownDeal)?
        .add_counter(revision.clone())?;
    s.diplomacy.journal.push(DiplomacyJournalEntry {
        round,
        event: DiplomacyEvent::Countered {
            deal_id: id,
            revision: revision.clone(),
        },
    });
    Ok(())
}

/// `Deal::add_counter` and the `Countered` row.
pub fn counter(
    state: &mut GameState,
    id: DealId,
    revision: &DealRevision,
) -> Result<(), DiplomacyError> {
    let string = |s: &mut GameState| counter_string(s, id, revision);
    let core = core_half!("diplomacy::counter", |s: &mut GameState| crate::diplomacy_write::counter(s, id, revision));
    write(Area::TradeDiplomacy, state, "diplomacy::counter", string, core)
}

fn decline_string(s: &mut GameState, id: DealId, actor: &PlayerId) -> Result<(), DiplomacyError> {
    let round = s.round;
    let deal = s
        .diplomacy
        .active_deals
        .get_mut(&id)
        .ok_or(DiplomacyError::UnknownDeal)?;
    deal.status = DealStatus::Declined;
    let revision = deal.latest().number;
    s.diplomacy.journal.push(DiplomacyJournalEntry {
        round,
        event: DiplomacyEvent::Declined {
            deal_id: id,
            actor: actor.clone(),
            revision,
        },
    });
    s.diplomacy.archive_deal(id, round)
}

/// The decline of the window: status, `Declined` row, archive.
pub fn decline(state: &mut GameState, id: DealId, actor: &PlayerId) -> Result<(), DiplomacyError> {
    let string = |s: &mut GameState| decline_string(s, id, actor);
    let core = core_half!("diplomacy::decline", |s: &mut GameState| crate::diplomacy_write::decline(s, id, actor));
    write(Area::TradeDiplomacy, state, "diplomacy::decline", string, core)
}

fn accept_string(
    s: &mut GameState,
    id: DealId,
    actor: &PlayerId,
    fair: bool,
) -> Result<(), DiplomacyError> {
    let round = s.round;
    let deal = s
        .diplomacy
        .active_deals
        .get(&id)
        .cloned()
        .ok_or(DiplomacyError::UnknownDeal)?;
    let revision = deal.latest().clone();
    let active = s.diplomacy.active_deals.get_mut(&id).expect("checked deal");
    active.status = DealStatus::Accepted;
    s.diplomacy.journal.push(DiplomacyJournalEntry {
        round,
        event: DiplomacyEvent::Accepted {
            deal_id: id,
            actor: actor.clone(),
            revision: revision.number,
        },
    });
    for (index, term) in revision
        .proposer_terms
        .iter()
        .chain(&revision.recipient_terms)
        .enumerate()
    {
        if term.is_immediate() {
            s.diplomacy.journal.push(DiplomacyJournalEntry {
                round,
                event: DiplomacyEvent::ImmediateApplied {
                    deal_id: id,
                    term_index: u8::try_from(index).unwrap_or(u8::MAX),
                },
            });
        }
    }
    let future = revision
        .proposer_terms
        .iter()
        .chain(&revision.recipient_terms)
        .any(|term| !term.is_immediate());
    let active = s.diplomacy.active_deals.get_mut(&id).expect("checked deal");
    active.status = if future {
        DealStatus::Active
    } else {
        DealStatus::Fulfilled
    };
    // Immediate transfers only is a transaction and is judged as one: a fair trade builds trust and
    // an unfair one builds nothing. A bundle with any promise in it is a deal.
    let relationship = if future {
        Some(RelationshipEvent::DealAccepted {
            a: deal.proposer.clone(),
            b: deal.recipient.clone(),
        })
    } else if fair {
        Some(RelationshipEvent::FairTransaction {
            a: deal.proposer.clone(),
            b: deal.recipient.clone(),
        })
    } else {
        None
    };
    if let Some(event) = relationship {
        apply_relationship_event(s, &event)?;
    }
    for term in &revision.recipient_terms {
        if let DealTerm::Attack { player: target, .. } = term
            && target != &deal.proposer
            && target != &deal.recipient
        {
            apply_relationship_event(
                s,
                &RelationshipEvent::AntiThirdPartyDeal {
                    payer: deal.proposer.clone(),
                    attacker: deal.recipient.clone(),
                    target: target.clone(),
                },
            )?;
        }
    }
    if !future {
        s.diplomacy.journal.push(DiplomacyJournalEntry {
            round,
            event: DiplomacyEvent::DealSettled {
                deal_id: id,
                status: DealStatus::Fulfilled,
            },
        });
        s.diplomacy.archive_deal(id, round)?;
    }
    Ok(())
}

/// What an accepted deal does to the book once its immediate transfers have moved. `fair` is the pricing of
/// the immediate legs, taken before anything moved.
pub fn accept(
    state: &mut GameState,
    id: DealId,
    actor: &PlayerId,
    fair: bool,
) -> Result<(), DiplomacyError> {
    let string = |s: &mut GameState| accept_string(s, id, actor, fair);
    let core = core_half!("diplomacy::accept", |s: &mut GameState| crate::diplomacy_write::accept(s, id, actor, fair));
    write(Area::TradeDiplomacy, state, "diplomacy::accept", string, core)
}

/// `signals::emit_signal` (`consume_allowance`) and `emit_signal_in_open_contact`.
pub fn emit_signal(
    state: &mut GameState,
    draft: &SignalDraft,
    consume_allowance: bool,
) -> Result<SignalId, SignalError> {
    let string =
        |s: &mut GameState| super::signals::emit_signal_string(s, draft.clone(), consume_allowance);
    let core = core_half!("diplomacy::emit_signal", |s: &mut GameState| crate::diplomacy_write::emit_signal(
        s,
        &draft.speaker,
        &draft.target,
        &draft.statement,
        draft.expires_round,
        consume_allowance
    ));
    write(Area::TradeDiplomacy, state, "diplomacy::emit_signal", string, core)
}

/// `signals::judge_event`.
pub fn judge_event(
    state: &mut GameState,
    event: &DiplomacyEventContext,
) -> Result<(), DiplomacyError> {
    let string = |s: &mut GameState| super::signals::judge_event_string(s, event);
    let core = core_half!("diplomacy::judge_event", |s: &mut GameState| crate::diplomacy_write::judge_event(s, event));
    write(Area::TradeDiplomacy, state, "diplomacy::judge_event", string, core)
}

/// `signals::settle_deadlines`.
pub fn settle_signals(state: &mut GameState, completed_round: u32) -> Result<(), DiplomacyError> {
    let string = |s: &mut GameState| super::signals::settle_signals_string(s, completed_round);
    let core =
        core_half!("diplomacy::settle_signals", |s: &mut GameState| crate::diplomacy_write::settle_signals(s, completed_round));
    write(Area::TradeDiplomacy, state, "diplomacy::settle_signals", string, core)
}

/// `promises::evaluate_event`.
pub fn evaluate_event(
    state: &mut GameState,
    event: &DiplomacyEventContext,
) -> Result<(), PromiseError> {
    let string = |s: &mut GameState| super::promises::evaluate_event_string(s, event);
    let core = core_half!("diplomacy::evaluate_event", |s: &mut GameState| crate::diplomacy_write::evaluate_event(s, event));
    write(Area::TradeDiplomacy, state, "diplomacy::evaluate_event", string, core)
}

/// `promises::settle_deadlines`.
pub fn settle_deadlines(state: &mut GameState, completed_round: u32) -> Result<(), PromiseError> {
    let string = |s: &mut GameState| super::promises::settle_deadlines_string(s, completed_round);
    let core =
        core_half!("diplomacy::settle_deadlines", |s: &mut GameState| crate::diplomacy_write::settle_deadlines(s, completed_round));
    write(Area::TradeDiplomacy, state, "diplomacy::settle_deadlines", string, core)
}

/// `promises::settle_game_end`.
pub fn settle_game_end(state: &mut GameState) -> Result<(), PromiseError> {
    let string = |s: &mut GameState| super::promises::settle_game_end_string(s);
    let core = core_half!("diplomacy::settle_game_end", |s: &mut GameState| crate::diplomacy_write::settle_game_end(s));
    write(Area::TradeDiplomacy, state, "diplomacy::settle_game_end", string, core)
}

/// `fulfill_payment` before the goods move. A read: the state is unchanged on both sides.
pub fn payment_due(
    state: &mut GameState,
    id: DealId,
    promiser: &PlayerId,
    term_index: usize,
) -> Result<Due, PromiseError> {
    let string = |s: &mut GameState| super::promises::payment_due_string(s, id, promiser, term_index);
    let core =
        core_half!("diplomacy::payment_due", |s: &mut GameState| crate::diplomacy_write::payment_due(s, id, promiser, term_index));
    write(Area::TradeDiplomacy, state, "diplomacy::payment_due", string, core)
}

/// `fulfill_payment` after the goods moved.
pub fn payment_made(
    state: &mut GameState,
    id: DealId,
    promiser: &PlayerId,
    term_index: usize,
) -> Result<(), PromiseError> {
    let string = |s: &mut GameState| super::promises::payment_made_string(s, id, promiser, term_index);
    let core =
        core_half!("diplomacy::payment_made", |s: &mut GameState| crate::diplomacy_write::payment_made(s, id, promiser, term_index));
    write(Area::TradeDiplomacy, state, "diplomacy::payment_made", string, core)
}
