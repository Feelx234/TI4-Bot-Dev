//! cabal: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Reanimator, Devour (ships
//! and ground forces), The Terror Between (ships and ground forces) and the four participation marks Devour
//! reads. Not ported: The Stillness of Stars (an ask), Al'Raith Ix Ianovar, the Crucible.
//!
//! A destroyed model is captured by a row on the Cabal's sheet (`misc.captured_units`: owner, unit type); the
//! marks are `cabal:devour:{system}[|{planet}]` holding the seat that took part.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, EventRec, Key};
use ti4_engine_core::tables::Scope;

/// Reanimator (`unit:..:cabal_mech:GROUND_FORCE_DESTROYED:after`).
pub const REANIMATOR: Slot = Slot(3001);
/// Devour, ships (`ability:..:devour:SHIP_DESTROYED:after`).
pub const DEVOUR_SHIPS: Slot = Slot(3002);
/// Devour, ground forces (`ability:..:devour:GROUND_FORCE_DESTROYED:after`).
pub const DEVOUR_GROUND: Slot = Slot(3003);
/// The Terror Between, ships (`unit:..:cabal_flagship:SHIP_DESTROYED:after`).
pub const TERROR_SHIPS: Slot = Slot(3004);
/// The Terror Between, ground forces (`unit:..:cabal_flagship:GROUND_FORCE_DESTROYED:after`).
pub const TERROR_GROUND: Slot = Slot(3005);
/// The participation marks: space start (write), space end (clear), ground start, ground end.
pub const MARK_SPACE: Slot = Slot(3006);
pub const CLEAR_SPACE: Slot = Slot(3007);
pub const MARK_GROUND: Slot = Slot(3008);
pub const CLEAR_GROUND: Slot = Slot(3009);

const SCOPE: Scope = Scope(u32::MAX);
const CABAL: Faction = Faction::TheVuilraithCabal;

fn base(unit: Unit) -> Option<UnitBaseType> {
    SCOPE.unit(unit).map(|d| d.base_type)
}

/// One of the Cabal's flagships is in the system (`has_flagship_text_in`, a Cabal seat).
fn flagship_present(s: &CoreState, owner: Seat, system: System) -> bool {
    board::space_units(s, system).iter().any(|p| p.owner() == owner && p.kind() == Unit::TheTerrorBetween)
}

fn mark_key(system: System, planet: Option<Planet>) -> MarkKey {
    MarkKey::CabalDevour(system, planet)
}

fn mark_held(s: &CoreState, key: &MarkKey, who: Seat) -> bool {
    matches!(s.marks().tail.get(key), Some(MarkVal::Atom(Atom::Seat(holder))) if *holder == who)
}

/// The model a capture effect may take: a non-structure unit of a seated, non-neutral victim; `own` lets the
/// captor's own models count.
fn destroyed_model(event: &EventRec, captor: Seat, own: bool) -> Option<(Seat, Unit)> {
    let victim = event.payload.seat(Key::Player)?;
    if !own && victim == captor {
        return None;
    }
    let unit = event.payload.unit(Key::Unit)?;
    let data = SCOPE.unit(unit)?;
    (!data.structure).then_some((victim, unit))
}

fn capture(eff: &mut Eff<'_, '_>, victim: Seat, unit: Unit) {
    let me = eff.owner();
    if let Some(p) = eff.state().players_mut().by_seat_mut(me) {
        p.misc.captured_units.push((victim, unit));
    }
}

fn took_part(event: &EventRec, who: Seat) -> bool {
    event.payload.seat(Key::Attacker) == Some(who) || event.payload.seat(Key::Defender) == Some(who)
}

// -- Reanimator -------------------------------------------------------------------------------------

fn reanimator_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let p = &c.event.payload;
    let (Some(player), Some(unit), Some(system), Some(planet)) = (p.seat(Key::Player), p.unit(Key::Unit), p.system(Key::System), p.planet(Key::Planet)) else {
        return false;
    };
    player == me
        && base(unit) == Some(UnitBaseType::Infantry)
        && board::planet_units(c.state, planet).iter().any(|u| u.owner() == me && base(u.kind()) == Some(UnitBaseType::Mech))
        // The flagship's clause takes this loss too, and one destroyed model is captured once.
        && !flagship_present(c.state, me, system)
}

fn reanimator(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some(unit) = eff.event.payload.unit(Key::Unit) {
        capture(eff, me, unit);
    }
    Ok(())
}

// -- Devour -----------------------------------------------------------------------------------------

fn devour_ships_when(c: &CondCx<'_>) -> bool {
    let Some(system) = c.event.payload.system(Key::System) else { return false };
    c.event.payload.flag(Key::DuringSpaceCombat) == Some(true)
        && mark_held(c.state, &mark_key(system, None), c.def.owner)
        && !flagship_present(c.state, c.def.owner, system)
}

fn devour_ground_when(c: &CondCx<'_>) -> bool {
    let p = &c.event.payload;
    let (Some(system), Some(planet)) = (p.system(Key::System), p.planet(Key::Planet)) else { return false };
    p.code(Key::Cause) == Some(c.codes.ground_combat)
        && mark_held(c.state, &mark_key(system, Some(planet)), c.def.owner)
        && !flagship_present(c.state, c.def.owner, system)
}

fn devour(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some((victim, unit)) = destroyed_model(eff.event, me, false) {
        capture(eff, victim, unit);
    }
    Ok(())
}

// -- The Terror Between -----------------------------------------------------------------------------

fn terror_when(c: &CondCx<'_>) -> bool {
    c.event.payload.system(Key::System).is_some_and(|system| flagship_present(c.state, c.def.owner, system))
}

fn terror(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some((victim, unit)) = destroyed_model(eff.event, me, true) {
        capture(eff, victim, unit);
    }
    Ok(())
}

// -- the participation marks --------------------------------------------------------------------------

fn event_mark(event: &EventRec, ground: bool) -> Option<MarkKey> {
    let system = event.payload.system(Key::System)?;
    if ground {
        return Some(mark_key(system, Some(event.payload.planet(Key::Planet)?)));
    }
    Some(mark_key(system, None))
}

fn space_part_when(c: &CondCx<'_>) -> bool {
    event_mark(c.event, false).is_some() && took_part(c.event, c.def.owner)
}

fn ground_part_when(c: &CondCx<'_>) -> bool {
    event_mark(c.event, true).is_some() && took_part(c.event, c.def.owner)
}

fn write_mark(eff: &mut Eff<'_, '_>, ground: bool) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some(key) = event_mark(eff.event, ground) {
        eff.state().marks_mut().tail.edit().insert(key, MarkVal::Atom(Atom::Seat(me)));
    }
    Ok(())
}

fn clear_mark(eff: &mut Eff<'_, '_>, ground: bool) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some(key) = event_mark(eff.event, ground) {
        if mark_held(eff.state(), &key, me) {
            eff.state().marks_mut().tail.edit().remove(&key);
        }
    }
    Ok(())
}

/// Register the abilities, in the string order of the Cabal's `timing_abilities`: Reanimator, Devour (ships,
/// ground forces), The Terror Between (ships, ground forces), then the four marks.
pub fn register_abilities(t: &mut AbilityTable) {
    use EventKind as E;
    use Relation::After;
    let on = Applies::Faction(CABAL);
    t.add(Spec::new(REANIMATOR, AbilityKind::Unit, E::GroundForceDestroyed, After, reanimator).applies(on).when(reanimator_when));
    t.add(Spec::new(DEVOUR_SHIPS, AbilityKind::Ability, E::ShipDestroyed, After, devour).applies(on).when(devour_ships_when));
    t.add(Spec::new(DEVOUR_GROUND, AbilityKind::Ability, E::GroundForceDestroyed, After, devour).applies(on).when(devour_ground_when));
    t.add(Spec::new(TERROR_SHIPS, AbilityKind::Unit, E::ShipDestroyed, After, terror).applies(on).when(terror_when));
    t.add(Spec::new(TERROR_GROUND, AbilityKind::Unit, E::GroundForceDestroyed, After, terror).applies(on).when(terror_when));
    t.add(Spec::new(MARK_SPACE, AbilityKind::Ability, E::SpaceCombatStarted, After, |e| write_mark(e, false)).applies(on).when(space_part_when));
    t.add(Spec::new(CLEAR_SPACE, AbilityKind::Ability, E::SpaceCombatEnded, After, |e| clear_mark(e, false)).applies(on).when(space_part_when));
    t.add(Spec::new(MARK_GROUND, AbilityKind::Ability, E::GroundCombatStarted, After, |e| write_mark(e, true)).applies(on).when(ground_part_when));
    t.add(Spec::new(CLEAR_GROUND, AbilityKind::Ability, E::GroundCombatEnded, After, |e| clear_mark(e, true)).applies(on).when(ground_part_when));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_slots_in_the_registration_order() {
        let mut t = AbilityTable::default();
        register_abilities(&mut t);
        let slots: Vec<u16> = t.specs().iter().map(|s| s.slot.0).collect();
        assert_eq!(slots, (3001..=3009).collect::<Vec<_>>());
    }
}
