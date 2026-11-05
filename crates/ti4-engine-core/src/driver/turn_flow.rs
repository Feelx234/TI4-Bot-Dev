//! The turn flow of the action phase (package D1b-5 and the Action -> Status step of D1b-6): who has the next turn, what
//! a turn that ends leaves behind, and the three events that follow it. A function of the position only; the hooks that run
//! between the steps (`technology::end_turn`, Minister of War, the fleet sweep, the `TURN_PASSED` reaction windows) stay with
//! the string engine and are lent: the dual counts a step in which one of them changed the position.
//!
//! The string functions this mirrors: `phase::{begin_action_turn, advance_turn, advance_phase (Action arm)}` and the parts of
//! `Game::advance_turn_inner` that are choice-free (`game.rs`). L5: composes `rules::trade`, `rules::diplomacy` and
//! `query::seats`, which no area may import from another.

use crate::containers::SmallList;
use crate::ids::Seat;
use crate::query::seats::initiative_order;
use crate::generated::StrategyCard;
use crate::rules::diplomacy_book::PromiseError;
use crate::rules::{diplomacy, diplomacy_book, leaders, strategy_flow, trade};
use crate::state::{CoreState, Phase, TransientFlags};

/// `phase::begin_action_turn`: the next turn of the phase, for `seat`.
pub fn begin_action_turn(core: &mut CoreState, seat: Seat) {
    let meta = core.meta_mut();
    meta.action_turn += 1;
    meta.active = Some(seat);
    meta.turn_seq += 1;
}

/// `GameState::all_passed`.
#[must_use]
// mutants:skip: `advance_turn` would walk the order and find nobody all the same; the shortcut changes no result.
pub fn all_passed(core: &CoreState) -> bool {
    core.players().iter().all(|p| p.econ.passed)
}

/// `phase::advance_turn`: the next seat in initiative order that has not passed, or `None` when everyone has (the turn then
/// belongs to nobody). Both per-turn tallies of the trade slice are restored first: one transaction per neighbour and one
/// contact per seat each turn.
pub fn advance_turn(core: &mut CoreState) -> Option<Seat> {
    trade::clear_turn_transactions(core);
    diplomacy::clear_turn_initiations(core);
    if all_passed(core) {
        core.meta_mut().active = None;
        return None;
    }
    let order = initiative_order(core);
    if order.is_empty() {
        core.meta_mut().active = None;
        return None;
    }
    // Initiative order is not seating order: advance by the current seat's place in it.
    let order = order.as_slice();
    let start = core.meta().active.and_then(|current| order.iter().position(|s| *s == current));
    for step in 1..=order.len() {
        let index = start.map_or(step - 1, |start| (start + step) % order.len());
        let candidate = order[index];
        if core.players().by_seat(candidate).is_some_and(|p| !p.econ.passed) {
            begin_action_turn(core, candidate);
            return Some(candidate);
        }
    }
    core.meta_mut().active = None;
    None
}

/// `phase::advance_phase`, Action arm: the status phase begins and the turn belongs to nobody.
pub fn action_phase_ended(core: &mut CoreState) {
    let meta = core.meta_mut();
    meta.phase = Phase::Status;
    meta.active = None;
}

/// `phase::begin_next_round`: the strategy phase of the next round, with `cards` laid out on the mat. The effects that end
/// with the round run first, while the round number still names it (Darktalon Treilla's hero, due promises and signals,
/// the relationships' decay); then the counters, the mat and the round's transaction tallies.
///
/// # Errors
/// The settlement of a promise that cannot be settled (a broken book); the position is then partly changed, as the string's.
pub fn begin_next_round(core: &mut CoreState, cards: &[StrategyCard]) -> Result<(), PromiseError> {
    core.meta_mut().phase = Phase::Strategy;
    leaders::end_of_round(core);
    let round = core.meta().round;
    diplomacy_book::settle_deadlines(core, round)?;
    diplomacy::decay_relationships(core);
    let meta = core.meta_mut();
    meta.round += 1;
    meta.action_turn = 0;
    meta.active = None;
    strategy_flow::lay_out_mat(core, cards);
    trade::reset_round(core);
    Ok(())
}

/// `phase::advance_phase`, Strategy arm, the part that is not a faction hook: Political Stability is spent (the retention
/// covered the strategy phase just played).
pub fn reset_stability(core: &mut CoreState) {
    let seats: SmallList<Seat, 8> = core.players().iter().map(|p| p.seat).collect();
    for seat in seats.as_slice() {
        if let Some(p) = core.players_mut().by_seat_mut(*seat) {
            p.econ.stability = false;
        }
    }
}

/// `phase::advance_phase`, Strategy arm, after the faction hooks have placed their initiative tokens: the action phase begins
/// with nobody active. Returns the seat first in initiative order, whose turn the caller then begins
/// ([`begin_action_turn`]); `None` for a table without seats.
pub fn strategy_phase_flip(core: &mut CoreState) -> Option<Seat> {
    let first = initiative_order(core).as_slice().first().copied();
    let meta = core.meta_mut();
    meta.phase = Phase::Action;
    meta.action_turn = 0;
    meta.active = None;
    first
}

/// The hooks between the steps of `Game::advance_turn_inner` that the string runs as bodies of its own and the dual counts as
/// `turn::hooks [bail: <name>]` (a census of the string driver: the core runs each of them natively as a job of `driver::turn::end`,
/// see `docs/ZEROBAIL_RESUME.md`), in the order the string runs them. `fleet::enforce_everywhere` between them is a core window
/// (`driver::windows::enforce_all`); the `TURN_PASSED` reaction windows are the A5 shim's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    /// Minister of War: discard the law to retrieve a command token and act again (before the turn moves on).
    MinisterOfWar,
    /// `technology::end_turn`: the technologies that read "at the end of your turn".
    TechnologyEndTurn,
    /// `legendary::end_turn`: the legendary planets that read "at the end of your turn".
    LegendaryEndTurn,
}

/// The hooks in the order `Game::advance_turn_inner` runs them.
pub const HOOKS: [Hook; 3] = [Hook::MinisterOfWar, Hook::TechnologyEndTurn, Hook::LegendaryEndTurn];

/// `faction_techs::return_spec_ops` (the turn prelude): "at the start of your next turn, place each unit that is on this card
/// on a planet you control in your home system", on the first such planet (by id); with none controlled they wait on the card.
/// `infantry` is the unit placed (`sol_infantry2`, the adapter's to name). Returns how many were placed.
pub fn return_spec_ops(core: &mut CoreState, seat: Seat, infantry: crate::generated::Unit) -> u32 {
    let Some(p) = core.players().by_seat(seat) else { return 0 };
    let waiting = p.misc.spec_ops_card;
    let Some(home) = p.home_system else { return 0 };
    if waiting == 0 {
        return 0;
    }
    let mut here: Vec<_> = crate::query::board::controlled_planets(core, seat).as_slice().iter().filter(|(system, _)| *system == home).map(|(_, planet)| *planet).collect();
    here.sort_unstable();
    let Some(planet) = here.first().copied() else { return 0 };
    for _ in 0..waiting {
        core.board_mut().planet_mut(planet).units.push(crate::state::Piece::new(infantry, seat));
    }
    if let Some(p) = core.players_mut().by_seat_mut(seat) {
        p.misc.spec_ops_card = 0;
    }
    waiting
}

/// The turn prelude has run for the turn in progress (`Game::prepared_turn_seq`).
pub fn mark_prepared(core: &mut CoreState) {
    let turn = core.meta().turn_seq;
    core.pending_mut().prepared_turn_seq = Some(turn);
}

/// What the end of an action does with the turn (`Game::finish_action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// The seat is offered the closing menu (`TURN_CLOSING`): something besides "end your turn" can still be done.
    Closing,
    /// The turn passes on (`advance_turn`).
    Advance,
}

/// `Game::finish_action` after `ACTION_COMPLETED`: in the action phase, a turn that is not retained counts the action and
/// offers the closing menu when it holds more than "end your turn" (`closing_len` options). Master Plan's additional action
/// and every other phase pass on at once. Returns what the string does next; the position carries the seat in the menu.
pub fn finish_action(core: &mut CoreState, closing_len: usize) -> Finish {
    let Some(active) = core.meta().active else { return Finish::Advance };
    let retained = core.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION);
    if core.meta().phase == Phase::Action && !retained {
        let p = core.pending_mut();
        p.actions_this_turn = p.actions_this_turn.saturating_add(1);
        p.turn_closing = Some(active);
        if closing_len > 1 {
            return Finish::Closing;
        }
        p.turn_closing = None;
    }
    Finish::Advance
}

/// The top of `Game::advance_turn_inner`: an additional action (Master Plan and friends) keeps the turn. Returns whether the
/// turn is retained; the flag is spent.
pub fn take_retention(core: &mut CoreState) -> bool {
    let flags = &mut core.transient_mut().misc.flags;
    let retained = flags.has(TransientFlags::ADDITIONAL_ACTION);
    if retained {
        flags.clear(TransientFlags::ADDITIONAL_ACTION);
    }
    retained
}

/// The scratch of a turn that really ends (`Game::advance_turn_inner` after the retention checks): a new turn is a new action
/// for the negotiation limit, and a Black Market or Expedition marker outlives no turn.
pub fn end_turn_scratch(core: &mut CoreState) {
    trade::clear_action_negotiations(core);
    let flags = &mut core.transient_mut().misc.flags;
    flags.clear(TransientFlags::BLACK_MARKET);
    flags.clear(TransientFlags::EXPEDITION_CLAIMED);
}

/// `Game::advance_turn_inner` forgets the component actions that failed this turn (`Pending::failed_component_actions`). A separate
/// step from [`end_turn_scratch`]: the dual lends that one without the `Pending` slice, the live turn-end composer calls both.
pub fn forget_failed_components(core: &mut CoreState) {
    core.pending_mut().failed_component_actions.clear();
    core.pending_mut().failed_keys.clear();
}

/// What the end of a turn announces, in the order the string pushes it. `Passed` is the plain `TURN_PASSED` and the typed event
/// of the same name; `Skipped` and `Puppet` are the plain lines `TURN_SKIPPED:{seat}` and `TURN_PUPPET:{seat}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announce {
    Passed(Seat),
    Skipped(Seat),
    Puppet(Seat),
}

/// The choice-free tail of `Game::advance_turn_inner`, from `phase::advance_turn` on: pass the turn, announce that `ended`'s
/// turn ended, skip the seat Crisis names, and give a Puppets on a String grant back to the passer.
///
/// `ended` is the seat whose turn it was (`None` when nobody was active: nothing is announced). The last turn of the phase
/// still ends (its windows open) although there is no next turn, unless the passer holds a puppet grant.
pub fn pass_on(core: &mut CoreState, ended: Option<Seat>) -> SmallList<Announce, 4> {
    let mut out = SmallList::new();
    let next = advance_turn(core);
    let puppet = core.transient().misc.flags.has(TransientFlags::PUPPET_ACTION);
    if next.is_none() && !puppet {
        if let Some(ended) = ended {
            out.push(Announce::Passed(ended));
        }
        return out;
    }
    let Some(ended) = ended else { return out };
    out.push(Announce::Passed(ended));
    if core.transient().misc.flags.has(TransientFlags::SKIP_NEXT_TURN) {
        core.transient_mut().misc.flags.clear(TransientFlags::SKIP_NEXT_TURN);
        if let Some(skipped) = core.meta().active {
            out.push(Announce::Skipped(skipped));
            let _ = advance_turn(core);
        }
    }
    // A fresh turn for the passer (new turn counter, start-of-turn hooks run again), and the seat stays passed.
    if core.transient().misc.flags.has(TransientFlags::PUPPET_ACTION) {
        core.transient_mut().misc.flags.clear(TransientFlags::PUPPET_ACTION);
        begin_action_turn(core, ended);
        out.push(Announce::Puppet(ended));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Faction, StrategyCard};
    use crate::state::{Player, Players};

    /// Three seats, initiative by seat unless a test changes it.
    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec((0..3).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect());
        s.seats = crate::state::Seats::new(&[Seat(0), Seat(1), Seat(2)], &[Seat(0), Seat(1), Seat(2)], 3);
        for (i, c) in StrategyCard::ALL[..3].iter().enumerate() {
            s.strategy.initiative.insert(*c, i as i32 + 1);
            s.players.edit(i).strat.push(*c);
        }
        s.meta.active = Some(Seat(0));
        s.meta.phase = Phase::Action;
        s
    }

    fn pass(s: &mut CoreState, seat: u8) {
        s.players.edit(usize::from(seat)).econ.passed = true;
    }

    #[test]
    fn the_turn_goes_to_the_next_seat_in_initiative_order_and_counts_up() {
        let mut s = table();
        assert_eq!(advance_turn(&mut s), Some(Seat(1)));
        assert_eq!((s.meta.active, s.meta.turn_seq, s.meta.action_turn), (Some(Seat(1)), 1, 1));
        assert_eq!(advance_turn(&mut s), Some(Seat(2)));
        assert_eq!(advance_turn(&mut s), Some(Seat(0)), "wraps round");
        assert_eq!((s.meta.turn_seq, s.meta.action_turn), (3, 3));
    }

    #[test]
    fn a_seat_that_passed_is_skipped_and_when_all_passed_nobody_has_the_turn() {
        let mut s = table();
        pass(&mut s, 1);
        assert_eq!(advance_turn(&mut s), Some(Seat(2)));
        pass(&mut s, 2);
        assert_eq!(advance_turn(&mut s), Some(Seat(0)), "the only seat left keeps coming back");
        pass(&mut s, 0);
        let seq = s.meta.turn_seq;
        assert_eq!(advance_turn(&mut s), None);
        assert_eq!((s.meta.active, s.meta.turn_seq), (None, seq), "no turn begins");
    }

    #[test]
    fn initiative_order_decides_not_seating() {
        let mut s = table();
        // Seat 2 holds the lowest initiative now.
        s.strategy.initiative_overrides.insert(Seat(2), 0);
        s.meta.active = Some(Seat(2));
        assert_eq!(advance_turn(&mut s), Some(Seat(0)), "after the first in initiative comes the second");
        s.meta.active = None;
        assert_eq!(advance_turn(&mut s), Some(Seat(2)), "no active seat: the front of the order");
    }

    #[test]
    fn the_per_turn_tallies_are_restored_even_when_the_turn_passes_to_nobody() {
        let mut s = table();
        s.trade.transactions_this_turn.insert(Seat(0), Default::default());
        for i in 0..3 {
            pass(&mut s, i);
        }
        assert_eq!(advance_turn(&mut s), None);
        assert!(s.trade.transactions_this_turn.is_empty());
    }

    #[test]
    fn a_retained_turn_spends_the_flag_and_changes_nothing_else() {
        let mut s = table();
        assert!(!take_retention(&mut s));
        s.transient.misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
        assert!(take_retention(&mut s));
        assert!(!s.transient.misc.flags.has(TransientFlags::ADDITIONAL_ACTION));
        assert_eq!((s.meta.active, s.meta.turn_seq), (Some(Seat(0)), 0));
    }

    #[test]
    fn the_scratch_of_an_ended_turn_clears_the_markers_that_outlive_no_turn() {
        let mut s = table();
        s.transient.misc.flags.set(TransientFlags::BLACK_MARKET);
        s.transient.misc.flags.set(TransientFlags::EXPEDITION_CLAIMED);
        s.transient.misc.flags.set(TransientFlags::SKIP_NEXT_TURN);
        end_turn_scratch(&mut s);
        assert_eq!(s.transient.misc.flags.0, TransientFlags::SKIP_NEXT_TURN, "only the two markers go");
    }

    #[test]
    fn passing_on_announces_the_end_of_the_turn_that_ended() {
        let mut s = table();
        let out = pass_on(&mut s, Some(Seat(0)));
        assert_eq!(out.as_slice(), &[Announce::Passed(Seat(0))]);
        assert_eq!(s.meta.active, Some(Seat(1)));
        // The last turn of the phase still ends.
        for i in 0..3 {
            pass(&mut s, i);
        }
        let out = pass_on(&mut s, Some(Seat(1)));
        assert_eq!((out.as_slice(), s.meta.active), (&[Announce::Passed(Seat(1))][..], None));
        assert!(pass_on(&mut s, None).is_empty(), "no one was active: nothing is announced");
    }

    #[test]
    fn crisis_skips_the_seat_the_turn_arrived_at() {
        let mut s = table();
        s.transient.misc.flags.set(TransientFlags::SKIP_NEXT_TURN);
        let out = pass_on(&mut s, Some(Seat(0)));
        assert_eq!(out.as_slice(), &[Announce::Passed(Seat(0)), Announce::Skipped(Seat(1))]);
        assert_eq!(s.meta.active, Some(Seat(2)));
        assert!(!s.transient.misc.flags.has(TransientFlags::SKIP_NEXT_TURN));
    }

    #[test]
    fn a_puppet_grant_gives_the_passer_a_fresh_turn_while_it_stays_passed() {
        let mut s = table();
        pass(&mut s, 0);
        pass(&mut s, 1);
        pass(&mut s, 2);
        s.transient.misc.flags.set(TransientFlags::PUPPET_ACTION);
        let seq = s.meta.turn_seq;
        let out = pass_on(&mut s, Some(Seat(1)));
        assert_eq!(out.as_slice(), &[Announce::Passed(Seat(1)), Announce::Puppet(Seat(1))]);
        assert_eq!((s.meta.active, s.meta.turn_seq), (Some(Seat(1)), seq + 1));
        assert!(s.players.get(1).econ.passed, "the grant is one action, not a return from pass");
    }

    #[test]
    fn the_next_round_begins_in_the_strategy_phase_with_the_mat_laid_out_and_the_tallies_reset() {
        let mut s = table();
        s.meta.round = 3;
        s.meta.action_turn = 7;
        s.meta.phase = Phase::Status;
        s.trade.negotiations_this_round.insert(Seat(0), 2);
        let cards = &StrategyCard::ALL[..4];
        // A seat under Political Stability keeps its card: it is not on the mat.
        s.players.edit(1).econ.stability = true;
        begin_next_round(&mut s, cards).unwrap();
        assert_eq!((s.meta.phase, s.meta.round, s.meta.action_turn, s.meta.active), (Phase::Strategy, 4, 0, None));
        assert_eq!(s.strategy.unclaimed.as_slice(), &[cards[0], cards[2], cards[3]][..], "the card of the kept seat stays off the mat");
        assert!(s.trade.negotiations_this_round.is_empty());
    }

    #[test]
    fn an_action_opens_the_closing_menu_only_when_more_than_ending_the_turn_is_offered() {
        let mut s = table();
        assert_eq!(finish_action(&mut s, 1), Finish::Advance);
        assert_eq!((s.pending.turn_closing, s.pending.actions_this_turn), (None, 1), "counted, nothing to close");
        assert_eq!(finish_action(&mut s, 2), Finish::Closing);
        assert_eq!((s.pending.turn_closing, s.pending.actions_this_turn), (Some(Seat(0)), 2));
    }

    #[test]
    fn a_failed_component_action_is_remembered_once_and_forgotten_by_the_turn_end_step() {
        use crate::state::Code;
        let mut s = table();
        s.pending.note_failed_component(Code(7));
        s.pending.note_failed_component(Code(7));
        s.pending.note_failed_component(Code(9));
        assert_eq!(s.pending.failed_component_actions.as_slice(), [Code(7), Code(9)]);
        forget_failed_components(&mut s);
        assert!(s.pending.failed_component_actions.is_empty());
    }

    #[test]
    fn a_retained_turn_or_another_phase_does_not_count_the_action_or_close() {
        let mut s = table();
        s.transient.misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
        assert_eq!(finish_action(&mut s, 5), Finish::Advance);
        assert_eq!((s.pending.turn_closing, s.pending.actions_this_turn), (None, 0));
        s.transient.misc.flags.clear(TransientFlags::ADDITIONAL_ACTION);
        s.meta.phase = Phase::Agenda;
        assert_eq!(finish_action(&mut s, 5), Finish::Advance);
        assert_eq!(s.pending.actions_this_turn, 0);
        s.meta.phase = Phase::Action;
        s.meta.active = None;
        assert_eq!(finish_action(&mut s, 5), Finish::Advance, "no one is active");
        assert_eq!(s.pending.actions_this_turn, 0);
    }

    #[test]
    fn the_action_phase_ends_into_the_status_phase_with_no_active_seat() {
        let mut s = table();
        action_phase_ended(&mut s);
        assert_eq!((s.meta.phase, s.meta.active), (Phase::Status, None));
    }

    #[test]
    fn the_lent_hooks_of_a_turn_end_run_in_the_order_of_the_string() {
        assert_eq!(HOOKS, [Hook::MinisterOfWar, Hook::TechnologyEndTurn, Hook::LegendaryEndTurn]);
    }

    /// A planet with a system, and that system.
    fn a_planet_with_a_system() -> (crate::generated::Planet, crate::generated::System) {
        crate::generated::Planet::ALL.iter().find_map(|p| p.data().system.map(|s| (*p, s))).expect("a placed planet")
    }

    #[test]
    fn spec_ops_wait_on_the_card_when_nothing_can_take_them() {
        let unit = crate::generated::Unit::ALL[0];
        let (planet, home) = a_planet_with_a_system();
        let mut s = table();
        s.players.edit(0).misc.spec_ops_card = 2;
        assert_eq!(return_spec_ops(&mut s, Seat(0), unit), 0, "no home system");
        s.players.edit(0).home_system = Some(home);
        assert_eq!(return_spec_ops(&mut s, Seat(0), unit), 0, "no planet controlled there");
        assert_eq!(s.players.edit(0).misc.spec_ops_card, 2, "they wait on the card");
        s.players.edit(0).misc.spec_ops_card = 0;
        s.board.planet_mut(planet).control = Some(Seat(0));
        assert_eq!(return_spec_ops(&mut s, Seat(0), unit), 0, "nothing waiting");
        assert!(s.board.planet_mut(planet).units.is_empty());
        assert_eq!(return_spec_ops(&mut s, Seat(7), unit), 0, "a seat that is not seated");
    }

    #[test]
    fn spec_ops_land_on_the_first_controlled_planet_of_the_home_system() {
        let unit = crate::generated::Unit::ALL[0];
        let (planet, home) = a_planet_with_a_system();
        let mut s = table();
        s.players.edit(1).home_system = Some(home);
        s.players.edit(1).misc.spec_ops_card = 3;
        s.board.planet_mut(planet).control = Some(Seat(1));
        assert_eq!(return_spec_ops(&mut s, Seat(1), unit), 3);
        assert_eq!(s.board.planet_mut(planet).units.len(), 3);
        assert_eq!(s.players.edit(1).misc.spec_ops_card, 0);
        // A single unit waiting lands too (the card is not read as a count of more than one).
        s.players.edit(1).misc.spec_ops_card = 1;
        assert_eq!(return_spec_ops(&mut s, Seat(1), unit), 1);
        assert_eq!(s.board.planet_mut(planet).units.len(), 4);
        mark_prepared(&mut s);
        assert_eq!(s.pending.prepared_turn_seq, Some(s.meta.turn_seq));
    }
}
