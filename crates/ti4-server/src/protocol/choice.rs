//! Protocol types for choices, options, and decision context.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ti4_engine::choice::{Choice, ChoiceOption};
use ti4_engine::decision_context::{
    ConstraintKind, DecisionContext, DecisionSource, DecisionTarget, OutstandingConstraint,
};
use ti4_model::id::PlayerId;
use ti4_model::state::Phase;

/// One engine-offered option presented to the actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOptionDto {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub payload: BTreeMap<String, Value>,
}

impl From<&ChoiceOption> for ChoiceOptionDto {
    fn from(opt: &ChoiceOption) -> Self {
        Self {
            id: opt.id.clone(),
            kind: opt.kind.clone(),
            label: opt.label.clone(),
            payload: opt.payload.clone(),
        }
    }
}

/// Outstanding transaction constraint visible only to the actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutstandingConstraintDto {
    pub kind: ConstraintKind,
    pub amount: i64,
    pub paid: i64,
}

impl From<&OutstandingConstraint> for OutstandingConstraintDto {
    fn from(c: &OutstandingConstraint) -> Self {
        Self {
            kind: c.kind,
            amount: c.amount,
            paid: c.paid,
        }
    }
}

/// Redacted decision context describing the question being put to the seat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionContextDto {
    pub version: u16,
    pub actor: PlayerId,
    pub source: DecisionSource,
    pub subtype: String,
    pub phase: Phase,
    pub round: u32,
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<DecisionTarget>,
    /// Outstanding constraints in flight. Populated ONLY for the actor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outstanding: Vec<OutstandingConstraintDto>,
}

impl DecisionContextDto {
    /// Constructs context projection. If `is_actor` is false, `outstanding` is strictly empty.
    #[must_use]
    pub fn from_engine(ctx: &DecisionContext, is_actor: bool) -> Self {
        let outstanding = if is_actor {
            ctx.outstanding
                .iter()
                .map(OutstandingConstraintDto::from)
                .collect()
        } else {
            Vec::new()
        };

        Self {
            version: ctx.version,
            actor: ctx.actor.clone(),
            source: ctx.source.clone(),
            subtype: ctx.subtype.clone(),
            phase: ctx.phase,
            round: ctx.round,
            optional: ctx.optional,
            target: ctx.target.clone(),
            outstanding,
        }
    }
}

/// A pending choice issued to the active seat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingChoiceDto {
    /// Opaque nonce identifying this specific decision opportunity.
    pub nonce: String,
    /// Seat expected to decide.
    pub actor: PlayerId,
    /// Player-facing prompt text.
    pub prompt: String,
    /// Engine-generated legal options.
    pub options: Vec<ChoiceOptionDto>,
    /// Structured decision context and outstanding constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<DecisionContextDto>,
}

impl PendingChoiceDto {
    /// Constructs a pending choice DTO from an engine `Choice`.
    #[must_use]
    pub fn from_choice(choice: &Choice, nonce: String, is_actor: bool) -> Self {
        Self {
            nonce,
            actor: choice.player.clone(),
            prompt: choice.prompt.clone(),
            options: choice.options.iter().map(ChoiceOptionDto::from).collect(),
            context: choice
                .context
                .as_ref()
                .map(|ctx| DecisionContextDto::from_engine(ctx, is_actor)),
        }
    }
}
