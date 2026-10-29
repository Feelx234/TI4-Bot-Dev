//! The movement step: the question (`tactical::movement_options` over `movable`) and the selection of one ship
//! (`Game::begin_one_move`), which opens its hold or sails at once.

use crate::choice::generic::Rule;
use crate::choice::tactical::{Ask, Opt, Subtype};
use crate::choice::{Choice, Opt as COpt, Opts};
use crate::generated::{Breakthrough, System, Unit};
use crate::ids::Seat;
use crate::kernel::fx_move;
use crate::kernel::stage::{Illegal, Sx};
use crate::kernel::Kernel;
use crate::query::reach::{self, SeatFleet, SeatReach};
use crate::query::board;
use crate::state::{CoreState, Piece};

use super::{Loading, Machine, Stage, Step};
use crate::query::transit::Hold;

/// The hull at `index` among the seat's units of the space area of `origin`.
fn hull(core: &CoreState, seat: Seat, origin: System, index: u16) -> Option<Piece> {
    board::space_units(core, origin).iter().filter(|p| p.owner() == seat).nth(usize::from(index)).copied()
}

/// The question of the movement step: one option per distinguishable move, then "finish movement". `None` when the map
/// slice holds no galaxy.
pub(super) fn ask(core: &CoreState, k: &Kernel, seat: Seat) -> Option<Choice> {
    let mut opts = Opts::default();
    if let Some(active) = core.meta().active_system {
        let rules = SeatReach::new(core, k, seat, true)?.rules_for(core, k, active);
        let mut seen: Vec<(Unit, bool, System, bool, bool)> = Vec::new();
        for found in SeatFleet::new(core, k, seat).movable(&rules) {
            let piece = hull(core, seat, found.origin, found.index)?;
            let key = (found.kind, piece.sustained(), found.origin, found.gravity_drive, found.ionian);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            opts.0.push(COpt::Tactical(Opt::Move {
                origin: found.origin,
                index: found.index,
                kind: found.kind,
                damaged: piece.sustained(),
                capacity: i8::try_from(found.capacity).unwrap_or(i8::MAX),
                gd: found.gravity_drive,
                ionian: found.ionian,
                route: 0,
            }));
        }
    }
    opts.0.push(COpt::Tactical(Opt::DoneMoving));
    Some(Choice {
        actor: seat,
        ask: crate::choice::Ask::Tactical(Ask::Move { seat }),
        ctx: super::ctx(Rule::R89_2, Subtype::MovementStep, None),
        opts,
    })
}

pub(super) fn answer(m: &mut Machine, sx: &mut Sx<'_>, opt: Opt) -> Result<(), Illegal> {
    match opt {
        Opt::DoneMoving => {
            m.step = Some(Step::Finished);
            Ok(())
        }
        Opt::Move { origin, index, kind, damaged, capacity, gd, ionian, route } => {
            // The answer must be one of the offered options (the route suffix is not part of the option).
            let offered = ask(sx.state, sx.kernel, m.seat).ok_or(Illegal)?;
            let base = Opt::Move { origin, index, kind, damaged, capacity, gd, ionian, route: 0 };
            if !offered.opts.0.contains(&COpt::Tactical(base)) {
                return Err(Illegal);
            }
            begin_one_move(m, sx, origin, index, (gd, ionian), route)
        }
        _ => Err(Illegal),
    }
}

/// `Game::begin_one_move`: the route, the spent boosts, Gravleash's anchor, the hold, and the sail when nothing is to
/// be loaded.
fn begin_one_move(m: &mut Machine, sx: &mut Sx<'_>, origin: System, index: u16, boosts: (bool, bool), route: u8) -> Result<(), Illegal> {
    let seat = m.seat;
    let active = sx.state.meta().active_system.ok_or(Illegal)?;
    let ship = hull(sx.state, seat, origin, index).ok_or(Illegal)?;
    let reach = SeatReach::new(sx.state, sx.kernel, seat, true).ok_or_else(|| {
        m.bailed = Some(super::Bail::NoGalaxy);
        Illegal
    })?;
    let rules = reach.rules_for(sx.state, sx.kernel, active);
    let (path, gd, ionian) = if route == 0 {
        let value = reach::effective_move_value_for_ship(sx.state, sx.kernel, seat, origin, ship.kind(), index, boosts).ok_or(Illegal)?;
        let path = rules.path_from_ship(origin, value, Some(ship.kind())).ok_or(Illegal)?;
        (path, boosts.0, boosts.1)
    } else {
        // The routes of the offer are the string's list (display facts); the adapter names the chosen one.
        let Some(pick) = m.route.take() else {
            m.bailed = Some(super::Bail::Route);
            return Err(Illegal);
        };
        if pick.path.first() != Some(&origin) || pick.path.last() != Some(&active) {
            return Err(Illegal);
        }
        (pick.path, pick.gd, pick.ionian)
    };
    if gd && !fx_move::use_gravity_drive(sx.state, seat) {
        return Err(Illegal);
    }
    if ionian && !fx_move::use_ionian(sx.state, seat) {
        return Err(Illegal);
    }
    if sx.state.players().by_seat(seat).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::GravleashManeuvers)) {
        let own = reach::anchor_move_value(sx.state, sx.kernel, seat, origin, ship.kind(), index);
        fx_move::anchor_gravleash(sx.state, origin, own);
    }
    let hold = Hold::for_ship(sx.state, sx.kernel, seat, origin, ship, &path);
    if hold.is_complete() {
        let lost = super::cargo::sail(m, sx, &Loading { origin, ship, path, hold })?;
        m.set_stage(Stage::Moving);
        m.step = Some(Step::Sailed { lost });
    } else {
        m.load = Some(Loading { origin, ship, path, hold });
        m.set_stage(Stage::Loading);
        m.step = Some(Step::Selected);
    }
    Ok(())
}
