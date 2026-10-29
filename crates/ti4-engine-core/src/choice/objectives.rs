//! Decision types of the `objectives` area: scoring and the secret objective hand limit.
//!
//! Owner: package A4 (docs/ENGINE_CORE_STATE.md 4.4). Ported: "score an objective" (rule 61.6, the status
//! window and the action- and agenda-timed secret windows) and "return a secret objective to the deck" (rule
//! 45.4). The adapter side is `ti4-engine/src/render/objectives.rs`.

use crate::generated::SecretObjective;
use crate::ids::Seat;
use crate::state::{ObjectiveRef, PlotCard};

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "Score an objective": the seat's victory points now (an option previews one more).
    Score { vp: u8 },
    /// "Return a secret objective to the deck": secrets held plus scored (an option previews one less).
    ReturnSecret { held: u8 },
    /// Plots Within Plots, second step: "place a plot card with `token`'s control token on it".
    PlotCard { token: Seat },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Score this objective (a public one, or a secret: in hand, or made public by Classified Document Leaks).
    Score(ObjectiveRef),
    /// Return this secret objective.
    Return(SecretObjective),
    /// Plots Within Plots: score `secret`, which `token` scored, as a plot (no victory point).
    Plot { secret: SecretObjective, token: Seat },
    /// Place this plot card.
    PlaceCard(PlotCard),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// The status window offers publics and secrets in one list.
    ScoreObjective,
    /// An action- or agenda-timed window offers only the secret its event satisfied.
    ScoreSecretObjective,
    ReturnOverSecretHandLimit,
    PlotCard,
}

fn ctx(rule: super::generic::Rule, subtype: Subtype) -> super::Ctx {
    ctx_of(super::Origin::Rule(rule), subtype)
}

fn ctx_of(origin: super::Origin, subtype: Subtype) -> super::Ctx {
    super::Ctx {
        origin,
        subtype: super::Subtype::Objectives(subtype),
        optional: false,
        target: None,
        outstanding: crate::containers::SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

/// One thing a scoring question offers: an objective, or (Plots Within Plots) another player's scored secret
/// scored as a plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Offer {
    Objective(ObjectiveRef),
    Plot { secret: SecretObjective, token: Seat },
}

impl Offer {
    #[must_use]
    pub const fn opt(self) -> Opt {
        match self {
            Self::Objective(o) => Opt::Score(o),
            Self::Plot { secret, token } => Opt::Plot { secret, token },
        }
    }
}

/// "Score an objective" (rule 61.6) over `offers`, then the decline.
#[must_use]
pub fn score_choice(who: crate::ids::Seat, vp: u8, status: bool, offers: &[Offer]) -> super::Choice {
    let mut opts: super::Opts = offers.iter().map(|o| super::Opt::Objectives(o.opt())).collect();
    opts.push(super::Opt::Decline);
    super::Choice {
        actor: who,
        ask: super::Ask::Objectives(Ask::Score { vp }),
        ctx: ctx(super::generic::Rule::R61_6, if status { Subtype::ScoreObjective } else { Subtype::ScoreSecretObjective }),
        opts,
    }
}

/// "Return a secret objective to the deck" (rule 45.4) over the hand. The windows that draw secrets share it
/// (areas do not import each other).
#[must_use]
pub fn return_secret_choice(who: crate::ids::Seat, held: u8, hand: &[SecretObjective]) -> super::Choice {
    super::Choice {
        actor: who,
        ask: super::Ask::Objectives(Ask::ReturnSecret { held }),
        ctx: ctx(super::generic::Rule::R45_4, Subtype::ReturnOverSecretHandLimit),
        opts: hand.iter().map(|x| super::Opt::Objectives(Opt::Return(*x))).collect(),
    }
}

/// Plots Within Plots, second step: the Firmament chooses which of the `remaining` plot cards goes into play
/// with `token`'s control token on it. `source` is the interned content text of the faction ability.
#[must_use]
pub fn plot_card_choice(who: Seat, token: Seat, source: crate::state::Code, remaining: &[PlotCard]) -> super::Choice {
    super::Choice {
        actor: who,
        ask: super::Ask::Objectives(Ask::PlotCard { token }),
        ctx: ctx_of(super::Origin::FactionAbility(super::generic::Content::Other(source)), Subtype::PlotCard),
        opts: remaining.iter().map(|c| super::Opt::Objectives(Opt::PlaceCard(*c))).collect(),
    }
}
