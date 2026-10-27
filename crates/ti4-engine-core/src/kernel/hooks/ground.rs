//! Invasion and ground-combat hooks (`factions/hooks_ground.rs`, 8 slots) and the staged ground events.

use super::{CombatUnit, CommitCandidate, CommitOrigin, hook_group};
use crate::containers::SmallList;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::{Cx, Kernel};
use crate::state::{CoreState, Piece};

/// Planets a hook forces into combat (sorted, no duplicates).
pub type PlanetList = SmallList<Planet, 4>;
/// Commit candidates (invasion).
pub type CandidateList = SmallList<CommitCandidate, 4>;
/// Pieces (temporary space commits).
pub type PieceList = SmallList<Piece, 4>;

hook_group! {
    /// `hooks_ground::GroundHooks`.
    GroundHooks {
        /// Extra hits at the end of the ground rolls (summed). String: `invasion`.
        sum ground_rolls_extra_hits(s: &CoreState, k: &Kernel, who: Seat, system: System, planet: Planet, hits: usize, opponent_hits: usize) -> usize;
        /// The unit may use sustain damage in ground combat; all modules must allow. String: `invasion`.
        all may_sustain(s: &CoreState, k: &Kernel, unit: CombatUnit) -> bool;
        /// Extra units that can be committed to the landing. String: `invasion`.
        concat commit_candidates(s: &CoreState, k: &Kernel, invader: Seat, system: System, already: &[CommitOrigin]) -> CandidateList;
        /// Space units that may be committed as if ground forces. String: `invasion`.
        concat temporary_space_commit_candidates(s: &CoreState, k: &Kernel, invader: Seat, system: System) -> PieceList;
        /// A piece counts as a ground force for this round. String: `invasion`, `action_cards`.
        any temporary_ground_force(s: &CoreState, k: &Kernel, who: Seat, system: System, planet: Planet, unit: Piece) -> bool;
        /// At the start of a ground-combat round, for each participant. String: `invasion`.
        each ground_combat_round_started(cx: &mut Cx<'_>, who: Seat, system: System, planet: Planet) -> ();
        /// Custodians Token needs no influence for this player. String: `agenda_effects`, `thunders_edge`.
        any custodians_free(s: &CoreState, k: &Kernel, who: Seat) -> bool;
        /// Planets in `system` that must be fought for. String: `invasion`.
        concat_sorted forced_combat_planets(s: &CoreState, k: &Kernel, invader: Seat, system: System) -> PlanetList;
    }
}
