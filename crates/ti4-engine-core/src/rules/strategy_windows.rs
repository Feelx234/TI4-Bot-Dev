//! The strategy card effects that ask (package B8; string `strategy_cards`), as [`Window`]s: so far the
//! Trade primary (`trade_primary`).
//!
//! Each is the string function split at its questions: `step` runs to the next question, `answer` is the only
//! place an answer changes the position. What the window cannot do stays with the string callers (the
//! adapter bails): the Siphon reaction to a replenish (an Obsidian plot), the diplomacy promise a favour
//! keeps, a question asked inside a battle or an invasion. The `TRADE_GOODS_GAINED` event of the three trade goods and
//! what a replenish announces (Cabal's `COMMODITIES_REPLENISHED`, the Trade Agreement) are staged by the window
//! (`fx::replenish_announced`; package W10), for whoever flushes the staged events.

use crate::choice::strategy::{ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::containers::SmallList;
use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::Next;
use crate::query::seats;
use crate::state::{Code, CoreState};

/// The trade goods the Trade primary gives.
pub const TRADE_PRIMARY_GOODS: i32 = 3;

/// The suspension points of [`TradePrimary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TStage {
    Start,
    /// Waiting on "let another player replenish commodities".
    Pick,
    Done,
}

/// "Gain 3 trade goods and replenish your commodities; choose any number of other players: they replenish
/// their commodities."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradePrimary {
    pub who: Seat,
    /// The interned sources of the two staged `TRADE_GOODS_GAINED` events (`"trade_primary"`, `"trade_agreement"`), when the
    /// window announces its gains itself ([`Self::with_sources`]); `None`: the caller announces the three trade goods (the
    /// action cards' child) and the table is one a replenish announces nothing at.
    pub sources: Option<(Code, Code)>,
    stage: TStage,
    /// The seats not yet asked about, in seating order.
    remaining: SmallList<Seat, 16>,
}

impl TradePrimary {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, sources: None, stage: TStage::Start, remaining: SmallList::new() }
    }

    /// [`Self::new`] with the interned texts of the staged events' sources (`"trade_primary"`, `"trade_agreement"`).
    #[must_use]
    pub const fn with_sources(who: Seat, primary: Code, agreement: Code) -> Self {
        Self { who, sources: Some((primary, agreement)), stage: TStage::Start, remaining: SmallList::new() }
    }

    const ASK: Ask = Ask::Strategy(SAsk::TradeReplenish);

    fn gain_and_replenish(&self, s: &mut CoreState, agreement: Code) {
        match self.sources {
            Some((primary, _)) => {
                fx::gain_trade_goods_staged(s, self.who, TRADE_PRIMARY_GOODS, primary);
            }
            None => {
                fx::gain_trade_goods(s, self.who, TRADE_PRIMARY_GOODS);
            }
        }
        self.replenish(s, self.who, agreement);
    }

    /// A replenish announces what a table listens to (Cabal's `COMMODITIES_REPLENISHED`, a Trade Agreement): through `fx::replenish_announced`
    /// with the interned text of the agreement's source (`agreement`, the window's own when it has one). Two seats of one faction (the announcement
    /// is keyed by faction) replenish plainly.
    fn replenish(&self, s: &mut CoreState, seat: Seat, agreement: Code) {
        let agreement = self.sources.map_or(agreement, |(_, agreement)| agreement);
        if fx::replenish_announced(s, seat, agreement).is_none() && self.sources.is_none() {
            fx::replenish_commodities(s, seat);
        }
    }

    /// Seats below their printed commodity value (the string `_replenishable`).
    fn needs(core: &CoreState, seat: Seat) -> bool {
        core.players().by_seat(seat).is_some_and(|p| p.econ.commodities < i16::from(p.faction.data().commodities))
    }
}

impl Window for TradePrimary {
    type Stage = TStage;

    fn stage(&self) -> TStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == TStage::Start {
            self.gain_and_replenish(sx.state, sx.table.codes.not_from_supply[3]);
            self.remaining = seats::seating_order(sx.state).as_slice().iter().copied().filter(|s| *s != self.who).collect();
            self.stage = TStage::Pick;
        }
        if self.stage == TStage::Pick {
            let core: &CoreState = sx.state;
            self.remaining.retain(|s| Self::needs(core, *s));
            if self.remaining.is_empty() {
                self.stage = TStage::Done;
            }
        }
        match self.stage {
            TStage::Pick => Next::Continue(Self::ASK),
            TStage::Start | TStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &crate::kernel::Kernel) -> Option<Choice> {
        (self.stage == TStage::Pick).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: ctx(StrategyCard::Trade, false, SSub::TradeReplenish),
            opts: self
                .remaining
                .as_slice()
                .iter()
                .map(|s| Opt::Strategy(SOpt::Replenish(*s)))
                .chain(std::iter::once(Opt::Done))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != TStage::Pick {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Done) => self.stage = TStage::Done,
            Pick::Opt(Opt::Strategy(SOpt::Replenish(seat))) if self.remaining.as_slice().contains(&seat) => {
                self.replenish(sx.state, seat, sx.table.codes.not_from_supply[3]);
                self.remaining.retain(|s| *s != seat);
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}
