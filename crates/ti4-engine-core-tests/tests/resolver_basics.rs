//! D4-2c: `kernel::resolver` without a host: the registry of abilities, the order the players are asked in, the usage counters and the checkpoint.

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind, Frequency, Relation};
use ti4_engine_core::kernel::resolver::{AbilityDef, CannotDef, Counter, Registry, TimingError, UsedScope};
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::state::{Code, EventKind, Phase};
use ti4_engine_core_tests::prelude::*;

fn id(n: u16) -> AbilityId {
    AbilityId { kind: AbilityKind::Other, key: Code(n) }
}

fn def(n: u16, owner: u8, event: EventKind, relation: Relation) -> AbilityDef {
    AbilityDef::new(id(n), Seat(owner), event, relation)
}

#[test]
fn a_new_ability_is_once_per_trigger_mandatory_unconditional_and_the_builders_set_one_thing_each() {
    let d = def(1, 0, EventKind::TurnBegan, Relation::When);
    assert_eq!((d.frequency, d.optional, d.repeatable, d.has_condition, d.arg), (Frequency::OncePerTrigger, false, false, false, 0));
    assert_eq!(d.with_frequency(Frequency::OncePerRound).frequency, Frequency::OncePerRound);
    assert!(d.with_optional(true).optional && d.with_repeatable(true).repeatable && d.with_condition().has_condition);
    assert_eq!(d.with_arg(9).arg, 9);
    assert!(!d.with_optional(true).repeatable, "one builder changes one field");
}

#[test]
fn the_registry_buckets_abilities_by_event_and_relation_keeping_registration_order() {
    let mut r = Registry::default();
    assert!(r.is_empty() && r.len() == 0);
    r.register([def(1, 0, EventKind::TurnBegan, Relation::When), def(2, 1, EventKind::TurnBegan, Relation::After), def(3, 2, EventKind::TurnBegan, Relation::When)]);
    r.register([def(4, 0, EventKind::TurnEndedBy, Relation::When)]);
    assert_eq!(r.len(), 4);
    let keys = |e, rel| r.for_event(e, rel).map(|d| d.id.key.0).collect::<Vec<_>>();
    assert_eq!(keys(EventKind::TurnBegan, Relation::When), vec![1, 3], "in the order registered");
    assert_eq!(keys(EventKind::TurnBegan, Relation::After), vec![2]);
    assert_eq!(keys(EventKind::TurnEndedBy, Relation::When), vec![4]);
    assert!(keys(EventKind::TurnEndedBy, Relation::After).is_empty() && keys(EventKind::RoundBegan, Relation::When).is_empty());
    r.forbid(CannotDef { label: Code(7), arg: 1 });
    assert_eq!(r.cannots(), &[CannotDef { label: Code(7), arg: 1 }]);
}

#[test]
fn players_are_asked_from_the_speaker_in_the_strategy_and_agenda_phases_and_from_the_active_player_otherwise() {
    let mut r = Resolver::new(&[Seat(2), Seat(0), Seat(1)], Some(Seat(0)));
    r.set_seating_order(&[Seat(0), Seat(1), Seat(2)]);
    r.set_speaker(Some(Seat(1)));
    // Action phase: the active player first, then the others in INITIATIVE order (user ruling on S1, docs/STRING_ENGINE_BUGS.md; the string rotates
    // its initiative list from the active seat, which here would give 0, 1, 2).
    r.set_phase(Phase::Action);
    assert_eq!(r.player_order().as_slice(), &[Seat(0), Seat(2), Seat(1)]);
    r.set_active_player(Some(Seat(1)));
    assert_eq!(r.player_order().as_slice(), &[Seat(1), Seat(2), Seat(0)]);
    // Strategy and agenda phases: the seating order, starting at the speaker.
    for phase in [Phase::Strategy, Phase::Agenda] {
        r.set_phase(phase);
        assert_eq!(r.player_order().as_slice(), &[Seat(1), Seat(2), Seat(0)], "{phase:?}");
    }
    r.set_speaker(None);
    assert_eq!(r.player_order().as_slice(), &[Seat(0), Seat(1), Seat(2)], "no speaker: the sequence as it is");
    // Someone who is not in the sequence goes first.
    r.set_phase(Phase::Action);
    r.set_active_player(Some(Seat(5)));
    assert_eq!(r.player_order().as_slice(), &[Seat(5), Seat(2), Seat(0), Seat(1)]);
    // Without a seating order the strategy phase falls back on the initiative order.
    let mut bare = Resolver::new(&[Seat(2), Seat(0)], None);
    bare.set_phase(Phase::Strategy);
    bare.set_speaker(Some(Seat(0)));
    assert_eq!(bare.player_order().as_slice(), &[Seat(0), Seat(2)]);
}

#[test]
fn usage_is_remembered_per_scope_and_the_counters_move_with_turns_and_rounds() {
    let mut r = Resolver::new(&[Seat(0)], Some(Seat(0)));
    r.spend(id(1), UsedScope::Turn, 1);
    r.spend(id(2), UsedScope::Round, 1);
    r.spend(id(3), UsedScope::Trigger, 17);
    assert_eq!(r.used_keys(), vec![(id(1), UsedScope::Turn, 1), (id(2), UsedScope::Round, 1), (id(3), UsedScope::Trigger, 17)], "in the order spent");
    r.spend(id(1), UsedScope::Turn, 1);
    assert_eq!(r.used_keys().len(), 3, "spending twice is one use");
    r.begin_turn(Seat(0)).unwrap();
    r.begin_round().unwrap();
    r.sync_lifecycle(7, 9);
    r.set_counters(u64::MAX, 1);
    assert_eq!(r.begin_round(), Err(TimingError::CounterExhausted(Counter::Round)));
    r.set_counters(1, u64::MAX);
    assert_eq!(r.begin_turn(Seat(0)), Err(TimingError::CounterExhausted(Counter::Turn)));
    assert_eq!(Resolver::DEFAULT_MAXIMUM_DEPTH, 100);
    r.set_maximum_depth(5);
    assert_eq!(r.maximum_depth(), 5);
    assert!(std::panic::catch_unwind(|| Resolver::new(&[], None).set_maximum_depth(0)).is_err(), "a resolver needs room for its root event");
}

#[test]
fn a_checkpoint_restores_the_registry_and_the_usage_exactly() {
    let mut r = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
    r.register([def(1, 0, EventKind::TurnBegan, Relation::When)]);
    r.spend(id(1), UsedScope::Turn, 1);
    let cp = r.checkpoint();
    let before = format!("{cp:?}");
    r.register([def(2, 1, EventKind::TurnEndedBy, Relation::After)]);
    r.forbid(CannotDef { label: Code(1), arg: 0 });
    r.spend(id(2), UsedScope::Round, 1);
    r.begin_turn(Seat(1)).unwrap();
    assert_ne!(format!("{:?}", r.checkpoint()), before);
    r.restore(cp);
    assert_eq!(format!("{:?}", r.checkpoint()), before, "restored");
    assert_eq!(r.registry().len(), 1);
    assert!(r.registry().cannots().is_empty());
    assert_eq!(r.used_keys().len(), 1);
    assert_eq!(r.for_event(EventKind::TurnEndedBy, Relation::After).count(), 0);
}

/// S1 (user ruling): an action-phase window offers the ACTIVE player first and then the other players in initiative order, not the initiative list
/// rotated from the active seat (the string's order, fixed at `Game::new`). The two differ when the active seat is not first in the list.
#[test]
fn an_action_phase_window_offers_the_active_player_first_and_the_others_in_initiative_order() {
    let mut r = Resolver::new(&[Seat(3), Seat(0), Seat(2), Seat(1)], Some(Seat(2)));
    r.set_seating_order(&[Seat(0), Seat(1), Seat(2), Seat(3)]);
    r.set_phase(Phase::Action);
    assert_eq!(r.player_order().as_slice(), &[Seat(2), Seat(3), Seat(0), Seat(1)], "rotated from the active seat it would be 2, 1, 3, 0");
    r.set_active_player(Some(Seat(3)));
    assert_eq!(r.player_order().as_slice(), &[Seat(3), Seat(0), Seat(2), Seat(1)]);
    r.set_active_player(Some(Seat(1)));
    assert_eq!(r.player_order().as_slice(), &[Seat(1), Seat(3), Seat(0), Seat(2)]);
}
