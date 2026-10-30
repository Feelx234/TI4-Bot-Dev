use ti4_engine_core::containers::{Cow, SeatSet, SmallList, SortedMap};
use ti4_engine_core::generated::{ActionCard, Agenda, Leader, SecretObjective, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::state as c;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::diplomacy as md;
use ti4_model::id::{ActionCardId, PlayerId, SystemId};
use ti4_model::state::GameState;

use super::agenda::{outcome, outcome_text};
use super::cards::{note_key, note_text};
use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Trade;

pub struct Trade;

// ---- diplomacy -----------------------------------------------------------------------------

fn rel(r: md::Relationship) -> c::Relationship {
    c::Relationship { trust: r.trust, threat: r.threat, cooperation: r.cooperation, hostility: r.hostility }
}

fn rel_back(r: c::Relationship) -> md::Relationship {
    md::Relationship { trust: r.trust, threat: r.threat, cooperation: r.cooperation, hostility: r.hostility }
}

/// A note of a deal: `{alias}:{faction}`, and a Support for the Throne is `support:{faction}` (`promissory::support`).
fn deal_note(text: &str) -> R<c::NoteKey> {
    match text.strip_prefix("support:") {
        Some(faction) => Ok(c::NoteKey { note: ti4_engine_core::generated::PromissoryNote::SupportForTheThrone, faction: id::<ti4_engine_core::generated::Faction>(S, "faction", faction)? }),
        None => note_key(text),
    }
}

fn deal_note_text(k: c::NoteKey) -> String {
    if k.note == ti4_engine_core::generated::PromissoryNote::SupportForTheThrone {
        format!("support:{}", txt(k.faction))
    } else {
        note_text(k)
    }
}

fn asset(cx: &Ctx, a: &md::TransferAsset) -> R<c::TransferAsset> {
    let _ = cx;
    Ok(match a {
        md::TransferAsset::TradeGoods(n) => c::TransferAsset::TradeGoods(*n),
        md::TransferAsset::Commodities(n) => c::TransferAsset::Commodities(*n),
        md::TransferAsset::CulturalFragments(n) => c::TransferAsset::CulturalFragments(*n),
        md::TransferAsset::HazardousFragments(n) => c::TransferAsset::HazardousFragments(*n),
        md::TransferAsset::IndustrialFragments(n) => c::TransferAsset::IndustrialFragments(*n),
        md::TransferAsset::UnknownFragments(n) => c::TransferAsset::UnknownFragments(*n),
        md::TransferAsset::PromissoryNote(t) => c::TransferAsset::PromissoryNote(deal_note(t)?),
        md::TransferAsset::ActionCard(t) => c::TransferAsset::ActionCard(id::<ActionCard>(S, "action card", t.as_str())?),
        md::TransferAsset::SecretObjective(t) => {
            c::TransferAsset::SecretObjective(id::<SecretObjective>(S, "secret objective", t)?)
        }
    })
}

fn asset_back(a: &c::TransferAsset) -> md::TransferAsset {
    match a {
        c::TransferAsset::TradeGoods(n) => md::TransferAsset::TradeGoods(*n),
        c::TransferAsset::Commodities(n) => md::TransferAsset::Commodities(*n),
        c::TransferAsset::CulturalFragments(n) => md::TransferAsset::CulturalFragments(*n),
        c::TransferAsset::HazardousFragments(n) => md::TransferAsset::HazardousFragments(*n),
        c::TransferAsset::IndustrialFragments(n) => md::TransferAsset::IndustrialFragments(*n),
        c::TransferAsset::UnknownFragments(n) => md::TransferAsset::UnknownFragments(*n),
        c::TransferAsset::PromissoryNote(k) => md::TransferAsset::PromissoryNote(deal_note_text(*k)),
        c::TransferAsset::ActionCard(x) => md::TransferAsset::ActionCard(ActionCardId::new(txt(*x))),
        c::TransferAsset::SecretObjective(x) => md::TransferAsset::SecretObjective(txt(*x).to_owned()),
    }
}

pub(crate) fn term(cx: &mut Ctx, t: &md::DealTerm) -> R<c::DealTerm> {
    Ok(match t {
        md::DealTerm::ImmediateTransfer(a) => c::DealTerm::ImmediateTransfer(asset(cx, a)?),
        md::DealTerm::FuturePayment { asset: a, deadline_round } => {
            c::DealTerm::FuturePayment { asset: asset(cx, a)?, deadline_round: *deadline_round }
        }
        md::DealTerm::DoNotActivate { system, deadline_round } => c::DealTerm::DoNotActivate {
            system: id::<System>(S, "system", system.as_str())?,
            deadline_round: *deadline_round,
        },
        md::DealTerm::DoNotAttack { player, deadline_round } => {
            c::DealTerm::DoNotAttack { player: cx.seat(S, player.as_str())?, deadline_round: *deadline_round }
        }
        md::DealTerm::Vote { agenda, outcome: o, deadline_round } => c::DealTerm::Vote {
            agenda: id::<Agenda>(S, "agenda", agenda)?,
            outcome: outcome(cx, S, o)?,
            deadline_round: *deadline_round,
        },
        md::DealTerm::Attack { player, deadline_round } => {
            c::DealTerm::Attack { player: cx.seat(S, player.as_str())?, deadline_round: *deadline_round }
        }
        md::DealTerm::ReplenishFor { beneficiary, deadline_round } => {
            c::DealTerm::ReplenishFor { beneficiary: cx.seat(S, beneficiary.as_str())?, deadline_round: *deadline_round }
        }
        md::DealTerm::UseLeaderFor { leader, beneficiary, deadline_round } => c::DealTerm::UseLeaderFor {
            leader: id::<Leader>(S, "leader", leader)?,
            beneficiary: cx.seat(S, beneficiary.as_str())?,
            deadline_round: *deadline_round,
        },
    })
}

pub(crate) fn term_back(cx: &Ctx, t: &c::DealTerm) -> R<md::DealTerm> {
    Ok(match t {
        c::DealTerm::ImmediateTransfer(a) => md::DealTerm::ImmediateTransfer(asset_back(a)),
        c::DealTerm::FuturePayment { asset, deadline_round } => {
            md::DealTerm::FuturePayment { asset: asset_back(asset), deadline_round: *deadline_round }
        }
        c::DealTerm::DoNotActivate { system, deadline_round } => {
            md::DealTerm::DoNotActivate { system: SystemId::new(txt(*system)), deadline_round: *deadline_round }
        }
        c::DealTerm::DoNotAttack { player, deadline_round } => {
            md::DealTerm::DoNotAttack { player: PlayerId::new(cx.owner(S, *player)?), deadline_round: *deadline_round }
        }
        c::DealTerm::Vote { agenda, outcome, deadline_round } => md::DealTerm::Vote {
            agenda: txt(*agenda).to_owned(),
            outcome: outcome_text(cx, S, *outcome)?,
            deadline_round: *deadline_round,
        },
        c::DealTerm::Attack { player, deadline_round } => {
            md::DealTerm::Attack { player: PlayerId::new(cx.owner(S, *player)?), deadline_round: *deadline_round }
        }
        c::DealTerm::ReplenishFor { beneficiary, deadline_round } => md::DealTerm::ReplenishFor {
            beneficiary: PlayerId::new(cx.owner(S, *beneficiary)?),
            deadline_round: *deadline_round,
        },
        c::DealTerm::UseLeaderFor { leader, beneficiary, deadline_round } => md::DealTerm::UseLeaderFor {
            leader: txt(*leader).to_owned(),
            beneficiary: PlayerId::new(cx.owner(S, *beneficiary)?),
            deadline_round: *deadline_round,
        },
    })
}

fn pstatus(s: md::PromiseStatus) -> c::PromiseStatus {
    match s {
        md::PromiseStatus::Pending => c::PromiseStatus::Pending,
        md::PromiseStatus::Fulfilled => c::PromiseStatus::Fulfilled,
        md::PromiseStatus::Broken => c::PromiseStatus::Broken,
        md::PromiseStatus::Expired => c::PromiseStatus::Expired,
    }
}

fn pstatus_back(s: c::PromiseStatus) -> md::PromiseStatus {
    match s {
        c::PromiseStatus::Pending => md::PromiseStatus::Pending,
        c::PromiseStatus::Fulfilled => md::PromiseStatus::Fulfilled,
        c::PromiseStatus::Broken => md::PromiseStatus::Broken,
        c::PromiseStatus::Expired => md::PromiseStatus::Expired,
    }
}

fn dstatus(s: md::DealStatus) -> c::DealStatus {
    match s {
        md::DealStatus::Proposed => c::DealStatus::Proposed,
        md::DealStatus::Accepted => c::DealStatus::Accepted,
        md::DealStatus::Active => c::DealStatus::Active,
        md::DealStatus::Declined => c::DealStatus::Declined,
        md::DealStatus::Fulfilled => c::DealStatus::Fulfilled,
        md::DealStatus::Broken => c::DealStatus::Broken,
        md::DealStatus::Expired => c::DealStatus::Expired,
        md::DealStatus::Legacy => c::DealStatus::Legacy,
    }
}

fn dstatus_back(s: c::DealStatus) -> md::DealStatus {
    match s {
        c::DealStatus::Proposed => md::DealStatus::Proposed,
        c::DealStatus::Accepted => md::DealStatus::Accepted,
        c::DealStatus::Active => md::DealStatus::Active,
        c::DealStatus::Declined => md::DealStatus::Declined,
        c::DealStatus::Fulfilled => md::DealStatus::Fulfilled,
        c::DealStatus::Broken => md::DealStatus::Broken,
        c::DealStatus::Expired => md::DealStatus::Expired,
        c::DealStatus::Legacy => md::DealStatus::Legacy,
    }
}

fn terms<const N: usize>(cx: &mut Ctx, v: &[md::DealTerm]) -> R<SmallList<c::DealTerm, N>> {
    let mut out = SmallList::new();
    for t in v {
        out.push(term(cx, t)?);
    }
    Ok(out)
}

pub(crate) fn revision(cx: &mut Ctx, r: &md::DealRevision) -> R<c::DealRevision> {
    Ok(c::DealRevision {
        number: r.number,
        author: cx.seat(S, r.author.as_str())?,
        proposer_terms: terms(cx, &r.proposer_terms)?,
        recipient_terms: terms(cx, &r.recipient_terms)?,
        proposer_statuses: r.proposer_statuses.iter().map(|s| pstatus(*s)).collect(),
        recipient_statuses: r.recipient_statuses.iter().map(|s| pstatus(*s)).collect(),
    })
}

pub(crate) fn revision_back(cx: &Ctx, r: &c::DealRevision) -> R<md::DealRevision> {
    Ok(md::DealRevision {
        number: r.number,
        author: PlayerId::new(cx.owner(S, r.author)?),
        proposer_terms: r.proposer_terms.iter().map(|t| term_back(cx, t)).collect::<R<_>>()?,
        recipient_terms: r.recipient_terms.iter().map(|t| term_back(cx, t)).collect::<R<_>>()?,
        proposer_statuses: r.proposer_statuses.iter().map(|s| pstatus_back(*s)).collect(),
        recipient_statuses: r.recipient_statuses.iter().map(|s| pstatus_back(*s)).collect(),
    })
}

pub(crate) fn statement(cx: &mut Ctx, s: &md::SignalStatement) -> R<c::SignalStatement> {
    Ok(match s {
        md::SignalStatement::WillVote { agenda, outcome: o } => {
            c::SignalStatement::WillVote { agenda: id::<Agenda>(S, "agenda", agenda)?, outcome: outcome(cx, S, o)? }
        }
        md::SignalStatement::AttackIfYouActivate { system } => {
            c::SignalStatement::AttackIfYouActivate { system: id::<System>(S, "system", system.as_str())? }
        }
    })
}

pub(crate) fn statement_back(cx: &Ctx, s: &c::SignalStatement) -> R<md::SignalStatement> {
    Ok(match s {
        c::SignalStatement::WillVote { agenda, outcome } => {
            md::SignalStatement::WillVote { agenda: txt(*agenda).to_owned(), outcome: outcome_text(cx, S, *outcome)? }
        }
        c::SignalStatement::AttackIfYouActivate { system } => {
            md::SignalStatement::AttackIfYouActivate { system: SystemId::new(txt(*system)) }
        }
    })
}

fn kind(k: md::SignalKind) -> c::SignalKind {
    match k {
        md::SignalKind::Request => c::SignalKind::Request,
        md::SignalKind::Threat => c::SignalKind::Threat,
        md::SignalKind::Assurance => c::SignalKind::Assurance,
        md::SignalKind::Warning => c::SignalKind::Warning,
    }
}

fn kind_back(k: c::SignalKind) -> md::SignalKind {
    match k {
        c::SignalKind::Request => md::SignalKind::Request,
        c::SignalKind::Threat => md::SignalKind::Threat,
        c::SignalKind::Assurance => md::SignalKind::Assurance,
        c::SignalKind::Warning => md::SignalKind::Warning,
    }
}

fn sstatus(s: md::SignalStatus) -> c::SignalStatus {
    use md::SignalStatus as M;
    match s {
        M::Open => c::SignalStatus::Open,
        M::Honoured => c::SignalStatus::Honoured,
        M::Broken => c::SignalStatus::Broken,
        M::Heeded => c::SignalStatus::Heeded,
        M::Ignored => c::SignalStatus::Ignored,
        M::Triggered => c::SignalStatus::Triggered,
        M::CarriedOut => c::SignalStatus::CarriedOut,
        M::Bluffed => c::SignalStatus::Bluffed,
    }
}

fn sstatus_back(s: c::SignalStatus) -> md::SignalStatus {
    use md::SignalStatus as M;
    match s {
        c::SignalStatus::Open => M::Open,
        c::SignalStatus::Honoured => M::Honoured,
        c::SignalStatus::Broken => M::Broken,
        c::SignalStatus::Heeded => M::Heeded,
        c::SignalStatus::Ignored => M::Ignored,
        c::SignalStatus::Triggered => M::Triggered,
        c::SignalStatus::CarriedOut => M::CarriedOut,
        c::SignalStatus::Bluffed => M::Bluffed,
    }
}

pub(crate) fn signal(cx: &mut Ctx, s: &md::Signal) -> R<c::Signal> {
    Ok(c::Signal {
        id: c::SignalId(s.id.0),
        speaker: cx.seat(S, s.speaker.as_str())?,
        target: cx.seat(S, s.target.as_str())?,
        kind: kind(s.kind),
        statement: statement(cx, &s.statement)?,
        status: sstatus(s.status),
        created_round: s.created_round,
        expires_round: s.expires_round,
    })
}

pub(crate) fn signal_back(cx: &Ctx, s: &c::Signal) -> R<md::Signal> {
    Ok(md::Signal {
        id: md::SignalId(s.id.0),
        speaker: PlayerId::new(cx.owner(S, s.speaker)?),
        target: PlayerId::new(cx.owner(S, s.target)?),
        kind: kind_back(s.kind),
        statement: statement_back(cx, &s.statement)?,
        status: sstatus_back(s.status),
        created_round: s.created_round,
        expires_round: s.expires_round,
    })
}

fn snapshot(s: &md::DealRelationshipSnapshot) -> c::DealRelationshipSnapshot {
    c::DealRelationshipSnapshot {
        proposer_to_recipient: rel(s.proposer_to_recipient),
        recipient_to_proposer: rel(s.recipient_to_proposer),
    }
}

fn snapshot_back(s: &c::DealRelationshipSnapshot) -> md::DealRelationshipSnapshot {
    md::DealRelationshipSnapshot {
        proposer_to_recipient: rel_back(s.proposer_to_recipient),
        recipient_to_proposer: rel_back(s.recipient_to_proposer),
    }
}

fn event(cx: &mut Ctx, e: &md::DiplomacyEvent) -> R<c::DiplomacyEvent> {
    use md::DiplomacyEvent as M;
    Ok(match e {
        M::Offered { deal_id, proposer, recipient, relationship_snapshot, revision: r } => c::DiplomacyEvent::Offered {
            deal_id: c::DealId(deal_id.0),
            proposer: cx.seat(S, proposer.as_str())?,
            recipient: cx.seat(S, recipient.as_str())?,
            relationship_snapshot: snapshot(relationship_snapshot),
            revision: revision(cx, r)?,
        },
        M::Countered { deal_id, revision: r } => {
            c::DiplomacyEvent::Countered { deal_id: c::DealId(deal_id.0), revision: revision(cx, r)? }
        }
        M::Accepted { deal_id, actor, revision } => c::DiplomacyEvent::Accepted {
            deal_id: c::DealId(deal_id.0),
            actor: cx.seat(S, actor.as_str())?,
            revision: *revision,
        },
        M::Declined { deal_id, actor, revision } => c::DiplomacyEvent::Declined {
            deal_id: c::DealId(deal_id.0),
            actor: cx.seat(S, actor.as_str())?,
            revision: *revision,
        },
        M::ImmediateApplied { deal_id, term_index } => {
            c::DiplomacyEvent::ImmediateApplied { deal_id: c::DealId(deal_id.0), term_index: *term_index }
        }
        M::PromiseSettled { deal_id, term_index, status } => c::DiplomacyEvent::PromiseSettled {
            deal_id: c::DealId(deal_id.0),
            term_index: *term_index,
            status: pstatus(*status),
        },
        M::DealSettled { deal_id, status } => {
            c::DiplomacyEvent::DealSettled { deal_id: c::DealId(deal_id.0), status: dstatus(*status) }
        }
        M::SignalEmitted { signal_id } => c::DiplomacyEvent::SignalEmitted { signal_id: c::SignalId(signal_id.0) },
        M::SignalJudged { signal: s } => c::DiplomacyEvent::SignalJudged { signal: signal(cx, s)? },
    })
}

fn event_back(cx: &Ctx, e: &c::DiplomacyEvent) -> R<md::DiplomacyEvent> {
    use md::DiplomacyEvent as M;
    Ok(match e {
        c::DiplomacyEvent::Offered { deal_id, proposer, recipient, relationship_snapshot, revision } => M::Offered {
            deal_id: md::DealId(deal_id.0),
            proposer: PlayerId::new(cx.owner(S, *proposer)?),
            recipient: PlayerId::new(cx.owner(S, *recipient)?),
            relationship_snapshot: snapshot_back(relationship_snapshot),
            revision: revision_back(cx, revision)?,
        },
        c::DiplomacyEvent::Countered { deal_id, revision } => {
            M::Countered { deal_id: md::DealId(deal_id.0), revision: revision_back(cx, revision)? }
        }
        c::DiplomacyEvent::Accepted { deal_id, actor, revision } => {
            M::Accepted { deal_id: md::DealId(deal_id.0), actor: PlayerId::new(cx.owner(S, *actor)?), revision: *revision }
        }
        c::DiplomacyEvent::Declined { deal_id, actor, revision } => {
            M::Declined { deal_id: md::DealId(deal_id.0), actor: PlayerId::new(cx.owner(S, *actor)?), revision: *revision }
        }
        c::DiplomacyEvent::ImmediateApplied { deal_id, term_index } => {
            M::ImmediateApplied { deal_id: md::DealId(deal_id.0), term_index: *term_index }
        }
        c::DiplomacyEvent::PromiseSettled { deal_id, term_index, status } => {
            M::PromiseSettled { deal_id: md::DealId(deal_id.0), term_index: *term_index, status: pstatus_back(*status) }
        }
        c::DiplomacyEvent::DealSettled { deal_id, status } => {
            M::DealSettled { deal_id: md::DealId(deal_id.0), status: dstatus_back(*status) }
        }
        c::DiplomacyEvent::SignalEmitted { signal_id } => M::SignalEmitted { signal_id: md::SignalId(signal_id.0) },
        c::DiplomacyEvent::SignalJudged { signal } => M::SignalJudged { signal: signal_back(cx, signal)? },
    })
}

fn pair_map<V: Copy>(
    cx: &Ctx,
    m: &std::collections::BTreeMap<PlayerId, std::collections::BTreeMap<PlayerId, V>>,
) -> R<SortedMap<(Seat, Seat), V, 8>> {
    let mut out = SortedMap::new();
    for (a, row) in m {
        for (b, v) in row {
            out.insert((cx.seat(S, a.as_str())?, cx.seat(S, b.as_str())?), *v);
        }
    }
    Ok(out)
}

fn pair_map_back<V: Copy>(
    cx: &Ctx,
    m: &SortedMap<(Seat, Seat), V, 8>,
    keep_empty_rows: &std::collections::BTreeSet<PlayerId>,
) -> R<std::collections::BTreeMap<PlayerId, std::collections::BTreeMap<PlayerId, V>>> {
    let mut out: std::collections::BTreeMap<PlayerId, std::collections::BTreeMap<PlayerId, V>> =
        keep_empty_rows.iter().map(|p| (p.clone(), std::collections::BTreeMap::new())).collect();
    for ((a, b), v) in m.iter() {
        out.entry(PlayerId::new(cx.owner(S, *a)?)).or_default().insert(PlayerId::new(cx.owner(S, *b)?), *v);
    }
    Ok(out)
}

fn diplomacy(cx: &mut Ctx, d: &md::DiplomacyState) -> R<c::Diplomacy> {
    let mut o = c::Diplomacy {
        enabled: d.enabled,
        rules: cx.texts.intern(S, &d.rules_version)?,
        next_deal_id: d.next_deal_id,
        next_signal_id: d.next_signal_id,
        ..c::Diplomacy::default()
    };
    // A relationships row with no entries cannot be a key of the flat typed map: unmapped.
    if d.relationships.values().any(std::collections::BTreeMap::is_empty) {
        return Err(bad(S, "diplomacy: an empty relationships row", ""));
    }
    for ((a, b), v) in pair_map(cx, &d.relationships)?.iter() {
        o.relationships.insert((*a, *b), rel(*v));
    }
    for (deal_id, deal) in &d.active_deals {
        if deal_id != &deal.id {
            return Err(bad(S, "diplomacy: deal key differs from its id", deal_id.0));
        }
        o.active_deals.push(c::Deal {
            id: c::DealId(deal.id.0),
            proposer: cx.seat(S, deal.proposer.as_str())?,
            recipient: cx.seat(S, deal.recipient.as_str())?,
            created_round: deal.created_round,
            revisions: deal.revisions.iter().map(|r| revision(cx, r)).collect::<R<_>>()?,
            status: dstatus(deal.status),
        });
    }
    for h in &d.history {
        o.history.push(c::TerminalDealSummary {
            id: c::DealId(h.id.0),
            proposer: cx.seat(S, h.proposer.as_str())?,
            recipient: cx.seat(S, h.recipient.as_str())?,
            created_round: h.created_round,
            terminal_round: h.terminal_round,
            status: dstatus(h.status),
            latest_revision: h.latest_revision.as_ref().map(|r| revision(cx, r)).transpose()?,
            legacy_promise: h.legacy_promise.as_deref().map(|t| cx.texts.intern(S, t)).transpose()?,
            legacy_status: h.legacy_status.map(pstatus),
        });
    }
    for s in &d.recent_signals {
        o.recent_signals.push(signal(cx, s)?);
    }
    for j in &d.journal {
        o.journal.push(c::DiplomacyJournalEntry { round: j.round, event: event(cx, &j.event)? });
    }
    for (p, set) in &d.initiations_this_turn {
        o.initiations_this_turn.insert(
            cx.seat(S, p.as_str())?,
            set.iter().map(|x| cx.seat(S, x.as_str())).collect::<R<SeatSet>>()?,
        );
    }
    o.last_attacks = pair_map(cx, &d.last_attacks)?;
    o.last_breaches = pair_map(cx, &d.last_breaches)?;
    for (round, a, b) in &d.judged_attacks {
        o.judged_attacks.insert((*round, cx.seat(S, a.as_str())?, cx.seat(S, b.as_str())?), ());
    }
    Ok(o)
}

fn diplomacy_back(cx: &Ctx, d: &c::Diplomacy) -> R<md::DiplomacyState> {
    let none = std::collections::BTreeSet::new();
    let mut o = md::DiplomacyState {
        enabled: d.enabled,
        rules_version: cx.texts.text(S, d.rules)?.to_owned(),
        next_deal_id: d.next_deal_id,
        next_signal_id: d.next_signal_id,
        ..md::DiplomacyState::default()
    };
    o.relationships = pair_map_back(cx, &d.relationships, &none)?
        .into_iter()
        .map(|(k, row)| (k, row.into_iter().map(|(b, v)| (b, rel_back(v))).collect()))
        .collect();
    for deal in &d.active_deals {
        o.active_deals.insert(
            md::DealId(deal.id.0),
            md::Deal {
                id: md::DealId(deal.id.0),
                proposer: PlayerId::new(cx.owner(S, deal.proposer)?),
                recipient: PlayerId::new(cx.owner(S, deal.recipient)?),
                created_round: deal.created_round,
                revisions: deal.revisions.iter().map(|r| revision_back(cx, r)).collect::<R<_>>()?,
                status: dstatus_back(deal.status),
            },
        );
    }
    for h in &d.history {
        o.history.push(md::TerminalDealSummary {
            id: md::DealId(h.id.0),
            proposer: PlayerId::new(cx.owner(S, h.proposer)?),
            recipient: PlayerId::new(cx.owner(S, h.recipient)?),
            created_round: h.created_round,
            terminal_round: h.terminal_round,
            status: dstatus_back(h.status),
            latest_revision: h.latest_revision.as_ref().map(|r| revision_back(cx, r)).transpose()?,
            legacy_promise: h.legacy_promise.map(|c| cx.texts.text(S, c).map(str::to_owned)).transpose()?,
            legacy_status: h.legacy_status.map(pstatus_back),
        });
    }
    o.recent_signals = d.recent_signals.iter().map(|s| signal_back(cx, s)).collect::<R<_>>()?;
    o.journal = d
        .journal
        .iter()
        .map(|j| Ok(md::DiplomacyJournalEntry { round: j.round, event: event_back(cx, &j.event)? }))
        .collect::<R<_>>()?;
    o.initiations_this_turn = d
        .initiations_this_turn
        .iter()
        .map(|(p, set)| {
            Ok((
                PlayerId::new(cx.owner(S, *p)?),
                set.iter().map(|s| cx.owner(S, s).map(PlayerId::new)).collect::<R<_>>()?,
            ))
        })
        .collect::<R<_>>()?;
    o.last_attacks = pair_map_back(cx, &d.last_attacks, &none)?;
    o.last_breaches = pair_map_back(cx, &d.last_breaches, &none)?;
    o.judged_attacks = d
        .judged_attacks
        .keys()
        .map(|(r, a, b)| Ok((r, PlayerId::new(cx.owner(S, a)?), PlayerId::new(cx.owner(S, b)?))))
        .collect::<R<_>>()?;
    Ok(o)
}

// ---- the slice ---------------------------------------------------------------------------

fn seat_map<V: Copy, const N: usize>(
    cx: &Ctx,
    m: &std::collections::BTreeMap<PlayerId, V>,
) -> R<SortedMap<Seat, V, N>> {
    let mut out = SortedMap::new();
    for (k, v) in m {
        out.insert(cx.seat(S, k.as_str())?, *v);
    }
    Ok(out)
}

fn seat_map_back<V: Copy, const N: usize>(
    cx: &Ctx,
    m: &SortedMap<Seat, V, N>,
) -> R<std::collections::BTreeMap<PlayerId, V>> {
    m.iter().map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), *v))).collect()
}

impl SliceCodec for Trade {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut t = c::TradeState::default();
        t.this_round = Cow::new(
            gs.transactions_this_round
                .iter()
                .map(|(a, b)| Ok((cx.seat(S, a.as_str())?, cx.seat(S, b.as_str())?)))
                .collect::<R<Vec<_>>>()?,
        );
        t.negotiations_this_action = seat_map(cx, &gs.negotiations_this_action)?;
        t.negotiations_this_round = seat_map(cx, &gs.negotiations_this_round)?;
        for (p, set) in &gs.transactions_this_turn {
            t.transactions_this_turn.insert(
                cx.seat(S, p.as_str())?,
                set.iter().map(|x| cx.seat(S, x.as_str())).collect::<R<SeatSet>>()?,
            );
        }
        t.traded_goods = seat_map(cx, &gs.traded_goods)?;
        t.traded_goods_for_promissory = seat_map(cx, &gs.traded_goods_for_promissory)?;
        let mut promises = SortedMap::new();
        for ((a, b, text), kept) in &gs.promises {
            promises.insert((cx.seat(S, a.as_str())?, cx.seat(S, b.as_str())?, cx.texts.intern(S, text)?), *kept);
        }
        t.promises = Cow::new(promises);
        for (k, v) in &gs.support_holders {
            t.support_holders.insert(cx.seat(S, k.as_str())?, cx.seat(S, v.as_str())?);
        }
        t.diplomacy = Cow::new(diplomacy(cx, &gs.diplomacy)?);
        core.trade = t;
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let t = core.trade();
        gs.transactions_this_round = t
            .this_round
            .iter()
            .map(|(a, b)| Ok((PlayerId::new(cx.owner(S, *a)?), PlayerId::new(cx.owner(S, *b)?))))
            .collect::<R<_>>()?;
        gs.negotiations_this_action = seat_map_back(cx, &t.negotiations_this_action)?;
        gs.negotiations_this_round = seat_map_back(cx, &t.negotiations_this_round)?;
        gs.transactions_this_turn = t
            .transactions_this_turn
            .iter()
            .map(|(p, set)| {
                Ok((
                    PlayerId::new(cx.owner(S, *p)?),
                    set.iter().map(|s| cx.owner(S, s).map(PlayerId::new)).collect::<R<_>>()?,
                ))
            })
            .collect::<R<_>>()?;
        gs.traded_goods = seat_map_back(cx, &t.traded_goods)?;
        gs.traded_goods_for_promissory = seat_map_back(cx, &t.traded_goods_for_promissory)?;
        gs.promises = t
            .promises
            .iter()
            .map(|((a, b, text), kept)| {
                Ok((
                    (PlayerId::new(cx.owner(S, *a)?), PlayerId::new(cx.owner(S, *b)?), cx.texts.text(S, *text)?.to_owned()),
                    *kept,
                ))
            })
            .collect::<R<_>>()?;
        gs.support_holders = t
            .support_holders
            .iter()
            .map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), PlayerId::new(cx.owner(S, *v)?))))
            .collect::<R<_>>()?;
        gs.diplomacy = diplomacy_back(cx, &t.diplomacy)?;
        Ok(())
    }
}
