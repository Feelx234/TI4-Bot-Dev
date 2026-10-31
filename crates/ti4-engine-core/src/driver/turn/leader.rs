//! The leader component actions of the action menu (`Game::apply_choice`, the `component|leader|` arm, over `perform_leader_action` and
//! `leaders::use_leader`): the window of each leader, its facts computed from the core, the card exhausted when it resolved, then the verdict. A
//! leader whose window is not routed yet is the counted gap [`MenuGap::Unrouted`] (named by the option's key).
//!
//! Routed: Ggrocuto Rinn (Xxcha agent, an ACTION), Carth of Golden Sands (Hacan agent, printed "during the action phase": a free use). The rest of the agents, the commanders, the heroes and the Ssruu copies are still lent
//! (docs/KM3_RESUME.md lists them with what each needs).

use super::component::{conclude, Verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::choice::turn::Opt;
use crate::generated::Leader;
use crate::ids::Seat;
use crate::kernel::fx_strategy;
use crate::kernel::turn_codes::{MenuGap, TurnGap};
use crate::rules::leader_windows::{HacanAgent, XxchaAgent};
use crate::state::{PlainKind, PlainRec};

/// Run the use of `leader` chosen as option `key`.
pub fn run(sx: &mut crate::kernel::stage::Sx<'_>, who: Seat, key: u32, leader: Leader, borrowed: bool) -> Flow {
    match (leader, borrowed) {
        (Leader::GgrocutoRinn, false) => xxcha_agent(sx, who, key),
        (Leader::CarthOfGoldenSands, false) => carth(sx, who, key),
        // mutants:skip (the key default): every option on the menu has a key
        _ => gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(5, if borrowed { Opt::Borrowed(leader) } else { Opt::Leader(leader) }.key().unwrap_or(0)))),
    }
}

/// Ggrocuto Rinn: ready an exhausted planet (asked when there are several), and remove an infantry from a planet next to one of the seat's. An
/// ACTION: the card exhausts, Extreme Duress settles, and a resolved use ends the action. The string arm has a map in every live game.
fn xxcha_agent(sx: &mut crate::kernel::stage::Sx<'_>, who: Seat, key: u32) -> Flow {
    let mut window = XxchaAgent::new(who, true);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    let used = window.used;
    if used {
        let _ = fx_strategy::exhaust_ready_agent(sx.state, sx.kernel, who, Leader::GgrocutoRinn);
    }
    conclude(sx, who, key, if used { Verdict::Resolved } else { Verdict::Failed { withhold: true } }, true)
}

/// Carth of Golden Sands: gain 2 commodities or replenish another player's. "During the action phase" is not an ACTION (OP-07): the use does not take the
/// turn, so there is no Extreme Duress and no end of the action; the string says `LEADER_ABILITY_RESOLVED` and the seat is asked for its action again.
fn carth(sx: &mut crate::kernel::stage::Sx<'_>, who: Seat, key: u32) -> Flow {
    let agreement = sx.table.codes.explore.filled.then_some(sx.table.codes.explore.trade_agreement);
    let mut window = agreement.map_or_else(|| HacanAgent::new(who), |source| HacanAgent::with_source(who, source));
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    let used = window.used;
    if used {
        let _ = fx_strategy::exhaust_ready_agent(sx.state, sx.kernel, who, Leader::CarthOfGoldenSands);
    }
    if super::component::announce_staged(sx) == Flow::Halt {
        return Flow::Halt;
    }
    if used {
        sx.plain(PlainRec::bare(PlainKind::LeaderAbilityResolved));
    } else {
        sx.plain(PlainRec::bare(PlainKind::ComponentActionFailed));
        sx.state.pending_mut().note_failed_key(key);
    }
    Flow::Done
}
