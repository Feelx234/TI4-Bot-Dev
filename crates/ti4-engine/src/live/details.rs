//! The display facts of a core question that only the state knows (`Choice.details`), computed from the `CoreState` alone: the
//! string builds them from its position (`tokens::with_pool_details`, ...). Display only: invisible to the rules and to every
//! hash, so the oracle's choice comparison (`details` included) is what guards them.

use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::state::CoreState;

use crate::choice::Choice;

/// Complete the rendered `rendered` of `choice`, asked in `core`.
pub fn complete(core: &CoreState, k: &ti4_engine_core::kernel::Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    if let Ask::Tactical(_) = choice.ask {
        crate::tactical_live::complete(core, k, choice, rendered);
    }
    if let Ask::Combat(_) = choice.ask {
        crate::combat_live::complete(core, k, choice, rendered);
    }
    if let Ask::Strategy(SAsk::GainPool { left }) = choice.ask {
        pool_details(core, choice, usize::from(left), rendered);
    }
}

/// `tokens::with_pool_details(.., "gain", Some(left))`: the pools as they stand, the tokens in reinforcements, Letnev's Armada.
fn pool_details(core: &CoreState, choice: &CoreChoice, left: usize, rendered: &mut Choice) {
    let Some(p) = core.players().by_seat(choice.actor) else { return };
    let d = &mut rendered.details;
    d.insert("kind".to_owned(), "command_tokens".into());
    d.insert("mode".to_owned(), "gain".into());
    d.insert(
        "pools".to_owned(),
        serde_json::json!({ "tactic": p.econ.tactic_tokens, "fleet": p.econ.fleet_tokens, "strategic": p.econ.strategic_tokens }),
    );
    d.insert("reinforcements".to_owned(), fx::tokens_in_reinforcements(core, choice.actor).into());
    if p.faction == ti4_engine_core::generated::Faction::TheBaronyOfLetnev {
        d.insert("fleet_supply_bonus".to_owned(), 2.into());
    }
    d.insert("tokens_to_place".to_owned(), left.into());
}
