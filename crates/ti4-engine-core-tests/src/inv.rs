//! The invariants the properties check (P6, docs/todo/D4-core-tests.md 5.2). Each returns the list of violations, empty when it holds.

use ti4_engine_core::generated::UnitBaseType;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::query::fleet::{held_on_board, plastic};
use ti4_engine_core::state::CoreState;
use ti4_engine_core::tables::Scope;

/// The scope every kit scenario plays in: every source.
pub const ALL_SOURCES: Scope = Scope(u32::MAX);

/// Per seat and base unit type: the pieces on the board and captured elsewhere do not exceed the plastic of the game.
#[must_use]
pub fn supply_limits(s: &CoreState) -> Vec<String> {
    let mut bad = Vec::new();
    for p in s.players().iter() {
        for base in UnitBaseType::ALL {
            let Some(limit) = plastic(base) else { continue };
            let held = held_on_board(s, ALL_SOURCES, p.seat, base);
            if held > limit {
                bad.push(format!("seat {}: {held} {base:?} held, plastic {limit}", p.seat.0));
            }
        }
    }
    bad
}

/// Goods, commodities and tokens are never negative; commodities never exceed the faction's printed limit.
#[must_use]
pub fn counters(s: &CoreState) -> Vec<String> {
    let mut bad = Vec::new();
    for p in s.players().iter() {
        let e = &p.econ;
        for (name, v) in [("trade goods", e.trade_goods), ("commodities", e.commodities), ("tactic tokens", e.tactic_tokens), ("fleet tokens", e.fleet_tokens), ("strategic tokens", e.strategic_tokens)] {
            if v < 0 {
                bad.push(format!("seat {}: {name} = {v}", p.seat.0));
            }
        }
        let limit = i16::from(p.faction.data().commodities);
        if e.commodities > limit {
            bad.push(format!("seat {}: {} commodities, limit {limit}", p.seat.0, e.commodities));
        }
    }
    bad
}

/// Between two states of one run, every seat's victory points moved exactly by the rows appended to the VP ledger.
#[must_use]
pub fn vp_matches_ledger(before: &CoreState, after: &CoreState) -> Vec<String> {
    let mut bad = Vec::new();
    for p in after.players().iter() {
        let was = before.players().by_seat(p.seat).map_or(0, |x| i32::from(x.econ.victory_points));
        let rows: i32 = after.ledger.vp.iter().skip(before.ledger.vp.len()).filter(|r| r.player == p.seat).map(|r| r.delta).sum();
        let now = i32::from(p.econ.victory_points);
        if now - was != rows {
            bad.push(format!("seat {}: victory points {was} -> {now}, ledger rows {rows}", p.seat.0));
        }
    }
    bad
}

/// Every unit on the board belongs to a seated seat or to the neutral owner range of the owner table.
#[must_use]
pub fn owners_exist(s: &CoreState) -> Vec<String> {
    let mut bad = Vec::new();
    let n = s.seats().n_owners();
    for (sys, cell) in s.board().systems() {
        for piece in cell.units.as_slice() {
            if usize::from(piece.owner().0) >= n {
                bad.push(format!("{sys:?}: a piece of owner {} (only {n} owners)", piece.owner().0));
            }
        }
    }
    bad
}

/// All the invariants that hold in every state; `before` is the state the run started from.
///
/// # Errors
/// The violations, one text each.
pub fn conservation(before: &CoreState, after: &CoreState) -> Result<(), Vec<String>> {
    let mut bad = supply_limits(after);
    bad.extend(counters(after));
    bad.extend(vp_matches_ledger(before, after));
    bad.extend(owners_exist(after));
    if bad.is_empty() { Ok(()) } else { Err(bad) }
}

/// Seats helper for tests: the seat of the player at table position `i`.
#[must_use]
pub fn seat(i: u8) -> Seat {
    Seat(i)
}
