//! The decider decorator that decides who answers a seat.
//!
//! [`ControlledDecider`] sits between the reviewer's trace wrapper and the real learned policy:
//!
//! ```text
//! engine Table ──> TraceBot / MlpTraceBot ──> ControlledDecider ──> learned policy
//!                        │                           │
//!                 scores every option        decides who answers, and why
//! ```
//!
//! Underneath the trace on purpose. The trace scores the options and records the decision before the
//! decorator is consulted, so a choice a person made still shows the scores, probabilities and
//! feature projections the policy would have had — which is the point of showing them side by side.
//! Above the trace, an overridden decision would be recorded with nothing in it.
//!
//! It is a [`Decider`], so the engine validates its answer through the same `Table::ask`/`settle`
//! path as any policy answer: nothing here can apply a move the engine did not offer.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_model::id::PlayerId;

use crate::control::{ChoiceFingerprint, Provenance, SeatControl};

/// A human's answer, waiting for the exact choice it was made against.
///
/// The fingerprint is checked before the answer is used, so an answer made for one offer can never be
/// spent on a different one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedAnswer {
    pub fingerprint: ChoiceFingerprint,
    pub option_id: String,
}

/// The waiting answer for one live branch.
///
/// One slot rather than a queue: the engine asks one question at a time, so a second answer stored
/// here is either a race or a bug, and neither should be silently spent later.
#[derive(Clone, Debug, Default)]
pub struct ManualInbox(Rc<RefCell<Option<QueuedAnswer>>>);

impl ManualInbox {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Offer an answer, returning whatever was displaced so the controller can say so rather than
    /// losing it quietly.
    pub fn put(&self, answer: QueuedAnswer) -> Option<QueuedAnswer> {
        self.0.borrow_mut().replace(answer)
    }

    /// Take the waiting answer if it was made against this exact offer.
    ///
    /// # Errors
    /// The displaced option id when an answer was waiting but for a *different* choice. It is dropped
    /// rather than left to linger, and the caller counts it: a mismatch is a controller bug, and
    /// silently spending it later would turn a visible bug into an invisible one.
    pub fn take_matching(&self, fingerprint: &ChoiceFingerprint) -> Result<Option<String>, String> {
        let mut waiting = self.0.borrow_mut();
        let Some(current) = waiting.as_mut() else {
            return Ok(None);
        };
        if &current.fingerprint != fingerprint {
            let stale = current.option_id.clone();
            *waiting = None;
            return Err(stale);
        }
        Ok(waiting.take().map(|answer| answer.option_id))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_none()
    }

    pub fn clear(&self) {
        *self.0.borrow_mut() = None;
    }
}

/// One answered decision, in the shape the branch tree wants.
///
/// This is the live-stream twin of [`crate::control::ReplayRecord`]: the record says what a rebuild
/// must replay, this says what just happened and who did it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnsweredDecision {
    pub actor: PlayerId,
    pub prompt: String,
    pub chosen: String,
    pub fingerprint: ChoiceFingerprint,
    pub provenance: Provenance,
}

/// Shared answer log for one live branch, appended to in ask order.
#[derive(Clone, Debug, Default)]
pub struct AnswerLog(Rc<RefCell<Vec<AnsweredDecision>>>);

impl AnswerLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&self, entry: AnsweredDecision) {
        self.0.borrow_mut().push(entry);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    #[must_use]
    pub fn entries(&self) -> Vec<AnsweredDecision> {
        self.0.borrow().clone()
    }
}

/// Counts of the defensive fallbacks below.
///
/// A non-zero `unanswered` while a person is driving is a controller bug: the live controller must
/// never advance a manual seat that is still waiting for an answer. The counts exist so "the game
/// carried on" cannot be mistaken for "the manual path worked".
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ManualFallbacks {
    /// Manual asks that arrived with no answer waiting.
    pub unanswered: usize,
    /// Waiting answers rejected because they belonged to a different offer.
    pub stale: usize,
}

/// What the control state said about one ask, before anything was answered.
enum Decision {
    /// A person answered this exact offer.
    Human {
        answer: ChoiceOption,
        fingerprint: ChoiceFingerprint,
    },
    /// The policy answers, and `delegated` distinguishes "let the bot have this one" from a seat
    /// that is simply on Auto.
    Policy {
        fingerprint: ChoiceFingerprint,
        delegated: bool,
    },
}

/// Answers for one physical seat: the policy underneath, and who is allowed to answer right now.
pub struct ControlledDecider {
    inner: Box<dyn Decider>,
    seat: PlayerId,
    modes: Rc<RefCell<SeatControl>>,
    inbox: ManualInbox,
    log: AnswerLog,
    unanswered: Rc<Cell<usize>>,
    stale: Rc<Cell<usize>>,
}

impl ControlledDecider {
    pub fn new(
        inner: Box<dyn Decider>,
        seat: PlayerId,
        modes: Rc<RefCell<SeatControl>>,
        inbox: ManualInbox,
        log: AnswerLog,
    ) -> Self {
        Self {
            inner,
            seat,
            modes,
            inbox,
            log,
            unanswered: Rc::new(Cell::new(0)),
            stale: Rc::new(Cell::new(0)),
        }
    }

    #[must_use]
    pub fn seat(&self) -> &PlayerId {
        &self.seat
    }

    #[must_use]
    pub fn fallbacks(&self) -> ManualFallbacks {
        ManualFallbacks {
            unanswered: self.unanswered.get(),
            stale: self.stale.get(),
        }
    }

    /// Read the control state for one ask. Never touches the policy underneath.
    fn decide(&mut self, choice: &Choice) -> Decision {
        let fingerprint = ChoiceFingerprint::from_choice(choice);
        let (delegated, manual) = {
            let mut modes = self.modes.borrow_mut();
            let delegated = modes.take_delegation(&self.seat);
            (delegated, modes.is_manual(&self.seat))
        };
        if delegated || !manual {
            return Decision::Policy {
                fingerprint,
                delegated,
            };
        }
        match self.inbox.take_matching(&fingerprint) {
            Ok(Some(option_id)) => {
                if let Some(answer) = choice.option(&option_id).cloned() {
                    return Decision::Human {
                        answer,
                        fingerprint,
                    };
                }
                // A queued id that is not on offer cannot happen while the fingerprint binds the
                // ordered ids; if it ever does, the engine's own validation is the place that says
                // so, so the answer is passed down as a policy answer and refused there.
                self.stale.set(self.stale.get() + 1);
                return Decision::Policy {
                    fingerprint,
                    delegated: false,
                };
            }
            Ok(None) => {
                // The controller is meant to pause before this is reachable. Replying from the
                // policy keeps the game alive and *counted*: the frozen engine has no error variant
                // for "the controller forgot", and returning NotOffered would poison the step,
                // which is a worse failure than a visible counter.
                self.unanswered.set(self.unanswered.get() + 1);
                debug_assert!(
                    false,
                    "manual seat {} was asked with no answer queued",
                    self.seat
                );
            }
            Err(_stale) => {
                self.stale.set(self.stale.get() + 1);
            }
        }
        Decision::Policy {
            fingerprint,
            delegated: false,
        }
    }

    fn record(
        &self,
        choice: &Choice,
        answer: &ChoiceOption,
        fingerprint: &ChoiceFingerprint,
        provenance: Provenance,
    ) {
        self.log.push(AnsweredDecision {
            actor: self.seat.clone(),
            prompt: choice.prompt.clone(),
            chosen: answer.id.clone(),
            fingerprint: fingerprint.clone(),
            provenance,
        });
    }
}

impl Decider for ControlledDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        match self.decide(choice) {
            Decision::Human {
                answer,
                fingerprint,
            } => {
                self.record(choice, &answer, &fingerprint, Provenance::Human);
                Ok(answer)
            }
            Decision::Policy {
                fingerprint,
                delegated,
            } => {
                let answer = self.inner.choose(choice)?;
                let provenance = if delegated {
                    Provenance::DelegatedToPolicy
                } else {
                    Provenance::Policy
                };
                self.record(choice, &answer, &fingerprint, provenance);
                Ok(answer)
            }
        }
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        match self.decide(choice) {
            Decision::Human {
                answer,
                fingerprint,
            } => {
                self.record(choice, &answer, &fingerprint, Provenance::Human);
                Ok(answer)
            }
            Decision::Policy {
                fingerprint,
                delegated,
            } => {
                let answer = self.inner.choose_seeing(choice, seen)?;
                let provenance = if delegated {
                    Provenance::DelegatedToPolicy
                } else {
                    Provenance::Policy
                };
                self.record(choice, &answer, &fingerprint, provenance);
                Ok(answer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{SeatControl, SeatMode};
    use ti4_engine::choice::ChoiceOption;

    /// A policy underneath the decorator: always takes the first option, and counts being asked.
    struct FirstAlways {
        calls: Rc<Cell<usize>>,
    }

    impl Decider for FirstAlways {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            self.calls.set(self.calls.get() + 1);
            choice
                .options
                .first()
                .cloned()
                .ok_or_else(|| IllegalChoice::NoOptions {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                })
        }
    }

    fn seat(name: &str) -> PlayerId {
        PlayerId::new(name)
    }

    fn choice(actor: &str, ids: &[&str]) -> Choice {
        Choice::new(
            seat(actor),
            "choose an action",
            ids.iter()
                .map(|id| ChoiceOption::labelled(*id, "action", format!("label {id}")))
                .collect(),
        )
    }

    struct Harness {
        decider: ControlledDecider,
        modes: Rc<RefCell<SeatControl>>,
        inbox: ManualInbox,
        log: AnswerLog,
        inner_calls: Rc<Cell<usize>>,
    }

    fn harness() -> Harness {
        let inner_calls = Rc::new(Cell::new(0));
        let modes = Rc::new(RefCell::new(SeatControl::all_auto()));
        let inbox = ManualInbox::new();
        let log = AnswerLog::new();
        let decider = ControlledDecider::new(
            Box::new(FirstAlways {
                calls: Rc::clone(&inner_calls),
            }),
            seat("seat2"),
            Rc::clone(&modes),
            inbox.clone(),
            log.clone(),
        );
        Harness {
            decider,
            modes,
            inbox,
            log,
            inner_calls,
        }
    }

    #[test]
    fn an_auto_seat_hands_the_decision_to_the_policy() {
        let mut harness = harness();
        let offer = choice("seat2", &["tactical", "pass"]);
        let answer = harness.decider.choose(&offer).expect("auto answers");
        assert_eq!(answer.id, "tactical");
        assert_eq!(harness.inner_calls.get(), 1);
        assert_eq!(harness.decider.fallbacks(), ManualFallbacks::default());
        let logged = harness.log.entries();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].provenance, Provenance::Policy);
        assert_eq!(logged[0].actor, seat("seat2"));
        assert_eq!(logged[0].chosen, "tactical");
    }

    #[test]
    fn a_queued_answer_is_used_without_consulting_the_policy() {
        let mut harness = harness();
        let offer = choice("seat2", &["tactical", "pass"]);
        let fingerprint = ChoiceFingerprint::from_choice(&offer);
        harness
            .modes
            .borrow_mut()
            .set_mode(&seat("seat2"), SeatMode::Manual);
        harness.inbox.put(QueuedAnswer {
            fingerprint: fingerprint.clone(),
            option_id: "pass".to_owned(),
        });

        let answer = harness
            .decider
            .choose(&offer)
            .expect("the human answer is used");
        assert_eq!(answer.id, "pass");
        assert_eq!(
            harness.inner_calls.get(),
            0,
            "a human answer must not consult the policy"
        );
        assert!(harness.inbox.is_empty(), "the answer is consumed once");
        let logged = harness.log.entries();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].provenance, Provenance::Human);
        assert_eq!(logged[0].fingerprint, fingerprint);
    }

    #[test]
    fn an_answer_for_a_different_offer_is_dropped_and_counted() {
        let mut harness = harness();
        harness
            .modes
            .borrow_mut()
            .set_mode(&seat("seat2"), SeatMode::Manual);
        let other = choice("seat2", &["something", "else"]);
        harness.inbox.put(QueuedAnswer {
            fingerprint: ChoiceFingerprint::from_choice(&other),
            option_id: "something".to_owned(),
        });

        let offer = choice("seat2", &["tactical", "pass"]);
        let answer = harness
            .decider
            .choose(&offer)
            .expect("a stale answer is not used");
        assert_eq!(answer.id, "tactical", "the policy answers instead");
        assert_eq!(
            harness.decider.fallbacks(),
            ManualFallbacks {
                unanswered: 0,
                stale: 1
            }
        );
        assert!(
            harness.inbox.is_empty(),
            "a rejected answer does not linger for a later choice"
        );
    }

    #[test]
    fn a_manual_ask_with_no_answer_falls_back_counted_not_hidden() {
        let mut harness = harness();
        harness
            .modes
            .borrow_mut()
            .set_mode(&seat("seat2"), SeatMode::Manual);
        let offer = choice("seat2", &["tactical", "pass"]);
        // `debug_assert!` fires on this path, so only the release-profile behaviour is exercised
        // here; R02-003's controller is what makes the path unreachable in a debug run.
        if cfg!(debug_assertions) {
            return;
        }
        let answer = harness.decider.choose(&offer).expect("play continues");
        assert_eq!(answer.id, "tactical");
        assert_eq!(
            harness.decider.fallbacks(),
            ManualFallbacks {
                unanswered: 1,
                stale: 0
            }
        );
        assert_eq!(harness.log.entries()[0].provenance, Provenance::Policy);
    }

    #[test]
    fn delegation_lets_the_policy_answer_once_and_keeps_the_seat_manual() {
        let mut harness = harness();
        let modes = Rc::clone(&harness.modes);
        modes
            .borrow_mut()
            .set_mode(&seat("seat2"), SeatMode::Manual);
        modes.borrow_mut().delegate_once(&seat("seat2"));

        let offer = choice("seat2", &["tactical", "pass"]);
        assert_eq!(
            harness.decider.choose(&offer).expect("delegated").id,
            "tactical"
        );
        assert_eq!(
            harness.log.entries()[0].provenance,
            Provenance::DelegatedToPolicy
        );
        assert!(
            modes.borrow().is_manual(&seat("seat2")),
            "one-shot delegation does not change the seat's mode"
        );

        // A second ask is manual again with nothing queued, which is the path the live controller
        // must never reach; its `debug_assert!` fires in a debug run, so only the release profile
        // walks it here.
        if !cfg!(debug_assertions) {
            let _ = harness.decider.choose(&offer);
            let logged = harness.log.entries();
            assert_eq!(logged.len(), 2);
            assert_eq!(logged[1].provenance, Provenance::Policy);
            assert_eq!(harness.decider.fallbacks().unanswered, 1);
        }
    }

    #[test]
    fn the_deciders_fingerprint_is_the_ones_the_panel_would_show() {
        let harness = harness();
        let offer = choice("seat2", &["tactical", "pass"]);
        let panel = crate::control::PendingManualChoice::new(
            crate::control::BranchId::SOURCE,
            3,
            1,
            None,
            &offer,
        )
        .expect("well formed");
        assert_eq!(
            panel.fingerprint,
            ChoiceFingerprint::from_choice(&offer),
            "the answer the human clicks and the answer the decider accepts must be keyed identically"
        );
        let _ = harness;
    }
}
