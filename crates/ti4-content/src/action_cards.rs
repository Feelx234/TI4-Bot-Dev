//! When an action card is played, in groups.
//!
//! The corpus prints the timing of a card as free text (`window`), with some fifty different
//! texts. A reader of the whole deck wants them in a few groups, and no field of the corpus has
//! one. The groups are written by hand here, beside the corpus and not in it: the files of
//! `content/` are copies of the oracle.
//!
//! The key is the printed window (its [`ActionCardWindow`]) and not the card, so the copies of a
//! card and the cards of one timing cannot disagree. A window that is not listed has no group; the
//! test of this module fails for it.

use crate::card_fields::ActionCardWindow;

/// The group of the timing of an action card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionCardTiming {
    /// "ACTION": the card is a component action.
    Action,
    /// After the player of the card activates a system.
    OwnActivation,
    /// After another player activates a system or moves ships into it.
    OtherActivation,
    /// A combat, and the rolls of the unit abilities around it.
    Combat,
    /// An invasion: the landing, the ground combat and the control of a planet.
    Invasion,
    /// The turn or the action of a player, outside of a tactical action.
    Turn,
    StrategyPhase,
    StatusPhase,
    AgendaPhase,
    /// In every phase.
    AnyTime,
}

impl ActionCardTiming {
    /// Every group, in the order in which a reader is shown them.
    pub const ALL: [Self; 10] = [
        Self::Action,
        Self::OwnActivation,
        Self::OtherActivation,
        Self::Combat,
        Self::Invasion,
        Self::Turn,
        Self::StrategyPhase,
        Self::StatusPhase,
        Self::AgendaPhase,
        Self::AnyTime,
    ];

    /// The name of the group for a reader.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Action => "Action",
            Self::OwnActivation => "Your activation",
            Self::OtherActivation => "Activation of another player",
            Self::Combat => "Combat",
            Self::Invasion => "Invasion",
            Self::Turn => "Turns and actions",
            Self::StrategyPhase => "Strategy phase",
            Self::StatusPhase => "Status phase",
            Self::AgendaPhase => "Agenda phase",
            Self::AnyTime => "Any time",
        }
    }
}

/// The group of a card with this `phase` and `window`, as the corpus prints them. `None` for a
/// window that this module does not know.
#[must_use]
pub fn timing_of(phase: &str, window: &str) -> Option<ActionCardTiming> {
    use ActionCardTiming as T;
    use ActionCardWindow as W;
    match phase {
        "Agenda" => return Some(T::AgendaPhase),
        "Strategy" => return Some(T::StrategyPhase),
        "Status" => return Some(T::StatusPhase),
        "Any" => return Some(T::AnyTime),
        _ => {}
    }
    let Ok(window) = ActionCardWindow::parse(window) else {
        return None;
    };
    Some(match window {
        W::Action => T::Action,

        W::AfterYouActivateSystem
        | W::AfterYouActivateSystemWithYourShips
        | W::AfterYouActivateSystemWithOtherShips
        | W::AfterYouActivateAnomaly => T::OwnActivation,

        W::AfterOtherActivatesSystemWithYourCommandToken
        | W::AfterOtherActivatesSystemWithYourStructures
        | W::AfterOtherActivatesSystemWithYourUnits
        | W::AfterShipsMoveIntoSystemWithYourShips
        | W::AfterActivePlayerMovesIntoActiveSystem => T::OtherActivation,

        W::StartOfCombat
        | W::StartOfCombatRound
        | W::StartOrEndOfCombatRound
        | W::StartOfFirstSpaceCombatRound
        | W::StartOfAnnounceRetreatsAsDefender
        | W::AfterOpponentDeclaresRetreat
        | W::BeforeAssigningSpaceCombatHits
        | W::WhenYourShipSustainsDamage
        | W::AfterOtherSustainsAgainstYourHit
        | W::AfterOwnShipDestroyedInSpaceCombat
        | W::WhenYourLastShipIsDestroyed
        | W::AfterYouWinSpaceCombat
        | W::BeforeAssigningSpaceCannonHits
        | W::BeforeAntiFighterBarrageRoll
        | W::AfterOtherRollsUnitAbility => T::Combat,

        W::StartOfInvasion
        | W::StartOfInvasionAgainstPds
        | W::AfterOtherCommitsToYourPlanet
        | W::AfterYourGroundForcesRoll
        | W::WhenYouGainPlanet
        | W::AfterOtherGainsYourPlanet => T::Invasion,

        W::AfterYouPerformAction
        | W::WhenOtherPerformsStrategicAction
        | W::WhenYourUnitsUseProduction
        | W::StartOfOtherTurnWithReadiedStrategyCard
        | W::EndOfTurnIfYouPassed
        | W::EndOfTurnIfTwoNotPassed => T::Turn,

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ti4_model::content_types::{ContentType, POK};

    use super::*;
    use crate::loader::ContentStore;

    #[test]
    fn every_action_card_of_the_corpus_has_a_group() {
        for record in ContentStore::embedded().records(ContentType::ActionCards) {
            let phase = record.text("phase").unwrap_or_default();
            let window = record.text("window").unwrap_or_default();
            assert!(
                timing_of(phase, window).is_some(),
                "no group for the window {window:?} ({phase})"
            );
        }
    }

    #[test]
    fn the_groups_of_a_game_have_the_cards_that_a_reader_expects() {
        let mut names: BTreeMap<ActionCardTiming, BTreeSet<&str>> = BTreeMap::new();
        for record in ContentStore::embedded().records(ContentType::ActionCards) {
            if !record.in_sources(POK) {
                continue;
            }
            let timing = timing_of(
                record.text("phase").unwrap_or_default(),
                record.text("window").unwrap_or_default(),
            )
            .expect("a group");
            names
                .entry(timing)
                .or_default()
                .insert(record.text("name").unwrap_or_default());
        }
        let count = |timing| names.get(&timing).map_or(0, BTreeSet::len);

        assert_eq!(count(ActionCardTiming::Action), 29);
        assert_eq!(count(ActionCardTiming::OwnActivation), 8);
        assert_eq!(count(ActionCardTiming::OtherActivation), 4);
        assert_eq!(count(ActionCardTiming::Combat), 15);
        assert_eq!(count(ActionCardTiming::Invasion), 8);
        assert_eq!(count(ActionCardTiming::AgendaPhase), 20);
        assert!(names[&ActionCardTiming::AnyTime].contains("Sabotage"));
        assert!(names[&ActionCardTiming::Combat].contains("Morale Boost"));
    }

    #[test]
    fn the_labels_are_all_different() {
        let labels: BTreeSet<_> = ActionCardTiming::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels.len(), ActionCardTiming::ALL.len());
    }
}
