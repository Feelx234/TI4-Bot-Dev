//! The chain after a planet changes hands (package W8, 5a; the string `InvasionWindow::finish_control_gain` up to the exploration), as
//! a [`Window`] that never asks.
//!
//! L1Z1X's Assimilate converts the PDS and the space docks of the other players standing on the planet as it changes hands, before
//! anything else looks at what is standing on it. The capture is recorded for the windows that follow (`last_control_gained`: Infiltrate acts
//! on the planet, Reparations on its former holder), `PLANET_CONTROL_GAINED` is announced (its WHEN and AFTER windows are the abilities of
//! the table), and the rival structures still standing are destroyed: after the announcement from rules version 1
//! (`RULES_INFILTRATE_FIX`: Infiltrate replaces them, so they must stand while it is offered), before it in a version 0 game.
//!
//! Not covered (the adapter returns `None`, the string result stands): a Last Bastion seat, Integrated Economy, the Thunder's Edge legendary
//! planet and Fracture's relic, nested activity the ability table does not carry. The exploration that follows a first claim is the
//! caller's (it asks for the deck and draws).

use crate::choice::{Choice, Pick};
use crate::generated::{Ability, Planet, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::fleet;
use crate::state::{CoreState, EventKind, Key, Payload, Piece, PlanetFlags, Val};
use crate::tables::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one capture.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub invader: Seat,
    pub system: System,
    pub planet: Planet,
    /// The holder before the capture.
    pub previous: Option<Seat>,
    /// `rules_version >= RULES_INFILTRATE_FIX`: the rival structures are destroyed after the announcement.
    pub infiltrate_fix: bool,
    /// The resolver carries the timing machinery (the string `ctx.emit` announces).
    pub announce: bool,
}

/// The window.
#[derive(Debug, Clone, Copy)]
pub struct ControlGained {
    pub f: Facts,
    /// The unit roster, for the form Assimilate places (`None`: an Assimilate seat cannot be followed).
    pub roster: Option<&'static crate::query::tech::UnitRoster>,
    /// Assimilate had to place a unit and the roster was missing.
    pub unsupported: bool,
    stage: Stage,
}

impl ControlGained {
    #[must_use]
    pub const fn new(f: Facts, roster: Option<&'static crate::query::tech::UnitRoster>) -> Self {
        Self { f, roster, unsupported: false, stage: Stage::Start }
    }
}

/// Whether `who` has the faction ability Assimilate (L1Z1X).
#[must_use]
pub fn assimilates(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Assimilate))
}

/// `faction_abilities::control_gained` for Assimilate: "replace each PDS or space dock that belongs to another player on that planet
/// with a PDS or space dock from your reinforcements", in the list order, while the reinforcements hold one of the kind. The unit
/// placed is the seat's own form of the kind (`placed_unit`). The planet's list is written even when nothing is replaced (the string
/// inserts the entry). `false` when a replacement needs a roster that is missing.
fn assimilate(sx: &mut Sx<'_>, f: Facts, roster: Option<&'static crate::query::tech::UnitRoster>) -> bool {
    let scope = Scope(u32::from(sx.kernel.sources.0));
    let standing: Vec<Piece> = sx.state.board().planet(f.planet).units.as_slice().to_vec();
    let mut taken = [0_i64; 2];
    let mut converted: Vec<Piece> = Vec::with_capacity(standing.len());
    for piece in standing {
        let base = piece.kind().data().base_type;
        let slot = match base {
            UnitBaseType::Pds => Some(0),
            UnitBaseType::Spacedock => Some(1),
            _ => None,
        };
        if let (true, Some(slot)) = (piece.owner() != f.invader, slot) {
            let Some(roster) = roster else { return false };
            let Some(own) = crate::kernel::fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, f.invader, base) else {
                return false;
            };
            let room = fleet::remaining_all(sx.state, scope, f.invader, own) - taken[slot];
            if room > 0 {
                taken[slot] += 1;
                converted.push(Piece::new(own, f.invader));
                continue;
            }
        }
        converted.push(piece);
    }
    let cell = sx.state.board_mut().planet_mut(f.planet);
    cell.units.clear();
    for piece in converted {
        cell.units.push(piece);
    }
    cell.flags.set(PlanetFlags::UNITS_ENTRY, true);
    true
}

/// `InvasionWindow::destroy_rival_structures`: the structures of the other players left on the planet are destroyed; no rival ground
/// force can stand here, the invader holds the planet because every one of them is dead.
fn destroy_rival_structures(s: &mut CoreState, f: Facts) {
    let cell = s.board_mut().planet_mut(f.planet);
    let doomed: Vec<Piece> = cell.units.as_slice().iter().copied().filter(|u| u.owner() != f.invader && u.kind().data().structure).collect();
    for piece in doomed {
        if let Some(at) = cell.units.as_slice().iter().position(|u| *u == piece) {
            cell.units.remove(at);
        }
    }
}

impl Window for ControlGained {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        if assimilates(sx.state, f.invader) && !assimilate(sx, f, self.roster) {
            self.unsupported = true;
        }
        if !f.infiltrate_fix {
            destroy_rival_structures(sx.state, f);
        }
        sx.state.transient_mut().combat.last_control_gained = Some((f.system, f.planet, f.invader, f.previous));
        let mut payload = Payload::new().with(Key::Player, Val::Seat(f.invader)).with(Key::Planet, Val::Planet(f.planet)).with(Key::System, Val::System(f.system));
        if let Some(previous) = f.previous {
            payload.set(Key::PreviousOwner, Val::Seat(previous));
        }
        if f.announce {
            let _ = sx.emit(fx::event(EventKind::PlanetControlGained, payload));
        }
        if f.infiltrate_fix {
            destroy_rival_structures(sx.state, f);
        }
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
