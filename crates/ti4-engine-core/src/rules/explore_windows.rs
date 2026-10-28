//! The exploration questions (package B5; string `exploration.rs`) as [`Window`]s: the deck choice of a
//! two-trait planet, Distant Suns' pick of one of two cards, the Ion Storm's faceup side and the reward
//! choice of the instant cards.
//!
//! Each is the string code between "decide whether to ask" and "act on the answer", split at its one
//! question: `step` runs to the question, `answer` is the only place an answer changes the position. What the
//! window leaves to the string caller (bails of the adapter, `explore_write`): the options of a reward
//! question and the effect behind them (commodities, trade goods, an action card, a mech or infantry placed:
//! other areas' rules, see `docs/ENGINE_KNOWN_ISSUES.md`), and the hook that decides Distant Suns applies.

use crate::choice::agenda::Shown;
use crate::choice::explore::{Ask as EAsk, Opt as EOpt, Step, Subtype as ESub};
use crate::choice::relic::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Explore, ExploreType, Planet, Relic, System, Wormhole};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board;
use crate::state::{CoreState, RoundKind};
use crate::tables::Scope;

use super::explore;

fn ctx(origin: Content, subtype: ESub) -> Ctx {
    Ctx {
        origin: Origin::Content(origin),
        subtype: Subtype::Explore(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

fn opts(list: impl IntoIterator<Item = EOpt>) -> Opts {
    list.into_iter().map(Opt::Explore).collect::<Opts>()
}

/// The suspension points of the windows of this file: at the start, at the question, finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XStage {
    Start,
    Asking,
    Done,
}

// -- which deck ---------------------------------------------------------------------------------------

/// `exploration::choose_deck` for a planet with the given traits now (printed ones and attachments'):
/// none gives no deck, one gives it without a question, more ask. `chosen` is the deck; the string code
/// falls back to the first trait only when nobody answers, which the adapter does not follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseDeck {
    pub who: Seat,
    traits: SmallList<ExploreType, 4>,
    stage: XStage,
    pub chosen: Option<ExploreType>,
}

impl ChooseDeck {
    #[must_use]
    pub fn new(who: Seat, traits: &[ExploreType]) -> Self {
        Self { who, traits: SmallList::from_slice(traits), stage: XStage::Start, chosen: None }
    }

    const ASK: Ask = Ask::Explore(EAsk::ChooseDeck);
}

impl Window for ChooseDeck {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            match self.traits.as_slice() {
                [] => self.stage = XStage::Done,
                [only] => {
                    self.chosen = Some(*only);
                    self.stage = XStage::Done;
                }
                _ => self.stage = XStage::Asking,
            }
        }
        match self.stage {
            XStage::Asking => Next::Continue(Self::ASK),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: ctx(Content::Step(Step::Exploration), ESub::Step(Step::Exploration)),
            opts: opts(self.traits.as_slice().iter().map(|t| EOpt::Deck(*t))),
        })
    }

    fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Explore(EOpt::Deck(t)))) if self.traits.as_slice().contains(&t) => {
                self.chosen = Some(t);
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Distant Suns -------------------------------------------------------------------------------------

/// Naaz Distant Suns: "draw 1 additional card, resolve 1 of the two, discard the other". The first card is
/// drawn already; this draws the second (nothing to ask when the deck is empty) and asks which to resolve.
/// `chosen` is the card to resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DistantSuns {
    pub who: Seat,
    deck: ExploreType,
    first: Explore,
    second: Option<Explore>,
    stage: XStage,
    pub chosen: Explore,
}

impl DistantSuns {
    #[must_use]
    pub const fn new(who: Seat, deck: ExploreType, first: Explore) -> Self {
        Self { who, deck, first, second: None, stage: XStage::Start, chosen: first }
    }
}

impl Window for DistantSuns {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            self.second = explore::draw(sx.state, self.deck);
            self.stage = if self.second.is_some() { XStage::Asking } else { XStage::Done };
        }
        match (self.stage, self.second) {
            (XStage::Asking, Some(second)) => Next::Continue(Ask::Explore(EAsk::DistantSuns { first: self.first, second })),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        let second = self.second.filter(|_| self.stage == XStage::Asking)?;
        Some(Choice {
            actor: self.who,
            ask: Ask::Explore(EAsk::DistantSuns { first: self.first, second }),
            ctx: ctx(Content::Explore(self.first), ESub::Card(self.first)),
            opts: opts([EOpt::Card(self.first), EOpt::Card(second)]),
        })
    }

    fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, self.second, pick) {
            (XStage::Asking, Some(second), Pick::Opt(Opt::Explore(EOpt::Card(c)))) if c == self.first || c == second => {
                self.chosen = c;
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- the Ion Storm ------------------------------------------------------------------------------------

/// "Place the ion storm token in this system with either side faceup": asks the side, then places it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IonStorm {
    pub who: Seat,
    system: System,
    stage: XStage,
    pub face: Option<Wormhole>,
}

impl IonStorm {
    #[must_use]
    pub const fn new(who: Seat, system: System) -> Self {
        Self { who, system, stage: XStage::Start, face: None }
    }

    const ASK: Ask = Ask::Explore(EAsk::IonStorm);
}

impl Window for IonStorm {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            self.stage = XStage::Asking;
        }
        if self.stage == XStage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: ctx(Content::Step(Step::IonStorm), ESub::Step(Step::IonStorm)),
            opts: opts([EOpt::Face(Wormhole::Alpha), EOpt::Face(Wormhole::Beta)]),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Explore(EOpt::Face(f)))) if matches!(f, Wormhole::Alpha | Wormhole::Beta) => {
                explore::place_ion_storm(sx.state, self.system, f);
                self.face = Some(f);
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- the reward of an instant card --------------------------------------------------------------------

/// The reward question of an instant card. The options are the string side's (they depend on holdings the
/// card's other effects read); `skip_single` is `ask_unless_single`: one option is taken unasked.
/// `chosen` is the option taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reward {
    pub who: Seat,
    pub card: Step,
    options: SmallList<EOpt, 3>,
    skip_single: bool,
    stage: XStage,
    pub chosen: Option<EOpt>,
}

impl Reward {
    /// `card` is one of the five reward steps.
    #[must_use]
    pub fn new(who: Seat, card: Step, options: &[EOpt], skip_single: bool) -> Self {
        Self { who, card, options: SmallList::from_slice(options), skip_single, stage: XStage::Start, chosen: None }
    }
}

impl Window for Reward {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            if self.skip_single && self.options.len() == 1 {
                self.chosen = Some(self.options.as_slice()[0]);
                self.stage = XStage::Done;
            } else {
                self.stage = XStage::Asking;
            }
        }
        if self.stage == XStage::Asking { Next::Continue(Ask::Explore(EAsk::Reward(self.card))) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Explore(EAsk::Reward(self.card)),
            ctx: ctx(Content::Step(self.card), ESub::Step(self.card)),
            opts: opts(self.options.as_slice().iter().copied()),
        })
    }

    fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Explore(o))) if self.options.as_slice().contains(&o) => {
                self.chosen = Some(o);
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- relics that ask ----------------------------------------------------------------------------------

fn relic_ctx(relic: Relic, subtype: RSub) -> Ctx {
    Ctx {
        origin: Origin::Content(Content::Relic(relic)),
        subtype: Subtype::Relic(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

fn holds(core: &CoreState, who: Seat, relic: Relic) -> bool {
    core.players().by_seat(who).is_some_and(|p| p.cards.relics.as_slice().contains(&relic))
}

/// `relics::offer_dominus_orb`: "before you move units, you may purge this card to move out of systems that
/// hold your own command tokens". Offered while a command token of the holder pins units of theirs outside
/// the active system. `used` is the string function's answer (the card was purged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DominusOrb {
    pub who: Seat,
    stage: XStage,
    pub used: bool,
}

impl DominusOrb {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: XStage::Start, used: false }
    }

    const ASK: Ask = Ask::Relic(RAsk::DominusOrb);

    fn pinned(&self, core: &CoreState) -> bool {
        let active = core.meta().active_system;
        board::systems_with_token(core, self.who).iter().filter_map(System::from_index).any(|system| {
            Some(system) != active && board::has_space_units_of(core, system, self.who)
        })
    }
}

impl Window for DominusOrb {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            self.stage = if holds(sx.state, self.who, Relic::DominusOrb) && self.pinned(sx.state) {
                XStage::Asking
            } else {
                XStage::Done
            };
        }
        if self.stage == XStage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: relic_ctx(Relic::DominusOrb, RSub::DominusOrbPurgeToMove),
            opts: [Opt::Relic(ROpt::Purge), Opt::Decline].into_iter().collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Relic(ROpt::Purge))) => {
                let activation = sx.state.meta().activation_seq;
                if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                    p.round_marks.push(RoundKind::DominusOrb, activation);
                }
                explore::purge_relic(sx.state, self.who, Relic::DominusOrb);
                self.used = true;
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

/// `relics::crown_of_emphidia_explore_with`, up to the choice: a ready Crown, a planet the holder controls
/// that has a trait to explore. The answer exhausts the Crown; the exploring itself is the string caller's
/// (`planet` is the chosen planet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrownExplore {
    pub who: Seat,
    scope: Scope,
    stage: XStage,
    pub planet: Option<Planet>,
}

impl CrownExplore {
    #[must_use]
    pub const fn new(who: Seat, scope: Scope) -> Self {
        Self { who, scope, stage: XStage::Start, planet: None }
    }

    const ASK: Ask = Ask::Relic(RAsk::CrownOfEmphidia);

    fn options(&self, core: &CoreState) -> Vec<Planet> {
        board::controlled_planets(core, self.who)
            .as_slice()
            .iter()
            .map(|(_, p)| *p)
            .filter(|p| {
                let d = p.data();
                crate::tables::source_bit(d.source) & self.scope.0 != 0 && !d.traits.is_empty()
            })
            .collect()
    }
}

impl Window for CrownExplore {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            let ready = crate::query::relic::ready(sx.state, self.who, Relic::TheCrownOfEmphidia);
            self.stage = if ready && !self.options(sx.state).is_empty() { XStage::Asking } else { XStage::Done };
        }
        if self.stage == XStage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: relic_ctx(Relic::TheCrownOfEmphidia, RSub::CrownOfEmphidiaChoosePlanet),
            opts: self
                .options(s)
                .into_iter()
                .map(|p| Opt::Relic(ROpt::Planet(p, Shown { votes: 0, system: board::system_of(s, self.scope, p) })))
                .chain(std::iter::once(Opt::Decline))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Relic(ROpt::Planet(p, _)))) if self.options(sx.state).contains(&p) => {
                if explore::exhaust_relic(sx.state, self.who, Relic::TheCrownOfEmphidia) {
                    self.planet = Some(p);
                }
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
