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
pub mod saar;
pub mod tellurian;

/// Infantry II (`inf2`): after the owner's Infantry II is destroyed, roll a die.
pub const INFANTRY_II_ROLLS: Slot = Slot(1);

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
        let defs = table.registry(s, seats, &mut |spec, _, _| AbilityId { kind: AbilityKind::Other, key: Code(spec.slot.0) });
        resolver.register(defs);
        Driver::new(Emit { rec, done: false }, resolver, table, 10)
    }
}

#[cfg(test)]
mod layering {
    /// `tools/engine_core/check_layers.py core` failed the first gate run of stage 2: the round agents sat in this
    /// area and imported the L4 areas `combat` and `invasion`. The abilities that read another area live in the
    /// faction crate (L6); this area imports no other rule area.
    #[test]
    fn the_reactions_area_imports_no_other_rule_area() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules/reactions");
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            for other in ["combat", "invasion", "production", "agenda", "leaders", "strategy", "trade", "action_cards"] {
                let import = format!("crate::rules::{other}");
                let in_tests_only = text.matches(&import).count() == text.split("#[cfg(test)]").nth(1).map_or(0, |t| t.matches(&import).count());
                assert!(!text.contains(&import) || in_tests_only, "{} imports {import}", path.display());
            }
            checked += 1;
        }
        assert!(checked >= 5);
    }
}
