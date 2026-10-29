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
use ti4_engine_core::kernel::fx_plots::{PRINTED, card_text};
use ti4_engine_core::state::ObjectiveRef;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;
use crate::preview::{Delta, Preview, Quantity};

/// `objectives::SCORE_KIND`.
pub const SCORE_KIND: &str = "score";
/// `secrets::RETURN_KIND`.
pub const RETURN_KIND: &str = "return";
/// `firmament::PLOT_OPTION`: the id prefix of a scoring option that scores a secret as a plot.
pub const PLOT_OPTION: &str = "plot|";
/// The option kind of a plot card placement.
pub const PLOT_CARD_KIND: &str = "plot_card";
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

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::PlotCard { token } => format!("place a plot card with {}'s control token on it", r.seat_text(*token)),
            Ask::Score { .. } => "score an objective".to_owned(),
            Ask::ReturnSecret { .. } => "return a secret objective to the deck".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::ScoreObjective => "score_objective",
            Subtype::ScoreSecretObjective => "score_secret_objective",
            Subtype::ReturnOverSecretHandLimit => "return_over_secret_hand_limit",
            Subtype::PlotCard => "plot_card",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match (a, o) {
            (Ask::Score { vp }, Opt::Plot { secret, token }) => {
                // Plots Within Plots: a plot scores with no victory point.
                let vp = i64::from(*vp);
                let token = r.seat_text(*token);
                ChoiceOption::labelled(format!("{PLOT_OPTION}{}|{token}", secret.as_str()), SCORE_KIND, format!("{} as a plot with {token}'s control token", secret.as_str()))
                    .previewed(Preview::certain(vec![Delta::new(Quantity::VictoryPoints, vp, vp)]))
            }
            (Ask::PlotCard { token }, Opt::PlaceCard(card)) => {
                let text = card_text(*card);
                let mut name = text.to_owned();
                name[..1].make_ascii_uppercase();
                ChoiceOption::labelled(text, PLOT_CARD_KIND, format!("place {name} with {}'s control token", r.seat_text(*token)))
            }
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

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::PlotCard { .. } => PRINTED.iter().find(|c| card_text(**c) == id).map(|c| Opt::PlaceCard(*c)),
            Ask::Score { .. } if id.starts_with(PLOT_OPTION) => {
                let (secret, token) = id[PLOT_OPTION.len()..].split_once('|')?;
                Some(Opt::Plot { secret: SecretObjective::from_id(secret)?, token: r.seat(token)? })
            }
            Ask::Score { .. } => PublicObjective::from_id(id)
                .map(ObjectiveRef::Public)
                .or_else(|| SecretObjective::from_id(id).map(ObjectiveRef::Secret))
                .map(Opt::Score),
            Ask::ReturnSecret { .. } => SecretObjective::from_id(id).map(Opt::Return),
        }
    }
}
