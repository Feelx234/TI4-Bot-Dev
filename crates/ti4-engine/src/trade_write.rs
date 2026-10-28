//! `Area::TradeDiplomacy` (package B4), the mutation half that is not a transaction: the diplomacy
//! relationship table, its decay and the per-turn initiation tally as `ti4_engine_core::rules::diplomacy`
//! over the Trade slice.
//!
//! Every function answers `None` when the position is outside what the core holds (more than 16
//! owners, a relationships row the flat typed map cannot hold, a seat the table does not know): the
//! caller keeps the string write. The deal book, the signals and the promise ledger stay string; see
//! `docs/ENGINE_KNOWN_ISSUES.md`, B4.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::diplomacy as rules;
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::diplomacy::SignalKind;
    use ti4_model::state::GameState;
    use ti4_model::{DiplomacyError, PlayerId};

    use crate::core_sync::{Ctx, Footprint, Sources, with_core};
    use crate::diplomacy::RelationshipEvent;

    /// The relationship table reads the round (Meta) and writes the diplomacy state (Trade).
    const DIPLOMACY: Footprint = Footprint::new(&[Slice::Meta, Slice::Trade], &[Slice::Trade]);

    fn lend<T>(gs: &mut GameState, f: impl FnOnce(&mut CoreState) -> T) -> Option<T> {
        let mut cx = Ctx::of(gs).ok()?;
        with_core(gs, &mut cx, Sources::default(), DIPLOMACY, |core, _| f(core)).ok()
    }

    fn kind(k: SignalKind) -> ti4_engine_core::state::SignalKind {
        match k {
            SignalKind::Request => ti4_engine_core::state::SignalKind::Request,
            SignalKind::Threat => ti4_engine_core::state::SignalKind::Threat,
            SignalKind::Assurance => ti4_engine_core::state::SignalKind::Assurance,
            SignalKind::Warning => ti4_engine_core::state::SignalKind::Warning,
        }
    }

    fn event(cx: &Ctx, e: &RelationshipEvent) -> Option<rules::RelEvent> {
        use RelationshipEvent as E;
        let s = |p: &PlayerId| cx.owners.seat(p.as_str());
        Some(match e {
            E::FairTransaction { a, b } => rules::RelEvent::FairTransaction { a: s(a)?, b: s(b)? },
            E::DealAccepted { a, b } => rules::RelEvent::DealAccepted { a: s(a)?, b: s(b)? },
            E::PromiseFulfilled { promiser, beneficiary } => rules::RelEvent::PromiseFulfilled { promiser: s(promiser)?, beneficiary: s(beneficiary)? },
            E::PromiseBroken { promiser, beneficiary } => rules::RelEvent::PromiseBroken { promiser: s(promiser)?, beneficiary: s(beneficiary)? },
            E::DirectAttack { attacker, victim, activation_seq } => {
                rules::RelEvent::DirectAttack { attacker: s(attacker)?, victim: s(victim)?, activation_seq: *activation_seq }
            }
            E::SignalMade { speaker, target, kind: k } => rules::RelEvent::SignalMade { speaker: s(speaker)?, target: s(target)?, kind: kind(*k) },
            E::AssuranceKept { speaker, target } => rules::RelEvent::AssuranceKept { speaker: s(speaker)?, target: s(target)? },
            E::AssuranceBroken { speaker, target } => rules::RelEvent::AssuranceBroken { speaker: s(speaker)?, target: s(target)? },
            E::RequestHeeded { speaker, target } => rules::RelEvent::RequestHeeded { speaker: s(speaker)?, target: s(target)? },
            E::RequestIgnored { speaker, target } => rules::RelEvent::RequestIgnored { speaker: s(speaker)?, target: s(target)? },
            E::ThreatCarriedOut { speaker, target } => rules::RelEvent::ThreatCarriedOut { speaker: s(speaker)?, target: s(target)? },
            E::ThreatBluffed { speaker, target } => rules::RelEvent::ThreatBluffed { speaker: s(speaker)?, target: s(target)? },
            E::AntiThirdPartyDeal { payer, attacker, target } => {
                rules::RelEvent::AntiThirdPartyDeal { payer: s(payer)?, attacker: s(attacker)?, target: s(target)? }
            }
        })
    }

    /// `diplomacy::apply_relationship_event`. A neutral owner (no seat) is a bail, not a no-op: the
    /// string engine decides it from the matrix rows.
    pub fn relationship_event(gs: &mut GameState, e: &RelationshipEvent) -> Option<Result<(), DiplomacyError>> {
        let probe = Ctx::of(gs).ok()?;
        let typed = event(&probe, e)?;
        lend(gs, |core| rules::apply_relationship_event(core, typed).map_err(|rules::SelfPair| DiplomacyError::SelfPair))
    }

    /// `diplomacy::decay_relationships`.
    pub fn decay_relationships(gs: &mut GameState) -> Option<()> {
        lend(gs, |core| rules::decay_relationships(core))
    }

    /// `DiplomacyState::consume_initiation`.
    pub fn consume_initiation(gs: &mut GameState, actor: &PlayerId, target: &PlayerId) -> Option<bool> {
        let probe = Ctx::of(gs).ok()?;
        let (a, t): (Seat, Seat) = (probe.owners.seat(actor.as_str())?, probe.owners.seat(target.as_str())?);
        lend(gs, |core| rules::consume_initiation(core, a, t))
    }

    /// `DiplomacyState::clear_turn_initiations`.
    pub fn clear_turn_initiations(gs: &mut GameState) -> Option<()> {
        lend(gs, |core| rules::clear_turn_initiations(core))
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::state::GameState;
    use ti4_model::{DiplomacyError, PlayerId};

    use crate::diplomacy::RelationshipEvent;

    pub fn relationship_event(_: &mut GameState, _: &RelationshipEvent) -> Option<Result<(), DiplomacyError>> {
        None
    }
    pub fn decay_relationships(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn consume_initiation(_: &mut GameState, _: &PlayerId, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn clear_turn_initiations(_: &mut GameState) -> Option<()> {
        None
    }
}

#[cfg(test)]
mod tests {
    use ti4_model::content_types::POK;
    use ti4_model::diplomacy::{DiplomacyState, SignalKind};
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::diplomacy::{RelationshipEvent as E, relations};
    use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};

    fn p(s: &str) -> PlayerId {
        PlayerId::new(s)
    }

    fn table(enabled: bool) -> GameState {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "letnev")], POK);
        state.round = 3;
        let ids: Vec<PlayerId> = state.players.iter().map(|x| x.id.clone()).collect();
        state.diplomacy = DiplomacyState::for_players(&ids, enabled);
        state
    }

    fn events() -> Vec<E> {
        let (a, b, c) = (p("a"), p("b"), p("c"));
        let two = |x: &PlayerId, y: &PlayerId| (x.clone(), y.clone());
        let (s, t) = two(&a, &b);
        vec![
            E::FairTransaction { a: a.clone(), b: b.clone() },
            E::DealAccepted { a: b.clone(), b: c.clone() },
            E::PromiseFulfilled { promiser: a.clone(), beneficiary: b.clone() },
            E::PromiseBroken { promiser: a.clone(), beneficiary: c.clone() },
            E::DirectAttack { attacker: a.clone(), victim: b.clone(), activation_seq: 7 },
            E::DirectAttack { attacker: a.clone(), victim: b.clone(), activation_seq: 7 },
            E::SignalMade { speaker: s.clone(), target: t.clone(), kind: SignalKind::Threat },
            E::SignalMade { speaker: s.clone(), target: t.clone(), kind: SignalKind::Warning },
            E::SignalMade { speaker: s.clone(), target: t.clone(), kind: SignalKind::Assurance },
            E::SignalMade { speaker: s.clone(), target: t.clone(), kind: SignalKind::Request },
            E::AssuranceKept { speaker: c.clone(), target: a.clone() },
            E::AssuranceBroken { speaker: c.clone(), target: a.clone() },
            E::RequestHeeded { speaker: b.clone(), target: c.clone() },
            E::RequestIgnored { speaker: b.clone(), target: c.clone() },
            E::ThreatCarriedOut { speaker: a.clone(), target: c.clone() },
            E::ThreatBluffed { speaker: a.clone(), target: c.clone() },
            E::AntiThirdPartyDeal { payer: a.clone(), attacker: b.clone(), target: c.clone() },
            // A neutral owner: no row, nothing happens.
            E::FairTransaction { a: a.clone(), b: p("neutral") },
            // One seat twice: the error.
            E::FairTransaction { a: a.clone(), b: a.clone() },
        ]
    }

    /// The core writes the same state as the string rule, event by event, and the dual run finds no
    /// difference; the relationships decay and the initiation tally agree too.
    #[test]
    fn the_relationship_table_runs_in_lockstep_with_the_core() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual);
        with_mask(mask, || {
            for enabled in [true, false] {
                let mut state = table(enabled);
                for e in events() {
                    let mut by_core = state.clone();
                    let core = super::relationship_event(&mut by_core, &e);
                    let neutral = matches!(&e, E::FairTransaction { b, .. } if b.as_str() == "neutral");
                    assert_eq!(core.is_some(), !neutral, "{e:?}: the core half runs unless an owner has no seat");
                    let result = relations::apply_relationship_event(&mut state, &e);
                    if let Some(core) = core {
                        assert_eq!(core, result, "{e:?}");
                        assert_eq!(by_core, state, "{e:?}");
                    }
                }
                if enabled {
                    let r = state.diplomacy.relationship(&p("b"), &p("a"));
                    assert!(r.trust != 0 || r.threat != 0, "the table moved something: {r:?}");
                }
                let mut by_core = state.clone();
                assert!(super::decay_relationships(&mut by_core).is_some());
                relations::decay_relationships(&mut state);
                assert_eq!(by_core, state, "decay");
                for (x, y, first) in [("a", "b", true), ("a", "b", false), ("a", "a", false), ("b", "a", true)] {
                    let mut by_core = state.clone();
                    assert_eq!(super::consume_initiation(&mut by_core, &p(x), &p(y)), Some(first));
                    assert_eq!(relations::consume_initiation(&mut state, &p(x), &p(y)), first);
                    assert_eq!(by_core, state, "initiation {x} {y}");
                }
                let mut by_core = state.clone();
                assert!(super::clear_turn_initiations(&mut by_core).is_some());
                relations::clear_turn_initiations(&mut state);
                assert_eq!(by_core, state, "clear");
            }
        });
        let r = report();
        let counts = &r.areas[&Area::TradeDiplomacy];
        assert!(counts.calls > 20);
        assert!(r.differences.is_empty(), "{:?}", r.differences);
    }
}
