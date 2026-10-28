//! The move of a retreat (LRR 78.7b/d; string `combat::retreat_to`), as a [`Window`] that never asks.
//!
//! In order: the Alastor participants of the player are forgotten, Bastion notes which ships retreat, the
//! ships with a move value leave under their own power, ground forces and fighters follow while there is
//! capacity for them, what does not fit is lost (the ship-lost note and the unit leaving the space area; with
//! staging, a `retreat_capacity` destruction waits for its announcement), and a command token goes to the
//! destination (with staging, `COMMAND_TOKEN_PLACED` is staged).
//!
//! Not covered (the adapter returns `None`, the string result stands; see `dual_window::retreat_to`): a Naaz
//! player (Eidolon forms flip back where they land; a Voltron on a planet retreats with the fleet), a unit
//! that moves like a ship without being one (Floating Factory), a unit type the core has no row for.
//!
//! The Bastion note is the direct hook (`bastion_note_retreat`): the observer table's `ShipRetreated` carries
//! no ship list, and a hook call at the same program point is the same bookkeeping.

use super::assign::remove_ship;
use super::Rd;
use crate::choice::{Choice, Pick};
use crate::generated::{System, UnitBaseType};
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
        let own: Vec<Piece> = sx.state.board().sys(f.system).units.iter().filter(|p| p.owner() == f.player).copied().collect();
        let (mut movers, mut carried) = (Vec::new(), Vec::new());
        for p in own {
            let Some(d) = rd.data(p.kind()) else { continue };
            if d.ship && d.movement.unwrap_or(0) > 0 {
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
