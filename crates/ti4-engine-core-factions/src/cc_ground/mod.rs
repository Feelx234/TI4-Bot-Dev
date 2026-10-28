//! Package CC, the ground-and-ship line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod firmament;
pub mod ghost;
pub mod mahact;
pub mod mentak;
pub mod borrowed;
pub mod sides;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Firmament, firmament::register),
    (Id::Ghost, ghost::register),
    (Id::Mahact, mahact::register),
    (Id::Mentak, mentak::register),
    (Id::Borrowed, borrowed::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Firmament, firmament::register_movement),
    (Id::Ghost, ghost::register_movement),
    (Id::Mentak, mentak::register_movement),
];

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
