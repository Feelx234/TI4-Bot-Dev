//! nomad: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `cards.transaction_reach` | `tcs_reach` | Q | `cards.transaction_reach` (no seam: a pair predicate on a legacy slot) |
//! | `combat.grants_sustain` | `nomad_agents::grants_sustain` | Q | seam `grants_sustain` (the allow side of `may_sustain`, added by CE) |
//! | `combat.borrowed_stats` | `nomad_agents::borrowed_stats` | Q | seam `borrowed_stats` (P4, `Step::BorrowedStats`, nothing to perform) |
//! | `direct.nomad_watches_agent_exhaustion` | `watches_agent_exhaustion` | Q | `direct` |
//! | `direct.nomad_agents_watches_roll_step` | `nomad_agents::watches_roll_step` | Q (the plan said W; the function is a state read) | `direct` |
//! | movement `unit_adjacent_systems`, `ignores_command_tokens` | | Q | `movement.rs` (A2, done) |
//! | `direct.nomad_agents_agent_readied` | `agent_readied` | O | `observe.rs` (blocked: see there) |
//! | `timing_abilities`, `commander_unlocked`, `leader_action`, `use_leader` | | W | B7 (leaders) |
//!
//! Known gap: the string `BorrowedStats.unit` is the piece as it stands (the damaged one of its type when the type
//! cannot sustain by itself); the core `BorrowedStats.unit` is a `Unit` kind, so this port answers the kind and the
//! window picks the piece.

use ti4_engine_core::generated::{Faction, Leader, PromissoryNote, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::hooks::{BorrowedStats, CombatCtx, CombatUnit, HookTable};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::seam::{Q, Replacement, SeatSystemSite, Source, Step, Verdict};
use ti4_engine_core::kernel::{Kernel, Next};
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::{CoreState, LeaderStatus, NoteKey};

use crate::common::{is_faction, mark, player, rec, scope, system, unit};

const SOURCE: Source = Source::Faction(Faction::TheNomad);
pub(crate) const CAVALRY: NoteKey = NoteKey { note: PromissoryNote::TheCavalry, faction: Faction::TheNomad };

/// Only the lent stats of The Cavalry (`borrowed_stats`), for a kernel that registers the other slots another way.
pub fn register_borrowed(t: &mut HookTable) {
    t.seam.borrowed_stats.push(Replacement { source: SOURCE, step: Step::BorrowedStats, offer: borrowed_offer, perform: |_, _, _| Next::Done });
}

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.general.leader_action.push(leader_action);
    t.cards.transaction_reach.push(transaction_reach);
    t.seam.grants_sustain.push(SOURCE, grants_sustain_rule);
    register_borrowed(t);
    t.direct.nomad_watches_agent_exhaustion.push(watches_agent_exhaustion);
    t.direct.nomad_agents_watches_roll_step.push(watches_roll_step);
}

// -- Temporal Command Suite ---------------------------------------------------------------------

/// The transaction Temporal Command Suite allows reaches its partner without adjacency while it is being negotiated.
pub fn transaction_reach(s: &CoreState, _: &Kernel, a: Seat, b: Seat) -> bool {
    mark(s, MarkKey::NomadTcsNegotiating(a, b)).is_some() || mark(s, MarkKey::NomadTcsNegotiating(b, a)).is_some()
}

/// Whether the engine should announce `AGENT_EXHAUSTED` events at all: a seated Nomad that holds Temporal Command Suite.
pub fn watches_agent_exhaustion(s: &CoreState) -> bool {
    s.players().iter().any(|p| p.faction == Faction::TheNomad && p.tech.owned.contains(Technology::TemporalCommandSuite))
}

/// Whether a combat's "Roll Dice" step end has anyone to tell: some seat holds a readied The Thundarian, or a seat
/// holding a readied Ssruu could borrow a seated Nomad's agent.
pub fn watches_roll_step(s: &CoreState) -> bool {
    let holds = |p: &ti4_engine_core::state::Player, status: &[LeaderStatus]| p.leaders.status.get(&Leader::TheThundarian).is_some_and(|h| status.contains(h));
    s.players().iter().any(|p| holds(p, &[LeaderStatus::Readied]))
        || (s.players().iter().any(|p| p.leaders.status.get(&Leader::Ssruu) == Some(&LeaderStatus::Readied))
            && s.players().iter().any(|p| holds(p, &[LeaderStatus::Readied, LeaderStatus::Exhausted])))
}

// -- Quantum Manipulator and The Cavalry ----------------------------------------------------------

/// The ship type `who` has chosen for The Cavalry in the combat in `at`, while the note is still in play with them.
pub(crate) fn cavalry_ship_type(s: &CoreState, who: Seat, at: System) -> Option<Unit> {
    let fields = rec(mark(s, MarkKey::NomadCavalry(who))?)?;
    let [marked, kind] = fields else { return None };
    let held = s.cards().promissory.get(&CAVALRY) == Some(&who) && s.cards().promissory_faceup.contains_key(&CAVALRY);
    (system(marked) == Some(at) && held).then(|| unit(kind)).flatten()
}

fn grants_sustain_rule(q: Q<'_>, u: &CombatUnit) -> Verdict {
    if grants_sustain(q.s, q.k, *u) { Verdict::Allow } else { Verdict::Abstain }
}

/// Quantum Manipulator and The Cavalry give SUSTAIN DAMAGE against a hit in a space combat. The mech may use its own
/// "while in a space area during combat"; The Cavalry's ship has it while it is undamaged (ships of one type are
/// interchangeable: once one of the type is damaged, no other ship of that type can sustain).
pub fn grants_sustain(s: &CoreState, _: &Kernel, u: CombatUnit) -> bool {
    if u.ctx != CombatCtx::Space {
        return false;
    }
    if u.unit == Unit::QuantumManipulator {
        return true;
    }
    let Some(sys) = u.system else { return false };
    cavalry_ship_type(s, u.owner, sys) == Some(u.unit)
        && !s.board().sys(sys).units.as_slice().iter().any(|p| p.owner() == u.owner && p.kind() == u.unit && p.sustained())
}

/// The Nomad's own flagship, upgrade included (`action_cards::placed_unit_id(.., "flagship")`).
fn nomad_flagship(s: &CoreState, k: &Kernel) -> Option<Unit> {
    let nomad = s.seats().seating_order().iter().copied().find(|seat| is_faction(s, *seat, Faction::TheNomad))?;
    let held = player(s, nomad)?;
    let kind = if held.tech.owned.contains(Technology::MemoriaII) { Unit::MemoriaII } else { Unit::MemoriaI };
    Some(k.hooks.strategy.unit_form_override(s, k, nomad, UnitBaseType::Flagship, kind).unwrap_or(kind))
}

fn borrowed_offer(q: Q<'_>, site: &SeatSystemSite) -> Option<BorrowedStats> {
    borrowed_stats(q.s, q.k, site.who, site.system)
}

/// The Cavalry: the Nomad flagship's combat value and ANTI-FIGHTER BARRAGE for the chosen ship. Only the combat *value*
/// is lent (the ship keeps its own number of dice). The card lends the barrage *value* only: a borrower that already
/// has a barrage keeps its own number of dice; one with none fires the flagship's number of dice. `None` when the ship
/// type is not in the space area. The barrage is `(hits_on, dice)`, as `BorrowedStats` has it.
pub fn borrowed_stats(s: &CoreState, k: &Kernel, who: Seat, at: System) -> Option<BorrowedStats> {
    let kind = cavalry_ship_type(s, who, at)?;
    let flagship = scope(k).unit(nomad_flagship(s, k)?)?;
    let own = scope(k).unit(kind)?;
    let own_barrage = own.anti_fighter_barrage.filter(|r| r.hits_on > 0);
    let flagship_barrage = flagship.anti_fighter_barrage.filter(|r| r.hits_on > 0);
    let dice = if own_barrage.is_some() { own.anti_fighter_barrage.map_or(0, |r| r.dice) } else { flagship.anti_fighter_barrage.map_or(0, |r| r.dice) };
    // The borrowing ship: the damaged one of its type when the type cannot sustain by itself, else the first in board order.
    let ships = || s.board().sys(at).units.as_slice().iter().filter(|p| p.owner() == who && p.kind() == kind);
    let piece = ships().find(|p| !own.sustain_damage && p.sustained()).or_else(|| ships().next())?;
    Some(BorrowedStats {
        unit: kind,
        sustained: piece.sustained(),
        galvanized: piece.galvanized(),
        hits_on: flagship.combat.filter(|r| r.hits_on > 0).map(|r| r.hits_on as i8),
        barrage: flagship_barrage.map(|r| (r.hits_on as i8, dice as i8)),
    })
}

// -- leader hooks (package F2) ----------------------------------------

/// Navarch Feng, unlock: "Have 1 scored secret objective."
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::NavarchFeng).then(|| s.objectives().scored[usize::from(who.0)].secret.len() >= 1)
}

/// Ahk-Syl Siven: not yet used this round and the flagship (Memoria I or II) on the board (`nomad::leader_action`).
pub fn leader_action(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::AhkSylSiven).then(|| {
        let active = mark(s, MarkKey::NomadHero(who)).and_then(crate::common::num) == Some(s.meta().round);
        !active && ti4_engine_core::query::leader_reads::space_pieces(s, who).any(|(_, p)| matches!(p.kind(), Unit::MemoriaI | Unit::MemoriaII))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_movement::testkit::table;
    use ti4_engine_core::keys::{Atom, MarkVal};
    use ti4_engine_core::state::Piece;

    const NOMAD: Faction = Faction::TheNomad;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn cavalry(s: &mut CoreState, holder: Seat, at: System, kind: Unit) {
        s.marks_mut().tail.edit().insert(MarkKey::NomadCavalry(holder), MarkVal::Rec(vec![MarkVal::Atom(Atom::System(at)), MarkVal::Atom(Atom::Unit(kind))]));
        s.cards_mut().promissory.edit().insert(CAVALRY, holder);
        s.cards_mut().promissory_faceup.edit().insert(CAVALRY, ());
    }

    fn site(owner: Seat, at: System, kind: Unit, ctx: CombatCtx) -> CombatUnit {
        CombatUnit { owner, system: Some(at), planet: None, unit: kind, ctx }
    }

    #[test]
    fn the_mech_sustains_in_space_only() {
        let (s, k) = table(&[NOMAD, SOL]);
        let at = ti4_engine_core::ids::system_at(30);
        assert!(grants_sustain(&s, &k, site(Seat(0), at, Unit::QuantumManipulator, CombatCtx::Space)));
        assert!(!grants_sustain(&s, &k, site(Seat(0), at, Unit::QuantumManipulator, CombatCtx::Ground)));
        assert!(!grants_sustain(&s, &k, site(Seat(0), at, Unit::CruiserI, CombatCtx::Space)));
    }

    #[test]
    fn the_cavalry_ship_sustains_until_one_of_its_type_is_damaged() {
        let (mut s, k) = table(&[NOMAD, SOL]);
        let at = ti4_engine_core::ids::system_at(30);
        s.board_mut().sys_mut(at).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        cavalry(&mut s, Seat(1), at, Unit::CruiserI);
        assert!(grants_sustain(&s, &k, site(Seat(1), at, Unit::CruiserI, CombatCtx::Space)));
        assert!(!grants_sustain(&s, &k, site(Seat(1), at, Unit::DestroyerI, CombatCtx::Space)), "another type");
        s.board_mut().sys_mut(at).units.push(Piece::new(Unit::CruiserI, Seat(1)).with_sustained(true));
        assert!(!grants_sustain(&s, &k, site(Seat(1), at, Unit::CruiserI, CombatCtx::Space)), "one of the type is damaged");
    }

    #[test]
    fn the_cavalry_lends_the_flagships_combat_value_and_the_barrage_value() {
        let (mut s, k) = table(&[NOMAD, SOL]);
        let at = ti4_engine_core::ids::system_at(30);
        assert!(borrowed_stats(&s, &k, Seat(1), at).is_none(), "no card");
        s.board_mut().sys_mut(at).units.push(Piece::new(Unit::DestroyerI, Seat(1)));
        cavalry(&mut s, Seat(1), at, Unit::DestroyerI);
        let flagship = Unit::MemoriaI.data();
        let got = borrowed_stats(&s, &k, Seat(1), at).expect("lends");
        assert_eq!(got.unit, Unit::DestroyerI);
        assert_eq!(got.hits_on, flagship.combat.map(|r| r.hits_on as i8));
        assert_eq!(got.barrage, flagship.anti_fighter_barrage.map(|r| (r.hits_on as i8, Unit::DestroyerI.data().anti_fighter_barrage.unwrap().dice as i8)));
        s.players_mut().edit(0).tech.owned.insert(Technology::MemoriaII);
        assert_eq!(borrowed_stats(&s, &k, Seat(1), at).unwrap().hits_on, Unit::MemoriaII.data().combat.map(|r| r.hits_on as i8), "the upgrade");
    }

    #[test]
    fn tcs_reach_is_symmetric() {
        let (mut s, k) = table(&[NOMAD, SOL]);
        assert!(!transaction_reach(&s, &k, Seat(0), Seat(1)));
        s.marks_mut().tail.edit().insert(MarkKey::NomadTcsNegotiating(Seat(0), Seat(1)), MarkVal::Atom(Atom::Num(1)));
        assert!(transaction_reach(&s, &k, Seat(1), Seat(0)));
    }

    #[test]
    fn the_watches_follow_the_tech_and_the_agents() {
        let (mut s, _) = table(&[NOMAD, Faction::TheYssarilTribes]);
        assert!(!watches_agent_exhaustion(&s));
        s.players_mut().edit(0).tech.owned.insert(Technology::TemporalCommandSuite);
        assert!(watches_agent_exhaustion(&s));
        assert!(!watches_roll_step(&s));
        s.players_mut().edit(0).leaders.status.insert(Leader::TheThundarian, LeaderStatus::Readied);
        assert!(watches_roll_step(&s));
        s.players_mut().edit(0).leaders.status.insert(Leader::TheThundarian, LeaderStatus::Exhausted);
        assert!(!watches_roll_step(&s));
        s.players_mut().edit(1).leaders.status.insert(Leader::Ssruu, LeaderStatus::Readied);
        assert!(watches_roll_step(&s), "Ssruu could borrow the exhausted agent");
    }

    #[test]
    fn registering_fills_the_slots() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!((t.seam.grants_sustain.len(), t.seam.borrowed_stats.len()), (1, 1));
        assert_eq!(t.registered(), 7, "three seam providers and two direct slots, the legacy slot of cards.transaction_reach, the two leader hooks");
        assert_eq!((t.general.commander_unlocked.len(), t.general.leader_action.len()), (1, 1));
    }
}
