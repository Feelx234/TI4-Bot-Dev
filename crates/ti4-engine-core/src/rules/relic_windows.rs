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
use crate::choice::strategy::Pool;
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

// -- Fabrication and Black Market Forgery -------------------------------------------------------------

/// What a [`Fabrication`] pays for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FabMode {
    /// Fabrication (Naaz) or Black Market Forgery (`bmf`: the held note goes home after the relic): purge 2 fragments of one type for
    /// the top relic.
    Relic { bmf: bool },
    /// Fabrication: purge 1 fragment for 1 command token of a chosen pool.
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FStage {
    Start,
    Kind,
    Pool,
    Done,
}

/// The relic fragments traits the Naaz card names, in the order of the string (`TYPES`).
const FRAGMENT_TYPES: [crate::generated::ExploreType; 3] = [crate::generated::ExploreType::Cultural, crate::generated::ExploreType::Hazardous, crate::generated::ExploreType::Industrial];

/// Fabrication (Naaz): "ACTION: Purge 2 of your relic fragments of the same type to gain 1 relic; or purge 1 relic fragment to gain 1
/// command token" and Black Market Forgery (its promissory note, played by the holder): "ACTION: Purge 2 of your relic fragments of the
/// same type to gain 1 relic. Then, return this card to the Naaz player." A frontier fragment counts as any type and is spent after the
/// matching ones. The type is asked when more than one qualifies (a seat holding only frontier fragments has one choice); the token
/// action also asks the pool. Nothing is written until the last answer; `done` is whether the action resolved. `source` is the interned
/// `fabrication` or `bmf`, the question's origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fabrication {
    pub who: Seat,
    mode: FabMode,
    source: Code,
    stage: FStage,
    kind: Option<crate::generated::ExploreType>,
    pub done: bool,
}

impl Fabrication {
    #[must_use]
    pub const fn new(who: Seat, mode: FabMode, source: Code) -> Self {
        Self { who, mode, source, stage: FStage::Start, kind: None, done: false }
    }

    fn held(core: &CoreState, who: Seat, kind: crate::generated::ExploreType) -> i32 {
        core.players().by_seat(who).map_or(0, |p| i32::from(p.cards.fragments.get(kind).unwrap_or(0)))
    }

    /// The types offered, in the order of the string.
    fn kinds(&self, core: &CoreState) -> Vec<crate::generated::ExploreType> {
        use crate::generated::ExploreType;
        let who = self.who;
        match self.mode {
            FabMode::Relic { .. } => {
                let frontier = Self::held(core, who, ExploreType::Frontier);
                let mut kinds: Vec<ExploreType> = FRAGMENT_TYPES.iter().copied().filter(|k| Self::held(core, who, *k) + frontier >= 2).collect();
                // Only frontier fragments: every type is the same purge, so one is offered.
                if kinds.iter().all(|k| Self::held(core, who, *k) == 0) {
                    kinds.truncate(1);
                }
                kinds
            }
            FabMode::Token => FRAGMENT_TYPES.iter().copied().chain(std::iter::once(ExploreType::Frontier)).filter(|k| Self::held(core, who, *k) > 0).collect(),
        }
    }

    const fn count(&self) -> u8 {
        match self.mode {
            FabMode::Relic { .. } => 2,
            FabMode::Token => 1,
        }
    }

    /// `fragments - count` of `kind`, matching ones first, the shortfall from the frontier ones. `false` (nothing written) when short.
    fn purge(core: &mut CoreState, who: Seat, kind: crate::generated::ExploreType, count: i16) -> bool {
        use crate::generated::ExploreType;
        let matching = Self::held(core, who, kind);
        let frontier = if kind == ExploreType::Frontier { 0 } else { Self::held(core, who, ExploreType::Frontier) };
        if matching + frontier < i32::from(count) {
            return false;
        }
        let from_matching = i16::try_from(matching).unwrap_or(i16::MAX).min(count);
        let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
        if from_matching > 0 {
            let have = p.cards.fragments.get(kind).unwrap_or(0);
            p.cards.fragments.set(kind, have - from_matching);
        }
        if count > from_matching {
            let have = p.cards.fragments.get(ExploreType::Frontier).unwrap_or(0);
            p.cards.fragments.set(ExploreType::Frontier, have - (count - from_matching));
        }
        true
    }

    /// The Black Market Forgery note the seat holds (a note of another faction), if any.
    fn bmf_note(core: &CoreState, who: Seat) -> Option<crate::state::NoteKey> {
        let own = core.players().by_seat(who)?.faction;
        core.cards().promissory.iter().find(|(key, holder)| **holder == who && key.note == crate::generated::PromissoryNote::BlackMarketForgery && key.faction != own).map(|(key, _)| *key)
    }

    fn ctx(&self, subtype: RSub) -> crate::choice::Ctx {
        crate::choice::Ctx {
            origin: crate::choice::Origin::FactionAbility(crate::choice::generic::Content::Other(self.source)),
            subtype: crate::choice::Subtype::Relic(subtype),
            optional: false,
            target: None,
            outstanding: crate::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    fn pay_relic(&mut self, s: &mut CoreState, kind: crate::generated::ExploreType) {
        if s.decks().relic.cards().is_empty() || !Self::purge(s, self.who, kind, 2) {
            return;
        }
        if crate::kernel::fx_relic::relic_gain(s, self.who).is_none() {
            return;
        }
        if let (FabMode::Relic { bmf: true }, Some(note)) = (self.mode, Self::bmf_note(s, self.who)) {
            crate::kernel::fx::give_back_note(s, note);
        }
        self.done = true;
    }

    fn pay_token(&mut self, s: &mut CoreState, kind: crate::generated::ExploreType, pool: Pool) {
        if Self::purge(s, self.who, kind, 1) {
            self.done = crate::kernel::fx::gain_token(s, self.who, pool, 1) > 0;
        }
    }

    fn picked_kind(&mut self, s: &mut CoreState, kind: crate::generated::ExploreType) {
        match self.mode {
            FabMode::Relic { .. } => {
                self.pay_relic(s, kind);
                self.stage = FStage::Done;
            }
            FabMode::Token => {
                self.kind = Some(kind);
                self.stage = FStage::Pool;
            }
        }
    }
}

impl Window for Fabrication {
    type Stage = FStage;

    fn stage(&self) -> FStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == FStage::Start {
            let token_room = !matches!(self.mode, FabMode::Token) || crate::kernel::fx::tokens_in_reinforcements(sx.state, self.who) > 0;
            // The note is the action's: a seat that holds none cannot play it.
            let note = !matches!(self.mode, FabMode::Relic { bmf: true }) || Self::bmf_note(sx.state, self.who).is_some();
            let kinds = if token_room && note { self.kinds(sx.state) } else { Vec::new() };
            match kinds.as_slice() {
                [] => self.stage = FStage::Done,
                [only] => self.picked_kind(sx.state, *only),
                _ => self.stage = FStage::Kind,
            }
        }
        match self.stage {
            FStage::Kind => Next::Continue(Ask::Relic(RAsk::FragmentType { count: self.count() })),
            FStage::Pool => Next::Continue(Ask::Relic(RAsk::TokenPool)),
            FStage::Start | FStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.stage {
            FStage::Kind => Some(Choice { actor: self.who, ask: Ask::Relic(RAsk::FragmentType { count: self.count() }), ctx: self.ctx(RSub::FragmentType), opts: opts(self.kinds(s).into_iter().map(ROpt::Fragment)) }),
            FStage::Pool => Some(Choice { actor: self.who, ask: Ask::Relic(RAsk::TokenPool), ctx: self.ctx(RSub::TokenPool), opts: opts([Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().map(ROpt::Pool)) }),
            FStage::Start | FStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (FStage::Kind, Pick::Opt(Opt::Relic(ROpt::Fragment(kind)))) if self.kinds(sx.state).contains(&kind) => {
                self.picked_kind(sx.state, kind);
                Ok(())
            }
            (FStage::Pool, Pick::Opt(Opt::Relic(ROpt::Pool(pool)))) => {
                let kind = self.kind.ok_or(Illegal)?;
                self.pay_token(sx.state, kind, pool);
                self.stage = FStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

in_invasion!(Neuraloop, Codex, TitanPrototype, StellarConverter, GainTechnology);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::{ExploreType as T, Faction, PromissoryNote};
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{NoteKey, Player, Players, Seats};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = [Seat(0), Seat(1)];
        s.seats = Seats::new(&list, &list, 2);
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s.decks_mut().relic.set(vec![Relic::ALL[3], Relic::ALL[4]]);
        s
    }

    fn give(s: &mut CoreState, seat: u8, kind: T, n: i16) {
        s.players_mut().by_seat_mut(Seat(seat)).unwrap().cards.fragments.set(kind, n);
    }

    fn held(s: &CoreState, seat: u8, kind: T) -> i16 {
        s.players().by_seat(Seat(seat)).unwrap().cards.fragments.get(kind).unwrap_or(0)
    }

    fn driver(mode: FabMode) -> Driver<Fabrication> {
        Driver::new(Fabrication::new(Seat(0), mode, Code(5)), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    fn pick(kind: T) -> Pick {
        Pick::Opt(Opt::Relic(ROpt::Fragment(kind)))
    }

    #[test]
    fn fabrication_asks_the_type_among_those_that_qualify_spends_matching_fragments_first_and_gains_the_top_relic() {
        let k = Kernel::plain();
        let mut s = table();
        give(&mut s, 0, T::Cultural, 2);
        give(&mut s, 0, T::Hazardous, 1);
        give(&mut s, 0, T::Frontier, 1);
        let mut d = driver(FabMode::Relic { bmf: false });
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("two types qualify") };
        assert_eq!(q.ask, Ask::Relic(RAsk::FragmentType { count: 2 }));
        assert_eq!(q.opts.as_slice(), &[Opt::Relic(ROpt::Fragment(T::Cultural)), Opt::Relic(ROpt::Fragment(T::Hazardous))], "the frontier fragment makes up the hazardous pair");
        assert_eq!(d.resume(&mut s, &k, pick(T::Industrial)), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(DriverError::Illegal));
        assert_eq!(held(&s, 0, T::Cultural), 2, "nothing before a legal answer");
        assert_eq!(d.resume(&mut s, &k, pick(T::Hazardous)).unwrap(), Status::Done);
        assert!(d.window.done);
        assert_eq!((held(&s, 0, T::Hazardous), held(&s, 0, T::Frontier), held(&s, 0, T::Cultural)), (0, 0, 2));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice(), [Relic::ALL[3]]);
    }

    #[test]
    fn fabrication_with_a_lone_type_or_only_frontier_fragments_asks_nothing_and_with_none_or_no_relic_does_nothing() {
        let k = Kernel::plain();
        let mut s = table();
        give(&mut s, 0, T::Frontier, 3);
        let mut d = driver(FabMode::Relic { bmf: false });
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done, "only frontier fragments: one purge, no question");
        assert_eq!((held(&s, 0, T::Frontier), d.window.done), (1, true));
        // One short.
        let mut s = table();
        give(&mut s, 0, T::Cultural, 1);
        let mut d = driver(FabMode::Relic { bmf: false });
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        // No relic left: the fragments stay.
        let mut s = table();
        give(&mut s, 0, T::Cultural, 2);
        s.decks_mut().relic.set(vec![]);
        let mut d = driver(FabMode::Relic { bmf: false });
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((held(&s, 0, T::Cultural), d.window.done), (2, false));
    }

    #[test]
    fn fabrication_scores_the_shard_at_once_and_black_market_forgery_sends_its_note_home() {
        let k = Kernel::plain();
        let mut s = table();
        give(&mut s, 0, T::Cultural, 2);
        s.decks_mut().relic.set(vec![crate::kernel::fx_relic::SHARD]);
        let mut d = driver(FabMode::Relic { bmf: false });
        d.run(&mut s, &k).unwrap();
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.victory_points, 1);
        // The note of the other seat's faction, held; it goes home to its owner after the relic.
        let note = NoteKey { note: PromissoryNote::BlackMarketForgery, faction: Faction::ALL[1] };
        let mut s = table();
        give(&mut s, 0, T::Cultural, 2);
        s.cards_mut().promissory.edit().insert(note, Seat(0));
        let mut d = driver(FabMode::Relic { bmf: true });
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(s.cards().promissory.get(&note), Some(&Seat(1)));
        assert!(d.window.done);
        // Without the note the action does not resolve and nothing is paid.
        let mut s = table();
        give(&mut s, 0, T::Cultural, 2);
        let mut d = driver(FabMode::Relic { bmf: true });
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((held(&s, 0, T::Cultural), d.window.done), (2, false));
        // The seat's own faction's note is not the holder's to play.
        let own = NoteKey { note: PromissoryNote::BlackMarketForgery, faction: Faction::ALL[0] };
        let mut s = table();
        give(&mut s, 0, T::Cultural, 2);
        s.cards_mut().promissory.edit().insert(own, Seat(0));
        let mut d = driver(FabMode::Relic { bmf: true });
        d.run(&mut s, &k).unwrap();
        assert!(!d.window.done);
    }

    #[test]
    fn fabrication_turns_one_fragment_into_a_token_of_the_chosen_pool_and_needs_room_in_the_reinforcements() {
        let k = Kernel::plain();
        let mut s = table();
        give(&mut s, 0, T::Industrial, 1);
        give(&mut s, 0, T::Frontier, 2);
        let tokens = |s: &CoreState| {
            let e = &s.players().by_seat(Seat(0)).unwrap().econ;
            (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
        };
        let before = tokens(&s);
        let mut d = driver(FabMode::Token);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("two kinds are held") };
        assert_eq!(q.ask, Ask::Relic(RAsk::FragmentType { count: 1 }));
        assert_eq!(q.opts.as_slice(), &[Opt::Relic(ROpt::Fragment(T::Industrial)), Opt::Relic(ROpt::Fragment(T::Frontier))]);
        let Status::Asking(p) = d.resume(&mut s, &k, pick(T::Frontier)).unwrap() else { panic!("which pool") };
        assert_eq!(p.ask, Ask::Relic(RAsk::TokenPool));
        assert_eq!(p.opts.len(), 3);
        assert_eq!(held(&s, 0, T::Frontier), 2, "nothing is paid before the pool");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Relic(ROpt::Pool(Pool::Fleet)))).unwrap(), Status::Done);
        assert!(d.window.done);
        assert_eq!((held(&s, 0, T::Frontier), held(&s, 0, T::Industrial)), (1, 1));
        assert_eq!(tokens(&s), (before.0, before.1 + 1, before.2));
        // A lone kind goes straight to the pool; a full command sheet cannot gain one.
        let mut s = table();
        give(&mut s, 0, T::Hazardous, 1);
        let mut d = driver(FabMode::Token);
        let Status::Asking(p) = d.run(&mut s, &k).unwrap() else { panic!("the pool") };
        assert_eq!(p.ask, Ask::Relic(RAsk::TokenPool));
        let mut s = table();
        give(&mut s, 0, T::Hazardous, 1);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.tactic_tokens = 16;
        let mut d = driver(FabMode::Token);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((held(&s, 0, T::Hazardous), d.window.done), (1, false));
    }
}
