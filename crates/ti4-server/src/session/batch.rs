//! Private, bounded engine replay for a staged workflow confirmation.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, DecisionRecord, IllegalChoice, Table};
use ti4_engine::decision_context::{DecisionContext, DecisionTarget};
use ti4_engine::game::Game;
use ti4_model::id::PlayerId;
use ti4_model::state::{GameState, Phase};

use super::SessionConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    pub request_id: String,
    pub expected_version: u64,
    pub nonce: String,
    pub plan: MovementPlan,
}

#[derive(Debug, Clone)]
pub struct MovementPlan {
    pub kind: BatchKind,
    pub destination: String,
    pub steps: Vec<MovementStep>,
}

// Reject cross-workflow steps at the request boundary, before entering private replay.
// The original movement request without a kind remains a supported request shape.
#[derive(Deserialize)]
#[serde(untagged)]
enum WirePlan {
    Typed(TypedPlan),
    Legacy(LegacyMovement),
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TypedPlan {
    TacticalMovement {
        destination: String,
        steps: Vec<MovementOnlyStep>,
    },
    Payment {
        steps: Vec<PaymentStep>,
    },
    AgendaVotePlanets {
        steps: Vec<VoteStep>,
    },
    Production {
        destination: String,
        steps: Vec<ProductionStep>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyMovement {
    destination: String,
    steps: Vec<MovementOnlyStep>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MovementOnlyStep {
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
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PaymentStep {
    Exhaust { planet: String },
    TradeGood,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum VoteStep {
    VotePlanet { planet: String },
    DoneVoting,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProductionStep {
    Produce { unit: String, count: u32 },
    DoneProducing,
}

impl<'de> Deserialize<'de> for MovementPlan {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let plan = WirePlan::deserialize(deserializer)?;
        Ok(match plan {
            WirePlan::Legacy(plan) => Self {
                kind: BatchKind::TacticalMovement,
                destination: plan.destination,
                steps: plan.steps.into_iter().map(Into::into).collect(),
            },
            WirePlan::Typed(TypedPlan::TacticalMovement { destination, steps }) => Self {
                kind: BatchKind::TacticalMovement,
                destination,
                steps: steps.into_iter().map(Into::into).collect(),
            },
            WirePlan::Typed(TypedPlan::Payment { steps }) => Self {
                kind: BatchKind::Payment,
                destination: String::new(),
                steps: steps.into_iter().map(Into::into).collect(),
            },
            WirePlan::Typed(TypedPlan::AgendaVotePlanets { steps }) => Self {
                kind: BatchKind::AgendaVotePlanets,
                destination: String::new(),
                steps: steps.into_iter().map(Into::into).collect(),
            },
            WirePlan::Typed(TypedPlan::Production { destination, steps }) => Self {
                kind: BatchKind::Production,
                destination,
                steps: steps.into_iter().map(Into::into).collect(),
            },
        })
    }
}

impl From<MovementOnlyStep> for MovementStep {
    fn from(step: MovementOnlyStep) -> Self {
        match step {
            MovementOnlyStep::Move {
                origin,
                unit,
                damaged,
            } => Self::Move {
                origin,
                unit,
                damaged,
            },
            MovementOnlyStep::Load {
                origin,
                unit,
                source,
                damaged,
            } => Self::Load {
                origin,
                unit,
                source,
                damaged,
            },
            MovementOnlyStep::DoneLoading => Self::DoneLoading,
            MovementOnlyStep::DoneMoving => Self::DoneMoving,
        }
    }
}
impl From<PaymentStep> for MovementStep {
    fn from(step: PaymentStep) -> Self {
        match step {
            PaymentStep::Exhaust { planet } => Self::Exhaust { planet },
            PaymentStep::TradeGood => Self::TradeGood,
        }
    }
}
impl From<VoteStep> for MovementStep {
    fn from(step: VoteStep) -> Self {
        match step {
            VoteStep::VotePlanet { planet } => Self::VotePlanet { planet },
            VoteStep::DoneVoting => Self::DoneVoting,
        }
    }
}
impl From<ProductionStep> for MovementStep {
    fn from(step: ProductionStep) -> Self {
        match step {
            ProductionStep::Produce { unit, count } => Self::Produce { unit, count },
            ProductionStep::DoneProducing => Self::DoneProducing,
        }
    }
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

impl MovementStep {
    fn subtype(&self) -> &'static str {
        match self {
            Self::Move { .. } | Self::DoneMoving => "movement_step",
            Self::Load { .. } | Self::DoneLoading => "load_cargo",
            Self::Exhaust { .. } | Self::TradeGood => "pay_resources",
            Self::VotePlanet { .. } | Self::DoneVoting => "vote_exhaust_planet",
            Self::Produce { .. } | Self::DoneProducing => "produce_unit",
        }
    }

    fn matches_option(&self, option: &ChoiceOption) -> bool {
        let value = |key: &str| option.payload.get(key).and_then(serde_json::Value::as_str);
        match self {
            Self::Move {
                origin,
                unit,
                damaged,
            } => {
                option.kind == "move"
                    && value("origin") == Some(origin.as_str())
                    && value("unit") == Some(unit.as_str())
                    && option
                        .payload
                        .get("damaged")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                        == *damaged
            }
            Self::Load {
                origin,
                unit,
                source,
                damaged,
            } => {
                option.kind == "load"
                    && value("system") == Some(origin.as_str())
                    && value("unit") == Some(unit.as_str())
                    && value("source") == source.as_deref()
                    && option
                        .payload
                        .get("damaged")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                        == *damaged
            }
            Self::DoneLoading => option.id == "done_loading",
            Self::DoneMoving => option.id == "done_moving",
            Self::Exhaust { planet } => {
                option.kind == "pay" && option.id == format!("exhaust|{planet}")
            }
            Self::TradeGood => option.kind == "pay" && option.id == "trade_good",
            Self::VotePlanet { planet } => option.kind == "vote_planet" && option.id == *planet,
            Self::DoneVoting => option.id == "decline",
            Self::DoneProducing => option.id == "done_producing",
            Self::Produce { unit, count } => {
                option.kind == "produce"
                    && value("unit") == Some(unit.as_str())
                    && option
                        .payload
                        .get("count")
                        .and_then(serde_json::Value::as_u64)
                        == Some(u64::from(*count))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchFailure {
    pub failed_step: usize,
    /// Stable machine-readable reason.
    pub reason: String,
    pub expected: String,
    /// Kinds of the options offered at the failed step (only for the batch's own seat).
    pub offered_summary: Vec<String>,
    /// Human-readable explanation of what went wrong and where.
    pub message: String,
    pub planned_steps: usize,
    /// The decision the engine offered instead of the planned step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offered: Option<Box<OfferedDecision>>,
}

/// What the engine asked at a failed step. Option ids and the prompt are only filled in when
/// the decision belongs to the batch's own seat, so another seat's private choice never leaks.
#[derive(Debug, Clone, Serialize)]
pub struct OfferedDecision {
    pub subtype: Option<String>,
    pub own_seat: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub option_ids: Vec<String>,
}

impl BatchFailure {
    fn new(step: usize, reason: impl Into<String>, expected: impl Into<String>) -> Self {
        let reason = reason.into();
        let expected = expected.into();
        Self {
            failed_step: step,
            message: format!("step {step}: {reason} (expected {expected})"),
            reason,
            expected,
            offered_summary: Vec::new(),
            planned_steps: 0,
            offered: None,
        }
    }

    fn with_planned_steps(mut self, planned_steps: usize) -> Self {
        self.planned_steps = planned_steps;
        self
    }
}

const MAX_REPORTED_OPTIONS: usize = 16;

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
    pub boundary_state: GameState,
}

impl Decider for PrivateDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let mut script = self.0.lock().expect("batch script lock");
        if let Some(record) = script.prefix.pop_front() {
            let matching: Vec<_> = choice
                .options
                .iter()
                .filter(|o| o.id == record.chosen)
                .collect();
            return if choice.player == record.player
                && choice.prompt == record.prompt
                && choice.ids()
                    == record
                        .offered
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                && choice.context == record.context
                && matching.len() == 1
            {
                Ok(matching[0].clone())
            } else {
                Err(IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "batch prefix diverged".into(),
                })
            };
        }
        while script.kind == BatchKind::TacticalMovement
            && matches!(
                script.steps.get(script.next),
                Some(MovementStep::DoneLoading)
            )
            && choice
                .context
                .as_ref()
                .is_some_and(|c| c.subtype == "movement_step")
        {
            script.next += 1;
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
        let subtype = step.subtype();
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
                .filter(|o| step.matches_option(o))
                .collect()
        } else {
            Vec::new()
        };
        let own_seat = choice.player == script.actor;
        let offered_subtype = choice.context.as_ref().map(|c| c.subtype.clone());
        // The engine takes a lone payment option without asking, so a payment can settle before
        // every planned step is consumed. Once the actor is no longer being asked to pay, the
        // remaining payment steps were already covered and the batch ends at this boundary.
        let payment_settled = script.kind == BatchKind::Payment
            && index > 0
            && !(own_seat
                && offered_subtype
                    .as_deref()
                    .is_some_and(|s| s == "pay_resources" || s == "pay_influence"))
            && script.steps[index..]
                .iter()
                .all(|s| matches!(s, MovementStep::Exhaust { .. } | MovementStep::TradeGood));
        if matches.len() != 1 && payment_settled {
            script.steps.truncate(index);
            script.finished = true;
            return Err(IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "batch boundary reached".into(),
            });
        }
        if matches.len() != 1 {
            let reason = if !context_ok || !own_seat {
                "workflow interrupted"
            } else if matches.is_empty() {
                "option unavailable"
            } else {
                "ambiguous option"
            };
            script.failure = Some(step_rejection(
                choice,
                own_seat,
                reason,
                index,
                expected,
                matches.len(),
                script.steps.len(),
            ));
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

/// Describes why the engine's offer could not take a planned step, without exposing another
/// seat's private options.
fn step_rejection(
    choice: &Choice,
    own_seat: bool,
    reason: &str,
    index: usize,
    expected: String,
    matching: usize,
    planned_steps: usize,
) -> BatchFailure {
    let subtype = choice.context.as_ref().map(|c| c.subtype.clone());
    let asked = match (&subtype, own_seat) {
        (Some(subtype), true) => format!("your {subtype} decision"),
        (Some(subtype), false) => format!("another seat's {subtype} decision"),
        (None, true) => format!("your decision \"{}\"", choice.prompt),
        (None, false) => "another seat's decision".to_owned(),
    };
    let detail = match reason {
        "workflow interrupted" => format!(
            "the engine moved on to {asked} before planned step {index} ({expected}) could be applied"
        ),
        "option unavailable" => format!(
            "planned step {index} ({expected}) is not among the {} options offered for {asked}",
            choice.options.len()
        ),
        _ => format!(
            "planned step {index} ({expected}) matches {matching} options offered for {asked}"
        ),
    };
    let own_options = |field: fn(&ChoiceOption) -> String| -> Vec<String> {
        if own_seat {
            choice
                .options
                .iter()
                .take(MAX_REPORTED_OPTIONS)
                .map(field)
                .collect()
        } else {
            Vec::new()
        }
    };
    BatchFailure {
        failed_step: index,
        reason: reason.into(),
        message: format!("{reason}: {detail}"),
        expected,
        offered_summary: own_options(|o| o.kind.clone()),
        planned_steps,
        offered: Some(Box::new(OfferedDecision {
            subtype,
            own_seat,
            prompt: own_seat.then(|| choice.prompt.clone()),
            option_ids: own_options(|o| o.id.clone()),
        })),
    }
}

/// Replays the prefix and proves every staged choice against a fresh engine offer.
pub fn simulate(
    config: &SessionConfig,
    prefix: &[DecisionRecord],
    actor: &PlayerId,
    plan: &MovementPlan,
) -> Result<Simulation, BatchFailure> {
    if let Some(problem) = plan_problem(plan) {
        let mut failure =
            BatchFailure::new(0, "invalid batch plan", "valid workflow steps required")
                .with_planned_steps(plan.steps.len());
        failure.message = format!("invalid batch plan: {problem}");
        return Err(failure);
    }
    simulate_script(config, prefix, actor, plan).map_err(|failure| {
        let planned = plan.steps.len();
        if failure.planned_steps == 0 {
            failure.with_planned_steps(planned)
        } else {
            failure
        }
    })
}

/// Explains why a plan cannot be a batch of its kind, or `None` when its shape is valid.
fn plan_problem(plan: &MovementPlan) -> Option<String> {
    if plan.steps.is_empty() {
        return Some("the plan has no steps".to_owned());
    }
    if plan.steps.len() > 100 {
        return Some(format!(
            "the plan has {} steps; at most 100 are allowed",
            plan.steps.len()
        ));
    }
    let body = &plan.steps[..plan.steps.len() - 1];
    let last = plan.steps.last();
    let destination_needed = matches!(
        plan.kind,
        BatchKind::TacticalMovement | BatchKind::Production
    );
    if destination_needed && (plan.destination.is_empty() || plan.destination.len() > 64) {
        return Some("a destination system id of 1 to 64 characters is required".to_owned());
    }
    if !destination_needed && !plan.destination.is_empty() {
        return Some(format!("{:?} plans must not name a destination", plan.kind));
    }
    let problem = match plan.kind {
        BatchKind::TacticalMovement => (!matches!(last, Some(MovementStep::DoneMoving)))
            .then_some("a movement plan must end with done_moving")
            .or_else(|| {
                (!body.iter().all(|s| {
                    matches!(
                        s,
                        MovementStep::Move { .. } | MovementStep::Load { .. } | MovementStep::DoneLoading
                    )
                }))
                .then_some("a movement plan may only contain move, load and done_loading before done_moving")
            }),
        BatchKind::Payment => (!plan
            .steps
            .iter()
            .all(|s| matches!(s, MovementStep::Exhaust { .. } | MovementStep::TradeGood)))
        .then_some("a payment plan may only contain exhaust and trade_good steps"),
        BatchKind::AgendaVotePlanets => (!plan
            .steps
            .iter()
            .all(|s| matches!(s, MovementStep::VotePlanet { .. } | MovementStep::DoneVoting)))
        .then_some("a vote plan may only contain vote_planet and done_voting steps")
        .or_else(|| {
            body.iter()
                .any(|s| matches!(s, MovementStep::DoneVoting))
                .then_some("done_voting may only be the last step")
        }),
        BatchKind::Production => (!plan.steps.iter().all(|s| {
            matches!(
                s,
                MovementStep::Produce { count: 1..=100, .. } | MovementStep::DoneProducing
            )
        }))
        .then_some("a production plan may only contain produce steps with a count of 1 to 100 and done_producing")
        .or_else(|| {
            body.iter()
                .any(|s| matches!(s, MovementStep::DoneProducing))
                .then_some("done_producing may only be the last step")
        }),
    };
    problem.map(str::to_owned)
}

/// Reconstruct the view at the first unrecorded choice when a session is recovered
/// without the in-memory boundary state from a batch simulation.
pub fn replay_boundary_state(
    config: &SessionConfig,
    prefix: &[DecisionRecord],
) -> Result<GameState, BatchFailure> {
    let plan = MovementPlan {
        kind: BatchKind::TacticalMovement,
        destination: String::new(),
        steps: Vec::new(),
    };
    let actor = prefix
        .last()
        .map(|record| record.player.clone())
        .ok_or_else(|| BatchFailure::new(0, "empty replay", "recorded prefix"))?;
    simulate_script(config, prefix, &actor, &plan).map(|simulation| simulation.boundary_state)
}

fn simulate_script(
    config: &SessionConfig,
    prefix: &[DecisionRecord],
    actor: &PlayerId,
    plan: &MovementPlan,
) -> Result<Simulation, BatchFailure> {
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
                || game.table.log.records.len() != prefix.len() + guard.steps.len()
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
                boundary_state: game.state.clone(),
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

    #[test]
    fn request_schemas_reject_steps_from_other_baskets() {
        for (kind, step) in [
            (
                "payment",
                "{\"kind\":\"produce\",\"unit\":\"fighter\",\"count\":1}",
            ),
            ("agenda_vote_planets", "{\"kind\":\"trade_good\"}"),
            (
                "production",
                "{\"kind\":\"vote_planet\",\"planet\":\"jord\"}",
            ),
            (
                "tactical_movement",
                "{\"kind\":\"exhaust\",\"planet\":\"jord\"}",
            ),
        ] {
            let destination = if kind == "production" || kind == "tactical_movement" {
                ",\"destination\":\"22\""
            } else {
                ""
            };
            let json = format!("{{\"kind\":\"{kind}\"{destination},\"steps\":[{step}]}}");
            assert!(
                serde_json::from_str::<MovementPlan>(&json).is_err(),
                "{json}"
            );
        }
        let legacy = serde_json::from_str::<MovementPlan>(
            r#"{"destination":"22","steps":[{"kind":"done_moving"}]}"#,
        )
        .unwrap();
        assert_eq!(legacy.kind, BatchKind::TacticalMovement);
        let payment = serde_json::from_str::<MovementPlan>(
            r#"{"kind":"payment","steps":[{"kind":"trade_good"}]}"#,
        )
        .unwrap();
        assert!(matches!(payment.steps[0], MovementStep::TradeGood));
    }

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
    fn payment_ends_at_the_boundary_when_the_engine_settles_the_rest() {
        // Jord pays 2 of 3; the engine then spends the lone remaining trade good itself and
        // moves on, so the planned trade good step must not reject the batch.
        let mut decider = decider(
            BatchKind::Payment,
            vec![
                MovementStep::Exhaust {
                    planet: "jord".into(),
                },
                MovementStep::TradeGood,
            ],
        );
        let pay = offered(
            "pay_influence",
            vec![
                ChoiceOption::labelled("exhaust|jord", "pay", "exhaust"),
                ChoiceOption::labelled("trade_good", "pay", "spend"),
            ],
        );
        assert_eq!(decider.choose(&pay).unwrap().id, "exhaust|jord");
        let next = offered(
            "gain_command_token",
            vec![ChoiceOption::labelled("tactic", "pool", "tactic pool")],
        );
        assert!(decider.choose(&next).is_err());
        let script = decider.0.lock().unwrap();
        assert!(script.finished);
        assert!(script.failure.is_none());
        assert_eq!(script.steps.len(), 1);
    }

    #[test]
    fn payment_that_never_starts_still_reports_the_interruption() {
        let mut decider = decider(BatchKind::Payment, vec![MovementStep::TradeGood]);
        let next = offered(
            "gain_command_token",
            vec![ChoiceOption::labelled("tactic", "pool", "tactic pool")],
        );
        assert!(decider.choose(&next).is_err());
        let script = decider.0.lock().unwrap();
        let failure = script.failure.as_ref().unwrap();
        assert_eq!(failure.reason, "workflow interrupted");
        let offered = failure.offered.as_ref().unwrap();
        assert_eq!(offered.subtype.as_deref(), Some("gain_command_token"));
        assert_eq!(offered.option_ids, vec!["tactic".to_owned()]);
        assert!(
            failure.message.contains("gain_command_token"),
            "{}",
            failure.message
        );
    }

    #[test]
    fn prefix_rejects_an_offer_change_before_consuming_a_planned_step() {
        let pay = offered(
            "pay_resources",
            vec![ChoiceOption::labelled("trade_good", "pay", "spend")],
        );
        let mut script = decider(BatchKind::Payment, vec![MovementStep::TradeGood]);
        script.0.lock().unwrap().prefix.push_back(DecisionRecord {
            player: pay.player.clone(),
            prompt: pay.prompt.clone(),
            chosen: "trade_good".into(),
            offered: vec!["trade_good".into()],
            context: pay.context.clone(),
        });
        let mut changed = pay;
        changed.options.push(ChoiceOption::decline());
        assert!(script.choose(&changed).is_err());
        assert_eq!(script.0.lock().unwrap().next, 0);
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
