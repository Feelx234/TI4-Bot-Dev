//! The ability of a strategy card performed by a window that is not the card's own play: Strategize and Overrule (action cards), Hesh and
//! Prit (Naaz hero) and Mathis Mathinus (Winnu hero). The body of `rules::action_cards::perform` (package B6d), lifted to the kernel so that a
//! second area can use it; areas do not import each other, so the abilities stay the windows of the strategy areas and are named by a
//! [`Spawn`] request that the table's links start.
//!
//! Carried: the secondaries of Trade (the replenishment), Imperial (the draw of a secret objective and the hand limit), Politics (the draw of
//! two action cards), Technology (the paid research), Leadership, Construction and Diplomacy, and the primaries of Trade, Politics, Technology,
//! Leadership, Construction and Warfare, and Diplomacy. Not carried ([`Began::Unsupported`]: the adapter declines the run and the string
//! performs the ability):
//! * the Warfare secondary (`home_production`: a whole production window at the home system, with the hero offers, the Keleres agent window
//!   and the use counters of `production::resolve_timed` around it);
//! * the Imperial primary (the scoreable objectives, the award tail and the winner are the string's);
//! * the Thunder's Edge Construction and Warfare primaries (the choice of a production, the free tactical action).

use crate::containers::SmallList;
use crate::generated::{StrategyCard, StrategyCardKind, System};
use crate::ids::Seat;
use crate::kernel::children::{Report, Spawn, StructureFacts};
use crate::kernel::fx;
use crate::kernel::stage::Sx;
use crate::query::tech::UnitRoster;
use crate::state::{Code, CoreState};
use crate::tables::Scope;

/// `TECHNOLOGY_PRIMARY_SECOND_COST` and `TECHNOLOGY_SECONDARY_COST` (`strategy_cards`): the resources of the paid research.
const TECHNOLOGY_PRIMARY_SECOND_COST: i64 = 6;
const TECHNOLOGY_SECONDARY_COST: i64 = 4;
/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`: from this version the unofficial action cards leave the deck before a draw.
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;
/// `ti4_model::state::RULES_LETNEV_FLEET_POOL`: from this version the Letnev fleet pool may be emptied.
const RULES_LETNEV_FLEET_POOL: u32 = 4;
/// `ti4_model::state::RULES_BASE_DIPLOMACY`: from this version the base printing of Diplomacy readies a whole system.
const RULES_BASE_DIPLOMACY: u32 = 35;

/// What the adapter vouches for when a strategy card's ability runs in the core: the guards of the string windows that read the
/// string position (`strategy_cards_core`). `Default` carries nothing, so a play without the adapter's facts declines every
/// kind that needs one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Carried {
    /// The Politics draw is plain: no faction hook changes it and the hand stays within its limit (no question).
    pub plain_draw: bool,
    /// The research window is carried at this table: the rules version asks the payment, no seat has an effect on a research the
    /// core table does not hold, and the hooks of the core and of the string modules agree.
    pub research: bool,
    /// The payments of Leadership follow the production rules (no faction effect on a payment the production kernel does not hold).
    pub payments: bool,
    /// The Construction window is carried: the structures the seat places (`None`: a faction effect changes the placement).
    pub structures: Option<StructureFacts>,
    /// The interned `trade_primary`, the source of the three trade goods the Trade primary announces.
    pub trade_source: Code,
}

/// Everything [`begin`] reads besides the position.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub carried: Carried,
    /// A replenishment at this table causes events the core does not hold (a Cabal, a siphoning Firmament plot, a Trade Agreement in
    /// play).
    pub replenish_events: bool,
    /// `GameState::rules_version`.
    pub rules_version: u32,
    /// The faction units of the scope (the research).
    pub roster: Option<&'static UnitRoster>,
    /// The sources in play.
    pub scope: Scope,
    /// `GameState::tokens_held_by_mahact` of the performing seat.
    pub mahact_held: i32,
}

/// How an ability began.
pub enum Began {
    /// Performed, with nothing to ask.
    Done,
    /// A window of its own runs it: the child the table starts for this request.
    Run(Spawn),
    /// The kind is not carried (or not at this table): the string performs it.
    Unsupported,
}

/// The performed ability of `card` by `who`: the secondary or the primary. Abilities that need no question are applied now.
pub fn begin(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool, f: Facts) -> Began {
    let carried = f.carried;
    let official = f.rules_version >= RULES_OFFICIAL_ACTION_DECK;
    match (card.data().kind, primary) {
        (StrategyCardKind::Trade, false) => {
            // The string replenishes through the Siphon, Cabal and Trade Agreement hooks: events the core does not hold.
            if f.replenish_events {
                return Began::Unsupported;
            }
            fx::replenish_commodities(sx.state, who);
            Began::Done
        }
        (StrategyCardKind::Trade, true) => {
            if f.replenish_events {
                return Began::Unsupported;
            }
            Began::Run(Spawn::Trade { who })
        }
        (StrategyCardKind::Imperial, false) => {
            // A deck that has run out draws nothing; the limit is enforced only after a draw.
            if fx::draw_secret(sx.state, who).is_none() {
                return Began::Done;
            }
            Began::Run(Spawn::HandLimit { who })
        }
        (StrategyCardKind::Politics, primary) => {
            if !carried.plain_draw {
                return Began::Unsupported;
            }
            Began::Run(Spawn::Politics { who, primary, official })
        }
        (StrategyCardKind::Technology, primary) => {
            let (true, Some(roster)) = (carried.research, f.roster) else { return Began::Unsupported };
            let cost = if primary { TECHNOLOGY_PRIMARY_SECOND_COST } else { TECHNOLOGY_SECONDARY_COST };
            Began::Run(Spawn::Research { who, primary, cost, scope: f.scope, roster })
        }
        (StrategyCardKind::Leadership, primary) => {
            // A command token held in a Mahact pool changes the reinforcements the pool questions preview.
            if !carried.payments || f.mahact_held > 0 {
                return Began::Unsupported;
            }
            Began::Run(Spawn::Leadership { who, primary })
        }
        (StrategyCardKind::Construction, primary) => {
            // The Thunder's Edge primary asks a structure or a production first (the production is the string's).
            let (Some(facts), false) = (carried.structures, primary && card == StrategyCard::ConstructionThundersEdge) else {
                return Began::Unsupported;
            };
            Began::Run(Spawn::Construction { who, primary, facts })
        }
        (StrategyCardKind::Warfare, true) => {
            // The Thunder's Edge primary is a free tactical action (the string's).
            if f.mahact_held > 0 || card == StrategyCard::WarfareThundersEdge {
                return Began::Unsupported;
            }
            Began::Run(Spawn::Warfare { who, letnev_pool: f.rules_version >= RULES_LETNEV_FLEET_POOL })
        }
        (StrategyCardKind::Diplomacy, primary) => {
            let base_printing = primary && card == StrategyCard::DiplomacyBase && f.rules_version >= RULES_BASE_DIPLOMACY;
            Began::Run(Spawn::Diplomacy { who, card, primary, base_printing })
        }
        (StrategyCardKind::Warfare, false) | (StrategyCardKind::Imperial, true) => Began::Unsupported,
    }
}

/// What a finished ability leaves for the play.
pub struct Finished {
    /// The window left the carried path (a research only a faction waiver opens): the string performs the ability.
    pub failed: bool,
    /// The system the Diplomacy primary placed tokens in and the seats that got one (in seating order): the adapter announces them.
    pub tokens: Option<(System, SmallList<Seat, 8>)>,
}

/// What the ability announces when it is done, from the [`Report`] of its window: the three trade goods of the Trade primary
/// (staged, as the string stages them after its window). `trade_source` is the interned `trade_primary`.
pub fn finish(report: Report, s: &mut CoreState, trade_source: Code) -> Finished {
    match report {
        Report::Left => Finished { failed: true, tokens: None },
        Report::Placed(system, seats) => Finished { failed: false, tokens: Some((system, seats)) },
        Report::Goods(who, goods) => {
            fx::stage(s, fx::trade_goods_gained(who, goods, trade_source));
            Finished { failed: false, tokens: None }
        }
        Report::Nothing | Report::Paid(_) | Report::Chose(_) | Report::Destroyed(_) | Report::Built(_) | Report::Made(_) | Report::Removed(_) => Finished { failed: false, tokens: None },
    }
}
