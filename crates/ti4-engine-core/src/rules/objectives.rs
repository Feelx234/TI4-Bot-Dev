//! Objectives: the mutation half (package A4; string `objectives.rs`, `secrets.rs`, `GameState::reveal_objective`).
//!
//! The reads are `query::objectives`. Here the writes of the objective deck and the secret hand, each the body
//! of one string function over the slices it names: a public objective revealed (the next one, or the first of a
//! stage), a secret drawn, a score recorded (a public objective, a secret objective made public, a secret taken
//! from the hand): `kernel::fx::{score_public, score_secret}`, shared with the windows (areas do not import each
//! other); the questions are `rules::objective_windows`.
//!
//! What stays with the string callers: the price of a bought objective (`objectives::pay_for`, `secrets::pay_for`),
//! the leaders a score unlocks, the staged event of Yin's breakthrough, and the log (`docs/ENGINE_KNOWN_ISSUES.md`,
//! A4).

use crate::generated::PublicObjective;
use crate::query::objectives as q;
use crate::state::{CoreState, ObjectiveRef};

/// `GameState::reveal_objective` (81.2): the top facedown public objective turns faceup.
pub fn reveal_top(core: &mut CoreState) -> Option<PublicObjective> {
    let top = core.decks_mut().objective.draw()?;
    core.objectives_mut().revealed.push(ObjectiveRef::Public(top));
    Some(top)
}

/// `objectives::reveal_stage` (61.13, 61.14a): the first facedown objective of this stage, wherever it is in
/// the deck.
pub fn reveal_stage(core: &mut CoreState, stage: u8) -> Option<PublicObjective> {
    let at = core.decks().objective.cards().iter().position(|o| q::stage_of(*o) == Some(stage))?;
    let card = core.decks_mut().objective.remove(at);
    core.objectives_mut().revealed.push(ObjectiveRef::Public(card));
    Some(card)
}
