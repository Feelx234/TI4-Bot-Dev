//! Remote human decider implementation connecting engine decision sites to client channels.

use std::sync::{Arc, Mutex, mpsc};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_model::id::PlayerId;

use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::server::ActionAcceptedMsg;
use crate::protocol::status::RejectionReason;
use crate::session::worker::{PendingDecision, SessionShared};

/// Incoming choice submission from a client.
#[derive(Debug)]
pub struct ChoiceSubmission {
    pub seat: PlayerId,
    pub nonce: String,
    pub expected_version: u64,
    pub option_id: String,
    pub reply_tx: mpsc::Sender<Result<ActionAcceptedMsg, RejectionReason>>,
}

/// An engine decider that routes choices to a remote human player.
///
/// It generates an opaque nonce, records the pending choice on the session,
/// notifies connected viewers, and blocks on its per-seat inbox until an answer
/// is submitted and validated against the engine-offered options.
pub struct RemoteHumanDecider {
    seat: PlayerId,
    game_id: String,
    shared: Arc<Mutex<SessionShared>>,
    inbox_rx: mpsc::Receiver<ChoiceSubmission>,
}

impl RemoteHumanDecider {
    #[must_use]
    pub fn new(
        seat: PlayerId,
        game_id: String,
        shared: Arc<Mutex<SessionShared>>,
        inbox_rx: mpsc::Receiver<ChoiceSubmission>,
    ) -> Self {
        Self {
            seat,
            game_id,
            shared,
            inbox_rx,
        }
    }
}

impl Decider for RemoteHumanDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let nonce = format!("{:016x}", rand::random::<u64>());

        let current_version = {
            let mut shared = self.shared.lock().expect("session shared lock");
            shared.game_version += 1;
            let v = shared.game_version;
            shared.pending_decision = Some(PendingDecision {
                seat: self.seat.clone(),
                nonce: nonce.clone(),
                game_version: v,
                choice: choice.clone(),
            });
            shared.record_and_broadcast_event(
                format!("Decision required for {}: {}", choice.player, choice.prompt),
                "decision",
                Some(v),
            );
            shared.broadcast_pending_decision(choice, &nonce);
            v
        };

        while let Ok(submission) = self.inbox_rx.recv() {
            // If the session was stopped externally, abort cleanly
            if self.shared.lock().expect("shared lock").stopped {
                return Err(IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "session was stopped".to_owned(),
                });
            }

            // 1. Validate version match
            if submission.expected_version != current_version {
                let _ = submission.reply_tx.send(Err(RejectionReason::StaleVersion {
                    expected: submission.expected_version,
                    current: current_version,
                }));
                continue;
            }

            // 2. Validate nonce match
            if submission.nonce != nonce {
                let _ = submission.reply_tx.send(Err(RejectionReason::StaleNonce));
                continue;
            }

            // 3. Validate option is one of the legal choices offered by the engine
            if let Some(opt) = choice.options.iter().find(|o| o.id == submission.option_id) {
                let accepted = ActionAcceptedMsg {
                    protocol_version: PROTOCOL_VERSION,
                    game_id: self.game_id.clone(),
                    game_version: current_version,
                    option_id: submission.option_id,
                };
                let _ = submission.reply_tx.send(Ok(accepted));

                // Clear pending decision in shared state
                {
                    let mut shared = self.shared.lock().expect("shared lock");
                    shared.pending_decision = None;
                }

                return Ok(opt.clone());
            }

            // Option ID not in the offered set
            let _ = submission
                .reply_tx
                .send(Err(RejectionReason::UnknownOption {
                    option_id: submission.option_id,
                }));
        }

        // Channel disconnected
        Err(IllegalChoice::DeciderFailed {
            player: choice.player.clone(),
            prompt: choice.prompt.clone(),
            reason: "inbox channel closed".to_owned(),
        })
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let _ = seen;
        self.choose(choice)
    }
}
