//! The display facts of a reaction window (`trigger.rs`): one pure function over the event already in
//! hand where the decision is raised, so it recomputes identically on replay and cannot leak anything the
//! payload does not already say publicly.

use crate::containers::SmallList;
use crate::generated::{ActionCard, Agenda, Planet, StrategyCard, System};
use crate::ids::Seat;
use crate::state::{EventKind, EventRec, Key, Tally, Val};

use crate::choice::reaction::Relation;

/// `decision_context::TriggerKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TriggerKind {
    ActionCardPlayed,
    ActionCardDiscarded,
    SystemActivated,
    ShipMoved,
    StrategicActionBegan,
    StrategyCardChosen,
    StrategyPhaseBegan,
    TurnBegan,
    TurnPassed,
    PlayerPassed,
    ActionCompleted,
    StrategyCardsWouldReturn,
    AgendaPhaseBegan,
    AgendaRevealed,
    VotesCast,
    AgendaResolved,
    Transaction,
    PlanetControlGained,
    InvasionBegan,
    UnitsCommitted,
    GroundRolls,
    CombatStarted,
    AntiFighterBarrage,
    SpaceCannonHits,
    HitsToAssign,
    SustainDamage,
    ShipDestroyed,
    Retreat,
    SpaceCombatWon,
    ProductionUsed,
    UnitAbilityRolled,
    Other,
}

/// The kind of trigger an event kind belongs to, one `match` so a new window is one line.
#[must_use]
pub const fn kind_of(event: EventKind) -> TriggerKind {
    use EventKind as E;
    use TriggerKind as T;
    match event {
        E::ActionCardPlayed => T::ActionCardPlayed,
        E::ActionCardDiscarded => T::ActionCardDiscarded,
        E::SystemActivated => T::SystemActivated,
        E::ShipMoved => T::ShipMoved,
        E::StrategicActionBegan => T::StrategicActionBegan,
        E::StrategyCardChosen => T::StrategyCardChosen,
        E::StrategyPhaseBegan => T::StrategyPhaseBegan,
        E::TurnBegan => T::TurnBegan,
        E::TurnPassed => T::TurnPassed,
        E::PlayerPassed => T::PlayerPassed,
        E::ActionCompleted => T::ActionCompleted,
        E::StrategyCardsWouldReturn => T::StrategyCardsWouldReturn,
        E::AgendaPhaseBegan => T::AgendaPhaseBegan,
        E::AgendaRevealed => T::AgendaRevealed,
        E::VotesCast => T::VotesCast,
        E::AgendaResolved => T::AgendaResolved,
        E::TransactionOpened
        | E::TransactionResolved
        | E::TransactionRefused
        | E::TransactionOffered
        | E::TransactionRejected
        | E::TransactionAbandoned => T::Transaction,
        E::PlanetControlGained => T::PlanetControlGained,
        E::InvasionBegan => T::InvasionBegan,
        E::UnitsCommitted => T::UnitsCommitted,
        E::GroundRollsMade => T::GroundRolls,
        E::SpaceCombatStarted | E::CombatRoundStarted => T::CombatStarted,
        E::AntiFighterBarrageStarted => T::AntiFighterBarrage,
        E::SpaceCannonHits => T::SpaceCannonHits,
        E::HitsToAssign => T::HitsToAssign,
        E::SustainDamageUsed => T::SustainDamage,
        E::ShipDestroyed => T::ShipDestroyed,
        E::RetreatStepStarted | E::RetreatDeclared => T::Retreat,
        E::SpaceCombatWon => T::SpaceCombatWon,
        E::ProductionUsed => T::ProductionUsed,
        E::UnitAbilityRolled => T::UnitAbilityRolled,
        _ => T::Other,
    }
}

/// An action card or a strategy card named by a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerCard {
    Action(ActionCard),
    Strategy(StrategyCard),
    /// A card text outside the closed sets (adapter-interned).
    Other(crate::state::Code),
}

/// `decision_context::DecisionTrigger`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub event: EventKind,
    pub event_id: u64,
    pub relation: Relation,
    pub actor: Option<Seat>,
    pub subject: Option<Seat>,
    pub card: Option<TriggerCard>,
    pub agenda: Option<Agenda>,
    pub system: Option<System>,
    pub planet: Option<Planet>,
    pub units: SmallList<Tally, 4>,
    pub hits: Option<u32>,
    pub chain: Vec<u64>,
}

impl Trigger {
    /// The trigger for a window of `relation` opened by `event`; `chain` is the emission stack around it
    /// (ids, outermost first, the event itself excluded).
    #[must_use]
    pub fn from_event(event: &EventRec, relation: Relation, chain: &[u64]) -> Self {
        let kind = kind_of(event.kind);
        let p = &event.payload;
        let seat = |key: Key| p.seat(key);
        // Whose act it was. `player` names the actor for most events, but it names the victim of hits
        // and shots, the outcome of an agenda and nobody for phase and ground-roll events.
        let (actor, subject) = match kind {
            TriggerKind::SpaceCannonHits => (seat(Key::Gunner), seat(Key::Player)),
            TriggerKind::HitsToAssign | TriggerKind::ShipDestroyed => (None, seat(Key::Player)),
            TriggerKind::AgendaResolved => (None, seat(Key::ElectedPlayer)),
            TriggerKind::AgendaRevealed
            | TriggerKind::AgendaPhaseBegan
            | TriggerKind::StrategyPhaseBegan
            | TriggerKind::GroundRolls => (None, None),
            TriggerKind::Transaction => (seat(Key::Player), seat(Key::Partner)),
            TriggerKind::CombatStarted => (seat(Key::Attacker).or_else(|| seat(Key::Player)), seat(Key::Defender)),
            _ => (seat(Key::Player), None),
        };
        let card = if matches!(
            kind,
            TriggerKind::ActionCardPlayed
                | TriggerKind::ActionCardDiscarded
                | TriggerKind::StrategicActionBegan
                | TriggerKind::StrategyCardChosen
        ) {
            match p.get(Key::Card) {
                Some(Val::Action(c)) => Some(TriggerCard::Action(c)),
                Some(Val::Strategy(c)) => Some(TriggerCard::Strategy(c)),
                Some(Val::Code(c)) => Some(TriggerCard::Other(c)),
                _ => None,
            }
        } else {
            None
        };
        let agenda = if matches!(kind, TriggerKind::AgendaRevealed | TriggerKind::AgendaResolved) {
            p.agenda(Key::Agenda)
        } else {
            None
        };
        Self {
            kind,
            event: event.kind,
            event_id: event.id,
            relation,
            actor,
            subject,
            card,
            agenda,
            system: p.system(Key::System),
            planet: p.planet(Key::Planet),
            units: p.units.clone(),
            hits: p.int(Key::Hits).and_then(|h| u32::try_from(h).ok()),
            chain: chain.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Payload;

    fn ev(kind: EventKind, payload: Payload) -> EventRec {
        EventRec { id: 7, kind, payload, cancelled: false }
    }

    #[test]
    fn the_victim_of_hits_is_the_subject_not_the_actor() {
        let p = Payload::new().with(Key::Player, Val::Seat(Seat(2))).with(Key::Hits, Val::Int(3));
        let t = Trigger::from_event(&ev(EventKind::HitsToAssign, p), Relation::After, &[1, 2]);
        assert_eq!((t.actor, t.subject, t.hits), (None, Some(Seat(2)), Some(3)));
        assert_eq!(t.chain, vec![1, 2]);
        assert_eq!(t.kind, TriggerKind::HitsToAssign);
    }

    #[test]
    fn combat_start_prefers_the_attacker_and_other_events_are_other() {
        let p = Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Defender, Val::Seat(Seat(3)));
        let t = Trigger::from_event(&ev(EventKind::CombatRoundStarted, p), Relation::When, &[]);
        assert_eq!((t.actor, t.subject), (Some(Seat(1)), Some(Seat(3))));
        assert_eq!(kind_of(EventKind::TechnologyGained), TriggerKind::Other);
        assert_eq!(kind_of(EventKind::TransactionAbandoned), TriggerKind::Transaction);
    }
}
