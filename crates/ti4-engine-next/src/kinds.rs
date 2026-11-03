//! The constants of the product surface (docs/todo/D2-8-wrapper.md, lane B / 8d): the choice kinds, option ids and id prefixes
//! the clients and bots name. One module per legacy module the value came from, so `kinds::production::PAY_KIND` is the
//! native twin of `ti4_engine_legacy::production::PAY_KIND`; `tests/surface_parity.rs` compares every one with its original.
//!
//! The values are wire facts (a client matches on them, a recorded decision log names them): changing one is a format change.
//! Each is a copy, never a re-export, so this crate names no string rule module.

/// `action_cards.rs` / `reactions.rs`.
pub mod action_cards {
    /// The choice kind for discarding down to the hand limit.
    pub const DISCARD_KIND: &str = "discard";
    /// The choice kind for taking a revealed card.
    pub const TAKE_REVEALED_KIND: &str = "take_revealed_card";
    /// The choice kind for picking one of your own cards.
    pub const OWN_HAND_KIND: &str = "own_hand_card";
    /// The choice kind the oracle offers reaction cards under (`reactions::ACTION_CARD_KIND`).
    pub const ACTION_CARD_KIND: &str = "action_card";
}

/// `combat.rs`.
pub mod combat {
    /// The choice kind for cancelling a hit with Sustain Damage.
    pub const SUSTAIN_KIND: &str = "sustain";
    /// The choice kind for assigning a hit to one of your own units.
    pub const CASUALTY_KIND: &str = "casualty";
    /// The choice kind for announcing a retreat.
    pub const RETREAT_KIND: &str = "retreat";
    /// The choice kind for picking where to retreat to.
    pub const RETREAT_TO_KIND: &str = "retreat_to";
}

/// `diplomacy/candidates.rs` (the feature is dropped; the kind stays a wire name).
pub mod diplomacy {
    /// The choice kind that opens a diplomacy exchange.
    pub const OPEN_KIND: &str = "open_diplomacy";
}

/// `draft.rs`.
pub mod draft {
    /// The choice kind of the strategy-card draft.
    pub const STRATEGY_CARD_KIND: &str = "strategy_card";
}

/// `game.rs`.
pub mod game {
    /// OP-08: carry on with a tactical action after a pause to negotiate.
    pub const CONTINUE_ACTION_ID: &str = "continue_action";
    /// The kind of the same option.
    pub const CONTINUE_ACTION_KIND: &str = "continue_action";
    /// Emitted when a secondary-preview window closes on a planning fork.
    pub const SECONDARY_PREVIEW_COMPLETE: &str = "STRATEGY_SECONDARY_PREVIEW_COMPLETE";
    /// OP-08: the explicit end of an action-phase turn.
    pub const END_TURN_ID: &str = "end_turn";
    /// The kind of the same option.
    pub const END_TURN_KIND: &str = "end_turn";
    /// The action id that opens a tactical action.
    pub const TACTICAL_ACTION_ID: &str = "tactical";
}

/// `invasion.rs`.
pub mod invasion {
    /// The choice kind for committing a ground force to a planet.
    pub const COMMIT_KIND: &str = "commit";
    /// The choice kind for choosing which of your own ground forces dies.
    pub const GROUND_CASUALTY_KIND: &str = "ground_casualty";
}

/// `neutral_units.rs`.
pub mod neutral_units {
    /// The owner id carried by every neutral unit.
    pub const NEUTRAL: &str = "neutral";
}

/// `objectives.rs`.
pub mod objectives {
    /// Ten victory points wins (LRR 98).
    pub const VICTORY_TARGET: i32 = 10;
    /// The choice kind of scoring an objective.
    pub const SCORE_KIND: &str = "score";
}

/// `production.rs`.
pub mod production {
    /// The choice kind for producing one unit.
    pub const PRODUCE_KIND: &str = "produce";
    /// The choice kind for exhausting a planet to pay.
    pub const PAY_KIND: &str = "pay";
    /// The choice kind for placing a produced unit.
    pub const PLACE_KIND: &str = "place";
    /// The id standing for a system's space area.
    pub const SPACE: &str = "space";
    /// Separates the system from the spot in a placement outside the producing system (`"<system>@<planet or space>"`).
    pub const REMOTE_SEPARATOR: char = '@';
    /// The id of the option that exhausts Berekar Berekon.
    pub const AGENT_OPTION_ID: &str = "exhaust_agent|winnuagent";
    /// Kind of the option that exhausts Berekar Berekon for -2 on this use's combined cost.
    pub const AGENT_OPTION_KIND: &str = "production_agent";
    /// What may be produced at all. Structures arrive through Construction, not PRODUCTION.
    pub const BUILDABLE: [&str; 9] = ["fighter", "infantry", "carrier", "cruiser", "destroyer", "dreadnought", "mech", "flagship", "warsun"];
    /// A war sun cannot be produced without the technology that unlocks it (67.x).
    pub const UNLOCKED_BY: [(&str, &str); 1] = [("warsun", "ws")];
}

/// `promissory.rs`.
pub mod promissory {
    /// Support for the Throne is keyed by its owner rather than by an alias.
    pub const SUPPORT_PREFIX: &str = "support:";
}

/// `relics.rs`.
pub mod relics {
    /// The Shard of the Throne, which is worth a victory point simply for being held.
    pub const SHARD: &str = "shard";
}

/// `secondary_preview.rs`.
pub mod secondary_preview {
    /// How many answers one preview may be given.
    pub const MAX_ANSWERS: usize = 48;
}

/// `secrets.rs`.
pub mod secrets {
    /// 45.4: three in hand, counting scored ones.
    pub const HAND_LIMIT: usize = 3;
}

/// `seating.rs` (the rest of the module is setup, lane C).
pub mod seating {
    /// Mecatol Rex's tiles: defined once, in `crate::seating` (lane C).
    pub use crate::seating::{MECATOL, MECATOL_TE};
}

/// `starting_tech.rs`.
pub mod starting_tech {
    /// The kind of its options.
    pub const KIND: &str = "starting_technology";
}

/// `strategy.rs`.
pub mod strategy {
    /// Prefix of an option that follows a secondary without spending a strategy token (full id `follow|waived|<module index>|<waiver id>`).
    pub const WAIVED_SECONDARY_PREFIX: &str = "follow|waived|";
    /// The choice kind for a strategic-action secondary response.
    pub const STRATEGY_KIND: &str = "strategy";
    /// The choice kind for an ordinary action-phase action.
    pub const ACTION_KIND: &str = "action";
    /// The historical action id used when a player has exactly one strategic action available.
    pub const STRATEGIC_ACTION_ID: &str = "strategic";
}

/// `strategy_cards.rs`.
pub mod strategy_cards {
    /// Influence one command token is worth on the Leadership primary.
    pub const INFLUENCE_PER_TOKEN: i64 = 3;
    /// The choice kind of the Technology card's research.
    pub const RESEARCH_KIND: &str = "research";
}

/// `tactical.rs`.
pub mod tactical {
    /// The detail key that carries a move option's routes.
    pub const ROUTES_DETAIL: &str = "routes";
    /// The choice kind for activating a system.
    pub const ACTIVATE_KIND: &str = "activate";
    /// The choice kind for moving one ship into the active system.
    pub const MOVE_KIND: &str = "move";
    /// How many routes one ship is offered at most (the default included).
    pub const MAX_ROUTES: usize = 5;
}

/// `timing.rs`.
pub mod timing {
    /// The detail key that names the active player of a window.
    pub const ACTIVE_DETAIL: &str = "active";
}

/// `transactions.rs`.
pub mod transactions {
    /// The choice kind of a transaction answer.
    pub const ANSWER_KIND: &str = "transaction";
    /// Prefix of a free-form transaction id.
    pub const FREE_FORM_PREFIX: &str = "x;";
    /// The choice kind of an offer.
    pub const OFFER_KIND: &str = "offer";
    /// The choice kind that opens a transaction.
    pub const OPEN_KIND: &str = "open_transaction";
}

/// `transit.rs`.
pub mod transit {
    /// The choice kind for loading a unit into a ship's hold.
    pub const LOAD_KIND: &str = "load";
    /// A rift roll of this or less removes the ship from the board (41.2).
    pub const RIFT_DESTROYS_ON: u32 = 3;
}
