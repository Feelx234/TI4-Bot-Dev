//! Committing ground forces (LRR 49.2; the loop of the string `commit_ground_forces`), as a [`Window`].
//!
//! Stages: `Choosing` re-reads the invader's ground forces in space and the planets they may land on (the
//! custodians token can come down mid-sequence), and ends the window when nothing is left to land; `Asking`
//! puts "commit ground forces in {system}"; the answer lands the unit (`Landing`) or ends the commitment.
//!
//! Not covered (the adapter returns `None`, the string result stands): the Omniopiares toll, the commit
//! candidates other than the space area (faction hooks with a galaxy), and a ship-form unit that flips on
//! landing (Naaz Eidolon): the window sets `unsupported` and the adapter drops the comparison.

use super::{commit_opts, landable, landable_planets, Rd};
use crate::choice::generic::Rule;
use crate::choice::invasion::{Ask as IAsk, Opt as IOpt, Subtype as ISub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board as qboard;
use crate::query::reach;
use crate::state::CoreState;

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Choosing,
    Asking,
    Done,
}

/// The commitment of `invader` in `system`.
#[derive(Debug, Clone)]
pub struct CommitGround {
    pub invader: Seat,
    pub system: System,
    /// The planets ground forces were committed to, in planet order.
    pub committed: SmallList<Planet, 8>,
    /// A landing the core does not follow happened (a ship-form unit flipping).
    pub unsupported: bool,
    stage: Stage,
}

impl CommitGround {
    #[must_use]
    pub fn new(invader: Seat, system: System) -> Self {
        Self { invader, system, committed: SmallList::new(), unsupported: false, stage: Stage::Choosing }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }
}

impl Window for CommitGround {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Choosing {
            let scope = Self::rd(sx.state, sx.kernel).scope();
            let none_here = qboard::planets_in(sx.state, scope, self.system).is_empty();
            self.stage = if none_here || landable(Self::rd(sx.state, sx.kernel), self.invader, self.system).is_empty() {
                Stage::Done
            } else {
                Stage::Asking
            };
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Invasion(IAsk::CommitGround { system: self.system })),
            Stage::Choosing | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| {
            let rd = Self::rd(s, k);
            let troops = landable(rd, self.invader, self.system);
            let planets = landable_planets(rd, self.system);
            Choice {
                actor: self.invader,
                ask: Ask::Invasion(IAsk::CommitGround { system: self.system }),
                ctx: Ctx {
                    origin: Origin::Rule(Rule::R49),
                    subtype: Subtype::Invasion(ISub::CommitGroundForces),
                    optional: false,
                    target: Some(Target::System(self.system)),
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: commit_opts(&troops, &planets),
            }
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Invasion(IOpt::DoneCommitting)) => self.stage = Stage::Done,
            Pick::Opt(Opt::Invasion(IOpt::Commit { index, planet, .. })) => {
                let troops = landable(Self::rd(sx.state, sx.kernel), self.invader, self.system);
                let piece = *troops.get(usize::from(index)).ok_or(Illegal)?;
                if Self::rd(sx.state, sx.kernel).data(piece.kind()).is_some_and(|d| d.ship) {
                    self.unsupported = true;
                }
                let units = &mut sx.state.board_mut().sys_mut(self.system).units;
                if let Some(at) = units.iter().position(|u| *u == piece) {
                    units.remove(at);
                }
                // The string `system_mut(..).planet_units.entry(..)` creates the planet's entry and touches the tile.
                let cell = sx.state.board_mut().planet_mut(planet);
                cell.units.push(piece);
                cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
                sx.state.board_mut().sys_mut(self.system).flags.set(crate::state::SysFlags::TOUCHED, true);
                if !self.committed.iter().any(|p| *p == planet) {
                    self.committed.push(planet);
                    self.committed.as_mut_slice().sort_unstable();
                }
                self.stage = Stage::Choosing;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}
