//! The printed-window table of action-card reactions (`reactions.rs`): where each printed window text
//! hooks onto the engine's events, and the guard that narrows it ("you", "another player", "your planet").
//!
//! A card's window is its generated [`Timing`]; the table maps every non-`Action` timing to the event it
//! listens for, whether it resolves `WHEN` or `AFTER` it, and a [`Guard`]. Guards are data (an enum),
//! not closures: [`Guard::holds`] reads the event and the state. Whether the card then has anything to do
//! (22.3) is the action-card package's question (`reaction_can_matter`), not the window's.

use crate::choice::reaction::Relation;
use crate::generated::{ActionCard, Timing, Unit};
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::keys::prediction::Prediction;
use crate::state::{CoreState, EventKind, EventRec, Key, Outcome, Phase, Val};

/// Whether a printed window applies to a seat, given the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// The window applies whoever the event names.
    Anyone,
    /// The event's `player` is this seat ("you").
    ActorIs,
    /// Somebody else ("another player", "your opponent").
    ActorIsNot,
    /// A combatant named by the event (`attacker` / `defender`).
    Combatant,
    /// A combatant, and only in the first round (a combat starts once, the round event fires every round).
    CombatantFirstRound,
    /// A party to the transaction (`player` or `partner`).
    PartyToTransaction,
    /// The seat holds strategy cards (Political Stability).
    HoldsStrategyCards,
    /// The active seat has a readied strategy card and is not this seat (Extreme Duress).
    AnotherSeatHasReadiedCard,
    /// Another player committed to a planet this seat controls.
    CommitOnYourPlanet,
    /// Another player's sustain against a hit this seat's units produced (Direct Hit).
    DirectHit,
    /// Another player's activation, and the activated system holds this seat's units.
    OtherActivatesWithYourUnits,
    /// ... holds this seat's command token.
    OtherActivatesWithYourCommandToken,
    /// ... holds this seat's space docks or PDS.
    OtherActivatesWithYourStructures,
    /// Another player plays an action card that is not Sabotage (and whose Last Bastion commander does
    /// not stop it).
    OtherPlaysCardNotSabotage,
    /// Another player is elected (`elected_player`, set only for a real seat).
    AnotherPlayerElected,
    /// This seat's last ship in the system was destroyed (`last`).
    YourLastShip,
    /// This seat's ship was destroyed during a space combat.
    YourShipDestroyedDuringSpaceCombat,
    /// This seat voted for or predicted another outcome than the one about to resolve (Deadly Plot).
    VotedOrPredictedAnother,
    /// At least two players have not passed (Crisis).
    TwoPlayersNotPassed,
    /// Another player gained a planet this seat controlled (`previous_owner`).
    OtherGainsYourPlanet,
}

/// Where a printed window hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSpec {
    pub event: EventKind,
    pub relation: Relation,
    pub guard: Guard,
}

const fn w(event: EventKind, relation: Relation, guard: Guard) -> Option<WindowSpec> {
    Some(WindowSpec { event, relation, guard })
}

/// The window of a printed timing; `None` for `Action` (a component action, not a reaction).
#[must_use]
pub const fn window_of(timing: Timing) -> Option<WindowSpec> {
    use EventKind as E;
    use Guard as G;
    use Relation::{After, When};
    use Timing as T;
    match timing {
        T::Action => None,
        T::AfterAgendaRevealed => w(E::AgendaRevealed, After, G::Anyone),
        T::WhenAgendaRevealed => w(E::AgendaRevealed, When, G::Anyone),
        T::StartOfAgendaPhase => w(E::AgendaPhaseBegan, After, G::Anyone),
        T::StartOfCombatRound | T::StartOrEndOfCombatRound => w(E::CombatRoundStarted, After, G::Combatant),
        T::StartOfFirstSpaceCombatRound => w(E::SpaceCombatStarted, After, G::Combatant),
        T::AfterYouActivateSystem
        | T::AfterYouActivateSystemWithYourShips
        | T::AfterYouActivateSystemWithOtherShips
        | T::AfterYouActivateAnomaly => w(E::SystemActivated, After, G::ActorIs),
        T::WhenOtherPlaysActionCardNotSabotage => w(E::ActionCardPlayed, When, G::OtherPlaysCardNotSabotage),
        T::AfterOtherActivatesSystemWithYourUnits => w(E::SystemActivated, After, G::OtherActivatesWithYourUnits),
        T::AfterOtherActivatesSystemWithYourCommandToken => {
            w(E::SystemActivated, After, G::OtherActivatesWithYourCommandToken)
        }
        T::AfterOtherActivatesSystemWithYourStructures => w(E::SystemActivated, After, G::OtherActivatesWithYourStructures),
        T::StartOfCombat => w(E::CombatRoundStarted, After, G::CombatantFirstRound),
        T::WhenOtherChoosesStrategyCard => w(E::StrategyCardChosen, After, G::ActorIsNot),
        T::StartOfInvasionAgainstPds | T::StartOfInvasion => w(E::InvasionBegan, After, G::Anyone),
        T::AfterActivePlayerMovesIntoActiveSystem => w(E::ShipMoved, After, G::ActorIs),
        T::AfterShipsMoveIntoSystemWithYourShips => w(E::ShipMoved, After, G::Anyone),
        T::StartOfOtherTurnWithReadiedStrategyCard => w(E::TurnBegan, After, G::AnotherSeatHasReadiedCard),
        T::WhenYouWouldReturnStrategyCards => w(E::StrategyCardsWouldReturn, When, G::HoldsStrategyCards),
        T::EndOfTurnIfYouPassed => w(E::PlayerPassed, After, G::ActorIs),
        T::WhenYouAreElected => w(E::AgendaResolved, When, G::ActorIs),
        T::WhenOtherIsElected => w(E::AgendaResolved, When, G::AnotherPlayerElected),
        T::WhenOutcomeWouldBeResolved => w(E::AgendaResolved, When, G::VotedOrPredictedAnother),
        T::WhenOtherPerformsStrategicAction => w(E::StrategicActionBegan, When, G::ActorIsNot),
        T::EndOfTurnIfTwoNotPassed => w(E::TurnPassed, After, G::TwoPlayersNotPassed),
        T::AfterYouPerformAction => w(E::ActionCompleted, After, G::ActorIs),
        T::WhenYouGainPlanet => w(E::PlanetControlGained, When, G::ActorIs),
        T::AfterOtherGainsYourPlanet => w(E::PlanetControlGained, After, G::OtherGainsYourPlanet),
        T::AfterYouCastVotes => w(E::VotesCast, After, G::ActorIs),
        T::AfterSpeakerVotes => w(E::VotesCast, After, G::Anyone),
        T::AfterYouWinSpaceCombat => w(E::SpaceCombatWon, After, G::ActorIs),
        T::AfterOtherDiscardsComponentActionCard => w(E::ActionCardDiscarded, After, G::ActorIsNot),
        T::WhenNegotiatingTransaction => w(E::TransactionOpened, When, G::PartyToTransaction),
        T::WhenYourUnitsUseProduction => w(E::ProductionUsed, After, G::ActorIs),
        T::AfterOwnShipDestroyedInSpaceCombat => w(E::ShipDestroyed, After, G::YourShipDestroyedDuringSpaceCombat),
        T::WhenYourLastShipIsDestroyed => w(E::ShipDestroyed, When, G::YourLastShip),
        T::WhenYourShipSustainsDamage => w(E::SustainDamageUsed, When, G::ActorIs),
        T::AfterOtherSustainsAgainstYourHit => w(E::SustainDamageUsed, After, G::DirectHit),
        T::BeforeAntiFighterBarrageRoll => w(E::AntiFighterBarrageStarted, When, G::ActorIs),
        T::StartOfStrategyPhase => w(E::StrategyPhaseBegan, After, G::Anyone),
        T::BeforeAssigningSpaceCombatHits => w(E::HitsToAssign, When, G::ActorIs),
        T::BeforeAssigningSpaceCannonHits => w(E::SpaceCannonHits, When, G::ActorIs),
        T::AfterOpponentDeclaresRetreat => w(E::RetreatDeclared, After, G::ActorIsNot),
        T::StartOfAnnounceRetreatsAsDefender => w(E::RetreatStepStarted, After, G::ActorIs),
        T::AfterYourGroundForcesRoll => w(E::GroundRollsMade, After, G::ActorIs),
        T::AfterOtherRollsUnitAbility => w(E::UnitAbilityRolled, After, G::ActorIsNot),
        T::AfterOtherCommitsToYourPlanet => w(E::UnitsCommitted, After, G::CommitOnYourPlanet),
        T::AfterNeighborsTransact => w(E::TransactionResolved, After, G::Anyone),
    }
}

/// Typed events the engine actually emits for the windows (a window mapped to an event nobody emits is
/// a table entry, not a connection).
pub const EMITTED_EVENTS: [EventKind; 34] = [
    EventKind::ActionCardPlayed,
    EventKind::AgendaResolved,
    EventKind::ActionCompleted,
    EventKind::PlanetControlGained,
    EventKind::PlayerPassed,
    EventKind::ShipMoved,
    EventKind::VotesCast,
    EventKind::AgendaPhaseBegan,
    EventKind::AgendaRevealed,
    EventKind::CombatRoundStarted,
    EventKind::InvasionBegan,
    EventKind::ProductionUsed,
    EventKind::SpaceCombatStarted,
    EventKind::SpaceCombatWon,
    EventKind::StrategyCardChosen,
    EventKind::StrategicActionBegan,
    EventKind::SystemActivated,
    EventKind::ShipDestroyed,
    EventKind::StrategyPhaseBegan,
    EventKind::SustainDamageUsed,
    EventKind::AntiFighterBarrageStarted,
    EventKind::GroundRollsMade,
    EventKind::HitsToAssign,
    EventKind::UnitAbilityRolled,
    EventKind::RetreatDeclared,
    EventKind::RetreatStepStarted,
    EventKind::SpaceCannonHits,
    EventKind::UnitsCommitted,
    EventKind::TurnPassed,
    EventKind::TurnBegan,
    EventKind::StrategyCardsWouldReturn,
    EventKind::ActionCardDiscarded,
    EventKind::TransactionResolved,
    EventKind::TransactionOpened,
];

fn actor_is(e: &EventRec, who: Seat) -> bool {
    e.payload.seat(Key::Player) == Some(who)
}

fn actor_is_not(e: &EventRec, who: Seat) -> bool {
    e.payload.seat(Key::Player).is_some_and(|p| p != who)
}

fn is_sabotage(card: Option<Val>) -> bool {
    matches!(
        card,
        Some(Val::Action(ActionCard::Sabotage1 | ActionCard::Sabotage2 | ActionCard::Sabotage3 | ActionCard::Sabotage4))
    )
}

fn structure_of(who: Seat) -> impl Fn(&crate::state::Piece) -> bool {
    move |u| u.owner() == who && (u.kind() == Unit::SpaceDockI || u.kind() == Unit::PDSI)
}

impl Guard {
    /// Whether the window applies to `who` for this event.
    #[must_use]
    pub fn holds(self, e: &EventRec, who: Seat, s: &CoreState, k: &Kernel) -> bool {
        let p = &e.payload;
        match self {
            Self::Anyone => true,
            Self::ActorIs => actor_is(e, who),
            Self::ActorIsNot => actor_is_not(e, who),
            Self::Combatant => p.seat(Key::Attacker) == Some(who) || p.seat(Key::Defender) == Some(who),
            Self::CombatantFirstRound => {
                Self::Combatant.holds(e, who, s, k) && p.int(Key::Round).is_none_or(|round| round == 1)
            }
            Self::PartyToTransaction => p.seat(Key::Player) == Some(who) || p.seat(Key::Partner) == Some(who),
            Self::HoldsStrategyCards => {
                actor_is(e, who) && s.players().by_seat(who).is_some_and(|pl| !pl.strat.is_empty())
            }
            Self::AnotherSeatHasReadiedCard => {
                actor_is_not(e, who)
                    && s.meta()
                        .active
                        .and_then(|a| s.players().by_seat(a))
                        .is_some_and(|pl| pl.strat.iter().any(|c| !pl.strat_exhausted.contains(*c)))
            }
            Self::CommitOnYourPlanet => actor_is_not(e, who) && p.seat(Key::Controller) == Some(who),
            Self::DirectHit => {
                actor_is_not(e, who)
                    && p.seat(Key::Producer) == Some(who)
                    // Dreadnought II and its kin cannot be destroyed by Direct Hit.
                    && p.flag(Key::DirectHittable) != Some(false)
            }
            Self::OtherActivatesWithYourUnits => {
                actor_is_not(e, who)
                    && p.system(Key::System)
                        .is_some_and(|sys| s.board().sys(sys).units.as_slice().iter().any(|u| u.owner() == who))
            }
            Self::OtherActivatesWithYourCommandToken => {
                actor_is_not(e, who) && p.system(Key::System).is_some_and(|sys| s.board().sys(sys).cmd.contains(who))
            }
            Self::OtherActivatesWithYourStructures => {
                actor_is_not(e, who)
                    && p.system(Key::System)
                        .is_some_and(|sys| s.board().sys(sys).units.as_slice().iter().any(structure_of(who)))
            }
            Self::OtherPlaysCardNotSabotage => {
                actor_is_not(e, who)
                    && !is_sabotage(p.get(Key::Card))
                    // Last Bastion commander: "Your action cards cannot be canceled by 'Sabotage'".
                    && !p.seat(Key::Player).is_some_and(|actor| k.hooks.direct.bastion_commander_blocks_sabotage(s, actor))
            }
            Self::AnotherPlayerElected => p.seat(Key::ElectedPlayer).is_some_and(|elected| elected != who),
            Self::YourLastShip => actor_is(e, who) && p.flag(Key::Last) == Some(true),
            Self::YourShipDestroyedDuringSpaceCombat => {
                actor_is(e, who) && p.flag(Key::DuringSpaceCombat) == Some(true)
            }
            Self::VotedOrPredictedAnother => {
                let Some(outcome) = p.outcome(Key::Player) else { return false };
                let a = s.agenda();
                let voted_other = a.votes.get(&who).is_some_and(|v| *v != outcome);
                let predicted_other = a.predictions.get(&who).is_some_and(|v| match v {
                    Prediction::Plain(o) | Prediction::Tagged(o, _) => *o != outcome,
                    // `none|<bar>`: the text before the bar is not the outcome.
                    Prediction::Barred(_) => true,
                });
                voted_other || predicted_other
            }
            Self::TwoPlayersNotPassed => s.players().iter().filter(|pl| !pl.econ.passed).count() >= 2,
            Self::OtherGainsYourPlanet => actor_is_not(e, who) && p.seat(Key::PreviousOwner) == Some(who),
        }
    }
}

/// Whether the phase of a reaction window needs the speaker order (strategy and agenda): the window's
/// players are offered from the speaker in those phases.
#[must_use]
pub const fn window_uses_speaker_order(phase: Phase) -> bool {
    phase.uses_speaker_order()
}

/// Whether an `Outcome` value is a real seat (the guard for "another player is elected").
#[must_use]
pub const fn is_seat_outcome(o: Outcome) -> bool {
    matches!(o, Outcome::Seat(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Faction, Planet, System};
    use crate::state::{Payload, Player, Players, Tally};

    fn ev(kind: EventKind, p: Payload) -> EventRec {
        EventRec { id: 1, kind, payload: p, cancelled: false }
    }

    fn state() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s
    }

    #[test]
    fn every_non_action_timing_has_a_window_on_an_emitted_event() {
        for t in Timing::ALL {
            match window_of(t) {
                None => assert_eq!(t, Timing::Action),
                Some(spec) => assert!(EMITTED_EVENTS.contains(&spec.event), "{t:?} -> {:?}", spec.event),
            }
        }
        assert_eq!(Timing::ALL.iter().filter(|t| window_of(**t).is_some()).count(), Timing::COUNT - 1);
    }

    #[test]
    fn guards_divide_one_event_between_yours_and_anothers() {
        let s = state();
        let k = Kernel::plain();
        let e = ev(EventKind::SystemActivated, Payload::new().with(Key::Player, Val::Seat(Seat(0))));
        assert!(Guard::ActorIs.holds(&e, Seat(0), &s, &k) && !Guard::ActorIs.holds(&e, Seat(1), &s, &k));
        assert!(Guard::ActorIsNot.holds(&e, Seat(1), &s, &k) && !Guard::ActorIsNot.holds(&e, Seat(0), &s, &k));
        let first = ev(EventKind::CombatRoundStarted, Payload::new().with(Key::Attacker, Val::Seat(Seat(0))).with(Key::Round, Val::Int(2)));
        assert!(Guard::Combatant.holds(&first, Seat(0), &s, &k));
        assert!(!Guard::CombatantFirstRound.holds(&first, Seat(0), &s, &k), "round 2 is not a combat start");
    }

    #[test]
    fn sabotage_cannot_be_sabotaged_and_structures_count() {
        let mut s = state();
        let mut k = Kernel::plain();
        let play = |card| ev(EventKind::ActionCardPlayed, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Card, Val::Action(card)));
        assert!(Guard::OtherPlaysCardNotSabotage.holds(&play(ActionCard::ALL[0]), Seat(0), &s, &k));
        assert!(!Guard::OtherPlaysCardNotSabotage.holds(&play(ActionCard::Sabotage2), Seat(0), &s, &k));
        k.hooks.direct.bastion_commander_blocks_sabotage.push(|_, _| true);
        assert!(!Guard::OtherPlaysCardNotSabotage.holds(&play(ActionCard::ALL[0]), Seat(0), &s, &k));

        let sys = System::ALL[20];
        crate::kernel::fx::place_unit(&mut s, Seat(0), sys, None, Unit::PDSI);
        let act = ev(EventKind::SystemActivated, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::System, Val::System(sys)));
        assert!(Guard::OtherActivatesWithYourStructures.holds(&act, Seat(0), &s, &k));
        assert!(Guard::OtherActivatesWithYourUnits.holds(&act, Seat(0), &s, &k));
        assert!(!Guard::OtherActivatesWithYourCommandToken.holds(&act, Seat(0), &s, &k));
        let _ = (Planet::ALL[0], Tally { owner: Seat(0), unit: Unit::PDSI, count: 1 });
    }

    #[test]
    fn vote_and_prediction_guard_reads_the_agenda_slice() {
        let mut s = state();
        let k = Kernel::plain();
        let resolving = ev(EventKind::AgendaResolved, Payload::new().with(Key::Player, Val::Outcome(Outcome::For)));
        assert!(!Guard::VotedOrPredictedAnother.holds(&resolving, Seat(0), &s, &k));
        s.agenda_mut().votes.insert(Seat(0), Outcome::Against);
        assert!(Guard::VotedOrPredictedAnother.holds(&resolving, Seat(0), &s, &k));
        s.agenda_mut().votes.insert(Seat(0), Outcome::For);
        s.agenda_mut().predictions.insert(Seat(0), Prediction::Plain(Outcome::Against));
        assert!(Guard::VotedOrPredictedAnother.holds(&resolving, Seat(0), &s, &k));
    }

    #[test]
    fn crisis_counts_players_who_have_not_passed() {
        let mut s = state();
        let k = Kernel::plain();
        let e = ev(EventKind::TurnPassed, Payload::new());
        assert!(Guard::TwoPlayersNotPassed.holds(&e, Seat(0), &s, &k));
        s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.passed = true;
        assert!(!Guard::TwoPlayersNotPassed.holds(&e, Seat(0), &s, &k));
    }
}
