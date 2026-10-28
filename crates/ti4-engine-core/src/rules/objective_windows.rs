//! The objective questions (package A4; string `objectives::ScoringWindow`, `secrets::enforce_hand_limit`), as
//! [`Window`]s: "score an objective" (rule 61.6) and "return a secret objective to the deck" (rule 45.4).
//!
//! [`ScoreOne`] is the question of one player in a scoring window and the state half of the answer. Which
//! objectives are on offer is the read half's answer (`query::objectives`, loaded by the adapter through the
//! typed objective view) and is given to the window; the order the window walks the table in, and who may still
//! score what in it, stay with the string `ScoringWindow` (see `docs/ENGINE_KNOWN_ISSUES.md`, A4).
//! [`HandLimit`] reads everything it needs from the state.

use crate::choice::generic::Rule;
use crate::choice::objectives::{Ask as OAsk, Opt as OOpt, Subtype as OSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::SecretObjective;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::relic;
use crate::state::{CoreState, ObjectiveRef};

/// The seat's points as the question shows them.
fn points(core: &CoreState, who: Seat) -> u8 {
    core.players().by_seat(who).map_or(0, |p| u8::try_from(p.econ.victory_points.max(0)).unwrap_or(u8::MAX))
}

fn ctx(rule: Rule, subtype: OSub) -> Ctx {
    Ctx {
        origin: Origin::Rule(rule),
        subtype: Subtype::Objectives(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
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
    offers: SmallList<ObjectiveRef, 24>,
    /// The feat occurrence the player's one score is recorded under (an event window with the one-per-player cap).
    occurrence: Option<u64>,
    stage: SStage,
    /// What was scored; `None` for a decline.
    pub scored: Option<ObjectiveRef>,
}

impl ScoreOne {
    #[must_use]
    pub fn new(who: Seat, status: bool, offers: &[ObjectiveRef], occurrence: Option<u64>) -> Self {
        Self { who, status, offers: offers.iter().copied().collect(), occurrence, stage: SStage::Start, scored: None }
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
        let subtype = if self.status { OSub::ScoreObjective } else { OSub::ScoreSecretObjective };
        let mut opts: Opts = self.offers.iter().map(|o| Opt::Objectives(OOpt::Score(*o))).collect();
        opts.push(Opt::Decline);
        Some(Choice {
            actor: self.who,
            ask: Ask::Objectives(OAsk::Score { vp: points(s, self.who) }),
            ctx: ctx(Rule::R61_6, subtype),
            opts,
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != SStage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Objectives(OOpt::Score(obj))) if self.offers.as_slice().contains(&obj) => {
                let public_now = sx.state.objectives().revealed.as_slice().contains(&obj);
                match obj {
                    ObjectiveRef::Secret(s) if !public_now => {
                        // `secrets::award`: a window that offers a secret not in the hand is stale.
                        if !fx::score_secret(sx.state, self.who, s) {
                            return Err(Illegal);
                        }
                    }
                    _ => {
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

// -- the secret objective hand limit ----------------------------------------------------------------

/// `secrets::HAND_LIMIT` (45.4): three, scored ones counting.
pub const HAND_LIMIT: usize = 3;

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

    /// `secrets::held_count`: secrets in hand plus the ones already scored.
    fn held(core: &CoreState, who: Seat) -> usize {
        let hand = core.players().by_seat(who).map_or(0, |p| p.cards.secrets.len());
        hand + core.objectives().scored[usize::from(who.0)].secret.len()
    }

    fn hand(core: &CoreState, who: Seat) -> SmallList<SecretObjective, 4> {
        core.players().by_seat(who).map_or_else(SmallList::new, |p| p.cards.secrets.clone())
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
                HStage::Check => {
                    let limit = HAND_LIMIT + relic::secret_objective_bonus(sx.state, self.who);
                    let hand = Self::hand(sx.state, self.who);
                    if Self::held(sx.state, self.who) <= limit || hand.is_empty() {
                        // Every one scored: nothing to hand back.
                        self.stage = HStage::Done;
                    } else if let [only] = hand.as_slice() {
                        fx::return_secret(sx.state, self.who, *only);
                    } else {
                        self.stage = HStage::Asking;
                    }
                }
                HStage::Asking => {
                    let held = u8::try_from(Self::held(sx.state, self.who)).unwrap_or(u8::MAX);
                    return Next::Continue(Ask::Objectives(OAsk::ReturnSecret { held }));
                }
                HStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != HStage::Asking {
            return None;
        }
        let held = u8::try_from(Self::held(s, self.who)).unwrap_or(u8::MAX);
        Some(Choice {
            actor: self.who,
            ask: Ask::Objectives(OAsk::ReturnSecret { held }),
            ctx: ctx(Rule::R45_4, OSub::ReturnOverSecretHandLimit),
            opts: Self::hand(s, self.who).iter().map(|x| Opt::Objectives(OOpt::Return(*x))).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (HStage::Asking, Pick::Opt(Opt::Objectives(OOpt::Return(s)))) if Self::hand(sx.state, self.who).as_slice().contains(&s) => {
                fx::return_secret(sx.state, self.who, s);
                self.stage = HStage::Check;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
