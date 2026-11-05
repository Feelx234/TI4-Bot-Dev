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

/// `planets::in_system`: every planet printed on the tile (in the source scope, planet order), then every planet
/// placed there during play (planet order).
#[must_use]
pub fn planets_in(core: &CoreState, scope: Scope, system: System) -> PlanetList {
    let mut out = PlanetList::new();
    for planet in system.data().planets {
        if scope.0 & crate::tables::source_bit(planet.data().source) != 0 {
            out.push(*planet);
        }
    }
    // The string side lists the planets in record (id text) order, which is the enum order.
    out.as_mut_slice().sort_unstable();
    for (planet, _) in placed_in(core, system) {
        out.push(planet);
    }
    out
}

/// The planets an effect placed into `system` (with the system), in planet order: the placed set of the board's index, a few cells.
fn placed_in(core: &CoreState, system: System) -> impl Iterator<Item = (Planet, System)> + '_ {
    core.board().planet_index().placed.iter().filter_map(move |i| {
        let planet = Planet::from_index(i)?;
        (core.board().planet(planet).placed_in == Some(system)).then_some((planet, system))
    })
}

/// Every planet whose entries live in this system (printed or placed), scope-free. These are the
/// planets `SystemState::{planet_control, planet_units}` can hold.
pub fn planets_of(core: &CoreState, system: System) -> impl Iterator<Item = Planet> + '_ {
    let placed = &core.board().planet_index().placed;
    system.data().planets.iter().copied().filter(move |p| !placed.contains(p.index())).chain(placed_in(core, system).map(|(p, _)| p))
}

/// Every planet an effect placed into a system (Mirage and the like), with the system, in planet order: one scan of the board. [`planets_of`] runs
/// this scan again for each system it is asked about, which a loop over all the systems turns into systems x planets cell reads; such a loop takes
/// the list once and asks [`planets_of_in`].
#[must_use]
pub fn placed_planets(core: &CoreState) -> Vec<(Planet, System)> {
    core.board().planet_index().placed.iter().filter_map(|i| Planet::from_index(i).and_then(|p| core.board().planet(p).placed_in.map(|s| (p, s)))).collect()
}

/// [`planets_of`] with the board's placed planets given (`placed` is [`placed_planets`] of the same position): the same planets in the same order.
pub fn planets_of_in<'a>(core: &'a CoreState, placed: &'a [(Planet, System)], system: System) -> impl Iterator<Item = Planet> + 'a {
    system.data().planets.iter().copied().filter(move |p| core.board().planet(*p).placed_in.is_none()).chain(placed.iter().filter(move |(_, s)| *s == system).map(|(p, _)| *p))
}

/// The systems where the seat has a unit, in the space area or on one of the system's planets (the planet's home system, [`home_system`]): what a loop over
/// `systems()` finds with "the space area or a planet of the system holds a unit of the seat", taken from the board's indexes.
#[must_use]
pub fn systems_with_units_anywhere(core: &CoreState, seat: Seat) -> SysSet {
    let board = core.board();
    let mut out = board.sys_index().with_units[usize::from(seat.0)];
    for i in board.planet_index().with_units[usize::from(seat.0)].iter() {
        if let Some(system) = Planet::from_index(i).and_then(|planet| home_system(core, planet)) {
            out.insert(system.index());
        }
    }
    out
}

/// The systems whose space area holds a unit of any owner.
#[must_use]
pub fn systems_with_any_units(core: &CoreState) -> SysSet {
    let mut out = SysSet::EMPTY;
    for set in &core.board().sys_index().with_units {
        out.union_with(set);
    }
    out
}

/// The planets holding a unit of any owner.
#[must_use]
pub fn planets_with_any_units(core: &CoreState) -> crate::state::PlanetSet {
    let mut out = crate::state::PlanetSet::EMPTY;
    for set in &core.board().planet_index().with_units {
        out.union_with(set);
    }
    out
}

/// The planets of a set in planet order.
pub fn planets_of_set(set: crate::state::PlanetSet) -> impl Iterator<Item = Planet> {
    let mut rest = set;
    std::iter::from_fn(move || {
        let i = rest.iter().next()?;
        rest.remove(i);
        Planet::from_index(i)
    })
}

/// The systems of a set in board order.
pub fn systems_of(set: SysSet) -> impl Iterator<Item = System> {
    let mut rest = set;
    std::iter::from_fn(move || {
        let i = rest.iter().next()?;
        rest.remove(i);
        System::from_index(i)
    })
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
    for i in core.board().planet_index().controlled[usize::from(seat.0)].iter() {
        let Some(planet) = Planet::from_index(i) else { continue };
        if let Some(system) = core.board().planet(planet).placed_in.or(planet.data().system) {
            out.push((system, planet));
        }
    }
    out.as_mut_slice().sort_unstable();
    out
}

/// The number of planets the seat controls.
#[must_use]
pub fn controlled_count(core: &CoreState, seat: Seat) -> usize {
    core.board().planet_index().controlled[usize::from(seat.0)].len()
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
    core.board().sys_index().with_token[usize::from(seat.0)]
}

/// `GameState::systems_with_units_of`.
#[must_use]
pub fn systems_with_units_of(core: &CoreState, seat: Seat) -> SysSet {
    core.board().sys_index().with_units[usize::from(seat.0)]
}

/// `GameState::systems_with_ships_other_than`: systems whose space area holds a unit of another owner.
#[must_use]
pub fn systems_with_units_other_than(core: &CoreState, seat: Seat) -> SysSet {
    let ix = core.board().sys_index();
    let mut out = SysSet::EMPTY;
    for (other, set) in ix.with_units.iter().enumerate() {
        if other != usize::from(seat.0) {
            out.union_with(set);
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

/// Whether the planet is legendary now: printed so, or made so by the Nano-Forge attached to it.
#[must_use]
pub fn is_legendary(core: &CoreState, planet: Planet) -> bool {
    planet.data().legendary || core.board().planet(planet).attach.as_slice().contains(&crate::generated::Attachment::Nanoforge)
}

/// The attachments on a planet, in list order (`GameState::planet_attachments`).
#[must_use]
pub fn attachments(core: &CoreState, planet: Planet) -> &SmallList<crate::generated::Attachment, 3> {
    &core.board().planet(planet).attach
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `planets_of_in` with the board's placed list lists what `planets_of` lists, for every system, also with planets placed into several systems.
    #[test]
    fn planets_of_in_lists_what_planets_of_lists() {
        let mut s = CoreState::all_loaded();
        for (i, planet) in [Planet::ALL[0], Planet::ALL[40], Planet::ALL[Planet::ALL.len() - 1]].into_iter().enumerate() {
            s.board_mut().planet_mut(planet).placed_in = Some(System::ALL[3 + i * 50]);
        }
        let placed = placed_planets(&s);
        assert_eq!(placed.len(), 3);
        for system in System::ALL {
            let a: Vec<Planet> = planets_of(&s, system).collect();
            let b: Vec<Planet> = planets_of_in(&s, &placed, system).collect();
            assert_eq!(a, b, "{system:?}");
        }
        assert!(placed_planets(&CoreState::all_loaded()).is_empty());
    }
}
