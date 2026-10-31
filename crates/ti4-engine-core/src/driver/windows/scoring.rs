//! The scoring window of a tactical event as ONE window the core drives and holds (lane M5; the string `Game::{open_occurrence_event_scoring,
//! step_event_scoring, score_step}` over `objectives::ScoringWindow`): after a space cannon offense, a space combat or an invasion step that
//! recorded a feat, every seat in initiative order may score the secret objectives that feat unlocks. L5: it walks the table with the
//! [`ScoringBook`] over the offers the position gives ([`CoreOffers`], the core's `secrets::scoreable_event`), asks one seat at a time with a
//! [`ScoreOne`] child, and keeps the book as the typed payload [`crate::state::Scoring`] on the `Pending` stack.
//!
//! The plain events of the string's `step_event_scoring` are written here: `OBJECTIVE_SCORED:{objective}` after a score and `GAME_FINISHED` when
//! the score ended the game (the window closes then). What the core does not do (the leaders a PUBLIC score unlocks, Yin's staged event, the
//! Firmament's plots) is not in an event window of the agreed content: a leaked secret scored as a public objective is the one reachable case
//! and the string's `award_tail` is not run for it (counted in the resume note).

use crate::choice::objectives::{score_choice, Ask as OAsk, Offer};
use crate::choice::{Ask, Choice, Pick};
use crate::containers::SmallList;
use crate::generated::SecretObjective;
use crate::ids::Seat;
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::rules::objective_windows::ScoreOne;
use crate::rules::scoring_book::{Asked, Offers, Outcome, ScoringBook};
use crate::state::{CoreState, Feat, ObjectiveRef, PlainArg, PlainKind, PlainRec, Scoring, ScoringTiming, Window as OpenWindow, WindowKind};

/// The feat that unlocks an event-timed secret objective (`secrets::feat_for`, by the card instead of its alias).
#[must_use]
pub const fn feat_of(secret: SecretObjective) -> Option<Feat> {
    Some(match secret {
        SecretObjective::DestroyTheirGreatestShip => Feat::DestroyedACapitalShip,
        SecretObjective::MakeAnExampleOfTheirWorld => Feat::BombardedOutTheLastGroundForces,
        SecretObjective::TurnTheirFleetsToDust => Feat::SpaceCannonTookTheLastNonFighters,
        SecretObjective::FightWithPrecision => Feat::BarrageTookTheLastFighters,
        SecretObjective::SparkARebellion => Feat::WonAgainstThePointsLeader,
        SecretObjective::UnveilFlagship => Feat::WonBesideASurvivingFlagship,
        SecretObjective::BetrayAFriend => Feat::WonAgainstANoteHolder,
        SecretObjective::BraveTheVoid => Feat::WonInAnAnomaly,
        SecretObjective::DarkenTheSkies => Feat::WonInARivalHome,
        SecretObjective::DemonstrateYourPower => Feat::HeldThreeShipsAfterASpaceCombat,
        SecretObjective::DriveTheDebate => Feat::ElectedByAnAgenda,
        SecretObjective::ProveEndurance => Feat::LastToPass,
        SecretObjective::BecomeAMartyr => Feat::LostAHomePlanet,
        _ => return None,
    })
}

/// The window an event occurrence opens (`ScoringWindow::for_occurrence` over `initiative_order`): every seat in initiative order, a combat's
/// occurrence capped at one scoring per player.
#[must_use]
pub fn event_scoring_window(core: &CoreState, occurrence: u64, one_per_player: bool) -> OpenWindow {
    let order: Vec<Seat> = crate::query::seats::initiative_order(core).iter().copied().collect();
    let book = ScoringBook::new(&order, false, one_per_player, true);
    OpenWindow::EventScoring(payload_of(&book, ScoringTiming::Action, Some(occurrence), one_per_player))
}

/// Open the window of `occurrence` on the stack (`open_occurrence_event_scoring`). Returns the window opened.
pub fn open_event_scoring(core: &mut CoreState, occurrence: u64, one_per_player: bool) -> OpenWindow {
    let w = event_scoring_window(core, occurrence, one_per_player);
    core.pending_mut().open(w.clone());
    w
}

fn payload_of(book: &ScoringBook, timing: ScoringTiming, occurrence: Option<u64>, one_per_player: bool) -> Scoring {
    Scoring {
        pending: book.pending().iter().copied().collect(),
        public_used: book.public_used(),
        secret_used: book.secret_used(),
        plot_card: book.plot_card(),
        timing,
        occurrence,
        one_per_player,
    }
}

/// The lists the position gives for an event window (`secrets::scoreable_event`): the secrets in the hand whose feat was recorded for the
/// occurrence, then (from rules version 24) the secrets made public that were not scored yet. No publics (an event window never offers one),
/// no plots (the Firmament is dropped).
pub struct CoreOffers<'a> {
    pub core: &'a CoreState,
    pub timing: ScoringTiming,
    pub occurrence: u64,
}

impl CoreOffers<'_> {
    fn phase_matches(&self, secret: SecretObjective) -> bool {
        use crate::generated::ObjectivePhase as P;
        matches!((secret.data().phase, self.timing), (P::Action, ScoringTiming::Action) | (P::Agenda, ScoringTiming::Agenda) | (P::Status, ScoringTiming::Status))
    }

    fn did(&self, seat: Seat, feat: Feat) -> bool {
        self.core.players().by_seat(seat).is_some_and(|p| p.misc.event_feats.as_slice().contains(&(feat, self.occurrence)))
    }

    fn scored(&self, seat: Seat, secret: SecretObjective) -> bool {
        self.core.objectives().scored[usize::from(seat.0)].secret.contains(secret)
    }
}

impl Offers for CoreOffers<'_> {
    fn publics(&mut self, _: Seat) -> SmallList<ObjectiveRef, 24> {
        SmallList::new()
    }

    fn secrets(&mut self, seat: Seat) -> SmallList<SecretObjective, 8> {
        let mut out: SmallList<SecretObjective, 8> = SmallList::new();
        if self.timing == ScoringTiming::Status {
            return out;
        }
        if let Some(p) = self.core.players().by_seat(seat) {
            for s in p.cards.secrets.as_slice() {
                if self.phase_matches(*s) && !self.scored(seat, *s) && feat_of(*s).is_some_and(|f| self.did(seat, f)) && out.len() < 8 {
                    out.push(*s);
                }
            }
        }
        // A secret made public keeps its printed timing: an event secret is scored the moment the event happens, once per player.
        if self.core.meta().rules_at_least(LEAKED_EVENT_SECRET) {
            for o in self.core.objectives().revealed.as_slice() {
                let ObjectiveRef::Secret(s) = *o else { continue };
                if !self.scored(seat, s) && !out.as_slice().contains(&s) && self.phase_matches(s) && feat_of(s).is_some_and(|f| self.did(seat, f)) && out.len() < 8 {
                    out.push(s);
                }
            }
        }
        out
    }

    fn plots(&mut self, _: Seat) -> SmallList<(SecretObjective, Seat), 8> {
        SmallList::new()
    }

    fn scored_at_occurrence(&mut self, seat: Seat) -> bool {
        self.core.objectives().scored_feats.iter().any(|(s, o)| *s == seat && *o == self.occurrence)
    }
}

/// `RULES_LEAKED_EVENT_SECRET` (`ti4_model::state`): a secret made public keeps its printed timing from this version.
const LEAKED_EVENT_SECRET: u32 = 24;

/// The open event scoring window.
#[derive(Debug, Clone)]
pub struct EventScoring {
    pub p: Scoring,
    /// The score ended the game: the window closes (the string drops it before `GAME_FINISHED`).
    over: bool,
}

impl EventScoring {
    #[must_use]
    pub const fn new(p: Scoring) -> Self {
        Self { p, over: false }
    }

    fn book(&self) -> ScoringBook {
        ScoringBook::restore(self.p.pending.as_slice(), self.p.public_used, self.p.secret_used, self.p.plot_card, self.p.timing == ScoringTiming::Status, self.p.one_per_player, self.p.occurrence.is_some())
    }

    fn offers<'a>(&self, core: &'a CoreState) -> CoreOffers<'a> {
        CoreOffers { core, timing: self.p.timing, occurrence: self.p.occurrence.unwrap_or(0) }
    }

    /// The seat asked next and what it may score.
    fn asked(&self, core: &CoreState) -> Option<Asked> {
        if self.over {
            return None;
        }
        self.book().asked(&mut self.offers(core))
    }

    fn points(core: &CoreState, who: Seat) -> u8 {
        core.players().by_seat(who).map_or(0, |p| u8::try_from(p.econ.victory_points.max(0)).unwrap_or(u8::MAX))
    }
}

impl Window for EventScoring {
    type Stage = bool;

    fn stage(&self) -> bool {
        self.over
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        match self.asked(sx.state) {
            Some(asked) => Next::Continue(Ask::Objectives(OAsk::Score { vp: Self::points(sx.state, asked.seat) })),
            None => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        let asked = self.asked(s)?;
        Some(score_choice(asked.seat, Self::points(s, asked.seat), self.p.timing == ScoringTiming::Status, asked.offers.as_slice()))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let asked = self.asked(sx.state).ok_or(Illegal)?;
        let offers: Vec<Offer> = asked.offers.as_slice().to_vec();
        let capped = self.p.occurrence.filter(|_| self.p.one_per_player);
        let mut one = ScoreOne::new(asked.seat, self.p.timing == ScoringTiming::Status, &offers, capped);
        let _ = one.step(sx);
        let revealed_before = |obj: ObjectiveRef, s: &CoreState| s.objectives().revealed.as_slice().contains(&obj);
        let public_before = one_public_before(&asked, pick, sx.state, revealed_before);
        one.answer(sx, pick)?;
        let outcome = if let Some((_, token, placed)) = one.plotted {
            Outcome::PlotScored { token, placed }
        } else if let Some(obj) = one.scored {
            Outcome::Scored { secret: matches!(obj, ObjectiveRef::Secret(_)) && !public_before }
        } else {
            Outcome::Declined
        };
        let mut book = self.book();
        book.after(&asked, outcome, &mut self.offers(sx.state));
        self.p = payload_of(&book, self.p.timing, self.p.occurrence, self.p.one_per_player);
        if let Some(obj) = one.scored {
            fx::plain(sx.state, PlainRec::one(PlainKind::ObjectiveScored, PlainArg::Obj(obj)));
        }
        if sx.state.meta().finished {
            fx::plain(sx.state, PlainRec::bare(PlainKind::GameFinished));
            self.over = true;
        }
        Ok(())
    }
}

/// Whether the option picked was an objective that was already on the table (a leaked secret scored as a public objective).
fn one_public_before(asked: &Asked, pick: Pick, core: &CoreState, revealed: impl Fn(ObjectiveRef, &CoreState) -> bool) -> bool {
    let _ = asked;
    match pick {
        Pick::Opt(crate::choice::Opt::Objectives(crate::choice::objectives::Opt::Score(obj))) => revealed(obj, core),
        _ => false,
    }
}

impl Held for EventScoring {
    const KIND: WindowKind = WindowKind::EventScoring;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        let OpenWindow::EventScoring(p) = core.pending().window(WindowKind::EventScoring)? else { return None };
        Some(Self::new(p.clone()))
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::EventScoring(self.p.clone()));
    }
}

/// Run the open event scoring window to its next question or its end (the arm's `LiveFn`).
///
/// # Errors
/// See [`crate::driver::live::LiveError`].
pub fn advance_event_scoring(
    core: &mut CoreState,
    env: &crate::driver::live::Env<'_>,
    answer: Option<Pick>,
) -> Result<(crate::driver::live::Live, crate::driver::live::Out), crate::driver::live::LiveError> {
    super::chain::sync_initiative(core);
    crate::driver::live::advance::<EventScoring>(core, env, answer)
}

/// The question the open window waits on (the arm's `AskFn`).
///
/// # Errors
/// As [`crate::driver::live::ask`].
pub fn ask_event_scoring(core: &CoreState, env: &crate::driver::live::Env<'_>) -> Result<Option<crate::driver::live::Asking>, crate::driver::live::LiveError> {
    crate::driver::live::ask::<EventScoring>(core, env)
}
