//! The scoring windows as HELD windows (package KS1, lane C): the status phase's 81.1 window (`Window::Scoring`, `Arm::Scoring`) and the
//! windows of an action- or agenda-timed secret (`Window::EventScoring`, `Arm::EventScoring`), `Game::step_scoring`,
//! `Game::step_event_scoring` and `Game::score_step`.
//!
//! The machine is ONE type over a const flag. Its payload on the stack is the bookkeeping of the window ([`ScoringBook`]: who is asked
//! next, what each player used, the occurrence the window settles); WHO is asked and WHAT they may score is a function of that book and
//! the position ([`CoreOffers`], over `Seen`: the same queries the string engine's typed view calls), so `from_pending(to_pending(m))`
//! is the machine and a clone of the position mid-window continues identically. One question per step, as the string asks; a step that
//! finds nobody left to ask ends the window (the string's next step is its close, which the arm functions of `status.rs` run).
//!
//! What stays out, by decision, as counted gaps (`TurnGap`): Plots Within Plots (the Firmament is dropped, `plot_card` is never set), a
//! secret the core cannot judge (Strengthen Bonds, Produce en Masse), and a commander of a faction whose unlock hook no module registers
//! yet (the unlock check after a public score, see `turn::start::unlockable`).

use crate::choice::objectives::{score_choice, Offer};
use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::generated::{ObjectivePhase, SecretObjective};
use crate::ids::Seat;
use crate::kernel::fx::finish_if_won;
use crate::kernel::fx_pay::PayEnv;
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::rules::leaders;
use crate::rules::objective_windows::ScoreOne;
use crate::rules::scoring_book::{Asked, Offers, Outcome, ScoringBook};
use crate::seen::Seen;
use crate::state::{CoreState, ObjectiveRef, PlainArg, PlainKind, PlainRec, Scoring, ScoringTiming, Window as OpenWindow, WindowKind};

use super::super::turn::inline::gap;

/// The phase a window's timing scores secrets of.
const fn phase_of(timing: ScoringTiming) -> ObjectivePhase {
    match timing {
        ScoringTiming::Status => ObjectivePhase::Status,
        ScoringTiming::Action => ObjectivePhase::Action,
        ScoringTiming::Agenda => ObjectivePhase::Agenda,
    }
}

/// The lists the position gives a scoring window: the objectives the core's queries find a seat able to score.
pub struct CoreOffers<'a> {
    core: &'a CoreState,
    seen: Seen<'a>,
    timing: ScoringTiming,
    occurrence: Option<u64>,
    /// A list the core cannot give (a secret it cannot judge): the window must stop with a gap.
    pub unsupported: bool,
}

impl<'a> CoreOffers<'a> {
    #[must_use]
    pub fn new(core: &'a CoreState, kernel: &'a Kernel, timing: ScoringTiming, occurrence: Option<u64>) -> Self {
        let seen = Seen::new(core, kernel, Seat(0)).with_blanked(reach::blanked(core)); // mutants:skip: the bound seat is not read by the public accessors used here
        Self { core, seen, timing, occurrence, unsupported: false }
    }
}

impl Offers for CoreOffers<'_> {
    fn publics(&mut self, seat: Seat) -> SmallList<ObjectiveRef, 24> {
        self.seen.scoreable_publics(seat).unwrap_or_else(|| {
            self.unsupported = true;
            SmallList::new()
        })
    }

    fn secrets(&mut self, seat: Seat) -> SmallList<SecretObjective, 8> {
        self.seen.scoreable_secrets(seat, phase_of(self.timing), self.occurrence).unwrap_or_else(|| {
            self.unsupported = true;
            SmallList::new()
        })
    }

    // mutants:skip: an empty list either way (the Firmament is dropped)
    fn plots(&mut self, _: Seat) -> SmallList<(SecretObjective, Seat), 8> {
        // Plots Within Plots belongs to the Firmament (dropped).
        SmallList::new()
    }

    fn scored_at_occurrence(&mut self, seat: Seat) -> bool {
        self.occurrence.is_some_and(|o| self.core.objectives().scored_feats.iter().any(|f| *f == (seat, o)))
    }
}

/// The scoring window: `EVENT` false is the status window, true an event window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoringWin<const EVENT: bool> {
    book: ScoringBook,
    timing: ScoringTiming,
    occurrence: Option<u64>,
    one_per_player: bool,
}

/// The status window's machine.
pub type StatusScoring = ScoringWin<false>;
/// The machine of an action- or agenda-timed secret's window.
pub type EventScoring = ScoringWin<true>;

/// Why the window cannot go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stuck {
    /// Plots Within Plots left a card to place.
    PlotCard,
    /// A secret the core cannot judge.
    Secret,
}

impl<const EVENT: bool> ScoringWin<EVENT> {
    /// A fresh status window over `order` (initiative order, first first): `Game::step_status`.
    #[must_use]
    pub fn status(order: &[Seat]) -> StatusScoring {
        // mutants:skip: the flags of the new book are rebuilt from the payload (`of_payload`) before the window is ever run
        ScoringWin { book: ScoringBook::new(order, true, false, false), timing: ScoringTiming::Status, occurrence: None, one_per_player: false }
    }

    /// A fresh event window over `order` for one occurrence (`ScoringWindow::for_occurrence`).
    #[must_use]
    pub fn event(order: &[Seat], timing: ScoringTiming, occurrence: u64, one_per_player: bool) -> EventScoring {
        // mutants:skip: the flags of the new book are rebuilt from the payload (`of_payload`) before the window is ever run
        ScoringWin { book: ScoringBook::new(order, false, one_per_player, true), timing, occurrence: Some(occurrence), one_per_player }
    }

    /// The bookkeeping.
    #[must_use]
    pub const fn book(&self) -> &ScoringBook {
        &self.book
    }

    fn payload(&self) -> Scoring {
        Scoring {
            pending: SmallList::from_slice(self.book.pending()),
            public_used: self.book.public_used(),
            secret_used: self.book.secret_used(),
            plot_card: self.book.plot_card(),
            timing: self.timing,
            occurrence: self.occurrence,
            one_per_player: self.one_per_player,
        }
    }

    fn of_payload(p: &Scoring) -> Self {
        let status = p.timing == ScoringTiming::Status;
        Self {
            book: ScoringBook::restore(p.pending.as_slice(), p.public_used, p.secret_used, p.plot_card, status, p.one_per_player, p.occurrence.is_some()),
            timing: p.timing,
            occurrence: p.occurrence,
            one_per_player: p.one_per_player,
        }
    }

    /// The player the window asks next, or why it cannot say.
    fn next(&self, core: &CoreState, k: &Kernel) -> Result<Option<Asked>, Stuck> {
        if self.book.plot_card().is_some() {
            return Err(Stuck::PlotCard);
        }
        let mut offers = CoreOffers::new(core, k, self.timing, self.occurrence);
        let asked = self.book.asked(&mut offers);
        if offers.unsupported { Err(Stuck::Secret) } else { Ok(asked) }
    }

    /// The one-player window over what `asked` may score.
    fn one(&self, asked: &Asked, k: &Kernel, core: &CoreState) -> ScoreOne {
        let offers: Vec<Offer> = asked.offers.iter().copied().collect();
        let env = PayEnv { scope: reach::scope_of(k), blanked: reach::blanked(core) };
        // Only a window with the one-per-player cap records the occurrence a seat used (`ScoringWindow::score`); an agenda's window does not.
        ScoreOne::new(asked.seat, !EVENT, &offers, self.occurrence.filter(|_| self.one_per_player)).with_price(env)
    }
}

impl<const EVENT: bool> Window for ScoringWin<EVENT> {
    type Stage = usize;

    // mutants:skip: the stage is a label for the trace; the driver rebuilds the window from the payload and never branches on it
    fn stage(&self) -> usize {
        self.book.pending().len()
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if sx.state.meta().finished {
            return Next::Done;
        }
        match self.next(sx.state, sx.kernel) {
            Err(Stuck::PlotCard) => {
                gap(sx, TurnGap::PlotCard);
                Next::Done
            }
            Err(Stuck::Secret) => {
                gap(sx, TurnGap::ScoringSecret);
                Next::Done
            }
            Ok(None) => Next::Done,
            Ok(Some(asked)) => {
                let mut one = self.one(&asked, sx.kernel, sx.state);
                one.step(sx)
            }
        }
    }

    fn ask_of(&self, core: &CoreState, k: &Kernel) -> Option<Choice> {
        if core.meta().finished {
            return None;
        }
        let asked = self.next(core, k).ok()??;
        // `ScoreOne::ask_of` answers only after its own `step`; the question is the same function of the position.
        // mutants:skip: the seat asked exists, the default is unreachable
        let vp = core.players().by_seat(asked.seat).map_or(0, |p| u8::try_from(p.econ.victory_points.max(0)).unwrap_or(u8::MAX));
        Some(score_choice(asked.seat, vp, !EVENT, asked.offers.as_slice()))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let asked = self.next(sx.state, sx.kernel).ok().flatten().ok_or(Illegal)?;
        let mut one = self.one(&asked, sx.kernel, sx.state);
        let _ = one.step(sx);
        let revealed_before: Vec<ObjectiveRef> = sx.state.objectives().revealed.as_slice().to_vec();
        one.answer(sx, pick)?;
        let outcome = match one.scored {
            Some(o) => Outcome::Scored { secret: matches!(o, ObjectiveRef::Secret(_)) && !revealed_before.contains(&o) },
            None => Outcome::Declined,
        };
        // `objectives::award_tail`: a public score may unlock a hero (51.7); no map, no event.
        if let Some(o) = one.scored
            && (revealed_before.contains(&o) || matches!(o, ObjectiveRef::Public(_)))
        {
            let Some(unlocking) = super::super::turn::start::unlockable(sx, asked.seat, true) else { return Ok(()) };
            leaders::unlock(sx.state, asked.seat, unlocking.as_slice());
        }
        if let Some(o) = one.scored {
            sx.plain(PlainRec::one(PlainKind::ObjectiveScored, PlainArg::Obj(o)));
        }
        finish_if_won(sx.state);
        let mut post = CoreOffers::new(sx.state, sx.kernel, self.timing, self.occurrence);
        self.book.after(&asked, outcome, &mut post);
        if post.unsupported {
            gap(sx, TurnGap::ScoringSecret);
        }
        if sx.state.meta().finished {
            sx.plain(PlainRec::bare(PlainKind::GameFinished));
        }
        Ok(())
    }
}

impl Held for ScoringWin<false> {
    const KIND: WindowKind = WindowKind::Scoring;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::Scoring)? {
            OpenWindow::Scoring(p) => Some(Self::of_payload(p)),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Scoring(self.payload()));
    }
}

impl Held for ScoringWin<true> {
    const KIND: WindowKind = WindowKind::EventScoring;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::EventScoring)? {
            OpenWindow::EventScoring(p) => Some(Self::of_payload(p)),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::EventScoring(self.payload()));
    }
}

/// Open the window of a secret timed on an event (`Game::open_occurrence_event_scoring`): the rule windows that end an event a secret asks
/// about (a pass: Prove Endurance; an agenda's close; a combat or an invasion) call it, and the next live call of `Arm::EventScoring`
/// serves it. `occurrence` is the feat occurrence the window settles (`Meta::feat_occurrence_seq` after `begin_feat_occurrence`).
pub fn open_event(core: &mut CoreState, timing: ScoringTiming, occurrence: u64, one_per_player: bool) {
    let order: Vec<Seat> = crate::query::seats::initiative_order(core).iter().copied().collect();
    EventScoring::event(&order, timing, occurrence, one_per_player).to_pending(core);
}

/// The feat that unlocks an event-timed secret objective (`secrets::feat_for`): the one table, [`crate::query::objectives::feat_of`].
pub use crate::query::objectives::feat_of;

/// The window an action-timed event occurrence opens (`ScoringWindow::for_occurrence` over `initiative_order`): every seat in initiative
/// order, a combat's occurrence capped at one scoring per player. The same machine as the status window's ([`ScoringWin`]); the aftermath
/// chain (lane M5) parks on it and learns what it opened from this value.
#[must_use]
pub fn event_scoring_window(core: &CoreState, occurrence: u64, one_per_player: bool) -> OpenWindow {
    let order: Vec<Seat> = crate::query::seats::initiative_order(core).iter().copied().collect();
    OpenWindow::EventScoring(EventScoring::event(&order, ScoringTiming::Action, occurrence, one_per_player).payload())
}

/// Open the action-timed window of `occurrence` on the stack (`open_occurrence_event_scoring`). Returns the window opened.
pub fn open_event_scoring(core: &mut CoreState, occurrence: u64, one_per_player: bool) -> OpenWindow {
    let w = event_scoring_window(core, occurrence, one_per_player);
    core.pending_mut().open(w.clone());
    w
}
