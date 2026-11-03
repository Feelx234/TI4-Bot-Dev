//! From `strategy_cards_core.rs` of the legacy crate (copy).

use serde_json::{json, Value};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::fx;
use ti4_engine_core::state::CoreState;

/// `tokens::with_pool_details`: the pools as they stand now and what the question hands out or moves.
pub fn pool_details(core: &CoreState, seat: Seat, letnev: bool, mode: &str, to_place: Option<usize>) -> serde_json::Map<String, Value> {
    let (tactic, fleet, strategic) = core
        .players()
        .by_seat(seat)
        .map_or((0, 0, 0), |p| (p.econ.tactic_tokens, p.econ.fleet_tokens, p.econ.strategic_tokens));
    let mut d = serde_json::Map::new();
    d.insert("kind".to_owned(), "command_tokens".into());
    d.insert("mode".to_owned(), mode.into());
    d.insert("pools".to_owned(), json!({ "tactic": tactic, "fleet": fleet, "strategic": strategic }));
    d.insert("reinforcements".to_owned(), fx::tokens_in_reinforcements(core, seat).into());
    if letnev {
        d.insert("fleet_supply_bonus".to_owned(), 2.into());
    }
    match (mode, to_place) {
        ("gain" | "buy", Some(count)) => {
            d.insert("tokens_to_place".to_owned(), count.into());
        }
        ("redistribute" | "restack", _) => {
            d.insert("total".to_owned(), (tactic + fleet + strategic).into());
        }
        _ => {}
    }
    d
}
