//! Movement and adjacency hooks (`factions/hooks_movement.rs`, 20 slots). Package A2 builds the
//! core's `CoreRules` from these (`wormhole_limits`, `blocked_borders`, ... below reproduce the string
//! engine's composed queries).

use super::{MoveSite, PassSite, hook_group};
use crate::containers::{Set, SmallList};
use crate::generated::{Unit, Wormhole};
use crate::ids::{Seat, SystemIx};
use crate::kernel::Kernel;
use crate::state::CoreState;

/// A set of systems.
pub type SystemSet = Set<SystemIx, 4>;
/// A set of wormhole kinds.
pub type WormSet = Set<Wormhole, 1>;
/// Pairs of systems (links, blocked borders).
pub type Pairs = SmallList<(SystemIx, SystemIx), 4>;
/// Wormholes a module adds: `(system, kind)`.
pub type ExtraWormholes = SmallList<(SystemIx, Wormhole), 4>;
/// Owners a mover may pass through.
pub type Owners = SmallList<Seat, 4>;
/// Systems.
pub type Systems = SmallList<SystemIx, 4>;

hook_group! {
    /// `hooks_movement::MovementHooks`.
    MovementHooks {
        /// Wormholes a module adds to the map. String: `game` (galaxy build), `transit`.
        concat extra_wormholes(s: &CoreState) -> ExtraWormholes;
        /// Systems a module links (adjacency), as unordered pairs. String: `movement`, `transit`.
        concat linked_systems(s: &CoreState, k: &Kernel, who: Seat) -> Pairs;
        /// Added to a ship's move value (summed). String: `movement`.
        sum move_bonus(s: &CoreState, site: MoveSite) -> i32;
        /// A ship may move through ships of other owners. String: `movement`.
        any may_move_through_ships(s: &CoreState, k: &Kernel, site: PassSite) -> bool;
        /// The part of [`Self::may_move_through_ships`] that does not read the active system (Yssaril's
        /// flagship), so the rules builder asks it once per seat and ship type instead of once per
        /// destination. A2 split; the string engine has one hook whose result the two slots OR together.
        any may_move_through_ships_anywhere(s: &CoreState, k: &Kernel, who: Seat, ship: Unit) -> bool;
        /// A player may enter a supernova. String: `movement`.
        any may_enter_supernova(s: &CoreState, k: &Kernel, who: Seat) -> bool;
        /// A player may pass through a supernova. String: `movement`.
        any may_pass_through_supernova(s: &CoreState, k: &Kernel, who: Seat) -> bool;
        /// A unit carries cargo for free. String: `movement`, `transit`.
        any free_cargo(s: &CoreState, k: &Kernel, unit: crate::state::Piece) -> bool;
        /// Passage through `system` is blocked for `who`. String: `movement`.
        any blocks_passage(s: &CoreState, k: &Kernel, who: Seat, system: SystemIx) -> bool;
        /// `who` cannot activate `system`. String: `tactical`.
        any cannot_activate(s: &CoreState, k: &Kernel, who: Seat, system: SystemIx) -> bool;
        /// Systems adjacent to a ship's position for this ship type, beyond the map. String: `movement`.
        concat_sorted unit_adjacent_systems(s: &CoreState, k: &Kernel, who: Seat, ship: Unit) -> Systems;
        /// A ship type ignores command tokens when moving. String: `tactical`.
        any ignores_command_tokens(s: &CoreState, k: &Kernel, who: Seat, ship: Unit) -> bool;
        /// Extra gravity rifts a module makes. String: `movement`.
        concat_sorted gravity_rifts(s: &CoreState) -> Systems;
        /// Rolls for a rift in `system` are skipped for `who`. String: `movement`.
        any rift_roll_exempt(s: &CoreState, who: Seat, system: SystemIx) -> bool;
        /// Rifts are ignored with one extra step. String: `movement`.
        any rift_crucible(s: &CoreState, who: Seat) -> bool;
        /// `who` ignores nebulae. String: `movement`.
        any ignores_nebulae(s: &CoreState, who: Seat) -> bool;
        /// Owners `who` may move through. String: `movement`.
        concat_sorted passable_owners(s: &CoreState, who: Seat) -> Owners;
        /// Borders (as pairs) blocked for `who`. String: `movement`.
        concat blocked_borders(s: &CoreState, who: Seat) -> Pairs;
        /// Own units may be passed through. String: `movement`.
        any own_unit_passage(s: &CoreState, k: &Kernel, site: PassSite) -> bool;
        /// Wormhole kinds `who` may use; the dispatcher intersects the `Some`s. String: `movement`.
        first usable_wormhole_kinds(s: &CoreState, who: Seat) -> Option<WormSet>;
        /// Systems whose wormholes are suppressed. String: `movement`.
        concat_sorted severed_systems(s: &CoreState) -> Systems;
        /// A ship with no cargo may pass through enemy ships. String: `movement`.
        any unladen_pass(s: &CoreState, who: Seat) -> bool;
    }
}

impl MovementHooks {
    /// The wormhole restriction for a mover (`wormhole_limits`): severed systems and the intersection
    /// of every module's allowed kinds (`None` = no restriction).
    #[must_use]
    pub fn wormhole_limits(&self, s: &CoreState, who: Seat) -> (Systems, Option<WormSet>) {
        let severed = self.severed_systems(s);
        let mut allowed: Option<WormSet> = None;
        for f in &self.usable_wormhole_kinds {
            if let Some(kinds) = f(s, who) {
                allowed = Some(match allowed {
                    Some(so_far) => so_far.iter().filter(|w| kinds.contains(*w)).collect(),
                    None => kinds,
                });
            }
        }
        (severed, allowed)
    }

    /// The links with each pair normalised (smaller first), self-links dropped, sorted, no duplicates
    /// (`linked_systems_by`).
    #[must_use]
    pub fn linked_pairs(&self, s: &CoreState, k: &Kernel, who: Seat) -> Pairs {
        normalised(self.linked_systems(s, k, who))
    }

    /// The blocked borders, normalised as the links are (`blocked_borders`).
    #[must_use]
    pub fn blocked_pairs(&self, s: &CoreState, who: Seat) -> Pairs {
        normalised(self.blocked_borders(s, who))
    }

    /// The wormholes the modules add, grouped by system in system order, kinds in kind order
    /// (`extra_wormholes`); one `(system, kind)` per entry, no duplicates.
    #[must_use]
    pub fn extra_wormhole_set(&self, s: &CoreState) -> ExtraWormholes {
        let mut all: Vec<(SystemIx, Wormhole)> = self.extra_wormholes(s).iter().copied().collect();
        all.sort();
        all.dedup();
        all.into_iter().collect()
    }
}

fn normalised(pairs: Pairs) -> Pairs {
    let mut v: Vec<(SystemIx, SystemIx)> = pairs
        .iter()
        .filter(|(a, b)| a != b)
        .map(|&(a, b)| if a < b { (a, b) } else { (b, a) })
        .collect();
    v.sort();
    v.dedup();
    v.into_iter().collect()
}

