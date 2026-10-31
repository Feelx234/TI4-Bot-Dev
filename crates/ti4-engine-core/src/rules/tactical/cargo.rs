//! The cargo hold of the ship that moves (`transit::CargoWindow`), the sail with its gravity-rift rolls
//! (`Game::sail`, `transit::survives_gravity_rifts`, `transit::apply_move`) and the arrival event
//! (`Game::note_arrival`).

use crate::choice::generic::Rule;
use crate::choice::tactical::{Ask, Opt, Subtype};
use crate::choice::{Choice, Opt as COpt, Opts, Target};
use crate::containers::SmallList;
use crate::generated::Unit;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::fx_move;
use crate::kernel::stage::{Illegal, Sx};
use crate::query::reach::SeatReach;
use crate::query::transit;
use crate::state::{EventKind, Key, List, Payload, RngDomain, Tally, Val};

use super::{Loading, Machine, Plain, Stage, Step};

/// A rift roll of this or less removes the ship from the board (41.2); the dice have ten sides (`dice::SIDES`).
const RIFT_DESTROYS_ON: u32 = 3;
const DICE_SIDES: u32 = 10;

/// `CargoWindow::pending_choice`.
pub(super) fn ask(l: &Loading, seat: Seat) -> Option<Choice> {
    let (picks, (ground, fighters, available)) = l.hold.options()?;
    let free = i8::try_from(l.hold.free_slots()).unwrap_or(i8::MAX);
    let about = crate::choice::Ask::Tactical(Ask::Load { seat, ship: l.ship.kind(), origin: l.origin, free });
    let mut opts = Opts::default();
    for p in picks {
        opts.0.push(COpt::Tactical(Opt::Load {
            index: p.index,
            kind: p.cand.piece.kind(),
            source: p.cand.source,
            pickup: p.cand.system,
            damaged: p.cand.piece.sustained(),
            galvanized: p.cand.piece.galvanized(),
            remaining: i8::try_from(p.remaining).unwrap_or(i8::MAX),
            ground: p.ground,
            fighters: p.fighters,
        }));
    }
    opts.0.push(COpt::Tactical(Opt::DoneLoading { ground, fighters, available }));
    Some(Choice { actor: seat, ask: about, ctx: super::ctx(Rule::R95, Subtype::LoadCargo, Some(Target::System(l.origin))), opts })
}

pub(super) fn answer(m: &mut Machine, sx: &mut Sx<'_>, opt: Opt) -> Result<(), Illegal> {
    let mut l = m.load.take().ok_or(Illegal)?;
    let ok = match opt {
        Opt::Load { index, .. } => l.hold.load(index),
        Opt::DoneLoading { .. } => l.hold.close(),
        _ => false,
    };
    if !ok {
        m.load = Some(l);
        return Err(Illegal);
    }
    if l.hold.is_complete() {
        let lost = sail(m, sx, &l)?;
        m.set_stage(Stage::Moving);
        m.step = Some(Step::Sailed { lost });
    } else {
        m.load = Some(l);
        m.step = Some(Step::Loaded);
    }
    Ok(())
}

/// Roll the route's gravity rifts, then move the ship or lose it; then the ion storm, the arrival and the plain event.
/// Returns whether the ship was lost.
pub(super) fn sail(m: &mut Machine, sx: &mut Sx<'_>, l: &Loading) -> Result<bool, Illegal> {
    let seat = m.seat;
    let active = sx.state.meta().active_system.ok_or(Illegal)?;
    let reach = SeatReach::new(sx.state, sx.kernel, seat, true).ok_or(Illegal)?;
    let rules = reach.rules_for(sx.state, sx.kernel, active);
    // Nav Suite and the Circlet of the Void spare the roll (41.2).
    let mut survives = true;
    if !(rules.anomalies_ignored || rules.rifts_ignored) {
        let exited = l.path.split_last().map_or(&[][..], |(_, before)| before);
        for system in exited {
            if rules.is_gravity_rift(*system) && !rules.rift_roll_exempt.contains(system.index()) {
                let face = sx.die(RngDomain::Dice, DICE_SIDES);
                m.dice.push(face);
                if face <= RIFT_DESTROYS_ON {
                    survives = false;
                    break;
                }
            }
        }
    }
    let cargo = l.hold.cargo();
    if survives {
        fx_move::move_ship(sx.state, l.origin, active, l.ship);
        for c in &cargo {
            fx_move::take_aboard(sx.state, c.system, active, c.piece, c.source);
        }
    } else {
        fx_move::destroy_ship(sx.state, l.origin, l.ship);
        for c in &cargo {
            fx_move::lose_passenger(sx.state, c.system, c.piece, c.source);
        }
    }
    fx_move::flip_ion_storm(sx.state, l.origin, active);
    if survives {
        arrived(m, sx, l, &cargo, active);
        m.note(sx, Plain::ShipMoved);
    } else {
        m.note(sx, Plain::ShipLost);
    }
    Ok(!survives)
}

/// `Game::note_arrival`: count the arrival and announce it with what arrived, the origin, the route and its wormhole hops.
fn arrived(m: &mut Machine, sx: &mut Sx<'_>, l: &Loading, cargo: &[transit::Cand], active: crate::generated::System) {
    let seat = m.seat;
    fx_move::count_arrival(sx.state);
    let mut payload = Payload::new().with(Key::Player, Val::Seat(seat)).with(Key::System, Val::System(active));
    let mut counts: Vec<(Seat, Unit, u32)> = Vec::new();
    for piece in std::iter::once(l.ship).chain(cargo.iter().map(|c| c.piece)) {
        match counts.iter_mut().find(|(o, u, _)| *o == piece.owner() && *u == piece.kind()) {
            Some(entry) => entry.2 += 1,
            None => counts.push((piece.owner(), piece.kind(), 1)),
        }
    }
    counts.sort_by_key(|(o, u, _)| (*o, u.index()));
    payload.units = counts.into_iter().map(|(owner, unit, count)| Tally { owner, unit, count }).collect();
    payload.set(Key::Origin, Val::System(l.origin));
    payload.set(Key::Unit, Val::Unit(l.ship.kind()));
    payload.set_list(Key::Path, List::Systems(l.path.iter().copied().collect::<SmallList<_, 8>>()));
    if let Some(hops) = transit::wormhole_hops(sx.state, seat, l.origin, &l.path) {
        payload.set(Key::Wormholes, Val::Int(i64::from(hops)));
    }
    let _ = m.emit(sx, fx::event(EventKind::ShipMoved, payload));
    m.note(sx, Plain::Typed(EventKind::ShipMoved));
}
