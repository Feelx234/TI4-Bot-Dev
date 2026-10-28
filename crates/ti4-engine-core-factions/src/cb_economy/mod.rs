//! Package CB, the economy line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod argent;
pub mod cabal;
pub mod deepwrought;
pub mod keleres;
pub mod winnu;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Argent, argent::register),
    (Id::Cabal, cabal::register),
    (Id::Deepwrought, deepwrought::register),
    (Id::KeleresMentak, keleres::register_m),
    (Id::KeleresXxcha, keleres::register_x),
    (Id::KeleresArgent, keleres::register_a),
    (Id::Winnu, winnu::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Argent, argent::register_movement),
    (Id::Cabal, cabal::register_movement),
    (Id::Deepwrought, deepwrought::register_movement),
    (Id::Winnu, winnu::register_movement),
];

/// A planet atom of a mark value (package-local: `common` has the other atom readers).
pub(crate) fn planet(v: &ti4_engine_core::keys::MarkVal) -> Option<ti4_engine_core::generated::Planet> {
    match v {
        ti4_engine_core::keys::MarkVal::Atom(ti4_engine_core::keys::Atom::Planet(p)) => Some(*p),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod testkit {
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::kernel::Kernel;
    use ti4_engine_core::state::{CoreState, Player, Players};

    /// A state with one player per faction (seat i plays `factions[i]`), every slice loaded, and a kernel with no modules.
    pub fn table(factions: &[Faction]) -> (CoreState, Kernel) {
        let mut s = CoreState::all_loaded();
        *s.players_mut() = Players::from_vec(factions.iter().enumerate().map(|(i, f)| Player::new(Seat(i as u8), *f)).collect());
        let seats: Vec<Seat> = (0..factions.len() as u8).map(Seat).collect();
        *s.seats_mut() = ti4_engine_core::state::Seats::new(&seats, &seats, factions.len() as u8);
        (s, Kernel::plain())
    }
}
