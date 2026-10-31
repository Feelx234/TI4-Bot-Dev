//! The combats a faction rule forces at the end of the commitment (LRR 49.2; the string `InvasionWindow::add_forced_combats` and
//! `coexist_instead`), as a [`Window`]: Titans' Coalescence ("Awaken put the invader's units beside another player's units this
//! activation") makes the invader fight on those planets although nothing was committed there, and Slumberstate Computing lets the
//! invader coexist instead, when no other units were committed and the planet belongs to another player who has a ground force there.
//!
//! The planets come from the kernel's `ground.forced_combat_planets` hook, in its order. Each one the commitment does not already hold is
//! either asked (Slumberstate: `Fight` or `Coexist`) or added to the commitment. A coexisting invader writes itself into the planet's
//! coexisting set (coexistence 3.1: the holder keeps control) and the planet is not added. [`ForcedCombats::added`] is the planets added to
//! the commitment, in order.

use super::{is_ground_force_here, Rd};
use crate::choice::generic::Content;
use crate::choice::invasion::{Ask as IAsk, Opt as IOpt, Subtype as ISub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Breakthrough, Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board as qboard, reach};
use crate::state::{CoreState, PlanetFlags};

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// Walking the forced planets.
    Walk,
    /// Waiting on "fight or coexist" for `planets[at]`.
    Asking,
    Done,
}

/// The forced combats of `invader` in `system`, given the planets already committed.
#[derive(Debug, Clone)]
pub struct ForcedCombats {
    pub invader: Seat,
    pub system: System,
    committed: Vec<Planet>,
    /// "If you commit no other units": read before anything is added.
    committed_nothing: bool,
    forced: Vec<Planet>,
    at: usize,
    /// The planets added to the commitment, in order.
    pub added: Vec<Planet>,
    stage: Stage,
}

impl ForcedCombats {
    #[must_use]
    pub fn new(invader: Seat, system: System, committed: Vec<Planet>) -> Self {
        let committed_nothing = committed.is_empty();
        Self { invader, system, committed, committed_nothing, forced: Vec::new(), at: 0, added: Vec::new(), stage: Stage::Start }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// Slumberstate's question exists: the invader holds the breakthrough, another player controls the planet and a rival ground
    /// force stands there. The controller is returned.
    fn holder_to_coexist_with(&self, s: &CoreState, k: &Kernel, planet: Planet) -> Option<Seat> {
        let holds = s.players().by_seat(self.invader).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::SlumberstateComputing));
        if !holds {
            return None;
        }
        let holder = qboard::control(s, planet).filter(|h| *h != self.invader)?;
        let rd = Self::rd(s, k);
        let rival = s.board().planet(planet).units.iter().any(|u| u.owner() != self.invader && is_ground_force_here(rd, u.owner(), self.system, planet, *u));
        rival.then_some(holder)
    }

    fn ask_for(&self, holder: Seat) -> Ask {
        Ask::Invasion(IAsk::CoexistInstead { system: self.system, planet: self.forced[self.at], holder })
    }
}

impl Window for ForcedCombats {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.forced = sx.kernel.hooks.ground.forced_combat_planets(sx.state, sx.kernel, self.invader, self.system).as_slice().to_vec();
            self.stage = Stage::Walk;
        }
        while self.stage == Stage::Walk {
            let Some(planet) = self.forced.get(self.at).copied() else {
                self.stage = Stage::Done;
                break;
            };
            if self.committed.contains(&planet) || self.added.contains(&planet) {
                self.at += 1;
            } else if self.committed_nothing && self.holder_to_coexist_with(sx.state, sx.kernel, planet).is_some() {
                self.stage = Stage::Asking;
            } else {
                self.added.push(planet);
                self.at += 1;
            }
        }
        match self.stage {
            Stage::Asking => match self.forced.get(self.at).and_then(|p| self.holder_to_coexist_with(sx.state, sx.kernel, *p)) {
                Some(holder) => Next::Continue(self.ask_for(holder)),
                None => Next::Done,
            },
            _ => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let planet = *self.forced.get(self.at)?;
        let holder = self.holder_to_coexist_with(s, k, planet)?;
        let mut opts = Opts::default();
        opts.push(Opt::Invasion(IOpt::CoexistFight));
        opts.push(Opt::Invasion(IOpt::CoexistInstead));
        Some(Choice {
            actor: self.invader,
            ask: self.ask_for(holder),
            ctx: Ctx {
                origin: Origin::Content(Content::Breakthrough(Breakthrough::SlumberstateComputing)),
                subtype: Subtype::Invasion(ISub::CoalescenceCoexist),
                optional: false,
                target: Some(Target::Planet { system: self.system, planet }),
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let planet = *self.forced.get(self.at).ok_or(Illegal)?;
        match pick {
            Pick::Opt(Opt::Invasion(IOpt::CoexistFight)) => self.added.push(planet),
            Pick::Opt(Opt::Invasion(IOpt::CoexistInstead)) => {
                // `coexistence::begin` with the holder keeping control (3.1): the newcomer coexists.
                let cell = sx.state.board_mut().planet_mut(planet);
                cell.coexist.insert(self.invader);
                cell.flags.set(PlanetFlags::COEXIST_ENTRY, true);
            }
            _ => return Err(Illegal),
        }
        self.at += 1;
        self.stage = Stage::Walk;
        Ok(())
    }
}
