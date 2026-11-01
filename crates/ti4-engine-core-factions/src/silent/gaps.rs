//! The LOUD gaps of the live path: string abilities the core table has no rule for yet, in scope, each a row here. A native game registers a row for every
//! seat (`register_live`), and when the seat holds the card and its event is announced the condition answers `Unknown::AbilityGap(slot)`: the live step stops
//! with a counted bail instead of playing on without the ability (the dual does not register them: the string runs next to the core there, and
//! `Recording::unported` already bails a window that finds one eligible).
//!
//! The predicates are coarse on purpose (the seat plays the faction, holds the note or the technology): a gap that fires too often is a reason to port the
//! ability, a gap that never fires is a silent wrong game. `ti4-engine`'s `ability_audit` pins the list against what the string registers.
//! A row's index is its slot (`FIRST_SLOT + index`) and the position of its name in the engine's table.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, LeaderType, PromissoryNote, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{no_effect, AbilityTable, CondCx, Slot, Spec, Unknown};
use ti4_engine_core::state::{EventKind, LeaderStatus};

/// The slot of the first row.
pub const FIRST_SLOT: u16 = 41_000;

/// One gap: the registered ability's kind, event and relation, and who could use it.
pub struct Gap {
    pub kind: AbilityKind,
    pub event: EventKind,
    pub relation: Relation,
    pub holds: fn(&CondCx<'_>) -> bool,
}

#[allow(dead_code, reason = "no loud gap row is left (integration 13); a lane that lists one uses the predicates again")]
fn faction(c: &CondCx<'_>, f: Faction) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.faction == f)
}

/// The seat plays `f` and its leader of this kind can act: an agent is readied, a commander or a hero is unlocked.
#[allow(dead_code, reason = "no loud gap row is left (integration 13); a lane that lists one uses the predicates again")]
fn leader(c: &CondCx<'_>, f: Faction, kind: LeaderType) -> bool {
    faction(c, f)
        && c.state.players().by_seat(c.def.owner).is_some_and(|p| {
            p.leaders.status.iter().any(|(l, st)| {
                l.data().kind == kind
                    && match kind {
                        LeaderType::Agent => *st == LeaderStatus::Readied,
                        _ => matches!(*st, LeaderStatus::Unlocked | LeaderStatus::Readied),
                    }
            })
        })
}

/// A rule that only follows another loud gap (a clear, a return, the end of a hero's turn): the upstream ability stops the step first, so this one is
/// never reached on its own.
#[allow(dead_code, reason = "no loud gap row is left (integration 13); a lane that lists one uses the predicates again")]
fn downstream(_: &CondCx<'_>) -> bool {
    false
}

#[allow(dead_code, reason = "no loud gap row is left (integration 13); a lane that lists one uses the predicates again")]
fn tech(c: &CondCx<'_>, t: Technology) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.tech.owned.contains(t))
}

/// The seat holds the note of another faction (a note at home is nobody's to play).
#[allow(dead_code, reason = "no loud gap row is left (integration 13); a lane that lists one uses the predicates again")]
fn note(c: &CondCx<'_>, n: PromissoryNote) -> bool {
    let me: Seat = c.def.owner;
    c.state.players().by_seat(me).is_some_and(|p| c.state.cards().promissory.iter().any(|(key, holder)| key.note == n && *holder == me && key.faction != p.faction))
}

/// The rows, in the order of the engine's name table (`ability_audit::GAP_NAMES`).
pub const GAPS: &[Gap] = &[
    // -- abilities
    // -- leaders
    // -- promissory notes (held by any seat)
    // -- technologies
    // -- units
    // -- ported by no lane yet
    // -- the copies Ssruu makes of an agent of a seated faction
    // -- the activation abilities lane M6c lists as gaps (its audit); delete the row when the lane ports the rule
];

/// The condition of every row: the seat could use the ability, and the core cannot say whether it is eligible now.
fn gap_condition(c: &CondCx<'_>) -> bool {
    let Some(index) = usize::try_from(c.def.arg).ok().and_then(|arg| arg.checked_sub(usize::from(FIRST_SLOT))) else { return false };
    GAPS.get(index).is_some_and(|gap| (gap.holds)(c)) && c.unknown(Unknown::AbilityGap(Slot(FIRST_SLOT + u16::try_from(index).unwrap_or(0))))
}

/// The slot of row `index`.
#[must_use]
pub const fn slot(index: usize) -> Slot {
    Slot(FIRST_SLOT + index as u16)
}

/// Register every row for every seat (the live path's table only).
pub fn register_live(t: &mut AbilityTable) {
    for (index, gap) in GAPS.iter().enumerate() {
        t.add(Spec::new(slot(index), gap.kind, gap.event, gap.relation, no_effect).when(gap_condition));
    }
}
