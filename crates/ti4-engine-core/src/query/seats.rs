//! Seating, speaker and initiative order (`GameState::{clockwise_from, initiative_order}`).

use crate::containers::SmallList;
use crate::ids::Seat;
use crate::state::CoreState;

/// Seats in an order.
pub type SeatOrder = SmallList<Seat, 8>;

/// `GameState.seating_order`.
#[must_use]
pub fn seating_order(core: &CoreState) -> SeatOrder {
    SeatOrder::from_slice(core.seats().seating_order())
}

/// The position of a seat in the seating order.
#[must_use]
pub fn position(core: &CoreState, seat: Seat) -> Option<usize> {
    core.seats().seating_order().iter().position(|s| *s == seat)
}

/// `GameState::clockwise_from`: the seating order rotated to begin at `seat` (unrotated when the
/// seat does not sit).
#[must_use]
pub fn clockwise_from(core: &CoreState, seat: Seat) -> SeatOrder {
    let order = core.seats().seating_order();
    let Some(start) = order.iter().position(|s| *s == seat) else {
        return SeatOrder::from_slice(order);
    };
    let mut out = SeatOrder::new();
    for s in order[start..].iter().chain(&order[..start]) {
        out.push(*s);
    }
    out
}

/// The order of the strategy and agenda phases: clockwise from the speaker.
#[must_use]
pub fn speaker_order(core: &CoreState) -> SeatOrder {
    clockwise_from(core, core.meta().speaker)
}

/// The seat after `seat` clockwise (itself when alone or not seated).
#[must_use]
pub fn next_after(core: &CoreState, seat: Seat) -> Seat {
    let order = core.seats().seating_order();
    match order.iter().position(|s| *s == seat) {
        Some(i) => order[(i + 1) % order.len()],
        None => seat,
    }
}

/// `GameState::initiative_order`: players by the lowest initiative among their strategy cards (99
/// without one), an override replacing it, ties by seating (stable).
#[must_use]
pub fn initiative_order(core: &CoreState) -> SeatOrder {
    let strategy = core.strategy();
    let mut keyed: SmallList<(i32, usize, Seat), 8> = SmallList::new();
    for p in core.players().iter() {
        let printed = p.strat.as_slice().iter().map(|c| strategy.initiative.get(c).copied().unwrap_or(99)).min().unwrap_or(99);
        let initiative = strategy.initiative_overrides.get(&p.seat).copied().unwrap_or(printed);
        keyed.push((initiative, position(core, p.seat).unwrap_or(usize::MAX), p.seat));
    }
    // Stable by (initiative, seat position): the insertion order of equal keys is the player order.
    let items = keyed.as_mut_slice();
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 && (items[j - 1].0, items[j - 1].1) > (items[j].0, items[j].1) {
            items.swap(j - 1, j);
            j -= 1;
        }
    }
    keyed.as_slice().iter().map(|k| k.2).fold(SeatOrder::new(), |mut o, s| {
        o.push(s);
        o
    })
}

/// Whether the seat is a player (not just an owner of neutral units, the Styx holder, ...).
#[must_use]
pub fn is_player(core: &CoreState, seat: Seat) -> bool {
    core.seats().player_of(seat).is_some()
}
