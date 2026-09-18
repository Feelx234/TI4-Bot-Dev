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
//! path as any policy answer: nothing here can apply a move the engine did not offer. All the state
//! it consults lives in the branch's [`Gate`], because a pause taken inside `ask` has to be answered
//! from another thread — see [`crate::live`] for why that is the only way to pause at every
//! decision.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_model::id::PlayerId;

use crate::control::{ChoiceFingerprint, Provenance};
use crate::live::{Gate, GateDecision};

/// A human's answer, waiting for the exact choice it was made against.
///
/// The fingerprint is checked before the answer is used, so an answer made for one offer can never be
/// spent on a different one. This is how R02-004's replay prefix and the unit tests answer a choice
/// without a person being asked; a live branch answers through the gate instead.
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
pub struct ManualInbox(Arc<std::sync::Mutex<Option<QueuedAnswer>>>);

impl ManualInbox {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<QueuedAnswer>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Offer an answer, returning whatever was displaced so the controller can say so rather than
    /// losing it quietly.
    pub fn put(&self, answer: QueuedAnswer) -> Option<QueuedAnswer> {
        self.lock().replace(answer)
    }

    /// Take the waiting answer if it was made against this exact offer.
    ///
    /// # Errors
    /// The displaced option id when an answer was waiting but for a *different* choice. It is dropped
    /// rather than left to linger, and the caller counts it: a mismatch is a controller bug, and
    /// silently spending it later would turn a visible bug into an invisible one.
    pub fn take_matching(&self, fingerprint: &ChoiceFingerprint) -> Result<Option<String>, String> {
        let mut waiting = self.lock();
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
        self.lock().is_none()
    }

    pub fn clear(&self) {
        *self.lock() = None;
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
pub struct AnswerLog(Arc<std::sync::Mutex<Vec<AnsweredDecision>>>);

impl AnswerLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<AnsweredDecision>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn push(&self, entry: AnsweredDecision) {
        self.lock().push(entry);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    #[must_use]
    pub fn entries(&self) -> Vec<AnsweredDecision> {
        self.lock().clone()
    }

    /// Provenance in answer order, for the tests that care about who answered what.
    #[must_use]
    pub fn provenance(&self) -> Vec<Provenance> {
        self.lock().iter().map(|entry| entry.provenance).collect()
    }
}

/// Counts of the defensive fallbacks.
///
/// A non-zero `unanswered` while a person is driving is a controller bug: the controller must never
/// advance a manual seat that is still waiting for an answer. The counts exist so "the game carried
/// on" cannot be mistaken for "the manual path worked".
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ManualFallbacks {
    /// Manual asks that arrived with no answer waiting and nothing able to wait.
    pub unanswered: usize,
    /// Waiting answers rejected because they belonged to a different offer.
    pub stale: usize,
}

/// Answers for one physical seat: the policy underneath, and the branch gate that says who answers.
pub struct ControlledDecider {
    inner: Box<dyn Decider>,
    seat: PlayerId,
    gate: Arc<Gate>,
}

impl ControlledDecider {
    pub fn new(inner: Box<dyn Decider>, seat: PlayerId, gate: Arc<Gate>) -> Self {
        Self { inner, seat, gate }
    }

    #[must_use]
    pub fn seat(&self) -> &PlayerId {
        &self.seat
    }

    /// The branch's control gate, for a wrapper that wants to read or extend it.
    #[must_use]
    pub fn gate(&self) -> &Arc<Gate> {
        &self.gate
    }

    /// Counts of the defensive fallbacks; see [`ManualFallbacks`].
    #[must_use]
    pub fn fallbacks(&self) -> ManualFallbacks {
        self.gate.fallbacks()
    }

    /// Ask the gate, then record whoever it said answered.
    fn answer_via_gate(
        &mut self,
        choice: &Choice,
        policy: impl FnOnce(&mut Box<dyn Decider>) -> Result<ChoiceOption, IllegalChoice>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let fingerprint = ChoiceFingerprint::from_choice(choice);
        match self.gate.decision_for(&self.seat, choice) {
            GateDecision::Human { option_id } => {
                let answer = choice.option(&option_id).cloned().ok_or_else(|| {
                    IllegalChoice::NotOffered {
                        player: self.seat.clone(),
                        chosen: option_id.clone(),
                        offered: choice.ids().into_iter().map(str::to_owned).collect(),
                    }
                })?;
                self.log(choice, &answer, &fingerprint, Provenance::Human);
                Ok(answer)
            }
            GateDecision::Policy { delegated } => {
                let answer = policy(&mut self.inner)?;
                let provenance = if delegated {
                    Provenance::DelegatedToPolicy
                } else {
                    Provenance::Policy
                };
                self.log(choice, &answer, &fingerprint, provenance);
                Ok(answer)
            }
        }
    }

    fn log(
        &self,
        choice: &Choice,
        answer: &ChoiceOption,
        fingerprint: &ChoiceFingerprint,
        provenance: Provenance,
    ) {
        self.gate.log().push(AnsweredDecision {
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
        self.gate.record_delivery(false);
        self.answer_via_gate(choice, |inner| inner.choose(choice))
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        self.gate.record_delivery(true);
        self.answer_via_gate(choice, |inner| inner.choose_seeing(choice, seen))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{BranchId, PendingManualChoice, SeatControl, SeatMode};
    use std::cell::Cell;
    use std::rc::Rc;

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
        gate: Arc<Gate>,
        inner_calls: Rc<Cell<usize>>,
    }

    /// A detached gate: nothing can park, so the fallback paths are observable instead of fatal.
    fn harness() -> Harness {
        let inner_calls = Rc::new(Cell::new(0));
        let gate = Arc::new(Gate::detached(SeatControl::all_auto()));
        let decider = ControlledDecider::new(
            Box::new(FirstAlways {
                calls: Rc::clone(&inner_calls),
            }),
            seat("seat2"),
            Arc::clone(&gate),
        );
        Harness {
            decider,
            gate,
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
        let logged = harness.gate.log().entries();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].provenance, Provenance::Policy);
        assert_eq!(logged[0].actor, seat("seat2"));
        assert_eq!(logged[0].chosen, "tactical");
        assert_eq!(
            harness.gate.delivery(),
            (1, 0),
            "the viewless entry point is recorded as such"
        );
    }

    #[test]
    fn a_queued_answer_is_used_without_consulting_the_policy() {
        let mut harness = harness();
        let offer = choice("seat2", &["tactical", "pass"]);
        let fingerprint = ChoiceFingerprint::from_choice(&offer);
        harness.gate.set_mode(&seat("seat2"), SeatMode::Manual);
        harness.gate.inbox().put(QueuedAnswer {
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
        assert!(harness.gate.inbox().is_empty(), "the answer is used once");
        let logged = harness.gate.log().entries();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].provenance, Provenance::Human);
        assert_eq!(logged[0].fingerprint, fingerprint);
    }

    #[test]
    fn an_answer_for_a_different_offer_is_dropped_and_counted() {
        let mut harness = harness();
        harness.gate.set_mode(&seat("seat2"), SeatMode::Manual);
        let other = choice("seat2", &["something", "else"]);
        harness.gate.inbox().put(QueuedAnswer {
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
            harness.gate.inbox().is_empty(),
            "a rejected answer does not linger for a later choice"
        );
    }

    #[test]
    fn a_manual_ask_with_no_answer_falls_back_counted_not_hidden() {
        let mut harness = harness();
        harness.gate.set_mode(&seat("seat2"), SeatMode::Manual);
        let offer = choice("seat2", &["tactical", "pass"]);
        // The detached gate's `debug_assert!` marks this as the controller bug it is, so only the
        // release profile walks it. R02-003's live gate parks instead of falling back.
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
        assert_eq!(harness.gate.log().provenance(), vec![Provenance::Policy]);
    }

    #[test]
    fn delegation_lets_the_policy_answer_once_and_keeps_the_seat_manual() {
        let mut harness = harness();
        harness.gate.set_mode(&seat("seat2"), SeatMode::Manual);
        harness.gate.delegate_seat_once(&seat("seat2"));

        let offer = choice("seat2", &["tactical", "pass"]);
        let answer = harness.decider.choose(&offer).expect("delegated");
        assert_eq!(answer.id, "tactical", "the policy answered this one");
        assert_eq!(
            harness.gate.log().provenance(),
            vec![Provenance::DelegatedToPolicy]
        );
        assert!(
            harness.gate.snapshot().seats.is_manual(&seat("seat2")),
            "one-shot delegation does not change the seat's mode"
        );
        assert!(
            !harness.gate.snapshot().seats.has_delegation(&seat("seat2")),
            "the delegation is consumed by the decision it was made for"
        );

        // The next ask is manual again with nothing queued, which a live gate would park on; the
        // detached gate's `debug_assert!` fires in a debug run, so only release walks it.
        if !cfg!(debug_assertions) {
            let _ = harness.decider.choose(&offer);
            let logged = harness.gate.log().provenance();
            assert_eq!(
                logged,
                vec![Provenance::DelegatedToPolicy, Provenance::Policy]
            );
            assert_eq!(harness.decider.fallbacks().unanswered, 1);
        }
    }

    #[test]
    fn the_deciders_fingerprint_is_the_ones_the_panel_would_show() {
        let offer = choice("seat2", &["tactical", "pass"]);
        let panel =
            PendingManualChoice::new(BranchId::SOURCE, 3, 1, None, &offer).expect("well formed");
        assert_eq!(
            panel.fingerprint,
            ChoiceFingerprint::from_choice(&offer),
            "the answer a person clicks and the answer the decider accepts must be keyed identically"
        );
    }
}
