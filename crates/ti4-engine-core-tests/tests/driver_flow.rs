//! D4-2b: the pure parts of the driver: the action menu (`driver::menu`), the chain after a movement step (`driver::aftermath`) and the
//! turn flow (`driver::turn_flow`), on states built by the scenario kit.

use ti4_engine_core::driver::aftermath::{announced, next, note_feats, opens_invasion, Settled};
use ti4_engine_core::driver::menu::{closing_menu, second_action, turn_menu, Entry, Source, ORDER};
use ti4_engine_core::driver::turn_flow::{self, Announce, Finish};
use ti4_engine_core::state::{AStage, PlainKind, TransientFlags};
use ti4_engine_core_tests::prelude::*;

fn parts(of: &[(Source, &'static str)]) -> Vec<(Source, Vec<&'static str>)> {
    of.iter().map(|(s, o)| (*s, vec![*o])).collect()
}

#[test]
fn the_menu_lists_each_present_source_in_menu_order_whatever_order_the_parts_come_in() {
    covers!("driver::menu::turn_menu");
    let given = [(Source::ActionCards, "card"), (Source::Strategic, "strategic"), (Source::Leaders, "leader"), (Source::Tactical, "tactical")];
    let forward = turn_menu(&parts(&given), false, &[]);
    let mut reversed = given;
    reversed.reverse();
    assert_eq!(forward, ["strategic", "tactical", "leader", "card"]);
    assert_eq!(turn_menu(&parts(&reversed), false, &[]), forward, "the parts' order is irrelevant");
    assert!(turn_menu::<&str>(&[], false, &[]).is_empty());
}

#[test]
fn every_source_in_the_order_is_reachable_and_thunders_edge_vanishes_only_when_it_is_on() {
    covers!("driver::menu::turn_menu");
    let all: Vec<(Source, &'static str)> = ORDER.iter().map(|s| (*s, "x")).collect();
    let mut tagged: Vec<(Source, Vec<String>)> = ORDER.iter().map(|s| (*s, vec![format!("{s:?}")])).collect();
    let off = turn_menu(&tagged, false, &[]);
    assert_eq!(off.len(), 13);
    let on = turn_menu(&tagged, true, &[]);
    assert_eq!(on.len(), 12);
    assert!(!on.contains(&"ThundersEdge".to_string()));
    tagged.reverse();
    assert_eq!(turn_menu(&tagged, false, &[]), off);
    let _ = all;
}

#[test]
fn failed_options_are_filtered_out_wherever_they_came_from() {
    covers!("driver::menu::turn_menu");
    let given = [(Source::Strategic, "pass"), (Source::Leaders, "agent"), (Source::Faction, "ability")];
    assert_eq!(turn_menu(&parts(&given), false, &["agent", "never offered"]), ["pass", "ability"]);
}

fn entries() -> Vec<Entry<&'static str>> {
    vec![
        Entry { option: "pass", free: false, pass: true },
        Entry { option: "move", free: false, pass: false },
        Entry { option: "trade", free: true, pass: false },
    ]
}

#[test]
fn the_closing_menu_starts_with_ending_the_turn_and_a_second_action_brings_back_everything_but_pass() {
    covers!("driver::menu::closing_menu");
    assert_eq!(closing_menu("end", &entries(), false, &[]), ["end", "trade"]);
    assert_eq!(closing_menu("end", &entries(), true, &["claim"]), ["end", "move", "trade", "claim"]);
    assert_eq!(closing_menu("end", &[], true, &[]), ["end"]);
}

#[test]
fn a_second_action_needs_fleet_logistics_and_fewer_than_two_actions() {
    covers!("driver::menu::second_action");
    let plain = Scenario::lobby(2).state();
    assert!(!second_action(&plain, Seat(0), 0));
    let with = Scenario::lobby(2).tech(0, Technology::FleetLogistics).state();
    assert!(second_action(&with, Seat(0), 0));
    assert!(second_action(&with, Seat(0), 1));
    assert!(!second_action(&with, Seat(0), 2));
    assert!(!second_action(&with, Seat(1), 0), "the other seat lacks the technology");
}

#[test]
fn the_aftermath_chain_reaches_done_from_every_start_whatever_the_facts() {
    covers!("driver::aftermath::next");
    for holds in [false, true] {
        for contacts in [false, true] {
            let mut stage = next(Settled::Fighting { holds, contacts });
            let mut steps = 0;
            while stage != AStage::Done {
                steps += 1;
                assert!(steps < 6, "the chain must end: {stage:?}");
                stage = match stage {
                    AStage::Negotiating { invade } => next(Settled::Negotiating { invade }),
                    AStage::Invading => next(Settled::Invading { contacts }),
                    AStage::Producing => next(Settled::Producing),
                    AStage::Fighting | AStage::Done => unreachable!(),
                };
            }
        }
    }
}

#[test]
fn the_invasion_only_follows_a_fight_the_active_seat_holds() {
    covers!("driver::aftermath::next");
    assert_eq!(next(Settled::Fighting { holds: true, contacts: false }), AStage::Invading);
    assert_eq!(next(Settled::Fighting { holds: false, contacts: false }), AStage::Producing);
    assert_eq!(next(Settled::Invading { contacts: true }), AStage::Negotiating { invade: false }, "never a second invasion");
    assert!(opens_invasion(AStage::Invading));
    for other in [AStage::Fighting, AStage::Producing, AStage::Done, AStage::Negotiating { invade: true }] {
        assert!(!opens_invasion(other), "{other:?}");
    }
}

#[test]
fn announcements_and_the_feat_note() {
    covers!("driver::aftermath::announced");
    assert_eq!(announced(Settled::Fighting { holds: false, contacts: true }, true), Some(PlainKind::SpaceCombatResolved));
    assert_eq!(announced(Settled::Fighting { holds: false, contacts: true }, false), None);
    assert_eq!(announced(Settled::Producing, false), Some(PlainKind::ProductionResolved));
    assert_eq!(announced(Settled::Invading { contacts: false }, false), Some(PlainKind::InvasionResolved));
    assert_eq!(announced(Settled::Negotiating { invade: false }, true), None);
    for (noted, outcome, want) in [(false, true, true), (true, true, false), (false, false, false), (true, false, false)] {
        assert_eq!(note_feats(noted, outcome), want, "noted {noted}, outcome {outcome}");
    }
}

/// Three seats with initiative 1, 2, 3 in seat order, in the action phase on seat 0's turn.
fn table() -> CoreState {
    let mut s = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).state();
    for (i, card) in StrategyCard::ALL[..3].iter().enumerate() {
        s.strategy_mut().initiative.insert(*card, i as i32 + 1);
        s.players_mut().by_seat_mut(Seat(i as u8)).unwrap().strat.push(*card);
    }
    s
}

fn passed(s: &mut CoreState, seat: u8) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().econ.passed = true;
}

#[test]
fn the_turn_walks_the_initiative_order_around_the_table_and_skips_the_seats_that_passed() {
    covers!("driver::turn_flow::advance_turn");
    let mut s = table();
    let turn0 = s.meta().turn_seq;
    assert_eq!(turn_flow::advance_turn(&mut s), Some(Seat(1)));
    assert_eq!(turn_flow::advance_turn(&mut s), Some(Seat(2)));
    assert_eq!(turn_flow::advance_turn(&mut s), Some(Seat(0)), "round the table");
    assert_eq!(s.meta().turn_seq, turn0 + 3);
    passed(&mut s, 1);
    assert_eq!(turn_flow::advance_turn(&mut s), Some(Seat(2)));
    passed(&mut s, 0);
    passed(&mut s, 2);
    assert!(turn_flow::all_passed(&s));
    let seq = s.meta().turn_seq;
    assert_eq!(turn_flow::advance_turn(&mut s), None);
    assert_eq!((s.meta().active, s.meta().turn_seq), (None, seq));
}

#[test]
fn ending_the_action_phase_enters_the_status_phase() {
    covers!("driver::turn_flow::action_phase_ended");
    let mut s = table();
    turn_flow::action_phase_ended(&mut s);
    assert_eq!((s.meta().phase, s.meta().active), (Phase::Status, None));
}

#[test]
fn an_action_counts_and_opens_the_closing_menu_unless_the_turn_is_retained() {
    covers!("driver::turn_flow::finish_action");
    let mut s = table();
    assert_eq!(turn_flow::finish_action(&mut s, 3), Finish::Closing);
    assert_eq!(s.pending().actions_this_turn, 1);
    assert_eq!(s.pending().turn_closing, Some(Seat(0)));
    let mut only_end = table();
    assert_eq!(turn_flow::finish_action(&mut only_end, 1), Finish::Advance);
    assert_eq!(only_end.pending().turn_closing, None);
    let mut retained = table();
    retained.transient_mut().misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
    assert_eq!(turn_flow::finish_action(&mut retained, 3), Finish::Advance);
    assert_eq!(retained.pending().actions_this_turn, 0);
    assert!(turn_flow::take_retention(&mut retained));
    assert!(!turn_flow::take_retention(&mut retained), "the flag is spent");
}

#[test]
fn passing_on_announces_skips_and_puppet_grants_in_the_order_the_string_pushes_them() {
    covers!("driver::turn_flow::pass_on");
    let mut s = table();
    assert_eq!(turn_flow::pass_on(&mut s, Some(Seat(0))).as_slice(), &[Announce::Passed(Seat(0))]);
    assert_eq!(s.meta().active, Some(Seat(1)));
    let mut crisis = table();
    crisis.transient_mut().misc.flags.set(TransientFlags::SKIP_NEXT_TURN);
    assert_eq!(turn_flow::pass_on(&mut crisis, Some(Seat(0))).as_slice(), &[Announce::Passed(Seat(0)), Announce::Skipped(Seat(1))]);
    assert_eq!(crisis.meta().active, Some(Seat(2)));
    let mut puppet = table();
    for i in 0..3 {
        passed(&mut puppet, i);
    }
    puppet.transient_mut().misc.flags.set(TransientFlags::PUPPET_ACTION);
    assert_eq!(turn_flow::pass_on(&mut puppet, Some(Seat(1))).as_slice(), &[Announce::Passed(Seat(1)), Announce::Puppet(Seat(1))]);
    assert_eq!(puppet.meta().active, Some(Seat(1)));
    assert!(turn_flow::pass_on(&mut table(), None).is_empty());
}

#[test]
fn the_next_round_lays_out_the_mat_and_restarts_the_turn_counters() {
    covers!("driver::turn_flow::begin_next_round");
    let mut s = table();
    s.meta_mut().phase = Phase::Status;
    s.meta_mut().round = 2;
    s.meta_mut().action_turn = 5;
    turn_flow::begin_next_round(&mut s, &StrategyCard::ALL[..4]).expect("no promise to settle");
    assert_eq!((s.meta().phase, s.meta().round, s.meta().action_turn, s.meta().active), (Phase::Strategy, 3, 0, None));
    assert!(!s.strategy().unclaimed.as_slice().is_empty());
}

#[test]
fn end_of_turn_scratch_clears_the_two_markers_only() {
    covers!("driver::turn_flow::end_turn_scratch");
    let mut s = table();
    for flag in [TransientFlags::BLACK_MARKET, TransientFlags::EXPEDITION_CLAIMED, TransientFlags::SKIP_NEXT_TURN] {
        s.transient_mut().misc.flags.set(flag);
    }
    turn_flow::end_turn_scratch(&mut s);
    let f = s.transient().misc.flags;
    assert!(!f.has(TransientFlags::BLACK_MARKET) && !f.has(TransientFlags::EXPEDITION_CLAIMED) && f.has(TransientFlags::SKIP_NEXT_TURN));
}

#[test]
fn the_children_table_is_complete_once_registered_and_empty_before() {
    covers!("driver::children::register");
    let mut table = ti4_engine_core::kernel::stage::AbilityTable::default();
    assert!(!table.links.complete(), "a table nobody registered on declines its children");
    ti4_engine_core::driver::children::register(&mut table);
    assert!(table.links.complete());
}
