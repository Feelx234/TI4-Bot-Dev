//! Private, bounded engine replay for a single tactical movement confirmation.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, DecisionRecord, IllegalChoice, Table};
use ti4_engine::decision_context::{DecisionContext, DecisionTarget};
use ti4_engine::game::Game;
use ti4_model::id::PlayerId;
use ti4_model::state::Phase;

use super::SessionConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    pub request_id: String,
    pub expected_version: u64,
    pub nonce: String,
    pub plan: MovementPlan,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementPlan {
    #[serde(default)]
    pub kind: BatchKind,
    #[serde(default)]
    pub destination: String,
    pub steps: Vec<MovementStep>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchKind {
    #[default]
    TacticalMovement,
    Payment,
    AgendaVotePlanets,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MovementStep {
    Move {
        origin: String,
        unit: String,
        damaged: bool,
    },
    Load {
        origin: String,
        unit: String,
        source: Option<String>,
        damaged: bool,
    },
    DoneLoading,
    DoneMoving,
    Exhaust {
        planet: String,
    },
    TradeGood,
    VotePlanet {
        planet: String,
    },
    DoneVoting,
    Produce {
        unit: String,
        count: u32,
    },
    DoneProducing,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchFailure {
    pub failed_step: usize,
    pub reason: String,
    pub expected: String,
    pub offered_summary: Vec<String>,
}

impl BatchFailure {
    fn new(step: usize, reason: impl Into<String>, expected: impl Into<String>) -> Self {
        Self {
            failed_step: step,
            reason: reason.into(),
            expected: expected.into(),
            offered_summary: Vec::new(),
        }
    }
}

struct Script {
    prefix: VecDeque<DecisionRecord>,
    steps: Vec<MovementStep>,
    next: usize,
    actor: PlayerId,
    destination: String,
    kind: BatchKind,
    payment_subtype: Option<String>,
    workflow_context: Option<DecisionContext>,
    selected: Vec<ChoiceOption>,
    failure: Option<BatchFailure>,
    finished: bool,
}

struct PrivateDecider(Arc<Mutex<Script>>);

pub struct Simulation {
    pub decisions: Vec<DecisionRecord>,
    pub selected: Vec<ChoiceOption>,
    pub transitions: Vec<(usize, Phase, u32)>,
    pub winner: Option<Option<PlayerId>>,
}

impl Decider for PrivateDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let mut script = self.0.lock().expect("batch script lock");
        if let Some(record) = script.prefix.pop_front() {
            return choice
                .options
                .iter()
                .find(|o| o.id == record.chosen && choice.player == record.player)
                .cloned()
                .ok_or_else(|| IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "batch prefix diverged".into(),
                });
        }
        if script.next == script.steps.len() {
            script.finished = true;
            return Err(IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "batch boundary reached".into(),
            });
        }
        let index = script.next;
        let step = &script.steps[index];
        let expected = format!("{step:?}");
        let subtype = match step {
            MovementStep::Move { .. } | MovementStep::DoneMoving => "movement_step",
            MovementStep::Load { .. } | MovementStep::DoneLoading => "load_cargo",
            MovementStep::Exhaust { .. } | MovementStep::TradeGood => match script.kind {
                BatchKind::Payment => "pay_resources", // influence is accepted below
                _ => "invalid",
            },
            MovementStep::VotePlanet { .. } | MovementStep::DoneVoting => "vote_exhaust_planet",
            MovementStep::Produce { .. } | MovementStep::DoneProducing => "produce_unit",
        };
        let context_ok = choice.context.as_ref().is_some_and(|c| {
            c.actor == script.actor && (c.subtype == subtype ||
                (script.kind == BatchKind::Payment && c.subtype == "pay_influence" && subtype == "pay_resources")) &&
                script.payment_subtype.as_ref().is_none_or(|first| *first == c.subtype) &&
                (script.kind == BatchKind::TacticalMovement || script.workflow_context.as_ref().is_none_or(|first| {
                    first.source == c.source && first.phase == c.phase && first.round == c.round
                        && first.target == c.target && first.subtype == c.subtype
                        && first.outstanding.iter().all(|owed| c.outstanding.iter().any(|next| {
                            next.kind == owed.kind && next.amount == owed.amount && next.paid >= owed.paid
                        }))
                })) &&
                (if let MovementStep::Load { origin, .. } = step {
                    matches!(&c.target, Some(DecisionTarget::System(id)) if id.as_str() == origin)
                } else if script.kind == BatchKind::Production {
                    matches!(&c.target, Some(DecisionTarget::System(id)) if id.as_str() == script.destination)
                } else if subtype == "load_cargo" || script.kind != BatchKind::TacticalMovement { true } else {
                    c.target.as_ref().is_none_or(|target| matches!(target, DecisionTarget::System(id) if id.as_str() == script.destination))
                })
        });
        let matches: Vec<_> = if choice.player == script.actor && context_ok {
            choice
                .options
                .iter()
                .filter(|o| match step {
                    MovementStep::Move {
                        origin,
                        unit,
                        damaged,
                    } => {
                        o.kind == "move"
                            && o.payload.get("origin").and_then(serde_json::Value::as_str)
                                == Some(origin.as_str())
                            && o.payload.get("unit").and_then(serde_json::Value::as_str)
                                == Some(unit.as_str())
                            && o.payload
                                .get("damaged")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false)
                                == *damaged
                    }
                    MovementStep::Load {
                        origin,
                        unit,
                        source,
                        damaged,
                    } => {
                        o.kind == "load"
                            && o.payload.get("system").and_then(serde_json::Value::as_str)
                                == Some(origin.as_str())
                            && o.payload.get("unit").and_then(serde_json::Value::as_str)
                                == Some(unit.as_str())
                            && o.payload.get("source").and_then(serde_json::Value::as_str)
                                == source.as_deref()
                            && o.payload
                                .get("damaged")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false)
                                == *damaged
                    }
                    MovementStep::DoneLoading => o.id == "done_loading",
                    MovementStep::DoneMoving => o.id == "done_moving",
                    MovementStep::Exhaust { planet } => {
                        o.kind == "pay" && o.id == format!("exhaust|{planet}")
                    }
                    MovementStep::TradeGood => o.kind == "pay" && o.id == "trade_good",
                    MovementStep::VotePlanet { planet } => {
                        o.kind == "vote_planet" && o.id == *planet
                    }
                    MovementStep::DoneVoting => o.id == "decline",
                    MovementStep::DoneProducing => o.id == "done_producing",
                    MovementStep::Produce { unit, count } => {
                        o.kind == "produce"
                            && o.payload.get("unit").and_then(serde_json::Value::as_str)
                                == Some(unit.as_str())
                            && o.payload.get("count").and_then(serde_json::Value::as_u64)
                                == Some(u64::from(*count))
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        if matches.len() != 1 {
            script.failure = Some(BatchFailure {
                failed_step: index,
                reason: if !context_ok || choice.player != script.actor {
                    "workflow interrupted"
                } else if matches.is_empty() {
                    "option unavailable"
                } else {
                    "ambiguous option"
                }
                .into(),
                expected,
                offered_summary: if choice.player == script.actor {
                    choice
                        .options
                        .iter()
                        .take(16)
                        .map(|o| o.kind.clone())
                        .collect()
                } else {
                    Vec::new()
                },
            });
            return Err(IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "batch step rejected".into(),
            });
        }
        let selected = (*matches[0]).clone();
        if script.kind == BatchKind::Payment && script.payment_subtype.is_none() {
            script.payment_subtype = choice.context.as_ref().map(|c| c.subtype.clone());
        }
        if script.workflow_context.is_none() {
            script.workflow_context = choice.context.clone();
        }
        script.selected.push(selected.clone());
        script.next += 1;
        Ok(selected)
    }
}

/// Replays the prefix and proves every staged choice against a fresh engine offer.
pub fn simulate(
    config: &SessionConfig,
    prefix: &[DecisionRecord],
    actor: &PlayerId,
    plan: &MovementPlan,
) -> Result<Simulation, BatchFailure> {
    let valid = match plan.kind {
        BatchKind::TacticalMovement => {
            !plan.destination.is_empty()
                && plan.destination.len() <= 64
                && matches!(plan.steps.last(), Some(MovementStep::DoneMoving))
                && plan.steps[..plan.steps.len().saturating_sub(1)]
                    .iter()
                    .all(|s| {
                        matches!(
                            s,
                            MovementStep::Move { .. }
                                | MovementStep::Load { .. }
                                | MovementStep::DoneLoading
                        )
                    })
        }
        BatchKind::Payment => {
            plan.destination.is_empty()
                && plan
                    .steps
                    .iter()
                    .all(|s| matches!(s, MovementStep::Exhaust { .. } | MovementStep::TradeGood))
        }
        BatchKind::AgendaVotePlanets => {
            plan.destination.is_empty()
                && plan.steps.iter().all(|s| {
                    matches!(
                        s,
                        MovementStep::VotePlanet { .. } | MovementStep::DoneVoting
                    )
                })
                && plan.steps[..plan.steps.len().saturating_sub(1)]
                    .iter()
                    .all(|s| !matches!(s, MovementStep::DoneVoting))
        }
        BatchKind::Production => {
            !plan.destination.is_empty()
                && plan.destination.len() <= 64
                && plan.steps.iter().all(|s| {
                    matches!(
                        s,
                        MovementStep::Produce { count: 1..=100, .. } | MovementStep::DoneProducing
                    )
                })
                && plan.steps[..plan.steps.len().saturating_sub(1)]
                    .iter()
                    .all(|s| !matches!(s, MovementStep::DoneProducing))
        }
    };
    if !valid || plan.steps.is_empty() || plan.steps.len() > 100 {
        return Err(BatchFailure::new(
            0,
            "invalid batch plan",
            "valid workflow steps required",
        ));
    }
    let script = Arc::new(Mutex::new(Script {
        prefix: prefix.iter().cloned().collect(),
        steps: plan.steps.clone(),
        next: 0,
        actor: actor.clone(),
        destination: plan.destination.clone(),
        kind: plan.kind,
        payment_subtype: None,
        workflow_context: None,
        selected: Vec::new(),
        failure: None,
        finished: false,
    }));
    let table = Table::with_default(Box::new(PrivateDecider(script.clone())));
    let mut game = Game::with_table(config.state.clone(), ContentStore::embedded(), table);
    if let Some(galaxy) = &config.galaxy {
        game = game.with_galaxy(galaxy.clone());
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut phase = game.state.phase;
    let mut round = game.state.round;
    let mut transitions = Vec::new();
    for _ in 0..(prefix
        .len()
        .saturating_add(plan.steps.len())
        .saturating_mul(8)
        .saturating_add(128))
    {
        if Instant::now() >= deadline {
            return Err(BatchFailure::new(
                script.lock().expect("script").next,
                "simulation timed out",
                "next engine choice",
            ));
        }
        let result = game.step();
        if game.state.phase != phase || game.state.round != round {
            phase = game.state.phase;
            round = game.state.round;
            if game.table.log.records.len() > prefix.len() {
                transitions.push((game.table.log.records.len(), phase, round));
            }
        }
        let guard = script.lock().expect("script");
        if let Some(failure) = &guard.failure {
            return Err(failure.clone());
        }
        if let Some(error) = &result.error
            && !(guard.finished && error.to_string().contains("batch boundary reached"))
        {
            return Err(BatchFailure::new(
                guard.next,
                format!("engine error: {error}"),
                "next planned choice",
            ));
        }
        if guard.finished || result.finished {
            if guard.next != guard.steps.len() || !guard.prefix.is_empty() {
                return Err(BatchFailure::new(
                    guard.next,
                    "game ended before plan completed",
                    "next planned choice",
                ));
            }
            if game.table.log.records.get(..prefix.len()) != Some(prefix)
                || game.table.log.records.len() != prefix.len() + plan.steps.len()
            {
                return Err(BatchFailure::new(0, "replay diverged", "recorded prefix"));
            }
            let winner = (game.state.finished || result.finished)
                .then(|| {
                    game.state
                        .players
                        .iter()
                        .max_by_key(|player| player.victory_points)
                        .map(|player| player.id.clone())
                })
                .flatten();
            return Ok(Simulation {
                decisions: game.table.log.records[prefix.len()..].to_vec(),
                selected: guard.selected.clone(),
                transitions,
                winner: (game.state.finished || result.finished).then_some(winner),
            });
        }
        if let Some(err) = result.error {
            return Err(BatchFailure::new(
                guard.next,
                format!("engine error: {err}"),
                "next planned choice",
            ));
        }
    }
    Err(BatchFailure::new(
        0,
        "simulation step limit exceeded",
        "next engine choice",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine::decision_context::{DecisionContext, DecisionSource};

    fn offered(subtype: &str, options: Vec<ChoiceOption>) -> Choice {
        let player = PlayerId::new("p1");
        Choice::new(player.clone(), "basket", options).contextualized(DecisionContext::new(
            player,
            DecisionSource::Rule("test".into()),
            subtype,
            Phase::Action,
            1,
        ))
    }

    fn decider(kind: BatchKind, steps: Vec<MovementStep>) -> PrivateDecider {
        PrivateDecider(Arc::new(Mutex::new(Script {
            prefix: VecDeque::new(),
            steps,
            next: 0,
            actor: PlayerId::new("p1"),
            destination: String::new(),
            kind,
            payment_subtype: None,
            workflow_context: None,
            selected: Vec::new(),
            failure: None,
            finished: false,
        })))
    }

    #[test]
    fn payment_checks_each_repeated_trade_good_against_a_fresh_offer() {
        let mut decider = decider(
            BatchKind::Payment,
            vec![MovementStep::TradeGood, MovementStep::TradeGood],
        );
        let pay = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("trade_good", "pay", "spend")],
        );
        assert_eq!(decider.choose(&pay).unwrap().id, "trade_good");
        let gone = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("exhaust|jord", "pay", "exhaust")],
        );
        assert!(decider.choose(&gone).is_err());
        let script = decider.0.lock().unwrap();
        assert_eq!(script.next, 1);
        assert_eq!(script.failure.as_ref().unwrap().failed_step, 1);
    }

    #[test]
    fn payment_cannot_change_currency_mid_batch() {
        let mut decider = decider(
            BatchKind::Payment,
            vec![MovementStep::TradeGood, MovementStep::TradeGood],
        );
        let pay = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("trade_good", "pay", "spend")],
        );
        decider.choose(&pay).unwrap();
        assert!(
            decider
                .choose(&offered("pay_influence", pay.options))
                .is_err()
        );
        assert_eq!(
            decider.0.lock().unwrap().failure.as_ref().unwrap().reason,
            "workflow interrupted"
        );
    }

    #[test]
    fn payment_cannot_spill_into_a_different_payment_offer() {
        let mut decider = decider(
            BatchKind::Payment,
            vec![MovementStep::TradeGood, MovementStep::TradeGood],
        );
        let pay = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("trade_good", "pay", "spend")],
        );
        decider.choose(&pay).unwrap();
        let mut next_bill = pay.clone();
        next_bill.context.as_mut().unwrap().source = DecisionSource::Rule("different bill".into());
        assert!(decider.choose(&next_bill).is_err());
        assert_eq!(
            decider.0.lock().unwrap().failure.as_ref().unwrap().reason,
            "workflow interrupted"
        );
    }

    #[test]
    fn late_mismatch_reports_step_fifty_without_consuming_it() {
        let mut decider = decider(BatchKind::Payment, vec![MovementStep::TradeGood; 50]);
        let pay = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("trade_good", "pay", "spend")],
        );
        for _ in 0..49 {
            decider.choose(&pay).unwrap();
        }
        let unavailable = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("exhaust|jord", "pay", "exhaust")],
        );
        assert!(decider.choose(&unavailable).is_err());
        let script = decider.0.lock().unwrap();
        assert_eq!(script.failure.as_ref().unwrap().failed_step, 49);
        assert_eq!(script.selected.len(), 49);
    }

    #[test]
    fn vote_stops_before_another_seat_answers() {
        let mut decider = decider(
            BatchKind::AgendaVotePlanets,
            vec![MovementStep::VotePlanet {
                planet: "jord".into(),
            }],
        );
        let choice = offered(
            "vote_exhaust_planet",
            vec![ChoiceOption::labelled("jord", "vote_planet", "vote")],
        );
        let mut other = choice.clone();
        other.player = PlayerId::new("p2");
        assert!(decider.choose(&other).is_err());
        assert_eq!(
            decider
                .0
                .lock()
                .unwrap()
                .failure
                .as_ref()
                .unwrap()
                .failed_step,
            0
        );
    }
}
