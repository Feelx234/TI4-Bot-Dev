//! Diplomacy (rules v1) that changes the typed state: the relationship-delta table, the decay at the
//! end of a round, the per-turn initiation tally. Package B4. String side: `diplomacy/relations.rs`
//! and `ti4_model::DiplomacyState::{consume_initiation, clear_turn_initiations, prune_signals}`.
//!
//! The deal book, the signals and the promise ledger (`diplomacy/{window,signals,promises}.rs`) are
//! not here: their records carry free text and per-term asset lists that the engine keeps as strings
//! (see `docs/ENGINE_KNOWN_ISSUES.md`, B4).

use crate::ids::Seat;
use crate::state::{CoreState, Relationship, SignalKind};

/// One semantic event of `diplomacy::RelationshipEvent`, over seats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelEvent {
    FairTransaction { a: Seat, b: Seat },
    DealAccepted { a: Seat, b: Seat },
    PromiseFulfilled { promiser: Seat, beneficiary: Seat },
    PromiseBroken { promiser: Seat, beneficiary: Seat },
    DirectAttack { attacker: Seat, victim: Seat, activation_seq: u32 },
    SignalMade { speaker: Seat, target: Seat, kind: SignalKind },
    AssuranceKept { speaker: Seat, target: Seat },
    AssuranceBroken { speaker: Seat, target: Seat },
    RequestHeeded { speaker: Seat, target: Seat },
    RequestIgnored { speaker: Seat, target: Seat },
    ThreatCarriedOut { speaker: Seat, target: Seat },
    ThreatBluffed { speaker: Seat, target: Seat },
    AntiThirdPartyDeal { payer: Seat, attacker: Seat, target: Seat },
}

/// A directional pair of one seat with itself (`DiplomacyError::SelfPair`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfPair;

/// `Relationship::adjusted`.
#[must_use]
pub fn adjusted(r: Relationship, trust: i16, threat: i16, cooperation: i16, hostility: i16) -> Relationship {
    Relationship {
        trust: (r.trust + trust).clamp(-100, 100),
        threat: u8::try_from((i16::from(r.threat) + threat).clamp(0, 100)).unwrap_or_default(),
        cooperation: (r.cooperation + cooperation).clamp(-100, 100),
        hostility: u8::try_from((i16::from(r.hostility) + hostility).clamp(0, 100)).unwrap_or_default(),
    }
}

/// `Relationship::decay`.
#[must_use]
pub fn decayed(r: Relationship) -> Relationship {
    Relationship {
        trust: r.trust * 90 / 100,
        cooperation: r.cooperation * 90 / 100,
        threat: u8::try_from(u16::from(r.threat) * 80 / 100).unwrap_or_default(),
        hostility: u8::try_from(u16::from(r.hostility) * 80 / 100).unwrap_or_default(),
    }
}

fn adjust(core: &mut CoreState, observer: Seat, subject: Seat, d: [i16; 4]) -> Result<(), SelfPair> {
    if observer == subject {
        return Err(SelfPair);
    }
    let rel = core.trade_mut().diplomacy.edit().relationships.get_or_insert_with((observer, subject), Relationship::default);
    *rel = adjusted(*rel, d[0], d[1], d[2], d[3]);
    Ok(())
}

/// A seat holds a row of the relationship matrix when any pair names it as the observer.
fn has_row(core: &CoreState, who: Seat) -> bool {
    core.trade().diplomacy.relationships.keys().any(|(observer, _)| observer == who)
}

fn named(e: &RelEvent) -> [Option<Seat>; 3] {
    use RelEvent::*;
    match *e {
        FairTransaction { a, b } | DealAccepted { a, b } => [Some(a), Some(b), None],
        PromiseFulfilled { promiser, beneficiary } | PromiseBroken { promiser, beneficiary } => [Some(promiser), Some(beneficiary), None],
        DirectAttack { attacker, victim, .. } => [Some(attacker), Some(victim), None],
        SignalMade { speaker, target, .. }
        | AssuranceKept { speaker, target }
        | AssuranceBroken { speaker, target }
        | RequestHeeded { speaker, target }
        | RequestIgnored { speaker, target }
        | ThreatCarriedOut { speaker, target }
        | ThreatBluffed { speaker, target } => [Some(speaker), Some(target), None],
        AntiThirdPartyDeal { payer, attacker, target } => [Some(payer), Some(attacker), Some(target)],
    }
}

/// `diplomacy::apply_relationship_event`: the fixed v1 delta of one event. Nothing happens with
/// diplomacy off or when a named seat has no row in the matrix (a neutral owner).
///
/// # Errors
/// [`SelfPair`] when the event names one seat twice.
pub fn apply_relationship_event(core: &mut CoreState, event: RelEvent) -> Result<(), SelfPair> {
    use RelEvent::*;
    if core.trade().diplomacy.enabled {
        // XF-1: structured diplomacy is switched off; dead after the flip (D2-8), deleted in D4.
        crate::unsupported::hit(crate::unsupported::Disabled::Diplomacy);
    }
    if !core.trade().diplomacy.enabled || !named(&event).into_iter().flatten().all(|s| has_row(core, s)) {
        return Ok(());
    }
    let round = core.meta().round;
    match event {
        FairTransaction { a, b } => {
            adjust(core, a, b, [3, 0, 3, 0])?;
            adjust(core, b, a, [3, 0, 3, 0])
        }
        DealAccepted { a, b } => {
            adjust(core, a, b, [5, 0, 5, 0])?;
            adjust(core, b, a, [5, 0, 5, 0])
        }
        PromiseFulfilled { promiser, beneficiary } => adjust(core, beneficiary, promiser, [20, 0, 15, 0]),
        PromiseBroken { promiser, beneficiary } => {
            adjust(core, beneficiary, promiser, [-40, 15, 0, 25])?;
            core.trade_mut().diplomacy.edit().last_breaches.insert((beneficiary, promiser), round);
            Ok(())
        }
        DirectAttack { attacker, victim, activation_seq } => {
            let d = core.trade_mut().diplomacy.edit();
            if d.judged_attacks.insert((activation_seq, attacker, victim), ()).is_some() {
                return Ok(());
            }
            adjust(core, victim, attacker, [-10, 15, 0, 20])?;
            core.trade_mut().diplomacy.edit().last_attacks.insert((victim, attacker), round);
            Ok(())
        }
        SignalMade { speaker, target, kind } => match kind {
            SignalKind::Threat => adjust(core, target, speaker, [0, 10, 0, 5]),
            SignalKind::Warning => adjust(core, target, speaker, [0, 4, 0, 2]),
            SignalKind::Assurance => adjust(core, target, speaker, [2, 0, 0, 0]),
            SignalKind::Request => Ok(()),
        },
        AssuranceKept { speaker, target } => adjust(core, target, speaker, [8, 0, 4, 0]),
        AssuranceBroken { speaker, target } => {
            adjust(core, target, speaker, [-25, 10, 0, 10])?;
            core.trade_mut().diplomacy.edit().last_breaches.insert((target, speaker), round);
            Ok(())
        }
        RequestHeeded { speaker, target } => adjust(core, speaker, target, [4, 0, 2, 0]),
        RequestIgnored { speaker, target } => adjust(core, speaker, target, [-5, 0, 0, 5]),
        ThreatCarriedOut { speaker, target } => adjust(core, target, speaker, [0, 10, 0, 0]),
        ThreatBluffed { speaker, target } => adjust(core, target, speaker, [0, -10, 0, 0]),
        AntiThirdPartyDeal { payer, attacker, target } => {
            adjust(core, target, payer, [-3, 0, 0, 3])?;
            adjust(core, target, attacker, [-3, 0, 0, 3])
        }
    }
}

/// `diplomacy::decay_relationships`: every relationship decays, and the signals that expired before
/// the next round are dropped.
pub fn decay_relationships(core: &mut CoreState) {
    if !core.trade().diplomacy.enabled {
        return;
    }
    crate::unsupported::hit(crate::unsupported::Disabled::Diplomacy);
    let next = core.meta().round.saturating_add(1);
    let d = core.trade_mut().diplomacy.edit();
    let rows: Vec<_> = d.relationships.iter().map(|(k, v)| (*k, decayed(*v))).collect();
    for (k, v) in rows {
        d.relationships.insert(k, v);
    }
    d.recent_signals.retain(|s| s.expires_round >= next);
}

/// `DiplomacyState::consume_initiation`: the first contact of `actor` with `target` this turn.
pub fn consume_initiation(core: &mut CoreState, actor: Seat, target: Seat) -> bool {
    actor != target
        && core.trade_mut().diplomacy.edit().initiations_this_turn.get_or_insert_with(actor, Default::default).insert(target)
}

/// `DiplomacyState::clear_turn_initiations`.
pub fn clear_turn_initiations(core: &mut CoreState) {
    core.trade_mut().diplomacy.edit().initiations_this_turn.clear();
}
