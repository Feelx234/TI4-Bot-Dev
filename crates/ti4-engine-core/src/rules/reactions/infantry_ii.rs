//! Infantry II (`infantry_ii.rs` of the string engine), effect 2 of the generic card (rules version
//! `RULES_TECH_EFFECTS`): "After this unit is destroyed, roll 1 die. If the result is 6 or greater, place the
//! unit on this card." The units on the card are a count in the owner's `Inf2Card` mark; they come back at the
//! start of the owner's next turn (`returns`, string: it places units on a planet).

use crate::choice::reaction::{AbilityKind, Relation};
use crate::generated::{Technology, Unit};
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, CondCx, Eff, Spec};
use crate::kernel::resolver::TimingError;
use crate::keys::{Atom, MarkKey, MarkVal};
use crate::query::tech;
use crate::state::{CoreState, EventKind, Key, RollPurpose};

use super::INFANTRY_II_ROLLS;

/// The die result that puts the unit on the card.
pub const THRESHOLD: u8 = 6;
/// `Dice::new()`: ten sides.
const SIDES: u32 = 10;
/// `RULES_TECH_EFFECTS`.
const RULES_TECH_EFFECTS: u32 = 15;

fn active(s: &CoreState, who: Seat) -> bool {
    s.meta().rules_at_least(RULES_TECH_EFFECTS) && tech::owns(s, who, Technology::InfantryII)
}

/// Infantry II units on the card.
#[must_use]
pub fn units_on_card(s: &CoreState, who: Seat) -> u32 {
    match s.marks().tail.get(&MarkKey::Inf2Card(who)) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

fn set_on_card(s: &mut CoreState, who: Seat, count: u32) {
    if count == 0 {
        s.marks_mut().tail.edit().remove(&MarkKey::Inf2Card(who));
    } else {
        s.marks_mut().tail.edit().insert(MarkKey::Inf2Card(who), MarkVal::Atom(Atom::Num(count)));
    }
}

fn rolls_condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && c.event.payload.unit(Key::Unit) == Some(Unit::InfantryII) && active(c.state, me)
}

fn rolls(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let faces = eff.roll(me, 1, Some(THRESHOLD), RollPurpose::Ability(INFANTRY_II_ROLLS.0), SIDES);
    if faces.first().is_some_and(|f| *f >= THRESHOLD) {
        let now = units_on_card(eff.state(), me) + 1;
        set_on_card(eff.state(), me, now);
    }
    Ok(())
}

pub fn register(t: &mut AbilityTable) {
    t.add(
        Spec::new(INFANTRY_II_ROLLS, AbilityKind::Technology, EventKind::GroundForceDestroyed, Relation::After, rolls).when(rolls_condition),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::kernel::fx;
    use crate::state::{Payload, Player, Players, Val};

    #[test]
    fn a_card_count_round_trips_through_the_mark() {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0])]);
        assert_eq!(units_on_card(&s, Seat(0)), 0);
        set_on_card(&mut s, Seat(0), 2);
        assert_eq!(units_on_card(&s, Seat(0)), 2);
        set_on_card(&mut s, Seat(0), 0);
        assert!(s.marks().tail.get(&MarkKey::Inf2Card(Seat(0))).is_none());
        let _ = (fx::event(EventKind::TurnBegan, Payload::new()), Val::Flag(true));
    }
}
