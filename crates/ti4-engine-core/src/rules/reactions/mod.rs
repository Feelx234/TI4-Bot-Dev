//! The ability rules behind the reaction windows (package `core-reactions`; docs/ENGINE_REACTIONS.md): the
//! `Spec`s of the generic abilities that are registered for every seat (technologies, leaders, the rules of
//! the table), one file each. The faction abilities live in the faction packages.
//!
//! [`register`] adds them to an `AbilityTable` in the per-owner registration order of the string
//! `reactions::arm`: the reaction-card slots, Instinct Training, I48S, Infantry II, Magen Defense, then the
//! factions.

use crate::kernel::ability::{AbilityTable, Slot};

pub mod infantry_ii;
pub mod instinct_training;
pub mod l1z1x_agent;
pub mod round_agent;
pub mod saar;
pub mod tellurian;

/// Infantry II (`inf2`): after the owner's Infantry II is destroyed, roll a die.
pub const INFANTRY_II_ROLLS: Slot = Slot(1);

/// The abilities the string registers ahead of the per-seat reaction-card slots (`reactions::arm`): the round
/// agents.
pub fn register_first(t: &mut AbilityTable) {
    round_agent::register(t);
}

/// Add the ported generic abilities that follow the slots, in the string registration order.
pub fn register(t: &mut AbilityTable) {
    instinct_training::register(t);
    l1z1x_agent::register(t);
    infantry_ii::register(t);
}

/// Add the technologies the string registers after the faction modules (`factions::timing_abilities` chains
/// `technology::timing_abilities` behind the per-faction hooks).
pub fn register_after_factions(t: &mut AbilityTable) {
    tellurian::register(t);
    saar::register(t);
}

/// A test window that emits one event, and the drivers the tests of the abilities run.
#[cfg(test)]
pub(crate) mod testkit {
    use crate::choice::reaction::{AbilityId, AbilityKind};
    use crate::choice::{Choice, Pick};
    use crate::driver::Driver;
    use crate::ids::Seat;
    use crate::kernel::ability::AbilityTable;
    use crate::kernel::stage::{Illegal, Sx, Window};
    use crate::kernel::{Kernel, Next, Resolver};
    use crate::state::{Code, CoreState, EventRec};

    #[derive(Clone)]
    pub struct Emit {
        pub rec: EventRec,
        pub done: bool,
    }

    impl Window for Emit {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.done
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.done {
                self.done = true;
                let _ = sx.emit(self.rec.clone());
            }
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }

    /// A driver over a window that emits `rec`, the registry of `table` for `seats` (ids are `Code(slot)`).
    pub fn driver(table: AbilityTable, s: &CoreState, seats: &[Seat], rec: EventRec) -> Driver<Emit> {
        let mut resolver = Resolver::new(seats, Some(seats[0]));
        let defs = table.registry(s, seats, &mut |spec, _| AbilityId { kind: AbilityKind::Other, key: Code(spec.slot.0) });
        resolver.register(defs);
        Driver::new(Emit { rec, done: false }, resolver, table, 10)
    }
}
