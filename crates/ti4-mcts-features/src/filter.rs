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
use ti4_engine_core::{Seat, SysSet};

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

/// Where a seat's units with PRODUCTION, SPACE CANNON and DEEP SPACE CANNON stand, and every map system's neighbours: one
/// pass over the board, shared by all the systems of one activation (or one encoding).
#[derive(Debug, Clone)]
pub struct Board {
    pub produce: SysSet,
    pub cannon: SysSet,
    pub deep: SysSet,
    /// The map's systems and their neighbours (same order).
    pub systems: Vec<System>,
    pub adjacent: Vec<SysSet>,
}

impl Board {
    #[must_use]
    pub fn of(seen: &Seen<'_>, me: Seat) -> Self {
        let systems = seen.map_systems();
        let adjacent = seen.adjacent_of_each(&systems);
        let (mut produce, mut cannon, mut deep) = (SysSet::EMPTY, SysSet::EMPTY, SysSet::EMPTY);
        for system in &systems {
            let view = seen.system(*system);
            let planets = view.planets();
            let any = |test: &dyn Fn(Piece) -> bool| mine(view.units(), me, test) || planets.as_slice().iter().any(|p| mine(view.planet(*p).units, me, test));
            if any(&|p| p.kind().data().production.is_some()) {
                produce.insert(system.index());
            }
            if any(&|p| p.kind().data().space_cannon.is_some()) {
                cannon.insert(system.index());
            }
            if any(&|p| p.kind().data().space_cannon.is_some() && p.kind().data().deep_space_cannon) {
                deep.insert(system.index());
            }
        }
        Self { produce, cannon, deep, systems, adjacent }
    }

    /// The neighbours of a system (empty off the map).
    #[must_use]
    pub fn adjacent_to(&self, system: System) -> SysSet {
        self.systems.iter().position(|s| *s == system).map_or(SysSet::EMPTY, |i| self.adjacent[i])
    }

    /// The reasons `system` is worth activating for the board's seat `me` (0: none).
    #[must_use]
    pub fn reasons(&self, seen: &Seen<'_>, me: Seat, system: System) -> u8 {
        let mut why = 0;
        if !seen.movable_into(me, system).is_empty() {
            why |= MOVE;
        }
        if self.produce.contains(system.index()) {
            why |= PRODUCE;
        }
        if self.cannon.contains(system.index()) || self.adjacent_to(system).intersects(&self.deep) {
            why |= SHOOT;
        }
        why
    }
}

/// The reasons `system` is worth activating for `me` (0: none). For many systems build one [`Board`] and ask it.
#[must_use]
pub fn reasons(seen: &Seen<'_>, me: Seat, system: System) -> u8 {
    Board::of(seen, me).reasons(seen, me, system)
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
    Some(activate_on(seen, choice, &Board::of(seen, choice.actor)))
}

/// [`activate`] with the board of the actor already built (`choice` must be an activation).
#[must_use]
pub fn activate_on(seen: &Seen<'_>, choice: &Choice, board: &Board) -> Kept {
    let me = choice.actor;
    let why: Vec<u8> = choice
        .opts
        .iter()
        .map(|o| system_of(o).map_or(MOVE | PRODUCE | SHOOT, |system| board.reasons(seen, me, system)))
        .collect();
    let mut keep: Vec<bool> = why.iter().map(|w| *w != 0).collect();
    let passthrough = !keep.iter().any(|k| *k);
    if passthrough {
        keep.iter_mut().for_each(|k| *k = true);
    }
    Kept { keep, why, passthrough }
}

/// The options a tree node of this choice has: the kept ones for `Activate`, all of them otherwise.
#[must_use]
pub fn options(seen: &Seen<'_>, choice: &Choice) -> Vec<usize> {
    activate(seen, choice).map_or_else(|| (0..choice.opts.len()).collect(), |k| k.indexes())
}
