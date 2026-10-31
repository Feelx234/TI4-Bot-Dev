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
use crate::kernel::{fx, fx_move};
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
/// [`Self::used`]: some planet was devoured (a galvanized ground force that falls is staged with the string's trailing `|g` field).
#[derive(Debug, Clone)]
pub struct NekroHero {
    pub who: Seat,
    /// The interned `nekrohero`, the source of the staged trade goods.
    source: Code,
    planets: Vec<Devourable>,
    stage: FStage,
    chosen: Option<usize>,
    pub used: bool,
}

impl NekroHero {
    #[must_use]
    pub fn new(who: Seat, source: Code, planets: Vec<Devourable>) -> Self {
        Self { who, source, planets, stage: FStage::Start, chosen: None, used: false }
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
            if fleet::is_ground_force(piece.kind()) {
                fx_ground_stage(sx.state, system, planet, *piece);
            }
        }
        fx::gain_trade_goods_staged(sx.state, self.who, target.goods, self.source);
        if let Some(tech) = tech {
            fx::grant_technology(sx.state, self.who, tech);
        }
    }
}

/// `hooks_ground::stage_ground_force_destroyed` with the cause `nekrohero`.
fn fx_ground_stage(core: &mut CoreState, system: System, planet: Planet, piece: crate::state::Piece) {
    crate::kernel::fx_ground::stage_destroyed(core, system, planet, piece, DestroyReason::LeaderBare(Leader::UNITDSGNFLAYESH));
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

// -- Kyver, Blade and Key ---------------------------------------------------------------------------

/// What Kyver does to one player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    Take,
    Discard,
    Neither,
}

/// The suspension points of [`KyverHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KStage {
    Start,
    /// Waiting on a player: which card of their hand to show.
    Show,
    /// Waiting on the holder: what happens to the player who showed a card.
    Decide,
    /// The hand limit of the holder (a child).
    Limit,
    Done,
}

/// Kyver, Blade and Key (Yssaril hero): "ACTION: Each other player shows you 1 action card from their hand. For each player, choose 1 of
/// those cards and add it to your hand or force that player to discard 3 random action cards. Then, purge this card." The players holding
/// a card are asked in seat order which card to show (one distinct printed card is shown unasked); then the holder decides for each shown
/// card, none changing before the last decision: the taken cards go first (each staged as taken), then each discarding player loses three
/// random cards from the `YssarilHero` stream (a die over the whole hand each time, staged as discarded), then the hand limit of the holder
/// is the action-card area's window as a child (`limited`: false for a seat whose faction lifts the limit, Yssaril's Crafty).
/// [`Self::used`]: some card was shown. `unsupported`: the table has no printed-name groups or no link to the hand-limit window.
#[derive(Debug, Clone)]
pub struct KyverHero {
    pub who: Seat,
    limited: bool,
    groups: Vec<u8>,
    stage: KStage,
    /// The other seats holding an action card, in seat order, and the one being asked.
    targets: Vec<Seat>,
    at: usize,
    /// The cards shown, in the order of the targets.
    shown: Vec<(Seat, crate::generated::ActionCard)>,
    decided: Vec<Fate>,
    hand: crate::kernel::children::ErasedChild,
    pub used: bool,
    pub unsupported: bool,
}

impl KyverHero {
    #[must_use]
    pub fn new(who: Seat, limited: bool) -> Self {
        Self {
            who,
            limited,
            groups: Vec::new(),
            stage: KStage::Start,
            targets: Vec::new(),
            at: 0,
            shown: Vec::new(),
            decided: Vec::new(),
            hand: crate::kernel::children::ErasedChild::idle(),
            used: false,
            unsupported: false,
        }
    }

    /// The first copy of each printed card in the hand of `seat`, in the order of the names.
    fn distinct(&self, core: &CoreState, seat: Seat) -> Vec<crate::generated::ActionCard> {
        let Some(p) = core.players().by_seat(seat) else { return Vec::new() };
        let mut seen: Vec<u8> = Vec::new();
        let mut firsts: Vec<crate::generated::ActionCard> = Vec::new();
        for card in p.cards.hand.as_slice() {
            let Some(group) = self.groups.get(*card as usize).copied() else { return Vec::new() };
            if !seen.contains(&group) {
                seen.push(group);
                firsts.push(*card);
            }
        }
        firsts.sort_by_key(|card| card.name_rank());
        firsts
    }

    /// Ask the next player (or go on to the decisions).
    fn next_show(&mut self, sx: &mut Sx<'_>) {
        while let Some(&target) = self.targets.get(self.at) {
            match self.distinct(sx.state, target).as_slice() {
                [] => {
                    self.at += 1;
                }
                [only] => {
                    self.shown.push((target, *only));
                    self.at += 1;
                }
                _ => {
                    self.stage = KStage::Show;
                    return;
                }
            }
        }
        self.used = !self.shown.is_empty();
        self.stage = if self.shown.is_empty() { KStage::Done } else { KStage::Decide };
    }

    /// Every decision is made: change the hands.
    fn apply(&mut self, sx: &mut Sx<'_>) {
        let decided: Vec<(Seat, crate::generated::ActionCard, Fate)> = self.shown.iter().zip(&self.decided).map(|((s, c), f)| (*s, *c, *f)).collect();
        for (target, card, fate) in &decided {
            if *fate == Fate::Take {
                fx::take_action_card_staged(sx.state, self.who, *target, *card);
            }
        }
        for (target, _, fate) in &decided {
            if *fate != Fate::Discard {
                continue;
            }
            for _ in 0..3 {
                let held = sx.state.players().by_seat(*target).map_or(0, |p| p.cards.hand.as_slice().len());
                if held == 0 {
                    break;
                }
                let sides = u32::try_from(held).unwrap_or(u32::MAX);
                let index = sx.die(crate::state::RngDomain::YssarilHero, sides) as usize - 1;
                let picked = sx.state.players().by_seat(*target).and_then(|p| p.cards.hand.as_slice().get(index).copied());
                if let Some(picked) = picked {
                    fx::discard_action_card_staged(sx.state, *target, picked);
                }
            }
        }
        if !self.limited {
            self.stage = KStage::Done;
            return;
        }
        match sx.table.links.start(&crate::kernel::children::Spawn::DrawCards { who: self.who, count: 0, drop_unofficial: false }) {
            Some(window) => {
                self.hand.start(window);
                self.stage = KStage::Limit;
            }
            None => {
                self.unsupported = true;
                self.stage = KStage::Done;
            }
        }
    }
}

impl Window for KyverHero {
    type Stage = KStage;

    fn stage(&self) -> KStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        use crate::kernel::stage::Ran;
        if self.stage == KStage::Start {
            let Some(groups) = sx.table.facts.card_group_table() else {
                self.unsupported = true;
                self.stage = KStage::Done;
                return Next::Done;
            };
            self.groups = groups.to_vec();
            self.targets = sx
                .state
                .players()
                .iter()
                .filter(|p| p.seat != self.who && !p.cards.hand.as_slice().is_empty())
                .map(|p| p.seat)
                .collect();
            self.next_show(sx);
        }
        if self.stage == KStage::Limit {
            match self.hand.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    self.unsupported |= report == crate::kernel::children::Report::Left;
                    self.stage = KStage::Done;
                }
                None => self.stage = KStage::Done,
            }
        }
        match self.stage {
            KStage::Show => Next::Continue(Ask::Cards(crate::choice::cards::Ask::OwnHand { reason: crate::choice::cards::Reason::Show { viewer: self.who } })),
            KStage::Decide => {
                let (target, card) = self.shown[self.decided.len()];
                Next::Continue(Ask::Leaders(LAsk::KyverFate { target, card }))
            }
            KStage::Start | KStage::Limit | KStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
        match self.stage {
            KStage::Show => {
                let target = *self.targets.get(self.at)?;
                let reason = Reason::Show { viewer: self.who };
                Some(Choice {
                    actor: target,
                    ask: Ask::Cards(CAsk::OwnHand { reason }),
                    ctx: Ctx {
                        origin: Origin::FactionAbility(Content::Leader(Leader::KyverBladeAndKey)),
                        subtype: Subtype::Cards(reason.subtype()),
                        optional: false,
                        target: None,
                        outstanding: SmallList::new(),
                        space_battle: false,
                        invasion_seq: None,
                    },
                    opts: self.distinct(s, target).into_iter().map(|card| Opt::Cards(COpt::Card(card))).collect::<Opts>(),
                })
            }
            KStage::Decide => {
                let (target, card) = *self.shown.get(self.decided.len())?;
                Some(Choice {
                    actor: self.who,
                    ask: Ask::Leaders(LAsk::KyverFate { target, card }),
                    ctx: ctx(Leader::KyverBladeAndKey, LSub::KyverFate),
                    opts: [Opt::Leaders(LOpt::KyverTake), Opt::Leaders(LOpt::KyverDiscard), Opt::Decline].into_iter().collect::<Opts>(),
                })
            }
            KStage::Limit => self.hand.ask_of(s, k),
            KStage::Start | KStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        use crate::choice::cards::Opt as COpt;
        match (self.stage, pick) {
            (KStage::Limit, pick) => self.hand.answer(sx, pick),
            (KStage::Show, Pick::Opt(Opt::Cards(COpt::Card(card)))) => {
                let target = *self.targets.get(self.at).ok_or(Illegal)?;
                if !self.distinct(sx.state, target).contains(&card) {
                    return Err(Illegal);
                }
                self.shown.push((target, card));
                self.at += 1;
                self.next_show(sx);
                Ok(())
            }
            (KStage::Decide, Pick::Opt(chosen)) => {
                let fate = match chosen {
                    Opt::Leaders(LOpt::KyverTake) => Fate::Take,
                    Opt::Leaders(LOpt::KyverDiscard) => Fate::Discard,
                    Opt::Decline => Fate::Neither,
                    _ => return Err(Illegal),
                };
                self.decided.push(fate);
                if self.decided.len() == self.shown.len() {
                    self.apply(sx);
                }
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- It Feeds on Carrion ------------------------------------------------------------------------------

use crate::kernel::children::{ErasedChild, Made, Report, SeeingLimits, Spawn};
use crate::state::Piece;

/// The suspension points of [`CabalHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStage {
    Start,
    /// The next affected owner and system is enforced (a child).
    Next,
    Enforcing,
    Done,
}

/// The owner and system whose cargo the captured ships strand, with what the adapter read for the enforcement there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stranded {
    pub owner: Seat,
    pub system: System,
    pub limits: SeeingLimits,
}

/// It Feeds on Carrion (Cabal hero): "ACTION: Each other player rolls a die for each of his non-fighter ships that are in or adjacent to a
/// system that contains a dimensional tear. On a 1-3, capture that unit. If this causes a player's ground forces or fighters to be removed,
/// also capture those units. Then, purge this card." `targets` are the ships rolled for, in the order the string lists them (the system
/// text order, the board order inside a system): each draws a die from the dice stream, a 1 to 3 captures it from the board. The cargo the
/// captures strand is the enforcement of the capacity of each owner and system that lost a ship (`stranded`, in the string's order of
/// (owner text, system text)): a child asks the owner which units go, and every fighter or ground force in the space area that is gone
/// afterwards is captured by the holder (a row for the owner, in the order of the unit ids). `playable`: a Cabal seat, a tear and a map
/// (the adapter read them; an unplayable card changes nothing). [`Self::used`]: the card resolved.
#[derive(Debug, Clone)]
pub struct CabalHero {
    pub who: Seat,
    playable: bool,
    targets: Vec<(System, Piece)>,
    stranded: Vec<Stranded>,
    stage: CStage,
    /// The pairs that lost a ship, in the order of `stranded`.
    affected: Vec<usize>,
    at: usize,
    before: Vec<crate::generated::Unit>,
    child: ErasedChild,
    pub used: bool,
    pub unsupported: bool,
}

impl CabalHero {
    #[must_use]
    pub fn new(who: Seat, playable: bool, targets: Vec<(System, Piece)>, stranded: Vec<Stranded>) -> Self {
        Self { who, playable, targets, stranded, stage: CStage::Start, affected: Vec::new(), at: 0, before: Vec::new(), child: ErasedChild::idle(), used: false, unsupported: false }
    }

    /// The fighters and ground forces of `owner` in the space area of `system`, ordered by unit form (the order of the id text).
    fn cargo(core: &CoreState, owner: Seat, system: System) -> Vec<crate::generated::Unit> {
        let mut held: Vec<crate::generated::Unit> =
            core.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == owner && fleet::consumes_capacity(p.kind())).map(|p| p.kind()).collect();
        // The unit forms are numbered in the order of their id text (the string sorts the ids).
        held.sort();
        held
    }

    /// The captures the removal of cargo caused: whatever was there before and is not now.
    fn capture_the_gone(&mut self, sx: &mut Sx<'_>) {
        let Some(pair) = self.affected.get(self.at).and_then(|i| self.stranded.get(*i)).copied() else { return };
        let mut after = Self::cargo(sx.state, pair.owner, pair.system);
        for unit in std::mem::take(&mut self.before) {
            if let Some(index) = after.iter().position(|kept| *kept == unit) {
                after.remove(index);
            } else if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                p.misc.captured_units.push((pair.owner, unit));
            }
        }
    }
}

impl Window for CabalHero {
    type Stage = CStage;

    fn stage(&self) -> CStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        use crate::kernel::stage::Ran;
        loop {
            match self.stage {
                CStage::Start => {
                    if !self.playable {
                        self.stage = CStage::Done;
                        continue;
                    }
                    self.used = true;
                    // Every die first, in the order of the ships; then the captures, in the same order.
                    let mut hit: Vec<(System, Piece)> = Vec::new();
                    for (system, piece) in &self.targets {
                        if sx.die(crate::state::RngDomain::Dice, 10) <= 3 {
                            hit.push((*system, *piece));
                        }
                    }
                    let mut touched: Vec<(Seat, System)> = Vec::new();
                    for (system, piece) in hit {
                        if fx::capture_from_board(sx.state, self.who, system, None, piece) {
                            touched.push((piece.owner(), system));
                        }
                    }
                    self.affected = self.stranded.iter().enumerate().filter(|(_, s)| touched.contains(&(s.owner, s.system))).map(|(i, _)| i).collect();
                    self.stage = CStage::Next;
                }
                CStage::Next => {
                    let Some(pair) = self.affected.get(self.at).and_then(|i| self.stranded.get(*i)).copied() else {
                        self.stage = CStage::Done;
                        continue;
                    };
                    self.before = Self::cargo(sx.state, pair.owner, pair.system);
                    match sx.table.links.start(&Spawn::Seeing { who: pair.owner, system: pair.system, limits: pair.limits }) {
                        Some(window) => {
                            self.child.start(window);
                            self.stage = CStage::Enforcing;
                        }
                        None => {
                            self.unsupported = true;
                            self.stage = CStage::Done;
                        }
                    }
                }
                CStage::Enforcing => match self.child.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(report)) => {
                        debug_assert!(matches!(report, Report::Removed(_)));
                        self.capture_the_gone(sx);
                        self.at += 1;
                        self.stage = CStage::Next;
                    }
                    None => {
                        self.at += 1;
                        self.stage = CStage::Next;
                    }
                },
                CStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == CStage::Enforcing { self.child.ask_of(s, k) } else { None }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == CStage::Enforcing { self.child.answer(sx, pick) } else { Err(Illegal) }
    }
}

// -- Hesh and Prit, Mathis Mathinus -----------------------------------------------------------------------

use crate::generated::{StrategyCard, StrategyCardKind};
use crate::kernel::fx_perform::{self, Facts as PerformFacts, Began};
use crate::query::economy;

/// Which child a performed ability ended in: the staged announcements the adapter makes afterwards.
fn perform_step(child: &mut ErasedChild, sx: &mut Sx<'_>, trade_source: Code) -> Option<Result<fx_perform::Finished, Next>> {
    use crate::kernel::stage::Ran;
    match child.step(sx)? {
        Ran::Asks(ask) => Some(Err(Next::Continue(ask))),
        Ran::Pass(next) => Some(Err(next)),
        Ran::Done(report) => Some(Ok(fx_perform::finish(report, sx.state, trade_source))),
    }
}

/// The suspension points of [`NaazHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NStage {
    Start,
    /// Waiting on the strategy card (up to two; declining ends the choice).
    Card,
    /// The next chosen card's secondary begins.
    Next,
    Performing,
    Done,
}

/// Hesh and Prit (Naaz hero): "ACTION: Gain 1 relic and perform the secondary ability of up to 2 strategy cards. You do not need to spend
/// command tokens for these abilities. Then, purge this card." With a relic in the deck, the seat chooses up to two cards among `cards`
/// (the cards that are readied, or unchosen, in the order of the adapter; a card whose secondary costs a token is offered only while the
/// seat has one in its reinforcements), declining ends the choice; then the relic is gained and each chosen secondary is performed in turn
/// by the ability's own window as a child ([`fx_perform::begin`]). A chosen secondary that costs a token stops the run when the
/// reinforcements ran out. [`Self::used`]: the relic was gained. `unsupported`: an ability the core does not carry (the Warfare secondary at a
/// table the production windows do not carry or after a secondary that changes what a production reads, a hook-changed draw, a research a faction waiver opens) or no link to the child: the adapter declines the run.
#[derive(Debug, Clone)]
pub struct NaazHero {
    pub who: Seat,
    facts: PerformFacts,
    cards: Vec<StrategyCard>,
    stage: NStage,
    chosen: Vec<StrategyCard>,
    at: usize,
    child: ErasedChild,
    pub used: bool,
    pub unsupported: bool,
    /// What the home production of the Warfare secondary made, in order (the adapter announces it).
    pub home_made: Option<Made>,
}

/// Whether the secondary of `card` changes what a production reads: Leadership gains command tokens (the fleet pool), Technology may
/// research a unit upgrade, Construction places a space dock.
fn changes_production(card: StrategyCard) -> bool {
    matches!(card.data().kind, StrategyCardKind::Leadership | StrategyCardKind::Technology | StrategyCardKind::Construction)
}

/// "Every secondary but Leadership's (influence) costs a command token."
fn costs_token(card: StrategyCard) -> bool {
    card.data().kind != StrategyCardKind::Leadership
}

impl NaazHero {
    #[must_use]
    pub fn new(who: Seat, facts: PerformFacts, cards: Vec<StrategyCard>) -> Self {
        Self { who, facts, cards, stage: NStage::Start, chosen: Vec::new(), at: 0, child: ErasedChild::idle(), used: false, unsupported: false, home_made: None }
    }

    fn spare(&self, core: &CoreState) -> bool {
        economy::tokens_in_reinforcements(core, self.who, self.facts.mahact_held) > 0
    }

    /// The cards that can still be chosen: not chosen yet, and affordable.
    fn candidates(&self, core: &CoreState) -> Vec<StrategyCard> {
        let spare = self.spare(core);
        self.cards.iter().copied().filter(|card| !self.chosen.contains(card)).filter(|card| spare || !costs_token(*card)).collect()
    }

    fn choose_next(&mut self, sx: &mut Sx<'_>) {
        if self.chosen.len() < 2 && !self.candidates(sx.state).is_empty() {
            self.stage = NStage::Card;
        } else {
            self.gain(sx);
        }
    }

    /// Every choice is made: the relic, then the secondaries.
    fn gain(&mut self, sx: &mut Sx<'_>) {
        if crate::kernel::fx_relic::relic_gain(sx.state, self.who).is_none() {
            self.stage = NStage::Done;
            return;
        }
        self.used = true;
        self.stage = NStage::Next;
    }
}

impl Window for NaazHero {
    type Stage = NStage;

    fn stage(&self) -> NStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                NStage::Start => {
                    if sx.state.decks().relic.is_empty() {
                        self.stage = NStage::Done;
                    } else {
                        self.choose_next(sx);
                    }
                }
                NStage::Card => return Next::Continue(Ask::Leaders(LAsk::NaazCard)),
                NStage::Next => {
                    let Some(card) = self.chosen.get(self.at).copied() else {
                        self.stage = NStage::Done;
                        continue;
                    };
                    // A secondary that costs a token ends the run when the reinforcements have none.
                    if costs_token(card) && !self.spare(sx.state) {
                        self.stage = NStage::Done;
                        continue;
                    }
                    // The facts of the home production were read where the card started: a secondary performed before it that changes what the
                    // production reads (the fleet pool, the units that may be built, the capacity of the home system) makes them stale.
                    if card.data().kind == StrategyCardKind::Warfare && self.chosen[..self.at].iter().any(|before| changes_production(*before)) {
                        self.unsupported = true;
                        self.stage = NStage::Done;
                        continue;
                    }
                    match fx_perform::begin(sx, self.who, card, false, self.facts) {
                        Began::Done => self.at += 1,
                        Began::Unsupported => {
                            self.unsupported = true;
                            self.stage = NStage::Done;
                        }
                        Began::Run(request) => match sx.table.links.start(&request) {
                            Some(window) => {
                                self.child.start(window);
                                self.stage = NStage::Performing;
                            }
                            None => {
                                self.unsupported = true;
                                self.stage = NStage::Done;
                            }
                        },
                    }
                }
                NStage::Performing => match perform_step(&mut self.child, sx, self.facts.carried.trade_source) {
                    Some(Err(next)) => return next,
                    Some(Ok(done)) => {
                        self.unsupported |= done.failed;
                        if done.made.is_some() {
                            self.home_made = done.made;
                        }
                        self.at += 1;
                        self.stage = if done.failed { NStage::Done } else { NStage::Next };
                    }
                    None => {
                        self.at += 1;
                        self.stage = NStage::Next;
                    }
                },
                NStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            NStage::Card => Some(Choice {
                actor: self.who,
                ask: Ask::Leaders(LAsk::NaazCard),
                ctx: ctx(Leader::HeshAndPrit, LSub::HeroCard),
                opts: self.candidates(s).into_iter().map(|card| Opt::Leaders(LOpt::HeroCard(card))).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
            }),
            NStage::Performing => self.child.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (NStage::Performing, pick) => self.child.answer(sx, pick),
            (NStage::Card, Pick::Opt(Opt::Decline)) => {
                self.gain(sx);
                Ok(())
            }
            (NStage::Card, Pick::Opt(Opt::Leaders(LOpt::HeroCard(card)))) => {
                if !self.candidates(sx.state).contains(&card) {
                    return Err(Illegal);
                }
                self.chosen.push(card);
                self.choose_next(sx);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

/// The suspension points of [`WinnuHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WStage {
    Start,
    Card,
    Performing,
    Done,
}

/// Mathis Mathinus (Winnu hero): "ACTION: Choose 1 strategy card on the board; perform its primary ability, then each other player may
/// follow its secondary." The seat chooses one of `cards` (a question even for one) and the primary of the card is performed by the
/// ability's own window as a child ([`fx_perform::begin`]); who may follow is the game's (`Game::select_leader_strategy_followers`).
/// [`Self::chosen`]: the card. `unsupported`: a primary the core does not carry (the Imperial primary at a table the scoring tables do not follow, the Thunder's Edge Construction and
/// Warfare primaries at a table whose facts the adapter could not give, a hook-changed draw, a research a faction waiver opens) or no link to the child.
/// [`Self::tokens`]: the system the Diplomacy primary placed tokens in and the seats that got one, announced by the adapter.
#[derive(Debug, Clone)]
pub struct WinnuHero {
    pub who: Seat,
    facts: PerformFacts,
    cards: Vec<StrategyCard>,
    stage: WStage,
    pub chosen: Option<StrategyCard>,
    child: ErasedChild,
    pub tokens: Option<(System, SmallList<Seat, 8>)>,
    /// The public objective the Imperial primary scored (what the score sets off is the adapter's).
    pub scored: Option<crate::generated::PublicObjective>,
    /// The system of the free tactical action of the Thunder's Edge Warfare primary: the game plays it next (`Ability::FreeTactical`).
    pub free: Option<System>,
    /// What the use of PRODUCTION of the Thunder's Edge Construction primary made, with the system it ran in (the adapter announces it).
    pub produced: Option<(System, Made)>,
    pub unsupported: bool,
}

impl WinnuHero {
    #[must_use]
    pub fn new(who: Seat, facts: PerformFacts, cards: Vec<StrategyCard>) -> Self {
        Self { who, facts, cards, stage: WStage::Start, chosen: None, child: ErasedChild::idle(), tokens: None, scored: None, free: None, produced: None, unsupported: false }
    }
}

impl Window for WinnuHero {
    type Stage = WStage;

    fn stage(&self) -> WStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                WStage::Start => self.stage = if self.cards.is_empty() { WStage::Done } else { WStage::Card },
                WStage::Card => return Next::Continue(Ask::Leaders(LAsk::WinnuCard)),
                WStage::Performing => match perform_step(&mut self.child, sx, self.facts.carried.trade_source) {
                    Some(Err(next)) => return next,
                    Some(Ok(done)) => {
                        self.unsupported |= done.failed;
                        self.tokens = done.tokens;
                        self.scored = done.scored;
                        self.free = done.free;
                        self.produced = done.made.zip(done.made_in).map(|(made, system)| (system, made));
                        self.stage = WStage::Done;
                    }
                    None => self.stage = WStage::Done,
                },
                WStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            WStage::Card => Some(Choice {
                actor: self.who,
                ask: Ask::Leaders(LAsk::WinnuCard),
                ctx: ctx(Leader::MathisMathinus, LSub::HeroCard),
                opts: self.cards.iter().map(|card| Opt::Leaders(LOpt::HeroCard(*card))).collect::<Opts>(),
            }),
            WStage::Performing => self.child.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (WStage::Performing, pick) => self.child.answer(sx, pick),
            (WStage::Card, Pick::Opt(Opt::Leaders(LOpt::HeroCard(card)))) => {
                if !self.cards.contains(&card) {
                    return Err(Illegal);
                }
                self.chosen = Some(card);
                match fx_perform::begin(sx, self.who, card, true, self.facts) {
                    Began::Done => self.stage = WStage::Done,
                    Began::Unsupported => {
                        self.unsupported = true;
                        self.stage = WStage::Done;
                    }
                    Began::Run(request) => match sx.table.links.start(&request) {
                        Some(window) => {
                            self.child.start(window);
                            self.stage = WStage::Performing;
                        }
                        None => {
                            self.unsupported = true;
                            self.stage = WStage::Done;
                        }
                    },
                }
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Flock Migration ------------------------------------------------------------------------------------

use crate::query::transit::{self, Cand};
use crate::query::board as qboard;

/// What the Argent hero reads of the string position: the order of its board (the string keeps systems under their id text) and the two
/// map rules a relocation checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFacts {
    /// Every system of the string board, in the order of its id text.
    pub order: Vec<System>,
    /// The systems on the map (a coordinate or a wormhole): a relocation needs both ends on it.
    pub on_map: Vec<System>,
    /// The systems the seat's movement rules let it enter (laws, technologies, faction effects).
    pub enterable: Vec<System>,
}

/// The suspension points of [`ArgentHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AStage {
    Start,
    /// Waiting on the ship that moves next.
    Ship,
    /// Waiting on its destination.
    To,
    /// Waiting on a unit carried aboard.
    Cargo,
    Done,
}

/// Mirik Aun Sissiri (Argent hero): "ACTION: Move any number of your ships from any systems to any number of other systems that contain 1 of
/// your command tokens and no other players' ships. Then, purge this card." One ship at a time, each moved once: the seat chooses a ship
/// (one per distinct stack of a system that still has a copy that has not arrived there), then a destination, then the units carried aboard
/// until the ship is full or the seat stops (the hold is `capacity + 8` units long). Declining any question ends the migration. The
/// relocation checks the ends (on the map, enterable), the cargo (present, within the capacity); a refused cargo is dropped, a refused
/// relocation ends the card. [`Self::moved`]: some ship moved.
#[derive(Debug, Clone)]
pub struct ArgentHero {
    pub who: Seat,
    facts: MigrationFacts,
    stage: AStage,
    ships: Vec<(System, Piece)>,
    destinations: Vec<System>,
    from: Option<(System, Piece)>,
    targets: Vec<System>,
    to: Option<System>,
    offered: Vec<Cand>,
    cargo: Vec<Cand>,
    /// The ships that moved already, by where they arrived.
    arrived: Vec<(System, Piece)>,
    pub moved: bool,
}

impl ArgentHero {
    #[must_use]
    pub fn new(who: Seat, facts: MigrationFacts) -> Self {
        Self { who, facts, stage: AStage::Start, ships: Vec::new(), destinations: Vec::new(), from: None, targets: Vec::new(), to: None, offered: Vec::new(), cargo: Vec::new(), arrived: Vec::new(), moved: false }
    }

    fn is_ship(piece: Piece) -> bool {
        piece.kind().data().ship
    }

    /// The systems that hold the seat's command token and no other player's ship, in the string's order.
    fn destinations_of(&self, core: &CoreState) -> Vec<System> {
        self.facts
            .order
            .iter()
            .copied()
            .filter(|system| qboard::has_token(core, *system, self.who))
            .filter(|system| !qboard::space_units(core, *system).iter().any(|p| p.owner() != self.who && Self::is_ship(*p)))
            .collect()
    }

    /// Each distinct ship (system, form, damage) that can still move somewhere, in the string's order.
    fn ships_of(&self, core: &CoreState, destinations: &[System]) -> Vec<(System, Piece)> {
        let mut found: Vec<(System, Piece)> = Vec::new();
        for system in &self.facts.order {
            for piece in qboard::space_units(core, *system).iter().filter(|p| p.owner() == self.who && Self::is_ship(**p)) {
                if !found.iter().any(|(at, seen)| at == system && seen == piece) {
                    found.push((*system, *piece));
                }
            }
        }
        found
            .into_iter()
            .filter(|(from, _)| destinations.iter().any(|to| to != from))
            .filter(|(from, piece)| {
                let here = qboard::space_units(core, *from).iter().filter(|p| **p == *piece).count();
                let came = self.arrived.iter().filter(|(at, other)| at == from && other == piece).count();
                here > came
            })
            .collect()
    }

    /// Choose the next ship, or end.
    fn next_ship(&mut self, sx: &mut Sx<'_>) {
        self.destinations = self.destinations_of(sx.state);
        self.ships = self.ships_of(sx.state, &self.destinations);
        self.stage = if self.ships.is_empty() { AStage::Done } else { AStage::Ship };
    }

    fn capacity(&self) -> i64 {
        self.from.map_or(0, |(_, ship)| crate::query::fleet::capacity(ship.kind()))
    }

    /// The destination is chosen: the hold is asked, or the ship moves.
    fn to_chosen(&mut self, sx: &mut Sx<'_>, to: System) {
        self.to = Some(to);
        let Some((from, _)) = self.from else { return };
        self.offered = transit::loadable(sx.state, sx.kernel, self.who, from, false);
        self.cargo.clear();
        self.ask_cargo_or_move(sx);
    }

    fn ask_cargo_or_move(&mut self, sx: &mut Sx<'_>) {
        let room = self.capacity() + 8;
        if !self.offered.is_empty() && i64::try_from(self.cargo.len()).unwrap_or(i64::MAX) < room {
            self.stage = AStage::Cargo;
        } else {
            self.relocate(sx);
        }
    }

    /// The relocation with the cargo; when it is refused, the one without; when that is refused, the card ends.
    fn relocate(&mut self, sx: &mut Sx<'_>) {
        let (Some((from, ship)), Some(to)) = (self.from, self.to) else {
            self.stage = AStage::Done;
            return;
        };
        let ends = self.facts.on_map.contains(&from) && self.facts.on_map.contains(&to) && self.facts.enterable.contains(&to);
        let carried = if ends && self.cargo_fits(sx.state, sx.kernel, from, ship) { std::mem::take(&mut self.cargo) } else { Vec::new() };
        if !ends {
            self.stage = AStage::Done;
            return;
        }
        fx_move::move_ship(sx.state, from, to, ship);
        for cand in &carried {
            fx_move::take_aboard(sx.state, cand.system, to, cand.piece, cand.source);
        }
        self.arrived.push((to, ship));
        self.moved = true;
        self.next_ship(sx);
    }

    /// `transit::relocate_ships_with_cargo`'s checks of the cargo: every unit is still there (copies counted, after the ship is lifted) and the
    /// units that take a slot fit the ship.
    fn cargo_fits(&self, core: &CoreState, k: &Kernel, from: System, ship: Piece) -> bool {
        if self.cargo.is_empty() {
            return true;
        }
        let mut standing: Vec<Piece> = qboard::space_units(core, from).to_vec();
        if let Some(at) = standing.iter().position(|p| *p == ship) {
            standing.remove(at);
        } else {
            return false;
        }
        let mut planets: Vec<(Planet, Vec<Piece>)> = Vec::new();
        let mut paying = 0_i64;
        for cand in &self.cargo {
            let pool: Option<&mut Vec<Piece>> = match cand.source {
                None => Some(&mut standing),
                Some(planet) => {
                    if !planets.iter().any(|(p, _)| *p == planet) {
                        planets.push((planet, qboard::planet_units(core, planet).to_vec()));
                    }
                    planets.iter_mut().find(|(p, _)| *p == planet).map(|(_, units)| units)
                }
            };
            let found = pool.and_then(|units| units.iter().position(|p| *p == cand.piece).map(|at| units.remove(at)));
            if found.is_none() {
                return false;
            }
            if !k.hooks.movement.free_cargo(core, k, cand.piece) {
                paying += 1;
            }
        }
        paying <= crate::query::fleet::capacity(ship.kind())
    }
}

impl Window for ArgentHero {
    type Stage = AStage;

    fn stage(&self) -> AStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == AStage::Start {
            self.next_ship(sx);
        }
        match self.stage {
            AStage::Ship => Next::Continue(Ask::Leaders(LAsk::FlockShip)),
            AStage::To => Next::Continue(Ask::Leaders(LAsk::FlockTo)),
            AStage::Cargo => Next::Continue(Ask::Leaders(LAsk::FlockCargo)),
            AStage::Start | AStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        let c = |ask: LAsk, subtype: LSub, opts: Opts| Some(Choice { actor: self.who, ask: Ask::Leaders(ask), ctx: ctx(Leader::MirikAunSissiri, subtype), opts });
        match self.stage {
            AStage::Ship => c(
                LAsk::FlockShip,
                LSub::FlockShip,
                self.ships
                    .iter()
                    .enumerate()
                    .map(|(index, (system, piece))| Opt::Leaders(LOpt::FlockShip { index: u8::try_from(index).unwrap_or(u8::MAX), system: *system, unit: piece.kind() }))
                    .chain(std::iter::once(Opt::Decline))
                    .collect(),
            ),
            AStage::To => c(LAsk::FlockTo, LSub::FlockTo, self.targets.iter().map(|to| Opt::Leaders(LOpt::FlockTo(*to))).chain(std::iter::once(Opt::Decline)).collect()),
            AStage::Cargo => c(
                LAsk::FlockCargo,
                LSub::FlockCargo,
                self.offered
                    .iter()
                    .enumerate()
                    .map(|(index, cand)| Opt::Leaders(LOpt::FlockCargo { index: u8::try_from(index).unwrap_or(u8::MAX), unit: cand.piece.kind() }))
                    .chain(std::iter::once(Opt::Decline))
                    .collect(),
            ),
            AStage::Start | AStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        match (self.stage, chosen) {
            // Declining any question ends the migration.
            (AStage::Ship | AStage::To, Opt::Decline) => {
                self.stage = AStage::Done;
                Ok(())
            }
            (AStage::Ship, Opt::Leaders(LOpt::FlockShip { index, .. })) => {
                let (system, piece) = *self.ships.get(usize::from(index)).ok_or(Illegal)?;
                self.from = Some((system, piece));
                self.targets = self.destinations.iter().copied().filter(|to| *to != system).collect();
                self.stage = AStage::To;
                Ok(())
            }
            (AStage::To, Opt::Leaders(LOpt::FlockTo(to))) => {
                if !self.targets.contains(&to) {
                    return Err(Illegal);
                }
                self.to_chosen(sx, to);
                Ok(())
            }
            (AStage::Cargo, Opt::Decline) => {
                self.relocate(sx);
                Ok(())
            }
            (AStage::Cargo, Opt::Leaders(LOpt::FlockCargo { index, .. })) => {
                let at = usize::from(index);
                if at >= self.offered.len() {
                    return Err(Illegal);
                }
                let cand = self.offered.remove(at);
                self.cargo.push(cand);
                self.ask_cargo_or_move(sx);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Conservator Procyon -------------------------------------------------------------------------------

use crate::choice::explore::{Opt as EOpt, Step as EStep};
use crate::choice::strategy::{Ask as SAsk, Opt as SOpt};
use crate::generated::{Explore, ExploreResolution, ExploreType, Wormhole};
use crate::kernel::fx_explore;
use crate::query::explore::Holdings;

/// `ti4_model::state::RULES_COMMAND_TOKEN_POOL`: from this version each command token an exploration card gives goes into a pool of the
/// seat's choice.
const RULES_COMMAND_TOKEN_POOL: u32 = 12;
/// `ti4_model::state::RULES_MIRAGE_FRONTIER`: from this version the Mirage drawn from a frontier token is placed in the token's system.
const RULES_MIRAGE_FRONTIER: u32 = 27;
/// `ti4_model::state::RULES_NO_EMPTY_CONVERT`: from this version a seat without commodities has nothing to convert.
const RULES_NO_EMPTY_CONVERT: u32 = 19;
/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`.
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;

/// What [`EmpyreanHero`] reads of the position that the core does not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontierFacts {
    /// The interned empty text of an exploration log row that is still open.
    pub empty: Code,
    /// `GameState::rules_version`.
    pub rules_version: u32,
    /// `GameState::tokens_held_by_mahact` of the seat (the reinforcements a pool question previews differ).
    pub mahact_held: i32,
    /// The interned `trade_agreement`, the source of the goods a Trade Agreement moves when a Merchant Station replenishes; `None`: this
    /// table's replenishment is one the core does not follow (a siphoning plot, two seats of one faction).
    pub agreement: Option<Code>,
    /// A Lost Crew's two action cards are drawn plainly (no faction hook changes the draw or the hand limit).
    pub draw_plain: bool,
}

/// How a frontier card resolved (the words of the log row are the adapter's, from `exploration::row_words`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// A relic fragment of this deck was gained.
    Fragment(ExploreType),
    /// The card did what it says.
    Resolved,
    /// The card had nothing to act on (the ion storm without a planet, a gamma token without an active system): nothing happened.
    Unresolved,
}

/// One explored token, as the adapter closes its log row: the system, the card, how it resolved, what the seat held before the draw and
/// after the last effect, and the system a gamma token was newly placed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontierRow {
    pub system: System,
    pub card: Explore,
    pub how: How,
    pub before: Holdings,
    pub after: Holdings,
    pub gamma: Option<System>,
}

/// The suspension points of [`EmpyreanHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EStage {
    Start,
    /// The next token under a ship is explored.
    Next,
    /// A command token goes into a pool of the seat's choice.
    Pooling,
    /// The Merchant Station's choice (a child).
    Rewarding,
    /// The hand limit after a secret objective was drawn (a child).
    Limiting,
    /// A Lost Crew's two action cards (a child).
    Drawing,
    Done,
}

/// The card being resolved.
#[derive(Debug, Clone, Copy)]
struct Open {
    system: System,
    card: Explore,
    how: How,
    before: Holdings,
    gamma: Option<System>,
    goods: i32,
}

/// Conservator Procyon (Empyrean hero): "ACTION: Place 1 frontier token in each system that does not contain any planets and does not already
/// have a frontier token. Then, explore each frontier token that is in a system that contains 1 or more of your ships. Then, purge this
/// card." The tokens in `placed` (the planetless systems, the adapter's read of the map) are added, then every token under a ship of the seat
/// is explored in `order`: the token is spent, the top card of the frontier deck is drawn and its log row opened
/// (`exploration::explore_frontier`), the card resolves with no planet (a fragment, command tokens and trade goods, a relic, a secret
/// objective, action cards, a Merchant Station's choice, the Enigmatic Device kept in the play area, a gamma token where the active system
/// is, the Mirage in the token's system; the ion storm needs a planet and does nothing), and the seat's holdings around it close the row.
/// `order` lists every system that can hold a token in the text order of the system names (the core never sees the text: the adapter sorts).
/// A frontier deck that runs out is a failed exploration: the whole run is undone (the string restores the state, the dice and the stream
/// at the hero level) after the questions already asked, and the card is not used. [`Self::used`]: the card resolved.
/// [`Self::unsupported`]: a card of another deck, a table whose replenishment or draw the core does not follow, a command token in a Mahact
/// fleet pool, or no link to a child: the adapter declines the run. [`Self::rows`] are the log rows to close, in order.
#[derive(Debug, Clone)]
pub struct EmpyreanHero {
    pub who: Seat,
    facts: FrontierFacts,
    placed: Vec<System>,
    order: Vec<System>,
    stage: EStage,
    /// The position the run started from: a failed exploration restores it.
    snapshot: Option<Box<CoreState>>,
    targets: Vec<System>,
    at: usize,
    open: Option<Open>,
    rows: Vec<FrontierRow>,
    pool_left: u8,
    child: ErasedChild,
    pub used: bool,
    pub unsupported: bool,
}

impl EmpyreanHero {
    #[must_use]
    pub fn new(who: Seat, placed: Vec<System>, order: Vec<System>, facts: FrontierFacts) -> Self {
        Self { who, facts, placed, order, stage: EStage::Start, snapshot: None, targets: Vec::new(), at: 0, open: None, rows: Vec::new(), pool_left: 0, child: ErasedChild::idle(), used: false, unsupported: false }
    }

    /// The tokens explored, as log rows to close, in order.
    #[must_use]
    pub fn rows(&self) -> &[FrontierRow] {
        &self.rows
    }

    /// The run holds something the window does not carry: the adapter declines it.
    fn decline(&mut self) {
        self.unsupported = true;
        self.stage = EStage::Done;
    }

    /// The frontier deck ran out: the exploration failed, so the run is undone and the card is not used.
    fn fail(&mut self, sx: &mut Sx<'_>) {
        if let Some(before) = self.snapshot.take() {
            *sx.state = *before;
        }
        self.rows.clear();
        self.open = None;
        self.used = false;
        self.stage = EStage::Done;
    }

    /// `exploration::explore_frontier`: the token is spent, the card drawn and its row opened, then the card resolves as far as its first
    /// question.
    fn begin_one(&mut self, sx: &mut Sx<'_>, system: System) {
        let who = self.who;
        fx_explore::take_frontier(sx.state, system);
        let Some(card) = fx_explore::draw(sx.state, ExploreType::Frontier) else { return self.fail(sx) };
        fx_explore::open_record(sx.state, who, ExploreType::Frontier, card, None, Some(system), self.facts.empty);
        let before = crate::query::explore::holdings(sx.state, who, None);
        self.open = Some(Open { system, card, how: How::Resolved, before, gamma: None, goods: 0 });
        let data = card.data();
        match data.resolution {
            ExploreResolution::Fragment => {
                fx_explore::gain_fragment(sx.state, who, data.deck);
                self.set_how(How::Fragment(data.deck));
                self.finish_one(sx);
            }
            // A card of another deck (an attachment, a planet's reward) is not in the frontier deck of any game.
            ExploreResolution::Attach => self.decline(),
            ExploreResolution::Instant | ExploreResolution::Token => self.instant(sx, card, system),
        }
    }

    fn set_how(&mut self, how: How) {
        if let Some(open) = &mut self.open {
            open.how = how;
        }
    }

    /// `exploration::resolve_instant` for a card drawn with no planet.
    fn instant(&mut self, sx: &mut Sx<'_>, card: Explore, system: System) {
        let who = self.who;
        match card {
            Explore::MinorEntropicField => self.gain_tokens(sx, 1, 1),
            Explore::EntropicField => self.gain_tokens(sx, 1, 2),
            Explore::MajorEntropicField => self.gain_tokens(sx, 1, 3),
            Explore::KeleresShip1 | Explore::KeleresShip2 => self.gain_tokens(sx, 2, 0),
            Explore::DeadWorld => {
                crate::kernel::fx_relic::relic_gain(sx.state, who);
                self.finish_one(sx);
            }
            Explore::DerelictVessel1 | Explore::DerelictVessel2 => {
                if fx::draw_secret(sx.state, who).is_none() {
                    return self.finish_one(sx);
                }
                self.run_child(sx, &Spawn::HandLimit { who }, EStage::Limiting);
            }
            Explore::EnigmaticDevice1 | Explore::EnigmaticDevice2 => {
                fx_explore::play_area_card(sx.state, who, card);
                self.finish_one(sx);
            }
            Explore::GammaRelay | Explore::GammaWormhole => {
                match sx.state.meta().active_system {
                    Some(here) => {
                        let fresh = sx.state.explore().wormhole_tokens.get(&Wormhole::Gamma) != Some(&here);
                        fx_explore::place_wormhole_token(sx.state, Wormhole::Gamma, here);
                        if let Some(open) = &mut self.open {
                            open.gamma = fresh.then_some(here);
                        }
                    }
                    None => self.set_how(How::Unresolved),
                }
                self.finish_one(sx);
            }
            // "This system" is the token's from the Mirage fix on; the ion storm needs a planet to name its system.
            Explore::Mirage => {
                let placed = self.facts.rules_version >= RULES_MIRAGE_FRONTIER && fx_explore::place_planet(sx.state, who, system, crate::generated::Planet::Mirage);
                if !placed {
                    self.set_how(How::Unresolved);
                }
                self.finish_one(sx);
            }
            Explore::IonStorm => {
                self.set_how(How::Unresolved);
                self.finish_one(sx);
            }
            Explore::LostCrew1 | Explore::LostCrew2 => {
                if !self.facts.draw_plain {
                    return self.decline();
                }
                let request = Spawn::DrawCards { who, count: 2, drop_unofficial: self.facts.rules_version >= RULES_OFFICIAL_ACTION_DECK };
                self.run_child(sx, &request, EStage::Drawing);
            }
            Explore::MerchantStation1 | Explore::MerchantStation2 => {
                let has_commodities = sx.state.players().by_seat(who).is_some_and(|p| p.econ.commodities >= 1);
                let mut options = vec![EOpt::Replenish];
                if self.facts.rules_version < RULES_NO_EMPTY_CONVERT || has_commodities {
                    options.push(EOpt::Convert);
                }
                let request = Spawn::Reward { who, step: EStep::MerchantStation, options: SmallList::from_slice(&options), skip_single: true };
                self.run_child(sx, &request, EStage::Rewarding);
            }
            _ => self.decline(),
        }
    }

    fn run_child(&mut self, sx: &mut Sx<'_>, request: &Spawn, stage: EStage) {
        match sx.table.links.start(request) {
            Some(window) => {
                self.child.start(window);
                self.stage = stage;
            }
            None => self.decline(),
        }
    }

    /// `gain_command_tokens`, then the trade goods the card owes (no announcement: the string explores with no timing).
    fn gain_tokens(&mut self, sx: &mut Sx<'_>, count: u8, goods: i32) {
        if let Some(open) = &mut self.open {
            open.goods = goods;
        }
        if count > 0 && self.facts.mahact_held > 0 {
            return self.decline();
        }
        if count > 0 && self.facts.rules_version >= RULES_COMMAND_TOKEN_POOL {
            self.pool_left = count;
            self.stage = EStage::Pooling;
            return;
        }
        if count > 0 {
            fx::gain_token(sx.state, self.who, crate::choice::strategy::Pool::Strategic, i32::from(count));
        }
        self.finish_goods(sx);
    }

    fn finish_goods(&mut self, sx: &mut Sx<'_>) {
        let goods = self.open.map_or(0, |open| open.goods);
        fx::gain_trade_goods(sx.state, self.who, goods);
        self.finish_one(sx);
    }

    /// The card is resolved: the row is complete and the next token follows.
    fn finish_one(&mut self, sx: &mut Sx<'_>) {
        if let Some(open) = self.open.take() {
            let after = crate::query::explore::holdings(sx.state, self.who, None);
            self.rows.push(FrontierRow { system: open.system, card: open.card, how: open.how, before: open.before, after, gamma: open.gamma });
        }
        self.at += 1;
        self.stage = EStage::Next;
    }

    /// The Merchant Station's choice was made.
    fn merchant_station(&mut self, sx: &mut Sx<'_>, report: Report) {
        let who = self.who;
        if report == Report::Chose(Some(EOpt::Convert)) {
            let moved = fx_explore::spend_commodities(sx.state, who, None);
            fx::gain_trade_goods(sx.state, who, moved);
        } else {
            // Replenish: the Cabal note and the Trade Agreement follow, as `fx::replenish_announced` stages them.
            let Some(agreement) = self.facts.agreement else { return self.decline() };
            fx::replenish_announced(sx.state, who, agreement);
        }
        self.finish_one(sx);
    }
}

impl Window for EmpyreanHero {
    type Stage = EStage;

    fn stage(&self) -> EStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        use crate::kernel::stage::Ran;
        loop {
            match self.stage {
                EStage::Start => {
                    self.snapshot = Some(Box::new(sx.state.clone()));
                    for system in &self.placed {
                        sx.state.explore_mut().frontier.insert(*system);
                    }
                    // The string walks its set of system names: the text order, which only the adapter knows.
                    let who = self.who;
                    let targets: Vec<System> = self
                        .order
                        .iter()
                        .copied()
                        .filter(|system| sx.state.explore().frontier.contains(*system))
                        .filter(|system| sx.state.board().sys(*system).units.as_slice().iter().any(|p| p.owner() == who && p.kind().data().ship))
                        .collect();
                    self.targets = targets;
                    self.stage = EStage::Next;
                }
                EStage::Next => {
                    let Some(system) = self.targets.get(self.at).copied() else {
                        self.snapshot = None;
                        self.used = true;
                        self.stage = EStage::Done;
                        continue;
                    };
                    self.begin_one(sx, system);
                }
                EStage::Pooling => return Next::Continue(Ask::Strategy(SAsk::GainPool { left: self.pool_left })),
                EStage::Rewarding | EStage::Limiting | EStage::Drawing => match self.child.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(report)) => match self.stage {
                        EStage::Rewarding => self.merchant_station(sx, report),
                        _ => {
                            if report == Report::Left {
                                self.decline();
                            } else {
                                self.finish_one(sx);
                            }
                        }
                    },
                    None => self.finish_one(sx),
                },
                EStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            EStage::Pooling => Some(crate::choice::strategy::gain_pool_choice(self.who, self.pool_left)),
            EStage::Rewarding | EStage::Limiting | EStage::Drawing => self.child.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (EStage::Rewarding | EStage::Limiting | EStage::Drawing, pick) => self.child.answer(sx, pick),
            (EStage::Pooling, Pick::Opt(Opt::Strategy(SOpt::Pool(pool)))) => {
                fx::gain_token(sx.state, self.who, pool, 1);
                self.pool_left = self.pool_left.saturating_sub(1);
                if self.pool_left == 0 {
                    self.finish_goods(sx);
                }
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
