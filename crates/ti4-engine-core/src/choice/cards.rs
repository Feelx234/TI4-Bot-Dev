//! Decision types of the `cards` area: action cards.
//!
//! Owner: package B6 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the own-hand question
//! (`action_cards::choose_from_own_hand`): one option per distinct printed card, then optionally decline.
//! The adapter side is `ti4-engine/src/render/cards.rs`. Never text; every type is `Copy`.

use crate::generated::{ActionCard, Planet, System};
use crate::state::NoteKey;
use crate::ids::Seat;
use crate::keys::Outcome;

use super::agenda::Frame;

/// What a card's pick question is about (`action_cards::pick`'s `kind`): it decides the option kind and the
/// subtype tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum What {
    /// kind `player`: a seat.
    Player,
    /// kind `elect`: a seat, Confusing Legal Text's redirect.
    Elect,
    /// kind `recall`: a system holding the asked seat's command token.
    Recall,
    /// kind `system`: a system to bombard (Tactical Bombardment).
    Bombard,
    /// kind `system`: a system a cruiser is placed in (War Effort).
    Deploy,
    /// kind `silence`: a system holding the asked seat's ships (In The Silence Of Space).
    Silence,
    /// kind `promissory note`: a note of the asked seat's hand (Diplomatic Pressure).
    Note,
    /// kind `planet`: a `system|planet` spot.
    Planet,
}

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
    /// `action_cards::pick`: the played `card` asks the actor to pick one of several (the string side takes the only
    /// option without asking).
    Pick { card: ActionCard, what: What },
    /// `action_cards::predicted_outcome`: a Rider (or Sanction) names the outcome it predicts on the agenda in `frame`.
    Predict { card: ActionCard, frame: Frame },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// The first copy in hand of one printed card.
    Card(ActionCard),
    /// A seat (`What::Player`, `What::Elect`).
    Seat(Seat),
    /// A system (`What::Recall`).
    System(System),
    /// A planet and the system the string board files it under (`What::Planet`).
    Spot(System, Planet),
    /// A predicted outcome.
    Outcome(Outcome),
    /// A promissory note (`What::Note`).
    Note(NoteKey),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    StallTacticsDiscard,
    DeepgloomStallTacticsDiscard,
    MalleonDiscard,
    C4rDiscard,
    ShowActionCard,
    /// `<card alias>_pick_<kind>`, or `<card alias>_elect` for `What::Elect`.
    Pick { card: ActionCard, what: What },
    /// `predict_agenda_outcome`.
    PredictOutcome,
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
