//! String `Event` <-> core `EventRec` for the facts a reaction trigger reads, and the core `Trigger`
//! rendered as the string `DecisionTrigger` (the dual of `trigger.rs`).

use std::collections::BTreeSet;

use serde_json::Value;
use ti4_engine_core::choice::reaction::Relation as CRel;
use ti4_engine_core::generated::{ActionCard, Agenda, Planet, StrategyCard, System, Unit};
use ti4_engine_core::kernel::trigger::{Trigger, TriggerCard, TriggerKind as CKind};
use ti4_engine_core::state::{EventRec, Key, Payload, Tally, Val};
use ti4_engine_core::Seat;
use ti4_model::id::{PlanetId, PlayerId, SystemId};

use super::text::{event_kind, event_text, relation_text};
use crate::wire::decision_context::{DecisionTrigger, TriggerKind, TriggerUnits};
use crate::wire::event::Event;
use crate::generated::IdStr;
use crate::render::Render;

/// The seat names an event's trigger facts mention.
#[must_use]
pub fn seat_names(event: &Event) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for key in ["player", "gunner", "partner", "elected_player", "attacker", "defender"] {
        if let Some(t) = event.text(key) {
            if !t.is_empty() {
                names.insert(t.to_owned());
            }
        }
    }
    if let Some(units) = event.payload.get("units").and_then(Value::as_array) {
        for u in units {
            if let Some(o) = u.get("owner").and_then(Value::as_str) {
                names.insert(o.to_owned());
            }
        }
    }
    names
}

/// The core record of an event, carrying exactly the facts a trigger reads. `None` when one of them has
/// a form the typed payload does not hold (the comparison is then skipped).
#[must_use]
pub fn event_rec(r: &Render, event: &Event) -> Option<EventRec> {
    let mut p = Payload::new();
    let seat = |t: &str| r.seat(t);
    for (key, k) in [
        ("player", Key::Player),
        ("gunner", Key::Gunner),
        ("partner", Key::Partner),
        ("elected_player", Key::ElectedPlayer),
        ("attacker", Key::Attacker),
        ("defender", Key::Defender),
    ] {
        match event.payload.get(key) {
            None => {}
            Some(Value::String(t)) if t.is_empty() => {}
            Some(Value::String(t)) => p.set(k, Val::Seat(seat(t)?)),
            // an outcome that is not a seat (an agenda's `for`): the trigger reads it as a seat or not at all
            Some(_) => return None,
        }
    }
    if let Some(Value::String(c)) = event.payload.get("card") {
        let val = if let Some(a) = ActionCard::from_id(c) {
            Val::Action(a)
        } else if let Some(s) = StrategyCard::from_id(c) {
            Val::Strategy(s)
        } else {
            Val::Code(r.code(c))
        };
        p.set(Key::Card, val);
    }
    if let Some(Value::String(a)) = event.payload.get("agenda") {
        p.set(Key::Agenda, Val::Agenda(Agenda::from_id(a)?));
    }
    if let Some(Value::String(s)) = event.payload.get("system") {
        p.set(Key::System, Val::System(System::from_id(s)?));
    }
    if let Some(Value::String(s)) = event.payload.get("planet") {
        p.set(Key::Planet, Val::Planet(Planet::from_id(s)?));
    }
    if let Some(h) = event.payload.get("hits") {
        p.set(Key::Hits, Val::Int(h.as_i64()?));
    }
    if let Some(units) = event.payload.get("units").and_then(Value::as_array) {
        for u in units {
            // Entries the string trigger would drop (a missing field) are dropped here too.
            let (Some(owner), Some(ty), Some(count)) = (
                u.get("owner").and_then(Value::as_str),
                u.get("unit_type").and_then(Value::as_str),
                u.get("count").and_then(Value::as_u64),
            ) else {
                continue;
            };
            let Ok(count) = u32::try_from(count) else { continue };
            p.units.push(Tally { owner: seat(owner)?, unit: Unit::from_id(ty)?, count });
        }
    }
    Some(EventRec { id: event.id, kind: event_kind(r, &event.event_type), payload: p, cancelled: false })
}

fn kind(k: CKind) -> TriggerKind {
    match k {
        CKind::ActionCardPlayed => TriggerKind::ActionCardPlayed,
        CKind::ActionCardDiscarded => TriggerKind::ActionCardDiscarded,
        CKind::SystemActivated => TriggerKind::SystemActivated,
        CKind::ShipMoved => TriggerKind::ShipMoved,
        CKind::StrategicActionBegan => TriggerKind::StrategicActionBegan,
        CKind::StrategyCardChosen => TriggerKind::StrategyCardChosen,
        CKind::StrategyPhaseBegan => TriggerKind::StrategyPhaseBegan,
        CKind::TurnBegan => TriggerKind::TurnBegan,
        CKind::TurnPassed => TriggerKind::TurnPassed,
        CKind::PlayerPassed => TriggerKind::PlayerPassed,
        CKind::ActionCompleted => TriggerKind::ActionCompleted,
        CKind::StrategyCardsWouldReturn => TriggerKind::StrategyCardsWouldReturn,
        CKind::AgendaPhaseBegan => TriggerKind::AgendaPhaseBegan,
        CKind::AgendaRevealed => TriggerKind::AgendaRevealed,
        CKind::VotesCast => TriggerKind::VotesCast,
        CKind::AgendaResolved => TriggerKind::AgendaResolved,
        CKind::Transaction => TriggerKind::Transaction,
        CKind::PlanetControlGained => TriggerKind::PlanetControlGained,
        CKind::InvasionBegan => TriggerKind::InvasionBegan,
        CKind::UnitsCommitted => TriggerKind::UnitsCommitted,
        CKind::GroundRolls => TriggerKind::GroundRolls,
        CKind::CombatStarted => TriggerKind::CombatStarted,
        CKind::AntiFighterBarrage => TriggerKind::AntiFighterBarrage,
        CKind::SpaceCannonHits => TriggerKind::SpaceCannonHits,
        CKind::HitsToAssign => TriggerKind::HitsToAssign,
        CKind::SustainDamage => TriggerKind::SustainDamage,
        CKind::ShipDestroyed => TriggerKind::ShipDestroyed,
        CKind::Retreat => TriggerKind::Retreat,
        CKind::SpaceCombatWon => TriggerKind::SpaceCombatWon,
        CKind::ProductionUsed => TriggerKind::ProductionUsed,
        CKind::UnitAbilityRolled => TriggerKind::UnitAbilityRolled,
        CKind::Other => TriggerKind::Other,
    }
}

/// The string trigger a core trigger renders as.
#[must_use]
pub fn trigger_text(r: &Render, t: &Trigger) -> DecisionTrigger {
    let who = |s: Seat| PlayerId::new(r.seat_text(s));
    DecisionTrigger {
        kind: kind(t.kind),
        event_type: event_text(r, t.event),
        event_id: t.event_id,
        relation: relation_text(match t.relation {
            CRel::When => CRel::When,
            CRel::After => CRel::After,
        })
        .to_owned(),
        actor: t.actor.map(who),
        subject: t.subject.map(who),
        card: t.card.map(|c| match c {
            TriggerCard::Action(a) => a.as_str().to_owned(),
            TriggerCard::Strategy(s) => s.as_str().to_owned(),
            TriggerCard::Other(code) => r.code_text(code),
        }),
        agenda: t.agenda.map(|a| a.as_str().to_owned()),
        system: t.system.map(|s| SystemId::new(s.as_str())),
        planet: t.planet.map(|p| PlanetId::new(p.as_str())),
        units: t
            .units
            .iter()
            .map(|u| TriggerUnits { owner: who(u.owner), unit_type: u.unit.as_str().to_owned(), count: u.count })
            .collect(),
        hits: t.hits,
        chain: t.chain.clone(),
    }
}

/// The core's version of `DecisionTrigger::from_event`, or `None` when the event holds a fact the typed
/// payload cannot (the dual skips it).
#[must_use]
pub fn core_trigger(event: &Event, relation: &str, chain: &[u64]) -> Option<DecisionTrigger> {
    let names = seat_names(event);
    let r = Render::new(names.into_iter().collect());
    let rec = event_rec(&r, event)?;
    let rel = match relation {
        "when" => CRel::When,
        "after" => CRel::After,
        _ => return None,
    };
    Some(trigger_text(&r, &Trigger::from_event(&rec, rel, chain)))
}
