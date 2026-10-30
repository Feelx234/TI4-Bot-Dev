//! `Area::Driver` (package D1b-5, D1b-6): the turn flow of the action phase as `ti4_engine_core::driver::turn_flow` functions
//! behind `dual::write`. Each wrapper holds both sides: the string write it replaces and the core rule. The core half answers
//! `None` (the string runs) when the position is outside the tables (more than 16 owners). The hooks between the steps of
//! `Game::advance_turn_inner` stay with the string and are lent; [`check_pass_on`] compares the choice-free tail after the
//! string has run it.

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::dual::{Area, write};

/// `phase::begin_action_turn`.
pub fn begin_action_turn(state: &mut GameState, player: &PlayerId, string: impl Fn(&mut GameState, &PlayerId)) {
    write(Area::Driver, state, "turn::begin_action_turn", |s| string(s, player), |s| real::begin_action_turn(s, player));
}

/// `phase::advance_turn`.
pub fn advance_turn(state: &mut GameState, string: impl Fn(&mut GameState) -> Option<PlayerId>) -> Option<PlayerId> {
    write(Area::Driver, state, "turn::advance_turn", string, real::advance_turn)
}

/// The Action arm of `phase::advance_phase`.
pub fn action_phase_ended(state: &mut GameState) {
    write(
        Area::Driver,
        state,
        "turn::action_phase_ended",
        |s| {
            s.phase = ti4_model::state::Phase::Status;
            s.active = None;
        },
        real::action_phase_ended,
    );
}

/// `phase::begin_next_round` (the caller supplies the strategy cards of the round). The core half bows out when a card id is
/// unknown to the tables or the promise book cannot settle (`PromiseError` is the string's to report).
pub fn begin_next_round(state: &mut GameState, cards: &[ti4_model::id::StrategyCardId], string: impl Fn(&mut GameState)) {
    write(
        Area::Driver,
        state,
        "turn::begin_next_round",
        |s| {
            string(s);
        },
        |s| real::begin_next_round(s, cards),
    );
}

/// The retention check at the top of `Game::advance_turn_inner` (and its two repeats): an additional action keeps the
/// turn; the flag is spent.
pub fn take_retention(state: &mut GameState) -> bool {
    write(
        Area::Driver,
        state,
        "turn::take_retention",
        |s| {
            let kept = s.transient_flags.has(ti4_model::state::TransientFlags::ADDITIONAL_ACTION);
            s.transient_flags.clear(ti4_model::state::TransientFlags::ADDITIONAL_ACTION);
            kept
        },
        real::take_retention,
    )
}

/// The scratch of a turn that really ends (`Game::advance_turn_inner`, after the retention checks).
pub fn end_turn_scratch(state: &mut GameState, string: impl Fn(&mut GameState)) {
    write(Area::Driver, state, "turn::end_turn_scratch", |s| { string(s); }, real::end_turn_scratch);
}

/// What the tail of an ended turn announced, as the lines the string pushes: `PASSED:{seat}`, `SKIPPED:{seat}`,
/// `PUPPET:{seat}` (see `Announce`).
pub type Announced = Vec<String>;

/// Compare the choice-free tail of `Game::advance_turn_inner` (`before`: the position when it began, `after`: the one it
/// left, `announced`: what the string announced) with the core's `pass_on`. A step in which a reaction window or another
/// hook changed the turn counters elsewhere is the string's to answer: only the turn state is compared. `None` when the
/// dual is off or the position is outside the tables.
pub fn check_pass_on(before: &GameState, after: &GameState, ended: Option<&PlayerId>, announced: &Announced) {
    if crate::dual::backend(Area::Driver) != crate::dual::Backend::Dual {
        return;
    }
    if announced.iter().any(|a| a == "TAINTED") {
        // A reaction window of `TURN_PASSED` changed the turn (Crisis): the core's tail ran on the position before it.
        crate::dual::note_bail("turn::pass_on", "a reaction window of TURN_PASSED changed the turn");
        return;
    }
    let Some(core) = real::pass_on(before, ended) else {
        crate::dual::note_bail("turn::pass_on", "the position is outside the tables");
        return;
    };
    let string = (after.active.clone(), after.turn_seq, after.action_turn, after.transient_flags, announced.clone());
    crate::dual::compare_value(Area::Driver, "turn::pass_on", &string, &core);
}

/// `phase::advance_phase`, Strategy arm: Political Stability is spent.
pub fn reset_stability(state: &mut GameState) {
    write(
        Area::Driver,
        state,
        "turn::stability_reset",
        |s| {
            for player in &mut s.players {
                player.stability = false;
            }
        },
        real::reset_stability,
    );
}

/// `phase::advance_phase`, Strategy arm, once the faction hooks have run: the action phase begins with nobody active; returns
/// the seat first in initiative order.
pub fn strategy_phase_flip(state: &mut GameState) -> Option<PlayerId> {
    write(
        Area::Driver,
        state,
        "turn::strategy_phase_ended",
        |s| {
            let first = s.initiative_order().first().cloned();
            s.phase = ti4_model::state::Phase::Action;
            s.action_turn = 0;
            s.active = None;
            first
        },
        real::strategy_phase_flip,
    )
}

/// `faction_techs::return_spec_ops`, the turn prelude's placement of the Spec Ops units waiting on the card.
pub fn return_spec_ops(state: &mut GameState, player: &PlayerId) -> u32 {
    write(Area::Driver, state, "turn::return_spec_ops", |s| crate::faction_techs::return_spec_ops(s, player), |s| real::return_spec_ops(s, player))
}

/// The prelude has run: `prepared_turn_seq` is the turn's (a `Game` field; the core's `Pending.prepared_turn_seq`). Compared
/// as the sequence number the core would record.
pub fn check_prepared(state: &GameState, recorded: Option<u32>) {
    if crate::dual::backend(Area::Driver) != crate::dual::Backend::Dual {
        return;
    }
    let Some(core) = real::prepared(state) else {
        crate::dual::note_bail("turn::prepared", "the position is outside the tables");
        return;
    };
    crate::dual::compare_value(Area::Driver, "turn::prepared", &recorded, &core);
}

/// `passed = true` of the seat that passes (`Game::apply_choice`, the "pass" arm).
pub fn mark_passed(state: &mut GameState, player: &PlayerId) {
    write(
        Area::Driver,
        state,
        "turn::pass",
        |s| {
            if let Some(seat) = s.player_mut(player) {
                seat.passed = true;
            }
        },
        |s| real::mark_passed(s, player),
    );
}

/// Prove Endurance at the pass: the occurrence the last seat to pass records, if it is the last.
pub fn last_to_pass(state: &mut GameState, player: &PlayerId) -> Option<u64> {
    write(
        Area::Driver,
        state,
        "turn::last_to_pass",
        |s| {
            if s.players.iter().all(|seat| seat.passed) {
                let occurrence = s.begin_feat_occurrence();
                s.record_event_feat(player, ti4_model::state::Feat::LastToPass, occurrence);
                Some(occurrence.0)
            } else {
                None
            }
        },
        |s| real::last_to_pass(s, player),
    )
}

/// An explicit pass declines the extra action Master Plan may have granted.
pub fn decline_additional_action(state: &mut GameState) {
    write(
        Area::Driver,
        state,
        "turn::decline_additional_action",
        |s| s.transient_flags.clear(TransientFlags::ADDITIONAL_ACTION),
        real::decline_additional_action,
    );
}

/// The writes of the close of a tactical action (`Game::close_tactical`): no system is active and the legacy pending tag is
/// cleared (the aftermath window, a field of `Game`, is dropped by the caller).
pub fn close_tactical_writes(state: &mut GameState) {
    write(
        Area::Driver,
        state,
        "turn::close_tactical",
        |s| {
            s.active_system = None;
            s.pending = None;
        },
        real::close_tactical_writes,
    );
}

/// What `close_tactical` decides from the position, as the string decides it: does the Crown of Emphidia offer an
/// exploration, does Dark Energy Tap fire, does anyone listen for the end of the action (T'ro and its kin).
pub type CloseGates = (bool, bool, bool);

/// Compare the three gates of `close_tactical` and its continuation with the core's (`driver::tactical_tail`). `ssruu_lends`:
/// a Yssaril seat with a readied Ssruu could copy an agent (the string's `borrowable_agents`), which the core does not know.
pub fn check_close(state: &GameState, sources: ti4_model::content_types::SourceSet, string: CloseGates, ssruu_lends: bool, then: (bool, bool, u8)) {
    if crate::dual::backend(Area::Driver) != crate::dual::Backend::Dual {
        return;
    }
    let Some(core) = real::close_gates(state, sources, ssruu_lends) else {
        crate::dual::note_bail("turn::close_gates", "the position is outside the tables");
        return;
    };
    crate::dual::compare_value(Area::Driver, "turn::close_gates", &string, &core);
    let (leader_strategy, secondary, chosen) = then;
    crate::dual::compare_value(Area::Driver, "turn::close_then", &chosen, &real::then(leader_strategy, secondary));
}

/// The sources of the action menu as the string marks them: the position in `driver::menu::ORDER`.
pub mod src {
    pub const STRATEGIC: u8 = 0;
    pub const TACTICAL: u8 = 1;
    pub const TRANSACTIONS: u8 = 2;
    pub const CONTACTS: u8 = 3;
    pub const PAYMENTS: u8 = 4;
    pub const RELICS: u8 = 5;
    pub const EXPLORATION: u8 = 6;
    pub const TECHNOLOGY: u8 = 7;
    pub const THUNDERS_EDGE: u8 = 8;
    pub const FACTION: u8 = 9;
    pub const MAPPED: u8 = 10;
    pub const LEADERS: u8 = 11;
    pub const ACTION_CARDS: u8 = 12;
}

/// The option ids each source of the action menu contributed (by [`src`]), in the order the string appended them.
pub type MenuParts = Vec<(u8, Vec<String>)>;

/// Compare `Game::turn_options` (`parts`: what each source gave, `thunders_edge`: the table has the expansion on, `failed`:
/// the component actions that already failed this turn, `menu`: the option ids the string offers) with the core's assembly.
pub fn check_turn_menu(parts: &MenuParts, thunders_edge: bool, failed: &[String], menu: &[String]) {
    if crate::dual::backend(Area::Driver) != crate::dual::Backend::Dual {
        return;
    }
    let core = real::turn_menu(parts, thunders_edge, failed);
    crate::dual::compare_value(Area::Driver, "turn::menu", &menu.to_vec(), &core);
}

/// One option of the turn menu as the closing menu sees it: `(id, free, pass)`.
pub type ClosingEntry = (String, bool, bool);

/// Compare `Game::closing_options` with the core's assembly. `second` is whether the string allowed a second action;
/// the core decides it from the seat's technologies and the actions taken (`actions`).
pub fn check_closing(
    before: &GameState,
    player: &PlayerId,
    actions: u8,
    second: bool,
    end_turn: &str,
    turn: &[ClosingEntry],
    expedition: &[String],
    menu: &[String],
) {
    if crate::dual::backend(Area::Driver) != crate::dual::Backend::Dual {
        return;
    }
    let Some((core_second, core)) = real::closing_menu(before, player, actions, end_turn, turn, expedition) else {
        crate::dual::note_bail("turn::closing", "the position is outside the tables");
        return;
    };
    crate::dual::compare_value(Area::Driver, "turn::closing", &(second, menu.to_vec()), &(core_second, core));
}

/// Compare `Game::finish_action` after `ACTION_COMPLETED` (`before`: the position then, `actions_before` the actions taken
/// so far this turn, `closing_len` the options of the closing menu the string counted, `after`: what the string left in
/// `(turn_closing, actions_this_turn)`) with the core's `finish_action`. `closing_len` is 0 when the string did not count
/// (a retained turn, another phase).
pub fn check_finish(before: &GameState, actions_before: u8, closing_len: usize, after: (Option<&PlayerId>, u8)) {
    if crate::dual::backend(Area::Driver) != crate::dual::Backend::Dual {
        return;
    }
    let Some(core) = real::finish_action(before, actions_before, closing_len) else {
        crate::dual::note_bail("turn::finish_action", "the position is outside the tables");
        return;
    };
    let string = (after.0.cloned(), after.1, after.0.is_some());
    crate::dual::compare_value(Area::Driver, "turn::finish_action", &string, &core);
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::driver::turn_flow as rules;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::id::PlayerId;
    use ti4_model::state::{GameState, TransientFlags};

    use crate::core_sync::{Footprint, Sources};

    const TURN: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Strategy, Slice::Trade, Slice::Transient],
        &[Slice::Meta, Slice::Trade, Slice::Transient],
    );

    /// The end of an action reads the phase, the active seat, the additional-action flag and the held counters.
    const FINISH: Footprint = Footprint::new(&[Slice::Meta, Slice::Transient, Slice::Pending], &[]);

    /// The closing menu reads the seat's technologies.
    const MENU: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[]);

    /// The round change reads and writes more: the leaders' statuses, the promise book, the relationships, the mat.
    const ROUND: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Strategy, Slice::Trade, Slice::Transient],
        &[Slice::Meta, Slice::Players, Slice::Strategy, Slice::Trade],
    );

    fn lend<T>(gs: &mut GameState, f: impl FnOnce(&mut CoreState, &crate::render::Render) -> T) -> Option<T> {
        lend_with(gs, TURN, f)
    }

    fn lend_with<T>(gs: &mut GameState, fp: Footprint, f: impl FnOnce(&mut CoreState, &crate::render::Render) -> T) -> Option<T> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), fp, |core, _| f(core, &r)).ok()
    }

    const CLOSE: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards, Slice::Explore], &[]);
    const SPEC_OPS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Players, Slice::Board]);
    const CLOSE_WRITES: Footprint = Footprint::new(&[Slice::Meta, Slice::Pending], &[Slice::Meta, Slice::Pending]);

    pub fn close_tactical_writes(gs: &mut GameState) -> Option<()> {
        lend_with(gs, CLOSE_WRITES, |core, _| ti4_engine_core::driver::tactical_tail::close_writes(core))
    }

    /// `(crown, det, tro)` on a copy of the position.
    pub fn close_gates(before: &GameState, sources: ti4_model::content_types::SourceSet, ssruu_lends: bool) -> Option<super::CloseGates> {
        use ti4_engine_core::driver::tactical_tail as tail;
        let mut gs = before.clone();
        let scope = crate::core_adapter::scope(sources);
        let active = gs.active.clone();
        let system = gs.active_system.clone();
        lend_with(&mut gs, CLOSE, |core, r| {
            let seat = match &active {
                Some(p) => Some(r.seat(p.as_str())?),
                None => None,
            };
            let system = match &system {
                Some(s) => Some(<ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(s.as_str())?),
                None => None,
            };
            let crown = seat.is_some_and(|seat| tail::crown_offers(core, seat, scope));
            let det = seat.zip(system).is_some_and(|(seat, system)| tail::det_fires(core, seat, system));
            Some((crown, det, tail::tro_window(core, ssruu_lends)))
        })?
    }

    pub fn then(leader_strategy: bool, secondary: bool) -> u8 {
        use ti4_engine_core::driver::tactical_tail::{Then, then};
        match then(leader_strategy, secondary) {
            Then::LeaderStrategy => 0,
            Then::Secondary => 1,
            Then::FinishAction => 2,
        }
    }

    const FLIP: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Strategy], &[Slice::Meta]);
    const STABILITY: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Players]);

    pub fn reset_stability(gs: &mut GameState) -> Option<()> {
        lend_with(gs, STABILITY, |core, _| rules::reset_stability(core))
    }

    pub fn strategy_phase_flip(gs: &mut GameState) -> Option<Option<PlayerId>> {
        lend_with(gs, FLIP, |core, r| Some(rules::strategy_phase_flip(core).map(|s| r.player(s))))?
    }

    const PASS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Meta, Slice::Players]);

    pub fn mark_passed(gs: &mut GameState, p: &PlayerId) -> Option<()> {
        lend_with(gs, PASS, |core, r| r.seat(p.as_str()).map(|seat| ti4_engine_core::driver::dispatch::mark_passed(core, seat)))?
    }

    pub fn last_to_pass(gs: &mut GameState, p: &PlayerId) -> Option<Option<u64>> {
        lend_with(gs, PASS, |core, r| Some(ti4_engine_core::driver::dispatch::last_to_pass(core, r.seat(p.as_str())?)))?
    }

    pub fn decline_additional_action(gs: &mut GameState) -> Option<()> {
        lend(gs, |core, _| ti4_engine_core::driver::dispatch::decline_additional_action(core))
    }

    pub fn return_spec_ops(gs: &mut GameState, p: &PlayerId) -> Option<u32> {
        lend_with(gs, SPEC_OPS, |core, r| {
            let infantry = <ti4_engine_core::generated::Unit as crate::generated::IdStr>::from_id("sol_infantry2")?;
            Some(rules::return_spec_ops(core, r.seat(p.as_str())?, infantry))
        })?
    }

    pub fn prepared(gs: &GameState) -> Option<Option<u32>> {
        let mut gs = gs.clone();
        lend_with(&mut gs, FINISH, |core, _| {
            rules::mark_prepared(core);
            Some(core.pending().prepared_turn_seq)
        })?
    }

    pub fn begin_action_turn(gs: &mut GameState, p: &PlayerId) -> Option<()> {
        lend(gs, |core, r| r.seat(p.as_str()).map(|seat| rules::begin_action_turn(core, seat)))?
    }

    pub fn advance_turn(gs: &mut GameState) -> Option<Option<PlayerId>> {
        lend(gs, |core, r| rules::advance_turn(core).map(|seat| r.player(seat)))
    }

    pub fn action_phase_ended(gs: &mut GameState) -> Option<()> {
        lend(gs, |core, _| rules::action_phase_ended(core))
    }

    pub fn take_retention(gs: &mut GameState) -> Option<bool> {
        lend(gs, |core, _| rules::take_retention(core))
    }

    pub fn begin_next_round(gs: &mut GameState, cards: &[ti4_model::id::StrategyCardId]) -> Option<()> {
        let list: Vec<ti4_engine_core::generated::StrategyCard> = cards.iter().map(|c| crate::core_adapter::typed(c.as_str())).collect::<Option<_>>()?;
        lend_with(gs, ROUND, |core, _| rules::begin_next_round(core, &list).ok())?
    }

    pub fn end_turn_scratch(gs: &mut GameState) -> Option<()> {
        lend(gs, |core, _| rules::end_turn_scratch(core))
    }

    pub fn turn_menu(parts: &super::MenuParts, thunders_edge: bool, failed: &[String]) -> Vec<String> {
        use ti4_engine_core::driver::menu::{ORDER, turn_menu};
        let typed: Vec<_> = parts.iter().filter_map(|(i, ids)| Some((*ORDER.get(usize::from(*i))?, ids.clone()))).collect();
        turn_menu(&typed, thunders_edge, failed)
    }

    pub fn closing_menu(
        before: &GameState,
        player: &PlayerId,
        actions: u8,
        end_turn: &str,
        turn: &[super::ClosingEntry],
        expedition: &[String],
    ) -> Option<(bool, Vec<String>)> {
        use ti4_engine_core::driver::menu::{Entry, closing_menu, second_action};
        let mut gs = before.clone();
        let second = lend_with(&mut gs, MENU, |core, r| Some(second_action(core, r.seat(player.as_str())?, actions)))??;
        let entries: Vec<Entry<String>> = turn.iter().map(|(option, free, pass)| Entry { option: option.clone(), free: *free, pass: *pass }).collect();
        Some((second, closing_menu(end_turn.to_owned(), &entries, second, expedition)))
    }

    /// `finish_action` on a copy of `before`: `(turn_closing, actions_this_turn, closing offered)`.
    pub fn finish_action(before: &GameState, actions_before: u8, closing_len: usize) -> Option<(Option<PlayerId>, u8, bool)> {
        let mut gs = before.clone();
        lend_with(&mut gs, FINISH, |core, r| {
            core.pending_mut().actions_this_turn = actions_before;
            let out = rules::finish_action(core, closing_len);
            let p = core.pending();
            Some((p.turn_closing.map(|s| r.player(s)), p.actions_this_turn, out == rules::Finish::Closing))
        })?
        .map(|(closing, actions, offered)| (closing.clone(), actions, offered && closing.is_some()))
    }

    /// `(active, turn_seq, action_turn, flags, announced lines)` after the tail.
    pub type PassOn = (Option<PlayerId>, u32, u32, TransientFlags, Vec<String>);

    /// The tail on a copy of `before`: `(active, turn_seq, action_turn, flags, announced lines)`.
    pub fn pass_on(
        before: &GameState,
        ended: Option<&PlayerId>,
    ) -> Option<PassOn> {
        let mut gs = before.clone();
        let lines = lend(&mut gs, |core, r| {
            let ended: Option<Seat> = match ended {
                Some(p) => Some(r.seat(p.as_str())?),
                None => None,
            };
            Some(
                rules::pass_on(core, ended)
                    .as_slice()
                    .iter()
                    .map(|a| match a {
                        rules::Announce::Passed(s) => format!("PASSED:{}", r.player(*s)),
                        rules::Announce::Skipped(s) => format!("SKIPPED:{}", r.player(*s)),
                        rules::Announce::Puppet(s) => format!("PUPPET:{}", r.player(*s)),
                    })
                    .collect::<Vec<String>>(),
            )
        })??;
        Some((gs.active.clone(), gs.turn_seq, gs.action_turn, gs.transient_flags, lines))
    }
}

#[cfg(not(feature = "typed-core"))]
mod real {
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    pub fn close_tactical_writes(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn close_gates(_: &GameState, _: ti4_model::content_types::SourceSet, _: bool) -> Option<super::CloseGates> {
        None
    }
    pub fn then(_: bool, _: bool) -> u8 {
        0
    }
    pub fn reset_stability(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn strategy_phase_flip(_: &mut GameState) -> Option<Option<PlayerId>> {
        None
    }
    pub fn mark_passed(_: &mut GameState, _: &PlayerId) -> Option<()> {
        None
    }
    pub fn last_to_pass(_: &mut GameState, _: &PlayerId) -> Option<Option<u64>> {
        None
    }
    pub fn decline_additional_action(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn return_spec_ops(_: &mut GameState, _: &PlayerId) -> Option<u32> {
        None
    }
    pub fn prepared(_: &GameState) -> Option<Option<u32>> {
        None
    }
    pub fn begin_action_turn(_: &mut GameState, _: &PlayerId) -> Option<()> {
        None
    }
    pub fn advance_turn(_: &mut GameState) -> Option<Option<PlayerId>> {
        None
    }
    pub fn action_phase_ended(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn take_retention(_: &mut GameState) -> Option<bool> {
        None
    }
    pub fn begin_next_round(_: &mut GameState, _: &[ti4_model::id::StrategyCardId]) -> Option<()> {
        None
    }
    pub fn end_turn_scratch(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn finish_action(_: &GameState, _: u8, _: usize) -> Option<(Option<PlayerId>, u8, bool)> {
        None
    }
    pub fn turn_menu(_: &super::MenuParts, _: bool, _: &[String]) -> Vec<String> {
        Vec::new()
    }
    pub fn closing_menu(_: &GameState, _: &PlayerId, _: u8, _: &str, _: &[super::ClosingEntry], _: &[String]) -> Option<(bool, Vec<String>)> {
        None
    }
    pub type PassOn = (Option<PlayerId>, u32, u32, ti4_model::state::TransientFlags, Vec<String>);
    pub fn pass_on(_: &GameState, _: Option<&PlayerId>) -> Option<PassOn> {
        None
    }
}

#[cfg(all(test, feature = "typed-core"))]
mod tests {
    use ti4_model::state::{Phase, TransientFlags};

    use super::*;
    use crate::dual::{AreaMask, Backend, report, take_report, with_mask};
    use crate::strategy_secondary_dual_tests::{dealt, pid};

    const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

    /// Three seats with a card each, `a` to act.
    fn table() -> GameState {
        let mut state = dealt(&F3, "Politics");
        for seat in ["b", "c"] {
            let card = state.unclaimed_strategy_cards.remove(0);
            assert!(state.deal_strategy_card(&pid(seat), card));
        }
        state
    }

    fn driver() -> AreaMask {
        AreaMask::STRING.with(Area::Driver, Backend::Dual)
    }

    #[test]
    fn the_turn_passes_round_the_table_and_the_core_agrees_with_the_string() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = table();
        let mut order = Vec::new();
        let r = with_mask(driver(), || {
            for _ in 0..4 {
                order.push(state.active.clone().unwrap());
                super::super::phase::advance_turn(&mut state);
            }
            state.player_mut(&pid("b")).unwrap().passed = true;
            order.push(state.active.clone().unwrap());
            super::super::phase::advance_turn(&mut state);
            report()
        });
        let _ = take_report();
        assert_eq!(order, [pid("a"), pid("b"), pid("c"), pid("a"), pid("b")]);
        assert_eq!(state.active, Some(pid("c")), "b has passed: the turn skips it");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let calls = r.areas.get(&Area::Driver).map_or(0, |c| c.calls);
        assert!(calls >= 5, "every advance ran both halves: {calls}");
        assert!(!r.windows.keys().any(|k| k.starts_with("turn::") && k.contains("[bail")), "{:?}", r.windows);
    }

    #[test]
    fn a_retained_turn_and_the_end_of_the_action_phase_agree_too() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = table();
        state.transient_flags.set(TransientFlags::ADDITIONAL_ACTION);
        state.transient_flags.set(TransientFlags::BLACK_MARKET);
        let (kept, again) = with_mask(driver(), || {
            let kept = take_retention(&mut state);
            let again = take_retention(&mut state);
            end_turn_scratch(&mut state, |s| s.transient_flags.clear(TransientFlags::BLACK_MARKET));
            action_phase_ended(&mut state);
            (kept, again)
        });
        let r = report();
        let _ = take_report();
        assert!(kept && !again);
        assert_eq!((state.phase, state.active.clone()), (Phase::Status, None));
        assert!(!state.transient_flags.has(TransientFlags::BLACK_MARKET));
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    }

    #[test]
    fn a_tail_in_which_a_reaction_window_changed_the_turn_is_a_bail_not_a_difference() {
        let _serial = crate::dual::tests::serial();
        let before = table();
        let mut after = before.clone();
        super::super::phase::advance_turn_text(&mut after);
        after.transient_flags.set(TransientFlags::SKIP_NEXT_TURN);
        let _ = take_report();
        with_mask(driver(), || {
            check_pass_on(&before, &after, Some(&pid("a")), &vec!["PASSED:a".to_owned(), "TAINTED".to_owned()]);
            check_pass_on(&before, &after, Some(&pid("a")), &vec!["PASSED:a".to_owned()]);
        });
        let r = report();
        let _ = take_report();
        assert!(r.windows.keys().any(|k| k == "turn::pass_on [bail: a reaction window of TURN_PASSED changed the turn]"), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 1, "only the untainted tail with the flag set differs: {:?}", r.differences);
    }

    #[test]
    fn the_tail_of_an_ended_turn_is_compared_with_what_the_string_announced() {
        let _serial = crate::dual::tests::serial();
        let before = table();
        let ended = pid("a");
        // The string's tail: the turn passes to b and the end of a's turn is announced.
        let mut after = before.clone();
        super::super::phase::advance_turn_text(&mut after);
        let _ = take_report();
        with_mask(driver(), || {
            check_pass_on(&before, &after, Some(&ended), &vec!["PASSED:a".to_owned()]);
            // A string that forgot the announcement, or moved the turn to the wrong seat, differs.
            check_pass_on(&before, &after, Some(&ended), &Vec::new());
            let mut wrong = after.clone();
            wrong.active = Some(pid("c"));
            check_pass_on(&before, &wrong, Some(&ended), &vec!["PASSED:a".to_owned()]);
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 2, "{:?}", r.differences);
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_menu_the_string_offers_is_the_core_assembly_of_its_sources() {
        let _serial = crate::dual::tests::serial();
        let parts: MenuParts = vec![
            (src::STRATEGIC, ids(&["strategic|a"])),
            (src::TACTICAL, ids(&["tactical"])),
            (src::LEADERS, ids(&["component|leader|x"])),
            (src::ACTION_CARDS, ids(&["action_card|0"])),
        ];
        let ok = ids(&["strategic|a", "tactical", "component|leader|x", "action_card|0"]);
        let _ = take_report();
        with_mask(driver(), || {
            check_turn_menu(&parts, false, &[], &ok);
            // The string put the cards before the leaders, forgot the filter, or kept a failed action: each differs.
            check_turn_menu(&parts, false, &[], &ids(&["strategic|a", "tactical", "action_card|0", "component|leader|x"]));
            check_turn_menu(&parts, false, &ids(&["tactical"]), &ok);
            check_turn_menu(&parts, false, &ids(&["tactical"]), &ids(&["strategic|a", "component|leader|x", "action_card|0"]));
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 2, "{:?}", r.differences);
    }

    #[test]
    fn the_closing_menu_with_fleet_logistics_offers_a_second_action_and_without_it_only_the_free_options() {
        let _serial = crate::dual::tests::serial();
        let mut with_fl = table();
        with_fl.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("fl"));
        let without = table();
        let a = pid("a");
        let turn = vec![
            ("pass".to_owned(), false, true),
            ("tactical".to_owned(), false, false),
            ("open|b".to_owned(), true, false),
        ];
        let _ = take_report();
        with_mask(driver(), || {
            // One action taken: a second is offered with the technology.
            check_closing(&with_fl, &a, 1, true, "end_turn", &turn, &[], &ids(&["end_turn", "tactical", "open|b"]));
            check_closing(&without, &a, 1, false, "end_turn", &turn, &ids(&["commit"]), &ids(&["end_turn", "open|b", "commit"]));
            // Two actions taken: no third. The string saying it allowed one differs in the flag AND the options.
            check_closing(&with_fl, &a, 2, true, "end_turn", &turn, &[], &ids(&["end_turn", "tactical", "open|b"]));
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 1, "the third call: {:?}", r.differences);
    }

    #[test]
    fn the_source_numbers_of_the_string_marks_are_the_positions_of_the_core_order() {
        use ti4_engine_core::driver::menu::{ORDER, Source};
        let marks = [
            (src::STRATEGIC, Source::Strategic),
            (src::TACTICAL, Source::Tactical),
            (src::TRANSACTIONS, Source::Transactions),
            (src::CONTACTS, Source::Contacts),
            (src::PAYMENTS, Source::Payments),
            (src::RELICS, Source::Relics),
            (src::EXPLORATION, Source::Exploration),
            (src::TECHNOLOGY, Source::Technology),
            (src::THUNDERS_EDGE, Source::ThundersEdge),
            (src::FACTION, Source::Faction),
            (src::MAPPED, Source::Mapped),
            (src::LEADERS, Source::Leaders),
            (src::ACTION_CARDS, Source::ActionCards),
        ];
        for (mark, source) in marks {
            assert_eq!(ORDER[usize::from(mark)], source);
        }
    }

    fn close_position() -> GameState {
        let mut state = table();
        state.active_system = Some(ti4_model::id::SystemId::new("18"));
        state.pending = Some("move".to_owned());
        state
    }

    #[test]
    fn the_close_of_a_tactical_action_clears_the_system_and_the_pending_tag_in_both_halves() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = close_position();
        with_mask(driver(), || close_tactical_writes(&mut state));
        let r = report();
        let _ = take_report();
        assert_eq!((state.active_system.clone(), state.pending.clone()), (None, None));
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas.get(&Area::Driver).map_or(0, |c| c.calls) >= 1, "the write was compared: {:?}", r.areas);
    }

    #[test]
    fn a_tro_listener_dark_energy_tap_and_the_crown_are_read_from_the_position_by_the_core() {
        let _serial = crate::dual::tests::serial();
        let sources = ti4_model::content_types::DEFAULT;
        let mut state = close_position();
        let system = ti4_model::id::SystemId::new("18");
        let probe = |state: &GameState| real::close_gates(state, sources, false);
        assert_eq!(probe(&state), Some((false, false, false)), "nothing listens at a plain table");
        // A readied Sardakk agent listens for the end of the action.
        state.player_mut(&pid("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("sardakkagent"), ti4_model::state::LeaderStatus::Readied);
        assert_eq!(probe(&state), Some((false, false, true)));
        // Dark Energy Tap fires for a ship of the active seat in a system with a frontier token.
        state.frontier_tokens.insert(system.clone());
        state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("det"));
        assert_eq!(probe(&state), Some((false, false, true)), "no ship there yet");
        crate::fixtures::put(&mut state, &system, "cruiser", &pid("a"), 1);
        assert_eq!(probe(&state), Some((false, true, true)));
        // Every gate and the continuation are compared against the string's in one call.
        let _ = take_report();
        with_mask(driver(), || {
            check_close(&state, sources, (false, true, true), false, (false, true, 1));
            check_close(&state, sources, (false, false, true), false, (false, false, 2));
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 1, "only the gate the string got wrong differs: {:?}", r.differences);
    }

    #[test]
    fn spec_ops_waiting_on_the_card_land_on_the_home_planet_in_both_halves() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = table();
        let seat = pid("a");
        let home = state.player(&seat).unwrap().home_system.clone().expect("a home system");
        state.player_mut(&seat).unwrap().spec_ops_card = 2;
        let before: usize = state.system_state(&home).planet_units.values().flatten().filter(|u| u.type_id.as_str() == "sol_infantry2").count();
        let placed = with_mask(driver(), || return_spec_ops(&mut state, &seat));
        let r = report();
        let _ = take_report();
        let after: usize = state.system_state(&home).planet_units.values().flatten().filter(|u| u.type_id.as_str() == "sol_infantry2").count();
        assert_eq!((placed, after - before, state.player(&seat).unwrap().spec_ops_card), (2, 2, 0));
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas.get(&Area::Driver).map_or(0, |c| c.calls) >= 1, "the write was compared: {:?}", r.areas);
        // Nothing waiting, nothing placed; the prepared turn is the turn in progress.
        let again = with_mask(driver(), || return_spec_ops(&mut state, &seat));
        assert_eq!(again, 0);
        let _ = take_report();
        with_mask(driver(), || {
            check_prepared(&state, Some(state.turn_seq));
            check_prepared(&state, None);
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 1, "only the unrecorded prepared turn differs: {:?}", r.differences);
    }

    #[test]
    fn the_strategy_phase_ends_into_the_action_phase_with_the_first_in_initiative_and_stability_spent() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = table();
        state.phase = Phase::Strategy;
        state.player_mut(&pid("b")).unwrap().stability = true;
        let before_seq = state.turn_seq;
        with_mask(driver(), || {
            super::super::phase::advance_phase(&mut state);
        });
        let r = report();
        let _ = take_report();
        assert_eq!(state.phase, Phase::Action);
        assert!(state.active.is_some() && state.turn_seq == before_seq + 1, "the first turn began");
        assert!(state.players.iter().all(|p| !p.stability), "stability is spent");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas.get(&Area::Driver).map_or(0, |c| c.calls) >= 3, "the three writes were compared: {:?}", r.areas);
    }

    #[test]
    fn the_last_seat_to_pass_records_prove_endurance_and_an_explicit_pass_declines_the_extra_action() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = table();
        state.transient_flags.set(TransientFlags::ADDITIONAL_ACTION);
        let (first, second, third) = with_mask(driver(), || {
            mark_passed(&mut state, &pid("a"));
            let first = last_to_pass(&mut state, &pid("a"));
            mark_passed(&mut state, &pid("b"));
            let second = last_to_pass(&mut state, &pid("b"));
            mark_passed(&mut state, &pid("c"));
            let third = last_to_pass(&mut state, &pid("c"));
            decline_additional_action(&mut state);
            (first, second, third)
        });
        let r = report();
        let _ = take_report();
        assert_eq!((first, second), (None, None), "others have yet to pass");
        assert!(third.is_some(), "c is the last to pass");
        assert!(state.player(&pid("c")).unwrap().event_feats.iter().any(|(f, _)| *f == ti4_model::state::Feat::LastToPass));
        assert!(!state.transient_flags.has(TransientFlags::ADDITIONAL_ACTION));
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas.get(&Area::Driver).map_or(0, |c| c.calls) >= 7, "every write was compared: {:?}", r.areas);
    }
}
