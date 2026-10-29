//! The end of a space combat: who won, why it ended and which ships were lost (the payload of the
//! string `SPACE_COMBAT_ENDED`, `CombatWindow::end_payload`), as pure functions of the position.

use super::{ships_of, Rd};
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;

/// `combat::finished`: a side has no ship left in the space area.
#[must_use]
pub fn finished(rd: Rd<'_>, system: System, attacker: Seat, defender: Seat) -> bool {
    ships_of(rd, attacker, system).is_empty() || ships_of(rd, defender, system).is_empty()
}

/// `combat::winner`: the only side with ships left (both or neither: none).
#[must_use]
pub fn winner(rd: Rd<'_>, system: System, attacker: Seat, defender: Seat) -> Option<Seat> {
    match (!ships_of(rd, attacker, system).is_empty(), !ships_of(rd, defender, system).is_empty()) {
        (true, false) => Some(attacker),
        (false, true) => Some(defender),
        _ => None,
    }
}

/// Why the fight ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Retreat,
    Draw,
    Destruction,
    RoundLimit,
}

/// What `SPACE_COMBAT_ENDED` reports (besides the system, the sides and the round count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndReport {
    pub reason: EndReason,
    /// The sides that did not win (empty without a winner).
    pub losers: SmallList<Seat, 2>,
    /// Who left, in the order they left.
    pub retreated: SmallList<Seat, 2>,
    /// Every ship a side had when the fight opened that is neither still there nor arrived with a
    /// retreating fleet, side by side (attacker first), in the order of the opening fleet.
    pub destroyed: Vec<(Seat, Unit)>,
}

/// `CombatWindow::end_payload`. `winner` is the outcome's, `fled` the retreats (who and the ships that
/// arrived), `at_start` each side's fleet when the fight opened (the Eidolon flip adds to it).
#[must_use]
pub fn end_report(
    rd: Rd<'_>,
    system: System,
    sides: [Seat; 2],
    winner: Option<Seat>,
    declared_draw: bool,
    fled: &[(Seat, Vec<Unit>)],
    at_start: &[(Seat, Vec<Unit>)],
) -> EndReport {
    let reason = if !fled.is_empty() {
        EndReason::Retreat
    } else if declared_draw {
        EndReason::Draw
    } else if finished(rd, system, sides[0], sides[1]) {
        EndReason::Destruction
    } else {
        EndReason::RoundLimit
    };
    let mut destroyed = Vec::new();
    for side in sides {
        let mut pool: Vec<Unit> = ships_of(rd, side, system).iter().map(|p| p.kind()).collect();
        for (who, arrived) in fled {
            if *who == side {
                pool.extend(arrived.iter().copied());
            }
        }
        for (owner, fleet) in at_start {
            if *owner != side {
                continue;
            }
            for kind in fleet {
                if let Some(at) = pool.iter().position(|held| held == kind) {
                    pool.remove(at);
                } else {
                    destroyed.push((side, *kind));
                }
            }
        }
    }
    let losers = sides.into_iter().filter(|s| winner.is_some_and(|won| won != *s)).collect();
    let retreated = fled.iter().map(|(who, _)| *who).collect();
    EndReport { reason, losers, retreated, destroyed }
}
