//! From `core_combat.rs` of the legacy crate (copy).

use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::rules::combat as cc;
use ti4_engine_core::rules::combat::Rd;
use ti4_engine_core::state::Piece;

use crate::generated::IdStr;

/// The `sustainers` detail of a sustain question, from the core state it is asked in.
#[must_use]
pub fn sustainers_detail(kernel: &Kernel, core: &ti4_engine_core::state::CoreState, ask: &ti4_engine_core::choice::combat::Ask, actor: Seat) -> serde_json::Value {
    let ti4_engine_core::choice::combat::Ask::CancelHit { system, in_combat, ability, staging, .. } = *ask else {
        return serde_json::Value::Null;
    };
    let rd = Rd::new(core, kernel).with_blanked(ti4_engine_core::query::reach::blanked(core));
    let available: Vec<(bool, Piece)> = cc::sustain_candidates(rd, actor, system, in_combat, ability, staging)
        .into_iter()
        .map(|(site, p)| (matches!(site, ti4_engine_core::choice::combat::SustainSite::Planet { .. }), p))
        .collect();
    let counts = cc::sustainer_counts(rd, system, actor, in_combat, &available);
    let map: std::collections::BTreeMap<String, u64> = counts.into_iter().map(|(u, n)| (u.as_str().to_owned(), n)).collect();
    serde_json::to_value(map).unwrap_or(serde_json::Value::Null)
}
