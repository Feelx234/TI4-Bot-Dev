//! Decision types of the `reaction` area: reaction windows and ability offers (package A5).
//!
//! A reaction window (`WHEN`/`AFTER` an event, LRR 1.19/1.20) asks one seat which of its eligible
//! abilities to resolve, with a decline when any of them is optional. The ask carries the event kind
//! and the relation; the options are [`AbilityId`]s in registration order (the order of the string
//! resolver's bucket), then the generic decline. The adapter renders the prompt `"{when|after} {EVENT}"`,
//! the subtype `reaction_{when|after}_{EVENT}` and the option id (`kind:owner:key:EVENT:relation`).

use crate::generated::ActionCard;
use crate::ids::Seat;
use crate::state::{Code, EventKind};

/// The point relative to an event at which an ability may resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    /// Resolve before the event's ordinary effect.
    When,
    /// Resolve after the event's ordinary effect.
    After,
}

/// The longest scope in which an ability may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Frequency {
    /// Once for each triggering event occurrence.
    OncePerTrigger,
    /// Once during a turn.
    OncePerTurn,
    /// Once during a round.
    OncePerRound,
    /// No cross-window usage limit.
    Unlimited,
}

/// The first segment of an ability id (`ability:`, `leader:`, `reaction:`, ...): what kind of card or
/// rule is behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbilityKind {
    Ability,
    Leader,
    Breakthrough,
    Technology,
    Unit,
    Promissory,
    /// The standing action-card slot of a seat (`reaction:`).
    Reaction,
    /// A relic, a rule or any other id; the key says which.
    Other,
}

/// A registered ability, without its owner: the kind, the card or rule it belongs to (an index into
/// the adapter's key table until the owning package types it) and the clause of that card (`0` for
/// the only one; the string key `empyreanbt_allow` is card `empyreanbt`, clause `allow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbilityId {
    pub kind: AbilityKind,
    pub key: Code,
}

/// What the area asks. Parameters only; the option list is derived by the resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// The window of `relation` around an event of `event`.
    Window { event: EventKind, relation: Relation },
    /// Instinct Training (Xxcha): exhaust it and spend a strategy token to cancel the action card `card`.
    InstinctTraining { card: ActionCard },
    /// I48S (the L1Z1X agent): exhaust it to let `target` (the active player, `None`: nobody is active) replace 1
    /// infantry in the active system with a mech.
    AgentSwap { target: Option<Seat> },
    /// Which action card to play into the window of `relation` around `event` (the reaction slot of a seat holding
    /// two different cards for it).
    PlayCard { event: EventKind, relation: Relation },
    /// Crimson Legionnaire (Mahact; `second`: the II): "gain 1 commodity or convert 1 to a trade good".
    Legionnaire { second: bool },
    /// The Cavalry (a promissory note of the Nomad): which non-fighter ship takes the Nomad flagship's abilities.
    Cavalry,
    /// Technological Singularity (the Nekro): "gain a technology, or assimilate instead" (`nekro::take_from`).
    Assimilate,
    /// The Watcher (Empyrean mech): which of the mechs in reach cancels the action card `actor` played.
    Watcher { actor: Seat },
    /// Salvage Operations (Mentak): produce one ship of a type destroyed in the combat.
    Salvage,
    /// Indoctrination (Yin), at the start of a ground combat on `planet`: spend influence to replace 1 of the opponent's infantry with 1 infantry
    /// (or, with Moyin's Ashes, a mech) of the Yin.
    Indoctrination { planet: crate::generated::Planet },
    /// Icarus Drive (Ghosts mech), after an activation: remove the mech to place or move a Creuss wormhole token into its system.
    IcarusDrive,
    /// Captain Mendosa (Saar agent; `copy`: Ssruu copying it), after an activation: which of the activator's slower ships takes the move value
    /// of the fastest ship on the board.
    AgentShip { copy: bool },
    /// Edict (Mahact), "when you win a combat": whose command token goes into the fleet pool when several opponents qualify.
    Edict,
    /// Terragenesis (Titans), after `planet` was explored: place a sleeper token on it, or move one there.
    Terragenesis { planet: crate::generated::Planet },
    /// A question of a formerly silent ability (lane G).
    G(super::silent::Ask),
}

/// What a Nekro takes of an opponent's technology through an ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Take {
    /// Gain the technology (`gain|{source}|{tech}`).
    Gain,
    /// The X token of the Valefar Assimilator on it (`x|..`).
    TokenX,
    /// The Y token of the Valefar Assimilator on it (`y|..`).
    TokenY,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Resolve this ability.
    Ability(AbilityId),
    /// Use the technology or the agent the question is about.
    Use,
    /// Play this card (the alias of the first copy in the hand).
    Card(ActionCard),
    /// Gain 1 commodity (Crimson Legionnaire).
    Gain,
    /// Convert 1 commodity to a trade good (Crimson Legionnaire).
    Convert,
    /// A ship type (The Cavalry).
    Ship(crate::generated::Unit),
    /// A technology of `source` taken the way `how` says (Technological Singularity).
    Take { how: Take, tech: crate::generated::Technology, source: Seat },
    /// The place of a mech (the Watcher): a system and a planet, `None` for its space area.
    Site { system: crate::generated::System, planet: Option<crate::generated::Planet> },
    /// A ship to produce for `cost` resources (Salvage Operations); the option id is the unit's base type.
    Produce { unit: crate::generated::Unit, cost: i16 },
    /// Replace an infantry of `opponent` with a unit of `base` (the infantry or the mech) for `cost` influence (Indoctrination); the option id is
    /// the base type.
    Indoctrinate { base: crate::generated::UnitBaseType, cost: i16, opponent: crate::ids::Seat },
    /// A Creuss token into a system (Icarus Drive); the option id is `{kind}|{system}`.
    Token(crate::generated::Wormhole, crate::generated::System),
    /// A ship that Captain Mendosa raises from move `own` to move `top`, the `n`th of the question (`ship|{n}`).
    Boost { n: u8, system: crate::generated::System, unit: crate::generated::Unit, own: u8, top: u8 },
    /// An opponent whose token Edict takes (the option id is the seat's text).
    Opponent(Seat),
    /// A sleeper token from the supply (`place`), or the one on the planet `from` (`move|{from}`) (Terragenesis).
    Sleeper { from: Option<crate::generated::Planet> },
    /// An answer of a formerly silent ability (lane G).
    G(super::silent::Opt),
}

/// The machine identifier of the question (`DecisionContext.subtype`): `reaction_{relation}_{EVENT}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    Window { event: EventKind, relation: Relation },
    InstinctTraining,
    AgentSwap,
    PlayCard { event: EventKind, relation: Relation },
    /// `legionnaire_payment`.
    Legionnaire,
    /// `cavalry_ship`.
    Cavalry,
    /// `assimilate`.
    Assimilate,
    /// `watcher_remove`.
    Watcher,
    /// `salvage_produce`.
    Salvage,
    /// `indoctrination_replace`.
    Indoctrination,
    /// `icarus_drive`.
    IcarusDrive,
    /// `agent_ship`.
    AgentShip,
    /// `edict_opponent`.
    Edict,
    /// `terragenesis_token`.
    Terragenesis,
    /// A question of a formerly silent ability (lane G).
    G(super::silent::Subtype),
}
