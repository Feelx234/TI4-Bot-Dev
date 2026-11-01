//! Child windows of other areas, and the calls into them (package B6d, layering; docs/ENGINE_CORE_STATE.md 4.1).
//!
//! A rule area (L4) never imports another area. A card effect that runs another area's window as a child (an action card
//! pays a bill, enforces the hand limit, researches, explores a reward, performs a strategy card) cannot name the window's
//! type, so it asks the kernel: it puts a plain-data request ([`Spawn`]) to the table ([`AreaLinks::spawn`]) and gets an
//! erased window back ([`ErasedWindow`]), which it runs the way [`super::stage::Child`] runs a same-area window
//! ([`ErasedChild`]). What the finished window did comes back as a plain value ([`Report`]).
//!
//! The table is filled by a module that is allowed to import every area (`crate::driver::children::register`, layer L5); the
//! kernel only defines the shapes. A table that was not filled has no link: a window that needs one must decline its run
//! (`unsupported`, a counted bail of the adapter) and never panic.

use crate::choice::explore::{Opt as EOpt, Step as EStep};
use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Planet, PublicObjective, StrategyCard, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::query::tech::UnitRoster;
use crate::state::{Code, CoreState};
use crate::tables::Scope;

/// The unit a seat places for one structure kind and whether it goes in the space area
/// (`strategy_cards::construction_placement`), as the caller resolved it from the unit tables; `None`: the
/// kind may not be placed at all (a faction effect forbids it).
pub type Placement = Option<(Unit, bool)>;

/// What a Construction window needs of the position that the core does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructureFacts {
    /// The PDS the seat places.
    pub pds: Placement,
    /// The space dock the seat places.
    pub dock: Placement,
    /// The code of the source `place_structure` (the adapter's text table).
    pub source: Code,
    /// Titans' Hecatoncheires, an alternative to a PDS: the mech and the infantry the seat places instead ("When you would place a PDS on a
    /// planet, you may place 1 mech and 1 infantry on that planet instead"); `None`: the seat has no such alternative (or a faction effect
    /// forbids one of the two). It is offered only while both fit in the reinforcements.
    pub alt: Option<(Unit, Unit)>,
}

/// What a production by an ability (`Spawn::Produce`) needs of the position that the core does not hold: the numbers the string
/// window opens with (`ProductionWindow::for_ability`) and the facts of the seat its questions read. The adapter computes them on the
/// position the use starts from; one use is one system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProduceFacts {
    pub who: Seat,
    pub system: System,
    /// The units the ability allows (replaces the system's printed capacity).
    pub limit: i32,
    /// The highest printed cost of one unit (Umbat: 4); `None`: no cap.
    pub max_unit_cost: Option<i32>,
    /// The units that do not count against the limit (the Cabal commander's two).
    pub small_exemptions: i32,
    /// The cost reduction of the faction hooks in this system (`production_cost_reduction`).
    pub discount: i32,
    pub fleet_limit: i32,
    pub armada: i32,
    /// Nomad's commander: the flagship is free.
    pub flagship_free: bool,
    pub buildable: [Option<Unit>; 9],
}

/// What `Spawn::ProduceOne` (one unit outside a use of PRODUCTION: Chaos Mapping, Freelancers) needs of the position that the core does not
/// hold. Plain data of the state layer (a hosted run keeps it between two questions).
pub use crate::state::OneFacts;

/// What `Spawn::Seeing` (the enforcement of the fleet supply and the capacity of one seat in one system, `fleet::enforce_seeing`) needs of the
/// position that the core does not hold: the fleet limit as the string works it out (pool, laws, abilities), the one the question shows
/// and the fighter types that count as half a ship for the seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeeingLimits {
    /// The limit the enforcement judges by.
    pub enforce: i64,
    /// The limit the question shows.
    pub shown: i64,
    /// The fighter types counted as half (Naalu Hybrid Crystal Fighter II), at most one per unit form.
    pub halves: [Option<Unit>; 2],
}

/// What `Spawn::Home` (a use of PRODUCTION at a seat's home system outside a tactical action: the Warfare secondary) needs of the position
/// that the core does not hold: the numbers the string window opens with (`ProductionWindow::new`, the facts of the seat) and the one
/// rule of the version that decides whether the window resets the discount a tactical action left unspent. The adapter computes them on
/// the position the ability starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomeFacts {
    /// The use: `limit` is the PRODUCTION total of the system (the capacity), `discount` the cost reduction of the faction hooks there.
    pub produce: ProduceFacts,
    /// `rules_version >= RULES_WINNU_AGENT_PRODUCTION`: Berekar Berekon is offered to the use, and the discount an earlier tactical
    /// action left unspent is not read.
    pub agent_rules: bool,
}

/// How many systems the free tactical action of the Thunder's Edge Warfare primary can name (the map, the systems in play beside it).
pub const FREE_SYSTEMS: usize = 96;
/// How many systems the production of the Thunder's Edge Construction primary can name: every system the string holds an entry for, when a bonus
/// to PRODUCTION is the same everywhere (War Machine), so a late game's board.
pub const TE_USES: usize = 64;

/// The systems the free tactical action of the Thunder's Edge Warfare primary may be played in ("any system": the printed map and every
/// system in play beside it), in the order the string offers them, and the interned source `te6warfare` of the question
/// (`Spawn::TeWarfare`). `None` from [`Self::new`]: more systems than the facts hold, the adapter declines the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeSystems {
    pub source: Code,
    len: u8,
    ids: [System; FREE_SYSTEMS],
}

impl FreeSystems {
    #[must_use]
    pub fn new(source: Code, systems: &[System]) -> Option<Self> {
        if systems.len() > FREE_SYSTEMS {
            return None;
        }
        let mut ids = [System::ALL[0]; FREE_SYSTEMS];
        ids[..systems.len()].copy_from_slice(systems);
        Some(Self { source, len: u8::try_from(systems.len()).ok()?, ids })
    }

    /// The systems, in the order of the question.
    #[must_use]
    pub fn systems(&self) -> &[System] {
        &self.ids[..usize::from(self.len)]
    }
}

/// What `Spawn::ConstructionTe` (the Thunder's Edge Construction primary) needs of the position that the core does not hold beyond the
/// structures: the interned source `te4construction` of the choice and the facts of the use of PRODUCTION in each system that can produce,
/// in the order the string offers them (`HomeFacts`: the same use as the Warfare secondary's, in another system). `None` from [`Self::new`]:
/// more systems than the facts hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeUses {
    pub source: Code,
    len: u8,
    uses: [Option<HomeFacts>; TE_USES],
}

impl TeUses {
    #[must_use]
    pub fn new(source: Code, uses: &[HomeFacts]) -> Option<Self> {
        if uses.len() > TE_USES {
            return None;
        }
        let mut held = [None; TE_USES];
        for (slot, facts) in held.iter_mut().zip(uses) {
            *slot = Some(*facts);
        }
        Some(Self { source, len: u8::try_from(uses.len()).ok()?, uses: held })
    }

    /// The uses, in the order of the question.
    pub fn uses(&self) -> impl Iterator<Item = HomeFacts> + '_ {
        self.uses[..usize::from(self.len)].iter().filter_map(|u| *u)
    }
}

/// What `Spawn::Imperial` (the Imperial primary) needs of the position that the core does not hold: the public objectives the seat could
/// score (the string's `objectives::scoreable_on`), up to sixteen, and the environment of the payment of a bought objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImperialFacts {
    pub scoreable: [Option<PublicObjective>; 16],
    pub env: crate::kernel::fx_pay::PayEnv,
}

/// The units a finished production made, in order: the unit and the planet it stands on (`None`: the space area).
pub type Made = SmallList<(Unit, Option<Planet>), 12>;

/// The interned texts and the flag the hit flows of a card read (`Spawn::Absorb`; the adapter's, like `Codes`): a window carries
/// no text of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitTexts {
    /// `supply::staging_enabled`: every loss is recorded and a planet-standing sustainer is named by its planet.
    pub staging: bool,
    /// The interned source `letnevcommander` of the commander's payment, offered after a sustain.
    pub commander: Code,
    /// The interned subtype `assign_casualty` of the casualty question.
    pub casualty: Code,
    /// The interned cause `effect_hit` of a loss from the hit of an effect.
    pub effect_hit: Code,
}

/// What a finished child window tells its parent, as plain data (the parent cannot read the window's fields: it does not
/// know its type).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Report {
    /// Nothing the parent reads.
    #[default]
    Nothing,
    /// A bill: whether it was paid in full.
    Paid(bool),
    /// A one-question reward window: the option taken.
    Chose(Option<EOpt>),
    /// The window left the path the core carries (a research only a faction waiver opens): the string performs the card.
    Left,
    /// A seat gained trade goods the parent announces (the Trade primary's three), staged as the string stages them.
    Goods(Seat, i32),
    /// The Diplomacy primary: the system it placed command tokens in and the seats that got one, in seating order.
    Placed(System, SmallList<Seat, 8>),
    /// A ship was destroyed (its destruction is staged for the announcement of the play) or none could be.
    Destroyed(bool),
    /// A structure was placed (the Titan Prototype).
    Built(bool),
    /// A production by an ability finished: what it made.
    Made(Made),
    /// An enforcement finished: how many units it removed.
    Removed(u32),
    /// The Imperial primary finished: the public objective the holder scored, if any (what the score sets off is the adapter's).
    Scored(Option<PublicObjective>),
    /// The Thunder's Edge Warfare primary finished: the system of the free tactical action the game plays next (`None`: no system to offer).
    Free(Option<System>),
    /// The Thunder's Edge Construction primary finished: the use of PRODUCTION it ran, the system and what it made (`None`: a structure was chosen).
    Produced(Option<(System, Made)>),
    /// An exploration finished: how it ended.
    Explored(ExploreEnd),
}

/// How an exploration ended (`Report::Explored`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploreEnd {
    /// The card was resolved and its row closed.
    Explored,
    /// The deck ran out.
    Empty,
    /// The exploration declined its run (a card the core does not carry, no texts, no link to a child).
    Unsupported,
}

/// A window of another area, run as a child: the object-safe face of [`Window`] plus what the finished window reports.
pub trait ErasedWindow: std::fmt::Debug {
    /// [`Window::step`].
    fn step(&mut self, sx: &mut Sx<'_>) -> Next;
    /// [`Window::ask_of`].
    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice>;
    /// [`Window::answer`].
    ///
    /// # Errors
    /// The pick is not an option of the question.
    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal>;
    /// A copy of the window (a checkpoint of the parent is a clone).
    fn boxed_clone(&self) -> Box<dyn ErasedWindow>;
    /// What the window did, read once it has finished.
    fn report(&self) -> Report;
}

impl Clone for Box<dyn ErasedWindow> {
    fn clone(&self) -> Self {
        self.boxed_clone()
    }
}

/// A [`Window`] with the function that reads its [`Report`]: the only implementation of [`ErasedWindow`].
#[derive(Debug, Clone)]
pub struct Erased<W: Window> {
    window: W,
    report: fn(&W) -> Report,
}

impl<W: Window + std::fmt::Debug + 'static> Erased<W> {
    /// Erase `window`; `report` reads what it did once it has finished.
    #[must_use]
    pub fn boxed(window: W, report: fn(&W) -> Report) -> Box<dyn ErasedWindow> {
        Box::new(Self { window, report })
    }
}

impl<W: Window + std::fmt::Debug + 'static> ErasedWindow for Erased<W> {
    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.window.step(sx)
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.window.ask_of(s, k)
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.window.answer(sx, pick)
    }

    fn boxed_clone(&self) -> Box<dyn ErasedWindow> {
        Box::new(self.clone())
    }

    fn report(&self) -> Report {
        (self.report)(&self.window)
    }
}

/// A child of another area that is running inside a window: [`super::stage::Child`] over an [`ErasedWindow`].
///
/// The parent holds it as a field (the state stays `(window, state)`, so a checkpoint is a clone) and delegates `step` until
/// it returns [`Ran::Done`] (with the child's [`Report`]), `ask_of` while it runs, `answer` for a pick its question got.
#[derive(Debug, Clone, Default)]
pub struct ErasedChild {
    window: Option<Box<dyn ErasedWindow>>,
}

impl ErasedChild {
    /// No child.
    #[must_use]
    pub const fn idle() -> Self {
        Self { window: None }
    }

    /// Start `window` as the child.
    pub fn start(&mut self, window: Box<dyn ErasedWindow>) {
        self.window = Some(window);
    }

    /// Whether a child is running.
    #[must_use]
    pub const fn running(&self) -> bool {
        self.window.is_some()
    }

    /// Step the child. `None` when there is none.
    pub fn step(&mut self, sx: &mut Sx<'_>) -> Option<Ran<Report>> {
        let window = self.window.as_mut()?;
        match window.step(sx) {
            Next::Continue(ask) => Some(Ran::Asks(ask)),
            Next::Done => self.window.take().map(|done| Ran::Done(done.report())),
            other => Some(Ran::Pass(other)),
        }
    }

    /// The question the child waits on.
    #[must_use]
    pub fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.window.as_ref()?.ask_of(s, k)
    }

    /// Give the child its answer.
    ///
    /// # Errors
    /// There is no child, or the pick is not an option of its question.
    pub fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.window.as_mut().ok_or(Illegal)?.answer(sx, pick)
    }
}

/// A child window of another area, asked for by its parent (the arguments of the window's constructor, as data).
#[allow(clippy::large_enum_variant, reason = "a request is built once per child window and moved to the table; boxing the facts of the largest variant (the Thunder's Edge Construction uses) would cost an allocation per start")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spawn {
    /// `production::pay::PayBill`: `who` pays `cost` of `kind`; reports [`Report::Paid`].
    Bill { who: Seat, kind: Spend, cost: i64 },
    /// `objective_windows::HandLimit`: `who` returns secret objectives over the limit.
    HandLimit { who: Seat },
    /// `explore_windows::Reward`: one reward question; reports [`Report::Chose`].
    Reward { who: Seat, step: EStep, options: SmallList<EOpt, 3>, skip_single: bool },
    /// The Technology ability (`production::strategy_cards::Research`): `cost` resources for the paid research.
    Research { who: Seat, primary: bool, cost: i64, scope: Scope, roster: &'static UnitRoster },
    /// The Technology Rider's free research (`production::strategy_cards::Research::rider`): reports [`Report::Left`] when an answer
    /// left the plain path (a technology only a faction waiver opens).
    RiderResearch { who: Seat, scope: Scope, roster: &'static UnitRoster },
    /// The Leadership ability.
    Leadership { who: Seat, primary: bool },
    /// The Construction ability.
    Construction { who: Seat, primary: bool, facts: StructureFacts },
    /// The primary of the Thunder's Edge Construction: a structure or a use of PRODUCTION in one of `uses`, then another structure; reports
    /// [`Report::Produced`] (or [`Report::Left`] when the use holds something the core does not carry).
    ConstructionTe { who: Seat, facts: StructureFacts, uses: TeUses },
    /// The Titan Prototype's build for `who`: three resources for a structure, else a trade good (`goods` is the interned
    /// source `relic`); reports [`Report::Built`].
    TitanBuild { who: Seat, facts: StructureFacts, goods: Code },
    /// `action_cards::draw::DrawCards`: `who` draws `count` action cards and discards over the hand limit; reports
    /// [`Report::Left`] when the table gave no printed-name groups (the hand limit could not be asked).
    DrawCards { who: Seat, count: usize, drop_unofficial: bool },
    /// The Politics ability; `official`: the unofficial action cards leave the deck before the draw.
    Politics { who: Seat, primary: bool, official: bool },
    /// The Warfare primary; `letnev_pool`: the Letnev fleet pool may be emptied.
    Warfare { who: Seat, letnev_pool: bool },
    /// The primary of the Thunder's Edge Warfare (`strategy_primaries::TeWarfare`): the redistribution before the action and the system of the
    /// free tactical action; reports [`Report::Free`] (the tactical action and the redistribution after it are the game's).
    TeWarfare { who: Seat, letnev_pool: bool, free: FreeSystems },
    /// The Diplomacy ability of `card`; the primary readies every planet of the system when `base_printing`.
    Diplomacy { who: Seat, card: StrategyCard, primary: bool, base_printing: bool },
    /// The Trade primary; reports [`Report::Goods`].
    Trade { who: Seat },
    /// `combat::effect_hits::Destroy`: `who` chooses one of their ships in `system` and it is destroyed (staged); reports
    /// [`Report::Destroyed`]. `card` is the source of the question, `subtype` its interned subtype and `cause` the interned cause
    /// of the staged destruction.
    Destroy { who: Seat, system: System, card: ActionCard, subtype: Code, cause: Code },
    /// `combat::effect_hits::Absorb`: `hits` hits produced by `producer` land on the ships of `who` in `system`.
    Absorb { who: Seat, producer: Seat, system: System, hits: u16, texts: HitTexts },
    /// `production::window::ProductionWindow` for a production by an ability (`ProductionWindow::for_ability`): reports [`Report::Made`].
    Produce { facts: ProduceFacts },
    /// `production::one::ProduceOne`: one unit for `who` in `system`; reports [`Report::Left`] when the unit stands over the fleet supply
    /// or the capacity of its system (the string asks which units go), else [`Report::Nothing`].
    ProduceOne { who: Seat, system: System, facts: OneFacts },
    /// `tactical::fleet::Enforce`: `who` removes the units over the fleet supply, then over capacity, in `system`, one question each;
    /// reports [`Report::Removed`].
    Seeing { who: Seat, system: System, limits: SeeingLimits },
    /// `production::home::HomeProduction`: a use of PRODUCTION at the home system of `facts.produce.who` outside a tactical action (the Warfare
    /// secondary): the Hacan hero's offer, the production, the effects at the end of the use, the Self-Assembly Routines offer; reports
    /// [`Report::Made`], or [`Report::Left`] when the run holds something the core does not carry.
    Home { facts: HomeFacts },
    /// [`Self::Home`] with the facts read from the position when the use starts (`home_facts`): the Warfare secondary performed after an
    /// ability that changed what a production reads (tokens, a unit upgrade, a space dock). Reports as [`Self::Home`].
    HomeAt { who: Seat, system: System, roster: &'static UnitRoster, agent_rules: bool },
    /// `driver::windows::scoring::OccurrenceScoring`: the scoring window an action-timed event occurrence opens (`ScoringWindow::for_occurrence` over the
    /// initiative order: a won combat's feat, one scoring per player when `one_per_player`), run to its end.
    EventScoring { occurrence: u64, one_per_player: bool },
    /// `strategy_primaries::ImperialPrimary`: score a public objective, then a point for Mecatol Rex or a secret objective; reports
    /// [`Report::Scored`], or [`Report::Left`] when the table has no link to the hand limit window.
    Imperial { who: Seat, facts: ImperialFacts },
    /// Self-Assembly Routines' offer at the end of a use of PRODUCTION in `system` (a window of the faction crate: the table's extra link).
    SelfAssembly { who: Seat, system: System },
    /// `explore_windows::Exploring` of the frontier token in `system`, spent first and announced to nobody (the string explores it with no
    /// timing handle); reports [`Report::Explored`]. `named`: the exploration names the system of the token (the string's `explore_frontier`);
    /// Exploration Probe's draw does not (`explore_with(.., FRONTIER, None)`: no system in the log row, nowhere for a Mirage).
    ExploreFrontier { who: Seat, system: System, named: bool },
}

/// Starts the child window a [`Spawn`] names; `None` for a request the table does not know.
pub type SpawnFn = fn(&Spawn) -> Option<Box<dyn ErasedWindow>>;

/// `technology::researchable` for a card that researches outside a strategy card: what the seat can research now.
pub type ResearchableFn = fn(&CoreState, &Kernel, Scope, Seat) -> SmallList<Technology, 40>;

/// `technology::research` of a technology a card chose, the bill (if any) already paid; whether it was researched.
pub type ResearchFn = fn(&mut CoreState, &Kernel, Scope, UnitRoster, Seat, Technology) -> bool;

/// What a window that hosts `ProduceOne` in its own payload calls (the state is [`crate::state::OneRun`]; the window object is rebuilt from it at
/// every call by the production area): the facts the seat produces with, one step, the question, one answer.
/// The units a seat may produce (nine forms) and the fleet limit they are judged by.
pub type BuildableFacts = ([Option<Unit>; 9], i32);

#[derive(Clone, Copy)]
pub struct OneLink {
    /// What the seat may produce and the fleet limit it is judged by (`production::home::{buildable_for, fleet_limit}`).
    pub facts: fn(&CoreState, &Kernel, &UnitRoster, Seat) -> Option<BuildableFacts>,
    /// The limits `fleet::enforce_seeing` judges the system by when the unit stands over its supply or its capacity (`Spawn::Seeing`).
    pub limits: fn(&CoreState, &Kernel, Seat, System) -> SeeingLimits,
    /// Run the unit to its next question or its end (`Next::Done`).
    pub step: fn(crate::state::OneRun, &mut Sx<'_>) -> (crate::state::OneRun, Next),
    /// The question it waits on.
    pub ask: fn(&crate::state::OneRun, &CoreState, &Kernel) -> Option<Choice>,
    /// Apply an answer.
    pub answer: fn(crate::state::OneRun, &mut Sx<'_>, Pick) -> Result<crate::state::OneRun, Illegal>,
}

impl std::fmt::Debug for OneLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OneLink")
    }
}

/// The calls a window makes into other areas, by function pointer. Filled by `crate::driver::children::register`; empty by
/// default (a window that needs a link then declines its run).
#[derive(Clone, Copy, Default)]
pub struct AreaLinks {
    /// Start a child window.
    pub spawn: Option<SpawnFn>,
    /// The technologies a seat can research.
    pub researchable: Option<ResearchableFn>,
    /// Research a technology.
    pub research: Option<ResearchFn>,
    /// Start a child window the faction crate owns (Self-Assembly Routines): consulted when [`Self::spawn`] does not know the request.
    pub spawn_extra: Option<SpawnFn>,
    /// `ProduceOne` kept by its host's payload (Freelancers inside an exploration).
    pub one: Option<OneLink>,
}

impl std::fmt::Debug for AreaLinks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AreaLinks").field("spawn", &self.spawn.is_some()).field("researchable", &self.researchable.is_some()).field("research", &self.research.is_some()).field("spawn_extra", &self.spawn_extra.is_some()).field("one", &self.one.is_some()).finish()
    }
}

impl AreaLinks {
    /// Whether every link is there.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.spawn.is_some() && self.researchable.is_some() && self.research.is_some()
    }

    /// Start the child window `request` names. `None`: no link, or a request the link does not know.
    #[must_use]
    pub fn start(&self, request: &Spawn) -> Option<Box<dyn ErasedWindow>> {
        (self.spawn?)(request).or_else(|| (self.spawn_extra?)(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::cards::{Ask as CAsk, Opt as COpt, What};
    use crate::choice::{Ask, Opt, Origin, Subtype};
    use crate::driver::{Driver, Status};
    use crate::generated::ActionCard;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;

    /// A toy window of "another area": asks `left` yes/no questions of seat 0 and counts the yes answers.
    #[derive(Debug, Clone)]
    struct Asks {
        left: u8,
        got: u8,
        waiting: bool,
    }

    impl Window for Asks {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.waiting
        }
        fn step(&mut self, _: &mut Sx<'_>) -> Next {
            if self.left == 0 {
                return Next::Done;
            }
            self.waiting = true;
            Next::Continue(Ask::Cards(CAsk::Pick { card: ActionCard::Spy, what: What::Accept }))
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            (self.waiting && self.left > 0).then(|| Choice {
                actor: Seat(0),
                ask: Ask::Cards(CAsk::Pick { card: ActionCard::Spy, what: What::Accept }),
                ctx: crate::choice::Ctx {
                    origin: Origin::ActionCard(ActionCard::Spy),
                    subtype: Subtype::Cards(crate::choice::cards::Subtype::Pick { card: ActionCard::Spy, what: What::Accept }),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: [Opt::Cards(COpt::Answer(true)), Opt::Cards(COpt::Answer(false))].into_iter().collect(),
            })
        }
        fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
            let Pick::Opt(Opt::Cards(COpt::Answer(yes))) = pick else { return Err(Illegal) };
            self.got += u8::from(yes);
            self.left -= 1;
            self.waiting = false;
            Ok(())
        }
    }

    /// The toy table: a bill of `cost` becomes `cost` questions, and the report is the number of yes answers.
    fn toy_spawn(request: &Spawn) -> Option<Box<dyn ErasedWindow>> {
        let Spawn::Bill { cost, .. } = request else { return None };
        Some(Erased::boxed(Asks { left: u8::try_from(*cost).ok()?, got: 0, waiting: false }, |w| Report::Paid(w.left == 0 && w.got > 0)))
    }

    /// A parent that starts the child the table gives it, runs it and keeps its report.
    #[derive(Debug, Clone)]
    struct Parent {
        child: ErasedChild,
        started: bool,
        reports: Vec<Report>,
        refused: bool,
    }

    impl Window for Parent {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.started
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            loop {
                if !self.child.running() {
                    if self.started {
                        return Next::Done;
                    }
                    self.started = true;
                    match sx.table.links.start(&Spawn::Bill { who: Seat(0), kind: Spend::Influence, cost: 2 }) {
                        Some(window) => self.child.start(window),
                        None => {
                            self.refused = true;
                            return Next::Done;
                        }
                    }
                }
                match self.child.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(report)) => self.reports.push(report),
                    None => return Next::Done,
                }
            }
        }
        fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
            self.child.ask_of(s, k)
        }
        fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
            self.child.answer(sx, pick)
        }
    }

    fn parent() -> Parent {
        Parent { child: ErasedChild::idle(), started: false, reports: Vec::new(), refused: false }
    }

    fn run(table: AbilityTable, answers: &[usize]) -> (Driver<Parent>, usize) {
        let mut s = CoreState::all_loaded();
        let k = Kernel::plain();
        let mut d = Driver::new(parent(), Resolver::new(&[Seat(0)], Some(Seat(0))), table, 1);
        let mut status = d.run(&mut s, &k).unwrap();
        let mut asked = 0;
        while let Status::Asking(choice) = status {
            let pick = choice.pick(answers[asked]).unwrap();
            asked += 1;
            status = d.resume(&mut s, &k, pick).unwrap();
        }
        (d, asked)
    }

    /// A window of another area runs as an erased child: its questions go through the parent, its picks come back to it,
    /// and the finished window's report is what the parent reads.
    #[test]
    fn an_erased_child_asks_through_its_parent_and_reports_when_it_finishes() {
        let mut table = AbilityTable::default();
        table.links.spawn = Some(toy_spawn);
        let (d, asked) = run(table, &[0, 1]);
        assert_eq!(asked, 2, "the child's two questions came out of the parent");
        assert_eq!(d.window.reports, vec![Report::Paid(true)], "one report, read from the finished window");
        assert!(!d.window.child.running() && !d.window.refused);
    }

    /// A table without the link gives no child: the parent sees `None` and must decline (here: it ends, refused).
    #[test]
    fn a_table_without_the_link_starts_no_child() {
        let (d, asked) = run(AbilityTable::default(), &[]);
        assert_eq!(asked, 0);
        assert!(d.window.refused && d.window.reports.is_empty());
        assert!(!AreaLinks::default().complete());
        assert!(AreaLinks::default().start(&Spawn::HandLimit { who: Seat(0) }).is_none());
    }

    /// A request the registered link does not know is `None` too.
    #[test]
    fn a_request_the_link_does_not_know_starts_no_child() {
        assert!(toy_spawn(&Spawn::Trade { who: Seat(0) }).is_none());
        let mut table = AbilityTable::default();
        table.links.spawn = Some(|_| None);
        let (d, _) = run(table, &[]);
        assert!(d.window.refused);
    }

    /// A clone of the erased child is a checkpoint: stepping the clone does not move the original.
    #[test]
    fn a_clone_of_an_erased_child_is_a_checkpoint() {
        let mut child = ErasedChild::idle();
        child.start(toy_spawn(&Spawn::Bill { who: Seat(0), kind: Spend::Influence, cost: 1 }).unwrap());
        let copy = child.clone();
        assert!(copy.running() && child.running());
        let mut s = CoreState::all_loaded();
        let k = Kernel::plain();
        // Answer through a driver on the clone only.
        let mut d = Driver::new(Parent { child: copy, started: true, reports: Vec::new(), refused: false }, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(choice) = d.run(&mut s, &k).unwrap() else { panic!("the clone asks") };
        let pick = choice.pick(0).unwrap();
        assert!(matches!(d.resume(&mut s, &k, pick).unwrap(), Status::Done));
        assert_eq!(d.window.reports, vec![Report::Paid(true)]);
        assert!(child.running() && child.ask_of(&s, &k).is_none(), "the original child did not move: it asked nothing");
    }
}
