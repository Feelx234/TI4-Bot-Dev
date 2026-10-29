//! What the tactical action reads besides the reach of a ship (package A2b): the systems a seat may activate
//! (`tactical::activatable_with`) and the hold of a ship that moves (`transit::loadable`, `transit::CargoWindow`).
//!
//! Pure reads over `&CoreState` and the kernel. Slices: `Meta`, `Players`, `Board`, `Map`, and what the movement
//! hooks read.

use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::hex::Hex;
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::state::{CoreState, Piece, SysFlags};

use super::board;
use super::reach::scope_of;

/// `ti4_model::state::RULES_NEXUS_ACTIVATION`.
pub const RULES_NEXUS_ACTIVATION: u32 = 21;

/// The systems `seat` may activate, in the order of the offer: the printed map in hex order, then the other systems the
/// board has an entry for (the Fracture's, in id order), then the Wormhole Nexus once rules version 21 lets it be
/// activated. `holds_leave`: Mahact's commander lets the seat activate a system holding its token.
///
/// `None` when the map slice holds no galaxy.
#[must_use]
pub fn activatable(core: &CoreState, k: &Kernel, seat: Seat, holds_leave: bool) -> Option<Vec<System>> {
    let galaxy = core.galaxy()?;
    let mut placed: Vec<(Hex, System)> = System::ALL.iter().filter_map(|s| galaxy.coord_of(*s).map(|h| (h, *s))).collect();
    placed.sort_unstable();
    let order = placed
        .into_iter()
        .map(|(_, s)| s)
        .chain(core.board().systems().filter(|(_, c)| c.flags.has(SysFlags::TOUCHED)).map(|(s, _)| s))
        .chain(
            (core.meta().rules_version >= RULES_NEXUS_ACTIVATION
                && galaxy.in_play().contains(System::MalliceLockedNexus.index())
                && galaxy.coord_of(System::MalliceLockedNexus).is_none())
            .then_some(System::MalliceLockedNexus),
        );
    let mut seen = [false; System::COUNT];
    let mut found = Vec::new();
    for system in order {
        if galaxy.is_hyperlane(system) {
            continue;
        }
        if (board::has_token(core, system, seat) && !holds_leave) || seen[system.index()] {
            continue;
        }
        seen[system.index()] = true;
        if k.hooks.movement.cannot_activate(core, k, seat, system) {
            continue;
        }
        found.push(system);
    }
    Some(found)
}

/// One unit that could ride in the hold (`transit::Cargo` with the facts the options and the sums need).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cand {
    pub piece: Piece,
    /// The planet it stands on; `None`: the space area.
    pub source: Option<Planet>,
    /// The system it is picked up from.
    pub system: System,
    pub ground: bool,
    pub fighter: bool,
    /// Transporting it uses no capacity slot (`MovementHooks::free_cargo`).
    pub free: bool,
}

fn consumes(k: &Kernel, unit: Unit) -> bool {
    scope_of(k).unit(unit).is_some_and(|d| matches!(d.base_type, UnitBaseType::Fighter) || (matches!(d.base_type, UnitBaseType::Infantry | UnitBaseType::Mech) && !d.structure))
}

/// `transit::loadable_by`: the seat's units in `origin` it could take aboard, the space area first, then the planets
/// in id order. Fighters and ground forces are not picked up from a system holding the seat's command token other than
/// the active one, unless the ship ignores tokens.
#[must_use]
pub fn loadable(core: &CoreState, k: &Kernel, seat: Seat, origin: System, ignore_tokens: bool) -> Vec<Cand> {
    if !ignore_tokens && core.meta().active_system != Some(origin) && board::has_token(core, origin, seat) {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut push = |piece: Piece, source: Option<Planet>| {
        found.push(Cand { piece, source, system: origin, ground: false, fighter: false, free: false });
    };
    for p in board::space_units(core, origin).iter().filter(|p| p.owner() == seat && consumes(k, p.kind())) {
        push(*p, None);
    }
    let mut planets: Vec<Planet> = board::planets_of(core, origin).collect();
    planets.sort_unstable();
    for planet in planets {
        for p in board::planet_units(core, planet).iter().filter(|p| p.owner() == seat && consumes(k, p.kind())) {
            push(*p, Some(planet));
        }
    }
    found
}

/// The hold of one ship (`transit::CargoWindow`): the candidates, the capacity and what is aboard. Candidates are tracked
/// by index, never by value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hold {
    pub cands: Vec<Cand>,
    pub capacity: i64,
    /// Candidate indices in the order they were taken aboard.
    pub loaded: SmallList<u16, 12>,
    pub closed: bool,
}

/// One pickup option of the hold, with the numbers its payload shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickUp {
    pub index: u16,
    pub cand: Cand,
    pub remaining: i64,
    pub ground: u8,
    pub fighters: u8,
}

impl Hold {
    /// `CargoWindow::for_ship`: the units of the origin, then of each system the route enters (once each), up to the
    /// ship's capacity.
    #[must_use]
    pub fn for_ship(core: &CoreState, k: &Kernel, seat: Seat, origin: System, ship: Piece, path: &[System]) -> Self {
        let data = scope_of(k).unit(ship.kind());
        let capacity = data.and_then(|d| d.capacity).map_or(0, i64::from);
        let ignore_tokens = k.hooks.movement.ignores_command_tokens(core, k, seat, ship.kind());
        let mut cands = loadable(core, k, seat, origin, ignore_tokens);
        let mut seen = vec![origin];
        for step in path {
            if seen.contains(step) {
                continue;
            }
            seen.push(*step);
            cands.extend(loadable(core, k, seat, *step, ignore_tokens));
        }
        // A ship with no capacity carries nothing that needs a slot (only Ral Nel's structures could ride; dropped).
        if capacity <= 0 {
            cands.retain(|c| !consumes(k, c.piece.kind()));
        }
        let any_hook = !k.hooks.movement.free_cargo.is_empty();
        for c in &mut cands {
            let base = scope_of(k).unit(c.piece.kind()).map(|d| d.base_type);
            c.ground = matches!(base, Some(UnitBaseType::Infantry | UnitBaseType::Mech));
            c.fighter = matches!(base, Some(UnitBaseType::Fighter));
            c.free = any_hook && k.hooks.movement.free_cargo(core, k, c.piece);
        }
        let no_free_riders = !cands.iter().any(|c| c.free);
        Self { cands, capacity, loaded: SmallList::new(), closed: capacity <= 0 && no_free_riders }
    }

    fn slots_left(&self) -> i64 {
        let used = self.loaded.iter().filter(|i| !self.cands[usize::from(**i)].free).count();
        self.capacity - i64::try_from(used).unwrap_or(i64::MAX)
    }

    /// Slots still unused.
    #[must_use]
    pub fn free_slots(&self) -> i64 {
        self.slots_left()
    }

    /// Indices still loadable, in candidate order: not aboard, and a slot is left or the unit rides free.
    fn free(&self) -> Vec<u16> {
        let room = self.slots_left() > 0;
        (0..self.cands.len())
            .filter(|i| {
                let i = u16::try_from(*i).unwrap_or(u16::MAX);
                !self.loaded.contains(&i) && (room || self.cands[usize::from(i)].free)
            })
            .map(|i| u16::try_from(i).unwrap_or(u16::MAX))
            .collect()
    }

    /// `CargoWindow::is_complete`.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.closed || self.loaded.len() >= self.cands.len() || (self.slots_left() <= 0 && self.free().is_empty())
    }

    fn count(&self, pick: impl Fn(&Cand) -> bool) -> u8 {
        u8::try_from(self.loaded.iter().filter(|i| pick(&self.cands[usize::from(**i)])).count()).unwrap_or(u8::MAX)
    }

    /// The pickup options of `CargoWindow::pending_choice` (interchangeable units once) and the figures of the close
    /// option `(ground aboard, fighters aboard, ground still to take)`. `None` when the hold is complete or offers nothing.
    #[must_use]
    pub fn options(&self) -> Option<(Vec<PickUp>, (u8, u8, u8))> {
        if self.is_complete() {
            return None;
        }
        let free = self.free();
        if free.is_empty() {
            return None;
        }
        let (ground, fighters) = (self.count(|c| c.ground), self.count(|c| c.fighter));
        let remaining = self.slots_left();
        let mut seen: Vec<(Unit, bool, bool, Option<Planet>, System)> = Vec::new();
        let mut options = Vec::new();
        for index in &free {
            let cand = self.cands[usize::from(*index)];
            let key = (cand.piece.kind(), cand.piece.sustained(), cand.piece.galvanized(), cand.source, cand.system);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            options.push(PickUp { index: *index, cand, remaining, ground, fighters });
        }
        let available = u8::try_from(free.iter().filter(|i| self.cands[usize::from(**i)].ground).count()).unwrap_or(u8::MAX);
        Some((options, (ground, fighters, available)))
    }

    /// `CargoWindow::resolve` for a pickup: whether `index` was offered (it is then aboard).
    pub fn load(&mut self, index: u16) -> bool {
        let Some((options, _)) = self.options() else { return false };
        if !options.iter().any(|o| o.index == index) {
            return false;
        }
        self.loaded.push(index);
        true
    }

    /// `CargoWindow::resolve` for the decline: the hold is closed.
    pub fn close(&mut self) -> bool {
        if self.options().is_none() {
            return false;
        }
        self.closed = true;
        true
    }

    /// What is aboard, in the order it was taken.
    #[must_use]
    pub fn cargo(&self) -> Vec<Cand> {
        self.loaded.iter().map(|i| self.cands[usize::from(*i)]).collect()
    }
}

/// `game::wormhole_hops`: the hops of a route supplied by wormhole adjacency rather than by a hex border. Ability links
/// are classified out; the Ghosts' Quantum Entanglement also joins an alpha to a beta when Nexus Sovereignty or a law
/// turns ordinary wormholes off. `None` when the map slice holds no galaxy.
#[must_use]
pub fn wormhole_hops(core: &CoreState, seat: Seat, origin: System, path: &[System]) -> Option<u32> {
    use crate::tables::WH_ALPHA_BETA;
    let galaxy = core.galaxy()?;
    let mut map = (**galaxy).clone();
    map.extra_links.clear();
    let quantum = core.players().by_seat(seat).is_some_and(|p| p.faction == crate::generated::Faction::TheGhostsOfCreuss);
    let mut quantum_map = map.clone();
    if quantum && map.nexus_wormholes_off {
        quantum_map.nexus_wormholes_off = false;
    }
    let has_alpha_or_beta = |s: System| quantum_map.wormhole_kinds(s) & WH_ALPHA_BETA != 0;
    let distance = |a: System, b: System| -> Option<i32> {
        let (a, b) = (map.coord_of(a)?, map.coord_of(b)?);
        let (dq, dr) = (a.q - b.q, a.r - b.r);
        Some((dq.abs() + dr.abs() + (dq + dr).abs()) / 2)
    };
    let mut previous = origin;
    let mut hops = 0_u32;
    for step in path {
        if *step != previous && distance(previous, *step) != Some(1) {
            let ordinary = map.adjacent(previous).contains(step.index()) && map.wormhole_kinds(previous) != 0 && map.wormhole_kinds(*step) != 0;
            let entangled = quantum && has_alpha_or_beta(previous) && has_alpha_or_beta(*step);
            hops += u32::from(ordinary || entangled);
        }
        previous = *step;
    }
    Some(hops)
}
