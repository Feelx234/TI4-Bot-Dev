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
use crate::choice::explore::{Ask as EAsk, Opt as EOpt, PayReward, Step, Subtype as ESub};
use crate::choice::relic::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Explore, ExploreType, Planet, Relic, System, UnitBaseType, Wormhole};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board;
use crate::state::{CoreState, RoundKind};
use crate::tables::Scope;

use super::explore;

fn ctx(origin: Content, subtype: ESub, inv: Option<u64>) -> Ctx {
    Ctx {
        origin: Origin::Content(origin),
        subtype: Subtype::Explore(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: inv,
    }
}

fn opts(list: impl IntoIterator<Item = EOpt>) -> Opts {
    list.into_iter().map(Opt::Explore).collect::<Opts>()
}

/// A window whose question is asked while an invasion is active carries the invasion's sequence number
/// (the string `Table::ask_seeing` adds it to every context with no other target).
pub trait InInvasion: Sized {
    #[must_use]
    fn in_invasion(self, seq: Option<u64>) -> Self;
}

macro_rules! in_invasion {
    ($($t:ty),*) => {$(
        impl InInvasion for $t {
            fn in_invasion(mut self, seq: Option<u64>) -> Self {
                self.inv = seq;
                self
            }
        }
    )*};
}
in_invasion!(ChooseDeck, DistantSuns, IonStorm, Reward, DominusOrb, CrownExplore, PayMechOrInfantry);
pub(super) use in_invasion;

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
    inv: Option<u64>,
}

impl ChooseDeck {
    #[must_use]
    pub fn new(who: Seat, traits: &[ExploreType]) -> Self {
        Self { who, traits: SmallList::from_slice(traits), stage: XStage::Start, chosen: None, inv: None }
    }

    /// The window as it stands at its question (an exploration that stored its progress resumes it): more than one trait.
    #[must_use]
    pub fn asking(who: Seat, traits: &[ExploreType]) -> Self {
        Self { who, traits: SmallList::from_slice(traits), stage: XStage::Asking, chosen: None, inv: None }
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
            ctx: ctx(Content::Step(Step::Exploration), ESub::Step(Step::Exploration), self.inv),
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
    inv: Option<u64>,
}

impl DistantSuns {
    #[must_use]
    pub const fn new(who: Seat, deck: ExploreType, first: Explore) -> Self {
        Self { who, deck, first, second: None, stage: XStage::Start, chosen: first, inv: None }
    }

    /// The second card drawn (once the window stepped).
    #[must_use]
    pub const fn second_card(&self) -> Option<Explore> {
        self.second
    }

    /// The window as it stands at its question, the second card drawn already.
    #[must_use]
    pub const fn asking(who: Seat, deck: ExploreType, first: Explore, second: Explore) -> Self {
        Self { who, deck, first, second: Some(second), stage: XStage::Asking, chosen: first, inv: None }
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
            ctx: ctx(Content::Explore(self.first), ESub::Card(self.first), self.inv),
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
    inv: Option<u64>,
}

impl IonStorm {
    #[must_use]
    pub const fn new(who: Seat, system: System) -> Self {
        Self { who, system, stage: XStage::Start, face: None, inv: None }
    }

    /// The window as it stands at its question.
    #[must_use]
    pub const fn asking(who: Seat, system: System) -> Self {
        Self { who, system, stage: XStage::Asking, face: None, inv: None }
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
            ctx: ctx(Content::Step(Step::IonStorm), ESub::Step(Step::IonStorm), self.inv),
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
    inv: Option<u64>,
}

impl Reward {
    /// `card` is one of the five reward steps.
    #[must_use]
    pub fn new(who: Seat, card: Step, options: &[EOpt], skip_single: bool) -> Self {
        Self { who, card, options: SmallList::from_slice(options), skip_single, stage: XStage::Start, chosen: None, inv: None }
    }

    /// The window as it stands at its question.
    #[must_use]
    pub fn asking(who: Seat, card: Step, options: &[EOpt]) -> Self {
        Self { who, card, options: SmallList::from_slice(options), skip_single: false, stage: XStage::Asking, chosen: None, inv: None }
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
            ctx: ctx(Content::Step(self.card), ESub::Step(self.card), self.inv),
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

// -- a mech or an infantry pays -------------------------------------------------------------------------

/// `exploration::pay_with_mech_or_infantry`: "if you have at least 1 mech on this planet, or if you remove 1
/// infantry from it". A mech satisfies the card by being there; an infantry only when the seat agrees to remove
/// it (the question); neither (or a planet no system holds) cannot resolve it. `paid` is the string function's
/// answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayMechOrInfantry {
    pub who: Seat,
    card: Explore,
    planet: Planet,
    reward: PayReward,
    stage: XStage,
    pub paid: bool,
    inv: Option<u64>,
}

impl PayMechOrInfantry {
    #[must_use]
    pub const fn new(who: Seat, card: Explore, planet: Planet, reward: PayReward) -> Self {
        Self { who, card, planet, reward, stage: XStage::Start, paid: false, inv: None }
    }

    /// The window as it stands at its question (an infantry is there to remove).
    #[must_use]
    pub const fn asking(who: Seat, card: Explore, planet: Planet, reward: PayReward) -> Self {
        Self { who, card, planet, reward, stage: XStage::Asking, paid: false, inv: None }
    }

    const fn ask(&self) -> Ask {
        Ask::Explore(EAsk::PayInfantry { card: self.card, planet: self.planet, reward: self.reward })
    }

    /// The position of the seat's first infantry in the planet's unit list.
    fn infantry(&self, core: &CoreState) -> Option<usize> {
        board::planet_units(core, self.planet)
            .iter()
            .position(|u| u.owner() == self.who && u.kind().data().base_type == UnitBaseType::Infantry)
    }
}

impl Window for PayMechOrInfantry {
    type Stage = XStage;

    fn stage(&self) -> XStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == XStage::Start {
            let cell = sx.state.board().planet(self.planet);
            self.stage = if !(cell.flags.has(crate::state::PlanetFlags::UNITS_ENTRY) || cell.control.is_some()) {
                XStage::Done
            } else if board::planet_units(sx.state, self.planet)
                .iter()
                .any(|u| u.owner() == self.who && u.kind().data().base_type == UnitBaseType::Mech)
            {
                self.paid = true;
                XStage::Done
            } else if self.infantry(sx.state).is_some() {
                XStage::Asking
            } else {
                XStage::Done
            };
        }
        if self.stage == XStage::Asking { Next::Continue(self.ask()) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == XStage::Asking).then(|| Choice {
            actor: self.who,
            ask: self.ask(),
            ctx: ctx(Content::Explore(self.card), ESub::Card(self.card), self.inv),
            opts: opts([EOpt::RemoveInfantry, EOpt::KeepInfantry]),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (XStage::Asking, Pick::Opt(Opt::Explore(EOpt::RemoveInfantry))) => {
                let at = self.infantry(sx.state).ok_or(Illegal)?;
                explore::remove_unit_at(sx.state, self.planet, at);
                self.paid = true;
                self.stage = XStage::Done;
                Ok(())
            }
            (XStage::Asking, Pick::Opt(Opt::Explore(EOpt::KeepInfantry))) => {
                self.stage = XStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- relics that ask ----------------------------------------------------------------------------------

pub(super) fn relic_ctx(relic: Relic, subtype: RSub, inv: Option<u64>) -> Ctx {
    Ctx {
        origin: Origin::Content(Content::Relic(relic)),
        subtype: Subtype::Relic(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: inv,
    }
}

pub(super) fn holds(core: &CoreState, who: Seat, relic: Relic) -> bool {
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
    inv: Option<u64>,
}

impl DominusOrb {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: XStage::Start, used: false, inv: None }
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
            ctx: relic_ctx(Relic::DominusOrb, RSub::DominusOrbPurgeToMove, self.inv),
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
    inv: Option<u64>,
}

impl CrownExplore {
    #[must_use]
    pub const fn new(who: Seat, scope: Scope) -> Self {
        Self { who, scope, stage: XStage::Start, planet: None, inv: None }
    }

    const ASK: Ask = Ask::Relic(RAsk::CrownOfEmphidia);

    /// A ready Crown and a planet to explore: the window would ask.
    #[must_use]
    pub fn offers(core: &CoreState, who: Seat, scope: Scope) -> bool {
        crate::query::relic::ready(core, who, Relic::TheCrownOfEmphidia) && !Self::new(who, scope).options(core).is_empty()
    }

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
            ctx: relic_ctx(Relic::TheCrownOfEmphidia, RSub::CrownOfEmphidiaChoosePlanet, self.inv),
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

// -- one exploration ------------------------------------------------------------------------------------------

use crate::choice::strategy::{Ask as SAsk, Opt as SOpt};
use crate::generated::{Attachment, ExploreResolution};
use crate::kernel::children::{ErasedChild, Report, Spawn};
use crate::kernel::stage::Ran;
use crate::kernel::{fx, fx_explore, fx_strategy};
use crate::query::explore as qexplore;
use crate::state::{EDeck, EHow, EStage, EThen, ETarget, EWhy, ExploreRun};

/// `ti4_model::state::RULES_COMMAND_TOKEN_POOL`: from this version each command token an exploration card gives goes into a pool of the
/// seat's choice.
const RULES_COMMAND_TOKEN_POOL: u32 = 12;
/// `ti4_model::state::RULES_MIRAGE_FRONTIER`: from this version the Mirage drawn from a frontier token is placed in the token's system.
const RULES_MIRAGE_FRONTIER: u32 = 27;
/// `ti4_model::state::RULES_NO_EMPTY_CONVERT`: from this version a seat without commodities has nothing to convert.
const RULES_NO_EMPTY_CONVERT: u32 = 19;
/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`.
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;
/// `ti4_model::state::RULES_DMZ_EXPLORATION`.
const RULES_DMZ_EXPLORATION: u32 = 34;

/// The research facility's second form: on a planet that prints a technology specialty it is the `...stat` record (+1 resource and +1
/// influence), otherwise the record that gives the planet that specialty.
#[must_use]
pub const fn stat_form(attachment: Attachment) -> Attachment {
    match attachment {
        Attachment::BioticResearchFacility => Attachment::BioticResearchFacilityStat,
        Attachment::CyberneticResearchFacility => Attachment::CyberneticResearchFacilityStat,
        Attachment::PropulsionResearchFacility => Attachment::PropulsionResearchFacilityStat,
        Attachment::WarfareResearchFacility => Attachment::WarfareResearchFacilityStat,
        other => other,
    }
}

/// `exploration::attachment_for`: the attachment an `Attach` card puts on `planet`.
#[must_use]
pub fn attachment_for(card: Explore, planet: Planet) -> Option<Attachment> {
    let attachment = card.data().attachment?;
    Some(if qexplore::has_specialty(planet) { stat_form(attachment) } else { attachment })
}

/// `planets::traits_now`: the decks a planet explores into, its printed traits (in the scope) and then the traits an attachment gives.
#[must_use]
pub fn traits_now(core: &CoreState, scope: Scope, planet: Planet) -> SmallList<ExploreType, 4> {
    let mut out: SmallList<ExploreType, 4> = SmallList::new();
    for t in qexplore::traits_of(scope, planet).as_slice() {
        out.push(*t);
    }
    for attachment in board::attachments(core, planet).as_slice() {
        for t in attachment.data().traits {
            if let Some(deck) = qexplore::deck_of(*t)
                && !out.as_slice().contains(&deck)
            {
                out.push(deck);
            }
        }
    }
    out
}

/// `exploration::explore_in` (with the deck choice of its callers, `exploration::choose_deck`): one planet or frontier token explored.
/// A planet's deck is the seat's choice among its traits now (or given); the card is drawn (Naaz Distant Suns draws a second and asks
/// which to resolve), the log row opened, the card resolved (relic fragments, attachments with the Demilitarized Zone, every instant
/// card with the questions it asks and the windows of other areas it runs as children), the row closed with the adapter's words
/// ([`fx_explore::WordsFn`]), and for a planet the faction hooks and `PLANET_EXPLORED` follow. The data is [`ExploreRun`]: a host that
/// keeps an exploration between two questions (the invasion's first claim) stores it and resumes with [`Exploring::hosted`].
///
/// Left to the host, as a counted bail ([`ExploreRun::why`]): Freelancers (a unit produced with influence), Titans' Terragenesis, and
/// (when `hosted`) the questions of a window of another area (the discard of a draw over the hand limit, the secret objective's hand limit).
/// [`ExploreRun::empty`]: the deck ran out, nothing was explored.
#[derive(Debug, Clone)]
pub struct Exploring {
    run: ExploreRun,
    /// A window of another area (the draw of action cards, the hand limit).
    linked: ErasedChild,
    /// The window is kept by its payload: a child that asks cannot be kept.
    hosted: bool,
    /// How Freelancers' unit is asked and run (`ProduceOne` is the production area's: a host supplies the link, a window that steps takes the table's).
    one: Option<crate::kernel::children::OneLink>,
}

/// The most questions a child of another area may put to the tape in one step (a draw's discards, a hand limit's returns).
const LINKED_TAPE_LIMIT: u8 = 16;

impl Exploring {
    /// A planet explored; `deck` is the seat's choice or given.
    #[must_use]
    pub fn planet(who: Seat, planet: Planet, deck: EDeck, announce: bool) -> Self {
        Self::new(ExploreRun::new(who, ETarget::Planet(planet), deck, announce))
    }

    /// A frontier token explored; `take_token`: the token is spent first (the caller has not spent it); `named`: the exploration names the
    /// system of the token (the log row, a Mirage), as the string's `explore_frontier` does and Exploration Probe's draw does not.
    #[must_use]
    pub fn frontier(who: Seat, system: System, announce: bool, take_token: bool, named: bool) -> Self {
        let mut run = ExploreRun::new(who, ETarget::Frontier(system), EDeck::Fixed(ExploreType::Frontier), announce);
        run.take_token = take_token;
        run.named = named;
        Self::new(run)
    }

    #[must_use]
    pub fn new(run: ExploreRun) -> Self {
        Self { run, linked: ErasedChild::idle(), hosted: false, one: None }
    }

    /// The exploration as a host that keeps [`ExploreRun`] in a payload runs it: a child of another area that asks is not carried.
    #[must_use]
    pub fn hosted(run: ExploreRun) -> Self {
        Self { run, linked: ErasedChild::idle(), hosted: true, one: None }
    }

    /// The link to `ProduceOne` for a window that is asked before it steps (a host that rebuilds the window from its payload).
    #[must_use]
    pub const fn with_one(mut self, link: crate::kernel::children::OneLink) -> Self {
        self.one = Some(link);
        self
    }

    /// The sequence number of the invasion the position is in (the string `Table::ask_seeing` puts it on every question asked meanwhile).
    fn inv(s: &CoreState) -> Option<u64> {
        if s.loaded.has(crate::state::Slice::Transient) { s.transient().invasion.active.as_ref().map(|a| a.seq) } else { None }
    }

    #[must_use]
    pub const fn run(&self) -> &ExploreRun {
        &self.run
    }

    #[must_use]
    pub fn into_run(self) -> ExploreRun {
        self.run
    }

    /// The card was resolved and its row closed.
    #[must_use]
    pub const fn explored(&self) -> bool {
        self.run.finished
    }

    /// The deck ran out: nothing was explored.
    #[must_use]
    pub const fn empty(&self) -> bool {
        self.run.empty
    }

    /// Why the exploration is not carried, if it is not.
    #[must_use]
    pub const fn unsupported(&self) -> Option<EWhy> {
        self.run.why
    }

    fn planet_of(&self) -> Option<Planet> {
        match self.run.target {
            ETarget::Planet(p) => Some(p),
            ETarget::Frontier(_) => None,
        }
    }

    fn scope(k: &Kernel) -> Scope {
        Scope(u32::from(k.sources.0))
    }

    fn fail(&mut self, why: EWhy) {
        self.run.why = Some(why);
        self.run.stage = EStage::Done;
    }

    // -- the children, as they stand at their question ----------------------------------------------------------

    fn deck_child(&self, s: &CoreState) -> ChooseDeck {
        ChooseDeck::asking(self.run.who, self.run.traits.as_slice()).in_invasion(Self::inv(s))
    }

    fn suns_child(&self, s: &CoreState) -> Option<DistantSuns> {
        let (deck, first, second) = (self.run.deck?, self.run.card?, self.run.second?);
        Some(DistantSuns::asking(self.run.who, deck, first, second).in_invasion(Self::inv(s)))
    }

    fn ion_child(&self, s: &CoreState) -> Option<IonStorm> {
        let planet = self.planet_of()?;
        Some(IonStorm::asking(self.run.who, planet.data().system?).in_invasion(Self::inv(s)))
    }

    fn reward_child(&self, s: &CoreState, then: EThen) -> Option<Reward> {
        Some(Reward::asking(self.run.who, ExploreRun::step_of(then)?, self.run.options.as_slice()).in_invasion(Self::inv(s)))
    }

    fn pay_child(&self, s: &CoreState, then: EThen) -> Option<PayMechOrInfantry> {
        let EThen::Paying(reward) = then else { return None };
        Some(PayMechOrInfantry::asking(self.run.who, self.run.card?, self.planet_of()?, reward).in_invasion(Self::inv(s)))
    }

    // -- the flow -------------------------------------------------------------------------------------------------

    fn start(&mut self, sx: &mut Sx<'_>) {
        if !sx.table.codes.explore.filled || sx.table.facts.explore_words.is_none() {
            return self.fail(EWhy::NoTexts);
        }
        match (self.run.target, self.run.deck_mode) {
            (_, EDeck::Fixed(deck)) => self.run.deck = Some(deck),
            (ETarget::Planet(planet), EDeck::Choose) => {
                let traits = traits_now(sx.state, Self::scope(sx.kernel), planet);
                match traits.as_slice() {
                    [] => {
                        self.run.stage = EStage::Done;
                        return;
                    }
                    [only] => self.run.deck = Some(*only),
                    _ => {
                        self.run.traits = traits;
                        self.run.stage = EStage::Deck;
                        return;
                    }
                }
            }
            (ETarget::Frontier(_), EDeck::Choose) => self.run.deck = Some(ExploreType::Frontier),
        }
        self.draw(sx);
    }

    /// The draw (and Distant Suns' second card): the row is opened once the card to resolve is known.
    fn draw(&mut self, sx: &mut Sx<'_>) {
        let who = self.run.who;
        let Some(deck) = self.run.deck else { return self.fail(EWhy::Draw) };
        if let ETarget::Frontier(system) = self.run.target
            && self.run.take_token
        {
            fx_explore::take_frontier(sx.state, system);
        }
        self.run.before = qexplore::holdings(sx.state, who, self.planet_of());
        let Some(card) = fx_explore::draw(sx.state, deck) else {
            self.run.empty = true;
            self.run.stage = EStage::Done;
            return;
        };
        self.run.card = Some(card);
        if let Some(planet) = self.planet_of()
            && sx.kernel.hooks.economy.explore_extra_draw(sx.state, sx.kernel, who, planet)
        {
            let mut suns = DistantSuns::new(who, deck, card).in_invasion(Self::inv(sx.state));
            match suns.step(sx) {
                Next::Continue(ask) => {
                    self.run.second = suns.second_card();
                    self.run.stage = EStage::Suns;
                    let _ = ask;
                    return;
                }
                _ => self.run.card = Some(suns.chosen),
            }
        }
        self.open(sx);
    }

    /// The row of the log opened before the card resolves, then the card.
    fn open(&mut self, sx: &mut Sx<'_>) {
        let (who, Some(deck), Some(card)) = (self.run.who, self.run.deck, self.run.card) else { return self.fail(EWhy::Draw) };
        let Some(empty) = fx_explore::empty_text(sx.state) else { return self.fail(EWhy::NoTexts) };
        let (planet, system) = match self.run.target {
            ETarget::Planet(p) => (Some(p), None),
            ETarget::Frontier(s) => (None, self.run.named.then_some(s)),
        };
        self.run.row = u32::try_from(sx.state.explore().log.len()).ok();
        fx_explore::open_record(sx.state, who, deck, card, planet, system, empty);
        let data = card.data();
        match data.resolution {
            ExploreResolution::Fragment => {
                fx_explore::gain_fragment(sx.state, who, data.deck);
                self.run.how = EHow::Fragment(data.deck);
                self.close(sx);
            }
            ExploreResolution::Attach => match planet {
                // Discarded rather than silently applied to nothing.
                None => {
                    self.run.how = EHow::Discarded;
                    self.close(sx);
                }
                Some(planet) => {
                    if let Some(attachment) = attachment_for(card, planet) {
                        explore::attach(sx.state, planet, attachment);
                    }
                    if card == Explore::DemilitarizedZone && sx.state.meta().rules_at_least(RULES_DMZ_EXPLORATION) {
                        explore::demilitarize(sx.state, Self::scope(sx.kernel), planet);
                    }
                    self.run.how = EHow::Attached;
                    self.close(sx);
                }
            },
            ExploreResolution::Instant | ExploreResolution::Token => self.instant(sx, card),
        }
    }

    /// `exploration::resolve_instant`.
    fn instant(&mut self, sx: &mut Sx<'_>, card: Explore) {
        let who = self.run.who;
        let planet = self.planet_of();
        match card {
            Explore::MinorEntropicField => self.gain_tokens(sx, 1, 1),
            Explore::EntropicField => self.gain_tokens(sx, 1, 2),
            Explore::MajorEntropicField => self.gain_tokens(sx, 1, 3),
            Explore::KeleresShip1 | Explore::KeleresShip2 => self.gain_tokens(sx, 2, 0),
            Explore::GammaRelay | Explore::GammaWormhole => {
                match sx.state.meta().active_system {
                    Some(here) => {
                        let fresh = sx.state.explore().wormhole_tokens.get(&Wormhole::Gamma) != Some(&here);
                        fx_explore::place_wormhole_token(sx.state, Wormhole::Gamma, here);
                        self.run.gamma = fresh.then_some(here);
                    }
                    None => self.run.how = EHow::Unresolved,
                }
                self.close(sx);
            }
            Explore::IonStorm => {
                match self.ion_child(sx.state) {
                    Some(mut ion) => match ion.step(sx) {
                        Next::Continue(_) => self.run.stage = EStage::Ion,
                        _ => self.close(sx),
                    },
                    None => {
                        self.run.how = EHow::Unresolved;
                        self.close(sx);
                    }
                }
            }
            Explore::EnigmaticDevice1 | Explore::EnigmaticDevice2 => {
                fx_explore::play_area_card(sx.state, who, card);
                self.close(sx);
            }
            Explore::Mirage => {
                let system = match planet {
                    Some(here) => here.data().system,
                    None => match self.run.target {
                        ETarget::Frontier(system) if self.run.named && sx.state.meta().rules_at_least(RULES_MIRAGE_FRONTIER) => Some(system),
                        _ => None,
                    },
                };
                if !system.is_some_and(|system| fx_explore::place_planet(sx.state, who, system, Planet::Mirage)) {
                    self.run.how = EHow::Unresolved;
                }
                self.close(sx);
            }
            Explore::Freelancers1 | Explore::Freelancers2 | Explore::Freelancers3 => {
                match planet.and_then(|p| p.data().system) {
                    None if planet.is_none() => {
                        self.run.how = EHow::Unresolved;
                        self.close(sx);
                    }
                    None => self.fail(EWhy::Freelancers),
                    Some(system) => self.freelancers(sx, card, system),
                }
            }
            Explore::DeadWorld => {
                crate::kernel::fx_relic::relic_gain(sx.state, who);
                self.close(sx);
            }
            Explore::AbandonedWarehouses1 | Explore::AbandonedWarehouses2 | Explore::AbandonedWarehouses3 | Explore::AbandonedWarehouses4 => {
                let mut options = vec![EOpt::Gain];
                if self.can_offer_convert(sx) {
                    options.push(EOpt::Convert);
                }
                self.ask_reward(sx, EThen::Warehouses, &options, true);
            }
            Explore::MerchantStation1 | Explore::MerchantStation2 => {
                let mut options = vec![EOpt::Replenish];
                if self.can_offer_convert(sx) {
                    options.push(EOpt::Convert);
                }
                self.ask_reward(sx, EThen::Merchant, &options, true);
            }
            Explore::FunctioningBase1 | Explore::FunctioningBase2 | Explore::FunctioningBase3 | Explore::FunctioningBase4 => {
                let (goods, commodities) = self.econ(sx);
                let mut options = vec![EOpt::Gain];
                if goods >= 1 {
                    options.push(EOpt::SpendTradeGood);
                }
                if commodities >= 1 {
                    options.push(EOpt::SpendCommodity);
                }
                self.ask_reward(sx, EThen::FunctioningBase, &options, false);
            }
            Explore::LocalFabricators1 | Explore::LocalFabricators2 | Explore::LocalFabricators3 | Explore::LocalFabricators4 => {
                if planet.is_none() {
                    return self.close(sx);
                }
                let (goods, commodities) = self.econ(sx);
                let forbidden = self.mech_forbidden(sx);
                let mut options = vec![EOpt::Gain];
                if goods >= 1 && !forbidden {
                    options.push(EOpt::SpendTradeGood);
                }
                if commodities >= 1 && !forbidden {
                    options.push(EOpt::SpendCommodity);
                }
                self.ask_reward(sx, EThen::Fabricators, &options, false);
            }
            Explore::MercenaryOutfit1 | Explore::MercenaryOutfit2 | Explore::MercenaryOutfit3 => {
                if planet.is_none() {
                    return self.close(sx);
                }
                self.ask_reward(sx, EThen::Mercenaries, &[EOpt::Place, EOpt::PlaceNothing], false); // mutants:skip (two options: nothing is skipped)
            }
            Explore::CoreMine1 | Explore::CoreMine2 | Explore::CoreMine3 => self.pay(sx, PayReward::TradeGood),
            Explore::Expedition1 | Explore::Expedition2 | Explore::Expedition3 => self.pay(sx, PayReward::ReadyPlanet),
            Explore::VolatileFuelSource1 | Explore::VolatileFuelSource2 | Explore::VolatileFuelSource3 => self.pay(sx, PayReward::CommandToken),
            Explore::DerelictVessel1 | Explore::DerelictVessel2 => {
                if fx::draw_secret(sx.state, who).is_none() {
                    return self.close(sx);
                }
                self.link(sx, &Spawn::HandLimit { who }, EThen::Derelict);
            }
            Explore::LostCrew1 | Explore::LostCrew2 => {
                let request = Spawn::DrawCards { who, count: 2, drop_unofficial: sx.state.meta().rules_at_least(RULES_OFFICIAL_ACTION_DECK) };
                self.link(sx, &request, EThen::LostCrew);
            }
            _ => {
                self.run.how = EHow::Unresolved;
                self.close(sx);
            }
        }
    }

    fn econ(&self, sx: &Sx<'_>) -> (i32, i32) {
        sx.state.players().by_seat(self.run.who).map_or((0, 0), |p| (i32::from(p.econ.trade_goods), i32::from(p.econ.commodities))) // mutants:skip (the seat is always there)
    }

    /// From rules version 19 only a seat holding at least one commodity is offered the conversion.
    fn can_offer_convert(&self, sx: &Sx<'_>) -> bool {
        !sx.state.meta().rules_at_least(RULES_NO_EMPTY_CONVERT) || self.econ(sx).1 >= 1
    }

    /// Whether a faction effect forbids placing the seat's mech (the options that place one are not offered).
    fn mech_forbidden(&self, sx: &Sx<'_>) -> bool {
        let Some(roster) = sx.table.facts.roster else { return false };
        let who = self.run.who;
        fx_strategy::placed_unit(sx.state, sx.kernel, Self::scope(sx.kernel), roster, who, crate::generated::UnitBaseType::Mech)
            .is_some_and(|mech| sx.kernel.hooks.economy.effect_placement_forbidden(sx.state, sx.kernel, who, mech))
    }

    /// A reward question: one option left is taken unasked when `skip_single`.
    fn ask_reward(&mut self, sx: &mut Sx<'_>, then: EThen, options: &[EOpt], skip_single: bool) {
        self.run.options = SmallList::from_slice(options);
        let Some(step) = ExploreRun::step_of(then) else { return self.fail(EWhy::NoLink) };
        let mut reward = Reward::new(self.run.who, step, options, skip_single).in_invasion(Self::inv(sx.state));
        match reward.step(sx) {
            Next::Continue(_) => self.run.stage = EStage::Rewarding(then),
            _ => {
                if let Some(chosen) = reward.chosen {
                    self.rewarded(sx, then, chosen);
                } else {
                    self.close(sx);
                }
            }
        }
    }

    /// A mech or an infantry pays for the card (`exploration::pay_with_mech_or_infantry`).
    fn pay(&mut self, sx: &mut Sx<'_>, reward: PayReward) {
        let (Some(planet), Some(card)) = (self.planet_of(), self.run.card) else { return self.close(sx) };
        let then = EThen::Paying(reward);
        let mut pay = PayMechOrInfantry::new(self.run.who, card, planet, reward).in_invasion(Self::inv(sx.state));
        match pay.step(sx) {
            Next::Continue(_) => self.run.stage = EStage::Paying(then),
            _ => self.paid(sx, reward, pay.paid),
        }
    }

    fn paid(&mut self, sx: &mut Sx<'_>, reward: PayReward, paid: bool) {
        if paid {
            match reward {
                PayReward::TradeGood => self.gain_goods(sx, 1),
                PayReward::ReadyPlanet => {
                    if let Some(planet) = self.planet_of() {
                        sx.state.board_mut().planet_mut(planet).flags.set(crate::state::PlanetFlags::EXHAUSTED, false);
                    }
                }
                PayReward::CommandToken => return self.gain_tokens(sx, 1, 0),
            }
        }
        self.close(sx);
    }

    /// Trade goods gained and announced (the string's `gain_trade_goods_announced`).
    fn gain_goods(&mut self, sx: &mut Sx<'_>, amount: i32) {
        let who = self.run.who;
        let gained = fx::gain_trade_goods(sx.state, who, amount);
        if gained > 0 && self.run.announce {
            let _ = sx.emit(fx::trade_goods_gained(who, gained, sx.table.codes.explore.goods));
        }
    }

    /// `gain_command_tokens`, then the trade goods the card owes.
    fn gain_tokens(&mut self, sx: &mut Sx<'_>, count: u8, goods: i32) {
        self.run.goods = goods;
        // (A command token in a Mahact fleet pool is not in the reinforcements: `fx::tokens_in_reinforcements` reads the marks.)
        if count > 0 && sx.state.meta().rules_at_least(RULES_COMMAND_TOKEN_POOL) { // mutants:skip (no card gives zero tokens)
            self.run.pool_left = count;
            self.run.stage = EStage::Pooling;
            return;
        }
        if count > 0 { // mutants:skip (no card gives zero tokens)
            fx::gain_token(sx.state, self.run.who, crate::choice::strategy::Pool::Strategic, i32::from(count));
        }
        self.finish_goods(sx);
    }

    fn finish_goods(&mut self, sx: &mut Sx<'_>) {
        let goods = std::mem::take(&mut self.run.goods);
        self.gain_goods(sx, goods);
        self.close(sx);
    }

    /// Freelancers ("You may produce 1 unit in this system. You may spend influence as if it were resources to produce this unit"): the unit is
    /// the production area's `ProduceOne`, its plain state kept in the run (`ExploreRun::one`) and rebuilt at every call through the table's link.
    fn freelancers(&mut self, sx: &mut Sx<'_>, card: Explore, system: System) {
        let who = self.run.who;
        let (Some(link), Some(roster)) = (sx.table.links.one, sx.table.facts.roster) else { return self.fail(EWhy::NoLink) };
        let Some((buildable, fleet_limit)) = (link.facts)(sx.state, sx.kernel, roster, who) else { return self.fail(EWhy::NoLink) };
        let codes = sx.table.codes.explore;
        let facts = crate::state::OneFacts { buildable, fleet_limit, name: Some(codes.freelancers), alias: Some(codes.cards[card as usize]) };
        // The permission to pay with influence is the position's, for this one production.
        sx.state.meta_mut().influence_pays_for_units.insert(who);
        self.run.one = Some(crate::state::OneRun::new(who, system, facts));
        self.run.stage = EStage::Producing;
    }

    /// Run Freelancers' unit to its next question; when it is done the permission goes and the card closes.
    fn produce(&mut self, sx: &mut Sx<'_>) -> Option<Next> {
        let (Some(link), Some(run)) = (self.one, self.run.one) else {
            self.fail(EWhy::NoLink);
            return None;
        };
        let (run, next) = (link.step)(run, sx);
        self.run.one = Some(run);
        match next {
            Next::Continue(ask) => Some(Next::Continue(ask)),
            Next::Done => {
                self.leave_producing(sx);
                None
            }
            other => Some(other),
        }
    }

    fn leave_producing(&mut self, sx: &mut Sx<'_>) {
        let who = self.run.who;
        sx.state.meta_mut().influence_pays_for_units.remove(who);
        let Some(run) = self.run.one.take() else { return self.fail(EWhy::NoLink) };
        // A unit that stands over the supply or the capacity of its system: the string asks which units go, which the window cannot.
        if run.unsupported {
            return self.fail(EWhy::Freelancers);
        }
        // The card resolved when a unit stands (the string's `produce_one_paying_with_influence` answers whether one was made).
        if run.made.is_none() {
            self.run.how = EHow::Unresolved;
        }
        self.close(sx);
    }

    /// Start a window of another area as a child. A host that keeps the exploration in a payload cannot keep the child between two calls, so it
    /// runs at once: a child that finishes without a question is carried, one that asks is not.
    fn link(&mut self, sx: &mut Sx<'_>, request: &Spawn, then: EThen) {
        match sx.table.links.start(request) {
            Some(window) => {
                self.linked.start(window);
                self.run.stage = EStage::Linked(then);
                if self.hosted {
                    match self.linked.step(sx) {
                        Some(Ran::Done(Report::Left)) | None => self.fail(EWhy::NoLink),
                        Some(Ran::Done(_)) => self.close(sx),
                        Some(Ran::Asks(_)) => self.link_on_tape(sx),
                        Some(Ran::Pass(_)) => self.fail(EWhy::LinkedAsks),
                    }
                }
            }
            None => self.fail(EWhy::NoLink),
        }
    }

    /// A child of another area that asks (the discard of a draw over the hand limit, the secret objective's hand limit) inside a hosted exploration:
    /// the host cannot keep the child between two calls, so its questions go to the TAPE (`Sx::ask`): the driver rolls the step back and replays it
    /// with the answers given so far, and the child is rebuilt by the same request each time.
    fn link_on_tape(&mut self, sx: &mut Sx<'_>) {
        for _ in 0..LINKED_TAPE_LIMIT { // mutants:skip (a bound no hand reaches)
            let Some(mut choice) = self.linked.ask_of(sx.state, sx.kernel) else { return self.fail(EWhy::LinkedAsks) };
            if choice.ctx.invasion_seq.is_none() && !choice.ctx.space_battle {
                choice.ctx.invasion_seq = Self::inv(sx.state);
            }
            let pick = sx.ask(&choice);
            if self.linked.answer(sx, pick).is_err() {
                return self.fail(EWhy::LinkedAsks);
            }
            match self.linked.step(sx) {
                Some(Ran::Done(Report::Left)) | None => return self.fail(EWhy::NoLink),
                Some(Ran::Done(_)) => return self.close(sx),
                Some(Ran::Asks(_)) => {}
                Some(Ran::Pass(_)) => return self.fail(EWhy::LinkedAsks),
            }
        }
        self.fail(EWhy::LinkedAsks);
    }

    /// The answer to a reward question.
    fn rewarded(&mut self, sx: &mut Sx<'_>, then: EThen, chosen: EOpt) {
        let who = self.run.who;
        match then {
            EThen::Warehouses => {
                if chosen == EOpt::Convert {
                    let moved = fx_explore::spend_commodities(sx.state, who, Some(2));
                    self.gain_goods(sx, moved);
                } else {
                    fx_explore::gain_commodities(sx.state, who, 2);
                }
                self.close(sx);
            }
            EThen::Merchant => {
                if chosen == EOpt::Convert {
                    let moved = fx_explore::spend_commodities(sx.state, who, None);
                    self.gain_goods(sx, moved);
                } else {
                    // Replenish: the Cabal note and the Trade Agreement follow, as `fx::replenish_announced` stages them.
                    if fx::replenish_announced(sx.state, who, sx.table.codes.explore.trade_agreement).is_none() {
                        return self.fail(EWhy::Replenish);
                    }
                }
                self.close(sx);
            }
            EThen::FunctioningBase => match chosen {
                EOpt::SpendTradeGood | EOpt::SpendCommodity => {
                    if let Some(p) = sx.state.players_mut().by_seat_mut(who) {
                        if chosen == EOpt::SpendTradeGood {
                            p.econ.trade_goods -= 1;
                        } else {
                            p.econ.commodities -= 1;
                        }
                    }
                    let request = Spawn::DrawCards { who, count: 1, drop_unofficial: sx.state.meta().rules_at_least(RULES_OFFICIAL_ACTION_DECK) };
                    self.link(sx, &request, EThen::BaseCard);
                }
                _ => {
                    fx_explore::gain_commodities(sx.state, who, 1);
                    self.close(sx);
                }
            },
            EThen::Fabricators => match chosen {
                EOpt::SpendTradeGood | EOpt::SpendCommodity => {
                    if self.place(sx, crate::generated::UnitBaseType::Mech) {
                        if let Some(p) = sx.state.players_mut().by_seat_mut(who) {
                            if chosen == EOpt::SpendTradeGood {
                                p.econ.trade_goods -= 1;
                            } else {
                                p.econ.commodities -= 1;
                            }
                        }
                    }
                    if self.run.why.is_none() {
                        self.close(sx);
                    }
                }
                _ => {
                    fx_explore::gain_commodities(sx.state, who, 1);
                    self.close(sx);
                }
            },
            EThen::Mercenaries => {
                if chosen == EOpt::Place {
                    self.place(sx, crate::generated::UnitBaseType::Infantry);
                }
                if self.run.why.is_none() {
                    self.close(sx);
                }
            }
            _ => self.close(sx),
        }
    }

    /// `exploration::place_on_planet`: one unit of a base type on the planet.
    fn place(&mut self, sx: &mut Sx<'_>, base: crate::generated::UnitBaseType) -> bool {
        let Some(planet) = self.planet_of() else { return false }; // mutants:skip (only planet cards place)
        let Some(roster) = sx.table.facts.roster else {
            self.fail(EWhy::NoLink);
            return false;
        };
        let Some(system) = board::system_of(sx.state, Self::scope(sx.kernel), planet) else { return false }; // mutants:skip (a planet has a system)
        let scope = Self::scope(sx.kernel);
        fx_strategy::place_for(sx.state, sx.kernel, scope, roster, self.run.who, system, Some(planet), base).is_some()
    }

    /// The card is resolved: the row is closed with the adapter's words, the faction hooks and the event follow.
    fn close(&mut self, sx: &mut Sx<'_>) {
        let who = self.run.who;
        let (Some(card), Some(row), Some(words)) = (self.run.card, self.run.row, sx.table.facts.explore_words) else { return self.fail(EWhy::NoTexts) };
        let planet = self.planet_of();
        let after = qexplore::holdings(sx.state, who, planet);
        let kept = sx.state.players().by_seat(who).and_then(|p| p.cards.explore.as_slice().last().copied());
        let attached = planet.and_then(|p| board::attachments(sx.state, p).as_slice().last().copied());
        let row_in = fx_explore::RowIn { planet, card, how: self.run.how, before: self.run.before, after, gamma: self.run.gamma, ion: self.run.ion, kept, attached };
        let Some(text) = words(sx.state, &row_in) else { return self.fail(EWhy::NoTexts) };
        if fx_explore::close_row(sx.state, row as usize, text).is_none() {
            return self.fail(EWhy::NoTexts);
        }
        self.run.finished = true;
        self.run.stage = EStage::Done;
        if let Some(planet) = planet {
            // Pre-Fab Arcologies et al. run first (they change the state), then the typed event.
            let kernel = sx.kernel;
            kernel.hooks.economy.explored(&mut sx.cx(), who, planet);
            if self.run.announce
                && let Some(deck) = self.run.deck
            {
                let codes = sx.table.codes.explore;
                let payload = crate::state::Payload::new()
                    .with(crate::state::Key::Player, crate::state::Val::Seat(who))
                    .with(crate::state::Key::Planet, crate::state::Val::Planet(planet))
                    .with(crate::state::Key::Deck, crate::state::Val::Code(codes.decks[deck as usize]))
                    .with(crate::state::Key::Card, crate::state::Val::Code(codes.cards[card as usize]));
                let _ = sx.emit(fx::event(crate::state::EventKind::PlanetExplored, payload));
            }
        }
    }
}

impl Window for Exploring {
    type Stage = EStage;

    fn stage(&self) -> EStage {
        self.run.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.one = self.one.or(sx.table.links.one);
        loop {
            if self.run.why.is_some() {
                self.run.stage = EStage::Done;
            }
            match self.run.stage {
                EStage::Start => self.start(sx),
                EStage::Deck => return Next::Continue(Ask::Explore(EAsk::ChooseDeck)),
                EStage::Suns => {
                    let Some(suns) = self.suns_child(sx.state) else { return self.fail_next(EWhy::Draw) };
                    let ask = suns.ask_of(sx.state, sx.kernel).map(|c| c.ask);
                    return ask.map_or_else(|| self.fail_next(EWhy::Draw), Next::Continue);
                }
                EStage::Pooling => return Next::Continue(Ask::Strategy(SAsk::GainPool { left: self.run.pool_left })),
                EStage::Rewarding(then) => {
                    let Some(child) = self.reward_child(sx.state, then) else { return self.fail_next(EWhy::NoLink) };
                    return Next::Continue(Ask::Explore(EAsk::Reward(child.card)));
                }
                EStage::Paying(then) => {
                    let Some(child) = self.pay_child(sx.state, then) else { return self.fail_next(EWhy::NoLink) };
                    let ask = child.ask_of(sx.state, sx.kernel).map(|c| c.ask);
                    return ask.map_or_else(|| self.fail_next(EWhy::NoLink), Next::Continue);
                }
                EStage::Ion => return Next::Continue(Ask::Explore(EAsk::IonStorm)),
                EStage::Producing => {
                    if let Some(next) = self.produce(sx) {
                        return next;
                    }
                }
                EStage::Linked(then) => match self.linked.step(sx) {
                    Some(Ran::Asks(ask)) => {
                        if self.hosted {
                            self.fail(EWhy::LinkedAsks);
                            continue;
                        }
                        return Next::Continue(ask);
                    }
                    Some(Ran::Pass(next)) => {
                        if self.hosted { // mutants:skip (a child that hands back a Next is not one of the windows linked here)
                            self.fail(EWhy::LinkedAsks);
                            continue;
                        }
                        return next;
                    }
                    Some(Ran::Done(report)) => {
                        if report == Report::Left {
                            self.fail(EWhy::NoLink);
                        } else {
                            let _ = then;
                            self.close(sx);
                        }
                    }
                    // (A child the exploration lost between two calls cannot be resumed: a host runs it at once, `link`.)
                    None => self.fail(EWhy::NoLink),
                },
                EStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let mut choice = match self.run.stage {
            EStage::Deck => self.deck_child(s).ask_of(s, k),
            EStage::Suns => self.suns_child(s)?.ask_of(s, k),
            EStage::Pooling => Some(crate::choice::strategy::gain_pool_choice(self.run.who, self.run.pool_left)),
            EStage::Rewarding(then) => self.reward_child(s, then)?.ask_of(s, k),
            EStage::Paying(then) => self.pay_child(s, then)?.ask_of(s, k),
            EStage::Ion => self.ion_child(s)?.ask_of(s, k),
            EStage::Linked(_) => self.linked.ask_of(s, k),
            EStage::Producing => (self.one?.ask)(&self.run.one?, s, k),
            _ => None,
        }?;
        // Every question asked while an invasion is open names it (the string `Table::ask_seeing`), the children of other areas included.
        if choice.ctx.invasion_seq.is_none() && !choice.ctx.space_battle { // mutants:skip (a question that names an invasion names this one)
            choice.ctx.invasion_seq = Self::inv(s);
        }
        Some(choice)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.one = self.one.or(sx.table.links.one);
        match self.run.stage {
            EStage::Producing => {
                let (link, run) = (self.one.ok_or(Illegal)?, self.run.one.ok_or(Illegal)?);
                self.run.one = Some((link.answer)(run, sx, pick)?);
                // Settle at once, like every other stage: a host that asks after an answer (the invasion) must not see the unit's stale question
                // (a payment that is covered, a placement with one spot).
                let _ = self.produce(sx);
                Ok(())
            }
            EStage::Deck => {
                let mut child = self.deck_child(sx.state);
                child.answer(sx, pick)?;
                self.run.deck = child.chosen;
                self.draw(sx);
                Ok(())
            }
            EStage::Suns => {
                let mut child = self.suns_child(sx.state).ok_or(Illegal)?;
                child.answer(sx, pick)?;
                self.run.card = Some(child.chosen);
                self.open(sx);
                Ok(())
            }
            EStage::Pooling => {
                let Pick::Opt(Opt::Strategy(SOpt::Pool(pool))) = pick else { return Err(Illegal) };
                fx::gain_token(sx.state, self.run.who, pool, 1);
                self.run.pool_left = self.run.pool_left.saturating_sub(1);
                if self.run.pool_left == 0 {
                    self.finish_goods(sx);
                }
                Ok(())
            }
            EStage::Rewarding(then) => {
                let mut child = self.reward_child(sx.state, then).ok_or(Illegal)?;
                child.answer(sx, pick)?;
                let chosen = child.chosen.ok_or(Illegal)?;
                self.rewarded(sx, then, chosen);
                Ok(())
            }
            EStage::Paying(then) => {
                let mut child = self.pay_child(sx.state, then).ok_or(Illegal)?;
                child.answer(sx, pick)?;
                let EThen::Paying(reward) = then else { return Err(Illegal) };
                self.paid(sx, reward, child.paid);
                Ok(())
            }
            EStage::Ion => {
                let mut child = self.ion_child(sx.state).ok_or(Illegal)?;
                let before = sx.state.meta().ion_storm;
                child.answer(sx, pick)?;
                // Only a token that differs from the one in play is a new token of the row.
                if sx.state.meta().ion_storm != before {
                    self.run.ion = sx.state.meta().ion_storm;
                }
                self.close(sx);
                Ok(())
            }
            EStage::Linked(_) => self.linked.answer(sx, pick),
            _ => Err(Illegal),
        }
    }
}

impl Exploring {
    fn fail_next(&mut self, why: EWhy) -> Next {
        self.fail(why);
        Next::Done
    }
}
