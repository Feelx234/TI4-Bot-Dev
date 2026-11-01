//! Production as the child of a leader (package W10, lent-bodies lane 7): Letani Miasmiala (Arborec hero) and Umbat (Muaat agent).
//!
//! The production window is a window of a use; a leader that produces runs one use at a time as a child (`Spawn::Produce`, a production
//! by an ability: `ProductionWindow::ability`). The adapter hands the window the facts of every use it may open (`ProduceFacts`: the
//! buildable units, the fleet limit, the discount of the system, the exemptions), computed on the position the action starts from.
//!
//! What stays outside, and is declined by the adapter: the effects at the end of a use that the child does not run (Hacan's breakthrough
//! on three ships, the Prophecy of Ixth, a swapped planet value) and the announcement of the units the string stages for each use.

use crate::choice::generic::Content;
use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, Made, ProduceFacts, Report, Spawn};
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::fleet;
use crate::state::{CoreState, LeaderStatus};

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

/// The facts of the use `who` would open in `system`, if the adapter gave any.
fn facts_of(facts: &[ProduceFacts], who: Seat, system: System) -> Option<ProduceFacts> {
    facts.iter().find(|f| f.who == who && f.system == system).copied()
}

/// What a child use told its parent: the units it made (nothing: a use that made none).
fn made_of(report: Option<Report>) -> Made {
    match report {
        Some(Report::Made(made)) => made,
        _ => Made::new(),
    }
}

// -- Letani Miasmiala -------------------------------------------------------------------------------

/// The suspension points of [`ArborecHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HStage {
    Start,
    /// The next system of the hero's systems is opened as a use.
    Next,
    /// A use is running.
    Producing,
    Done,
}

/// Letani Miasmiala (Arborec hero): "ACTION: Produce any number of units in any number of systems that contain 1 or more of your ground
/// forces. Then, purge this card." The systems are those holding a ground force of the seat when the action starts, in system order (the
/// string lists them once); each is one use of production by an ability, run as a child, with no limit to the number of units (the
/// purse and the box bound it). A system where nothing can be built is passed over without a question. [`Self::used`]: some unit was made.
/// [`Self::made`]: what each use made, by system, for the announcements the string stages. `unsupported`: the table has no link to the
/// production window, or the adapter gave no facts for a system.
#[derive(Debug, Clone)]
pub struct ArborecHero {
    pub who: Seat,
    facts: Vec<ProduceFacts>,
    stage: HStage,
    systems: Vec<System>,
    at: usize,
    child: ErasedChild,
    pub made: Vec<(System, Made)>,
    pub unsupported: bool,
}

impl ArborecHero {
    #[must_use]
    pub fn new(who: Seat, facts: Vec<ProduceFacts>) -> Self {
        Self { who, facts, stage: HStage::Start, systems: Vec::new(), at: 0, child: ErasedChild::idle(), made: Vec::new(), unsupported: false }
    }

    /// Some unit was made.
    #[must_use]
    pub fn used(&self) -> bool {
        self.made.iter().any(|(_, made)| !made.as_slice().is_empty())
    }

    /// The systems holding a ground force of the seat, in system order.
    #[must_use]
    pub fn ground_force_systems(core: &CoreState, who: Seat) -> Vec<System> {
        let mine = |p: &crate::state::Piece| p.owner() == who && fleet::is_ground_force(p.kind());
        let mut found: Vec<System> = core.board().systems().filter(|(_, cell)| cell.units.as_slice().iter().any(mine)).map(|(system, _)| system).collect();
        for (planet, cell) in core.board().planets() {
            if cell.units.as_slice().iter().any(mine)
                && let Some(system) = cell.placed_in.or(planet.data().system)
            {
                found.push(system);
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }
}

impl Window for ArborecHero {
    type Stage = HStage;

    fn stage(&self) -> HStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                HStage::Start => {
                    self.systems = Self::ground_force_systems(sx.state, self.who);
                    self.stage = HStage::Next;
                }
                HStage::Next => {
                    let Some(system) = self.systems.get(self.at).copied() else {
                        self.stage = HStage::Done;
                        continue;
                    };
                    let started = facts_of(&self.facts, self.who, system).and_then(|facts| sx.table.links.start(&Spawn::Produce { facts }));
                    match started {
                        Some(window) => {
                            self.child.start(window);
                            self.stage = HStage::Producing;
                        }
                        None => {
                            self.unsupported = true;
                            self.stage = HStage::Done;
                        }
                    }
                }
                HStage::Producing => match self.child.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(report)) => {
                        if let Some(system) = self.systems.get(self.at).copied() {
                            self.made.push((system, made_of(Some(report))));
                        }
                        self.at += 1;
                        self.stage = HStage::Next;
                    }
                    None => {
                        self.at += 1;
                        self.stage = HStage::Next;
                    }
                },
                HStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == HStage::Producing { self.child.ask_of(s, k) } else { None }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == HStage::Producing { self.child.answer(sx, pick) } else { Err(Illegal) }
    }
}

// -- Umbat ----------------------------------------------------------------------------------------

/// The suspension points of [`Umbat`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UStage {
    Start,
    /// Waiting on the holder: which player may produce.
    Player,
    /// Waiting on the chosen player: in which system (more than one qualifies).
    System,
    Producing,
    Done,
}

/// Umbat (Muaat agent): "ACTION: Exhaust this card to choose a player: that player may produce up to 2 units that each have a cost of 4 or less
/// in a system that contains one of their war suns or their flagship." Every player with such a system in which something can be built is
/// offered to the holder (a decline spends nothing); the chosen player picks the system when there is more than one (a decline spends the
/// agent and produces nothing); the production is one use by an ability for that player, with a limit of 2 and a cost ceiling of 4. The
/// agent is spent once a player was chosen. The carrier is the agent's own seat (readied) or a seat holding a readied Ssruu while another seat
/// holds Umbat, readied or exhausted ([`Self::copied`]: "This card has the text ability of each other player's agent, even if that agent is
/// exhausted"). A copy is atomic: the adapter restores the position when the recorded decisions stop before the window is over.
/// [`Self::used`]: the agent was spent. [`Self::made`]: what the use made.
#[derive(Debug, Clone)]
pub struct Umbat {
    pub who: Seat,
    facts: Vec<ProduceFacts>,
    stage: UStage,
    /// The players that can produce, in seat order, with the systems they could produce in.
    targets: Vec<(Seat, Vec<System>)>,
    /// The player chosen to produce.
    pub target: Option<Seat>,
    child: ErasedChild,
    pub made: Option<(System, Made)>,
    pub used: bool,
    pub unsupported: bool,
    /// The text is Ssruu's copy of Umbat (the carrier is no Muaat seat holding the card).
    pub copied: bool,
}

impl Umbat {
    #[must_use]
    pub fn new(who: Seat, facts: Vec<ProduceFacts>) -> Self {
        Self { who, facts, stage: UStage::Start, targets: Vec::new(), target: None, child: ErasedChild::idle(), made: None, used: false, unsupported: false, copied: false }
    }

    /// The text `who` may act on now (`factions::muaat::umbat_available`): Muaat's own readied agent, else a copy through a readied Ssruu
    /// of the agent another seat holds (readied or exhausted). `Some(copied)`; `None`: neither.
    #[must_use]
    pub fn carried(core: &CoreState, who: Seat) -> Option<bool> {
        let me = core.players().by_seat(who)?;
        if me.faction == crate::generated::Faction::TheEmbersOfMuaat && me.leaders.status.get(&Leader::Umbat) == Some(&LeaderStatus::Readied) {
            return Some(false);
        }
        let held = |s: &LeaderStatus| matches!(s, LeaderStatus::Readied | LeaderStatus::Exhausted);
        (me.leaders.status.get(&Leader::Ssruu) == Some(&LeaderStatus::Readied)
            && core.players().iter().any(|p| p.seat != who && p.leaders.status.get(&Leader::Umbat).is_some_and(held)))
        .then_some(true)
    }

    /// The systems holding a war sun or the flagship of the seat in the space area, in system order.
    pub fn candidates(core: &CoreState, who: Seat) -> Vec<System> {
        let mut found: Vec<System> = core
            .board()
            .systems()
            .filter(|(_, cell)| {
                cell.units
                    .as_slice()
                    .iter()
                    .any(|p| p.owner() == who && matches!(p.kind().data().base_type, UnitBaseType::Warsun | UnitBaseType::Flagship))
            })
            .map(|(system, _)| system)
            .collect();
        found.sort_unstable();
        found
    }

    fn start_use(&mut self, sx: &mut Sx<'_>, target: Seat, system: System) {
        let started = facts_of(&self.facts, target, system).and_then(|facts| sx.table.links.start(&Spawn::Produce { facts }));
        match started {
            Some(window) => {
                self.child.start(window);
                self.stage = UStage::Producing;
            }
            None => {
                self.unsupported = true;
                self.stage = UStage::Done;
            }
        }
    }

    fn systems_of(&self, seat: Seat) -> &[System] {
        self.targets.iter().find(|(s, _)| *s == seat).map_or(&[], |(_, systems)| systems.as_slice())
    }

    fn player_ask() -> Ask {
        Ask::Leaders(LAsk::UmbatPlayer)
    }

    fn system_ask() -> Ask {
        Ask::Leaders(LAsk::UmbatSystem)
    }
}

impl Window for Umbat {
    type Stage = UStage;

    fn stage(&self) -> UStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == UStage::Start {
            let carried = Self::carried(sx.state, self.who);
            self.copied = carried == Some(true);
            if carried.is_some() {
                let seats: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
                for seat in seats {
                    let systems: Vec<System> = Self::candidates(sx.state, seat)
                        .into_iter()
                        .filter(|system| {
                            facts_of(&self.facts, seat, *system)
                                .and_then(|facts| sx.table.links.start(&Spawn::Produce { facts }))
                                .is_some_and(|probe| probe.ask_of(sx.state, sx.kernel).is_some())
                        })
                        .collect();
                    if !systems.is_empty() {
                        self.targets.push((seat, systems));
                    }
                }
            }
            self.stage = if self.targets.is_empty() { UStage::Done } else { UStage::Player };
        }
        if self.stage == UStage::Producing {
            match self.child.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    let made = made_of(Some(report));
                    if let Some(system) = self.made.as_ref().map(|(system, _)| *system) {
                        self.made = Some((system, made));
                    }
                    self.stage = UStage::Done;
                }
                None => self.stage = UStage::Done,
            }
        }
        match self.stage {
            UStage::Player => Next::Continue(Self::player_ask()),
            UStage::System => Next::Continue(Self::system_ask()),
            UStage::Start | UStage::Producing | UStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            UStage::Player => Some(Choice {
                actor: self.who,
                ask: Self::player_ask(),
                ctx: ctx(Leader::Umbat, LSub::UmbatPlayer),
                opts: self.targets.iter().map(|(seat, _)| Opt::Leaders(LOpt::Player(*seat))).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
            }),
            UStage::System => {
                let target = self.target?;
                Some(Choice {
                    actor: target,
                    ask: Self::system_ask(),
                    ctx: ctx(Leader::Umbat, LSub::UmbatSystem),
                    opts: self.systems_of(target).iter().map(|system| Opt::Leaders(LOpt::System(*system))).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
                })
            }
            UStage::Producing => self.child.ask_of(s, k),
            UStage::Start | UStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == UStage::Producing {
            return self.child.answer(sx, pick);
        }
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        match (self.stage, chosen) {
            (UStage::Player, Opt::Decline) => {
                self.stage = UStage::Done;
                Ok(())
            }
            (UStage::Player, Opt::Leaders(LOpt::Player(target))) if self.targets.iter().any(|(seat, _)| *seat == target) => {
                self.target = Some(target);
                self.used = true;
                match self.systems_of(target) {
                    [only] => {
                        let system = *only;
                        self.made = Some((system, Made::new()));
                        self.start_use(sx, target, system);
                    }
                    _ => self.stage = UStage::System,
                }
                Ok(())
            }
            // The agent is spent and nothing is produced.
            (UStage::System, Opt::Decline) => {
                self.stage = UStage::Done;
                Ok(())
            }
            (UStage::System, Opt::Leaders(LOpt::System(system))) => {
                let target = self.target.ok_or(Illegal)?;
                if !self.systems_of(target).contains(&system) {
                    return Err(Illegal);
                }
                self.made = Some((system, Made::new()));
                self.start_use(sx, target, system);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
