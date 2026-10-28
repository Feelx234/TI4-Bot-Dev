//! The strategy card effects that ask (package B8; string `strategy_cards`), as [`Window`]s: so far the
//! Trade primary (`trade_primary`).
//!
//! Each is the string function split at its questions: `step` runs to the next question, `answer` is the only
//! place an answer changes the position. What the window cannot do stays with the string callers (the
//! adapter bails): the `TRADE_GOODS_GAINED` event of the three trade goods (staged by the caller after the
//! window), the Siphon / Cabal / Trade Agreement reactions to a replenish, the diplomacy promise a favour
//! keeps, a question asked inside a battle or an invasion.

use crate::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::Next;
use crate::query::seats;
use crate::rules::strategy_flow::replenish;
use crate::state::CoreState;

/// The context of a strategy card question.
fn ctx(card: StrategyCard, secondary: bool, subtype: SSub) -> Ctx {
    Ctx {
        origin: Origin::StrategyCard { card, secondary },
        subtype: Subtype::Strategy(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

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
    stage: TStage,
    /// The seats not yet asked about, in seating order.
    remaining: SmallList<Seat, 16>,
}

impl TradePrimary {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: TStage::Start, remaining: SmallList::new() }
    }

    const ASK: Ask = Ask::Strategy(SAsk::TradeReplenish);

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
            fx::gain_trade_goods(sx.state, self.who, TRADE_PRIMARY_GOODS);
            replenish(sx.state, self.who);
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
                replenish(sx.state, seat);
                self.remaining.retain(|s| *s != seat);
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}
