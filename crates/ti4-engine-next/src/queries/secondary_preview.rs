//! The types of `secondary_preview.rs` of the legacy crate (the preview of a follower's strategy-card secondary), with the thin `Choice`.
//! [`preview_secondary`] runs the core's strategic arm on a throwaway native `Game` (D2-8f): `Game::prepare_hypothetical_secondary` opens a
//! one-follower window on a copy of the position and the steps are driven by a scripted table, as the host's planning drafts are.
//! `tests/secondary_preview.rs` compares the answers with the legacy string flow. `MAX_ANSWERS` is in `kinds`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ti4_content::StrategyCardKind;

use super::strategy::SecondaryBlocker;
use crate::wire::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};

pub use crate::kinds::secondary_preview::MAX_ANSWERS;

/// What the follower would be asked, as of the position given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[allow(
    clippy::large_enum_variant,
    reason = "one short-lived value per request; boxing the choice would only complicate every reader"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum SecondaryPreview {
    /// The next question: the window question when `step` is 0, a follow-up after that.
    Question {
        choice: Choice,
        step: usize,
        /// Technology: how the engine would pay the 4 resources once a technology is chosen.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payment: Option<PaymentPreview>,
        /// Faction prompts the real flow would put first that the preview declined to get here.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<SkippedPrompt>,
    },
    /// The window would not open for this seat at all.
    WouldNotBeAsked { blocker: SecondaryBlocker },
    /// The answers given end the secondary: nothing further would be asked.
    Complete {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped: Vec<SkippedPrompt>,
        /// Scripted answers after the first that the flow never asked for (it ended earlier than
        /// the script assumed: a purchase of more tokens than the seat can afford). Payment
        /// answers for a payment the engine settled by itself (a lone option) do not count.
        #[serde(default, skip_serializing_if = "is_zero")]
        unused_answers: usize,
    },
    /// A scripted answer is not offered by the question the engine asks at that point: the
    /// planned answers do not fit the game as it is now. `choice` is that question.
    Rejected {
        /// Index into the given `answers` of the answer that is not offered.
        at: usize,
        answer: String,
        choice: Box<Choice>,
    },
    /// The preview cannot say (an unexpected question, an answer that is not offered, ...).
    Unavailable { detail: String },
}

const fn is_zero(count: &usize) -> bool {
    *count == 0
}

/// How the engine would pay a Technology secondary's resources: the first plan of
/// [`crate::payment::plans`], which is the one `paid_research` applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PaymentPreview {
    /// Resources owed.
    pub cost: i64,
    /// Planets that would be exhausted.
    pub planets: Vec<String>,
    /// Trade goods that would be spent.
    pub trade_goods: i32,
    /// What the plan is worth (at least `cost`).
    pub worth: i64,
}

/// A question outside the card's own that the real flow would ask first, answered "decline" here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SkippedPrompt {
    pub subtype: String,
    pub prompt: String,
}

/// Which decision subtypes are the card's own follow-up questions (the ones a client prepares).
/// Empty for the cards whose follow-up is not previewed.
#[must_use]
pub fn follow_up_subtypes(card: StrategyCardKind) -> &'static [&'static str] {
    match card {
        StrategyCardKind::Technology => &["research_technology"],
        StrategyCardKind::Diplomacy => &["ready_planet"],
        StrategyCardKind::Construction => &["place_structure"],
        // Production: the build list, then (after a scripted build) the payment or placement
        // question the engine asks next, if it asks one at all.
        StrategyCardKind::Warfare => &["produce_unit", "pay_resources", "place_unit"],
        // Leadership: the payment of each token is the player's choice, one planet or trade good
        // per question (`pay_influence`; the engine has no automatic rule for it), then the
        // token's pool, then the "again?" question. A client scripts the whole purchase to have
        // the engine check it. Trade, Politics and Imperial resolve without a question (or draw
        // cards, which a preview must not run).
        StrategyCardKind::Leadership => &[
            "pay_influence",
            "gain_command_token",
            "buy_token_with_influence",
        ],
        StrategyCardKind::Politics | StrategyCardKind::Trade | StrategyCardKind::Imperial => &[],
    }
}

const MAX_SKIPPED_PROMPTS: usize = 8;
/// Steps the preview drives at most (one for the window, one for the follow; a few more for a window that closes by itself).
const STEP_BOUND: usize = 12;

#[derive(Default)]
struct Captured {
    /// An outcome fixed by the window question itself.
    done: Option<SecondaryPreview>,
    window_seen: bool,
    question: Option<Choice>,
    rejected: Option<(usize, String, Choice)>,
    consumed: usize,
    skipped: Vec<SkippedPrompt>,
    failure: Option<String>,
}

struct Previewer {
    answers: Vec<String>,
    scripted: VecDeque<String>,
    own: &'static [&'static str],
    kind: StrategyCardKind,
    out: Rc<RefCell<Captured>>,
}

fn is_payment_answer(id: &str) -> bool {
    id.starts_with("exhaust|") || id == "trade_good" || id == "commodity"
}

fn stop(choice: &Choice, reason: &str) -> IllegalChoice {
    IllegalChoice::DeciderFailed { player: choice.player.clone(), prompt: choice.prompt.clone(), reason: reason.to_owned() }
}

impl Previewer {
    /// The window question of the follower: the first thing the one-follower window asks (`StrategySecondaryWindow::pending_choice`).
    fn window(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let mut out = self.out.borrow_mut();
        out.window_seen = true;
        let unavailable = |detail: &str| SecondaryPreview::Unavailable { detail: detail.to_owned() };
        let Some((first, rest)) = self.answers.split_first() else {
            out.done = Some(SecondaryPreview::Question { choice: choice.clone(), step: 0, payment: None, skipped: Vec::new() });
            return Err(stop(choice, "preview boundary"));
        };
        let Some(answer) = choice.options.iter().find(|option| &option.id == first) else {
            out.done = Some(unavailable("that answer is not offered by the window question"));
            return Err(stop(choice, "preview boundary"));
        };
        if answer.is_decline() || answer.id == "no" {
            out.done = Some(SecondaryPreview::Complete { skipped: Vec::new(), unused_answers: rest.len() });
            return Err(stop(choice, "preview boundary"));
        }
        if self.own.is_empty() {
            out.done = Some(if rest.is_empty() { SecondaryPreview::Complete { skipped: Vec::new(), unused_answers: 0 } } else { unavailable("this card has no follow-up question to preview") });
            return Err(stop(choice, "preview boundary"));
        }
        let _ = self.kind;
        Ok(answer.clone())
    }
}

impl Decider for Previewer {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        if !self.out.borrow().window_seen {
            if choice.details.get("kind").and_then(serde_json::Value::as_str) == Some("strategy_secondary") {
                return self.window(choice);
            }
            self.out.borrow_mut().failure = Some(format!("unexpected question: {}", choice.prompt));
            return Err(stop(choice, "unexpected question"));
        }
        let subtype = choice.context.as_ref().map_or("", |context| context.subtype.as_str());
        let mut out = self.out.borrow_mut();
        if self.own.contains(&subtype) {
            // The engine takes a lone payment option without asking (the real batch skips those steps the same way): payment answers are
            // dropped once the flow moves past payments.
            if !subtype.starts_with("pay_") {
                while self.scripted.front().is_some_and(|id| is_payment_answer(id)) {
                    self.scripted.pop_front();
                    out.consumed += 1;
                }
            }
            if let Some(wanted) = self.scripted.pop_front() {
                let at = 1 + out.consumed;
                out.consumed += 1;
                return match choice.options.iter().find(|option| option.id == wanted) {
                    Some(option) => Ok(option.clone()),
                    None => {
                        out.rejected = Some((at, wanted, choice.clone()));
                        Err(stop(choice, "scripted answer not offered"))
                    }
                };
            }
            out.question = Some(choice.clone());
            return Err(stop(choice, "preview boundary"));
        }
        if out.skipped.len() < MAX_SKIPPED_PROMPTS
            && let Some(decline) = choice.options.iter().find(|option| option.is_decline())
        {
            out.skipped.push(SkippedPrompt { subtype: subtype.to_owned(), prompt: choice.prompt.clone() });
            return Ok(decline.clone());
        }
        out.failure = Some(format!("unexpected question {subtype:?}: {}", choice.prompt));
        Err(stop(choice, "unexpected question"))
    }
}

/// What `follower` would be asked for `card` (played by `primary`) if the window opened in `state`, after the `answers` given so far.
///
/// Read only by construction: the position is copied into a throwaway native `Game` (a disposable fork with a one-follower secondary window,
/// `Game::prepare_hypothetical_secondary`); nothing is written to the caller's. Cards whose follow-up is not previewed (Politics, Imperial,
/// Trade) never run past the window question. What the preview cannot know is what happens between now and the moment the window really
/// opens: the primary's own effects, followers asked earlier in clockwise order and action cards.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "the position plus the question's identity")]
pub fn preview_secondary(
    state: &ti4_model::state::GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    card: &ti4_model::id::StrategyCardId,
    primary: &ti4_model::id::PlayerId,
    follower: &ti4_model::id::PlayerId,
    answers: &[String],
) -> SecondaryPreview {
    let run = std::panic::AssertUnwindSafe(|| preview_inner(state, content, sources, galaxy, card, primary, follower, answers));
    std::panic::catch_unwind(run).unwrap_or_else(|_| SecondaryPreview::Unavailable { detail: "the preview failed internally".to_owned() })
}

#[allow(clippy::too_many_arguments, reason = "see preview_secondary")]
fn preview_inner(
    state: &ti4_model::state::GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    card: &ti4_model::id::StrategyCardId,
    primary: &ti4_model::id::PlayerId,
    follower: &ti4_model::id::PlayerId,
    answers: &[String],
) -> SecondaryPreview {
    let unavailable = |detail: &str| SecondaryPreview::Unavailable { detail: detail.to_owned() };
    if answers.len() > MAX_ANSWERS {
        return unavailable("too many answers");
    }
    if primary == follower || state.player(follower).is_none() || state.player(primary).is_none() {
        return unavailable("not a follower of this action");
    }
    let Some(kind) = super::strategy_cards::card_kind(content, card.as_str()) else {
        return unavailable("unknown strategy card");
    };
    let own = follow_up_subtypes(kind);
    let out = Rc::new(RefCell::new(Captured::default()));
    let previewer = Previewer { answers: answers.to_vec(), scripted: answers.iter().skip(1).cloned().collect(), own, kind, out: out.clone() };
    let mut game = crate::game::Game::with_table(state.clone(), content, Table::with_default(Box::new(previewer))).with_sources(sources);
    if let Some(galaxy) = galaxy {
        game = game.with_galaxy(galaxy.clone());
    }
    if let Err(error) = game.try_load() {
        return SecondaryPreview::Unavailable { detail: format!("the flow stopped: {error}") };
    }
    if let Err(error) = game.prepare_hypothetical_secondary(follower, primary, card) {
        return SecondaryPreview::Unavailable { detail: format!("the flow stopped: {error}") };
    }
    let mut completed = false;
    let mut last_error = None;
    let mut seen_events = game.events.len();
    for _ in 0..STEP_BOUND {
        let result = game.step();
        completed = game.events[seen_events..].iter().any(|event| event == crate::kinds::game::SECONDARY_PREVIEW_COMPLETE);
        seen_events = game.events.len();
        if let Some(error) = result.error {
            last_error = Some(error);
            break;
        }
        if completed || result.finished {
            break;
        }
    }
    let captured = std::mem::take(&mut *out.borrow_mut());
    if let Some(done) = captured.done {
        return done;
    }
    if let Some(choice) = captured.question {
        let payment = payment_preview(state, sources, galaxy, follower, &choice);
        return SecondaryPreview::Question { choice, step: answers.len(), payment, skipped: captured.skipped };
    }
    if let Some((at, answer, choice)) = captured.rejected {
        return SecondaryPreview::Rejected { at, answer, choice: Box::new(choice) };
    }
    if let Some(detail) = captured.failure {
        return SecondaryPreview::Unavailable { detail };
    }
    if let Some(error) = last_error {
        return SecondaryPreview::Unavailable { detail: format!("the flow stopped: {error}") };
    }
    if !completed {
        return unavailable("the preview did not settle");
    }
    if !captured.window_seen {
        return match blocker(state, galaxy, sources, card, primary, follower) {
            Some(blocker) => SecondaryPreview::WouldNotBeAsked { blocker },
            None => unavailable("the window question could not be built"),
        };
    }
    SecondaryPreview::Complete { skipped: captured.skipped, unused_answers: answers.len().saturating_sub(1).saturating_sub(captured.consumed) }
}

/// Why the follower is not asked (`StrategySecondaryWindow::blocker`): `None` when the window would ask, or the position cannot be held.
fn blocker(
    state: &ti4_model::state::GameState,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    sources: ti4_model::content_types::SourceSet,
    card: &ti4_model::id::StrategyCardId,
    primary: &ti4_model::id::PlayerId,
    follower: &ti4_model::id::PlayerId,
) -> Option<SecondaryBlocker> {
    use ti4_engine_core::driver::kernel_env::KernelEnv;
    use ti4_engine_core::driver::windows::strategic::facts::follower_facts;
    use ti4_engine_core::rules::strategy::{self, Cx, Env};
    use ti4_engine_core::state::SliceSet;

    let view = super::view::Loaded::of(state, galaxy, sources, SliceSet::ALL)?;
    let (core, kernel) = (view.core(), view.kernel());
    let card = <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id(card.as_str())?;
    let (primary, who) = (view.seat(primary)?, view.seat(follower)?);
    if follower_facts(core, kernel, primary, card, who).eligible {
        return None;
    }
    let kind = card.data().kind;
    if kind == ti4_engine_core::generated::StrategyCardKind::Leadership {
        return Some(SecondaryBlocker::CannotAffordInfluence);
    }
    let env = KernelEnv::new(core, kernel);
    let cx = Cx { core, kernel, env: &env };
    let tokens = core.players().by_seat(who).is_some_and(|p| p.econ.strategic_tokens > 0);
    if strategy::secondary_costs_token(Some(kind)) && !strategy::secondary_is_free(&cx, who, Some(kind)) && !tokens {
        return Some(SecondaryBlocker::NoStrategyToken);
    }
    use ti4_engine_core::generated::StrategyCardKind as K;
    Some(match kind {
        K::Technology if !env.can_research(who) => SecondaryBlocker::NothingToResearch,
        K::Technology => SecondaryBlocker::CannotPayResources,
        K::Diplomacy => SecondaryBlocker::NoExhaustedPlanet,
        K::Trade => SecondaryBlocker::CommoditiesFull,
        K::Warfare => SecondaryBlocker::NoProduction,
        _ => SecondaryBlocker::Other,
    })
}

/// Technology: the plan `paid_research` would apply for this question's cost (the first plan `payment::plans` lists).
fn payment_preview(
    state: &ti4_model::state::GameState,
    sources: ti4_model::content_types::SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    follower: &ti4_model::id::PlayerId,
    choice: &Choice,
) -> Option<PaymentPreview> {
    let context = choice.context.as_ref()?;
    if context.subtype != "research_technology" {
        return None;
    }
    let cost = choice.options.iter().filter(|option| !option.is_decline()).find_map(|option| option.payload.get("cost").and_then(serde_json::Value::as_i64))?;
    if cost <= 0 {
        return None;
    }
    let view = super::objectives::ObjView::build(state, sources, galaxy)?;
    let plan = view.first_resource_plan(follower, cost)?;
    Some(PaymentPreview { cost, worth: plan.worth, planets: plan.planets, trade_goods: plan.trade_goods })
}
