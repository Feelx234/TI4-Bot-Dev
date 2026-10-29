//! Strategize and Overrule (package B6d; the string `action_cards::perform_strategy_card`): after the pick of a strategy
//! card, the holder performs its secondary (Strategize) or primary (Overrule) ability, as a child window by the kind of the
//! card. The abilities are the windows of the strategy areas (`production::strategy_cards`, `strategy_primaries`,
//! `objective_windows`); the guards that read the string side (a draw a hook changes, a payment the production kernel does not
//! follow) are the adapter's facts ([`Carried`]).
//!
//! Carried: the secondaries of Trade (the replenishment), Imperial (the draw of a secret objective and the hand limit),
//! Politics (the draw of two action cards), Technology (the paid research) and the primaries named in [`begin`]. A kind the
//! window does not carry marks the run unsupported (the adapter declines it and the string performs the ability), one test
//! each in `effects::tests`: Leadership, Construction, Diplomacy and Warfare (production windows whose questions read the
//! string position) and the Thunder's Edge Construction and Warfare primaries (strings first: the free tactical action).

use crate::choice::{Choice, Pick};
use crate::generated::{StrategyCard, StrategyCardKind};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::rules::objective_windows::HandLimit;
use crate::rules::production::strategy_cards::Research;
use crate::state::CoreState;

use super::Env;

/// `TECHNOLOGY_SECONDARY_COST` (`strategy_cards`): the resources the Technology secondary's research costs.
const TECHNOLOGY_SECONDARY_COST: i64 = 4;
/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`: from this version the unofficial action cards leave the deck before a draw.
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;

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
}

/// The child window of an ability that asks.
#[derive(Debug, Clone)]
pub enum Performed {
    /// The paid research of the Technology secondary.
    Research(Research),
    /// A secret objective was drawn and the hand limit is enforced.
    Limit(HandLimit),
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
    let kind = card.data().kind;
    if primary {
        return Began::Unsupported;
    }
    match kind {
        StrategyCardKind::Trade => {
            // The string replenishes through the Siphon, Cabal and Trade Agreement hooks: events the core does not hold.
            if env.facts.replenish_events {
                return Began::Unsupported;
            }
            fx::replenish_commodities(sx.state, who);
            Began::Done
        }
        StrategyCardKind::Imperial => {
            // A deck that has run out draws nothing; the limit is enforced only after a draw.
            if fx::draw_secret(sx.state, who).is_none() {
                return Began::Done;
            }
            Began::Run(Performed::Limit(HandLimit::new(who)))
        }
        StrategyCardKind::Politics => {
            if !env.facts.carried.plain_draw {
                return Began::Unsupported;
            }
            fx::draw_action_cards(sx.state, who, 2, env.facts.rules_version >= RULES_OFFICIAL_ACTION_DECK);
            Began::Done
        }
        StrategyCardKind::Technology => {
            let (true, Some(roster)) = (env.facts.carried.research, env.facts.roster) else { return Began::Unsupported };
            Began::Run(Performed::Research(Research::new(who, false, TECHNOLOGY_SECONDARY_COST, env.scope).with_roster(*roster)))
        }
        StrategyCardKind::Leadership | StrategyCardKind::Construction | StrategyCardKind::Diplomacy | StrategyCardKind::Warfare => {
            Began::Unsupported
        }
    }
}

impl Performed {
    /// Whether the window left the carried path (the run is the string's).
    pub(super) const fn failed(&self) -> bool {
        match self {
            Self::Research(w) => w.failed,
            Self::Limit(_) => false,
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
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self {
            Self::Research(w) => w.ask_of(s, k),
            Self::Limit(w) => w.ask_of(s, k),
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self {
            Self::Research(w) => w.answer(sx, pick),
            Self::Limit(w) => w.answer(sx, pick),
        }
    }
}
