//! Decision types of the `tactical` area: the activation of a system, the movement step and the cargo hold of
//! the ship that moves (LRR 89.1, 89.2, 95). Package A2b. Adapter side: `ti4-engine/src/render/tactical.rs`.
//!
//! Everything an option says about itself is in the option (the unit type of a hull, its damage, the pickup
//! system of a passenger, the slots left): the renderer needs no state to write the string engine's id, label
//! and payload. The diplomacy contact options the string appends to the movement question are not here
//! (structured diplomacy is unsupported; a table with it on stays on the string).

use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;

/// What a unit has to be removed for (`fleet::enforce_seeing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Over {
    /// More non-fighter ships than the fleet supply allows (37.1).
    Supply,
    /// More fighters and ground forces than the ships carry (16.3).
    Capacity,
}

/// The activation hooks that put a use-or-decline question (`faction_techs`, `mahact_units`): the active seat's Spatial Conduit Cylinders, the
/// Nullification Field of another seat, a Mahact Starlancer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hook {
    /// "make the system adjacent to every system with your units" (technology `scc`).
    SpatialConduit,
    /// "exhaust and spend a strategy token to end the active seat's turn" (technology `nf`).
    NullificationField,
    /// "spend the active seat's command token to end their turn" (the Mahact mech).
    Starlancer,
}

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// `seat` picks the system to activate (89.1).
    Activate { seat: Seat },
    /// `seat` picks the next ship to move, or finishes the movement step (89.2).
    Move { seat: Seat },
    /// `seat` fills the hold of `ship`, which moves from `origin`; `free` slots are left (95).
    Load { seat: Seat, ship: Unit, origin: System, free: i8 },
    /// `seat` removes a unit from `system` (it is `over` its limit); the figures are the display facts of the string
    /// question: ships charged against the fleet supply and its limit, capacity consumed and provided.
    Remove { seat: Seat, system: System, over: Over, charged: i32, limit: i32, consumed: i32, transport: i32 },
    /// `seat` may use `hook` after `active` activated `system`.
    Hook { seat: Seat, hook: Hook, active: Seat, system: System },
    /// Scanlink Drone Network: `seat` may explore a planet of `system` that holds its units.
    Scanlink { seat: Seat, system: System },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Activate the system (id: the system id).
    Activate(System),
    /// Move the hull at `index` of the seat's units in the space area of `origin`. `gd` and `ionian` are the
    /// boosts the move spends (the verb of the id); `route` is the route of the offered list the answer names
    /// (0: the default route, no suffix). A hull is offered once per `(kind, damaged, origin, gd, ionian)`.
    Move { origin: System, index: u16, kind: Unit, damaged: bool, capacity: i8, gd: bool, ionian: bool, route: u8 },
    /// Finish the movement step (89.2b).
    DoneMoving,
    /// Take candidate `index` of the hold aboard. `ground` and `fighters` count what is aboard already,
    /// `remaining` the slots left before this pickup.
    Load { index: u16, kind: Unit, source: Option<Planet>, pickup: System, damaged: bool, galvanized: bool, remaining: i8, ground: u8, fighters: u8 },
    /// Carry nothing further. `available` counts the ground forces still to take.
    DoneLoading { ground: u8, fighters: u8, available: u8 },
    /// Remove candidate `index` (a unit `kind` of the seat in the space area); interchangeable units are offered once.
    Remove { index: u16, kind: Unit },
    /// Use the hook (id `use`).
    Use,
    /// Explore this planet (Scanlink Drone Network).
    Explore(Planet),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `activate_system` (rule 89.1).
    ActivateSystem,
    /// `movement_step` (rule 89.2).
    MovementStep,
    /// `load_cargo` (rule 95), about the origin system.
    LoadCargo,
    /// `remove_unit` (the string question carries no context; the dual compares without one).
    RemoveUnit,
    /// `spatial_conduit_link`, `nullification_field_end_turn`, `starlancer_end_turn`.
    Hook(Hook),
    /// `scanlink_explore`.
    ScanlinkExplore,
}
