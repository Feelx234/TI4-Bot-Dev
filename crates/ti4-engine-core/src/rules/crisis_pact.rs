//! Galactic Crisis Pact (rules version 31; `agenda_effects::effect_crisis`, lane E2): "Each player may perform the secondary ability of the elected
//! strategy card without spending a command token." One window per player, clockwise from the speaker: the question is the ordinary secondary
//! question of the card without the token (`strategy::crisis_pact_choice`), asked only of a seat that could do something with the secondary. The
//! ability itself (a child window of the card) is the driver's (`driver::agenda::crisis`), as it is in the follower window.

use crate::choice::generic::Rule;
use crate::choice::strategy::{ctx, rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::generated::{StrategyCard, StrategyCardKind};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::CoreState;

/// Where the window has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStage {
    Start,
    Asking,
    Done,
}

/// The window of one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrisisPact {
    pub who: Seat,
    pub card: StrategyCard,
    /// Whether the seat could do something with the secondary (`strategy::secondary_can_do_something`, Leadership: three influence): the driver's fact.
    eligible: bool,
    stage: CStage,
    /// The player chose to perform the secondary.
    pub accepted: bool,
}

impl CrisisPact {
    #[must_use]
    pub const fn new(who: Seat, card: StrategyCard, eligible: bool) -> Self {
        Self { who, card, eligible, stage: CStage::Start, accepted: false }
    }

    const fn ask(&self) -> Ask {
        Ask::Strategy(SAsk::CrisisPact { card: self.card })
    }
}

impl Window for CrisisPact {
    type Stage = CStage;

    fn stage(&self) -> CStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        if self.stage == CStage::Start {
            self.stage = if self.eligible { CStage::Asking } else { CStage::Done };
        }
        if self.stage == CStage::Asking { Next::Continue(self.ask()) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        // The ordinary secondary question without a token: Leadership's is the purchase question itself (52.3), the others carry the card's context.
        let leadership = self.card.data().kind == StrategyCardKind::Leadership;
        (self.stage == CStage::Asking).then(|| Choice {
            actor: self.who,
            ask: self.ask(),
            ctx: if leadership { rule_ctx(Rule::R52_3, SSub::BuyTokenWithInfluence) } else { ctx(self.card, true, SSub::FollowSecondary) },
            opts: [Opt::Strategy(SOpt::Skip), Opt::Strategy(SOpt::Follow)].into_iter().collect::<Opts>(),
        })
    }

    fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (CStage::Asking, Pick::Opt(Opt::Strategy(SOpt::Skip))) => {
                self.stage = CStage::Done;
                Ok(())
            }
            (CStage::Asking, Pick::Opt(Opt::Strategy(SOpt::Follow))) => {
                self.accepted = true;
                self.stage = CStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
