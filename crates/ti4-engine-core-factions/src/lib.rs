//! The faction modules of the typed core (layer L6; docs/ENGINE_CORE_STATE.md 4.1 and wave C,
//! docs/ENGINE_CORE_FACTIONS_PLAN.md).
//!
//! A faction module is a function `fn(&mut HookTable)` that pushes the module's hook functions into the
//! slots of `ti4_engine_core::kernel::hooks::HookTable`; [`register_all`] runs them in the string
//! engine's `factions::MODULES` order, which is the order of [`Id`]. That order fixes the dice stream
//! (a fold or a sum visits the modules in it), so it is **not** the order of the packages.
//!
//! # Layout (disjoint file ownership)
//!
//! Five packages CA to CE, one directory each (`ca_combat/` ... `ce_movement/`), one sub-directory per
//! faction, one file per tier (`movement.rs` for package A2's slots, then `query.rs` tier Q, `observe.rs`
//! tier O, `offer.rs` tier W). A package edits only the files of its own directory; this file, [`common`]
//! and the [`Id`] list are the steward's (package C0) and do not change when a package merges, so five
//! branches merge in any order without a conflict. Which package owns which faction is [`PACKAGES`].
//!
//! # Rules
//!
//! * Imports only from `ti4_engine_core` (L0-L5, in practice `state`, `query`, `kernel`, `generated`).
//! * No `String`, `&str`, `serde`, `format!`, `unsafe` in non-test code (`tools/engine_core/lint_core.sh`).
//! * A hook is called for every seat: it checks its own condition (`faction == ..` or the card held)
//!   and returns the neutral value otherwise. See `ti4_engine_core::kernel::hooks`.
//! * One slot function per string `Hooks` entry, same name, same condition, same order of effects.
//! * Providers that are not factions (laws, relics, technology, promissory notes, leaders) register on the
//!   same seams through `ti4_engine_core::kernel::seam`; they live with the package that owns the area.

use ti4_engine_core::hooks::HookTable;

pub mod common;
pub mod cards;
pub mod magen_defense;

pub mod ca_combat;
pub mod cb_economy;
pub mod cc_ground;
pub mod cd_registry;
pub mod ce_movement;

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

/// What a package exports: its modules and their registration functions.
pub type Modules = &'static [(Id, fn(&mut HookTable))];

/// The five packages, `CA` to `CE`: all their tiers.
pub const PACKAGES: [Modules; 5] = [
    ca_combat::MODULES,
    cb_economy::MODULES,
    cc_ground::MODULES,
    cd_registry::MODULES,
    ce_movement::MODULES,
];

/// The movement slots only (package A2), per package.
const MOVEMENT: [Modules; 5] = [
    ca_combat::MOVEMENT,
    cb_economy::MOVEMENT,
    cc_ground::MOVEMENT,
    cd_registry::MOVEMENT,
    ce_movement::MOVEMENT,
];

/// The modules no package provides yet (empty when wave C is done).
#[must_use]
pub fn missing() -> Vec<Id> {
    Id::ALL.into_iter().filter(|id| !PACKAGES.iter().any(|s| s.iter().any(|(i, _)| i == id))).collect()
}

fn register_in_order(table: &mut HookTable, packages: &[Modules]) {
    for id in Id::ALL {
        for package in packages {
            for (i, register) in *package {
                if *i == id {
                    register(table);
                }
            }
        }
    }
}

/// Fill `table` with every faction module, in [`Id`] order whichever package provides it.
pub fn register_all(table: &mut HookTable) {
    register_in_order(table, &PACKAGES);
}

/// A table with the movement slots of every faction filled (package A2), in [`Id`] order.
#[must_use]
pub fn movement_table() -> HookTable {
    let mut table = HookTable::new();
    register_in_order(&mut table, &MOVEMENT);
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_module_has_two_providers() {
        for id in Id::ALL {
            let n = PACKAGES.iter().flat_map(|s| s.iter()).filter(|(i, _)| *i == id).count();
            assert!(n <= 1, "{id:?} is provided {n} times");
        }
    }

    #[test]
    fn every_module_is_provided() {
        assert_eq!(missing(), Vec::<Id>::new());
    }

    #[test]
    fn the_movement_table_is_the_movement_part_of_the_full_table() {
        let mut full = HookTable::new();
        register_all(&mut full);
        let movement = movement_table();
        assert_eq!(movement.registered(), movement.movement.registered(), "the movement table fills movement slots only");
        assert_eq!(full.movement.registered(), movement.movement.registered(), "the full table's movement slots are the same");
        assert_eq!(MOVEMENT.iter().map(|m| m.len()).sum::<usize>(), 14, "A2 ports 14 factions");
    }

    #[test]
    fn registration_follows_the_dispatch_order_not_the_package_order() {
        // Ids are registered in `Id::ALL` order: each movement list is a subsequence of it.
        for m in MOVEMENT {
            assert!(m.windows(2).all(|w| w[0].0 < w[1].0), "a package lists its modules in dispatch order");
        }
    }
}
