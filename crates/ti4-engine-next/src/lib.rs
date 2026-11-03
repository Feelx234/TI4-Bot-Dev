//! The product-facing engine crate. Facade stage: everything not declared here is re-exported from
//! `ti4-engine-legacy` (the directory `crates/ti4-engine`, docs/todo/D2-7-legacy-split.md). A module declared in this
//! file shadows the glob re-export of the same name; the crate is filled module by module (docs/todo/D2-8-wrapper.md).
//!
//! One section per lane (section 6.2 of the plan): a lane edits only its own section.


// ---- lane C (native setup and position builders, D2-8c; docs/D28C_RESUME.md) ----
pub mod builders;
pub mod deck;
pub mod fixtures;
pub mod map_feasibility;
pub mod opening;
pub mod seating;
pub mod setup;
pub mod starting_tech;

pub use deck::{EXPLORATION_TRAITS, StartingDecks, build_starting_decks};
pub use seating::{SeatingError, build_board, deploy, home_systems, neutral_systems};
pub use setup::{
    SetupError, cards_per_player, seeded_speaker, start_game, start_game_seeded,
    start_game_seeded_with_card_set, strategy_card_setup, strategy_card_setup_for_set,
};

pub use ti4_engine_legacy::*;

// ---- lane A (foundation, 8b1-8b3): generated tables, wire types, glue, Observed ----
/// The generated adapter tables: one source file, included from the legacy directory (not copied; plan 1.4).
#[path = "../../ti4-engine/src/generated/mod.rs"]
pub mod generated;
/// The wire and format types (copies; `tests/wire_parity.rs`).
pub mod wire;
/// The items of the legacy crate that the glue names outside itself (Appendix A of the plan): copies.
pub mod ties;
/// What a decider may see (8b3).
pub mod observed;
// The glue (8b2): copies of the legacy modules, cut loose from the string rule modules.
pub mod core_sync;
pub mod kernel_sync;
pub mod live;
pub mod product;
pub mod render;
pub mod seen_load;

// ---- lane B (surface, 8d): constants, queries, previews ----
/// The constants of the product surface (choice kinds, option ids, prefixes); `tests/surface_parity.rs` compares each with its legacy original.
pub mod kinds;
/// The native queries and the result types they return (one file per legacy module).
pub mod queries;

// The public paths of the surface. Each module below is the legacy module of the same name (facade glob, until D2-9) with the NATIVE items
// on top; an explicit item shadows the glob. Only items whose types are shared with the legacy crate are exported here; the ones that
// carry a type the legacy `Observed`/`Game` also speaks (the result types `Movable`, `MoveRoute`, `Cargo`, `Spend`, `CardProgress`, `Imagined`,
// ...) stay in `queries` until lane D's swap flips `choice`/`game` (docs/D28B_RESUME.md lists the exact lines to add then).
pub mod action_cards {
    pub use crate::kinds::action_cards::{DISCARD_KIND, OWN_HAND_KIND, TAKE_REVEALED_KIND};
    pub use crate::queries::action_cards::{hand_limit, insider_peek, insider_peek_key, is_insider_peek_key};
    pub use crate::ties::action_cards::name_of;
    pub use ti4_engine_legacy::action_cards::*;
}
pub mod combat {
    pub use crate::kinds::combat::{CASUALTY_KIND, RETREAT_KIND, RETREAT_TO_KIND, SUSTAIN_KIND};
    pub use crate::queries::combat::{cancellable_hits, opponent_with_ships, space_cannon_guns};
    pub use ti4_engine_legacy::combat::*;
}
pub mod diplomacy {
    pub use ti4_engine_legacy::diplomacy::*;
    pub mod candidates {
        pub use crate::kinds::diplomacy::OPEN_KIND;
        pub use ti4_engine_legacy::diplomacy::candidates::*;
    }
}
pub mod entropic_scars {
    pub use crate::queries::entropic_scars::{abilities_usable, is_scar};
    pub use ti4_engine_legacy::entropic_scars::*;
}
pub mod exploration {
    pub use crate::queries::content::{attachment_for, frontier_systems, resolution, traits_of};
    pub use ti4_engine_legacy::exploration::*;
}
pub mod faction_abilities {
    pub use crate::queries::faction_abilities::has;
    pub use ti4_engine_legacy::faction_abilities::*;
}
pub mod factions {
    pub use ti4_engine_legacy::factions::*;
    pub use crate::queries::factions::{Asset, AssetKind, assets};
    pub mod hooks_economy {
        pub use crate::queries::factions::production_cost_reduction;
        pub use ti4_engine_legacy::factions::hooks_economy::*;
    }
    pub mod mahact {
        pub use crate::queries::factions::fleet_pool_owners;
        pub use ti4_engine_legacy::factions::mahact::*;
    }
}
pub mod fleet {
    pub use crate::queries::fleet::{Arrival, Standing, UNLIMITED_FLEET_BILL, is_unlimited, standing};
    pub use ti4_engine_legacy::fleet::*;
}
pub mod fleet_strength {
    pub use crate::queries::fleet_strength::*;
}
pub mod fracture {
    pub use crate::queries::fracture::{is_fracture_system, systems};
    pub use ti4_engine_legacy::fracture::*;
}
pub mod invasion {
    pub use crate::kinds::invasion::{COMMIT_KIND, GROUND_CASUALTY_KIND};
    pub use crate::queries::invasion::bombardable;
    pub use ti4_engine_legacy::invasion::*;
}
pub mod legendary {
    pub use crate::queries::legendary::{available, ionian_available};
    pub use ti4_engine_legacy::legendary::*;
}
pub mod objectives {
    pub use crate::kinds::objectives::{SCORE_KIND, VICTORY_TARGET};
    pub use crate::queries::content::{leader, points_for, winner};
    pub use crate::queries::objectives::{CardProgress, Cost, Imagined, Position, bought_progress_at, cost_family_token, cost_of, counting_progress, family_token, home_planets_not_controlled, neighbour_planet_counts, remaining_position_progress, scoreable_on};
    pub use ti4_engine_legacy::objectives::*;
}
pub mod planets {
    pub use crate::queries::planets::{in_system, tech_specialties_now, traits_now};
    pub use ti4_engine_legacy::planets::*;
}
pub mod production {
    pub use crate::kinds::production::{AGENT_OPTION_ID, AGENT_OPTION_KIND, BUILDABLE, PAY_KIND, PLACE_KIND, PRODUCE_KIND, REMOTE_SEPARATOR, SPACE, UNLOCKED_BY};
    pub use crate::queries::production::{Spend, available, native_payment_planets, buildable_for, capacity, placements, planet_value_boosts, price_of_under, trade_good_worth};
    pub use ti4_engine_legacy::production::*;
}
pub mod promissory {
    pub use crate::kinds::promissory::SUPPORT_PREFIX;
    pub use crate::queries::promissory::{alias_of, faction_name, held_by, holder_of, may_receive, note_id, owner_of};
    pub use ti4_engine_legacy::promissory::*;
}
pub mod reactions {
    pub use crate::kinds::action_cards::ACTION_CARD_KIND;
    pub use ti4_engine_legacy::reactions::*;
}
pub mod relics {
    pub use crate::kinds::relics::SHARD;
    pub use ti4_engine_legacy::relics::*;
}
pub mod supply {
    pub use crate::queries::supply::{base_type_of, held, plastic, remaining};
    pub use ti4_engine_legacy::supply::*;
}
pub mod valuation {
    pub use crate::queries::valuation::{BASE_UNIT_VALUE, MULTIPLIER_CEILING, MULTIPLIER_FLOOR, base_unit_value, clamp, fleet_strength, planet_multipliers, planet_value, stranded_troops, system_value, unit_value};
    pub use ti4_engine_legacy::valuation::*;
}
pub mod reach_query {
    pub use crate::queries::reach_query::{ReachOrigin, who_can_reach};
    pub use ti4_engine_legacy::reach_query::*;
}
pub mod secondary_preview {
    pub use crate::kinds::secondary_preview::MAX_ANSWERS;
    pub use crate::queries::secondary_preview::{SecondaryPreview, preview_secondary};
    pub use ti4_engine_legacy::secondary_preview::*;
}
pub mod secrets {
    pub use crate::kinds::secrets::HAND_LIMIT;
    pub use crate::queries::content::scored_count;
    pub use crate::queries::secrets::{Position, progress, ship_systems_beside_anomaly};
    pub use ti4_engine_legacy::secrets::*;
}
pub mod strategy {
    pub use crate::queries::strategy::SecondaryResolution;
    pub use crate::kinds::strategy::{ACTION_KIND, STRATEGIC_ACTION_ID, STRATEGY_KIND, WAIVED_SECONDARY_PREFIX};
    pub use ti4_engine_legacy::strategy::*;
}
pub mod strategy_cards {
    pub use crate::kinds::strategy_cards::{INFLUENCE_PER_TOKEN, RESEARCH_KIND};
    pub use crate::queries::strategy_cards::card_kind;
    pub use crate::ties::strategy_cards::card_name;
    pub use ti4_engine_legacy::strategy_cards::*;
}
pub mod tactical {
    pub use crate::kinds::tactical::{ACTIVATE_KIND, MAX_ROUTES, MOVE_KIND, ROUTES_DETAIL};
    pub use crate::queries::tactical::{Movable, MoveRoute, effective_move_value_for_ship, movable_into, routes_into, routes_of};
    pub use ti4_engine_legacy::tactical::*;
}
pub mod technology {
    pub use crate::queries::technology::{active_aliases, colour_type, faction_of, gravity_drive_available, is_unit_upgrade, name, prerequisites, replaced_for_faction, COLOURS};
    pub use ti4_engine_legacy::technology::*;
}
pub mod timing {
    pub use crate::kinds::timing::ACTIVE_DETAIL;
    pub use crate::ties::timing::Relation;
    pub use ti4_engine_legacy::timing::*;
}
pub mod transactions {
    pub use crate::kinds::transactions::{ANSWER_KIND, FREE_FORM_PREFIX, OFFER_KIND, OPEN_KIND};
    pub use ti4_engine_legacy::transactions::*;
}
pub mod transit {
    pub use crate::kinds::transit::{LOAD_KIND, RIFT_DESTROYS_ON};
    pub use crate::queries::transit::{Cargo, CargoSource, loadable};
    pub use ti4_engine_legacy::transit::*;
}

// ---- lane C (setup, 8c): setup, seating, deck, opening, starting_tech, map_feasibility, fixtures ----

// ---- lane D (wrapper, 8e/8f): game ----
/// The native `Game` (the typed core owns the loop). Reached as `ti4_engine::native::Game` until the swap commit makes it `ti4_engine::game`.
#[path = "game/mod.rs"]
pub mod native;
// THE SWAP: the public paths of the wire types, `Observed` and the `Game` are the thin crate's. Everything that speaks `Choice`, `Table`, `Decider`
// or `Game` now speaks these; the legacy modules of the glob that still name the legacy types are listed in docs/D28D_RESUME.md.
pub mod game {
    pub use crate::kinds::game::*;
    pub use crate::native::{Game, GameError, GameSnapshot, RunError, StepResult};
}
pub mod choice {
    pub use crate::observed::*;
    pub use crate::wire::choice::*;
}
/// Thin deciders on legacy tables, for oracles and the tests that still drive a string window (never production).
pub mod oracle_adapter;
/// The trigger a reaction decision carries (copy; pure).
pub mod trigger;
pub use wire::{decision_context, dice, event, fingerprint, observation, preview, rng};
/// A wire value of the legacy crate read as the same value of this one (a `Choice`, a `DecisionRecord`, ...): one serde shape,
/// `tests/wire_parity.rs`. For tests and oracles that still drive a string window; never for the game loop.
///
/// # Panics
/// Never for the wire types.
pub fn from_legacy_wire<A: serde::Serialize, B: serde::de::DeserializeOwned>(a: &A) -> B {
    serde_json::from_value(serde_json::to_value(a).expect("a wire value serialises")).expect("this crate reads the same wire value")
}
// The root re-exports of the legacy crate whose types moved (an explicit `pub use` shadows the glob).
pub use choice::{AlwaysDecline, Choice, ChoiceOption, Decider, DecisionLog, DecisionRecord, DecisionRecordFields, FirstOption, IllegalChoice, Scripted, SeededRandom, Table, distinct_units, first_of_each, options_from, unit_label, validate};
pub use dice::{Dice, Roll};
pub use fingerprint::{CanonicalHash, CanonicalHashVersion, decision_hash, event_hash};
pub use game::{Game, GameError, RunError, StepResult};
pub use rng::GameRng;
pub use strategy::SecondaryResolution;
pub use timing::Relation;
pub use transit::{Cargo, CargoSource};

// ---- lane E (product edges) and lane F (gates) add no module here ----
