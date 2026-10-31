//! D4-2d: the small windows of the space combat: `AssignHits` (LRR 78.6 and 15.2a), `SustainOffer` and `SustainCommander` (LRR 87), `BeginRound`
//! (LRR 78.1/78.2 announcements), `Retreat` (LRR 78.7: the fleet leaves, cargo beyond capacity is lost, a command token goes to the destination).
//! Expected values come from the rules text, not from the string engine.

use ti4_engine_core::choice::Subtype;
use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub, SustainSite};
use ti4_engine_core::choice::leaders::Opt as LOpt;
use ti4_engine_core::generated::Leader;
use ti4_engine_core::rules::combat::assign::{Assign, AssignHits};
use ti4_engine_core::rules::combat::begin_round::{BeginRound, Facts as BeginFacts};
use ti4_engine_core::rules::combat::retreat::{Facts as RetreatFacts, Retreat};
use ti4_engine_core::rules::combat::sustain_commander::{pays_on_sustain, SustainCommander};
use ti4_engine_core::rules::combat::sustain_offer::{Offer, SustainOffer};
use ti4_engine_core::state::{Code, EventKind, Key, LeaderStatus, Piece};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn c(o: COpt) -> Opt {
    Opt::Combat(o)
}

fn sys() -> System {
    plain_systems()[0]
}

fn dest() -> System {
    plain_systems()[1]
}

/// Seat 0 (Muaat) fights seat 1 (Arborec) in `sys()`, `dest()` is next to it.
fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys(), dest()]).tokens(0, 3, 3, 2)
}

fn assign(a_staging: bool, non_fighters_first: bool) -> Assign {
    Assign { player: Seat(0), system: sys(), ability: false, non_fighters_first, subtype: Code(5), cause: Code(6), during_space_combat: true, staging: a_staging, announce: a_staging }
}

fn units_of(rig: &Rig<impl Window>, seat: u8) -> Vec<(Unit, bool)> {
    rig.state.board().sys(sys()).units.iter().filter(|p| p.owner() == Seat(seat)).map(|p| (p.kind(), p.sustained())).collect()
}

// ---- AssignHits ------------------------------------------------------------------------------

#[test]
fn a_single_candidate_is_lost_without_a_question() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc, AssignHits::new(assign(false, false), 1));
    rig.run();
    assert!(rig.is_done(), "one ship: nothing to choose");
    assert_eq!(units_of(&rig, 0), vec![]);
    assert_eq!(units_of(&rig, 1), vec![(Unit::CruiserI, false)], "the other side is untouched");
    assert_eq!(rig.driver.window.remaining, 0);
}

#[test]
fn identical_ships_are_one_option_and_a_damaged_one_is_another() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::FighterI, 0, 1);
    let (mut state, kernel) = sc.build();
    // The first dreadnought is damaged: it is a distinct loss from the healthy one.
    let first = state.board_mut().sys_mut(sys()).units.as_mut_slice().iter_mut().find(|p| p.kind() == Unit::DreadnoughtI).unwrap();
    *first = first.with_sustained(true);
    let mut rig = Rig::from_parts(state, kernel, AssignHits::new(assign(false, false), 1));
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::AssignHit));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::Casualty(Code(5))), "the caller's subtype text names the question");
    assert_eq!(rig.asked().actor, Seat(0), "the owner of the ships chooses");
    let kinds: Vec<(Unit, bool)> = rig.opts().iter().map(|o| if let Opt::Combat(COpt::Destroy { unit, damaged, .. }) = o { (*unit, *damaged) } else { panic!("{o:?}") }).collect();
    assert_eq!(kinds, vec![(Unit::DreadnoughtI, true), (Unit::DreadnoughtI, false), (Unit::CruiserI, false), (Unit::FighterI, false)], "one option per type and damage, in board order");
    rig.assert_options_sound();
    // The healthy dreadnought is chosen (option 1): the damaged one stays.
    rig.pick(1);
    assert!(rig.is_done());
    let left = units_of(&rig, 0);
    assert_eq!(left.iter().filter(|(u, _)| *u == Unit::DreadnoughtI).collect::<Vec<_>>(), vec![&(Unit::DreadnoughtI, true)], "the damaged dreadnought survived");
    assert_eq!(left.len(), 4);
}

#[test]
fn hits_beyond_the_ships_have_no_effect_and_the_last_ship_needs_no_choice() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 0, 1);
    let mut rig = Rig::new(sc, AssignHits::new(assign(false, false), 3));
    rig.run();
    assert_eq!(rig.asked().ctx.outstanding.as_slice()[0].amount, 3, "the question says how many hits are owed");
    rig.pick(0);
    assert!(rig.is_done(), "the second hit takes the only ship left without asking, the third finds nothing (15.2a)");
    assert_eq!(units_of(&rig, 0), vec![]);
}

#[test]
fn hits_bound_to_non_fighters_skip_the_fighters_while_a_non_fighter_remains() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::FighterI, 0, 2).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 0, 1);
    let mut rig = Rig::new(sc.clone(), AssignHits::new(assign(false, true), 1));
    rig.run();
    let kinds: Vec<Unit> = rig.opts().iter().filter_map(|o| if let Opt::Combat(COpt::Destroy { unit, .. }) = o { Some(*unit) } else { None }).collect();
    assert_eq!(kinds, vec![Unit::CruiserI, Unit::DestroyerI], "no fighter is offered");
    // Two hits: both non-fighters go, then the fighters become the only candidates.
    let mut two = Rig::new(sc, AssignHits::new(assign(false, true), 3));
    two.run();
    two.pick(0);
    let kinds: Vec<Unit> = two.opts().iter().filter_map(|o| if let Opt::Combat(COpt::Destroy { unit, .. }) = o { Some(*unit) } else { None }).collect();
    assert_eq!(kinds, vec![Unit::FighterI], "once only fighters are left they take the hit (one option: identical fighters)");
    two.pick(0);
    assert!(two.is_done());
    assert_eq!(units_of(&two, 0), vec![(Unit::FighterI, false)], "three hits, two non-fighters and one fighter lost");
}

#[test]
fn only_an_offered_loss_is_legal() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 0, 1);
    let mut rig = Rig::new(sc, AssignHits::new(assign(false, false), 1));
    rig.run();
    rig.expect_illegal(Pick::Opt(c(COpt::Destroy { index: 9, unit: Unit::CruiserI, damaged: false })));
    rig.expect_illegal(Pick::Opt(c(COpt::Stay)));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.expect_illegal(Pick::Number(1));
}

#[test]
fn with_staging_a_lost_ship_is_announced_and_the_last_one_says_so() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 0, 1);
    let mut rig = Rig::new(sc, AssignHits::new(assign(true, false), 2));
    rig.run();
    rig.pick(0);
    assert!(rig.is_done());
    let lost: Vec<_> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::ShipDestroyed).collect();
    assert_eq!(lost.len(), 2, "one SHIP_DESTROYED per ship lost");
    assert_eq!(lost[0].payload.unit(Key::Unit), Some(Unit::CruiserI));
    assert_eq!(lost[0].payload.flag(Key::Last), Some(false), "a ship is still left");
    assert_eq!(lost[1].payload.unit(Key::Unit), Some(Unit::DestroyerI));
    assert_eq!(lost[1].payload.flag(Key::Last), Some(true), "no ship of the owner is left");
    assert_eq!(lost[1].payload.seat(Key::Player), Some(Seat(0)));
    assert_eq!(lost[1].payload.flag(Key::DuringSpaceCombat), Some(true));
    assert_eq!(rig.state.transient().combat.last_ship_destroyed, Some((sys(), Seat(0), Unit::DestroyerI)));
}

#[test]
fn without_staging_nothing_is_announced() {
    covers!("rules::combat::assign::AssignHits");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 1);
    let mut rig = Rig::new(sc, AssignHits::new(assign(false, false), 1));
    rig.run();
    assert!(rig.is_done());
    assert!(rig.driver.emitted.is_empty());
    assert_eq!(rig.state.transient().combat.last_ship_destroyed, None);
}

// ---- SustainOffer ----------------------------------------------------------------------------

fn offer(in_combat: bool, staging: bool) -> Offer {
    Offer { player: Seat(0), producer: Seat(1), system: sys(), ability: false, in_combat, non_fighters_first: false, source: Code(9), staging, announce: true }
}

#[test]
fn sustain_damage_cancels_one_hit_damages_the_unit_and_announces_its_use() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::FighterI, 0, 2);
    let mut rig = Rig::new(sc, SustainOffer::new(offer(true, false), 1));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::SustainDamage));
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::CancelHit { hits: 1, in_combat: true, .. })));
    let opts = rig.opts();
    assert_eq!(opts.len(), 2, "one sustain per type (two identical dreadnoughts) and the decline: {opts:?}");
    assert!(matches!(opts[0], Opt::Combat(COpt::Sustain { site: SustainSite::Space(0), unit: Unit::DreadnoughtI, galvanized: false })));
    assert_eq!(opts[1], Opt::Decline);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done(), "the one hit is cancelled");
    assert_eq!(units_of(&rig, 0)[0], (Unit::DreadnoughtI, true));
    assert_eq!(units_of(&rig, 0)[1], (Unit::DreadnoughtI, false), "only the one that sustained is damaged");
    let used: Vec<_> = rig.driver.emitted.iter().filter(|e| e.kind == EventKind::SustainDamageUsed).collect();
    assert_eq!(used.len(), 1);
    assert_eq!(used[0].payload.unit(Key::Unit), Some(Unit::DreadnoughtI));
    assert_eq!(used[0].payload.seat(Key::Player), Some(Seat(0)));
    assert_eq!(used[0].payload.seat(Key::Producer), Some(Seat(1)));
    assert_eq!(used[0].payload.flag(Key::DuringSpaceCombat), Some(true));
    assert_eq!(rig.state.transient().combat.last_sustain, Some((sys(), Seat(0), Unit::DreadnoughtI, Seat(1), true)));
    assert!(rig.driver.window.last.is_some(), "the unit that sustained is remembered for Direct Hit");
    ti4_engine_core_tests::assert_snapshot("combat_small/sustain_one", &rig.snapshot());
}

#[test]
fn a_second_hit_asks_again_until_no_undamaged_sustainer_is_left() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 2).unit(sys(), None, Unit::WarSun, 0, 1);
    let mut rig = Rig::new(sc, SustainOffer::new(offer(true, false), 5));
    rig.run();
    assert_eq!(rig.opts().len(), 3, "dreadnought, war sun, decline");
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::CancelHit { hits: 4, .. })), "the owed hits drop by one");
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::CancelHit { hits: 3, .. })));
    assert_eq!(rig.opts().len(), 2, "both dreadnoughts are damaged now: the war sun and the decline");
    rig.pick(0);
    assert!(rig.is_done(), "nothing left that can sustain: the rest of the hits is for the assignment");
    assert!(units_of(&rig, 0).iter().all(|(_, damaged)| *damaged));
}

#[test]
fn declining_keeps_the_hits_and_the_units_undamaged() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1);
    let mut rig = Rig::new(sc, SustainOffer::new(offer(true, false), 2));
    rig.run();
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(units_of(&rig, 0), vec![(Unit::DreadnoughtI, false)]);
    assert!(rig.driver.emitted.is_empty());
}

#[test]
fn no_question_without_hits_or_without_a_sustainer() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let with = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1);
    let mut none = Rig::new(with, SustainOffer::new(offer(true, false), 0));
    none.run();
    assert!(none.is_done(), "no hits: nothing to cancel");
    let without = arena().unit(sys(), None, Unit::CruiserI, 0, 2).unit(sys(), None, Unit::FighterI, 0, 2);
    let mut rig = Rig::new(without, SustainOffer::new(offer(true, false), 2));
    rig.run();
    assert!(rig.is_done(), "cruisers and fighters have no SUSTAIN DAMAGE");
    let (mut state, kernel) = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).build();
    let only = state.board_mut().sys_mut(sys()).units.as_mut_slice().iter_mut().next().unwrap();
    *only = only.with_sustained(true);
    let mut spent = Rig::from_parts(state, kernel, SustainOffer::new(offer(true, false), 2));
    spent.run();
    assert!(spent.is_done(), "a damaged unit cannot sustain again");
}

#[test]
fn a_foreign_sustain_or_a_number_is_illegal() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), None, Unit::CruiserI, 0, 1);
    let mut rig = Rig::new(sc, SustainOffer::new(offer(true, false), 1));
    rig.run();
    rig.expect_illegal(Pick::Opt(c(COpt::Sustain { site: SustainSite::Space(1), unit: Unit::CruiserI, galvanized: false })));
    rig.expect_illegal(Pick::Opt(c(COpt::Stay)));
    rig.expect_illegal(Pick::Number(0));
}

#[test]
fn non_euclidean_shielding_cancels_two_hits_per_sustain() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    let sc = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec]).map_from(&[sys(), dest()]).unit(sys(), None, Unit::DreadnoughtI, 0, 1).tech(0, Technology::NonEuclideanShielding);
    let mut rig = Rig::new(sc, SustainOffer::new(offer(true, false), 2));
    rig.run();
    rig.pick(0);
    assert!(rig.is_done(), "both hits are cancelled by the one use");
    let without = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec]).map_from(&[sys(), dest()]).unit(sys(), None, Unit::DreadnoughtI, 0, 2);
    let mut plain = Rig::new(without, SustainOffer::new(offer(true, false), 2));
    plain.run();
    plain.pick(0);
    assert!(!plain.is_done(), "without the technology one use cancels one hit: the second dreadnought is offered");
}

#[test]
fn a_space_cannon_hit_is_not_a_combat_hit_for_the_sustain_of_a_combat_only_grant() {
    covers!("rules::combat::sustain_offer::SustainOffer");
    // The printed SUSTAIN DAMAGE works against any hit; only the module grants (Metali Void Shielding) are for combat hits.
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1);
    let mut rig = Rig::new(sc, SustainOffer::new(offer(false, false), 1));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Combat(CAsk::CancelHit { in_combat: false, .. })));
    rig.pick(0);
    assert!(rig.is_done());
    let used = rig.driver.emitted.iter().find(|e| e.kind == EventKind::SustainDamageUsed).unwrap();
    assert_eq!(used.payload.flag(Key::DuringSpaceCombat), Some(false));
}

// ---- SustainCommander (Rear Admiral Farran) --------------------------------------------------

fn farran(status: Option<LeaderStatus>) -> Rig<SustainOffer> {
    let sc = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec]).map_from(&[sys(), dest()]).unit(sys(), None, Unit::DreadnoughtI, 0, 1);
    let (mut state, kernel) = sc.build();
    if let Some(st) = status {
        state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::RearAdmiralFarran, st);
    }
    state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 0;
    Rig::from_parts(state, kernel, SustainOffer::new(offer(true, false), 1))
}

#[test]
fn the_unlocked_commander_offers_a_trade_good_after_the_sustain() {
    covers!("rules::combat::sustain_commander::SustainCommander");
    covers!("rules::combat::sustain_offer::SustainOffer");
    let mut rig = farran(Some(LeaderStatus::Unlocked));
    rig.run();
    rig.pick(0);
    assert!(!rig.is_done(), "the commander's question follows the sustain");
    assert_eq!(rig.asked().ask, Ask::Leaders(ti4_engine_core::choice::leaders::Ask::SustainCommander));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::GainTradeGood), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1, "1 trade good gained");
    assert!(rig.driver.emitted.iter().any(|e| e.kind == EventKind::SustainDamageUsed), "the use is announced after the offer is answered");
}

#[test]
fn declining_the_commander_gains_nothing_and_a_locked_or_absent_commander_asks_nothing() {
    covers!("rules::combat::sustain_commander::SustainCommander");
    let mut rig = farran(Some(LeaderStatus::Unlocked));
    rig.run();
    rig.pick(0);
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0);
    for status in [Some(LeaderStatus::Locked), Some(LeaderStatus::Exhausted), Some(LeaderStatus::Purged), None] {
        let mut rig = farran(status);
        rig.run();
        rig.pick(0);
        assert!(rig.is_done(), "{status:?}: only an unlocked commander is asked");
        assert!(!pays_on_sustain(&rig.state, Seat(0)));
    }
    assert!(pays_on_sustain(&farran(Some(LeaderStatus::Unlocked)).state, Seat(0)));
    assert!(!pays_on_sustain(&farran(Some(LeaderStatus::Unlocked)).state, Seat(1)), "the offer is for the holder only");
}

#[test]
fn the_commander_window_alone_refuses_every_pick_that_is_not_its_own() {
    covers!("rules::combat::sustain_commander::SustainCommander");
    let (mut state, kernel) = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec]).map_from(&[sys(), dest()]).build();
    state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::RearAdmiralFarran, LeaderStatus::Unlocked);
    let mut rig = Rig::from_parts(state, kernel, SustainCommander::new(Seat(0), Code(3)));
    rig.run();
    rig.expect_illegal(Pick::Opt(c(COpt::Stay)));
    rig.expect_illegal(Pick::Number(1));
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1);
}

// ---- BeginRound ------------------------------------------------------------------------------

fn begin(round: u32, announce: bool, listens: bool) -> BeginFacts {
    BeginFacts { attacker: Seat(0), defender: Seat(1), system: sys(), round, phase: Code(4), announce, round_seq_listens: listens, munitions_start: [false, false] }
}

fn fleets() -> Scenario {
    arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 1, 2)
}

#[test]
fn the_first_round_announces_the_battle_and_the_round_and_records_the_starting_fleets() {
    covers!("rules::combat::begin_round::BeginRound");
    let mut rig = Rig::new(fleets(), BeginRound::new(begin(1, true, false)));
    rig.run();
    assert!(rig.is_done(), "the opening never asks");
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::SpaceCombatStarted, EventKind::CombatRoundStarted]);
    assert_eq!(rig.driver.emitted[0].payload.seat(Key::Player), Some(Seat(0)), "the attacker starts the battle");
    assert_eq!(rig.driver.emitted[1].payload.int(Key::Round), Some(1));
    assert_eq!(rig.driver.emitted[1].payload.seat(Key::Defender), Some(Seat(1)));
    assert_eq!(rig.driver.emitted[1].payload.int(Key::RoundSeq), None, "no round agent listens");
    let p = &rig.state.transient().combat.presentation;
    assert_eq!((p.phase, p.round), (Code(4), 1));
    let kinds: Vec<Unit> = p.round_start.iter().map(|x| x.kind()).collect();
    assert_eq!(kinds, vec![Unit::CruiserI, Unit::DestroyerI, Unit::DestroyerI], "attacker's ships, then the defender's");
    assert_eq!(p.barrage_start, p.round_start, "round 1 keeps the starting fleets for the barrage record");
    assert_eq!(rig.state.meta().combat_round_seq, 1);
}

#[test]
fn a_later_round_announces_only_the_round_and_clears_the_hit_records() {
    covers!("rules::combat::begin_round::BeginRound");
    let (mut state, kernel) = fleets().build();
    state.transient_mut().combat.combat_round_hits.insert(Seat(1), 2);
    state.transient_mut().combat.presentation.edit().remaining_hits.insert(Seat(0), 1);
    state.transient_mut().combat.presentation.edit().barrage_start = vec![Piece::new(Unit::FighterI, Seat(0))];
    let mut rig = Rig::from_parts(state, kernel, BeginRound::new(begin(2, true, true)));
    rig.run();
    let kinds: Vec<EventKind> = rig.driver.emitted.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EventKind::CombatRoundStarted], "SPACE_COMBAT_STARTED is for round 1 only");
    assert_eq!(rig.driver.emitted[0].payload.int(Key::RoundSeq), Some(1), "a listening round agent gets the running round number");
    let combat = &rig.state.transient().combat;
    assert!(combat.combat_round_hits.is_empty() && combat.presentation.remaining_hits.is_empty());
    assert_eq!(combat.presentation.round, 2);
    assert_eq!(combat.presentation.barrage_start.len(), 1, "round 2 does not touch the barrage's starting fleet");
}

#[test]
fn without_the_timing_machinery_the_opening_changes_the_state_but_announces_nothing() {
    covers!("rules::combat::begin_round::BeginRound");
    let mut rig = Rig::new(fleets(), BeginRound::new(begin(1, false, false)));
    rig.run();
    assert!(rig.is_done() && rig.driver.emitted.is_empty());
    assert_eq!(rig.state.transient().combat.presentation.round, 1);
    let mut twice = Rig::from_parts(rig.state.clone(), rig.kernel.clone(), BeginRound::new(begin(2, false, false)));
    twice.run();
    assert_eq!(twice.state.meta().combat_round_seq, 2, "the counter is global to the game and always grows");
}

// ---- Retreat ---------------------------------------------------------------------------------

fn retreat_facts(staging: bool) -> RetreatFacts {
    RetreatFacts { player: Seat(0), system: sys(), destination: dest(), staging, cause: Code(7), pool: Code(8) }
}

fn with_cargo(sc: Scenario, cargo: &[Unit]) -> (CoreState, Kernel) {
    let (mut state, kernel) = sc.build();
    for u in cargo {
        state.board_mut().sys_mut(sys()).units.push(Piece::new(*u, Seat(0)));
    }
    (state, kernel)
}

#[test]
fn the_fleet_moves_with_the_cargo_it_has_room_for_and_the_rest_is_lost() {
    covers!("rules::combat::retreat::Retreat");
    // A dreadnought carries one unit; a cruiser I none; the enemy's ship stays.
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1).unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 1, 1);
    let (state, kernel) = with_cargo(sc, &[Unit::FighterI, Unit::FighterI, Unit::InfantryI]);
    let mut rig = Rig::from_parts(state, kernel, Retreat::new(retreat_facts(false)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.stranded, 2, "capacity 1: two of the three carried units do not fit");
    let there: Vec<Unit> = rig.state.board().sys(dest()).units.iter().map(|p| p.kind()).collect();
    assert_eq!(there, vec![Unit::DreadnoughtI, Unit::CruiserI, Unit::FighterI], "ships first, then the cargo that fits, in board order");
    assert_eq!(units_of(&rig, 0), vec![], "nothing of the player is left behind");
    assert_eq!(units_of(&rig, 1), vec![(Unit::DestroyerI, false)], "the opponent's ship stays");
    assert!(rig.state.board().sys(dest()).cmd.contains(Seat(0)), "a command token goes to the destination");
    assert!(rig.driver.emitted.is_empty() && rig.state.transient().combat.pending_destructions.is_empty(), "without staging nothing is announced or pending");
}

#[test]
fn with_staging_the_token_is_staged_and_a_stranded_ship_waits_for_its_announcement() {
    covers!("rules::combat::retreat::Retreat");
    let sc = arena().unit(sys(), None, Unit::DreadnoughtI, 0, 1);
    let (state, kernel) = with_cargo(sc, &[Unit::FighterI, Unit::FighterI, Unit::InfantryI]);
    let mut rig = Rig::from_parts(state, kernel, Retreat::new(retreat_facts(true)));
    rig.run();
    assert_eq!(rig.driver.window.stranded, 2);
    let pending = rig.state.transient().combat.pending_destructions.as_slice();
    assert_eq!(pending.len(), 1, "of the two stranded units only the fighter is a ship; the infantry is lost without a destruction announcement: {pending:?}");
    assert!(pending.iter().all(|d| d.owner == Seat(0) && d.system == sys() && d.reason == Code(7) && d.during_space_combat));
    assert_eq!(pending[0].unit, Unit::FighterI);
}

#[test]
fn a_fleet_without_cargo_just_moves_and_places_its_token() {
    covers!("rules::combat::retreat::Retreat");
    let sc = arena().unit(sys(), None, Unit::CruiserI, 0, 2);
    let mut rig = Rig::new(sc, Retreat::new(retreat_facts(false)));
    rig.run();
    assert_eq!(rig.driver.window.stranded, 0);
    assert_eq!(rig.state.board().sys(dest()).units.len(), 2);
    assert_eq!(rig.state.board().sys(sys()).units.len(), 0);
    assert!(rig.state.board().sys(dest()).cmd.contains(Seat(0)));
    ti4_engine_core_tests::assert_snapshot("combat_small/retreat_plain", &rig.snapshot());
}

#[test]
fn the_retreat_window_never_asks() {
    covers!("rules::combat::retreat::Retreat");
    let mut rig = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1), Retreat::new(retreat_facts(false)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.ask_of(&rig.state, &rig.kernel), None);
}

#[test]
fn munitions_reserves_is_asked_at_the_opening_and_paid_below_rules_version_eight() {
    covers!("rules::combat::begin_round::BeginRound");
    use ti4_engine_core::choice::generic::Content;
    use ti4_engine_core::choice::Origin;
    let sc = || fleets().goods(0, 3).goods(1, 3);
    let facts = |start: [bool; 2]| BeginFacts { munitions_start: start, ..begin(1, true, false) };
    // The holder with the goods is asked; a payment spends 2 trade goods and marks the round for the roll step.
    let mut rig = Rig::new(sc(), BeginRound::new(facts([true, false])));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.asked().ask, Ask::Combat(CAsk::MunitionsStart { have: 3 }));
    assert_eq!(rig.asked().ctx.subtype, Subtype::Combat(CSub::MunitionsRoll));
    assert_eq!(rig.asked().ctx.origin, Origin::FactionAbility(Content::Ability(ti4_engine_core::generated::Ability::MunitionsReserves)));
    assert_eq!(rig.opts(), vec![c(COpt::MunitionsStart), Opt::Decline]);
    rig.pick(0);
    assert!(rig.is_done());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.econ.trade_goods, 1, "2 of the 3 trade goods paid");
    assert!(p.combat_timers.munitions_round.is(1), "for this round only: the round sequence of the opening");
    // Declining pays nothing and marks nothing.
    let mut declined = Rig::new(sc(), BeginRound::new(facts([true, false])));
    declined.run();
    declined.decline();
    assert!(declined.is_done());
    let p = declined.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.trade_goods, p.combat_timers.munitions_round.get()), (3, None));
    // Exactly the 2 trade goods pay.
    let (exact, kernel) = sc().goods(0, 2).build();
    let mut rig = Rig::from_parts(exact, kernel, BeginRound::new(facts([true, false])));
    rig.run();
    assert!(!rig.is_done(), "two trade goods are enough");
    // Not asked: a side that does not hold the ability, or holds fewer than 2 trade goods.
    let mut none = Rig::new(sc(), BeginRound::new(facts([false, false])));
    none.run();
    assert!(none.is_done());
    let (mut poor, kernel) = sc().build();
    poor.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 1;
    let mut rig = Rig::from_parts(poor, kernel, BeginRound::new(facts([true, false])));
    rig.run();
    assert!(rig.is_done(), "one trade good cannot pay");
    // Both sides: the attacker first, then the defender.
    let mut both = Rig::new(sc(), BeginRound::new(facts([true, true])));
    both.run();
    assert_eq!(both.asked().actor, Seat(0));
    both.pick(0);
    assert_eq!(both.asked().actor, Seat(1));
}
