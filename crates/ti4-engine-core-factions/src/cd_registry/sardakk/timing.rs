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
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload, Piece};

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

/// The commitment the payload names is the one of the skip: the player and the system of the mark.
fn finishing(s: &CoreState, owner: Seat, p: &Payload) -> bool {
    skip(s).is_some_and(|(who, system)| who == owner && p.system(Key::System) == Some(system)) && p.seat(Key::Player) == Some(owner)
}

fn finish_when(c: &CondCx<'_>) -> bool {
    finishing(c.state, c.def.owner, &c.event.payload)
}

/// What stays in the space area when `me`'s ships go back to the reinforcements.
fn without_ships_of(units: &[Piece], me: Seat) -> Vec<Piece> {
    units.iter().copied().filter(|p| !(p.owner() == me && p.kind().data().ship)).collect()
}

fn finish(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    eff.state().marks_mut().tail.edit().remove(&MarkKey::SardakkHeroSkip());
    ti4_engine_core::rules::leaders::purge(eff.state(), me, HERO);
    // The ships of the space area go back to the reinforcements (the board is the supply's count).
    let kept = without_ships_of(eff.state().board().sys(system).units.as_slice(), me);
    eff.state().board_mut().sys_mut(system).units.clear();
    for piece in kept {
        eff.state().board_mut().sys_mut(system).units.push(piece);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(HERO_FINISH, AbilityKind::Leader, EventKind::GroundCommitmentFinished, Relation::After, finish).when(finish_when));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::{Faction, System, Unit};
    use ti4_engine_core::state::Val;

    #[test]
    fn only_the_owners_ships_of_the_space_area_go_back() {
        let units = [Piece::new(Unit::CruiserI, Seat(0)), Piece::new(Unit::CruiserI, Seat(1)), Piece::new(Unit::InfantryI, Seat(0)), Piece::new(Unit::FighterI, Seat(0))];
        assert_eq!(without_ships_of(&units, Seat(0)), vec![units[1], units[2]], "another player's ship and the owner's ground force stay");
    }

    #[test]
    fn the_finish_is_the_one_of_the_skip() {
        let (mut s, _) = table(&[Faction::SardakkNorr, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        let other = System::ALL[1];
        s.marks_mut().tail.edit().insert(MarkKey::SardakkHeroSkip(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Seat(Seat(0))), MarkVal::Atom(Atom::System(system))]));
        let p = |who: Seat, at: System| Payload::new().with(Key::Player, Val::Seat(who)).with(Key::System, Val::System(at));
        assert!(finishing(&s, Seat(0), &p(Seat(0), system)));
        assert!(!finishing(&s, Seat(0), &p(Seat(0), other)), "another system");
        assert!(!finishing(&s, Seat(0), &p(Seat(1), system)), "another player commits");
        assert!(!finishing(&s, Seat(1), &p(Seat(1), system)), "the mark is the other seat's");
        s.marks_mut().tail.edit().remove(&MarkKey::SardakkHeroSkip());
        assert!(!finishing(&s, Seat(0), &p(Seat(0), system)), "no skip under way");
    }
}
