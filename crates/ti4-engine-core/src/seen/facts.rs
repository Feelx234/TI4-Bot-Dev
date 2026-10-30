//! The position's headline facts and the seats: `Observed::{round, phase, active_player, speaker,
//! initiative_order, players, custodians_removed, seat, card_initiative, strategy_card_goods}`.

use super::Seen;
use crate::containers::Set;
use crate::generated::{Breakthrough, Explore, Faction, Relic, StrategyCard};
use crate::ids::Seat;
use crate::query::seats::{self, SeatOrder};
use crate::state::{FragmentCounts, MetaFlags, Phase, PlayerLeaders, TechSet};

/// A seat's public standing: what anybody at the table can count (`choice::PublicSeat`). Hand sizes, never
/// contents.
#[derive(Debug, Clone, Copy)]
pub struct SeatView<'a> {
    pub seat: Seat,
    pub faction: Faction,
    pub victory_points: i32,
    pub trade_goods: i32,
    pub commodities: i32,
    pub tactic_tokens: i32,
    /// The fleet pool, including the foreign tokens in the Mahact's pool (Edict), as the table shows it.
    pub fleet_tokens: i32,
    pub strategic_tokens: i32,
    pub strategy_cards: &'a [StrategyCard],
    pub exhausted_strategy_cards: Set<StrategyCard, 1>,
    pub technologies: TechSet,
    pub exhausted_technologies: TechSet,
    pub action_cards_held: usize,
    pub secret_objectives_held: usize,
    pub passed: bool,
    pub relics: &'a [Relic],
    pub exhausted_relics: Set<Relic, 1>,
    pub exploration_cards: &'a [Explore],
    pub relic_fragments: FragmentCounts,
    pub breakthrough: Option<Breakthrough>,
    pub leaders: &'a PlayerLeaders,
}

impl Seen<'_> {
    /// The round number (`Observed::round`).
    #[must_use]
    pub fn round(&self) -> u32 {
        self.st().meta().round
    }

    /// Which phase the round is in (`Observed::phase`).
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.st().meta().phase
    }

    /// The seat whose action or turn is resolving (`Observed::active_player`).
    #[must_use]
    pub fn active(&self) -> Option<Seat> {
        self.st().meta().active
    }

    /// The speaker (`Observed::speaker`).
    #[must_use]
    pub fn speaker(&self) -> Seat {
        self.st().meta().speaker
    }

    /// Whether Mecatol's custodians token has been removed (`Observed::custodians_removed`).
    #[must_use]
    pub fn custodians_removed(&self) -> bool {
        self.st().meta().flags.has(MetaFlags::CUSTODIANS_REMOVED)
    }

    /// The players, in the order of the game's player list (`Observed::players`).
    #[must_use]
    pub fn players(&self) -> &[Seat] {
        self.st().seats().players()
    }

    /// Players in the public strategy-card initiative order (`Observed::initiative_order`).
    #[must_use]
    pub fn initiative_order(&self) -> SeatOrder {
        seats::initiative_order(self.st())
    }

    /// The seating order, clockwise (`GameState.seating_order`).
    #[must_use]
    pub fn seating_order(&self) -> SeatOrder {
        seats::seating_order(self.st())
    }

    /// A seat's public standing (`Observed::seat`); `None` for an owner that is not a player.
    #[must_use]
    pub fn seat(&self, seat: Seat) -> Option<SeatView<'_>> {
        let p = self.st().players().by_seat(seat)?;
        let foreign = self.kernel().hooks.direct.mahact_foreign_tokens(self.st(), seat);
        Some(SeatView {
            seat,
            faction: p.faction,
            victory_points: i32::from(p.econ.victory_points),
            trade_goods: i32::from(p.econ.trade_goods),
            commodities: i32::from(p.econ.commodities),
            tactic_tokens: i32::from(p.econ.tactic_tokens),
            fleet_tokens: i32::from(p.econ.fleet_tokens) + foreign,
            strategic_tokens: i32::from(p.econ.strategic_tokens),
            strategy_cards: p.strat.as_slice(),
            exhausted_strategy_cards: p.strat_exhausted,
            technologies: p.tech.owned,
            exhausted_technologies: p.tech.exhausted,
            action_cards_held: p.cards.hand.len(),
            secret_objectives_held: p.cards.secrets.len(),
            passed: p.econ.passed,
            relics: p.cards.relics.as_slice(),
            exhausted_relics: p.cards.relics_exhausted,
            exploration_cards: p.cards.explore.as_slice(),
            relic_fragments: p.cards.fragments,
            breakthrough: p.cards.breakthrough,
            leaders: &p.leaders,
        })
    }

    /// The initiative number printed on a strategy card (`Observed::card_initiative`).
    #[must_use]
    pub fn card_initiative(&self, card: StrategyCard) -> Option<i32> {
        self.st().strategy().initiative.get(&card).copied()
    }

    /// Trade goods sitting on a strategy card (`Observed::strategy_card_goods`), 0 for none.
    #[must_use]
    pub fn strategy_card_goods(&self, card: StrategyCard) -> i32 {
        self.st().strategy().goods.get(&card).copied().unwrap_or(0)
    }
}
