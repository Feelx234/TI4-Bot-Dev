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

use crate::generated::StrategyCard;
use crate::ids::Seat;
use crate::kernel::children::Report;
use crate::kernel::fx_perform::{self, Facts as PerformFacts};
use crate::kernel::stage::Sx;
use crate::state::CoreState;

use super::Env;

pub use crate::kernel::fx_perform::{Began, Carried, Finished};

/// The facts the performed ability reads, from the play's environment.
fn facts_of(env: Env) -> PerformFacts {
    PerformFacts {
        carried: env.facts.carried,
        replenish_events: env.facts.replenish_events,
        rules_version: env.facts.rules_version,
        roster: env.facts.roster,
        scope: env.scope,
        mahact_held: env.mahact_held,
        // The action cards that perform an ability leave the home production and the Imperial primary to the string.
        home: fx_perform::Home::Unsupported,
        imperial: None,
        // (The Thunder's Edge primaries of Overrule are the string's.)
        free: None,
        te_uses: None,
    }
}

/// The performed ability of `card` by `who`: the secondary or the primary. Abilities that need no question are applied now.
pub(super) fn begin(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool, env: Env) -> Began {
    fx_perform::begin(sx, who, card, primary, facts_of(env))
}

/// What the ability announces when it is done, from the [`Report`] of its window.
pub(super) fn finish(report: Report, s: &mut CoreState, env: Env) -> Finished {
    fx_perform::finish(report, s, env.facts.carried.trade_source)
}
