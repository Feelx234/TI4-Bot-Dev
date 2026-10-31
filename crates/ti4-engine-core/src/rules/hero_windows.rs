//! Heroes composed from existing windows and kernel effects (package W10, lent-bodies lane 8): UNIT.DSGN.FLAYESH (Nekro).
//!
//! Each hero is the arm of the string `use_leader` split at its questions, as in `leader_windows`: `step` runs to the next question,
//! `answer` is the only place an answer changes the position, and the effect itself happens after the last answer. Whether the card
//! resolved (the string's `bool`) is the window's `used` field; the caller purges the hero.

use crate::choice::generic::Content;
use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, Planet, System, Technology};
use crate::ids::Seat;
use crate::keys::DestroyReason;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::fleet;
use crate::state::{Code, CoreState, SysFlags};

fn ctx(leader: Leader, subtype: LSub) -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Leader(leader)),
        subtype: Subtype::Leaders(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

// -- UNIT.DSGN.FLAYESH ------------------------------------------------------------------------------

/// One planet the Nekro hero can devour, as the adapter read it (the string reads: the technology specialties now, the planet's
/// resources and influence now, the technologies of the colour that the seat may gain, in the content's order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Devourable {
    pub system: System,
    pub planet: Planet,
    /// The planet's resources and influence together (`planet_value_now`), the trade goods the hero gives.
    pub goods: i32,
    pub techs: Vec<Technology>,
}

/// The suspension points of [`NekroHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FStage {
    Start,
    /// Waiting on the planet (more than one qualifies).
    Planet,
    /// Waiting on the technology (more than one matches the planet's colours).
    Tech,
    Done,
}

/// UNIT.DSGN.FLAYESH (Nekro hero): "ACTION: Choose 1 planet that has a technology specialty in a system that contains 1 or more of your
/// units; destroy all other players' units on that planet, gain trade goods equal to its combined resources and influence value and gain
/// 1 technology of the specialty's colour. Then, purge this card." The planet and the technology are chosen before anything changes (a
/// single candidate is taken unasked; no technology candidate gains none); then every other seat's unit on the planet leaves it, the
/// ground forces staged as destroyed (cause `nekrohero`), the goods are gained (announced) and the technology is granted.
/// [`Self::used`]: some planet was devoured. [`Self::unsupported`]: a galvanized ground force fell (the staged row cannot hold it).
#[derive(Debug, Clone)]
pub struct NekroHero {
    pub who: Seat,
    /// The interned `nekrohero`, the source of the staged trade goods.
    source: Code,
    planets: Vec<Devourable>,
    stage: FStage,
    chosen: Option<usize>,
    pub used: bool,
    pub unsupported: bool,
}

impl NekroHero {
    #[must_use]
    pub fn new(who: Seat, source: Code, planets: Vec<Devourable>) -> Self {
        Self { who, source, planets, stage: FStage::Start, chosen: None, used: false, unsupported: false }
    }

    fn chosen_planet(&self) -> Option<&Devourable> {
        self.planets.get(self.chosen?)
    }

    /// The planet is chosen: ask for the technology or go on.
    fn planet_chosen(&mut self, sx: &mut Sx<'_>, at: usize) {
        self.chosen = Some(at);
        match self.planets[at].techs.as_slice() {
            [] => self.devour(sx, None),
            [only] => {
                let only = *only;
                self.devour(sx, Some(only));
            }
            _ => self.stage = FStage::Tech,
        }
    }

    /// Everything is answered: change the position.
    fn devour(&mut self, sx: &mut Sx<'_>, tech: Option<Technology>) {
        self.stage = FStage::Done;
        let Some(target) = self.chosen_planet().cloned() else { return };
        self.used = true;
        let (system, planet) = (target.system, target.planet);
        let victims: Vec<crate::state::Piece> = sx.state.board().planet(planet).units.as_slice().iter().copied().filter(|p| p.owner() != self.who).collect();
        if !victims.is_empty() {
            sx.state.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
        }
        for piece in &victims {
            if let Some(at) = sx.state.board().planet(planet).units.as_slice().iter().position(|p| p == piece) {
                sx.state.board_mut().planet_mut(planet).units.remove(at);
            }
            if fleet::is_ground_force(piece.kind())
                && fx_ground_stage(sx.state, system, planet, *piece).is_err()
            {
                self.unsupported = true;
            }
        }
        fx::gain_trade_goods_staged(sx.state, self.who, target.goods, self.source);
        if let Some(tech) = tech {
            fx::grant_technology(sx.state, self.who, tech);
        }
    }
}

/// `hooks_ground::stage_ground_force_destroyed` with the cause `nekrohero`.
fn fx_ground_stage(core: &mut CoreState, system: System, planet: Planet, piece: crate::state::Piece) -> Result<(), ()> {
    crate::kernel::fx_ground::stage_destroyed(core, system, planet, piece, DestroyReason::LeaderBare(Leader::UNITDSGNFLAYESH))
}

impl Window for NekroHero {
    type Stage = FStage;

    fn stage(&self) -> FStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == FStage::Start {
            match self.planets.len() {
                0 => self.stage = FStage::Done,
                1 => self.planet_chosen(sx, 0),
                _ => self.stage = FStage::Planet,
            }
        }
        match self.stage {
            FStage::Planet => Next::Continue(Ask::Leaders(LAsk::FlayeshPlanet)),
            FStage::Tech => Next::Continue(Ask::Leaders(LAsk::FlayeshTech)),
            FStage::Start | FStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.stage {
            FStage::Planet => Some(Choice {
                actor: self.who,
                ask: Ask::Leaders(LAsk::FlayeshPlanet),
                ctx: ctx(Leader::UNITDSGNFLAYESH, LSub::FlayeshPlanet),
                opts: self.planets.iter().map(|d| Opt::Leaders(LOpt::Devour(d.system, d.planet))).collect::<Opts>(),
            }),
            FStage::Tech => Some(Choice {
                actor: self.who,
                ask: Ask::Leaders(LAsk::FlayeshTech),
                ctx: ctx(Leader::UNITDSGNFLAYESH, LSub::FlayeshTech),
                opts: self.chosen_planet()?.techs.iter().map(|t| Opt::Leaders(LOpt::Gain(*t))).collect::<Opts>(),
            }),
            FStage::Start | FStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Leaders(chosen)) = pick else { return Err(Illegal) };
        match (self.stage, chosen) {
            (FStage::Planet, LOpt::Devour(system, planet)) => {
                let at = self.planets.iter().position(|d| d.system == system && d.planet == planet).ok_or(Illegal)?;
                self.planet_chosen(sx, at);
                Ok(())
            }
            (FStage::Tech, LOpt::Gain(tech)) => {
                if !self.chosen_planet().is_some_and(|d| d.techs.contains(&tech)) {
                    return Err(Illegal);
                }
                self.devour(sx, Some(tech));
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
