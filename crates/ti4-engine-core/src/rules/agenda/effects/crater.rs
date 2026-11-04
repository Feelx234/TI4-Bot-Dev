//! Ixthian Artifact, For (rules version 36; the printed card): "The speaker rolls 1 die. If the result is 6-10, each player may research 2
//! technologies. If the result is 1-5, destroy all units in Mecatol Rex's system, and each player with units in systems adjacent to Mecatol Rex's
//! system destroys 3 of their own units in each of those systems."
//!
//! * 6-10: each player in speaker order is asked, as the agenda's question, for up to two free researches (`Research::agenda`, a child window;
//!   prerequisites apply, a decline ends that player's turn at it).
//! * 1-5: every unit in the Mecatol Rex system is destroyed (ships, ground forces and structures, whoever owns them); then, system by system
//!   (by system number) and player by player (speaker order), a player with more than 3 units in an adjacent system chooses the 3 that are
//!   destroyed, one question each (`Ask::Crater`); a player with 3 or fewer loses them all, and a choice among units that are all the same is
//!   not asked.
//!
//! The string effect (`agenda_effects::effect_artifact`) destroys every ship that cannot sustain damage in every system and defers the research:
//! a known string bug (S6, `docs/STRING_ENGINE_BUGS.md`), which the core does not copy.

use super::{Resolve, Stage};
use crate::choice::agenda::{Ask as AAsk, Opt as AOpt};
use crate::choice::{Ask, Choice, Opt, Opts, Origin, Pick};
use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{Report, Spawn};
use crate::kernel::stage::{Illegal, Ran, Sx};
use crate::kernel::Next;
use crate::query::board;
use crate::state::{CoreState, Piece};
use crate::tables::Scope;

/// The most units a player destroys in one adjacent system.
const LOSS: u8 = 3;

/// Where the destruction of the adjacent systems stands: the system being served (its place in [`near`]) and the units still to destroy there for the
/// seat at `Resolve::cursor` (zero: that seat has not started).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct At {
    pub(super) system: u8,
    pub(super) left: u8,
}

impl At {
    pub(super) const START: Self = Self { system: 0, left: 0 };
}

/// The Mecatol Rex system of the map: the Thunder's Edge tile when the galaxy has it, else the base tile.
fn mecatol(core: &CoreState) -> System {
    if core.galaxy().is_some_and(|g| g.coord_of(System::MecatolRex112).is_some()) { System::MecatolRex112 } else { System::MecatolRex18 }
}

/// The systems adjacent to Mecatol Rex's, in system order (none while the position holds no galaxy).
fn near(core: &CoreState) -> SmallList<System, 12> {
    let home = mecatol(core);
    let mut out = SmallList::new();
    if let Some(set) = board::neighbours(core, home) {
        for at in set.iter() {
            let system = crate::ids::system_at(at);
            if system != home {
                out.push(system);
            }
        }
    }
    out.as_mut_slice().sort_unstable();
    out
}

/// A unit of `who` in `system` and where it stands (`None`: the space area), in list order: the space area first, then the planets.
fn held(core: &CoreState, scope: Scope, system: System, who: Seat) -> Vec<(Option<Planet>, Piece)> {
    let mut out: Vec<(Option<Planet>, Piece)> = core.board().sys(system).units.iter().filter(|p| p.owner() == who).map(|p| (None, *p)).collect();
    for planet in board::planets_in(core, scope, system).as_slice() {
        out.extend(core.board().planet(*planet).units.iter().filter(|p| p.owner() == who).map(|p| (Some(*planet), *p)));
    }
    out
}

/// One unit is destroyed where it stands. A ground force on a planet is staged as destroyed (the hooks that react to it).
fn destroy(core: &mut CoreState, system: System, planet: Option<Planet>, piece: Piece) -> bool {
    match planet {
        None => {
            let units = &mut core.board_mut().sys_mut(system).units;
            let Some(at) = units.iter().position(|u| *u == piece) else { return false };
            units.remove(at);
        }
        Some(planet) => {
            let units = &mut core.board_mut().planet_mut(planet).units;
            let Some(at) = units.iter().position(|u| *u == piece) else { return false };
            units.remove(at);
            if matches!(piece.kind().data().base_type, UnitBaseType::Infantry | UnitBaseType::Mech) {
                crate::kernel::fx_ground::stage_destroyed(core, system, planet, piece, crate::keys::DestroyReason::Agenda(Agenda::IxthianArtifact));
            }
        }
    }
    true
}

impl Resolve {
    /// The effect: the die, then the researches or the craters.
    pub(super) fn crater(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Apply => {
                    let face = sx.die(crate::state::RngDomain::Dice, super::DIE_SIDES);
                    self.faces.push(face);
                    let Some(facts) = &self.facts else {
                        self.unsupported = true;
                        self.stage = Stage::Done;
                        return Next::Done;
                    };
                    self.cursor = 0;
                    if face >= 6 {
                        if facts.roster.is_none() {
                            self.unsupported = true;
                            self.stage = Stage::Done;
                            return Next::Done;
                        }
                        self.stage = Stage::Drawing;
                    } else {
                        let scope = facts.scope;
                        Self::destroy_mecatol(sx.state, scope);
                        self.crater_at = At::START;
                        self.stage = Stage::Asking;
                    }
                }
                Stage::Drawing => {
                    if let Some(next) = self.research_step(sx) {
                        return next;
                    }
                }
                Stage::Asking => {
                    let scope = self.facts.as_ref().map_or(Scope(0), |f| f.scope);
                    match self.crater_next(sx.state, scope) {
                        Some((_, system, left)) => return Next::Continue(Ask::Agenda(AAsk::Crater { system, left })),
                        None => self.stage = Stage::Done,
                    }
                }
                Stage::Done => return Next::Done,
            }
        }
    }

    /// A roll of 6-10: the next player in speaker order may research up to two technologies (a child window), until every player has. `None`: the
    /// loop goes on.
    fn research_step(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        if self.draw.running() {
            match self.draw.step(sx) {
                Some(Ran::Asks(a)) => return Some(Next::Continue(a)),
                Some(Ran::Pass(next)) => return Some(next),
                Some(Ran::Done(Report::Left)) => {
                    self.unsupported = true;
                    self.stage = Stage::Done;
                    return Some(Next::Done);
                }
                Some(Ran::Done(_)) | None => self.cursor += 1,
            }
            return None;
        }
        let order = crate::query::seats::speaker_order(sx.state);
        let Some(&seat) = order.as_slice().get(usize::from(self.cursor)) else {
            self.stage = Stage::Done;
            return Some(Next::Done);
        };
        let Some((scope, Some(roster))) = self.facts.as_ref().map(|f| (f.scope, f.roster)) else {
            self.unsupported = true;
            self.stage = Stage::Done;
            return Some(Next::Done);
        };
        match sx.table.links.start(&Spawn::AgendaResearch { who: seat, scope, roster, agenda: Agenda::IxthianArtifact, times: 2 }) {
            Some(window) => self.draw.start(window),
            None => {
                self.unsupported = true;
                self.stage = Stage::Done;
                return Some(Next::Done);
            }
        }
        None
    }

    /// Every unit in the Mecatol Rex system is destroyed: the space area, then each planet (the ground forces staged as destroyed).
    fn destroy_mecatol(core: &mut CoreState, scope: Scope) {
        let system = mecatol(core);
        let ships: Vec<Piece> = core.board().sys(system).units.iter().copied().collect();
        for piece in ships {
            destroy(core, system, None, piece);
        }
        for planet in board::planets_in(core, scope, system).as_slice() {
            let standing: Vec<Piece> = core.board().planet(*planet).units.iter().copied().collect();
            for piece in standing {
                destroy(core, system, Some(*planet), piece);
            }
        }
    }

    /// The next question of the craters: the seat that chooses, the system and the units still to destroy there. Systems and seats that need no choice
    /// are served on the way (a seat with `left` units or fewer loses them all; units that are all alike go without a question).
    fn crater_next(&mut self, core: &mut CoreState, scope: Scope) -> Option<(Seat, System, u8)> {
        let systems = near(core);
        let order = crate::query::seats::speaker_order(core);
        loop {
            let system = *systems.as_slice().get(usize::from(self.crater_at.system))?;
            let Some(&seat) = order.as_slice().get(usize::from(self.cursor)) else {
                self.crater_at = At { system: self.crater_at.system + 1, left: 0 };
                self.cursor = 0;
                continue;
            };
            let units = held(core, scope, system, seat);
            let left = if self.crater_at.left == 0 { LOSS } else { self.crater_at.left };
            let alike = units.windows(2).all(|w| w[0] == w[1]);
            if units.is_empty() || units.len() <= usize::from(left) || alike {
                for (planet, piece) in units.into_iter().take(usize::from(left)) {
                    destroy(core, system, planet, piece);
                }
                self.cursor += 1;
                self.crater_at.left = 0;
                continue;
            }
            self.crater_at.left = left;
            return Some((seat, system, left));
        }
    }

    /// The question of the craters: one of the units the seat holds in the system, each kind at each place once.
    pub(super) fn crater_question(&self, s: &CoreState) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let scope = self.facts.as_ref().map_or(Scope(0), |f| f.scope);
        let system = *near(s).as_slice().get(usize::from(self.crater_at.system))?;
        let seat = *crate::query::seats::speaker_order(s).as_slice().get(usize::from(self.cursor))?;
        let mut opts = Opts::default();
        let mut seen: Vec<(Option<Planet>, Piece)> = Vec::new();
        for unit in held(s, scope, system, seat) {
            if !seen.contains(&unit) {
                seen.push(unit);
                opts.push(Opt::Agenda(AOpt::Unit { planet: unit.0, piece: unit.1 }));
            }
        }
        Some(super::super::question(seat, AAsk::Crater { system, left: self.crater_at.left }, Origin::Agenda(Agenda::IxthianArtifact), opts))
    }

    /// The answer: the unit is destroyed; the seat is served when it has destroyed all it had to.
    pub(super) fn crater_answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Some(question) = self.crater_question(sx.state) else { return Err(Illegal) };
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        if !question.opts.as_slice().contains(&chosen) {
            return Err(Illegal);
        }
        let Opt::Agenda(AOpt::Unit { planet, piece }) = chosen else { return Err(Illegal) };
        let Ask::Agenda(AAsk::Crater { system, .. }) = question.ask else { return Err(Illegal) };
        if !destroy(sx.state, system, planet, piece) {
            return Err(Illegal);
        }
        self.crater_at.left -= 1;
        if self.crater_at.left == 0 {
            self.cursor += 1;
        }
        Ok(())
    }
}
