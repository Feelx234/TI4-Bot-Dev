//! The objective questions (package A4; string `objectives::ScoringWindow`, `secrets::enforce_hand_limit`), as
//! [`Window`]s: "score an objective" (rule 61.6) and "return a secret objective to the deck" (rule 45.4).
//!
//! [`ScoreOne`] is the question of one player in a scoring window and the state half of the answer. Which
//! objectives are on offer is the read half's answer (`query::objectives`, loaded by the adapter through the
//! typed objective view) and is given to the window; the order the window walks the table in, and who may still
//! score what in it, stay with the string `ScoringWindow` (see `docs/ENGINE_KNOWN_ISSUES.md`, A4).
//! [`HandLimit`] reads everything it needs from the state.

use crate::choice::objectives::{Ask as OAsk, Offer, Opt as OOpt, plot_card_choice, return_secret_choice, score_choice};
use crate::choice::{Ask, Choice, Opt, Pick};
use crate::containers::SmallList;
use crate::generated::{PublicObjective, SecretObjective};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::fx_pay::{self, PayEnv};
use crate::kernel::fx_plots;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::objectives as q;
use crate::state::{Code, CoreState, ObjectiveRef, PlotCard};

/// The seat's points as the question shows them.
fn points(core: &CoreState, who: Seat) -> u8 {
    core.players().by_seat(who).map_or(0, |p| u8::try_from(p.econ.victory_points.max(0)).unwrap_or(u8::MAX))
}

// -- score an objective -----------------------------------------------------------------------------

/// The suspension points of [`ScoreOne`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SStage {
    Start,
    Asking,
    Done,
}

/// "Score an objective": one player's turn in a scoring window, over the objectives on offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreOne {
    pub who: Seat,
    /// The status window (publics and secrets in one list) or an event-timed one (secrets only).
    pub status: bool,
    offers: SmallList<Offer, 24>,
    /// The feat occurrence the player's one score is recorded under (an event window with the one-per-player cap).
    occurrence: Option<u64>,
    /// What the price reads besides the state and the kernel. Without it a priced offer cannot be taken (a
    /// bought objective is never free).
    price: Option<PayEnv>,
    stage: SStage,
    /// What was scored; `None` for a decline (and for a plot, which is no scored objective).
    pub scored: Option<ObjectiveRef>,
    /// Plots Within Plots: the secret scored as a plot, the player whose token goes on the card, and whether the
    /// card was placed at once (one card was left) or the Firmament is asked next ([`PlaceCard`]).
    pub plotted: Option<(SecretObjective, Seat, bool)>,
}

impl ScoreOne {
    #[must_use]
    pub fn new(who: Seat, status: bool, offers: &[Offer], occurrence: Option<u64>) -> Self {
        Self { who, status, offers: offers.iter().copied().collect(), occurrence, price: None, stage: SStage::Start, scored: None, plotted: None }
    }

    /// The same window able to charge the price of a bought objective (`objectives::pay_for`) and of Destroy
    /// Heretical Works / Form a Spy Network (`secrets::pay_for`).
    #[must_use]
    pub const fn with_price(mut self, env: PayEnv) -> Self {
        self.price = Some(env);
        self
    }

    /// `objectives::award`: a bought objective is paid on taking. `false` when it cannot be paid.
    fn pay_public(&self, sx: &mut Sx<'_>, obj: PublicObjective) -> bool {
        match q::cost_of(obj) {
            None => true,
            Some(cost) => self.price.is_some_and(|env| fx_pay::pay_cost(sx.state, sx.kernel, env, self.who, cost)),
        }
    }
}

impl ScoreOne {
    /// `secrets::pay_for`: the two priced secrets need the environment of a price, every other secret is free.
    fn priced_ok(&self, sx: &mut Sx<'_>, secret: SecretObjective) -> bool {
        if !matches!(secret, SecretObjective::DestroyHereticalWorks | SecretObjective::FormASpyNetwork) {
            return true;
        }
        self.price.is_some() && fx_pay::pay_secret(sx.state, self.who, secret)
    }
}

impl Window for ScoreOne {
    type Stage = SStage;

    fn stage(&self) -> SStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == SStage::Start {
            self.stage = SStage::Asking;
        }
        match self.stage {
            SStage::Asking => Next::Continue(Ask::Objectives(OAsk::Score { vp: points(sx.state, self.who) })),
            SStage::Start | SStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != SStage::Asking {
            return None;
        }
        Some(score_choice(self.who, points(s, self.who), self.status, self.offers.as_slice()))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != SStage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Objectives(OOpt::Plot { secret, token })) if self.offers.as_slice().contains(&Offer::Plot { secret, token }) => {
                // `ScoringWindow::resolve`: no victory point, no scored objective, neither cap used. One card
                // left is placed now, several: the Firmament is asked next.
                if !fx_plots::score_as_plot(sx.state, self.who, secret, token) {
                    return Err(Illegal);
                }
                let placed = match fx_plots::remaining_cards(sx.state).as_slice() {
                    [only] => fx_plots::place_card(sx.state, self.who, *only, token),
                    _ => false,
                };
                self.plotted = Some((secret, token, placed));
            }
            Pick::Opt(Opt::Objectives(OOpt::Score(obj))) if self.offers.as_slice().contains(&Offer::Objective(obj)) => {
                let public_now = sx.state.objectives().revealed.as_slice().contains(&obj);
                match obj {
                    ObjectiveRef::Secret(s) if !public_now => {
                        // `secrets::award`: a window that offers a secret not in the hand is stale; the price is
                        // paid before the score, and a secret whose price cannot be met is not scored.
                        if !hand_holds(sx.state, self.who, s) || !(self.priced_ok(sx, s)) {
                            return Err(Illegal);
                        }
                        if !fx::score_secret(sx.state, self.who, s) {
                            return Err(Illegal);
                        }
                    }
                    _ => {
                        if let ObjectiveRef::Public(p) = obj
                            && !self.pay_public(sx, p)
                        {
                            return Err(Illegal);
                        }
                        fx::score_public(sx.state, self.who, obj);
                    }
                }
                fx::finish_if_won(sx.state);
                if let Some(n) = self.occurrence {
                    fx::record_occurrence_score(sx.state, self.who, n);
                }
                self.scored = Some(obj);
            }
            _ => return Err(Illegal),
        }
        self.stage = SStage::Done;
        Ok(())
    }
}

// -- Plots Within Plots: the plot card -------------------------------------------------------------

/// The suspension points of [`PlaceCard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Asking,
    Done,
}

/// Plots Within Plots, second step: the Firmament chooses which plot card goes into play with `token`'s
/// control token on it. No victory point and no scored objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceCard {
    pub who: Seat,
    pub token: Seat,
    /// The interned text of the faction ability the question is attributed to.
    source: Code,
    stage: PStage,
    pub placed: Option<PlotCard>,
}

impl PlaceCard {
    #[must_use]
    pub const fn new(who: Seat, token: Seat, source: Code) -> Self {
        Self { who, token, source, stage: PStage::Asking, placed: None }
    }
}

impl Window for PlaceCard {
    type Stage = PStage;

    fn stage(&self) -> PStage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        match self.stage {
            PStage::Asking => Next::Continue(Ask::Objectives(OAsk::PlotCard { token: self.token })),
            PStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == PStage::Asking).then(|| plot_card_choice(self.who, self.token, self.source, fx_plots::remaining_cards(s).as_slice()))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (PStage::Asking, Pick::Opt(Opt::Objectives(OOpt::PlaceCard(card)))) if fx_plots::place_card(sx.state, self.who, card, self.token) => {
                self.placed = Some(card);
                self.stage = PStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- the secret objective hand limit ----------------------------------------------------------------

/// The suspension points of [`HandLimit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HStage {
    Check,
    Asking,
    Done,
}

/// `secrets::enforce_hand_limit`: over the limit, an unscored secret goes back to the bottom of the deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandLimit {
    pub who: Seat,
    stage: HStage,
}

impl HandLimit {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: HStage::Check }
    }
}

impl Window for HandLimit {
    type Stage = HStage;

    fn stage(&self) -> HStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                HStage::Check | HStage::Asking => match q::hand_step(sx.state, self.who) {
                    q::HandStep::Within => self.stage = HStage::Done,
                    q::HandStep::Return(only) => fx::return_secret(sx.state, self.who, only),
                    q::HandStep::Ask { held, .. } => {
                        self.stage = HStage::Asking;
                        return Next::Continue(Ask::Objectives(OAsk::ReturnSecret { held }));
                    }
                },
                HStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != HStage::Asking {
            return None;
        }
        match q::hand_step(s, self.who) {
            q::HandStep::Ask { held, hand } => Some(return_secret_choice(self.who, held, hand.as_slice())),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (HStage::Asking, Pick::Opt(Opt::Objectives(OOpt::Return(s)))) if hand_holds(sx.state, self.who, s) => {
                fx::return_secret(sx.state, self.who, s);
                self.stage = HStage::Check;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

fn hand_holds(core: &CoreState, who: Seat, secret: SecretObjective) -> bool {
    core.players().by_seat(who).is_some_and(|p| p.cards.secrets.as_slice().contains(&secret))
}
