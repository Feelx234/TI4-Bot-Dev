//! D4-2c: the display facts of a reaction window (`kernel::trigger`): whose act the event was, the card, the agenda, the system and the chain.

use ti4_engine_core::choice::reaction::Relation;
use ti4_engine_core::kernel::trigger::{kind_of, Trigger, TriggerCard, TriggerKind};
use ti4_engine_core::state::{Code, EventKind, EventRec, Key, Payload, Tally, Val};
use ti4_engine_core_tests::prelude::*;

fn ev(kind: EventKind, p: Payload) -> EventRec {
    EventRec { id: 11, kind, payload: p, cancelled: false }
}

fn seat(n: u8) -> Val {
    Val::Seat(Seat(n))
}

#[test]
fn every_event_kind_has_a_trigger_kind_and_the_transaction_events_share_one() {
    covers!("kernel::trigger::kind_of");
    let mut other = 0;
    for e in EventKind::ALL {
        if kind_of(e) == TriggerKind::Other {
            other += 1;
        }
    }
    assert!(other > 0 && other < EventKind::ALL.len(), "some events are not windows, most are");
    for e in [EventKind::TransactionOpened, EventKind::TransactionResolved, EventKind::TransactionRefused, EventKind::TransactionOffered, EventKind::TransactionRejected, EventKind::TransactionAbandoned] {
        assert_eq!(kind_of(e), TriggerKind::Transaction, "{e:?}");
    }
    assert_eq!(kind_of(EventKind::SpaceCombatStarted), TriggerKind::CombatStarted);
    assert_eq!(kind_of(EventKind::CombatRoundStarted), TriggerKind::CombatStarted);
    assert_eq!(kind_of(EventKind::RetreatDeclared), TriggerKind::Retreat);
    assert_eq!(kind_of(EventKind::RetreatStepStarted), TriggerKind::Retreat);
    assert_eq!(kind_of(EventKind::ActionCardPlayed), TriggerKind::ActionCardPlayed);
}

#[test]
fn the_actor_and_the_subject_follow_the_kind_of_event() {
    covers!("kernel::trigger::Trigger");
    let player_partner = Payload::new().with(Key::Player, seat(0)).with(Key::Partner, seat(2));
    let t = Trigger::from_event(&ev(EventKind::TransactionOpened, player_partner), Relation::When, &[]);
    assert_eq!((t.actor, t.subject), (Some(Seat(0)), Some(Seat(2))));
    let shot = Payload::new().with(Key::Gunner, seat(1)).with(Key::Player, seat(3)).with(Key::Hits, Val::Int(2));
    let t = Trigger::from_event(&ev(EventKind::SpaceCannonHits, shot), Relation::When, &[]);
    assert_eq!((t.actor, t.subject, t.hits), (Some(Seat(1)), Some(Seat(3)), Some(2)), "the victim is the subject");
    let elected = Payload::new().with(Key::Player, seat(0)).with(Key::ElectedPlayer, seat(1)).with(Key::Agenda, Val::Agenda(Agenda::PoliticalCensure));
    let t = Trigger::from_event(&ev(EventKind::AgendaResolved, elected), Relation::When, &[]);
    assert_eq!((t.actor, t.subject, t.agenda), (None, Some(Seat(1)), Some(Agenda::PoliticalCensure)));
    for phase in [EventKind::AgendaPhaseBegan, EventKind::StrategyPhaseBegan, EventKind::GroundRollsMade] {
        let t = Trigger::from_event(&ev(phase, Payload::new().with(Key::Player, seat(0))), Relation::After, &[]);
        assert_eq!((t.actor, t.subject), (None, None), "{phase:?}: nobody's act");
    }
    let t = Trigger::from_event(&ev(EventKind::TurnBegan, Payload::new().with(Key::Player, seat(2))), Relation::After, &[]);
    assert_eq!((t.actor, t.subject), (Some(Seat(2)), None));
}

#[test]
fn the_card_is_named_only_for_the_events_that_play_or_choose_one_and_a_text_card_stays_other() {
    covers!("kernel::trigger::Trigger");
    let played = |v: Val| Payload::new().with(Key::Player, seat(0)).with(Key::Card, v);
    let t = Trigger::from_event(&ev(EventKind::ActionCardPlayed, played(Val::Action(ActionCard::Sabotage1))), Relation::When, &[]);
    assert_eq!(t.card, Some(TriggerCard::Action(ActionCard::Sabotage1)));
    let t = Trigger::from_event(&ev(EventKind::StrategyCardChosen, played(Val::Strategy(StrategyCard::Trade))), Relation::After, &[]);
    assert_eq!(t.card, Some(TriggerCard::Strategy(StrategyCard::Trade)));
    let t = Trigger::from_event(&ev(EventKind::StrategicActionBegan, played(Val::Code(Code(4)))), Relation::When, &[]);
    assert_eq!(t.card, Some(TriggerCard::Other(Code(4))));
    let t = Trigger::from_event(&ev(EventKind::StrategicActionBegan, played(Val::Int(1))), Relation::When, &[]);
    assert_eq!(t.card, None, "a value that is not a card");
    let t = Trigger::from_event(&ev(EventKind::SystemActivated, played(Val::Action(ActionCard::Sabotage1))), Relation::After, &[]);
    assert_eq!(t.card, None, "an activation names no card");
}

#[test]
fn the_agenda_is_named_only_when_revealed_or_resolved() {
    covers!("kernel::trigger::Trigger");
    let with = |kind| Trigger::from_event(&ev(kind, Payload::new().with(Key::Agenda, Val::Agenda(Agenda::ALL[0]))), Relation::After, &[]);
    assert_eq!(with(EventKind::AgendaRevealed).agenda, Some(Agenda::ALL[0]));
    assert_eq!(with(EventKind::AgendaResolved).agenda, Some(Agenda::ALL[0]));
    assert_eq!(with(EventKind::VotesCast).agenda, None);
}

#[test]
fn place_units_hits_and_the_chain_are_copied_and_a_negative_hit_count_is_dropped() {
    covers!("kernel::trigger::Trigger");
    let system = System::ALL[20];
    let mut p = Payload::new().with(Key::Player, seat(1)).with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(Planet::ALL[0])).with(Key::Hits, Val::Int(-1));
    p.units.push(Tally { owner: Seat(1), unit: Unit::FighterI, count: 2 });
    let t = Trigger::from_event(&ev(EventKind::HitsToAssign, p), Relation::After, &[3, 5, 8]);
    assert_eq!((t.system, t.planet, t.hits), (Some(system), Some(Planet::ALL[0]), None));
    assert_eq!(t.units.as_slice(), &[Tally { owner: Seat(1), unit: Unit::FighterI, count: 2 }]);
    assert_eq!((t.chain.as_slice(), t.event, t.event_id, t.relation, t.kind), (&[3, 5, 8][..], EventKind::HitsToAssign, 11, Relation::After, TriggerKind::HitsToAssign));
}

#[test]
fn a_combat_start_prefers_the_attacker_over_the_player() {
    covers!("kernel::trigger::Trigger");
    let p = Payload::new().with(Key::Player, seat(0)).with(Key::Attacker, seat(2)).with(Key::Defender, seat(1));
    let t = Trigger::from_event(&ev(EventKind::SpaceCombatStarted, p), Relation::After, &[]);
    assert_eq!((t.actor, t.subject), (Some(Seat(2)), Some(Seat(1))));
    let only_player = Payload::new().with(Key::Player, seat(0)).with(Key::Defender, seat(1));
    let t = Trigger::from_event(&ev(EventKind::CombatRoundStarted, only_player), Relation::After, &[]);
    assert_eq!(t.actor, Some(Seat(0)));
}
