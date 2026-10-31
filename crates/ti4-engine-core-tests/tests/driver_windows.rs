//! D4A4: the windows of `driver::windows`: the head of the aftermath chain (enforcement, space cannon offense, the snapshot and the
//! opening of the fight), the end of a space combat, the fleet enforcement of every seat, and the Minister of War.
//!
//! Expectations come from the rules text: the fleet supply is the number of command tokens in the fleet pool (LRR 3.1/"fleet pool"),
//! "if a player has more ships in a system than the fleet supply allows, they remove ships of their choice" (LRR 33 at the end of a
//! turn, 22.3); Minister of War: "after performing an action ... discard this card to remove 1 of your command tokens from the game
//! board and return it to your reinforcements -- then you may perform 1 additional action".

use ti4_engine_core::choice::agenda::Opt as AOpt;
use ti4_engine_core::choice::tactical::{Ask as TAsk, Over};
use ti4_engine_core::choice::turn::Opt as TOpt;
use ti4_engine_core::driver::windows::aftermath::{FightOver, Facts, Head, OverFacts, OverStage, Stage as HeadStage};
use ti4_engine_core::driver::windows::enforce_all::EnforceAll;
use ti4_engine_core::driver::windows::minister::MinisterOfWar;
use ti4_engine_core::generated::Agenda;
use ti4_engine_core::rules::combat::feats::Outcome;
use ti4_engine_core::rules::combat::space_cannon_offense::Facts as CannonFacts;
use ti4_engine_core::rules::tactical::fleet::Limits;
use ti4_engine_core::state::{Code, TransientFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn other() -> System {
    plain_systems()[1]
}

fn limits(n: i64) -> Limits {
    Limits { enforce: n, shown: n, halves: [None, None] }
}

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys(), other()])
}

fn count(s: &CoreState, system: System, seat: u8, unit: Unit) -> usize {
    s.board().sys(system).units.as_slice().iter().filter(|p| p.kind() == unit && p.owner() == Seat(seat)).count()
}

// ---- EnforceAll -------------------------------------------------------------------------------

#[test]
fn enforce_all_walks_the_pairs_in_order_and_asks_each_owner_about_their_own_overflow() {
    covers!("driver::windows::enforce_all::EnforceAll");
    let sc = || table().unit(sys(), None, Unit::CruiserI, 0, 3).unit(other(), None, Unit::DestroyerI, 1, 2).unit(other(), None, Unit::CruiserI, 0, 1);
    let pairs = || vec![(Seat(0), sys(), limits(1)), (Seat(1), other(), limits(1)), (Seat(0), other(), limits(9))];
    let mut rig = Rig::new(sc(), EnforceAll::new(pairs()));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the first pair is the first seat's");
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })));
    rig.assert_options_sound();
    rig.pick(0);
    rig.pick(0);
    assert_eq!(rig.asked().actor, Seat(1), "then the second pair, asked of its own owner");
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.removed, 3, "two cruisers and one destroyer");
    assert_eq!(count(&rig.state, sys(), 0, Unit::CruiserI), 1);
    assert_eq!(count(&rig.state, other(), 1, Unit::DestroyerI), 1);
    assert_eq!(count(&rig.state, other(), 0, Unit::CruiserI), 1, "a fleet that fits is left alone");
    smoke("driver_windows/enforce_all", || Rig::new(sc(), EnforceAll::new(pairs())));
}

#[test]
fn enforce_all_with_nothing_over_or_no_pairs_asks_nothing() {
    covers!("driver::windows::enforce_all::EnforceAll");
    let mut none = Rig::new(table(), EnforceAll::new(vec![]));
    none.run();
    assert!(none.is_done());
    assert_eq!(none.driver.window.removed, 0);
    let mut fits = Rig::new(table().unit(sys(), None, Unit::CruiserI, 0, 2), EnforceAll::new(vec![(Seat(0), sys(), limits(2))]));
    fits.run();
    assert!(fits.is_done());
    assert_eq!(count(&fits.state, sys(), 0, Unit::CruiserI), 2);
}

// ---- Minister of War --------------------------------------------------------------------------

fn minister_state(tokens: &[System]) -> CoreState {
    let mut sc = table();
    for s in tokens {
        sc = sc.command_token(*s, 0);
    }
    let mut st = sc.state();
    st.agenda_mut().laws.insert(Agenda::MinisterOfWar, ti4_engine_core::state::Outcome::Empty);
    st
}

fn minister(tokens: &[System]) -> Rig<MinisterOfWar> {
    Rig::from_parts(minister_state(tokens), kernel_all(), MinisterOfWar::new(Seat(0), tokens.to_vec(), Agenda::MinisterOfWar, Code(1), Code(2)))
}

#[test]
fn the_minister_of_war_discards_the_law_takes_a_token_back_to_the_tactic_pool_and_keeps_the_turn() {
    covers!("driver::windows::minister::MinisterOfWar");
    let mut rig = minister(&[sys(), other()]);
    let before = rig.state.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens;
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Agenda(AOpt::DiscardLaw), Opt::Decline]);
    rig.expect_illegal(Pick::Opt(Opt::Turn(TOpt::Token(sys()))));
    rig.pick(0);
    assert!(rig.driver.window.discarded);
    assert!(!rig.state.agenda().laws.contains_key(&Agenda::MinisterOfWar), "the card is gone");
    assert_eq!(rig.opts(), vec![Opt::Turn(TOpt::Token(sys())), Opt::Turn(TOpt::Token(other()))]);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Turn(TOpt::Token(plain_systems()[2]))));
    rig.pick(1);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.retrieved, Some(other()));
    assert!(!rig.state.board().sys(other()).cmd.contains(Seat(0)), "the token leaves the board");
    assert!(rig.state.board().sys(sys()).cmd.contains(Seat(0)), "the other token stays");
    let after = rig.state.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens;
    assert_eq!(after, before + 1, "it returns to the tactic pool");
    assert!(rig.state.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION), "one additional action");
}

#[test]
fn declining_the_minister_of_war_or_having_no_token_changes_nothing() {
    covers!("driver::windows::minister::MinisterOfWar");
    let mut rig = minister(&[sys()]);
    rig.run().decline();
    assert!(rig.is_done());
    assert!(!rig.driver.window.discarded && rig.driver.window.retrieved.is_none());
    assert!(rig.state.agenda().laws.contains_key(&Agenda::MinisterOfWar), "declined: the law stays");
    assert!(!rig.state.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION));
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    let mut none = minister(&[]);
    none.run();
    assert!(none.is_done(), "no token on the board: nothing is offered");
    assert!(none.state.agenda().laws.contains_key(&Agenda::MinisterOfWar));
    smoke_quiet(|| minister(&[sys(), other()]));
}

// ---- Head of the aftermath chain --------------------------------------------------------------

fn head_facts(skip: bool) -> Facts {
    Facts {
        player: Seat(0),
        system: sys(),
        limits: limits(9),
        cannon: CannonFacts { active: Seat(0), system: sys(), announce: false, staging: false, reaching: vec![], mapped: false, kind: Code(1), cause: Code(2), casualty: Code(3), commander: Code(4), jolnar: vec![], unit_texts: vec![], planet_texts: vec![] },
        notes: Default::default(),
        skip_to_commit: skip,
        pre_roll: Code(77),
    }
}

fn fight() -> Scenario {
    table().unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::DestroyerI, 1, 1)
}

#[test]
fn the_head_opens_a_two_sided_fight_with_the_snapshot_of_the_system_before_it() {
    covers!("driver::windows::aftermath::Head");
    let mut rig = Rig::new(fight(), Head::new(head_facts(false)));
    rig.run();
    assert!(rig.is_done(), "nothing over the supply and no guns: it runs straight through");
    let before = rig.driver.window.before.clone().expect("the snapshot");
    assert_eq!(before.sides.as_slice(), &[Seat(0), Seat(1)], "both sides had ships, in seating order");
    assert_eq!(rig.state.transient().combat.active_space_combat, Some((sys(), Seat(0), Seat(1))), "the active seat attacks");
    assert_eq!(rig.state.transient().combat.presentation.phase, Code(77), "the presentation phase is the interned pre_roll");
    assert_eq!(rig.state.transient().combat.presentation.battle_seq, rig.start_state().meta().combat_round_seq + 1);
    assert!(rig.driver.window.drawn.is_empty());
    assert_eq!(rig.stages.last().map(String::as_str), Some("Done"));
    smoke_quiet(|| Rig::new(fight(), Head::new(head_facts(false))));
}

#[test]
fn the_head_with_one_side_only_opens_no_fight_and_the_sardakk_skip_goes_straight_to_the_snapshot() {
    covers!("driver::windows::aftermath::Head");
    let mut lone = Rig::new(table().unit(sys(), None, Unit::CruiserI, 0, 1), Head::new(head_facts(false)));
    lone.run();
    assert!(lone.is_done());
    assert_eq!(lone.driver.window.before.clone().unwrap().sides.as_slice(), &[Seat(0)]);
    assert!(lone.state.transient().combat.active_space_combat.is_none(), "one side: nothing to fight");
    let mut skip = Rig::new(fight(), Head::new(head_facts(true)));
    skip.run();
    assert!(skip.is_done());
    assert!(skip.driver.window.before.is_some(), "the snapshot is still taken");
    assert!(skip.state.transient().combat.active_space_combat.is_none(), "Commit Ground Forces step: no space combat");
    assert!(!skip.stages.iter().any(|s| s == "Firing"), "no space cannon either: {:?}", skip.stages);
}

#[test]
fn the_head_enforces_the_active_seats_fleet_before_the_guns_and_the_fight() {
    covers!("driver::windows::aftermath::Head");
    let sc = || table().unit(sys(), None, Unit::CruiserI, 0, 3).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let mut f = head_facts(false);
    f.limits = limits(1);
    let mut rig = Rig::new(sc(), Head::new(f.clone()));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })));
    assert!(rig.state.transient().combat.active_space_combat.is_none(), "the fight is not open yet");
    rig.pick(0);
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, sys(), 0, Unit::CruiserI), 1);
    assert!(rig.state.transient().combat.active_space_combat.is_some());
    assert!(rig.stages.iter().any(|s| s == "Enforcing") && rig.stages.iter().any(|s| s == "Done"));
    assert!(HeadStage::Enforcing != HeadStage::Done);
    smoke("driver_windows/head_enforce", || Rig::new(sc(), Head::new(f.clone())));
}

#[test]
fn the_head_fires_the_space_cannon_of_the_defender_at_the_attacker_before_the_fight() {
    covers!("driver::windows::aftermath::Head");
    let (p1, _) = (sys().data().planets[0], ());
    let sc = || table().unit(sys(), Some(p1), Unit::PDSI, 1, 2).control(p1, 1).unit(sys(), None, Unit::CruiserI, 0, 2);
    let mut seen_hits = [false; 3];
    for seed in 0..30u64 {
        let mut rig = Rig::new(sc(), Head::new(head_facts(false))).seeded(seed);
        rig.run();
        rig.auto(Policy::First);
        assert!(rig.is_done(), "{}", rig.transcript());
        assert_eq!(rig.driver.window.drawn.len(), 2, "one entry per PDS");
        let hits: usize = rig.driver.window.drawn.iter().map(|(on, faces)| faces.iter().filter(|f| **f >= *on).count()).sum();
        let left = count(&rig.state, sys(), 0, Unit::CruiserI);
        assert_eq!(left, 2 - hits, "each hit costs the attacker a ship");
        seen_hits[hits] = true;
        assert_eq!(rig.stages.iter().any(|s| s == "Firing"), hits > 0, "a hit is asked about in the Firing stage, a miss passes through it");
    }
    assert!(seen_hits[0] && seen_hits[1], "both a miss and a hit occur over 30 seeds: {seen_hits:?}");
}

use ti4_engine_core_tests::rig::Policy;

// ---- FightOver --------------------------------------------------------------------------------

fn over_facts(winner: Option<Seat>, rounds: u32, announce: bool) -> OverFacts {
    OverFacts {
        player: Seat(0),
        system: sys(),
        outcome: Outcome { winner, rounds },
        sides: [Seat(0), Seat(1)].into_iter().collect(),
        announce,
        opponents_key: Code(9),
        pairs: vec![],
    }
}

#[test]
fn the_end_of_a_fight_clears_the_scratch_remembers_the_loser_and_says_whether_an_invasion_follows() {
    covers!("driver::windows::aftermath::FightOver");
    let sc = || {
        let mut s = table().unit(sys(), None, Unit::CruiserI, 0, 1);
        s = s.active(Some(0));
        s
    };
    let mut st = sc().state();
    st.transient_mut().combat.active_space_combat = Some((sys(), Seat(0), Seat(1)));
    let mut rig = Rig::from_parts(st, kernel_all(), FightOver::new(over_facts(Some(Seat(0)), 3, false)));
    rig.run();
    assert!(rig.is_done());
    assert!(rig.driver.window.holds, "the active seat's ship holds the space: the invasion step follows");
    assert!(rig.state.transient().combat.active_space_combat.is_none(), "the fight's scratch is cleared");
    assert_eq!(rig.state.transient().combat.last_combat_sides, Some((sys(), [Seat(1)].into_iter().collect())), "the opponents of the winner");
    assert!(OverStage::Announcing != OverStage::Done);
}

#[test]
fn a_fight_of_no_rounds_or_a_draw_remembers_nobody_and_a_lost_fight_gives_no_invasion() {
    covers!("driver::windows::aftermath::FightOver");
    let mut st = table().unit(sys(), None, Unit::DestroyerI, 1, 1).state();
    st.transient_mut().combat.active_space_combat = Some((sys(), Seat(0), Seat(1)));
    let mut lost = Rig::from_parts(st.clone(), kernel_all(), FightOver::new(over_facts(Some(Seat(1)), 2, false)));
    lost.run();
    assert!(lost.is_done());
    assert!(!lost.driver.window.holds, "the active seat has no ship left: no invasion");
    assert_eq!(lost.state.transient().combat.last_combat_sides, Some((sys(), [Seat(0)].into_iter().collect())));
    let mut draw = Rig::from_parts(st.clone(), kernel_all(), FightOver::new(over_facts(None, 2, false)));
    draw.run();
    assert!(draw.state.transient().combat.last_combat_sides.is_none(), "a draw has no winner");
    let mut none = Rig::from_parts(st, kernel_all(), FightOver::new(over_facts(Some(Seat(0)), 0, false)));
    none.run();
    assert!(none.state.transient().combat.last_combat_sides.is_none(), "no rounds, no winner");
    assert!(none.state.transient().combat.active_space_combat.is_none());
}

#[test]
fn the_end_of_a_fight_then_enforces_every_listed_fleet() {
    covers!("driver::windows::aftermath::FightOver");
    let sc = || table().unit(sys(), None, Unit::CruiserI, 0, 3);
    let mut f = over_facts(Some(Seat(0)), 1, false);
    f.pairs = vec![(Seat(0), sys(), limits(2))];
    let mut rig = Rig::new(sc(), FightOver::new(f.clone()));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })));
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(count(&rig.state, sys(), 0, Unit::CruiserI), 2);
    assert!(rig.stages.iter().any(|s| s == "Enforcing"));
    smoke("driver_windows/fight_over", || Rig::new(sc(), FightOver::new(f.clone())));
}
