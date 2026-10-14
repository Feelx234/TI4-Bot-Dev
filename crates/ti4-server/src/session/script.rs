//! A script of a seat: run as a draft on a copy of the game, or sent to the live game.
//!
//! A script is what the page of a player keeps of a draft or of a staged step
//! ([`ti4_host::planning::draft::DraftStep`]). The server keeps none: a run plays the whole
//! script on a copy and forgets it ([`run_draft`]), and a script that is sent lives only while
//! the game answers its questions from it ([`ti4_host::planning::script::ScriptRuns`]). Here the
//! server tells each seat what became of its script, and checks a script on a copy before it is
//! sent ([`check_script`]).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ti4_engine::choice::{
    Choice, ChoiceOption, Decider, DecisionRecord, IllegalChoice, SeatObservation, Table,
};
use ti4_engine::game::Game;
use ti4_host::planning::draft::{DraftStep, ScriptAnswer, needless, script_answer};
use ti4_host::planning::script::{ACTION_OVER, Ended, tactical_complete};
use ti4_host::protocol::PROTOCOL_VERSION;
use ti4_host::protocol::server::{DraftAppliedMsg, ServerMessage};
use ti4_host::protocol::status::ViewerRole;
use ti4_host::view::reaction_modes::{Filtered, filter};
use ti4_model::id::PlayerId;

use crate::session::worker::SessionShared;

/// How many engine steps the check of a script takes at most, as a run of a draft.
const STEP_BOUND: usize = 64;

impl SessionShared {
    /// Tells the connections of each seat what became of its script.
    fn tell_scripts(&mut self, ended: Vec<Ended>) {
        for end in ended {
            let message = ServerMessage::DraftApplied(DraftAppliedMsg {
                protocol_version: PROTOCOL_VERSION,
                game_id: self.game_id.clone(),
                nonce: end.nonce,
                stopped: end.stopped,
            });
            self.subscribers.retain(|_, subscriber| {
                !matches!(&subscriber.viewer, ViewerRole::Player(viewer) if *viewer == end.seat)
                    || subscriber.tx.try_send(message.clone()).is_ok()
            });
        }
    }

    /// Begins to answer `seat` from `script`, whose first answer is given by the caller: `next`
    /// and `script` are as [`script_answer`] left them after it.
    pub(crate) fn begin_script(
        &mut self,
        seat: &PlayerId,
        nonce: &str,
        script: Vec<DraftStep>,
        next: usize,
    ) {
        let active = self.latest_state.active.clone();
        let ended = self
            .scripts
            .begin(seat, nonce, script.clone(), script, next, active);
        self.tell_scripts(ended);
    }

    /// The answer of the script of the asked seat to `choice`, when it has one
    /// ([`ti4_host::planning::script::ScriptRuns::answer`]).
    pub(crate) fn script_option(&mut self, choice: &Choice) -> Option<ChoiceOption> {
        let (option, ended) = self.scripts.answer(choice);
        self.tell_scripts(ended);
        option
    }

    /// A step of the game is complete: a script whose action or turn is over ends.
    pub(crate) fn scripts_after_step(&mut self, tactical_complete: bool) {
        let active = self.latest_state.active.clone();
        let ended = self.scripts.after_step(tactical_complete, active.as_ref());
        self.tell_scripts(ended);
    }
}

/// What a check of a script on a copy of the game found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Check {
    /// The game takes the script as far as the copy can tell: to its end, to a question that
    /// is not of the script, or to a decision of another seat.
    Plays,
    /// The game does not offer what entry `step` asks for.
    Refused { step: usize, reason: String },
    /// The copy did not come to the choice that the live game waits in.
    Unknown,
}

struct Probe {
    /// The decisions of the live game since the copy was made, which the copy plays again.
    tail: VecDeque<DecisionRecord>,
    /// The choice that the live game waits in. `None` once the copy came to it.
    pending: Option<Choice>,
    actor: PlayerId,
    /// The cards that the actor is never offered: the live game declines a window of such
    /// cards before the seat, and so before its script, is asked.
    never: ti4_host::view::reaction_modes::Modes,
    script: Vec<DraftStep>,
    next: usize,
    verdict: Option<Check>,
}

impl Probe {
    fn answer(&mut self, choice: &Choice) -> Option<ChoiceOption> {
        if self.verdict.is_some() {
            return None;
        }
        if let Some(record) = self.tail.pop_front() {
            let option = (record.player == choice.player)
                .then(|| choice.option(&record.chosen).cloned())
                .flatten();
            if option.is_none() {
                self.verdict = Some(Check::Unknown);
            }
            return option;
        }
        // From here on the copy asks what the live game asks next: the actor is asked as its
        // seat is, behind its Never cards.
        let narrowed;
        let choice = if choice.player == self.actor {
            match filter(choice, &self.never) {
                Filtered::Declined { option, .. } => return Some(option),
                Filtered::Narrowed(choice) => {
                    narrowed = *choice;
                    &narrowed
                }
                Filtered::Unchanged => choice,
            }
        } else {
            choice
        };
        if let Some(pending) = self.pending.take()
            && (pending.player != choice.player || pending.ids() != choice.ids())
        {
            self.verdict = Some(Check::Unknown);
            return None;
        }
        if choice.player != self.actor {
            // What another seat decides is not known here: the script is fine so far.
            self.verdict = Some(Check::Plays);
            return None;
        }
        match script_answer(&self.script, self.next, choice, &self.actor) {
            ScriptAnswer::Answer {
                option,
                next,
                script,
            } => {
                self.next = next;
                if let Some(script) = script {
                    self.script = script;
                }
                Some(option)
            }
            ScriptAnswer::Done | ScriptAnswer::Interrupted { .. } => {
                self.verdict = Some(Check::Plays);
                None
            }
            ScriptAnswer::Mismatch { step, reason } => {
                self.verdict = Some(Check::Refused { step, reason });
                None
            }
        }
    }
}

struct ProbeDecider(Arc<Mutex<Probe>>);

impl Decider for ProbeDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0
            .lock()
            .expect("probe lock")
            .answer(choice)
            .ok_or_else(|| IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "the check of the script stops here".to_owned(),
            })
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        _seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        self.choose(choice)
    }
}

/// Plays `script` for `actor` on `game`, a copy of the live game at its last completed step.
///
/// The copy first plays `tail` again, the decisions of the live game since that step, and must
/// then ask `pending`, the choice that the live game waits in. From there the script answers.
/// The copy has the dice of the live game, so what it plays is what the live game will play,
/// up to the first decision of another seat. `never` is the Never cards of the actor.
pub(crate) fn check_script(
    mut game: Game<'static>,
    tail: Vec<DecisionRecord>,
    pending: &Choice,
    script: &[DraftStep],
    actor: &PlayerId,
    never: ti4_host::view::reaction_modes::Modes,
) -> Check {
    let probe = Arc::new(Mutex::new(Probe {
        tail: tail.into(),
        pending: Some(pending.clone()),
        actor: actor.clone(),
        never,
        script: script.to_vec(),
        next: 0,
        verdict: None,
    }));
    // The table of the timing resolver stays as the copy has it: the live game leaves it alone
    // too, and its answers are not in the decision log.
    game.table = Table::with_default(Box::new(ProbeDecider(probe.clone())));
    let mut events = game.events.len();
    for _ in 0..STEP_BOUND {
        let result = game.step();
        let mut probe = probe.lock().expect("probe lock");
        if probe.verdict.is_some() {
            break;
        }
        if result.error.is_some() || probe.pending.is_some() && probe.tail.is_empty() {
            // The copy failed, or a step ended before the choice of the live game was asked.
            probe.verdict = Some(Check::Unknown);
            break;
        }
        if probe.pending.is_none() && tactical_complete(&game.events[events..]) {
            let left = &probe.script[probe.next..];
            probe.verdict = Some(if needless(left) {
                Check::Plays
            } else {
                Check::Refused {
                    step: probe.next,
                    reason: ACTION_OVER.to_owned(),
                }
            });
            break;
        }
        if result.finished || probe.next >= probe.script.len() {
            probe.verdict = Some(Check::Plays);
            break;
        }
        events = game.events.len();
    }
    drop(game);
    let verdict = probe.lock().expect("probe lock").verdict.clone();
    verdict.unwrap_or(Check::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::mpsc;
    use ti4_engine::decision_context::{DecisionContext, DecisionSource};
    use ti4_host::planning::steps::PlanStep;
    use ti4_model::state::{GameState, Phase};

    use crate::session::worker::Subscriber;

    fn shared() -> SessionShared {
        let ids = [PlayerId::new("a"), PlayerId::new("b")];
        let mut state = GameState::new(&ids, &[], BTreeMap::new(), None, 1);
        state.active = Some(PlayerId::new("a"));
        SessionShared::new("g".to_owned(), state)
    }

    fn listen(shared: &mut SessionShared, id: u64, seat: &str) -> mpsc::Receiver<ServerMessage> {
        let (tx, rx) = mpsc::sync_channel(8);
        let viewer = ViewerRole::Player(PlayerId::new(seat));
        shared.subscribers.insert(id, Subscriber { viewer, tx });
        rx
    }

    fn told(rx: &mpsc::Receiver<ServerMessage>) -> Option<DraftAppliedMsg> {
        match rx.try_recv() {
            Ok(ServerMessage::DraftApplied(message)) => Some(message),
            Ok(other) => panic!("another message: {other:?}"),
            Err(_) => None,
        }
    }

    /// A question of seat `player` with this subtype and these option ids.
    fn asked(player: &str, subtype: &str, options: &[&str]) -> Choice {
        let options = options
            .iter()
            .map(|id| ChoiceOption::new(*id, "x"))
            .collect();
        Choice::new(PlayerId::new(player), "a question", options).contextualized(
            DecisionContext::new(
                PlayerId::new(player),
                DecisionSource::Rule("test".to_owned()),
                subtype,
                Phase::Action,
                1,
            ),
        )
    }

    fn script() -> Vec<DraftStep> {
        vec![
            DraftStep::choose("tactical"),
            DraftStep::choose("18"),
            DraftStep::Step(PlanStep::DoneMoving),
        ]
    }

    #[test]
    fn a_script_answers_its_seat_and_is_told_only_to_that_seat_when_it_ends() {
        let mut shared = shared();
        let a = listen(&mut shared, 1, "a");
        let b = listen(&mut shared, 2, "b");
        let seat = PlayerId::new("a");
        // The first answer was given by the caller.
        shared.begin_script(&seat, "n1", script(), 1);
        assert_eq!(told(&a), None, "entries are left");

        let system = asked("a", "activate_system", &["18", "19"]);
        assert_eq!(
            shared.script_option(&system).map(|o| o.id),
            Some("18".into())
        );
        let moving = asked("a", "movement_step", &["done_moving"]);
        assert_eq!(
            shared.script_option(&moving).map(|o| o.id),
            Some("done_moving".into())
        );
        let end = told(&a).expect("the seat is told");
        assert_eq!((end.nonce.as_str(), end.stopped), ("n1", None));
        assert_eq!(told(&b), None, "another seat is told nothing");
        // The script is gone: the next question is of the player.
        assert_eq!(shared.script_option(&moving), None);
        assert_eq!(told(&a), None);
    }

    #[test]
    fn a_script_with_one_entry_is_played_by_its_first_answer() {
        let mut shared = shared();
        let a = listen(&mut shared, 1, "a");
        shared.begin_script(
            &PlayerId::new("a"),
            "n1",
            vec![DraftStep::choose("pass")],
            1,
        );
        assert_eq!(told(&a).expect("told").stopped, None);
        assert!(shared.scripts.is_empty());
    }
}
