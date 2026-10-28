//! Action cards (package B6; LRR 2, 22): the reads of `action_cards::{hand_limit, is_component_action,
//! is_playable}` that the core can answer from its own state. This is slice 1 of the package: reads only.
//! What needs another area's string rules (payment, research, the unit catalogue, neighbours) is not
//! asked here and the string function stands; each such place is a documented bail (see
//! [`has_legal_target`]). No `Env` trait: the faction answers are plain arguments.

use crate::generated::ActionCard;
use crate::ids::Seat;
use crate::query::laws;
use crate::state::CoreState;

/// `action_cards::HAND_LIMIT`: the printed limit (LRR 2.4).
pub const HAND_LIMIT: usize = 7;

/// `action_cards::is_component_action`: the printed window is "Action".
#[must_use]
pub fn is_component_action(card: ActionCard) -> bool {
    card.data().component_action
}

/// `action_cards::hand_limit`: the law cap of Sanctions, then the faction override (`faction`, the
/// string module's hook, applied last as it overrides the law).
#[must_use]
pub fn hand_limit(core: &CoreState, blanked: bool, faction: impl FnOnce(usize) -> usize) -> usize {
    faction(laws::action_card_limit(core, blanked, HAND_LIMIT))
}

/// The seat-independent gates at the head of `action_cards::is_playable`: Political Censure's owner, the
/// faction bar and Political Secret's target cannot play action cards. `false` bars the play.
#[must_use]
pub fn may_play(core: &CoreState, blanked: bool, who: Seat, faction_forbids: bool, political_secret_bars: bool) -> bool {
    !(laws::censure_bars(core, blanked, who) || faction_forbids || political_secret_bars)
}

/// `action_cards::has_legal_target` for the cards whose target list is a plain read of the core:
/// `Some(answer)`, or `None` for the arms that need string-only rules (the bail: the string function
/// answers). A card without a requirement answers `Some(true)`.
///
/// Bails (`None`): Meltdown, Cripple Defenses, Lucky Shot, Plague, Uprising, Seize Artifact, Messiah and
/// Frontline Deployment, War Effort, Tactical Bombardment, Unstable Planet, Scuttle (planet catalogue,
/// unit catalogue, neighbours or the controlled-spot rules).
#[must_use]
pub fn has_legal_target(core: &CoreState, blanked: bool, who: Seat, card: ActionCard) -> Option<bool> {
    let rival = |keep: &dyn Fn(&crate::state::Player) -> bool| core.players().iter().any(|p| p.seat != who && keep(p));
    use ActionCard as C;
    match card {
        C::Insubordination => Some(rival(&|p| p.econ.tactic_tokens > 0)),
        C::Spy => Some(rival(&|p| !p.cards.hand.is_empty())),
        C::RepealLaw => Some(!laws::in_play(core, blanked).is_empty()),
        C::UnexpectedAction => Some(!crate::query::board::systems_with_token(core, who).is_empty()),
        C::ReactorMeltdown
        | C::CrippleDefenses
        | C::LuckyShot
        | C::Plague
        | C::Uprising
        | C::SeizeArtifact
        | C::RiseOfAMessiah
        | C::FrontlineDeployment
        | C::WarEffort
        | C::TacticalBombardment
        | C::UnstablePlanet
        | C::Scuttle => None,
        _ => Some(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::Idx;

    #[test]
    fn component_action_is_the_printed_window() {
        assert!(is_component_action(ActionCard::Spy));
        assert!(!is_component_action(ActionCard::Blitz));
    }

    /// One test for every documented bail: the core declines, the string function answers.
    #[test]
    fn every_bail_declines_and_the_rest_answers() {
        let core = CoreState::default();
        let who = Seat::from_index(0).unwrap();
        for card in [
            ActionCard::ReactorMeltdown,
            ActionCard::CrippleDefenses,
            ActionCard::LuckyShot,
            ActionCard::Plague,
            ActionCard::Uprising,
            ActionCard::SeizeArtifact,
            ActionCard::RiseOfAMessiah,
            ActionCard::FrontlineDeployment,
            ActionCard::WarEffort,
            ActionCard::TacticalBombardment,
            ActionCard::UnstablePlanet,
            ActionCard::Scuttle,
        ] {
            assert_eq!(has_legal_target(&core, false, who, card), None, "{card:?}");
        }
        assert_eq!(has_legal_target(&core, false, who, ActionCard::Blitz), Some(true));
        assert_eq!(has_legal_target(&core, false, who, ActionCard::Spy), Some(false), "no rival");
    }
}
