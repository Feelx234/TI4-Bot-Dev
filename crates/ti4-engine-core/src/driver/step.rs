//! `Session::step` (package D1b2): one step of the game loop that RUNS the arms the core owns, where `driver::session` only
//! names them. A step reads the position's facts, takes the arm of [`step_arm`] and performs what that arm does in the
//! core; the part of an arm that is still a string body is reported as [`Lent`] (the dual counts a step by its lent part, the
//! reverse lend of D2 runs it).
//!
//! The arms the core runs: the turn prelude's placement of the Spec Ops units and the mark that the turn is prepared; the phase
//! job `step_phase` does between the phases of a round that need no hook (the end of the action phase; the strategy phase's
//! flip into the action phase after its faction hooks); the question the table is asked ([`Step::Asks`], which one is
//! `ask_arm`'s). Everything else is lent by name: the windows of the finished areas follow their own machines
//! (`d1::pending`), the status and agenda halves are D1a's, tactical and aftermath wait on the chain composer's later stages.
//!
//! L5: reads the position only through the facts and the turn flow.

use super::session::{ask_arm, step_arm, Arm, AskArm, Facts};
use super::turn_flow;
use crate::generated::Unit;
use crate::state::{CoreState, Phase};

/// What of an arm is still a string body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lent {
    /// The whole arm: its window or phase job is not run by the core.
    Arm(Arm),
    /// The start-of-turn technologies, the commander unlocks, Military Support and the `TURN_BEGAN` window.
    TurnHooks,
    /// The faction hooks between the strategy phase and the action phase (Naalu's initiative token, Imperial Arbiter) and the
    /// `STRATEGY_PHASE_ENDED` / `ACTION_PHASE_BEGAN` events.
    StrategyHooks,
    /// The `STATUS_PHASE_BEGAN` / `ACTION_PHASE_ENDED` events of the action phase's end.
    PhaseEvents,
}

/// What a step did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The game is over, or an error report stops the loop.
    Stopped(Arm),
    /// The arm's core part ran; `lent` is what the string still has to do for the step.
    Ran { arm: Arm, lent: Option<Lent> },
    /// A question is put to the table (the arm of the question).
    Asks(AskArm),
    /// The arm is in the caller's [`NativeArms`] and has a live function: the caller runs it (`live::live_arm`, `live::ask_arm_fn`)
    /// with its `Env`; nothing is lent.
    Native(Arm),
    /// The arm is not run by the core.
    Lend(Lent),
}

/// The arms the core runs LIVE (package KF1): one bit per [`Arm`]. An arm in the set that has an entry in the arm registry
/// (`live::live_arm`) is [`Step::Native`]; any other arm is what it was (`Step::Lend`, or the part the core ran). The empty set
/// is the dual's world: [`step`] behaves exactly as before the live loop existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeArms(u32);

impl NativeArms {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self((1 << 20) - 1);

    /// The set of exactly `arms`.
    #[must_use]
    pub const fn of(arms: &[Arm]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < arms.len() {
            bits |= 1 << (arms[i] as u32);
            i += 1;
        }
        Self(bits)
    }

    /// The bit set as a number (the report of a run: the names are the engine's, `live::arms`).
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Every arm that has a live function today (the registry's own list).
    #[must_use]
    pub fn registered() -> Self {
        let mut bits = 0;
        for arm in Arm::ALL {
            if super::live::live_arm(arm).is_some() {
                bits |= 1 << (arm as u32);
            }
        }
        Self(bits)
    }

    #[must_use]
    pub const fn has(self, arm: Arm) -> bool {
        self.0 & (1 << (arm as u32)) != 0
    }
}

/// What the caller supplies that the position does not hold.
#[derive(Debug, Clone, Copy)]
pub struct Needs {
    /// `legal_options` has a question.
    pub asks: bool,
    /// The unit a Spec Ops return places (`sol_infantry2`): the adapter names it, the core holds no text.
    pub spec_ops_unit: Unit,
}

/// One step of the loop over `facts` (taken from the position before the arm is chosen).
pub fn step(core: &mut CoreState, facts: &Facts, needs: Needs, native: NativeArms) -> Step {
    let arm = step_arm(facts, needs.asks);
    if native.has(arm) && super::live::live_arm(arm).is_some() {
        return Step::Native(arm);
    }
    match arm {
        Arm::Finished | Arm::Blocked => Step::Stopped(arm),
        Arm::TurnPrelude => {
            let Some(active) = core.meta().active else { return Step::Lend(Lent::Arm(arm)) };
            turn_flow::return_spec_ops(core, active, needs.spec_ops_unit);
            turn_flow::mark_prepared(core);
            Step::Ran { arm, lent: Some(Lent::TurnHooks) }
        }
        Arm::Phase => match core.meta().phase {
            Phase::Action if core.players().iter().all(|p| p.econ.passed) => {
                turn_flow::action_phase_ended(core);
                Step::Ran { arm, lent: Some(Lent::PhaseEvents) }
            }
            Phase::Strategy => {
                turn_flow::reset_stability(core);
                // The faction hooks run here in the string (`strategy_phase_ended`): they only read and write the initiative
                // overrides, which the core reads from the position it is given.
                match turn_flow::strategy_phase_flip(core) {
                    Some(first) => {
                        turn_flow::begin_action_turn(core, first);
                        Step::Ran { arm, lent: Some(Lent::StrategyHooks) }
                    }
                    None => Step::Lend(Lent::Arm(arm)),
                }
            }
            _ => Step::Lend(Lent::Arm(arm)),
        },
        Arm::Ask => Step::Asks(ask_arm(facts)),
        _ => Step::Lend(Lent::Arm(arm)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::ids::Seat;
    use crate::state::{Player, Players};

    fn table(phase: Phase) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s.meta.phase = phase;
        s.meta.round = 2;
        s
    }

    fn needs() -> Needs {
        Needs { asks: false, spec_ops_unit: Unit::ALL[0] }
    }

    fn facts(core: &CoreState) -> Facts {
        Facts::of(core, false, false, false, true)
    }

    #[test]
    fn a_finished_game_stops_the_loop_and_an_open_window_is_lent_by_its_arm() {
        let mut s = table(Phase::Action);
        s.meta.finished = true;
        let f = facts(&s);
        assert_eq!(step(&mut s, &f, needs(), NativeArms::NONE), Step::Stopped(Arm::Finished));
        let mut s = table(Phase::Action);
        s.meta.active = Some(Seat(0));
        s.pending.stack.push(crate::state::Window::Untyped { kind: crate::state::WindowKind::Trade, actor: None, data: [0; 3] });
        let f = facts(&s);
        assert_eq!(step(&mut s, &f, needs(), NativeArms::NONE), Step::Lend(Lent::Arm(Arm::Trade)));
    }

    #[test]
    fn the_turn_prelude_prepares_the_turn_and_hands_the_hooks_on() {
        let mut s = table(Phase::Action);
        s.meta.active = Some(Seat(1));
        s.meta.turn_seq = 4;
        let f = facts(&s);
        assert_eq!(step_arm(&f, true), Arm::TurnPrelude);
        let out = step(&mut s, &f, Needs { asks: true, ..needs() }, NativeArms::NONE);
        assert_eq!(out, Step::Ran { arm: Arm::TurnPrelude, lent: Some(Lent::TurnHooks) });
        assert_eq!(s.pending.prepared_turn_seq, Some(4), "the turn is the prepared one");
    }

    #[test]
    fn the_end_of_the_action_phase_runs_in_the_core_once_everyone_has_passed() {
        let mut s = table(Phase::Action);
        s.meta.active = Some(Seat(0));
        s.pending.prepared_turn_seq = Some(s.meta.turn_seq);
        for seat in [Seat(0), Seat(1)] {
            s.players.edit(usize::from(seat.0)).econ.passed = true;
        }
        let f = Facts::of(&s, false, true, false, true);
        assert_eq!(step_arm(&f, false), Arm::Phase);
        let out = step(&mut s, &f, needs(), NativeArms::NONE);
        assert_eq!(out, Step::Ran { arm: Arm::Phase, lent: Some(Lent::PhaseEvents) });
        assert_eq!((s.meta.phase, s.meta.active), (Phase::Status, None));
    }

    #[test]
    fn the_strategy_phase_flips_into_the_action_phase_with_the_first_in_initiative() {
        let mut s = table(Phase::Strategy);
        s.players.edit(1).econ.stability = true;
        let f = Facts::of(&s, false, false, false, true);
        assert_eq!(step_arm(&f, false), Arm::Phase);
        let out = step(&mut s, &f, needs(), NativeArms::NONE);
        assert_eq!(out, Step::Ran { arm: Arm::Phase, lent: Some(Lent::StrategyHooks) });
        assert_eq!(s.meta.phase, Phase::Action);
        assert!(s.meta.active.is_some(), "the first turn began");
        assert_eq!((s.meta.action_turn, s.meta.turn_seq), (1, 1));
        assert!(!s.players.edit(1).econ.stability, "stability is spent");
    }

    #[test]
    fn an_arm_is_native_only_when_it_is_in_the_set_and_has_a_live_function() {
        let open = |kind: crate::state::WindowKind| {
            let mut s = table(Phase::Status);
            s.pending.stack.push(crate::state::Window::Untyped { kind, actor: None, data: [0; 3] });
            s
        };
        let mut s = open(crate::state::WindowKind::Tokens);
        let f = facts(&s);
        assert_eq!(step(&mut s, &f, needs(), NativeArms::NONE), Step::Lend(Lent::Arm(Arm::Tokens)), "the empty set is today's behaviour");
        assert_eq!(step(&mut s, &f, needs(), NativeArms::of(&[Arm::Tokens])), Step::Native(Arm::Tokens));
        assert_eq!(step(&mut s, &f, needs(), NativeArms::registered()), Step::Native(Arm::Tokens));
        assert_eq!(step(&mut s, &f, needs(), NativeArms::of(&[Arm::Voting])), Step::Lend(Lent::Arm(Arm::Tokens)), "another arm's bit does not count");
        // An arm in the set with no live function yet is still lent: the set can be written before the lane lands.
        let mut s = open(crate::state::WindowKind::Voting);
        let f = facts(&s);
        assert_eq!(step(&mut s, &f, needs(), NativeArms::ALL), Step::Lend(Lent::Arm(Arm::Voting)));
        assert_eq!(NativeArms::registered(), NativeArms::of(&[Arm::Tokens, Arm::EventScoring, Arm::Aftermath, Arm::Tactical, Arm::Invasion]));
        assert_eq!(NativeArms::ALL.bits().count_ones(), 20);
        assert!(NativeArms::NONE.bits() == 0 && !NativeArms::NONE.has(Arm::Tokens));
    }

    #[test]
    fn a_question_is_named_by_the_arm_that_asks_it() {
        let mut s = table(Phase::Action);
        s.meta.active = Some(Seat(0));
        s.pending.prepared_turn_seq = Some(s.meta.turn_seq);
        let f = Facts::of(&s, false, true, false, true);
        assert_eq!(step(&mut s, &f, Needs { asks: true, ..needs() }, NativeArms::NONE), Step::Asks(AskArm::ActionMenu));
        let f = Facts::of(&s, false, true, true, true);
        assert_eq!(step(&mut s, &f, Needs { asks: true, ..needs() }, NativeArms::NONE), Step::Asks(AskArm::Closing));
    }
}
