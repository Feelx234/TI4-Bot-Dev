//! The reverse direction of the lend (docs/todo/D2-prereqs.md D2-5 items 1 and 7): the string `GameState` of a
//! `CoreState`, and the audit of what a string body wrote.
//!
//! After the flip `Game` owns the `CoreState` and a leftover string module runs on a materialised `GameState`
//! (`reverse_lend::with_string`). Nothing here decides a rule.

use serde_json::Value;
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::state::GameState;

use super::codec::Codec;
use super::ctx::Ctx;
use super::error::R;

/// The slice of a top-level field of the serialised `GameState`. `core_sync_gate` and the footprint audit use it; a
/// field no slice owns is `Transient`.
#[must_use]
pub fn slice_of_field(key: &str) -> Slice {
    match key {
        "players" => Slice::Players,
        "seating_order" => Slice::Seats,
        "speaker" | "phase" | "round" | "active" | "finished" | "active_system" | "strategy_cards_per_player"
        | "rules_version" | "thunders_edge" | "legacy_device_ids" | "custodians_removed" | "fracture_in_play"
        | "nexus_unlocked" | "styx_holder" | "thunders_edge_system" | "galactic_event" | "ion_storm"
        | "influence_pays_for_units" | "combat_round_seq" | "production_seq" | "activation_seq" | "agenda_seq"
        | "turn_seq" | "action_turn" | "feat_occurrence_seq" | "combat_draw_round" => Slice::Meta,
        "diplomacy" | "transactions_this_round" | "negotiations_this_action" | "negotiations_this_round"
        | "transactions_this_turn" | "traded_goods" | "traded_goods_for_promissory" | "promises"
        | "support_holders" => Slice::Trade,
        "unclaimed_strategy_cards" | "strategy_card_goods" | "card_initiative" | "initiative_overrides" => Slice::Strategy,
        "board" | "exhausted_planets" | "planet_attachments" | "placed_planets" | "purged_systems" => Slice::Board,
        "pending" => Slice::Pending,
        "revealed_objectives" | "scored_objectives" | "scored_feat_occurrences" => Slice::Objectives,
        "objective_deck" | "exploration_decks" | "relic_deck" | "agenda_deck" | "action_card_deck" | "secret_deck" => {
            Slice::Decks
        }
        "discarded_action_cards" | "salvage_yard" | "promissory_notes" | "promissory_faceup" => Slice::Cards,
        "exploration_log" | "frontier_tokens" | "ingress_tokens" | "breach_tokens" | "expedition_slices"
        | "wormhole_tokens" => Slice::Explore,
        "agenda_log" | "agenda_casts" | "laws" | "agenda_veto_replacement" | "covert_agenda"
        | "agenda_elected_override" | "agenda_votes" | "agenda_choices" | "agenda_in_play" | "agenda_predictions" => {
            Slice::Agenda
        }
        "relic_log" | "vp_ledger" => Slice::Ledger,
        "rng_seed" => Slice::Rng,
        "faction_marks" => Slice::Marks,
        _ => Slice::Transient,
    }
}

/// The string state of `core`: `base` (the last materialisation, or the state the game started from) with every
/// loaded slice stored into it. `Map` stores nothing (the string `Galaxy` is the caller's) and `Rng` only the seed
/// (the stream positions are [`rng_positions`]).
///
/// # Errors
/// A typed value without a text.
pub fn materialize(core: &CoreState, cx: &Ctx, base: &GameState) -> R<GameState> {
    let mut gs = base.clone();
    Codec::store(cx, core, &mut gs, core.loaded)?;
    Ok(gs)
}

/// The stream positions of the typed generator as the string engine's map (`GameRng::set_positions`).
///
/// # Errors
/// A domain without a text.
pub fn rng_positions(core: &CoreState) -> R<crate::rng::RngPositions> {
    super::codec::rng::positions_in(&core.texts, core.rng())
}

/// The fields of `after` that differ from `before` and belong to a slice outside `writes`: `(slice, field)`, in
/// field order. A lent body must write nothing outside its declared slices (the footprint audit, D2-5 item 7).
#[must_use]
pub fn written_outside(before: &GameState, after: &GameState, writes: SliceSet) -> Vec<(Slice, String)> {
    let (Ok(Value::Object(a)), Ok(Value::Object(b))) = (serde_json::to_value(before), serde_json::to_value(after)) else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .map(|k| (slice_of_field(k), k.clone()))
        .filter(|(slice, _)| !writes.has(*slice))
        .collect()
}
