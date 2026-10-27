//! Control, planets of a system, units, tokens, coexistence and neighbours on the typed board
//! (`planets.rs`, `coexistence.rs`, `GameState::{controlled_planets, systems_with_*}`).

use crate::bits::SysSet;
use crate::containers::{SeatSet, SmallList};
use crate::galaxy::Adjacency;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::state::{CoreState, Piece, PlanetFlags};
use crate::tables::Scope;

/// Planets of one system.
pub type PlanetList = SmallList<Planet, 8>;
/// `(system, planet)` pairs in the string order (system text, then planet text).
pub type Controlled = SmallList<(System, Planet), 16>;

/// The system a planet is printed on, whatever the scope (`PlanetData.system`).
#[must_use]
pub fn printed_system(planet: Planet) -> Option<System> {
    planet.data().system
}

/// Where the board keeps the planet's entries: the placed system, else the printed one. The codec
/// guarantees every entry of a planet lives in this system.
#[must_use]
pub fn home_system(core: &CoreState, planet: Planet) -> Option<System> {
    core.board().planet(planet).placed_in.or(planet.data().system)
}

/// `planets::system_of`: a placed planet's tile, else its printed tile (when the planet is in the
/// source scope), else the system that holds an entry of it.
#[must_use]
pub fn system_of(core: &CoreState, scope: Scope, planet: Planet) -> Option<System> {
    let cell = core.board().planet(planet);
    if let Some(system) = cell.placed_in {
        return Some(system);
    }
    let data = planet.data();
    if let Some(system) = data.system
        && scope.0 & crate::tables::source_bit(data.source) != 0
    {
        return Some(system);
    }
    let has_entry = cell.control.is_some() || cell.flags.has(PlanetFlags::UNITS_ENTRY);
    if has_entry { data.system } else { None }
}

/// `planets::in_system`: every planet printed on the tile (in the source scope), then every planet
/// placed there during play (planet order).
#[must_use]
pub fn planets_in(core: &CoreState, scope: Scope, system: System) -> PlanetList {
    let mut out = PlanetList::new();
    for planet in system.data().planets {
        if scope.0 & crate::tables::source_bit(planet.data().source) != 0 {
            out.push(*planet);
        }
    }
    for (planet, cell) in core.board().planets() {
        if cell.placed_in == Some(system) {
            out.push(planet);
        }
    }
    out
}

/// Every planet whose entries live in this system (printed or placed), scope-free. These are the
/// planets `SystemState::{planet_control, planet_units}` can hold.
pub fn planets_of(core: &CoreState, system: System) -> impl Iterator<Item = Planet> + '_ {
    system.data().planets.iter().copied().filter(move |p| core.board().planet(*p).placed_in.is_none()).chain(
        core.board().planets().filter(move |(_, c)| c.placed_in == Some(system)).map(|(p, _)| p),
    )
}

/// Who controls the planet.
#[must_use]
pub fn control(core: &CoreState, planet: Planet) -> Option<Seat> {
    core.board().planet(planet).control
}

/// Whether the seat controls the planet (a coexister does not: see [`controls_for_scoring`]).
#[must_use]
pub fn controls(core: &CoreState, seat: Seat, planet: Planet) -> bool {
    control(core, planet) == Some(seat)
}

/// `GameState::controlled_planets`: `(system, planet)` of every planet the seat controls, sorted by
/// system then planet.
#[must_use]
pub fn controlled_planets(core: &CoreState, seat: Seat) -> Controlled {
    let mut out = Controlled::new();
    for (planet, cell) in core.board().planets() {
        if cell.control == Some(seat)
            && let Some(system) = cell.placed_in.or(planet.data().system)
        {
            out.push((system, planet));
        }
    }
    out.as_mut_slice().sort_unstable();
    out
}

/// The number of planets the seat controls.
#[must_use]
pub fn controlled_count(core: &CoreState, seat: Seat) -> usize {
    core.board().planets().filter(|(_, c)| c.control == Some(seat)).count()
}

/// `SystemState::controls_a_planet`.
#[must_use]
pub fn controls_a_planet_in(core: &CoreState, system: System, seat: Seat) -> bool {
    planets_of(core, system).any(|p| controls(core, seat, p))
}

/// The planet's cell is exhausted (`GameState::exhausted_planets`).
#[must_use]
pub fn exhausted(core: &CoreState, planet: Planet) -> bool {
    core.board().planet(planet).flags.has(PlanetFlags::EXHAUSTED)
}

/// Units in the space area of a system, in list order.
#[must_use]
pub fn space_units(core: &CoreState, system: System) -> &[Piece] {
    core.board().sys(system).units.as_slice()
}

/// Units on a planet, in list order.
#[must_use]
pub fn planet_units(core: &CoreState, planet: Planet) -> &[Piece] {
    core.board().planet(planet).units.as_slice()
}

/// `SystemState::has_units_of`: space area only.
#[must_use]
pub fn has_space_units_of(core: &CoreState, system: System, seat: Seat) -> bool {
    space_units(core, system).iter().any(|u| u.owner() == seat)
}

/// `SystemState::command_tokens` contains the seat.
#[must_use]
pub fn has_token(core: &CoreState, system: System, seat: Seat) -> bool {
    core.board().sys(system).cmd.contains(seat)
}

/// `GameState::systems_with_token`.
#[must_use]
pub fn systems_with_token(core: &CoreState, seat: Seat) -> SysSet {
    let mut out = SysSet::EMPTY;
    for (system, cell) in core.board().systems() {
        if cell.cmd.contains(seat) {
            out.insert(system.index());
        }
    }
    out
}

/// `GameState::systems_with_units_of`.
#[must_use]
pub fn systems_with_units_of(core: &CoreState, seat: Seat) -> SysSet {
    let mut out = SysSet::EMPTY;
    for (system, cell) in core.board().systems() {
        if cell.units.as_slice().iter().any(|u| u.owner() == seat) {
            out.insert(system.index());
        }
    }
    out
}

/// `GameState::systems_with_ships_other_than`: systems whose space area holds a unit of another owner.
#[must_use]
pub fn systems_with_units_other_than(core: &CoreState, seat: Seat) -> SysSet {
    let mut out = SysSet::EMPTY;
    for (system, cell) in core.board().systems() {
        if cell.units.as_slice().iter().any(|u| u.owner() != seat) {
            out.insert(system.index());
        }
    }
    out
}

fn coexist_in(core: &CoreState, system: System, planet: Planet) -> SeatSet {
    if home_system(core, planet) == Some(system) { core.board().planet(planet).coexist } else { SeatSet::EMPTY }
}

/// `coexistence::coexisters`: everyone coexisting on the planet, the controller aside. Empty when
/// `system` is not the planet's own system (the string side reads that system's map).
#[must_use]
pub fn coexisters(core: &CoreState, system: System, planet: Planet) -> SeatSet {
    coexist_in(core, system, planet)
}

/// `coexistence::is_coexisting`.
#[must_use]
pub fn is_coexisting(core: &CoreState, system: System, planet: Planet, seat: Seat) -> bool {
    coexist_in(core, system, planet).contains(seat)
}

/// `coexistence::in_coexistence`.
#[must_use]
pub fn in_coexistence(core: &CoreState, system: System, planet: Planet) -> bool {
    !coexist_in(core, system, planet).is_empty()
}

/// `coexistence::controls_for_scoring` (rule 13).
#[must_use]
pub fn controls_for_scoring(core: &CoreState, system: System, planet: Planet, seat: Seat) -> bool {
    (home_system(core, planet) == Some(system) && controls(core, seat, planet))
        || is_coexisting(core, system, planet, seat)
}

/// Neighbouring systems (`Galaxy::adjacent`), or `None` while the map slice holds no galaxy.
#[must_use]
pub fn neighbours(core: &CoreState, system: System) -> Option<SysSet> {
    core.galaxy().map(|g| g.adjacent(system))
}

/// Every system's neighbours at once, for a run of queries on this board.
#[must_use]
pub fn adjacency(core: &CoreState) -> Option<Adjacency> {
    core.galaxy().map(|g| g.adjacency())
}

/// The attachments on a planet, in list order (`GameState::planet_attachments`).
#[must_use]
pub fn attachments(core: &CoreState, planet: Planet) -> &SmallList<crate::generated::Attachment, 3> {
    &core.board().planet(planet).attach
}
