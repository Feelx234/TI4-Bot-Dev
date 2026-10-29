//! arborec: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Letani Warrior II, the
//! roll ("After this unit is destroyed, roll 1 die. If the result is 6 or greater, place the unit on this card.").
//! The units on the card are a count in the owner's `ArborecLw2` mark; they come back at the start of the next turn
//! (a placement, the string's). Not ported: Mitosis, Bioplasmosis, the flagship, the commander, Stymie, the return.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::tech;
use ti4_engine_core::state::{CoreState, EventKind, Key, RollPurpose};

/// Letani Warrior II (`unit:arborec:arborec_infantry2:GROUND_FORCE_DESTROYED:after`).
pub const WARRIOR_ROLLS: Slot = Slot(6001);

/// The die result that puts the unit on the card.
const THRESHOLD: u8 = 6;
/// `Dice::new()`: ten sides.
const SIDES: u32 = 10;

fn on_card(s: &CoreState, who: Seat) -> u32 {
    match s.marks().tail.get(&MarkKey::ArborecLw2(who)) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && c.event.payload.unit(Key::Unit) == Some(Unit::LetaniWarriorII) && tech::owns(c.state, me, Technology::LetaniWarriorII)
}

fn rolls(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let faces = eff.roll(me, 1, Some(THRESHOLD), RollPurpose::Ability(WARRIOR_ROLLS.0), SIDES);
    if faces.first().is_some_and(|f| *f >= THRESHOLD) {
        let now = on_card(eff.state(), me) + 1;
        eff.state().marks_mut().tail.edit().insert(MarkKey::ArborecLw2(me), MarkVal::Atom(Atom::Num(now)));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(
        Spec::new(WARRIOR_ROLLS, AbilityKind::Unit, EventKind::GroundForceDestroyed, Relation::After, rolls)
            .applies(Applies::Faction(Faction::TheArborec))
            .when(condition),
    );
}
