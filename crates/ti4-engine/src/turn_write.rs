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
    let Some(core) = real::pass_on(before, ended) else {
        crate::dual::note_bail("turn::pass_on", "the position is outside the tables");
        return;
    };
    let string = (after.active.clone(), after.turn_seq, after.action_turn, after.transient_flags, announced.clone());
    crate::dual::compare_value(Area::Driver, "turn::pass_on", &string, &core);
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

    fn lend<T>(gs: &mut GameState, f: impl FnOnce(&mut CoreState, &crate::render::Render) -> T) -> Option<T> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), TURN, |core, _| f(core, &r)).ok()
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

    pub fn end_turn_scratch(gs: &mut GameState) -> Option<()> {
        lend(gs, |core, _| rules::end_turn_scratch(core))
    }

    /// The tail on a copy of `before`: `(active, turn_seq, action_turn, flags, announced lines)`.
    pub fn pass_on(
        before: &GameState,
        ended: Option<&PlayerId>,
    ) -> Option<(Option<PlayerId>, u32, u32, TransientFlags, Vec<String>)> {
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
    pub fn end_turn_scratch(_: &mut GameState) -> Option<()> {
        None
    }
    pub fn pass_on(_: &GameState, _: Option<&PlayerId>) -> Option<(Option<PlayerId>, u32, u32, ti4_model::state::TransientFlags, Vec<String>)> {
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
}
