//! Xxcha's Peace Accords (`faction_abilities::strategy_resolved`, lane E2): "After you resolve the primary or secondary ability of the
//! Diplomacy strategy card, you may gain control of 1 planet that does not contain any units and is in or adjacent to a system that contains a
//! planet you control."
//!
//! The window asks which planet (or declines) and gives the seat control of it. What follows in the string, the exploration of the planet by
//! its first trait, and Integrated Economy's production before it, are the caller's (child windows: `PeaceAccords::annexed`). The Thunder's Edge
//! planet (`legendary::control_gained`'s breakthrough) is never offered: it is placed during play and printed on no system, and the candidates are the
//! planets the system records print.
//!
//! As in the string the planet is taken with a plain `set_control` (no `PLANET_CONTROL_GAINED`, no faction hook of a control gain), and a
//! planet that was destroyed (purged) is offered all the same and keeps no controller.

use crate::choice::strategy::{ability_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::generated::{Ability, Planet, System};
use crate::ids::{system_at, Seat};
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::{board, objectives};
use crate::state::{CoreState, PlanetFlags, Slice, SysFlags};
use crate::SysSet;

/// Where the window has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Start,
    Asking,
    Done,
}

/// The window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeaceAccords {
    pub who: Seat,
    stage: PStage,
    /// The planet the seat took: its exploration is the caller's.
    pub annexed: Option<(System, Planet)>,
}

/// `faction_abilities::annexable`: the empty, uncontrolled planets (not Mecatol Rex, not a space station) in or next to a system with a planet
/// `who` controls, in the string's order: the systems by id text, the planets of each in the order of the system record. `Vec::new()` for a
/// position without the map (the string's "in or next to needs the map").
#[must_use]
pub fn annexable(core: &CoreState, who: Seat) -> Vec<(System, Planet)> {
    // (A position without the map slice is a game without a galaxy.)
    let Some(galaxy) = core.loaded.has(Slice::Map).then(|| core.galaxy()).flatten() else { return Vec::new() };
    let mut reachable = SysSet::EMPTY;
    for (system, _) in board::controlled_planets(core, who).as_slice() {
        reachable.insert(system.index());
        reachable.union_with(&galaxy.adjacent(*system));
    }
    let mut found = Vec::new();
    for index in reachable.iter() {
        let system = system_at(index);
        for planet in system.data().planets.iter().copied() {
            if planet != Planet::MecatolRex && board::control(core, planet).is_none() && !objectives::is_space_station(planet) && board::planet_units(core, planet).is_empty() {
                found.push((system, planet));
            }
        }
    }
    found
}

/// Whether the seat has the ability (Xxcha).
#[must_use]
pub fn has_peace_accords(core: &CoreState, who: Seat) -> bool {
    core.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::PeaceAccords))
}

impl PeaceAccords {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: PStage::Start, annexed: None }
    }

    /// The question; `controlled` is `GameState::controlled_planets(..).len()`, the base of the preview of the gain.
    fn ask(core: &CoreState, who: Seat) -> Ask {
        Ask::Strategy(SAsk::PeaceAccords { controlled: u8::try_from(board::controlled_planets(core, who).as_slice().len()).unwrap_or(u8::MAX) })
    }
}

impl Window for PeaceAccords {
    type Stage = PStage;

    fn stage(&self) -> PStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == PStage::Start {
            self.stage = if has_peace_accords(sx.state, self.who) && !annexable(sx.state, self.who).is_empty() { PStage::Asking } else { PStage::Done };
        }
        if self.stage == PStage::Asking { Next::Continue(Self::ask(sx.state, self.who)) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == PStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Self::ask(s, self.who),
            ctx: ability_ctx(Ability::PeaceAccords, SSub::PeaceAccordsAnnex),
            opts: annexable(s, self.who).into_iter().map(|(system, planet)| Opt::Strategy(SOpt::Annex(system, planet))).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (PStage::Asking, Pick::Opt(Opt::Strategy(SOpt::Annex(system, planet)))) if annexable(sx.state, self.who).contains(&(system, planet)) => {
                let cell = sx.state.board_mut().planet_mut(planet);
                // (`set_control`: a destroyed planet has no card to take.)
                if !cell.flags.has(PlanetFlags::PURGED) {
                    cell.control = Some(self.who);
                }
                sx.state.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
                self.annexed = Some((system, planet));
                self.stage = PStage::Done;
                Ok(())
            }
            (PStage::Asking, Pick::Opt(Opt::Decline)) => {
                self.stage = PStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
