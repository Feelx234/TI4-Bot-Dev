//! The free technology windows of a turn (package W12; string `technology::start_turn` and `technology::end_turn`) as
//! [`Window`]s: Psychoarchaeology (start of a turn), Predictive Intelligence and Bio-Stims (end of a turn).
//!
//! Each is the string code split at its questions. Psychoarchaeology asks again after each use until no specialty planet is
//! ready or the owner declines; Predictive Intelligence moves one token per answer (at most as many as the owner holds) and
//! exhausts the card when anything moved; Bio-Stims readies one planet or technology and exhausts itself.
//! What stays with the string (the adapter bails): Transit Diodes and Chaos Mapping at the start of a turn.

use crate::choice::agenda::Shown;
use crate::choice::strategy::Pool;
use crate::choice::technology::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Planet, System, Technology};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board;
use crate::state::{Code, CoreState};

fn ctx(tech: Technology, subtype: TSub) -> Ctx {
    Ctx {
        origin: Origin::Content(Content::Technology(tech)),
        subtype: Subtype::Technology(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

fn owns(s: &CoreState, who: Seat, tech: Technology) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(tech))
}

fn ready_card(s: &CoreState, who: Seat, tech: Technology) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(tech) && !p.tech.exhausted.contains(tech))
}

/// `planets::tech_specialties_now` is not empty: a printed specialty or an attached one.
fn specialty(s: &CoreState, planet: Planet) -> bool {
    !planet.data().specialties.is_empty() || board::attachments(s, planet).as_slice().iter().any(|a| !a.data().specialties.is_empty())
}

/// The planets `who` controls, with their system, in the order of the string's `controlled_planets` (system id, then planet id).
fn controlled(s: &CoreState, who: Seat) -> impl Iterator<Item = (System, Planet)> {
    board::controlled_planets(s, who).as_slice().to_vec().into_iter()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Asking,
    Done,
}

// -- Psychoarchaeology ------------------------------------------------------------------------------

/// "Psychoarchaeology: exhaust a specialty for 1 trade good": each ready specialty planet may be converted once; declining
/// preserves it for a later use. `source` is the interned `"psychoarchaeology"`, the source of the staged trade good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Psychoarchaeology {
    pub who: Seat,
    source: Code,
    stage: Stage,
}

impl Psychoarchaeology {
    #[must_use]
    pub const fn new(who: Seat, source: Code) -> Self {
        Self { who, source, stage: Stage::Start }
    }

    /// Whether the card applies at all (owned: it is never exhausted).
    #[must_use]
    pub fn applies(s: &CoreState, who: Seat) -> bool {
        owns(s, who, Technology::Psychoarchaeology)
    }

    fn candidates(&self, s: &CoreState) -> Vec<(System, Planet)> {
        controlled(s, self.who).filter(|(_, p)| !board::exhausted(s, *p) && specialty(s, *p)).collect()
    }
}

impl Window for Psychoarchaeology {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if Self::applies(sx.state, self.who) && !self.candidates(sx.state).is_empty() { Stage::Asking } else { Stage::Done };
        }
        if self.stage == Stage::Asking { Next::Continue(Ask::Technology(TAsk::Psychoarchaeology)) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Technology(TAsk::Psychoarchaeology),
            ctx: ctx(Technology::Psychoarchaeology, TSub::PsychoarchaeologyExhaustSpecialty),
            opts: self
                .candidates(s)
                .into_iter()
                .map(|(system, planet)| Opt::Technology(TOpt::Planet(planet, Shown { votes: 0, system: Some(system) })))
                .chain(std::iter::once(Opt::Decline))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => self.stage = Stage::Done,
            Pick::Opt(Opt::Technology(TOpt::Planet(planet, _))) if self.candidates(sx.state).iter().any(|(_, p)| *p == planet) => {
                fx::exhaust_planet(sx.state, planet);
                fx::gain_trade_goods_staged(sx.state, self.who, 1, self.source);
                // Asked again while a ready specialty is left.
                self.stage = if self.candidates(sx.state).is_empty() { Stage::Done } else { Stage::Asking };
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- the end of a turn: Predictive Intelligence, then Bio-Stims -----------------------------------------

/// The suspension points of [`TurnEnd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EStage {
    Start,
    /// Waiting on a token to move (Predictive Intelligence).
    Restack,
    /// Waiting on what to ready (Bio-Stims).
    Ready,
    Done,
}

/// The free windows at the end of an action-phase turn: Predictive Intelligence (move tokens between pools, once the card is
/// ready; exhausted when anything moved), then Bio-Stims (ready a specialty planet or a technology; exhausts itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnEnd {
    pub who: Seat,
    stage: EStage,
    /// Tokens the owner may still move (the total held when the window began, less those moved).
    left: i32,
    changed: bool,
}

const POOLS: [Pool; 3] = [Pool::Tactic, Pool::Fleet, Pool::Strategic];

fn held(s: &CoreState, who: Seat, pool: Pool) -> i16 {
    s.players().by_seat(who).map_or(0, |p| match pool {
        Pool::Tactic => p.econ.tactic_tokens,
        Pool::Fleet => p.econ.fleet_tokens,
        Pool::Strategic => p.econ.strategic_tokens,
    })
}

fn pool_mut(s: &mut CoreState, who: Seat, pool: Pool) -> Option<&mut i16> {
    s.players_mut().by_seat_mut(who).map(|p| match pool {
        Pool::Tactic => &mut p.econ.tactic_tokens,
        Pool::Fleet => &mut p.econ.fleet_tokens,
        Pool::Strategic => &mut p.econ.strategic_tokens,
    })
}

impl TurnEnd {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: EStage::Start, left: 0, changed: false }
    }

    /// Whether either card applies (a ready Predictive Intelligence or Bio-Stims).
    #[must_use]
    pub fn applies(s: &CoreState, who: Seat) -> bool {
        ready_card(s, who, Technology::PredictiveIntelligence) || ready_card(s, who, Technology::BioStims)
    }

    fn moves(&self, s: &CoreState) -> Vec<(Pool, Pool)> {
        let mut out = Vec::new();
        for from in POOLS {
            if held(s, self.who, from) <= 0 {
                continue;
            }
            for to in POOLS {
                if from != to {
                    out.push((from, to));
                }
            }
        }
        out
    }

    /// The Bio-Stims options: exhausted specialty planets of the owner, then exhausted technologies other than Bio-Stims.
    fn readyable(&self, s: &CoreState) -> Vec<TOpt> {
        let planets = controlled(s, self.who)
            .filter(|(_, p)| board::exhausted(s, *p) && specialty(s, *p))
            .map(|(system, p)| TOpt::Planet(p, Shown { votes: 0, system: Some(system) }));
        let techs = s
            .players()
            .by_seat(self.who)
            .into_iter()
            .flat_map(|p| p.tech.exhausted.iter())
            .filter(|t| *t != Technology::BioStims)
            .map(TOpt::ReadyTechnology);
        planets.chain(techs).collect()
    }

    /// Move on from the Predictive Intelligence part (settled: the card is exhausted when anything moved) to Bio-Stims.
    fn to_bio_stims(&mut self, s: &mut CoreState) {
        if self.changed {
            fx_exhaust(s, self.who, Technology::PredictiveIntelligence);
            self.changed = false;
        }
        self.stage = if ready_card(s, self.who, Technology::BioStims) && !self.readyable(s).is_empty() { EStage::Ready } else { EStage::Done };
    }
}

fn fx_exhaust(s: &mut CoreState, who: Seat, tech: Technology) {
    crate::kernel::fx_strategy::exhaust_technology(s, who, tech);
}

impl Window for TurnEnd {
    type Stage = EStage;

    fn stage(&self) -> EStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == EStage::Start {
            if ready_card(sx.state, self.who, Technology::PredictiveIntelligence) {
                self.left = POOLS.iter().map(|p| i32::from(held(sx.state, self.who, *p))).sum();
                if self.left > 0 && !self.moves(sx.state).is_empty() {
                    self.stage = EStage::Restack;
                } else {
                    self.to_bio_stims(sx.state);
                }
            } else {
                self.to_bio_stims(sx.state);
            }
        }
        match self.stage {
            EStage::Restack => Next::Continue(Ask::Technology(TAsk::Restack)),
            EStage::Ready => Next::Continue(Ask::Technology(TAsk::BioStims)),
            EStage::Start | EStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.stage {
            EStage::Restack => Some(Choice {
                actor: self.who,
                ask: Ask::Technology(TAsk::Restack),
                ctx: ctx(Technology::PredictiveIntelligence, TSub::PredictiveIntelligenceRedistribute),
                opts: self.moves(s).into_iter().map(|(from, to)| Opt::Technology(TOpt::Move { from, to })).chain(std::iter::once(Opt::Done)).collect::<Opts>(),
            }),
            EStage::Ready => Some(Choice {
                actor: self.who,
                ask: Ask::Technology(TAsk::BioStims),
                ctx: ctx(Technology::BioStims, TSub::BioStimsReady),
                opts: self.readyable(s).into_iter().map(Opt::Technology).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
            }),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (EStage::Restack, Pick::Opt(Opt::Done)) => self.to_bio_stims(sx.state),
            (EStage::Restack, Pick::Opt(Opt::Technology(TOpt::Move { from, to }))) if self.moves(sx.state).contains(&(from, to)) => {
                if let Some(n) = pool_mut(sx.state, self.who, from) {
                    *n -= 1;
                }
                if let Some(n) = pool_mut(sx.state, self.who, to) {
                    *n += 1;
                }
                self.changed = true;
                self.left -= 1;
                if self.left <= 0 || self.moves(sx.state).is_empty() {
                    self.to_bio_stims(sx.state);
                }
            }
            (EStage::Ready, Pick::Opt(Opt::Decline)) => self.stage = EStage::Done,
            (EStage::Ready, Pick::Opt(Opt::Technology(done))) if self.readyable(sx.state).contains(&done) => {
                match done {
                    TOpt::Planet(planet, _) => {
                        fx::ready_planet(sx.state, planet);
                    }
                    TOpt::ReadyTechnology(tech) => {
                        if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                            p.tech.exhausted.remove(tech);
                        }
                    }
                    TOpt::Move { .. } => return Err(Illegal),
                }
                fx_exhaust(sx.state, self.who, Technology::BioStims);
                self.stage = EStage::Done;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}
