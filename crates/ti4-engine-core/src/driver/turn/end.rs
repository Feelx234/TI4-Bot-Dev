//! The end of an action and the end of a turn (`Game::finish_action`, `advance_turn_inner`, `pass_on_tail`) as jobs.
//!
//! [`run_end`] is `advance_turn_inner`, in the string's order: the retention check (an additional action keeps the turn), Minister of
//! War, the per-turn scratch, the end-of-turn windows (technologies, legendary planets), a second retention check (Jupiter Brain), the
//! fleet sweep and the passing of the turn with its announcements. [`run_finish`] is `finish_action`: `ACTION_COMPLETED`, then the
//! closing menu or the end of the turn.

use super::facts;
use super::inline::{emit_typed, gap, halted, last_plain_is, player_payload, run, Flow};
use crate::choice::legendary::When;
use crate::driver::turn_flow;
use crate::driver::windows::enforce_all::EnforceAll;
use crate::driver::windows::minister::MinisterOfWar;
use crate::generated::Agenda;
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::query::{explore, laws, reach};
use crate::rules::legendary::{Legendary, END_OF_TURN};
use crate::rules::technology_windows::TurnEnd;
use crate::state::{CoreState, EventKind, Outcome, Phase, PlainKind, PlainRec, TransientFlags};
use crate::keys::Word;

/// The retained turn's announcement (`TURN_RETAINED`).
fn retained(sx: &mut Sx<'_>) {
    sx.plain(PlainRec::bare(PlainKind::TurnRetained));
}

/// `Game::minister_of_war`: the owner of the elected law is offered to discard it for a command token and an additional action.
fn minister_of_war(sx: &mut Sx<'_>) -> Flow {
    let blanked = reach::blanked(sx.state);
    let Some(Outcome::Seat(owner)) = laws::elected(sx.state, blanked, Agenda::MinisterOfWar) else { return Flow::Done };
    if sx.state.players().by_seat(owner).is_none() {
        return Flow::Done;
    }
    let systems: Vec<_> = sx.state.board().systems().filter(|(_, cell)| cell.cmd.contains(owner)).map(|(system, _)| system).collect();
    let codes = sx.table.codes.turn;
    let mut window = MinisterOfWar::new(owner, systems, Agenda::MinisterOfWar, codes.minister_prompt, codes.minister_source);
    run(sx, &mut window)
}

/// `legendary::end_turn`: the abilities of the legendary planets the seat controls, used at the end of its turn.
fn legendary(sx: &mut Sx<'_>, active: Seat) -> Flow {
    // Nothing to offer, nothing asked and nothing announced: the window does not need the tables of the sources.
    if !END_OF_TURN.iter().any(|p| explore::legendary_available(sx.state, active, *p)) {
        return Flow::Done;
    }
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else {
        gap(sx, TurnGap::NoRoster);
        return Flow::Halt;
    };
    let drop_unofficial = sx.state.meta().rules_at_least(OFFICIAL_ACTION_DECK);
    let mut window = Legendary::new(active, When::EndOfTurn, sx.table.codes.turn.legendary, reach::scope_of(sx.kernel), roster, drop_unofficial);
    let flow = run(sx, &mut window);
    if flow == Flow::Done && window.unsupported {
        gap(sx, TurnGap::LegendaryUnsupported);
        return Flow::Halt;
    }
    flow
}

/// `ti4_model::state::RULES_OFFICIAL_ACTION_DECK`: from this version the unofficial action cards leave the deck before a draw.
const OFFICIAL_ACTION_DECK: u32 = 30;

/// `fleet::enforce_everywhere`.
fn fleet_sweep(sx: &mut Sx<'_>) -> Flow {
    let pairs = facts::enforce_pairs(sx.state, sx.kernel);
    run(sx, &mut EnforceAll::new(pairs))
}

/// `Game::advance_turn_inner`.
pub fn run_end(sx: &mut Sx<'_>) -> Flow {
    sx.state.pending_mut().turn_closing = None;
    // A new turn re-offers everything: the withholding of a failed component action is scoped to the turn whose action failed.
    turn_flow::forget_failed_components(sx.state);
    if turn_flow::take_retention(sx.state) {
        retained(sx);
        return Flow::Done;
    }
    if sx.state.meta().phase == Phase::Action {
        if minister_of_war(sx) == Flow::Halt {
            return Flow::Halt;
        }
        if turn_flow::take_retention(sx.state) {
            retained(sx);
            return Flow::Done;
        }
    }
    let ended = sx.state.meta().active;
    sx.state.pending_mut().actions_this_turn = 0;
    turn_flow::end_turn_scratch(sx.state);
    if sx.state.meta().phase == Phase::Action
        && let Some(active) = ended
    {
        if run(sx, &mut TurnEnd::new(active)) == Flow::Halt {
            return Flow::Halt;
        }
        // Readying last: what The Acropolis readied survives into the next round rather than being offered straight back.
        if legendary(sx, active) == Flow::Halt {
            return Flow::Halt;
        }
        // Jupiter Brain grants an additional action from inside that window.
        if turn_flow::take_retention(sx.state) {
            retained(sx);
            return Flow::Done;
        }
    }
    if ended.is_some() && fleet_sweep(sx) == Flow::Halt {
        return Flow::Halt;
    }
    pass_on(sx, ended)
}

/// `Game::pass_on_tail`: the turn passes on, its end is announced (`TURN_PASSED`), the seat Crisis names is skipped and a Puppets on a
/// String grant gives the passer a fresh turn. The tail goes on in the position the reaction windows of `TURN_PASSED` left.
fn pass_on(sx: &mut Sx<'_>, ended: Option<Seat>) -> Flow {
    let next = turn_flow::advance_turn(sx.state);
    let puppet = sx.state.transient().misc.flags.has(TransientFlags::PUPPET_ACTION);
    if next.is_none() && !puppet {
        // The last turn of the action phase still ends: its "when a player's turn ends" windows open, there is just no next turn.
        if let Some(ended) = ended {
            return announce_passed(sx, ended);
        }
        return Flow::Done;
    }
    let Some(ended) = ended else { return Flow::Done };
    if announce_passed(sx, ended) == Flow::Halt {
        return Flow::Halt;
    }
    if sx.state.transient().misc.flags.has(TransientFlags::SKIP_NEXT_TURN) {
        sx.state.transient_mut().misc.flags.clear(TransientFlags::SKIP_NEXT_TURN);
        if let Some(skipped) = sx.state.meta().active {
            sx.plain(PlainRec::one(PlainKind::TurnSkipped, crate::state::PlainArg::Seat(skipped)));
            let _ = turn_flow::advance_turn(sx.state);
        }
    }
    // Puppets on a String: a fresh turn (new `turn_seq`, start-of-turn hooks, `TURN_BEGAN`) and the seat stays passed.
    if sx.state.transient().misc.flags.has(TransientFlags::PUPPET_ACTION) {
        sx.state.transient_mut().misc.flags.clear(TransientFlags::PUPPET_ACTION);
        turn_flow::begin_action_turn(sx.state, ended);
        sx.plain(PlainRec::one(PlainKind::TurnPuppet, crate::state::PlainArg::Seat(ended)));
    }
    Flow::Done
}

/// `TURN_PASSED`, plain then typed (the typed event is mirrored into the queue after the plain line, as the string pushes them).
fn announce_passed(sx: &mut Sx<'_>, ended: Seat) -> Flow {
    sx.plain(PlainRec::bare(PlainKind::TurnPassed));
    emit_typed(sx, EventKind::TurnPassed, player_payload(ended))
}

/// `Game::emit_action_completed`'s `component` flag: the last event of the queue is `COMPONENT_ACTION_RESOLVED`.
fn component_resolved(core: &CoreState) -> bool {
    last_plain_is(core, PlainKind::ComponentActionResolved)
}

/// Cards a faction effect showed "for this action" stop being visible with it (`hooks_cards::clear_reveals(Action)`).
fn clear_action_reveals(core: &mut CoreState) {
    let doomed: Vec<_> = core
        .marks()
        .tail
        .iter()
        .filter(|(k, _)| matches!(k, crate::state::MarkKey::CardsReveal(kind, ..) if *kind == Word::Action))
        .map(|(k, _)| *k)
        .collect();
    for key in doomed {
        core.marks_mut().tail.edit().remove(&key);
    }
}

/// `Game::finish_action`.
pub fn run_finish(sx: &mut Sx<'_>) -> Flow {
    if let Some(active) = sx.state.meta().active {
        // The resolver's clock is the one the step began with, as the string's is the one its last `sync_timing_context` left.
        let payload = player_payload(active).with(crate::state::Key::Component, crate::state::Val::Flag(component_resolved(sx.state)));
        if emit_typed(sx, EventKind::ActionCompleted, payload) == Flow::Halt {
            return Flow::Halt;
        }
        clear_action_reveals(sx.state);
        let retained = sx.state.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION);
        if sx.state.meta().phase == Phase::Action && !retained {
            let p = sx.state.pending_mut();
            p.actions_this_turn = p.actions_this_turn.saturating_add(1);
            p.turn_closing = Some(active);
            let len = match super::closing::options(sx.state, sx.kernel, active) {
                Ok(options) => options.len(),
                Err(why) => {
                    gap(sx, why);
                    return Flow::Halt;
                }
            };
            if len > 1 {
                sx.plain(PlainRec::bare(PlainKind::TurnClosing));
                return Flow::Done;
            }
            sx.state.pending_mut().turn_closing = None;
        }
    }
    if halted(sx) { Flow::Halt } else { run_end(sx) }
}
