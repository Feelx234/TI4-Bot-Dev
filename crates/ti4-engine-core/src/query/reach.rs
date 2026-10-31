//! Reach: which ships of a seat can move into a system, from the typed state and the hook table.
//!
//! Package A2. [`SeatReach`] is the typed `Board::for_player` + `MovementRules::with_laws` +
//! `action_cards::apply_movement_effects` + the `PassSite` hooks of `core_adapter::SeatReach`: the
//! rules one mover carries into every destination, built once per seat and position from `&CoreState`
//! and the kernel's [`HookTable`](crate::hooks::HookTable). [`SeatFleet`] is the destination-free half
//! of `tactical::movable_into` (move values for every boost), and [`SeatFleet::movable`] the loop.
//!
//! The faction modules are reached only through `kernel.hooks.movement`. A hook that does not read the
//! active system is asked once per ship type here; only `may_move_through_ships` (Mentak's Corsair)
//! is asked per destination.
//!
//! What the core cannot see is the source scope (it comes from the [`Kernel`]) and the galaxy of the
//! map slice (the adapter loads it; [`CoreState::galaxy`]).

use std::sync::Arc;

use crate::bits::{SysSet, UnitSet};
use crate::containers::Cow;
use crate::generated::{Breakthrough, Faction, Planet, Relic, System, Technology, TileBack, Unit};
use crate::ids::{Seat, SystemIx, system_at};
use crate::kernel::Kernel;
use crate::hooks::{MoveSite, PassSite};
use crate::keys::MarkKey;
use crate::movement::CoreRules;
use crate::state::{CoreState, Phase, RoundKind, SysFlags};
use crate::tables::{Flags as _, Scope, moves_as_ship};

use super::laws;

/// `laws::blanked`: Law's Order, "all laws are blank until the end of this turn". The Keleres mark
/// holds the turn it was played in; it only counts in the action phase.
#[must_use]
pub fn blanked(core: &CoreState) -> bool {
    core.meta().phase == Phase::Action
        && core
            .marks()
            .tail
            .get(&MarkKey::KeleresLawsBlank())
            .is_some_and(|v| matches!(v, crate::keys::MarkVal::Atom(crate::keys::Atom::Num(n)) if *n == core.meta().turn_seq))
}

/// The Valefar Assimilator cards (`nekro::TOKENS`).
const ASSIMILATORS: [Technology; 2] = [Technology::ValefarAssimilatorX, Technology::ValefarAssimilatorY];

/// `nekro::live_token`: the technology a Valefar Assimilator of `seat` stands for, while another player
/// still owns it.
fn live_token(core: &CoreState, seat: Seat, card: Technology) -> Option<Technology> {
    let p = core.players().by_seat(seat)?;
    if p.faction != Faction::TheNekroVirus || !p.tech.owned.contains(card) {
        return None;
    }
    let target = *p.tech.assimilated.get(&card)?;
    core.players().iter().any(|o| o.seat != seat && o.tech.owned.contains(target)).then_some(target)
}

/// `nekro::assimilators_with_tokens`: how many Valefar Assimilator cards of `seat` have their token on a technology (Nekro Acidos' unlock).
#[must_use]
pub fn assimilators_with_tokens(core: &CoreState, seat: Seat) -> usize {
    ASSIMILATORS.iter().filter(|card| live_token(core, seat, **card).is_some()).count()
}

/// `technology::has_technology_text`: the technology is owned, or carried by a Valefar Assimilator
/// (the gate for what a technology's effect does; not ownership for prerequisites).
#[must_use]
pub fn technology_text(core: &CoreState, seat: Seat, tech: Technology) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(tech))
        || ASSIMILATORS.iter().any(|card| live_token(core, seat, *card) == Some(tech))
}

/// `factions::flagship_has_text`: the unit is that flagship, or it is the Nekro flagship (the Alastor) of a Nekro seat holding Valefar
/// Assimilator Z whose Z token sits on the faction that owns `flagship`. Only text abilities follow; stats never do.
#[must_use]
pub fn flagship_text(core: &CoreState, owner: Seat, unit: Unit, flagship: Unit) -> bool {
    use crate::keys::{Atom, MarkVal};
    if unit == flagship {
        return true;
    }
    if unit != Unit::TheAlastor {
        return false;
    }
    let Some(p) = core.players().by_seat(owner) else { return false };
    if p.faction != Faction::TheNekroVirus || p.cards.breakthrough != Some(Breakthrough::ValefarAssimilatorZ) {
        return false;
    }
    let Some(MarkVal::List(items)) = core.marks().tail.get(&MarkKey::NekroToken(owner)) else { return false };
    items.iter().any(|i| match i {
        MarkVal::Atom(Atom::Faction(f)) => f.data().units.iter().copied().find(|u| u.data().base_type == crate::generated::UnitBaseType::Flagship) == Some(flagship),
        _ => false,
    })
}

/// The scope of the kernel as the tables' mask.
#[must_use]
pub fn scope_of(k: &Kernel) -> Scope {
    Scope(u32::from(k.sources.0))
}

/// `fracture::systems`: the Fracture tiles in the scope, in corpus order (the id order of the tiles).
fn fracture_systems(scope: Scope) -> impl Iterator<Item = System> {
    System::ALL
        .iter()
        .copied()
        .filter(move |s| scope.system(*s).is_some_and(|d| d.tile_back == Some(TileBack::Fracture)))
}

/// `fracture::egress_systems`: the Fracture tiles named "egress".
fn is_egress(s: System) -> bool {
    matches!(s, System::FractureEgressLeft | System::FractureEgressRight)
}

fn link(list: &mut Vec<(SystemIx, SysSet)>, a: SystemIx, b: SystemIx) {
    for (from, to) in [(a, b), (b, a)] {
        match list.iter_mut().find(|(k, _)| *k == from) {
            Some((_, set)) => {
                set.insert(to.index());
            }
            None => {
                let mut set = SysSet::EMPTY;
                set.insert(to.index());
                list.push((from, set));
            }
        }
    }
}

/// `movement::PlayerAdjacency::neighbours`: the systems next to `system` as `seat` sees the map (the wormholes the pieces carry, the links the
/// modules make, minus the borders they block); `None` while the map slice holds no galaxy. The abilities that read "adjacent" in the
/// activation window (Aetherstream, the Ghosts agent) use it.
#[must_use]
pub fn neighbours_for(core: &CoreState, k: &Kernel, seat: Seat, system: SystemIx) -> Option<SysSet> {
    if !core.loaded.has(crate::state::Slice::Map) {
        return None;
    }
    let galaxy = core.galaxy()?;
    let hooks = &k.hooks.movement;
    let extras = hooks.extra_wormhole_set(core);
    let mut found = if extras.is_empty() {
        galaxy.adjacent(system)
    } else {
        let mut copy = (**galaxy).clone();
        for (at, kind) in extras.as_slice() {
            copy.add_token_wormholes(*at, crate::tables::wormhole_bit(*kind));
        }
        copy.adjacent(system)
    };
    for (a, b) in hooks.linked_pairs(core, k, seat).as_slice() {
        if *a == system {
            found.insert(b.index());
        } else if *b == system {
            found.insert(a.index());
        }
    }
    found.remove(system.index());
    for (a, b) in hooks.blocked_pairs(core, seat).as_slice() {
        if *a == system {
            found.remove(b.index());
        } else if *b == system {
            found.remove(a.index());
        }
    }
    Some(found)
}

/// Aetherstream's "adjacent to an anomaly" (`empyrean::adjacent_to_anomaly`): a neighbour of `system` for `seat` is a printed anomaly or a
/// gravity rift a piece made.
#[must_use]
pub fn adjacent_to_anomaly(core: &CoreState, k: &Kernel, seat: Seat, system: SystemIx) -> Option<bool> {
    let neighbours = neighbours_for(core, k, seat, system)?;
    let rifts = k.hooks.movement.gravity_rifts(core);
    let scope = scope_of(k);
    Some(neighbours.iter().any(|i| {
        let other = system_at(i);
        rifts.as_slice().contains(&other)
            || scope.system(other).is_some_and(|d| [crate::tables::SystemFlags::NEBULA, crate::tables::SystemFlags::SUPERNOVA, crate::tables::SystemFlags::ASTEROID_FIELD, crate::tables::SystemFlags::GRAVITY_RIFT].iter().any(|f| d.has(*f)))
    }))
}

/// One seat's movement rules for every destination of one position.
#[derive(Debug, Clone)]
pub struct SeatReach {
    template: CoreRules,
    seat: Seat,
    /// The mover's unit types on the board (space areas), the sites of the pass hooks.
    types: UnitSet,
    /// The types that pass other players' ships wherever they go (hooks that ignore the active system).
    passing_anywhere: UnitSet,
    /// Systems holding any of the mover's units (`own_unit_systems` when a type earns steps).
    own_unit_systems: SysSet,
    /// Whether some module answers `may_move_through_ships` per destination.
    per_destination: bool,
}

impl SeatReach {
    /// The rules of `seat`: `effects` also applies the cards in play (`apply_movement_effects`, what a
    /// real move and `movable_into` use); without, the rules `Observed::can_reach` builds.
    ///
    /// `None` when the map slice holds no galaxy.
    #[must_use]
    pub fn new(core: &CoreState, k: &Kernel, seat: Seat, effects: bool) -> Option<Self> {
        let scope = scope_of(k);
        let hooks = &k.hooks.movement;
        let blanked = blanked(core);
        let base = core.map().adjacency()?;

        // The map as this mover sees it: wormholes the pieces carry, severed systems, restricted kinds.
        let (severed, allowed) = hooks.wormhole_limits(core, seat);
        let extras = hooks.extra_wormhole_set(core);
        let mut adjacency = base.clone();
        let mut mover_core: Option<CoreState> = None;
        if !severed.is_empty() || allowed.is_some() || !extras.is_empty() {
            let mut galaxy = (**core.galaxy()?).clone();
            for (system, kind) in extras.as_slice() {
                galaxy.add_token_wormholes(*system, crate::tables::wormhole_bit(*kind));
            }
            for system in severed.as_slice() {
                galaxy.suppress_wormholes_at(*system);
            }
            if let Some(kinds) = allowed {
                galaxy.retain_wormhole_kinds(kinds.iter().fold(0, |m, w| m | crate::tables::wormhole_bit(w)));
            }
            let moved = galaxy.adjacency();
            if moved != **base {
                adjacency = Arc::new(moved);
            }
            let mut copy = core.clone();
            copy.map.galaxy = Some(Cow::new(galaxy));
            mover_core = Some(copy);
        }
        let seen: &CoreState = mover_core.as_ref().unwrap_or(core);

        // `Board::for_player`.
        let mut enemy_ships = SysSet::EMPTY;
        let mut own_tokens = SysSet::EMPTY;
        for (system, cell) in core.board().systems() {
            if cell.units.as_slice().iter().any(|p| p.owner() != seat && scope.unit(p.kind()).is_some_and(|d| d.ship)) {
                enemy_ships.insert(system.index());
            }
            if cell.cmd.contains(seat) {
                own_tokens.insert(system.index());
            }
        }
        let mut rules = CoreRules::with_board(adjacency, scope, System::ALL[0], enemy_ships, own_tokens);

        // `with_laws`.
        rules.nebulae_open = laws::nebulae_passable(core, blanked);
        let mut extra: Vec<(SystemIx, SysSet)> = Vec::new();
        // `apply_faction_modules`: Antimass Deflectors, then the hooks.
        if technology_text(core, seat, Technology::AntimassDeflectors) {
            rules.asteroid_fields_open = true;
        }
        for (a, b) in hooks.linked_pairs(seen, k, seat).as_slice() {
            link(&mut extra, *a, *b);
        }
        rules.supernovae_open = hooks.may_enter_supernova(core, k, seat);
        rules.supernovae_pass_through = hooks.may_pass_through_supernova(core, k, seat);
        rules.nebulae_ignored = hooks.ignores_nebulae(core, seat);
        for (a, b) in hooks.blocked_pairs(core, seat).as_slice() {
            for (from, to) in [(*a, *b), (*b, *a)] {
                match rules.blocked_edges.iter_mut().find(|(k, _)| *k == from) {
                    Some((_, set)) => {
                        set.insert(to.index());
                    }
                    None => {
                        let mut set = SysSet::EMPTY;
                        set.insert(to.index());
                        rules.blocked_edges.push((from, set));
                    }
                }
            }
        }
        rules.unladen_pass = hooks.unladen_pass(core, seat);
        // Aetherpassage: a system whose only foreign ships belong to players who allow the passage no
        // longer blocks; one that also holds anybody else's still does.
        let owners = hooks.passable_owners(core, seat);
        if !owners.is_empty() {
            let cleared: Vec<usize> = rules
                .enemy_ships
                .iter()
                .filter(|i| {
                    let mut foreign = core
                        .board()
                        .sys(system_at(*i))
                        .units
                        .as_slice()
                        .iter()
                        .filter(|p| p.owner() != seat && scope.unit(p.kind()).is_some_and(|d| d.ship))
                        .peekable();
                    foreign.peek().is_some() && foreign.all(|p| owners.as_slice().contains(&p.owner()))
                })
                .collect();
            for i in cleared {
                rules.enemy_ships.remove(i);
            }
        }
        // Aerie Hololattice: the system may still be entered, but not crossed.
        if !hooks.blocks_passage.is_empty() {
            for (system, cell) in core.board().systems() {
                if cell.flags.has(SysFlags::TOUCHED) && hooks.blocks_passage(core, k, seat, system) {
                    rules.barred_transit.insert(system.index());
                }
            }
        }
        // The pass hooks, once per unit type the mover owns on the board.
        let mut types = UnitSet::EMPTY;
        let mut own_unit_systems = SysSet::EMPTY;
        for (system, cell) in core.board().systems() {
            let mut holds = false;
            for p in cell.units.as_slice().iter().filter(|p| p.owner() == seat) {
                types.insert(p.kind().index());
                holds = true;
            }
            if holds {
                own_unit_systems.insert(system.index());
            }
        }
        for (planet, cell) in core.board().planets() {
            if cell.units.as_slice().iter().any(|p| p.owner() == seat)
                && let Some(system) = super::board::home_system(core, planet)
            {
                own_unit_systems.insert(system.index());
            }
        }
        let mut passing_anywhere = UnitSet::EMPTY;
        for ix in types.iter() {
            let ship = Unit::from_index(ix).expect("a unit index is in range");
            if hooks.may_move_through_ships_anywhere(core, k, seat, ship) {
                passing_anywhere.insert(ix);
            }
            if hooks.own_unit_passage(core, k, PassSite { player: seat, active: System::ALL[0], ship }) {
                rules.own_passage_types.insert(ix);
            }
            let near = hooks.unit_adjacent_systems(core, k, seat, ship);
            if !near.is_empty() {
                let mut set = SysSet::EMPTY;
                for s in near.as_slice() {
                    set.insert(s.index());
                }
                rules.ship_adjacent.push((ship, set));
            }
            if hooks.ignores_command_tokens(core, k, seat, ship) {
                rules.token_free_types.insert(ix);
            }
        }
        if !rules.own_passage_types.is_empty() {
            rules.own_unit_systems = own_unit_systems;
        }
        // The Fracture.
        if core.meta().flags.has(crate::state::MetaFlags::FRACTURE_IN_PLAY) {
            let chain: Vec<System> = fracture_systems(scope).collect();
            for pair in chain.windows(2) {
                link(&mut extra, pair[0], pair[1]);
            }
            for ingress in core.explore().ingress.iter() {
                for egress in chain.iter().copied().filter(|s| is_egress(*s)) {
                    link(&mut extra, ingress, egress);
                }
            }
        }
        extra.sort_by_key(|(k, _)| *k);
        rules.extra_adjacency = extra;

        if effects {
            Self::apply_effects(core, k, seat, &mut rules);
        }
        let per_destination = !hooks.may_move_through_ships.is_empty();
        Some(Self { template: rules, seat, types, passing_anywhere, own_unit_systems, per_destination })
    }

    /// `action_cards::apply_movement_effects`.
    fn apply_effects(core: &CoreState, k: &Kernel, seat: Seat, rules: &mut CoreRules) {
        let hooks = &k.hooks.movement;
        let player = core.players().by_seat(seat);
        rules.rifts_ignored = player.is_some_and(|p| p.cards.relics.as_slice().contains(&Relic::CircletOfTheVoid));
        // `hooks_movement::apply_rift_effects`.
        if hooks.rift_crucible(core, seat) {
            rules.rifts_ignored = true;
            rules.rift_extra_steps = 1;
        }
        for system in hooks.gravity_rifts(core).as_slice() {
            if hooks.rift_roll_exempt(core, seat, *system) {
                rules.rift_roll_exempt.insert(system.index());
            }
            rules.gravity_rift_systems.insert(system.index());
        }
        let Some(p) = player else { return };
        let activation = core.meta().activation_seq;
        if p.tech.owned.contains(Technology::LightWaveDeflector) {
            rules.ignore_enemy_ships = true;
        }
        if p.card_timers.anomalies_ignored_activation.is(activation) {
            rules.anomalies_ignored = true;
        }
        if p.round_marks.contains(RoundKind::DominusOrb, activation) {
            rules.command_tokens_ignored = true;
        }
        if p.card_timers.silence_activation.is(activation) {
            rules.ignore_enemy_ships_from = p.card_timers.silence_system;
        }
    }

    /// The seat these rules are for.
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    /// The rules with no destination yet (the active system is the first system).
    #[must_use]
    pub const fn template(&self) -> &CoreRules {
        &self.template
    }

    /// The rules for moving into `active`: the template with its active system and the pass sets that
    /// read it. The dual run checks this equals the translation of `with_laws` built for `active`.
    #[must_use]
    pub fn rules_for(&self, core: &CoreState, k: &Kernel, active: SystemIx) -> CoreRules {
        let mut rules = self.template.clone();
        rules.active = active;
        rules.passing_ship_types = self.passing_anywhere;
        if self.per_destination {
            for ix in self.types.iter() {
                let ship = Unit::from_index(ix).expect("a unit index is in range");
                if k.hooks.movement.may_move_through_ships(core, k, PassSite { player: self.seat, active, ship }) {
                    rules.passing_ship_types.insert(ix);
                }
            }
        }
        rules
    }

    /// Systems holding any of the mover's units.
    #[must_use]
    pub const fn own_unit_systems(&self) -> &SysSet {
        &self.own_unit_systems
    }
}

/// A ship that can reach the destination (`tactical::Movable`, typed). `index` counts the seat's units
/// in the space area of `origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Movable {
    pub origin: System,
    pub index: u16,
    pub kind: Unit,
    pub capacity: i64,
    pub gravity_drive: bool,
    pub ionian: bool,
}

#[derive(Debug, Clone, Copy)]
struct FleetShip {
    origin: System,
    index: u16,
    kind: Unit,
    capacity: i64,
    /// Move value with (no boost, Gravity Drive, Ionian, both).
    values: [i32; 4],
}

/// `tactical::effective_move_value_for_ship`: the printed move, the Flank Speed bonus, the boosts, the
/// modules' bonus for this ship, then Gravleash's anchor.
#[allow(clippy::too_many_arguments)]
fn ship_value(
    core: &CoreState,
    k: &Kernel,
    seat: Seat,
    origin: System,
    kind: Unit,
    movement: Option<u8>,
    base_type: crate::generated::UnitBaseType,
    index: Option<u16>,
    boosts: (bool, bool),
) -> i32 {
    let player = core.players().by_seat(seat);
    let flank = i32::from(player.is_some_and(|p| p.card_timers.move_bonus_activation.is(core.meta().activation_seq)));
    let own = i32::from(movement.unwrap_or(0))
        + flank
        + i32::from(boosts.0)
        + i32::from(boosts.1)
        + k.hooks.movement.move_bonus(core, MoveSite { player: seat, origin, index, ship: kind });
    if player.is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::GravleashManeuvers))
        && base_type != crate::generated::UnitBaseType::Fighter
    {
        own.max(core.transient().movement.gravleash_move_values.get(&origin).copied().unwrap_or(0))
    } else {
        own
    }
}

/// `tactical::effective_move_value_with_boosts` (no ship index): the move value of a ship type leaving
/// `origin`; `None` for a type outside the scope.
#[must_use]
pub fn effective_move_value(
    core: &CoreState,
    k: &Kernel,
    seat: Seat,
    origin: System,
    kind: Unit,
    boosts: (bool, bool),
) -> Option<i32> {
    let data = scope_of(k).unit(kind)?;
    Some(ship_value(core, k, seat, origin, kind, data.movement, data.base_type, None, boosts))
}

/// `tactical::effective_move_value_for_ship` for the hull at `index` of the seat's units in the space area of `origin`
/// (`begin_one_move` asks the route of the ship it chose with the index, as the offer did).
#[must_use]
pub fn effective_move_value_for_ship(
    core: &CoreState,
    k: &Kernel,
    seat: Seat,
    origin: System,
    kind: Unit,
    index: u16,
    boosts: (bool, bool),
) -> Option<i32> {
    let data = scope_of(k).unit(kind)?;
    Some(ship_value(core, k, seat, origin, kind, data.movement, data.base_type, Some(index), boosts))
}

/// What Gravleash Maneuvers anchors for a ship leaving `origin` (`begin_one_move`): the printed move, the modules'
/// bonus for this hull and the Flank Speed bonus, without the anchor itself or any boost.
#[must_use]
pub fn anchor_move_value(core: &CoreState, k: &Kernel, seat: Seat, origin: System, kind: Unit, index: u16) -> i32 {
    let Some(data) = scope_of(k).unit(kind) else { return 0 };
    let flank = i32::from(
        core.players().by_seat(seat).is_some_and(|p| p.card_timers.move_bonus_activation.is(core.meta().activation_seq)),
    );
    i32::from(data.movement.unwrap_or(0)) + k.hooks.movement.move_bonus(core, MoveSite { player: seat, origin, index: Some(index), ship: kind }) + flank
}

/// One seat's ships that can move, with their move values for every boost: the destination-independent
/// half of `tactical::movable_into`, in the same order.
#[derive(Debug, Clone)]
pub struct SeatFleet {
    ships: Vec<FleetShip>,
    gravity: bool,
    ionian: bool,
}

impl SeatFleet {
    #[must_use]
    pub fn new(core: &CoreState, k: &Kernel, seat: Seat) -> Self {
        let scope = scope_of(k);
        let player = core.players().by_seat(seat);
        let activation = core.meta().activation_seq;
        let mut ships = Vec::new();
        for (origin, cell) in core.board().systems() {
            for (index, hull) in cell.units.as_slice().iter().filter(|p| p.owner() == seat).enumerate() {
                let Some(data) = scope.unit(hull.kind()) else { continue };
                if !moves_as_ship(data) {
                    continue;
                }
                let value = |gd: bool, ion: bool| {
                    ship_value(core, k, seat, origin, hull.kind(), data.movement, data.base_type, Some(index as u16), (gd, ion))
                };
                ships.push(FleetShip {
                    origin,
                    index: index as u16,
                    kind: hull.kind(),
                    capacity: i64::from(data.capacity.unwrap_or(0)),
                    values: [value(false, false), value(true, false), value(false, true), value(true, true)],
                });
            }
        }
        let gravity = player.is_some_and(|p| {
            p.tech.owned.contains(Technology::GravityDrive) && !p.timers.gravity_drive_used_activation.is(activation)
        });
        let ionian = player.is_some_and(|p| !p.cards.legendary_exhausted.contains_key(&Planet::Tempesta))
            && super::board::controls(core, seat, Planet::Tempesta);
        Self { ships, gravity, ionian }
    }

    /// `tactical::movable_into` for the destination of `rules`: every ship that reaches it, the
    /// cheapest boost first (Gravity Drive before the Ionian Fuel Refinery before both).
    #[must_use]
    pub fn movable(&self, rules: &CoreRules) -> Vec<Movable> {
        let mut found = Vec::new();
        let active = rules.active;
        for ship in &self.ships {
            if ship.origin == active {
                continue;
            }
            let reach = |slot: usize| rules.can_reach_ship(ship.origin, ship.values[slot], Some(ship.kind));
            let boosts = if reach(0) {
                Some((false, false))
            } else {
                [((true, false), 1), ((false, true), 2), ((true, true), 3)]
                    .into_iter()
                    .filter(|((gd, ion), _)| (!gd || self.gravity) && (!ion || self.ionian))
                    .find(|(_, slot)| reach(*slot))
                    .map(|(b, _)| b)
            };
            if let Some((gravity_drive, ionian)) = boosts {
                found.push(Movable {
                    origin: ship.origin,
                    index: ship.index,
                    kind: ship.kind,
                    capacity: ship.capacity,
                    gravity_drive,
                    ionian,
                });
            }
        }
        found
    }

    /// The move value of the ship at `index` of `origin` with these boosts, if the seat has it here.
    #[must_use]
    pub fn move_value(&self, origin: System, index: u16, boosts: (bool, bool)) -> Option<i32> {
        let slot = usize::from(boosts.0) + 2 * usize::from(boosts.1);
        self.ships.iter().find(|s| s.origin == origin && s.index == index).map(|s| s.values[slot])
    }
}
