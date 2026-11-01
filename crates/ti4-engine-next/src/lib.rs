//! The product-facing engine crate. Facade stage: everything not declared here is re-exported from
//! `ti4-engine-legacy` (the directory `crates/ti4-engine`, docs/todo/D2-7-legacy-split.md). A module declared in this
//! file shadows the glob re-export of the same name; the crate is filled module by module (docs/todo/D2-8-wrapper.md).
//!
//! One section per lane (section 6.2 of the plan): a lane edits only its own section.

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
    pub use crate::ties::action_cards::name_of;
    pub use ti4_engine_legacy::action_cards::*;
}
pub mod combat {
    pub use crate::kinds::combat::{CASUALTY_KIND, RETREAT_KIND, RETREAT_TO_KIND, SUSTAIN_KIND};
    pub use ti4_engine_legacy::combat::*;
}
pub mod diplomacy {
    pub use ti4_engine_legacy::diplomacy::*;
    pub mod candidates {
        pub use crate::kinds::diplomacy::OPEN_KIND;
        pub use ti4_engine_legacy::diplomacy::candidates::*;
    }
}
pub mod exploration {
    pub use crate::queries::content::{attachment_for, resolution, traits_of};
    pub use ti4_engine_legacy::exploration::*;
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
    pub use ti4_engine_legacy::invasion::*;
}
pub mod objectives {
    pub use crate::kinds::objectives::{SCORE_KIND, VICTORY_TARGET};
    pub use crate::queries::content::{leader, points_for, winner};
    pub use ti4_engine_legacy::objectives::*;
}
pub mod production {
    pub use crate::kinds::production::{AGENT_OPTION_ID, AGENT_OPTION_KIND, BUILDABLE, PAY_KIND, PLACE_KIND, PRODUCE_KIND, REMOTE_SEPARATOR, SPACE, UNLOCKED_BY};
    pub use crate::queries::production::{capacity, placements, price_of_under, trade_good_worth};
    pub use ti4_engine_legacy::production::*;
}
pub mod promissory {
    pub use crate::kinds::promissory::SUPPORT_PREFIX;
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
pub mod secondary_preview {
    pub use crate::kinds::secondary_preview::MAX_ANSWERS;
    pub use ti4_engine_legacy::secondary_preview::*;
}
pub mod secrets {
    pub use crate::kinds::secrets::HAND_LIMIT;
    pub use crate::queries::content::scored_count;
    pub use ti4_engine_legacy::secrets::*;
}
pub mod strategy {
    pub use crate::kinds::strategy::{ACTION_KIND, STRATEGIC_ACTION_ID, STRATEGY_KIND, WAIVED_SECONDARY_PREFIX};
    pub use ti4_engine_legacy::strategy::*;
}
pub mod strategy_cards {
    pub use crate::kinds::strategy_cards::{INFLUENCE_PER_TOKEN, RESEARCH_KIND};
    pub use crate::ties::strategy_cards::card_name;
    pub use ti4_engine_legacy::strategy_cards::*;
}
pub mod tactical {
    pub use crate::kinds::tactical::{ACTIVATE_KIND, MAX_ROUTES, MOVE_KIND, ROUTES_DETAIL};
    pub use ti4_engine_legacy::tactical::*;
}
pub mod technology {
    pub use crate::queries::technology::{active_aliases, colour_type, faction_of, gravity_drive_available, is_unit_upgrade, name, prerequisites, replaced_for_faction, COLOURS};
    pub use ti4_engine_legacy::technology::*;
}
pub mod timing {
    pub use crate::kinds::timing::ACTIVE_DETAIL;
    pub use ti4_engine_legacy::timing::*;
}
pub mod transactions {
    pub use crate::kinds::transactions::{ANSWER_KIND, FREE_FORM_PREFIX, OFFER_KIND, OPEN_KIND};
    pub use ti4_engine_legacy::transactions::*;
}
pub mod transit {
    pub use crate::kinds::transit::{LOAD_KIND, RIFT_DESTROYS_ON};
    pub use ti4_engine_legacy::transit::*;
}

// ---- lane C (setup, 8c): setup, seating, deck, opening, starting_tech, map_feasibility, fixtures ----

// ---- lane D (wrapper, 8e/8f): game ----

// ---- lane E (product edges) and lane F (gates) add no module here ----
