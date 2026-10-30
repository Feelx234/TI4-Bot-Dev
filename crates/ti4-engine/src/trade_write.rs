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
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::rules::diplomacy as rules;
    use ti4_engine_core::rules::trade as rules_notes;
    use ti4_engine_core::state::{CoreState, NoteKey, Slice};
    use ti4_model::diplomacy::SignalKind;
    use ti4_model::state::GameState;
    use ti4_model::{DiplomacyError, PlayerId};

    use crate::core_adapter::typed;
    use crate::generated::IdStr;
    use crate::core_sync::keys::text::parse_note;
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

    /// `GameState::clear_transactions`.
    pub fn clear_turn_transactions(gs: &mut GameState) -> Option<()> {
        trade(gs, |core| ti4_engine_core::rules::trade::clear_turn_transactions(core))
    }

    /// `negotiations_this_action.clear()`.
    pub fn clear_action_negotiations(gs: &mut GameState) -> Option<()> {
        trade(gs, |core| ti4_engine_core::rules::trade::clear_action_negotiations(core))
    }

    /// The round reset of the trade tallies.
    pub fn reset_round(gs: &mut GameState) -> Option<()> {
        trade(gs, |core| ti4_engine_core::rules::trade::reset_round(core))
    }

    fn trade<T>(gs: &mut GameState, f: impl FnOnce(&mut CoreState) -> T) -> Option<T> {
        let mut cx = Ctx::of(gs).ok()?;
        with_core(gs, &mut cx, Sources::default(), Footprint::new(&[Slice::Trade], &[Slice::Trade]), |core, _| f(core)).ok()
    }

    /// `DiplomacyState::clear_turn_initiations`.
    pub fn clear_turn_initiations(gs: &mut GameState) -> Option<()> {
        lend(gs, |core| rules::clear_turn_initiations(core))
    }

    // -- promissory notes changing hands ------------------------------------------------------

    /// The notes: the table of holders and faceup notes (Cards), the Support loans (Trade), the points
    /// (Players, Ledger). Faction ids come from the seats.
    const NOTES: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger],
        &[Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger],
    );

    /// The typed key of a `alias:faction` note id.
    pub fn note_key(text: &str) -> Option<NoteKey> {
        let (alias, faction) = text.split_once(':')?;
        Some(NoteKey { note: parse_note(alias)?, faction: typed::<Faction>(faction)? })
    }

    fn notes<T>(gs: &mut GameState, f: impl FnOnce(&mut CoreState, &Ctx) -> Option<T>) -> Option<T> {
        let mut cx = Ctx::of(gs).ok()?;
        let seats = Ctx::of(gs).ok()?;
        with_core(gs, &mut cx, Sources::default(), NOTES, |core, _| f(core, &seats)).ok().flatten()
    }

    /// `promissory::take`.
    pub fn take_note(gs: &mut GameState, holder: &PlayerId, note: &str) -> Option<()> {
        let k = note_key(note)?;
        notes(gs, |core, cx| {
            rules_notes::take_note(core, cx.owners.seat(holder.as_str())?, k);
            Some(())
        })
    }

    /// `promissory::give_back`.
    pub fn give_back(gs: &mut GameState, note: &str) -> Option<()> {
        let k = note_key(note)?;
        notes(gs, |core, _| rules_notes::give_back(core, k))
    }

    /// `promissory::play_action_note`.
    pub fn play_action_note(gs: &mut GameState, player: &PlayerId, note: &str) -> Option<bool> {
        let k = note_key(note)?;
        notes(gs, |core, cx| Some(rules_notes::play_action_note(core, cx.owners.seat(player.as_str())?, k)))
    }

    /// `promissory::play_convoys`.
    pub fn play_convoys(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        notes(gs, |core, cx| Some(rules_notes::play_convoys(core, cx.owners.seat(player.as_str())?, |f| f.as_str())))
    }

    /// `promissory::receive` (a `support:<faction>` note).
    pub fn receive_support(gs: &mut GameState, holder: &PlayerId, note: &str) -> Option<bool> {
        let faction = typed::<Faction>(note.strip_prefix(crate::promissory::SUPPORT_PREFIX)?)?;
        notes(gs, |core, cx| rules_notes::receive_support(core, cx.owners.seat(holder.as_str())?, faction))
    }

    /// `promissory::return_support`.
    pub fn return_support(gs: &mut GameState, owner: &PlayerId) -> Option<bool> {
        notes(gs, |core, cx| Some(rules_notes::return_support(core, cx.owners.seat(owner.as_str())?)))
    }

    /// `promissory::trade_agreement_on_replenish` up to the staged event: the holder and the goods.
    pub fn trade_agreement(gs: &mut GameState, owner: &PlayerId) -> Option<Option<(PlayerId, i32)>> {
        let ids = Ctx::of(gs).ok()?.owners.clone();
        notes(gs, |core, cx| {
            let moved = rules_notes::trade_agreement(core, cx.owners.seat(owner.as_str())?)?;
            moved.map(|(holder, n)| ids.id(holder).map(|h| Some((PlayerId::new(h), n)))).unwrap_or(Some(None))
        })
    }

    /// [`NOTES`] and the board: the units that make an activation spend a note.
    const ACTIVATION: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger, Slice::Board],
        &[Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger],
    );

    /// `promissory::use_ceasefire`.
    pub fn use_ceasefire(gs: &mut GameState, mover: &PlayerId) -> Option<bool> {
        notes(gs, |core, cx| rules_notes::use_ceasefire(core, cx.owners.seat(mover.as_str())?))
    }

    /// `promissory::spend_support_on_activation`: the owners whose Support was spent, in the order of their ids.
    pub fn spend_on_activation(gs: &mut GameState, activator: &PlayerId, system: &ti4_model::id::SystemId) -> Option<Vec<PlayerId>> {
        let system = typed::<ti4_engine_core::generated::System>(system.as_str())?;
        let ids = Ctx::of(gs).ok()?.owners.clone();
        let mut cx = Ctx::of(gs).ok()?;
        let seats = Ctx::of(gs).ok()?;
        let owners = with_core(gs, &mut cx, Sources::default(), ACTIVATION, |core, _| {
            rules_notes::spend_support_on_activation(core, seats.owners.seat(activator.as_str())?, system)
        })
        .ok()
        .flatten()?;
        let mut out: Vec<PlayerId> = owners.as_slice().iter().map(|s| ids.id(*s).map(PlayerId::new)).collect::<Option<_>>()?;
        out.sort();
        Some(out)
    }

    /// `promissory::return_all_foreign`.
    pub fn return_all_foreign(gs: &mut GameState, alias: &str) -> Option<()> {
        let note = parse_note(alias)?;
        notes(gs, |core, _| rules_notes::return_all_foreign(core, note))
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
    pub fn clear_turn_transactions(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn clear_action_negotiations(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn reset_round(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn take_note(_: &mut GameState, _: &PlayerId, _: &str) -> Option<()> {
        None
    }
    pub fn give_back(_: &mut GameState, _: &str) -> Option<()> {
        None
    }
    pub fn play_action_note(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn play_convoys(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn receive_support(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn return_support(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn return_all_foreign(_: &mut GameState, _: &str) -> Option<()> {
        None
    }
    pub fn use_ceasefire(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn spend_on_activation(_: &mut GameState, _: &PlayerId, _: &ti4_model::id::SystemId) -> Option<Vec<PlayerId>> {
        None
    }
    pub fn trade_agreement(_: &mut GameState, _: &PlayerId) -> Option<Option<(PlayerId, i32)>> {
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

    /// Every promissory write agrees with the core, state for state: a note taken (faceup or in hand), sent
    /// home, played from the hand, Trade Convoys, a Support lent (its point and ledger row) and returned,
    /// every foreign copy returned. A faction played by two seats is a bail.
    #[test]
    fn the_promissory_writes_run_in_lockstep_with_the_core() {
        use crate::promissory as pn;
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual);
        with_mask(mask, || {
            let mut state = table(false);
            pn::deal(&mut state, ti4_content::ContentStore::embedded(), POK);
            let (a, b) = (p("a"), p("b"));
            let notes: Vec<String> = state.promissory_notes.keys().cloned().collect();
            assert!(notes.len() > 10, "dealt: {notes:?}");
            let mut core_runs = 0;
            let mut check = |name: &str, run: &dyn Fn(&mut GameState, bool) -> bool| {
                let mut s = state.clone();
                let mut c = state.clone();
                let from_core = run(&mut c, true);
                run(&mut s, false);
                if from_core {
                    core_runs += 1;
                    assert_eq!(c, s, "{name}");
                }
            };
            for note in &notes {
                check(&format!("take {note}"), &|s, core| {
                    if core {
                        super::take_note(s, &a, note).is_some()
                    } else {
                        pn::take(s, ti4_content::ContentStore::embedded(), &a, note);
                        true
                    }
                });
                check(&format!("give_back {note}"), &|s, core| {
                    // A note first lent to a, then sent home.
                    pn::take(s, ti4_content::ContentStore::embedded(), &a, note);
                    if core {
                        super::give_back(s, note).is_some()
                    } else {
                        pn::give_back(s, note);
                        true
                    }
                });
                check(&format!("play_action_note {note}"), &|s, core| {
                    pn::take(s, ti4_content::ContentStore::embedded(), &a, note);
                    if core {
                        super::play_action_note(s, &a, note).is_some()
                    } else {
                        pn::play_action_note(s, &a, note);
                        true
                    }
                });
            }
            for owner in ["sol", "hacan", "letnev"] {
                let support = pn::support(owner);
                check(&format!("support {owner}"), &|s, core| {
                    let lend = |s: &mut GameState| {
                        let _ = pn::receive(s, &b, &support);
                    };
                    if core {
                        let ok = super::receive_support(s, &b, &support).is_some();
                        let id = s.players.iter().find(|x| x.faction.as_str() == owner).map(|x| x.id.clone()).unwrap();
                        ok && super::return_support(s, &id).is_some()
                    } else {
                        lend(s);
                        let id = s.players.iter().find(|x| x.faction.as_str() == owner).map(|x| x.id.clone()).unwrap();
                        pn::return_support(s, &id);
                        true
                    }
                });
            }
            for alias in ["convoys", "ceasefire", "ta", "cf"] {
                check(&format!("foreign {alias}"), &|s, core| {
                    for note in s.promissory_notes.clone().keys().filter(|n| pn::alias_of(n) == alias) {
                        pn::take(s, ti4_content::ContentStore::embedded(), &a, note);
                    }
                    if core {
                        let ok = super::return_all_foreign(s, alias).is_some();
                        ok && super::play_convoys(s, &a).is_some()
                    } else {
                        pn::return_all_foreign(s, alias);
                        pn::play_convoys(s, &a);
                        true
                    }
                });
            }
            for (owner_id, owner_faction, holds, commodities) in [("a", "sol", "b", 3), ("b", "hacan", "a", 2), ("a", "sol", "a", 3), ("a", "sol", "b", 0)] {
                check(&format!("trade agreement {owner_faction} held by {holds}"), &|s, core| {
                    let owner = p(owner_id);
                    s.player_mut(&owner).unwrap().commodities = commodities;
                    s.promissory_notes.insert(pn::note_id("ta", owner_faction), p(holds));
                    if core {
                        let moved = super::trade_agreement(s, &owner);
                        let Some(moved) = moved else { return false };
                        let expect = (holds != owner_id && commodities > 0).then(|| (p(holds), commodities));
                        assert_eq!(moved, expect);
                        true
                    } else {
                        let _ = pn::trade_agreement_on_replenish(s, &owner);
                        true
                    }
                });
            }
            assert!(core_runs > 30, "the core half ran {core_runs} times");
        });
        let r = report();
        assert!(r.differences.is_empty(), "{:?}", r.differences);
    }

    /// The per-turn, per-action and per-round records clear the same on the core: the tallies are filled
    /// first (a transaction, two negotiations, a resolved deal) so a clear that forgets one shows.
    #[test]
    fn the_trade_records_clear_in_lockstep_with_the_core() {
        use crate::transactions as tx;
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual);
        with_mask(mask, || {
            let (a, b) = (p("a"), p("b"));
            let filled = || {
                let mut s = table(false);
                tx::record_transaction(&mut s, &a, &b);
                tx::note_negotiation(&mut s, &a);
                tx::note_negotiation(&mut s, &b);
                s.transactions_this_round.push((a.clone(), b.clone()));
                s
            };
            for (name, string, core) in [
                ("turn", tx::clear_turn_transactions as fn(&mut GameState), super::clear_turn_transactions as fn(&mut GameState) -> Option<()>),
                ("action", tx::clear_action_negotiations, super::clear_action_negotiations),
                ("round", tx::reset_round, super::reset_round),
            ] {
                let (mut s, mut c) = (filled(), filled());
                assert!(core(&mut c).is_some(), "{name}: the core half runs");
                string(&mut s);
                assert_eq!(c, s, "{name}");
                assert!(s.transactions_this_round.is_empty() == (name == "round"), "{name}");
                assert!((name == "turn") == s.transactions_this_turn.is_empty() || name == "round", "{name}");
                assert!(name == "turn" || s.negotiations_this_action.is_empty(), "{name}");
            }
        });
        assert!(report().differences.is_empty());
    }

    /// Every string-only case answers `None` and leaves the state exactly as it was, so the string write
    /// that follows starts from an untouched position: a note id the tables do not know, a faction played
    /// by two seats (the owner of a note, of a Support, of a Trade Agreement), a planet card in a deal and
    /// an owner without a seat (the neutral units).
    #[test]
    fn every_bail_leaves_the_state_to_the_string_engine() {
        use crate::promissory as pn;
        use crate::transactions::{Offer, PlanetTerm, Terms};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual);
        with_mask(mask, || {
            let (a, b, c) = (p("a"), p("b"), p("c"));
            let mut unique = table(true);
            pn::deal(&mut unique, ti4_content::ContentStore::embedded(), POK);
            let before = unique.clone();
            // An id the tables do not know.
            assert_eq!(super::take_note(&mut unique, &a, "nosuchnote:sol"), None);
            assert_eq!(super::take_note(&mut unique, &a, "cf:nosuchfaction"), None);
            assert_eq!(super::take_note(&mut unique, &a, "nocolon"), None);
            assert_eq!(super::receive_support(&mut unique, &a, "support:nosuchfaction"), None);
            assert_eq!(super::receive_support(&mut unique, &a, "cf:sol"), None);
            assert_eq!(super::return_all_foreign(&mut unique, "nosuchnote"), None);
            assert_eq!(unique, before);

            // Two seats of one faction: no unique owner.
            let mut twins = crate::fixtures::seated_game(&[("a", "sol"), ("b", "sol"), ("c", "hacan")], POK);
            pn::deal(&mut twins, ti4_content::ContentStore::embedded(), POK);
            twins.promissory_notes.insert(pn::note_id("ta", "sol"), c.clone());
            twins.player_mut(&a).unwrap().commodities = 2;
            let before = twins.clone();
            assert_eq!(super::give_back(&mut twins, &pn::note_id("ta", "sol")), None);
            assert_eq!(super::receive_support(&mut twins, &c, &pn::support("sol")), None);
            assert_eq!(super::trade_agreement(&mut twins, &a), None);
            assert_eq!(twins, before, "a bail writes nothing");

            // A planet card in a deal stays on the string engine.
            #[cfg(feature = "typed-core")]
            {
                let planet = Terms {
                    planet: Some(PlanetTerm { planet: ti4_model::id::PlanetId::new("jord"), moves_to: ti4_model::id::PlanetId::new("jord") }),
                    ..Terms::default()
                };
                let offer = Offer { proposer: a.clone(), partner: b.clone(), given: planet, received: Terms::default() };
                let mut s = table(false);
                let before = s.clone();
                assert_eq!(crate::trade_core::exchange(&mut s, ti4_content::ContentStore::embedded(), &offer), None);
                assert_eq!(s, before);
            }

            // An owner without a seat is the string engine's (the row check needs the matrix).
            let mut s = table(true);
            let before = s.clone();
            let neutral = E::FairTransaction { a: a.clone(), b: p("neutral") };
            assert!(super::relationship_event(&mut s, &neutral).is_none());
            assert_eq!(s, before);
        });
        assert!(report().differences.is_empty());
    }
}
