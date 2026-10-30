//! The feats of a finished space combat (the string `combat::note_combat_feats_at`): the secrets that ask about the event
//! cannot be read off the board afterwards, so they are decided from the system as it was before the fight ([`Before`]: the
//! sides, their war suns and flagships, the promissory notes held) and as it is after it.
//!
//! [`feats`] reads the position and lists the `(seat, feat)` pairs; [`record`] writes them against the combat's occurrence.

use super::Rd;
use crate::generated::{System, UnitBaseType};
use crate::ids::Seat;
use crate::state::{Before, CoreState, Feat};
use crate::tables::{Flags, SystemFlags};

/// How the fight ended: the winner (none for a draw) and the rounds fought.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub winner: Option<Seat>,
    pub rounds: u32,
}

fn capitals(rd: Rd<'_>, seat: Seat, system: System) -> usize {
    rd.s.board().sys(system).units.iter().filter(|u| u.owner() == seat).filter(|u| rd.data(u.kind()).is_some_and(|d| matches!(d.base_type, UnitBaseType::Warsun | UnitBaseType::Flagship))).count()
}

fn non_fighters(rd: Rd<'_>, seat: Seat, system: System) -> usize {
    rd.s.board().sys(system).units.iter().filter(|u| u.owner() == seat && rd.is_ship(u.kind()) && !rd.is_fighter(u.kind())).count()
}

fn has_flagship(rd: Rd<'_>, seat: Seat, system: System) -> bool {
    rd.s.board().sys(system).units.iter().filter(|u| u.owner() == seat).any(|u| rd.data(u.kind()).is_some_and(|d| d.base_type == UnitBaseType::Flagship))
}

/// The feats a finished combat earned, in the order the string records them. Nothing for a fight of no rounds.
#[must_use]
pub fn feats(rd: Rd<'_>, system: System, before: &Before, outcome: Outcome) -> Vec<(Seat, Feat)> {
    let mut out = Vec::new();
    if outcome.rounds == 0 {
        return out;
    }
    let sides = before.sides.as_slice();
    // Destroy Their Greatest Ship: a drop in one side's count is the other side's feat, both when both lost one.
    for side in sides {
        let had = before.capitals.as_slice().iter().find(|(s, _)| s == side).map_or(0, |(_, n)| usize::from(*n)); // mutants:skip: every side is in the snapshot's capital list
        if had == 0 {
            continue;
        }
        if capitals(rd, *side, system) < had {
            for other in sides.iter().filter(|o| *o != side) {
                out.push((*other, Feat::DestroyedACapitalShip));
            }
        }
    }
    // Demonstrate Your Power: whoever is still standing there with three non-fighter ships.
    for side in sides {
        if non_fighters(rd, *side, system) >= 3 {
            out.push((*side, Feat::HeldThreeShipsAfterASpaceCombat));
        }
    }
    let Some(winner) = outcome.winner else { return out };
    let anomaly = rd.scope().system(system).is_some_and(|d| {
        d.has(SystemFlags::NEBULA) || d.has(SystemFlags::SUPERNOVA) || d.has(SystemFlags::SCAR) || d.has(SystemFlags::ASTEROID_FIELD) || d.has(SystemFlags::GRAVITY_RIFT)
    });
    if anomaly {
        out.push((winner, Feat::WonInAnAnomaly));
    }
    if rd.s.players().iter().any(|p| p.seat != winner && p.home_system == Some(system)) {
        out.push((winner, Feat::WonInARivalHome));
    }
    // A flagship is a capital ship, so the string's `capital_ships_of > 0` is implied.
    if has_flagship(rd, winner, system) {
        out.push((winner, Feat::WonBesideASurvivingFlagship));
    }
    // "The most victory points" includes a tie.
    let most = rd.s.players().iter().map(|p| p.econ.victory_points).max().unwrap_or(0); // mutants:skip: a position with a winner has players
    for loser in sides.iter().filter(|s| **s != winner) {
        if rd.s.players().by_seat(*loser).is_some_and(|p| p.econ.victory_points == most) {
            out.push((winner, Feat::WonAgainstThePointsLeader));
        }
        if before.notes.as_slice().contains(&(winner, *loser)) {
            out.push((winner, Feat::WonAgainstANoteHolder));
        }
    }
    out
}

/// `GameState::record_event_feat` for each pair against `occurrence`.
pub fn record(core: &mut CoreState, list: &[(Seat, Feat)], occurrence: u64) {
    for (seat, feat) in list {
        if let Some(p) = core.players_mut().by_seat_mut(*seat) {
            let entry = (*feat, occurrence);
            if !p.misc.event_feats.as_slice().contains(&entry) {
                p.misc.event_feats.push(entry);
                p.misc.event_feats.as_mut_slice().sort_unstable();
            }
        }
    }
}
