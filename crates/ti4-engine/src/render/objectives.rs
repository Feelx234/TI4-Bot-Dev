//! Renderer of the `objectives` area: `impl Family` for `ti4_engine_core::choice::objectives` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package A4 (docs/ENGINE_CORE_STATE.md 4.4). Ported: "score an objective" (`objectives::ScoringWindow::
//! pending_choice`: prompt `score an objective`, subtype `score_objective` in the status window and
//! `score_secret_objective` in an event window, options of kind `score` labelled with the objective id, then the
//! generic decline) and "return a secret objective to the deck" (`secrets::enforce_hand_limit`: subtype
//! `return_over_secret_hand_limit`, options of kind `return`). The previews (one more point, one secret less)
//! are display facts and not part of a choice's identity.

use ti4_engine_core::choice::objectives::{Ask, Opt, Subtype};
use ti4_engine_core::generated::{PublicObjective, SecretObjective};
use ti4_engine_core::state::ObjectiveRef;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;
use crate::preview::{Delta, Preview, Quantity};

/// `objectives::SCORE_KIND`.
pub const SCORE_KIND: &str = "score";
/// `secrets::RETURN_KIND`.
pub const RETURN_KIND: &str = "return";
/// `objectives::VICTORY_TARGET`.
const TARGET: i64 = ti4_engine_core::kernel::fx::VICTORY_TARGET as i64;

/// The id text of an objective.
#[must_use]
pub fn objective_text(o: ObjectiveRef) -> &'static str {
    match o {
        ObjectiveRef::Public(p) => p.as_str(),
        ObjectiveRef::Secret(s) => s.as_str(),
    }
}

/// The renderer of the objective questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::Score { .. } => "score an objective".to_owned(),
            Ask::ReturnSecret { .. } => "return a secret objective to the deck".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::ScoreObjective => "score_objective",
            Subtype::ScoreSecretObjective => "score_secret_objective",
            Subtype::ReturnOverSecretHandLimit => "return_over_secret_hand_limit",
        }
        .to_owned()
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            (Ask::Score { vp }, Opt::Score(obj)) => {
                let id = objective_text(*obj);
                let vp = i64::from(*vp);
                ChoiceOption::labelled(id, SCORE_KIND, id)
                    .previewed(Preview::certain(vec![Delta::new(Quantity::VictoryPoints, vp, (vp + 1).min(TARGET))]))
            }
            (Ask::ReturnSecret { held }, Opt::Return(s)) => {
                let id = s.as_str();
                let before = i64::from(*held);
                ChoiceOption::labelled(id, RETURN_KIND, format!("return {id}"))
                    .previewed(Preview::certain(vec![Delta::new(Quantity::SecretObjectivesHeld, before, before - 1)]))
            }
            (ask, opt) => panic!("option {opt:?} in a question {ask:?}"),
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::Score { .. } => PublicObjective::from_id(id)
                .map(ObjectiveRef::Public)
                .or_else(|| SecretObjective::from_id(id).map(ObjectiveRef::Secret))
                .map(Opt::Score),
            Ask::ReturnSecret { .. } => SecretObjective::from_id(id).map(Opt::Return),
        }
    }
}
