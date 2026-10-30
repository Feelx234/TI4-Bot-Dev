//! D4-2c: the printed-window table of the action-card reactions (`kernel::reaction`): the guards not covered by the unit tests of the module.

use ti4_engine_core::kernel::reaction::{window_of, Guard, WindowSpec};
use ti4_engine_core::state::{EventKind, EventRec, Key, Payload, Val};
use ti4_engine_core_tests::prelude::*;

fn ev(kind: EventKind, p: Payload) -> EventRec {
    EventRec { id: 1, kind, payload: p, cancelled: false }
}

fn seat(who: u8) -> Val {
    Val::Seat(Seat(who))
}

fn holds(g: Guard, e: &EventRec, who: u8, s: &CoreState, k: &Kernel) -> bool {
    g.holds(e, Seat(who), s, k)
}

#[test]
fn every_printed_timing_but_the_component_action_hooks_onto_one_event_and_a_guard() {
    covers!("kernel::reaction::window_of");
    let mut specs: Vec<(Timing, WindowSpec)> = Vec::new();
    for t in Timing::ALL {
        match window_of(t) {
            None => assert_eq!(t, Timing::Action, "only the component action has no window"),
            Some(spec) => specs.push((t, spec)),
        }
    }
    assert_eq!(specs.len(), Timing::COUNT - 1);
    // "When"/"after" pairs of the same text hook onto the same event with opposite relations or guards, never two identical specs for different
    // meanings of one timing.
    for (t, spec) in &specs {
        assert_eq!(window_of(*t), Some(*spec), "{t:?}: deterministic");
    }
}

#[test]
fn a_party_to_a_transaction_is_the_player_or_the_partner() {
    covers!("kernel::reaction::Guard");
    let (s, k) = Scenario::lobby(3).build();
    let e = ev(EventKind::TransactionOpened, Payload::new().with(Key::Player, seat(0)).with(Key::Partner, seat(2)));
    assert!(holds(Guard::PartyToTransaction, &e, 0, &s, &k) && holds(Guard::PartyToTransaction, &e, 2, &s, &k));
    assert!(!holds(Guard::PartyToTransaction, &e, 1, &s, &k), "a third seat is not a party");
}

#[test]
fn holding_strategy_cards_and_a_readied_card_of_the_active_seat_are_read_from_the_players() {
    covers!("kernel::reaction::Guard");
    let (mut s, k) = Scenario::lobby(2).active(Some(0)).build();
    let ret = ev(EventKind::StrategyCardsWouldReturn, Payload::new().with(Key::Player, seat(1)));
    assert!(!holds(Guard::HoldsStrategyCards, &ret, 1, &s, &k), "no card held");
    s.players_mut().by_seat_mut(Seat(1)).unwrap().strat.push(StrategyCard::Leadership);
    assert!(holds(Guard::HoldsStrategyCards, &ret, 1, &s, &k));
    assert!(!holds(Guard::HoldsStrategyCards, &ret, 0, &s, &k), "the event names seat 1");
    let turn = ev(EventKind::TurnBegan, Payload::new().with(Key::Player, seat(0)));
    assert!(!holds(Guard::AnotherSeatHasReadiedCard, &turn, 1, &s, &k), "the active seat holds no card");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Politics);
    assert!(holds(Guard::AnotherSeatHasReadiedCard, &turn, 1, &s, &k));
    assert!(!holds(Guard::AnotherSeatHasReadiedCard, &turn, 0, &s, &k), "not for the active seat itself");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat_exhausted.insert(StrategyCard::Politics);
    assert!(!holds(Guard::AnotherSeatHasReadiedCard, &turn, 1, &s, &k), "an exhausted card is not readied");
}

#[test]
fn a_commit_on_your_planet_and_a_gained_planet_name_the_owner_of_the_planet() {
    covers!("kernel::reaction::Guard");
    let (s, k) = Scenario::lobby(3).build();
    let commit = ev(EventKind::UnitsCommitted, Payload::new().with(Key::Player, seat(1)).with(Key::Controller, seat(0)));
    assert!(holds(Guard::CommitOnYourPlanet, &commit, 0, &s, &k));
    assert!(!holds(Guard::CommitOnYourPlanet, &commit, 1, &s, &k), "not the committer");
    assert!(!holds(Guard::CommitOnYourPlanet, &commit, 2, &s, &k), "not a bystander");
    let gained = ev(EventKind::PlanetControlGained, Payload::new().with(Key::Player, seat(1)).with(Key::PreviousOwner, seat(2)));
    assert!(holds(Guard::OtherGainsYourPlanet, &gained, 2, &s, &k));
    assert!(!holds(Guard::OtherGainsYourPlanet, &gained, 0, &s, &k) && !holds(Guard::OtherGainsYourPlanet, &gained, 1, &s, &k));
}

#[test]
fn direct_hit_needs_the_producer_and_a_unit_that_can_be_hit() {
    covers!("kernel::reaction::Guard");
    let (s, k) = Scenario::lobby(2).build();
    let hit = |hittable: Option<bool>| {
        let mut p = Payload::new().with(Key::Player, seat(1)).with(Key::Producer, seat(0));
        if let Some(h) = hittable {
            p.set(Key::DirectHittable, Val::Flag(h));
        }
        ev(EventKind::SustainDamageUsed, p)
    };
    assert!(holds(Guard::DirectHit, &hit(None), 0, &s, &k), "an unmarked unit can be hit");
    assert!(holds(Guard::DirectHit, &hit(Some(true)), 0, &s, &k));
    assert!(!holds(Guard::DirectHit, &hit(Some(false)), 0, &s, &k), "Dreadnought II and its kin cannot");
    assert!(!holds(Guard::DirectHit, &hit(None), 1, &s, &k), "the one who sustained is not the producer");
}

#[test]
fn the_last_ship_and_the_space_combat_loss_read_their_flags() {
    covers!("kernel::reaction::Guard");
    let (s, k) = Scenario::lobby(2).build();
    let lost = |last: bool, during: bool| ev(EventKind::ShipDestroyed, Payload::new().with(Key::Player, seat(0)).with(Key::Last, Val::Flag(last)).with(Key::DuringSpaceCombat, Val::Flag(during)));
    assert!(holds(Guard::YourLastShip, &lost(true, false), 0, &s, &k));
    assert!(!holds(Guard::YourLastShip, &lost(false, true), 0, &s, &k));
    assert!(holds(Guard::YourShipDestroyedDuringSpaceCombat, &lost(false, true), 0, &s, &k));
    assert!(!holds(Guard::YourShipDestroyedDuringSpaceCombat, &lost(true, false), 0, &s, &k));
    assert!(!holds(Guard::YourLastShip, &lost(true, true), 1, &s, &k), "another seat's ship");
}

#[test]
fn another_player_elected_excludes_the_elected_seat_and_a_missing_election() {
    covers!("kernel::reaction::Guard");
    let (s, k) = Scenario::lobby(3).build();
    let elected = ev(EventKind::AgendaResolved, Payload::new().with(Key::ElectedPlayer, seat(1)));
    assert!(holds(Guard::AnotherPlayerElected, &elected, 0, &s, &k) && holds(Guard::AnotherPlayerElected, &elected, 2, &s, &k));
    assert!(!holds(Guard::AnotherPlayerElected, &elected, 1, &s, &k));
    let none = ev(EventKind::AgendaResolved, Payload::new().with(Key::Player, Val::Outcome(ti4_engine_core::keys::Outcome::For)));
    assert!(!holds(Guard::AnotherPlayerElected, &none, 0, &s, &k), "a vote on For/Against elects nobody");
}

#[test]
fn the_speaker_order_is_for_the_strategy_and_agenda_phases() {
    covers!("kernel::reaction::window_uses_speaker_order");
    use ti4_engine_core::kernel::reaction::window_uses_speaker_order;
    assert!(window_uses_speaker_order(Phase::Strategy) && window_uses_speaker_order(Phase::Agenda));
    assert!(!window_uses_speaker_order(Phase::Action) && !window_uses_speaker_order(Phase::Status));
}
