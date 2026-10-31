//! Strategize and Overrule (package B6d; the string `action_cards::perform_strategy_card`): after the pick of a strategy
//! card, the holder performs its secondary (Strategize) or primary (Overrule) ability, as a child window by the kind of the
//! card. The abilities are the windows of the strategy areas (`production::strategy_cards`, `strategy_primaries`,
//! `strategy_windows`, `objective_windows`); this area does not import them: it names the child by a [`Spawn`] request and the
//! table's links start it (`driver::children`), and what the window did comes back as a [`Report`]. The guards that read the
//! string side (a draw a hook changes, a payment the production kernel does not follow, the placement of a structure) are the
//! adapter's facts ([`Carried`]).
//!
//! Carried: the secondaries of Trade (the replenishment), Imperial (the draw of a secret objective and the hand limit),
//! Politics (the draw of two action cards), Technology (the paid research), Leadership, Construction and Diplomacy, and the
//! primaries of Trade, Politics, Technology, Leadership, Construction and Warfare.
//!
//! The Diplomacy primary is carried too: the tokens it places are announced by the adapter after the window
//! ([`Finished::tokens`]: the string stages those events on its own state).
//!
//! Not carried (the run is `unsupported`: the adapter declines it and the string performs the ability; one test each in the
//! adapter's `action_cards_b6d_tests`):
//! * the Warfare secondary (`home_production`: a whole production window at the home system);
//! * the Imperial primary (the scoreable objectives, the award tail and the winner are the string's);
//! * the Thunder's Edge Construction and Warfare primaries (the choice of a production, the free tactical action).

use crate::containers::SmallList;
use crate::generated::{StrategyCard, StrategyCardKind, System};
use crate::ids::Seat;
use crate::kernel::children::{Report, Spawn, StructureFacts};
use crate::kernel::stage::Sx;
use crate::kernel::fx;
use crate::state::{Code, CoreState};

use super::Env;

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

/// How an ability began.
pub(super) enum Began {
    /// Performed, with nothing to ask.
    Done,
    /// A window of its own runs it: the child the table starts for this request.
    Run(Spawn),
    /// The kind is not carried (or not at this table): the string performs it.
    Unsupported,
}

/// The performed ability of `card` by `who`: the secondary or the primary. Abilities that need no question are applied now.
pub(super) fn begin(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool, env: Env) -> Began {
    let carried = env.facts.carried;
    let official = env.facts.rules_version >= RULES_OFFICIAL_ACTION_DECK;
    match (card.data().kind, primary) {
        (StrategyCardKind::Trade, false) => {
            // The string replenishes through the Siphon, Cabal and Trade Agreement hooks: events the core does not hold.
            if env.facts.replenish_events {
                return Began::Unsupported;
            }
            fx::replenish_commodities(sx.state, who);
            Began::Done
        }
        (StrategyCardKind::Trade, true) => {
            if env.facts.replenish_events {
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
            let (true, Some(roster)) = (carried.research, env.facts.roster) else { return Began::Unsupported };
            let cost = if primary { TECHNOLOGY_PRIMARY_SECOND_COST } else { TECHNOLOGY_SECONDARY_COST };
            Began::Run(Spawn::Research { who, primary, cost, scope: env.scope, roster })
        }
        (StrategyCardKind::Leadership, primary) => {
            // A command token held in a Mahact pool changes the reinforcements the pool questions preview.
            if !carried.payments || env.mahact_held > 0 {
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
            if env.mahact_held > 0 || card == StrategyCard::WarfareThundersEdge {
                return Began::Unsupported;
            }
            Began::Run(Spawn::Warfare { who, letnev_pool: env.facts.rules_version >= RULES_LETNEV_FLEET_POOL })
        }
        (StrategyCardKind::Diplomacy, primary) => {
            let base_printing = primary && card == StrategyCard::DiplomacyBase && env.facts.rules_version >= RULES_BASE_DIPLOMACY;
            Began::Run(Spawn::Diplomacy { who, card, primary, base_printing })
        }
        (StrategyCardKind::Warfare, false) | (StrategyCardKind::Imperial, true) => Began::Unsupported,
    }
}

/// What a finished ability leaves for the play.
pub(super) struct Finished {
    /// The window left the carried path (a research only a faction waiver opens): the string performs the ability.
    pub failed: bool,
    /// The system the Diplomacy primary placed tokens in and the seats that got one (in seating order): the adapter announces them.
    pub tokens: Option<(System, SmallList<Seat, 8>)>,
}

/// What the ability announces when it is done, from the [`Report`] of its window: the three trade goods of the Trade primary
/// (staged, as the string stages them after its window).
pub(super) fn finish(report: Report, s: &mut CoreState, env: Env) -> Finished {
    match report {
        Report::Left => Finished { failed: true, tokens: None },
        Report::Placed(system, seats) => Finished { failed: false, tokens: Some((system, seats)) },
        Report::Goods(who, goods) => {
            fx::stage(s, fx::trade_goods_gained(who, goods, env.facts.carried.trade_source));
            Finished { failed: false, tokens: None }
        }
        Report::Nothing | Report::Paid(_) | Report::Chose(_) | Report::Destroyed(_) | Report::Built(_) | Report::Made(_) => Finished { failed: false, tokens: None },
    }
}
