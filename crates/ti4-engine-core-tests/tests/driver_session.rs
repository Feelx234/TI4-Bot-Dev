//! D4-2b: `driver::session` (`step_arm`, `ask_arm`: which arm of the turn loop runs) over every combination of open windows.

use ti4_engine_core::driver::session::{Arm, AskArm, Facts, ask_arm, bit, step_arm};
use ti4_engine_core::state::{Phase, WindowKind as K};

const KINDS: [K; 15] = [
    K::StrategySecondary,
    K::StrategySecondaryAfterTactical,
    K::LeaderStrategy,
    K::LeaderFollowers,
    K::Scoring,
    K::EventScoring,
    K::Tokens,
    K::Voting,
    K::ExecutiveOrder,
    K::AgendaQueueAfterEventScoring,
    K::Tactical,
    K::Aftermath,
    K::Trade,
    K::Diplomacy,
    K::AgendaTalks,
];

fn facts(open: u32, phase: Phase, flags: u8) -> Facts {
    Facts {
        finished: false,
        blocked: false,
        open,
        phase,
        turn_prepared: flags & 1 != 0,
        has_active: flags & 2 != 0,
        status_resolved: flags & 4 != 0,
        agenda_resolved: flags & 8 != 0,
        strategy_announced: flags & 16 != 0,
        round: 1 + u32::from(flags >> 5),
        starting_picks_done: flags & 32 != 0,
        turn_closing: flags & 64 != 0,
    }
}

#[test]
fn every_window_kind_has_its_own_bit() {
    let mut seen = 0u32;
    for k in KINDS {
        assert_eq!(bit(k).count_ones(), 1);
        assert_eq!(seen & bit(k), 0, "{k:?} shares a bit");
        seen |= bit(k);
    }
    assert_eq!(seen.count_ones(), 15);
}

#[test]
fn a_finished_game_and_a_blocked_one_override_every_window_and_every_phase() {
    for open in [0u32, bit(K::Tactical), bit(K::Voting) | bit(K::Trade), (1 << 15) - 1] {
        for phase in [Phase::Strategy, Phase::Action, Phase::Status, Phase::Agenda] {
            let done = Facts { finished: true, blocked: true, ..facts(open, phase, 0xff) };
            assert_eq!(step_arm(&done, true), Arm::Finished, "finished wins over blocked");
            assert_eq!(ask_arm(&done), AskArm::None);
            let blocked = Facts { blocked: true, ..facts(open, phase, 0xff) };
            assert_eq!(step_arm(&blocked, true), Arm::Blocked);
            assert_eq!(ask_arm(&blocked), AskArm::None);
        }
    }
}

#[test]
fn the_step_chain_takes_the_first_open_window_in_the_documented_order() {
    let order = [
        (K::LeaderFollowers, Arm::LeaderFollowers),
        (K::StrategySecondary, Arm::Secondary),
        (K::EventScoring, Arm::EventScoring),
        (K::Scoring, Arm::Scoring),
        (K::Tokens, Arm::Tokens),
        (K::Voting, Arm::Voting),
        (K::Diplomacy, Arm::Diplomacy),
        (K::Aftermath, Arm::Aftermath),
        (K::AgendaTalks, Arm::AgendaTalks),
        (K::Trade, Arm::Trade),
        (K::Tactical, Arm::Tactical),
    ];
    for (i, (kind, arm)) in order.iter().enumerate() {
        // The window alone gives its arm, whatever the phase.
        for phase in [Phase::Strategy, Phase::Action, Phase::Status, Phase::Agenda] {
            assert_eq!(step_arm(&facts(bit(*kind), phase, 0), false), *arm, "{kind:?} alone in {phase:?}");
        }
        // It beats every window after it in the chain.
        let later = order[i + 1..].iter().fold(0, |m, (k, _)| m | bit(*k));
        assert_eq!(step_arm(&facts(bit(*kind) | later, Phase::Action, 0xff), true), *arm, "{kind:?} before the rest");
    }
}

#[test]
fn windows_that_no_arm_names_never_change_the_arm() {
    let ignored = bit(K::StrategySecondaryAfterTactical) | bit(K::LeaderStrategy) | bit(K::ExecutiveOrder) | bit(K::AgendaQueueAfterEventScoring);
    for phase in [Phase::Strategy, Phase::Action, Phase::Status, Phase::Agenda] {
        for flags in 0..=127u8 {
            for asks in [false, true] {
                assert_eq!(step_arm(&facts(ignored, phase, flags), asks), step_arm(&facts(0, phase, flags), asks), "{phase:?} {flags}");
            }
            assert_eq!(ask_arm(&facts(ignored, phase, flags)), ask_arm(&facts(0, phase, flags)));
        }
    }
}

#[test]
fn with_no_window_open_the_phase_decides() {
    // Action phase: the turn prelude until the turn is prepared (and somebody is active), then the question or the phase step.
    assert_eq!(step_arm(&Facts { turn_prepared: false, has_active: true, ..facts(0, Phase::Action, 0) }, true), Arm::TurnPrelude);
    assert_eq!(step_arm(&Facts { turn_prepared: false, has_active: false, ..facts(0, Phase::Action, 0) }, true), Arm::Ask, "nobody active: no prelude");
    assert_eq!(step_arm(&Facts { turn_prepared: true, has_active: true, ..facts(0, Phase::Action, 0) }, false), Arm::Phase);
    // Status and agenda: once, until resolved.
    assert_eq!(step_arm(&Facts { status_resolved: false, ..facts(0, Phase::Status, 0) }, true), Arm::Status);
    assert_eq!(step_arm(&Facts { status_resolved: true, ..facts(0, Phase::Status, 0) }, true), Arm::Ask);
    assert_eq!(step_arm(&Facts { agenda_resolved: false, ..facts(0, Phase::Agenda, 0) }, true), Arm::Agenda);
    assert_eq!(step_arm(&Facts { agenda_resolved: true, ..facts(0, Phase::Agenda, 0) }, false), Arm::Phase);
    // The first strategy phase announces once the starting picks are done.
    let announce = Facts { round: 1, strategy_announced: false, starting_picks_done: true, ..facts(0, Phase::Strategy, 0) };
    assert_eq!(step_arm(&announce, true), Arm::StrategyAnnounce);
    assert_eq!(step_arm(&Facts { round: 2, ..announce }, true), Arm::Ask);
    assert_eq!(step_arm(&Facts { strategy_announced: true, ..announce }, true), Arm::Ask);
    assert_eq!(step_arm(&Facts { starting_picks_done: false, ..announce }, true), Arm::Ask);
}

#[test]
fn the_ask_chain_agrees_with_the_step_chain_wherever_both_name_the_same_window() {
    let pairs = [
        (K::LeaderFollowers, Arm::LeaderFollowers, AskArm::LeaderFollowers),
        (K::EventScoring, Arm::EventScoring, AskArm::EventScoring),
        (K::Scoring, Arm::Scoring, AskArm::Scoring),
        (K::Tokens, Arm::Tokens, AskArm::Tokens),
        (K::Voting, Arm::Voting, AskArm::Voting),
        (K::Diplomacy, Arm::Diplomacy, AskArm::Diplomacy),
        (K::Aftermath, Arm::Aftermath, AskArm::Aftermath),
        (K::AgendaTalks, Arm::AgendaTalks, AskArm::AgendaTalks),
        (K::Tactical, Arm::Tactical, AskArm::Tactical),
    ];
    // Every subset of the nine windows, with neither the secondary nor the trade window open (those two arms differ by design).
    for mask in 0u32..(1 << pairs.len()) {
        let open = pairs.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).fold(0, |m, (_, (k, _, _))| m | bit(*k));
        let step = step_arm(&facts(open, Phase::Action, 0xff), true);
        let ask = ask_arm(&facts(open, Phase::Action, 0xff));
        match pairs.iter().find(|(_, a, _)| *a == step) {
            Some((_, _, expected)) => assert_eq!(ask, *expected, "open {open:b}"),
            None => assert_eq!(open, 0, "an open window must give a window arm ({step:?})"),
        }
    }
}

#[test]
fn the_ask_after_the_windows_follows_the_phase() {
    let base = |phase, flags| facts(0, phase, flags);
    assert_eq!(ask_arm(&Facts { starting_picks_done: false, ..base(Phase::Strategy, 0) }), AskArm::StartingTech);
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, ..base(Phase::Strategy, 0) }), AskArm::Draft);
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, turn_closing: false, ..base(Phase::Action, 0) }), AskArm::ActionMenu);
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, turn_closing: true, ..base(Phase::Action, 0) }), AskArm::Closing);
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, ..facts(bit(K::StrategySecondary), Phase::Action, 0) }), AskArm::Secondary);
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, turn_closing: true, ..facts(bit(K::StrategySecondary), Phase::Action, 0) }), AskArm::Secondary, "the follower window comes before the closing menu");
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, ..base(Phase::Status, 0) }), AskArm::Nothing);
    assert_eq!(ask_arm(&Facts { starting_picks_done: true, ..base(Phase::Agenda, 0) }), AskArm::Nothing);
    assert_eq!(ask_arm(&Facts { starting_picks_done: false, ..facts(bit(K::Tactical), Phase::Strategy, 0) }), AskArm::Tactical, "a window before the starting picks");
}

#[test]
fn both_arms_are_total_over_every_combination() {
    let mut n = 0u64;
    for open in 0u32..(1 << 15) {
        for phase in [Phase::Strategy, Phase::Action, Phase::Status, Phase::Agenda] {
            for flags in [0u8, 0x3f, 0x7f, 0b0101_0101, 0b0010_1010] {
                let f = facts(open, phase, flags);
                let _ = (step_arm(&f, false), step_arm(&f, true), ask_arm(&f));
                n += 1;
            }
        }
    }
    assert_eq!(n, 32768 * 4 * 5);
}
