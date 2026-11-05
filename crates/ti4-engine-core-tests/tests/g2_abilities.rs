//! Lane G2: the leader, unit and technology abilities of the Winnu, Muaat, Ghosts, Empyrean, Cabal and Sol that were loud gaps, as rules of the core table
//! (`ti4_engine_core_factions::silent::{winnu, muaat, ghost, empyrean, cabal, sol}`). The lockstep against the string is in `ti4-engine` (`silent_live_tests`,
//! `combat_live`); these tests pin the rules, including the declines and the conditions the lockstep does not reach.

use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt};
use ti4_engine_core::generated::{Leader, Planet, System, Technology, Unit};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::query::board;
use ti4_engine_core::state::{EventKind, Key, LeaderStatus, List, Payload, Val};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;
use ti4_engine_core_tests::windows::EmitEvent;

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::silent::register_before_movement(&mut t);
    ti4_engine_core_factions::silent::register_before_muaat(&mut t);
    ti4_engine_core_factions::silent::register_abilities(&mut t);
    // The note of the activation is an ability of the activation window (`activation::register` registers it in the string's module order).
    ti4_engine_core_factions::silent::winnu::register_activation(&mut t);
    t
}

fn rig(scenario: Scenario, edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
    let (mut state, kernel) = scenario.build();
    edit(&mut state);
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(table());
    r.run();
    r
}

fn set_leader(s: &mut CoreState, seat: u8, leader: Leader, status: LeaderStatus) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().leaders.status.insert(leader, status);
}

fn status_of(r: &Rig<EmitEvent>, seat: u8, leader: Leader) -> Option<LeaderStatus> {
    r.state.players().by_seat(Seat(seat)).unwrap().leaders.status.get(&leader).copied()
}

fn seat_event(kind: EventKind, seat: u8) -> ti4_engine_core::state::EventRec {
    fx::event(kind, Payload::new().with(Key::Player, Val::Seat(Seat(seat))))
}

fn mark_of(r: &Rig<EmitEvent>, key: MarkKey) -> Option<MarkVal> {
    r.state.marks().tail.get(&key).cloned()
}

fn offer_first(r: &mut Rig<EmitEvent>) {
    let at = r.opts().iter().position(|o| *o != Opt::Decline).expect("an ability is offered");
    r.pick(at);
}

const WINNU: [Faction; 2] = [Faction::TheWinnu, Faction::TheEmiratesOfHacan];

// -- the Winnu: Reclamation and its marks -----------------------------------------------------------------------------------------

fn gained(seat: u8, system: System, planet: Planet) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::PlanetControlGained, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)))
}

fn tactical_mark(s: &mut CoreState, seat: u8) {
    s.marks_mut().tail.edit().insert(MarkKey::PrivateSeat(Seat(seat), Word::WinnuTactical), MarkVal::Atom(Atom::Num(1)));
}

fn activated_by(seat: u8) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::SystemActivated, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(System::MecatolRex18)))
}

/// The string registers the note that a tactical action began as an ability of the Winnu (`ability:*:reclamation_tactical:SYSTEM_ACTIVATED:after`), not as a
/// step of the activation: it answers the Winnu's own activation only, and it is a window ability like any other (a mixed window offers it and a pass skips it).
#[test]
fn the_winnu_note_that_a_tactical_action_began_as_an_ability_of_their_own_activation() {
    covers!("silent::winnu::RECLAMATION_TACTICAL");
    let key = MarkKey::PrivateSeat(Seat(0), Word::WinnuTactical);
    let r = rig(Scenario::new(&WINNU), |_| {}, activated_by(0));
    assert!(r.is_done(), "alone in its window the note is not asked: {}", r.transcript());
    assert_eq!(mark_of(&r, key), Some(MarkVal::Atom(Atom::Num(1))));
    let other = rig(Scenario::new(&WINNU), |_| {}, activated_by(1));
    assert_eq!(mark_of(&other, key), None, "another seat's activation is no tactical action of the Winnu");
    // With another ability of the Winnu in the window the note is one of the offers, and declining skips it.
    use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
    use ti4_engine_core::kernel::ability::{no_effect, Slot, Spec};
    let mut t = table();
    t.add(Spec::new(Slot(60_000), AbilityKind::Other, EventKind::SystemActivated, Relation::After, no_effect).optional());
    let (state, kernel) = Scenario::new(&WINNU).build();
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(activated_by(0))).with_abilities(t);
    r.run();
    while !r.is_done() && r.asked().actor != Seat(0) {
        r.decline();
    }
    assert_eq!(r.opts().len(), 3, "the stand-in, the note and decline: {}", r.transcript());
    r.decline();
    while !r.is_done() {
        r.decline();
    }
    assert_eq!(mark_of(&r, key), None, "a pass over the window skips the note, as in the string");
}

#[test]
fn the_winnu_note_the_planets_they_gain_in_their_own_tactical_action_only() {
    covers!("silent::winnu::RECLAMATION_MARK");
    let key = MarkKey::PrivateSeat(Seat(0), Word::WinnuGained);
    let entry = |system: System, planet: Planet| MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet))]);
    let r = rig(Scenario::new(&WINNU), |s| tactical_mark(s, 0), gained(0, System::MecatolRex18, Planet::MecatolRex));
    assert_eq!(mark_of(&r, key), Some(MarkVal::List(vec![entry(System::MecatolRex18, Planet::MecatolRex)])));
    // A second gain is appended in order.
    let (mut s, kernel) = Scenario::new(&WINNU).build();
    tactical_mark(&mut s, 0);
    s.marks_mut().tail.edit().insert(key, MarkVal::List(vec![entry(System::MecatolRex18, Planet::MecatolRex)]));
    let mut again = Rig::from_parts(s, kernel, EmitEvent::new(gained(0, System::Wellon, Planet::Wellon))).with_abilities(table());
    again.run();
    assert_eq!(mark_of(&again, key), Some(MarkVal::List(vec![entry(System::MecatolRex18, Planet::MecatolRex), entry(System::Wellon, Planet::Wellon)])));
    // No tactical mark, another seat's gain, another seat's turn: nothing is noted.
    assert_eq!(mark_of(&rig(Scenario::new(&WINNU), |_| {}, gained(0, System::MecatolRex18, Planet::MecatolRex)), key), None, "no tactical action began");
    assert_eq!(mark_of(&rig(Scenario::new(&WINNU), |s| tactical_mark(s, 0), gained(1, System::MecatolRex18, Planet::MecatolRex)), key), None, "another seat's gain");
    assert_eq!(mark_of(&rig(Scenario::new(&WINNU).active(Some(1)), |s| tactical_mark(s, 0), gained(0, System::MecatolRex18, Planet::MecatolRex)), key), None, "not the Winnu's turn");
    let hacan = rig(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheWinnu]), |s| tactical_mark(s, 0), gained(0, System::MecatolRex18, Planet::MecatolRex));
    assert_eq!(mark_of(&hacan, key), None, "only the Winnu keep the note");
}

#[test]
fn the_notes_of_the_winnu_go_when_a_turn_begins() {
    covers!("silent::winnu::RECLAMATION_CLEAR");
    let edit = |s: &mut CoreState| {
        tactical_mark(s, 0);
        s.marks_mut().tail.edit().insert(MarkKey::PrivateSeat(Seat(0), Word::WinnuGained), MarkVal::List(Vec::new()));
    };
    let r = rig(Scenario::new(&WINNU), edit, seat_event(EventKind::TurnBegan, 1));
    assert_eq!(mark_of(&r, MarkKey::PrivateSeat(Seat(0), Word::WinnuGained)), None);
    assert_eq!(mark_of(&r, MarkKey::PrivateSeat(Seat(0), Word::WinnuTactical)), None);
    // Either note alone keeps the ability eligible; with neither it changes nothing.
    let only_tactical = rig(Scenario::new(&WINNU), |s| tactical_mark(s, 0), seat_event(EventKind::TurnBegan, 0));
    assert_eq!(mark_of(&only_tactical, MarkKey::PrivateSeat(Seat(0), Word::WinnuTactical)), None);
    let neither = rig(Scenario::new(&WINNU), |_| {}, seat_event(EventKind::TurnBegan, 0));
    assert!(neither.is_done());
}

fn took_mecatol(s: &mut CoreState) {
    s.marks_mut().tail.edit().insert(
        MarkKey::PrivateSeat(Seat(0), Word::WinnuGained),
        MarkVal::List(vec![MarkVal::Rec(vec![MarkVal::Atom(Atom::System(System::MecatolRex18)), MarkVal::Atom(Atom::Planet(Planet::MecatolRex))])]),
    );
}

fn structures(r: &Rig<EmitEvent>, planet: Planet, base: ti4_engine_core::generated::UnitBaseType) -> usize {
    board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(0) && p.kind().data().base_type == base).count()
}

#[test]
fn reclamation_places_a_pds_and_a_space_dock_on_mecatol_rex_or_nothing() {
    covers!("silent::winnu::RECLAMATION");
    use ti4_engine_core::generated::UnitBaseType::{Pds, Spacedock};
    let scenario = || Scenario::new(&WINNU).control(Planet::MecatolRex, 0);
    let mut r = rig(scenario(), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::Window { .. })), "optional: {}", r.transcript());
    offer_first(&mut r);
    assert!(r.is_done());
    assert_eq!((structures(&r, Planet::MecatolRex, Pds), structures(&r, Planet::MecatolRex, Spacedock)), (1, 1));
    // Declined: nothing is placed.
    let mut declined = rig(scenario(), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    declined.decline();
    assert_eq!((structures(&declined, Planet::MecatolRex, Pds), structures(&declined, Planet::MecatolRex, Spacedock)), (0, 0));
    // A space dock already there: both structures or nothing, so nothing is offered.
    let docked = rig(scenario().unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::SpaceDockI, 0, 1), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    assert!(docked.is_done(), "{}", docked.transcript());
    // Two PDS already there: no room for a third.
    let fortified = rig(scenario().unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::PDSI, 0, 2), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    assert!(fortified.is_done(), "{}", fortified.transcript());
    // The planet was lost again, the planet noted is not Mecatol Rex's, another seat's action: nothing is offered.
    assert!(rig(Scenario::new(&WINNU), took_mecatol, seat_event(EventKind::ActionCompleted, 0)).is_done(), "no longer controlled");
    assert!(rig(scenario(), |_| {}, seat_event(EventKind::ActionCompleted, 0)).is_done(), "nothing noted");
    assert!(rig(scenario(), took_mecatol, seat_event(EventKind::ActionCompleted, 1)).is_done(), "another seat's action");
}

#[test]
fn the_reclaimer_places_a_pds_or_a_space_dock_on_the_planet_it_stands_on() {
    covers!("silent::winnu::RECLAIMER");
    use ti4_engine_core::generated::UnitBaseType::{Pds, Spacedock};
    let scenario = || Scenario::new(&WINNU).control(Planet::MecatolRex, 0).unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::Reclaimer, 0, 1).unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::SpaceDockI, 0, 1);
    // The dock is there: only the PDS fits, the question has that one answer.
    let mut r = rig(scenario(), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    offer_first(&mut r);
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Reclaimer))), "{}", r.transcript());
    assert_eq!(r.opts(), vec![Opt::Reaction(ROpt::G(GOpt::Reclaimer { pds: true, system: System::MecatolRex18, planet: Planet::MecatolRex }))]);
    r.pick(0);
    assert_eq!((structures(&r, Planet::MecatolRex, Pds), structures(&r, Planet::MecatolRex, Spacedock)), (1, 1));
    // No mech: no offer.
    let bare = rig(Scenario::new(&WINNU).control(Planet::MecatolRex, 0), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    assert!(bare.is_done() || !matches!(bare.asked().ask, Ask::Reaction(RAsk::G(GAsk::Reclaimer))), "{}", bare.transcript());
    // Not controlled any more: no offer.
    let lost = rig(Scenario::new(&WINNU).unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::Reclaimer, 0, 1), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    assert!(lost.is_done(), "{}", lost.transcript());
}

// -- the Winnu: Berekar Berekon and Hegemonic Trade Policy ------------------------------------------------------------------------

fn produced_by(seat: u8) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::ProductionUsed, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(System::MecatolRex18)))
}

#[test]
fn berekar_berekon_discounts_another_seats_production_by_two() {
    covers!("silent::winnu::AGENT");
    let mut r = rig(Scenario::new(&WINNU), |s| set_leader(s, 0, Leader::BerekarBerekon, LeaderStatus::Readied), produced_by(1));
    offer_first(&mut r);
    assert!(r.is_done());
    assert_eq!(status_of(&r, 0, Leader::BerekarBerekon), Some(LeaderStatus::Exhausted));
    assert_eq!(r.state.transient().production.discount_remaining, 2);
    // Declined, exhausted, or the Winnu's own use (the dialog of the use offers it there): nothing.
    let mut declined = rig(Scenario::new(&WINNU), |s| set_leader(s, 0, Leader::BerekarBerekon, LeaderStatus::Readied), produced_by(1));
    declined.decline();
    assert_eq!((status_of(&declined, 0, Leader::BerekarBerekon), declined.state.transient().production.discount_remaining), (Some(LeaderStatus::Readied), 0));
    assert!(rig(Scenario::new(&WINNU), |s| set_leader(s, 0, Leader::BerekarBerekon, LeaderStatus::Exhausted), produced_by(1)).is_done());
    assert!(rig(Scenario::new(&WINNU), |s| set_leader(s, 0, Leader::BerekarBerekon, LeaderStatus::Readied), produced_by(0)).is_done(), "the owner's own use");
}

#[test]
fn ssruu_copies_berekar_berekon_for_any_use_of_production() {
    covers!("silent::winnu::AGENT_COPY");
    let seats = [Faction::TheYssarilTribes, Faction::TheWinnu, Faction::TheEmiratesOfHacan];
    let edit = |s: &mut CoreState| {
        set_leader(s, 0, Leader::Ssruu, LeaderStatus::Readied);
        set_leader(s, 1, Leader::BerekarBerekon, LeaderStatus::Exhausted);
    };
    for user in [0u8, 2] {
        let mut r = rig(Scenario::new(&seats), edit, produced_by(user));
        let at = r.opts().iter().position(|o| *o != Opt::Decline).expect("the copy is offered");
        r.pick(at);
        assert_eq!(status_of(&r, 0, Leader::Ssruu), Some(LeaderStatus::Exhausted), "use by seat {user}");
        assert_eq!(r.state.transient().production.discount_remaining, 2);
        assert_eq!(status_of(&r, 1, Leader::BerekarBerekon), Some(LeaderStatus::Exhausted), "the lent agent stays as it was");
    }
    // Ssruu exhausted: not offered.
    assert!(rig(Scenario::new(&seats), |s| { edit(s); set_leader(s, 0, Leader::Ssruu, LeaderStatus::Exhausted); }, produced_by(2)).is_done());
}

#[test]
fn hegemonic_trade_policy_swaps_the_values_of_one_planet_for_the_use() {
    covers!("silent::winnu::HTP");
    // A planet whose resources and influence differ.
    let planet = ti4_engine_core_tests::scenario::planets_with_influence(&[0, 1, 2, 3]).into_iter().find(|p| p.data().resources != p.data().influence).expect("a lopsided planet");
    let scenario = || Scenario::new(&WINNU).tech(0, Technology::HegemonicTradePolicy).control(planet, 0);
    let mut r = rig(scenario(), |_| {}, produced_by(0));
    offer_first(&mut r);
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::TradePolicy))), "{}", r.transcript());
    assert_eq!(r.opts().len(), 1);
    r.pick(0);
    assert!(r.state.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::HegemonicTradePolicy));
    assert_eq!(r.state.transient().production.value_swapped_planet, Some(planet));
    assert_eq!(mark_of(&r, MarkKey::ProductionValueSwapSeq()), Some(MarkVal::Atom(Atom::Num(r.state.meta().production_seq))));
    // Exhausted card, another seat's use, no planet: nothing.
    assert!(rig(scenario(), |s| { s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(Technology::HegemonicTradePolicy); }, produced_by(0)).is_done());
    assert!(rig(scenario(), |_| {}, produced_by(1)).is_done());
    assert!(rig(Scenario::new(&WINNU).tech(0, Technology::HegemonicTradePolicy), |_| {}, produced_by(0)).is_done());
    // Declined: nothing changes.
    let mut declined = rig(scenario(), |_| {}, produced_by(0));
    declined.decline();
    assert_eq!(declined.state.transient().production.value_swapped_planet, None);
}

// -- Magmus ----------------------------------------------------------------------------------------------------------------------

#[test]
fn magmus_gains_a_trade_good_after_a_strategy_token_is_spent_and_is_unlocked_by_a_war_sun() {
    covers!("silent::muaat::MAGMUS");
    covers!("silent::muaat::UNLOCK");
    let seats = [Faction::TheEmbersOfMuaat, Faction::TheEmiratesOfHacan];
    let spent = |seat: u8| fx::event(EventKind::StrategyTokenSpent, Payload::new().with(Key::Player, Val::Seat(Seat(seat))));
    let goods = |r: &Rig<EmitEvent>, seat: u8| r.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods;
    let mut r = rig(Scenario::new(&seats), |s| set_leader(s, 0, Leader::Magmus, LeaderStatus::Unlocked), spent(0));
    offer_first(&mut r);
    assert_eq!(goods(&r, 0), 1);
    let mut declined = rig(Scenario::new(&seats), |s| set_leader(s, 0, Leader::Magmus, LeaderStatus::Unlocked), spent(0));
    declined.decline();
    assert_eq!(goods(&declined, 0), 0);
    assert!(rig(Scenario::new(&seats), |s| set_leader(s, 0, Leader::Magmus, LeaderStatus::Unlocked), spent(1)).is_done(), "another seat's token");
    assert!(rig(Scenario::new(&seats), |s| set_leader(s, 0, Leader::Magmus, LeaderStatus::Locked), spent(0)).is_done(), "a locked commander");
    // The unlock: a use of PRODUCTION that made a war sun, by the Muaat.
    let units_key = Key::Other(ti4_engine_core::state::Code(0));
    let produced = |unit: Unit, seat: u8| {
        let mut p = Payload::new().with(Key::Player, Val::Seat(Seat(seat)));
        p.set_list(units_key, List::Produced([(unit, ti4_engine_core::choice::production::Spot::Space)].into_iter().collect()));
        fx::event(EventKind::UnitsProduced, p)
    };
    let locked = |s: &mut CoreState| set_leader(s, 0, Leader::Magmus, LeaderStatus::Locked);
    let sun = rig(Scenario::new(&seats), locked, produced(Unit::PrototypeWarSunI, 0));
    assert_eq!(status_of(&sun, 0, Leader::Magmus), Some(LeaderStatus::Unlocked));
    assert_eq!(status_of(&rig(Scenario::new(&seats), locked, produced(Unit::CruiserI, 0)), 0, Leader::Magmus), Some(LeaderStatus::Locked), "a cruiser is no war sun");
    assert_eq!(status_of(&rig(Scenario::new(&seats), locked, produced(Unit::PrototypeWarSunI, 1)), 0, Leader::Magmus), Some(LeaderStatus::Locked), "another seat's war sun");
}

// -- Sai Seravus -----------------------------------------------------------------------------------------------------------------

fn ghost_scenario(system: System) -> Scenario {
    Scenario::new(&[Faction::TheGhostsOfCreuss, Faction::TheEmiratesOfHacan]).map_from(&[system]).active_system(system)
}

fn ghost_key(s: &CoreState) -> MarkKey {
    MarkKey::PrivateGhostCommander(Seat(0), ti4_engine_core::keys::DecimalTextOrd(s.meta().activation_seq))
}

fn arrived(seat: u8, system: System, unit: Unit, hops: i64) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::ShipMoved, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(system)).with(Key::Unit, Val::Unit(unit)).with(Key::Wormholes, Val::Int(hops)))
}

#[test]
fn sai_seravus_counts_the_capacity_ships_that_arrived_through_a_wormhole() {
    covers!("silent::ghost::MOVED");
    let system = plain_systems()[0];
    let unlocked = |s: &mut CoreState| set_leader(s, 0, Leader::SaiSeravus, LeaderStatus::Unlocked);
    let count = |r: &Rig<EmitEvent>| r.state.marks().tail.get(&ghost_key(&r.state)).cloned();
    let r = rig(ghost_scenario(system), unlocked, arrived(0, system, Unit::CarrierI, 1));
    assert_eq!(count(&r), Some(MarkVal::Atom(Atom::Num(1))));
    // Another arrival adds to the count.
    let (mut s, kernel) = ghost_scenario(system).build();
    unlocked(&mut s);
    let key = ghost_key(&s);
    s.marks_mut().tail.edit().insert(key, MarkVal::Atom(Atom::Num(2)));
    let mut again = Rig::from_parts(s, kernel, EmitEvent::new(arrived(0, system, Unit::CarrierI, 2))).with_abilities(table());
    again.run();
    assert_eq!(count(&again), Some(MarkVal::Atom(Atom::Num(3))));
    // No wormhole, a ship without capacity, another seat's ship, another system, a locked commander: nothing is counted.
    assert_eq!(count(&rig(ghost_scenario(system), unlocked, arrived(0, system, Unit::CarrierI, 0))), None, "no wormhole");
    assert_eq!(count(&rig(ghost_scenario(system), unlocked, arrived(0, system, Unit::DestroyerI, 1))), None, "a destroyer has no capacity");
    assert_eq!(count(&rig(ghost_scenario(system), unlocked, arrived(1, system, Unit::CarrierI, 1))), None, "another seat's ship");
    assert_eq!(count(&rig(ghost_scenario(system), unlocked, arrived(0, plain_systems()[1], Unit::CarrierI, 1))), None, "not the active system");
    assert_eq!(count(&rig(ghost_scenario(system), |_| {}, arrived(0, system, Unit::CarrierI, 1))), None, "a locked commander");
}

fn finished(seat: u8, system: System) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::MovementFinished, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(system)).with(Key::ShipsMoved, Val::Int(1)))
}

fn counted(n: u32) -> impl Fn(&mut CoreState) {
    move |s: &mut CoreState| {
        set_leader(s, 0, Leader::SaiSeravus, LeaderStatus::Unlocked);
        let key = ghost_key(s);
        s.marks_mut().tail.edit().insert(key, MarkVal::Atom(Atom::Num(n)));
    }
}

fn fighters(r: &Rig<EmitEvent>, system: System) -> usize {
    board::space_units(&r.state, system).iter().filter(|p| p.owner() == Seat(0) && p.kind().data().base_type == ti4_engine_core::generated::UnitBaseType::Fighter).count()
}

#[test]
fn sai_seravus_places_fighters_up_to_the_count_the_free_capacity_and_the_box() {
    covers!("silent::ghost::FINISHED");
    let system = plain_systems()[0];
    let scenario = || ghost_scenario(system).unit(system, None, Unit::CarrierI, 0, 1);
    let fighter_options = |r: &Rig<EmitEvent>| r.opts().iter().filter_map(|o| match o { Opt::Reaction(ROpt::G(GOpt::Fighters { count, .. })) => Some(*count), _ => None }).collect::<Vec<_>>();
    // Three ships came through, the carrier takes four: 1, 2 or 3.
    let mut r = rig(scenario(), counted(3), finished(0, system));
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::CommanderFighters))), "{}", r.transcript());
    assert_eq!(fighter_options(&r), vec![1, 2, 3]);
    assert_eq!(r.opts().last(), Some(&Opt::Decline));
    r.pick(1);
    assert!(r.is_done());
    assert_eq!(fighters(&r, system), 2);
    assert_eq!(r.state.marks().tail.get(&ghost_key(&r.state)), None, "the count is spent");
    // Six came through, only four fit.
    let capped = rig(scenario(), counted(6), finished(0, system));
    assert_eq!(fighter_options(&capped), vec![1, 2, 3, 4]);
    // Declined: no fighters, the count is spent all the same.
    let mut declined = rig(scenario(), counted(3), finished(0, system));
    declined.decline();
    assert_eq!(fighters(&declined, system), 0);
    assert_eq!(declined.state.marks().tail.get(&ghost_key(&declined.state)), None);
    // No unused capacity: nothing is asked and the count goes.
    let full = rig(ghost_scenario(system).unit(system, None, Unit::DestroyerI, 0, 1), counted(2), finished(0, system));
    assert!(full.is_done(), "{}", full.transcript());
    assert_eq!(full.state.marks().tail.get(&ghost_key(&full.state)), None);
    // Another seat's movement, or no count: the ability is not eligible.
    assert!(rig(scenario(), counted(3), finished(1, system)).is_done());
    assert!(rig(scenario(), |s| set_leader(s, 0, Leader::SaiSeravus, LeaderStatus::Unlocked), finished(0, system)).is_done());
}

// -- the Empyrean ----------------------------------------------------------------------------------------------------------------

const EMPYREAN: [Faction; 2] = [Faction::TheEmpyrean, Faction::TheEmiratesOfHacan];

fn planetless() -> System {
    System::ALL.iter().copied().find(|s| s.data().planets.is_empty() && !s.data().hyperlane && s.data().source == ti4_engine_core::generated::Source::Pok).expect("a planetless system")
}

fn moved_in(seat: u8, system: System, ships: i64) -> ti4_engine_core::state::EventRec {
    fx::event(EventKind::MovementFinished, Payload::new().with(Key::Player, Val::Seat(Seat(seat))).with(Key::System, Val::System(system)).with(Key::ShipsMoved, Val::Int(ships)))
}

#[test]
fn acamar_gives_the_mover_a_command_token_after_a_move_into_a_system_without_planets() {
    covers!("silent::empyrean::ACAMAR");
    let tokens = |r: &Rig<EmitEvent>| {
        let e = &r.state.players().by_seat(Seat(1)).unwrap().econ;
        (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
    };
    let ready = |s: &mut CoreState| set_leader(s, 0, Leader::Acamar, LeaderStatus::Readied);
    let before = tokens(&rig(Scenario::new(&EMPYREAN), |_| {}, moved_in(1, planetless(), 1)));
    let mut r = rig(Scenario::new(&EMPYREAN), ready, moved_in(1, planetless(), 1));
    offer_first(&mut r);
    // The mover, not the Empyrean, picks the pool.
    assert_eq!(r.asked().actor, Seat(1), "{}", r.transcript());
    r.pick(1);
    assert!(r.is_done());
    assert_eq!(tokens(&r), (before.0, before.1 + 1, before.2));
    assert_eq!(status_of(&r, 0, Leader::Acamar), Some(LeaderStatus::Exhausted));
    // Declined, a system with planets, no ship moved, a spent agent: nothing.
    let mut declined = rig(Scenario::new(&EMPYREAN), ready, moved_in(1, planetless(), 1));
    declined.decline();
    assert_eq!(status_of(&declined, 0, Leader::Acamar), Some(LeaderStatus::Readied));
    assert!(rig(Scenario::new(&EMPYREAN), ready, moved_in(1, plain_systems()[0], 1)).is_done(), "planets");
    assert!(rig(Scenario::new(&EMPYREAN), ready, moved_in(1, planetless(), 0)).is_done(), "no ship moved");
    assert!(rig(Scenario::new(&EMPYREAN), |s| set_leader(s, 0, Leader::Acamar, LeaderStatus::Exhausted), moved_in(1, planetless(), 1)).is_done(), "exhausted");
}

#[test]
fn xuange_returns_the_token_after_another_player_moves_ships_in() {
    covers!("silent::empyrean::XUANGE");
    let system = plain_systems()[0];
    let scenario = || Scenario::new(&EMPYREAN).command_token(system, 0);
    let unlocked = |s: &mut CoreState| set_leader(s, 0, Leader::Xuange, LeaderStatus::Unlocked);
    let arrival = |seat: u8, s: System| arrived(seat, s, Unit::CarrierI, 0);
    let mut r = rig(scenario(), unlocked, arrival(1, system));
    offer_first(&mut r);
    assert!(!board::has_token(&r.state, system, Seat(0)));
    let mut declined = rig(scenario(), unlocked, arrival(1, system));
    declined.decline();
    assert!(board::has_token(&declined.state, system, Seat(0)));
    assert!(rig(scenario(), unlocked, arrival(0, system)).is_done(), "the holder's own move");
    assert!(rig(Scenario::new(&EMPYREAN), unlocked, arrival(1, system)).is_done(), "no token there");
    assert!(rig(scenario(), |_| {}, arrival(1, system)).is_done(), "a locked commander");
}

// -- the Cabal and Sol -----------------------------------------------------------------------------------------------------------

#[test]
fn the_stillness_of_stars_converts_the_commodities_and_captures_a_unit() {
    covers!("silent::cabal::STILLNESS");
    let seats = [Faction::TheVuilraithCabal, Faction::TheEmiratesOfHacan];
    let replenished = |value: i64| fx::event(EventKind::CommoditiesReplenished, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Commodities, Val::Int(value)));
    let ready = |s: &mut CoreState| set_leader(s, 0, Leader::TheStillnessOfStars, LeaderStatus::Readied);
    let scenario = || Scenario::new(&seats).commodities(1, 2);
    let mut r = rig(scenario(), ready, replenished(2));
    offer_first(&mut r);
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Stillness))), "{}", r.transcript());
    let cruiser = r.opts().iter().position(|o| matches!(o, Opt::Reaction(ROpt::G(GOpt::Capture { unit: Unit::CruiserI, victim: Seat(1) })))).expect("a cruiser costs 2");
    let carrier = r.opts().iter().any(|o| matches!(o, Opt::Reaction(ROpt::G(GOpt::Capture { unit: Unit::CarrierI, .. }))));
    assert!(!carrier, "a carrier costs 3");
    r.pick(cruiser);
    assert!(r.is_done());
    let econ = &r.state.players().by_seat(Seat(1)).unwrap().econ;
    assert_eq!((econ.commodities, econ.trade_goods), (0, 2), "the commodities became trade goods");
    assert_eq!(r.state.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice(), &[(Seat(1), Unit::CruiserI)]);
    assert_eq!(status_of(&r, 0, Leader::TheStillnessOfStars), Some(LeaderStatus::Exhausted));
    // A commodity value of 0 qualifies no unit: the commodities are converted, nothing is captured.
    let mut none = rig(scenario(), ready, replenished(0));
    offer_first(&mut none);
    assert!(none.is_done(), "{}", none.transcript());
    assert!(none.state.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice().is_empty());
    assert_eq!(none.state.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 2);
    // Declined, the Cabal's own replenishment, an exhausted agent: nothing.
    let mut declined = rig(scenario(), ready, replenished(2));
    declined.decline();
    assert_eq!(declined.state.players().by_seat(Seat(1)).unwrap().econ.commodities, 2);
    assert!(rig(Scenario::new(&seats).commodities(0, 2), ready, fx::event(EventKind::CommoditiesReplenished, Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::Commodities, Val::Int(2)))).is_done());
    assert!(rig(scenario(), |s| set_leader(s, 0, Leader::TheStillnessOfStars, LeaderStatus::Exhausted), replenished(2)).is_done());
}

#[test]
fn claire_gibson_adds_an_infantry_to_a_planet_the_sol_hold_when_a_ground_combat_starts() {
    covers!("silent::sol::CLAIRE_GIBSON");
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let seats = [Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan];
    let combat = || fx::event(EventKind::GroundCombatStarted, Payload::new().with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)));
    let unlocked = |s: &mut CoreState| set_leader(s, 0, Leader::ClaireGibson, LeaderStatus::Unlocked);
    let infantry = |r: &Rig<EmitEvent>| board::planet_units(&r.state, planet).iter().filter(|p| p.owner() == Seat(0) && p.kind().data().base_type == ti4_engine_core::generated::UnitBaseType::Infantry).count();
    let mut r = rig(Scenario::new(&seats).control(planet, 0), unlocked, combat());
    offer_first(&mut r);
    assert_eq!(infantry(&r), 1);
    let mut declined = rig(Scenario::new(&seats).control(planet, 0), unlocked, combat());
    declined.decline();
    assert_eq!(infantry(&declined), 0);
    assert!(rig(Scenario::new(&seats), unlocked, combat()).is_done(), "a planet the Sol do not hold");
    assert!(rig(Scenario::new(&seats).control(planet, 0), |_| {}, combat()).is_done(), "a locked commander");
    // The holder of a planet who is the other seat is not offered the Sol's commander.
    assert!(rig(Scenario::new(&seats).control(planet, 1), unlocked, combat()).is_done());
}

// -- what the first tests did not reach (found by the mutation check) ---------------------------------------------------------

/// A question of these abilities is put as a plain one: not optional, not in a battle, not in an invasion.
fn assert_plain_ctx(r: &Rig<EmitEvent>) {
    let c = &r.asked().ctx;
    assert!(!c.optional && !c.space_battle && c.invasion_seq.is_none() && c.target.is_none(), "{c:?}");
}

#[test]
fn the_questions_of_the_abilities_are_plain() {
    covers!("silent::winnu::RECLAIMER");
    covers!("silent::winnu::HTP");
    covers!("silent::ghost::FINISHED");
    covers!("silent::cabal::STILLNESS");
    let scenario = || Scenario::new(&WINNU).control(Planet::MecatolRex, 0).unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::Reclaimer, 0, 1);
    // Reclamation is offered first, the Reclaimer second.
    let mut r = rig(scenario(), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    r.pick(1);
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Reclaimer))), "{}", r.transcript());
    assert_plain_ctx(&r);
    let system = plain_systems()[0];
    let ghost = rig(ghost_scenario(system).unit(system, None, Unit::CarrierI, 0, 1), counted(2), finished(0, system));
    assert_plain_ctx(&ghost);
    let seats = [Faction::TheVuilraithCabal, Faction::TheEmiratesOfHacan];
    let mut cabal = rig(Scenario::new(&seats).commodities(1, 2), |s| set_leader(s, 0, Leader::TheStillnessOfStars, LeaderStatus::Readied), fx::event(EventKind::CommoditiesReplenished, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Commodities, Val::Int(1))));
    offer_first(&mut cabal);
    assert_plain_ctx(&cabal);
    // Hegemonic Trade Policy.
    let planet = ti4_engine_core_tests::scenario::planets_with_influence(&[0, 1, 2, 3]).into_iter().find(|p| p.data().resources != p.data().influence).expect("a lopsided planet");
    let mut htp = rig(Scenario::new(&WINNU).tech(0, Technology::HegemonicTradePolicy).control(planet, 0), |_| {}, produced_by(0));
    offer_first(&mut htp);
    assert_plain_ctx(&htp);
}

#[test]
fn the_stillness_of_stars_offers_exactly_the_cheap_forms_with_a_model_left() {
    covers!("silent::cabal::STILLNESS");
    let seats = [Faction::TheVuilraithCabal, Faction::TheEmiratesOfHacan];
    let replenished = |value: i64| fx::event(EventKind::CommoditiesReplenished, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Commodities, Val::Int(value)));
    let ready = |s: &mut CoreState| set_leader(s, 0, Leader::TheStillnessOfStars, LeaderStatus::Readied);
    let offered = |r: &Rig<EmitEvent>| r.opts().iter().filter_map(|o| match o { Opt::Reaction(ROpt::G(GOpt::Capture { unit, .. })) => Some(*unit), _ => None }).collect::<Vec<_>>();
    // A commodity value of 1: a destroyer costs 1, a fighter and an infantry half of that (two for the price).
    let mut r = rig(Scenario::new(&seats).commodities(1, 1), ready, replenished(1));
    offer_first(&mut r);
    assert_eq!(offered(&r), vec![Unit::DestroyerI, Unit::FighterI, Unit::InfantryI]);
    // All eight destroyers are on the board: none is left in the box to capture.
    let system = plain_systems()[0];
    let mut gone = rig(Scenario::new(&seats).commodities(1, 1).unit(system, None, Unit::DestroyerI, 1, 8), ready, replenished(1));
    offer_first(&mut gone);
    assert_eq!(offered(&gone), vec![Unit::FighterI, Unit::InfantryI]);
    // One commodity held converts as well (a conversion of nothing would stage nothing; one is something).
    gone.pick(0);
    let econ = &gone.state.players().by_seat(Seat(1)).unwrap().econ;
    assert_eq!((econ.commodities, econ.trade_goods), (0, 1));
}

#[test]
fn a_ship_with_a_capacity_of_one_counts_for_sai_seravus() {
    covers!("silent::ghost::MOVED");
    let system = plain_systems()[0];
    let r = rig(ghost_scenario(system), |s| set_leader(s, 0, Leader::SaiSeravus, LeaderStatus::Unlocked), arrived(0, system, Unit::CruiserII, 1));
    assert_eq!(r.state.marks().tail.get(&ghost_key(&r.state)), Some(&MarkVal::Atom(Atom::Num(1))));
}

#[test]
fn claire_gibson_needs_a_planet_in_the_event_and_a_model_in_the_box() {
    covers!("silent::sol::CLAIRE_GIBSON");
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let seats = [Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan];
    let unlocked = |s: &mut CoreState| set_leader(s, 0, Leader::ClaireGibson, LeaderStatus::Unlocked);
    let combat = || fx::event(EventKind::GroundCombatStarted, Payload::new().with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)));
    // No planet in the event.
    assert!(rig(Scenario::new(&seats).control(planet, 0), unlocked, fx::event(EventKind::GroundCombatStarted, Payload::new().with(Key::System, Val::System(system)))).is_done());
    // Infantry have no plastic limit: any number on the board still leaves one in the box.
    let mut crowded = rig(Scenario::new(&seats).control(planet, 0).unit(system, Some(planet), Unit::InfantryI, 0, 30), unlocked, combat());
    offer_first(&mut crowded);
    assert_eq!(board::planet_units(&crowded.state, planet).len(), 31);
}

#[test]
fn the_notes_of_the_winnu_go_with_either_note_and_reclamation_needs_the_models_in_the_box() {
    covers!("silent::winnu::RECLAMATION_CLEAR");
    covers!("silent::winnu::RECLAMATION");
    // The planets noted alone are cleared too.
    let r = rig(Scenario::new(&WINNU), took_mecatol, seat_event(EventKind::TurnBegan, 0));
    assert_eq!(mark_of(&r, MarkKey::PrivateSeat(Seat(0), Word::WinnuGained)), None);
    // Three space docks stand elsewhere: none is left in the box, so the pair cannot be placed.
    let elsewhere = plain_systems()[0];
    let planet = elsewhere.data().planets[0];
    let r = rig(Scenario::new(&WINNU).control(Planet::MecatolRex, 0).unit(elsewhere, Some(planet), Unit::SpaceDockI, 0, 3), took_mecatol, seat_event(EventKind::ActionCompleted, 0));
    assert!(r.is_done(), "{}", r.transcript());
}

// -- Voidwatch ----------------------------------------------------------------------------------------------------------------

fn with_note_order(mut t: AbilityTable) -> AbilityTable {
    use ti4_engine_core::kernel::ability::{PlayFacts, SharedPlay};
    t.facts.plays = ti4_engine_core::kernel::ability::Plays::of(Some(PlayFacts {
        scope: 0,
        blanked: false,
        action_card: ti4_engine_core::state::Code(0),
        supply_base: ti4_engine_core::state::Code(0),
        note_order: |a, b| a.cmp(&b),
        seats: Vec::new(),
        name_group: Vec::new(),
        has_effect: Vec::new(),
        skimmer: false,
        roster: None,
        rules_version: 35,
        tech_order: |a, b| a.cmp(&b),
        shared: SharedPlay::default(),
    }));
    t
}

fn watch_rig(scenario: Scenario, edit: impl FnOnce(&mut CoreState), event: ti4_engine_core::state::EventRec) -> Rig<EmitEvent> {
    let (mut state, kernel) = scenario.build();
    edit(&mut state);
    let mut r = Rig::from_parts(state, kernel, EmitEvent::new(event)).with_abilities(with_note_order(table()));
    r.run();
    r
}

fn hold(s: &mut CoreState, note: ti4_engine_core::generated::PromissoryNote, of: Faction, by: u8) {
    s.cards_mut().promissory.edit().insert(ti4_engine_core::state::NoteKey { note, faction: of }, Seat(by));
}

#[test]
fn voidwatch_makes_the_mover_give_a_note_from_the_hand() {
    covers!("silent::empyrean::VOIDWATCH");
    use ti4_engine_core::generated::PromissoryNote as N;
    let system = plain_systems()[0];
    let planet = system.data().planets.first().copied();
    let scenario = || {
        let s = Scenario::new(&EMPYREAN).unit(system, None, Unit::CarrierI, 0, 1);
        let _ = planet;
        s
    };
    let setup = |s: &mut CoreState| {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::Voidwatch);
        hold(s, N::Ceasefire, Faction::TheEmpyrean, 1);
        hold(s, N::PoliticalSecret, Faction::TheEmpyrean, 1);
    };
    let mut r = watch_rig(scenario(), setup, moved_in(1, system, 1));
    assert_eq!(r.asked().actor, Seat(1), "the mover chooses\n{}", r.transcript());
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Voidwatch))));
    // The notes in the order of the table, then the mover's own Support for the Throne.
    assert_eq!(
        r.opts(),
        vec![
            Opt::Reaction(ROpt::G(GOpt::Note(ti4_engine_core::state::NoteKey { note: N::Ceasefire, faction: Faction::TheEmpyrean }))),
            Opt::Reaction(ROpt::G(GOpt::Note(ti4_engine_core::state::NoteKey { note: N::PoliticalSecret, faction: Faction::TheEmpyrean }))),
            Opt::Reaction(ROpt::G(GOpt::Support(Faction::TheEmiratesOfHacan))),
        ]
    );
    assert_plain_ctx(&r);
    r.pick(0);
    assert!(r.is_done());
    assert_eq!(r.state.cards().promissory.get(&ti4_engine_core::state::NoteKey { note: N::Ceasefire, faction: Faction::TheEmpyrean }), Some(&Seat(0)), "the Empyrean holds it now");
    // The Support for the Throne goes into the holder's play area and scores.
    let mut support = watch_rig(scenario(), setup, moved_in(1, system, 1));
    support.pick(2);
    assert_eq!(support.state.trade().support_holders.get(&Seat(1)), Some(&Seat(0)));
    // Support already lent out: not offered; a note in a play area: not offered.
    let lent = watch_rig(scenario(), |s| { setup(s); s.trade_mut().support_holders.insert(Seat(1), Seat(0)); }, moved_in(1, system, 1));
    assert_eq!(lent.opts().len(), 2);
    let faceup = watch_rig(scenario(), |s| { setup(s); s.cards_mut().promissory_faceup.edit().insert(ti4_engine_core::state::NoteKey { note: N::Ceasefire, faction: Faction::TheEmpyrean }, ()); }, moved_in(1, system, 1));
    assert_eq!(faceup.opts().len(), 2);
    // Without the count of ships that moved the event is no arrival.
    let uncounted = fx::event(EventKind::MovementFinished, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::System, Val::System(system)));
    assert!(watch_rig(scenario(), setup, uncounted).is_done(), "no count of ships");
    // An Alliance counts only while its owner's commander is unlocked.
    let alliance = ti4_engine_core::state::NoteKey { note: N::Alliance, faction: Faction::TheEmpyrean };
    let hidden = watch_rig(scenario(), |s| { setup(s); hold(s, N::Alliance, Faction::TheEmpyrean, 1); }, moved_in(1, system, 1));
    assert!(!hidden.opts().contains(&Opt::Reaction(ROpt::G(GOpt::Note(alliance)))));
    let shown = watch_rig(scenario(), |s| { setup(s); hold(s, N::Alliance, Faction::TheEmpyrean, 1); set_leader(s, 0, Leader::Xuange, LeaderStatus::Unlocked); }, moved_in(1, system, 1));
    assert!(shown.opts().contains(&Opt::Reaction(ROpt::G(GOpt::Note(alliance)))));
    // No unit of the holder there, no ship moved, the holder's own move, no technology: nothing is asked.
    let other = plain_systems()[1];
    assert!(watch_rig(scenario(), setup, moved_in(1, other, 1)).is_done(), "no unit of the holder");
    assert!(watch_rig(scenario(), setup, moved_in(1, system, 0)).is_done(), "no ship moved");
    assert!(watch_rig(scenario(), setup, moved_in(0, system, 1)).is_done(), "the holder's own move");
    assert!(watch_rig(scenario(), |s| { setup(s); s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.remove(Technology::Voidwatch); }, moved_in(1, system, 1)).is_done(), "no technology");
}

#[test]
fn voidwatch_sees_a_unit_on_a_planet_of_the_system_too() {
    covers!("silent::empyrean::VOIDWATCH");
    use ti4_engine_core::generated::PromissoryNote as N;
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let setup = |s: &mut CoreState| {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::Voidwatch);
        hold(s, N::Ceasefire, Faction::TheEmpyrean, 1);
    };
    let r = watch_rig(Scenario::new(&EMPYREAN).unit(system, Some(planet), Unit::InfantryI, 0, 1), setup, moved_in(1, system, 1));
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::G(GAsk::Voidwatch))), "{}", r.transcript());
}

// -- Garv and Gunn and Dart and Tai: who is offered them ------------------------------------------------------------------------

fn explorable_planet() -> (System, Planet) {
    let scope = ti4_engine_core::tables::Scope(u32::MAX);
    let planet = Planet::ALL.iter().copied().find(|p| p.data().system.is_some() && !ti4_engine_core::query::explore::traits_of(scope, *p).is_empty() && p.data().source == ti4_engine_core::generated::Source::Pok).expect("a planet with a trait");
    (planet.data().system.expect("a system"), planet)
}

fn stocked(s: &mut CoreState) {
    let d = s.decks_mut();
    for deck in 0..d.explore.len() {
        d.explore[deck] = ti4_engine_core::containers::Deck::new(vec![ti4_engine_core::generated::Explore::ALL[deck]]);
        d.explore_present |= 1 << deck;
    }
}

const NAAZ: [Faction; 2] = [Faction::TheNaazRokhaAlliance, Faction::TheEmiratesOfHacan];

#[test]
fn garv_and_gunn_are_offered_for_an_explorable_planet_of_the_player_whose_turn_ends() {
    covers!("silent::naaz::AGENT");
    let (system, planet) = explorable_planet();
    let ready = |s: &mut CoreState| { stocked(s); set_leader(s, 0, Leader::GarvAndGunn, LeaderStatus::Readied); };
    let scenario = || Scenario::new(&NAAZ).map_from(&[system]).control(planet, 1);
    let mut r = rig(scenario(), ready, seat_event(EventKind::TurnPassed, 1));
    assert_eq!(r.asked().actor, Seat(0), "the agent's owner accepts\n{}", r.transcript());
    assert!(matches!(r.asked().ask, Ask::Reaction(RAsk::Window { .. })));
    r.decline();
    assert_eq!(status_of(&r, 0, Leader::GarvAndGunn), Some(LeaderStatus::Readied));
    // The ending player's planet; the agent's owner has none.
    assert!(rig(Scenario::new(&NAAZ).map_from(&[system]).control(planet, 0), ready, seat_event(EventKind::TurnPassed, 1)).is_done(), "no planet of the ending player");
    // The deck of the planet's trait is empty or absent; the agent is exhausted.
    assert!(rig(scenario(), |s| { set_leader(s, 0, Leader::GarvAndGunn, LeaderStatus::Readied); }, seat_event(EventKind::TurnPassed, 1)).is_done(), "no deck");
    assert!(rig(scenario(), |s| { ready(s); for deck in 0..s.decks().explore.len() { s.decks_mut().explore[deck] = ti4_engine_core::containers::Deck::new(Vec::new()); } }, seat_event(EventKind::TurnPassed, 1)).is_done(), "empty decks");
    assert!(rig(scenario(), |s| { ready(s); set_leader(s, 0, Leader::GarvAndGunn, LeaderStatus::Exhausted); }, seat_event(EventKind::TurnPassed, 1)).is_done());
    // Cards in a deck the position does not list as present are no cards.
    assert!(rig(scenario(), |s| { ready(s); s.decks_mut().explore_present = 0; }, seat_event(EventKind::TurnPassed, 1)).is_done(), "decks not present");
}

/// The pairs of Ssruu's copies are registered where the lender holds the agent card AND the borrower holds Ssruu.
#[test]
fn ssruus_copies_are_registered_for_the_seats_that_hold_both_cards_only() {
    covers!("silent::naaz::AGENT_COPY");
    covers!("silent::winnu::AGENT_COPY");
    use ti4_engine_core::kernel::resolver::AbilityDef;
    let seats = [Faction::TheYssarilTribes, Faction::TheNaazRokhaAlliance, Faction::TheWinnu, Faction::TheEmiratesOfHacan];
    let copies_of = |edit: &dyn Fn(&mut CoreState), slot: ti4_engine_core::kernel::ability::Slot| -> Vec<(Seat, Option<Seat>)> {
        let mut s = Scenario::new(&seats).state();
        edit(&mut s);
        let order = [Seat(0), Seat(1), Seat(2), Seat(3)];
        let defs: Vec<AbilityDef> = table().registry(&s, &order, &mut |spec, _, _| ti4_engine_core::choice::reaction::AbilityId { kind: spec.kind, key: ti4_engine_core::state::Code(spec.slot.0) });
        defs.iter().filter(|d| d.slot_bits() == slot.arg()).map(|d| (d.owner, d.pair_source())).collect()
    };
    for (slot, agent) in [(ti4_engine_core_factions::silent::naaz::AGENT_COPY, Leader::GarvAndGunn), (ti4_engine_core_factions::silent::winnu::AGENT_COPY, Leader::BerekarBerekon)] {
        let lender = if agent == Leader::GarvAndGunn { 1 } else { 2 };
        // Ssruu on seat 0 only: one copy.
        let both = |s: &mut CoreState| { set_leader(s, 0, Leader::Ssruu, LeaderStatus::Readied); set_leader(s, lender, agent, LeaderStatus::Readied); };
        assert_eq!(copies_of(&both, slot), vec![(Seat(0), Some(Seat(lender)))], "{agent:?}");
        // The lender holds no agent card: none. The borrower holds no Ssruu: none.
        assert_eq!(copies_of(&|s: &mut CoreState| set_leader(s, 0, Leader::Ssruu, LeaderStatus::Readied), slot), Vec::new(), "{agent:?}: no lender");
        assert_eq!(copies_of(&|s: &mut CoreState| set_leader(s, lender, agent, LeaderStatus::Readied), slot), Vec::new(), "{agent:?}: no borrower");
    }
}

#[test]
fn ssruus_copy_of_garv_and_gunn_needs_a_readied_ssruu_and_a_lender_holding_the_agent() {
    covers!("silent::naaz::AGENT_COPY");
    let (system, planet) = explorable_planet();
    let seats = [Faction::TheYssarilTribes, Faction::TheNaazRokhaAlliance, Faction::TheEmiratesOfHacan];
    let ready = |s: &mut CoreState| {
        stocked(s);
        set_leader(s, 0, Leader::Ssruu, LeaderStatus::Readied);
        set_leader(s, 1, Leader::GarvAndGunn, LeaderStatus::Exhausted);
    };
    let scenario = || Scenario::new(&seats).map_from(&[system]).control(planet, 2);
    let mut r = rig(scenario(), ready, seat_event(EventKind::TurnPassed, 2));
    assert_eq!(r.asked().actor, Seat(0), "{}", r.transcript());
    r.decline();
    assert_eq!(status_of(&r, 0, Leader::Ssruu), Some(LeaderStatus::Readied));
    assert!(rig(scenario(), |s| { ready(s); set_leader(s, 0, Leader::Ssruu, LeaderStatus::Exhausted); }, seat_event(EventKind::TurnPassed, 2)).is_done(), "Ssruu spent");
    assert!(rig(scenario(), |s| { stocked(s); set_leader(s, 0, Leader::Ssruu, LeaderStatus::Readied); }, seat_event(EventKind::TurnPassed, 2)).is_done(), "the lender holds no agent card");
    // No Ssruu card: only the agent's own window (the Naaz seat's), never a copy asked of seat 0.
    let own = rig(scenario(), |s| { stocked(s); set_leader(s, 1, Leader::GarvAndGunn, LeaderStatus::Readied); }, seat_event(EventKind::TurnPassed, 2));
    assert!(!own.is_done(), "the agent's own window is offered: {}", own.transcript());
    assert_eq!(own.asked().actor, Seat(1), "{}", own.transcript());
    assert!(rig(Scenario::new(&seats).map_from(&[system]), ready, seat_event(EventKind::TurnPassed, 2)).is_done(), "no planet to explore");
}

#[test]
fn dart_and_tai_are_offered_for_a_planet_taken_from_another_player() {
    covers!("silent::naaz::COMMANDER");
    let (system, planet) = explorable_planet();
    let took = |previous: Option<u8>, by: u8| {
        let mut p = Payload::new().with(Key::Player, Val::Seat(Seat(by))).with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet));
        if let Some(prev) = previous {
            p.set(Key::PreviousOwner, Val::Seat(Seat(prev)));
        }
        fx::event(EventKind::PlanetControlGained, p)
    };
    let unlocked = |s: &mut CoreState| { stocked(s); set_leader(s, 0, Leader::DartAndTai, LeaderStatus::Unlocked); };
    let scenario = || Scenario::new(&NAAZ).map_from(&[system]).control(planet, 0);
    let mut r = rig(scenario(), unlocked, took(Some(1), 0));
    assert_eq!(r.asked().actor, Seat(0), "{}", r.transcript());
    r.decline();
    assert!(rig(scenario(), unlocked, took(None, 0)).is_done(), "a planet nobody held is explored by the first claim, not by the commander");
    assert!(rig(scenario(), unlocked, took(Some(1), 1)).is_done(), "another seat's gain");
    assert!(rig(scenario(), |s| { stocked(s); }, took(Some(1), 0)).is_done(), "a locked commander");
    assert!(rig(Scenario::new(&NAAZ).map_from(&[system]).control(planet, 1), unlocked, took(Some(1), 0)).is_done(), "the planet is no longer the Naaz's");
    assert!(rig(scenario(), |s| { set_leader(s, 0, Leader::DartAndTai, LeaderStatus::Unlocked); }, took(Some(1), 0)).is_done(), "no deck");
}
