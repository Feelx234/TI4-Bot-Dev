//! Decision types of the `cards` area: action cards.
//!
//! Owner: package B6 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the own-hand question
//! (`action_cards::choose_from_own_hand`): one option per distinct printed card, then optionally decline.
//! The adapter side is `ti4-engine/src/render/cards.rs`. Never text; every type is `Copy`.

use crate::generated::ActionCard;
use crate::ids::Seat;

/// Why a seat is asked to pick a card from its own hand; it decides the prompt and the subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    /// Yssaril Stall Tactics.
    StallTactics,
    /// Stall Tactics allowed by Deepgloom Executable.
    DeepgloomStallTactics,
    /// Nekro Malleon's discard.
    MalleonDiscard,
    /// The C4R discard (`ERROR_ERROR`).
    C4rDiscard,
    /// Shown to `viewer` (`show_action_card`).
    Show { viewer: Seat },
}

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Pick one action card from the asked seat's own hand.
    OwnHand { reason: Reason },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// The first copy in hand of one printed card.
    Card(ActionCard),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    StallTacticsDiscard,
    DeepgloomStallTacticsDiscard,
    MalleonDiscard,
    C4rDiscard,
    ShowActionCard,
}

impl Reason {
    /// The subtype the reason names.
    #[must_use]
    pub const fn subtype(self) -> Subtype {
        match self {
            Self::StallTactics => Subtype::StallTacticsDiscard,
            Self::DeepgloomStallTactics => Subtype::DeepgloomStallTacticsDiscard,
            Self::MalleonDiscard => Subtype::MalleonDiscard,
            Self::C4rDiscard => Subtype::C4rDiscard,
            Self::Show { .. } => Subtype::ShowActionCard,
        }
    }
}
