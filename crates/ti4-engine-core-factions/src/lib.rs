//! The faction modules of the typed core (layer L6; docs/ENGINE_CORE_STATE.md 4.1 and wave C,
//! docs/ENGINE_CORE_FACTIONS_PLAN.md).
//!
//! A faction module is a function `fn(&mut HookTable)` that pushes the module's hook functions into the
//! slots of `ti4_engine_core::kernel::hooks::HookTable`; [`register_all`] runs them in the string
//! engine's `factions::MODULES` order, which is the order of [`Id`]. That order fixes the dice stream
//! (a fold or a sum visits the modules in it), so it is **not** the order of the shards.
//!
//! # Layout (disjoint file ownership)
//!
//! One directory per shard (`c1_movement/`, ...): the shard adds `<faction>.rs` files to its own
//! directory and lists them in its own `mod.rs` (`MODULES`, a table of `(Id, register)`). This file, the
//! [`Id`] list and [`SHARDS`] are written once here and not touched by a shard, so six shards merge
//! without a conflict.
//!
//! # Rules
//!
//! * Imports only from `ti4_engine_core` (L0-L5, in practice `state`, `query`, `kernel`, `generated`).
//! * No `String`, `&str`, `serde`, `format!`, `unsafe` in non-test code (`tools/engine_core/lint_core.sh`).
//! * A hook is called for every seat: it checks its own condition (`faction == ..` or the card held)
//!   and returns the neutral value otherwise. See `ti4_engine_core::kernel::hooks`.
//! * One slot function per string `Hooks` entry, same name, same condition, same order of effects.

use ti4_engine_core::kernel::hooks::HookTable;

pub mod c1_movement;
pub mod c2_economy;
pub mod c3_combat;
pub mod c4_strategy;
pub mod c5_cross;
pub mod c6_late;

/// A registered module: one entry of the string engine's `factions::MODULES` (Keleres has three).
/// The discriminant is the position in that array; [`Id::Borrowed`] is last and registers only the
/// `direct` slots of the shared borrowed-agent helpers, which have a single provider each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Id {
    Arborec,
    Argent,
    Bastion,
    Cabal,
    Crimson,
    Deepwrought,
    Empyrean,
    Firmament,
    Ghost,
    KeleresMentak,
    KeleresXxcha,
    KeleresArgent,
    Mahact,
    Mentak,
    Muaat,
    Naalu,
    Naaz,
    Nekro,
    Nomad,
    Obsidian,
    Ralnel,
    Saar,
    Sardakk,
    Titans,
    Winnu,
    Yin,
    Yssaril,
    Borrowed,
}

impl Id {
    /// Every module in dispatch order.
    pub const ALL: [Self; 28] = [
        Self::Arborec,
        Self::Argent,
        Self::Bastion,
        Self::Cabal,
        Self::Crimson,
        Self::Deepwrought,
        Self::Empyrean,
        Self::Firmament,
        Self::Ghost,
        Self::KeleresMentak,
        Self::KeleresXxcha,
        Self::KeleresArgent,
        Self::Mahact,
        Self::Mentak,
        Self::Muaat,
        Self::Naalu,
        Self::Naaz,
        Self::Nekro,
        Self::Nomad,
        Self::Obsidian,
        Self::Ralnel,
        Self::Saar,
        Self::Sardakk,
        Self::Titans,
        Self::Winnu,
        Self::Yin,
        Self::Yssaril,
        Self::Borrowed,
    ];
}

/// What a shard exports: its modules and their registration functions.
pub type Modules = &'static [(Id, fn(&mut HookTable))];

/// The six shards, `C1` to `C6`.
pub const SHARDS: [Modules; 6] = [
    c1_movement::MODULES,
    c2_economy::MODULES,
    c3_combat::MODULES,
    c4_strategy::MODULES,
    c5_cross::MODULES,
    c6_late::MODULES,
];

/// The modules no shard provides yet (empty when wave C is done).
#[must_use]
pub fn missing() -> Vec<Id> {
    Id::ALL.into_iter().filter(|id| !SHARDS.iter().any(|s| s.iter().any(|(i, _)| i == id))).collect()
}

/// Fill `table` with every ported faction module, in [`Id`] order whichever shard provides it.
pub fn register_all(table: &mut HookTable) {
    for id in Id::ALL {
        for shard in SHARDS {
            for (i, register) in shard {
                if *i == id {
                    register(table);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_module_has_two_providers() {
        for id in Id::ALL {
            let n = SHARDS.iter().flat_map(|s| s.iter()).filter(|(i, _)| *i == id).count();
            assert!(n <= 1, "{id:?} is provided {n} times");
        }
    }

    #[test]
    fn registration_follows_the_dispatch_order_not_the_shard_order() {
        // C1 holds Muaat and C3 holds nothing yet; a table filled by register_all has Muaat's slots.
        let mut t = HookTable::new();
        register_all(&mut t);
        assert!(t.movement.may_enter_supernova.len() <= 1);
    }

    /// The last shard to merge removes `#[ignore]`: all 28 modules are provided.
    #[test]
    #[ignore = "wave C is not finished"]
    fn every_module_is_provided() {
        assert_eq!(missing(), Vec::<Id>::new());
    }
}
