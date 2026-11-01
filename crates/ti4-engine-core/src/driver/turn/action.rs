//! The option of the action menu a seat chose, run as one step (package KM2, lane F; the `Phase::Action` arm of `Game::apply_choice`).
//!
//! [`run_action`] is `TurnJob::Action(key)`: the closing-menu bookkeeping (a non-free option taken from the closing menu is the second
//! action), then the body of the option. Pass is the core's end to end. The component actions run through [`component`]; one whose body
//! the core does not run yet stops the step with the counted gap [`MenuGap::Unrouted`] (the kind and the option's key name it), leaving the position as it was.

use super::component;
use super::inline::{emit_typed, gap, player_payload, Flow};
use crate::choice::turn::Opt;
use crate::driver::dispatch::{self, Dispatch};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};
use crate::state::{EventKind, PlainKind, PlainRec};

/// The gap code of an option the core does not run yet (`MenuGap::Unrouted`).
#[must_use]
pub const fn gap_code(d: &Dispatch) -> u8 {
    match d {
        Dispatch::Pass => 0,
        Dispatch::Tactical => 1,
        Dispatch::Strategic(_) => 2,
        Dispatch::EndTurn => 3,
        Dispatch::Token => 4,
        Dispatch::Leader { .. } => 5,
        Dispatch::Technology(_) => 6,
        Dispatch::Relic => 7,
        Dispatch::Device(_) => 8,
        Dispatch::ActionCard => 9,
        Dispatch::Transaction(_) => 10,
        Dispatch::Faction(_) => 11,
        Dispatch::Refused => 12,
    }
}

/// `Game::apply_choice`, `Phase::Action`.
pub fn run_action(sx: &mut Sx<'_>, key: u32) -> Flow {
    let Some(opt) = Opt::from_key(key) else {
        gap(sx, TurnGap::Menu(MenuGap::Refused));
        return Flow::Halt;
    };
    let Some(active) = sx.state.meta().active else {
        gap(sx, TurnGap::Menu(MenuGap::Refused));
        return Flow::Halt;
    };
    // The closing menu: anything that takes an action is Fleet Logistics' second one; the turn is closed again when it finishes.
    if sx.state.pending().turn_closing.is_some() && !super::action_menu::is_free(opt) {
        sx.state.pending_mut().turn_closing = None;
        crate::rules::trade::clear_action_negotiations(sx.state);
    }
    let d = dispatch::dispatch(opt);
    match d {
        Dispatch::Pass => pass(sx, active),
        Dispatch::Tactical => tactical(sx, active),
        Dispatch::Strategic(card) => crate::driver::windows::strategic::begin::run_begin(sx, card),
        Dispatch::Transaction(with) => transaction(sx, active, with),
        Dispatch::Relic => match opt {
            Opt::Purge(kind) => super::relic::purge(sx, active, key, kind),
            Opt::UseRelic(relic) => super::relic::use_relic(sx, active, key, relic),
            _ => unrouted(sx, opt),
        },
        Dispatch::Faction(action) => super::faction::run(sx, active, key, action),
        Dispatch::Leader { leader, borrowed } => super::leader::run(sx, active, key, leader, borrowed),
        Dispatch::Technology(tech) => super::technology::run(sx, active, key, tech),
        Dispatch::ActionCard => match opt {
            Opt::Play { index, card } => component::play_card(sx, active, key, card, Some(index)),
            Opt::Salvaged(card) => component::play_card(sx, active, key, card, None),
            _ => unrouted(sx, opt),
        },
        _ => unrouted(sx, opt),
    }
}

/// The option has no native body yet: the counted gap names it (the kind, and the option's key).
fn unrouted(sx: &mut Sx<'_>, opt: Opt) -> Flow {
    gap(sx, TurnGap::Menu(MenuGap::Unrouted(gap_code(&dispatch::dispatch(opt)), opt.key().unwrap_or(0)))); // mutants:skip: every option on the menu has a key
    Flow::Halt
}

/// A tactical action begins (`Game::apply_choice`: `settle_extreme_duress`, the window, `TACTICAL_ACTION_BEGAN`); the activation question is the next
/// step's (the tactical arm).
fn tactical(sx: &mut Sx<'_>, active: Seat) -> Flow {
    if component::settle_extreme_duress(sx, active, false) == Flow::Halt {
        return Flow::Halt;
    }
    crate::driver::windows::tactical::open_tactical(sx.state, active);
    Flow::Done
}

/// A transaction opens with `with`: Extreme Duress settles, then the negotiation is noted, the pair's transaction is spent and the window stands on
/// the stack (the caller runs it to its first question in the same call: `ask::run_chosen`).
fn transaction(sx: &mut Sx<'_>, active: Seat, with: Seat) -> Flow {
    if component::settle_extreme_duress(sx, active, false) == Flow::Halt {
        return Flow::Halt;
    }
    crate::driver::windows::trade::open(sx.state, sx.kernel, active, with);
    Flow::Done
}

/// The pass: `passed`, the legendary planets' pass abilities, Prove Endurance, `PLAYER_PASSED` and the end of the turn.
fn pass(sx: &mut Sx<'_>, active: Seat) -> Flow {
    dispatch::mark_passed(sx.state, active);
    if super::end::legendary_pass(sx, active) == Flow::Halt {
        return Flow::Halt;
    }
    if let Some(occurrence) = dispatch::last_to_pass(sx.state, active) {
        // Watchful Ojz: "Be the last person to pass during the Action Phase" unlocks at the pass itself.
        super::start::commanders(sx, active);
        crate::driver::windows::scoring::open_event(sx.state, crate::state::ScoringTiming::Action, occurrence, false);
    }
    sx.plain(PlainRec::bare(PlainKind::PlayerPassed));
    if emit_typed(sx, EventKind::PlayerPassed, player_payload(active)) == Flow::Halt {
        return Flow::Halt;
    }
    dispatch::decline_additional_action(sx.state);
    super::end::run_end(sx)
}
