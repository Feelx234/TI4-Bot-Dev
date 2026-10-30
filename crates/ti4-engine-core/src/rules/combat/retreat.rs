//! The move of a retreat (LRR 78.7b/d; string `combat::retreat_to`), as a [`Window`] that never asks.
//!
//! In order: the Alastor participants of the player are forgotten, Bastion notes which ships retreat, the
//! ships with a move value leave under their own power, ground forces and fighters follow while there is
//! capacity for them, what does not fit is lost (the ship-lost note and the unit leaving the space area; with
//! staging, a `retreat_capacity` destruction waits for its announcement), and a command token goes to the
//! destination (with staging, `COMMAND_TOKEN_PLACED` is staged).
//!
//! Since package W3 the window also carries the dual-form units: an Eidolon Maximum on a planet of the system retreats
//! with the fleet while its owner has a ship in the space area (it leaves its planet), a unit that moves like a ship
//! without being one (the Floating Factory) is a mover, and every ship-form dual-form unit of the player in the
//! destination takes its ground form there (Z-Grav Eidolon).
//!
//! Not covered (the adapter returns `None`, the string result stands; see `dual_window::retreat_to`): a unit type the
//! core has no row for, and the Last Bastion.
//!
//! The Bastion note is the direct hook (`bastion_note_retreat`): the observer table's `ShipRetreated` carries
//! no ship list, and a hook call at the same program point is the same bookkeeping.

use super::assign::remove_ship;
use super::Rd;
use crate::choice::{Choice, Pick};
use crate::generated::{System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, PendingDestruction, EventKind, Key, Payload, Piece, SysFlags, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one retreat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub player: Seat,
    pub system: System,
    pub destination: System,
    /// `supply::staging_enabled`.
    pub staging: bool,
    /// The interned `"retreat_capacity"`.
    pub cause: Code,
    /// The interned `"reinforcements"`.
    pub pool: Code,
}

#[derive(Debug, Clone, Copy)]
pub struct Retreat {
    pub f: Facts,
    /// Units that did not fit and were lost.
    pub stranded: usize,
    stage: Stage,
}

impl Retreat {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, stranded: 0, stage: Stage::Start }
    }
}

impl Window for Retreat {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        {
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.direct.nekro_units_alastor_clear(&mut cx, f.system, Some(f.player));
        }
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let scope = rd.scope();
        let mut own: Vec<Piece> = sx.state.board().sys(f.system).units.iter().filter(|p| p.owner() == f.player).copied().collect();
        // The Eidolon Maximum fights from its planet while its owner has a ship in the space area (`planetary_maximum_participates`).
        let mut planetary: Vec<Piece> = Vec::new();
        if own.iter().any(|p| rd.is_ship(p.kind())) {
            for planet in super::planets_of_sorted(sx.state, f.system).iter() {
                planetary.extend(sx.state.board().planet(*planet).units.iter().filter(|u| u.owner() == f.player && u.kind() == Unit::EidolonMaximum).copied());
            }
        }
        own.extend(planetary.iter().copied());
        let (mut movers, mut carried) = (Vec::new(), Vec::new());
        for p in own {
            let Some(d) = rd.data(p.kind()) else { continue };
            if crate::tables::moves_as_ship(d) && d.movement.unwrap_or(0) > 0 {
                movers.push(p);
            } else if d.base_type == UnitBaseType::Fighter || matches!(d.base_type, UnitBaseType::Infantry | UnitBaseType::Mech) {
                carried.push(p);
            }
        }
        let room: i64 = movers.iter().filter_map(|p| rd.data(p.kind())).map(|d| i64::from(d.capacity.unwrap_or(0))).sum();
        let room = usize::try_from(room.max(0)).unwrap_or(0);
        let stranded = carried.split_off(room.min(carried.len()));
        let mut leaving = movers;
        leaving.extend(carried);
        let ships: Vec<Piece> = leaving.iter().filter(|p| rd.is_ship(p.kind())).copied().collect();
        let stranded_ships: Vec<bool> = stranded.iter().map(|p| rd.is_ship(p.kind())).collect();
        {
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.direct.bastion_note_retreat(&mut cx, f.system, f.destination, f.player, &ships);
        }
        // The planet model leaves its planet explicitly (the move below takes from the space area only).
        for p in planetary.iter().filter(|p| leaving.contains(p)) {
            for planet in super::planets_of_sorted(sx.state, f.system).iter() {
                let units = &mut sx.state.board_mut().planet_mut(*planet).units;
                if let Some(at) = units.iter().position(|u| u == p) {
                    units.remove(at);
                    break;
                }
            }
        }
        for p in &leaving {
            let units = &mut sx.state.board_mut().sys_mut(f.system).units;
            if let Some(at) = units.iter().position(|u| u == p) {
                units.remove(at);
            }
        }
        for p in &leaving {
            let cell = sx.state.board_mut().sys_mut(f.destination);
            cell.units.push(*p);
            cell.flags.set(SysFlags::TOUCHED, true);
        }
        // A dual-form unit is a ship only in the active system during its battle: one that retreats takes its ground form
        // again where it lands (`fleet::flip_to_ground_forms`, for the retreating player alone).
        crate::kernel::fx_strategy::flip_space_area(sx.state, scope, &[f.player], f.destination, false);
        for (p, was_ship) in stranded.iter().zip(&stranded_ships) {
            remove_ship(sx, f.system, f.player, *p);
            if f.staging && *was_ship {
                sx.state.transient_mut().combat.pending_destructions.push(PendingDestruction {
                    system: f.system,
                    owner: f.player,
                    unit: p.kind(),
                    reason: f.cause,
                    during_space_combat: true,
                });
            }
        }
        fx::place_command_token(sx.state, f.player, f.destination);
        if f.staging {
            let payload = Payload::new()
                .with(Key::Player, Val::Seat(f.player))
                .with(Key::System, Val::System(f.destination))
                .with(Key::Pool, Val::Code(f.pool));
            fx::stage(sx.state, fx::event(EventKind::CommandTokenPlaced, payload));
        }
        self.stranded = stranded.len();
        self.stage = Stage::Done;
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
