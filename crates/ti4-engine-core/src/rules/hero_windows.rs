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

use crate::kernel::children::{ErasedChild, Report, SeeingLimits, Spawn};
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
                        if crate::rules::production::supply::capture_from_board(sx.state, self.who, system, None, piece) {
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
