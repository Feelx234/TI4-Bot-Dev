//! `fx_ground`: the destruction of units on a planet by an effect, with the ground forces that fell staged for the
//! coordinator (package B6c; the string `action_cards::destroy_on_planet`). The agenda area's `clear_planet` is the
//! same body for an agenda's cause; areas do not import each other, so the action-card cards use this one.

use crate::containers::SmallList;
use crate::generated::{Planet, System, UnitBaseType};
use crate::keys::{Atom, DestroyReason, MarkKey, MarkVal, Word};
use crate::state::{CoreState, Piece, PlanetFlags, SysFlags};
use crate::tables::Scope;

/// The next `private:#staged:ground:` number: after the highest still staged.
fn next_ground(core: &CoreState) -> u32 {
    core.marks()
        .tail
        .iter()
        .filter_map(|(k, _)| if let MarkKey::PrivateStagedGround(n) = k { Some(*n) } else { None })
        .max()
        .map_or(0, |n| n + 1)
}

/// `hooks_ground::stage_ground_force_destroyed` for a piece the caller has removed. `Err` for a galvanized one (the
/// string row carries a `|g` tail the typed row cannot hold).
///
/// # Errors
/// The piece is galvanized.
pub fn stage_destroyed(core: &mut CoreState, system: System, planet: Planet, piece: Piece, reason: DestroyReason) -> Result<(), ()> {
    if piece.galvanized() {
        return Err(());
    }
    let key = MarkKey::PrivateStagedGround(next_ground(core));
    let row = MarkVal::Rec(vec![
        MarkVal::Atom(Atom::Word(Word::Destroyed)),
        MarkVal::Atom(Atom::System(system)),
        MarkVal::Atom(Atom::Planet(planet)),
        MarkVal::Atom(Atom::Seat(piece.owner())),
        MarkVal::Atom(Atom::Unit(piece.kind())),
        MarkVal::Atom(Atom::Word(if piece.sustained() { Word::True } else { Word::False })),
        MarkVal::Atom(Atom::Destroy(reason)),
    ]);
    core.marks_mut().tail.edit().insert(key, row);
    Ok(())
}

/// `action_cards::destroy_on_planet`: remove the first `limit` units of `base` from the planet (anyone's, in list
/// order), stage every ground force that fell, and return how many were destroyed. `Err` when a fallen piece is
/// galvanized (the position is not expressed; nothing is compared).
///
/// # Errors
/// A galvanized ground force fell.
pub fn destroy_on_planet(
    core: &mut CoreState,
    scope: Scope,
    system: System,
    planet: Planet,
    base: UnitBaseType,
    limit: Option<usize>,
    reason: DestroyReason,
) -> Result<usize, ()> {
    // `state.system_mut` creates the system's entry.
    core.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
    if !core.board().planet(planet).flags.has(PlanetFlags::UNITS_ENTRY) {
        return Ok(0);
    }
    let mut destroyed = 0;
    let mut fallen: SmallList<Piece, 40> = SmallList::new();
    core.board_mut().planet_mut(planet).units.retain(|p| {
        if limit.is_some_and(|cap| destroyed >= cap) {
            return true;
        }
        let d = p.kind().data();
        let hit = scope.0 & crate::tables::source_bit(d.source) != 0 && d.base_type == base;
        if hit {
            destroyed += 1;
            if matches!(d.base_type, UnitBaseType::Infantry | UnitBaseType::Mech) {
                fallen.push(*p);
            }
        }
        !hit
    });
    for piece in fallen.as_slice() {
        stage_destroyed(core, system, planet, *piece, reason)?;
    }
    Ok(destroyed)
}
