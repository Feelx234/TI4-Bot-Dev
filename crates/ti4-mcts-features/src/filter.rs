//! The hard filter on `Tactical(Activate)` (docs/MCTS_BOT_PLAN.md 2.5).
//!
//! A system stays an option only if one of these holds for the actor:
//!
//! 1. **Move**: one of my units can move into it (`Seen::movable_into`, the reach the rule bot uses);
//! 2. **Produce**: a unit of mine with PRODUCTION stands in it (space area or one of its planets);
//! 3. **Shoot**: a unit of mine with SPACE CANNON stands in it, or one with DEEP SPACE CANNON (PDS II) in an adjacent system.
//!
//! When nothing survives, every option is kept (never zero options). The same function is used for the data, the training
//! and the search.

use ti4_engine_core::choice::{strategy, tactical, Ask, Choice, Opt};
use ti4_engine_core::generated::System;
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::Piece;
use ti4_engine_core::Seat;

/// Why a system was kept (bits of [`Kept::why`]).
pub const MOVE: u8 = 1;
pub const PRODUCE: u8 = 2;
pub const SHOOT: u8 = 4;

/// The filter's verdict over the options of one `Activate` ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// Per option: kept.
    pub keep: Vec<bool>,
    /// Per option: the reasons ([`MOVE`] | [`PRODUCE`] | [`SHOOT`]); 0 for a dropped option (or a non-system option).
    pub why: Vec<u8>,
    /// Nothing survived, so everything is kept.
    pub passthrough: bool,
}

impl Kept {
    /// Options kept.
    #[must_use]
    pub fn kept(&self) -> usize {
        self.keep.iter().filter(|k| **k).count()
    }

    /// The kept option indexes.
    #[must_use]
    pub fn indexes(&self) -> Vec<usize> {
        self.keep.iter().enumerate().filter(|(_, k)| **k).map(|(i, _)| i).collect()
    }
}

fn mine(units: &[Piece], me: Seat, test: impl Fn(Piece) -> bool) -> bool {
    units.iter().any(|p| p.owner() == me && test(*p))
}

/// Whether one of `me`'s units anywhere in `system` (space or planets) passes `test`.
fn in_system(seen: &Seen<'_>, me: Seat, system: System, test: impl Fn(Piece) -> bool + Copy) -> bool {
    let view = seen.system(system);
    mine(view.units(), me, test) || view.planets().as_slice().iter().any(|p| mine(view.planet(*p).units, me, test))
}

/// The reasons `system` is worth activating for `me` (0: none).
#[must_use]
pub fn reasons(seen: &Seen<'_>, me: Seat, system: System) -> u8 {
    let mut why = 0;
    if !seen.movable_into(me, system).is_empty() {
        why |= MOVE;
    }
    if in_system(seen, me, system, |p| p.kind().data().production.is_some()) {
        why |= PRODUCE;
    }
    let cannon = |p: Piece| p.kind().data().space_cannon.is_some();
    let deep = |p: Piece| p.kind().data().space_cannon.is_some() && p.kind().data().deep_space_cannon;
    if in_system(seen, me, system, cannon) || seen.adjacent(system).iter().filter_map(System::from_index).any(|n| in_system(seen, me, n, deep)) {
        why |= SHOOT;
    }
    why
}

/// The system an activation option names (`Tactical(Activate)` or Thunder's Edge Warfare's `Strategy(System)`).
#[must_use]
pub const fn system_of(o: &Opt) -> Option<System> {
    match o {
        Opt::Tactical(tactical::Opt::Activate(system)) | Opt::Strategy(strategy::Opt::System(system)) => Some(*system),
        _ => None,
    }
}

/// Whether the ask is an activation (the asks the filter applies to).
#[must_use]
pub const fn is_activation(ask: &Ask) -> bool {
    matches!(ask, Ask::Tactical(tactical::Ask::Activate { .. }) | Ask::Strategy(strategy::Ask::TeWarfareTactical))
}

/// The filter of an activation choice; `None` for any other ask. Options that are not a system (none today) are kept.
#[must_use]
pub fn activate(seen: &Seen<'_>, choice: &Choice) -> Option<Kept> {
    if !is_activation(&choice.ask) {
        return None;
    }
    let me = choice.actor;
    let why: Vec<u8> = choice
        .opts
        .iter()
        .map(|o| system_of(o).map_or(MOVE | PRODUCE | SHOOT, |system| reasons(seen, me, system)))
        .collect();
    let mut keep: Vec<bool> = why.iter().map(|w| *w != 0).collect();
    let passthrough = !keep.iter().any(|k| *k);
    if passthrough {
        keep.iter_mut().for_each(|k| *k = true);
    }
    Some(Kept { keep, why, passthrough })
}

/// The options a tree node of this choice has: the kept ones for `Activate`, all of them otherwise.
#[must_use]
pub fn options(seen: &Seen<'_>, choice: &Choice) -> Vec<usize> {
    activate(seen, choice).map_or_else(|| (0..choice.opts.len()).collect(), |k| k.indexes())
}
