//! The core driver (`docs/MCTS_BOT_PLAN.md` 4.4): a whole game through [`Position`], every seat a [`SeatAgent`], the game's own streams
//! answering the dice ([`crate::Mode::Play`]). With the rule bot at every seat it is the product's game.

use crate::Outcome;
use crate::agent::{Answer, Applied, SeatAgent};
use crate::position::{Node, Position};

/// When a game is stopped unfinished.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_steps: usize,
    pub max_rounds: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_steps: 20_000, max_rounds: 30 }
    }
}

/// How a game ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    pub outcome: Outcome,
    pub steps: usize,
    pub decisions: usize,
    pub error: Option<String>,
}

/// One answered question of a recorded game: what a replay gives back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub seat: u8,
    pub answer: Answer,
}

/// Play `pos` on with `agents[seat]` answering the questions of each seat, to the end or the limits. `record` receives every answer
/// in order (a replay of it through [`replay`] is the same game). `observe`: every agent is told every answer and every roll (off for
/// agents that keep no tree: it costs a clone of the answer per seat).
pub fn play(pos: &mut Position, agents: &mut [Box<dyn SeatAgent>], limits: Limits, observe: bool, mut record: impl FnMut(&Position, &Record)) -> Ended {
    let mut error = None;
    loop {
        if pos.steps() >= limits.max_steps || pos.round() > limits.max_rounds {
            break;
        }
        match pos.node() {
            Node::End => break,
            Node::Failed(why) => {
                error = Some(why.to_owned());
                break;
            }
            Node::Chance(req) => {
                error = Some(format!("a chance request in play: {req:?} (a search position was handed to the driver)"));
                break;
            }
            Node::Decide(choice) => {
                let seat = choice.actor.0;
                let Some(agent) = agents.get_mut(usize::from(seat)) else {
                    error = Some(format!("no agent for seat {seat}"));
                    break;
                };
                let choice = choice.clone();
                let decision = match agent.decide(pos, &choice) {
                    Ok(d) => d,
                    Err(e) => {
                        error = Some(e);
                        break;
                    }
                };
                let answer = decision.answer.clone();
                let pick = if observe { pos.resolve(&answer) } else { None };
                if let Err(e) = pos.apply(decision) {
                    error = Some(format!("seat {seat}: {e}"));
                    break;
                }
                let rec = Record { seat, answer };
                record(pos, &rec);
                if observe {
                    let applied = Applied::Decision { seat, answer: rec.answer.clone(), choice: Box::new(choice), pick };
                    for a in agents.iter_mut() {
                        a.observe(pos, &applied);
                    }
                    for (request, faces) in pos.rolled().to_vec() {
                        let applied = Applied::Chance { request, faces };
                        for a in agents.iter_mut() {
                            a.observe(pos, &applied);
                        }
                    }
                }
            }
        }
    }
    Ended { outcome: pos.outcome(), steps: pos.steps(), decisions: pos.decisions(), error }
}

/// Replay recorded answers on `pos` (a fresh start of the same game): the same game, answer for answer.
///
/// # Errors
/// The first answer that is not taken, with its number.
pub fn replay(pos: &mut Position, records: &[Record]) -> Result<(), String> {
    for (i, r) in records.iter().enumerate() {
        match pos.node() {
            Node::Decide(choice) if choice.actor.0 == r.seat => {}
            other => return Err(format!("answer {i}: the game waits for {other:?}, the record is seat {}", r.seat)),
        }
        let result = match &r.answer {
            Answer::Pick(p) => pos.answer(*p),
            Answer::Index(k) => pos.answer_index(*k),
            Answer::Id(id) => pos.answer_id(id),
        };
        result.map_err(|e| format!("answer {i}: {e}"))?;
    }
    Ok(())
}
