//! winnu: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported (package W9): the unlock of Rickar Rickani,
//! "Enter into a combat in the Mecatol Rex system". Not ported: the commander's own bonus (a hook), Imperator.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Leader, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus, Payload};

/// The unlock when a space combat starts (`leader:*:winnucommander_unlock:SPACE_COMBAT_STARTED:after`).
pub const UNLOCK_SPACE: Slot = Slot(15_001);
/// ... and a ground combat (`...:GROUND_COMBAT_STARTED:after`).
pub const UNLOCK_GROUND: Slot = Slot(15_002);

/// A combat of `who` in a Mecatol Rex system (either printing) while the commander is locked.
fn unlocks(s: &CoreState, who: Seat, event: &Payload) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::RickarRickani) == Some(&LeaderStatus::Locked))
        && event.system(Key::System).is_some_and(|system| matches!(system, System::MecatolRex18 | System::MecatolRex112))
        && (event.seat(Key::Attacker) == Some(who) || event.seat(Key::Defender) == Some(who))
}

fn condition(c: &CondCx<'_>) -> bool {
    unlocks(c.state, c.def.owner, &c.event.payload)
}

fn unlock(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.read().0.players().by_seat(me).is_some_and(|p| p.leaders.status.get(&Leader::RickarRickani) == Some(&LeaderStatus::Locked)) {
        ti4_engine_core::rules::leaders::unlock(eff.state(), me, &[Leader::RickarRickani]);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(UNLOCK_SPACE, AbilityKind::Leader, EventKind::SpaceCombatStarted, Relation::After, unlock).when(condition));
    t.add(Spec::new(UNLOCK_GROUND, AbilityKind::Leader, EventKind::GroundCombatStarted, Relation::After, unlock).when(condition));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::state::Val;

    fn combat(system: System, attacker: Seat, defender: Seat) -> Payload {
        Payload::new().with(Key::System, Val::System(system)).with(Key::Attacker, Val::Seat(attacker)).with(Key::Defender, Val::Seat(defender))
    }

    #[test]
    fn a_combat_at_mecatol_unlocks_a_locked_commander_of_a_seat_in_it() {
        let (mut s, _) = table(&[Faction::TheWinnu, Faction::TheEmiratesOfHacan, Faction::TheArborec]);
        // No leader row yet: nothing to unlock.
        assert!(!unlocks(&s, Seat(0), &combat(System::MecatolRex18, Seat(0), Seat(1))));
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::RickarRickani, LeaderStatus::Locked);
        assert!(unlocks(&s, Seat(0), &combat(System::MecatolRex18, Seat(0), Seat(1))), "as the attacker");
        assert!(unlocks(&s, Seat(0), &combat(System::MecatolRex112, Seat(1), Seat(0))), "as the defender, the other printing");
        assert!(!unlocks(&s, Seat(0), &combat(System::MecatolRex18, Seat(1), Seat(2))), "a combat of two others");
        assert!(!unlocks(&s, Seat(0), &combat(System::Wellon, Seat(0), Seat(1))), "another system");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::RickarRickani, LeaderStatus::Unlocked);
        assert!(!unlocks(&s, Seat(0), &combat(System::MecatolRex18, Seat(0), Seat(1))), "already unlocked");
    }
}
