//! The placement of units from the reinforcements (`action_cards::place_units_choosing`, the planets a seat controls) and the
//! promissory notes that use it, as [`Window`]s (package W14, lent-bodies lane 5): Military Support.
//!
//! The window reads the position and asks the one question; the adapter vouches for what the core does not hold (the form the seat
//! places with its upgrades and faction overrides, a faction rule that forbids placing it, the sources in play). Everything the
//! answer changes happens in `answer`: the units placed and, for a note, what the note does after them.

use crate::choice::generic::Content;
use crate::choice::promissory::{Ask as PAsk, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Planet, PromissoryNote, System, Unit};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next, fx};
use crate::query::{board, fleet, laws, reach};
use crate::state::{Code, CoreState, EventKind, EventRec, Key, NoteKey, Payload, Val};
use crate::tables::Scope;

/// What the window does after the placement (or when there is nothing to place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Nothing more.
    Nothing,
    /// Military Support: "Remove 1 token from the Sol player's strategy pool, if able, and return it to their reinforcements.
    /// Then, ... return this card to the Sol player." The strategy token of `owner` is spent and announced (`reason` is the interned
    /// `military_support`), and the note of `owner`'s faction goes home.
    MilitarySupport { owner: Seat, reason: Code },
    /// ???_ERROR_ERROR_??? (Nekro): "Exhaust this card to place 1 PDS on a planet you control". The planets are the ones under the
    /// structure cap (79.2), the question is the card's own (not optional, no question for one planet), and the card is exhausted once
    /// the PDS stands: a placement that happened nowhere leaves the card ready.
    C4rPds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Start,
    Asking,
    Done,
}

/// "place up to `count` `unit` on any planet `who` controls" (optionally within one system), the units of the placer's
/// reinforcements: `optional` adds the decline of a "may". All the units go to the one planet chosen; one planet that the
/// box lets them all go to is no decision when the placement is not optional. A planet adds no fleet or capacity load, so the
/// pool and capacity limits never cut a placement on a planet. Nothing is asked and nothing is placed when `unit` is `None` (the
/// seat places nothing, or a rule forbids it), when the box holds none, or when no planet qualifies; the `then` clause happens
/// either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceUnits {
    pub who: Seat,
    unit: Option<Unit>,
    count: u8,
    optional: bool,
    within: Option<System>,
    scope: Scope,
    ability: Code,
    then: Then,
    stage: PStage,
    /// How many units arrived.
    pub placed: u8,
}

impl PlaceUnits {
    /// `unit`: the form the seat places (`placed_unit_id`), `None` when it places nothing or a faction rule forbids placing it
    /// (`effect_placement_forbidden`). `ability` is the interned text the question names as its source.
    #[must_use]
    #[allow(clippy::too_many_arguments, reason = "one placement question: who, what, how many, where, optional, which card, what follows")]
    pub const fn new(who: Seat, unit: Option<Unit>, count: u8, within: Option<System>, optional: bool, scope: Scope, ability: Code, then: Then) -> Self {
        Self { who, unit, count, optional, within, scope, ability, then, stage: PStage::Start, placed: 0 }
    }

    /// How many units the box lets the placer put on the board now (`supply::allowed`).
    fn wanted(&self, s: &CoreState) -> u8 {
        let Some(unit) = self.unit else { return 0 };
        let left = fleet::remaining_all(s, self.scope, self.who, unit);
        u8::try_from(left.clamp(0, i64::from(self.count))).unwrap_or(0)
    }

    /// The planets the placer controls, in the order of the string's `placement_spots` (system id, then planet id), not demilitarized.
    fn spots(&self, s: &CoreState) -> SmallList<(System, Planet), 24> {
        let blanked = reach::blanked(s);
        board::controlled_planets(s, self.who)
            .as_slice()
            .iter()
            .copied()
            .filter(|(system, planet)| self.within.is_none_or(|only| only == *system) && !laws::planet_is_demilitarized(s, blanked, *planet))
            .filter(|(_, planet)| !self.capped() || self.under_cap(s, blanked, *planet))
            .collect()
    }

    /// The placement is the card's PDS: the structure cap applies and the question is the card's own.
    const fn capped(&self) -> bool {
        matches!(self.then, Then::C4rPds)
    }

    /// Whether the placed unit's base type is still under the structure cap on `planet` (79.2).
    fn under_cap(&self, s: &CoreState, blanked: bool, planet: Planet) -> bool {
        self.unit.is_some_and(|unit| laws::structure_allowed(s, blanked, self.who, planet, unit.data().base_type))
    }

    /// The question the window waits on.
    fn question(&self, s: &CoreState, unit: Unit) -> PAsk {
        if self.capped() { PAsk::C4rPds } else { PAsk::PlaceUnits { unit, fit: self.wanted(s) } }
    }

    fn ctx(&self) -> Ctx {
        Ctx {
            origin: if self.capped() { Origin::FactionAbility(Content::Technology(crate::generated::Technology::ERRORERROR)) } else { Origin::FactionAbility(Content::Other(self.ability)) },
            subtype: Subtype::Promissory(if self.capped() { PSub::C4rPlanet } else { PSub::PlaceUnitsFromReinforcements }),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    fn follow(&self, s: &mut CoreState) {
        match self.then {
            Then::Nothing => {}
            Then::C4rPds => {
                if self.placed > 0 {
                    crate::kernel::fx_strategy::exhaust_technology(s, self.who, crate::generated::Technology::ERRORERROR);
                }
            }
            Then::MilitarySupport { owner, reason } => {
                if fx::spend_strategic_token(s, owner) {
                    // `supply::note_strategy_token_spent(.., "military_support")`.
                    fx::stage(
                        s,
                        EventRec {
                            id: 0,
                            kind: EventKind::StrategyTokenSpent,
                            payload: Payload::new().with(Key::Player, Val::Seat(owner)).with(Key::Reason, Val::Code(reason)),
                            cancelled: false,
                        },
                    );
                }
                if let Some(faction) = s.players().by_seat(owner).map(|p| p.faction) {
                    fx::give_back_note(s, NoteKey { note: PromissoryNote::MilitarySupport, faction });
                }
            }
        }
    }
}

impl Window for PlaceUnits {
    type Stage = PStage;

    fn stage(&self) -> PStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == PStage::Start {
            let wanted = self.wanted(sx.state);
            let spots = self.spots(sx.state);
            if wanted == 0 || spots.as_slice().is_empty() {
                self.follow(sx.state);
                self.stage = PStage::Done;
            } else if spots.as_slice().len() == 1 && !self.optional {
                let (system, planet) = spots.as_slice()[0];
                self.put(sx.state, system, planet, wanted);
                self.follow(sx.state);
                self.stage = PStage::Done;
            } else {
                self.stage = PStage::Asking;
            }
        }
        match (self.stage, self.unit) {
            (PStage::Asking, Some(unit)) => Next::Continue(Ask::Promissory(self.question(sx.state, unit))),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        let unit = self.unit.filter(|_| self.stage == PStage::Asking)?;
        let mut opts: Opts = self.spots(s).as_slice().iter().map(|(system, planet)| Opt::Promissory(POpt::Spot(*system, *planet))).collect();
        if self.optional {
            opts.push(Opt::Decline);
        }
        Some(Choice { actor: self.who, ask: Ask::Promissory(self.question(s, unit)), ctx: self.ctx(), opts })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != PStage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) if self.optional => {}
            Pick::Opt(Opt::Promissory(POpt::Spot(system, planet))) if self.spots(sx.state).as_slice().contains(&(system, planet)) => {
                let fit = self.wanted(sx.state);
                self.put(sx.state, system, planet, fit);
            }
            _ => return Err(Illegal),
        }
        self.follow(sx.state);
        self.stage = PStage::Done;
        Ok(())
    }
}

impl PlaceUnits {
    fn put(&mut self, s: &mut CoreState, system: System, planet: Planet, count: u8) {
        let Some(unit) = self.unit else { return };
        for _ in 0..count {
            fx::place_unit(s, self.who, system, Some(planet), unit);
        }
        self.placed = count;
    }
}

/// The first copy of each printed card in `seat`'s hand, in the order of the names (`action_cards::first_of_each`): `groups` says which
/// card ids are the same printed card. Empty when the seat is unknown or a card has no group.
fn distinct_cards(core: &CoreState, seat: Seat, groups: &[u8]) -> Vec<crate::generated::ActionCard> {
    let Some(p) = core.players().by_seat(seat) else { return Vec::new() };
    let mut seen: Vec<u8> = Vec::new();
    let mut firsts: Vec<crate::generated::ActionCard> = Vec::new();
    for card in p.cards.hand.as_slice() {
        let Some(group) = groups.get(*card as usize).copied() else { return Vec::new() };
        if !seen.contains(&group) {
            seen.push(group);
            firsts.push(*card);
        }
    }
    firsts.sort_by_key(|card| card.name_rank());
    firsts
}

// -- Stall Tactics ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Start,
    Card,
    Place,
    Done,
}

/// Stall Tactics (Yssaril): "ACTION: Discard 1 action card", then the mech's DEPLOY: "After you use your Stall Tactics faction ability,
/// you may place 1 mech on a planet you control." The seat discards a card of its hand (one option per distinct printed card in the order
/// of the names, no question with one; the discard waits for the game step as a `CardsStaged` mark), then the placement of the mech is
/// the optional [`PlaceUnits`] question (`source` is the interned `stall_tactics`, `ability` the `yssaril_mech` of the placement). `done` is whether the action resolved: the hand held a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StallTactics {
    pub who: Seat,
    source: Code,
    place: PlaceUnits,
    groups: Vec<u8>,
    stage: TStage,
    pub done: bool,
}

impl StallTactics {
    /// `mech`: the form the seat places (`None`: nothing, or a faction rule forbids it); `ability` the interned `yssaril_mech`.
    #[must_use]
    pub fn new(who: Seat, mech: Option<Unit>, scope: Scope, source: Code, ability: Code) -> Self {
        Self { who, source, place: PlaceUnits::new(who, mech, 1, None, true, scope, ability, Then::Nothing), groups: Vec::new(), stage: TStage::Start, done: false }
    }

    fn cards(&self, core: &CoreState) -> Vec<crate::generated::ActionCard> {
        distinct_cards(core, self.who, &self.groups)
    }

    fn discarded(&mut self, sx: &mut Sx<'_>, card: crate::generated::ActionCard) {
        fx::discard_action_card_staged(sx.state, self.who, card);
        self.done = true;
        self.stage = TStage::Place;
    }

    fn place_step(&mut self, sx: &mut Sx<'_>) -> Next {
        let next = self.place.step(sx);
        if matches!(next, Next::Done) {
            self.stage = TStage::Done;
        }
        next
    }
}

impl Window for StallTactics {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == TStage::Start {
            self.groups = sx.table.facts.card_group_table().map(<[u8]>::to_vec).unwrap_or_default();
            let cards = self.cards(sx.state);
            match cards.as_slice() {
                [] => self.stage = TStage::Done,
                [only] => self.discarded(sx, *only),
                _ => self.stage = TStage::Card,
            }
        }
        match self.stage {
            TStage::Card => Next::Continue(Ask::Cards(crate::choice::cards::Ask::OwnHand { reason: crate::choice::cards::Reason::StallTactics })),
            TStage::Place => self.place_step(sx),
            TStage::Start | TStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
        match self.stage {
            TStage::Card => Some(Choice {
                actor: self.who,
                ask: Ask::Cards(CAsk::OwnHand { reason: Reason::StallTactics }),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Other(self.source)),
                    subtype: Subtype::Cards(Reason::StallTactics.subtype()),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: self.cards(s).into_iter().map(|card| Opt::Cards(COpt::Card(card))).collect::<Opts>(),
            }),
            TStage::Place => self.place.ask_of(s, k),
            TStage::Start | TStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        use crate::choice::cards::Opt as COpt;
        match (self.stage, pick) {
            (TStage::Card, Pick::Opt(Opt::Cards(COpt::Card(card)))) if self.cards(sx.state).contains(&card) => {
                self.discarded(sx, card);
                Ok(())
            }
            (TStage::Place, pick) => {
                self.place.answer(sx, pick)?;
                if self.place.stage() == PStage::Done {
                    self.stage = TStage::Done;
                }
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Mageon Implants ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MStage {
    Start,
    Card,
    Limit,
    Done,
}

/// Mageon Implants (Yssaril): "ACTION: Exhaust this card to look at another player's action cards. Choose 1 of those cards and add it to
/// your hand." The card is exhausted, then the seat takes one of the cards of `target`'s hand: one option per distinct printed card in the
/// order of the names, no decline, and no question for one (the look itself is the string's display only). The card moves to the end of the
/// taker's hand and the event waits for the game step (a `CardsStaged` mark); a hand over its limit is the action-card area's window as a
/// child (`limited`: false for a seat whose faction lifts the limit, Yssaril's Crafty). `done` is whether the action resolved: the card was
/// ready, the target another seat holding a card. `unsupported`: the table
/// has no printed-name groups or no link to the hand limit window, the caller declines the run.
#[derive(Debug, Clone)]
pub struct MageonImplants {
    pub who: Seat,
    target: Seat,
    groups: Vec<u8>,
    stage: MStage,
    limited: bool,
    hand: crate::kernel::children::ErasedChild,
    pub done: bool,
    pub unsupported: bool,
}

impl MageonImplants {
    #[must_use]
    pub fn new(who: Seat, target: Seat, limited: bool) -> Self {
        Self { who, target, groups: Vec::new(), stage: MStage::Start, limited, hand: crate::kernel::children::ErasedChild::idle(), done: false, unsupported: false }
    }

    fn ready(&self, core: &CoreState) -> bool {
        self.who != self.target
            && core.players().by_seat(self.who).is_some_and(|p| p.tech.owned.contains(crate::generated::Technology::MageonImplants) && !p.tech.exhausted.contains(crate::generated::Technology::MageonImplants))
            && core.players().by_seat(self.target).is_some_and(|p| !p.cards.hand.as_slice().is_empty())
    }

    fn taken(&mut self, sx: &mut Sx<'_>, card: crate::generated::ActionCard) {
        fx::take_action_card_staged(sx.state, self.who, self.target, card);
        if !self.limited {
            self.stage = MStage::Done;
            return;
        }
        match sx.table.links.start(&crate::kernel::children::Spawn::DrawCards { who: self.who, count: 0, drop_unofficial: false }) {
            Some(window) => {
                self.hand.start(window);
                self.stage = MStage::Limit;
            }
            None => {
                self.unsupported = true;
                self.stage = MStage::Done;
            }
        }
    }
}

impl Window for MageonImplants {
    type Stage = MStage;

    fn stage(&self) -> MStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        use crate::kernel::stage::Ran;
        if self.stage == MStage::Start {
            match sx.table.facts.card_group_table() {
                Some(groups) if self.ready(sx.state) => {
                    self.groups = groups.to_vec();
                    crate::kernel::fx_strategy::exhaust_technology(sx.state, self.who, crate::generated::Technology::MageonImplants);
                    self.done = true;
                    match distinct_cards(sx.state, self.target, &self.groups).as_slice() {
                        [] => {
                            self.unsupported = true;
                            self.stage = MStage::Done;
                        }
                        [only] => self.taken(sx, *only),
                        _ => self.stage = MStage::Card,
                    }
                }
                None => {
                    self.unsupported = true;
                    self.stage = MStage::Done;
                }
                Some(_) => self.stage = MStage::Done,
            }
        }
        if self.stage == MStage::Limit {
            match self.hand.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    self.unsupported |= report == crate::kernel::children::Report::Left;
                    self.stage = MStage::Done;
                }
                None => self.stage = MStage::Done,
            }
        }
        match self.stage {
            MStage::Card => Next::Continue(Ask::Cards(crate::choice::cards::Ask::TakeRevealed { owner: self.target })),
            MStage::Start | MStage::Limit | MStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Subtype as CSub};
        match self.stage {
            MStage::Card => Some(Choice {
                actor: self.who,
                ask: Ask::Cards(CAsk::TakeRevealed { owner: self.target }),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Technology(crate::generated::Technology::MageonImplants)),
                    subtype: Subtype::Cards(CSub::TakeRevealedActionCard),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: distinct_cards(s, self.target, &self.groups).into_iter().map(|card| Opt::Cards(COpt::Card(card))).collect::<Opts>(),
            }),
            MStage::Limit => self.hand.ask_of(s, k),
            MStage::Start | MStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        use crate::choice::cards::Opt as COpt;
        match (self.stage, pick) {
            (MStage::Limit, pick) => self.hand.answer(sx, pick),
            (MStage::Card, Pick::Opt(Opt::Cards(COpt::Card(card)))) if distinct_cards(sx.state, self.target, &self.groups).contains(&card) => {
                self.taken(sx, card);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- ???_ERROR_ERROR_???, the draw -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DStage {
    Start,
    Card,
    Drawing,
    Done,
}

/// ???_ERROR_ERROR_??? (Nekro), the ACTION that draws: "Exhaust this card and discard 1 action card to draw 1 action card." The discard
/// is chosen as for Stall Tactics (one option per distinct printed card, none for one; the discard waits for the game step as a
/// `CardsStaged` mark), the card is exhausted, and the draw is the action-card area's window as a child (its hand-limit question
/// included). `done` is whether the action resolved: the hand held a card. `unsupported`: the table has no link to the draw window, the
/// caller declines the run.
#[derive(Debug, Clone)]
pub struct C4rDraw {
    pub who: Seat,
    drop_unofficial: bool,
    groups: Vec<u8>,
    stage: DStage,
    draw: crate::kernel::children::ErasedChild,
    pub done: bool,
    pub unsupported: bool,
}

impl C4rDraw {
    #[must_use]
    pub fn new(who: Seat, drop_unofficial: bool) -> Self {
        Self { who, drop_unofficial, groups: Vec::new(), stage: DStage::Start, draw: crate::kernel::children::ErasedChild::idle(), done: false, unsupported: false }
    }

    fn cards(&self, core: &CoreState) -> Vec<crate::generated::ActionCard> {
        distinct_cards(core, self.who, &self.groups)
    }

    fn discarded(&mut self, sx: &mut Sx<'_>, card: crate::generated::ActionCard) {
        fx::discard_action_card_staged(sx.state, self.who, card);
        crate::kernel::fx_strategy::exhaust_technology(sx.state, self.who, crate::generated::Technology::ERRORERROR);
        self.done = true;
        match sx.table.links.start(&crate::kernel::children::Spawn::DrawCards { who: self.who, count: 1, drop_unofficial: self.drop_unofficial }) {
            Some(window) => {
                self.draw.start(window);
                self.stage = DStage::Drawing;
            }
            None => {
                self.unsupported = true;
                self.stage = DStage::Done;
            }
        }
    }
}

impl Window for C4rDraw {
    type Stage = DStage;

    fn stage(&self) -> DStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        use crate::kernel::stage::Ran;
        if self.stage == DStage::Start {
            self.groups = sx.table.facts.card_group_table().map(<[u8]>::to_vec).unwrap_or_default();
            let cards = self.cards(sx.state);
            match cards.as_slice() {
                [] => self.stage = DStage::Done,
                [only] => self.discarded(sx, *only),
                _ => self.stage = DStage::Card,
            }
        }
        if self.stage == DStage::Drawing {
            match self.draw.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    self.unsupported |= report == crate::kernel::children::Report::Left;
                    self.stage = DStage::Done;
                }
                None => self.stage = DStage::Done,
            }
        }
        match self.stage {
            DStage::Card => Next::Continue(Ask::Cards(crate::choice::cards::Ask::OwnHand { reason: crate::choice::cards::Reason::C4rDiscard })),
            DStage::Start | DStage::Drawing | DStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
        match self.stage {
            DStage::Card => Some(Choice {
                actor: self.who,
                ask: Ask::Cards(CAsk::OwnHand { reason: Reason::C4rDiscard }),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Technology(crate::generated::Technology::ERRORERROR)),
                    subtype: Subtype::Cards(Reason::C4rDiscard.subtype()),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: self.cards(s).into_iter().map(|card| Opt::Cards(COpt::Card(card))).collect::<Opts>(),
            }),
            DStage::Drawing => self.draw.ask_of(s, k),
            DStage::Start | DStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        use crate::choice::cards::Opt as COpt;
        match (self.stage, pick) {
            (DStage::Drawing, pick) => self.draw.answer(sx, pick),
            (DStage::Card, Pick::Opt(Opt::Cards(COpt::Card(card)))) if self.cards(sx.state).contains(&card) => {
                self.discarded(sx, card);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::{Attachment, Faction, UnitBaseType};
    use crate::kernel::Resolver;
    use crate::kernel::stage::AbilityTable;
    use crate::state::{Player, Players};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..3u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s.seats = crate::state::Seats::new(&[Seat(0), Seat(1), Seat(2)], &[Seat(0), Seat(1), Seat(2)], 3);
        s
    }

    fn driver(w: PlaceUnits) -> Driver<PlaceUnits> {
        Driver::new(w, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    fn unit_of(base: UnitBaseType) -> Unit {
        Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none()).expect("a plain unit")
    }

    /// Three planets in three systems, controlled by seat 0 (the last by seat 1).
    fn planets() -> [(System, Planet); 3] {
        let mut found: Vec<(System, Planet)> = Planet::ALL.iter().copied().filter_map(|p| p.data().system.map(|s| (s, p))).step_by(40).take(3).collect();
        found.sort_unstable();
        [found[0], found[1], found[2]]
    }

    fn control(s: &mut CoreState, owned: usize) {
        for (i, (_, planet)) in planets().iter().enumerate() {
            s.board_mut().planet_mut(*planet).control = Some(if i < owned { Seat(0) } else { Seat(1) });
        }
    }

    fn count(s: &CoreState, planet: Planet, unit: Unit) -> usize {
        s.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind() == unit).count()
    }

    fn window(unit: Unit, optional: bool, then: Then) -> PlaceUnits {
        PlaceUnits::new(Seat(0), Some(unit), 2, None, optional, Scope(u32::MAX), Code(0), then)
    }

    #[test]
    fn a_lone_planet_takes_the_units_without_a_question_unless_the_placement_is_optional() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let mut s = table();
        control(&mut s, 1);
        let mut d = driver(window(infantry, false, Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), d.window.placed), (2, 2));
        let mut s = table();
        control(&mut s, 1);
        let mut d = driver(window(infantry, true, Then::Nothing));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("a \"may\" is asked with one spot") };
        assert_eq!(c.opts.as_slice(), &[Opt::Promissory(POpt::Spot(planets()[0].0, planets()[0].1)), Opt::Decline]);
        assert_eq!(count(&s, planets()[0].1, infantry), 0, "nothing before the answer");
    }

    #[test]
    fn several_planets_are_offered_in_system_order_and_only_a_listed_one_is_taken() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let mut s = table();
        control(&mut s, 2);
        let mut d = driver(window(infantry, false, Then::Nothing));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.ask, Ask::Promissory(PAsk::PlaceUnits { unit: infantry, fit: 2 }));
        assert_eq!(c.opts.as_slice(), &[Opt::Promissory(POpt::Spot(planets()[0].0, planets()[0].1)), Opt::Promissory(POpt::Spot(planets()[1].0, planets()[1].1))], "no decline when it is not a may");
        // The planet of the other seat, a decline of a placement that is not optional, and a planet in the wrong system.
        for bad in [Pick::Opt(Opt::Promissory(POpt::Spot(planets()[2].0, planets()[2].1))), Pick::Opt(Opt::Decline), Pick::Opt(Opt::Promissory(POpt::Spot(planets()[1].0, planets()[0].1)))] {
            assert_eq!(d.resume(&mut s, &k, bad), Err(DriverError::Illegal), "{bad:?}");
        }
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[1])).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), count(&s, planets()[1].1, infantry)), (0, 2));
    }

    #[test]
    fn a_decline_places_nothing_and_a_system_limit_hides_the_other_systems() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let mut s = table();
        control(&mut s, 2);
        let mut d = driver(window(infantry, true, Then::Nothing));
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), d.window.placed), (0, 0));
        // Within one system: only its planet, and (not optional) no question at all.
        let mut s = table();
        control(&mut s, 2);
        let mut within = PlaceUnits::new(Seat(0), Some(infantry), 2, Some(planets()[1].0), false, Scope(u32::MAX), Code(0), Then::Nothing);
        within.placed = 0;
        let mut d = driver(within);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), count(&s, planets()[1].1, infantry)), (0, 2));
    }

    #[test]
    fn nothing_is_asked_when_the_box_is_empty_when_the_unit_is_unavailable_or_the_planet_is_demilitarized() {
        let k = Kernel::plain();
        let mech = unit_of(UnitBaseType::Mech);
        // The box holds four mechs; all four stand on the board.
        let mut s = table();
        control(&mut s, 2);
        for _ in 0..4 {
            fx::place_unit(&mut s, Seat(0), planets()[0].0, Some(planets()[0].1), mech);
        }
        let mut d = driver(window(mech, true, Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(d.window.placed, 0);
        // One mech left in the box: one is placed of the two asked for, and the question says so.
        let mut s = table();
        control(&mut s, 2);
        for _ in 0..3 {
            fx::place_unit(&mut s, Seat(0), planets()[0].0, Some(planets()[0].1), mech);
        }
        let mut d = driver(window(mech, true, Then::Nothing));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.ask, Ask::Promissory(PAsk::PlaceUnits { unit: mech, fit: 1 }));
        d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[1])).unwrap();
        assert_eq!((count(&s, planets()[1].1, mech), d.window.placed), (1, 1));
        // No unit form (the seat places nothing, or a faction rule forbids it).
        let mut s = table();
        control(&mut s, 2);
        let mut d = driver(PlaceUnits::new(Seat(0), None, 2, None, true, Scope(u32::MAX), Code(0), Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        // A demilitarized planet is not offered (the exploration attachment, at the rules version that has it).
        let infantry = unit_of(UnitBaseType::Infantry);
        let mut s = table();
        control(&mut s, 2);
        s.meta_mut().rules_version = crate::query::laws::RULES_DMZ_EXPLORATION;
        s.board_mut().planet_mut(planets()[0].1).attach.push(Attachment::DemilitarizedZone);
        let mut d = driver(window(infantry, false, Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done, "one planet is left: no question");
        assert_eq!((count(&s, planets()[0].1, infantry), count(&s, planets()[1].1, infantry)), (0, 2));
    }

    #[test]
    fn military_support_spends_the_token_of_its_owner_announces_it_and_sends_the_note_home_whatever_the_answer() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let note = NoteKey { note: PromissoryNote::MilitarySupport, faction: Faction::from_index(1).unwrap() };
        for decline in [false, true] {
            let mut s = table();
            control(&mut s, 2);
            s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = 2;
            s.cards_mut().promissory.edit().insert(note, Seat(0));
            let then = Then::MilitarySupport { owner: Seat(1), reason: Code(7) };
            let mut d = driver(PlaceUnits::new(Seat(0), Some(infantry), 2, None, true, Scope(u32::MAX), Code(0), then));
            let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
            assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 2, "nothing before the answer");
            let answer = if decline { Opt::Decline } else { c.opts.as_slice()[0] };
            assert_eq!(d.resume(&mut s, &k, Pick::Opt(answer)).unwrap(), Status::Done);
            assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 1, "the owner pays, not the holder");
            assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 0);
            assert_eq!(s.cards().promissory.get(&note), Some(&Seat(1)), "the note is home");
            assert_eq!(s.pending().staged.len(), 1, "the spent token is announced");
            let announced = &s.pending().staged[0];
            assert_eq!(announced.kind, EventKind::StrategyTokenSpent);
            assert_eq!(announced.payload, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Reason, Val::Code(Code(7))), "the owner, and the reason the adapter named");
            assert_eq!(count(&s, planets()[0].1, infantry), if decline { 0 } else { 2 });
        }
        // Nothing to place and no token: the note still goes home and nothing is announced.
        let mut s = table();
        s.cards_mut().promissory.edit().insert(note, Seat(0));
        let mut d = driver(PlaceUnits::new(Seat(0), Some(infantry), 2, None, true, Scope(u32::MAX), Code(0), Then::MilitarySupport { owner: Seat(1), reason: Code(0) }));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(s.cards().promissory.get(&note), Some(&Seat(1)));
        assert!(s.pending().staged.is_empty());
    }

    fn stall_driver(unit: Option<Unit>) -> Driver<StallTactics> {
        let mut t = AbilityTable::default();
        t.facts.card_groups = (0..crate::generated::ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
        Driver::new(StallTactics::new(Seat(0), unit, Scope(u32::MAX), Code(1), Code(2)), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), t, 1)
    }

    #[test]
    fn stall_tactics_discards_the_chosen_card_staged_for_the_game_step_then_asks_for_the_mech_as_a_may() {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
        use crate::generated::ActionCard as C;
        use crate::keys::MarkKey;
        let (k, mech) = (Kernel::plain(), unit_of(UnitBaseType::Mech));
        let mut s = table();
        control(&mut s, 2);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(C::ALL[7]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(C::ALL[3]);
        let mut d = stall_driver(Some(mech));
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks which card") };
        assert_eq!(q.ask, Ask::Cards(CAsk::OwnHand { reason: Reason::StallTactics }));
        let mut ranked = vec![C::ALL[7], C::ALL[3]];
        ranked.sort_by_key(|c| c.name_rank());
        let opts: Vec<Opt> = ranked.iter().map(|c| Opt::Cards(COpt::Card(*c))).collect();
        assert_eq!(q.opts.as_slice(), opts.as_slice(), "no decline: the discard is the action");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Cards(COpt::Card(C::ALL[9])))), Err(DriverError::Illegal));
        let Status::Asking(place) = d.resume(&mut s, &k, Pick::Opt(opts[0])).unwrap() else { panic!("the mech may be placed") };
        assert!(d.window.done, "the action resolved with the discard");
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 1);
        assert!(s.marks().tail.get(&MarkKey::CardsStaged(0)).is_some(), "the discard waits for the game step");
        assert_eq!(place.ask, Ask::Promissory(PAsk::PlaceUnits { unit: mech, fit: 1 }));
        assert_eq!(place.opts.as_slice().last(), Some(&Opt::Decline));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!(count(&s, planets()[0].1, mech) + count(&s, planets()[1].1, mech), 0);
    }

    #[test]
    fn stall_tactics_takes_a_lone_card_without_a_question_places_the_mech_and_needs_a_card() {
        use crate::generated::ActionCard as C;
        let (k, mech) = (Kernel::plain(), unit_of(UnitBaseType::Mech));
        let mut s = table();
        control(&mut s, 2);
        for _ in 0..2 {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(C::ALL[3]);
        }
        let mut d = stall_driver(Some(mech));
        let Status::Asking(place) = d.run(&mut s, &k).unwrap() else { panic!("two copies of one printed card are one option: straight to the mech") };
        d.resume(&mut s, &k, Pick::Opt(place.opts.as_slice()[1])).unwrap();
        assert_eq!(count(&s, planets()[1].1, mech), 1);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 1);
        // An empty hand: the action does not resolve and nothing is asked.
        let mut s = table();
        control(&mut s, 2);
        let mut d = stall_driver(Some(mech));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        // A card but no mech to place (or no planet): the discard still resolves.
        let mut s = table();
        control(&mut s, 2);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(C::ALL[3]);
        let mut d = stall_driver(None);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.done);
    }

    fn draw_driver(links: bool) -> Driver<C4rDraw> {
        let mut t = AbilityTable::default();
        t.facts.card_groups = (0..crate::generated::ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
        if links {
            crate::driver::children::register(&mut t);
        }
        Driver::new(C4rDraw::new(Seat(0), false), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), t, 1)
    }

    #[test]
    fn the_nekro_draw_discards_exhausts_and_draws_one_card_and_needs_a_card_in_hand() {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason};
        use crate::generated::{ActionCard as C, Technology};
        let k = Kernel::plain();
        let setup = |hand: &[C]| {
            let mut s = table();
            s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::ERRORERROR);
            for card in hand {
                s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(*card);
            }
            s.decks_mut().action.set(vec![C::ALL[11], C::ALL[12]]);
            s
        };
        // One distinct card: no question, the card goes, the top of the deck comes.
        let mut s = setup(&[C::ALL[3]]);
        let mut d = draw_driver(true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.done && !d.window.unsupported);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[C::ALL[11]]);
        assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::ERRORERROR));
        assert!(s.marks().tail.get(&crate::keys::MarkKey::CardsStaged(0)).is_some());
        // Several: the question has no decline and names the reason; nothing happens before the answer.
        let mut s = setup(&[C::ALL[3], C::ALL[7]]);
        let mut d = draw_driver(true);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.ask, Ask::Cards(CAsk::OwnHand { reason: Reason::C4rDiscard }));
        assert!(!q.opts.as_slice().contains(&Opt::Decline));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2);
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Cards(COpt::Card(C::ALL[9])))), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(q.opts.as_slice()[0])).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 2, "one out, one in");
        // No card: nothing resolves. No link to the draw window: the run is declined.
        let mut s = setup(&[]);
        let mut d = draw_driver(true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        let mut s = setup(&[C::ALL[3]]);
        let mut d = draw_driver(false);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.unsupported);
    }

    #[test]
    fn the_c4r_pds_is_asked_under_the_structure_cap_and_exhausts_the_card_only_when_the_unit_stands() {
        use crate::generated::Technology;
        let (k, pds) = (Kernel::plain(), unit_of(UnitBaseType::Pds));
        let setup = |on_first: usize| {
            let mut s = table();
            control(&mut s, 2);
            s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::ERRORERROR);
            for _ in 0..on_first {
                fx::place_unit(&mut s, Seat(0), planets()[0].0, Some(planets()[0].1), pds);
            }
            s
        };
        let exhausted = |s: &CoreState| s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::ERRORERROR);
        let w = || PlaceUnits::new(Seat(0), Some(pds), 1, None, false, Scope(u32::MAX), Code(0), Then::C4rPds);
        // Both planets are under the cap: the card's own question, no decline, the context names the card and the subtype.
        let mut s = setup(1);
        let mut d = driver(w());
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.ask, Ask::Promissory(PAsk::C4rPds));
        assert_eq!((c.ctx.origin, c.ctx.subtype), (Origin::FactionAbility(Content::Technology(Technology::ERRORERROR)), Subtype::Promissory(PSub::C4rPlanet)));
        assert_eq!(c.opts.as_slice(), &[Opt::Promissory(POpt::Spot(planets()[0].0, planets()[0].1)), Opt::Promissory(POpt::Spot(planets()[1].0, planets()[1].1))]);
        assert!(!exhausted(&s), "nothing before the answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[0])).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, pds), d.window.placed, exhausted(&s)), (2, 1, true));
        // The first planet is at the cap of two: it is not offered, the lone planet takes the PDS without a question.
        let mut s = setup(2);
        let mut d = driver(w());
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, pds), count(&s, planets()[1].1, pds), exhausted(&s)), (2, 1, true));
        // Homeland Defense Act lifts the cap: the first planet is offered again.
        let mut s = setup(2);
        s.agenda_mut().laws.insert(crate::generated::Agenda::HomelandDefenseAct, crate::keys::Outcome::For);
        let mut d = driver(w());
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.opts.as_slice().len(), 2);
        // Nothing stands (the cap on every planet, or the box empty): the card stays ready.
        let mut s = setup(2);
        for _ in 0..2 {
            fx::place_unit(&mut s, Seat(0), planets()[1].0, Some(planets()[1].1), pds);
        }
        let mut d = driver(w());
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((d.window.placed, exhausted(&s)), (0, false));
    }

    fn mageon_driver(limited: bool, links: bool, groups: bool) -> Driver<MageonImplants> {
        let mut t = AbilityTable::default();
        if groups {
            t.facts.card_groups = (0..crate::generated::ActionCard::COUNT).map(|i| u8::try_from(i).unwrap_or(u8::MAX)).collect();
        }
        if links {
            crate::driver::children::register(&mut t);
        }
        Driver::new(MageonImplants::new(Seat(0), Seat(1), limited), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), t, 1)
    }

    fn mageon_table(mine: &[crate::generated::ActionCard], theirs: &[crate::generated::ActionCard]) -> CoreState {
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(crate::generated::Technology::MageonImplants);
        for card in mine {
            s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(*card);
        }
        for card in theirs {
            s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.hand.push(*card);
        }
        s
    }

    #[test]
    fn mageon_implants_takes_the_chosen_card_to_the_end_of_the_hand_stages_the_event_and_exhausts_the_card() {
        use crate::choice::cards::{Ask as CAsk, Opt as COpt, Subtype as CSub};
        use crate::generated::{ActionCard as C, Technology};
        let k = Kernel::plain();
        let exhausted = |s: &CoreState| s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::MageonImplants);
        let hand = |s: &CoreState, seat: u8| s.players().by_seat(Seat(seat)).unwrap().cards.hand.as_slice().to_vec();
        // Two copies of one card and another one: two options, in the order of the names, no decline; the question names the card.
        let mut s = mageon_table(&[C::ALL[9]], &[C::ALL[7], C::ALL[3], C::ALL[7]]);
        let mut d = mageon_driver(true, true, true);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.ask, Ask::Cards(CAsk::TakeRevealed { owner: Seat(1) }));
        assert_eq!((q.actor, q.ctx.origin, q.ctx.subtype), (Seat(0), Origin::FactionAbility(Content::Technology(Technology::MageonImplants)), Subtype::Cards(CSub::TakeRevealedActionCard)));
        let mut names = [C::ALL[3], C::ALL[7]];
        names.sort_by_key(|c| c.name_rank());
        assert_eq!(q.opts.as_slice(), &names.map(|c| Opt::Cards(COpt::Card(c))));
        assert!(exhausted(&s), "the card is exhausted before the question");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Cards(COpt::Card(C::ALL[9])))), Err(DriverError::Illegal), "a card of the taker's own hand");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Cards(COpt::Card(C::ALL[7])))).unwrap(), Status::Done);
        assert!(d.window.done && !d.window.unsupported);
        assert_eq!(hand(&s, 0), [C::ALL[9], C::ALL[7]], "at the end of the hand");
        assert_eq!(hand(&s, 1), [C::ALL[3], C::ALL[7]], "the first copy went");
        use crate::keys::{Atom, MarkKey, MarkVal, Word};
        let row = MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(Word::Taken)), MarkVal::Atom(Atom::Seat(Seat(0))), MarkVal::Atom(Atom::Seat(Seat(1))), MarkVal::Atom(Atom::Card(C::ALL[7]))]);
        assert_eq!(s.marks().tail.get(&MarkKey::CardsStaged(0)), Some(&row));
        // One printed card (two copies): no question.
        let mut s = mageon_table(&[], &[C::ALL[7], C::ALL[7]]);
        let mut d = mageon_driver(true, true, true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((hand(&s, 0), hand(&s, 1), exhausted(&s)), (vec![C::ALL[7]], vec![C::ALL[7]], true));
    }

    #[test]
    fn mageon_implants_needs_a_ready_card_another_seat_and_a_card_to_take_and_writes_nothing_otherwise() {
        use crate::generated::{ActionCard as C, Technology};
        let k = Kernel::plain();
        let refused = |mut s: CoreState| {
            let before = s.clone();
            let mut d = mageon_driver(true, true, true);
            assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
            assert!(!d.window.done && !d.window.unsupported);
            assert_eq!(s, before, "nothing written");
        };
        let mut no_card = mageon_table(&[], &[C::ALL[7]]);
        no_card.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.remove(Technology::MageonImplants);
        refused(no_card);
        let mut spent = mageon_table(&[], &[C::ALL[7]]);
        spent.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(Technology::MageonImplants);
        refused(spent);
        refused(mageon_table(&[], &[]));
        // The target is the taker itself.
        let mut s = mageon_table(&[C::ALL[7]], &[]);
        let before = s.clone();
        let mut d = Driver::new(MageonImplants::new(Seat(0), Seat(0), true), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(s, before);
    }

    #[test]
    fn mageon_implants_follows_the_take_with_the_hand_limit_unless_the_faction_lifts_it_and_declines_a_table_without_links() {
        use crate::choice::cards::Ask as CAsk;
        use crate::generated::ActionCard as C;
        let k = Kernel::plain();
        let seven: Vec<C> = (10..17).map(|i| C::ALL[i]).collect();
        // Over the limit by the taken card: the discard question of the draw window follows.
        let mut s = mageon_table(&seven, &[C::ALL[7]]);
        let mut d = mageon_driver(true, true, true);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.ask, Ask::Cards(CAsk::HandLimit { held: 8 }));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(q.opts.as_slice()[0])).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 7);
        // Crafty: no limit, no question.
        let mut s = mageon_table(&seven, &[C::ALL[7]]);
        let mut d = mageon_driver(false, true, true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 8);
        // No link to the draw window, or no printed-name groups: the run is declined.
        let mut s = mageon_table(&seven, &[C::ALL[7]]);
        let mut d = mageon_driver(true, false, true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.unsupported);
        let mut s = mageon_table(&[], &[C::ALL[7]]);
        let mut d = mageon_driver(true, true, false);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.unsupported);
    }
}
