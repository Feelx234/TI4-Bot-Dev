//! The display facts of a core question that only the state knows (`Choice.details`), computed from the `CoreState` alone: the
//! string builds them from its position (`tokens::with_pool_details`, ...). Display only: invisible to the rules and to every
//! hash, so the oracle's choice comparison (`details` included) is what guards them.

use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::state::CoreState;

use crate::choice::Choice;

/// Complete the rendered `rendered` of `choice`, asked in `core`.
pub fn complete(core: &CoreState, kernel: &Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    match choice.ask {
        Ask::Tactical(_) => crate::tactical_live::complete(core, kernel, choice, rendered),
        Ask::Combat(_) => crate::combat_live::complete(core, kernel, choice, rendered),
        Ask::Strategy(SAsk::GainPool { left }) => pool_details(core, choice.actor, "gain", Some(usize::from(left)), rendered),
        // 81.5: the whole sheet is rearranged (`tokens::with_pool_details(.., "redistribute", None)`).
        Ask::Strategy(SAsk::StatusRedistribute { .. }) => pool_details(core, choice.actor, "redistribute", None, rendered),
        // Warfare's redistributions: the whole sheet is rearranged.
        Ask::Strategy(SAsk::WarfareRedistribute { .. } | SAsk::TeWarfareRedistribute { .. } | SAsk::TeWarfareRedistributeAfter { .. }) => {
            pool_details(core, choice.actor, "redistribute", None, rendered);
        }
        // Carth of Golden Sands' branch: the seat's own commodities, which the "gain 2" branch adds to (`leaders::hacan_agent`'s `.detailed("commodities", ..)`).
        Ask::Leaders(ti4_engine_core::choice::leaders::Ask::HacanBranch) => {
            rendered.details.insert("commodities".to_owned(), super::strategic::commodity_standing(core, choice.actor));
        }
        // Integrated Economy's question (`production::integrated_economy_loop`) is a plain `Choice::new`: the string gives it no context.
        Ask::Production(ti4_engine_core::choice::production::Ask::Economy(_)) => rendered.context = None,
        _ => {}
    }
    // The placement of lane G4's abilities (Yin Spinner, the Naalu mech): the offer card with a caption per spot.
    crate::render::silent4::complete(choice, rendered);
    super::turn::complete(core, choice, rendered);
    super::menu::complete(core, choice, rendered);
    super::agenda::complete(core, choice, rendered);
    super::strategic::complete(core, kernel, choice, rendered);
    super::explore::complete(core, choice, rendered);
}

/// `tokens::with_pool_details(.., mode, to_place)`: the pools as they stand, the tokens in reinforcements, Letnev's Armada, then the tokens
/// still to place (a gain) or the total held (a redistribution).
pub(super) fn pool_details(core: &CoreState, actor: Seat, mode: &str, to_place: Option<usize>, rendered: &mut Choice) {
    let Some(p) = core.players().by_seat(actor) else { return };
    let d = &mut rendered.details;
    d.insert("kind".to_owned(), "command_tokens".into());
    d.insert("mode".to_owned(), mode.into());
    d.insert(
        "pools".to_owned(),
        serde_json::json!({ "tactic": p.econ.tactic_tokens, "fleet": p.econ.fleet_tokens, "strategic": p.econ.strategic_tokens }),
    );
    d.insert("reinforcements".to_owned(), fx::tokens_in_reinforcements(core, actor).into());
    if p.faction == ti4_engine_core::generated::Faction::TheBaronyOfLetnev {
        d.insert("fleet_supply_bonus".to_owned(), 2.into());
    }
    match (mode, to_place) {
        ("gain" | "buy", Some(count)) => {
            d.insert("tokens_to_place".to_owned(), count.into());
        }
        ("redistribute", _) => {
            d.insert("total".to_owned(), (p.econ.tactic_tokens + p.econ.fleet_tokens + p.econ.strategic_tokens).into());
        }
        _ => {}
    }
}
