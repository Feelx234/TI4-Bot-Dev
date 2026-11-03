//! What every component action of the action menu shares (package KM2, lane F2b): the Extreme Duress a seat owes once it takes an action, the
//! announcements of what the body staged, and the end of the action (`COMPONENT_ACTION_RESOLVED` / `COMPONENT_ACTION_FAILED`, the failed-action
//! withholding, `finish_action`).
//!
//! The string functions this mirrors are the arms of `Game::apply_choice`'s `Phase::Action` branch (`faction|`, `component|leader|`,
//! `component|tech|`, `action_card|`, relics and Enigmatic Device cards): `play_*`/`perform_*`, then `announce_staged_destructions`,
//! `announce_staged_cards`, `settle_extreme_duress`, the verdict event and `finish_action`. Each route of [`super::action`] is a body that
//! returns whether the action resolved; [`conclude`] is everything after it.

use super::end::run_finish;
use super::inline::{emit_typed, sync_clock, Flow};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::fx;
use crate::state::{Code, EventKind, Key, Payload, PlainArg, PlainKind, PlainRec, Val};

/// Extreme Duress settles when the target takes an action (`Game::settle_extreme_duress`). A strategic action is the one case the card does
/// not punish: the duress lifts quietly. Every other action triggers the punishment: the seat's trade goods go to the holder, every action card
/// left in its hand is discarded (and announced, one by one: a Reverse Engineer may take a component action). A pass neither triggers nor lifts it.
///
/// `Flow::Halt` when a discard's reaction window found the tape dry.
pub fn settle_extreme_duress(sx: &mut Sx<'_>, who: Seat, strategic: bool) -> Flow {
    let Some(by) = sx.state.players().by_seat(who).and_then(|p| p.econ.duress_by) else { return Flow::Done };
    if strategic {
        if let Some(p) = sx.state.players_mut().by_seat_mut(who) {
            p.econ.duress_by = None;
        }
        return Flow::Done;
    }
    let (goods, discarded) = {
        let Some(p) = sx.state.players_mut().by_seat_mut(who) else { return Flow::Done };
        let goods = i32::from(p.econ.trade_goods);
        p.econ.trade_goods = 0;
        // The cards are named one by one below, so they are taken out, not cleared away.
        let discarded: Vec<_> = p.cards.hand.as_slice().to_vec();
        p.cards.hand.clear();
        p.econ.duress_by = None;
        (goods, discarded)
    };
    if let Some(holder) = sx.state.players_mut().by_seat_mut(by) {
        holder.econ.trade_goods = holder.econ.trade_goods.saturating_add(i16::try_from(goods).unwrap_or(0)); // mutants:skip: a seat holds an i16 of goods, the default is unreachable
    }
    let source = sx.table.codes.turn.extreme_duress;
    fx::stage_gain(sx.state, by, goods, source);
    sx.plain(PlainRec::one(PlainKind::ExtremeDuress, PlainArg::Seat(who)));
    for card in discarded {
        sx.state.cards_mut().discarded_action.edit().push(card);
        sync_clock(sx);
        let payload = Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Card, Val::Action(card));
        if emit_typed(sx, EventKind::ActionCardDiscarded, payload) == Flow::Halt {
            return Flow::Halt;
        }
    }
    Flow::Done
}

/// A cause no staged destruction carries: `drain_staged` renames the destructions of a retreat, and a component action has none.
const NO_RETREAT: (Code, Code) = (Code(u16::MAX), Code(u16::MAX));

/// `announce_staged_destructions` and `announce_staged_cards` after a body: the ground forces, the staged events and the ships an effect destroyed
/// without a resolver in reach, then the card moves an effect staged.
pub fn announce_staged(sx: &mut Sx<'_>) -> Flow {
    // `announce_staged_ground_events`: the ground forces and the planet control, then the events resolver-less effects staged (`TRADE_GOODS_GAINED`,
    // `UNITS_PRODUCED`, `STRATEGY_TOKEN_SPENT`: the string's `supply::try_flush_staged_events`), the same two steps the turn prelude announces.
    if super::prelude::announce_staged(sx) == Flow::Halt {
        return Flow::Halt;
    }
    if !sx.state.transient().combat.pending_destructions.is_empty() {
        sync_clock(sx);
        crate::rules::combat::assign::drain_staged(sx, true, NO_RETREAT);
        if super::inline::halted(sx) {
            return Flow::Halt;
        }
    }
    crate::driver::windows::status_finish::announce_staged_cards(sx)
}

/// How a body ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The body resolved: `COMPONENT_ACTION_RESOLVED`, then the end of the action.
    Resolved,
    /// The body could not resolve: `COMPONENT_ACTION_FAILED`, and (`withhold`) the option is not offered again this turn.
    Failed { withhold: bool },
}

/// The verdict of a body that reports whether it resolved. A failed body (withheld from the menu for the rest of the turn) is the defensive twin of the
/// string's: the menu offers an option only when its body can resolve (`leaders::can_resolve_action`, the windows' own probes), so no position
/// reaches the `Failed` arm through the menu.
// mutants:skip
#[must_use]
pub const fn verdict(resolved: bool) -> Verdict {
    if resolved { Verdict::Resolved } else { Verdict::Failed { withhold: true } }
}

/// [`conclude`] for the faction and leader arms, which announce what the body staged.
pub fn conclude_announced(sx: &mut Sx<'_>, who: Seat, key: u32, verdict: Verdict) -> Flow {
    conclude(sx, who, key, verdict, true)
}

/// [`conclude`] for the technology, relic, device and action-card arms, which announce nothing after the body.
pub fn conclude_quiet(sx: &mut Sx<'_>, who: Seat, key: u32, verdict: Verdict) -> Flow {
    conclude(sx, who, key, verdict, false) // mutants:skip: no quiet arm stages a destruction or a card move yet; the flag is the string's arm table
}

/// Everything after a body (`Game::apply_choice`): what the body staged is announced (`announce`: the faction and leader arms do it, the
/// technology, relic, device and action-card arms do not), Extreme Duress settles, the verdict is told, and a
/// resolved action ends (`finish_action`). `key` is the option's key (the failed-action withholding).
pub fn conclude(sx: &mut Sx<'_>, who: Seat, key: u32, verdict: Verdict, announce: bool) -> Flow {
    if announce && announce_staged(sx) == Flow::Halt || settle_extreme_duress(sx, who, false) == Flow::Halt {
        return Flow::Halt;
    }
    match verdict {
        Verdict::Resolved => {
            sx.plain(PlainRec::bare(PlainKind::ComponentActionResolved));
            run_finish(sx)
        }
        Verdict::Failed { withhold } => {
            sx.plain(PlainRec::bare(PlainKind::ComponentActionFailed));
            if withhold {
                sx.state.pending_mut().note_failed_key(key);
            }
            Flow::Done
        }
    }
}

/// An action card played as the action (`action_cards::perform`, the `action_card|` arm): the card leaves the hand (or is purged off the salvage
/// yard), is announced and resolved inside the step, and the verdict follows. A play that was cancelled leaves the turn open (22.4); the card
/// is gone either way (22.3).
pub fn play_card(sx: &mut Sx<'_>, who: Seat, key: u32, card: crate::generated::ActionCard, from_hand: Option<u8>) -> Flow {
    let taken = match from_hand {
        Some(index) => {
            let Some(p) = sx.state.players_mut().by_seat_mut(who) else { return super::inline::gap_flow(sx, crate::kernel::turn_codes::TurnGap::ReplayDiverged) };
            if p.cards.hand.as_slice().get(usize::from(index)) != Some(&card) {
                return super::inline::gap_flow(sx, crate::kernel::turn_codes::TurnGap::ReplayDiverged);
            }
            p.cards.hand.remove(usize::from(index));
            true
        }
        None => crate::rules::legendary::purge_salvaged(sx.state, who, card),
    };
    if !taken {
        return super::inline::gap_flow(sx, crate::kernel::turn_codes::TurnGap::ReplayDiverged);
    }
    let mark = fx::journal_mark(sx.resolver);
    let played = match crate::rules::action_cards::announce::play_inline(sx, who, card) {
        Ok(played) => played,
        Err(_) => {
            if !super::inline::halted(sx) {
                super::inline::gap(sx, crate::kernel::turn_codes::TurnGap::EmissionRefused);
            }
            return Flow::Halt;
        }
    };
    if super::inline::halted(sx) {
        return Flow::Halt;
    }
    // The string mirrors the timing log of the play into its event queue when the call returns.
    fx::mirror_journal(sx.state, sx.resolver, mark);
    conclude_quiet(sx, who, key, if played { Verdict::Resolved } else { Verdict::Failed { withhold: false } })
}

/// The events a window staged for the announcement its string twin makes at once (`spend_strategy_token_announced`: Orbital Drop's token), announced
/// now, in the order staged, through the reaction windows (a window they open may stage more).
pub fn flush_staged_events(sx: &mut Sx<'_>) -> Flow {
    while fx::has_staged(sx.state) {
        for rec in fx::drain_staged(sx.state) {
            if super::inline::halted(sx) {
                return Flow::Halt;
            }
            if sx.emit_mirrored_bare(rec).is_err() {
                if !super::inline::halted(sx) {
                    super::inline::gap(sx, crate::kernel::turn_codes::TurnGap::EmissionRefused);
                }
                return Flow::Halt;
            }
            // What the windows of that event staged (Pillage's gain) is the string's nested emission: announced inside the event, so before the card moves
            // that follow it (`Live::emit_typed` ends with them): the Nekro agent's discard follows the gain AND the pillage of the gain.
            if super::prelude::announce_staged(sx) == Flow::Halt || crate::driver::windows::status_finish::announce_staged_cards(sx) == Flow::Halt { // mutants:skip: a halted step is rolled back by the driver whatever the loop does after the halt
                return Flow::Halt;
            }
        }
    }
    if super::inline::halted(sx) { Flow::Halt } else { Flow::Done }
}

/// The events a window staged, announced at once the way the string does for the spends and gains it announces itself (`hooks_economy::emit`,
/// `supply::gain_trade_goods_staged` with a resolver in reach), and only while a seated faction has a module: the string's `stage_event` is a no-op
/// without one, so the staged rows are dropped.
pub fn announce_events(sx: &mut Sx<'_>) -> Flow {
    if sx.table.facts.staging {
        flush_staged_events(sx)
    } else {
        let _ = fx::drain_staged(sx.state);
        Flow::Done
    }
}
