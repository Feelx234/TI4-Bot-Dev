//! Strategize and Overrule (package B6d; the string `action_cards::perform_strategy_card`): after the pick of a strategy
//! card, the holder performs its secondary (Strategize) or primary (Overrule) ability, as a child window by the kind of the
//! card. The abilities are the windows of the strategy areas (`production::strategy_cards`, `strategy_primaries`,
//! `strategy_windows`, `objective_windows`); the guards that read the string side (a draw a hook changes, a payment the
//! production kernel does not follow, the placement of a structure) are the adapter's facts ([`Carried`]).
//!
//! Carried: the secondaries of Trade (the replenishment), Imperial (the draw of a secret objective and the hand limit),
//! Politics (the draw of two action cards), Technology (the paid research), Leadership, Construction and Diplomacy, and the
//! primaries of Trade, Politics, Technology, Leadership, Construction and Warfare.
//!
//! Not carried (the run is `unsupported`: the adapter declines it and the string performs the ability; one test each in the
//! adapter's `action_cards_b6d_tests`):
//! * the Warfare secondary (`home_production`: a whole production window at the home system);
//! * the Imperial primary (the scoreable objectives, the award tail and the winner are the string's);
//! * the Diplomacy primary (the announcements of the tokens it places are staged on the string state);
//! * the Thunder's Edge Construction and Warfare primaries (the choice of a production, the free tactical action).

use crate::choice::{Choice, Pick};
use crate::generated::{StrategyCard, StrategyCardKind};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::rules::objective_windows::HandLimit;
use crate::rules::production::strategy_cards::{Construction, Leadership, Research, StructureFacts};
use crate::rules::strategy_primaries::{DMode, Diplomacy, Politics, WarfarePrimary};
use crate::rules::strategy_windows::{TradePrimary, TRADE_PRIMARY_GOODS};
use crate::state::{Code, CoreState};

use super::Env;

/// `TECHNOLOGY_PRIMARY_SECOND_COST` and `TECHNOLOGY_SECONDARY_COST` (`strategy_cards`): the resources of the paid research.
const TECHNOLOGY_PRIMARY_SECOND_COST: i64 = 6;
const TECHNOLOGY_SECONDARY_COST: i64 = 4;
/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`: from this version the unofficial action cards leave the deck before a draw.
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;
/// `ti4_model::state::RULES_LETNEV_FLEET_POOL`: from this version the Letnev fleet pool may be emptied.
const RULES_LETNEV_FLEET_POOL: u32 = 4;

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

/// The child window of an ability that asks.
#[derive(Debug, Clone)]
pub enum Performed {
    /// The paid research of the Technology secondary, or the primary's free and paid research.
    Research(Research),
    /// A secret objective was drawn and the hand limit is enforced.
    Limit(HandLimit),
    Leadership(Leadership),
    Construction(Construction),
    Politics(Politics),
    Warfare(WarfarePrimary),
    Diplomacy(Diplomacy),
    Trade(TradePrimary),
}

/// How an ability began.
pub(super) enum Began {
    /// Performed, with nothing to ask.
    Done,
    /// A window of its own runs it.
    Run(Performed),
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
            Began::Run(Performed::Trade(TradePrimary::new(who)))
        }
        (StrategyCardKind::Imperial, false) => {
            // A deck that has run out draws nothing; the limit is enforced only after a draw.
            if fx::draw_secret(sx.state, who).is_none() {
                return Began::Done;
            }
            Began::Run(Performed::Limit(HandLimit::new(who)))
        }
        (StrategyCardKind::Politics, primary) => {
            if !carried.plain_draw {
                return Began::Unsupported;
            }
            Began::Run(Performed::Politics(Politics::new(who, primary, official)))
        }
        (StrategyCardKind::Technology, primary) => {
            let (true, Some(roster)) = (carried.research, env.facts.roster) else { return Began::Unsupported };
            let cost = if primary { TECHNOLOGY_PRIMARY_SECOND_COST } else { TECHNOLOGY_SECONDARY_COST };
            Began::Run(Performed::Research(Research::new(who, primary, cost, env.scope).with_roster(*roster)))
        }
        (StrategyCardKind::Leadership, primary) => {
            // A command token held in a Mahact pool changes the reinforcements the pool questions preview.
            if !carried.payments || env.mahact_held > 0 {
                return Began::Unsupported;
            }
            Began::Run(Performed::Leadership(Leadership::new(who, primary)))
        }
        (StrategyCardKind::Construction, primary) => {
            // The Thunder's Edge primary asks a structure or a production first (the production is the string's).
            let (Some(facts), false) = (carried.structures, primary && card == StrategyCard::ConstructionThundersEdge) else {
                return Began::Unsupported;
            };
            Began::Run(Performed::Construction(Construction::new(who, primary, facts)))
        }
        (StrategyCardKind::Warfare, true) => {
            // The Thunder's Edge primary is a free tactical action (the string's).
            if env.mahact_held > 0 || card == StrategyCard::WarfareThundersEdge {
                return Began::Unsupported;
            }
            Began::Run(Performed::Warfare(WarfarePrimary::new(who, env.facts.rules_version >= RULES_LETNEV_FLEET_POOL)))
        }
        (StrategyCardKind::Diplomacy, false) => Began::Run(Performed::Diplomacy(Diplomacy::new(who, card, DMode::Secondary))),
        (StrategyCardKind::Warfare, false) | (StrategyCardKind::Imperial, true) | (StrategyCardKind::Diplomacy, true) => Began::Unsupported,
    }
}

impl Performed {
    /// Whether the window left the carried path (the run is the string's).
    pub(super) const fn failed(&self) -> bool {
        match self {
            Self::Research(w) => w.failed,
            _ => false,
        }
    }

    /// What the ability announces when it is done: the three trade goods of the Trade primary (staged, as the string stages
    /// them after its window).
    pub(super) fn finish(&self, s: &mut CoreState, env: Env) {
        if let Self::Trade(w) = self {
            fx::stage(s, fx::trade_goods_gained(w.who, TRADE_PRIMARY_GOODS, env.facts.carried.trade_source));
        }
    }
}

impl Window for Performed {
    type Stage = ();

    fn stage(&self) {}

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        match self {
            Self::Research(w) => w.step(sx),
            Self::Limit(w) => w.step(sx),
            Self::Leadership(w) => w.step(sx),
            Self::Construction(w) => w.step(sx),
            Self::Politics(w) => w.step(sx),
            Self::Warfare(w) => w.step(sx),
            Self::Diplomacy(w) => w.step(sx),
            Self::Trade(w) => w.step(sx),
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self {
            Self::Research(w) => w.ask_of(s, k),
            Self::Limit(w) => w.ask_of(s, k),
            Self::Leadership(w) => w.ask_of(s, k),
            Self::Construction(w) => w.ask_of(s, k),
            Self::Politics(w) => w.ask_of(s, k),
            Self::Warfare(w) => w.ask_of(s, k),
            Self::Diplomacy(w) => w.ask_of(s, k),
            Self::Trade(w) => w.ask_of(s, k),
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self {
            Self::Research(w) => w.answer(sx, pick),
            Self::Limit(w) => w.answer(sx, pick),
            Self::Leadership(w) => w.answer(sx, pick),
            Self::Construction(w) => w.answer(sx, pick),
            Self::Politics(w) => w.answer(sx, pick),
            Self::Warfare(w) => w.answer(sx, pick),
            Self::Diplomacy(w) => w.answer(sx, pick),
            Self::Trade(w) => w.answer(sx, pick),
        }
    }
}
