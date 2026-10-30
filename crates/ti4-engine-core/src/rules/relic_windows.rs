//! The questions of the relics' ACTIONs (package B5; string `relics.rs`) as [`Window`]s: the Neuraloop's relic to
//! purge, the Codex's action cards, the Titan Prototype's builder, the Stellar Converter's target and the
//! technology of the Enigmatic Device and the Maw of Worlds.
//!
//! Each is the string code between "decide whether to ask" and "act on the answer". What a window writes is
//! said at the window; the rest of the relic's effect (the roll of the Neuraloop, the payment and the
//! structure of the Titan Prototype, the research behind a technology) stays with the string caller, which
//! the adapter documents as a bail (`docs/ENGINE_KNOWN_ISSUES.md`).

use crate::choice::agenda::Shown;
use crate::choice::relic::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::generated::{Planet, PublicObjective, Relic, System, Technology};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, Report, Spawn, StructureFacts};
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{Code, CoreState, ObjectiveRef, RngDomain};

use super::explore;
use super::explore_windows::{InInvasion, XStage, in_invasion, relic_ctx};

fn opts(list: impl IntoIterator<Item = ROpt>) -> Opts {
    list.into_iter().map(Opt::Relic).collect::<Opts>()
}

fn choice(who: Seat, ask: RAsk, relic: Relic, subtype: RSub, inv: Option<u64>, opts: Opts) -> Choice {
    Choice { actor: who, ask: Ask::Relic(ask), ctx: relic_ctx(relic, subtype, inv), opts }
}

// -- the Neuraloop ------------------------------------------------------------------------------------

/// `relics::neuraloop`: the holder is asked which of their relics to purge to replace the revealed objective (any
/// relic, the Neuraloop itself included, or nothing). On a relic named the pool is checked before anything is
/// paid (the public objective deck, then the secret deck; an empty pool resolves nothing and costs nothing), one
/// die of the pool's size is rolled on the Neuraloop's own stream, the relic is purged, the revealed objective
/// leaves the revealed list and the replacement (a secret becomes public) takes its place at the end of it.
/// `chosen` is the relic named; `replacement` the objective that came in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neuraloop {
    pub holder: Seat,
    revealed: PublicObjective,
    stage: XStage,
    pub chosen: Option<Relic>,
    pub replacement: Option<ObjectiveRef>,
    inv: Option<u64>,
}

impl Neuraloop {
    #[must_use]
    pub const fn new(holder: Seat, revealed: PublicObjective) -> Self {
        Self { holder, revealed, stage: XStage::Start, chosen: None, replacement: None, inv: None }
    }

    /// The tail after the relic is named: roll, purge, swap. Nothing happens for an empty pool.
    fn replace(&mut self, sx: &mut Sx<'_>, relic: Relic) {
        let decks = sx.state.decks();
        let pool: Vec<ObjectiveRef> = decks
            .objective
            .cards()
            .iter()
            .map(|o| ObjectiveRef::Public(*o))
            .chain(decks.secret.cards().iter().map(|o| ObjectiveRef::Secret(*o)))
            .collect();
        if pool.is_empty() {
            return;
        }
        let sides = u32::try_from(pool.len()).unwrap_or(u32::MAX);
        let roll = sx.die(RngDomain::Neuraloop, sides) as usize;
        let pick = pool[roll.saturating_sub(1).min(pool.len() - 1)];
        explore::purge_relic(sx.state, self.holder, relic);
        let gone = ObjectiveRef::Public(self.revealed);
        sx.state.objectives_mut().revealed.retain(|o| *o != gone);
        // The pick is in the pool, hence in exactly one deck (a card is never in both).
        match pick {
            ObjectiveRef::Public(o) => {
                if let Some(at) = sx.state.decks().objective.cards().iter().position(|c| *c == o) {
                    sx.state.decks_mut().objective.remove(at);
                }
            }
            ObjectiveRef::Secret(o) => {
                if let Some(at) = sx.state.decks().secret.cards().iter().position(|c| *c == o) {
                    sx.state.decks_mut().secret.remove(at);
                }
            }
        }
        sx.state.objectives_mut().revealed.push(pick);
        self.replacement = Some(pick);
    }

    fn spendable(&self, core: &CoreState) -> Vec<Relic> {
        core.players().by_seat(self.holder).map(|p| p.cards.relics.as_slice().to_vec()).unwrap_or_default()
    }

    const fn ask(&self) -> Ask {
        Ask::Relic(RAsk::Neuraloop { revealed: self.revealed })
    }
}

impl Window for Neuraloop {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            self.stage = if self.spendable(sx.state).is_empty() { XStage::Done } else { XStage::Asking };
        }
        if self.stage == XStage::Asking { Next::Continue(self.ask()) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| {
            let list = self.spendable(s).into_iter().map(ROpt::PurgeRelic).map(Opt::Relic).chain(std::iter::once(Opt::Decline));
            Choice {
                actor: self.holder,
                ask: self.ask(),
                ctx: relic_ctx(Relic::Neuraloop, RSub::NeuraloopChooseRelicToPurge, self.inv),
                opts: list.collect::<Opts>(),
            }
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Relic(ROpt::PurgeRelic(r)))) if self.spendable(sx.state).contains(&r) => {
                self.chosen = Some(r);
                self.replace(sx, r);
                self.stage = XStage::Done;
                Ok(())
            }
            (XStage::Asking, Pick::Opt(Opt::Decline)) => {
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- the Codex ----------------------------------------------------------------------------------------

/// `relics::codex`: up to three action cards from the discard pile, asked one at a time against the pile as it
/// stands (an empty pile or a decline ends it). Each card taken leaves the pile for the seat's hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codex {
    pub who: Seat,
    stage: XStage,
    pub taken: u8,
    inv: Option<u64>,
}

impl Codex {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: XStage::Start, taken: 0, inv: None }
    }

    const ASK: Ask = Ask::Relic(RAsk::Codex);
}

impl Window for Codex {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage != XStage::Done {
            self.stage = if self.taken >= 3 || sx.state.cards().discarded_action.is_empty() { XStage::Done } else { XStage::Asking };
        }
        if self.stage == XStage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| {
            let held = s.players().by_seat(self.who).map_or(0, |p| p.cards.hand.len());
            let shown = Shown { votes: i32::try_from(held).unwrap_or(i32::MAX), system: None };
            let list = s.cards().discarded_action.iter().map(|c| Opt::Relic(ROpt::Take(*c, shown))).chain(std::iter::once(Opt::Decline));
            Choice {
                actor: self.who,
                ask: Self::ASK,
                ctx: relic_ctx(Relic::TheCodex, RSub::CodexTakeActionCard, self.inv),
                opts: list.collect::<Opts>(),
            }
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Relic(ROpt::Take(card, _)))) if sx.state.cards().discarded_action.contains(&card) => {
                explore::take_discarded_action_card(sx.state, self.who, card);
                self.taken += 1;
                Ok(())
            }
            (XStage::Asking, Pick::Opt(Opt::Decline)) => {
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- the Titan Prototype ------------------------------------------------------------------------------

/// The suspension points of [`TitanPrototype`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Start,
    Asking,
    /// The named player builds or takes the trade good ([`Spawn::TitanBuild`]).
    Building,
    Done,
}

/// `relics::titan_prototype`: the owner names the player who may build (any seat, themselves included). Given the
/// position's [`StructureFacts`] and the interned source `relic` ([`Self::with_build`]) the window goes on with what the
/// named player does: three resources for a structure, else a trade good (a child window of the production area). Without
/// them it ends at the answer and the build is the string caller's. `chosen` is the seat; `built` whether a structure was
/// placed; `unsupported` that the table has no link to start the build with.
#[derive(Debug, Clone)]
pub struct TitanPrototype {
    pub who: Seat,
    stage: TStage,
    pub chosen: Option<Seat>,
    build: Option<(StructureFacts, Code)>,
    child: ErasedChild,
    pub built: bool,
    pub unsupported: bool,
    inv: Option<u64>,
}

impl TitanPrototype {
    #[must_use]
    pub fn new(who: Seat) -> Self {
        Self { who, stage: TStage::Start, chosen: None, build: None, child: ErasedChild::idle(), built: false, unsupported: false, inv: None }
    }

    /// Carry the build that follows the answer: the named player's `facts` and the interned source `relic` of the trade good.
    #[must_use]
    pub fn with_build(mut self, facts: StructureFacts, goods: Code) -> Self {
        self.build = Some((facts, goods));
        self
    }

    const ASK: Ask = Ask::Relic(RAsk::TitanPrototype);
}

impl Window for TitanPrototype {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == TStage::Start {
            self.stage = if sx.state.players().iter().next().is_none() { TStage::Done } else { TStage::Asking };
        }
        if self.stage == TStage::Building {
            if !self.child.running() {
                let Some(who) = self.chosen else { return self.fail() };
                let Some((facts, goods)) = self.build else { return self.fail() };
                match sx.table.links.start(&Spawn::TitanBuild { who, facts, goods }) {
                    Some(window) => self.child.start(window),
                    None => return self.fail(),
                }
            }
            match self.child.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    self.built = report == Report::Built(true);
                    self.stage = TStage::Done;
                }
                None => return self.fail(),
            }
        }
        if self.stage == TStage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            TStage::Asking => Some(choice(
                self.who,
                RAsk::TitanPrototype,
                Relic::JRXS455O,
                RSub::TitanPrototypeChooseBuilder,
                self.inv,
                opts(s.players().iter().map(|p| ROpt::Builder(p.seat))),
            )),
            TStage::Building => self.child.ask_of(s, k),
            TStage::Start | TStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (TStage::Asking, Pick::Opt(Opt::Relic(ROpt::Builder(seat)))) if sx.state.players().by_seat(seat).is_some() => {
                self.chosen = Some(seat);
                self.stage = if self.build.is_some() { TStage::Building } else { TStage::Done };
                Ok(())
            }
            (TStage::Building, pick) => self.child.answer(sx, pick),
            _ => Err(Illegal),
        }
    }
}

impl TitanPrototype {
    /// The build cannot be carried: the window ends and says so.
    fn fail(&mut self) -> Next {
        self.unsupported = true;
        self.stage = TStage::Done;
        Next::Done
    }
}

// -- the Stellar Converter ----------------------------------------------------------------------------

/// `relics::stellar_converter`, from the question on: the owner picks one of the planets in range and it is
/// destroyed (its units, control, coexistence and attachments go, the planet is marked purged). The targets are
/// the string caller's (they come from the map's adjacency and the unit catalogue); `destroyed` is the planet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StellarConverter {
    pub who: Seat,
    targets: Vec<(System, Planet)>,
    stage: XStage,
    pub destroyed: Option<Planet>,
    inv: Option<u64>,
}

impl StellarConverter {
    #[must_use]
    pub fn new(who: Seat, targets: &[(System, Planet)]) -> Self {
        Self { who, targets: targets.to_vec(), stage: XStage::Start, destroyed: None, inv: None }
    }

    const ASK: Ask = Ask::Relic(RAsk::StellarConverter);
}

impl Window for StellarConverter {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            self.stage = if self.targets.is_empty() { XStage::Done } else { XStage::Asking };
        }
        if self.stage == XStage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| {
            choice(
                self.who,
                RAsk::StellarConverter,
                Relic::StellarConverter,
                RSub::StellarConverterChooseTarget,
                self.inv,
                opts(self.targets.iter().map(|(system, planet)| ROpt::Destroy(*planet, Shown { votes: 0, system: Some(*system) }))),
            )
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Relic(ROpt::Destroy(planet, _)))) if self.targets.iter().any(|(_, p)| *p == planet) => {
                explore::purge_planet(sx.state, planet);
                self.destroyed = Some(planet);
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- the technology of the Enigmatic Device and the Maw of Worlds -------------------------------------

/// `relics::ask_technology`: "gain which technology", from the list the string side computed (what the seat does
/// not hold, its faction's own, narrowed to what it could research for the Enigmatic Device). The research or
/// the gain behind the answer is the string caller's; `chosen` is the technology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GainTechnology {
    pub who: Seat,
    relic: Relic,
    /// A source that is no relic (Research Team, an agenda): the interned text; `relic` is then not read.
    from: Option<Code>,
    options: Vec<Technology>,
    stage: XStage,
    pub chosen: Option<Technology>,
    inv: Option<u64>,
}

impl GainTechnology {
    /// `relic` is the card the question is about (`enigmaticdevice`, `mawofworlds`).
    #[must_use]
    pub fn new(who: Seat, relic: Relic, options: &[Technology]) -> Self {
        Self { who, relic, from: None, options: options.to_vec(), stage: XStage::Start, chosen: None, inv: None }
    }

    /// The same question for a source that is no relic (`researchteam`, an agenda's grant): `source` is its interned text.
    #[must_use]
    pub fn from_source(who: Seat, source: Code, options: &[Technology]) -> Self {
        Self { who, relic: Relic::ALL[0], from: Some(source), options: options.to_vec(), stage: XStage::Start, chosen: None, inv: None }
    }

    const fn ask(&self) -> Ask {
        match self.from {
            Some(source) => Ask::Relic(RAsk::GainTechnologyFrom { source }),
            None => Ask::Relic(RAsk::GainTechnology { relic: self.relic }),
        }
    }

    fn ctx(&self) -> crate::choice::Ctx {
        match self.from {
            Some(source) => crate::choice::Ctx {
                origin: crate::choice::Origin::Content(crate::choice::generic::Content::Other(source)),
                subtype: crate::choice::Subtype::Relic(RSub::ChooseTechnologyFrom(source)),
                optional: false,
                target: None,
                outstanding: crate::containers::SmallList::new(),
                space_battle: false,
                invasion_seq: self.inv,
            },
            None => relic_ctx(self.relic, RSub::ChooseTechnology(self.relic), self.inv),
        }
    }
}

impl Window for GainTechnology {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            self.stage = if self.options.is_empty() { XStage::Done } else { XStage::Asking };
        }
        if self.stage == XStage::Asking { Next::Continue(self.ask()) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: self.ask(),
            ctx: self.ctx(),
            opts: opts(self.options.iter().map(|t| ROpt::Tech(*t))),
        })
    }

    fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Relic(ROpt::Tech(t)))) if self.options.contains(&t) => {
                self.chosen = Some(t);
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

in_invasion!(Neuraloop, Codex, TitanPrototype, StellarConverter, GainTechnology);
