//! sardakk: the timing abilities of Sh'val, Harbinger (the hero), docs/ENGINE_REACTIONS.md (package B12).
//!
//! The skip itself ("skip directly to the Commit Ground Forces step", asked after the movement) is the string's offer and
//! writes the mark `sardakk:hero:skip` = `{player}|{system}`. One mandatory ability of every seat reads it:
//!
//! * `leader:*:sardakkhero:GROUND_COMMITMENT_FINISHED:after`: once the commitment of that player in that system is over the
//!   mark is removed, the card is purged and the player's ships in the space area of the system return to the reinforcements.
//!
//! The clean-up of a skip that never reached its commitment (`leader:*:sardakkhero_turn:TURN_BEGAN:after`) is not ported: no
//! followed window announces `TURN_BEGAN`, so nothing could run or test it.
//!
//! Not ported (the window bails, test `bail_every_family_that_stays_in_the_string_is_named`): the mech's hit (an
//! assignment of hits inside the ability), Exotrireme II, Tekklar Legion, the agent and Supremacy.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Leader;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, Key};

/// Sh'val, the finish (`leader:*:sardakkhero:GROUND_COMMITMENT_FINISHED:after`).
pub const HERO_FINISH: Slot = Slot(12001);

const HERO: Leader = Leader::ShvalHarbinger;

/// The player and system of the skip under way.
fn skip(s: &CoreState) -> Option<(Seat, ti4_engine_core::generated::System)> {
    match s.marks().tail.get(&MarkKey::SardakkHeroSkip()) {
        Some(MarkVal::Rec(v)) => match v.as_slice() {
            [MarkVal::Atom(Atom::Seat(who)), MarkVal::Atom(Atom::System(system))] => Some((*who, *system)),
            _ => None,
        },
        _ => None,
    }
}

fn finish_when(c: &CondCx<'_>) -> bool {
    let p = &c.event.payload;
    skip(c.state).is_some_and(|(who, system)| who == c.def.owner && p.system(Key::System) == Some(system)) && p.seat(Key::Player) == Some(c.def.owner)
}

fn finish(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    eff.state().marks_mut().tail.edit().remove(&MarkKey::SardakkHeroSkip());
    ti4_engine_core::rules::leaders::purge(eff.state(), me, HERO);
    // The ships of the space area go back to the reinforcements (the board is the supply's count).
    let kept: Vec<_> = eff.state().board().sys(system).units.as_slice().iter().copied().filter(|p| !(p.owner() == me && p.kind().data().ship)).collect();
    eff.state().board_mut().sys_mut(system).units.clear();
    for piece in kept {
        eff.state().board_mut().sys_mut(system).units.push(piece);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(HERO_FINISH, AbilityKind::Leader, EventKind::GroundCommitmentFinished, Relation::After, finish).when(finish_when));
}
