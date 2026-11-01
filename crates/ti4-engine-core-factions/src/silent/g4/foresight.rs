//! naalu: Foresight (`factions::naalu::foresight`): "After another player moves ships into a system that contains your ships: spend 1 token from your strategy pool to move your ships from that system into an adjacent
//! system that does not contain another player's ships". Optional; registered for every seat (`ability:*:foresight:MOVEMENT_FINISHED:after`), the condition names a Naalu seat with a strategy token and a
//! destination. The destinations are the neighbours of the active system for the Naalu (the links and the blocked borders of its faction hooks) that hold no token of the Naalu, can be entered under the rules
//! of its movement (laws, technologies, effects in play), hold no other player's ship, and are on the map (the string tries the relocation on a scratch copy). The ships go with everything of the Naalu in the
//! space area (the fighters and ground forces aboard), the strategy token is spent (announced when the game steps, staged) and placed in the new system, and `COMMAND_TOKEN_PLACED` (pool `strategy`) is announced
//! at once: its window opens inside this one.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt, Subtype as USub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Ability, Faction, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::query::{board, reach};
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload, Piece, Val};

use super::code;

/// Foresight (`ability:*:foresight:MOVEMENT_FINISHED:after`).
pub const FORESIGHT: Slot = Slot(44_007);

/// `ships_in`: the ships of `who` in the space area of `system`.
fn ships_in(s: &CoreState, who: Seat, system: System) -> Vec<Piece> {
    board::space_units(s, system).iter().filter(|p| p.owner() == who && p.kind().data().ship).copied().collect()
}

/// `PlayerAdjacency::neighbours`: the neighbours of `system` for `who` -- the map's (with the wormholes the modules add), the links of its faction hooks, less the borders it does not treat as adjacent.
fn neighbours(s: &CoreState, k: &Kernel, who: Seat, system: System) -> Vec<System> {
    let Some(galaxy) = s.galaxy() else { return Vec::new() };
    let extras = k.hooks.movement.extra_wormhole_set(s);
    let mut found: Vec<System> = if extras.is_empty() {
        galaxy.adjacent(system).iter().filter_map(System::from_index).collect()
    } else {
        let mut copy = (**galaxy).clone();
        for (at, kind) in extras.as_slice() {
            copy.add_token_wormholes(*at, ti4_engine_core::tables::wormhole_bit(*kind));
        }
        copy.adjacent(system).iter().filter_map(System::from_index).collect()
    };
    for (a, b) in k.hooks.movement.linked_pairs(s, k, who).as_slice() {
        if *a == system {
            found.push(*b);
        } else if *b == system {
            found.push(*a);
        }
    }
    found.retain(|other| *other != system);
    let blocked = k.hooks.movement.blocked_pairs(s, who);
    found.retain(|other| {
        let edge = if system < *other { (system, *other) } else { (*other, system) }; // mutants:skip (`<=`: a system is never its own neighbour)
        !blocked.as_slice().contains(&edge)
    });
    found.sort_unstable();
    found.dedup();
    found
}

/// The relocation of `transit::relocate_ships` is legal into `to`: both systems are on the map, `to` can be entered under the mover's rules and holds no other player's ships.
fn can_relocate(s: &CoreState, k: &Kernel, who: Seat, from: System, to: System) -> bool {
    let Some(galaxy) = s.galaxy() else { return false }; // mutants:skip (no destinations are listed without a galaxy)
    let on_map = |system: System| galaxy.coord_of(system).is_some() || galaxy.wormhole_systems().contains(system.index());
    if !on_map(from) || !on_map(to) {
        return false;
    }
    // (`effects` true as the string applies the cards in play; none of them changes what a system admits.)
    let enterable = reach::SeatReach::new(s, k, who, true).is_some_and(|r| r.rules_for(s, k, to).can_enter(to)); // mutants:skip (`true`: the cards in play do not change `can_enter`)
    enterable && !board::space_units(s, to).iter().any(|p| p.owner() != who && p.kind().data().ship)
}

/// `foresight_destinations`.
fn destinations(s: &CoreState, k: &Kernel, who: Seat, from: System) -> Vec<System> {
    if ships_in(s, who, from).is_empty() {
        return Vec::new();
    }
    neighbours(s, k, who, from).into_iter().filter(|to| !board::has_token(s, *to, who)).filter(|to| can_relocate(s, k, who, from, *to)).collect()
}

/// `foresight_window`: the system the ships leave and where they may go.
fn window(s: &CoreState, k: &Kernel, who: Seat, event: &ti4_engine_core::state::EventRec) -> Option<(System, Vec<System>)> {
    let mover = event.payload.seat(Key::Player)?;
    if mover == who || s.players().by_seat(who).is_none_or(|p| p.faction != Faction::TheNaaluCollective) {
        return None;
    }
    if event.payload.int(Key::ShipsMoved).unwrap_or(0) <= 0 {
        return None;
    }
    let from = event.payload.system(Key::System)?;
    if s.players().by_seat(who).is_none_or(|p| p.econ.strategic_tokens <= 0) {
        return None;
    }
    let destinations = destinations(s, k, who, from);
    (!destinations.is_empty()).then_some((from, destinations))
}

fn condition(c: &CondCx<'_>) -> bool {
    window(c.state, c.kernel, c.def.owner, c.event).is_some()
}

fn foresight(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let ready = {
        let (s, k) = eff.read();
        window(s, k, me, eff.event)
    };
    let Some((from, destinations)) = ready else { return Ok(()) };
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::U(UAsk::Foresight { from }))),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Ability(Ability::Foresight)),
            subtype: Subtype::Reaction(RSub::G(GSub::U(USub::Foresight))),
            optional: false,
            target: Some(Target::System(from)),
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: destinations.iter().map(|to| Opt::Reaction(ROpt::G(GOpt::U(UOpt::Destination(*to))))).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::U(UOpt::Destination(to))))) = eff.ask(&choice) else { return Ok(()) };
    if !destinations.contains(&to) {
        return Ok(());
    }
    let reason = eff.codes().silent.g4[code::NAALU];
    let pool = eff.codes().silent.g4[code::STRATEGY];
    {
        let s = eff.state();
        // The ships first, then everything else of the Naalu's in the space area (the fighters and ground forces aboard).
        let ships = ships_in(s, me, from);
        let units = &mut s.board_mut().sys_mut(from).units;
        for ship in &ships {
            if let Some(at) = units.iter().position(|p| p == ship) {
                units.remove(at);
            }
        }
        let carried: Vec<Piece> = units.iter().copied().filter(|p| p.owner() == me).collect();
        units.retain(|p| p.owner() != me);
        let there = &mut s.board_mut().sys_mut(to).units;
        for piece in ships.into_iter().chain(carried) {
            there.push(piece);
        }
        for system in [from, to] {
            s.board_mut().sys_mut(system).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
        }
        fx::spend_strategy_token_staged(s, me, reason);
        fx::place_command_token(s, me, to);
    }
    let payload = Payload::new().with(Key::Player, Val::Seat(me)).with(Key::System, Val::System(to)).with(Key::Pool, Val::Code(pool));
    eff.emit(fx::event(EventKind::CommandTokenPlaced, payload))?;
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(FORESIGHT, AbilityKind::Ability, EventKind::MovementFinished, Relation::After, foresight).optional().when(condition));
}
