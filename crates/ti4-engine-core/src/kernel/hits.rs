//! The "before a hit would be assigned" window shared by every batch of hits (string
//! `combat::open_hits_window`, `cancellable_hits`, `spend_cancellations`; package B2, usable by B1's
//! `HITS_TO_ASSIGN`).
//!
//! A card played in the window (Titans' Tellurian, Shields Holding) grants cancellations to the victim for the
//! current combat round (`Player.combat_timers.cancel_hits_round`, scoped to `meta.combat_round_seq`). Only the
//! growth of the pool while the event resolves is spent by the window, capped at the hits of the batch; an
//! older grant stays for the assignment that spends it.

use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::fx;
use crate::state::{CoreState, EventKind, Payload};

/// `combat::cancellable_hits`: the hits `who` may still cancel in the current combat round.
#[must_use]
pub fn cancellable(s: &CoreState, who: Seat) -> u32 {
    s.players()
        .by_seat(who)
        .and_then(|p| p.combat_timers.cancel_hits_round)
        .filter(|(round, _)| *round == s.meta().combat_round_seq)
        .map_or(0, |(_, hits)| hits)
}

/// `combat::grant_hit_cancellation`.
pub fn grant(s: &mut CoreState, who: Seat, hits: u32) {
    let round = s.meta().combat_round_seq;
    let held = cancellable(s, who);
    if let Some(p) = s.players_mut().by_seat_mut(who) {
        p.combat_timers.cancel_hits_round = Some((round, held + hits));
    }
}

/// `combat::spend_cancellations`: spend up to `wanted` of `who`'s cancellations, returning how many.
pub fn spend(s: &mut CoreState, who: Seat, wanted: u32) -> u32 {
    let available = cancellable(s, who);
    let spent = available.min(wanted);
    if spent > 0 {
        let round = s.meta().combat_round_seq;
        if let Some(p) = s.players_mut().by_seat_mut(who) {
            p.combat_timers.cancel_hits_round = Some((round, available - spent));
        }
    }
    spent
}

/// `combat::open_hits_window`: announce `kind` (with `payload`) before `hits` hits land on `victim`, and spend what
/// a card played in the window cancelled. Returns the hits that remain. Nothing is announced for no hits. Errors of
/// the emission are ignored: the hits they report are going to happen.
pub fn open_window(sx: &mut Sx<'_>, kind: EventKind, payload: Payload, victim: Seat, hits: u32) -> u32 {
    if hits == 0 {
        return 0;
    }
    let before = cancellable(sx.state, victim);
    let _ = sx.emit(fx::event(kind, payload));
    let gained = cancellable(sx.state, victim).saturating_sub(before);
    hits - spend(sx.state, victim, gained.min(hits))
}

/// Announce every staged event through the window's resolver in the order staged (the string `flush_events`:
/// cancels and errors are ignored, the changes they report have happened).
pub fn flush_staged(sx: &mut Sx<'_>) {
    for rec in fx::drain_staged(sx.state) {
        let _ = sx.emit(rec);
    }
}
