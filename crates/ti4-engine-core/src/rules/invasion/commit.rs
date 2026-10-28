//! Committing ground forces (LRR 49.2; the loop of the string `commit_ground_forces`), as a [`Window`].
//!
//! Stages: `Choosing` re-reads the invader's ground forces in space and the planets they may land on (the
//! custodians token can come down mid-sequence), and ends the window when nothing is left to land; `Asking`
//! puts "commit ground forces in {system}"; the answer lands the unit (`Landing`) or ends the commitment.
//!
//! Not covered (the adapter returns `None`, the string result stands): the Omniopiares toll, the commit
//! candidates other than the space area (faction hooks with a galaxy), and a ship-form unit that flips on
//! landing (Naaz Eidolon): the window sets `unsupported` and the adapter drops the comparison.

use super::{commit_opts_with, landable, landable_planets, Rd, Troop};
use crate::choice::generic::Rule;
use crate::choice::invasion::{Ask as IAsk, Opt as IOpt, Subtype as ISub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit};
use crate::state::Piece;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::{EventKind, Key, Payload, Val};
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

    /// The commit question of the position.
    #[must_use]
    pub fn question(&self, s: &CoreState, k: &Kernel) -> Choice {
        self.question_of(s, k, None)
    }

    /// The commit question over `pool` (the string `commit_pool`: space troops, temporary commits and a faction's
    /// candidates with their origins) or, with `None`, over the ground forces standing in the space area.
    #[must_use]
    pub fn question_of(&self, s: &CoreState, k: &Kernel, pool: Option<&[Troop]>) -> Choice {
        self.question_with(s, k, pool, &[])
    }

    /// [`Self::question_of`] with the planets a Crimson Revenant may be deployed to.
    #[must_use]
    pub fn question_with(&self, s: &CoreState, k: &Kernel, pool: Option<&[Troop]>, deploy: &[Planet]) -> Choice {
        let rd = Self::rd(s, k);
        let own: Vec<Troop>;
        let troops = match pool {
            Some(p) => p,
            None => {
                own = landable(rd, self.invader, self.system).into_iter().map(Troop::space).collect();
                &own
            }
        };
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
            opts: commit_opts_with(self.system, troops, &planets, deploy),
        }
    }

    pub(crate) fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
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
        (self.stage == Stage::Asking).then(|| self.question(s, k))
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
                if !self.committed.contains(&planet) {
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

/// One landing of the commit step: the answer `commit|<index>|<planet>` of the string
/// `InvasionWindow::commit_one` for an ordinary candidate (a ground force standing in the space area, no toll,
/// no faction origin). The window asks the commit question, lands the chosen unit, records it as the last
/// committed unit and announces `UNITS_COMMITTED` (with the controller of the planet, when it has one).
///
/// Not covered (the adapter returns `None`): a toll (Keleres), commit candidates with an origin, temporary space
/// commits, the Crimson deploy, a ship-form unit flipping on landing, the diplomacy promises
/// (`HostileEngagement`) and any reaction to the announcement.
#[derive(Debug, Clone)]
pub struct LandOne {
    pub invader: Seat,
    pub system: System,
    /// A landing the core does not follow happened (a ship-form unit flipping).
    pub unsupported: bool,
    /// The resolver carries the timing machinery (the string `ctx.emit` announces).
    pub announce: bool,
    /// The pool the string `commit_pool` built (`None`: the ground forces standing in the space area).
    pub pool: Option<Vec<Troop>>,
    /// The troop the answer landed.
    pub landed: Option<Troop>,
    /// The planets a Crimson Revenant may be deployed to (offered after the troops' landings).
    pub deploy: Vec<Planet>,
    /// The answer was a deploy (the Revenant came from reinforcements).
    pub deployed: bool,
    stage: Stage,
}

impl LandOne {
    #[must_use]
    pub const fn new(invader: Seat, system: System, announce: bool) -> Self {
        Self { invader, system, unsupported: false, announce, pool: None, landed: None, deploy: Vec::new(), deployed: false, stage: Stage::Choosing }
    }

    /// The same window offering the Crimson deploy on `planets`.
    #[must_use]
    pub fn with_deploy(mut self, planets: Vec<Planet>) -> Self {
        self.deploy = planets;
        self
    }

    /// The same window over the string side's pool, which may hold troops with an origin and temporary commits.
    #[must_use]
    pub fn with_pool(mut self, pool: Vec<Troop>) -> Self {
        self.pool = Some(pool);
        self
    }
}

impl Window for LandOne {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Choosing {
            let empty = match &self.pool {
                Some(pool) => pool.is_empty(),
                None => landable(CommitGround::rd(sx.state, sx.kernel), self.invader, self.system).is_empty(),
            } && self.deploy.is_empty();
            self.stage = if empty { Stage::Done } else { Stage::Asking };
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Invasion(IAsk::CommitGround { system: self.system })),
            Stage::Choosing | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| CommitGround::new(self.invader, self.system).question_with(s, k, self.pool.as_deref(), &self.deploy))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        if let Pick::Opt(Opt::Invasion(IOpt::Deploy { planet })) = pick {
            if !self.deploy.contains(&planet) {
                return Err(Illegal);
            }
            let piece = Piece::new(Unit::Revenant, self.invader);
            self.deployed = true;
            sx.state.transient_mut().combat.last_committed_unit = Some((self.invader, self.system, planet, piece));
            let cell = sx.state.board_mut().planet_mut(planet);
            cell.units.push(piece);
            cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
            sx.state.board_mut().sys_mut(self.system).flags.set(crate::state::SysFlags::TOUCHED, true);
            let mut payload = Payload::new()
                .with(Key::System, Val::System(self.system))
                .with(Key::Planet, Val::Planet(planet))
                .with(Key::Player, Val::Seat(self.invader));
            if let Some(holder) = qboard::control(sx.state, planet) {
                payload.set(Key::Controller, Val::Seat(holder));
            }
            if self.announce {
                let _ = sx.emit(fx::event(EventKind::UnitsCommitted, payload));
            }
            self.stage = Stage::Done;
            return Ok(());
        }
        let Pick::Opt(Opt::Invasion(IOpt::Commit { index, planet, .. })) = pick else { return Err(Illegal) };
        let troop = match &self.pool {
            Some(pool) => *pool.get(usize::from(index)).ok_or(Illegal)?,
            None => Troop::space(*landable(CommitGround::rd(sx.state, sx.kernel), self.invader, self.system).get(usize::from(index)).ok_or(Illegal)?),
        };
        let piece = troop.piece;
        if !troop.temporary && CommitGround::rd(sx.state, sx.kernel).data(piece.kind()).is_some_and(|d| d.ship) {
            self.unsupported = true;
        }
        self.landed = Some(troop);
        sx.state.transient_mut().combat.last_committed_unit = Some((self.invader, self.system, planet, piece));
        match troop.origin {
            None => {
                let units = &mut sx.state.board_mut().sys_mut(self.system).units;
                if let Some(at) = units.iter().position(|u| *u == piece) {
                    units.remove(at);
                }
            }
            Some((from_system, from_planet)) => {
                let units = &mut sx.state.board_mut().planet_mut(from_planet).units;
                if let Some(at) = units.iter().position(|u| *u == piece) {
                    units.remove(at);
                }
                sx.state.board_mut().sys_mut(from_system).flags.set(crate::state::SysFlags::TOUCHED, true);
            }
        }
        let cell = sx.state.board_mut().planet_mut(planet);
        cell.units.push(piece);
        cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
        sx.state.board_mut().sys_mut(self.system).flags.set(crate::state::SysFlags::TOUCHED, true);
        let mut payload = Payload::new()
            .with(Key::System, Val::System(self.system))
            .with(Key::Planet, Val::Planet(planet))
            .with(Key::Player, Val::Seat(self.invader));
        if let Some((from_system, from_planet)) = troop.origin {
            payload.set(Key::FromSystem, Val::System(from_system));
            payload.set(Key::FromPlanet, Val::Planet(from_planet));
        }
        if let Some(holder) = qboard::control(sx.state, planet) {
            payload.set(Key::Controller, Val::Seat(holder));
        }
        if self.announce {
            let _ = sx.emit(fx::event(EventKind::UnitsCommitted, payload));
        }
        self.stage = Stage::Done;
        Ok(())
    }
}
